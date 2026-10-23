//! The protocol-neutral wire codec for the agents affordances (F2a1).
//!
//! [`AgentsSurface`] projects one [`AgentsService`] as a [`ToolSurface`]: the
//! five `agents.*` tools, their strict argument decoding, and their encoding.
//! The surface reads no clock. The caller injects a [`CallClock`], so every
//! instant is an input and every result is deterministic.
//!
//! Decoding is strict. Unknown or missing keys, wrong types, negative numbers,
//! over-long or control-bearing text, unknown tokens, and duplicate keys are
//! `Invalid`, and the detail names the offending path such as
//! `arguments.items[1].kind`. A governed refusal from the service is a normal
//! `Refused` result with a stable machine code. A refusal detail never names an
//! invocation outside the caller's subtree.

use gauntlet_domain::{
    AdmissionKey, EffectClass, EpochSeconds, HarnessId, InvocationId, MicroUsd, RoleId,
    RunnerProfileId,
};
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::invocation_graph::{AdmissionRefusal, NodeStatus};
use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::role_result::encode_role_result;
use gauntlet_ports::tool_catalog::{
    ToolCatalog, ToolInvocation, ToolInvoker, ToolName, ToolOperation,
};
use gauntlet_ports::tool_surface::{ToolListing, ToolOutcome, ToolSurface};
use gauntlet_ports::{BudgetSlice, Deadline};

use super::agents::{
    AgentView, AgentsRefusal, AgentsService, CancelView, CollectView, SpawnArgs, SpawnOutcome,
};
use super::broker::DelegationRefusal;
use super::context::{ContextItem, ContextItemKind, ContextRefusal, MAX_ENVELOPE_BYTES};
use crate::fabric::SupportLevel;
use crate::fabric::affordance::AGENTS_AFFORDANCES;
use crate::fabric::selection::{Needs, Preference, Requirements};
use crate::tools::{EffectPolicy, RuntimeError, SemanticToolRuntime};

/// The tool names, in the order the catalog presents them.
const SPAWN_TOOL: &str = "agents.spawn";
const STATUS_TOOL: &str = "agents.status";
const COLLECT_TOOL: &str = "agents.collect";
const CANCEL_TOOL: &str = "agents.cancel";
const LIST_TOOL: &str = "agents.list";

/// The root name every decode error path starts from.
const ARGUMENTS: &str = "arguments";

/// Named ceilings on decoded arguments. A value above one is `Invalid`.
pub(crate) const MAX_ROLE_BYTES: usize = 64;
pub(crate) const MAX_LABEL_BYTES: usize = 256;
pub(crate) const MAX_CONTENT_BYTES: usize = MAX_ENVELOPE_BYTES;
pub(crate) const MAX_ITEMS: usize = 32;
pub(crate) const MAX_TOKEN_BYTES: usize = 128;
pub(crate) const MAX_KEY_BYTES: usize = 128;
pub(crate) const MAX_ID_BYTES: usize = 64;
const MIN_WINDOW: u64 = 1;
const MAX_WINDOW: u64 = 86_400;
const MAX_SUPPORT_LEVEL: u64 = 4;
const MAX_DEPTH_SUPPORT: u64 = 255;

/// The support ladder, indexed by `min_support_level`.
const SUPPORT_LADDER: [SupportLevel; 5] = [
    SupportLevel::Process,
    SupportLevel::Interactive,
    SupportLevel::Session,
    SupportLevel::Structured,
    SupportLevel::Native,
];

/// The note every collect result carries. A completed child is a claim.
const COLLECT_NOTE: &str = "completion is not settlement";

/// Detail for refusals whose underlying cause must not reach the caller.
const INTERNAL_DETAIL: &str = "the affordance could not complete; the detail is withheld";
const RUNNER_DETAIL: &str = "no runner accepted the invocation";
const SETTLED_DETAIL: &str = "the admission key already names a settled invocation";
const NOT_SUBTREE_DETAIL: &str = "the invocation is not in your subtree";

