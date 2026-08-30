//! The typed role definition — spec §10.1: "Roles are repository
//! artifacts, not harness state."
//!
//! A role file is UNTRUSTED CONFIGURATION. It becomes a
//! [`RoleDefinition`] only through [`RoleDefinition::load`], which
//!
//! 1. parses the front matter with the EXISTING config grammar
//!    (`crate::config::gauntlet_md` — one parse site for the grammar,
//!    no forked rule class; plan Task 17 reuse decision),
//! 2. resolves every `$VAR` / `${VAR}` indirection with the EXISTING
//!    resolve-layer semantics (`crate::config::resolve`'s expansion:
//!    presence-validated against the environment layer, the typed error
//!    names the variable and the locus, and the VALUE is never echoed —
//!    spec §8.1 "Secrets by indirection only"),
//! 3. validates the §10.1 field set — an unknown key anywhere is a typed
//!    error carrying its line, never a silently ignored field,
//! 4. compiles `context.allow` / `context.deny` into typed category
//!    data under the intersection invariant (see `category`): an allow
//!    naming a §13.2 envelope-denied category is an over-grant refused
//!    at construction; a deny shadowing the file's own allow is a
//!    contradictory contract refused at construction.
//!
//! The critic fan-out dimensions (spec §10.4, §13.3) load through this
//! same constructor — no separate grammar, no extra front-matter key.
//! The dimension rides the FILE STEM: `critic-<dimension>.md` loads as
//! `role: critic` with `dimension: Some(...)`, and a `critic-*` stem
//! whose front matter does not say `role: critic` is a typed
//! name/identity mismatch.
//!
//! Resolved indirection values live only in this in-memory structure.
//! They are never serialized by the installer (which materializes the
//! source file VERBATIM) and never appear in the lock, in logs, or in
//! error text (spec §8.1: "Generated files never contain credentials").

use gauntlet_domain::RoleId;

use crate::config::env_layer::EnvLayer;
use crate::config::error::ConfigError;
use crate::config::gauntlet_md;
use crate::config::gauntlet_md::{FmEntry, FmValue};
use crate::config::resolve::expand_indirections;

use super::category::{ContextCategory, ContextDenySet};

/// The `permissions.filesystem` access mode — a CLOSED vocabulary over the
/// spec's own mode words (COR-D06). Spec §10.1's grammar spells
/// `filesystem: read-only`; §10.2's role table and §17.3's tier prose
/// ("read-write limited to the workspace") spell the mode pair
/// read-write/read-only. Exactly those two tokens parse; anything else is a
/// typed refusal naming the token and its locus — a made-up mode word must
/// never silently weaken (or widen) the declared filesystem posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemMode {
    /// §10.1's own spelling: the role reads, it does not write.
    ReadOnly,
    /// §17.3's tier posture spelling: writes limited to the declared
    /// `write_paths` (which carry the specifics).
    ReadWrite,
}

impl FilesystemMode {
    /// The canonical role-file token.
    pub fn token(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "read-write",
        }
    }

    /// Parses one role-file token; `None` for anything outside the closed
    /// vocabulary.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "read-only" => Some(Self::ReadOnly),
            "read-write" => Some(Self::ReadWrite),
            _ => None,
        }
    }
}

/// The `permissions.network` access mode — a CLOSED vocabulary (COR-D06).
/// Spec §10.1's grammar spells `network: deny`; its `context.allow`/
/// `context.deny` grammar establishes the deny/allow word pair the
/// permission block reuses. Exactly those two tokens parse. Consistency
/// note: the ports sandbox names its runtime posture `NetworkPosture`
/// (`DenyByDefault` / `Allowlist { hosts }`) — a FINER runtime concept.
/// The role-file `allow` token is the coarser grant; at composition (plan
/// Task 20) it must lower into an allowlist requiring hosts, never a
/// blanket-open posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkMode {
    /// §10.1's own spelling: no network.
    Deny,
    /// The grant word of the §10.1 pair; scope is composition's to lower.
    Allow,
}

