# B2a report — six-category report service (task20)

Finished by the B2a finisher after builder B2a was interrupted during test
verification. B2a's landed work (module, lib.rs line, tests) was resumed, not
redone. One in-scope test failure was diagnosed and fixed minimally; all
verification steps were then run to completion.

## What landed

- `crates/gauntlet-app/src/report/mod.rs` (646 lines) — the pure read-side
  report service. B2a wrote it; the finisher added ONE change (below).
- `crates/gauntlet-app/src/lib.rs` line 22: `pub mod report;` (B2a's only
  edit to an existing file; alphabetical slot between `reference` and
  `roles`).
- `crates/gauntlet-app/tests/report.rs` (583 lines, 9 tests) — integration
  tests per the brief, using the `tests/{formation,roles,skills}.rs`
  convention (scripted read-only `StateStore` stub, no port changes).
- No port methods were added (B2a inventory decision: composition from the
  existing `load_run` / `resume` / `settlements` / `verification_records`
  reads was possible; `HumanEscalation` has no list read, so escalation
  state is classified from checkpoint projections — typed reads only).

### Finisher change (the one in-scope fix)

Run 1 of `just test-package gauntlet-app` failed 1/458:
`capability_rows_follow_the_manifest_and_the_records` — a settlement row of
a DIFFERENT run leaked into the unit universe because the module filtered
verification records by `run_id` but not settlement rows. Fix in
`build_report` (module scope, no test edit, no assertion weakened): filter
`settlements` by `row.run_id() == run_id`, symmetric with the existing
verification filter. This matches the test's asserted contract ("another
run's settlement row is not this run's proof") and the module's own
run-scope discipline.

## Public API surface of `gauntlet_app::report` (compose against this)

All items are `pub` through `gauntlet_app::report`. Types carry
`Debug, Clone` at minimum; exact derive sets noted.

```rust
pub enum StatusCategory {                 // Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash
    ImplementedAndProven,                 // settlement row AND the durable PASS record it names
    Provisional,                          // recorded work without recorded proof
    ConfiguredButUntested,                // configured into the run/surface, never attempted
    Proposed,                             // caller manifest declares the capability not wired
    Blocked,                              // durable checkpoint projects blocked
    Unknown,                              // no durable record names the unit
}
impl StatusCategory {
    pub const ALL: &'static [Self];       // report order; renderings use this order
    pub fn token(self) -> &'static str;   // "implemented-and-proven", "provisional",
                                          // "configured-but-untested", "proposed",
                                          // "blocked", "unknown" — plan spellings, exact
}
impl core::fmt::Display for StatusCategory { /* writes token() */ }

pub enum ReportError {                    // Debug, Clone, PartialEq; Display; std::error::Error
    Store { detail: String },             // store read failed; carries the store's locus
    ManifestKeyEmpty,                     // a manifest entry carried an empty capability key
    DuplicateManifestKey { key: String }, // two entries claim the same key
}
impl From<StateStoreError> for ReportError { /* -> ReportError::Store */ }

pub struct CapabilityManifestEntry {      // Debug, Clone, PartialEq, Eq — CALLER-SUPPLIED
    pub key: String,                      // capability key as the surface names it (e.g. §20.1 command)
    pub wired: bool,                      // caller's structural fact about its own surface
    pub evidence_unit: Option<UnitId>,    // run-scope unit bound as the capability's evidence
}
impl CapabilityManifestEntry {
    pub fn new(key: String, wired: bool, evidence_unit: Option<UnitId>)
        -> Result<Self, ReportError>;     // refuses empty/whitespace key (ManifestKeyEmpty)
}

pub struct UnitRow {                      // Debug, Clone, PartialEq, Eq
    pub unit: UnitId,
    pub category: StatusCategory,
    pub basis: String,                    // citation of the deciding records, never a claim
}

pub struct RunReportCapabilityRow {       // Debug, Clone, PartialEq, Eq
    pub key: String,
    pub wired: bool,
    pub category: StatusCategory,
    pub basis: String,
}

pub struct RunReport {                    // Debug, Clone, PartialEq, Eq
    pub run_id: RunId,
    pub run_present: bool,                // store holds a durable identity record for the run
    pub terminal_state: Option<TerminalState>,
    pub units: Vec<UnitRow>,              // every unit the durable records name, sorted by id
    pub capabilities: Vec<RunReportCapabilityRow>,  // manifest entries, sorted by key
    pub out_of_scope: Vec<UnitId>,        // projected out_of_scope, never attempted —
                                          // surfaced separately, NOT one of the six categories
}
impl RunReport {
    pub fn count(&self, category: StatusCategory) -> usize;
    pub fn to_json(&self) -> String;      // canonical machine rendering: fixed field order,
                                          // six category tokens always present in report order,
                                          // no clock read — identical records render byte-identically
    pub fn human_summary(&self) -> String; // concise human lines from the same struct
}

pub fn build_report(
    store: &dyn StateStore,
    run_id: &RunId,
    manifest: &[CapabilityManifestEntry],
    deadline: Deadline,
) -> Result<RunReport, ReportError>;      // read-only; an all-empty store yields all-empty
                                          // categories (run_present == false), never an error;
                                          // duplicate/empty manifest keys are typed refusals
```

