//! Baseline report assembly (task 26): the `.agents/reports/baselines.json`
//! document's typed shape, serialized with the repo's own JSON machinery
//! ([`gauntlet_ports::json`]) — no new dependencies, none at all.
//!
//! Top-level keys are EXACTLY `A`, `B`, `C`, `D` plus `_meta` (the plan
//! gate's `set(d) >= {'A','B','C','D'}` holds, and nothing else leaks in).
//! Every profile object carries its posture disclosure, its completion
//! basis, its legs, and its totals. The `_meta` block carries the honest-
//! posture wordings (deterministic fixture children — never a frontier-
//! model claim), the zero-payment disclosure, the regressions/cost-unit/
//! reliability figure disclosures, and the caller-supplied fixture facts
//! (task ids, digests).
//!
//! Assembling a report REQUIRES all four profiles: a report missing a
//! profile is not a partial measurement, it is a misconfigured run.

use gauntlet_ports::json::{JsonNumber, JsonValue};

use super::harness::BaselineError;
use super::metrics::LegResult;
use super::profiles::BaselineProfile;
use super::task::BaselineTask;

/// The posture disclosure per profile — what the posture DOES, in the
/// charter's own terms.
pub fn posture_description(profile: BaselineProfile) -> &'static str {
    match profile {
        BaselineProfile::A => {
            "builder-only: one builder dispatch per leg; completion is the builder \
             result document's own claim; zero posture-internal verification"
        }
        BaselineProfile::B => {
            "builder+self-review: the builder dispatch, then a reviewer dispatch \
             re-running the project's declared check; self-review shares the \
             builder's own criteria (that shared-criterion blind spot is part of \
             what is measured)"
        }
        BaselineProfile::C => {
            "builder+fresh-critic: the builder dispatch, then a fresh critic \
             dispatch whose envelope carries ONLY the task requirement text (the \
             builder's result document is structurally excluded); the critic \
             inspects the workspace artifact against the stated requirement"
        }
        BaselineProfile::D => {
            "full-Gauntlet: the real composed loop (reference freeze → dispatch → \
             observation → comparison → verification → settlement) over the task \
             project, with its own instruments, budget, and settlement machinery"
        }
    }
}

/// The completion-basis disclosure per profile — what "completed" MEANS.
pub fn completion_basis(profile: BaselineProfile) -> &'static str {
    match profile {
        BaselineProfile::A => "the builder result document's own claimed_complete (self-reported)",
        BaselineProfile::B => "the builder claim AND the review dispatch's self-check exit 0",
        BaselineProfile::C => "the builder claim AND the fresh critic's pass verdict",
        BaselineProfile::D => {
            "durable settlement: the loop reached its settled terminal with \
             instrument-produced verification records"
        }
    }
}

/// The honest-posture disclosure — the binding wording discipline, in every
/// report and the narrative: the measured subject is the orchestration
/// postures' verification economics, not model intelligence. The concrete
/// adapter is the composition root's to name and disclose (the app layer
/// describes the configuration fact, never the vendor token).
pub const POSTURE_DISCLOSURE: &str = "\
     Baseline measurement harness over deterministic fixture children under \
     the composition's real configured adapter (a real, configured runner \
     behind the port, the task-25 precedent). This is NOT a frontier-model \
     claim and must never be read as one. What is real: the AgentRunner port, \
     the contract delivery, poll classification, deadline/cancel mechanics, \
     the composed loop, and the durable records the metrics derive from. What \
     is fixture-scripted: the children's work products and their \
     discrimination behavior. The measured subject is the orchestration \
     postures' verification economics.";

/// The zero-payment disclosure (charter decision 6; m4 wording: the wall
/// figure is monotonic milliseconds, so sub-second legs carry real
/// figures).
pub const ZERO_PAYMENT_DISCLOSURE: &str = "\
     Fixture children report zero tokens and zero cost (mechanical protocol); \
     child wall time is zero and every leg's wall figure is the harness's own \
     external measurement (measured_wall_ms, monotonic milliseconds measured \
     inside the harness). The meaningful payment dimensions are dispatch \
     count, posture-internal verification burden, and externally measured \
     wall-clock; profile D's payment comes from its durable checkpoint \
     counters.";

