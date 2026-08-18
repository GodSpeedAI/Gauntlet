# Task 7 — Builder brief (round 1): ReferenceProvider — normalize, digest, freeze, verify

You are the builder for plan Task 7 in `/home/sprime01/projects/gauntlet`. This brief is
self-contained. You implement, test, and report. You do NOT commit (the orchestrator
commits after its own verification). You hold the build lock exclusively for the whole
slice: run `du -sh target` occasionally; never `cargo clean`.

## Read first (in this order)

1. `AGENTS.md` — rules that outrank this brief where they conflict (they won't).
2. `.agents/plans/gauntlet-impl.md` Task 7 section.
3. `scripts/reference-digest.sh` — the frozen bootstrap digest tool. Its header documents
   the normalization rules you must reproduce **exactly** in Rust.
4. `crates/gauntlet-ports/src/reference_provider.rs` — the port trait you implement.
5. `crates/gauntlet-ports/src/contract/reference_provider.rs` — the shared contract suite
   your implementation must pass.
6. `crates/gauntlet-domain/src/{reference,unit,values}.rs` — `Reference`,
   `ReferenceVersion`, `ReferenceDefinition`, `Unit::new`, `EvidenceRequirement`,
   `wire_enum!` enums.
7. `crates/gauntlet-app/src/state/{mod.rs,export.rs}` — the app service-module idiom to
   mirror (free functions / thin service types over `&dyn` ports, typed error enums,
   in-module test layout with fixed clock constants).
8. `.wayfinder/reference.yaml` (manifest wrapper; units intentionally empty),
   `.wayfinder/answer-key.md` (70 `## AK-NNN` blocks, no preamble), and
   `.gauntlet/reference.lock` (freeze record: answer-key + manifest digests).

## Hard rules

- **Never edit anything under `.wayfinder/`, never edit `.gauntlet/reference.lock`, never
  edit `scripts/reference-digest.sh`.** They are the ruler.
- No new external dependencies. `gauntlet-app` already depends on `sha2` (allowlisted
  expressly for this task). There is no serde/yaml crate anywhere and none may be added;
  all parsing is hand-rolled strict parsing, like Task 5's GAUNTLET.md front matter.
- Arch suite must stay green: no absolute paths or other hardcoded literals outside the
  declared defaults module; test temp dirs use the existing `Scratch` idiom
  (`std::env::temp_dir()` + pid + atomic counter; see
  `crates/adapters/gauntlet-adapter-evidence-fs/src/store.rs` tests or
  `crates/gauntlet-cli/tests/export_stack.rs`).
- Typed errors carrying the failure locus (block id, line number, path). No bare strings,
  no `unwrap`/`expect` in production paths, no panics on untrusted input (the answer key
  and manifest are untrusted external input: parse, validate, then admit).
- Match the repo's edition-2024 idiom. `unsafe` is forbidden workspace-wide.

## Deliverables

### 1. `crates/gauntlet-app/src/reference/normalize.rs` — pure normalization + digest

Reproduce `scripts/reference-digest.sh` byte-for-byte:

1. Input is raw bytes. Invalid UTF-8 is a typed error (the script errors; never coerce).
2. Convert CRLF and lone CR to LF.
3. Collapse any run of trailing blank lines so the normalized text ends with exactly one
   trailing LF; an empty (or all-blank-lines) input normalizes to zero bytes. NOTE the
   precise semantics from the script: split on `\n`, drop trailing `""` elements
   (the final newline plus any additional blank lines), rejoin with exactly one trailing
   newline unless nothing remains. A file whose last content line has content and a
   single `\n` is unchanged. A file ending with no newline gains one. Interior blank
   lines are preserved.
4. Nothing else: per-line leading/trailing/internal whitespace preserved exactly;
   no sorting, no trimming.
5. Digest: SHA-256 over the normalized UTF-8 bytes, exposed as the domain `Digest`
   (`Digest::new("sha256", hex)` — lowercase hex only).

Tests (in-module, under module path `reference::` — the gate filter depends on it):
- determinism: same logical content with CRLF vs LF vs lone-CR endings → identical
  digest; final-newline present vs absent vs doubled → identical digest (AK-021);
- sensitivity: appending one trailing space to any content line changes the digest;
  appending a blank interior line changes it; reordering two lines changes it;
- empty input → digest of the empty string; all-blank-lines input → same as empty;
- invalid UTF-8 → typed error;
- pin the normalization function on short synthetic inputs with expected exact output
  strings (not just digests).

### 2. `crates/gauntlet-app/src/reference/parse.rs` — answer-key parser, strict, per-block isolation

Parse `.wayfinder/answer-key.md` into `Vec<Unit>`:

- Blocks are `## AK-NNN` headings followed by field lines. Field keys that exist in the
  frozen key: `claim`, `verify_by`, `blast_radius`, `authority`, `evidence_required`
  (single-line flow mapping with `kind`, `locator`, `producer`), optional `depends_on`
  (flow sequence `[AK-008]` or `[AK-013, AK-040]`).
- Strict enum validation through the domain `wire_enum!` `parse` (e.g.
  `VerifyBy::parse("static_check")`): unknown value → that block fails with a typed
  error naming the field, the offending value, and the block id. Unknown field keys,
  missing required fields, malformed flow mapping (missing `}`/sub-key, stray text),
  a non-`AK-NNN` heading, or a duplicate block id are block-level errors.
- **Per-block error isolation is the specified behavior (plan Task 7 step 4): one
  malformed block fails that Unit, not the file.** The parse result therefore carries
  both the well-formed Units and the per-block failures with loci. A convenience
  "parse all or fail" wrapper may exist for callers that need all-or-nothing.
- The frozen key parses 70/70 clean (test asserts exactly 70 Units, ids AK-001…AK-070).
- Fixtures for malformed cases: hand-written small strings in-module (not files under
  `.wayfinder/`).

### 3. `crates/gauntlet-app/src/reference/manifest.rs` — minimal strict manifest reader

A narrow, strict parser for the frozen `.wayfinder/reference.yaml` shape (one root
`reference:` mapping; scalars, `>` folded scalars, `- ` sequences of scalars and of
mappings). It extracts what `ReferenceDefinition` needs from the manifest: `id`,
`version`, `source_refs`, `purpose`, `desired_state`, `invariants` (render each
`{id, statement}` pair as `"I1: <statement>"` — the id must survive), `constraints`,
`anti_goals`, `unknowns` (name = the `id`, statement = the `question` text),
`assumptions`, and the settlement narrative fields. Unknown keys or shapes are typed
errors naming the line — this is a frozen artifact; drift fails loudly.

**Deliberate, documented deferrals (do not invent mappings — record them):**

- `quality_dimensions` from the manifest are NOT materialized into the domain
  `Reference` at this task: their `evidence_required` entries are bare locator strings,
  and the domain `QualityDimension` requires full `EvidenceRequirement` (kind + locator
  + producer). Inventing kind/producer mappings would be inventing the ruler's
  semantics. The constructed `Reference` carries an empty `quality_dimensions` list;
  materialization is deferred until the comparison-contract task defines the mapping.
- `settlement.required_evidence` in the manifest is prose locator text, not
  `EvidenceKind` names; the constructed `SettlementCriteria` carries an empty
  `required_evidence` for the same reason.
- `minimum_reliability_passes` = 3, anchored to the manifest's own text ("spec §19.2
  default: >= 3 settlements …"); the manifest names no other number.
- `terminal_conditions` parse from the manifest list via `TerminalState::parse` (the
  manifest spellings match the wire tokens exactly; unknown → typed error).
- `success_criteria` are carried as strings verbatim; `hard_units` empty (the manifest
  names none).

Every one of these goes in your decision log with the reason. The critic will
adjudicate them blind.

### 4. `crates/gauntlet-app/src/reference/provider.rs` — the `ReferenceProvider` implementation

A `FileReferenceProvider` (name for semantic capability; vendor-free) implementing
`gauntlet_ports::ReferenceProvider`:

- Constructed from: the answer-key path, the manifest path, and an evidence handle for
  freezing (`Rc<dyn EvidenceStore>` or a small closure trait — your call, document it).
- `resolve(intent, _)`: reads both artifacts (bytes; UTF-8 errors are typed). The
  intent is the operator's verbatim intent (§5.3.3) — recorded into the draft's
  sources/definition path is NOT required; do not interpret it. Produces
  `ReferenceDraft { sources, definition }` where `sources` are the artifact paths as
  strings.
- `normalize(draft, _)`: parses manifest + answer key (per-block failures surface here
  as `NormalizeFailed` carrying the typed cause; a key with any malformed block cannot
  normalize all-or-nothing — expose the per-block failures in the error), builds
  `ReferenceDefinition` per the manifest module's rules, computes the digest over the
  **normalized answer-key bytes** and sets it as the definition digest, and constructs
  the domain `Reference` (its constructor enforces the §5.3.1 invariants).
  `Reference.digest` is the answer-key digest — the Units' ruler. (The manifest's own
  digest stays the lock's business; record this decision.)
- `digest(reference, _)`: returns the reference's digest (normalize computed it;
  identity, not re-derivation — document).