impl NetworkMode {
    /// The canonical role-file token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Allow => "allow",
        }
    }

    /// Parses one role-file token; `None` for anything outside the closed
    /// vocabulary.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "deny" => Some(Self::Deny),
            "allow" => Some(Self::Allow),
            _ => None,
        }
    }
}

/// One declared `permissions.write_paths` entry (COR-R1-2): the RAW token as
/// the role file spells it — an unresolved literal or a `$VAR` reference,
/// neither of which is a secret, so a refusal may echo it — and the RESOLVED
/// value, which validation consumes but no Display, log, or derived
/// rendering ever carries (spec §8.1: the value is never echoed).
#[derive(Clone, PartialEq, Eq)]
pub struct DeclaredPath {
    raw: String,
    resolved: String,
}

impl DeclaredPath {
    /// The token as the role file spells it — never a resolved value.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The resolved value — for validation only, never for display.
    pub fn resolved(&self) -> &str {
        &self.resolved
    }
}

impl core::fmt::Debug for DeclaredPath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // A resolved indirection VALUE may be a credential (spec §8.1), so
        // the derived rendering carries the raw token only.
        f.debug_struct("DeclaredPath")
            .field("raw", &self.raw)
            .finish()
    }
}

/// The permission block of a role file (spec §10.1 `permissions:`).
/// Keys are the four §10.1 spellings, all REQUIRED — a missing key must
/// not fall through to a grant-by-default reading. The `filesystem` and
/// `network` values are typed over the spec's own closed mode vocabularies
/// (see [`FilesystemMode`] / [`NetworkMode`]); `tools` entries are
/// validated against the §8.4 semantic-capability vocabulary supplied by
/// the composition root (`crates/gauntlet-cli/src/spec_vocab.rs` — three
/// of §8.4's own tokens collide with derived adapter identifiers, so the
/// boundary suite's designated place for the list is the composition root
/// and the application layer consumes it as data, exactly as doctor does);
/// `write_paths` entries are carried through indirection expansion as
/// [`DeclaredPath`]s and containment-checked by the installer against the
/// workspace, state, and protected roots — the RESOLVED value validating,
/// the RAW token naming any refusal (COR-R1-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePermissions {
    pub filesystem: FilesystemMode,
    pub write_paths: Vec<DeclaredPath>,
    pub network: NetworkMode,
    pub tools: Vec<String>,
}

/// One validated role definition. Constructible ONLY through
/// [`RoleDefinition::load`] — the validating constructor.
///
/// `Debug` is MANUAL and redacts the indirection-bearing fields on
/// purpose: resolved `$VAR` values may be credentials (spec §8.1), so no
/// derived rendering may ever print them. The same discipline the
/// config layer's resolved products follow.
#[derive(Clone, PartialEq, Eq)]
pub struct RoleDefinition {
    /// The install identifier: the source file's stem (one canonical
    /// name per role artifact; critic dimensions ride this stem).
    stem: String,
    role: RoleId,
    dimension: Option<String>,
    description: String,
    runner: Option<String>,
    model: Option<String>,
    permissions: RolePermissions,
    allow: Vec<ContextCategory>,
    deny: ContextDenySet,
    result_contract: String,
    deadline_seconds: Option<u64>,
    /// The prose after the closing fence — the role prompt body.
    body: String,
}

