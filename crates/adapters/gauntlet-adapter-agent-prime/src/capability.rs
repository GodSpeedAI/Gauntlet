//! Behavioral capability probe: discovers what the harness runtime
//! actually supports through observed behavior, with version as metadata only.
//!
//! Unlike the version probe which guards compatibility through a semver
//! range, the capability probe establishes whether the harness exposes
//! required surfaces (spawn dialect, list/delete control seams, python
//! runtime interface) by exercising them under a deadline.

use core::fmt;
use std::path::PathBuf;

use gauntlet_ports::deadline::Deadline;

use crate::config::SpawnDialect;

#[cfg(test)]
use gauntlet_domain::EpochSeconds;

/// Configuration for the capability probe: paths to the binary, python
/// interpreter, and runtime source (all injected by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProbeConfig {
    /// The harness binary path (absolute or bare name resolved through PATH).
    pub binary: PathBuf,
    /// The python interpreter path (usually `python3`).
    pub python: PathBuf,
    /// The runtime source directory (e.g., `~/.local/share/prime-agent/prime-agent-runtime/src`).
    pub runtime_src: PathBuf,
}

impl CapabilityProbeConfig {
    /// Discovers the capability probe configuration from a binary path,
    /// using environment variables where available and falling back to defaults.
    ///
    /// - Resolves bare names through PATH, then canonicalizes.
    /// - `runtime_src` = env PRIME_AGENT_RUNTIME_SRC if set, else `<launcher_dir>/../share/prime-agent/prime-agent-runtime/src`
    /// - `python` = env PRIME_AGENT_PYTHON if set, else `"python3"`
    pub fn discover(binary: &std::path::Path) -> Self {
        let env_runtime_src = std::env::var_os("PRIME_AGENT_RUNTIME_SRC");
        let env_python = std::env::var_os("PRIME_AGENT_PYTHON");
        let path_env = std::env::var_os("PATH");
        Self::discover_with(binary, path_env, env_runtime_src, env_python)
    }

    /// Pure discovery function that accepts environment variables explicitly,
    /// enabling unit tests without env-var race conditions.
    pub fn discover_with(
        binary: &std::path::Path,
        path_env: Option<std::ffi::OsString>,
        env_runtime_src: Option<std::ffi::OsString>,
        env_python: Option<std::ffi::OsString>,
    ) -> Self {
        // Resolve the binary path: if bare name, scan PATH for first executable; then canonicalize.
        let binary = resolve_binary(binary, path_env.as_ref());

        // Resolve python: env var or default
        let python = if let Some(py_env) = env_python {
            PathBuf::from(py_env)
        } else {
            PathBuf::from("python3")
        };

        // Resolve runtime_src: env var or <launcher_dir>/../share/prime-agent/prime-agent-runtime/src
        let runtime_src = if let Some(src_env) = env_runtime_src {
            PathBuf::from(src_env)
        } else {
            binary
                .parent()
                .map(|p| {
                    p.join("../share/prime-agent/prime-agent-runtime/src")
                        .canonicalize()
                        .unwrap_or_else(|_| p.join("../share/prime-agent/prime-agent-runtime/src"))
                })
                .unwrap_or_else(|| PathBuf::from("."))
        };

        Self {
            binary,
            python,
            runtime_src,
        }
    }
}

/// The control seam's (`list --json`) operational status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamStatus {
    /// The seam works and returns valid JSON.
    Works,
    /// The seam failed because the daemon is not running (informational).
    DaemonNotRunning,
    /// The seam failed for other reasons (blocker).
    Broken,
}

/// A single capability finding — a typed, actionable discovery or failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityFinding {
    /// The binary does not exist or cannot be found.
    BinaryAbsent,
    /// The binary's `--version` output could not be parsed as semver.
    VersionUnparsed { stdout: String, stderr: String },
    /// The python runtime could not be imported or tested.
    RuntimeUnimportable { detail: String },
    /// No spawn surface found in runtime.
    NoSpawnSurface,
    /// The runtime has removed APIs (run/background) coexisting with spawn.
    RemovedApiPresent { name: String },
    /// The control seam (`list --json`) is broken.
    ControlSeamBroken { detail: String },
}