- `freeze(run_id, reference, _)`: writes the **normalized answer-key bytes** into the
  evidence store via `put(bytes, EvidenceKind::File, …)` (kind decision: `File` —
  document it; the store refuses same-bytes-different-kind, so this is permanent),
  `pin`s the resulting `EvidenceRef` to the `run_id`, and returns the
  `ReferenceVersion` (`reference.freeze()` — id, version, digest). Freezing twice for
  the same run yields the same content-addressed ref (content addressing gives you
  this; test it).
- `verify_unchanged(version, _)`: re-reads the answer key, re-normalizes, re-digests,
  and compares against `version.digest()`. Mismatch →
  `ReferenceProviderError::ReferenceMutated { expected: version.digest, actual }` —
  both digests preserved (they are the evidence). Missing/unreadable/invalid-UTF-8
  artifact → `Unavailable`/`ResolveFailed`-class typed errors, NOT `Ok` and NOT
  `ReferenceMutated` (a broken artifact is not proof of mutation; do not conflate).

**Contract suite:** run `gauntlet_ports::contract::reference_provider::reference_provider_suite`
against your provider. The suite's factory hands you the artifact as
`Rc<RefCell<String>>` — so the provider must take its answer-key content through a
small artifact-source abstraction (a path source for production, the shared string for
the suite). Keep that seam minimal and documented. The suite run is an in-crate test
under `reference::` (see `crates/gauntlet-ports/src/contract/suite_runs.rs` for the
assertion idiom).