impl core::fmt::Debug for RoleDefinition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RoleDefinition")
            .field("stem", &self.stem)
            .field("role", &self.role)
            .field("dimension", &self.dimension)
            .field("allow", &self.allow)
            .field("deny", &self.deny)
            .field("result_contract", &self.result_contract)
            .field("deadline_seconds", &self.deadline_seconds)
            .field("runner", &"<resolved: redacted>")
            .field("model", &"<resolved: redacted>")
            .field("permissions", &"<resolved: redacted>")
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Role-file failures — typed, carrying the file's stem (and the line
/// where the front-matter grammar can give one). Never carries a
/// resolved indirection VALUE (spec §8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleError {
    /// The front matter violated the shared grammar. The parse error
    /// carries line and column.
    FrontMatter {
        role: String,
        source: gauntlet_md::GauntletMdError,
    },
    /// A required §10.1 field is absent.
    MissingField { role: String, field: &'static str },
    /// A required field is present but empty.
    EmptyValue { role: String, field: &'static str },
    /// A front-matter key outside the §10.1 field set. The role-file
    /// contract is closed, unlike the report-only posture GAUNTLET.md
    /// takes (spec §8.2): an unknown key here would silently widen or
    /// drop a constraint, so it is refused.
    UnknownField {
        role: String,
        field: String,
        line: usize,
    },
    /// A field's value shape does not match its §10.1 kind.
    KindMismatch {
        role: String,
        field: String,
        expected: &'static str,
        found: &'static str,
    },
    /// A context category token outside the closed vocabulary.
    UnknownContextCategory {
        role: String,
        field: &'static str,
        token: String,
        line: usize,
    },
    /// OVER-GRANT: `context.allow` names a category the §13.2 envelope
    /// never carries. Fail closed at construction — never a silent
    /// narrowing to what the builder permits.
    OverGrant {
        role: String,
        category: ContextCategory,
        line: usize,
    },
    /// The file's `context.deny` names a category its own
    /// `context.allow` also names — a contradictory contract, refused
    /// rather than silently resolved in either direction.
    DenyShadowsAllow {
        role: String,
        category: ContextCategory,
        line: usize,
    },
    /// `context.allow` is present but empty — a role that may read
    /// nothing is not a dispatchable contract.
    EmptyAllow { role: String },
    /// The stem's dimension suffix violates the name grammar.
    InvalidDimension { role: String, stem: String },
    /// A `critic-<dimension>` stem whose front matter is not
    /// `role: critic` — the file's name and identity disagree.
    RoleDimensionMismatch { role: String, stem: String },
    /// A `$VAR` the environment does not carry (or a malformed
    /// indirection). The wrapped config error names the VARIABLE and the
    /// locus, never the value (spec §8.1).
    Indirection { role: String, source: ConfigError },
    /// A `permissions.filesystem` token outside the spec's own mode
    /// vocabulary (§10.1 grammar, §10.2 table, §17.3 tier prose:
    /// read-only / read-write). A made-up mode word must never silently
    /// weaken or widen the declared posture.
    UnknownFilesystemMode {
        role: String,
        token: String,
        line: usize,
    },
    /// A `permissions.network` token outside the §10.1 deny/allow pair.
    UnknownNetworkMode {
        role: String,
        token: String,
        line: usize,
    },
    /// A `permissions.tools` entry outside the §8.4 semantic-capability
    /// vocabulary. These declarations are what a future PolicyProvider
    /// lowers into sandbox posture — a typo'd or widened token is a live
    /// grant question, refused at construction.
    UnknownToolToken {
        role: String,
        token: String,
        line: usize,
    },
    /// `deadline_seconds` is not a positive whole number.
    DeadlineSeconds { role: String, found: &'static str },
}

impl core::fmt::Display for RoleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrontMatter { role, source } => {
                write!(f, "role '{role}': {source}")
            }
            Self::MissingField { role, field } => {
                write!(
                    f,
                    "role '{role}': required field '{field}' is absent (spec 10.1)"
                )
            }
            Self::EmptyValue { role, field } => {
                write!(
                    f,
                    "role '{role}': field '{field}' is present but empty (spec 10.1)"
                )
            }
            Self::UnknownField { role, field, line } => write!(
                f,
                "role '{role}' line {line}: unknown field '{field}' — the 10.1 field set is \
                 closed; an unrecognized key would silently drop or widen a constraint"
            ),
            Self::KindMismatch {
                role,
                field,
                expected,
                found,
            } => write!(
                f,
                "role '{role}': field '{field}' expects {expected} but carries {found} \
                 (spec 10.1)"
            ),
            Self::UnknownContextCategory {
                role,
                field,
                token,
                line,
            } => write!(
                f,
                "role '{role}' line {line}: context.{field} names '{token}', outside the \
                 closed category vocabulary — a typo must never silently weaken a constraint"
            ),
            Self::OverGrant {
                role,
                category,
                line,
            } => write!(
                f,
                "role '{role}' line {line}: context.allow names '{category}', which the \
                 13.2 envelope never carries — a role file may narrow what the envelope \
                 builder permits, never grant beyond it; refused, not silently narrowed"
            ),
            Self::DenyShadowsAllow {
                role,
                category,
                line,
            } => write!(
                f,
                "role '{role}' line {line}: context.deny shadows context.allow on '{category}' \
                 — a contradictory contract is refused, not silently resolved"
            ),
            Self::EmptyAllow { role } => write!(
                f,
                "role '{role}': context.allow is empty — a role that may read nothing is not \
                 a dispatchable contract (spec 10.1)"
            ),
            Self::InvalidDimension { role, stem } => write!(
                f,
                "role '{role}': stem '{stem}' violates the identifier grammar — a stem is a \
                 lowercase name with hyphens, never a path"
            ),
            Self::RoleDimensionMismatch { role, stem } => write!(
                f,
                "role '{role}': stem '{stem}' names a critic dimension but the front matter \
                 does not declare role critic — the file's name and its identity disagree"
            ),
            Self::Indirection { role, source } => {
                write!(f, "role '{role}': {source}")
            }
            Self::DeadlineSeconds { role, found } => write!(
                f,
                "role '{role}': deadline_seconds expects a positive whole number but carries \
                 {found} (spec 10.1)"
            ),
            Self::UnknownFilesystemMode { role, token, line } => write!(
                f,
                "role '{role}' line {line}: permissions.filesystem names '{token}', outside the \
                 spec's own mode vocabulary of read-only and read-write (the spec 10.1 grammar; \
                 the spec 10.2 role table; the spec 17.3 tier prose); a made-up mode word must \
                 never silently weaken or widen the declared posture"
            ),
            Self::UnknownNetworkMode { role, token, line } => write!(
                f,
                "role '{role}' line {line}: permissions.network names '{token}', outside the \
                 spec's own deny / allow pair (the spec 10.1 grammar); an unknown network word \
                 must never silently become a grant"
            ),
            Self::UnknownToolToken { role, token, line } => write!(
                f,
                "role '{role}' line {line}: permissions.tools names '{token}', outside the 8.4 \
                 semantic-capability vocabulary — tools bind by capability, never by product, \
                 and a token outside the closed list has no binding"
            ),
        }
    }
}

