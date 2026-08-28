# Task 13 / Builder brief B4 — composition proofs p7 / p11 / g10 + teeth

You are a BUILDER. Read `AGENTS.md` first. B2's service and B3's arch rule
must already be landed and green (check `.agents/reports/task13/IN-FLIGHT.md`
before starting; if B2's Phase R/G are incomplete, stop and report — do not
build proofs against a red tree).

## Context

1. `.agents/reports/task13/recon-code.md` §8 (proofs harness: Scratch
   pattern, real stores in temp dirs, `CARGO_BIN_EXE_crash_helper`, the
   four abort points) and §5 (the composed PASS leg through the service,
   post-B2).
2. `.agents/reports/task13/recon-spec.md` — P7/P11 verbatim, §16.3 windows,
   §18.3 resume steps, AK-041/042/043/031 texts (recon-answerkey-residuals
   §a), the teeth-check MUST (§23.1).
3. Prereg D1–D5 (`.agents/research/preregistrations/task-13.prereg.yaml`) —
   the distinctions your proofs must not blur:
   - D1: AK-041 is an ORDER claim — the proof needs a reorder tooth, and the
     in-memory-before-durable shape is the one the pre-B2 controller had.
   - D2: P7 is a claim about the RESUME path — crash_helper against a bare
     store proves the primitive; the composed `resume_run` must perform or
     govern the completion and never re-run settled work.
   - D5: g10 must exercise a builder-context write ATTEMPT against the REAL
     confinement, not assert absence of an API.

## Deliverables — three proof files at the composition root

**Item 1 — `crates/gauntlet-cli/tests/p7_recovery.rs`.**
- `commit_order` test (AK-041's locator names it): through the COMPOSED
  loop (real controller, real stores), a settlement observably appends the
  durable event BEFORE the status row BEFORE the in-memory advance. Make
  the ordering observable honestly (the service returns the durable offset;
  the store's row appears only after; the projection advances only after).
  **Teeth:** reorder the §16.3 sequence (or neuter the order guard) and
  watch `commit_order` FAIL; revert byte-identical.
- Kill/resume legs (AK-042 / P7): for EACH crash window — before step 1
  (retry-as-if-never-verified), between 1 and 2 (half-applied; replay
  completes), after 2 (durable, memory lost) — kill and resume through the
  production `resume_run` path. Use the real process-abort `crash_helper`
  for the store-level windows against the SAME store file the composed run
  uses, then a FRESH controller composition (drop + recompose = the honest
  in-process restart, the established pattern) driving `resume_run`;
  assert: exactly one settlement record, no duplicate settlement event, no
  re-dispatch of the settled unit, run identity + prior evidence intact.
  If you add any new child-binary surface to make a real abort mid-loop,
  justify and record it as a deviation (prefer none).
- Anchored replay leg: resume after a checkpointed settlement; events below
  the anchor are not re-scanned (B1's anchor discipline, composed).
- Lost-evidence leg: resume loses NO evidence (pinned evidence refs still
  resolve; the §18.3 "run identity, prior evidence, and payment history
  survive restart intact" sentence).

**Item 2 — `crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`.**
Composed controller; a comparator that returns `pass` (clean comparison,
admitted through the real comparison chain) while NO verification record
exists (no evidence production / verifier refuses / ladder absent — pick
the honest shapes, cover ≥2): the unit does NOT settle — no
`SettlementCommitted` event, no settlements row, projection stays open.
Add the INCONCLUSIVE leg (verification returns INCONCLUSIVE → no settle,
distinct from FAIL — AK-005's three-state rule at this seam) and the
"comparator pass + FAIL verification" leg (refusal names the real cause).
**Teeth:** neuter the service's reads-verification-records gate (make
settle admit on comparator cleanliness) and watch the proof FAIL; revert.

**Item 3 — `crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs`.**
- Prevention leg: a builder-context write attempt targeting the answer-key
  path (OUTSIDE the workspace root) through the effect surfaces the loop
  actually uses → refused by containment; the key bytes on disk are
  untouched.
- Detection leg: a direct tamper of the answer key mid-run (the §11.4
  operator-hand-edit shape or raw file write) → the settle-time / boundary
  tripwire fires → run terminal `aborted{reference_mutated}` with BOTH
  digests pinned as evidence (the B2 A4 mechanism composed).
- **Teeth:** remove the digest-compare in the settle-time consult (make it
  a tautology — compare the controller's own frozen digest against itself)
  and watch the detection leg FAIL (the prereg D4 tautology tooth); revert.

**Item 4 — teeth record + focused gate.**
Record every tooth in `.agents/reports/task13/teeth-b4.md` (break → RED
test name + output tail → revert → cmp byte-identical → green). The plan's
Task-13 gate must pass:
```bash
cargo nextest run --test p7_recovery --test p11_critic_is_not_verifier && \
cargo nextest run -p gauntlet-app settle::
```
(plus g10's own run and the standing battery below).

## Constraints, checkpoints, contract

Standard: never touch `.wayfinder/` (READ it only), `.gauntlet/
reference.lock`, `deny.toml`, nextest config, `AGENTS.md`, `.jolli/`,
`.tmp/`, `.agents/prompts/`; no sub-agents; no parallelism increases;
`just ruler` before/after. Proof files may NOT weaken any existing proof.
Checkpoints: `builder-b4-checkpoints.md` STARTED/DONE per item. Result
contract: `builder-b4.md`.

```bash
just ruler
cargo nextest run --test p7_recovery --test p11_critic_is_not_verifier --test g10_builder_writes_answer_key
cargo nextest run -p gauntlet-app settle::
cargo nextest run --test p4_corrective_loop --test p6_budget_control --test p12_missing_is_not_passing --test p2_context_isolation --test p3_real_observation
just check && just lint && just boundaries && just deny
```
