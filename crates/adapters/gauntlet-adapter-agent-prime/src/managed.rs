//! Optional managed dependency support (WP-63): the harness is OPTIONAL,
//! never bundled and never installed silently. This module owns the
//! vendor-aware half of that promise:
//!
//! - **Detection** ([`detect`]): an explicit path (the [`BINARY_ENV`]
//!   variable) wins; otherwise the first match on `PATH`. A path that is
//!   set but unusable is an error, never a silent fall-through to a
//!   different binary. The report carries the location, where it came
//!   from, and the version the binary itself reports.
//! - **Compatibility**: the supported range is INJECTED (its single source
//!   of truth is the application defaults module, the same range the
//!   version probe has always taken). An incompatible install is refused
//!   with a message naming the found and the supported versions.
//! - **Opt-in setup** ([`manual_instructions`], [`verify_installer`],
//!   [`run_installer`]): this crate never downloads anything. The
//!   upstream distribution is a hosted installer script on a floating
//!   release channel, so there is no artifact whose bytes Gauntlet could
//!   pin by itself. Setup therefore prints the exact manual steps, and,
//!   only when the operator hands over an installer they obtained and
//!   reviewed together with its SHA-256, verifies that digest and (after
//!   the composition root's explicit confirmation) runs it pinned to the
//!   supported version, into a user-scoped prefix.
//!
//! Absence of the harness never affects any other adapter: nothing here
//! runs unless a Prime-dependent feature or `gauntlet agent ...` asks.

use core::fmt;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use gauntlet_ports::deadline::Deadline;
use sha2::{Digest, Sha256};

use crate::config::DEFAULT_BINARY_NAME;
use crate::probe::{ProbeError, VersionRange, probe};

/// The explicit-location variable: the full path of the harness binary.
/// It is the harness's own namespace (like its other `PRIME_AGENT_*`
/// variables), not a Gauntlet configuration key.
pub const BINARY_ENV: &str = "PRIME_AGENT_BIN";

/// The names `gauntlet agent setup <name>` accepts for this adapter.
pub const SETUP_NAMES: &[&str] = &["prime", "prime-agent"];

/// Whether `name` selects this adapter for `gauntlet agent setup`.
pub fn is_setup_name(name: &str) -> bool {
    SETUP_NAMES.contains(&name)
}

/// The official one-command installer (upstream README, "Install").
pub const OFFICIAL_INSTALLER_URL: &str = "https://app.primeintellect.ai/prime-agent/install.sh";
/// The official Windows installer (upstream README, "Install").
pub const OFFICIAL_INSTALLER_URL_WINDOWS: &str =
    "https://app.primeintellect.ai/prime-agent/install.ps1";
/// The upstream project page, where releases and source live.
pub const OFFICIAL_PROJECT_URL: &str = "https://github.com/PrimeIntellect-ai/prime-agent";

/// The installer's version-pin variable.
const INSTALLER_VERSION_ENV: &str = "PRIME_AGENT_VERSION";
/// The installer's user-scoped prefix variable.
const INSTALLER_PREFIX_ENV: &str = "PRIME_AGENT_RUST_PREFIX";

/// How a detected binary was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    /// The [`BINARY_ENV`] path.
    Explicit,
    /// The first matching entry on `PATH`.
    Path,
}

impl fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit => write!(f, "explicit path ({BINARY_ENV})"),
            Self::Path => write!(f, "PATH"),
        }
    }
}

/// A usable, compatible install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub binary: PathBuf,
    pub source: DetectionSource,
    pub version: (u64, u64, u64),
}

impl Detection {
    /// One line for humans: version and location.
    pub fn summary(&self) -> String {
        format!(
            "{} at {} (found via {})",
            format_version(self.version),
            self.binary.display(),
            self.source
        )
    }
}

/// Why no usable install was found. Every variant is actionable text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// [`BINARY_ENV`] is set but names no file.
    ExplicitMissing { path: PathBuf },
    /// Nothing on `PATH` (and no explicit path set).
    Absent,
    /// A binary was found but would not report a version.
    Unusable { binary: PathBuf, detail: String },
    /// A binary was found and its version is outside the supported range.
    Incompatible {
        binary: PathBuf,
        found: (u64, u64, u64),
        range: VersionRange,
    },
}

impl DetectError {
    /// True when no install exists at all (as opposed to a broken one).
    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent | Self::ExplicitMissing { .. })
    }
}

