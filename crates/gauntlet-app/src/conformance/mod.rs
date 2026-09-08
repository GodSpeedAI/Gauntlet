//! The conformance sweep's application services (task 27, B1; plan
//! §Task 27 step 1): adjudicate EVERY frozen-ruler unit against an
//! authored manifest over a closed command vocabulary, and assemble the
//! frozen `gauntlet.conformance@v1` report.
//!
//! # What this module is and is not (charter decisions 1-5)
//!
//! The sweep is a READ-ONLY instrument surface: it never settles, never
//! writes runtime state, never dispatches agents. The outputs are the
//! report document and the evidence log directory — nothing else. The
//! three outcomes stay three outcomes: `PASS`, `FAIL`, `INCONCLUSIVE`
//! never collapse, INCONCLUSIVE is never PASS, and missing evidence is
//! never passing evidence.
//!
//! # Fail-closed everywhere
//!
//! The manifest is authored input but untrusted input (spec §21.2): the
//! strict codec ([`manifest`]) parses a documented subset, the closed
//! structural vocabulary ([`commands`]) has no command string to
//! inject, the named predicates ([`predicates`]) are code, not
//! strings — and cross-validation against the parsed frozen ruler
//! refuses on any id-set, kind, or frozen-locator disagreement BEFORE
//! anything runs. A refused sweep writes no report.
//!
//! # The seam
//!
//! [`ConformanceExecutor`] is the execution seam (the task-23/26
//! precedent): this crate never spawns a process. Unit tests run a
//! deterministic fake over a fixture world (bare absolute executables
//! such as the true/false/echo binaries); the composition root
//! implements the real executor over the adapter layer under the
//! repository's compile-lock discipline.
//!
//! # Honesty rules encoded here
//!
//! - `evidence: none` adjudicates INCONCLUSIVE with the recorded
//!   reason — never PASS (charter decision 3);
//! - an absent artifact is INCONCLUSIVE (`no_artifact`), not FAIL; a
//!   predicate that REFUTES is FAIL;
//! - an executor that cannot run a command is INCONCLUSIVE with that
//!   reason — never silently PASS, never FAIL-by-default;
//! - identical commands are grouped and run ONCE, their result fanning
//!   out to every unit that cites them;
//! - PASS is structurally impossible without a non-empty, repo-relative,
//!   on-disk `evidence_ref` (the assembly refuses otherwise).

pub mod commands;
pub mod manifest;
pub mod predicates;
pub mod report;

use std::path::Path;

use gauntlet_ports::json::JsonValue;
use sha2::{Digest, Sha256};

pub use commands::{
    NextestSelector, SweepCommand, WORKSPACE_FEATURES, WORKSPACE_PACKAGES, command_display,
    command_slug,
};
pub use manifest::{
    EvidenceArm, MANIFEST_SCHEMA_TOKEN, ManifestEntry, ManifestError, StatusHint, parse_manifest,
    parse_manifest_with,
};
pub use report::{
    AssemblyRefusal, REPORT_SCHEMA_TOKEN, ReportHeader, UnitDisposition, assemble_report,
};

/// The evidence directory leaf name (single segment; the stamped run
/// directory lives under it beside the report).
pub const EVIDENCE_DIR_NAME: &str = "conformance-evidence";

/// The evidence manifest file name (the per-run record of command,
/// exit, duration, and digest).
pub const EVIDENCE_MANIFEST_NAME: &str = "MANIFEST";

/// One executed command's observed facts, as the executor reports them.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    /// The invocation the executor ran (its own concrete spelling —
    /// the adapter layer composes it; this layer's structural display
    /// is [`command_display`]).
    pub display: String,
    /// The command's exit status.
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration_ms: u64,
    /// Provenance for a code the executor's layer translated (COR-4:
    /// 169/186 are the wrapper's remaps of an inner 69/86; a direct
    /// inner exit of those codes is indistinguishable — disclosed).
    /// Empty when the exit needs no explanation; a FAIL reason carries
    /// it verbatim.
    pub exit_note: String,
}

/// Why the executor could not run a command. Environmental, typed — the
/// adjudicator records it as INCONCLUSIVE with this reason, never a
/// PASS and never a FAIL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteError {
    CannotRun { reason: String },
}

/// The execution seam (the task-23/26 precedent): everything that runs a
/// command lives behind this trait; this crate never spawns a process.
pub trait ConformanceExecutor {
    fn execute(&mut self, command: &SweepCommand) -> Result<CommandOutcome, ExecuteError>;
}

