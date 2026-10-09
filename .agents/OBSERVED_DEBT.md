# Observed debt

Concrete out-of-scope debt, gaps, risks or defects noticed while working. Each entry: evidence, impact, suggested next move. Update entries in place.

## OD-1 — `e2e_recovery` kill-window legs are timing-flaky (pre-existing, backend-independent)

* **Evidence (2026-10-08):** on the committed SQLite baseline (`git worktree` at `691ec1c`, isolated target dir) `cargo test -p gauntlet-cli --test e2e_recovery` failed in 4 of 14 sequential runs (2/8, 2/6). Failing tests were always the same two: `the_kill_in_the_produce_window_resumes_with_the_production_recovered` (assertions at `e2e_recovery.rs` ~729/747: dispatch count / production-row count off by one) and `the_kill_after_the_production_record_pins_the_resume_exactly_once_claim` (~1001). On the PostgreSQL cut-over the same two tests fail in the same way at a similar-or-higher rate (5/8 in one sample, small n). A post-kill writer-quiescence barrier (`test_support::quiesce_writers`, keyed on `application_name`) was added and is correct for PostgreSQL, but did not change the rate — the cause is not a late server commit.
* **Mechanism (observed):** the test polls for one durable production row and then SIGKILLs the child. Under load the child can complete another produce→re-attempt cycle between the poll and the kill (pre-kill logs show the same `evidence_ref` produced twice, ~5 s apart), so the "kill landed here" branch the assertions assume is sometimes not the branch that happened.
* **Impact:** recovery proofs P7/T-kill legs cannot be treated as deterministic evidence; AGENTS.md §4 ("repeated green races prove only the branch that happened to win"). Not caused by, and not worsened in kind by, the PostgreSQL migration.
* **Next move:** replace poll-then-kill with a deterministic pause at the boundary — a test-only hook in the mock runner/controller that blocks at the produce boundary until the test has taken its durable snapshot and sent SIGKILL — and add one deterministic variant per branch (row already present / absent / settled).

## OD-2 — workbench `PrimeDelegationDispatcher` is a test-matrix-only projection with diverging policy

* **Evidence (2026-10-08 recon):** no production TS caller of `PrimeDelegationDispatcher.delegate` (only `testing/cross-surface-matrix.ts:712`, optional ref in `prime-long-running.ts:365`); production model-facing `rlm.spawn` is emitted by Rust (`gauntlet-adapter-agent-prime/src/program.rs`). TS policy is `max_depth=3, max_children_per_parent=4`; the Rust prime adapter defaults `max_depth=1` (`config.rs` DEFAULT_MAX_DEPTH). `childCount` (prime-recursion.ts ~384) counts ALL sessions with that parent, not concurrently admitted ones as the policy doc says.
* **Impact:** two unreconciled depth/child policies; T09–T11 proofs exercise a dispatcher production does not call. The Agent Fabric's Rust `DelegationBroker` + `InvocationGraph` is now the canonical implementation of these semantics.
* **Next move:** after Phase F's fabric host exists, route the workbench projection through it and delete/limit the TS dispatcher to a client; align the limits in one config source; count only non-terminal children.

## OD-3 — real Prime child receives the task string only, not the context envelope

* **Evidence:** `workbench/src/real-prime-host.ts` has no reference to "envelope"; `spawnChild` argv carries `-p <task>`, the envelope only reaches the in-memory mailbox (`receiveMailboxMessage`). Result-contract pass/fail there is `exit_code === 0` only.
* **Impact:** the TS real-host path does not honor the structural fresh-context envelope; contract outcome is exit-code based, not semantic.
* **Next move:** superseded by the Rust broker path (envelope is the invocation context); remove or fix when OD-2 is resolved.

## OD-4 — compose-time authority gate call passes `recorded = None`