const SETUP_HINT: &str = "Prime Agent is optional; every other adapter keeps working. \
                          Run `gauntlet agent setup prime` for install steps";

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitMissing { path } => write!(
                f,
                "Prime Agent not found: {BINARY_ENV} points at {}, which is not a file. \
                 Fix or unset {BINARY_ENV}. {SETUP_HINT}.",
                path.display()
            ),
            Self::Absent => write!(
                f,
                "Prime Agent not found: no `{DEFAULT_BINARY_NAME}` on PATH and {BINARY_ENV} is \
                 not set. {SETUP_HINT}."
            ),
            Self::Unusable { binary, detail } => write!(
                f,
                "Prime Agent at {} could not report its version: {detail}. Reinstall it or \
                 point {BINARY_ENV} at a working binary. {SETUP_HINT}.",
                binary.display()
            ),
            Self::Incompatible {
                binary,
                found,
                range,
            } => write!(
                f,
                "Prime Agent at {} is version {} but Gauntlet supports {range}. Install a \
                 supported version, or point {BINARY_ENV} at one. {SETUP_HINT}.",
                binary.display(),
                format_version(*found),
            ),
        }
    }
}

impl std::error::Error for DetectError {}

fn format_version(version: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// Detects the harness: `explicit` (the [`BINARY_ENV`] value, empty means
/// unset) first, then the first file named `prime-agent` on `path_env`.
/// The located binary is asked for its version and checked against
/// `range`; `deadline` bounds that one `--version` run.
pub fn detect(
    explicit: Option<&OsStr>,
    path_env: Option<&OsStr>,
    range: &VersionRange,
    deadline: Deadline,
) -> Result<Detection, DetectError> {
    let (binary, source) = match explicit.filter(|value| !value.is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_file() {
                return Err(DetectError::ExplicitMissing { path });
            }
            (path, DetectionSource::Explicit)
        }
        None => (
            find_on_path(DEFAULT_BINARY_NAME, path_env).ok_or(DetectError::Absent)?,
            DetectionSource::Path,
        ),
    };
    match probe(&binary, range, deadline) {
        Ok(report) => Ok(Detection {
            binary: report.binary,
            source,
            version: report.version,
        }),
        Err(ProbeError::VersionUnsupported {
            binary,
            found,
            range,
        }) => Err(DetectError::Incompatible {
            binary,
            found,
            range,
        }),
        Err(ProbeError::BinaryAbsent { binary }) => {
            Err(DetectError::ExplicitMissing { path: binary })
        }
        Err(other @ (ProbeError::VersionUnparseable { .. } | ProbeError::ProbeFailed { .. })) => {
            Err(DetectError::Unusable {
                binary,
                detail: other.to_string(),
            })
        }
    }
}

/// [`detect`] over the process environment.
pub fn detect_from_env(range: &VersionRange, deadline: Deadline) -> Result<Detection, DetectError> {
    let explicit = std::env::var_os(BINARY_ENV);
    let path_env = std::env::var_os("PATH");
    detect(explicit.as_deref(), path_env.as_deref(), range, deadline)
}