Population rules (module docs pin the full list): implemented-and-proven =
settlement row + durable PASS record carrying that row's verification ref
(the pair is the proof); blocked projection outranks unproven attempt
records; settlement/PASS pair outranks a stale checkpoint projection;
provisional covers settlement-without-PASS, FAIL/INCONCLUSIVE records,
uncommitted PASS, in_progress projection, and settled projection without a
settlement row; capability rows never upgrade a not-wired declaration.

## Verification evidence (what the commands printed)

- `just check-package gauntlet-app` — PASS (B2a, exit 0, 5.09s; re-verified
  implicitly by all later runs).
- `just test-package gauntlet-app` — final run exit 0: **"458 tests run:
  458 passed, 0 skipped"**. Report tests: 12 total, all PASS —
  9 integration (`gauntlet-app::report …`) + 3 in-module
  (`report::tests::…`: exact tokens, empty manifest key refused, typed
  store failure).
- `just lint` — clippy over the report module: **zero findings** (clippy
  checked the whole gauntlet-app lib and reported only 2 errors in
  `control/controller.rs` — see out-of-scope reds).
- `just ruler` — **PASS**, exit 0: "answer key and manifest digests match
  the frozen lock".
- `just boundaries` — RED 224/228; **all 3 failures cite other slices'
  in-flight files, zero cite the report module** (details below).

## Teeth evidence

`removing_the_settlement_record_moves_the_unit_out_of_implemented_and_proven`
(tests/report.rs:274): same unit, same PASS verification record, settlement
record removed ⇒ `ImplementedAndProven` count 0, `Provisional` count 1,
basis names "no settlement record". Proof is the record pair read from the
store, not the intention. Companion teeth: `a_settlement_without_a_durable_pass_record_is_never_proven`,
`an_inconclusive_record_is_provisional_never_proven`,
`blocked_and_unknown_are_record_driven` (removing the blocked projection
empties the category),
`the_machine_rendering_is_deterministic_and_order_independent` (byte-identical
twice, and invariant under reversed store hand-over order),
`an_all_empty_store_populates_nothing` (all six categories empty, JSON
rendering still carries all six tokens as empty arrays, summary counts all
zero — exit-neutral),
`capability_rows_follow_the_manifest_and_the_records` (not-wired never
upgraded; foreign-run records prove nothing),
`duplicate_manifest_keys_are_a_typed_error`.

## Recorded divergence (B2a's, preserved)

Spec §26 and AK-067 phrase category 2 as **"implemented-but-provisional"**;
plan Task 20 step 3 and the brief mandate the exact spelling
**"provisional"**. The module follows the plan/brief spelling (`token()`
renders `provisional`) and records the divergence in the module docs and on
`StatusCategory::token`. AK-067 remains as written; the ruler was not
touched.

## Out-of-scope reds (recorded for the orchestrator, NOT fixed)

- `just lint` (workspace clippy, `-D warnings`): 2 errors, both in
  `crates/gauntlet-app/src/control/controller.rs` — unused import
  `WorkspaceSubject` (:145) and `clippy::needless_return` (:1120). That
  file is another slice's in-flight work (it is modified in git status and
  was being edited concurrently).
- `just boundaries`: 3 failures, all citing other slices' in-flight files:
  - `core_crates_contain_no_hardcoded_literals` (×2 twins): 4
    `hardcoded-absolute-path` violations at `control/controller.rs:7777-7778`
    (`'/tmp'`, `'/tmp/ws-fixture-{n}'`).
  - `answer_key_single_writer`: 4 violations, all in
    `crates/gauntlet-app/src/roles/install.rs` — `#[cfg(test)]` gating
    `DIRECTORY_ENTRY_FSYNC_CALLS` (I8 shape classification) and the
    resulting "stale" `WRITE_API_INVENTORY` entries (`fs::create_dir_all`,
    `fs::rename` blanked by the gated module). This looks like the in-flight
    D10 (write_atomic directory-entry fsync) work tripping the I8
    classification rule; the slice owning install.rs should classify the
    gated-item shape in the arch test's allowed grammar or drop the gate.
- Scope integrity itself is clean: `scope_integrity`, `escapes`, and
  `denylist` boundary tests all PASSED with the report module present.

## Environment failure (recorded, needs an orchestrator-level fix)

`sccache`'s server daemon inherits the `flock`'d fd from whatever
`flock .agents/reports/task20/compile.lock -c '<cargo>'` started it and then
holds `compile.lock` FOREVER after the compile ends, deadlocking every later
waiter (observed four times this session; fixed each time with a graceful
`sccache --stop-server`, which releases the lock; sccache auto-restarts and
is NOT disabled). Every agent using the compile lock is exposed. A durable
fix belongs outside this slice's edit boundary (e.g. a lock wrapper that
O_CLOEXECs the fd, or stopping the server in the `just` recipes).

## Deliberately not done (per brief/charter boundaries)

- No CLI wiring of the report and no capability manifest declaration — the
  CLI slice (B2) owns both; it composes against the API surface above.
- No port methods added; no new dependencies; nothing written by the report
  service (read-only over stores).
- No edits outside `report/mod.rs`, the lib.rs mod line,
  `tests/report.rs`, and the two B2a report/progress files. The 2 clippy
  errors and 3 boundary failures above were deliberately left for their
  owning slices.
- Builders do not commit (charter); all work is left uncommitted in the
  working tree for the orchestrator.
