# Task 22 — COR-2 correction report

2026-09-03. Builder: COR-2. Contract: `cor2-brief.md` (C6, C7), findings
F2-1/F2-2 of `falsifier-round2.md`. Progress log: `cor2-progress.md`.

## C6 — a zero-reason refusal must not compose (F2-2) — DONE, RED→GREEN

- **Typed composition error**: new variant
  `StatusTransitionError::EmptyRefusal` in
  `crates/gauntlet-ports/src/capability_ledger.rs` (doc + Display arm);
  `StatusTransition::new` now returns it for
  `TransitionReason::Refused(vec![])` before any target check. The
  `TransitionReason` enum doc records the never-empty rule.
- **RED first, named failure observed**: the suite case was written first;
  with the variant present but the check not yet wired,
  `capability_ledger_green_is_clean` FAILED with exactly F2-2's
  reproduction:
  - "an EMPTY refusal set composed into a transition record — a
    zero-reason refusal is a record the honest service can never produce"
  - "the zero-reason refusal APPLIED — the durable decision record gained
    a refusal with no reasons"
  then the check was wired and the suite went GREEN.
- **Suite case on all legs**: `a_zero_reason_refusal_does_not_compose`
  added to the shared suite (`crates/gauntlet-ports/src/contract/
  capability_ledger.rs`, wired into `capability_ledger_suite`). It asserts
  the typed `EmptyRefusal` from Observed AND from the F2-2-verbatim
  Provisional→Provisional re-record shape, that a composed (buggy) empty
  refusal must not land durably, that the honest non-empty refusal still
  composes and applies, and that the decision record carries exactly the
  one honest refusal. It runs on every factory: ports green/broken/
  caps-broken stubs and the sqlite adapter factory.
- **Sqlite adapter verified, no change needed**: `StatusTransition` has
  private fields and `new` is the only constructor (`decode` re-validates
  through `new`, so a stored zero-reason payload now reads as `Corrupt` —
  fail closed). Every `apply_transition` call on every leg has therefore
  already passed the shared composition validation; the sqlite leg also
  runs the new suite case through `SqliteCapabilityLedgerFactory`
  (green: `sqlite_capability_ledger_passes_the_shared_contract_suite`).
- **Module test — honest service path unaffected**:
  `the_honest_service_path_is_unaffected_by_the_zero_reason_refusal_gate`
  in `crates/gauntlet-app/src/memory/promotion.rs` tests — the honest
  two-settlement evaluation refuses, writes exactly one durable
  `Refused` transition with the reasons whole; the F2-2 zero-reason
  re-record composes nowhere and writes nothing; the full-satisfaction
  path still promotes with witnesses.

## C7 — chronology doc claim softened to the truth (F2-1) — DONE, docs-only

No behavior change, no assertion changes, tiebreak code untouched.

- `crates/gauntlet-app/src/memory/promotion.rs`: module doc header
  "Windows decide on substrate chronology, never import order" replaced by
  the honest tie rule — windows order by `(occurred_at, ledger seq)`;
  the seq tiebreak is deterministic but IS append order, so an
  `occurred_at` tie inside the decisive pair is order-dependent by
  construction (recorded residual, F2-1); the ordering removes only the
  coarser import-order dependency for facts with distinct instants.
  Same softening in the `ordered_by_occurrence` doc, the
  `settlement_window_facts` doc tail, and the `evaluate()` chronology
  comment.
