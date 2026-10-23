//! Value objects — spec §5.2 ("enums and newtypes — REQUIRED").
//!
//! Spec §5.2: "Everything in this section is typed. Stringly-typed state for
//! any field with a fixed value set is a conformance failure." Rules quoted
//! where they decide a shape:
//!
//! - "Unknown enum values in parsed input are a **parse error**, never a
//!   coerced default."
//! - "`Digest` carries its algorithm. A bare hex string is invalid."
//! - "`Outcome` is a three-state type. No implementation may provide
//!   `impl From<Outcome> for bool`."
//!
//! The §5.2 enums take their wire tokens from [`crate::wire`]; see that
//! module for why the tokens are derived from variant names rather than
//! written as literals.

use core::fmt;
use core::str::FromStr;

use crate::wire::wire_enum;

/// Which §5.2 value object an error belongs to — the typed failure locus for
/// value-level validation (AGENTS.md: typed errors carrying the locus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    // Newtypes
    RunId,
    InvocationId,
    IterationNo,
    UnitId,
    Digest,
    RoleId,
    EvidenceRef,
    VerificationRef,
    InstrumentName,
    EvidenceLocator,
    ResultContractPath,
    RunnerIdentity,
    ModelIdentity,
    SessionRef,
    ResidualId,
    ReferenceId,
    DimensionId,
    CapabilityId,
    RevisionId,
    SandboxPolicyRef,
    PermissionSetRef,
    WorkspaceRef,
    HarnessId,
    RunnerProfileId,
    AdmissionKey,
    Producer,
    // Fixed-value-set enums (§5.2) and the §5.3.8 capability status
    VerifyBy,
    BlastRadius,
    Authority,
    UnitStatus,
    Verdict,
    Outcome,
    Severity,
    TerminalState,
    ReviewMode,
    InvocationState,
    EvidenceKind,
    CapabilityStatus,
    RevisionDecision,
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::RunId => "RunId",
            Self::InvocationId => "InvocationId",
            Self::IterationNo => "IterationNo",
            Self::UnitId => "UnitId",
            Self::Digest => "Digest",
            Self::RoleId => "RoleId",
            Self::EvidenceRef => "EvidenceRef",
            Self::VerificationRef => "VerificationRef",
            Self::InstrumentName => "InstrumentName",
            Self::EvidenceLocator => "EvidenceLocator",
            Self::ResultContractPath => "ResultContractPath",
            Self::RunnerIdentity => "RunnerIdentity",
            Self::ModelIdentity => "ModelIdentity",
            Self::SessionRef => "SessionRef",
            Self::ResidualId => "ResidualId",
            Self::ReferenceId => "ReferenceId",
            Self::DimensionId => "DimensionId",
            Self::CapabilityId => "CapabilityId",
            Self::RevisionId => "RevisionId",
            Self::SandboxPolicyRef => "SandboxPolicyRef",
            Self::PermissionSetRef => "PermissionSetRef",
            Self::WorkspaceRef => "WorkspaceRef",
            Self::HarnessId => "HarnessId",
            Self::RunnerProfileId => "RunnerProfileId",
            Self::AdmissionKey => "AdmissionKey",
            Self::Producer => "Producer",
            Self::VerifyBy => "VerifyBy",
            Self::BlastRadius => "BlastRadius",
            Self::Authority => "Authority",
            Self::UnitStatus => "UnitStatus",
            Self::Verdict => "Verdict",
            Self::Outcome => "Outcome",
            Self::Severity => "Severity",
            Self::TerminalState => "TerminalState",
            Self::ReviewMode => "ReviewMode",
            Self::InvocationState => "InvocationState",
            Self::EvidenceKind => "EvidenceKind",
            Self::CapabilityStatus => "CapabilityStatus",
            Self::RevisionDecision => "RevisionDecision",
        };
        f.write_str(name)
    }
}