/// The regressions-figure disclosure (charter decision 7).
pub const REGRESSIONS_DISCLOSURE: &str = "\
     regressions counts properties that passed before the leg's work and fail at \
     completion. A/B/C postures carry no base-state comparison machinery: their \
     figure is the honest 0 with basis 'not-measured' unless the executor \
     provided a base-state comparison (basis 'base-state-comparison'). Profile \
     D's figure is executor-derived from durable facts (basis 'durable-facts') \
     when its composition derives one. The basis field travels beside every \
     count.";

/// The cost-unit disclosure (the durable counter's unit).
pub const COST_UNIT_DISCLOSURE: &str = "\
     payment.cost_usd carries the durable MicroUsd integer (micro-dollars), the \
     unit every durable cost counter in this runtime uses.";

/// The reliability disclosure — the A/B/C honesty statement (m1: no
/// denominator, no figure).
pub const RELIABILITY_DISCLOSURE: &str = "\
     settlement_reliability_per_10k is completions backed by durable \
     deterministic-instrument-produced evidence over total completions, at \
     per-10k fixed point. For A/B/C this is 0 BY CONSTRUCTION whenever the \
     posture completed something: the posture carries no durable substrate, \
     so no completion is instrument-backed. A leg (or profile) that completed \
     nothing carries NO figure (null). For D it joins the durable settlement \
     rows with their deciding verification records.";

/// The bounded-termination disclosure.
pub const BOUNDED_DISCLOSURE: &str = "\
     bounded_termination is true for every recorded leg: the A/B/C postures are \
     structurally finite (fixed dispatch plans), and a D leg produces a record \
     only when the composed loop reached a durable terminal (its budget and \
     deadline bounds included). A timed-out or refusing child is an outcome, not \
     an error — the bounded honest termination is a measured result.";

/// The profile-D held-out surface disclosure (critic residual 6, m2): the
/// RECORDED report names the mechanism and the fallback basis, not just
/// the operator stdout.
pub const D_HELD_OUT_SURFACE_DISCLOSURE: &str = "\
     profile D's held_out figure is judged over judge-owned instrument \
     snapshots of the dispatched work, taken at every facet execution (the \
     composed loop releases its per-unit workspaces, so the untouched leg \
     project copy is not the work product); each leg's held_out_basis field \
     names its basis — instrument-snapshot, or leg-copy-fallback when the \
     leg produced no snapshot at all (its held_out was judged over the leg \
     project copy, the disclosed fallback; A/B/C legs judge their own leg \
     copy, basis leg-copy).";

/// The run-id scope disclosure (critic residual 7, m3, adjudicated:
/// disclose, do not reseed).
pub const RUN_ID_SCOPE_DISCLOSURE: &str = "\
     profile D's run_id values are scoped to each leg's own state store, not \
     globally unique: one shared id seed is deliberate, for reproducibility, \
     and every leg runs in its own scratch world with its own database, so \
     there is no collision — but a run_id identifies a run WITHIN its leg's \
     store, not across stores.";

/// The instrument-run denominator disclosure (task-26 corrections, F4):
/// the `instrument_runs` figures are NOT comparable across profiles — the
/// denominators differ by construction, and both are recorded as measured.
pub const INSTRUMENT_RUN_DENOMINATOR_DISCLOSURE: &str = "\
     totals.instrument_runs is NOT comparable across profiles: the \
     denominators differ by construction. Profile D's figure counts its \
     durable verification records on SETTLING legs only (a refusing leg's \
     facet executions contribute none), while B and C count one \
     declared-check re-run per leg. Both are recorded as measured; a reader \
     comparing the raw figures across profiles is comparing different \
     denominators.";