const SPAWN_FIELDS: &[&str] = &[
    "role",
    "items",
    "requirements",
    "preference",
    "budget",
    "idempotency_key",
    "deadline_seconds",
];
const SPAWN_REQUIRED: &[&str] = &[
    "role",
    "items",
    "budget",
    "idempotency_key",
    "deadline_seconds",
];
const ITEM_FIELDS: &[&str] = &["kind", "label", "content"];
const REQUIREMENT_FIELDS: &[&str] = &[
    "workspace_mutation",
    "min_support_level",
    "needs_resume",
    "needs_model_selection",
    "needs_native_subagents",
    "needs_streaming",
    "needs_structured_lifecycle",
    "min_isolation",
    "min_depth_support",
];
const PREFERENCE_FIELDS: &[&str] = &["pin", "preferred_harness", "preferred_model"];
const BUDGET_FIELDS: &[&str] = &["rounds", "wall_seconds", "tokens", "cost_usd_micros"];
const TARGET_FIELDS: &[&str] = &["invocation_id"];
const NO_FIELDS: &[&str] = &[];

/// Decoded arguments are `Err(detail)` where the detail is `path: problem`.
type Decoded<T> = Result<T, String>;

/// The per-call instants the surface reads instead of a wall clock.
///
/// `at` stamps spawn, collect, and cancel. `deadline` bounds the read and
/// cancel calls. A spawn carries its own `deadline_seconds`, measured from `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallClock {
    pub at: EpochSeconds,
    pub deadline: Deadline,
}

/// The agents tool surface for one caller and one call clock.
pub struct AgentsSurface<'a> {
    service: AgentsService<'a>,
    clock: CallClock,
}

impl<'a> AgentsSurface<'a> {
    pub fn new(service: AgentsService<'a>, clock: CallClock) -> Self {
        Self { service, clock }
    }

    fn spawn(&self, arguments: &JsonValue) -> ToolOutcome {
        match decode_spawn(arguments, self.clock) {
            Ok(args) => match self.service.spawn(args) {
                Ok(outcome) => ToolOutcome::Ok(spawn_json(&outcome)),
                Err(refusal) => refused(&refusal),
            },
            Err(detail) => invalid(detail),
        }
    }

    fn status(&self, arguments: &JsonValue) -> ToolOutcome {
        let id = match decode_target(arguments) {
            Ok(id) => id,
            Err(detail) => return invalid(detail),
        };
        match self.service.status(&id, self.clock.deadline) {
            Ok(view) => ToolOutcome::Ok(agent_json(&view)),
            Err(refusal) => refused(&refusal),
        }
    }

    fn collect(&self, arguments: &JsonValue) -> ToolOutcome {
        let id = match decode_target(arguments) {
            Ok(id) => id,
            Err(detail) => return invalid(detail),
        };
        match self
            .service
            .collect(&id, self.clock.at, self.clock.deadline)
        {
            Ok(view) => ToolOutcome::Ok(collect_json(&view)),
            Err(refusal) => refused(&refusal),
        }
    }

    fn cancel(&self, arguments: &JsonValue) -> ToolOutcome {
        let id = match decode_target(arguments) {
            Ok(id) => id,
            Err(detail) => return invalid(detail),
        };
        match self.service.cancel(&id, self.clock.at, self.clock.deadline) {
            Ok(view) => ToolOutcome::Ok(cancel_json(&view)),
            Err(refusal) => refused(&refusal),
        }
    }

    fn list(&self, arguments: &JsonValue) -> ToolOutcome {
        if let Err(detail) = expect_object(arguments, ARGUMENTS)
            .and_then(|entries| check_fields(entries, ARGUMENTS, NO_FIELDS, &[]))
        {
            return invalid(detail);
        }
        match self.service.list(self.clock.deadline) {
            Ok(children) => ToolOutcome::Ok(object(vec![(
                "children",
                JsonValue::Array(children.iter().map(agent_json).collect()),
            )])),
            Err(refusal) => refused(&refusal),
        }
    }
}