* **Evidence:** `crates/gauntlet-cli/src/compose.rs:1236` calls `AuthorityGate::admit(.., recorded=None, ..)`, which skips the lattice conjunct; the per-dispatch call (`controller.rs:2239`) passes recorded facts.
* **Impact:** composition-time wrap check cannot catch lattice movement; delegation must always pass recorded facts (the Fabric's AgentsService does).
* **Next move:** confirm intent with the authority owner; pass recorded facts where available.

## OD-5 — delegation discards the `GateAdmission` containment proof

* **Status: resolved (bd5cab3).** `AgentsService::spawn` keeps the admission and the dispatch spec carries `admission.policy_ref()` (None under the no-backend contained posture); `RoleGrant.sandbox_policy` and the fabric role key were removed (a role TOML still naming it now fails as an unknown key). The command-template runner already refuses a dispatch carrying a policy when no containment seam is installed, so a higher-tier child cannot start unwrapped.
* **Remaining:** no end-to-end test runs a child through a live sandbox backend from the fabric path (the fabric has no containment seam wired; see OD-6).

## OD-6 — fabric profile `isolation` is declared, not enforced

* **Status: resolved as an honesty cap (13afcd2).** Effective isolation is the declared level only for a command-template profile with a wired containment seam; otherwise `none`. The gate's ceiling is the maximum effective isolation of available profiles, each cap is recorded in the composition report and printed by `fabric serve` and the doctor fabric section.
* **Remaining:** enabling real isolation for fabric children means wiring a containment seam in `fabric serve` (today `containment: None`); the cap lifts automatically once it is wired.

## OD-7 — `cli_surface` doctor smoke writes `crates/gauntlet-cli/.agents/reports/doctor.json`

* **Status: resolved in working tree.**
* **Evidence:** running `cargo test -p gauntlet-cli` leaves an untracked `crates/gauntlet-cli/.agents/` directory; the arch literal scan then fails (258/261) until it is removed.
* **Root cause (found 2026-10-09):** production defect, not a test defect. `doctor::assemble` skipped the environment layer when no `GAUNTLET.md` was discovered (`(None, None)` arm used `EffectiveConfig::from_defaults()` without calling `resolve`), so `GAUNTLET_REPORT_DIR` was ignored and the report fell back to `<project root>/.agents/reports`. The tests that set the variable (`fabric_cli`, `fabric_acceptance` doctor runs) therefore still wrote into the crate directory, which is the cwd under `cargo test`.
* **Fix:** the no-contract arm now runs `resolve` with no front matter, so the environment layer applies (`crates/gauntlet-app/src/doctor/mod.rs`). Regression test: `the_report_dir_env_is_honoured_without_a_contract`.

## OD-8 — fabric `Host` swallows a catalog build failure; schema checks duplicated

* **Status: partly resolved in working tree.** Resolved: the tool runtimes (agents, and program when present) are built once at `fabric serve` startup, and a build failure refuses to start with a non-zero exit and `refusing to start: ...` on stderr (`crates/gauntlet-cli/src/fabric_cmd.rs`, `startup_runtime`). Still open: retiring the wire decoder's duplicate schema checks, and the one-off `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run` failure (not reproduced in this slice).
* **Evidence:** `fabric_cmd.rs` `Host::tools()` returns an empty list and `call()` returns Refused `catalog_invalid` if the `SemanticToolRuntime` fails to build; the runtime's schema validator and the agents wire decoder both validate arguments. Also one unexplained `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run` failure seen once (passed on rerun).
* **Next move:** build the runtime once at `fabric serve` startup and fail to start on error; retire the wire decoder checks once the runtime is trusted; investigate the doctor test flake.

## OD-9 — codemode adapter: parallel bound enforced only inside the sandbox; port lacks cancel/memory classes

* **Status: partly resolved.** `ProgramLimit::Memory` now exists (wire `memory` -> `LimitExceeded(Memory)`, catalog token `heap` because `memory` is a reserved adapter segment in core crates). The parallel bound is enforced by the trusted Bun host against the guest's unsettled-call count; the Rust side cannot enforce it because the parent answers one call at a time, so no further defence is possible there. Still open: the port has no caller-side cancel (cancel happens only at the parent deadline).
* **Evidence (original):** the host protocol sends `call` one line at a time, so the Rust `DriverHost` never sees group boundaries and its parallel check (inside `call_group`) is skipped on this path. A program may finish with unawaited calls in flight (accepted; they are answered and recorded in the trace).
* **Next move:** if defense in depth is wanted, add an in-flight counter to the protocol (host reports concurrency) and a memory limit variant / cancel input to the port (needs approval).

## OD-10 — native `rlm.spawn` children inherit their Prime parent's MCP caller identity

* **Evidence (Phase I slice 1b, 2026-10-09):** a bridged invocation's `<invocation dir>/agent/settings.json` registers `mcpServers.gauntlet` with `--caller <this invocation id>` (`crates/adapters/gauntlet-adapter-agent-prime/src/mcp_bridge.rs`). Recon of prime-agent 0.9.8: native `rlm.spawn` children run inside the same rpc parent process and reuse that parent's MCP settings, so a `mcp.call_tool("gauntlet", "agents.spawn", ...)` issued from a native child is attributed to the PARENT's invocation id. Live-verified only that the parent reads the provisioned settings (`prime-agent mcp list`, `tests/mcp_bridge_live.rs`); the native-child inheritance itself was not exercised live.
* **Live result (2026-10-09, real model clinepass/cline-pass/deepseek-v4-flash, stub MCP server logging pid and argv, scratch agent dir with symlinked auth):** a model-driven Python cell in a Prime parent called `mcp.call_tool` successfully (first model-driven Python -> MCP evidence), provider auth resolved through the symlinks, and a native `rlm.spawn` child started its OWN MCP server process with the identical argv, including the parent's `--caller`. Inheritance is confirmed, not inferred. Prime 0.9.8 has no switch to disable `rlm.spawn` (it is a host request from the REPL), so it cannot be forbidden from the adapter.
* **Candidate mitigation (not built; semantic trade-off):** a per-caller single-holder lock in `fabric serve` so a second concurrent server for the same caller refuses to start. It would make native children unable to reach `agents.*`, but it fails the parent instead if a native child connects first.
* **Impact:** an accepted limitation. Gauntlet-admitted children created through `agents.spawn` get their own graph node and their own `--caller` through their own serve process. Only a harness-native child shares the parent's identity, so it cannot claim authority beyond its parent's.
* **Next move:** route every child through `agents.spawn` (no native `rlm.spawn` under a bridged profile), or give native children a per-child settings scope; then verify live with a real child.

## OD-11 — bridged credentials are symlinks: a token refresh during an invocation does not reach the user's `auth.json`

* **Evidence:** `auth.json` and `models.json` are symlinks from the invocation's agent dir to the user's `~/.prime/agent`, so provider logins resolve without any copy. Prime writes `auth.json` through `atomic_write` (temp file then rename: `.tmp/donor/prime-agent/crates/pa-core/src/settings/storage.rs` and `auth/storage.rs`). A rename replaces the symlink with a regular file inside the invocation dir. The user's file keeps the old refresh token, and the OAuth providers in the user's file (`openai-codex`, `clinepass`) may rotate or revoke that token.
* **Impact:** after a refresh inside a bridged invocation, the user's global login can go stale. Credentials are not duplicated or exposed, but they are not single-sourced after a refresh either.
* **Next move:** operator decision. Options: (a) after an invocation, sync a refreshed `auth.json` back to the source under the auth lock; (b) use env API keys for bridged profiles; (c) keep bridged profiles off OAuth providers. Add a test that a refresh is visible at the source.

## OD-12 — the Prime launch inherits the full parent environment; there is no env allowlist

* **Evidence:** `ProcessKernelTransport::submit` and the control calls use `std::process::Command` with no `env_clear()`, so a Prime parent inherits every variable the Gauntlet process has, including provider keys. The phase plan describes an env allowlist that the adapter does not implement. Slice 1b adds only `PRIME_AGENT_CODING_AGENT_DIR` (`rpc_command` in `transport.rs`) and leaves inheritance unchanged.
* **Impact:** not a regression, but the "env stays an allowlist" premise is false today. Credentials reach the harness through inheritance, not through the bridge.
* **Next move:** decide whether the Prime launch should `env_clear()` and pass an explicit allowlist (HOME, PATH, TMPDIR, the provider keys the chosen profile needs). That is a behavior change and needs approval.

## OD-13 — lifecycle supervision is poll-driven; history scan is O(n)

* **Evidence:** `LifecycleSupervisor` runs only inside `status` on a bound live node (no scheduled sampling); the high-water mark is derived by scanning persisted history per reading; the supervisor is proven against the in-memory graph at app level (PostgreSQL passes the shared graph contract only).
* **Next move:** persist a per-key high-water counter row; optional periodic sampler owned by the host.

## OD-14 — acceptance gaps: collect after restart is opaque; no stateful mock; doctor report path leak

* **Status: partly resolved in working tree.** Resolved: (a) collect after restart now reports `runner_unavailable` (app: `CollectOutcome::RunnerUnavailable`; wire: additive `runner_unavailable: true` on `agents.collect`, absent otherwise; acceptance: `acceptance_collect_after_restart_reports_runner_unavailable_and_never_completes`); (b) the doctor report path leak, fixed as OD-7. Still open: no stateful mock; `context_digest` covers the envelope body only; no exit-without-result fixture; late lifecycle samples not reachable at CLI level.
* **Evidence:** `fabric_acceptance.rs` — scripted children live only in the spawning process, so `agents.collect` from a fresh serve returned the opaque "internal" error (`BrokerError::Runner` on UnknownHandle) whereas `status` degrades to `runner_unavailable`; the mock keeps no state across calls (persistent-state parent not exercised); `gauntlet doctor --fabric-dir` wrote under `crates/gauntlet-cli/.agents` even with `GAUNTLET_REPORT_DIR` set (extends OD-7); `context_digest` covers the envelope body only; no exit-without-result fixture (AK-059 locator frozen); late lifecycle samples are not reachable at CLI level (covered by app-level `lifecycle_sequencing_*`).
* **Next move:** add a stateful mock runner so a persistent-state parent is exercised; add fixtures only through the AK-059 process.
