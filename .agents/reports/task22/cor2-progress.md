# Task 22 — COR-2 progress checkpoint

2026-09-03. Builder: COR-2. Contract: cor2-brief.md (C6, C7).

## C6 — zero-reason refusal must not compose (F2-2)

- RED first: new shared-suite case `a_zero_reason_refusal_does_not_compose`
  in `crates/gauntlet-ports/src/contract/capability_ledger.rs`, wired into
  `capability_ledger_suite` (runs on ALL legs: green stub, broken stub,
  caps-broken stub, sqlite adapter factory).
- Typed variant `StatusTransitionError::EmptyRefusal` added to
  `crates/gauntlet-ports/src/capability_ledger.rs` (+ Display arm).
- RED OBSERVED (behavioral): with the variant present but the check not yet
  wired, `capability_ledger_green_is_clean` FAILED with the named findings:
  "an EMPTY refusal set composed into a transition record" and "the
  zero-reason refusal APPLIED — the durable decision record gained a
  refusal with no reasons" — F2-2's reproduction, verbatim.
- GREEN: the empty-set check wired into `StatusTransition::new`
  (Refused(vec![]) → Err(EmptyRefusal) before any target check).
  `cargo nextest run -p gauntlet-ports capability_ledger` → 3/3 PASS.
- Module test added: `the_honest_service_path_is_unaffected_by_the_zero_
  reason_refusal_gate` in `crates/gauntlet-app/src/memory/promotion.rs`
  tests — honest refusal (2 settlements) evaluates, writes exactly one
  durable Refused transition with the reasons whole; the F2-2
  Provisional→Provisional zero-reason re-record composes nowhere
  (EmptyRefusal) and writes nothing; full satisfaction still promotes.
- Decode path re-validates through `StatusTransition::new`, so a stored
  zero-reason payload now reads as `Corrupt` (fail closed). Sqlite
  adapter needs NO code change: `StatusTransition` has private fields and
  `new` is the only constructor, so every `apply_transition` call on every
  leg has already passed the shared composition validation; the sqlite
  leg additionally runs the new suite case through its factory.

## C7 — chronology doc claim softened to the truth (F2-1)

Docs-only, no behavior, no assertion changes:
- promotion.rs module doc: "never import order" → the honest tie rule
  ((occurred_at, ledger seq); seq tiebreak deterministic but IS append
  order; ties order-dependent by construction; recorded residual F2-1).
- promotion.rs `ordered_by_occurrence` doc, `settlement_window_facts` doc
  tail, and the `evaluate()` chronology comment: same softening.
- Port doc `crates/gauntlet-ports/src/capability_ledger.rs` module doc:
  the identical categorical sentence ("a verdict must be a function of
  the recorded facts, not of the call order that happened to land them")
  softened identically — F2-1 names both docs.
- NOT touched: test bodies/comments (incl.
  `windows_decide_on_substrate_chronology_never_append_order`), the
  one-line occurred_at stamp comment in developmental.rs, tiebreak code.

## Status

Gates pending: plan gates, sqlite contract_suite, just boundaries,
just lint, just ruler (end). just ruler at start: PASS.
Nothing staged/committed; no sub-agents; zero new deps.
