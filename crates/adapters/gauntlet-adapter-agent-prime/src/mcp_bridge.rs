//! The per-invocation MCP bridge (Phase I slice 1b): how a Prime invocation
//! reaches Gauntlet's fabric MCP server with its caller identity fixed to
//! THAT invocation.
//!
//! Prime reads its MCP servers from `<agent dir>/settings.json` under
//! `mcpServers.<name>` and spawns each stdio server itself. So an invocation
//! with a bridge gets its own agent directory, `<invocation dir>/agent/`,
//! and the harness is launched with `PRIME_AGENT_CODING_AGENT_DIR` pointing
//! at it. The directory holds:
//!
//! - `settings.json`, GENERATED: `mcpServers.<name>` for the fabric server,
//!   with argv = the configured prefix, then `--caller <invocation id>` (the
//!   id comes from the runner contract, never from the model or a profile),
//!   then `--session-binding RLM_DEPTH=0`. The binding attests that only the
//!   root's own process is this invocation: a child process inherits the same
//!   settings, but its harness sets `RLM_DEPTH` to a deeper value, so its
//!   server is unbound and may not act on the invocation graph. The server
//!   entry also passes `RLM_DEPTH` and `RLM_SESSION_DIR` through from the
//!   harness's own process environment as tagged references (`{"env": NAME}`),
//!   plus the non-secret defaults the user's own settings carry
//!   (`defaultModel`, `defaultProvider`, `defaultThinkingLevel`). Nothing else
//!   is copied: the user's `mcpServers` may hold credentials, so it is never
//!   read into the generated file.
//! - `auth.json` and `models.json`, SYMLINKS to the user's agent-dir files
//!   when those exist. Provider logins and provider definitions are read
//!   through the link; no credential is copied. See the Phase I result in
//!   `.agents/plans/agent-fabric.md` for the rename-on-refresh caveat.
//!
//! Everything else the harness writes (sessions, logs, daemon state) lands in
//! the per-invocation directory, so the invocation's state is isolated.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gauntlet_domain::InvocationId;
use gauntlet_ports::agent_runner::AgentError;
use gauntlet_ports::json::{self, JsonValue};

use crate::config::AGENT_DIR_ENV;

/// The MCP server name the fabric server is registered under by default.
pub const DEFAULT_SERVER_NAME: &str = "gauntlet";

/// The per-invocation agent directory's name under the invocation directory.
const AGENT_SUBDIR: &str = "agent";
/// The harness settings file the MCP server table lives in.
const SETTINGS_FILE: &str = "settings.json";
/// The harness's default agent-state location under HOME, used only when the
/// agent-dir override is unset (the harness's own resolution order).
const HARNESS_HOME_DIR: &str = ".prime/agent";
/// Credential and provider files linked (never copied) from the source dir.
const LINKED_FILES: &[&str] = &["auth.json", "models.json"];
/// The non-secret settings keys carried into the generated settings file.
const COPIED_SETTINGS_KEYS: &[&str] = &["defaultModel", "defaultProvider", "defaultThinkingLevel"];
/// The per-invocation argument the fabric server reads its caller from.
const CALLER_FLAG: &str = "--caller";
/// The argument that binds the fabric server to a process environment value.
const SESSION_BINDING_FLAG: &str = "--session-binding";
/// The harness environment variable carrying the session's nesting depth.
const SESSION_DEPTH_ENV: &str = "RLM_DEPTH";
/// The harness environment variable carrying the session's own directory.
const SESSION_DIR_ENV: &str = "RLM_SESSION_DIR";
/// The depth the root invocation's own process carries.
const ROOT_SESSION_DEPTH: &str = "0";
/// The key of a tagged reference that copies a named environment variable.
const ENV_REFERENCE_KEY: &str = "env";

/// The MCP bridge configuration: the fabric server's launch, as composition
/// supplies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpBridge {
    /// The name the server is registered under in `mcpServers`.
    pub server_name: String,
    /// The executable the harness spawns (absolute, supplied by composition).
    pub program: PathBuf,
    /// The fixed argv prefix. Per-invocation `--caller <id>` and the session
    /// binding are appended to it.
    pub args: Vec<String>,
    /// The user's agent directory the non-secret defaults and the linked
    /// credential files are read from. `None` resolves the harness default
    /// from the environment at provisioning time.
    pub source_agent_dir: Option<PathBuf>,
}

