# Task 13 / Finisher brief B2-recovery — verify-and-record, then complete B2

You are the B2 FINISHER. The original B2 builder died to a usage limit
mid-flight (16th interruption instance). The recovered state is recorded in
`builder-b2-checkpoints.md` under "RECOVERED-AND-VERIFIED (orchestrator)" —
read it FIRST, then `builder-brief-b2.md` (the original scope, Items 1–5),
then this brief. The tree outranks every record: re-verify anything you
doubt, never redo what is recorded as landed.

## Phase 0 — recovered state (do not re-implement)

`crates/gauntlet-app/src/settle/mod.rs` EXISTS (Item 1 complete in shape:
service, operator-action path, resume completion, 16 tests) and is declared
in `lib.rs`; `controller.rs` gained only the `escape_json_text` pub(crate)
widening. Compile is GREEN with one dead-code warning (unused test helper
`pinned_records_containing`). **The 16 tests were never green: 4 pass / 12
fail, all fixture-level** — `earned_claim`'s
`VerificationAuthority::for_run` refuses with `ReferenceRefused(
ContractMismatch)` because the fixture's frozen-reference evidence digest
disagrees with the `ReferenceVersion` it constructed (plus two sibling
fixture loci at mod.rs:2623 and :2700).

## Phase 1 — make the fixture honest (fail-first re-derived)

Fix the FIXTURE (not the service, unless the service is genuinely wrong):
the frozen-reference evidence the fixture inserts must be the actual
normalized bytes whose digest IS the `ReferenceVersion`'s digest (follow how
`verify::authority`'s own in-crate tests and `p12_missing_is_not_passing`
earn a governed run — reuse their fixture patterns; do not loosen the
three-way digest equality). The two sibling fixture loci (2623, 2700) are
the same family. Record honestly in the checkpoints: the original builder's
fail-first observations are LOST; your RED evidence is the recovered RED
(4/12) you inherit, and your first DONE entry records fixture→green plus
which service behaviors the now-green tests actually discriminate. Delete or
use the dead helper (do not leave the warning). Gate:
`cargo nextest run -p gauntlet-app settle::` 16/16.

## Phase 2 — Items 2–5 of `builder-brief-b2.md`, verbatim

Item 2 (controller PASS-leg wiring through the service; last_offset
threading), Item 3 (resume completion: anchored settlements + evidence
productions + projection reconciliation), Item 4 (§21.3 dispositions:
answer-key read failure non-terminal; poll transport error via §11.5;
update the pinned crash-semantics test preserving its resume assertion),
Item 5 tests (AK-062 no-bypass — the service surface exists; prove it).
All constraints of the original brief apply unchanged (fail-first per
behavioral change; `control::` 75+, `verify::`, `compare::`, p4/p6/p12/p2
green; no new migration; forbidden paths; no sub-agents; ruler before and
after).

## Checkpoint discipline

Append to `builder-b2-checkpoints.md` after EACH phase/item: STARTED before
editing, DONE after (test names + RED/GREEN evidence). Never batch.

## Result contract

`builder-b2.md` — write it at the end (it does not exist yet): recovered
state inherited, Phase 1 outcome, Items 2–5 delivery, deviations, gate
tails verbatim, files touched, known gaps.

## Gates (run personally, record verbatim tails)

```bash
just ruler
just check-package gauntlet-app
just test-package gauntlet-app
cargo nextest run --test p4_corrective_loop --test p6_budget_control --test p12_missing_is_not_passing --test p2_context_isolation
just lint
```
