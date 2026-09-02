//! The role installer and drift verifier — plan Task 17; spec §10.1
//! ("`gauntlet roles install` writes them into whichever harness is
//! configured; `gauntlet roles verify` re-reads them back and reports
//! drift"), AK-063.
//!
//! # What install materializes
//!
//! The role files' bytes, VERBATIM, under the configured state root:
//! `<state_dir>/roles/installed/<harness>/<stem>.md` for a writable
//! role surface, `<state_dir>/roles/injectable/<harness>/<stem>.md` for
//! the advisory fallback (whose bodies are the per-invocation,
//! prompt-side injection artifacts). Nothing resolves into the written
//! bytes: indirections are validated for presence at LOAD, so no
//! environment value — least of all a secret — is ever written by
//! install (spec §8.1: "Generated files never contain credentials").
//!
//! Every installed path is checked lexically against its root before
//! any byte is written: the roles source must live inside the project
//! root (roles are repository artifacts, spec §10.1), installed state
//! must live inside the state root, and every `permissions.write_paths`
//! entry is validated in RESOLVED form — absolute entries as-is,
//! relative entries anchored at the project root (conservative; the
//! installer cannot see the dispatch-time workspace anchor) — against
//! both the outside-the-roots escape refusal and §17.5.5's in-project
//! protected set (the ruler directory as the defaults layer resolves
//! it, the frozen reference lock, the comparison contract, the tier
//! policies, and this module's own lock): a role file cannot point its
//! own writes at the operator's machine, nor at the ruler.
//!
//! # The lock, and its honest tamper story
//!
//! Install writes `.gauntlet/roles.lock` next to the project: the
//! schema token, the harness id, the enforcement level, and per role
//! the stem, identity, dimension, source path (relative to the project
//! root), installed path (relative to the state root), and the SHA-256
//! of the verbatim bytes — no timestamps, sorted entries, so an
//! unchanged tree reinstalls to a BYTE-IDENTICAL lock.
//!
//! The lock carries a self-digest (SHA-256 over its canonical body,
//! the `lock_digest` line excluded) and verify re-serializes the parsed
//! lock and compares it byte-for-byte with the file, so a hand edit to
//! any recorded field is a typed tamper failure. A self-digest is, by
//! itself, forgeable by an attacker who recomputes it — so the DRIFT
//! VERDICT never rests on the lock alone. Verify independently digests
//! the ACTUAL source bytes and the ACTUAL installed bytes against the
//! lock's recorded digest. Editing installed state and re-forging the
//! lock to match still fails, because the untouched repository source
//! no longer agrees with the forged record — the only way to hide that
//! drift is to edit the reviewed source file itself, which the
//! repository's own review surface sees. The cryptographic anchor is
//! the source tree, not the lock's opinion of itself. That story is
//! STRUCTURAL (COR-R1-1): every lock-recorded path is DERIVED at verify
//! time from the values verify already trusts — the configured roles
//! dir, the configured state root, the registered harness surface, the
//! enforcement level, and the stem — so a lock naming any other
//! in-project source, a fabricated harness directory, or an
//! unregistered label is a tamper refusal before any digest is even
//! compared. A redirect or relocation cannot bless drift; only the
//! reviewed source file can agree with the record.
//!
//! Verify without a prior install is a typed configuration failure:
//! missing evidence is not passing evidence.

use std::path::{Path, PathBuf};

use gauntlet_domain::Digest;

use crate::config::defaults;
use crate::config::env_layer::EnvLayer;

use super::definition::{RoleDefinition, RoleError, is_valid_stem};
use super::harness::{HarnessRegistry, RegistryError, RoleSurface};

/// The lock's file name, inside the runtime's reserved project
/// directory. Runtime-owned state — not a frozen reference; it never
/// gates doctor's contract checks (the frozen preregistration's D2).
pub const ROLES_LOCK_NAME: &str = "roles.lock";

/// The state-root tree the installer materializes under.
pub const STATE_TREE_NAME: &str = "roles";

/// The lock's schema token, versioned like every schema (Annex B).
pub const LOCK_SCHEMA: &str = "gauntlet-roles-lock@v1";

/// How the installed role surface is enforced. The advisory level is
/// the plan's prescribed fallback for a harness with no writable role
/// surface — recorded, visible weakness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enforcement {
    /// Installed role files are the harness's enforced role state.
    Writable,
    /// No writable surface: injection is per-invocation, prompt-side;
    /// the lock records the weakness instead of faking enforcement.
    Advisory,
}

impl Enforcement {
    fn token(self) -> &'static str {
        match self {
            Self::Writable => "writable",
            Self::Advisory => "advisory",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "writable" => Some(Self::Writable),
            "advisory" => Some(Self::Advisory),
            _ => None,
        }
    }

    fn from_surface(surface: RoleSurface) -> Self {
        match surface {
            RoleSurface::WritableRoleDir => Self::Writable,
            RoleSurface::AdvisoryPromptArtifacts => Self::Advisory,
        }
    }
}

/// One lock entry: one installed role artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    /// The install identifier (the source file's stem).
    pub role: String,
    /// The declared role identity token (spec §5.2).
    pub role_id: String,
    /// The critic fan-out dimension, when the stem names one.
    pub dimension: Option<String>,
    /// The source path, relative to the project root.
    pub source: String,
    /// The installed path, relative to the state root.
    pub installed: String,
    /// The SHA-256 of the verbatim bytes: source and installed agree by
    /// construction, and any divergence from either actual file is
    /// drift.
    pub digest: String,
}

/// The parsed roles lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolesLock {
    pub harness: String,
    pub enforcement: Enforcement,
    pub entries: Vec<LockEntry>,
}