fn find_on_path(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path_env?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// A [`PrimeAdapterConfig`](crate::PrimeAdapterConfig) whose binary is the
/// detected, compatible install. The composition root calls this before
/// building the runner, so a missing or incompatible harness fails with
/// the actionable [`DetectError`] instead of a spawn failure mid-run.
pub fn checked_config(
    range: &VersionRange,
    deadline: Deadline,
) -> Result<crate::PrimeAdapterConfig, DetectError> {
    let detection = detect_from_env(range, deadline)?;
    Ok(crate::PrimeAdapterConfig {
        binary: detection.binary,
        ..crate::PrimeAdapterConfig::default()
    })
}

/// The exact manual steps. Nothing here runs; this is text.
pub fn manual_instructions(pinned: (u64, u64, u64), range: &VersionRange) -> String {
    let pin = format_version(pinned);
    format!(
        "Prime Agent is an optional dependency. Gauntlet does not bundle it and never \
         downloads or installs it on its own.\n\
         Supported versions: {range}. Recommended pin: {pin}.\n\
         \n\
         Official distribution: {OFFICIAL_PROJECT_URL}\n\
         \n\
         Manual install (Linux / macOS):\n\
         \x20 1. Download the installer: {OFFICIAL_INSTALLER_URL}\n\
         \x20 2. Read it, and record its SHA-256 (sha256sum install.sh).\n\
         \x20 3. Run it pinned to the supported version, into your home directory:\n\
         \x20      {INSTALLER_VERSION_ENV}={pin} sh install.sh\n\
         \x20 The installer verifies the release tarball against the channel's SHA256SUMS. It \
         also takes over an existing TypeScript install of the same product and stops its \
         daemon, so read it first.\n\
         \n\
         Manual install (Windows, PowerShell): {OFFICIAL_INSTALLER_URL_WINDOWS}\n\
         \n\
         Or let Gauntlet run the installer you reviewed (Linux / macOS), after you confirm:\n\
         \x20 gauntlet agent setup prime --installer <path-to-install.sh> --sha256 <digest>\n\
         \n\
         Then check it with `gauntlet agent doctor`. If prime-agent is not on PATH, set \
         {BINARY_ENV} to its full path."
    )
}

/// Setup failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError {
    /// The installer file could not be read.
    InstallerUnreadable { path: PathBuf, detail: String },
    /// The installer's digest differs from the one supplied.
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    /// The supplied digest is not 64 hex characters.
    ChecksumMalformed { given: String },
    /// Installer execution is only supported on unix hosts.
    UnsupportedHost,
    /// No home directory to scope the install to.
    NoHome,
    /// The installer could not be started or exited unsuccessfully.
    InstallerFailed { detail: String },
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallerUnreadable { path, detail } => {
                write!(f, "cannot read installer {}: {detail}", path.display())
            }
            Self::ChecksumMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "installer {} has SHA-256 {actual}, not the expected {expected}; refusing to run it",
                path.display()
            ),
            Self::ChecksumMalformed { given } => write!(
                f,
                "--sha256 must be 64 hex characters (got {} characters)",
                given.len()
            ),
            Self::UnsupportedHost => write!(
                f,
                "running the installer is supported on Linux and macOS only; follow the manual \
                 steps from `gauntlet agent setup prime`"
            ),
            Self::NoHome => write!(f, "HOME is not set, so there is no user-scoped location"),
            Self::InstallerFailed { detail } => write!(f, "the installer failed: {detail}"),
        }
    }
}

impl std::error::Error for SetupError {}

/// Verifies `installer` against the lowercase-or-uppercase hex SHA-256
/// `expected`. Returns the verified bytes' digest on success.
pub fn verify_installer(installer: &Path, expected: &str) -> Result<String, SetupError> {
    let expected = expected.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SetupError::ChecksumMalformed { given: expected });
    }
    let bytes = std::fs::read(installer).map_err(|error| SetupError::InstallerUnreadable {
        path: installer.to_path_buf(),
        detail: error.to_string(),
    })?;
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(actual)
    } else {
        Err(SetupError::ChecksumMismatch {
            path: installer.to_path_buf(),
            expected,
            actual,
        })
    }
}

/// The user-scoped prefix the installer is pointed at: `<home>/.local`.
pub fn user_prefix(home: Option<&OsStr>) -> Result<PathBuf, SetupError> {
    let home = home.filter(|h| !h.is_empty()).ok_or(SetupError::NoHome)?;
    Ok(PathBuf::from(home).join(".local"))
}

