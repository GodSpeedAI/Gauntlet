# Task 7 — Runaway-memory defect in the manifest parser: root-cause report

**Date:** 2026-08-18 · **Found by:** operator (instrumented diagnosis), root-caused and
fixed by the orchestrator under ESC-002's Task-7 implementer authorization plus the
operator's direct instruction ("fix the ROOT CAUSE; do not treat this as an
infrastructure problem").

## Symptom (operator's evidence, not re-derived)

`reference::provider::tests::the_production_provider_passes_the_shared_contract_suite`
(and, once bisected, `freeze_is_content_addressed_and_idempotent_for_the_same_run`)
never terminate; RSS grows ~1.7 GiB → ~4.9 GiB in ~36 s with 2 threads; the worker
thread continuously commits pages while the main test thread waits; unattended, it
exhausts the WSL envelope. `cargo test --no-run` compiles in ~7 s / ~319 MiB — the
defect is in test execution, not the build.

## Root cause

`crates/gauntlet-app/src/reference/manifest.rs`'s tree traversal was written against
two conflicting position conventions at once:

- `parse_tree` called `value_of` with `pos` **on** the key line;
- `mapping` called `value_of` with `pos` **after** the key line (it pre-advanced).

Under the `parse_tree` convention, `value_of`'s nested-mapping arm handed `mapping()`
a `pos` still pointing at the (shallower) parent key line. `mapping`'s first loop
check — `line.indent < indent → break` — fires immediately, so it returned an **empty
map having consumed zero lines**. `parse_tree`'s `while pos < lines.len()` then
re-read the `reference:` line forever, and each iteration executed
`entries.push((key.to_owned(), node))`: two fresh heap `String`s plus vector growth,
unbounded. That is the page-commit runaway `strace` saw.

Two latent defects in the same core were unreachable behind the loop (the parse never
got past line 1) and surfaced once it could proceed:

- the folded-block trigger looked for `>` on the line *after* the key, but the frozen
  format writes it in the key line's remainder (`purpose: >`) — folded narratives
  would have mis-parsed as the literal scalar `">"` and then failed shape checks;
- `block_sequence` was entered with `pos` on the key line, so it read the wrong
  `item_indent`.

Why nothing caught this until now: D3's own unit tests and D4's provider tests were
**never executed** — the 2026-08-17 gate runs covered D1 (6/6) and D2 (20/20) only;
D3 landed uncheckpointed during the ESC-002 spawn deaths, and the write-only
finisher-2 session (D4R–D8) ran no cargo at all. The contract test was the first
execution of `parse_manifest` on any input.

## Fix (files and lines)

All in `crates/gauntlet-app/src/reference/manifest.rs`:

1. `value_of` — one position contract, stated in its doc comment: entered with `pos`
   ON the key line, **every** return path leaves it past the key line and the key's
   entire value block. The `>` marker now arrives via `rest == ">"`; all arms
   advance before recursing/returning.
2. `folded` — entered with `pos` on the first body line; consumes the run of lines
   strictly deeper than the key, trims, joins with single spaces. (The old
   `is_empty`-continue branch was dead code — `lex` drops blank lines.)