impl RolesLock {
    /// The canonical body: everything except the self-digest line,
    /// ending in a newline. Entries are in construction order — the
    /// installer builds them sorted, and the parser re-serializes to
    /// enforce exactly this byte form.
    fn serialize_body(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("schema: {LOCK_SCHEMA}\n"));
        out.push_str(&format!("harness: {}\n", self.harness));
        out.push_str(&format!("enforcement: {}\n", self.enforcement.token()));
        out.push_str("entries:\n");
        for entry in &self.entries {
            out.push_str(&format!("  - role: {}\n", entry.role));
            out.push_str(&format!("    role_id: {}\n", entry.role_id));
            let dimension = entry.dimension.as_deref().unwrap_or("-");
            out.push_str(&format!("    dimension: {dimension}\n"));
            out.push_str(&format!("    source: {}\n", entry.source));
            out.push_str(&format!("    installed: {}\n", entry.installed));
            out.push_str(&format!("    digest: {}\n", entry.digest));
        }
        out
    }

    /// The self-digest: SHA-256 over the canonical body (the
    /// `lock_digest` line excluded).
    fn self_digest(&self) -> String {
        sha256_hex(self.serialize_body().as_bytes())
    }

    /// The full lock bytes: canonical body plus the self-digest line.
    fn serialize(&self) -> String {
        format!(
            "{}lock_digest: sha256:{}\n",
            self.serialize_body(),
            self.self_digest()
        )
    }

    /// Strict parse: the file must BE this lock's canonical form. Any
    /// deviation is a schema failure naming the line; a self-digest
    /// mismatch is tamper. The parse re-serializes what it read and
    /// demands byte equality with the file, so only the canonical form
    /// is ever accepted.
    fn parse(text: &str) -> Result<Self, RolesError> {
        let schema_line = format!("schema: {LOCK_SCHEMA}");
        let lines: Vec<&str> = text.lines().collect();
        let bad = |line: usize, detail: &str| RolesError::LockSchema {
            line,
            detail: detail.to_owned(),
        };
        let Some(last) = lines.last() else {
            return Err(bad(1, "the lock is empty"));
        };
        let Some(stored_digest) = last.strip_prefix("lock_digest: sha256:") else {
            return Err(bad(
                lines.len(),
                "the last line must carry the lock self-digest",
            ));
        };
        if Digest::new("sha256", stored_digest).is_err() {
            return Err(bad(
                lines.len(),
                "the lock self-digest is not a valid digest",
            ));
        }
        let body = &lines[..lines.len() - 1];
        if sha256_hex(join_lines(body).as_bytes()) != stored_digest {
            return Err(RolesError::LockTampered {
                detail: "the lock body does not digest to its own self-digest — the lock was \
                 edited after it was written"
                    .to_owned(),
            });
        }
        fn schema_of<'a>(line: &'a [&'a str], index: usize) -> &'a str {
            line.get(index).copied().unwrap_or_default().trim()
        }
        if schema_of(body, 0) != schema_line {
            return Err(bad(1, "the lock must open with its schema line"));
        }
        let harness = schema_of(body, 1);
        let Some(harness) = harness.strip_prefix("harness: ").map(str::to_owned) else {
            return Err(bad(2, "the second line must carry the harness id"));
        };
        let Some(enforcement) = schema_of(body, 2)
            .strip_prefix("enforcement: ")
            .and_then(Enforcement::parse)
        else {
            return Err(bad(3, "the third line must carry the enforcement level"));
        };
        if schema_of(body, 3) != "entries:" {
            return Err(bad(4, "the fourth line must open the entries block"));
        }
        let mut entries = Vec::new();
        let mut at = 4;
        while at < body.len() {
            let line = body[at];
            let Some(role) = line.strip_prefix("  - role: ") else {
                return Err(bad(
                    at + 1,
                    "each entry must open with its role line at the entry indent",
                ));
            };
            let field = |offset: usize, name: &str| -> Result<String, RolesError> {
                let value = body.get(at + offset).copied().unwrap_or_default();
                value
                    .strip_prefix(&format!("    {name}: "))
                    .map(str::to_owned)
                    .ok_or_else(|| bad(at + offset + 1, "entry fields must follow the entry line"))
            };
            let role_id = field(1, "role_id")?;
            let dimension_raw = field(2, "dimension")?;
            let source = field(3, "source")?;
            let installed = field(4, "installed")?;
            let digest = field(5, "digest")?;
            let Some(hex) = digest.strip_prefix("sha256:") else {
                return Err(bad(at + 6, "an entry digest must be a sha256 token"));
            };
            if Digest::new("sha256", hex).is_err() {
                return Err(bad(at + 6, "an entry digest must carry valid digest hex"));
            }
            entries.push(LockEntry {
                role: role.to_owned(),
                role_id,
                dimension: (dimension_raw != "-").then_some(dimension_raw),
                source,
                installed,
                digest,
            });
            at += 6;
        }
        let lock = Self {
            harness,
            enforcement,
            entries,
        };
        // Canonical closure: what was parsed must re-serialize to the
        // exact bytes on disk — a hand-edited lock does not pass even
        // when its self-digest line is recomputed to match.
        if lock.serialize_body() != join_lines(body) {
            return Err(RolesError::LockTampered {
                detail: "the lock is not in its canonical form — locks are runtime-written \
                         documents, never hand-edited ones"
                    .to_owned(),
            });
        }
        // COR-D01: install can never write an empty-entries lock —
        // `EmptyRoleSet` refuses an empty roles directory before any lock
        // byte exists — so a canonical, self-digest-consistent lock with
        // zero entries is by construction hand-forged. Missing evidence is
        // not passing evidence, and an empty record is no record at all.
        if lock.entries.is_empty() {
            return Err(RolesError::LockTampered {
                detail: "the lock records no roles — install refuses an empty roles set \
                         (EmptyRoleSet), so an empty-entries lock is by construction \
                         hand-forged, never runtime-written"
                    .to_owned(),
            });
        }
        // COR-R1-3: install writes one entry per role stem (the roles tree
        // is keyed by stem, sorted, unique), so a repeated stem is a shape
        // install would never write — a canonical, self-digest-consistent
        // duplicate-entry lock is by construction hand-forged. Refused,
        // naming the stem.
        let mut seen = std::collections::BTreeSet::new();
        for entry in &lock.entries {
            if !seen.insert(entry.role.as_str()) {
                return Err(RolesError::LockTampered {
                    detail: format!(
                        "the lock records role '{}' more than once — install writes one entry \
                         per role stem, so a duplicate entry is hand-forged, never \
                         runtime-written",
                        entry.role
                    ),
                });
            }
        }
        Ok(lock)
    }
}