/// Everything the sweep reads, plus the caller-resolved facts it cannot
/// invent (time, the tree's head commit).
pub struct SweepInputs<'a> {
    /// The frozen answer key's bytes (the caller read them; the ruler is
    /// never written).
    pub ruler_text: &'a str,
    /// The ruler's repo-relative display path (recorded in the report).
    pub answer_key_path: String,
    /// The manifest's bytes.
    pub manifest_text: &'a str,
    /// The manifest's repo-relative display path.
    pub manifest_path: String,
    /// The root artifact paths resolve under (the workspace/project
    /// root at the CLI; a temp fixture world in tests).
    pub workspace_root: &'a Path,
    /// The package allowlist the execute arms validate against (the
    /// application default is [`WORKSPACE_PACKAGES`]; the composition
    /// root extends it with the adapters it wires — adapter identifiers
    /// never appear in application source, AK-002's own denylist).
    pub allowed_packages: &'a [&'a str],
    /// The feature-edge allowlist the execute arms validate against
    /// (COR-2: a feature-gated target under the default config compiles
    /// zero tests, so the config is part of the command; the application
    /// default is [`WORKSPACE_FEATURES`]).
    pub allowed_features: &'a [&'a str],
    /// RFC3339, caller-resolved (the app layer has no wall clock).
    pub generated_at: String,
    /// The evidence directory's timestamp slug (e.g. `20260907T101500Z`).
    pub evidence_stamp: String,
    /// The tree's head commit at sweep time (caller-resolved).
    pub tree_commit: String,
}

/// The completed sweep: the report document plus the evidence files the
/// caller must write beside it (relative paths, rooted at the report's
/// parent directory).
#[derive(Debug, Clone)]
pub struct SweepOutcome {
    pub report: JsonValue,
    pub evidence_files: Vec<(String, Vec<u8>)>,
    pub pass: usize,
    pub inconclusive: usize,
    pub fail: usize,
    pub commands_executed: usize,
}

/// Why a sweep refused before producing anything. Typed; each variant
/// carries its locus. No report is written on refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepRefusal {
    /// The frozen ruler itself did not parse (the sweep stops; the ruler
    /// is never repaired here).
    RulerParse { detail: String },
    /// The manifest violated the strict codec or the closed vocabulary.
    Manifest(ManifestError),
    /// Manifest ids the ruler does not carry.
    UnknownManifestIds { ids: Vec<String> },
    /// Ruler units the manifest does not cover — set equality is checked
    /// BOTH directions.
    MissingManifestIds { ids: Vec<String> },
    /// An entry's kind disagrees with the ruler's `evidence_required`
    /// kind.
    KindMismatch {
        id: String,
        manifest_kind: String,
        ruler_kind: String,
    },
    /// An entry's frozen locator disagrees with the ruler's verbatim
    /// locator (manifest drift, refused).
    FrozenLocatorMismatch {
        id: String,
        manifest_locator: String,
        ruler_locator: String,
    },
    /// Report assembly refused (the PASS-without-evidence invariant, or
    /// an invalid disposition shape).
    Assembly(AssemblyRefusal),
}

impl core::fmt::Display for SweepRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RulerParse { detail } => {
                write!(f, "the frozen answer key did not parse: {detail}")
            }
            Self::Manifest(cause) => write!(f, "the conformance manifest is invalid: {cause}"),
            Self::UnknownManifestIds { ids } => write!(
                f,
                "the manifest names units the ruler does not carry: {}",
                ids.join(", ")
            ),
            Self::MissingManifestIds { ids } => {
                write!(f, "ruler units the manifest omits: {}", ids.join(", "))
            }
            Self::KindMismatch {
                id,
                manifest_kind,
                ruler_kind,
            } => write!(
                f,
                "{id}: the manifest kind {manifest_kind} does not equal the ruler's \
                 evidence_required kind {ruler_kind}"
            ),
            Self::FrozenLocatorMismatch {
                id,
                manifest_locator,
                ruler_locator,
            } => write!(
                f,
                "{id}: the manifest frozen locator {manifest_locator:?} drifted from the \
                 ruler's {ruler_locator:?}"
            ),
            Self::Assembly(cause) => write!(f, "report assembly refused: {cause}"),
        }
    }
}

impl std::error::Error for SweepRefusal {}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// One command's executed result, fanned out to its units.
struct Execution {
    log_path: String,
    outcome: Result<CommandOutcome, ExecuteError>,
}

