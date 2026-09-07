# B2 report — task 26: app `baseline` module (completion contract)

Existence of this file with the slice battery green is the completion
claim. It carries NO settlement authority (charter §Battery). The
orchestrator owns integration (B3 wiring, plan gate) and closure.

## What landed (files)

- `crates/gauntlet-app/src/baseline/mod.rs` — module docs (honest
  posture + seam split + figure honesty) and the public re-export
  surface.
- `crates/gauntlet-app/src/baseline/profiles.rs` — `BaselineProfile`
  {A,B,C,D} and `BaselineScenario` {honest-success, lying-defect,
  subtle-defect, compromised-critic, silent} typed vocabularies;
  fail-closed parse naming token + vocabulary (tooth T4);
  `parse_profile_list` (dedup-free, empty segments refused).
- `crates/gauntlet-app/src/baseline/task.rs` — strict
  `baseline_task@v1` codec over the charter's exact six-field schema:
  unknown/duplicate/missing fields refused with 1-based lines; block
  scalars for prompt/requirement (YAML clip); absolute `held_out`
  refused; `scenarios` flow validated against the vocabulary,
  duplicates/empty refused; comments/fences/tabs/stray lines refused.
- `crates/gauntlet-app/src/baseline/harness.rs` — `BaselineExecutor`
  seam (`materialize_workspace` / `dispatch_role` / `run_full_loop` /
  `run_held_out`), typed request/outcome records, named request
  builders (`builder_request`, `review_request`, `critic_request` —
  the critic body is EXACTLY the requirement text), and
  `BaselineHarness::run_profile` implementing postures A/B/C/D + the
  held-out ground-truth run + harness-measured wall.
- `crates/gauntlet-app/src/baseline/metrics.rs` — classification
  predicates as small named functions (`is_false_completion`,
  `is_defect_escape`, `settlement_reliability_per_10k`,
  `classify_terminal` + `LoopTerminal`, `posture_burden`), per-leg
  `LegResult`/`TerminalFacts`/`LoopFacts`/`Payment`/
  `VerificationBurden`/`RegressionsBasis`, `Totals::from_legs`
  aggregation, and `derive_loop_metrics` — D's figures read from the
  run's DURABLE record (the `derive_run_metrics` pattern) with the
  executor's claimed terminal cross-checked against the durable last
  `RunTerminal` (disagreement = INCONCLUSIVE, never a figure).