/// Value-object validation failures. Typed, with the offending input carried
/// for diagnosis — never a coerced fallback value (spec §5.2, AK-004).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueError {
    /// A ULID-shaped identifier failed validation (length or Crockford
    /// Base32 charset). Spec §5.2: `RunId := ULID (lexicographically
    /// sortable, collision-resistant)`.
    UlidInvalid { kind: ValueKind, input: String },
    /// `IterationNo` is 1-based (spec §5.2): zero is invalid.
    IterationNoZero,
    /// A `UnitId` failed `^[A-Z]{2,8}-[0-9]{3,6}$` (spec §5.2).
    UnitIdPattern { input: String },
    /// A digest was not the `algo ":" hex` shape (spec §5.2: "A bare hex
    /// string is invalid").
    DigestShape { input: String },
    /// The digest algorithm segment was not a plain lowercase token.
    DigestAlgorithm { algo: String },
    /// The hex segment was empty, odd-length, or not lowercase hexadecimal.
    DigestHex { hex: String },
    /// A custom role name failed `[a-z][a-z0-9-]{1,31}` (spec §5.2).
    RoleNameInvalid { input: String },
    /// A custom role name collides with one of the six named roles.
    RoleNameReserved { input: String },
    /// An identifier/reference-like value was empty, whitespace-bearing, or
    /// path-rooted where a relative token is required.
    RefInvalid { kind: ValueKind, input: String },
    /// A fixed-value-set field received an unknown token — a parse error,
    /// never a coerced default (spec §5.2, AK-004).
    EnumUnknown { kind: ValueKind, value: String },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UlidInvalid { kind, input } => {
                write!(
                    f,
                    "{kind}: not a ULID (26 Crockford Base32 chars): {input:?}"
                )
            }
            Self::IterationNoZero => {
                write!(f, "IterationNo: zero is invalid (u32, 1-based)")
            }
            Self::UnitIdPattern { input } => {
                write!(
                    f,
                    "UnitId: {input:?} does not match uppercase prefix, dash, 3-6 digits"
                )
            }
            Self::DigestShape { input } => {
                write!(f, "Digest: {input:?} is not the algo-colon-hex shape")
            }
            Self::DigestAlgorithm { algo } => {
                write!(
                    f,
                    "Digest: algorithm {algo:?} must be a non-empty lowercase token"
                )
            }
            Self::DigestHex { hex } => {
                write!(
                    f,
                    "Digest: hex {hex:?} must be non-empty, even-length lowercase hex"
                )
            }
            Self::RoleNameInvalid { input } => {
                write!(
                    f,
                    "RoleId: custom name {input:?} must be lowercase lead then 1-31 of a-z 0-9 dash"
                )
            }
            Self::RoleNameReserved { input } => {
                write!(
                    f,
                    "RoleId: custom name {input:?} collides with a named role"
                )
            }
            Self::RefInvalid { kind, input } => {
                write!(
                    f,
                    "{kind}: {input:?} must be non-empty, whitespace-free, and not path-rooted"
                )
            }
            Self::EnumUnknown { kind, value } => {
                write!(
                    f,
                    "{kind}: unknown value {value:?} is a parse error, never a coerced default"
                )
            }
        }
    }
}

impl std::error::Error for ValueError {}

// ---------------------------------------------------------------------------
// §5.2 enums. Wire tokens are derived (see crate::wire).
// ---------------------------------------------------------------------------

wire_enum! {
    /// Spec §5.2: `VerifyBy := build | test | static_check | benchmark |
    /// visual_diff | runtime_probe | schema_check | security_scan |
    /// human_signoff`.
    pub enum VerifyBy {
        Build, Test, StaticCheck, Benchmark, VisualDiff, RuntimeProbe, SchemaCheck,
        SecurityScan, HumanSignoff,
    }
}

wire_enum! {
    /// Spec §5.2: `BlastRadius := contained | local | cross_cutting |
    /// irreversible`. The tier axis of a Unit; ordering (for raises only) is
    /// declaration order.
    pub enum BlastRadius {
        Contained, Local, CrossCutting, Irreversible,
    }
}

wire_enum! {
    /// Spec §5.2: `Authority := critic | lead_agent | human`.
    pub enum Authority {
        Critic, LeadAgent, Human,
    }
}

wire_enum! {
    /// Spec §5.2: `UnitStatus := open | in_progress | settled | blocked |
    /// out_of_scope`.
    pub enum UnitStatus {
        Open, InProgress, Settled, Blocked, OutOfScope,
    }
}

wire_enum! {
    /// Spec §5.2: `Verdict := pass | fail | inconclusive` — the comparison
    /// verdict (a distinct type from [`Outcome`], AK-003).
    pub enum Verdict {
        Pass, Fail, Inconclusive,
    }
}

wire_enum! {
    /// Spec §5.2: `Severity := blocking | major | minor | informational`.
    pub enum Severity {
        Blocking, Major, Minor, Informational,
    }
}

wire_enum! {
    /// Spec §5.2: `TerminalState := settled | blocked | budget_exhausted |
    /// escalated | aborted | inconclusive`.
    pub enum TerminalState {
        Settled, Blocked, BudgetExhausted, Escalated, Aborted, Inconclusive,
    }
}

wire_enum! {
    /// Spec §5.2: `ReviewMode := single | fanout | adaptive |
    /// high_assurance`.
    pub enum ReviewMode {
        Single, Fanout, Adaptive, HighAssurance,
    }
}

wire_enum! {
    /// Spec §5.2: `InvocationState := admitted | running | completed |
    /// failed | timed_out | cancelled`.
    pub enum InvocationState {
        Admitted, Running, Completed, Failed, TimedOut, Cancelled,
    }
}

wire_enum! {
    /// Spec §5.2: `EvidenceKind := file | diff | test_report | log | metric |
    /// screenshot | scan | transcript | external | attestation`.
    pub enum EvidenceKind {
        File, Diff, TestReport, Log, Metric, Screenshot, Scan, Transcript, External, Attestation,
    }
}

// ---------------------------------------------------------------------------
// Outcome — three-state, no boolean conversion (spec §5.2 rule, AK-005).
// ---------------------------------------------------------------------------

/// Spec §5.2: `Outcome := PASS | FAIL | INCONCLUSIVE`. The verification
/// outcome. Hand-written (not [`wire_enum!`]) because its wire form is
/// uppercase, unlike every other §5.2 vocabulary.
///
/// Spec §5.2: "`Outcome` is a three-state type. No implementation may provide
/// `impl From<Outcome> for bool`." There is deliberately no `is_ok`-style
/// shortcut here; use explicit matching, `== Outcome::Pass`, or
/// [`Outcome::as_pass`], which keeps all three states distinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Pass,
    Fail,
    Inconclusive,
}

impl Outcome {
    /// Every state, for exhaustive tests and UIs that must render all three.
    pub const ALL: &'static [Self] = &[Self::Pass, Self::Fail, Self::Inconclusive];

