# Task 13 — Fresh blind confirmation, round 2 — Role B (falsifier)

Date: 2026-08-28. Fresh context; no task13 report other than the brief was
read; no other task's reports, CURRENT_STATUS, lessons, or research files
were opened. Charter: `.agents/agents/falsifier.agent.md` (adopted verbatim;
its "File Structure" block names `tests/proofs/` etc., which do not exist in
this tree — the brief's artifact list is the real layout and was used).

Artifact attacked: the complete current working tree, principally
`crates/gauntlet-app/src/settle/**`, `control/controller.rs`,
`verify/{precondition,authority,ladder}.rs`,
`crates/gauntlet-ports/src/{state_store,producer_authority,verification_record}.rs`
and `contract/{state_store,stubs}.rs`,
`crates/adapters/gauntlet-adapter-state-sqlite/**`,
`crates/gauntlet-arch-tests/src/{lib,single_writer}.rs`,
`crates/gauntlet-cli/tests/{p7_recovery,p11_critic_is_not_verifier,g10_builder_writes_answer_key}.rs`,
against the frozen ruler AK-040/041/042/043/062(service half)/031/024(settle
half)/053 and spec §16, §18.2–18.4, §11.5–11.6, §21.3, invariants I1/I3/I8/I11.

## Verdict summary

**VERDICT: CONFIRM** — within the declared bound below, no frozen claim is
broken as written through the composed production path; nothing above MINOR
was found. One MINOR structural blind window (F-1) and three recorded
observations (OBS-1..3), all inside the repo's stated threat model.

## Threat model, search bound, budget

**Threat model.** Untrusted: builder/critic role output (result documents,
declared attempted lists, workspace filesystem effects), model output,
store payloads, out-of-band DB tampering. Trusted (per the artifact's own
declared residual assumption, `producer_authority.rs:36-40` and §7.1):
in-process runtime code and the composition root's adapter selection. I
attacked the trusted boundary too (LEG-1/LEG-3 below) to characterize it,
but a trusted-caller-only route is not a frozen-claim break under this
model — I say so explicitly per finding rather than inflating severity.

**Search bound.** Static: full read of the settle service, the two ports
admission authorities and their error surfaces, the sqlite adapter's
event log + state store commit/replay/tamper internals + inherent public
surface + crash helper, the controller's completion/settle/resume/drive
paths, `attempted_units`, `contained_locator_path`, the reference
provider's freeze/`verify_unchanged`, the precondition layer, the
verification authority's binding checks, the single-writer arch rule and
its pinned inventories, and the three proof files' composition
inventories. Dynamic: the seven executed probe commands below. NOT
examined: scheduler/budget/disposition internals beyond their settle
interactions; verify-ladder internals beyond `verify_claim`'s binding
checks; compare/observation subsystems; migration SQL beyond the
settlement/event tables' use; CLI arg surface beyond the three proof
targets; anything under `.agents/reports/task13/**` besides this file or
the other excluded trees (blindness); mutation testing and coverage
(excluded by the build lock); concurrency behavior (the controller is
single-threaded; I note the one concurrency-shaped residual inside F-1).

**Budget.** ~30 targeted file reads; 6 nextest invocations + 2 probe
compilations, all single cargo processes, package-scoped, no parallelism
changes, no `cargo clean`, no mutation runs. Probes compiled under
`~/.cache/task13-falsifier/` (outside repo and /tmp).

## Executed evidence (commands and observed results)

1. `cargo nextest run -p gauntlet-app settle::` — **22/22 PASS** (entry
   refusals for FAIL/INCONCLUSIVE with zero durable writes, idempotent
   same-key re-entry, same-key evidence divergence refusal, operator-action
   gates incl. PASS-present-still-refused, durable-block divergence,
   mutation abort with both digests pinned, resume completion, authority
   matrix incl. human-refuses-automatic + escalation routing).
2. `cargo nextest run -p gauntlet-ports producer_authority` — **22/22
   PASS**, including `a_fresh_verification_key_cannot_re_settle_a_settled_unit`,
   `a_settled_unit_re_attempted_under_a_fresh_honest_pass_refuses`, and
   `one_pass_record_cannot_settle_n_distinct_verification_refs` (the
   COR-3 mooring), plus the in-crate no-constructor teeth.
3. `cargo nextest run -p gauntlet-adapter-state-sqlite` — **50/50 PASS**:
   both settlement-append gates (`append_event`, `EventLog::append` refuse
   `SettlementCommitted` with zero durable writes), `contract_suite`
   (incl. earned-commit + never-re-admits cases), tamper evidence
   (`a_same_key_divergent_event_is_loud_on_replay`,
   `a_same_facts_duplicate_event_stays_the_idempotent_no_op`,
   offset-gap/negative-ts damage, changed-intent run re-record refused).
4. `cargo nextest run -p gauntlet-cli --test p7_recovery --test
   p11_critic_is_not_verifier --test g10_builder_writes_answer_key` —
   **12/12 PASS**: `commit_order`; crash before-event / between-event-and-row /
   after-row with a REAL `std::process::abort()` child over the same DB and
   fresh compositions through production `resume_run` (exactly one
   settlement record, no duplicate event, no re-dispatch, identity/evidence
   intact); settled-run re-resume; P11's four no-settle legs + PASS control;
   G10 prevention (locator at the key refused typed, key bytes identical)
   and detection (mid-run tamper → `aborted`, both digests pinned, nothing
   settles over the mutated ruler).
5. `cargo nextest run -p gauntlet-arch-tests` — **215/215 PASS**, incl.
   `answer_key_single_writer` (Rules A/B/C over all four core crates —
   `gauntlet-cli` included in `CORE_CRATES`; no production write-API token
   anywhere in the CLI).
6. `cargo nextest run -p gauntlet-app
   control::controller::tests::{a_declared_attempted_other_unit…,
   resume…}` — 3/3 PASS (declared attempted units outside the flight are
   pinned as `attempted_unit_mismatch` and never processed; resume restores
   budget/attempts from durable facts).
7. **Out-of-crate probe A** (the compile gate the ports doc explicitly
   defers to the confirmation round,
   `producer_authority.rs:~1660`/"the compiled out-of-crate probe is the
   confirmation round's job"): a probe crate outside the repository
   constructing `AdmittedSettlement { commit }` with a hand-built
   `SettlementCommit` → **compile refused, E0451 "private field"**. The raw
   commit value has no out-of-crate route to any store. (Probe:
   `~/.cache/task13-falsifier/probe-a`.)
8. **Out-of-crate probe B** against the real sqlite adapter, public APIs
   only (`~/.cache/task13-falsifier/probe-b`; output verbatim):
   - LEG-1 (admission forgery surface): hand-built PASS doc → public
     `VerificationRecordAuthority::record` → public
     `SettlementCommitAuthority::admit` → `commit_settlement` ⇒
     `offset=1, rows=["AK-100@vrf-1"], events=1`. A chain-free settlement
     composes out-of-crate (see OBS-1).
   - LEG-2 (fresh honest PASS at a settled unit, new evidence + new
     verification ref): admission **REFUSED** `UnitAlreadySettled`; rows and
     events unchanged — the double-settlement guard holds under execution.
   - LEG-3 (half-step interleaving): after an earned event-only append of
     key A (`append_settlement_event`), admission of key B for the same
     unit **ACCEPTED** (the guard reads rows, not events); after commit B
     and replay: `rows=["AK-100@vrf-1","AK-200@vrf-3","AK-200@vrf-4"],
     events=3` — two durable settlement rows for AK-200 (see F-1).

## Findings

### F-1 — MINOR — admission is blind to appended-but-unapplied settlement events

- **Claim touched:** AK-042 ("no double commit", "exactly one settlement
  record") — at the AUTHORITY/adapter level only; **no production path
  reaches this shape** (see reachability).
- **Locus:** `crates/gauntlet-ports/src/producer_authority.rs:771-822`
  (`SettlementCommitAuthority::admit` — the one-per-(run,unit) guard reads
  `state.settlements()` ROWS only) composed with
  `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:799`
  (`pub fn append_settlement_event` — the crash instrument's earned
  half-step: event durable, row deliberately unapplied) and
  `state_store.rs:1099` (settlements UNIQUE key is
  `(run_id, unit_id, verification_ref)`, not `(run_id, unit_id)`).
- **Scenario:** admit A → `append_settlement_event(A)` (row absent) →
  admit B (different verification ref, same unit; every §16.1-condition-1
  check genuinely holds for B) — the `UnitAlreadySettled` guard finds no
  row, so B admits → `commit_settlement(B)` → any later
  `complete_pending_settlements` replays event A → **two settlement rows
  and two events for one unit**. Executed (probe B LEG-3).
- **Why MINOR, not MAJOR:** the only production writer is
  `SettlementService::settle` → `commit_settlement` (atomic event+row;
  `settle/mod.rs:697`); `append_settlement_event` has exactly one caller
  in the tree — the `crash_helper` TEST instrument. Producing the
  interleaving requires trusted in-process code to deliberately reuse the
  instrument's half-step out of role. Within the declared threat model
  this is not an agent-reachable bypass; it is a structural blind window
  in the guard whose own crash-window semantics create it. If the guard is
  ever relied on across process boundaries, this becomes MAJOR — recording
  it so the boundary is drawn in the open.
- **Suggested hardening direction (for the orchestrator's queue, not
  mine):** have `admit` also consult settlement EVENTS at/after the
  checkpoint anchor, or key the `settlements` table on
  `(run_id, unit_id)`.

### OBS-1 — observation, no severity — the public authorities compose into a chain-free settlement (LEG-1)

`VerificationRecordAuthority::record` validates only run-durability and
the durable iteration anchor — it does not (and, in-process, cannot)
verify that the PASS was earned by the governed chain; the module doc
names the caller restriction as an expectation ("the caller is the
verified chain itself … never an arbitrary store holder",
`producer_authority.rs:416-421`), and the module's residual-trust note
(`:36-40`) covers exactly this. Consequence: any in-process module holding
the store can mint the durable PASS substrate and settle, with all
§16.1-store-side checks genuinely holding (LEG-1 executed green). This is
the declared composition-root trust assumption, not a hidden hole; the
COR-1 comment "a raw caller-built document has no route to the admission's
substrate" is accurate for the STORE trait surface (probe A: compile
refused) and should not be read as caller-identity enforcement at the
authority layer. No frozen claim broken: §16.4 makes the runtime the sole
writer, and this caller is the runtime's trust domain.

### OBS-2 — observation, no severity — prevention leg of AK-043 is lexical; detection carries the invariant

`contained_locator_path` (`controller.rs`, `Component`-based) is purely
lexical — a builder pre-planting a symlink at `workspace/<locator>` → the
ruler would have the controller's facet `fs::write` follow it out of the
workspace. AK-043 claims "prevented **or** detected": the detection half
is structural and independent of write mechanism — the provider re-reads
the key file and re-derives the digest at every iteration boundary
(`boundary_verify`) and immediately before settlement
(`settle_time_verify`, file re-read, not a comparison against a
controller-frozen digest — the AK-024 anti-tautology shape holds;
g10 leg 2 executed it green against a direct write, and a symlink-mediated
write differs only in mechanism). §22.2's "the harness is not a sandbox"
records the residual. No change to the verdict.

### OBS-3 — observation, no severity — same-admission duplicate event append

`append_settlement_event` does not itself check for an existing event/row;
re-invoking it with ONE admission appends duplicate same-key events
(log-only). Replay of duplicates is the designed idempotent no-op
(pinned by `a_same_facts_duplicate_event_stays_the_idempotent_no_op`), so
no second record mints. Same trusted-caller surface as F-1.

## What held under attack (attack-failed-to-find-a-bug, not "formally proven")

- **Double settlement, production path:** fresh honest PASS at a settled
  unit (new evidence + new ref) refuses at admission (executed, LEG-2 +
  ports tests); same-key replay is idempotent; same-key divergent evidence
  is a loud typed refusal at authority AND store replay (adapter tests);
  resume replay is anchored, idempotent, and re-scans nothing below the
  checkpoint (p7 legs); attempted lists naming units outside the flight
  are pinned and never verified or settled (controller test).
- **Admission forgery:** raw `AdmittedSettlement`/`AdmittedVerificationRecord`
  construction is a compile error out-of-crate (probe A, E0451 — the
  deferred out-of-crate gate obligation is now discharged); `append_event`
  and `EventLog::append` refuse `SettlementCommitted` (both surfaces,
  zero durable writes — adapter gates); `append_record` refuses the
  verification-record kind; the commit's timestamp is the service clock's,
  and the store cross-validates row↔event on every field including the
  stored timestamp (A3), so no caller-attested payload rides through; the
  settle request carries no readiness field (conditions 1–2 re-derived at
  entry over the claim itself — structurally confirmed by reading, and by
  the passing service tests).
- **Commit order:** event → keyed row → in-memory advance holds on the
  production path (`commit_settlement` atomic event+row; controller
  projection update strictly after; checkpoint at the next boundary), and
  p7's `commit_order` observes it through a delegating store probe plus
  checkpoint bracketing. A step-2 failure names the durable offset and the
  recovery that completes it (adapter + tamper tests).
- **Settle-time blindness/tautology:** `verify_unchanged` re-reads the key
  FILE and re-derives the digest; g10's detection leg fired through the
  composed controller with both digests pinned. The controller does freeze
  a digest, but the consult never compares against it.
- **Non-PASS settling:** FAIL/INCONCLUSIVE claims refuse at service ENTRY
  with zero durable writes (service tests + p11 composed legs); the three
  states stay distinct through every layer; missing evidence is
  INCONCLUSIVE, never PASS (P12-shaped legs in p11).
- **Single-writer:** no production write-API token exists in any core
  crate outside the three inventoried, guarded site families
  (`answer_key_single_writer` over domain/ports/app/cli, 215-test boundary
  suite green); the runtime writes Unit status only through the governed
  service + checkpoint projection; the answer key has no writer at all in
  the core (read-only provider seam).
- **Operator-action bypass (AK-062 service half):** `settle_operator_action`
  hits the same durable-PASS gate first and, even with PASS present,
  cannot settle (no readiness can travel with that request shape) — the
  positive operator half is out of Task-13 scope per the plan, and the
  service exposes no side door (tests 12/17 of the settle suite).

## Explicitly not examined (bound)

Scheduler/budget/disposition internals beyond settle interactions;
verify-ladder rung mechanics beyond `verify_claim`'s binding gates;
compare/observation subsystems; adapter selection wiring in the CLI
beyond the proof compositions; sqlite migration SQL beyond the
settlement/event/settlements tables' read paths; e2e/contract suites for
other ports; mutation testing, coverage, and `just` gates other than the
nextest runs above (build-lock discipline); all blinded report trees.
"Attack failed" above means exactly that — within these bounds, threat
model, and budget — not formal proof of absence.

**VERDICT: CONFIRM**