- `crates/gauntlet-ports/src/capability_ledger.rs` module doc: the
  identical categorical sentence ("a verdict must be a function of the
  recorded facts, not of the call order that happened to land them")
  softened identically — F2-1 names the port docs as carrying the claim
  too.
- Deliberately NOT touched: test bodies and test comments (including
  `windows_decide_on_substrate_chronology_never_append_order`), the
  one-line occurred_at stamp comment in `developmental.rs`, and the
  tiebreak implementation.

## Files changed (sha256 at report time)

| file | change | sha256 |
|------|--------|--------|
| `crates/gauntlet-ports/src/capability_ledger.rs` | C6 typed variant + composition check + never-empty doc; C7 port-doc softening | `b9721a9dd81404639bd95608685131286d5d6319c854a11da043aeec60bc9cda` |
| `crates/gauntlet-ports/src/contract/capability_ledger.rs` | C6 suite case wired into the shared suite | `99dff48a616eeb18b4536cc6da2970d2c4da4b2913f744de766ea974685084bb` |
| `crates/gauntlet-app/src/memory/promotion.rs` | C6 module test; C7 doc softening (4 sites) | `39d7709d728172dfc8530a58fc9e38f08eaa77264a8550241a05fc4608de5b71` |
| `.agents/reports/task22/cor2-progress.md` | checkpoint | `a10b5fe76a220b7b2b33bce1168293a41ab15929edca5f9b100e5f53d600d483` |
| `.agents/reports/task22/cor2-report.md` | this report | (this file) |

No other file touched. `.wayfinder/*`, `.gauntlet/reference.lock`, the
settlement path, `producer_authority`, state-sqlite settlement code, and
`.agents/reports/reference-candidates.md` untouched (its `M` in `git
status` is the pre-existing task22 delta recorded before COR-2 began).
Nothing staged or committed (0 staged files verified); no sub-agents;
zero new dependencies. Every cargo/nextest/just invocation ran under
`flock /tmp/gauntlet-build.lock sh -c '<cmd>; sccache --stop-server
>/dev/null 2>&1; true'`.

## Gates (all GREEN; verbatim tails)

Start: `just ruler`
```
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

RED (suite case in, check not yet wired):
```
FAIL [   0.007s] (3/3) gauntlet-ports contract::suite_runs::capability_ledger_green_is_clean
    CapabilityLedgerGreen is the reference stub and must run its suite clean, but reported:
    [capability_ledger::a_zero_reason_refusal_does_not_compose] … an EMPTY refusal set composed into a transition record …
    [capability_ledger::a_zero_reason_refusal_does_not_compose] … the zero-reason refusal APPLIED — the durable decision record gained a refusal with no reasons
error: test run failed
```

GREEN (after wiring the check) — `cargo nextest run -p gauntlet-ports capability_ledger`:
```
        PASS [   0.036s] (1/3) gauntlet-ports contract::suite_runs::capability_ledger_green_is_clean
        PASS [   0.040s] (2/3) gauntlet-ports contract::suite_runs::capability_ledger_broken_is_caught
        PASS [   0.047s] (3/3) gauntlet-ports contract::suite_runs::capability_ledger_caps_broken_is_caught
────────────
     Summary [   0.081s] 3 tests run: 3 passed, 112 skipped
```

Plan gate — `cargo nextest run --test p10_no_false_learning -p gauntlet-cli`:
```
        PASS [   0.073s] (4/4) gauntlet-cli::p10_no_false_learning revoke_is_sticky_and_has_no_flip_back
────────────
     Summary [   0.083s] 4 tests run: 4 passed, 0 skipped
```

Plan gate — `cargo nextest run -p gauntlet-app memory::`:
```
        PASS [   0.023s] (34/34) gauntlet-app memory::promotion::tests::the_honest_service_path_is_unaffected_by_the_zero_reason_refusal_gate
────────────
     Summary [   0.152s] 34 tests run: 34 passed, 480 skipped
```

Sqlite leg — `cargo nextest run -p gauntlet-adapter-state-sqlite --test contract_suite`:
```
        PASS [   0.159s] (3/3) gauntlet-adapter-state-sqlite::contract_suite sqlite_capability_ledger_passes_the_shared_contract_suite
────────────
     Summary [   0.167s] 3 tests run: 3 passed, 0 skipped
```

`just boundaries`:
```
     Summary [  63.924s] 228 tests run: 228 passed, 0 skipped
```

`just lint` (first run failed on `cargo fmt --check` over the new test;
applied `cargo fmt -p gauntlet-app -p gauntlet-ports`, then clean):
```
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 2m 32s
cargo fmt --check
```

After the fmt reformat, all package gates re-run GREEN: memory:: 34/34;
ports capability_ledger 3/3; sqlite contract_suite 3/3; p10 4/4.

End: `just ruler`
```
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

## Deviations

1. **C7 scope**: the binding text names the promotion module doc; the
   identical categorical sentence also stands in the port module doc
   (`crates/gauntlet-ports/src/capability_ledger.rs`), which F2-1 names
   explicitly ("promotion.rs module docs; port docs"). Both were softened
   with the same docs-only edit so the falsified claim does not survive
   in its other recorded location. No code, no assertions.
2. **RED mechanics**: the typed variant had to exist before the suite
   case compiles, so the RED observation was taken with the variant
   declared but the composition check NOT yet wired — the detector
   observed the current tree's true behavior (empty refusal composes AND
   applies) before the fix, then GREEN after wiring.
3. **Formatting**: `just lint` failed once on `cargo fmt --check` over
   the new module test; resolved by running rustfmt on the two touched
   crates and re-running all package gates green. No assertion or
   behavior changed in the reformat.