    /// The only named boolean-ish accessor, and it keeps the three states
    /// distinct: `Some(true)`, `Some(false)`, or `None` for INCONCLUSIVE
    /// (AK-005: INCONCLUSIVE never satisfies a settlement requirement — it is
    /// not a `false` that happens to fail the same check).
    pub fn as_pass(self) -> Option<bool> {
        match self {
            Self::Pass => Some(true),
            Self::Fail => Some(false),
            Self::Inconclusive => None,
        }
    }

    /// Exact-match parse of the §5.2 uppercase wire form; anything else is a
    /// typed parse error (AK-004), never a coerced default.
    pub fn parse(input: &str) -> Result<Self, ValueError> {
        match input {
            "PASS" => Ok(Self::Pass),
            "FAIL" => Ok(Self::Fail),
            "INCONCLUSIVE" => Ok(Self::Inconclusive),
            other => Err(ValueError::EnumUnknown {
                kind: ValueKind::Outcome,
                value: other.to_owned(),
            }),
        }
    }
}

impl FromStr for Outcome {
    type Err = ValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Inconclusive => "INCONCLUSIVE",
        };
        f.write_str(token)
    }
}

// ---------------------------------------------------------------------------
// Identifier newtypes
// ---------------------------------------------------------------------------

/// The Crockford Base32 alphabet a ULID is written in (spec §5.2 `RunId`).
const ULID_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// A ULID is 26 characters (spec §5.2 `RunId`; ULID standard form).
const ULID_LEN: usize = 26;

fn validate_ulid(kind: ValueKind, input: &str) -> Result<(), ValueError> {
    let valid = input.len() == ULID_LEN && input.bytes().all(|b| ULID_ALPHABET.contains(&b));
    if valid {
        Ok(())
    } else {
        Err(ValueError::UlidInvalid {
            kind,
            input: input.to_owned(),
        })
    }
}

macro_rules! ulid_newtype {
    ($(#[$doc:meta])* $vis:vis struct $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis struct $name {
            ulid: String,
        }

        impl $name {
            /// Validates the canonical uppercase Crockford Base32 form.
            /// The domain never generates ids — `IdGenerator` is a port
            /// (spec §6).
            pub fn new(input: &str) -> Result<Self, ValueError> {
                validate_ulid(ValueKind::$name, input)?;
                Ok(Self { ulid: input.to_owned() })
            }

            /// The validated ULID text.
            pub fn as_str(&self) -> &str {
                &self.ulid
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.ulid)
            }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                Self::new(input)
            }
        }
    };
}

ulid_newtype! {
    /// Spec §5.2: `RunId := ULID (lexicographically sortable,
    /// collision-resistant)`. Validation is hand-rolled (26-char Crockford
    /// Base32, canonical uppercase); no external crate.
    pub struct RunId
}

ulid_newtype! {
    /// Spec §5.3.5: a role invocation's `id` is a ULID. Same grammar as
    /// [`RunId`]; a distinct type so an invocation id cannot stand in for a
    /// run id.
    pub struct InvocationId
}

/// Spec §5.2: `IterationNo := u32 (1-based)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IterationNo {
    n: u32,
}

impl IterationNo {
    /// Creates an iteration number. Zero is rejected: "u32 (1-based)".
    pub fn new(n: u32) -> Result<Self, ValueError> {
        if n == 0 {
            Err(ValueError::IterationNoZero)
        } else {
            Ok(Self { n })
        }
    }

    /// The 1-based number.
    pub fn get(self) -> u32 {
        self.n
    }
}

impl fmt::Display for IterationNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.n)
    }
}

/// Spec §5.2: `UnitId := /^[A-Z]{2,8}-[0-9]{3,6}$/` — e.g. `AK-014`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId {
    raw: String,
}

impl UnitId {
    /// Validates the §5.2 pattern: 2-8 ASCII uppercase letters, one dash,
    /// 3-6 ASCII digits.
    pub fn new(input: &str) -> Result<Self, ValueError> {
        // Exactly one dash: 2-8 uppercase letters, then 3-6 digits.
        let mut halves = input.split('-');
        let shape = match (halves.next(), halves.next(), halves.next()) {
            (Some(prefix), Some(digits), None) => {
                (2..=8).contains(&prefix.len())
                    && prefix.bytes().all(|b| b.is_ascii_uppercase())
                    && (3..=6).contains(&digits.len())
                    && digits.bytes().all(|b| b.is_ascii_digit())
            }
            _ => false,
        };
        if shape {
            Ok(Self {
                raw: input.to_owned(),
            })
        } else {
            Err(ValueError::UnitIdPattern {
                input: input.to_owned(),
            })
        }
    }

    /// The validated identifier text.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl fmt::Display for UnitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl FromStr for UnitId {
    type Err = ValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

/// Spec §5.2: `Digest := algo ":" hex` (algorithm is configuration; the
/// default lives in the §8.6 defaults table, not here). The digest CARRIES
/// its algorithm — "A bare hex string is invalid" — and never computes one;
/// hashing is an instrument's job.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Digest {
    algo: String,
    hex: String,
}

