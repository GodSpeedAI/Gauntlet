# B3 report — handoff + consistency + bundle (task27)

Completion contract for charter slice B3 (wave 2). A claim with NO
settlement authority. Phase 1 landed 2026-09-07 and was accepted and
committed by the orchestrator; phase 2 ran under the orchestrator's
sequencing adjustment (intermediate re-sweep SKIPPED — the settlement
re-sweep regenerates conformance.json; the committed json's embedded
manifest sha is honestly stale in that window).

## Phase 1 — recap (committed by the orchestrator)

- reference-candidates.md: one dated 2026-09-07 batch entry, 8
  sub-entries covering the 17 `unrecorded` divergences.
- conformance-manifest.yaml: exactly 17 `divergence` flags flipped to
  `reference-candidates 2026-09-07`.
- scripts/pin-evidence-bundle.sh: new POSIX pin script (+`--check`).
- Battery then: CLI manifest tooth 1/1, arch-tests 245/245, ruler PASS.

## Phase 2 — what was built (files)

- **`.agents/reports/handoff.md`** (NEW; AK-067's frozen locator;
  sha256 `b2104e1a6323098abbf3726a50193e7a196aa400cf2f8aba0ec4fd4be15f533d`)
  — the canonical handoff, derived mechanically from the COMMITTED
  conformance.json (`gauntlet.conformance@v1`, 2026-09-07T21:19:36Z,
  tree a98b624, 69 PASS / 1 INCONCLUSIVE / 0 FAIL):
  - all six §26 categories as sections with per-category count headers:
    implemented_and_proven — 65 · implemented_but_provisional — 4 ·
    configured_but_untested — 0 · proposed — 1 · blocked — 0 ·
    unknown — 0; every unit exactly once in its category (a bullet per
    unit carrying its sweep basis);
  - every INCONCLUSIVE unit named with its verbatim reason: AK-067,
    `no_artifact: .agents/reports/handoff.md is absent` (in the
    proposed section, where the json's `status_category` puts it, +
    a summary section);
  - UNK-002 terminal disposition (LICENSE: the operator's call, STILL
    UNKNOWN, status honestly `unknown`) and UNK-003 terminal
    disposition (`runners.default` remains the operator's standing
    decision; repo-root GAUNTLET.md deliberately names none;
    fixture-level delegation the recorded pattern — task20/25/26
    cited) in the unknown section, marked fog-not-units;
  - Disclosures: (1) no repo-level durable run exists to export —
    VERIFIED BY EXECUTION (`gauntlet export` is run-bound; the default
    state store resolves under host scratch OUTSIDE the repo and is
    empty; the repo root carries no store — `.gauntlet/` holds
    policies/comparison-contract/reference.lock only; build's runs
    lived in scratch worlds; the pinned bundle IS the canonical
    export); (2) the four bounded units' bases quoted verbatim from
    conformance.json (AK-053/066/069/070); (3) the mutation-record
    basis = the dated task-24 settlement artifact, NOT re-executed on
    the final tree (charter decision 11); (4) 61 dated
    reference-candidates citations / 9 verbatim locators, ruler never
    moved; (5) single-sourcing statement.
  - The six-category predicate (`predicates.rs
    handoff_six_categories`) holds: non-empty UTF-8 naming all six
    tokens in the section headers — PROVEN by execution (the CLI
    committed-manifest tooth now adjudicates AK-067 PASS, see below).
  - An explicit evidence-of-the-moment paragraph states the window:
    this file post-dates the sweep it derives from; the settlement
    re-sweep re-adjudicates AK-067 against it.
- **`crates/gauntlet-arch-tests/src/docs_consistency.rs`** (NEW) — the
  wave-2 consistency logic, driven from the AK-068 target:
  - a fail-closed std-only mini JSON reader (the crate's declared
    posture: no internal deps, only `toml`) parsing the frozen
    `gauntlet.conformance@v1` shape — trailing input, truncated
    documents, trailing commas, bad escapes, foreign schemas, non-§26
    categories, duplicate ids all refuse;
  - `check_handoff_matches_conformance[_at]`: the handoff's six
    category sections vs the json's `status_category` grouping — set
    equality per category, declared-count == json-count ==
    bullet-count, one section per category, no unit in two sections;
  - `check_docs_claims_not_ahead_of_conformance[_at]`: every
    `^- <status_word> — AK-NNN:` line in the eight docs vs the json —
    proven claimable only where json says proven; provisional where
    json says proven OR provisional; the other four only on exact
    agreement; claims naming unknown ids fail closed;
  - env overrides `GAUNTLET_CONFORMANCE_JSON_FIXTURE` /
    `GAUNTLET_HANDOFF_FIXTURE` repoint the integration legs at
    fixture copies (the RED-tooth channel);
  - 8 in-crate unit tests (fixtures under temp dirs; real artifacts
    never mutated in tests).
