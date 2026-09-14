//! The harness version probe (plan Task 15 step 4: "Guard the harness API
//! surface behind a version probe; a probe mismatch is a `configuration`
//! failure surfaced by `doctor`, not a runtime surprise").
//!
//! The probe runs the configured binary with `--version`, parses the
//! output as a semver triple, and matches it against a typed
//! [`VersionRange`] INJECTED by the caller (correction RES-D8: the range
//! is configuration the composition root supplies from the application's
//! defaults module — never a bare literal here, and never something this
//! crate could move on its own). The upstream Annex-C surface (`rlm`
//! admission handles, `list_subagents`, `stop`) is package-internal; the
//! versioned range is the durable contract, and there is deliberately NO
//! install-layout or package-marker probe (dropped: coupling to install
//! layout is non-durable).
//!
//! ## Stream capture (gate-time correction COR-D1)
//!
//! The REAL harness prints its `--version` output to STDERR (verified at
//! gate time: stdout empty, stderr `0.8.0`, exit 0 — a conventionally
//! stdout-only probe read nothing and failed honestly as
//! `VersionUnparseable`; the prereg's P3 fired exactly as designed). The
//! probe therefore captures BOTH streams and parses the first semver
//! token from the COMBINED output (stdout first, then stderr — the
//! strict parser is unchanged). `VersionUnparseable` carries BOTH
//! streams' content so the locus is fully diagnosable whatever a future
//! harness does. The divergence is isolated to the version flag: the
//! control seam's `list --json` DOES print to stdout and is unaffected.
//!
//! Every failure is typed and configuration-class (spec §21.1/§21.2): the
//! binary absent, the version unparseable, or the version outside the
//! supported range each names its locus — never a bare string, never a
//! coerced pass. The fail-closed direction is unchanged.

use core::fmt;
use std::path::{Path, PathBuf};

use gauntlet_ports::deadline::Deadline;

/// The supported harness version range — a typed probe configuration the
/// caller injects (RES-D8). `min` is INCLUSIVE; `limit` is the EXCLUSIVE
/// upper bound: pre-1.0 minors may break the Annex-C surface without
/// notice, so each new minor is a deliberate re-verification that moves
/// the range in its owning defaults module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionRange {
    pub min: (u64, u64, u64),
    pub limit: (u64, u64, u64),
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (inclusive) .. {} (exclusive)",
            format_version(self.min),
            format_version(self.limit)
        )
    }
}

/// A successful probe: the binary and the version it reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub binary: PathBuf,
    pub version: (u64, u64, u64),
}

/// Probe failures — typed, configuration-class, carrying the locus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The configured binary does not exist (or is not a file, and no
    /// PATH entry resolves it).
    BinaryAbsent { binary: PathBuf },
    /// The binary ran but neither stream of its `--version` output
    /// carried a semver triple. BOTH streams' content is carried so the
    /// locus is fully diagnosable (COR-D1: this harness prints its
    /// version to stderr; a future one may differ).
    VersionUnparseable {
        binary: PathBuf,
        stdout: String,
        stderr: String,
    },
    /// The binary's version is outside the injected supported range.
    VersionUnsupported {
        binary: PathBuf,
        found: (u64, u64, u64),
        range: VersionRange,
    },
    /// The probe could not run to an output at all (spawn failure, or the
    /// binary did not exit before the deadline).
    ProbeFailed { binary: PathBuf, detail: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryAbsent { binary } => {
                write!(f, "runner binary not found at {}", binary.display())
            }
            Self::VersionUnparseable {
                binary,
                stdout,
                stderr,
            } => write!(
                f,
                "runner binary {} reported an unparseable version — stdout: {stdout:?}, \
                 stderr: {stderr:?} (expected a semver triple in either stream)",
                binary.display()
            ),
            Self::VersionUnsupported {
                binary,
                found,
                range,
            } => write!(
                f,
                "runner binary {} reports version {} which is outside the supported range \
                 {range} — moving the range is a deliberate, recorded re-verification in its \
                 owning defaults module",
                binary.display(),
                format_version(*found),
            ),
            Self::ProbeFailed { binary, detail } => write!(
                f,
                "probing runner binary {} failed: {detail}",
                binary.display()
            ),
        }
    }
}

impl std::error::Error for ProbeError {}