impl ToolSurface for AgentsSurface<'_> {
    fn tools(&self) -> Vec<ToolListing> {
        AGENTS_AFFORDANCES
            .iter()
            .map(|descriptor| ToolListing {
                name: descriptor.name.to_string(),
                description: descriptor.summary.to_string(),
                input_schema: descriptor.input_schema.to_string(),
            })
            .collect()
    }

    fn call(&self, name: &str, arguments: &JsonValue) -> ToolOutcome {
        match name {
            SPAWN_TOOL => self.spawn(arguments),
            STATUS_TOOL => self.status(arguments),
            COLLECT_TOOL => self.collect(arguments),
            CANCEL_TOOL => self.cancel(arguments),
            LIST_TOOL => self.list(arguments),
            other => invalid(format!("unknown tool: {other}")),
        }
    }
}

fn invalid(detail: String) -> ToolOutcome {
    ToolOutcome::Invalid { detail }
}

/// Maps a service refusal to its stable code. Details never carry ids beyond
/// the caller's own input, and never a cause that could name another subtree.
fn refused(refusal: &AgentsRefusal) -> ToolOutcome {
    let (code, detail): (&str, String) = match refusal {
        AgentsRefusal::SpawnNotPermitted { .. } => ("spawn_not_permitted", refusal.to_string()),
        AgentsRefusal::RoleNotGranted { .. } => ("role_not_granted", refusal.to_string()),
        AgentsRefusal::WorkspacePolicyNotSupported { .. } => {
            ("workspace_policy_unsupported", refusal.to_string())
        }
        AgentsRefusal::BudgetAboveCeiling { .. } => ("budget_ceiling", refusal.to_string()),
        AgentsRefusal::InvalidBudget(_) => ("budget_invalid", refusal.to_string()),
        AgentsRefusal::Authority(_) => ("authority_refused", refusal.to_string()),
        AgentsRefusal::Context(inner) => (context_code(inner), refusal.to_string()),
        AgentsRefusal::Delegation(inner) => delegation_refusal(inner, refusal),
        AgentsRefusal::NotYourSubtree => ("not_your_subtree", NOT_SUBTREE_DETAIL.to_string()),
        AgentsRefusal::Workspace(_) | AgentsRefusal::Broker(_) | AgentsRefusal::Graph(_) => {
            ("internal", INTERNAL_DETAIL.to_string())
        }
    };
    ToolOutcome::Refused {
        code: code.to_string(),
        detail,
    }
}

fn context_code(refusal: &ContextRefusal) -> &'static str {
    match refusal {
        ContextRefusal::KindNotAllowed { .. } => "context_not_allowed",
        _ => "context_invalid",
    }
}

fn delegation_refusal(inner: &DelegationRefusal, outer: &AgentsRefusal) -> (&'static str, String) {
    match inner {
        DelegationRefusal::Context(context) => (context_code(context), outer.to_string()),
        DelegationRefusal::NoEligibleRunner(_) => ("no_eligible_runner", outer.to_string()),
        DelegationRefusal::RunnerUnbound { .. }
        | DelegationRefusal::RunnerAdmission(_)
        | DelegationRefusal::RunnerGaveNoSessionIdentity { .. } => {
            ("runner_unavailable", RUNNER_DETAIL.to_string())
        }
        DelegationRefusal::Admission(admission) => match admission {
            AdmissionRefusal::DepthLimit { .. } => ("limit_depth", outer.to_string()),
            AdmissionRefusal::ChildLimit { .. } => ("limit_children", outer.to_string()),
            AdmissionRefusal::BudgetExceeded { .. } => ("limit_budget", outer.to_string()),
            AdmissionRefusal::KeyReusedWithDifferentFacts
            | AdmissionRefusal::InvocationIdReused => ("key_reuse", outer.to_string()),
            _ => ("admission_refused", outer.to_string()),
        },
        DelegationRefusal::AlreadyTerminal { .. } => ("key_settled", SETTLED_DETAIL.to_string()),
        DelegationRefusal::Graph(_) => ("internal", INTERNAL_DETAIL.to_string()),
    }
}

// ----- strict decoding -------------------------------------------------------

fn expect_object<'v>(value: &'v JsonValue, path: &str) -> Decoded<&'v [(String, JsonValue)]> {
    match value {
        JsonValue::Object(entries) => Ok(entries),
        _ => Err(format!("{path}: expected an object")),
    }
}