impl fmt::Display for CapabilityFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryAbsent => write!(
                f,
                "binary not found — ensure prime-agent is installed and on PATH, or run `prime-agent update`"
            ),
            Self::VersionUnparsed { stdout, stderr } => write!(
                f,
                "version output unparseable — stdout: {stdout:?}, stderr: {stderr:?}"
            ),
            Self::RuntimeUnimportable { detail } => write!(
                f,
                "runtime import failed: {detail} — run `prime-agent update` to ensure the runtime is installed"
            ),
            Self::NoSpawnSurface => write!(
                f,
                "no spawn or callable surface in runtime — run `prime-agent update` to upgrade the runtime"
            ),
            Self::RemovedApiPresent { name } => write!(
                f,
                "runtime has spawn and {name} which cannot coexist — run `prime-agent update` to resolve version conflict"
            ),
            Self::ControlSeamBroken { detail } => {
                write!(f, "control seam list --json failed: {detail}")
            }
        }
    }
}

impl std::error::Error for CapabilityFinding {}

/// The report of a capability probe: observed version (metadata), detected
/// spawn dialect, capability presence flags, and any findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    /// The semver version extracted from `--version`, if parseable (metadata only).
    pub version: Option<(u64, u64, u64)>,
    /// The detected spawn dialect, if a spawn surface exists.
    pub spawn_dialect: Option<SpawnDialect>,
    /// Whether the runtime has a `collect` callable.
    pub has_collect: bool,
    /// Whether the runtime has a `list_subagents` callable.
    pub has_list_subagents: bool,
    /// Whether the runtime has a `delete_subagent` callable.
    pub has_delete_subagent: bool,
    /// Whether the spawn method's return annotation mentions `RLMSpawnHandle`.
    pub spawn_returns_handle_fields: bool,
    /// The control seam (`list --json`) operational status.
    pub control_seam: SeamStatus,
    /// All typed findings from the probe (failures and diagnostics).
    pub findings: Vec<CapabilityFinding>,
}

impl CapabilityReport {
    /// Whether the runtime is sufficiently capable: dialect known, all
    /// required methods present, and control seam not broken (daemon running).
    pub fn is_sufficient(&self) -> bool {
        self.spawn_dialect.is_some()
            && self.has_collect
            && self.has_list_subagents
            && self.has_delete_subagent
            && self.control_seam != SeamStatus::Broken
    }
}

/// The poll interval while waiting for probes to complete, in milliseconds.
const PROBE_POLL_MILLIS: u64 = 10;

/// The maximum buffer for control seam JSON output, in bytes.
const CONTROL_SEAM_BUFFER_LIMIT: usize = 1024 * 1024; // 1 MB

