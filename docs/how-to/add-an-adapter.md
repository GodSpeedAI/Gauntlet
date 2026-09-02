# How-To: Implement a New Adapter

This guide explains how to implement a new adapter for any Gauntlet port and verify it against the port's existing shared contract test suite.

---

## Goal

Create a new adapter crate (e.g. `gauntlet-adapter-agent-myrunner`) that implements a `gauntlet-ports` trait without violating architectural boundaries or requiring bespoke test suites.

---

## Prerequisites

- Familiarity with Rust traits and `crates/gauntlet-ports`.
- Workspace compiling cleanly (`cargo check --workspace`).

---

## Procedure

### Step 1: Create the Adapter Crate

Add a new directory under `crates/adapters/`:

```bash
mkdir -p crates/adapters/gauntlet-adapter-agent-myrunner/src
mkdir -p crates/adapters/gauntlet-adapter-agent-myrunner/tests
```

Create `crates/adapters/gauntlet-adapter-agent-myrunner/Cargo.toml`:

```toml
[package]
name = "gauntlet-adapter-agent-myrunner"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
gauntlet-domain.workspace = true
gauntlet-ports.workspace = true

[dev-dependencies]
gauntlet-ports = { workspace = true, features = ["test-support"] }
```

> [!IMPORTANT]
> **Boundary Rule**: Adapters may depend **only** on `gauntlet-domain` and `gauntlet-ports`. Never add dependencies on `gauntlet-app` or `gauntlet-cli`.

### Step 2: Implement the Port Trait

In `crates/adapters/gauntlet-adapter-agent-myrunner/src/lib.rs`, implement the port trait:

```rust
use gauntlet_domain::values::InvocationId;
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus,
    Isolation, ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::role_result::RoleResultDoc;

pub struct MyRunnerAdapter {
    // Adapter configuration
}

impl AgentRunner for MyRunnerAdapter {
    fn invoke(&self, spec: &RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        // 1. Validate spec and permissions
        // 2. Spawn process or admit task
        // 3. Return InvocationHandle immediately
        todo!()
    }

    fn poll(&self, handle: &InvocationHandle, deadline: Deadline) -> Result<InvocationStatus, AgentError> {
        // Check execution state without blocking
        todo!()
    }

    fn collect(&self, handle: &InvocationHandle) -> Result<RoleResultDoc, AgentError> {
        // Parse role_result@v1 payload from declared contract path
        todo!()
    }

    fn cancel(&self, handle: &InvocationHandle, reason: CancelReason) -> Result<(), AgentError> {
        // Idempotent cancellation
        todo!()
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            isolation: Isolation::None,
            result_delivery: ResultDelivery::File,
            supports_native_subagents: false,
            max_depth: 1,
        }
    }
}
```

### Step 3: Wire into the Shared Contract Test Suite

In `crates/adapters/gauntlet-adapter-agent-myrunner/tests/contract_suite.rs`:

```rust
use gauntlet_adapter_agent_myrunner::MyRunnerAdapter;
use gauntlet_ports::contract::agent_runner;

#[test]
fn my_runner_passes_shared_contract() {
    agent_runner::test_runner_contract(|spec| {
        Box::new(MyRunnerAdapter::new(spec))
    });
}
```

### Step 4: Add to Workspace Manifest

Add the new crate to the root `Cargo.toml`:

```toml
[workspace]
members = [
    # ...
    "crates/adapters/gauntlet-adapter-agent-myrunner",
]

[workspace.dependencies]
gauntlet-adapter-agent-myrunner = { path = "crates/adapters/gauntlet-adapter-agent-myrunner" }
```

### Step 5: Wire into the Composition Root

In `crates/gauntlet-cli/src/compose.rs`, add the new runner selection branch:

```rust
use gauntlet_adapter_agent_myrunner::MyRunnerAdapter;

// Inside build_controller:
match ctx.config.agent_runner.as_str() {
    "myrunner" => Arc::new(MyRunnerAdapter::new(&ctx.config)?),
    // ...
}
```

---

## Validation

Run boundary tests and the shared contract suite:

```bash
cargo nextest run -p gauntlet-adapter-agent-myrunner
cargo nextest run -p gauntlet-arch-tests
```

All 228 architecture boundary tests and your adapter contract suite must pass.

---

## Common Failure Symptoms

| Symptom | Cause | Fix |
|---|---|---|
| `just boundaries` fails: `denylist::repo` | Adapter identifier leaked into domain or application layer. | Ensure the adapter name appears only in `crates/adapters/` or `gauntlet-cli/src/compose.rs`. |
| Contract suite fails: `admission_returns_before_completion` | `invoke()` blocked waiting for child process exit. | Ensure `invoke()` spawns in background and returns handle immediately. |
| Contract suite fails: `missing_result_yields_timeout` | Adapter returned `InvocationStatus::Completed` before result file existed. | Check `poll()` only reports completed after verifying contract file presence. |