fn join_lines(lines: &[&str]) -> String {
    let mut out = lines.join("\n");
    if !lines.is_empty() {
        out.push('\n');
    }
    out
}

/// Role installer and verifier failures — typed, carrying the locus.
/// Refusals carry the UNRESOLVED token or a shape description — a resolved
/// $VAR indirection VALUE never reaches error text (COR-R1-2, spec §8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolesError {
    /// The harness id is missing or empty. Fail closed: no runner is
    /// selected by default — that is the operator's call.
    HarnessUnspecified,
    /// The id names no registered harness.
    UnknownHarness { id: String },
    /// The registry itself was authored with an invalid id.
    Registry(RegistryError),
    /// The roles directory carried no role files — an install that
    /// wrote an empty lock would be dishonest success.
    EmptyRoleSet { dir: String },
    /// A role file failed its typed construction.
    Role { role: String, source: RoleError },
    /// A filesystem operation failed; the io error KIND is carried,
    /// never file contents.
    Io {
        locus: &'static str,
        path: String,
        kind: std::io::ErrorKind,
    },
    /// An input file is not UTF-8 text.
    NotUtf8 { path: String },
    /// The roles source directory is outside the project root — roles
    /// are repository artifacts (spec §10.1), not host files.
    SourceOutsideProject { path: String },
    /// A write_paths entry whose RESOLVED value is an absolute path outside
    /// the project and state roots — a role file steering its own writes at
    /// the operator's machine is refused. The refusal carries the RAW
    /// declared token, never the resolved value (COR-R1-2, spec §8.1).
    WritePathEscape { role: String, path: String },
    /// A write_paths entry whose RESOLVED value sits on the in-project
    /// protected set — inside a protected path, or covering one (a lexical
    /// prefix of it, e.g. the project root or the runtime dir). Spec
    /// §17.5.5: "The evidence store, state store, frozen reference, and
    /// answer key are outside every builder-writable path." The refusal
    /// carries the RAW declared token, never the resolved value (COR-R1-2).
    WritePathProtected {
        role: String,
        path: String,
        protected: String,
    },
    /// Verify ran where nothing was installed. Missing evidence is not
    /// passing evidence.
    LockMissing { path: String },
    /// The lock file does not decode as this schema.
    LockSchema { line: usize, detail: String },
    /// The lock was edited after it was written (content or form).
    LockTampered { detail: String },
    /// Drift: at least one role diverges from its installed record.
    Drift { entries: Vec<DriftRecord> },
}

/// One drifted role, with what the lock recorded and what is actually
/// there (digests, never contents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftRecord {
    pub role: String,
    pub kind: DriftKind,
    pub expected: String,
    pub actual: Option<String>,
}

/// Which side drifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// The repository source file is gone.
    SourceMissing,
    /// The repository source file no longer matches the installed
    /// record — reinstall to re-publish.
    SourceModified,
    /// The installed artifact is gone.
    InstalledMissing,
    /// The installed artifact no longer matches the record — out-of-band
    /// harness-side mutation (Annex C.4: harness state is agent-mutable).
    InstalledModified,
    /// An installed `.md` artifact the lock records nothing about — an
    /// out-of-band ADDITION to the harness surface (Annex C.4).
    InstalledExtra,
    /// A source role file the lock records nothing about — the install is
    /// stale; reinstall to record it.
    SourceExtra,
}

impl core::fmt::Display for DriftKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceMissing => f.write_str("source file missing"),
            Self::SourceModified => f.write_str("source file modified since install"),
            Self::InstalledMissing => f.write_str("installed artifact missing"),
            Self::InstalledModified => f.write_str("installed artifact modified out-of-band"),
            Self::InstalledExtra => f.write_str(
                "installed artifact absent from the lock — out-of-band harness addition",
            ),
            Self::SourceExtra => {
                f.write_str("source role file absent from the lock — stale install")
            }
        }
    }
}

