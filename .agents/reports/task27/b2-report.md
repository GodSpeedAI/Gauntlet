# B2 report — §25 docs set + docs_present (task27)

Completion contract for charter slice B2. A claim with NO settlement
authority. Scope held: only the files below were created or modified; the
lowercase `docs/` tree, `.wayfinder/*`, `.gauntlet/*`, `CURRENT_STATUS.yml`,
the charter, `reference-candidates.md`, `crates/gauntlet-cli/**`, and
`crates/gauntlet-app/**` were never touched (verified by `git status`
footprint audit at the end).

## 1. The eight files (all NEW, uppercase; per-unit truth NOT duplicated)

| File | Content (one line) |
|---|---|
| `docs/ARCHITECTURE.md` | Layers/crates/ports/adapters inventory, mechanical boundary enforcement, control flow; truth-locator disclaimer. |
| `docs/CONFORMANCE.md` | The charter decision-8 binding set: trust posture; sandbox backends + §17.3 tier mapping; Core/Extended/Self-improving profile adjudication (item-by-item met/partial/absent + evidence pointer); six enumerated advisory-only enforcements with triggers; UNK-001 terminal disposition under the stable phrase `cross_cutting containment — a declared substitution`, citing `.agents/reports/task14/structural-review-unk001.md`; frozen manifest never edited to record it. |
| `docs/CONFIGURATION.md` | §8 precedence, GAUNTLET.md contract, Annex E groups, tool capabilities, defaults table, doctor preflight, runners/UNK-003 posture. |
| `docs/SECURITY.md` | §17/§22 rules, workspace invariants, recorded boundaries (judge-forgery, host-readability), failure posture. |
| `docs/FAILURE-RECOVERY.md` | §21 classes + degradation table, §18 substrate/checkpoints/resume, the recorded re-production-not-adoption semantics (F-C4-1), budget governance. |
| `docs/OPERATOR-GUIDE.md` | §20.1 surface via the committed `cli_help.txt` golden snapshot (no verify-line quote — B1 is regenerating it), operator's role, everyday flows. |
| `docs/SELF-IMPROVEMENT.md` | Two stores, §19.2 promotion conjunction, optimizer proposals-only + eight-condition gate, dogfooding posture (honest partial), baselines incl. D-does-not-win, mutation posture. |
| `docs/HANDOFF.md` | Thin pointer ONLY: six §26 category rows with "see `.agents/reports/handoff.md`" count placeholders, the canonical locator named explicitly, truth-locator map. |

Every one of the eight carries `## Proven vs provisional`. AK-067 is
mentioned in HANDOFF.md with NO status word adjacent (the canonical
`.agents/reports/handoff.md` is wave-2 B3's file at authoring time —
claiming a status for it here would be ahead of evidence).

## 2. Status-claim inventory (33 claims, all `implemented_but_provisional`)

Every unit-level claim deliberately takes the record's own convention —
"55 units provisional through Task 26" (CURRENT_STATUS observations) — per
the charter's honesty rule. The sweep's `conformance.json` is the only
per-unit truth; where it adjudicates `implemented_and_proven` (PASS +
unbounded), B3's wave-2 cross-check may need to upgrade these lines
mechanically — upgrade-with-sweep-evidence-in-hand is the safe direction;
the reverse (doc ahead of conformance.json) never occurs in this set.