/// Probes the harness runtime: version, spawn dialect, capability methods,
/// and control seams. Never panics; all failures become typed findings.
pub fn probe_capabilities(config: &CapabilityProbeConfig, deadline: Deadline) -> CapabilityReport {
    let mut findings = Vec::new();
    let mut version = None;
    let mut spawn_dialect = None;
    let mut has_collect = false;
    let mut has_list_subagents = false;
    let mut has_delete_subagent = false;
    let mut spawn_returns_handle_fields = false;
    let mut control_seam = SeamStatus::Works;

    // Step 1: Check binary presence.
    let binary_path = if config.binary.is_file() {
        config.binary.clone()
    } else {
        match which(&config.binary) {
            Some(resolved) => resolved,
            None => {
                findings.push(CapabilityFinding::BinaryAbsent);
                return CapabilityReport {
                    version,
                    spawn_dialect,
                    has_collect,
                    has_list_subagents,
                    has_delete_subagent,
                    spawn_returns_handle_fields,
                    control_seam,
                    findings,
                };
            }
        }
    };

    // Step 2: Try to parse version (metadata only; failure is just a finding).
    if deadline_stands(deadline) {
        match run_bounded_command(&binary_path, &["--version"], deadline) {
            Ok((stdout, stderr, _)) => {
                if let Some(v) = parse_version(&format!("{}\n{}", stdout, stderr)) {
                    version = Some(v);
                } else {
                    findings.push(CapabilityFinding::VersionUnparsed { stdout, stderr });
                }
            }
            Err(_) => {
                // Version failure is not a blocker; it's just metadata.
            }
        }
    }

    // Step 3: Probe the runtime for spawn dialect and capability methods.
    if deadline_stands(deadline) {
        match probe_runtime_interface(&config.python, &config.runtime_src, deadline) {
            Ok(runtime_info) => {
                spawn_dialect = runtime_info.spawn_dialect;
                has_collect = runtime_info.has_collect;
                has_list_subagents = runtime_info.has_list_subagents;
                has_delete_subagent = runtime_info.has_delete_subagent;
                spawn_returns_handle_fields = runtime_info.spawn_returns_handle_fields;

                findings.extend(runtime_info.findings);
            }
            Err(detail) => {
                findings.push(CapabilityFinding::RuntimeUnimportable { detail });
            }
        }
    }

    // Step 4: Check control seam (list --json).
    if deadline_stands(deadline) {
        match run_bounded_command(&binary_path, &["list", "--json"], deadline) {
            Ok((stdout, stderr, exit_code)) => {
                // Check if it parses as JSON.
                if is_valid_json(&stdout) {
                    control_seam = SeamStatus::Works;
                } else if exit_code != 0 && is_daemon_not_running(&stdout, &stderr) {
                    control_seam = SeamStatus::DaemonNotRunning;
                } else {
                    control_seam = SeamStatus::Broken;
                    findings.push(CapabilityFinding::ControlSeamBroken {
                        detail: "list --json output is not valid JSON".to_owned(),
                    });
                }
            }
            Err(detail) => {
                control_seam = SeamStatus::Broken;
                findings.push(CapabilityFinding::ControlSeamBroken { detail });
            }
        }
    }

    CapabilityReport {
        version,
        spawn_dialect,
        has_collect,
        has_list_subagents,
        has_delete_subagent,
        spawn_returns_handle_fields,
        control_seam,
        findings,
    }
}

/// Runtime interface discovery result.
struct RuntimeInfo {
    spawn_dialect: Option<SpawnDialect>,
    has_collect: bool,
    has_list_subagents: bool,
    has_delete_subagent: bool,
    spawn_returns_handle_fields: bool,
    findings: Vec<CapabilityFinding>,
}