impl core::fmt::Display for RolesError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HarnessUnspecified => write!(
                f,
                "roles: no harness id — pass an explicit registered --harness id; no runner \
                 is selected by default (that is the operator's call)"
            ),
            Self::UnknownHarness { id } => write!(
                f,
                "roles: harness id '{id}' is not registered — register it in the composition \
                 root or pass a registered id; unknown ids fail closed"
            ),
            Self::Registry(inner) => write!(f, "roles: {inner}"),
            Self::EmptyRoleSet { dir } => write!(
                f,
                "roles: the roles directory '{dir}' carries no role files — an install that \
                 recorded nothing would be dishonest success (spec 10.1)"
            ),
            Self::Role { source, .. } => write!(f, "roles: {source}"),
            Self::Io { locus, path, kind } => {
                write!(f, "roles: {locus} at '{path}' failed: {kind}")
            }
            Self::NotUtf8 { path } => {
                write!(
                    f,
                    "roles: '{path}' is not UTF-8 text; role files are text artifacts"
                )
            }
            Self::SourceOutsideProject { path } => write!(
                f,
                "roles: the roles source '{path}' is outside the project root — roles are \
                 repository artifacts (spec 10.1), not host files"
            ),
            Self::WritePathEscape { role, path } => write!(
                f,
                "roles: role '{role}' declares write path '{path}', which escapes the project \
                 and state roots — a role file cannot aim its own writes at the operator's \
                 machine (the path is named as the file spells it; a resolved indirection \
                 value is never echoed, spec 8.1)"
            ),
            Self::WritePathProtected {
                role,
                path,
                protected,
            } => write!(
                f,
                "roles: role '{role}' declares write path '{path}', which resolves inside or \
                 covers the protected path '{protected}' — the frozen reference, answer key, \
                 comparison contract, tier policies, and roles lock are outside every \
                 builder-writable path (spec 17.5.5; the path is named as the file spells it; \
                 a resolved indirection value is never echoed, spec 8.1)"
            ),
            Self::LockMissing { path } => write!(
                f,
                "roles: no lock at '{path}' — nothing is installed, so there is nothing to \
                 verify; missing evidence is not passing evidence"
            ),
            Self::LockSchema { line, detail } => {
                write!(
                    f,
                    "roles: the lock does not decode at line {line}: {detail}"
                )
            }
            Self::LockTampered { detail } => {
                write!(f, "roles: the lock fails its integrity checks: {detail}")
            }
            Self::Drift { entries } => {
                write!(f, "roles: drift detected across {} role(s):", entries.len())?;
                for record in entries {
                    let kind = record.kind;
                    write!(f, " [{}] {kind}", record.role)?;
                    if let Some(actual) = &record.actual {
                        write!(f, " (recorded {}, actual {actual})", record.expected)?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RolesError {}

/// Install inputs: the resolved roots, the explicit harness id, the
/// registry (built by the composition root — a data map, never role
/// logic spread across match arms), and the §8.4 semantic-capability
/// vocabulary the role files' `permissions.tools` values are validated
/// against (supplied by the composition root for the same boundary-suite
/// reason doctor's capability list is — see `RolePermissions`'s doc).
pub struct InstallInput<'a> {
    pub project_root: &'a Path,
    pub state_dir: &'a Path,
    pub roles_dir: &'a Path,
    pub harness: &'a str,
    pub registry: &'a HarnessRegistry,
    pub env: &'a EnvLayer,
    pub tool_vocabulary: &'static [&'static str],
}

/// What one install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub harness: String,
    pub enforcement: Enforcement,
    pub lock_path: PathBuf,
    pub roles: usize,
    /// True when an EXISTING installed artifact's bytes were actually
    /// rewritten during this install — compared before/after per role
    /// (COR-R1-4: drift-relative, never merely lock-relative). Materializing
    /// a new artifact is publishing, not repair, so a first install is not a
    /// repair; the CLI message stays honest in both directions.
    pub repaired: bool,
}

/// Verifier inputs: the roots the lock's relative paths hang from, plus the
/// configured roles dir and the harness registry (COR-R1-1: the expected
/// paths are DERIVED from the configured surface — the roles dir, the state
/// root, the registered harness, the enforcement level, the stem — never
/// trusted from the lock's own records).
pub struct VerifyInput<'a> {
    pub project_root: &'a Path,
    pub state_dir: &'a Path,
    /// The configured roles dir (the same anchored resolution install used).
    pub roles_dir: &'a Path,
    /// The harness registry the lock's label must be registered in.
    pub registry: &'a HarnessRegistry,
}

/// What one verify found when everything held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub harness: String,
    pub enforcement: Enforcement,
    pub roles: usize,
}