| Doc | Unit | Backing record |
|---|---|---|
| ARCHITECTURE | AK-001, AK-002 | task 2 delivery; boundaries 237/237 in every closure battery through task 26 |
| ARCHITECTURE | AK-014 | task 2/16; `tests/contract/adapter_registry` target |
| ARCHITECTURE | AK-013 | task 16 (P8 mock+cli, zero test-body changes; task 15 prime adapter) — "both provisional" per task-16 settled summary |
| CONFORMANCE | AK-005 | task 11 composed p12 proof (`p12_missing_is_not_passing.rs`) |
| CONFORMANCE | AK-058 | task 24 (antigaming 19 tests + mutation 73/63/10/missed 0) — "53 provisional through Task 24 (51 + AK-058 + AK-070)" |
| CONFORMANCE | AK-066 | task 25 real-binary/live-bwrap E2E — "54 provisional through Task 25 (… + AK-066)" |
| CONFORMANCE, SELF-IMPROVEMENT | AK-069 | task 26 baselines.json incl. D-does-not-win — "55 provisional through Task 26 (… + AK-069)" |
| CONFORMANCE, SELF-IMPROVEMENT | AK-070 | task 24 recorded mutation artifact; charter decision 11 adjudicates it bounded ⇒ provisional |
| CONFIGURATION | AK-015 | task 5 config module + per-layer unit tests (resolve/canonical/fingerprint/env_layer/gauntlet_md) |
| CONFIGURATION | AK-016 | task 5 + task 26 COR-3 (290 → 0 literal sites, 26 inventoried writes) |
| CONFIGURATION | AK-017 | task 5 doctor; 2026-08-17 reference-candidates locator entry |
| CONFIGURATION | AK-019 | task 5; `tests/contract/env_example_complete` target |
| SECURITY | AK-022, AK-023 | task 7 p1 proof; composed tasks 13/20; real-binary task 25 |
| SECURITY | AK-043 | task 13 g10 + task 20 single-writer arch rule (`answer_key_single_writer`) |
| SECURITY | AK-044, AK-047, AK-048 | task 14 p5; task 25 real-world re-evidence ("real-world AK-044/AK-047/AK-048" per task-25 settled summary) |
| FAILURE-RECOVERY | AK-041, AK-042 | task 13 p7; task 25 real-binary recovery legs (spec-honest re-scope recorded) |
| FAILURE-RECOVERY | AK-051 | task 12 p6 + task 25 `e2e_budget` real binary |
| FAILURE-RECOVERY | AK-053, AK-054 | task 6 state module + contract suite; composed task 20; `export_stack` |
| OPERATOR-GUIDE | AK-060, AK-061 | task 20 — "47 provisional through Task 20 (… plus AK-060/AK-061)"; cli_surface 16/16 + mock E2E |
| OPERATOR-GUIDE | AK-062 | task 21 — "48 provisional through Task 21 (… + AK-062)" |
| SELF-IMPROVEMENT | AK-055 | task 22 — "49 provisional (… + AK-055)"; p10 4/4 |
| SELF-IMPROVEMENT | AK-056, AK-057 | task 23 — "51 provisional (… + AK-056 + AK-057)"; p9 4/4, promote:: 46/46 |
| HANDOFF | AK-068 | this slice: docs_present target 5/5 GREEN with RED-first teeth (below) |

Non-claim AK mentions (no status word adjacent, verified by the vocabulary
test): AK-049 (CONFORMANCE §sandbox prose; CONFIGURATION inside the AK-017
claim line), AK-067 (HANDOFF locator prose), AK-066 (CONFORMANCE claim-line
continuation citing the locator divergence).

## 3. RED-first teeth (each: break ⇒ RED ⇒ byte-identical sha256 restore ⇒ GREEN)

| Tooth | Mutation | Command | RED evidence | sha256 before == after |
|---|---|---|---|---|
| T1 delete a doc | `docs/SECURITY.md` moved away | `cargo nextest run -p gauntlet-arch-tests --test docs_present docs_set_present` | FAIL exit 100 — "docs-set: SECURITY.md unreadable: No such file or directory" | `b84d440aa2efb80939b36de1b499cac7896a0e1b0a126accfed07b53cbfd8a63` (restore `sha256sum -c` OK; re-run PASS) |
| T2 strip status section | `## Proven vs provisional` heading deleted from `docs/CONFIGURATION.md` | `… --test docs_present status_section_in_every_doc` | FAIL exit 100 — "docs/CONFIGURATION.md lacks the `## Proven vs provisional` section" | `b64f7a8c13d45e9fb054a7addff6cf3642c35a51324d00cad32c10f857433626` (OK; PASS) |
| T3 bogus status word | `- implemented_but_provisional — AK-060:` → `- proven — AK-060:` in `docs/OPERATOR-GUIDE.md` | `… --test docs_present status_claims_use_canonical_vocabulary` | FAIL exit 100 — "OPERATOR-GUIDE.md:84 names a ruler unit with a non-canonical status token (proven) but none of the six §26 words" | `dd31b4ec579281bec36472691cdca640e0d7a4bcfac976d44f5e6a742325f297` (OK; full target 5/5 PASS) |

No `git restore` was used at any point (all files untracked or backed up
by `cp`; restores verified by digest). An in-crate unit test
(`docs::tests::a_bogus_status_word_beside_a_unit_id_is_a_violation`)
additionally pins tooth 3's detector over a temp fixture world.

## 4. Deliverables outside the eight docs

- `tests/contract/docs_present.rs` — the AK-068 address file (5 tests:
  presence, status section, handoff pointer, CONFORMANCE binding content,
  status vocabulary), header citing the evidence locator, logic delegated
  to the crate (the architecture_boundaries precedent).