impl std::error::Error for RoleError {}

/// The role file's stem grammar: the install identifier. Lowercase
/// letters, digits, and hyphens, starting with a letter or digit — the
/// same closed shape the harness registry uses, so a stem can never
/// carry a path separator or traversal into an installed path.
pub fn is_valid_stem(stem: &str) -> bool {
    let bytes = stem.as_bytes();
    let first_ok = bytes
        .first()
        .is_some_and(|byte| byte.is_ascii_alphanumeric());
    first_ok
        && bytes.len() <= 64
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

impl RoleDefinition {
    /// The role's install identifier (the source file's stem).
    pub fn stem(&self) -> &str {
        &self.stem
    }

    /// The role identity (spec §5.2 `RoleId`).
    pub fn role(&self) -> &RoleId {
        &self.role
    }

    /// The critic fan-out dimension, when the stem names one (spec
    /// §10.4, §13.3).
    pub fn dimension(&self) -> Option<&str> {
        self.dimension.as_deref()
    }

    /// The role's declared description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The resolved runner token, when the file declares one.
    pub fn runner(&self) -> Option<&str> {
        self.runner.as_deref()
    }

    /// The resolved model token, when the file declares one.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The declared permission block.
    pub fn permissions(&self) -> &RolePermissions {
        &self.permissions
    }

    /// The declared `context.allow`, in file order.
    pub fn context_allow(&self) -> &[ContextCategory] {
        &self.allow
    }

    /// The typed deny-set the envelope builder consumes (the
    /// consume-side hook; composed dispatch is plan Task 20).
    pub fn context_deny_set(&self) -> &ContextDenySet {
        &self.deny
    }

    /// The EFFECTIVE §13.2 envelope grant: the allowed categories that
    /// the envelope may carry. Construction guarantees the allow never
    /// names an envelope-denied category, and the deny never shadows the
    /// allow, so this is exactly `allow ∩ ENVELOPE_PERMITTED` in file
    /// order — the narrowing a role file is allowed to express.
    pub fn envelope_grant(&self) -> Vec<ContextCategory> {
        self.allow
            .iter()
            .copied()
            .filter(|category| category.envelope_permitted())
            .collect()
    }

    /// The declared result-contract schema name (spec §10.3, Annex B).
    pub fn result_contract(&self) -> &str {
        &self.result_contract
    }

    /// The declared deadline, when the file carries one.
    pub fn deadline_seconds(&self) -> Option<u64> {
        self.deadline_seconds
    }

    /// The prose body after the closing fence — the role prompt the
    /// runner adapter injects.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The validating constructor: parses, resolves, validates, and
    /// compiles the context data. See the module documentation for the
    /// four construction obligations. `tool_vocabulary` is the §8.4
    /// semantic-capability list the `permissions.tools` values are
    /// validated against — supplied by the composition root (see
    /// [`RolePermissions`]'s doc note for why the list cannot live in
    /// this crate).
    pub fn load(
        stem: &str,
        text: &str,
        env: &EnvLayer,
        tool_vocabulary: &[&str],
    ) -> Result<Self, RoleError> {
        let role_of = stem.to_owned();
        if !is_valid_stem(stem) {
            return Err(RoleError::InvalidDimension {
                role: role_of,
                stem: stem.to_owned(),
            });
        }
        // The shared grammar over the FULL document — fences included;
        // it is the authority on the front matter and on unterminated
        // documents. The split only locates the prose body.
        let front = gauntlet_md::parse(text).map_err(|source| RoleError::FrontMatter {
            role: role_of.clone(),
            source,
        })?;
        let body = split_document(text);
        let mut fields = FieldWalk::default();
        for entry in &front.entries {
            walk_field(&role_of, entry, env, tool_vocabulary, &mut fields)?;
        }
        let mut definition = fields.finish(&role_of, stem)?;
        definition.body = body;
        Ok(definition)
    }
}

/// The accumulated field walk before the required-field finish.
#[derive(Default)]
struct FieldWalk {
    role: Option<RoleId>,
    description: Option<String>,
    runner: Option<String>,
    model: Option<String>,
    permissions: Option<RolePermissions>,
    allow: Option<Vec<ContextCategory>>,
    deny: Option<Vec<(ContextCategory, usize)>>,
    result_contract: Option<String>,
    deadline_seconds: Option<u64>,
}

impl FieldWalk {
    /// The construction finish: required-field presence, emptiness
    /// checks, the deny-shadow refusal, and the stem/identity coupling.
    fn finish(self, role: &str, stem: &str) -> Result<RoleDefinition, RoleError> {
        let missing = |field: &'static str| RoleError::MissingField {
            role: role.to_owned(),
            field,
        };
        let role_id = self.role.ok_or_else(|| missing("role"))?;
        let description = owned_non_empty(role, self.description, "description")?;
        let permissions = self.permissions.ok_or_else(|| missing("permissions"))?;
        let allow = self.allow.ok_or_else(|| missing("context.allow"))?;
        let deny_list = self.deny.ok_or_else(|| missing("context.deny"))?;
        if allow.is_empty() {
            return Err(RoleError::EmptyAllow {
                role: role.to_owned(),
            });
        }
        for (category, line) in &deny_list {
            if allow.contains(category) {
                return Err(RoleError::DenyShadowsAllow {
                    role: role.to_owned(),
                    category: *category,
                    line: *line,
                });
            }
        }
        let result_contract = owned_non_empty(role, self.result_contract, "result_contract")?;

        // The stem carries the identity: a plain stem must name the role
        // exactly; a `critic-<dimension>` stem must declare
        // `role: critic` and carry a well-formed dimension (the fan-out
        // files load through this same constructor — uniform treatment).
        let dimension = match stem.strip_prefix("critic-") {
            Some(suffix) if !suffix.is_empty() => {
                if role_id != RoleId::Critic {
                    return Err(RoleError::RoleDimensionMismatch {
                        role: role.to_owned(),
                        stem: stem.to_owned(),
                    });
                }
                if !is_valid_stem(suffix) {
                    return Err(RoleError::InvalidDimension {
                        role: role.to_owned(),
                        stem: stem.to_owned(),
                    });
                }
                Some(suffix.to_owned())
            }
            _ => None,
        };
        if dimension.is_none() && stem != role_id.name() {
            return Err(RoleError::RoleDimensionMismatch {
                role: role.to_owned(),
                stem: stem.to_owned(),
            });
        }

        Ok(RoleDefinition {
            stem: stem.to_owned(),
            role: role_id,
            dimension,
            description,
            runner: self.runner,
            model: self.model,
            permissions,
            allow,
            deny: ContextDenySet::new(
                deny_list
                    .into_iter()
                    .map(|(category, _)| category)
                    .collect(),
            ),
            result_contract,
            deadline_seconds: self.deadline_seconds,
            body: String::new(),
        })
    }
}
fn owned_non_empty(
    role: &str,
    value: Option<String>,
    field: &'static str,
) -> Result<String, RoleError> {
    let value = value.ok_or_else(|| RoleError::MissingField {
        role: role.to_owned(),
        field,
    })?;
    if value.trim().is_empty() {
        return Err(RoleError::EmptyValue {
            role: role.to_owned(),
            field,
        });
    }
    Ok(value)
}