/// Loads the role tree: every `<stem>.md` under `roles_dir`, sorted by
/// stem, each through the typed constructor.
fn load_role_tree(
    roles_dir: &Path,
    project_root: &Path,
    env: &EnvLayer,
    tool_vocabulary: &'static [&'static str],
) -> Result<Vec<(String, Vec<u8>, RoleDefinition)>, RolesError> {
    if !ensure_within_root(roles_dir, project_root) {
        return Err(RolesError::SourceOutsideProject {
            path: roles_dir.to_string_lossy().into_owned(),
        });
    }
    let read = std::fs::read_dir(roles_dir).map_err(|error| RolesError::Io {
        locus: "reading the roles directory",
        path: roles_dir.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for item in read {
        let item = item.map_err(|error| RolesError::Io {
            locus: "reading a roles directory entry",
            path: roles_dir.to_string_lossy().into_owned(),
            kind: error.kind(),
        })?;
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        files.push((stem.to_owned(), path));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        return Err(RolesError::EmptyRoleSet {
            dir: roles_dir.to_string_lossy().into_owned(),
        });
    }
    let mut loaded = Vec::new();
    for (stem, path) in files {
        if !is_valid_stem(&stem) {
            return Err(RolesError::Role {
                role: stem.clone(),
                source: RoleError::InvalidDimension {
                    role: stem.clone(),
                    stem,
                },
            });
        }
        let bytes = std::fs::read(&path).map_err(|error| RolesError::Io {
            locus: "reading a role file",
            path: path.to_string_lossy().into_owned(),
            kind: error.kind(),
        })?;
        let text = String::from_utf8(bytes.clone()).map_err(|_| RolesError::NotUtf8 {
            path: path.to_string_lossy().into_owned(),
        })?;
        let definition =
            RoleDefinition::load(&stem, &text, env, tool_vocabulary).map_err(|source| {
                RolesError::Role {
                    role: stem.clone(),
                    source,
                }
            })?;
        loaded.push((stem, bytes, definition));
    }
    Ok(loaded)
}

/// Installs the repository role artifacts into the named harness's role
/// surface and writes the lock. Re-installing an unchanged tree is a
/// byte-identical lock; reinstalling over drift is an explicit repair,
/// flagged in the report.
pub fn install(input: &InstallInput<'_>) -> Result<InstallReport, RolesError> {
    if input.harness.is_empty() {
        return Err(RolesError::HarnessUnspecified);
    }
    let Some(surface) = input.registry.surface(input.harness) else {
        return Err(RolesError::UnknownHarness {
            id: input.harness.to_owned(),
        });
    };
    let tree = load_role_tree(
        input.roles_dir,
        input.project_root,
        input.env,
        input.tool_vocabulary,
    )?;

    // Containment BEFORE any byte is written: every write target derives
    // from validated roots, and the role files' own write_paths may not
    // aim outside the project or state roots.
    let installed_dir = input
        .state_dir
        .join(STATE_TREE_NAME)
        .join(surface.dir_name())
        .join(input.harness);
    if !ensure_within_root(&installed_dir, input.state_dir) {
        return Err(RolesError::Io {
            locus: "resolving the installed directory",
            path: installed_dir.to_string_lossy().into_owned(),
            kind: std::io::ErrorKind::InvalidInput,
        });
    }
    for (_, _, definition) in &tree {
        for entry in &definition.permissions().write_paths {
            // COR-D03: validate the RESOLVED shape of every declared write
            // path. Absolute entries resolve to themselves; RELATIVE entries
            // anchor at the project root for validation — conservative, the
            // installer cannot see the dispatch-time workspace anchor a
            // relative entry will truly ride. Anchoring also closes the
            // relative-traversal hole: `../sibling` resolves outside the
            // roots and is refused as an escape like any absolute one. The
            // refusal echoes the RAW token; the resolved value never reaches
            // error text (COR-R1-2, spec §8.1).
            let resolved = if Path::new(entry.resolved()).is_absolute() {
                PathBuf::from(entry.resolved())
            } else {
                input.project_root.join(entry.resolved())
            };
            if !ensure_within_root(&resolved, input.project_root)
                && !ensure_within_root(&resolved, input.state_dir)
            {
                return Err(RolesError::WritePathEscape {
                    role: definition.stem().to_owned(),
                    path: entry.raw().to_owned(),
                });
            }
            // §17.5.5's posture for the in-project protected set: the
            // ruler surfaces sit outside every builder-writable path, so a
            // write path INSIDE a protected anchor or COVERING one (a
            // lexical prefix of it — the project root, the runtime dir)
            // is a typed refusal naming the anchor.
            if let Some(protected) = protected_path_covered(&resolved, input.project_root) {
                return Err(RolesError::WritePathProtected {
                    role: definition.stem().to_owned(),
                    path: entry.raw().to_owned(),
                    protected,
                });
            }
        }
    }

    std::fs::create_dir_all(&installed_dir).map_err(|error| RolesError::Io {
        locus: "creating the installed directory",
        path: installed_dir.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    let mut entries = Vec::new();
    // COR-R1-4: the repair flag is drift-relative — it reports TRUE when an
    // EXISTING installed artifact's bytes were actually rewritten during
    // this install (compared before/after per role), never merely because
    // lock bytes differ. Materializing a NEW artifact is publishing, not
    // repair: a first install repairs nothing and says so. A reinstall over
    // out-of-band installed drift reports the repair it performed; a
    // byte-identical reinstall repairs nothing and says so.
    let mut repaired = false;
    for (stem, bytes, definition) in &tree {
        let file_name = format!("{stem}.md");
        let target = installed_dir.join(&file_name);
        let before = std::fs::read(&target).ok();
        write_atomic(&target, bytes)?;
        if before
            .as_deref()
            .is_some_and(|previous| previous != bytes.as_slice())
        {
            repaired = true;
        }
        let source_rel = rel_to(input.project_root, &input.roles_dir.join(&file_name));
        let installed_rel = rel_to(
            input.state_dir,
            &Path::new(STATE_TREE_NAME)
                .join(surface.dir_name())
                .join(input.harness)
                .join(&file_name),
        );
        entries.push(LockEntry {
            role: stem.clone(),
            role_id: definition.role().name().to_owned(),
            dimension: definition.dimension().map(str::to_owned),
            source: source_rel,
            installed: installed_rel,
            // The prefixed digest token — the parser's strict shape (a
            // bare-hex digest here is a lock-schema failure at verify).
            digest: format!("sha256:{}", sha256_hex(bytes)),
        });
    }
    let lock = RolesLock {
        harness: input.harness.to_owned(),
        enforcement: Enforcement::from_surface(surface),
        entries,
    };
    let lock_path = defaults::runtime_dir_at(input.project_root).join(ROLES_LOCK_NAME);
    std::fs::create_dir_all(lock_path.parent().ok_or_else(|| RolesError::Io {
        locus: "resolving the lock directory",
        path: lock_path.to_string_lossy().into_owned(),
        kind: std::io::ErrorKind::InvalidInput,
    })?)
    .map_err(|error| RolesError::Io {
        locus: "creating the lock directory",
        path: lock_path.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    let serialized = lock.serialize();
    write_atomic(&lock_path, serialized.as_bytes())?;
    Ok(InstallReport {
        harness: input.harness.to_owned(),
        enforcement: lock.enforcement,
        lock_path,
        roles: lock.entries.len(),
        repaired,
    })
}

/// Re-reads the installed state and the repository sources, and
/// compares CONTENT digests against the lock: any divergence — an
/// out-of-band edit to installed state, a source edit since install, a
/// missing file, or a hand-edited or re-forged lock — is a typed
/// configuration failure.
/// What one installed role is, listed read-only (`gauntlet roles list`,
/// COR-D1 / §20.1's `roles list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedRole {
    /// The install identifier (the source file's stem).
    pub role: String,
    /// The declared role identity token.
    pub role_id: String,
    /// The critic fan-out dimension, when the stem names one.
    pub dimension: Option<String>,
}

/// A read-only enumeration of the INSTALLED roles, from the roles lock the
/// installer wrote (`.gauntlet/roles.lock`), strict-parsed so a tampered or
/// re-forged lock is the same typed refusal `verify` raises — a list that
/// trusted an unverified file would be a forged inventory. Read-only: no
/// source or installed byte is read, no digest compared, no state written;
/// drift detection remains `verify`'s job.
pub fn list(project_root: &Path) -> Result<ListReport, RolesError> {
    let lock_path = defaults::runtime_dir_at(project_root).join(ROLES_LOCK_NAME);
    let bytes = std::fs::read(&lock_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RolesError::LockMissing {
                path: lock_path.to_string_lossy().into_owned(),
            }
        } else {
            RolesError::Io {
                locus: "reading the roles lock",
                path: lock_path.to_string_lossy().into_owned(),
                kind: error.kind(),
            }
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|_| RolesError::NotUtf8 {
        path: lock_path.to_string_lossy().into_owned(),
    })?;
    let lock = RolesLock::parse(&text)?;
    Ok(ListReport {
        harness: lock.harness,
        enforcement: lock.enforcement,
        roles: lock
            .entries
            .into_iter()
            .map(|entry| ListedRole {
                role: entry.role,
                role_id: entry.role_id,
                dimension: entry.dimension,
            })
            .collect(),
    })
}

/// What one list found when the lock parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListReport {
    pub harness: String,
    pub enforcement: Enforcement,
    pub roles: Vec<ListedRole>,
}

pub fn verify(input: &VerifyInput<'_>) -> Result<VerifyReport, RolesError> {
    let lock_path = defaults::runtime_dir_at(input.project_root).join(ROLES_LOCK_NAME);
    let bytes = std::fs::read(&lock_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RolesError::LockMissing {
                path: lock_path.to_string_lossy().into_owned(),
            }
        } else {
            RolesError::Io {
                locus: "reading the roles lock",
                path: lock_path.to_string_lossy().into_owned(),
                kind: error.kind(),
            }
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|_| RolesError::NotUtf8 {
        path: lock_path.to_string_lossy().into_owned(),
    })?;
    let lock = RolesLock::parse(&text)?;

    // COR-R1-1: the lock's recorded paths are DERIVED, never trusted. Verify
    // rebuilds each entry's expected source and installed paths from the
    // values it already trusts — the configured roles dir, the configured
    // state root, the registered harness surface, the enforcement level, and
    // the role stem — and refuses any divergence BEFORE any digest
    // comparison: a digest-valid but shape-invalid lock is a tamper refusal,
    // never a drift verdict. The cryptographic anchor is the reviewed source
    // tree; the lock's own path opinions are claims, not anchors.
    let Some(surface) = input.registry.surface(&lock.harness) else {
        return Err(RolesError::LockTampered {
            detail: format!(
                "the lock names harness '{}' which is not registered — install only writes \
                 registered ids, so the label is hand-forged, never runtime-written",
                lock.harness
            ),
        });
    };
    if Enforcement::from_surface(surface) != lock.enforcement {
        return Err(RolesError::LockTampered {
            detail: format!(
                "the lock records enforcement '{}' but harness '{}' is registered with the \
                 '{}' surface — a cross-harness relabeled record is hand-forged, never \
                 runtime-written",
                lock.enforcement.token(),
                lock.harness,
                Enforcement::from_surface(surface).token()
            ),
        });
    }
    let harness_dir = Path::new(STATE_TREE_NAME)
        .join(surface.dir_name())
        .join(&lock.harness);
    for entry in &lock.entries {
        let file_name = format!("{}.md", entry.role);
        let expected_source = rel_to(input.project_root, &input.roles_dir.join(&file_name));
        if entry.source != expected_source {
            return Err(RolesError::LockTampered {
                detail: format!(
                    "entry '{}' records source '{}' but the configured roles dir derives \
                     '{} — lock-recorded paths are derived, never trusted",
                    entry.role, entry.source, expected_source
                ),
            });
        }
        let expected_installed = rel_to(input.state_dir, &harness_dir.join(&file_name));
        if entry.installed != expected_installed {
            return Err(RolesError::LockTampered {
                detail: format!(
                    "entry '{}' records installed '{}' but the state root and the registered \
                     surface derive '{} — lock-recorded paths are derived, never trusted",
                    entry.role, entry.installed, expected_installed
                ),
            });
        }
    }

    let mut drifts = Vec::new();
    for entry in &lock.entries {
        let source_path = contained_join(input.project_root, &entry.source)?;
        let installed_path = contained_join(input.state_dir, &entry.installed)?;
        let source_bytes = match std::fs::read(&source_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drifts.push(DriftRecord {
                    role: entry.role.clone(),
                    kind: DriftKind::SourceMissing,
                    expected: entry.digest.clone(),
                    actual: None,
                });
                None
            }
            Err(error) => {
                return Err(RolesError::Io {
                    locus: "reading a source role file",
                    path: source_path.to_string_lossy().into_owned(),
                    kind: error.kind(),
                });
            }
        };
        if let Some(bytes) = source_bytes {
            let actual = format!("sha256:{}", sha256_hex(&bytes));
            if actual != entry.digest {
                drifts.push(DriftRecord {
                    role: entry.role.clone(),
                    kind: DriftKind::SourceModified,
                    expected: entry.digest.clone(),
                    actual: Some(actual),
                });
            }
        }
        let installed_bytes = match std::fs::read(&installed_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drifts.push(DriftRecord {
                    role: entry.role.clone(),
                    kind: DriftKind::InstalledMissing,
                    expected: entry.digest.clone(),
                    actual: None,
                });
                None
            }
            Err(error) => {
                return Err(RolesError::Io {
                    locus: "reading an installed role artifact",
                    path: installed_path.to_string_lossy().into_owned(),
                    kind: error.kind(),
                });
            }
        };
        if let Some(bytes) = installed_bytes {
            let actual = format!("sha256:{}", sha256_hex(&bytes));
            if actual != entry.digest {
                drifts.push(DriftRecord {
                    role: entry.role.clone(),
                    kind: DriftKind::InstalledModified,
                    expected: entry.digest.clone(),
                    actual: Some(actual),
                });
            }
        }
    }

    // Set-membership drift (COR-D04): the lock's entry list is the complete
    // record of BOTH directories. An out-of-band ADDITION — a `.md` dropped
    // into the installed surface (Annex C.4: harness role state is
    // agent-mutable) or a new source file after install (stale install) —
    // is invisible to the per-entry digest loop above, so both directories
    // are listed and any `.md` the lock records nothing about is drift BY
    // NAME. Extra files are reported, never loaded: a drifted tree is not
    // parsed as configuration. Both directory roots are DERIVED surface
    // roots — the entry records were already checked (COR-R1-1) to equal
    // the paths the configured surface derives, so the record is a verified
    // mirror of the roots, not an independent anchor — and an unreadable
    // directory needs no separate report — the per-entry reads above have
    // already surfaced it as missing files or a typed io failure.
    let recorded: Vec<&str> = lock.entries.iter().map(|e| e.role.as_str()).collect();
    let parent_of = |relative: &str| -> Option<String> {
        Path::new(relative)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| {
                parent
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            })
    };
    if let Some(installed_dir_rel) = lock.entries.first().and_then(|e| parent_of(&e.installed)) {
        let installed_dir = contained_join(input.state_dir, &installed_dir_rel)?;
        for stem in md_stems(&installed_dir)? {
            if !recorded.contains(&stem.as_str()) {
                drifts.push(DriftRecord {
                    role: stem,
                    kind: DriftKind::InstalledExtra,
                    expected: "no lock record".to_owned(),
                    actual: Some("present on the installed surface".to_owned()),
                });
            }
        }
    }
    if let Some(source_dir_rel) = lock.entries.first().and_then(|e| parent_of(&e.source)) {
        let source_dir = contained_join(input.project_root, &source_dir_rel)?;
        for stem in md_stems(&source_dir)? {
            if !recorded.contains(&stem.as_str()) {
                drifts.push(DriftRecord {
                    role: stem,
                    kind: DriftKind::SourceExtra,
                    expected: "no lock record".to_owned(),
                    actual: Some("present in the roles source".to_owned()),
                });
            }
        }
    }
    if !drifts.is_empty() {
        return Err(RolesError::Drift { entries: drifts });
    }
    Ok(VerifyReport {
        harness: lock.harness,
        enforcement: lock.enforcement,
        roles: lock.entries.len(),
    })
}

/// Joins a lock-recorded relative path onto its root, refusing any
/// record that would escape (absolute records, or traversal segments).
/// A forged lock cannot aim verify at files outside the roots.
fn contained_join(root: &Path, relative: &str) -> Result<PathBuf, RolesError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() || !ensure_within_root(&root.join(relative_path), root) {
        return Err(RolesError::LockTampered {
            detail: format!(
                "a recorded path '{relative}' escapes its root — locks are runtime-written \
                 documents, never hand-edited ones"
            ),
        });
    }
    Ok(root.join(relative_path))
}