- `crates/gauntlet-arch-tests/src/docs.rs` — the checking logic
  (word-boundary matcher treating underscores as word characters so the
  bare token `proven` never matches inside `implemented_but_provisional`;
  `AK-` detection requires digits so the prose literal `AK-NNN` is not a
  claim) + 3 unit tests. Explicitly does NOT read `conformance.json` —
  that cross-check leg is B3's (charter).
- `crates/gauntlet-arch-tests/src/lib.rs` — `pub mod docs;`, one
  locator-table row, and the `evidence_locators` expected-set extended
  from four to five `[[test]]` entries (that pre-existing test pins the
  manifest's test-target set; the new entry necessarily joins it).
- `crates/gauntlet-arch-tests/Cargo.toml` — the `[[test]] docs_present`
  entry only (five-line diff).

## 5. Slice battery results

- `just check-package gauntlet-arch-tests` — GREEN (after one
  closure-deref compile fix, `.copied()`).
- `cargo nextest run -p gauntlet-arch-tests --test docs_present` — 5/5
  PASS. `--lib docs::` — 3/3 PASS. `--lib evidence_locators::` — 1/1 PASS.
- `cargo clippy -p gauntlet-arch-tests --all-targets -- -D warnings` —
  exit 0. `rustfmt --check` on both new Rust files — clean (the
  workspace-wide `cargo fmt --check` diffs that exist are in B1's
  untracked files, not mine).
- `just test-package gauntlet-arch-tests` — 245 tests: **230 passed, 4
  FAILED**. All four failures (`denylist::repo::adapter_selection_is_config_driven`,
  `adapter_registry selection_is_config_driven`,
  `denylist::repo::core_crates_contain_no_denied_identifiers`,
  `literals::repo::core_crates_contain_no_hardcoded_literals`) report
  violations located exclusively in `crates/gauntlet-app/src/conformance/commands.rs`
  and `…/manifest.rs` — **B1's untracked in-progress files** (git status
  confirms `?? crates/gauntlet-app/src/conformance/`). Wave-1 parallel
  interference, the recorded task-23 lesson class; zero violations in B2
  scope; not mine to fix (file-disjoint discipline). The orchestrator's
  phase-gate full battery re-runs this after B1 lands its arch-gate
  compliance.
- `just ruler` — PASS (answer key + manifest digests match the frozen
  lock).
- Compile-lock discipline held: every `flock /tmp/gauntlet-build.lock` its
  own top-level command, `sccache --stop-server` inside the same flock'd
  command, no compile run concurrently with B1, package-scoped before
  workspace-scoped, no parallelism raised.

## 6. Deviations

1. **`evidence_locators` expected-set edit** (lib.rs): required
   integration for the new `[[test]]` entry — within the permitted
   "minimal surface to src/lib.rs".
2. **Raw target-scoped nextest invocations** for teeth and scoped runs:
   no justfile recipe exists at test-target granularity
   (`test-package` is package-scoped); the charter's command-vocabulary
   precedent (task-24 `scripts/run-mutation.sh` class).
3. **All 33 claims are `implemented_but_provisional`** even where the
   record shows real-binary/composed-loop execution (AK-060/AK-061/AK-062/
   AK-066/AK-069 class): the working ledger's own convention says
   "provisional through Task 26", and the weaker side is the honest side
   until the sweep adjudicates. B3's cross-check may upgrade lines
   mechanically where conformance.json says `implemented_and_proven`.
4. **Full-package battery red on B1's WIP** (disclosed above) — my slice's
   targets and the ruler are green.
5. Proactive `clippy`/`rustfmt` on my scope (not in the listed slice
   battery; cheap insurance so `just lint` does not break on my files).

## 7. Handoff notes for B3 / the orchestrator

- The wave-2 cross-check can parse claim lines with the shape
  `^- <status_word> — AK-NNN:` (underscore spellings; `proposed`/
  `blocked`/`unknown` bare). The docs_present vocabulary test keeps every
  AK-adjacent status-like token canonical, so the parse is stable.
- HANDOFF.md's category table intentionally has placeholder counts; B3
  owns `.agents/reports/handoff.md` and the counts-match-consistency leg.
- CONFORMANCE.md's UNK-001 assertion anchors on the exact phrase
  `cross_cutting containment — a declared substitution` (also exported as
  `docs::UNK001_STABLE_PHRASE`).