/// Walks one top-level front-matter entry against the §10.1 field set.
fn walk_field(
    role: &str,
    entry: &FmEntry,
    env: &EnvLayer,
    tool_vocabulary: &[&str],
    fields: &mut FieldWalk,
) -> Result<(), RoleError> {
    match entry.key.as_str() {
        "role" => {
            let token = expect_text(role, entry, "role")?;
            let parsed = RoleId::parse(&token).map_err(|_| RoleError::KindMismatch {
                role: role.to_owned(),
                field: "role".to_owned(),
                expected: "a role name (spec 5.2)",
                found: "a token outside the role grammar",
            })?;
            fields.role = Some(parsed);
            Ok(())
        }
        "description" => {
            let text = expect_resolved_text(role, entry, "description", env)?;
            fields.description = Some(text);
            Ok(())
        }
        "runner" => {
            let text = expect_resolved_text(role, entry, "runner", env)?;
            fields.runner = Some(text);
            Ok(())
        }
        "model" => {
            let text = expect_resolved_text(role, entry, "model", env)?;
            fields.model = Some(text);
            Ok(())
        }
        "permissions" => walk_permissions(role, entry, env, tool_vocabulary, fields),
        "context" => walk_context(role, entry, fields),
        "result_contract" => {
            let text = expect_resolved_text(role, entry, "result_contract", env)?;
            fields.result_contract = Some(text);
            Ok(())
        }
        "deadline_seconds" => {
            let seconds = match &entry.value {
                FmValue::Int(n) if *n >= 1 => u64::try_from(*n).ok(),
                FmValue::Int(_) => {
                    return Err(RoleError::DeadlineSeconds {
                        role: role.to_owned(),
                        found: "a non-positive number",
                    });
                }
                other => {
                    return Err(RoleError::DeadlineSeconds {
                        role: role.to_owned(),
                        found: kind_word(other),
                    });
                }
            };
            fields.deadline_seconds = seconds;
            Ok(())
        }
        other => Err(RoleError::UnknownField {
            role: role.to_owned(),
            field: other.to_owned(),
            line: entry.line,
        }),
    }
}