### 5. `crates/gauntlet-cli` — `reference digest` subcommand

- `gauntlet reference digest --path <path>`: reads the file, applies the app
  normalization, prints exactly one line `sha256:<lowercase hex>` to stdout, exit 0.
  Missing/unreadable/invalid-UTF-8 → stderr message, non-zero exit. Follow the manual
  arg-loop idiom of the existing `doctor` command (main.rs). This exact output is diffed
  against `scripts/reference-digest.sh` in the task gate — byte-identical or the gate
  fails.
- Add `gauntlet-adapter-evidence-fs` to `gauntlet-cli` `[dev-dependencies]` (mirrors the
  existing `gauntlet-adapter-state-sqlite` dev-dep; the CLI is the composition root).

### 6. `crates/gauntlet-cli/tests/p1_reference_integrity.rs` — the P1 proof (composed path)

Location rationale (record it): the architecture suite's cargo-target-path rule
(exactly `crates/gauntlet-arch-tests` exempt) forbids core crates from declaring
repo-root test targets, so `tests/proofs/p1_reference_integrity` as spelled cannot be
materialized; the proof lives as an in-crate integration test at the composition root
with the target NAME the plan's gate uses. Use the real adapters (FsEvidenceStore,
EmbeddedStateStore) — this is the composed-path proof, not an isolated component test.

Copy the repo's frozen answer key into a `Scratch` dir as the test's active artifact
(NEVER point anything at `.wayfinder/` itself; the real one is the ruler). Test fns
(assertable names — locators cite them):

- `frozen_copy_outside_workspace` (AK-022): create two builder-workspace dirs under a
  scratch root; freeze through a real `FsEvidenceStore` rooted OUTSIDE those workspace
  dirs; assert the frozen object's path is outside every workspace root; assert the
  stored bytes are exactly the normalized key; assert the ref is pinned to the run.
- `builder_mutation_of_the_active_reference_aborts_with_reference_mutated` (AK-023
  mechanism): freeze; then overwrite the scratch answer key (the builder-fixture
  mutation — simulate a builder writing to the key); `verify_unchanged` returns
  `ReferenceMutated { expected, actual }` with expected == frozen digest and
  actual == re-derived digest; the frozen evidence-store copy is byte-unchanged.
- `digest_rechecked` (AK-024 mechanism/substrate): a small harness loop standing in for
  iteration boundaries — at each of ≥2 simulated boundaries and immediately before a
  simulated settlement, `verify_unchanged` is called; inject the mutation between
  boundary 1 and 2; assert the tripwire fires at the very next boundary (not at
  settlement only). The teeth for this task is: delete the boundary call site → this
  test fails. Make the call site a named helper so the tooth is surgical.
