//! LIVE check of the per-invocation MCP bridge against the installed
//! prime-agent. It provisions an agent dir exactly as the runner does and asks
//! the real `prime-agent mcp list` to read it. No model is called.
//!
//! Ignored by default because it needs the installed binary. It FAILS, rather
//! than skipping, when the binary is missing or the server is not listed.
//!
//! Run:
//!   cargo test -p gauntlet-adapter-agent-prime --test mcp_bridge_live -- --ignored --nocapture

use std::path::PathBuf;
use std::process::Command;

use gauntlet_adapter_agent_prime::config::{AGENT_DIR_ENV, DEFAULT_BINARY_NAME};
use gauntlet_adapter_agent_prime::mcp_bridge::{McpBridge, provision};
use gauntlet_domain::InvocationId;

const LIVE_INVOCATION: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-prime-bridge-live-{tag}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("the live scratch is creatable");
    dir
}

#[test]
#[ignore = "live: needs the installed prime-agent binary; run with `cargo test -p gauntlet-adapter-agent-prime --test mcp_bridge_live -- --ignored --nocapture`"]
fn prime_agent_mcp_list_sees_the_provisioned_fabric_server() {
    let invocation_dir = scratch("invocation");
    // An empty source dir: the live check must not link or read the user's
    // real provider files.
    let source = scratch("empty-source");
    let mut bridge = McpBridge::new(
        PathBuf::from("gauntlet"),
        vec!["fabric".to_owned(), "serve".to_owned()],
    );
    bridge.source_agent_dir = Some(source);
    let invocation_id = InvocationId::new(LIVE_INVOCATION).expect("a valid invocation id");
    let agent_dir = provision(&bridge, &invocation_dir, &invocation_id)
        .expect("provisioning succeeds in a scratch dir");

    let output = Command::new(DEFAULT_BINARY_NAME)
        .args(["mcp", "list"])
        .env(AGENT_DIR_ENV, &agent_dir)
        .output()
        .unwrap_or_else(|error| {
            panic!("the live check needs `{DEFAULT_BINARY_NAME}` on PATH: {error}")
        });
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "prime-agent mcp list failed: {stderr}"
    );
    assert!(
        stdout.contains("gauntlet: stdio"),
        "the provisioned server must be listed; stdout was: {stdout}\nstderr: {stderr}"
    );
    std::fs::remove_dir_all(&invocation_dir).ok();
}
