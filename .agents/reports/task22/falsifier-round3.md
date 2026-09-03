# Task 22 — falsifier round 3 (focused, post COR-2)

Blind executing falsifier, fresh context. Scope: verify COR-2 (C6 zero-reason
refusal guard, C7 honest tie/ordering docs) holds and nothing regressed. Round-2
summary line trusted for targeting only; all facts below re-derived from code
and executed commands. No `.agents/reports/task22/` files read except
`charter.md`.

## Executed attacks (one line each)

1. `just ruler` start — PASS (frozen lock digests match).
2. Scratch integration test over the REAL sqlite adapter
   (`gauntlet-adapter-state-sqlite/tests/falsifier_round3_scratch.rs`, deleted
   after the round), 4/4 PASS:
   1.1 F2-2 verbatim: compose `StatusTransition{Provisional→Provisional,
       Refused(vec![])}` → typed `Err(EmptyRefusal)`; (would-be) apply over the
       real ledger unreachable; `transitions()` stays empty — nothing appended.
   1.2 The `Observed→Provisional` zero-reason shape → typed `Err(EmptyRefusal)`;
       durable history stays empty.
   1.3 Control: honest non-empty `Refused([InsufficientSettlements{1,3}])`
       composes, applies over the real ledger, status `Provisional`, exactly one
       durable row — the guard did not disable the honest path.
   1.4 DECODE PATH: honest refused payload spliced to `"refusals":[]` →
       `StatusTransition::decode` = `Err(Corrupt)`; row hand-inserted into
       `capability_transitions` via a raw second connection (tamper_evidence
       precedent) → real-adapter read-back `capability()` AND `transitions()`
       both `Err(Corrupt)`; honest payload inserted the same way reads back fine
       (failure is content-driven, not insert mechanics); a later honest row
       does not launder the corrupt one.
3. C6 TOOTH: neutralized the guard in `StatusTransition::new`
   (`if refusals.is_empty()` → `if false && refusals.is_empty()`), sha256
   before `b9721a9dd81404639bd95608685131286d5d6319c854a11da043aeec60bc9cda`.
   All three detectors RED, each naming `a_zero_reason_refusal_does_not_compose`
   / "a zero-reason refusal composed": ports green leg, app module detector
   (`the_honest_service_path_is_unaffected_by_the_zero_reason_refusal_gate`),
   sqlite real-adapter suite leg. Suite also caught the knock-on
   (`StaleTransition` on the honest refusal after the zero-reason one landed).
   Restored byte-identically (sha256 after = before, diff clean); same three
   detectors GREEN.
4. C7 VERIFICATION: promotion.rs header docs (l.23–38), `ordered_by_occurrence`
   (l.409–416), `settlement_window_facts` (l.442–444), and the port docs
   (`capability_ledger.rs` l.44–52) all state the honest claim: `(occurred_at,
   ledger seq)` ordering, seq tiebreak IS append order, an `occurred_at` tie is
   order-dependent by construction — a recorded residual (F2-1/COR-2 C7), not a
   retired property. Behavior matches the claim exactly: stable `sort_by_key`
   over `occurred_at` only, read-back `ORDER BY seq ASC`. No categorical
   order-invariance claim remains anywhere (rg sweep over memory modules, p10,
   port). Docs-only: no assertion contradicts the tie residual; the C1
   chronology test `windows_decide_on_substrate_chronology_never_append_order`
   asserts chronology over occurrence-DIFFERING facts (halves (0,2)/(1,2)) and
   passes — consistent with, not in conflict with, the tie residual.
5. REGRESSION SWEEP — all green:
   - `cargo nextest run --test p10_no_false_learning -p gauntlet-cli` 4/4
   - `cargo nextest run -p gauntlet-app memory::` 34/34
   - ports `capability_ledger` suite legs 3/3 (green clean / broken caught /
     caps broken caught)
   - sqlite `--test contract_suite` 3/3
   - `just boundaries` 228/228
6. `just ruler` end — PASS.

## Findings

None. No new residuals found; COR-2's mechanism holds under attack and
nothing regressed. (Recorded-boundary reminders only, both already documented
in the port docs, not new: read-back fail-closed is the sole tripwire for a
row written through a raw substrate channel; the developmental log remains
forging-writable by a trusted-code writer — the sealed-mint candidate.)

## Restoration declaration

- `crates/gauntlet-ports/src/capability_ledger.rs` mutated and restored;
  sha256 before and after both
  `b9721a9dd81404639bd95608685131286d5d6319c854a11da043aeec60bc9cda` —
  byte-identical.
- Scratch test `falsifier_round3_scratch.rs` deleted.
- `git status --porcelain` after = byte-for-byte the pre-existing task22 delta
  snapshot taken at round start (no new entries, nothing staged/committed).
- Never touched: `.wayfinder/*`, `.gauntlet/reference.lock`, the settlement
  path, `reference-candidates.md`.

VERDICT: CONFIRM — attacks executed: 10; new findings: 0; regressions: 0;
restorations: 1/1 byte-identical; gates: ruler 2/2 PASS.