/// Probes the Python runtime by executing a test script that imports and
/// inspects the runtime module.
fn probe_runtime_interface(
    python: &PathBuf,
    runtime_src: &PathBuf,
    deadline: Deadline,
) -> Result<RuntimeInfo, String> {
    // The test snippet checks for spawn, callable, and removed APIs.
    let snippet = r#"import sys; sys.path.insert(0, sys.argv[1]); import rlm, inspect, json; result = {"spawn": callable(getattr(rlm,'spawn',None)), "legacy_callable": callable(rlm), "has_run": hasattr(rlm,'run'), "has_background": hasattr(rlm,'background'), "collect": callable(getattr(rlm,'collect',None)), "list_subagents": callable(getattr(rlm,'list_subagents',None)), "delete_subagent": callable(getattr(rlm,'delete_subagent',None)), "spawn_params": list(inspect.signature(rlm.spawn).parameters) if hasattr(rlm, 'spawn') else [], "spawn_returns": str(inspect.signature(rlm.spawn).return_annotation) if hasattr(rlm, 'spawn') else ""}; print(json.dumps(result))"#;

    let mut child = std::process::Command::new(python)
        .arg("-I")
        .arg("-c")
        .arg(snippet)
        .arg(runtime_src)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env_remove("PYTHONPATH")
        .current_dir(std::env::temp_dir())
        .spawn()
        .map_err(|e| format!("spawning python failed: {e}"))?;

    // Wait for the child with deadline.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    // Read stderr to include in error message
                    use std::io::Read;
                    let mut stderr_text = String::new();
                    if let Some(mut pipe) = child.stderr.take() {
                        let _ = pipe.read_to_string(&mut stderr_text);
                    }
                    let last_line = stderr_text
                        .lines()
                        .last()
                        .unwrap_or("python exited with failure");
                    return Err(format!("python exited with {status}: {last_line}"));
                }
                break;
            }
            Ok(None) => {
                if !deadline_stands(deadline) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("python probe passed deadline".to_owned());
                }
                std::thread::sleep(std::time::Duration::from_millis(PROBE_POLL_MILLIS));
            }
            Err(e) => {
                return Err(format!("waiting for python failed: {e}"));
            }
        }
    }

    // Read the output.
    use std::io::Read;
    let mut stdout_text = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_string(&mut stdout_text)
            .map_err(|e| format!("reading python output failed: {e}"))?;
    }

    let json_str = stdout_text.trim();
    let json_obj = match gauntlet_ports::json::parse(json_str) {
        Ok(obj) => obj,
        Err(_) => {
            return Err(format!("python output not valid JSON: {json_str:?}"));
        }
    };

    let mut findings = Vec::new();
    let mut spawn_dialect = None;
    let mut spawn_returns_handle_fields = false;

    // Extract JSON fields using the JsonValue::get method.
    let spawn = json_obj
        .get("spawn")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let legacy_callable = json_obj
        .get("legacy_callable")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let has_run = json_obj
        .get("has_run")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let has_background = json_obj
        .get("has_background")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let has_collect = json_obj
        .get("collect")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let has_list_subagents = json_obj
        .get("list_subagents")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let has_delete_subagent = json_obj
        .get("delete_subagent")
        .and_then(|v| match v {
            gauntlet_ports::json::JsonValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    // Determine dialect and validate state.
    // Spawn09: spawn callable AND !has_run AND !has_background AND valid params/returns.
    // RemovedApiPresent: only for has_run / has_background coexisting with spawn.
    // Callable08: spawn NOT callable AND callable(rlm).
    if spawn && (has_run || has_background) {
        // Conflict: spawn coexists with removed APIs.
        let removed = if has_run { "run" } else { "background" };
        findings.push(CapabilityFinding::RemovedApiPresent {
            name: removed.to_owned(),
        });
        spawn_dialect = None;
    } else if spawn {
        // Spawn callable present; check parameters and absence of run/background.
        let params = json_obj
            .get("spawn_params")
            .and_then(|v| match v {
                gauntlet_ports::json::JsonValue::Array(arr) => Some(
                    arr.iter()
                        .filter_map(|item| match item {
                            gauntlet_ports::json::JsonValue::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        if !params.is_empty() && params[0] == "prompt" && params.iter().any(|p| p == "name") {
            spawn_dialect = Some(SpawnDialect::Spawn09);

            // Check return annotation.
            if let Some(gauntlet_ports::json::JsonValue::Str(returns)) =
                json_obj.get("spawn_returns")
            {
                spawn_returns_handle_fields = returns.contains("RLMSpawnHandle");
            }
        } else {
            findings.push(CapabilityFinding::NoSpawnSurface);
            spawn_dialect = None;
        }
    } else if legacy_callable && !spawn {
        // Callable08: callable(rlm) but no spawn method.
        spawn_dialect = Some(SpawnDialect::Callable08);
    } else {
        findings.push(CapabilityFinding::NoSpawnSurface);
    }

    Ok(RuntimeInfo {
        spawn_dialect,
        has_collect,
        has_list_subagents,
        has_delete_subagent,
        spawn_returns_handle_fields,
        findings,
    })
}

/// Runs a command with arguments, bounded by the deadline, capturing
/// stdout and stderr.
fn run_bounded_command(
    binary: &PathBuf,
    args: &[&str],
    deadline: Deadline,
) -> Result<(String, String, i32), String> {
    let mut child = std::process::Command::new(binary)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawning {} failed: {e}", binary.display()))?;

    let _exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break status.code().unwrap_or(-1);
            }
            Ok(None) => {
                if !deadline_stands(deadline) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("command passed deadline".to_owned());
                }
                std::thread::sleep(std::time::Duration::from_millis(PROBE_POLL_MILLIS));
            }
            Err(e) => {
                return Err(format!("waiting for command failed: {e}"));
            }
        }
    };

    use std::io::Read;
    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_string(&mut stdout)
            .map_err(|e| format!("reading stdout failed: {e}"))?;
    }
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_string(&mut stderr)
            .map_err(|e| format!("reading stderr failed: {e}"))?;
    }

    Ok((
        stdout,
        stderr,
        child.wait().ok().and_then(|s| s.code()).unwrap_or(-1),
    ))
}