/// The judge-channel boundary disclosure (task-26 corrections, F1 leg 3,
/// adjudicated; REWORDED at COR-5 per the round-2 falsifier adjudication):
/// the EXECUTED finding, stated plainly. Under the fixture's conservative
/// `none` posture the dispatched child holds full same-user OS
/// permissions, so no judge material inside its reach can be made
/// unforgeable — the falsifier FORGED the channel successfully three ways
/// (wrapper swap-window with self-restore, in-place snapshot content
/// rewrites, count-preserving `.absent`→`.rs` replacement; rounds 1-2).
/// The defenses raise the bar and fail grading closed on detected tamper,
/// but detection at grading time is a race, not an invariant; the
/// recorded D held-out figures stand under the declared FIXED-CHILDREN
/// threat model, and a hostile-child threat model requires a privileged
/// judge channel — a recorded structural prerequisite, not something this
/// posture can deliver.
pub const JUDGE_CHANNEL_BOUNDARY_DISCLOSURE: &str = "\
     under the none posture the dispatched child holds full same-user OS \
     permissions, so every piece of judge material inside its reach — the \
     wrapper, the snapshots, the snapshot manifest, the leg's own state \
     and evidence stores — is forgeable by it, and WAS forged \
     successfully by executed hostile-child attacks: a wrapper \
     swap-window with self-restore, in-place snapshot content rewrites, \
     and count-preserving .absent→.rs replacement (task-26 falsifier \
     rounds 1-2). The defenses — the wrapper digest pin, the \
     nonce+count cross-check, the snapshot manifest — raise the bar and \
     fail the leg's held-out grading closed over judge-channel-tampered \
     (held_out inconclusive, false_completion null) when they detect \
     tamper, but detection at grading time is a race, not an invariant. \
     The recorded D held-out figures stand under the declared \
     FIXED-CHILDREN threat model (the fixture children are repo-owned, \
     git-pinned data — a hostile child is outside what they can exhibit). \
     A hostile-child threat model requires a privileged judge channel \
     (uid separation or a sandboxed judge) — a recorded structural \
     prerequisite, out of this task's fixed posture and not delivered by \
     any in-leg defense.";

/// The caller-supplied fixture facts and the assembled report.
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineReport {
    profiles: Vec<ProfileResult>,
    meta: ReportMeta,
}

/// The `_meta` inputs the app cannot know: which fixture tasks ran and
/// their digests (the CLI computes digests; the app layer does no IO).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReportMeta {
    pub task_ids: Vec<String>,
    /// `(task id, sha256 digest)` pairs the CLI derives from the fixture
    /// task set.
    pub fixture_digests: Vec<(String, String)>,
    /// The profiles the run declared (in CLI order).
    pub profiles: Vec<BaselineProfile>,
}

impl BaselineReport {
    /// Assembles the report. ALL FOUR profiles are required, exactly once
    /// each — a report missing a profile is refused, never emitted short.
    pub fn assemble(profiles: Vec<ProfileResult>, meta: ReportMeta) -> Result<Self, BaselineError> {
        for expected in BaselineProfile::ALL {
            let count = profiles
                .iter()
                .filter(|profile| profile.profile == *expected)
                .count();
            if count != 1 {
                return Err(BaselineError::Configuration {
                    detail: format!(
                        "the report requires exactly one result for profile {}, found \
                         {count} — a missing profile is a misconfigured run, not a \
                         partial report",
                        expected.as_str()
                    ),
                });
            }
        }
        if profiles.len() != BaselineProfile::ALL.len() {
            return Err(BaselineError::Configuration {
                detail: format!(
                    "the report takes exactly the four profile results, got {}",
                    profiles.len()
                ),
            });
        }
        // Report order is the charter's A, B, C, D — not CLI argument order
        // (built directly in declaration order; no re-sorting needed).
        let ordered: Vec<ProfileResult> = BaselineProfile::ALL
            .iter()
            .map(|expected| {
                profiles
                    .iter()
                    .find(|profile| profile.profile == *expected)
                    .expect("presence was verified above")
                    .clone()
            })
            .collect();
        Ok(Self {
            profiles: ordered,
            meta,
        })
    }

