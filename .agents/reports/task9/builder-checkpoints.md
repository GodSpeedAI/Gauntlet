# Task 9 — builder checkpoints (round 1)

Appended after every slice. The checkpoint file and the tree are the
contract if this session dies (AGENTS.md failure log).

## 2026-08-19 — D1: ports `facet_evidence` codec

- **Landed:** `crates/gauntlet-ports/src/observer_evidence.rs` (new module,
  declared + re-exported at the ports root in `src/lib.rs`):
  `FacetEvidenceDoc` (10-field wire document, schema token
  `facet_evidence@v1`, canonical `encode`, STRICT `decode`), typed
  `FacetEvidenceError` (Json/SchemaUnsupported/UnknownField/MissingField/
  TypeInvalid/ValueInvalid, field locus everywhere), and the shared
  `command_line(program, args)` argv renderer (POSIX single-quote rule,
  pinned). Module doc records the §12.2-vs-B.3 reconciliation the brief
  specified (per-facet env fingerprint lives in the evidence document; the
  bundle-level fingerprint is the assembler's) and the lossy-UTF-8 rule.
- **Files touched:** the new module + `crates/gauntlet-ports/src/lib.rs`
  (module declaration, re-exports). Nothing else.
- **Tests:** 7/7 `cargo nextest run -p gauntlet-ports observer_evidence`
  (round-trip + canonical-bytes identity, unknown-key tooth, missing-key
  tooth, empty-identity tooth on BOTH decode and construction, wrong schema
  token, wrong shapes (fractional exit, negative timestamp, non-string
  stdout), 6 command_line quoting pins).
- **Fail-first evidence:** new module — the discriminating tests failed as
  unresolved symbols before the module existed (compile-level fail-first,
  the honest form for a greenfield module); one intermediate red observed
  during development: the unknown-field test initially asserted a moved
  binding (compile error), fixed with `ref field`. No behavior was weakened
  to reach green.
- **Deviations:** none. Port surface NOT modified (additive module only),
  frozen contract suites untouched.
- **Open questions:** none blocking. `leaked_field` bounds a `&str`→`&'static`
  leak to the four declared identity-field constants (documented in source);
  flagged for the critic as a stylistic point, not a defect.

## 2026-08-19 — D2: the five observe adapters

- **Landed** (per crate under `crates/adapters/gauntlet-adapter-observe-*/`,
  each with `sha2.workspace = true` added — already adapter-allowlisted):
  - **filesystem** (4/4): bounded deterministic BFS walk (named caps
    `MAX_WALK_DEPTH=64`, `MAX_MANIFEST_ENTRIES=10_000`,
    `MAX_HASHED_BYTES=8 MiB`, all stated in the recorded command string);
    JSONL manifest in the doc's stdout (relative paths, forward slashes,
    file/dir/symlink kinds, size, `digest` sha256-or-null, `reason`,
    `error` fields); symlinks listed NEVER followed (pinned: nested file
    appears exactly once, `link-to-nested/` absent); depth-capped dirs and
    the entry cap produce explicit reason/truncation lines; unreadable
    entries recorded as `unreadable` lines (reality), unreadable ROOT →
    `InstrumentFailed`; manifest determinism pinned (same bytes → same
    manifest).
  - **git / versioning** (5/5): `FacetKind::Versioning`; wraps
    `git status --porcelain=v1 -b` with cwd=workspace; git nonzero →
    `InstrumentFailed` carrying exit code + stderr (documented Task-9
    exception, pinned by `a_non_repository_maps_to_...`); spawn error →
    `Unavailable`; signal-death → `InstrumentFailed`; factory materializes
    live workspaces via real `git init` (missing binary = honest factory
    error the suite reports).
  - **build** (3/3), **test** (3/3), **command** (5/5): declared
    `BuildCommand`/`TestCommand`/`CommandSpec { program, args }`
    constructor params; ANY exit status is a facet (pinned with `false`:
    facet exit Some(1) + matching evidence); spawn failure →
    `InstrumentFailed` (pinned in command's suite); signal-death →
    `InstrumentFailed`; command adapter's kind is a constructor parameter
    (pinned `FacetKind::Scanner` binding + suite factory binds `Test`,
    the same binding p3 gives the re-run self-check).
  - All five: `new` (process env) + `from_parts` (explicit pairs);
    `pub fn env_fingerprint(pairs)` per crate (sorted KEY=VALUE lines →
    sha256 → `sha256:<hex>`, unit-pinned in each crate); suite factories
    (ObserverFactory) with per-build Scratch roots cleaned on Drop.
  - Each `tests/shared_contract.rs`: UNMODIFIED `observer_suite(&factory)`
    asserted empty + the AK-028 provenance test (instrument name+version
    non-empty, exact command, exit recorded, evidence ref bytes
    strict-decode as `facet_evidence@v1`, exit status matching the facet).
- **Test counts:** filesystem 4/4, git 5/5, build 3/3, test 3/3, command
  5/5 — 20/20 across the five crates. Clippy clean (all-targets) after
  fixing `self: &Self` receivers and converting the test ScratchStore to
  Mutex+AtomicU64 (clippy `arc_with_non_send_sync` — the honest fix, not
  an allow).
- **Fail-first evidence:** new crates — compile-level fail-first (symbols
  unresolved before the adapters existed). Live red observed during the
  slice: three compile breaks fixed (missing `EvidenceKind` import,
  i32→i64 exit coercion, `//!` inside a doc block); the command adapter's
  missing-program test failed against the pre-exception-mapping draft by
  construction (spawn error path).
- **Deviations:** (1) `pub fn env_fingerprint` exposed at each adapter's
  crate root (brief specified `from_parts` computing it; the standalone fn
  is the same code, public so the composition-root proofs and the app
  assembler's caller can compute the SAME fingerprint without duplicating
  the rule a sixth time). (2) FacetEvidenceDoc constructor takes an
  `ObservedRun` group (clippy too_many_arguments at 9 args — grouped
  rather than allowed). (3) The suite factory for `command` binds
  `FacetKind::Test` (documented; the brief left the binding open).
- **Open questions:** none blocking. Known limitation recorded for the
  critic: git's own version is never probed (wrapper uniformity);
  `workspace.exists` truth is `symlink_metadata().is_ok()` (a broken
  symlink resolves as Unavailable).

## 2026-08-19 — D3: `gauntlet-app/src/observation/`

- **Landed:** module `observation` declared in `gauntlet-app/src/lib.rs`:
  - **bundle.rs** — typed Annex B.3 mirror: `ObservationBundleDoc`
    (canonical `to_json` in the annex's key order, STRICT `decode` with
    typed `ObservationDocError` at every level: doc, facet, nested
    instrument object, claims facet, gap entries), `FacetRecord`
    (`from_facet` named ctor from ports facets; absent optionals are JSON
    nulls), `ClaimsFacet` (const `source="builder"`, const
    `excluded_from_verification=true`; a wire document spelling the flag
    `false` FAILS TO DECODE — pinned), gap record types per B.3 shapes,
    `BundleContent` bag (construction + rebuild), `as_domain_view()`
    (locator `facet:<kind>`, instrument = kind token — documented
    mapping). Two documented narrowings: facet `summary` carries at most
    `{text}`; timestamps are epoch-second integers.
  - **digest.rs** — `normalized_bytes` (removes `observed_at` +
    per-facet `started_at`/`ended_at`, pins the self-referential
    `observation_digest` field to `""` — documented) and
    `observation_digest` (SHA-256 over the normalized bytes; infallible —
    SHA-256 hex satisfies the Digest grammar by construction).
  - **input.rs** — `InstrumentFacets` (PRIVATE constructor, built only by
    `VerificationInputs::of_bundle`) and `VerificationInputs` (facets +
    digest + bundle_ref + BOTH gap arrays; canonical `to_json` for the
    canary sweep). No `From`/accessor path from `ClaimsFacet`.
  - **assembly.rs** — `ObservationAssembler::assemble(plan, deadline)`:
    observers in plan order; **the single population site**
    (`gap_entries_for`) turns each `ObserverError` into BOTH §12.3
    entries (§21.3); per-facet provenance cross-check (store.get →
    strict-decode → exit-status agreement; disagreement recorded as
    `provenance_mismatch` data, facet stays); placeholder digest replaced
    by the computed one (normalized form pins the field, so the
    placeholder cannot leak — pinned); bundle bytes stored as
    `EvidenceKind::Log`; only a bundle-store failure aborts (typed
    `AssemblyError::BundleStoreWrite`).
- **Test counts:** 23/23 under `cargo nextest run -p gauntlet-app
  observation::` (NON-EMPTY — Task-8 filter lesson checked): bundle 8,
  digest 4, input 4, assembly 7. Clippy clean after one collapsible-if
  fix.
- **Fail-first evidence (LIVE, both reverted byte-identical):**
  - T4 probe: replaced the `"observed_at" => None` arm in
    `normalized_bytes` with a pass-through →
    `shifted_timestamps_do_not_change_the_digest` FAILED; reverted → 4/4.
  - T1 probe: deleted the `unavailable.push(missing)` at the single
    population site → `observer_failures_populate_both_gap_arrays_exactly_once`
    FAILED; reverted → 23/23.
- **Deviations / friction (for the critic):** the brief's input-test spec
  "inputs equal inputs built from the same bundle with claims content
  swapped" is UNSATISFIABLE as written — `VerificationInputs` carries the
  bundle digest per the brief's own field list, and §12.4 covers claims
  in the digest (forensic tamper-evidence). Implemented the satisfiable
  intended property: facets/gaps/bundle_ref equal under a claims swap,
  canonical bytes differ ONLY in the digest identifier, claims change →
  digest changes but facet inputs unchanged (comment recorded in the
  test). Everything else per brief.
- **Open questions:** none blocking. The DIGEST_PLACEHOLDER_HEX
  round-trips through a `map_err` into `AssemblyError::BundleStoreWrite`
  (a 64-zero hex is always a valid Digest — the map is unreachable but
  typed, not unwrapped).

## 2026-08-19 — D4: composition-root proofs (FINISHER-AFTER-INTERRUPTION)

- **Context:** the round-1 builder died from a usage limit mid-D4: the two
  proof files + the gauntlet-cli dev-dependency edit LANDED but were never
  executed. This slice is the finisher's: prove what landed, fix what is
  broken. D1–D3 re-verified green first (no drift): ports
  `observer_evidence` 7/7, app `observation::` 23/23 (filter non-empty),
  adapters filesystem 4/4, git 5/5, build 3/3, test 3/3, command 5/5.
- **First-ever executions:** `cargo nextest run --test p3_real_observation`
  → 2/2; `cargo nextest run --test g6_hide_failed_observations` → 3/3.
  Forensic note: the p3 binary was already warm in target/ (the dying
  builder got a compile in, never a run); g6 had NEVER compiled — its first
  build surfaced a live unused-import warning.
- **Fixes (fail-first, both observed red personally):**
  1. TEST-SIDE — g6 line 46 unused `Observer` import (a plain compile
     warning, a clippy `-D warnings` failure): removed; g6 3/3 clean.
  2. PRODUCTION (the one production-code fix of the finisher round) —
     clippy `arc_with_non_send_sync` (×2) in p3's
     `Arc::new(DeterministicClock::start_at(..))`: the mock adapter's
     `DeterministicClock` had a `Cell` interior (documented
     single-threaded), which cannot participate in the port's
     `Arc<dyn Clock>` wiring every composition demands. Fix: interior
     `Cell<EpochSeconds>` → `AtomicU64` (monotonic-max `sleep_until` via
     compare-exchange loop; snapshot `Clone` kept manual; observable
     semantics byte-identical — all 4 existing clock tests unchanged and
     green). Discriminating test added:
     `deterministic_clock::tests::concurrent_sleeps_settle_on_the_maximum_target_under_sharing`
     (threads sleeping to 10/90/30 settle on 90 — a `Cell` interior cannot
     even compile it). `gauntlet-adapter-agent-mock` 69/69 (68 + the new
     tooth). This is a change inside closed Task 8's adapter crate —
     flagged for the critic; rationale: the p3 composition root is exactly
     the "intended production/composition path" a closed-task green failed
     to prove reachable.
- **Brief-spec audit (finisher's, against the brief's D4 text):** both
  proofs assert everything the brief requires — p3: lying-builder driven
  to completion via the mock runner (invoke→poll→collect→
  `read_result_contract` cross-check), FilesystemObserver +
  CommandObserver(kind=Test) observing the SAME workspace, command facet
  `exit_status()==Some(1)` with strict-decoded failing evidence, manifest
  lists the artifact, claims facet carries the document's claims verbatim,
  `VerificationInputs` canonical bytes contain the FAIL-facing facet and
  NONE of the claims text (canary loop over every claim), digest stable
  across re-assembly (byte-identical docs) with the same
  DeterministicClock+env, claims swap moves the digest but not the facet
  inputs. g6: spawn-failure observer → BOTH gap arrays exactly-once with
  the failed kind + `has_gaps()` true + digest covers gap entries (rebuilt
  doc with an extra gap entry digests differently); store-put failure →
  gap entries (failing probe store + working assembler store); all-healthy
  → explicitly EMPTY typed arrays. NOTHING weaker than the brief found —
  no strengthening needed. One noted strength: p3 DERIVES the grep
  command from the result document's own declared `self_checks` entry
  rather than spelling it (the fixture's declared check, re-run as an
  instrument — exactly P3's story).
- **Open questions:** none blocking. Known limitation inherited from D2
  (recorded there): the gap records name the facet-kind token, not the
  instrument name.

## 2026-08-19 — D5: recorded acts (FINISHER-AFTER-INTERRUPTION)

- **deps.rs:** extended ONLY the existing adapter-layer `sha2` entry's
  REASON to name the observe adapters' use ("the five observe adapters'
  sha256:<hex> environment fingerprints plus the filesystem manifest's
  per-entry content digests (plan Task 9, spec 12.2)"). No new entries, no
  pin changes: verified first that `allowlist_entries_state_a_reason`
  pins word-count (≥6), not text, and `the_external_allowlists_are_pinned`
  pins NAMES (`rusqlite`, `sha2`) — both green after the edit
  (`cargo nextest run -p gauntlet-arch-tests allowlist` → 6/6).
- **reference-candidates.md:** three entries appended (AK-028, AK-029,
  AK-030), each declared as an ADDITIONAL INSTANCE of the existing
  locator-materialization class first recorded 2026-08-17 (AK-049), each
  citing REAL test names that exist and are green after this round's runs
  (per-adapter provenance tests, ports observer_evidence codec, app
  observation assembly/input tests, and the two composition-root proofs).
- **CURRENT_STATUS.md:** left alone in this slice (finisher updates the
  IN-FLIGHT line at D6 per the round contract).
- **Deviations:** none.
- **Open questions:** none.

## 2026-08-19 — D6: gate run (FINISHER-AFTER-INTERRUPTION) — boundaries was RED; reconciliation recorded below

- **Context:** the round-1 builder died before ANY D6 gate. The finisher ran
  them in the brief's order (lint/fmt last).
- **THE FINDING:** `just boundaries` FAILED on first run — 92 violation
  hits (46 distinct) in the D1–D3 code the dying builder never put in front
  of the boundary suite: the B.3/`facet_evidence@v1` wire key `"command"`
  (whole-literal adapter segment, AK-014 rule), facet-kind tokens
  `"test"`/`"build"` in app tests, and one adapter-name mention
  (`gauntlet-observe-filesystem`) in a ports doc comment. This is a
  DESIGN COLLISION the brief never saw: Task 9 mandates canonical
  encode/decode of B.3-family wire schemas in ports+app, while the Task-2
  denylist masked wire keys in exactly ONE module (role_result.rs, the B.2
  decoder — whose encoders legally live in adapters; Task 9's codecs are
  both encoder AND decoder in core).
- **The reconciliation (three parts, all recorded):**
  1. **Test/doc-side honest fixes (no gate edit):** facet-kind literals in
     tests spelled `FacetKind::Test.token()` / `FacetKind::Build.token()`
     (assembly.rs 1, bundle.rs 2, digest.rs 1, input.rs 1); the ports doc
     example de-named to the generic `gauntlet-observe-<x>` family (a
     PORTS doc should not name one specific adapter — substring class).
  2. **Production refactors so every surviving key literal sits in a
     masked position:** schema-driven (zip) serializers —
     `FacetEvidenceDoc::encode` zips `FIELDS`, `FacetRecord::to_json`
     zips `FACET_FIELDS`, `VerificationInputs::to_json` zips a new
     declared `FACET_INPUT_FIELDS` table (key order is the schema table's
     order BY CONSTRUCTION — canonical bytes unchanged, pinned by the
     existing round-trip/canary/digest tests: ports 7/7, app 23/23 before
     and after); decode's identity keys passed from the
     `NON_EMPTY_FIELDS` table; the stringly `field_value` match deleted
     for a typed zip (construction validation unchanged).
  3. **Deliberate, recorded boundary-suite growth (the critic MUST review
     this):** `ANNEX_B_CONTRACT_PARSER` (one file) →
     `ANNEX_B_CONTRACT_PARSERS` (role_result.rs + the three Task-9 codec
     modules: ports observer_evidence.rs, app observation/bundle.rs, app
     observation/input.rs), pinned by a NEW exact-list pin
     (`annex_b_contract_parsers_are_pinned`); the `"command"` wire-key
     annotation extended to name its B.3 uses; the array-element mask
     shape extended to rustfmt's line-initial element form (previous
     physical line ends in `[`/`,` — without this, real one-element-per-
     line schema tables are invisible to the shape), residual documented
     in the predicate doc; NEW mask test
     `annex_b_wire_keys_are_masked_in_the_task9_codec_modules_too` and an
     extended corners pin (multi-line table masked; line-initial literal
     after `=` still reports). Justification: identical class to the
     existing B.2 mask — matching the wire schema is not adapter
     selection (AK-014); all three mask gates (module, declared key,
     position) preserved; selection-shaped literals (match arms, lets,
     consts, non-key args) still report in every module including the
     four masked ones. The alternative — Task-9 codecs unable to spell
     their own declared schema — was unimplementable without evading the
     scanner (concat/format! tricks), which is the exact adversary the
     suite exists for.
- **Gate results (final, post-`cargo fmt` re-confirmation):**
  `cargo nextest run --test p3_real_observation` 2/2;
  `--test g6_hide_failed_observations` 3/3;
  `-p gauntlet-app observation::` 23/23 (non-empty);
  `-p gauntlet-ports observer` 9/9;
  adapters: filesystem 4/4, git 5/5, build 3/3, test 3/3, command 5/5;
  `just check` clean; `just deny` all-ok; `just boundaries` **192/192**
  (190 + the 2 new pins); `just ruler` PASS (frozen lock untouched);
  `cargo nextest run --workspace` **605/605**; `just lint` exit 0
  (clippy `-D warnings` + `fmt --check`). Lint/fmt LAST per contract.
- **fmt note:** `cargo fmt` reformatted 12 previously-landed files — the
  dying builder never ran fmt (the exact Task-8 recovered-miss shape); all
  gates re-run green on the formatted tree.
- **Clippy fixes during the slice:** one `manual_contains` (iter().any →
  .contains) in the new predicate.
- **Teeth NOT run** (per contract — the orchestrator runs T1–T4); the
  designed sites are recorded in builder-round1.md.

## 2026-08-19 — correction round: C1–C10 (adjudication `correction-adjudication.md`)

One builder, one pass, C1→C10 in order; fail-first discipline per item
(adjudication names each test's shape). D1–D10 NOT implemented. Frozen
surfaces untouched: `.wayfinder/`, `.agents/specs/`,
`crates/gauntlet-ports/src/contract/` (the command adapter's own
`tests/shared_contract.rs` is an adapter test file, not the frozen suite).

- **C1 (containment, five adapters) — FIXED.** Fail-first: new
  `workspace_resolution_is_contained_inside_the_observer_root` in each
  adapter's unit tests; all five FAILED pre-fix with
  `ref ../sibling must be refused, got Ok("<root>/../sibling")`
  (`--no-fail-fast` 25 run / 5 failed). Fix: `resolve_workspace` refuses
  refs with `.`/`..` components outright, canonicalizes the joined path,
  and requires `starts_with(canonical root)`; failures →
  `ObserverError::Unavailable` whose detail names containment (and the
  symlink case for escape-by-symlink). The ROOT is canonicalized ONCE at
  construction (`canonical_root`, stored; `unwrap_or(root)` — a
  nonexistent root is stored unresolved and every observe then fails the
  prefix check fail-closed; a symlinked ROOT canonicalizes through,
  operator config, pinned by the same test). Files: the five
  `crates/adapters/gauntlet-adapter-observe-*/src/lib.rs`.
- **C2 (mismatch populates BOTH arrays) — FIXED.** Fail-first:
  `garbage_evidence_populates_both_gap_arrays` FAILED pre-fix
  (`unavailable_evidence` was empty — exactly BIT-2a). Fix: the
  cross-check's single `mismatch` closure now pushes BOTH an
  `ObservationFailureRecord` and an `UnavailableEvidenceRecord` naming the
  kind. Locus: `crates/gauntlet-app/src/observation/assembly.rs`.
- **C3 (agreement checks) — FIXED.** Fail-first:
  `cross_wired_evidence_with_equal_exits_is_a_provenance_mismatch`
  FAILED pre-fix ("the cross-wiring is recorded: []" — zero mismatches)
  and `an_exitless_facet_against_exited_evidence_is_a_provenance_mismatch`
  FAILED pre-fix (None-bypass open). Fix: `cross_check_facet` compares
  instrument name, instrument version, exit status, command, and the
  timing window between facet record and strict-decoded evidence; only
  `Some` fields are compared EXCEPT exit status, where facet-`None`
  against evidence-`Some` is itself a mismatch; every mismatch flows
  through the same C2 failure site; first disagreement recorded (named
  detail), early return.
- **C4 (typed plan refusals) — FIXED.** Fail-first: three tests
  (`an_empty_observer_list_is_refused`,
  `a_badly_shaped_environment_fingerprint_is_refused` over 9 malformed
  shapes, `an_empty_project_revision_is_refused`) FAILED pre-fix with
  "must be refused, got Ok(AssembledBundle …)" (enum variants added
  first so the tests compile against the typed surface — the runtime
  fail IS the recorded evidence). Fix:
  `AssemblyError::{EmptyObserverList, BadFingerprintShape{fingerprint},
  EmptyProjectRevision}` + Display + `is_sha256_fingerprint`
  (`sha256:` + exactly 64 lowercase hex) validated at the top of
  `assemble`. Deviation noted: the shared test fixture
  `plan_with`'s fingerprint changed `sha256:0123abcd` (8 hex — now
  invalid BY DESIGN at plan level) to a 64-hex constant; evidence-doc
  fingerprints are untouched (D2 owns env semantics).
- **C5 (error lines respect the cap) — FIXED.** Fail-first:
  `manifest_error_lines_respect_the_entry_cap` FAILED pre-fix with
  "error lines must count against the cap: 10002 lines for a cap of
  10000" (exactly the critic's arithmetic). Fix: both error-line sites
  (read_dir failure, iteration error) check
  `lines.len() >= MAX_MANIFEST_ENTRIES` → mark truncated and stop
  pushing; invariant total ≤ cap+1 including the truncation line; the
  test restores permissions and asserts an explicit host precondition
  (root-running hosts fail loudly rather than pass vacuously).
- **C6 (non-UTF-8 determinism) — FIXED.** Fail-first:
  `lossy_colliding_names_get_distinct_deterministic_manifest_lines`
  FAILED pre-fix — at the pre-existing `debug_assert` ("the entry path
  names its own file name": the lossy spelling diverges from the real
  component — the defect itself), before the distinctness assertion
  could run; both are the same lossy-spelling locus. Fix: documented
  escape rule in `component_spelling`: pure-ASCII names render
  byte-identically to the historical lossy spelling (backslash→slash
  per component, as before); names containing any non-ASCII byte render
  `\xNN` for such bytes and double their own backslashes — injective on
  that class, and the per-directory sort + `relative` use the escaped
  spelling (total order; two lossy-colliding names → two distinct lines,
  stable across creation orders — asserted). The `debug_assert` now
  re-derives the spelling. Recorded residual (in the doc): an escaped
  spelling can still collide with a pure-ASCII name literally
  containing the escape text — the ASCII class must render verbatim.
  Existing manifest pins unchanged (shared-contract determinism test
  stayed green throughout).
- **C7 (maskable-intersection pin) — ADDED (pin, passes by design).
  `the_maskable_wire_key_intersection_is_pinned` in
  `gauntlet-arch-tests` `scanner_tests` derives the Denylist from the
  REAL workspace member list (`crate::deps::read_members` over
  `crate::walk::repo_root()`, the same source the repo scan uses) and
  pins `adapter_segments ∩ ANNEX_B_WIRE_KEYS == ["command"]`. Fail-first
  caveat, per contract instruction to say so: a PIN of current behavior
  cannot fail against the current tree; its discriminating direction is
  demonstrated in-test by the control member
  `gauntlet-adapter-observe-path` (widens the computed intersection to
  include "path"), proving the pin computes a real intersection rather
  than a constant. Helper `maskable_wire_keys` lives inside the test
  module (no dead production code) and reuses the mask's own inputs and
  predicate.
- **C8 (spawn-failure taxonomy) — FIXED.** Fail-first: updated g6 pin
  (adjudication cited in the comment) FAILED pre-fix ("a spawn failure
  classifies as unavailable"), and new unit tests
  `a_missing_program_classifies_as_unavailable` in build and test
  adapters FAILED pre-fix ("got Err(InstrumentFailed …)"). Fix: build/
  test/command spawn errors → `ObserverError::Unavailable` (matching
  git). Also updated: the command adapter's own editable contract-runner
  case renamed to `a_missing_program_is_unavailable_never_a_facet`
  (adapter test file, NOT the frozen ports suite — frozen suite accepts
  either variant, unedited). g6 still asserts BOTH gap arrays populate.
- **C9 (split-argument residual visibility) — ADDED (pin, passes by
  design). The critic-B split-argument fixture
  (`registry.select("builder",\n    "command",\n)`) added to
  `annex_b_mask_corners_are_pinned_to_the_documented_residual`,
  ASSERTING it masks today in all four codec modules, commented as the
  recorded residual with the D4 ledger trigger; anti-vacuity control:
  the same fixture outside the codec modules reports. Fail-first caveat
  (same class as C7): a pinned residual cannot fail against the tree
  that exhibits it; the outside-module control is what keeps the pin
  non-vacuous.
- **C10 (doc note) — DONE.** Module doc in
  `crates/gauntlet-app/src/observation/input.rs`: command/detail fields
  are provenance and diagnostic records — the instrument-produced exit
  status is the verdict carrier; builder-penned text in them cannot
  flip an outcome. Doc only, no behavior change, per adjudication.
- **AK-014 self-inflicted hits during the round (found by the boundary
  suite, fixed before any gate):** my first C2/C3 test assertions used
  whole literals `"test"`/`"command"` in `assembly.rs` (a core file) —
  replaced with `FacetKind::Test.token()` and an `"instrument"`-wording
  probe. The suite catching its own builder is the system working.
- **Files touched:** five
  `crates/adapters/gauntlet-adapter-observe-*/src/lib.rs`;
  `crates/adapters/gauntlet-adapter-observe-command/tests/shared_contract.rs`;
  `crates/gauntlet-app/src/observation/{assembly.rs, input.rs}`;
  `crates/gauntlet-arch-tests/src/denylist.rs`;
  `crates/gauntlet-cli/tests/g6_hide_failed_observations.rs`.
- **Gate results (final, post-fmt):** `--test p3_real_observation` 2/2;
  `--test g6_hide_failed_observations` 3/3; `-p gauntlet-app
  observation::` 29/29; `-p gauntlet-ports observer` 9/9; five adapter
  crates 29/29; `-p gauntlet-arch-tests` 193/193 (192 → +1 C7 pin);
  `cargo nextest run --workspace` **621/621** (605 → +16 new tests:
  5 containment + 3 C2/C3 + 3 C4 + 2 C5/C6 + 1 C7 + 2 C8; C9 extended an
  existing test, C10 doc-only); `just check` clean; `just deny` all-ok;
  `just boundaries` 193/193; `just ruler` PASS (frozen lock untouched);
  `just lint` exit 0 (clippy `-D warnings` + `fmt --check`). No
  `unwrap`/`expect` added to production paths (one `unwrap_or` in
  `canonical_root`, documented fail-closed). NOT committed; teeth NOT
  run — both per contract.