/// Rejects duplicate keys, unknown keys, and missing required keys, in that
/// order. Parsed documents already reject duplicates; this covers values built
/// in memory.
fn check_fields(
    entries: &[(String, JsonValue)],
    path: &str,
    allowed: &[&str],
    required: &[&str],
) -> Decoded<()> {
    for (index, (key, _)) in entries.iter().enumerate() {
        if entries[..index].iter().any(|(earlier, _)| earlier == key) {
            return Err(format!("{path}.{key}: duplicate field"));
        }
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{path}.{key}: unknown field"));
        }
    }
    for name in required {
        if !entries.iter().any(|(key, _)| key == name) {
            return Err(format!("{path}: missing field {name}"));
        }
    }
    Ok(())
}

fn field<'v>(entries: &'v [(String, JsonValue)], key: &str) -> Option<&'v JsonValue> {
    entries
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn required<'v>(
    entries: &'v [(String, JsonValue)],
    path: &str,
    key: &str,
) -> Decoded<&'v JsonValue> {
    field(entries, key).ok_or_else(|| format!("{path}: missing field {key}"))
}

fn text_value(value: &JsonValue, at: &str, max_bytes: usize, allow_empty: bool) -> Decoded<String> {
    let JsonValue::Str(text) = value else {
        return Err(format!("{at}: expected a string"));
    };
    if text.is_empty() && !allow_empty {
        return Err(format!("{at}: must not be empty"));
    }
    if text.len() > max_bytes {
        return Err(format!("{at}: exceeds {max_bytes} bytes"));
    }
    if text
        .chars()
        .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\t' | '\r')))
    {
        return Err(format!("{at}: contains a control character"));
    }
    Ok(text.clone())
}

fn uint_value(value: &JsonValue, at: &str, max: u64) -> Decoded<u64> {
    match value {
        JsonValue::Number(JsonNumber::Uint(n)) if *n <= max => Ok(*n),
        JsonValue::Number(JsonNumber::Uint(_)) => Err(format!("{at}: above the maximum {max}")),
        JsonValue::Number(JsonNumber::NegInt(_)) => Err(format!("{at}: must not be negative")),
        _ => Err(format!("{at}: expected an integer")),
    }
}

fn bool_value(value: &JsonValue, at: &str) -> Decoded<bool> {
    match value {
        JsonValue::Bool(flag) => Ok(*flag),
        _ => Err(format!("{at}: expected a boolean")),
    }
}

fn token_value(value: &JsonValue, at: &str) -> Decoded<String> {
    text_value(value, at, MAX_TOKEN_BYTES, false)
}

pub(crate) fn decode_spawn(arguments: &JsonValue, clock: CallClock) -> Decoded<SpawnArgs> {
    let entries = expect_object(arguments, ARGUMENTS)?;
    check_fields(entries, ARGUMENTS, SPAWN_FIELDS, SPAWN_REQUIRED)?;

    let role_at = "arguments.role";
    let role_text = text_value(
        required(entries, ARGUMENTS, "role")?,
        role_at,
        MAX_ROLE_BYTES,
        false,
    )?;
    let role = RoleId::parse(&role_text).map_err(|_| format!("{role_at}: not a valid role"))?;

    let items = decode_items(required(entries, ARGUMENTS, "items")?, "arguments.items")?;

    let requirements = match field(entries, "requirements") {
        Some(value) => decode_requirements(value, "arguments.requirements")?,
        None => Requirements::default(),
    };
    let preference = match field(entries, "preference") {
        Some(value) => decode_preference(value, "arguments.preference")?,
        None => Preference::default(),
    };
    let budget = decode_budget(required(entries, ARGUMENTS, "budget")?, "arguments.budget")?;

    let key_at = "arguments.idempotency_key";
    let key_text = text_value(
        required(entries, ARGUMENTS, "idempotency_key")?,
        key_at,
        MAX_KEY_BYTES,
        false,
    )?;
    let idempotency_key =
        AdmissionKey::new(&key_text).map_err(|_| format!("{key_at}: not a valid key"))?;

    let seconds_at = "arguments.deadline_seconds";
    let seconds = uint_value(
        required(entries, ARGUMENTS, "deadline_seconds")?,
        seconds_at,
        MAX_WINDOW,
    )?;
    if seconds < MIN_WINDOW {
        return Err(format!("{seconds_at}: must be at least {MIN_WINDOW}"));
    }
    let instant = clock
        .at
        .get()
        .checked_add(seconds)
        .ok_or_else(|| format!("{seconds_at}: overflows the clock"))?;

    Ok(SpawnArgs {
        role,
        items,
        requirements,
        preference,
        budget,
        idempotency_key,
        deadline: Deadline::at_epoch(EpochSeconds::new(instant)),
        at: clock.at,
    })
}

