//! The `.gauntlet/reference.lock` reader and the disk-vs-lock digest
//! check (plan Task 20): the typed twin of `scripts/verify-ruler.sh`,
//! reused by `gauntlet reference verify` and by `gauntlet run
//! --dry-run`'s preflight (charter D1: the preview verifies the
//! reference WITHOUT freezing — zero writes).
//!
//! The lock's ground truth is its recorded digests (the file's own note:
//! "this lock file is the ground truth digest is checked against, not
//! memory or a rerun of judgement"); this module recomputes the
//! application normalization digest over the CURRENT answer key and
//! manifest and compares. A moved, edited, or unreadable artifact is a
//! typed failure; nothing here ever writes.

use gauntlet_app::config::defaults;
use gauntlet_ports::json::JsonValue;

/// Why the reference lock check failed — typed, carrying both digests
/// where a comparison happened (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum LockError {
    /// The lock file is missing, unreadable, or not its schema.
    Lock { detail: String },
    /// The current artifact is unreadable or does not digest.
    Artifact { detail: String },
    /// The answer key's current digest differs from the frozen one —
    /// the I1 tripwire.
    AnswerKeyMutated { expected: String, actual: String },
    /// The manifest's current digest differs from the frozen one.
    ManifestMutated { expected: String, actual: String },
}

impl core::fmt::Display for LockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lock { detail } => write!(f, "the reference lock is unusable: {detail}"),
            Self::Artifact { detail } => {
                write!(
                    f,
                    "a frozen artifact is unusable against the lock: {detail}"
                )
            }
            Self::AnswerKeyMutated { expected, actual } => write!(
                f,
                "the answer key MUTATED against the reference lock: frozen {expected}, current \
                 {actual} — the ruler has moved (spec 9.4, I1)"
            ),
            Self::ManifestMutated { expected, actual } => write!(
                f,
                "the reference manifest MUTATED against the reference lock: frozen {expected}, \
                 current {actual}"
            ),
        }
    }
}

impl std::error::Error for LockError {}

/// The verified lock's facts (what `reference verify` prints).
#[derive(Debug, Clone, PartialEq)]
pub struct LockReport {
    pub answer_key_path: String,
    pub manifest_path: String,
    pub answer_key_digest: String,
    pub manifest_digest: String,
    pub unit_count: Option<u64>,
}

/// Verifies the project's reference lock against the artifacts on disk.
pub fn verify_lock(project_root: &std::path::Path) -> Result<LockReport, LockError> {
    let lock_path = defaults::reference_lock_at(project_root);
    let text = std::fs::read_to_string(&lock_path).map_err(|error| LockError::Lock {
        detail: format!("reading {}: {error}", lock_path.display()),
    })?;
    let parsed = gauntlet_ports::json::parse(&text).map_err(|error| LockError::Lock {
        detail: error.to_string(),
    })?;
    let string_field = |key: &str| -> Result<String, LockError> {
        match parsed.get(key) {
            Some(JsonValue::Str(text)) => Ok(text.clone()),
            _ => Err(LockError::Lock {
                detail: format!("the lock carries no string field {key:?}"),
            }),
        }
    };
    let key_rel = string_field("answer_key_path")?;
    let manifest_rel = string_field("manifest_path")?;
    let key_digest = string_field("answer_key_digest")?;
    let manifest_digest = string_field("manifest_digest")?;
    let unit_count = match parsed.get("unit_count") {
        Some(JsonValue::Number(number)) => number.as_u64(),
        _ => None,
    };
    let answer_key_path = project_root.join(&key_rel);
    let manifest_path = project_root.join(&manifest_rel);

    // The current artifacts' digests — the SAME normalization the CLI's
    // `reference digest` applies (two implementations of the same rules
    // agreeing is the determinism proof; here both are the application's).
    let current = |path: &std::path::Path| -> Result<String, LockError> {
        let bytes = std::fs::read(path).map_err(|error| LockError::Artifact {
            detail: format!("reading {}: {error}", path.display()),
        })?;
        gauntlet_app::reference::normalize::digest_of(&bytes)
            .map(|digest| digest.to_string())
            .map_err(|error| LockError::Artifact {
                detail: format!("{} does not digest: {error}", path.display()),
            })
    };
    let current_key = current(&answer_key_path)?;
    if current_key != key_digest {
        return Err(LockError::AnswerKeyMutated {
            expected: key_digest,
            actual: current_key,
        });
    }
    let current_manifest = current(&manifest_path)?;
    if current_manifest != manifest_digest {
        return Err(LockError::ManifestMutated {
            expected: manifest_digest,
            actual: current_manifest,
        });
    }
    Ok(LockReport {
        answer_key_path: key_rel,
        manifest_path: manifest_rel,
        answer_key_digest: key_digest,
        manifest_digest,
        unit_count,
    })
}