/// The `.md` file stems a directory carries, sorted — the set-membership
/// half of verify (COR-D04). `Ok(vec![])` when the directory is gone: the
/// per-entry reads have already reported the missing files, so the
/// membership scan has nothing separate to say. Any other read failure is
/// the caller's typed io error, never silently skipped.
fn md_stems(dir: &Path) -> Result<Vec<String>, RolesError> {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(RolesError::Io {
                locus: "listing a roles directory for set-membership drift",
                path: dir.to_string_lossy().into_owned(),
                kind: error.kind(),
            });
        }
    };
    let mut stems = Vec::new();
    for item in read {
        let item = item.map_err(|error| RolesError::Io {
            locus: "reading a roles directory entry",
            path: dir.to_string_lossy().into_owned(),
            kind: error.kind(),
        })?;
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            stems.push(stem.to_owned());
        }
    }
    stems.sort();
    Ok(stems)
}

/// Lexical containment: the child sits under the root, with no
/// traversal segment anywhere. Lexical on purpose — the installed tree
/// may not exist yet, and a resolved symlink check is the sandbox's
/// job, not the installer's (containment gates how contained a run is;
/// this is the installer's own write discipline).
fn ensure_within_root(child: &Path, root: &Path) -> bool {
    if !child.is_absolute() || !root.is_absolute() {
        return false;
    }
    if child
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return false;
    }
    child.starts_with(root)
}