impl Digest {
    /// Builds a digest from validated parts: a lowercase algorithm token and
    /// even-length lowercase hex.
    pub fn new(algo: &str, hex: &str) -> Result<Self, ValueError> {
        let algo_ok = !algo.is_empty()
            && algo.len() <= 32
            && algo.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            && algo
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !algo_ok {
            return Err(ValueError::DigestAlgorithm {
                algo: algo.to_owned(),
            });
        }
        let hex_ok = !hex.is_empty()
            && hex.len().is_multiple_of(2)
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex_ok {
            return Err(ValueError::DigestHex {
                hex: hex.to_owned(),
            });
        }
        Ok(Self {
            algo: algo.to_owned(),
            hex: hex.to_owned(),
        })
    }

    /// Parses `algo ":" hex`. More than one colon, a bare hex string, or any
    /// part failing validation is a typed error.
    pub fn parse(input: &str) -> Result<Self, ValueError> {
        let (algo, hex) = match input.split_once(':') {
            Some(pair) => pair,
            None => {
                return Err(ValueError::DigestShape {
                    input: input.to_owned(),
                });
            }
        };
        if hex.contains(':') {
            return Err(ValueError::DigestShape {
                input: input.to_owned(),
            });
        }
        Self::new(algo, hex)
    }

    /// The algorithm this digest was computed with.
    pub fn algorithm(&self) -> &str {
        &self.algo
    }

    /// The hexadecimal payload.
    pub fn hex(&self) -> &str {
        &self.hex
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algo, self.hex)
    }
}

impl FromStr for Digest {
    type Err = ValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

/// The payload of a custom [`RoleId`]. Opaque on purpose: the field is
/// private and there is no public constructor, so the only way any code
/// outside this module can obtain one is by reading it out of an existing
/// `RoleId` — and every `RoleId::Custom` this module builds went through
/// [`RoleId::custom`]/[`RoleId::parse`] and their §5.2 checks. A public
/// `Custom(String)` variant would be a public constructor that bypasses the
/// grammar and the reserved-name rule; the source-shape guard test below
/// pins that this payload never gains a public construction surface.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomRoleName(String);

/// Spec §5.2: `RoleId := orchestrator | builder | critic | verifier |
/// observer | optimizer | custom(name)`, where a custom name matches
/// `[a-z][a-z0-9-]{1,31}` and must not collide with a named role.
///
/// The `Custom` payload is [`CustomRoleName`], whose representation is
/// private: the only public construction paths for a custom role are
/// [`RoleId::custom`] and [`RoleId::parse`], both of which enforce the
/// grammar and reject the six reserved role spellings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RoleId {
    Orchestrator,
    Builder,
    Critic,
    Verifier,
    Observer,
    Optimizer,
    Custom(CustomRoleName),
}

impl RoleId {
    /// The six named roles (spec §5.2), for iteration and validation.
    pub const NAMED: &'static [(&'static str, Self)] = &[
        ("orchestrator", Self::Orchestrator),
        ("builder", Self::Builder),
        ("critic", Self::Critic),
        ("verifier", Self::Verifier),
        ("observer", Self::Observer),
        ("optimizer", Self::Optimizer),
    ];

    /// A custom role. The name is validated against the §5.2 grammar and
    /// rejected when it collides with a named role — one role, one spelling.
    pub fn custom(name: &str) -> Result<Self, ValueError> {
        if let Some((named, _)) = Self::NAMED.iter().find(|(named, _)| *named == name) {
            return Err(ValueError::RoleNameReserved {
                input: (*named).to_owned(),
            });
        }
        let bytes = name.as_bytes();
        let grammar = bytes.len() >= 2
            && bytes.len() <= 32
            && bytes[0].is_ascii_lowercase()
            && bytes[1..]
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
        if grammar {
            Ok(Self::Custom(CustomRoleName(name.to_owned())))
        } else {
            Err(ValueError::RoleNameInvalid {
                input: name.to_owned(),
            })
        }
    }

    /// Parses a role name: a named role token, or any string meeting the
    /// custom grammar (which then names a custom role). Unknown tokens that
    /// fail the grammar are a typed parse error.
    pub fn parse(input: &str) -> Result<Self, ValueError> {
        if let Some((_, role)) = Self::NAMED.iter().find(|(named, _)| *named == input) {
            return Ok(role.clone());
        }
        Self::custom(input)
    }

    /// The role name as written on the wire.
    pub fn name(&self) -> &str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Builder => "builder",
            Self::Critic => "critic",
            Self::Verifier => "verifier",
            Self::Observer => "observer",
            Self::Optimizer => "optimizer",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl CustomRoleName {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for RoleId {
    type Err = ValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

/// Shared validation for reference-like string newtypes: non-empty, no
/// whitespace, not rooted at `/` (paths resolve from configuration), at most
/// 1024 bytes.
fn validate_plain_ref(kind: ValueKind, input: &str) -> Result<(), ValueError> {
    let ok = !input.is_empty()
        && input.len() <= 1024
        && !input.starts_with('/')
        && !input.bytes().any(|b| b.is_ascii_whitespace());
    if ok {
        Ok(())
    } else {
        Err(ValueError::RefInvalid {
            kind,
            input: input.to_owned(),
        })
    }
}

/// Declares a validated string newtype over [`validate_plain_ref`].
macro_rules! string_newtype {
    ($(#[$doc:meta])* $vis:vis struct $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis struct $name {
            raw: String,
        }

        impl $name {
            /// Validates and adopts the value.
            pub fn new(input: &str) -> Result<Self, ValueError> {
                validate_plain_ref(ValueKind::$name, input)?;
                Ok(Self { raw: input.to_owned() })
            }

            /// The validated value.
            pub fn as_str(&self) -> &str {
                &self.raw
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.raw)
            }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                Self::new(input)
            }
        }
    };
}

