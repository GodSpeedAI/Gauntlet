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
//! must live inside the state root, and a `permissions.write_paths`
//! entry that resolves to an ABSOLUTE path outside the project or state
//! roots is a typed refusal — a role file cannot point its own writes
//! at the operator's machine.
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
//! the source tree, not the lock's opinion of itself.
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
/// Paths are operator-visible configuration, not secrets; no resolved
/// indirection VALUE is ever carried.
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
    /// A write_paths entry resolves to an absolute path outside the
    /// project and state roots — a role file steering its own writes at
    /// the operator's machine is refused.
    WritePathEscape { role: String, path: String },
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
}

impl core::fmt::Display for DriftKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceMissing => f.write_str("source file missing"),
            Self::SourceModified => f.write_str("source file modified since install"),
            Self::InstalledMissing => f.write_str("installed artifact missing"),
            Self::InstalledModified => f.write_str("installed artifact modified out-of-band"),
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
                "roles: role '{role}' declares a write path resolving to '{path}', outside \
                 the project and state roots — a role file cannot aim its own writes at the \
                 operator's machine"
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

/// Install inputs: the resolved roots, the explicit harness id, and the
/// registry (built by the composition root — a data map, never role
/// logic spread across match arms).
pub struct InstallInput<'a> {
    pub project_root: &'a Path,
    pub state_dir: &'a Path,
    pub roles_dir: &'a Path,
    pub harness: &'a str,
    pub registry: &'a HarnessRegistry,
    pub env: &'a EnvLayer,
}

/// What one install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub harness: String,
    pub enforcement: Enforcement,
    pub lock_path: PathBuf,
    pub roles: usize,
    /// True when a previous lock existed and differed — this install
    /// was an explicit REPAIR, recorded as such rather than silent.
    pub repaired: bool,
}

/// Verifier inputs: the roots the lock's relative paths hang from.
pub struct VerifyInput<'a> {
    pub project_root: &'a Path,
    pub state_dir: &'a Path,
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
            RoleDefinition::load(&stem, &text, env).map_err(|source| RolesError::Role {
                role: stem.clone(),
                source,
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
    let tree = load_role_tree(input.roles_dir, input.project_root, input.env)?;

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
            let path = Path::new(entry);
            if path.is_absolute()
                && !ensure_within_root(path, input.project_root)
                && !ensure_within_root(path, input.state_dir)
            {
                return Err(RolesError::WritePathEscape {
                    role: definition.stem().to_owned(),
                    path: entry.clone(),
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
    for (stem, bytes, definition) in &tree {
        let file_name = format!("{stem}.md");
        let target = installed_dir.join(&file_name);
        write_atomic(&target, bytes)?;
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
    let previous = std::fs::read(&lock_path).ok();
    let repaired = previous
        .as_ref()
        .is_some_and(|old| old.as_slice() != serialized.as_bytes());
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

/// Atomic file write (write beside, rename over): a reader never sees a
/// torn artifact — the §10.3 result-document discipline applied to the
/// installer's own outputs.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RolesError> {
    let staging = path.with_extension("staging");
    std::fs::write(&staging, bytes).map_err(|error| RolesError::Io {
        locus: "writing a role artifact",
        path: staging.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    std::fs::rename(&staging, path).map_err(|error| RolesError::Io {
        locus: "committing a role artifact",
        path: path.to_string_lossy().into_owned(),
        kind: error.kind(),
    })?;
    Ok(())
}