/// Walks the `permissions` block: the four §10.1 keys, all required, with
/// the `filesystem` / `network` VALUES parsed through their closed mode
/// vocabularies and every `tools` entry validated against the §8.4
/// capability list (COR-D06 — presence/kind/emptiness alone let a hostile
/// file widen past construction).
fn walk_permissions(
    role: &str,
    entry: &FmEntry,
    env: &EnvLayer,
    tool_vocabulary: &[&str],
    fields: &mut FieldWalk,
) -> Result<(), RoleError> {
    let Some(entries) = entry.value.as_map() else {
        return Err(RoleError::KindMismatch {
            role: role.to_owned(),
            field: "permissions".to_owned(),
            expected: "a mapping",
            found: kind_word(&entry.value),
        });
    };
    let mut filesystem: Option<FilesystemMode> = None;
    let mut write_paths: Option<Vec<DeclaredPath>> = None;
    let mut network: Option<NetworkMode> = None;
    let mut tools: Option<Vec<String>> = None;
    for leaf in entries {
        match leaf.key.as_str() {
            "filesystem" => {
                let (raw, resolved) =
                    expect_resolved_pair(role, leaf, "permissions.filesystem", env)?;
                let Some(mode) = FilesystemMode::parse(&resolved) else {
                    // COR-R1-2: the refusal names the UNRESOLVED token; a
                    // resolved $VAR value never reaches error text (spec §8.1).
                    return Err(RoleError::UnknownFilesystemMode {
                        role: role.to_owned(),
                        token: raw,
                        line: leaf.line,
                    });
                };
                filesystem = Some(mode);
            }
            "write_paths" => {
                write_paths = Some(
                    expect_resolved_string_list(role, leaf, "permissions.write_paths", env)?
                        .into_iter()
                        .map(|(raw, resolved)| DeclaredPath { raw, resolved })
                        .collect(),
                )
            }
            "network" => {
                let (raw, resolved) = expect_resolved_pair(role, leaf, "permissions.network", env)?;
                let Some(mode) = NetworkMode::parse(&resolved) else {
                    // COR-R1-2: the refusal names the UNRESOLVED token (spec §8.1).
                    return Err(RoleError::UnknownNetworkMode {
                        role: role.to_owned(),
                        token: raw,
                        line: leaf.line,
                    });
                };
                network = Some(mode);
            }
            "tools" => {
                let list = expect_resolved_string_list(role, leaf, "permissions.tools", env)?;
                let mut resolved_tools = Vec::with_capacity(list.len());
                for (raw, resolved) in list {
                    if !tool_vocabulary.contains(&resolved.as_str()) {
                        // COR-R1-2: the refusal names the UNRESOLVED token
                        // (spec §8.1); the resolved value validated against
                        // the vocabulary and is dropped here.
                        return Err(RoleError::UnknownToolToken {
                            role: role.to_owned(),
                            token: raw,
                            line: leaf.line,
                        });
                    }
                    resolved_tools.push(resolved);
                }
                tools = Some(resolved_tools);
            }
            other => {
                return Err(RoleError::UnknownField {
                    role: role.to_owned(),
                    field: format!("permissions.{other}"),
                    line: leaf.line,
                });
            }
        }
    }
    let missing = |field: &'static str| RoleError::MissingField {
        role: role.to_owned(),
        field,
    };
    fields.permissions = Some(RolePermissions {
        filesystem: filesystem.ok_or_else(|| missing("permissions.filesystem"))?,
        write_paths: write_paths.ok_or_else(|| missing("permissions.write_paths"))?,
        network: network.ok_or_else(|| missing("permissions.network"))?,
        tools: tools.ok_or_else(|| missing("permissions.tools"))?,
    });
    Ok(())
}