impl McpBridge {
    /// A bridge under the default server name, reading the harness default
    /// agent directory.
    pub fn new(program: PathBuf, args: Vec<String>) -> Self {
        Self {
            server_name: DEFAULT_SERVER_NAME.to_owned(),
            program,
            args,
            source_agent_dir: None,
        }
    }

    /// The exact argv the harness will spawn for `invocation_id`: the prefix,
    /// the caller, then the session binding to the root's depth.
    pub fn argv_for(&self, invocation_id: &InvocationId) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.push(CALLER_FLAG.to_owned());
        argv.push(invocation_id.as_str().to_owned());
        argv.push(SESSION_BINDING_FLAG.to_owned());
        argv.push(format!("{SESSION_DEPTH_ENV}={ROOT_SESSION_DEPTH}"));
        argv
    }
}

/// Provisions the per-invocation agent directory under `invocation_dir` and
/// returns its path. Nothing is spawned here; a failure is an admission
/// refusal, so no child can start without its bridge.
pub fn provision(
    bridge: &McpBridge,
    invocation_dir: &Path,
    invocation_id: &InvocationId,
) -> Result<PathBuf, AgentError> {
    let refused = |detail: String| AgentError::AdmissionRefused { detail };
    let agent_dir = invocation_dir.join(AGENT_SUBDIR);
    fs::create_dir_all(&agent_dir).map_err(|error| {
        refused(format!(
            "the per-invocation agent dir for {invocation_id} could not be created ({error})"
        ))
    })?;

    let source = match harness_agent_dir(bridge.source_agent_dir.as_deref()) {
        Some(dir) => Some(std::path::absolute(&dir).map_err(|error| {
            refused(format!(
                "the harness agent dir {} is not resolvable to an absolute path ({error})",
                dir.display()
            ))
        })?),
        None => None,
    };

    let carried = match &source {
        Some(dir) => carried_settings(dir).map_err(refused)?,
        None => Vec::new(),
    };
    let settings = settings_document(bridge, invocation_id, carried).map_err(refused)?;
    fs::write(agent_dir.join(SETTINGS_FILE), settings).map_err(|error| {
        refused(format!(
            "the settings for {invocation_id} could not be written ({error})"
        ))
    })?;

    if let Some(dir) = &source {
        for name in LINKED_FILES {
            let target = dir.join(name);
            if !target.is_file() {
                continue;
            }
            let link = agent_dir.join(name);
            // A replayed provisioning replaces its own earlier link; the
            // directory is this invocation's, so nothing foreign is removed.
            let _ = fs::remove_file(&link);
            link_file(&target, &link).map_err(|error| {
                refused(format!(
                    "the credential link {name} for {invocation_id} could not be created ({error})"
                ))
            })?;
        }
    }
    Ok(agent_dir)
}

/// The harness's agent directory: the configured one, else the override
/// variable, else the harness default under HOME. A leading `~/` expands
/// against HOME, as the harness itself does.
fn harness_agent_dir(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = configured {
        return Some(expand_home(dir));
    }
    match std::env::var_os(AGENT_DIR_ENV) {
        Some(dir) if !dir.is_empty() => Some(expand_home(Path::new(&dir))),
        _ => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(HARNESS_HOME_DIR)),
    }
}

fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Ok(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| path.to_path_buf()),
        Err(_) => path.to_path_buf(),
    }
}

/// The allowlisted, non-secret settings the user's file carries. A missing
/// file carries nothing; an unreadable or malformed one refuses admission
/// rather than silently changing the invocation's defaults.
fn carried_settings(source: &Path) -> Result<Vec<(String, JsonValue)>, String> {
    let text = match fs::read_to_string(source.join(SETTINGS_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("the harness settings could not be read ({error})")),
    };
    let document = json::parse(&text)
        .map_err(|error| format!("the harness settings are not valid JSON ({error})"))?;
    let JsonValue::Object(_) = &document else {
        return Err("the harness settings are not a JSON object".to_owned());
    };
    Ok(COPIED_SETTINGS_KEYS
        .iter()
        .filter_map(|key| {
            document
                .get(key)
                .map(|value| ((*key).to_owned(), value.clone()))
        })
        .collect())
}