fn decode_items(value: &JsonValue, at: &str) -> Decoded<Vec<ContextItem>> {
    let JsonValue::Array(elements) = value else {
        return Err(format!("{at}: expected an array"));
    };
    if elements.len() > MAX_ITEMS {
        return Err(format!("{at}: above the maximum of {MAX_ITEMS} items"));
    }
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| decode_item(element, &format!("{at}[{index}]")))
        .collect()
}

fn decode_item(value: &JsonValue, at: &str) -> Decoded<ContextItem> {
    let entries = expect_object(value, at)?;
    check_fields(entries, at, ITEM_FIELDS, ITEM_FIELDS)?;

    let kind_at = format!("{at}.kind");
    let token = token_value(required(entries, at, "kind")?, &kind_at)?;
    let kind =
        ContextItemKind::parse(&token).ok_or_else(|| format!("{kind_at}: unknown context kind"))?;
    let label = text_value(
        required(entries, at, "label")?,
        &format!("{at}.label"),
        MAX_LABEL_BYTES,
        false,
    )?;
    let content = text_value(
        required(entries, at, "content")?,
        &format!("{at}.content"),
        MAX_CONTENT_BYTES,
        true,
    )?;
    Ok(ContextItem {
        kind,
        label,
        content,
    })
}

fn decode_requirements(value: &JsonValue, at: &str) -> Decoded<Requirements> {
    let entries = expect_object(value, at)?;
    check_fields(entries, at, REQUIREMENT_FIELDS, &[])?;

    let mut requirements = Requirements::default();
    for (key, value) in entries {
        let field_at = format!("{at}.{key}");
        match key.as_str() {
            "workspace_mutation" => {
                requirements.workspace_mutation = match token_value(value, &field_at)?.as_str() {
                    "required" => Needs::Required,
                    "forbidden" => Needs::Forbidden,
                    "any" => Needs::DontCare,
                    _ => return Err(format!("{field_at}: unknown workspace mutation")),
                };
            }
            "min_support_level" => {
                let level = uint_value(value, &field_at, MAX_SUPPORT_LEVEL)?;
                requirements.min_support_level = usize::try_from(level)
                    .ok()
                    .and_then(|index| SUPPORT_LADDER.get(index).copied())
                    .ok_or_else(|| format!("{field_at}: not a support level"))?;
            }
            "needs_resume" => requirements.needs_resume = bool_value(value, &field_at)?,
            "needs_model_selection" => {
                requirements.needs_model_selection = bool_value(value, &field_at)?;
            }
            "needs_native_subagents" => {
                requirements.needs_native_subagents = bool_value(value, &field_at)?;
            }
            "needs_streaming" => requirements.needs_streaming = bool_value(value, &field_at)?,
            "needs_structured_lifecycle" => {
                requirements.needs_structured_lifecycle = bool_value(value, &field_at)?;
            }
            "min_isolation" => {
                let token = token_value(value, &field_at)?;
                requirements.min_isolation = Isolation::parse(&token)
                    .map_err(|_| format!("{field_at}: unknown isolation token"))?;
            }
            "min_depth_support" => {
                let depth = uint_value(value, &field_at, MAX_DEPTH_SUPPORT)?;
                requirements.min_depth_support = u8::try_from(depth)
                    .map_err(|_| format!("{field_at}: above the maximum {MAX_DEPTH_SUPPORT}"))?;
            }
            _ => return Err(format!("{field_at}: unknown field")),
        }
    }
    Ok(requirements)
}