- `crates/gauntlet-app/src/baseline/report.rs` — `BaselineReport` /
  `ProfileResult` / `ReportMeta`; JSON via `gauntlet_ports::json`
  (the repo's own machinery; NO new dependencies); top-level keys
  EXACTLY `A`,`B`,`C`,`D`,`_meta`; `assemble()` refuses any run
  missing one of the four profiles; six honest disclosures
  (posture/zero-payment/regressions/cost-units/reliability/bounded)
  carried verbatim in `_meta`.
- `crates/gauntlet-app/src/lib.rs` — one line: `pub mod baseline;`.
- `crates/gauntlet-app/tests/baseline_harness.rs` — integration test
  over a fake executor (below).

## Public API surface for B3 (exact names)

All paths `gauntlet_app::baseline::*` (also re-exported flat from
`gauntlet_app::baseline`).

```rust
// ---- profiles.rs
pub enum BaselineProfile { A, B, C, D }
impl BaselineProfile {
    pub const ALL: &'static [Self];
    pub fn all() -> &'static [Self];
    pub fn parse(token: &str) -> Result<Self, UnknownToken>;
    pub fn as_str(self) -> &'static str;          // "A".."D"
}
pub enum BaselineScenario { HonestSuccess, LyingDefect, SubtleDefect, CompromisedCritic, Silent }
impl BaselineScenario { /* same surface; as_str tokens are the
                         GAUNTLET_BASELINE_MODE values */ }
pub struct UnknownToken { pub input: String, pub vocabulary: &'static str }
pub enum ProfileListError { Unknown(UnknownToken), EmptySegment { at: usize } }
pub fn parse_profile_list(spec: &str) -> Result<Vec<BaselineProfile>, ProfileListError>;

// ---- task.rs
pub const SCHEMA_TOKEN: &str = "baseline_task@v1";
pub struct BaselineTask { /* opaque */ }
impl BaselineTask {
    pub fn id(&self) -> &str;
    pub fn prompt(&self) -> &str;                 // builder envelope body
    pub fn requirement(&self) -> &str;            // critic envelope body (verbatim input to C)
    pub fn held_out(&self) -> &std::path::Path;   // fixture-relative instrument
    pub fn scenarios(&self) -> &[BaselineScenario];
}
pub enum TaskError { SchemaToken{line,found}, UnknownField{line,field},
    DuplicateField{line,field}, MissingField{field}, EmptyField{field},
    HeldOutNotRelative{line,path}, UnknownScenario{line,source},
    ScenarioList{line,detail}, Malformed{line,detail} }
pub fn parse_task(text: &str) -> Result<BaselineTask, TaskError>;

// ---- harness.rs
pub enum Role { Builder, Reviewer, Critic }       // as_str: "builder"/"reviewer"/"critic"
pub const DECLARED_CHECK_PATH: &str = "scripts/check.sh";
pub struct DispatchRequest {
    pub role: Role,
    pub body: String,             // prompt | review directive | EXACTLY task.requirement()
    pub workspace: std::path::PathBuf,
    pub task_id: String,
    pub scenario: BaselineScenario,
    pub dispatch_index: usize,    // 0-based position within the leg
}                                 // NO field can carry a role result document (T2, structural)
pub struct DispatchOutcome {
    pub claimed_complete: bool,
    pub self_check_exit: Option<i32>,   // reviewer legs; None never passes
    pub verdict: Option<gauntlet_domain::Outcome>, // critic legs; None/Inconclusive never completes
    pub payment: Payment,
    pub regressions: Option<u64>,       // executor's OPTIONAL base-state comparison
    pub detail: String,
}
pub struct HeldOutVerdict { pub passed: bool, pub detail: String }
pub struct LoopRunOutcome {
    pub run_id: gauntlet_domain::RunId,
    pub final_state: gauntlet_domain::TerminalState, // CROSS-CHECKED vs durable record
    pub reserve_intact: bool,                        // executor computes vs its effective config
    pub tool_calls: Option<u64>,
    pub regressions: Option<u64>,                    // executor's optional durable-facts derivation
    pub state: std::sync::Arc<dyn gauntlet_ports::state_store::StateStore>,
}
pub enum BaselineError { Configuration{detail}, Execution{locus:&'static str,detail},
    InconclusiveEvidence{detail} }  // impl Display + Error
pub trait BaselineExecutor {
    fn materialize_workspace(&self, task: &BaselineTask, scenario: BaselineScenario,
        parent: &std::path::Path) -> Result<std::path::PathBuf, BaselineError>;
    fn dispatch_role(&self, request: &DispatchRequest) -> Result<DispatchOutcome, BaselineError>;
    fn run_full_loop(&self, task: &BaselineTask, project_root: &std::path::Path,
        scenario: BaselineScenario) -> Result<LoopRunOutcome, BaselineError>;
    fn run_held_out(&self, workspace: &std::path::Path, task: &BaselineTask)
        -> Result<HeldOutVerdict, BaselineError>;
}
pub fn builder_request(task: &BaselineTask, workspace: &Path, scenario: BaselineScenario,
    dispatch_index: usize) -> DispatchRequest;
pub fn review_request(/* same args */) -> DispatchRequest;
pub fn critic_request(/* same args */)  -> DispatchRequest;  // body == task.requirement() verbatim
pub struct BaselineHarness { /* new(clock) */ }
impl BaselineHarness {
    pub fn new(clock: std::sync::Arc<dyn gauntlet_ports::clock::Clock>) -> Self;
    pub fn run_profile(&self, executor: &dyn BaselineExecutor, profile: BaselineProfile,
        task: &BaselineTask, workspace_root: &Path, scenarios: &[BaselineScenario],
        deadline: gauntlet_ports::deadline::Deadline) -> Result<ProfileResult, BaselineError>;
}

// ---- metrics.rs
pub struct Payment { pub tokens: u64, pub tool_calls: u64, pub wall_seconds: u64,
    pub cost_usd_micro: u64 }            // add(): saturating sum
pub struct VerificationBurden { pub instrument_runs: u64, pub review_dispatches: u64 }
pub enum RegressionsBasis { NotMeasured, BaseStateComparison, DurableFacts } // as_str
pub enum LoopTerminal { Settled, RefusedTerminally, BoundedNotConverged }    // as_str, bounded()
pub fn is_false_completion(claimed_complete: bool, held_out_passed: bool) -> bool;
pub fn is_defect_escape(scenario: BaselineScenario, false_completion: bool) -> bool;
pub fn settlement_reliability_per_10k(deterministic_backed: u64, completions: u64) -> Option<u64>;
pub fn classify_terminal(state: gauntlet_domain::TerminalState) -> LoopTerminal;
pub fn posture_burden(profile: BaselineProfile) -> Option<VerificationBurden>; // None for D
pub struct TerminalFacts { pub classification: Option<LoopTerminal>,
    pub durable_state: Option<String>, pub bounded_termination: bool,
    pub reserve_intact: Option<bool> }
pub struct LoopFacts { pub run_id: String, pub settled_units: Vec<String>,
    pub verification_records: u64, pub deterministic_verified_settled: u64,
    pub iterations: u64 }
pub struct LegResult { /* pub fields: task_id, scenario, profile, claimed_complete,
    held_out_passed, false_completion, defect_escape, regressions, regressions_basis,
    dispatches, payment, measured_wall_ms, burden, settlement_reliability_per_10k,
    terminal: TerminalFacts, d: Option<LoopFacts> */ }
pub struct Totals { /* pub fields; */ }
impl Totals { pub fn from_legs(legs: &[LegResult]) -> Totals; }  // call ONCE PER PROFILE
pub enum LoopMetricsError { NotDurable{run}, State(_), Undecidable{run,detail},
    TerminalDisagreement{run,detail} }
pub fn derive_loop_metrics(state: &dyn StateStore, run: &RunId,
    claimed_terminal: TerminalState, tool_calls: Option<u64>,
    deadline: Deadline) -> Result<DerivedLoopMetrics, LoopMetricsError>;
pub struct DerivedLoopMetrics { pub d: LoopFacts, pub durable_terminal: TerminalState,
    pub dispatches: u64, pub review_dispatches: u64, pub payment: Payment }

// ---- report.rs
pub struct ProfileResult { pub profile: BaselineProfile, pub posture: &'static str,
    pub completion_basis: &'static str, pub legs: Vec<LegResult>, pub totals: Totals }
pub struct ReportMeta { pub task_ids: Vec<String>,
    pub fixture_digests: Vec<(String, String)>,  // CLI derives (app does no IO)
    pub profiles: Vec<BaselineProfile> }
pub struct BaselineReport { /* opaque */ }
impl BaselineReport {
    pub fn assemble(profiles: Vec<ProfileResult>, meta: ReportMeta)
        -> Result<Self, BaselineError>;       // requires EXACTLY A,B,C,D
    pub fn to_json(&self) -> gauntlet_ports::json::JsonValue;
    pub fn to_json_text(&self) -> String;     // the --out bytes
}
pub fn posture_description(profile: BaselineProfile) -> &'static str;
pub fn completion_basis(profile: BaselineProfile) -> &'static str;
pub const POSTURE_DISCLOSURE / ZERO_PAYMENT_DISCLOSURE / REGRESSIONS_DISCLOSURE /
    COST_UNIT_DISCLOSURE / RELIABILITY_DISCLOSURE / BOUNDED_DISCLOSURE: &str;
pub fn task_ids(tasks: &[BaselineTask]) -> Vec<String>;
```

### B3 wiring notes (decisions B3 inherits, all documented in mod docs)

- Per leg the harness calls `materialize_workspace` ONCE (fresh scratch
  copy per scenario; nothing writes into `tests/fixtures/`), then the
  posture's dispatches / `run_full_loop`, then `run_held_out` over the
  final workspace. A timed-out/refusing CHILD is an OUTCOME
  (`claimed_complete: false`), never an error — only a dispatch that
  cannot EXECUTE is `BaselineError::Execution`.
- D: `run_full_loop` must return the run's OWN `StateStore` handle (the
  evaluate `ExecutionOutcome` pattern); the harness derives everything
  durable and refuses on terminal disagreement. `reserve_intact` is the
  CLI's computation against its effective config.
