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

* **Evidence:** `AgentsService::spawn` (`gauntlet-app/src/delegation/agents.rs`) calls `AuthorityGate::admit` fresh with the caller's recorded facts but drops the returned `GateAdmission`; `DelegationBroker`/`RoleInvocationSpec` carry no containment proof and the runner's `CommandContainment` is fixed at composition time (`compose.rs` `wrap_checked_posture`, recorded=None — see OD-4).
* **Impact:** safe today only because the none-backend posture refuses every tier above `contained` (`dispatchable_posture`); with a live sandbox backend a delegated child could start under the composition-time wrap instead of one derived from its admission (AGENTS.md §5: authority and containment both precede process start).
* **Next move:** before enabling any non-none sandbox backend for delegated children, thread the admission into the spawn path (broker spec -> runner containment) and add a test that a child of a higher tier cannot start without its own wrapped command.

## OD-6 — fabric profile `isolation` is declared, not enforced

* **Evidence:** `crates/gauntlet-cli/src/fabric_compose.rs` accepts `isolation` per profile/limits, but a `CommandContainment` seam is wired only for the command-template runner token (as in `compose.rs`); other tokens receive none.
* **Impact:** a profile declaring process/container isolation without a containment seam is an honesty gap; related to OD-5.
* **Next move:** have the gate (or `fabric serve` startup/doctor) refuse or report any profile whose declared isolation has no wired containment before it is used.

## OD-7 — `cli_surface` doctor smoke writes `crates/gauntlet-cli/.agents/reports/doctor.json`

* **Evidence:** running `cargo test -p gauntlet-cli` leaves an untracked `crates/gauntlet-cli/.agents/` directory; the arch literal scan then fails (258/261) until it is removed.
* **Next move:** make the smoke set `GAUNTLET_REPORT_DIR` to a temp dir.

## OD-8 — fabric `Host` swallows a catalog build failure; schema checks duplicated

* **Evidence:** `fabric_cmd.rs` `Host::tools()` returns an empty list and `call()` returns Refused `catalog_invalid` if the `SemanticToolRuntime` fails to build; the runtime's schema validator and the agents wire decoder both validate arguments. Also one unexplained `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run` failure seen once (passed on rerun).
* **Next move:** build the runtime once at `fabric serve` startup and fail to start on error; retire the wire decoder checks once the runtime is trusted; investigate the doctor test flake.