/// The in-project protected set (COR-D03; spec §17.5.5: "The evidence
/// store, state store, frozen reference, and answer key are outside every
/// builder-writable path", with §10.2's Never column naming the comparison
/// contract and the roles lock the runtime's own record). Every anchor is
/// DERIVED, never spelled: the ruler directory is the answer key's own
/// parent as the defaults layer resolves it, and the runtime-dir anchors
/// are built from the same single-segment constants the defaults module
/// owns — so the protected set cannot drift from the configured surface,
/// and no joined path literal exists here (spec §8.1). The ruler dir and
/// the policies dir are DIRECTORY anchors — covering them or anything
/// under them is the same refusal as covering a file anchor.
fn protected_anchors(project_root: &Path) -> Vec<(String, PathBuf)> {
    let runtime = project_root.join(defaults::RUNTIME_DIR_NAME);
    let leaf = |name: &str| {
        (
            format!("{}/{name}", defaults::RUNTIME_DIR_NAME),
            runtime.join(name),
        )
    };
    let mut anchors = vec![
        leaf(defaults::REFERENCE_LOCK_NAME),
        leaf(defaults::COMPARISON_CONTRACT_NAME),
        leaf(defaults::POLICIES_DIR_NAME),
        leaf(ROLES_LOCK_NAME),
    ];
    if let Some(ruler_dir) = defaults::answer_key_at(project_root).parent() {
        anchors.insert(
            0,
            (rel_to(project_root, ruler_dir), ruler_dir.to_path_buf()),
        );
    }
    anchors
}