/// The generated settings document: the fabric server's MCP entry first, then
/// the carried defaults. Key order is fixed, so the text is deterministic.
fn settings_document(
    bridge: &McpBridge,
    invocation_id: &InvocationId,
    carried: Vec<(String, JsonValue)>,
) -> Result<String, String> {
    let program = bridge.program.to_str().ok_or_else(|| {
        format!(
            "the MCP server executable {} is not valid UTF-8",
            bridge.program.display()
        )
    })?;
    let argv = bridge
        .argv_for(invocation_id)
        .into_iter()
        .map(JsonValue::Str)
        .collect();
    let env = [SESSION_DEPTH_ENV, SESSION_DIR_ENV]
        .into_iter()
        .map(|name| (name.to_owned(), env_reference(name)))
        .collect();
    let server = JsonValue::Object(vec![
        ("type".to_owned(), JsonValue::Str("stdio".to_owned())),
        ("command".to_owned(), JsonValue::Str(program.to_owned())),
        ("args".to_owned(), JsonValue::Array(argv)),
        ("env".to_owned(), JsonValue::Object(env)),
    ]);
    let mut entries = vec![(
        "mcpServers".to_owned(),
        JsonValue::Object(vec![(bridge.server_name.clone(), server)]),
    )];
    entries.extend(carried);
    Ok(JsonValue::Object(entries).to_json_text())
}

/// A tagged reference that passes the harness's own value of `name` through.
fn env_reference(name: &str) -> JsonValue {
    JsonValue::Object(vec![(
        ENV_REFERENCE_KEY.to_owned(),
        JsonValue::Str(name.to_owned()),
    )])
}