- `the_run_carries_the_frozen_version`: construct a domain `Run` (RunDefinition with
  the frozen `ReferenceVersion`), `EmbeddedStateStore::record_run` it, `load_run`
  returns the run with the identical reference version — freeze binds to run identity
  durably (uses the state-sqlite dev-dep already present).

Honest scope note for the report (do not overclaim): iteration boundaries and
settlement are wired into the real controller at Tasks 12–13; P1 here proves the
provider/evidence composition and the tripwire primitive at the composition root.
AK-023/AK-024 claims remain substrate-level until then.

### 7. Doctor: close the two honest-unavailables promised to Task 7

In `crates/gauntlet-app/src/doctor/`, the check-2 deep sub-results currently report
`UNIT_SCHEMA_LATER` / `DIGEST_LATER` as unavailable pending Task 7. Close them:

- unit-schema: parse the configured answer key with the new parser; report 70/70
  well-formed Units (or the per-block failures) as the check's deep sub-result.
- digest-vs-lock: read `.gauntlet/reference.lock` (hand-rolled parse via
  `gauntlet_ports::json` if suitable — check its API; strict, unknown fields are
  findings), recompute the normalized answer-key digest, compare to the lock's
  answer-key digest; mismatch is a check failure (reference-class, loud).

Keep every existing doctor test green; update their expectations only where the
previously-unavailable sub-results now resolve (never weaken an assertion to get
green — if a test encoded "unavailable", encode the now-available truth and note it).

### 8. Reference-candidates entries (recorded-act, not ruler edits)

Append TWO dated entries to `.agents/reports/reference-candidates.md` (follow the
existing AK-049/AK-054 entry format exactly; same defect class — locator names a
repo-root test target that the cargo-target-path rule forbids materializing):

- AK-021 locator `tests/contract/reference::normalization_is_deterministic` → cite the
  real in-crate tests (`gauntlet-app reference::…` determinism tests).
- AK-022/023/024 locators under `tests/proofs/p1_reference_integrity…` → cite
  `gauntlet-cli tests/p1_reference_integrity.rs` fns.

Explicitly note in each: new defect vs existing class — these are **additional
instances of the existing locator-materialization class** (first recorded 2026-08-17,
AK-049). The ruler is not touched; `just ruler` must stay PASS.

## Gates — all must be green before you report done

```bash
cargo nextest run -p gauntlet-app reference::
cargo nextest run --test p1_reference_integrity
diff <(scripts/reference-digest.sh .wayfinder/answer-key.md) \
     <(cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md) && echo DIGESTS-AGREE
just check && just lint && just deny && just boundaries && just ruler
cargo nextest run --workspace
```

Also actually run `cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md`
and paste its output in the report.

## Teeth (propose, don't run — orchestrator runs them)

Document in your report the exact temporary edits that should make which gate fail:
(a) removing the boundary `verify_unchanged` call helper → `digest_rechecked` fails;
(b) making `normalize` strip per-line trailing whitespace → the digest diff gate fails
(and which determinism test also fires); (c) any other tooth you see.

## Report contract (admission basis)

- **Checkpoint as you go:** after EACH numbered deliverable above, append a dated line
  to `.agents/reports/task7/builder-checkpoints.md` (create it): what landed, file
  paths, test counts. Do not batch this at the end; your final message may be lost and
  the checkpoint file is the fallback record.
- Final report: `.agents/reports/task7/builder-round1.md` with sections: Artifact
  (files + line counts), Decision log (every deliberate choice + why, esp. the
  manifest deferrals, EvidenceKind::File, digest identity semantics, artifact-source
  seam), Gate output (verbatim tails), Teeth proposals, Residuals/limits (what this
  task does NOT claim — be specific about AK-023/024 substrate status), Reference
  candidates (the two entries, cross-checked against the file you appended to).
- Completion means that file exists and is complete — not that your final message says
  so. Keep the final message to a short pointer.

## Out of scope (do not touch)

Tasks 8+ machinery (runner, observers, comparator, controller), any controller wiring
of iteration boundaries, `.agents/CURRENT_STATUS.md` (orchestrator's), the spec, the
plan, `.wayfinder/**`, `.gauntlet/reference.lock`, `scripts/*`, `deny.toml`, the arch
suite (if you believe an arch-suite change is needed, STOP and record it as a blocker
in your report instead of editing it).