- **`tests/contract/docs_present.rs`** (B2's address file, mine now) —
  two new legs: (f) `handoff_matches_conformance_grouping`, (g)
  `docs_status_claims_never_ahead_of_conformance`; scope header
  updated. NO Cargo.toml change: the legs live INSIDE the docs_present
  target, so AK-068's swept evidence automatically covers them (the
  settlement re-sweep exercises the consistency checks).
- **`crates/gauntlet-arch-tests/src/lib.rs`** — one `pub mod
  docs_consistency;` line (alphabetical; the architecture_boundaries
  address-vs-logic precedent).
- **The eight docs** — 27 claim lines mechanically upgraded
  `implemented_but_provisional` → `implemented_and_proven` where
  conformance.json adjudicates the id proven (the full 27: AK-001,
  AK-002, AK-013, AK-014; AK-005, AK-058; AK-015, AK-016, AK-017,
  AK-019; AK-022, AK-023, AK-043, AK-044, AK-047, AK-048; AK-041,
  AK-042, AK-051, AK-054; AK-060, AK-061, AK-062; AK-055, AK-056,
  AK-057; AK-068). 6 lines stay provisional — exactly the bounded four
  (AK-053, AK-066, AK-069, AK-070 — the latter two each claimed in
  two docs). git diff: 27 insertions / 27 deletions, claim bullets
  only, no other prose touched. AK-067 has NO claim line in any doc
  (B2's deliberate design), so "its line stays whatever the json
  says" resolved to: nothing to change.
- **`docs/HANDOFF.md`** — pointer rows kept count-free BY DESIGN (the
  dispatch's "counts may simply reference the canonical file" option;
  the doc's own spec-§25 argument that a count here would be a
  competing source of truth); the stale "until it exists" prose
  replaced (the canonical report now exists and is kept equal by the
  consistency leg); the AK-068 claim line upgraded mechanically and
  its evidence description extended to the new legs.
- **`.agents/reports/evidence-bundle.sha256`** (NEW) — the executed
  pin: 50 files (conformance.json, handoff.md, baselines.json,
  mutation.json + the 45 sweep logs + MANIFEST of
  `conformance-evidence/20260907T211936Z`), repo-relative,
  `sha256sum -c`-verified from the repo root and from another cwd.
- **`crates/gauntlet-cli/tests/conformance_cli.rs`** (B1's file — see
  deviations) — two forced updates, both consequences of the handoff
  artifact landing, neither a weakening:
  1. `the_committed_manifest_agrees_with_the_frozen_ruler`: the
     pinned interim counts (pass==2 / inconclusive==68) encoded the
     PRE-handoff tree; with handoff.md present and satisfying the
     predicate, the validation-only sweep adjudicates AK-067 PASS →
     updated to pass==3 / inconclusive==67 with a wave-2 comment. The
     assertion got STRONGER (the artifact now proves itself through
     the real predicate).
  2. `an_absent_artifact_is_inconclusive_and_the_sweep_exits_zero`:
     the fixture resolved its artifact arm against the REAL repo root
     and leaned on handoff.md being absent ("the interim state the
     real AK-067 carries until wave 2 lands its artifact"); repointed
     to a guaranteed-absent path
     (`.agents/reports/fixture-artifact-that-never-exists.md`) so the
     no_artifact semantics stay pinned hermetically, independent of
     the real tree.

## Phase 2 — RED-first teeth record (sha256s)

- **Integration RED before the artifact existed:** with the two new
  legs compiled and `.agents/reports/handoff.md` absent, the run was
  RED for the expected reason — `handoff-consistency: handoff
  unreadable at .../handoff.md: No such file or directory` (nextest
  FAIL, 6/7 with leg (g) already green: all 33 claims were then at or
  behind evidence). After handoff.md landed: **7/7 PASS**.
- **Tooth A (charter T4 — move a unit between sections):** real
  handoff copied to a temp fixture, AK-067's bullet moved
  proposed → implemented_but_provisional with both headers' counts
  adjusted (the pure row-move), run under
  `GAUNTLET_HANDOFF_FIXTURE` → RED with 4 violations: "`proposed`
  section is missing 1 unit(s) ... AK-067", "`implemented_but_
  provisional` section lists unit(s) the json does not group there:
  AK-067", + both count-header mismatches. Digests: real handoff
  before == after == `b2104e1a6323098abbf3726a50193e7a196aa400cf2f8
  aba0ec4fd4be15f533d`; tampered copy `52769e4dae957a32dd89fd50aaa1b
  43d8dbaf5c2f0482c6d97eaab304ae82077`. No restore needed (the real
  file was never touched); plain re-run GREEN.
- **Tooth B (claim ahead of evidence):** real conformance.json copied
  to a temp fixture with AK-062 downgraded to
  `implemented_but_provisional` (docs now claim it proven after the
  mechanical upgrade), run under `GAUNTLET_CONFORMANCE_JSON_FIXTURE`
  → RED: "docs/OPERATOR-GUIDE.md:93 claims implemented_and_proven for
  AK-062 but conformance.json adjudicates it implemented_but_
  provisional (ahead of evidence)". Digests: real json before ==
  after == `aa4399016639ef7644b0f3cad505d14907a669d9b7348440ae1c89ff
  36c4f308`; tampered copy `ec186ebbda4a0a3ed2d0c52aa2be1a0012ff2eec
  0b8ebd22702ece955e13d62c`. This tooth is exactly the guard for the
  anticipated settlement correction: if the final json moves and the
  docs do not follow, the suite goes RED.
- **In-crate unit teeth (8/8 green):** moved-unit tooth (asserts BOTH
  the missing and the extra side), wrong-declared-count tooth,
  ahead/behind/exact/unknown-id claim matrix, claim-line shape
  parser, word-delimited id scanning, mini-json parse/refuse set,
  missing/malformed-input fail-closed legs.

## Phase 2 — battery (every flock its own top-level command; sccache
stopped inside the same locked command; package-scoped first; no
parallelism raised)

- `cargo nextest run -p gauntlet-arch-tests --no-fail-fast` →
  **255/255 PASS** (245 prior + 8 module units + 2 new docs_present
  legs; includes single_writer over the phase-1
  reference-candidates.md edit, the literal scanner, the denylist,
  evidence_locators — Cargo.toml untouched).
- `cargo nextest run -p gauntlet-cli --test conformance_cli
  --no-fail-fast` → **9/9 PASS** (after the two disclosed updates).
- `cargo nextest run -p gauntlet-cli --test cli_surface` → **4/4
  PASS**.
- `cargo fmt --check -p gauntlet-arch-tests -p gauntlet-cli` → clean;
  `cargo clippy` over both `--all-targets -- -D warnings` → exit 0
  (three collapsible-`if` findings fixed with edition-2024 let-chains;
  rustfmt applied).
- `./scripts/pin-evidence-bundle.sh --check` → **bundle verified**
  (50 files; handoff.md digest matches the pinned `b2104e1a…`).
- `just ruler` → **PASS** (run after every file wave; the ruler never
  moved).

## Anticipated settlement correction (kept cheap, per the
orchestrator's sequencing note)

After the settlement re-sweep flips AK-067 to PASS /
implemented_and_proven:

1. `.agents/reports/handoff.md`: move the AK-067 bullet from the
   proposed section to implemented_and_proven; update the two headers
   (65→66, 1→0); delete/replace the evidence-of-the-moment paragraph
   and the INCONCLUSIVE summary (zero left); refresh the provenance
   line to the new json stamp. ~5 lines.
2. Re-run `scripts/pin-evidence-bundle.sh` (the bundle pins
   conformance.json and handoff.md — both change at settlement).
3. `docs/HANDOFF.md` needs nothing (it carries no counts).
4. The consistency test verifies all of it mechanically; nothing else
   reads the counts.

## Deviations (phase 2)

1. **B1's `conformance_cli.rs` edited** (file-disjoint discipline):
   two tests updated, both forced by MY artifact landing (pinned
   interim counts; fixture resolving the real handoff path) — detailed
   above. No assertion was weakened; both still bite (the counts now
   pin the STRONGER post-handoff fact; the absent-artifact tooth is
   hermetic where it was tree-coupled).
2. **docs/HANDOFF.md kept count-free** rather than filling counts —
   the dispatch's explicitly allowed pointer form; spec-§25
   anti-competing-truth; keeps the settlement correction minimal.
3. **New crate module + one lib.rs mod line in gauntlet-arch-tests**
   (charter slice B3: "a sibling target in gauntlet-arch-tests — B3
   coordinates the file"); logic-in-crate/address-file is the suite's
   own precedent; legs added to the existing docs_present target
   rather than a new [[test]] so AK-068's swept evidence covers them
   with no manifest or Cargo.toml change.
4. **The consistency test checks grouping + counts + claim-admissibility
   only** — the handoff's prose obligations (reason texts, disclosure
   wording) are content, pinned by review and the sweep predicate,
   not by this mechanical test (the dispatch's contract was the
   grouping comparison).
5. **No mutation/coverage run** (not commissioned for this slice;
   charter decision 11 is the recorded-artifact posture; AGENTS.md
   expensive-evidence policy).

## Known limits / honest residuals

- The committed conformance.json's embedded manifest sha is stale
  against the flipped manifest until the settlement re-sweep
  regenerates it (the orchestrator's accepted window; disclosed in
  handoff.md's provenance note). The consistency test deliberately
  does NOT pin that sha — manifest-sha freshness is the sweep's own
  invariant.
- The pin was executed against the CURRENT json and handoff; both are
  re-pinned at settlement (playbook above).
- The mini JSON reader is deliberately small: it parses the frozen
  schema's grammar, not arbitrary JSON (no \uD800 surrogate-pair
  joining — the frozen report is ASCII-shaped; a surrogate pair
  yields the two replacement chars and, at worst, an id/category
  refusal, never a silent pass).
- This report is a claim with NO settlement authority; the
  orchestrator re-verifies each tooth personally.