    /// The report as the ports JSON value (key order: A, B, C, D, _meta).
    pub fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("A".to_owned(), self.profile_json(&BaselineProfile::A)),
            ("B".to_owned(), self.profile_json(&BaselineProfile::B)),
            ("C".to_owned(), self.profile_json(&BaselineProfile::C)),
            ("D".to_owned(), self.profile_json(&BaselineProfile::D)),
            ("_meta".to_owned(), self.meta_json()),
        ])
    }

    /// The report as JSON text (the `--out` file's bytes).
    pub fn to_json_text(&self) -> String {
        self.to_json().to_json_text()
    }

    fn profile_json(&self, profile: &BaselineProfile) -> JsonValue {
        let result = self
            .profiles
            .iter()
            .find(|candidate| &candidate.profile == profile)
            .expect("assemble() verified exactly one per profile");
        JsonValue::Object(vec![
            (
                "posture".to_owned(),
                JsonValue::Str(result.posture.to_owned()),
            ),
            (
                "completion_basis".to_owned(),
                JsonValue::Str(result.completion_basis.to_owned()),
            ),
            (
                "legs".to_owned(),
                JsonValue::Array(result.legs.iter().map(leg_json).collect()),
            ),
            ("totals".to_owned(), totals_json(&result.totals)),
        ])
    }

    fn meta_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "posture_disclosure".to_owned(),
                JsonValue::Str(POSTURE_DISCLOSURE.to_owned()),
            ),
            (
                "zero_payment_disclosure".to_owned(),
                JsonValue::Str(ZERO_PAYMENT_DISCLOSURE.to_owned()),
            ),
            (
                "regressions_disclosure".to_owned(),
                JsonValue::Str(REGRESSIONS_DISCLOSURE.to_owned()),
            ),
            (
                "cost_units_disclosure".to_owned(),
                JsonValue::Str(COST_UNIT_DISCLOSURE.to_owned()),
            ),
            (
                "settlement_reliability_disclosure".to_owned(),
                JsonValue::Str(RELIABILITY_DISCLOSURE.to_owned()),
            ),
            (
                "bounded_termination_disclosure".to_owned(),
                JsonValue::Str(BOUNDED_DISCLOSURE.to_owned()),
            ),
            (
                "d_held_out_surface_disclosure".to_owned(),
                JsonValue::Str(D_HELD_OUT_SURFACE_DISCLOSURE.to_owned()),
            ),
            (
                "run_id_scope_disclosure".to_owned(),
                JsonValue::Str(RUN_ID_SCOPE_DISCLOSURE.to_owned()),
            ),
            (
                "instrument_run_denominator_disclosure".to_owned(),
                JsonValue::Str(INSTRUMENT_RUN_DENOMINATOR_DISCLOSURE.to_owned()),
            ),
            (
                "judge_channel_boundary_disclosure".to_owned(),
                JsonValue::Str(JUDGE_CHANNEL_BOUNDARY_DISCLOSURE.to_owned()),
            ),
            (
                "task_ids".to_owned(),
                JsonValue::Array(
                    self.meta
                        .task_ids
                        .iter()
                        .map(|id| JsonValue::Str(id.clone()))
                        .collect(),
                ),
            ),
            (
                "fixture_digests".to_owned(),
                JsonValue::Object(
                    self.meta
                        .fixture_digests
                        .iter()
                        .map(|(id, digest)| (id.clone(), JsonValue::Str(digest.clone())))
                        .collect(),
                ),
            ),
            (
                "profiles".to_owned(),
                JsonValue::Array(
                    self.meta
                        .profiles
                        .iter()
                        .map(|profile| JsonValue::Str(profile.as_str().to_owned()))
                        .collect(),
                ),
            ),
            (
                "scenario_vocabulary".to_owned(),
                JsonValue::Array(
                    super::profiles::BaselineScenario::ALL
                        .iter()
                        .map(|scenario| JsonValue::Str(scenario.as_str().to_owned()))
                        .collect(),
                ),
            ),
        ])
    }
}

/// One profile's assembled result (the harness returns these; the report
/// orders and serializes them).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileResult {
    pub profile: BaselineProfile,
    pub posture: &'static str,
    pub completion_basis: &'static str,
    pub legs: Vec<LegResult>,
    pub totals: super::metrics::Totals,
}

fn uint(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(n))
}

