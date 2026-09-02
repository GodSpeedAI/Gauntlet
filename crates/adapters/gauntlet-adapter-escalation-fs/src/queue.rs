//! The durable queue itself. One ticket is one JSON file:
//! `<root>/<ticket-id>.json`, written create-temp → sync → rename so a
//! crash never leaves a half ticket. The schema token and field order
//! are fixed; unknown fields are refused on read, so a hand-forged or
//! foreign document cannot enter the operator's queue silently.
//!
//! COR-R2-2 (falsifier round 2, F-R2-1): the record's own `ticket` field
//! is bound to the id the file is read FOR — a decodable-but-lying file
//! (one whose `ticket` field names a DIFFERENT ticket) is refused on the
//! read path, quarantined out of the listing, and can never steer an
//! operator action's write-back onto another ticket: the write-back
//! targets the REQUESTED id, never the decoded record's say-so.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gauntlet_domain::{EpochSeconds, EvidenceRef, RunId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::human_escalation::{
    EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
    TicketId,
};
use gauntlet_ports::json::{JsonNumber, JsonValue};

/// The queue's directory name under the state dir (spec §6.2's
/// `escalations/`).
pub const ESCALATIONS_DIR_NAME: &str = "escalations";

/// The ticket record schema token (fixed; reads refuse any other).
pub const TICKET_SCHEMA: &str = "gauntlet-escalation-ticket@v1";

/// The ticket id prefix. Ids are `esc-<unix-millis-hex>-<counter-hex>`:
/// unique within the process by the counter, unique across restarts by
/// the clock, and refused-safe (a collision re-mints rather than
/// overwrites).
pub const TICKET_ID_PREFIX: &str = "esc-";

/// The in-process counter that keeps same-millisecond escalations
/// distinct.
static TICKET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The suffix of the in-flight temp file (single segment; renamed away
/// atomically).
const TEMP_SUFFIX: &str = ".tmp";

/// Why a queue operation failed — the read/resolve surface's typed
/// error (the port's own error covers `escalate`/`poll`).
#[derive(Debug, Clone, PartialEq)]
pub enum EscalationQueueError {
    /// The queue root could not be ensured.
    RootFailed { detail: String },
    /// A ticket file was unreadable or did not decode as its schema.
    RecordInvalid { ticket: String, detail: String },
    /// The queue directory could not be read.
    ListFailed { detail: String },
    /// The ticket was already resolved — a queue record is resolved
    /// once; a second action is refused, never overwritten.
    AlreadyResolved { ticket: String },
    /// The record decoded clean but its own `ticket` field names a
    /// DIFFERENT ticket than the one addressed (COR-R2-2's cross-wiring
    /// defense: the file's id and the record's id must agree, or the
    /// record is not this ticket's record).
    TicketIdentityMismatch { requested: String, claimed: String },
    /// The write of the resolved record failed.
    WriteFailed { detail: String },
}

impl core::fmt::Display for EscalationQueueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RootFailed { detail } => {
                write!(f, "ensuring the escalations queue root failed: {detail}")
            }
            Self::RecordInvalid { ticket, detail } => {
                write!(
                    f,
                    "escalation ticket {ticket} is not a valid record: {detail}"
                )
            }
            Self::ListFailed { detail } => {
                write!(f, "reading the escalations queue failed: {detail}")
            }
            Self::AlreadyResolved { ticket } => write!(
                f,
                "escalation ticket {ticket} is already resolved — a queue record resolves \
                 exactly once"
            ),
            Self::TicketIdentityMismatch { requested, claimed } => write!(
                f,
                "escalation ticket {requested} is not this record: the record's own ticket \
                 field names {claimed} — a document whose id disagrees with the ticket it was \
                 addressed to is refused (cross-wiring defense)"
            ),
            Self::WriteFailed { detail } => {
                write!(f, "writing the escalation record failed: {detail}")
            }
        }
    }
}

impl std::error::Error for EscalationQueueError {}

/// The operator action recorded on a resolved ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorAction {
    /// The operator resolved the escalation (accepted the block).
    Resolve,
    /// The operator instructed a retry of the escalated work.
    Retry,
}

impl OperatorAction {
    pub fn token(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Retry => "retry",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "resolve" => Some(Self::Resolve),
            "retry" => Some(Self::Retry),
            _ => None,
        }
    }
}