#[cfg(unix)]
fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn link_file(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the MCP bridge links credentials with symlinks, which this platform does not provide",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const INVOCATION: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
    const OTHER_INVOCATION: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

    fn id(text: &str) -> InvocationId {
        InvocationId::new(text).expect("a valid invocation id")
    }

    fn bridge(source: Option<PathBuf>) -> McpBridge {
        McpBridge {
            server_name: DEFAULT_SERVER_NAME.to_owned(),
            program: PathBuf::from("/opt/gauntlet/bin/gauntlet"),
            args: vec![
                "fabric".to_owned(),
                "serve".to_owned(),
                "--run".to_owned(),
                RUN.to_owned(),
            ],
            source_agent_dir: source,
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-prime-bridge-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the bridge test scratch is creatable");
        dir
    }

    /// A user agent dir with the four kinds of file the directory holds: a
    /// settings file whose `mcpServers` carries a secret, a credential file,
    /// a provider file, and the non-secret defaults.
    fn user_agent_dir(tag: &str) -> PathBuf {
        let dir = scratch(tag);
        fs::write(
            dir.join(SETTINGS_FILE),
            "{\"defaultModel\":\"m-1\",\"defaultProvider\":\"p-1\",\
             \"defaultThinkingLevel\":\"high\",\"skills\":[\"s\"],\
             \"mcpServers\":{\"other\":{\"type\":\"stdio\",\"command\":\"x\",\
             \"env\":{\"TOKEN\":\"SECRET-VALUE-XYZ\"}}}}",
        )
        .expect("settings written");
        fs::write(
            dir.join("auth.json"),
            "{\"openai-codex\":\"SECRET-VALUE-XYZ\"}",
        )
        .expect("auth written");
        fs::write(dir.join("models.json"), "{\"providers\":{}}").expect("models written");
        dir
    }

    #[test]
    fn settings_document_is_exact_and_carries_only_the_caller_and_binding_argv() {
        let source = scratch("exact-empty-source");
        let invocation_dir = scratch("exact-inv");
        let agent_dir = provision(
            &bridge(Some(source.clone())),
            &invocation_dir,
            &id(INVOCATION),
        )
        .expect("provisioning succeeds");
        let text = fs::read_to_string(agent_dir.join(SETTINGS_FILE)).expect("settings readable");
        assert_eq!(
            text,
            format!(
                "{{\"mcpServers\":{{\"gauntlet\":{{\"type\":\"stdio\",\
                 \"command\":\"/opt/gauntlet/bin/gauntlet\",\
                 \"args\":[\"fabric\",\"serve\",\"--run\",\"{RUN}\",\"--caller\",\"{INVOCATION}\",\
                 \"--session-binding\",\"RLM_DEPTH=0\"],\
                 \"env\":{{\"RLM_DEPTH\":{{\"env\":\"RLM_DEPTH\"}},\
                 \"RLM_SESSION_DIR\":{{\"env\":\"RLM_SESSION_DIR\"}}}}}}}}}}"
            )
        );
        assert!(
            !agent_dir.join("auth.json").exists(),
            "no source credential file, so no link"
        );
    }

    #[test]
    fn no_secret_is_copied_and_credentials_are_linked_not_copied() {
        let source = user_agent_dir("secrets-source");
        let invocation_dir = scratch("secrets-inv");
        let agent_dir = provision(
            &bridge(Some(source.clone())),
            &invocation_dir,
            &id(INVOCATION),
        )
        .expect("provisioning succeeds");
        let text = fs::read_to_string(agent_dir.join(SETTINGS_FILE)).expect("settings readable");
        assert!(!text.contains("SECRET-VALUE-XYZ"), "no secret in settings");
        assert!(
            !text.contains("\"other\""),
            "the user's mcpServers is never copied"
        );
        assert!(
            !text.contains("skills"),
            "non-allowlisted keys are not carried"
        );
        assert!(
            text.contains("\"defaultModel\":\"m-1\""),
            "allowlisted defaults are carried"
        );
        assert!(text.contains("\"defaultThinkingLevel\":\"high\""));

        for name in LINKED_FILES {
            let link = agent_dir.join(name);
            let meta = fs::symlink_metadata(&link).expect("the linked file exists");
            assert!(
                meta.file_type().is_symlink(),
                "{name} is a link, not a copy"
            );
            assert_eq!(
                fs::read_link(&link).expect("readable link"),
                source.join(name),
                "{name} points at the user's file"
            );
        }
        assert_eq!(
            fs::read_to_string(agent_dir.join("auth.json")).expect("through the link"),
            fs::read_to_string(source.join("auth.json")).expect("source readable"),
            "provider auth still resolves through the link"
        );
    }

    #[test]
    fn two_invocations_get_distinct_dirs_and_callers() {
        let source = scratch("two-source");
        let invocation_dir_a = scratch("two-a");
        let invocation_dir_b = scratch("two-b");
        let spec_bridge = bridge(Some(source));
        let dir_a = provision(&spec_bridge, &invocation_dir_a, &id(INVOCATION)).expect("a");
        let dir_b = provision(&spec_bridge, &invocation_dir_b, &id(OTHER_INVOCATION)).expect("b");
        assert_ne!(dir_a, dir_b, "each invocation owns its agent dir");
        let a = fs::read_to_string(dir_a.join(SETTINGS_FILE)).expect("a settings");
        let b = fs::read_to_string(dir_b.join(SETTINGS_FILE)).expect("b settings");
        assert!(a.contains(&format!("\"--caller\",\"{INVOCATION}\"")));
        assert!(b.contains(&format!("\"--caller\",\"{OTHER_INVOCATION}\"")));
        assert!(!a.contains(OTHER_INVOCATION), "a names only its own caller");
        assert!(!b.contains(INVOCATION), "b names only its own caller");
    }

    #[test]
    fn a_malformed_user_settings_file_refuses_admission() {
        let source = scratch("malformed-source");
        fs::write(source.join(SETTINGS_FILE), "{not json").expect("written");
        let invocation_dir = scratch("malformed-inv");
        let error = provision(&bridge(Some(source)), &invocation_dir, &id(INVOCATION))
            .expect_err("malformed settings are refused, not ignored");
        assert!(matches!(error, AgentError::AdmissionRefused { .. }));
    }

    #[test]
    fn argv_appends_only_the_caller_and_the_root_binding_to_the_prefix() {
        let argv = bridge(None).argv_for(&id(INVOCATION));
        assert_eq!(
            argv,
            vec![
                "fabric".to_owned(),
                "serve".to_owned(),
                "--run".to_owned(),
                RUN.to_owned(),
                "--caller".to_owned(),
                INVOCATION.to_owned(),
                "--session-binding".to_owned(),
                "RLM_DEPTH=0".to_owned(),
            ]
        );
    }
}