/// Runs an ALREADY VERIFIED installer under `sh`, pinned to `pinned`,
/// into `prefix`. The caller must have obtained explicit confirmation;
/// this function never prompts and never downloads (the installer does
/// its own fetching, which is why the operator confirms first).
pub fn run_installer(
    installer: &Path,
    pinned: (u64, u64, u64),
    prefix: &Path,
) -> Result<(), SetupError> {
    if !cfg!(unix) {
        return Err(SetupError::UnsupportedHost);
    }
    let status = std::process::Command::new("sh")
        .arg(installer)
        .env(INSTALLER_VERSION_ENV, format_version(pinned))
        .env(INSTALLER_PREFIX_ENV, prefix.as_os_str())
        .status()
        .map_err(|error| SetupError::InstallerFailed {
            detail: format!("could not start sh: {error}"),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(SetupError::InstallerFailed {
            detail: format!("exited with {status}"),
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use gauntlet_domain::EpochSeconds;
    use std::os::unix::fs::PermissionsExt;

    const RANGE: VersionRange = VersionRange {
        min: (0, 8, 0),
        limit: (1, 0, 0),
    };

    fn far_deadline() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(u64::MAX / 4))
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gauntlet-managed-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn stub(dir: &Path, name: &str, version: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho {version} >&2\n")).expect("stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[test]
    fn absent_everywhere_is_a_typed_actionable_absence() {
        let dir = scratch("absent");
        let err = detect(None, Some(dir.as_os_str()), &RANGE, far_deadline()).unwrap_err();
        assert_eq!(err, DetectError::Absent);
        assert!(err.is_absent());
        assert!(err.to_string().contains("gauntlet agent setup prime"));
    }

    #[test]
    fn found_on_path_reports_location_and_version() {
        let dir = scratch("path");
        let bin = stub(&dir, "prime-agent", "0.10.2");
        let found = detect(None, Some(dir.as_os_str()), &RANGE, far_deadline()).unwrap();
        assert_eq!(found.binary, bin);
        assert_eq!(found.version, (0, 10, 2));
        assert_eq!(found.source, DetectionSource::Path);
        assert!(found.summary().contains("0.10.2"));
    }

    #[test]
    fn explicit_path_wins_over_path() {
        let on_path = scratch("explicit-path");
        stub(&on_path, "prime-agent", "0.9.0");
        let elsewhere = scratch("explicit-bin");
        let chosen = stub(&elsewhere, "my-prime", "0.12.1");
        let found = detect(
            Some(chosen.as_os_str()),
            Some(on_path.as_os_str()),
            &RANGE,
            far_deadline(),
        )
        .unwrap();
        assert_eq!(found.binary, chosen);
        assert_eq!(found.version, (0, 12, 1));
        assert_eq!(found.source, DetectionSource::Explicit);
    }

    #[test]
    fn a_set_but_missing_explicit_path_never_falls_through_to_path() {
        let on_path = scratch("explicit-missing");
        stub(&on_path, "prime-agent", "0.9.0");
        let missing = on_path.join("nope");
        let err = detect(
            Some(missing.as_os_str()),
            Some(on_path.as_os_str()),
            &RANGE,
            far_deadline(),
        )
        .unwrap_err();
        assert_eq!(err, DetectError::ExplicitMissing { path: missing });
    }

    #[test]
    fn an_incompatible_version_is_refused_naming_found_and_supported() {
        let dir = scratch("incompat");
        stub(&dir, "prime-agent", "2.0.0");
        let err = detect(None, Some(dir.as_os_str()), &RANGE, far_deadline()).unwrap_err();
        assert!(matches!(err, DetectError::Incompatible { .. }));
        let text = err.to_string();
        assert!(text.contains("2.0.0"), "{text}");
        assert!(text.contains("0.8.0") && text.contains("1.0.0"), "{text}");
        assert!(!err.is_absent());
    }

    #[test]
    fn a_binary_with_no_version_is_unusable_not_a_crash() {
        let dir = scratch("noversion");
        stub(&dir, "prime-agent", "hello");
        let err = detect(None, Some(dir.as_os_str()), &RANGE, far_deadline()).unwrap_err();
        assert!(matches!(err, DetectError::Unusable { .. }), "{err}");
    }

    #[test]
    fn verify_installer_accepts_only_the_matching_digest() {
        let dir = scratch("verify");
        let file = dir.join("install.sh");
        std::fs::write(&file, b"echo hi\n").unwrap();
        let digest: String = Sha256::digest(b"echo hi\n")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(verify_installer(&file, &digest).unwrap(), digest);
        assert_eq!(
            verify_installer(&file, &digest.to_ascii_uppercase()).unwrap(),
            digest
        );
        let wrong = "0".repeat(64);
        assert!(matches!(
            verify_installer(&file, &wrong),
            Err(SetupError::ChecksumMismatch { .. })
        ));
        assert!(matches!(
            verify_installer(&file, "abc"),
            Err(SetupError::ChecksumMalformed { .. })
        ));
    }

    #[test]
    fn run_installer_pins_the_version_and_scopes_the_prefix() {
        let dir = scratch("run");
        let marker = dir.join("seen.txt");
        let script = dir.join("install.sh");
        std::fs::write(
            &script,
            format!(
                "echo \"$PRIME_AGENT_VERSION $PRIME_AGENT_RUST_PREFIX\" > {}\n",
                marker.display()
            ),
        )
        .unwrap();
        let prefix = dir.join("home/.local");
        run_installer(&script, (0, 10, 2), &prefix).unwrap();
        let seen = std::fs::read_to_string(&marker).unwrap();
        assert_eq!(seen.trim(), format!("0.10.2 {}", prefix.display()));
    }

    #[test]
    fn user_prefix_needs_a_home() {
        assert_eq!(user_prefix(None), Err(SetupError::NoHome));
        assert_eq!(
            user_prefix(Some(OsStr::new("/home/x"))).unwrap(),
            PathBuf::from("/home/x/.local")
        );
    }

    #[test]
    fn manual_instructions_state_the_contract() {
        let text = manual_instructions((0, 10, 2), &RANGE);
        assert!(text.contains("never downloads"));
        assert!(text.contains(OFFICIAL_INSTALLER_URL));
        assert!(text.contains("PRIME_AGENT_VERSION=0.10.2"));
    }
}