/// One durable escalation record (the file's decoded shape).
#[derive(Debug, Clone, PartialEq)]
pub struct EscalationRecord {
    pub ticket: TicketId,
    pub run_id: RunId,
    pub summary: String,
    pub context: EvidenceRef,
    pub created_at: EpochSeconds,
    pub state: EscalationState,
    /// The recorded operator action, when resolved.
    pub action: Option<OperatorAction>,
}

/// Why a ticket file was quarantined — the COARSE class only (COR-F4):
/// the entry names the failure class, never the file's bytes or fields,
/// so a tamper attempt learns nothing from the operator's own list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineClass {
    /// The ticket file could not be read.
    Unreadable,
    /// The file's bytes did not decode as `gauntlet-escalation-ticket@v1`.
    RecordInvalid,
}

impl QuarantineClass {
    pub fn token(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::RecordInvalid => "record-invalid",
        }
    }
}

/// A corrupt ticket file, quarantined out of the operator's list (COR-F4):
/// the id the file claims (its stem) and the coarse error class. Actions
/// on a quarantined ticket fail closed at the queue's read path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedTicket {
    pub ticket: String,
    pub class: QuarantineClass,
}

/// The queue listing: the decodable records plus every corrupt file
/// quarantined beside them (COR-F4).
#[derive(Debug, Clone, PartialEq)]
pub struct QueueListing {
    pub records: Vec<EscalationRecord>,
    pub quarantined: Vec<QuarantinedTicket>,
}

impl EscalationRecord {
    /// Whether an operator has acted on this ticket.
    pub fn is_open(&self) -> bool {
        !self.state.is_resolved()
    }
}

/// The filesystem escalation queue rooted at a directory (the
/// composition root passes `<state_dir>/escalations`).
pub struct FsEscalationQueue {
    root: PathBuf,
}

