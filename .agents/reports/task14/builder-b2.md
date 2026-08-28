# Task 14 slice B2 — result contract (orchestrator-implemented under heal+continue)

**Provenance:** the sub-agent usage window (reset 2026-08-29 01:44) was still
exhausted; per the operator's standing heal+continue instruction the
orchestrator implemented this slice personally, following the same brief
discipline (checkpoint file, gates recorded below). The preregistered
predictions that fired are named inline.

## What landed

| File | Purpose |
|---|---|
| `crates/gauntlet-app/src/govern/mod.rs` | Module docs (I9: authority=whether, containment=how contained; D2 corrected) + re-exports. |
| `crates/gauntlet-app/src/govern/posture.rs` | `required_isolation` (§17.3/§8.6 rung table: contained→none, local/cross→process, irreversible→vm-moot; monotone with `blast_rank`) and `review_lattice_movement` (AK-046: agent-originated movement refused in EVERY direction; operator raises only; the runtime never lowers). 3 tests. |
| `crates/gauntlet-app/src/govern/gate.rs` | `GovernDeps` (construction refuses inconsistent postures — prereg R1: `with_backend` demands both instances, `none` forbids both), `AuthorityGate::admit` (six conjuncts in §17.1 order: irreversible → authority ceiling → lattice defense → capability (AK-050) → no-backend-for-tier (§8.6) → policy resolve+validate CLEAN (AK-047 escalation, never fallback)), `GateAdmission` carries policy+validation (AK-048), `wrap_builder_command` (Tasks 15-16 surface), typed `GateRefusal` with summaries. 8 tests (incl. AK-045's conjunctive-refusal shape: maximal merit, one hard violation, still refused). |
| `crates/gauntlet-app/src/control/controller.rs` | `ControllerDeps.govern` (required field — 6 app + 6 cli construction sites wired); the gate runs in `dispatch()` BEFORE the reservation/spec/invoke (a refusal consumes no budget); `record_gate_admission` (pinned BEFORE the invocation — the evidence ORDER is AK-048's proof surface); `record_gate_refusal` (pinned every time, ticket on first refusal per unit, dedup ledger in LoopState); `DispatchAdmission::GovernanceRefused`; `classify_no_admission` extended so an all-gate-refused run ends `blocked` with the ticket, never mislabeled `budget_exhausted`; `spec.sandbox_policy` = the admitted policy ref (§5.3.5 backend half now real); `record_selection`'s exercisable authority is ONE source now (`govern.exercisable_authority()` — L1 deferral closed). |
| `crates/gauntlet-app/src/doctor/checks.rs` | Check 4 real: `BackendDescriptor` (composition-root MEASUREMENT), tier-posture table (none backend → contained-only dispatchable), `sandbox_capability_mismatch` failures for no-descriptor / mistokened / unmeasured / rung-too-low; conservative posture passes with the honest governance note. |
| `crates/gauntlet-app/src/doctor/mod.rs` | `DoctorInput.backend_measure` hook (fn(&str) -> Option<BackendDescriptor>) consulted only when a real backend is configured; 4 new check-4 tests. |
| `crates/gauntlet-cli/src/main.rs` + `Cargo.toml` | FIRST production adapter wiring in the composition root: `measure_backend` locates the process-isolation backend, accepts it only on identity match with the configured token, and VALIDATES a live policy — the rung is measured, never declared. Capability-named façades only (`ProcessIsolationBackend`, `locate`). |
| `crates/gauntlet-cli/tests/p4_corrective_loop.rs`, `p6_budget_control.rs` | Fixture posture moves `local`→`contained` with inline recorded reasons (prereg P2 fired exactly as predicted: the gate refused the mock's honest none isolation above contained; the proofs' subjects are tier-agnostic; the above-contained posture moves to p5_hard_gate against the REAL backend). p12's local fixtures do not compose the dispatch loop — unchanged. |

## Verified behavior (composed, not just unit)

- `GAUNTLET_SANDBOX_BACKEND=bwrap just doctor` — check 4 tier-rungs PASS on a
  LIVE measurement ("backend achieves process" from the real binary), check 5
  satisfied by the in-repo policy artifacts. Default posture: check 4 passes
  with the contained-only note; exit 0.
- Standing battery green after the wiring: app 345/345, cli 54/54, workspace
  1049/1049.

## Gate results

`just check` clean · `just test` **1049/1049** · `just lint` clean ·
`just fmt-check` clean · `just boundaries` **215/215** · `just doctor` exit 0 ·
`just deny` ok · `just ruler` PASS · `just prereg-check 14` PASS.

## Deviations / notes for the critic

- The literals scanner's statement joiner taught us two shapes the hard way:
  `;`-less tail expressions merge following consts into deadline-named
  statements (fixed with semicolon'd lets + `#[allow(clippy::let_and_return)]`
  with the load-bearing reason in a comment), and numeric literals must not
  share a joined statement with a deadline-carrying call (facts pre-bound).
- 'cohere' is a denied provider TERM — "incoherent posture" prose was
  reworded to "inconsistent posture".
- `GovernDeps::none_workspace()` ergonomics constructor added (the none
  posture never reads the workspace anchor).
- The gate's lattice conjunct compares dispatch facts with the same
  iteration's answer-key re-read — coincident by construction in the happy
  path (documented as drift/forgery defense, D3); the g7 proof exercises the
  review function and a forged-facts dispatch directly.