- `run_profile` refuses undeclared/empty scenario lists
  (`BaselineError::Configuration`) — pass `task.scenarios()`.
- Report: run `run_profile` per profile, call `Totals::from_legs` is
  already done inside `run_profile`; assemble with `ReportMeta`
  (fixture digests are the CLI's sha2 job) and write `to_json_text()` to
  `--out` (default `.agents/reports/baselines.json`).
- The plan gate's python key-set assert (`set(d) >= {'A','B','C','D'}`)
  is mirrored in `the_report_has_exactly_the_charter_keys_and_disclosures`
  (exact key vector `["A","B","C","D","_meta"]`).

## Test inventory (all green; RED-first proven where noted)

Teeth and refusals (in-module unit tests):

- T4 profile parse: `every_profile_token_parses`,
  `unknown_profile_tokens_are_refused_fail_closed` (lowercase/padded/
  unknown all refused, error names token + vocabulary).
- T4 scenario parse: `every_scenario_token_parses`,
  `unknown_scenario_tokens_are_refused`.
- `profile_list_parses_in_order_without_dedup`,
  `profile_list_refuses_empty_segments_and_unknown_members`.
- Task codec: `the_charter_fixture_shape_parses`,
  `field_order_is_free_and_blank_lines_are_skipped`,
  `wrong_schema_token_is_refused_with_its_line`,
  `unknown_fields_are_refused_with_their_line`,
  `duplicate_fields_are_refused`, `missing_fields_are_refused_by_name`
  (all six fields), `absolute_held_out_is_refused`,
  `scenarios_outside_the_vocabulary_are_refused_with_their_line`,
  `empty_and_duplicate_scenario_lists_are_refused`,
  `comments_fences_and_tabs_are_refused`,
  `empty_prompt_body_and_non_block_prompt_are_refused`,
  `a_line_without_a_key_shape_is_refused`.
- T1 predicates: `t1_false_completion_distinguishes_claimed_failed_rows`,
  `t1_defect_escape_requires_a_defect_scenario_and_a_false_completion`,
  `t1_settlement_reliability_is_the_pooled_ratio_or_none`,
  `t1_terminal_classification_covers_the_closed_state_set`,
  `posture_burden_matches_the_charter_per_profile`.
- Totals: `totals_sum_legs_and_pool_reliability`,
  `totals_of_a_completing_nothing_profile_have_no_reliability_figure`,
  `totals_pool_d_settlements_across_legs`.
- D durable derivation (fake StateStore):
  `derive_loop_metrics_reads_the_durable_record`,
  `a_model_instrument_settlement_is_not_deterministically_backed`,
  `a_terminal_disagreement_is_inconclusive_not_a_figure`,
  `a_non_durable_run_is_not_durable`,
  `a_missing_durable_terminal_is_undecidable`.
- Harness postures (in-module, fake executor):
  `posture_a_makes_one_dispatch_and_trusts_it`,
  `posture_b_reviews_with_the_declared_check` (failing check refuses,
  passing completes, reviewer body is the directive),
  `t2_the_critic_envelope_is_blind_to_the_builder_result` (body ==
  requirement verbatim, forged marker absent),
  `posture_c_completes_on_claim_and_critic_pass`,
  `honest_completion_is_not_a_false_completion`,
  `every_leg_gets_a_fresh_workspace`,
  `measured_wall_is_harness_measured` (RED-first: failed before the
  tick-count was pinned; assertion TIGHTENED to equality),
  `an_empty_scenario_list_is_refused`,
  `an_undeclared_scenario_is_refused`,
  `a_failing_dispatch_is_a_typed_execution_error`.
- Report: `the_report_keys_are_exactly_a_b_c_d_and_meta`,
  `assembling_without_every_profile_is_refused`,
  `report_order_is_charter_order`,
  `leg_json_carries_every_charter_metric_field`.

Integration (`tests/baseline_harness.rs`, fake executor over 4 profiles
x 5 scenarios):

- `the_postures_have_their_charter_dispatch_shapes` (A=1, B=2, C=2
  dispatches per leg; D delegates once per leg; 20 fresh workspaces).
- `the_derived_scenario_matrix_matches_the_charter` (the full expected
  escape shape incl. C catches subtle-defect, B/D escape it, C escapes
  compromised-critic while D refuses, silent bounded everywhere, D
  reserve intact on budget-bounded).
- `t2_no_critic_request_ever_carries_the_builder_result` (composed
  level: every critic body == requirement verbatim, marker absent).
- `d_pays_the_most_and_carries_the_reliability_figures` (RED-first:
  failed until the fake store scripted D's correction round — fixture
  fixed, assertion untouched).
- `the_report_has_exactly_the_charter_keys_and_disclosures` (key vector
  exactly A,B,C,D,_meta; per-leg charter metric list; disclosures carry
  "NOT a frontier-model claim" + "deterministic fixture children").
- `a_malformed_task_document_is_a_typed_refusal`.

## Slice battery results

- `just check-package gauntlet-app` — exit 0, NO warnings
  (flock'd top-level command; `sccache --stop-server` inside the same
  flock; no nested flock; no workspace-wide runs).
- `just test-package gauntlet-app` — exit 0, **618/618 passed**
  (includes every pre-existing gauntlet-app test: no regressions).

## Charter deviations (with reasons)

1. **B's second dispatch carries role `reviewer`, not `builder`.** The
   charter's profile-B prose says "a SECOND dispatch in the builder
   role", but the binding child-protocol contract (same charter §
   Interface contract) has the child branch on ROLE with a dedicated
   `reviewer` branch that re-runs `scripts/check.sh`. The wire token is
   the child protocol's; the MEASURED semantics (shared-criterion
   self-review) are exactly the charter's. Documented in harness.rs.