impl FsEscalationQueue {
    /// Opens (and ensures) the queue root.
    pub fn open(root: &Path) -> Result<Self, EscalationQueueError> {
        std::fs::create_dir_all(root).map_err(|error| EscalationQueueError::RootFailed {
            detail: format!("{}: {error}", root.display()),
        })?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// The queue root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every record in the queue, oldest first (created_at, then ticket
    /// id — a total, stable order for the operator's list), WITH the
    /// corrupt files quarantined. COR-F4 (falsifier round 1, F-4): one
    /// corrupt file no longer blinds the operator's ENTIRE queue — the
    /// valid records list as always and every corrupt file appears as a
    /// typed [`QuarantinedTicket`] (its id and a coarse error CLASS only,
    /// never any file content). The call still fails typed only when the
    /// queue ROOT itself is unreadable.
    pub fn list(&self) -> Result<QueueListing, EscalationQueueError> {
        let mut records = Vec::new();
        let mut quarantined = Vec::new();
        let entries =
            std::fs::read_dir(&self.root).map_err(|error| EscalationQueueError::ListFailed {
                detail: format!("{}: {error}", self.root.display()),
            })?;
        for entry in entries {
            let entry = entry.map_err(|error| EscalationQueueError::ListFailed {
                detail: error.to_string(),
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    quarantined.push(QuarantinedTicket {
                        ticket: stem.to_owned(),
                        class: QuarantineClass::Unreadable,
                    });
                    continue;
                }
            };
            match decode_record(&text) {
                // COR-R2-2: the record must be the record of the id its
                // FILE names — a decodable record claiming another
                // ticket's id is a forged member, quarantined like any
                // other invalid record (coarse class only, content never
                // echoed).
                Ok(record) if record.ticket.as_str() == stem => records.push(record),
                _ => quarantined.push(QuarantinedTicket {
                    ticket: stem.to_owned(),
                    class: QuarantineClass::RecordInvalid,
                }),
            }
        }
        records.sort_by(|left, right| {
            (left.created_at.get(), left.ticket.as_str())
                .cmp(&(right.created_at.get(), right.ticket.as_str()))
        });
        quarantined.sort_by(|left, right| left.ticket.cmp(&right.ticket));
        Ok(QueueListing {
            records,
            quarantined,
        })
    }

    /// Reads one ticket's record. The resolved path is validated to stay
    /// under the queue root BEFORE the read (COR-F3: containment on the
    /// read path, not just the grammar at the id layer). COR-R2-2: the
    /// decoded record's own `ticket` field must name the REQUESTED id —
    /// a decodable-but-lying file is refused typed, never admitted as
    /// another ticket's record.
    pub fn record(&self, ticket: &TicketId) -> Result<EscalationRecord, EscalationQueueError> {
        let path = self.contained_ticket_path(ticket)?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            EscalationQueueError::RecordInvalid {
                ticket: ticket.as_str().to_owned(),
                detail: error.to_string(),
            }
        })?;
        let record =
            decode_record(&text).map_err(|detail| EscalationQueueError::RecordInvalid {
                ticket: ticket.as_str().to_owned(),
                detail,
            })?;
        if record.ticket != *ticket {
            return Err(EscalationQueueError::TicketIdentityMismatch {
                requested: ticket.as_str().to_owned(),
                claimed: record.ticket.as_str().to_owned(),
            });
        }
        Ok(record)
    }

    /// Records the operator action on an OPEN ticket: the ticket becomes
    /// `Resolved` with the operator's identity and note. This is queue
    /// bookkeeping only — settling the escalated work belongs to the
    /// settlement service's operator path (§20.2, charter D9).
    ///
    /// COR-R2-2: the write-back targets the REQUESTED ticket id — never
    /// the decoded record's own `ticket` field, which (before the read
    /// path's identity binding) could disagree with the file addressed
    /// and steer one ticket's resolution over ANOTHER ticket's record.
    /// The read above already refuses any record whose ticket field
    /// disagrees; binding the write to the requested id keeps the
    /// invariant true by construction, not by trust.
    pub fn resolve(
        &self,
        ticket: &TicketId,
        operator: &str,
        note: &str,
        action: OperatorAction,
    ) -> Result<EscalationRecord, EscalationQueueError> {
        let mut record = self.record(ticket)?;
        if record.state.is_resolved() {
            return Err(EscalationQueueError::AlreadyResolved {
                ticket: ticket.as_str().to_owned(),
            });
        }
        record.state = EscalationState::Resolved {
            operator: operator.to_owned(),
            note: note.to_owned(),
        };
        record.action = Some(action);
        let text = encode_record(&record);
        // COR-F3: the write-back path validates the resolved path's
        // containment against the queue root too. COR-R2-2: the id
        // written through is the CALLER's requested id.
        self.write_atomically(ticket, &text)?;
        Ok(record)
    }

    /// The validated file path for one ticket: the queue-root join is
    /// checked for containment BEFORE any caller reads or writes through
    /// it (COR-F3, defense in depth behind the port's tightened id
    /// grammar): the file name carries no path separator, the join's
    /// parent is the queue root itself, and no traversal component
    /// survives. Refused as a typed [`EscalationQueueError::RecordInvalid`]
    /// naming the invariant — never silently rewritten.
    fn contained_ticket_path(&self, ticket: &TicketId) -> Result<PathBuf, EscalationQueueError> {
        let refused = |ticket: &TicketId| EscalationQueueError::RecordInvalid {
            ticket: ticket.as_str().to_owned(),
            detail: "the ticket id would resolve OUTSIDE the queue root — refused (queue \
                     containment)"
                .to_owned(),
        };
        let file = path_of_ticket(ticket);
        if file.chars().any(std::path::is_separator) {
            return Err(refused(ticket));
        }
        let path = self.root.join(&file);
        if path.parent() != Some(self.root.as_path())
            || path.components().any(|component| {
                !matches!(
                    component,
                    std::path::Component::Normal(_)
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(refused(ticket));
        }
        Ok(path)
    }

    /// Create-temp → sync → rename, with a best-effort parent-directory
    /// fsync (the R17-note-1 durability shape): a reader never observes a
    /// half-written ticket, and the directory entry is durable before the
    /// call returns. Temp name is fixed per ticket — a concurrent writer
    /// to the SAME ticket is serialized by the rename, and the last
    /// complete record wins (ticket files are written once by escalate,
    /// then at most once by resolve). The destination is the ticket's
    /// CONTAINED path (COR-F3): the write validates before it touches the
    /// filesystem.
    fn write_atomically(&self, ticket: &TicketId, text: &str) -> Result<(), EscalationQueueError> {
        use std::io::Write;
        let final_path = self.contained_ticket_path(ticket)?;
        let mut temp_name = final_path.as_os_str().to_owned();
        temp_name.push(TEMP_SUFFIX);
        let temp = PathBuf::from(temp_name);
        let mut file =
            std::fs::File::create(&temp).map_err(|error| EscalationQueueError::WriteFailed {
                detail: format!("{}: {error}", temp.display()),
            })?;
        file.write_all(text.as_bytes())
            .map_err(|error| EscalationQueueError::WriteFailed {
                detail: error.to_string(),
            })?;
        file.sync_all()
            .map_err(|error| EscalationQueueError::WriteFailed {
                detail: error.to_string(),
            })?;
        drop(file);
        std::fs::rename(&temp, &final_path).map_err(|error| EscalationQueueError::WriteFailed {
            detail: format!("{} -> {}: {error}", temp.display(), final_path.display()),
        })?;
        if let Ok(dir) = std::fs::File::open(&self.root) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

impl HumanEscalation for FsEscalationQueue {
    fn escalate(
        &self,
        item: EscalationItem,
        _deadline: Deadline,
    ) -> Result<EscalationTicket, HumanEscalationError> {
        // The same convention as the fs evidence store: the Deadline is
        // noted and not consulted — there is no waiting code here to
        // bound.
        let created_at = wall_now();
        let mut attempt = 0;
        loop {
            let counter = TICKET_COUNTER.fetch_add(1, Ordering::Relaxed);
            let ticket = TicketId::new(&format!(
                "{TICKET_ID_PREFIX}{:x}-{:x}",
                created_at.get(),
                counter
            ))
            .map_err(|error| HumanEscalationError::SubmitFailed {
                detail: error.to_string(),
            })?;
            let record = EscalationRecord {
                ticket: ticket.clone(),
                run_id: item.run_id.clone(),
                summary: item.summary.clone(),
                context: item.context.clone(),
                created_at,
                state: EscalationState::Open,
                action: None,
            };
            let text = encode_record(&record);
            if self
                .contained_ticket_path(&ticket)
                .map_err(|error| HumanEscalationError::SubmitFailed {
                    detail: error.to_string(),
                })?
                .exists()
            {
                // A cross-restart collision: re-mint with the next
                // counter value, never overwrite.
                attempt += 1;
                if attempt > 8 {
                    return Err(HumanEscalationError::SubmitFailed {
                        detail: format!("ticket id {ticket} collides with an existing record"),
                    });
                }
                continue;
            }
            self.write_atomically(&ticket, &text).map_err(|error| {
                HumanEscalationError::SubmitFailed {
                    detail: error.to_string(),
                }
            })?;
            return Ok(EscalationTicket::new(ticket, created_at));
        }
    }

    fn poll(
        &self,
        ticket: &EscalationTicket,
        _deadline: Deadline,
    ) -> Result<EscalationState, HumanEscalationError> {
        let path = self.contained_ticket_path(ticket.id()).map_err(|error| {
            HumanEscalationError::StateReadFailed {
                detail: error.to_string(),
            }
        })?;
        if !path.exists() {
            return Err(HumanEscalationError::TicketNotFound {
                ticket: ticket.id().clone(),
            });
        }
        let text = std::fs::read_to_string(&path).map_err(|error| {
            HumanEscalationError::StateReadFailed {
                detail: error.to_string(),
            }
        })?;
        let record = decode_record(&text)
            .map_err(|detail| HumanEscalationError::StateReadFailed { detail })?;
        // COR-R2-2: a run polls ITS OWN ticket — a decodable file whose
        // record names a different ticket is not this ticket's state and
        // must never be reported as it (the forged-`Resolved` shape that
        // would let a blocked run proceed on an operator action aimed
        // elsewhere).
        if record.ticket != *ticket.id() {
            return Err(HumanEscalationError::StateReadFailed {
                detail: format!(
                    "escalation ticket {} is not this record: the record's own ticket field \
                     names {} (cross-wiring defense)",
                    ticket.id().as_str(),
                    record.ticket.as_str()
                ),
            });
        }
        Ok(record.state)
    }
}

fn path_of_ticket(ticket: &TicketId) -> String {
    format!("{}.json", ticket.as_str())
}

/// The wall clock, read by the adapter (the same posture as the fs
/// evidence store's timestamps).
fn wall_now() -> EpochSeconds {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    EpochSeconds::new(secs)
}

/// The record's canonical JSON: fixed field order, so the same facts
/// encode byte-identically.
fn encode_record(record: &EscalationRecord) -> String {
    let (state, operator, note) = match &record.state {
        EscalationState::Open => ("open", None, None),
        EscalationState::Resolved { operator, note } => {
            ("resolved", Some(operator.as_str()), Some(note.as_str()))
        }
    };
    let action = record.action.map(OperatorAction::token);
    let field = |key: &str, value: &str| (key.to_owned(), JsonValue::Str(value.to_owned()));
    let mut entries: Vec<(String, JsonValue)> = vec![
        field("schema", TICKET_SCHEMA),
        field("ticket", record.ticket.as_str()),
        field("run_id", record.run_id.as_str()),
        field("summary", &record.summary),
        field("context", record.context.as_str()),
        (
            "created_at".to_owned(),
            JsonValue::Number(JsonNumber::Uint(record.created_at.get())),
        ),
        field("state", state),
    ];
    if let Some(operator) = operator {
        entries.push(field("operator", operator));
    }
    if let Some(note) = note {
        entries.push(field("note", note));
    }
    if let Some(action) = action {
        entries.push(field("action", action));
    }
    JsonValue::Object(entries).to_json_text()
}

/// Decodes one record, refusing unknown fields (drift is not data).
fn decode_record(text: &str) -> Result<EscalationRecord, String> {
    let parsed = gauntlet_ports::json::parse(text).map_err(|error| error.to_string())?;
    let JsonValue::Object(entries) = &parsed else {
        return Err("the record is not a JSON object".to_owned());
    };
    let required = [
        "schema",
        "ticket",
        "run_id",
        "summary",
        "context",
        "created_at",
        "state",
    ];
    for key in entries.iter().map(|(key, _)| key.as_str()) {
        if !required.contains(&key) && key != "operator" && key != "note" && key != "action" {
            return Err(format!("unknown record field {key:?}"));
        }
    }
    let string_field = |key: &str| -> Result<String, String> {
        match parsed.get(key) {
            Some(JsonValue::Str(text)) => Ok(text.clone()),
            other => Err(format!("field {key:?} is not a string: {other:?}")),
        }
    };
    if string_field("schema")? != TICKET_SCHEMA {
        return Err(format!(
            "schema {} is not {}",
            string_field("schema")?,
            TICKET_SCHEMA
        ));
    }
    let ticket = TicketId::new(&string_field("ticket")?).map_err(|error| error.to_string())?;
    let run_id = RunId::new(&string_field("run_id")?).map_err(|error| error.to_string())?;
    let context = EvidenceRef::new(&string_field("context")?).map_err(|error| error.to_string())?;
    let created_at = match parsed.get("created_at") {
        Some(JsonValue::Number(number)) => number
            .as_u64()
            .ok_or_else(|| "created_at is not a u64".to_owned())?,
        _ => return Err("field \"created_at\" is not a number".to_owned()),
    };
    let state_token = string_field("state")?;
    if state_token == "open" {
        // An open record carries no operator fields — a document that
        // claims both is malformed, not half-resolved.
        for key in ["operator", "note", "action"] {
            if parsed.get(key).is_some() {
                return Err(format!("an open record carries {key:?}"));
            }
        }
    }
    let state = match state_token.as_str() {
        "open" => EscalationState::Open,
        "resolved" => EscalationState::Resolved {
            operator: string_field("operator")?,
            note: string_field("note")?,
        },
        other => return Err(format!("unknown state token {other:?}")),
    };
    let action = match entries.iter().find(|(key, _)| key == "action") {
        Some((_, JsonValue::Str(token))) => Some(
            OperatorAction::parse(token)
                .ok_or_else(|| format!("unknown action token {token:?}"))?,
        ),
        Some((key, other)) => return Err(format!("field {key:?} is not a token: {other:?}")),
        None => None,
    };
    Ok(EscalationRecord {
        ticket,
        run_id,
        summary: string_field("summary")?,
        context,
        created_at: EpochSeconds::new(created_at),
        state,
        action,
    })
}