string_newtype! {
    /// A content-addressed evidence reference (spec §5.2 locator vocabulary;
    /// the store that resolves it is a port, §6). Non-empty, relative,
    /// whitespace-free.
    pub struct EvidenceRef
}

string_newtype! {
    /// A reference to a `VerificationRecord` (spec §5.3.6). Provisional id
    /// shape pending the Annex B schemas.
    pub struct VerificationRef
}

string_newtype! {
    /// The named instrument that produced a piece of evidence or a verdict
    /// (spec §5.3.6: outcomes are "produced by a named instrument").
    pub struct InstrumentName
}

string_newtype! {
    /// Where a piece of required evidence lives (spec §5.3.2
    /// `evidence_required.locator`).
    pub struct EvidenceLocator
}

string_newtype! {
    /// Where a delegated role MUST write its result document (spec §10.3).
    /// Relative by construction — absolute would be configuration.
    pub struct ResultContractPath
}

string_newtype! {
    /// A resolved runner identity (spec §5.3.5 `runner`): adapter identity is
    /// configuration, so the domain carries it as an opaque token.
    pub struct RunnerIdentity
}

string_newtype! {
    /// A resolved model identity (spec §5.3.5 `model`), recorded for
    /// provenance. Opaque to the domain.
    pub struct ModelIdentity
}

string_newtype! {
    /// An adapter-native session reference (spec §5.3.5 `session_ref`), for
    /// forensics.
    pub struct SessionRef
}

string_newtype! {
    /// A residual identifier (spec §5.1: the Comparison context communicates
    /// via "residual set").
    pub struct ResidualId
}

string_newtype! {
    /// A stable reference identifier (spec §5.3.1 `reference.id`).
    pub struct ReferenceId
}

string_newtype! {
    /// A quality-dimension identifier (spec §5.3.1).
    pub struct DimensionId
}

string_newtype! {
    /// A capability identifier (spec §5.3.8).
    pub struct CapabilityId
}

string_newtype! {
    /// A candidate-revision identifier (spec §5.3.8).
    pub struct RevisionId
}

string_newtype! {
    /// A sandbox policy reference (spec §5.3.5 `sandbox_policy_ref`).
    pub struct SandboxPolicyRef
}

string_newtype! {
    /// A resolved permission-set reference (spec §5.3.5 `permissions`,
    /// schema in §17.2 — deep schema lands with the authority-gate task).
    pub struct PermissionSetRef
}

string_newtype! {
    /// A workspace location handed to an invocation (spec §5.3.5
    /// `workspace_ref`). Opaque to the domain; workspaces come from the
    /// `WorkspaceProvider` port (§6).
    pub struct WorkspaceRef
}

string_newtype! {
    /// A harness kind identifier (spec §5.3.5 `harness`). Opaque to the
    /// domain: the harness vocabulary is configuration, never a literal here.
    pub struct HarnessId
}

string_newtype! {
    /// A runner profile identifier (spec §5.3.5 `runner_profile`). Opaque to
    /// the domain; profiles are resolved from configuration.
    pub struct RunnerProfileId
}

string_newtype! {
    /// The idempotency key of a role invocation admission (E2 design): the
    /// same key with identical immutable facts replays the same admission.
    pub struct AdmissionKey
}

/// A point in time supplied by the `Clock` port (§6). The domain never reads
/// a clock itself, so this is an uninterpreted count of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochSeconds {
    secs: u64,
}

impl EpochSeconds {
    pub fn new(secs: u64) -> Self {
        Self { secs }
    }

    pub fn get(self) -> u64 {
        self.secs
    }
}