fn decode_preference(value: &JsonValue, at: &str) -> Decoded<Preference> {
    let entries = expect_object(value, at)?;
    check_fields(entries, at, PREFERENCE_FIELDS, &[])?;

    let mut preference = Preference::default();
    if let Some(value) = field(entries, "pin") {
        let field_at = format!("{at}.pin");
        let token = token_value(value, &field_at)?;
        preference.pin = Some(
            RunnerProfileId::new(&token)
                .map_err(|_| format!("{field_at}: not a runner profile"))?,
        );
    }
    if let Some(value) = field(entries, "preferred_harness") {
        let field_at = format!("{at}.preferred_harness");
        let token = token_value(value, &field_at)?;
        preference.preferred_harness =
            Some(HarnessId::new(&token).map_err(|_| format!("{field_at}: not a harness"))?);
    }
    if let Some(value) = field(entries, "preferred_model") {
        preference.preferred_model = Some(token_value(value, &format!("{at}.preferred_model"))?);
    }
    Ok(preference)
}

fn decode_budget(value: &JsonValue, at: &str) -> Decoded<BudgetSlice> {
    let entries = expect_object(value, at)?;
    check_fields(entries, at, BUDGET_FIELDS, BUDGET_FIELDS)?;

    let rounds_at = format!("{at}.rounds");
    let rounds = uint_value(
        required(entries, at, "rounds")?,
        &rounds_at,
        u64::from(u32::MAX),
    )?;
    let wall_at = format!("{at}.wall_seconds");
    let wall_seconds = uint_value(required(entries, at, "wall_seconds")?, &wall_at, u64::MAX)?;
    let tokens = uint_value(
        required(entries, at, "tokens")?,
        &format!("{at}.tokens"),
        u64::MAX,
    )?;
    let cost = uint_value(
        required(entries, at, "cost_usd_micros")?,
        &format!("{at}.cost_usd_micros"),
        u64::MAX,
    )?;
    Ok(BudgetSlice {
        rounds: u32::try_from(rounds).map_err(|_| format!("{rounds_at}: out of range"))?,
        wall_seconds,
        tokens,
        cost_usd: MicroUsd::new(cost),
    })
}

fn decode_target(arguments: &JsonValue) -> Decoded<InvocationId> {
    let entries = expect_object(arguments, ARGUMENTS)?;
    check_fields(entries, ARGUMENTS, TARGET_FIELDS, TARGET_FIELDS)?;
    let at = "arguments.invocation_id";
    let text = text_value(
        required(entries, ARGUMENTS, "invocation_id")?,
        at,
        MAX_ID_BYTES,
        false,
    )?;
    InvocationId::new(&text).map_err(|_| format!("{at}: not an invocation id"))
}

// ----- encoding --------------------------------------------------------------

fn text(value: &str) -> JsonValue {
    JsonValue::Str(value.to_string())
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn optional_text(value: Option<&str>) -> JsonValue {
    value.map_or(JsonValue::Null, text)
}

fn spawn_json(outcome: &SpawnOutcome) -> JsonValue {
    object(vec![
        ("invocation_id", text(outcome.invocation_id.as_str())),
        ("depth", number(u64::from(outcome.depth))),
        ("runner_profile", text(outcome.runner_profile.as_str())),
        ("harness", text(outcome.harness.as_str())),
        ("launched", JsonValue::Bool(outcome.launched)),
    ])
}

fn status_token(status: NodeStatus) -> &'static str {
    match status {
        NodeStatus::Admitted => "admitted",
        NodeStatus::Running => "running",
        NodeStatus::Completed => "completed",
        NodeStatus::Failed(_) => "failed",
        NodeStatus::TimedOut => "timed_out",
        NodeStatus::Cancelled => "cancelled",
    }
}