/// Walks the `context` block: `allow` and `deny`, both required, through
/// the closed category vocabulary and the intersection invariants.
fn walk_context(role: &str, entry: &FmEntry, fields: &mut FieldWalk) -> Result<(), RoleError> {
    let Some(entries) = entry.value.as_map() else {
        return Err(RoleError::KindMismatch {
            role: role.to_owned(),
            field: "context".to_owned(),
            expected: "a mapping",
            found: kind_word(&entry.value),
        });
    };
    for leaf in entries {
        match leaf.key.as_str() {
            "allow" => fields.allow = Some(context_list(role, leaf, "allow")?),
            // Deny entries carry their list's line so a shadow refusal
            // names the true locus (the shared grammar's per-entry line).
            "deny" => {
                let line = leaf.line;
                fields.deny = Some(
                    context_list(role, leaf, "deny")?
                        .into_iter()
                        .map(|category| (category, line))
                        .collect(),
                );
            }
            other => {
                return Err(RoleError::UnknownField {
                    role: role.to_owned(),
                    field: format!("context.{other}"),
                    line: leaf.line,
                });
            }
        }
    }
    Ok(())
}

/// Parses one context list against the closed vocabulary, refusing an
/// allow that names an envelope-denied category.
fn context_list(
    role: &str,
    entry: &FmEntry,
    field: &'static str,
) -> Result<Vec<ContextCategory>, RoleError> {
    let FmValue::Seq(items) = &entry.value else {
        return Err(RoleError::KindMismatch {
            role: role.to_owned(),
            field: format!("context.{field}"),
            expected: "a list of category tokens",
            found: kind_word(&entry.value),
        });
    };
    let mut categories = Vec::new();
    for item in items {
        let token = item.raw();
        let Some(category) = ContextCategory::parse(&token) else {
            return Err(RoleError::UnknownContextCategory {
                role: role.to_owned(),
                field,
                token,
                line: entry.line,
            });
        };

        if field == "allow" && category.class() == super::category::CategoryClass::EnvelopeDenied {
            return Err(RoleError::OverGrant {
                role: role.to_owned(),
                category,
                line: entry.line,
            });
        }
        categories.push(category);
    }
    Ok(categories)
}