fn leg_json(leg: &LegResult) -> JsonValue {
    let mut fields = vec![
        ("task".to_owned(), JsonValue::Str(leg.task_id.clone())),
        (
            "scenario".to_owned(),
            JsonValue::Str(leg.scenario.as_str().to_owned()),
        ),
        (
            "claimed_complete".to_owned(),
            JsonValue::Bool(leg.claimed_complete),
        ),
        (
            "held_out".to_owned(),
            JsonValue::Str(leg.held_out.as_str().to_owned()),
        ),
        (
            "held_out_basis".to_owned(),
            JsonValue::Str(leg.held_out_basis.as_str().to_owned()),
        ),
        (
            "false_completion".to_owned(),
            match leg.false_completion {
                Some(derivable) => JsonValue::Bool(derivable),
                // F5: an inconclusive held-out reading derives NO figure —
                // null, never a silently derived false or true.
                None => JsonValue::Null,
            },
        ),
        (
            "defect_escape".to_owned(),
            JsonValue::Bool(leg.defect_escape),
        ),
        ("regressions".to_owned(), uint(leg.regressions)),
        (
            "regressions_basis".to_owned(),
            JsonValue::Str(leg.regressions_basis.as_str().to_owned()),
        ),
        ("dispatches".to_owned(), uint(leg.dispatches)),
        ("payment".to_owned(), payment_json(&leg.payment)),
        ("measured_wall_ms".to_owned(), uint(leg.measured_wall_ms)),
        ("verification_burden".to_owned(), burden_json(&leg.burden)),
        (
            "settlement_reliability_per_10k".to_owned(),
            match leg.settlement_reliability_per_10k {
                Some(figure) => uint(figure),
                None => JsonValue::Null,
            },
        ),
        ("terminal".to_owned(), terminal_json(&leg.terminal)),
    ];
    if let Some(d) = &leg.d {
        // D adds its durable-record facts (the charter's schema).
        fields.push(("run_id".to_owned(), JsonValue::Str(d.run_id.clone())));
        fields.push((
            "settled_units".to_owned(),
            JsonValue::Array(
                d.settled_units
                    .iter()
                    .map(|unit| JsonValue::Str(unit.clone()))
                    .collect(),
            ),
        ));
        fields.push((
            "verification_records".to_owned(),
            uint(d.verification_records),
        ));
        fields.push((
            "deterministic_verified_settled".to_owned(),
            uint(d.deterministic_verified_settled),
        ));
        fields.push(("iterations".to_owned(), uint(d.iterations)));
    }
    JsonValue::Object(fields)
}

fn payment_json(payment: &super::metrics::Payment) -> JsonValue {
    JsonValue::Object(vec![
        ("tokens".to_owned(), uint(payment.tokens)),
        ("tool_calls".to_owned(), uint(payment.tool_calls)),
        ("wall_seconds".to_owned(), uint(payment.wall_seconds)),
        ("cost_usd".to_owned(), uint(payment.cost_usd_micro)),
    ])
}

fn burden_json(burden: &super::metrics::VerificationBurden) -> JsonValue {
    JsonValue::Object(vec![
        ("instrument_runs".to_owned(), uint(burden.instrument_runs)),
        (
            "review_dispatches".to_owned(),
            uint(burden.review_dispatches),
        ),
    ])
}

fn terminal_json(terminal: &super::metrics::TerminalFacts) -> JsonValue {
    JsonValue::Object(vec![
        (
            "classification".to_owned(),
            match terminal.classification {
                Some(classification) => JsonValue::Str(classification.as_str().to_owned()),
                None => JsonValue::Null,
            },
        ),
        (
            "durable_state".to_owned(),
            match &terminal.durable_state {
                Some(state) => JsonValue::Str(state.clone()),
                None => JsonValue::Null,
            },
        ),
        (
            "bounded_termination".to_owned(),
            JsonValue::Bool(terminal.bounded_termination),
        ),
        (
            "reserve_intact".to_owned(),
            match terminal.reserve_intact {
                Some(intact) => JsonValue::Bool(intact),
                None => JsonValue::Null,
            },
        ),
    ])
}

fn totals_json(totals: &super::metrics::Totals) -> JsonValue {
    JsonValue::Object(vec![
        ("legs".to_owned(), uint(totals.legs)),
        ("completions".to_owned(), uint(totals.completions)),
        (
            "false_completions".to_owned(),
            uint(totals.false_completions),
        ),
        ("defect_escapes".to_owned(), uint(totals.defect_escapes)),
        ("regressions".to_owned(), uint(totals.regressions)),
        ("dispatches".to_owned(), uint(totals.dispatches)),
        ("tokens".to_owned(), uint(totals.payment.tokens)),
        ("tool_calls".to_owned(), uint(totals.payment.tool_calls)),
        ("cost_usd".to_owned(), uint(totals.payment.cost_usd_micro)),
        ("measured_wall_ms".to_owned(), uint(totals.measured_wall_ms)),
        (
            "instrument_runs".to_owned(),
            uint(totals.burden.instrument_runs),
        ),
        (
            "review_dispatches".to_owned(),
            uint(totals.burden.review_dispatches),
        ),
        (
            "settlement_reliability_per_10k".to_owned(),
            match totals.settlement_reliability_per_10k {
                Some(figure) => uint(figure),
                None => JsonValue::Null,
            },
        ),
    ])
}