fn format_version(version: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// Whether `version` sits inside the injected supported range
/// (min inclusive, limit exclusive).
pub fn is_supported(version: (u64, u64, u64), range: &VersionRange) -> bool {
    version >= range.min && version < range.limit
}

/// The poll interval while waiting for `--version` to exit.
const PROBE_POLL_MILLIS: u64 = 10;

/// Probes the configured binary: presence, `--version` (bounded by
/// `deadline`), semver parse, range match against the INJECTED range. A
/// resolved binary is returned with its report; every other outcome is a
/// typed [`ProbeError`].
pub fn probe(
    binary: &Path,
    range: &VersionRange,
    deadline: Deadline,
) -> Result<ProbeReport, ProbeError> {
    let resolved = if binary.is_file() {
        binary.to_path_buf()
    } else {
        // A bare name on PATH still resolves: try the PATH the way exec
        // would, so a configured `prime-agent` found through PATH probes.
        which(binary).ok_or_else(|| ProbeError::BinaryAbsent {
            binary: binary.to_path_buf(),
        })?
    };
    let mut child = std::process::Command::new(&resolved)
        .arg("--version")
        // COR-D1: BOTH streams are captured — the real harness prints its
        // version to stderr. Piping both is convention-proof; parsing
        // scans the combined text.
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| ProbeError::ProbeFailed {
            binary: resolved.clone(),
            detail: format!("spawning --version failed: {error}"),
        })?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if !deadline_stands(deadline) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ProbeError::ProbeFailed {
                        binary: resolved,
                        detail: "the --version probe passed its deadline".to_owned(),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(PROBE_POLL_MILLIS));
            }
            Err(error) => {
                return Err(ProbeError::ProbeFailed {
                    binary: resolved,
                    detail: format!("waiting for --version failed: {error}"),
                });
            }
        }
    };
    use std::io::Read;
    // The `--version` output is tiny (a few bytes); reading the pipes
    // AFTER exit cannot deadlock on full buffers.
    let mut stdout_text = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_string(&mut stdout_text)
            .map_err(|error| ProbeError::ProbeFailed {
                binary: resolved.clone(),
                detail: format!("reading the --version output failed: {error}"),
            })?;
    }
    let mut stderr_text = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_string(&mut stderr_text)
            .map_err(|error| ProbeError::ProbeFailed {
                binary: resolved.clone(),
                detail: format!("reading the --version stderr failed: {error}"),
            })?;
    }
    if !status.success() {
        return Err(ProbeError::ProbeFailed {
            binary: resolved,
            detail: format!("--version exited with {status}"),
        });
    }
    // Parse the COMBINED output (stdout first, then stderr); the strict
    // parser is unchanged (COR-D1).
    let combined = format!("{stdout_text}\n{stderr_text}");
    let version = parse_version(&combined).ok_or_else(|| ProbeError::VersionUnparseable {
        binary: resolved.clone(),
        stdout: stdout_text.trim().to_owned(),
        stderr: stderr_text.trim().to_owned(),
    })?;
    if !is_supported(version, range) {
        return Err(ProbeError::VersionUnsupported {
            binary: resolved,
            found: version,
            range: *range,
        });
    }
    Ok(ProbeReport {
        binary: resolved,
        version,
    })
}