fn expect_text(role: &str, entry: &FmEntry, field: &str) -> Result<String, RoleError> {
    match &entry.value {
        FmValue::Str(text) => Ok(text.clone()),
        other => Err(RoleError::KindMismatch {
            role: role.to_owned(),
            field: field.to_owned(),
            expected: "a string",
            found: kind_word(other),
        }),
    }
}

/// A string field whose indirections resolve now, against the
/// environment layer (presence-validated; the value never recorded).
fn expect_resolved_text(
    role: &str,
    entry: &FmEntry,
    field: &str,
    env: &EnvLayer,
) -> Result<String, RoleError> {
    expect_resolved_pair(role, entry, field, env).map(|(_, resolved)| resolved)
}

/// Resolves one text field, returning `(raw, resolved)`: the RAW token is
/// what a refusal may echo (COR-R1-2 — an unresolved literal or `$VAR`
/// reference is not a secret), the RESOLVED value is what validation
/// consumes and what must never reach error text (spec §8.1).
fn expect_resolved_pair(
    role: &str,
    entry: &FmEntry,
    field: &str,
    env: &EnvLayer,
) -> Result<(String, String), RoleError> {
    let raw = expect_text(role, entry, field)?;
    let locus = format!("roles.{role}.{field}");
    let resolved = expand_indirections(&raw, env, &locus, &mut Vec::new()).map_err(|source| {
        RoleError::Indirection {
            role: role.to_owned(),
            source,
        }
    })?;
    Ok((raw, resolved))
}

/// A string list whose items resolve now, as `(raw, resolved)` pairs —
/// validation consumes the resolved item, a refusal echoes the raw token
/// (COR-R1-2: the resolved value never reaches error text).
fn expect_resolved_string_list(
    role: &str,
    entry: &FmEntry,
    field: &str,
    env: &EnvLayer,
) -> Result<Vec<(String, String)>, RoleError> {
    let FmValue::Seq(items) = &entry.value else {
        return Err(RoleError::KindMismatch {
            role: role.to_owned(),
            field: field.to_owned(),
            expected: "a list of strings",
            found: kind_word(&entry.value),
        });
    };
    items
        .iter()
        .map(|item| {
            let locus = format!("roles.{role}.{field}");
            let raw = item.raw();
            let resolved =
                expand_indirections(&raw, env, &locus, &mut Vec::new()).map_err(|source| {
                    RoleError::Indirection {
                        role: role.to_owned(),
                        source,
                    }
                })?;
            Ok((raw, resolved))
        })
        .collect()
}

fn kind_word(value: &FmValue) -> &'static str {
    match value {
        FmValue::Str(_) => "a string",
        FmValue::Int(_) => "a number",
        FmValue::Bool(_) => "a boolean",
        FmValue::Seq(_) => "a list",
        FmValue::Map(_) => "a mapping",
    }
}

/// Locates the prose body: everything after the closing fence. Empty
/// when the document carries no front matter; the shared parse has
/// already named any structural failure (an unterminated opening fence
/// is its typed error), so this split never has to guess.
fn split_document(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some(first) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    if lines[first].trim() != "---" {
        return String::new();
    }
    let Some(close) = lines[first + 1..]
        .iter()
        .position(|line| line.trim() == "---")
    else {
        return String::new();
    };
    lines[first + 1 + close + 1..].join("\n")
}