/// Convenience: the task ids the report's `_meta` should name, from the
/// parsed tasks (the CLI's loop already has them).
pub fn task_ids(tasks: &[BaselineTask]) -> Vec<String> {
    tasks.iter().map(|task| task.id().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::metrics::{
        HeldOutBasis, HeldOutReading, Payment, RegressionsBasis, TerminalFacts, Totals,
        VerificationBurden,
    };
    use crate::baseline::profiles::BaselineScenario;

    /// The sample leg's wall figure, named (no numeral shares a unit with a
    /// duration-shaped identifier).
    const SAMPLE_WALL_FIGURE: u64 = 1_000;

    fn leg(profile: BaselineProfile, scenario: BaselineScenario) -> LegResult {
        LegResult {
            task_id: "t1".to_owned(),
            scenario,
            profile,
            claimed_complete: true,
            held_out: HeldOutReading::Fail,
            held_out_basis: HeldOutBasis::LegCopy,
            false_completion: Some(true),
            defect_escape: true,
            regressions: 0,
            regressions_basis: RegressionsBasis::NotMeasured,
            dispatches: 1,
            payment: Payment::default(),
            measured_wall_ms: SAMPLE_WALL_FIGURE,
            burden: VerificationBurden::default(),
            settlement_reliability_per_10k: Some(0),
            terminal: TerminalFacts {
                classification: None,
                durable_state: None,
                bounded_termination: true,
                reserve_intact: None,
            },
            d: None,
        }
    }

    fn profile_result(profile: BaselineProfile, scenarios: &[BaselineScenario]) -> ProfileResult {
        let legs: Vec<LegResult> = scenarios.iter().map(|s| leg(profile, *s)).collect();
        let totals = Totals::from_legs(&legs);
        ProfileResult {
            profile,
            posture: posture_description(profile),
            completion_basis: completion_basis(profile),
            legs,
            totals,
        }
    }

    fn full_results() -> Vec<ProfileResult> {
        let all = [
            BaselineScenario::HonestSuccess,
            BaselineScenario::LyingDefect,
            BaselineScenario::SubtleDefect,
            BaselineScenario::CompromisedCritic,
            BaselineScenario::Silent,
        ];
        vec![
            profile_result(BaselineProfile::A, &all),
            profile_result(BaselineProfile::B, &all),
            profile_result(BaselineProfile::C, &all),
            profile_result(BaselineProfile::D, &all),
        ]
    }

    /// The plan gate's key set: exactly A, B, C, D plus _meta — and the
    /// document reparses through the ports codec.
    #[test]
    fn the_report_keys_are_exactly_a_b_c_d_and_meta() {
        let report = BaselineReport::assemble(
            full_results(),
            ReportMeta {
                task_ids: vec!["t1".to_owned()],
                fixture_digests: vec![("t1".to_owned(), "sha256:ab".to_owned())],
                profiles: BaselineProfile::ALL.to_vec(),
            },
        )
        .expect("four profiles");
        let parsed = gauntlet_ports::json::parse(&report.to_json_text()).expect("valid JSON");
        let keys = parsed.keys();
        assert_eq!(keys, vec!["A", "B", "C", "D", "_meta"]);
        // Per-profile shape: posture, completion_basis, legs, totals.
        for key in ["A", "B", "C", "D"] {
            let profile = parsed.get(key).expect(key);
            assert_eq!(
                profile.keys(),
                vec![
                    "posture".to_owned(),
                    "completion_basis".to_owned(),
                    "legs".to_owned(),
                    "totals".to_owned()
                ]
            );
            assert_eq!(profile.get("legs").unwrap().keys(), Vec::<String>::new());
        }
        // The _meta carries the honest disclosures verbatim.
        let meta = parsed.get("_meta").expect("_meta");
        for field in [
            "posture_disclosure",
            "zero_payment_disclosure",
            "regressions_disclosure",
            "cost_units_disclosure",
            "settlement_reliability_disclosure",
            "bounded_termination_disclosure",
            "d_held_out_surface_disclosure",
            "run_id_scope_disclosure",
            "instrument_run_denominator_disclosure",
            "judge_channel_boundary_disclosure",
            "task_ids",
            "fixture_digests",
            "profiles",
            "scenario_vocabulary",
        ] {
            assert!(meta.get(field).is_some(), "_meta carries {field}");
        }
        let disclosure = match meta.get("posture_disclosure") {
            Some(JsonValue::Str(text)) => text.clone(),
            other => panic!("disclosure is a string, got {other:?}"),
        };
        assert!(disclosure.contains("NOT a frontier-model claim"));
    }

    /// A report missing a profile is refused — typed, naming the profile.
    #[test]
    fn assembling_without_every_profile_is_refused() {
        let mut results = full_results();
        results.retain(|result| result.profile != BaselineProfile::C);
        let error =
            BaselineReport::assemble(results, ReportMeta::default()).expect_err("C is missing");
        assert!(matches!(error, BaselineError::Configuration { .. }));
        assert!(error.to_string().contains('C'));

        let mut results = full_results();
        let extra_a = profile_result(BaselineProfile::A, &[]);
        results.push(extra_a);
        let error = BaselineReport::assemble(results, ReportMeta::default()).expect_err("A twice");
        assert!(matches!(error, BaselineError::Configuration { .. }));
    }

    /// Report order is A, B, C, D regardless of assembly order.
    #[test]
    fn report_order_is_charter_order() {
        let mut results = full_results();
        results.reverse();
        let report = BaselineReport::assemble(results, ReportMeta::default()).expect("four");
        let text = report.to_json_text();
        let a = text.find(r#""A":"#).expect("A");
        let b = text.find(r#""B":"#).expect("B");
        let c = text.find(r#""C":"#).expect("C");
        let d = text.find(r#""D":"#).expect("D");
        let meta = text.find(r#""_meta":"#).expect("_meta");
        assert!(a < b && b < c && c < d && d < meta);
    }

    /// The held-out token is pass/fail, and the leg's metric fields are the
    /// charter's list.
    #[test]
    fn leg_json_carries_every_charter_metric_field() {
        let mut leg = leg(BaselineProfile::A, BaselineScenario::LyingDefect);
        leg.d = None;
        let json = leg_json(&leg);
        let keys = json.keys();
        for field in [
            "task",
            "scenario",
            "claimed_complete",
            "held_out",
            "held_out_basis",
            "false_completion",
            "defect_escape",
            "regressions",
            "regressions_basis",
            "dispatches",
            "payment",
            "measured_wall_ms",
            "verification_burden",
            "settlement_reliability_per_10k",
            "terminal",
        ] {
            assert!(keys.iter().any(|key| key == field), "leg carries {field}");
        }
        assert!(!keys.iter().any(|key| key == "run_id"));
        assert_eq!(
            json.get("held_out"),
            Some(&JsonValue::Str("fail".to_owned()))
        );
        // m2: the judged basis travels beside the held-out token.
        assert_eq!(
            json.get("held_out_basis"),
            Some(&JsonValue::Str("leg-copy".to_owned()))
        );
    }

    /// F5 + F1 (task-26 corrections) at the serialized layer: an
    /// inconclusive held-out records the `"inconclusive"` token, a NULL
    /// false_completion (never a derived boolean), and the fail-closed
    /// basis tokens travel verbatim.
    #[test]
    fn an_inconclusive_leg_serializes_inconclusive_null_and_its_basis() {
        let mut inconclusive = leg(BaselineProfile::D, BaselineScenario::HonestSuccess);
        inconclusive.held_out = HeldOutReading::InstrumentError;
        inconclusive.held_out_basis = HeldOutBasis::JudgeChannelTampered;
        inconclusive.false_completion = None;
        let json = leg_json(&inconclusive);
        assert_eq!(
            json.get("held_out"),
            Some(&JsonValue::Str("inconclusive".to_owned()))
        );
        assert_eq!(json.get("false_completion"), Some(&JsonValue::Null));
        assert_eq!(
            json.get("held_out_basis"),
            Some(&JsonValue::Str("judge-channel-tampered".to_owned()))
        );

        let mut broken = leg(BaselineProfile::A, BaselineScenario::LyingDefect);
        broken.held_out = HeldOutReading::InstrumentError;
        broken.held_out_basis = HeldOutBasis::InstrumentError;
        broken.false_completion = None;
        let json = leg_json(&broken);
        assert_eq!(
            json.get("held_out_basis"),
            Some(&JsonValue::Str("held-out-instrument-error".to_owned()))
        );
        assert_eq!(json.get("false_completion"), Some(&JsonValue::Null));

        // A substantive row serializes exactly as before (the honest rows
        // are unchanged by the corrections).
        let substantive = leg(BaselineProfile::A, BaselineScenario::HonestSuccess);
        let json = leg_json(&substantive);
        assert_eq!(
            json.get("held_out"),
            Some(&JsonValue::Str("fail".to_owned()))
        );
        assert_eq!(json.get("false_completion"), Some(&JsonValue::Bool(true)));
    }
}