/// Resolves a bare command name through `PATH` (exec's lookup). Returns
/// the first existing FILE entry, if any.
fn which(binary: &Path) -> Option<PathBuf> {
    if binary.components().count() > 1 {
        // Contains a separator: not a bare name; existence was already
        // checked by the caller.
        return binary.is_file().then(|| binary.to_path_buf());
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Whether `deadline` still stands at the wall clock's now. The probe is
/// infrastructure: it reads the system clock directly (the `Clock` port is
/// the RUNTIME's source of now; a doctor/composition probe runs against
/// the host).
fn deadline_stands(deadline: Deadline) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    !deadline.exceeded_at(gauntlet_domain::EpochSeconds::new(now))
}

/// Parses the first `X.Y.Z` (optionally `v`-prefixed) token in the
/// output. Anything else is unparseable — never coerced.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    for token in text.split_whitespace() {
        let digits = token.strip_prefix('v').unwrap_or(token);
        let mut parts = digits.split('.');
        let (Some(major), Some(minor), Some(patch)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (Ok(major), Ok(minor), Ok(patch)) = (
            major.parse::<u64>(),
            minor.parse::<u64>(),
            patch.parse::<u64>(),
        ) else {
            continue;
        };
        if parts.next().is_some() {
            continue;
        }
        return Some((major, minor, patch));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_is_a_typed_injected_configuration() {
        let range = VersionRange {
            min: (0, 8, 0),
            limit: (1, 0, 0),
        };
        assert!(is_supported((0, 8, 0), &range));
        assert!(is_supported((0, 9, 7), &range));
        assert!(
            !is_supported((0, 7, 9), &range),
            "below the inclusive minimum"
        );
        assert!(!is_supported((1, 0, 0), &range), "the limit is exclusive");
        assert!(!is_supported((1, 2, 0), &range));
        // The range renders with both bounds and its inclusivity.
        let text = range.to_string();
        assert!(text.contains("0.8.0") && text.contains("1.0.0"), "{text}");
    }

    #[test]
    fn version_parsing_is_strict_and_tolerates_the_v_prefix() {
        assert_eq!(parse_version("0.8.0"), Some((0, 8, 0)));
        assert_eq!(parse_version("v0.8.0"), Some((0, 8, 0)));
        assert_eq!(
            parse_version("prime-agent 0.8.0 (nub-managed)"),
            Some((0, 8, 0))
        );
        assert_eq!(parse_version("10.20.30"), Some((10, 20, 30)));
        // Unparseable shapes: no coercion.
        assert_eq!(parse_version("zero point eight"), None);
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("0.8"), None);
        assert_eq!(parse_version("0.8.0.1"), None);
        assert_eq!(parse_version("0.8.x"), None);
        assert_eq!(parse_version("-1.2.3"), None);
    }

    #[test]
    fn error_display_names_the_locus_and_class() {
        let absent = ProbeError::BinaryAbsent {
            binary: PathBuf::from("no-such-binary"),
        };
        let text = absent.to_string();
        assert!(text.contains("no-such-binary"), "{text}");
        let unparseable = ProbeError::VersionUnparseable {
            binary: PathBuf::from("quiet"),
            stdout: String::new(),
            stderr: "hello".to_owned(),
        };
        let text = unparseable.to_string();
        assert!(text.contains("stdout") && text.contains("stderr"), "{text}");
        assert!(text.contains("hello"), "{text}");
        let unsupported = ProbeError::VersionUnsupported {
            binary: PathBuf::from("old"),
            found: (0, 3, 0),
            range: VersionRange {
                min: (0, 8, 0),
                limit: (1, 0, 0),
            },
        };
        let text = unsupported.to_string();
        assert!(text.contains("0.3.0") && text.contains("0.8.0"), "{text}");
    }

    // ------------------------------------------------------------------
    // COR-D1 hermetic fixtures: the REAL harness prints `--version` to
    // STDERR. These fixture scripts pin the combined-stream capture
    // WITHOUT touching the real harness (never part of any battery that
    // needs it). Unix-only: they spawn `/bin/sh` fixtures.
    // ------------------------------------------------------------------

    /// A hermetic executable fixture: a small shell script with the
    /// given stdout/stderr behavior.
    #[cfg(unix)]
    fn fixture_script(name: &str, stdout: &str, stderr: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-prime-probe-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).expect("fixture dir is creatable");
        let script = dir.join(name);
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s' '{stdout}'\nprintf '%s' '{stderr}' >&2\n"),
        )
        .expect("fixture script is writable");
        crate::test_support::make_executable_ready(&script);
        script
    }

    fn probe_range() -> VersionRange {
        VersionRange {
            min: (0, 8, 0),
            limit: (1, 0, 0),
        }
    }

    fn probe_bound() -> Deadline {
        use gauntlet_domain::EpochSeconds;
        // The probe reads the WALL clock (infrastructure), so the bound
        // is a generous span past now, not a fixed past instant.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the wall clock reads")
            .as_secs();
        Deadline::at_epoch(EpochSeconds::new(now + 120))
    }

    /// THE PIN (COR-D1): a binary that prints its version to STDERR ONLY
    /// (empty stdout, exit 0 — exactly the real harness's shape) probes
    /// clean.
    #[cfg(unix)]
    #[test]
    fn a_stderr_only_version_probes_clean() {
        let script = fixture_script("stderr-only", "", "0.8.0\n");
        let report = probe(&script, &probe_range(), probe_bound())
            .expect("a stderr-only version must probe clean");
        assert_eq!(report.version, (0, 8, 0));
        std::fs::remove_dir_all(script.parent().expect("fixture dir")).ok();
    }

    /// Regression guard: a conventional stdout-only version still probes
    /// clean (stdout is scanned first).
    #[cfg(unix)]
    #[test]
    fn a_stdout_only_version_probes_clean() {
        let script = fixture_script("stdout-only", "0.9.1\n", "");
        let report = probe(&script, &probe_range(), probe_bound())
            .expect("a stdout-only version must probe clean");
        assert_eq!(report.version, (0, 9, 1));
        std::fs::remove_dir_all(script.parent().expect("fixture dir")).ok();
    }

    /// Neither stream carries a semver: the typed error carries BOTH
    /// streams' content, fully diagnosable, never coerced.
    #[cfg(unix)]
    #[test]
    fn an_unparseable_probe_carries_both_streams() {
        let script = fixture_script("unparseable", "beep\n", "boop\n");
        let error = probe(&script, &probe_range(), probe_bound())
            .expect_err("no semver anywhere is unparseable");
        let ProbeError::VersionUnparseable { stdout, stderr, .. } = &error else {
            panic!("expected VersionUnparseable, got {error:?}")
        };
        assert_eq!(stdout, "beep");
        assert_eq!(stderr, "boop");
        let text = error.to_string();
        assert!(text.contains("beep") && text.contains("boop"), "{text}");
        std::fs::remove_dir_all(script.parent().expect("fixture dir")).ok();
    }
}