/// The protected anchor a resolved write path sits inside or covers (a
/// lexical prefix of), if any. "Covers" is the load-bearing direction: the
/// project root and the runtime dir are not themselves spelled in any
/// write_paths entry, but they cover every protected anchor, so declaring
/// either is declaring the ruler.
fn protected_path_covered(resolved: &Path, project_root: &Path) -> Option<String> {
    protected_anchors(project_root)
        .into_iter()
        .find_map(|(label, protected)| {
            (resolved.starts_with(&protected) || protected.starts_with(resolved)).then_some(label)
        })
}

/// Renders a path relative to its (pre-validated) root with forward
/// slashes, so the lock is host-portable text.
fn rel_to(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// The one hash idiom: SHA-256 as lowercase hex, the domain `Digest`
/// hex grammar.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes).iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Atomic file write (write beside, fsync, rename, fsync the parent): a
/// reader never sees a torn artifact, the staged bytes are flushed to the
/// file before the rename commits them, and the RENAME's directory entry is
/// flushed after — the §10.3 result-document discipline applied to the
/// installer's own outputs. The directory-entry half closes residual
/// R17-note-1 (plan Task 20 decision D10): the file fsync alone is not
/// crash-durable for a NEWLY CREATED file, because the durability of the
/// entry naming it lives in the parent directory, not the file's own bytes.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RolesError> {
    let staging = path.with_extension("staging");
    std::fs::write(&staging, bytes).map_err(|error| RolesError::Io {
        locus: "writing a role artifact",
        path: staging.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    // File durability half (COR-D05): fsync the staged bytes through a
    // fresh handle BEFORE the rename, so a crash right after the commit
    // leaves the target carrying its full contents. A read-only handle is
    // sufficient — fsync flushes the file, not the fd's write permission.
    std::fs::File::open(&staging)
        .and_then(|file| file.sync_all())
        .map_err(|error| RolesError::Io {
            locus: "fsyncing a staged role artifact",
            path: staging.to_string_lossy().into_owned(),
            kind: error.kind(),
        })?;
    std::fs::rename(&staging, path).map_err(|error| RolesError::Io {
        locus: "committing a role artifact",
        path: path.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    // Directory-entry durability half (R17-note-1 closed): fsync the PARENT
    // DIRECTORY so the rename's entry survives a crash after the commit. A
    // read-only handle on the directory is sufficient — fsync flushes the
    // directory's entries, not the fd's write permission — and `File::open`
    // IS the read-only open: it takes no write handle, so it stays outside
    // the single-writer gate's write-API inventory by design.
    let parent = path.parent().ok_or_else(|| RolesError::Io {
        locus: "locating a role artifact's parent directory",
        path: path.to_string_lossy().into_owned(),
        kind: std::io::ErrorKind::InvalidInput,
    })?;
    std::fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|error| RolesError::Io {
            locus: "fsyncing a role artifact's directory entry",
            path: parent.to_string_lossy().into_owned(),
            kind: error.kind(),
        })?;
    directory_entry_fsync_sensor::record();
    Ok(())
}

/// The directory-entry fsync's call sensor — the mutation tooth's
/// observability seam. TWO cfg-disjoint twin modules bind ONE name:
/// production builds bind a no-op (the production path stays
/// machine-code untouched); test builds bind the counting sensor the
/// tooth reads. Shape note: the single-writer gate's blanker classifies
/// gated MODULES (and functions), never gated statics or statements
/// (I8, fail-closed) — so the sensor lives in gated modules, not behind
/// a `#[cfg(test)]` static or a gated statement.
#[cfg(not(test))]
mod directory_entry_fsync_sensor {
    /// Production twin: nothing to count, nothing to read.
    pub(super) fn record() {}
}

#[cfg(test)]
mod directory_entry_fsync_sensor {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub(super) static CALLS: AtomicU64 = AtomicU64::new(0);

    pub(super) fn record() {
        CALLS.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MUTATION TOOTH for R17-note-1's closure (plan Task 20 D10): the
    /// directory-entry fsync must happen on EVERY `write_atomic` commit.
    /// WHY A CALL-COUNTER AND NOT A KERNEL OBSERVATION: no std API can read
    /// back whether a past fsync reached the platter — an in-process test
    /// that "checks durability" would be theater. The honest tooth pins the
    /// strongest in-process observable: that the commit path invoked the
    /// parent-directory fsync exactly once per write. A mutant that removes
    /// the `sync_all` on the parent directory (or opens the wrong node)
    /// leaves the counter unmoved and this test RED; a mutant that fsyncs
    /// but mis-renders the error locus is caught by the success-path
    /// assertions below. The file-level fsync half keeps its own guard: the
    /// staged bytes must be gone (renamed, not copied) and the content
    /// verbatim, so a mutant that degrades the whole helper to
    /// `std::fs::write` also goes red here.
    #[test]
    fn write_atomic_fsyncs_the_directory_entry() {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-write-atomic-tooth-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("the tooth scratch directory is creatable");
        let target = root.join("artifact.md");
        let before = directory_entry_fsync_sensor::CALLS.load(std::sync::atomic::Ordering::Relaxed);
        write_atomic(&target, b"verbatim bytes")
            .expect("the atomic write succeeds on a creatable path");
        let after = directory_entry_fsync_sensor::CALLS.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            after - before,
            1,
            "exactly one directory-entry fsync per committed write — a write_atomic that \
             skips the parent-directory fsync is a durability regression (R17-note-1)"
        );
        assert_eq!(
            std::fs::read(&target).expect("the committed artifact reads"),
            b"verbatim bytes",
            "the committed artifact is the bytes, verbatim"
        );
        assert!(
            !root.join("artifact.staging").exists(),
            "the staging file is renamed over, never left behind"
        );
        // A second write to the SAME path (the lock reinstall path) commits
        // through the same sequence.
        write_atomic(&target, b"second commit").expect("the overwrite succeeds");
        let final_count =
            directory_entry_fsync_sensor::CALLS.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            final_count - before,
            2,
            "every commit fsyncs the directory entry, not just the first"
        );
        std::fs::remove_dir_all(&root).expect("the tooth scratch is removable");
    }
}