fn agent_json(view: &AgentView) -> JsonValue {
    object(vec![
        ("invocation_id", text(view.invocation_id.as_str())),
        ("status", text(status_token(view.status))),
        ("depth", number(u64::from(view.depth))),
        ("harness", text(view.harness.as_str())),
        ("runner_profile", text(view.runner_profile.as_str())),
        (
            "parent",
            optional_text(view.parent.as_ref().map(InvocationId::as_str)),
        ),
        (
            "native_session_epoch",
            view.native_session_epoch
                .map_or(JsonValue::Null, |epoch| number(u64::from(epoch))),
        ),
    ])
}

fn collect_json(view: &CollectView) -> JsonValue {
    let (state, claim) = match view {
        CollectView::StillRunning(_) | CollectView::RunnerUnavailable(_) => ("still_running", None),
        CollectView::Claimed { claim, .. } => ("completed", Some(encode_role_result(claim))),
        CollectView::Failed(_) => ("failed", None),
        CollectView::TimedOut(_) => ("timed_out", None),
        CollectView::Cancelled(_) => ("cancelled", None),
        CollectView::AlreadyTerminal(_) => ("already_terminal", None),
    };
    let mut entries = vec![("state", text(state))];
    if let Some(claim) = claim {
        entries.push(("claim", text(&claim)));
    }
    // Additive and present only when true, so every other collect result keeps
    // its exact bytes.
    if matches!(view, CollectView::RunnerUnavailable(_)) {
        entries.push(("runner_unavailable", JsonValue::Bool(true)));
    }
    entries.push(("note", text(COLLECT_NOTE)));
    object(entries)
}

fn cancel_json(view: &CancelView) -> JsonValue {
    let ids =
        |list: &[InvocationId]| JsonValue::Array(list.iter().map(|id| text(id.as_str())).collect());
    object(vec![
        ("cancelled", ids(view.cancelled.as_slice())),
        ("unconfirmed", ids(view.unconfirmed.as_slice())),
    ])
}

/// The refusal code for an invocation whose caller is not the host's caller.
const CALLER_MISMATCH: &str = "caller_mismatch";

/// The `agents.*` operations as a [`ToolCatalog`]. It reads the same
/// descriptors the surface lists, so names, schemas, and effect classes have
/// one source.
pub struct AgentsCatalog;

impl ToolCatalog for AgentsCatalog {
    fn operations(&self) -> Vec<ToolOperation> {
        AGENTS_AFFORDANCES
            .iter()
            .filter_map(|descriptor| {
                Some(ToolOperation {
                    name: ToolName::new(descriptor.name).ok()?,
                    description: descriptor.summary.to_string(),
                    input_schema: parse(descriptor.input_schema).ok()?,
                    effect: descriptor.effect,
                })
            })
            .collect()
    }

    fn get(&self, name: &ToolName) -> Option<ToolOperation> {
        self.operations()
            .into_iter()
            .find(|operation| &operation.name == name)
    }
}

/// [`AgentsSurface`] as a handler: the invocation's arguments decode through
/// the same strict codec as the surface's own calls.
impl ToolInvoker for AgentsSurface<'_> {
    fn invoke(&self, invocation: &ToolInvocation) -> ToolOutcome {
        self.call(invocation.name.as_str(), &invocation.args)
    }
}

/// The `agents.*` semantic runtime for one caller. The policy admits only the
/// classes the catalog holds (`Read` and `ControlPlane`). Each handler refuses
/// an invocation that names another caller; the spawn, status, collect,
/// cancel, and list calls then run through the surface's own service, which
/// performs the canonical authority and lineage admission.
pub fn agents_runtime<'s>(
    invoker: &'s dyn ToolInvoker,
    caller: InvocationId,
) -> Result<SemanticToolRuntime<'s>, RuntimeError> {
    let policy = EffectPolicy::new(&[EffectClass::Read, EffectClass::ControlPlane]);
    let mut runtime = SemanticToolRuntime::new(&AgentsCatalog, policy)?;
    for operation in AgentsCatalog.operations() {
        let bound = caller.clone();
        runtime.register(&operation.name, move |invocation: &ToolInvocation| {
            if invocation.caller != bound {
                return ToolOutcome::Refused {
                    code: CALLER_MISMATCH.to_string(),
                    detail: "the invocation names another caller".to_string(),
                };
            }
            invoker.invoke(invocation)
        })?;
    }
    Ok(runtime)
}