2. **Critic envelope body is the requirement text ALONE** (no separate
   artifact-locator concatenation). The charter's requirement field is
   "critic envelope body (full property text; Y included)" and its
   scenario table says the requirement carries the marker tokens — the
   locator travels inside the requirement text, which keeps "body ==
   requirement verbatim" exactly assertable (tooth T2's pinned shape).
   The workspace itself reaches the critic via the request's workspace
   field.
3. **`payment.cost_usd` JSON field carries micro-dollars** (the durable
   MicroUsd integer, the repo's only durable cost unit), disclosed in
   `_meta.cost_units_disclosure`. The charter's schema name is kept;
   the unit is disclosed rather than silently re-scaled or floated.
4. **`held_out` is emitted as the token `"pass"`/`"fail"`** (charter
   wording); INCONCLUSIVE is unreachable by construction (the held-out
   instrument is exit-code deterministic; a failed execution is a typed
   harness error, never a verdict).
5. **Block-scalar grammar subset**: comments and `---` fences are
   REFUSED in task documents (strictness is the feature; the interface
   contract's fixture shape carries neither). If B1's fixture adds a
   comment, the parse refusal names the line — that is a fixture
   contract violation to fix in the fixture, not parser leniency.

No test was weakened to reach green; the two development-time REDs were
resolved by tightening an assertion (tick-exact wall) and by correcting
the test fixture (D's scripted correction round).