3. `mapping` — no longer pre-advances past the key line before calling `value_of`.
4. `block_sequence`'s mapping-item collector — collects the item's whole subtree
   (anything deeper than the dash) **preserving physical indents**, instead of
   requiring every continuation to sit exactly at `key_indent` (folded bodies inside
   items sit deeper) and normalizing indents to `key_indent` (which would have
   broken `folded`'s base comparison). Shape strictness is unchanged in effect: a
   stray deeper line that is not a folded body still fails `mapping`'s
   sits-deeper-than-its-mapping check.

One test correction, demonstrated wrong before changing: `unknown_keys_and_roots…`
pinned `UnknownKey { line: 2 }` for the `flavour` drift on `version`'s line, but that
line is physical line 3 and the sibling test pins `NotAnInteger { line: 3 }` for the
same line — the pin was off by one (never-run test). Corrected to `line: 3`; the
locus remains exactly asserted.

## Why the fix preserves ReferenceProvider semantics

The change is confined to tree *construction*; `Node` variants, the
`ManifestDefinition` validators, error variants, and their loci are untouched. The
frozen-manifest test (`the_frozen_manifest_parses_with_its_fields_intact`) asserts
every section count against `.wayfinder/reference.yaml` itself and passes — 12
invariants rendered `"<id>: <statement>"`, 7 constraints, 6 anti-goals, 3 success
criteria, 3 evidence locators, `minimum_reliability_passes = 3`, 6 terminal
conditions in order, 3 unknowns, 3 assumptions, 4 quality dimensions. The shared
`reference_provider_suite` (ports, unmodified) passes against the production
provider. No assertion anywhere was weakened; one internally contradictory test pin
was corrected with evidence.

## Before / after

| | before | after |
|---|---|---|
| provider contract test | never terminates; 1.7→4.9 GiB RSS @ ~36 s, unbounded | passes; whole cargo-process-tree peak ≈ 30 MiB, wall ≈ 0.02 s |
| manifest unit tests | never ran (would hang) | 10/10 in 0.00 s |
| `reference::` suite | — | 36/36 in 0.04 s |

## Verification commands and results (orchestrator-run, 2026-08-18)

- `cargo test -p gauntlet-app --lib reference::manifest` — 10 passed.
- `cargo test -p gauntlet-app --lib 'reference::provider::tests::the_production_provider_passes_the_shared_contract_suite' -- --exact` under `/usr/bin/time -v` — 1 passed, max RSS 30 452 KiB.
- `cargo nextest run -p gauntlet-app --lib reference::` — 36/36.
- `cargo nextest run -p gauntlet-app` — 109/109.
- `cargo nextest run -p gauntlet-cli --test p1_reference_integrity` — 4/4.
- `diff <(scripts/reference-digest.sh .wayfinder/answer-key.md) <(cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md)` — empty (agree; both equal the frozen lock `sha256:ccc7c99b…`).
- `just check && just lint && just deny && just boundaries && just ruler` — all 0; boundaries 190/190.
- `cargo nextest run --workspace` — 476/476.
- `just doctor` — exit 0; check 2 passes all four clauses (manifest / answer key 70 / unit schema 70/70 / digest-vs-lock).
- Teeth (each observed failing, then reverted, then full re-verification green):
  boundary-call removal → `digest_rechecked` fails; per-line `trim_end()` → both
  pinned normalize tests fail AND the script-vs-CLI diff gate disagrees on a
  synthetic trailing-space fixture (`5518ad7a…` vs `e49c81e2…`) — this extends the
  composed gate beyond the frozen key, which carries no trailing-whitespace lines;
  F4 freeze-guard removal → the freeze tripwire test fails; doctor digest-clause
  neutering → the moved-key test fails.

## Residual risks (bounded claims)

- The manifest dialect remains a purpose-built subset parser (block scalars `>`
  only, no `|`, no anchors, flow sequences of scalars only) — unchanged scope,
  now actually exercised. Deep-shape drift beyond the frozen vocabulary is a typed
  error, not a silent pass, on the paths the tests pin; shapes no test pins are
  unpinned by definition.
- The session was interrupted repeatedly before/during this work (ESC-002 class);
  all state reconstructable from files — verified by this recovery.
- Two clippy fixes (no-op self-replace removal; `MemoryStore::new`→`fresh`) and
  three boundary-scanner shape fixes (`"1/2"` needle built not spelled; "just"
  reworded out of an expect message; `FIELD_KEYS` const→fn per the tier-class
  residual's sanctioned expression position) accompanied the defect fix — all
  semantics-preserving, all listed for the blind critics.

## Correction round appendix (post-dating the report above)

Two blind critics (A: spec; B: adversarial — `.agents/reports/task7/critic-{a,b}-result.md`)
reviewed the artifact cold; the correction round closed, with fail-first tests:

- **B-F1 CRITICAL** — a reachable `unreachable!` panic: `reliability_from`
  accepted `">= 0 settlements"`, so a two-token manifest edit reached
  `SettlementCriteria::new(.., 0, ..)` through `resolve` and panicked. Closed:
  `settlement_of` refuses `minimum_reliability == 0` with the new typed
  `ReliabilityBelowOne { line }` before `build_definition`; pinned in the
  reliability drift test (`line: 11` locus).
- **A-M3 + B-F2 MAJOR (convergent)** — the lock's `manifest_digest` was
  whitelisted but never verified: the ruler's narrative half had no integrity
  check anywhere. Closed: `read_reference_lock` requires and parses
  `manifest_digest`; check 2's digest clause recomputes BOTH halves (manifest
  bytes now read once and shared) and fails `REFERENCE MOVED` on either
  mismatch; new test `a_moved_manifest_fails_check_two_as_a_reference_class_failure`
  (fail-first demonstrated: neutering the manifest-half compare fails it).
- **B-F3 MAJOR** — the lex tab guard was dead code (`line[..indent]` was
  spaces by construction). Closed: the leading whitespace run is measured over
  spaces AND tabs; a tab anywhere in it is a typed error; pinned for
  tab-first, spaces-then-tab, and (confirmation-critic follow-up) NBSP stays
  in the body and fails loudly — the guard does not widen what counts as
  indentation.
- **A-M1 MAJOR** — AK-025 had no recorded disposition. Closed: dated
  reference-candidates entry (locator-materialization class; both claim-halves
  dispositioned honestly).
- **B-F6 / B-F17 / A-M2-minor** — empty flow entries refused (+ test); the P1
  confinement assertion tied to the evidence-store root (SUT-owned placement);
  the provider header's strict-admission claim made accurate.

Confirmation critic (blind): **CONFIRM** — all corrections closed, no weakened
assertions; its three new minors: the NBSP indentation widening (fixed +
pinned above), a misplaced doc comment on `read_manifest` (fixed), and the
missing correction-round deferral ledger (written into CURRENT_STATUS.md).

Final state: workspace **479/479**, boundaries **190/190**, check/lint/deny 0,
`just ruler` PASS, `just doctor` exit 0 with check 2 fully closed on both
ruler halves, script-vs-CLI digest gates agree on the frozen key AND on a
synthetic trailing-space fixture.