/// Runs the sweep: parse, cross-validate, group-and-execute once per
/// distinct command, adjudicate every ruler unit, assemble the frozen
/// report. Err means REFUSED — no report, no evidence.
pub fn run_sweep(
    executor: &mut dyn ConformanceExecutor,
    inputs: &SweepInputs,
) -> Result<SweepOutcome, SweepRefusal> {
    // 1. The frozen ruler parses clean, or the sweep refuses.
    let parsed = crate::reference::parse::parse_answer_key(inputs.ruler_text);
    if let Some(failure) = parsed.failures.first() {
        return Err(SweepRefusal::RulerParse {
            detail: failure.to_string(),
        });
    }
    let ruler = &parsed.units;

    // 2. The manifest parses under the strict codec.
    let entries = parse_manifest_with(
        inputs.manifest_text,
        inputs.allowed_packages,
        inputs.allowed_features,
    )
    .map_err(SweepRefusal::Manifest)?;
    let by_id = |id: &str| entries.iter().find(|entry| entry.id == id);

    // 3. Set equality both directions.
    let unknown: Vec<String> = entries
        .iter()
        .map(|entry| entry.id.clone())
        .filter(|id| !ruler.iter().any(|unit| unit.id().as_str() == id))
        .collect();
    if !unknown.is_empty() {
        return Err(SweepRefusal::UnknownManifestIds { ids: unknown });
    }
    let missing: Vec<String> = ruler
        .iter()
        .map(|unit| unit.id().as_str().to_owned())
        .filter(|id| by_id(id).is_none())
        .collect();
    if !missing.is_empty() {
        return Err(SweepRefusal::MissingManifestIds { ids: missing });
    }

    // 4. Kind and frozen-locator agreement, per unit.
    for unit in ruler {
        let entry = by_id(unit.id().as_str()).expect("set equality was proven above");
        let ruler_kind = unit.evidence_required().kind().wire();
        if entry.kind != ruler_kind {
            return Err(SweepRefusal::KindMismatch {
                id: entry.id.clone(),
                manifest_kind: entry.kind.clone(),
                ruler_kind: ruler_kind.to_owned(),
            });
        }
        let ruler_locator = unit.evidence_required().locator().as_str();
        if entry.frozen_locator != ruler_locator {
            return Err(SweepRefusal::FrozenLocatorMismatch {
                id: entry.id.clone(),
                manifest_locator: entry.frozen_locator.clone(),
                ruler_locator: ruler_locator.to_owned(),
            });
        }
    }

    // 5. Group identical execute commands (ruler order) and run each
    //    ONCE.
    let mut unique: Vec<SweepCommand> = Vec::new();
    for unit in ruler {
        let entry = by_id(unit.id().as_str()).expect("set equality was proven above");
        if let EvidenceArm::Execute(command) = &entry.evidence
            && !unique.contains(command)
        {
            unique.push(command.clone());
        }
    }
    let mut executions: Vec<Execution> = Vec::with_capacity(unique.len());
    let mut evidence_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut manifest_lines: Vec<String> = Vec::new();
    let stamp_dir = [EVIDENCE_DIR_NAME, &inputs.evidence_stamp].join("/");
    for (index, command) in unique.iter().enumerate() {
        let number = index + 1;
        let log_name = format!("{number:02}-{}.log", command_slug(command));
        let log_path = [stamp_dir.as_str(), &log_name].join("/");
        let attempted = executor.execute(command);
        let execution = match &attempted {
            Ok(outcome) => {
                let mut log = Vec::new();
                log.extend_from_slice(format!("command: {}\n", outcome.display).as_bytes());
                log.extend_from_slice(format!("exit: {}\n", outcome.exit_code).as_bytes());
                log.extend_from_slice(b"--- stdout ---\n");
                log.extend_from_slice(&outcome.stdout);
                log.extend_from_slice(b"\n--- stderr ---\n");
                log.extend_from_slice(&outcome.stderr);
                evidence_files.push((log_path.clone(), log.clone()));
                manifest_lines.push(format!(
                    "{log_name} | command={} | exit={} | duration={}ms | sha256={}",
                    outcome.display,
                    outcome.exit_code,
                    outcome.duration_ms,
                    sha256_hex(&log)
                ));
                Execution {
                    log_path,
                    outcome: Ok(CommandOutcome {
                        display: outcome.display.clone(),
                        exit_code: outcome.exit_code,
                        stdout: outcome.stdout.clone(),
                        stderr: outcome.stderr.clone(),
                        duration_ms: outcome.duration_ms,
                        exit_note: outcome.exit_note.clone(),
                    }),
                }
            }
            Err(error) => Execution {
                log_path,
                outcome: Err(error.clone()),
            },
        };
        executions.push(execution);
    }
    let commands_executed = manifest_lines.len();
    if !manifest_lines.is_empty() {
        let mut body =
            String::from("# conformance evidence manifest (one line per executed command)\n");
        for line in &manifest_lines {
            body.push_str(line);
            body.push('\n');
        }
        let manifest_path = [stamp_dir.as_str(), EVIDENCE_MANIFEST_NAME].join("/");
        evidence_files.push((manifest_path, body.into_bytes()));
    }

    // 6. Adjudicate every ruler unit (ruler order).
    let command_index = |command: &SweepCommand| unique.iter().position(|other| other == command);
    let mut dispositions: Vec<report::UnitDisposition> = Vec::with_capacity(ruler.len());
    for unit in ruler {
        let entry = by_id(unit.id().as_str()).expect("set equality was proven above");
        let category_of = |outcome: &str, hint: StatusHint| -> &'static str {
            match outcome {
                "PASS" => {
                    if entry.bounded {
                        "implemented_but_provisional"
                    } else {
                        "implemented_and_proven"
                    }
                }
                "FAIL" => "blocked",
                // INCONCLUSIVE carries the entry's hint; a `none` hint
                // maps to unknown (disclosed): an unforeseen
                // inconclusive state is unknown, never a guess.
                _ => match hint {
                    StatusHint::ConfiguredButUntested => "configured_but_untested",
                    StatusHint::Proposed => "proposed",
                    _ => "unknown",
                },
            }
        };
        let mut row = report::UnitDisposition {
            id: unit.id().as_str().to_owned(),
            outcome: String::new(),
            status_category: String::new(),
            evidence_kind: entry.kind.clone(),
            frozen_locator: entry.frozen_locator.clone(),
            real_locator: String::new(),
            divergence: entry.divergence.clone(),
            evidence_ref: String::new(),
            basis: entry.basis.clone(),
            reason: String::new(),
        };
        match &entry.evidence {
            EvidenceArm::Execute(command) => {
                row.real_locator = command_display(command);
                let index = command_index(command).expect("grouped above");
                // The group's ONE result fans out to every unit citing
                // it (borrowed, never consumed).
                let execution = &executions[index];
                match &execution.outcome {
                    Ok(outcome) if outcome.exit_code == 0 => {
                        row.outcome = "PASS".to_owned();
                        row.evidence_ref = execution.log_path.clone();
                    }
                    Ok(outcome) => {
                        row.outcome = "FAIL".to_owned();
                        row.evidence_ref = execution.log_path.clone();
                        row.reason = if outcome.exit_note.is_empty() {
                            format!("command exited {}", outcome.exit_code)
                        } else {
                            format!(
                                "command exited {} — {}",
                                outcome.exit_code, outcome.exit_note
                            )
                        };
                    }
                    Err(ExecuteError::CannotRun { reason }) => {
                        row.outcome = "INCONCLUSIVE".to_owned();
                        row.reason = format!("cannot_run: {reason}");
                    }
                }
            }
            EvidenceArm::Artifact { path, predicate } => {
                row.real_locator = path.clone();
                match std::fs::read(inputs.workspace_root.join(path)) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        row.outcome = "INCONCLUSIVE".to_owned();
                        row.reason = format!("no_artifact: {path} is absent");
                    }
                    Err(error) => {
                        row.outcome = "INCONCLUSIVE".to_owned();
                        row.reason = format!("unreadable_artifact: {path}: {error}");
                    }
                    Ok(bytes) => match predicates::evaluate(predicate, &bytes) {
                        Err(unknown) => {
                            row.outcome = "INCONCLUSIVE".to_owned();
                            row.reason = format!("unknown_predicate: {unknown}");
                        }
                        Ok(predicates::Predicate::Holds) => {
                            row.outcome = "PASS".to_owned();
                            row.evidence_ref = path.clone();
                        }
                        Ok(predicates::Predicate::Refutes { reason }) => {
                            row.outcome = "FAIL".to_owned();
                            row.evidence_ref = path.clone();
                            row.reason = reason;
                        }
                    },
                }
            }
            EvidenceArm::None { reason } => {
                row.outcome = "INCONCLUSIVE".to_owned();
                row.reason = reason.clone();
            }
        }
        row.status_category = category_of(&row.outcome, entry.status_hint).to_owned();
        dispositions.push(row);
    }

    // 7. Assemble the frozen report (PASS-without-evidence refused
    //    here, for every caller).
    let header = ReportHeader {
        generated_at: inputs.generated_at.clone(),
        tree_commit: inputs.tree_commit.clone(),
        answer_key_path: inputs.answer_key_path.clone(),
        answer_key_sha256: sha256_hex(inputs.ruler_text.as_bytes()),
        manifest_path: inputs.manifest_path.clone(),
        manifest_sha256: sha256_hex(inputs.manifest_text.as_bytes()),
        commands_executed,
    };
    let written: Vec<String> = evidence_files
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    let report = report::assemble_report(&header, &dispositions, &written, inputs.workspace_root)
        .map_err(SweepRefusal::Assembly)?;
    let pass = dispositions
        .iter()
        .filter(|row| row.outcome == "PASS")
        .count();
    let inconclusive = dispositions
        .iter()
        .filter(|row| row.outcome == "INCONCLUSIVE")
        .count();
    let fail = dispositions
        .iter()
        .filter(|row| row.outcome == "FAIL")
        .count();
    Ok(SweepOutcome {
        report,
        evidence_files,
        pass,
        inconclusive,
        fail,
        commands_executed,
    })
}