/// Whether `deadline` still stands at the wall clock's now.
fn deadline_stands(deadline: Deadline) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    !deadline.exceeded_at(gauntlet_domain::EpochSeconds::new(now))
}

/// Resolves the binary path: if it has no path separator (bare name),
/// scans PATH for the first existing executable; then canonicalizes.
/// If unresolvable, returns the original path (probe will report BinaryAbsent).
fn resolve_binary(binary: &std::path::Path, path_env: Option<&std::ffi::OsString>) -> PathBuf {
    // If binary already has path separators, try to canonicalize directly
    if binary.components().count() > 1 {
        return std::fs::canonicalize(binary).unwrap_or_else(|_| binary.to_path_buf());
    }

    // Bare name: scan PATH for first executable, then canonicalize
    if let Some(path_os) = path_env {
        for dir in std::env::split_paths(path_os) {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                if let Ok(canonical) = std::fs::canonicalize(&candidate) {
                    return canonical;
                }
                // Fallback to the candidate if canonicalization fails
                return candidate;
            }
        }
    }

    // Not found in PATH; return the original bare name (probe will report BinaryAbsent)
    binary.to_path_buf()
}

/// Resolves a bare command name through `PATH`.
fn which(binary: &PathBuf) -> Option<PathBuf> {
    if binary.components().count() > 1 {
        return binary.is_file().then(|| binary.clone());
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Whether the text is valid JSON (at least structurally sound).
fn is_valid_json(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.len() > CONTROL_SEAM_BUFFER_LIMIT {
        return false;
    }
    matches!(
        gauntlet_ports::json::parse(trimmed),
        Ok(gauntlet_ports::json::JsonValue::Array(_))
            | Ok(gauntlet_ports::json::JsonValue::Object(_))
    )
}

/// Whether the error indicates the daemon is not running.
fn is_daemon_not_running(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{}\n{}", stdout, stderr);
    combined.contains("daemon")
        && (combined.contains("ENOENT") || combined.contains("Failed to connect"))
}

/// Parses the first `X.Y.Z` (optionally `v`-prefixed) token in the text.
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
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("gauntlet-probe-test-{}", std::process::id()))
            .join(format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_fake_binary(dir: &std::path::Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(script.as_bytes()).unwrap();
        drop(file);
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn make_fake_python(dir: &std::path::Path, script: &str) -> PathBuf {
        let path = dir.join("python");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(script.as_bytes()).unwrap();
        drop(file);
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn spawn09_ok_is_sufficient() {
        let tmpdir = temp_dir();

        // Fake binary that reports version.
        let binary_script =
            "#!/bin/sh\ncase \"$1\" in\n--version) echo 0.9.0;; \n*) echo '[]' ;;\nesac\n";
        let binary = make_fake_binary(&tmpdir, "prime-agent", binary_script);

        // Fake python that reports Spawn09.
        let py_script = "#!/bin/sh\necho '{\"spawn\":true,\"legacy_callable\":false,\"has_run\":false,\"has_background\":false,\"collect\":true,\"list_subagents\":true,\"delete_subagent\":true,\"spawn_params\":[\"prompt\",\"name\"],\"spawn_returns\":\"RLMSpawnHandle\"}'\n";
        let python = make_fake_python(&tmpdir, py_script);

        let runtime_src = tmpdir.clone();

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src,
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        assert_eq!(report.spawn_dialect, Some(SpawnDialect::Spawn09));
        assert!(report.has_collect);
        assert!(report.has_list_subagents);
        assert!(report.has_delete_subagent);
        assert!(report.spawn_returns_handle_fields);
        assert_eq!(report.control_seam, SeamStatus::Works);
        assert!(report.is_sufficient());
    }

    #[test]
    fn callable08_dialect_detected() {
        let tmpdir = temp_dir();

        let binary_script =
            "#!/bin/sh\ncase \"$1\" in\n--version) echo v0.8.0;; \n*) echo '[]' ;;\nesac\n";
        let binary = make_fake_binary(&tmpdir, "prime-agent", binary_script);

        let py_script = "#!/bin/sh\necho '{\"spawn\":false,\"legacy_callable\":true,\"has_run\":false,\"has_background\":false,\"collect\":true,\"list_subagents\":true,\"delete_subagent\":true,\"spawn_params\":[],\"spawn_returns\":\"\"}'\n";
        let python = make_fake_python(&tmpdir, py_script);

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src: tmpdir.clone(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        assert_eq!(report.spawn_dialect, Some(SpawnDialect::Callable08));
    }

    #[test]
    fn spawn_and_run_coexist_rejected() {
        let tmpdir = temp_dir();

        let binary_script = "#!/bin/sh\necho 'ok'\n";
        let binary = make_fake_binary(&tmpdir, "prime-agent", binary_script);

        let py_script = "#!/bin/sh\necho '{\"spawn\":true,\"legacy_callable\":false,\"has_run\":true,\"has_background\":false,\"collect\":true,\"list_subagents\":true,\"delete_subagent\":true,\"spawn_params\":[\"prompt\"],\"spawn_returns\":\"\"}'\n";
        let python = make_fake_python(&tmpdir, py_script);

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src: tmpdir.clone(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        assert_eq!(report.spawn_dialect, None);
        assert!(report.findings.iter().any(|f| matches!(
            f,
            CapabilityFinding::RemovedApiPresent { name } if name == "run"
        )));
    }

    #[test]
    fn spawn_with_callable_no_removed_apis_is_spawn09() {
        let tmpdir = temp_dir();

        let binary_script = "#!/bin/sh\necho 'ok'\n";
        let binary = make_fake_binary(&tmpdir, "prime-agent", binary_script);

        // Both spawn and callable are true, but no run/background => Spawn09.
        let py_script = "#!/bin/sh\necho '{\"spawn\":true,\"legacy_callable\":true,\"has_run\":false,\"has_background\":false,\"collect\":true,\"list_subagents\":true,\"delete_subagent\":true,\"spawn_params\":[\"prompt\",\"name\"],\"spawn_returns\":\"RLMSpawnHandle\"}'\n";
        let python = make_fake_python(&tmpdir, py_script);

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src: tmpdir.clone(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        assert_eq!(report.spawn_dialect, Some(SpawnDialect::Spawn09));
    }

    #[test]
    fn missing_binary_is_finding() {
        let tmpdir = temp_dir();
        let config = CapabilityProbeConfig {
            binary: tmpdir.join("nonexistent"),
            python: tmpdir.join("python"),
            runtime_src: tmpdir.clone(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        assert!(
            report
                .findings
                .iter()
                .any(|f| matches!(f, CapabilityFinding::BinaryAbsent))
        );
    }

    #[test]
    fn version_unparseable_is_only_metadata() {
        let tmpdir = temp_dir();

        let binary_script =
            "#!/bin/sh\ncase \"$1\" in\n--version) echo 'garbage';; \n*) echo '[]' ;;\nesac\n";
        let binary = make_fake_binary(&tmpdir, "prime-agent", binary_script);

        let py_script = "#!/bin/sh\necho '{\"spawn\":true,\"legacy_callable\":false,\"has_run\":false,\"has_background\":false,\"collect\":true,\"list_subagents\":true,\"delete_subagent\":true,\"spawn_params\":[\"prompt\",\"name\"],\"spawn_returns\":\"RLMSpawnHandle\"}'\n";
        let python = make_fake_python(&tmpdir, py_script);

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src: tmpdir.clone(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 10));
        let report = probe_capabilities(&config, deadline);

        // Version unparseable is just a finding; behavior is fine.
        assert!(
            report
                .findings
                .iter()
                .any(|f| matches!(f, CapabilityFinding::VersionUnparsed { .. }))
        );
        // But is_sufficient should still be true (if other conditions met).
        assert!(report.is_sufficient());
    }

    #[test]
    fn discover_with_defaults() {
        let tmpdir = temp_dir();
        let binary_path = tmpdir.join("test-binary");
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();

        let config = CapabilityProbeConfig::discover_with(&binary_path, None, None, None);

        // python should default to "python3"
        assert_eq!(config.python, PathBuf::from("python3"));
        // runtime_src should use the default layout: <launcher_dir>/../share/prime-agent/prime-agent-runtime/src
        assert!(
            config
                .runtime_src
                .to_string_lossy()
                .contains("share/prime-agent")
        );
    }

    #[test]
    fn discover_with_env_vars() {
        let tmpdir = temp_dir();
        let binary_path = tmpdir.join("test-binary");
        let python_path = tmpdir.join("python");
        let runtime_path = tmpdir.join("runtime/src");
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();
        std::fs::write(&python_path, "#!/bin/sh\n").unwrap();
        std::fs::create_dir_all(&runtime_path).unwrap();

        let config = CapabilityProbeConfig::discover_with(
            &binary_path,
            None,
            Some(runtime_path.clone().into_os_string()),
            Some(python_path.clone().into_os_string()),
        );

        assert_eq!(config.python, python_path);
        assert_eq!(config.runtime_src, runtime_path);
    }

    #[test]
    fn discover_bare_name_through_path() {
        let tmpdir = temp_dir();
        let bin_dir = tmpdir.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let share_dir = tmpdir.join("share/prime-agent/prime-agent-runtime/src");
        std::fs::create_dir_all(&share_dir).unwrap();

        let binary_path = bin_dir.join("prime-agent");
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();

        let path_env = bin_dir.clone();
        let config = CapabilityProbeConfig::discover_with(
            std::path::Path::new("prime-agent"),
            Some(path_env.into_os_string()),
            None,
            None,
        );

        // Binary should be resolved to the absolute path
        assert!(config.binary.is_absolute());
        assert!(config.binary.to_string_lossy().contains("prime-agent"));
        // runtime_src should be relative to resolved binary
        assert!(
            config
                .runtime_src
                .to_string_lossy()
                .contains("share/prime-agent")
        );
    }

    #[test]
    fn discover_bare_name_unresolvable_stays_bare() {
        let path_env = std::path::PathBuf::from("/nonexistent/bin");

        let config = CapabilityProbeConfig::discover_with(
            std::path::Path::new("nonexistent-binary"),
            Some(path_env.into_os_string()),
            None,
            None,
        );

        // Binary should remain bare name when unresolvable
        assert_eq!(config.binary, PathBuf::from("nonexistent-binary"));
    }

    #[ignore]
    // Run manually: `cargo test -p gauntlet-adapter-agent-prime -- --ignored probe_live_binary --nocapture`
    #[test]
    fn probe_live_binary() {
        let home = std::env::var("HOME").expect("HOME env var");
        let binary = PathBuf::from(&home).join(".local/bin/prime-agent");
        let python = PathBuf::from("python3");
        let runtime_src =
            PathBuf::from(&home).join(".local/share/prime-agent/prime-agent-runtime/src");

        if !binary.exists() || !runtime_src.exists() {
            eprintln!("Skipping live test: binary or runtime not found");
            return;
        }

        let config = CapabilityProbeConfig {
            binary,
            python,
            runtime_src,
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let deadline = Deadline::at_epoch(EpochSeconds::new(now + 30));
        let report = probe_capabilities(&config, deadline);

        println!("Report: {:#?}", report);
        // Assert on the live system's actual capabilities.
        assert_eq!(report.spawn_dialect, Some(SpawnDialect::Spawn09));
        assert!(report.has_collect);
        assert!(report.has_list_subagents);
        assert!(report.has_delete_subagent);
        assert_ne!(report.control_seam, SeamStatus::Broken);
        assert!(report.is_sufficient());
        assert!(report.version.is_some());
    }
}