impl fmt::Display for EpochSeconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // A canonical uppercase Crockford Base32 ULID (26 chars; no I, L, U, O).
    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn run_id_round_trips_a_valid_ulid() {
        let id = RunId::new(ULID).expect("canonical ULID accepted");
        assert_eq!(id.as_str(), ULID);
        assert_eq!(id.to_string(), ULID);
        assert_eq!(RunId::from_str(ULID).expect("from_str accepts"), id);
    }

    #[test]
    fn run_id_rejects_malformed_ulids() {
        let shorter = &ULID[..25];
        let longer = format!("{ULID}Z");
        let lowered = ULID.to_ascii_lowercase();
        // Crockford excludes I, L, U, O; each substitution is a rejection.
        let mut with_i = ULID.to_owned();
        with_i.replace_range(2..3, "I");
        let mut with_u = ULID.to_owned();
        with_u.replace_range(2..3, "U");
        let mut with_o = ULID.to_owned();
        with_o.replace_range(2..3, "O");
        let mut with_l = ULID.to_owned();
        with_l.replace_range(2..3, "L");
        for bad in [
            shorter,
            longer.as_str(),
            lowered.as_str(),
            with_i.as_str(),
            with_u.as_str(),
            with_o.as_str(),
            with_l.as_str(),
            "",
        ] {
            assert!(
                matches!(RunId::new(bad), Err(ValueError::UlidInvalid { .. })),
                "expected UlidInvalid for {bad:?}"
            );
        }
    }

    #[test]
    fn invocation_id_shares_the_ulid_grammar_but_not_the_type() {
        // Same grammar, distinct type (spec §5.3.5): an InvocationId cannot
        // be used where a RunId is required.
        let _ = InvocationId::new(ULID).expect("same grammar");
        assert!(matches!(
            InvocationId::new("not-a-ulid-at-all-surely"),
            Err(ValueError::UlidInvalid { .. })
        ));
    }

    #[test]
    fn iteration_no_is_one_based() {
        // Spec §5.2: `IterationNo := u32 (1-based)`.
        assert_eq!(IterationNo::new(0), Err(ValueError::IterationNoZero));
        let one = IterationNo::new(1).expect("one is the first iteration");
        assert_eq!(one.get(), 1);
        assert!(IterationNo::new(2).expect("two") > one);
    }

    #[test]
    fn unit_id_round_trips_the_spec_pattern() {
        // Spec §5.2: `/^[A-Z]{2,8}-[0-9]{3,6}$/`, e.g. AK-014.
        for good in ["AK-014", "AB-123", "ZZ-001", "ABCDEFGH-123456"] {
            let id = UnitId::new(good).expect("pattern-conforming id");
            assert_eq!(id.as_str(), good);
            assert_eq!(UnitId::from_str(good).expect("from_str"), id);
        }
    }

    #[test]
    fn unit_id_rejects_off_pattern_ids() {
        for bad in [
            "A-123",         // prefix shorter than 2 letters
            "ABCDEFGHI-123", // prefix longer than 8 letters
            "AK-12",         // fewer than 3 digits
            "AK-1234567",    // more than 6 digits
            "ak-014",        // lowercase prefix
            "Ak-014",        // mixed-case prefix
            "AK_014",        // wrong separator
            "AK--123",       // two separators
            "AK-014-X",      // trailing segment
            "AK-01a",        // non-digit in the numeric run
            "",              // empty
        ] {
            assert!(
                matches!(UnitId::new(bad), Err(ValueError::UnitIdPattern { .. })),
                "expected UnitIdPattern for {bad:?}"
            );
        }
    }

    #[test]
    fn digest_carries_its_algorithm() {
        // Spec §5.2: `Digest := algo ":" hex`, and "A bare hex string is
        // invalid."
        let digest = Digest::parse("sha256:a1b2c3d4").expect("valid digest");
        assert_eq!(digest.algorithm(), "sha256");
        assert_eq!(digest.hex(), "a1b2c3d4");
        assert_eq!(digest.to_string(), "sha256:a1b2c3d4");
        assert_eq!(
            Digest::from_str("sha256:a1b2c3d4").expect("from_str"),
            digest
        );
        let built = Digest::new("sha512", "e5f6a7b8").expect("parts validated");
        assert_eq!(built.algorithm(), "sha512");
        // The algorithm is carried, not assumed: the same hex payload under
        // two algorithms is two different digests.
        assert_ne!(
            Digest::new("sha256", "e5f6a7b8").expect("a"),
            Digest::new("sha512", "e5f6a7b8").expect("b")
        );
    }

    #[test]
    fn digest_rejects_bare_hex_and_malformed_shapes() {
        // "A bare hex string is invalid."
        assert!(matches!(
            Digest::parse("a1b2c3d4"),
            Err(ValueError::DigestShape { .. })
        ));
        assert!(matches!(
            Digest::parse("abc"),
            Err(ValueError::DigestShape { .. })
        ));
        // Empty algorithm, empty hex.
        assert!(matches!(
            Digest::parse(":a1b2c3d4"),
            Err(ValueError::DigestAlgorithm { .. })
        ));
        assert!(matches!(
            Digest::parse("sha256:"),
            Err(ValueError::DigestHex { .. })
        ));
        // Uppercase algorithm token.
        assert!(matches!(
            Digest::parse("SHA256:a1b2c3d4"),
            Err(ValueError::DigestAlgorithm { .. })
        ));
        // Non-hex, uppercase hex, odd-length hex.
        assert!(matches!(
            Digest::parse("sha256:nothex0"),
            Err(ValueError::DigestHex { .. })
        ));
        assert!(matches!(
            Digest::parse("sha256:A1B2C4"),
            Err(ValueError::DigestHex { .. })
        ));
        assert!(matches!(
            Digest::parse("sha256:abc"),
            Err(ValueError::DigestHex { .. })
        ));
        // A second colon makes the shape ambiguous — refused.
        assert!(matches!(
            Digest::parse("sha256:a1:b2"),
            Err(ValueError::DigestShape { .. })
        ));
    }

    #[test]
    fn role_id_parses_the_six_named_roles() {
        // Spec §5.2: orchestrator | builder | critic | verifier | observer |
        // optimizer.
        for (token, expected) in RoleId::NAMED {
            let parsed = RoleId::parse(token).expect("named role token");
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.name(), *token);
            assert_eq!(parsed.to_string(), *token);
        }
    }

    #[test]
    fn role_id_custom_names_follow_the_grammar() {
        // [a-z][a-z0-9-]{1,31}: at least two characters total.
        let custom = RoleId::custom("archivist").expect("grammar-conforming");
        assert_eq!(
            custom,
            RoleId::Custom(CustomRoleName("archivist".to_owned()))
        );
        assert_eq!(RoleId::parse("archivist"), Ok(custom.clone()));
        assert_eq!(custom.name(), "archivist");
        let dashed = RoleId::custom("x-9-role").expect("digits and dashes allowed");
        assert_eq!(dashed.name(), "x-9-role");
        let longest = format!("{}{}", "z", "-".repeat(31));
        let _ = RoleId::custom(&longest).expect("32 chars is within grammar");
        // Display/parse round-trip holds for every constructible value: the
        // reserved-name rule keeps a custom spelling from ever colliding
        // with a named one, so parse(display(x)) == x (spec 5.2).
        let round_tripped = RoleId::parse(&custom.to_string()).expect("own spelling parses");
        assert_eq!(round_tripped, custom);
    }

    #[test]
    fn role_id_rejects_bad_and_reserved_custom_names() {
        assert!(matches!(
            RoleId::custom("a"),
            Err(ValueError::RoleNameInvalid { .. })
        ));
        assert!(matches!(
            RoleId::custom("Uppercase"),
            Err(ValueError::RoleNameInvalid { .. })
        ));
        assert!(matches!(
            RoleId::custom("9digit"),
            Err(ValueError::RoleNameInvalid { .. })
        ));
        assert!(matches!(
            RoleId::custom("-leading"),
            Err(ValueError::RoleNameInvalid { .. })
        ));
        let too_long = format!("{}{}", "a", "b".repeat(32));
        assert!(matches!(
            RoleId::custom(&too_long),
            Err(ValueError::RoleNameInvalid { .. })
        ));
        // A custom name colliding with a named role is reserved: one role,
        // one spelling.
        for named in RoleId::NAMED.iter().map(|(token, _)| *token) {
            assert!(matches!(
                RoleId::custom(named),
                Err(ValueError::RoleNameReserved { .. })
            ));
        }
    }

    #[test]
    fn plain_refs_reject_empty_whitespace_and_rooted_values() {
        let _ = EvidenceRef::new("evd-000042").expect("plain token");
        assert!(matches!(
            EvidenceRef::new(""),
            Err(ValueError::RefInvalid { .. })
        ));
        assert!(matches!(
            EvidenceRef::new("has space"),
            Err(ValueError::RefInvalid { .. })
        ));
        let _ = InstrumentName::new("verifier-01").expect("plain token");
        let _ = ResultContractPath::new("result-contract").expect("plain token");
    }

    #[test]
    fn every_fixed_value_set_enum_rejects_unknown_values() {
        // Spec §5.2: "Unknown enum values in parsed input are a **parse
        // error**, never a coerced default. These fields gate execution
        // safety; lenient coercion is inappropriate." (AK-004.)
        //
        // For every §5.2 enum: each variant round-trips through its derived
        // wire token, a corrupted token is refused, and the plain unknown
        // token fails as EnumUnknown carrying the right ValueKind.
        macro_rules! enum_unknown_is_error {
            ($ty:ty, $kind:expr) => {{
                for variant in <$ty>::ALL {
                    let token = variant.wire();
                    assert_eq!(<$ty>::parse(&token).as_ref(), Ok(variant), "token {token}");
                    let corrupted = format!("{token}x");
                    assert!(
                        matches!(<$ty>::parse(&corrupted), Err(ValueError::EnumUnknown { .. })),
                        "corrupted token {corrupted:?} must not parse"
                    );
                }
                let err = <$ty>::parse("nope").expect_err("unknown token must not parse");
                assert!(
                    matches!(err, ValueError::EnumUnknown { kind, .. } if kind == $kind),
                    "wrong error locus for {err}"
                );
            }};
        }

        enum_unknown_is_error!(VerifyBy, ValueKind::VerifyBy);
        enum_unknown_is_error!(BlastRadius, ValueKind::BlastRadius);
        enum_unknown_is_error!(Authority, ValueKind::Authority);
        enum_unknown_is_error!(UnitStatus, ValueKind::UnitStatus);
        enum_unknown_is_error!(Verdict, ValueKind::Verdict);
        enum_unknown_is_error!(Severity, ValueKind::Severity);
        enum_unknown_is_error!(TerminalState, ValueKind::TerminalState);
        enum_unknown_is_error!(ReviewMode, ValueKind::ReviewMode);
        enum_unknown_is_error!(InvocationState, ValueKind::InvocationState);
        enum_unknown_is_error!(EvidenceKind, ValueKind::EvidenceKind);
    }

    #[test]
    fn outcome_wire_tokens_are_exact_and_case_sensitive() {
        // Outcome's wire form is uppercase (spec §5.2), unlike the other
        // vocabularies; matching stays exact either way.
        assert_eq!(Outcome::Pass.to_string(), "PASS");
        assert_eq!(Outcome::Fail.to_string(), "FAIL");
        assert_eq!(Outcome::Inconclusive.to_string(), "INCONCLUSIVE");
        for variant in Outcome::ALL {
            let token = variant.to_string();
            assert_eq!(Outcome::parse(&token), Ok(*variant));
            // Lowercase spellings must not coerce.
            assert!(
                matches!(
                    Outcome::parse(&token.to_ascii_lowercase()),
                    Err(ValueError::EnumUnknown { .. })
                ),
                "lowercase {token} must be a parse error"
            );
        }
        assert!(matches!(
            Outcome::parse("nope"),
            Err(ValueError::EnumUnknown {
                kind: ValueKind::Outcome,
                ..
            })
        ));
    }

    #[test]
    fn outcome_is_three_state_without_boolean_collapse() {
        // Spec §5.2: "`Outcome` is a three-state type. No implementation may
        // provide `impl From<Outcome> for bool`." The only boolean-ish view
        // keeps all three states distinguishable: INCONCLUSIVE is None, not
        // a false.
        assert_eq!(Outcome::ALL.len(), 3);
        assert_eq!(Outcome::Pass.as_pass(), Some(true));
        assert_eq!(Outcome::Fail.as_pass(), Some(false));
        assert_eq!(Outcome::Inconclusive.as_pass(), None);
    }

    #[test]
    fn outcome_has_no_boolean_conversion_surface_in_this_crate() {
        // AK-005 at the domain level: scan every source file's CODE for the
        // forbidden conversion and for any boolean-collapse METHOD
        // DEFINITION or trait implementation. Banning the bare call spelling
        // would also outlaw Result::is_ok, which the toolchain itself
        // recommends; banning the definition is what holds the invariant —
        // with none of these surfaces anywhere in this crate, Outcome cannot
        // gain one, and Outcome lives only in this crate.
        //
        // The needles are widened past the spec's literal sentence to every
        // spelling that collapses the three states:
        // - the spec letter: an `impl From<Outcome> for bool`;
        // - any other trait implemented *for* bool (`PartialEq<bool> for
        //   Outcome`, `PartialEq<Outcome> for bool`, …) — `outcome == true`
        //   compiles the moment one lands;
        // - method definitions that return an undiscriminated truth value
        //   (is_ok / is_success / is_pass / passed / to_bool / as_bool);
        // - an `as bool` cast.
        // Comment lines are stripped first: this module quotes the spec rule
        // verbatim in its docs, and a comment compiles to nothing. The
        // needles are assembled from fragments so this guard's own text
        // cannot satisfy itself. (Multi-line block comments are a blind spot
        // of the strip; the crate contains none.)
        let from_bool = ["impl From<Outcome>", " for ", "bool"].concat();
        let any_impl_for_bool = [" for ", "bool"].concat();
        let partial_eq_bool = ["impl PartialEq", "<bool>"].concat();
        let is_ok_method = ["fn is_", "ok"].concat();
        let is_success_method = ["fn is_", "success"].concat();
        let is_pass_method = ["fn is_", "pass"].concat();
        let passed_method = ["fn ", "passed"].concat();
        let to_bool_method = ["fn to_", "bool"].concat();
        let as_bool_method = ["fn as_", "bool"].concat();
        let as_bool_cast = [" as ", "bool"].concat();
        for (name, text) in crate::test_sources::read_all() {
            let code: String = text
                .lines()
                .filter(|line| !line.trim_start().starts_with('/'))
                .collect::<Vec<_>>()
                .join("\n");
            for needle in [
                from_bool.as_str(),
                any_impl_for_bool.as_str(),
                partial_eq_bool.as_str(),
                is_ok_method.as_str(),
                is_success_method.as_str(),
                is_pass_method.as_str(),
                passed_method.as_str(),
                to_bool_method.as_str(),
                as_bool_method.as_str(),
                as_bool_cast.as_str(),
            ] {
                assert!(
                    !code.contains(needle),
                    "{name}: {needle} collapses Outcome's three states (AK-005)"
                );
            }
        }
    }

    #[test]
    fn role_id_custom_construction_is_only_via_the_validating_paths() {
        // CA-1 regression guard. The §5.2 grammar and reserved-name rule are
        // enforceable only if no public constructor can produce an
        // unvalidated custom name; that guarantee is a compile-time fact
        // (RoleId::Custom's payload is CustomRoleName, whose representation
        // is private and constructorless outside this module), so it cannot
        // be exercised by a runtime test. What a test CAN do is pin the
        // source shape that keeps it a compile-time fact — the same
        // mechanism the AK-005 and §5.3.3 guards above use. If the payload
        // reverts to a bare String, or CustomRoleName grows a public
        // constructor, this fails. Comment lines are stripped first (this
        // module's own documentation names the banned shape).
        let (_, text) = crate::test_sources::read_all()
            .into_iter()
            .find(|(name, _)| name == "values.rs")
            .expect("values.rs is among the sources");
        let production = text
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a first element");
        let code: Vec<&str> = production
            .lines()
            .filter(|line| !line.trim_start().starts_with('/'))
            .collect();
        let payload_variant = ["Custom(", "CustomRoleName"].concat();
        assert!(
            code.iter().any(|line| line.contains(&payload_variant)),
            "the Custom variant must carry the opaque payload"
        );
        for line in &code {
            assert!(
                !line.contains("Custom(String)"),
                "a Custom variant over a bare String is a public constructor that \
                 bypasses the spec 5.2 grammar and reserved-name rule"
            );
        }
        // The payload type itself must stay opaque: no impl block for it may
        // contain any public item. The scan is block-aware (brace depth)
        // because a public constructor would sit on its own line inside the
        // impl, not on the line that names the type.
        let mut depth: i32 = 0;
        for line in &code {
            if depth == 0 {
                if line.trim_start().starts_with("impl CustomRoleName") {
                    depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
                }
                continue;
            }
            assert!(
                !line.contains("pub "),
                "CustomRoleName must not gain a public construction surface: {line}"
            );
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
        }
        assert_eq!(depth, 0, "unterminated CustomRoleName impl block");
    }
}
