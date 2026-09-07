//! The baseline measurement harness (task 26; plan §Task 26, AK-069):
//! runner profiles A/B/C/D over the same bounded tasks, with every figure
//! the plan names — held-out correctness, false completion, defect escape,
//! regressions, dispatch counts, payment, wall-clock, bounded termination,
//! verification burden, settlement reliability.
//!
//! # Honest posture (binding wording discipline)
//!
//! This module is a MEASUREMENT HARNESS, not a new set of loop
//! configurations: the composed loop is unchanged, and profile D runs
//! exactly the task-20/25 production path. The bounded tasks are driven by
//! DETERMINISTIC FIXTURE CHILDREN under the composition's real configured
//! adapter (the concrete adapter is the composition root's to name and
//! disclose) — that is NOT a frontier-model claim, and no wording in this
//! module, its reports, or its narrative may imply one. What the figures
//! measure is the ORCHESTRATION POSTURES' verification economics, not
//! model intelligence.
//!
//! # The seam split (the task-23 evaluate precedent, mirrored)
//!
//! - THIS crate owns: the typed profile/scenario vocabularies
//!   ([`profiles`], fail-closed parse), the strict `baseline_task@v1`
//!   codec ([`task`]), the leg orchestration over the executor seam
//!   ([`harness`]), the metric derivation and classification
//!   ([`metrics`], from dispatch outcomes and — for D — from the run's
//!   DURABLE record), and the report assembly ([`report`], the repo's own
//!   JSON machinery, zero new dependencies).
//! - The CLI composition root (builder B3) implements
//!   [`harness::BaselineExecutor`]: `materialize_workspace` (the
//!   task-20/25 scratch-world copy), `dispatch_role` (children through the
//!   real `AgentRunner` port — a handle, never an await), `run_full_loop`
//!   (the composed loop for profile D), and `run_held_out` (the fixture's
//!   ground-truth instrument). The app layer never spawns a process,
//!   touches an adapter, or reads the fixture tree.
//!
//! # Figure honesty
//!
//! Every classification predicate is a small named function with a unit
//! test that fails when its body flips (tooth T1). A posture lacking a
//! machinery records that honestly (`regressions_basis`, `reserve_intact:
//! null`, A/B/C settlement reliability of zero BY CONSTRUCTION) — no
//! metric is invented into existence, and missing evidence is never
//! passing evidence.

pub mod harness;
pub mod metrics;
pub mod profiles;
pub mod report;
pub mod task;

pub use harness::{
    BaselineError, BaselineExecutor, BaselineHarness, DECLARED_CHECK_DIR, DispatchOutcome,
    DispatchRequest, HeldOutVerdict, LoopRunOutcome, Role, builder_request, critic_request,
    declared_check_path, held_out_relative_path, review_request,
};
pub use metrics::{
    HeldOutBasis, HeldOutReading, LegResult, LoopFacts, LoopMetricsError, LoopTerminal, Payment,
    RegressionsBasis, TerminalFacts, Totals, VerificationBurden, classify_terminal,
    derive_loop_metrics, held_out_reading_from_exit, is_defect_escape, is_false_completion,
    posture_burden, settlement_reliability_per_10k,
};
pub use profiles::{
    BaselineProfile, BaselineScenario, ProfileListError, UnknownToken, parse_profile_list,
};
pub use report::{
    BOUNDED_DISCLOSURE, BaselineReport, COST_UNIT_DISCLOSURE, D_HELD_OUT_SURFACE_DISCLOSURE,
    INSTRUMENT_RUN_DENOMINATOR_DISCLOSURE, JUDGE_CHANNEL_BOUNDARY_DISCLOSURE, POSTURE_DISCLOSURE,
    ProfileResult, REGRESSIONS_DISCLOSURE, RELIABILITY_DISCLOSURE, RUN_ID_SCOPE_DISCLOSURE,
    ReportMeta, ZERO_PAYMENT_DISCLOSURE, completion_basis, posture_description,
};
pub use task::{BaselineTask, SCHEMA_TOKEN, TaskError, parse_task};
