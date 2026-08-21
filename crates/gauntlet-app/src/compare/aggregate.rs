//! Deterministic aggregation over `comparison_record@v1` documents (spec
//! §13.5; plan Task 10 D3; AK-032's mechanism).
//!
//! Spec §13.5, quoted where it shapes this module:
//!
//! - "Merging N comparison records MUST be deterministic and total."
//! - "Residuals are keyed by `(dimension, expected, observed_locator)`;
//!   duplicates merge, keeping the highest severity and the union of
//!   evidence refs."
//! - "Any `blocking` residual makes the aggregate verdict `fail`
//!   regardless of other dimensions and regardless of weighted scores."
//! - "Disagreement between comparators on the same key is preserved as a
//!   `disputed` flag, not averaged away; disputed blocking residuals route
//!   to verification or diagnosis, never to a majority vote."
//! - "Aggregation MUST NOT be performed by a model. It is arithmetic over
//!   typed records."
//!
//! This module is that arithmetic and nothing else: no `AgentRunner`, no
//! model, no clock, no randomness — the merge path reads typed records and
//! writes one. Determinism is by construction: every output collection is
//! emitted in canonical order (the merge key for residuals, the unit id
//! for hard gates, sorted-and-deduplicated unions everywhere), so the same
//! inputs in any order produce byte-identical canonical bytes (pinned by
//! the `deterministic_aggregation` test).
//!
//! The aggregate verdict rule (deterministic and total; the first matching
//! step applies):
//!
//! 1. any `blocking` residual or explicitly false hard gate ⇒ `fail`
//!    (a disputed blocking residual stays `disputed` so it routes to
//!    verification/diagnosis);
//! 2. otherwise any `disputed` residual ⇒ `inconclusive` (disagreement is
//!    never averaged away and never passes);
//! 3. otherwise any input verdict `inconclusive` or hard gate omitted by a
//!    contributor ⇒ `inconclusive` (missing evidence never becomes a pass,
//!    I8/§21.3);
//! 4. otherwise any input verdict `fail` ⇒ `fail`;
//! 5. otherwise `pass`.
//!
//! "Disagreement on the same key" (the `disputed` flag) means: two or more
//! contributors of the SAME merge key carried differing severities, or
//! their records carried differing verdict intents (one `fail`, one not).
//! A key only one comparator found is not disputed — it is a residual.

use std::collections::{BTreeMap, BTreeSet};

use gauntlet_domain::{EvidenceRef, Severity, UnitId, Verdict};
use gauntlet_ports::comparison_record::{
    ComparatorIdentity, ComparisonRecordDoc, HardGateResult, RecordContent, RecordVerdict,
    ResidualDoc,
};

/// The aggregate record's recorded provenance: the merge itself (spec
/// §13.5 — arithmetic over typed records; the empty model field says so).
pub const AGGREGATE_RUNNER: &str = "gauntlet-app::compare::aggregate";

/// Aggregation failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateError {
    /// Aggregating zero records is not a merge; an empty aggregate that
    /// exists would be a fabricated pass (I8).
    InputsEmpty,
    /// The records do not belong to one comparison: they disagree on a
    /// coupling field (run, iteration, digests, or contract version), so
    /// merging them would couple things that were never together.
    InputsDisagree {
        field: &'static str,
        first: String,
        other: String,
    },
}

impl core::fmt::Display for AggregateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InputsEmpty => write!(
                f,
                "aggregation needs at least one comparison record; an empty merge would be a \
                 fabricated pass"
            ),
            Self::InputsDisagree {
                field,
                first,
                other,
            } => write!(
                f,
                "the records disagree on {field}: {first:?} vs {other:?} — only records of one \
                 comparison merge"
            ),
        }
    }
}

impl std::error::Error for AggregateError {}

/// One residual under merge: everything its contributors said, reduced
/// deterministically. The `disputed` computation keeps the FACTS (which
/// severities and intents were seen) and derives the flag, so a single
/// contributor can never dispute with itself.
struct MergedResidual {
    id: String,
    dimension: String,
    expected: String,
    observed: String,
    unit_refs: BTreeSet<String>,
    evidence_refs: BTreeSet<String>,
    severities: BTreeSet<u8>,
    highest_severity: Severity,
    confidence: f64,
    saw_fail_intent: bool,
    saw_nonfail_intent: bool,
    /// A dispute recorded by a prior aggregation is itself evidence. Later
    /// aggregation may add disagreement but may never erase it (§13.5).
    inherited_dispute: bool,
    diagnosis: String,
    diagnosis_severity: Severity,
    cheapest_discriminating_test: String,
    test_severity: Severity,
    candidate_corrections: BTreeSet<String>,
}

impl MergedResidual {
    fn absorb(&mut self, residual: &ResidualDoc, fail_intent: bool) {
        if self.id.as_str() > residual.id().as_str() {
            self.id = residual.id().as_str().to_owned();
        }
        self.severities.insert(severity_rank(residual.severity()));
        if severity_rank(residual.severity()) > severity_rank(self.highest_severity) {
            self.highest_severity = residual.severity();
        }
        self.confidence = self.confidence.max(residual.confidence());
        self.saw_fail_intent |= fail_intent;
        self.saw_nonfail_intent |= !fail_intent;
        self.inherited_dispute |= residual.disputed();
        self.unit_refs.extend(
            residual
                .unit_refs()
                .iter()
                .map(|unit| unit.as_str().to_owned()),
        );
        self.evidence_refs.extend(
            residual
                .evidence_refs()
                .iter()
                .map(|e| e.as_str().to_owned()),
        );
        self.candidate_corrections
            .extend(residual.candidate_corrections().iter().cloned());
        self.diagnosis = prefer_text(
            std::mem::take(&mut self.diagnosis),
            residual.diagnosis(),
            residual.severity(),
            self.diagnosis_severity,
        );
        if self.diagnosis_severity != self.highest_severity
            && severity_rank(residual.severity()) == severity_rank(self.highest_severity)
        {
            self.diagnosis_severity = self.highest_severity;
        }
        self.cheapest_discriminating_test = prefer_text(
            std::mem::take(&mut self.cheapest_discriminating_test),
            residual.cheapest_discriminating_test(),
            residual.severity(),
            self.test_severity,
        );
        if self.test_severity != self.highest_severity
            && severity_rank(residual.severity()) == severity_rank(self.highest_severity)
        {
            self.test_severity = self.highest_severity;
        }
    }

    fn disputed(&self) -> bool {
        self.inherited_dispute
            || self.severities.len() > 1
            || (self.saw_fail_intent && self.saw_nonfail_intent)
    }

    fn into_doc(self) -> ResidualDoc {
        let disputed = self.disputed();
        let Self {
            id,
            dimension,
            expected,
            observed,
            unit_refs,
            evidence_refs,
            severities: _,
            highest_severity,
            confidence,
            saw_fail_intent: _,
            saw_nonfail_intent: _,
            inherited_dispute: _,
            diagnosis,
            diagnosis_severity: _,
            cheapest_discriminating_test,
            test_severity: _,
            candidate_corrections,
        } = self;
        ResidualDoc::new(
            gauntlet_domain::ResidualId::new(&id)
                .expect("the surviving id came from a decoded record"),
            dimension,
            unit_refs
                .into_iter()
                .map(|unit| UnitId::new(&unit).expect("ids came from decoded records"))
                .collect(),
            expected,
            observed,
            evidence_refs
                .into_iter()
                .map(|evidence| {
                    EvidenceRef::new(&evidence).expect("refs came from decoded records")
                })
                .collect(),
            highest_severity,
            confidence,
            disputed,
            diagnosis,
            cheapest_discriminating_test,
            candidate_corrections.into_iter().collect(),
        )
    }
}

/// Higher rank = more severe (blocking highest).
fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Blocking => 4,
        Severity::Major => 3,
        Severity::Minor => 2,
        Severity::Informational => 1,
    }
}

/// The deterministic text-merge rule: the incoming value replaces the kept
/// one only when the incoming contributor's severity ranks strictly higher
/// and the incoming value is non-empty, or when the kept value is empty
/// and the incoming is not; equal-severity ties take the lexicographically
/// smaller non-empty value. A pure function of the inputs alone.
fn prefer_text(
    kept: String,
    incoming: &str,
    incoming_severity: Severity,
    kept_severity: Severity,
) -> String {
    if incoming.is_empty() {
        return kept;
    }
    if kept.is_empty() {
        return incoming.to_owned();
    }
    match severity_rank(incoming_severity).cmp(&severity_rank(kept_severity)) {
        std::cmp::Ordering::Greater => incoming.to_owned(),
        std::cmp::Ordering::Less => kept,
        std::cmp::Ordering::Equal if incoming < kept.as_str() => incoming.to_owned(),
        std::cmp::Ordering::Equal => kept,
    }
}

/// Aggregates N comparison records of ONE comparison into a single record
/// (spec §13.5). Deterministic and total over agreeing inputs; see the
/// module doc for the verdict rule.
pub fn aggregate(docs: &[ComparisonRecordDoc]) -> Result<ComparisonRecordDoc, AggregateError> {
    let Some(anchor) = docs.first() else {
        return Err(AggregateError::InputsEmpty);
    };
    // Agreement: the records must agree on every coupling field. The check
    // is field-by-field so the error names the locus (spec 21.2).
    for doc in &docs[1..] {
        let pairs: [(&'static str, String, String); 6] = [
            (
                "run_id",
                anchor.run_id().as_str().to_owned(),
                doc.run_id().as_str().to_owned(),
            ),
            (
                "iteration_no",
                anchor.iteration_no().get().to_string(),
                doc.iteration_no().get().to_string(),
            ),
            (
                "reference_digest",
                anchor.reference_digest().to_string(),
                doc.reference_digest().to_string(),
            ),
            (
                "observation_digest",
                anchor.observation_digest().to_string(),
                doc.observation_digest().to_string(),
            ),
            (
                "envelope_digest",
                anchor.envelope_digest().to_string(),
                doc.envelope_digest().to_string(),
            ),
            (
                "comparison_contract_version",
                anchor.contract_version().to_owned(),
                doc.contract_version().to_owned(),
            ),
        ];
        for (field, first, other) in pairs {
            if first != other {
                return Err(AggregateError::InputsDisagree {
                    field,
                    first,
                    other,
                });
            }
        }
    }

    // Annex B.4 names its locator carrier `observed`; under the explicit
    // §13.5 reconciliation, that field IS `observed_locator`, not rendered
    // prose. The BTreeMap's canonical key order IS the output order.
    let mut residuals: BTreeMap<(String, String, String), MergedResidual> = BTreeMap::new();
    let mut any_inconclusive = false;
    let mut any_fail = false;
    for doc in docs {
        let fail_intent = doc.verdict().status() == Verdict::Fail;
        any_inconclusive |= doc.verdict().status() == Verdict::Inconclusive;
        any_fail |= fail_intent;
        for residual in doc.residuals() {
            residuals
                .entry((
                    residual.dimension().to_owned(),
                    residual.expected().to_owned(),
                    residual.observed().to_owned(),
                ))
                .and_modify(|merged| merged.absorb(residual, fail_intent))
                .or_insert_with(|| fresh_merge(residual, fail_intent));
        }
    }

    // Hard gates: keyed by unit; held only if EVERY contributor held it;
    // evidence refs are the union.
    let mut gates: BTreeMap<String, (bool, BTreeSet<String>, BTreeSet<usize>)> = BTreeMap::new();
    for (contributor_index, doc) in docs.iter().enumerate() {
        for gate in doc.hard_gate_results() {
            gates
                .entry(gate.unit_id().as_str().to_owned())
                .and_modify(|(held, refs, contributors)| {
                    *held &= gate.held();
                    contributors.insert(contributor_index);
                    refs.extend(gate.evidence_refs().iter().map(|e| e.as_str().to_owned()));
                })
                .or_insert_with(|| {
                    (
                        gate.held(),
                        gate.evidence_refs()
                            .iter()
                            .map(|e| e.as_str().to_owned())
                            .collect(),
                        BTreeSet::from([contributor_index]),
                    )
                });
        }
    }

    // A gate missing from any contributor is unavailable evidence (I8),
    // while an explicitly reported false gate is a deterministic failure.
    // Compute before consuming the map into B.4 output.
    let any_incomplete_gate = gates
        .values()
        .any(|(_, _, contributors)| contributors.len() != docs.len());
    let any_failed_gate = gates
        .values()
        .any(|(held, _, contributors)| !*held && contributors.len() == docs.len());

    // The verdict rule (module doc).
    let blocking = residuals
        .values()
        .any(|residual| residual.highest_severity == Severity::Blocking);
    let disputed_count = residuals
        .values()
        .filter(|residual| residual.disputed())
        .count();
    let disputed_blocking = residuals
        .values()
        .filter(|residual| residual.disputed() && residual.highest_severity == Severity::Blocking)
        .count();
    let status = if blocking || any_failed_gate {
        Verdict::Fail
    } else if disputed_count > 0 || any_inconclusive || any_incomplete_gate {
        Verdict::Inconclusive
    } else if any_fail {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
    let rationale = format!(
        "aggregate of {} record(s): {} residual(s) after key merge, {} blocking, {} disputed, \
         gates explicitly failed? {}, incomplete? {}; {} disputed blocking route to \
         verification/diagnosis, never a majority vote",
        docs.len(),
        residuals.len(),
        residuals
            .values()
            .filter(|residual| residual.highest_severity == Severity::Blocking)
            .count(),
        disputed_count,
        any_failed_gate,
        any_incomplete_gate,
        disputed_blocking,
    );

    let residuals: Vec<ResidualDoc> = residuals
        .into_values()
        .map(MergedResidual::into_doc)
        .collect();
    let hard_gate_results: Vec<HardGateResult> = gates
        .into_iter()
        .map(|(unit, (held, refs, contributors))| {
            // A contributor that omits a gate supplied no gate evidence. I8
            // makes that fail closed rather than treating absence as held.
            HardGateResult::new(
                UnitId::new(&unit).expect("ids came from decoded records"),
                held && contributors.len() == docs.len(),
                refs.into_iter()
                    .map(|evidence| {
                        EvidenceRef::new(&evidence).expect("refs came from decoded records")
                    })
                    .collect(),
            )
        })
        .collect();

    Ok(ComparisonRecordDoc::new(RecordContent {
        run_id: anchor.run_id().clone(),
        iteration_no: anchor.iteration_no(),
        reference_digest: anchor.reference_digest().clone(),
        observation_digest: anchor.observation_digest().clone(),
        envelope_digest: anchor.envelope_digest().clone(),
        contract_version: anchor.contract_version().to_owned(),
        identity: ComparatorIdentity::new(
            AGGREGATE_RUNNER.to_owned(),
            String::new(),
            String::new(),
        ),
        residuals,
        hard_gate_results,
        verdict: RecordVerdict::new(status, rationale),
    }))
}

fn fresh_merge(residual: &ResidualDoc, fail_intent: bool) -> MergedResidual {
    MergedResidual {
        id: residual.id().as_str().to_owned(),
        dimension: residual.dimension().to_owned(),
        expected: residual.expected().to_owned(),
        observed: residual.observed().to_owned(),
        unit_refs: residual
            .unit_refs()
            .iter()
            .map(|unit| unit.as_str().to_owned())
            .collect(),
        evidence_refs: residual
            .evidence_refs()
            .iter()
            .map(|e| e.as_str().to_owned())
            .collect(),
        severities: BTreeSet::from([severity_rank(residual.severity())]),
        highest_severity: residual.severity(),
        confidence: residual.confidence(),
        saw_fail_intent: fail_intent,
        saw_nonfail_intent: !fail_intent,
        inherited_dispute: residual.disputed(),
        diagnosis: residual.diagnosis().to_owned(),
        diagnosis_severity: residual.severity(),
        cheapest_discriminating_test: residual.cheapest_discriminating_test().to_owned(),
        test_severity: residual.severity(),
        candidate_corrections: residual.candidate_corrections().iter().cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AggregateError, aggregate};
    use gauntlet_domain::{
        Digest, EvidenceRef, IterationNo, ResidualId, RunId, Severity, UnitId, Verdict,
    };
    use gauntlet_ports::comparison_record::{
        ComparatorIdentity, ComparisonRecordDoc, HardGateResult, RecordContent, RecordVerdict,
        ResidualDoc,
    };

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn digest(hex: &str) -> Digest {
        Digest::new("sha256", hex).expect("valid digest parts")
    }

    fn residual_builder(
        id: &str,
        dimension: &str,
        expected: &str,
        observed: &str,
        severity: Severity,
    ) -> ResidualDoc {
        ResidualDoc::new(
            ResidualId::new(id).expect("valid id"),
            dimension.to_owned(),
            vec![],
            expected.to_owned(),
            observed.to_owned(),
            vec![EvidenceRef::new("evd-agg-0001").expect("valid evidence ref")],
            severity,
            0.5,
            false,
            String::new(),
            String::new(),
            vec![],
        )
    }

    /// A residual whose evidence ref is distinct, to prove unions.
    fn residual_with_ref(residual: &ResidualDoc, ref_text: &str) -> ResidualDoc {
        ResidualDoc::new(
            residual.id().clone(),
            residual.dimension().to_owned(),
            residual.unit_refs().to_vec(),
            residual.expected().to_owned(),
            residual.observed().to_owned(),
            vec![EvidenceRef::new(ref_text).expect("valid evidence ref")],
            residual.severity(),
            residual.confidence(),
            residual.disputed(),
            residual.diagnosis().to_owned(),
            residual.cheapest_discriminating_test().to_owned(),
            residual.candidate_corrections().to_vec(),
        )
    }

    fn doc(
        identity_runner: &str,
        residuals: Vec<ResidualDoc>,
        verdict: Verdict,
    ) -> ComparisonRecordDoc {
        ComparisonRecordDoc::new(RecordContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(4).expect("iterations are 1-based"),
            reference_digest: digest(&"11".repeat(32)),
            observation_digest: digest(&"22".repeat(32)),
            envelope_digest: digest(&"33".repeat(32)),
            contract_version: "1".to_owned(),
            identity: ComparatorIdentity::new(
                identity_runner.to_owned(),
                String::new(),
                "functional behavior".to_owned(),
            ),
            residuals,
            hard_gate_results: Vec::new(),
            verdict: RecordVerdict::new(verdict, String::new()),
        })
    }

    /// AK-032's named test: the same inputs in ANY order aggregate to
    /// byte-identical canonical bytes (spec 13.5 "deterministic and
    /// total"), and the merged shape keeps the highest severity, unions
    /// evidence, preserves disputes, and lets blocking force fail.
    #[test]
    fn preserves_an_already_disputed_residual() {
        let mut residual = residual_builder(
            "residual-disputed-0001",
            "functional behavior",
            "expected",
            "evd-observed-locator-0001",
            Severity::Major,
        );
        // A prior aggregation's dispute is consequential evidence; a later
        // aggregation must never erase it merely because it has one input.
        residual = ResidualDoc::new(
            residual.id().clone(),
            residual.dimension().to_owned(),
            residual.unit_refs().to_vec(),
            residual.expected().to_owned(),
            residual.observed().to_owned(),
            residual.evidence_refs().to_vec(),
            residual.severity(),
            residual.confidence(),
            true,
            residual.diagnosis().to_owned(),
            residual.cheapest_discriminating_test().to_owned(),
            residual.candidate_corrections().to_vec(),
        );
        let aggregate = aggregate(&[doc(
            "prior-aggregate",
            vec![residual],
            Verdict::Inconclusive,
        )])
        .expect("a single valid record aggregates");
        assert!(aggregate.residuals()[0].disputed());
        assert_eq!(aggregate.verdict().status(), Verdict::Inconclusive);
    }

    #[test]
    fn deterministic_aggregation() {
        let inputs = vec![
            doc(
                "critic-a",
                vec![
                    residual_builder(
                        "res-000001",
                        "functional behavior",
                        "tests pass",
                        "facet:test exit 1",
                        Severity::Blocking,
                    ),
                    residual_with_ref(
                        &residual_builder(
                            "res-000002",
                            "evidence completeness",
                            "manifest complete",
                            "one file missing",
                            Severity::Major,
                        ),
                        "evd-agg-0001",
                    ),
                ],
                Verdict::Fail,
            ),
            doc(
                "critic-b",
                vec![
                    // Same key as res-000002, DIFFERENT severity: disputed.
                    residual_with_ref(
                        &residual_builder(
                            "res-000003",
                            "evidence completeness",
                            "manifest complete",
                            "one file missing",
                            Severity::Minor,
                        ),
                        "evd-agg-0002",
                    ),
                    // A key only this critic found: a residual, not a dispute.
                    residual_builder(
                        "res-000004",
                        "correctness",
                        "edge case handled",
                        "panics on empty",
                        Severity::Minor,
                    ),
                ],
                Verdict::Fail,
            ),
            doc("critic-c", vec![], Verdict::Pass),
        ];
        let baseline = aggregate(&inputs).expect("the merge is total over agreeing inputs");
        let baseline_bytes = baseline.to_json();
        // Every permutation of three inputs (all six).
        let mut orders: Vec<Vec<&ComparisonRecordDoc>> = Vec::new();
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    if a != b && b != c && a != c {
                        orders.push(vec![&inputs[a], &inputs[b], &inputs[c]]);
                    }
                }
            }
        }
        assert_eq!(orders.len(), 6);
        for order in orders {
            let owned: Vec<ComparisonRecordDoc> = order.into_iter().cloned().collect();
            let merged = aggregate(&owned).expect("every order merges");
            assert_eq!(
                merged.to_json(),
                baseline_bytes,
                "input order must never change the aggregate bytes"
            );
        }
        // The merged shape is the 13.5 shape.
        assert_eq!(baseline.verdict().status(), Verdict::Fail);
        let manifest = baseline
            .residuals()
            .iter()
            .find(|residual| residual.observed() == "one file missing")
            .expect("the duplicate key merged");
        assert_eq!(
            manifest.severity(),
            Severity::Major,
            "highest severity wins"
        );
        assert!(
            manifest.disputed(),
            "the severity disagreement is preserved"
        );
        assert_eq!(
            manifest.evidence_refs().len(),
            2,
            "the union of both contributors' evidence refs"
        );
        // The single-contributor keys are NOT disputed.
        let only_b = baseline
            .residuals()
            .iter()
            .find(|residual| residual.observed() == "panics on empty")
            .expect("the unique key survives");
        assert!(!only_b.disputed());
        assert_eq!(baseline.residuals().len(), 3);
        assert_eq!(
            baseline.identity().runner(),
            "gauntlet-app::compare::aggregate"
        );
        assert_eq!(
            baseline.identity().model(),
            "",
            "no model in the merge path"
        );
        // Round-trips through the strict codec.
        assert_eq!(
            ComparisonRecordDoc::decode(&baseline_bytes).expect("canonical bytes decode"),
            baseline
        );
    }

    #[test]
    fn any_blocking_residual_forces_fail_regardless_of_everything_else() {
        // A disputed blocking key: the verdict is still fail (13.5:
        // "regardless"), and the dispute is preserved for
        // verification/diagnosis routing, never voted away.
        let blocking = residual_builder(
            "res-000001",
            "functional behavior",
            "tests pass",
            "facet:test exit 1",
            Severity::Blocking,
        );
        let lighter = residual_builder(
            "res-000002",
            "functional behavior",
            "tests pass",
            "facet:test exit 1",
            Severity::Informational,
        );
        let merged = aggregate(&[
            doc("critic-a", vec![blocking], Verdict::Fail),
            doc("critic-b", vec![lighter], Verdict::Pass),
        ])
        .expect("merges");
        assert_eq!(merged.verdict().status(), Verdict::Fail);
        let residual = &merged.residuals()[0];
        assert_eq!(residual.severity(), Severity::Blocking);
        assert!(
            residual.disputed(),
            "the dispute is preserved on the blocking key"
        );
        assert!(merged.verdict().rationale().contains("1 disputed blocking"));
    }

    #[test]
    fn verdict_intents_flow_deterministically() {
        // Non-blocking undisputed fail intents => fail.
        let merged = aggregate(&[
            doc(
                "critic-a",
                vec![residual_builder(
                    "res-000001",
                    "maintainability",
                    "small fns",
                    "one 400-line fn",
                    Severity::Minor,
                )],
                Verdict::Fail,
            ),
            doc("critic-b", vec![], Verdict::Pass),
        ])
        .expect("merges");
        assert_eq!(merged.verdict().status(), Verdict::Fail);
        // Any inconclusive input => inconclusive (missing evidence never
        // passes).
        let merged = aggregate(&[
            doc("critic-a", vec![], Verdict::Inconclusive),
            doc("critic-b", vec![], Verdict::Pass),
        ])
        .expect("merges");
        assert_eq!(merged.verdict().status(), Verdict::Inconclusive);
        // All pass, nothing found => pass.
        let merged = aggregate(&[
            doc("critic-a", vec![], Verdict::Pass),
            doc("critic-b", vec![], Verdict::Pass),
        ])
        .expect("merges");
        assert_eq!(merged.verdict().status(), Verdict::Pass);
        // Same key from fail-intent and pass-intent records: disputed even
        // at EQUAL severity (verdict-intent disagreement).
        let first = residual_builder(
            "res-000001",
            "security",
            "no unsafe",
            "one unsafe block",
            Severity::Minor,
        );
        let second = residual_builder(
            "res-000002",
            "security",
            "no unsafe",
            "one unsafe block",
            Severity::Minor,
        );
        let merged = aggregate(&[
            doc("critic-a", vec![first], Verdict::Fail),
            doc("critic-b", vec![second], Verdict::Pass),
        ])
        .expect("merges");
        assert!(merged.residuals()[0].disputed());
        assert_eq!(merged.verdict().status(), Verdict::Inconclusive);
    }

    #[test]
    fn hard_gates_hold_only_when_every_contributor_holds() {
        let with_gate = |held: bool, ref_text: &str| {
            ComparisonRecordDoc::new(RecordContent {
                run_id: RunId::new(ULID).expect("valid run id"),
                iteration_no: IterationNo::new(4).expect("iterations are 1-based"),
                reference_digest: digest(&"11".repeat(32)),
                observation_digest: digest(&"22".repeat(32)),
                envelope_digest: digest(&"33".repeat(32)),
                contract_version: "1".to_owned(),
                identity: ComparatorIdentity::new(
                    "critic".to_owned(),
                    String::new(),
                    String::new(),
                ),
                residuals: Vec::new(),
                hard_gate_results: vec![HardGateResult::new(
                    UnitId::new("AK-0014").expect("valid unit id"),
                    held,
                    vec![EvidenceRef::new(ref_text).expect("valid evidence ref")],
                )],
                verdict: RecordVerdict::new(Verdict::Pass, String::new()),
            })
        };
        let merged = aggregate(&[
            with_gate(true, "evd-agg-0001"),
            with_gate(false, "evd-agg-0002"),
        ])
        .expect("merges");
        assert_eq!(merged.hard_gate_results().len(), 1);
        assert!(
            !merged.hard_gate_results()[0].held(),
            "one broken gate breaks the merge"
        );
        assert_eq!(merged.hard_gate_results()[0].evidence_refs().len(), 2);
    }

    #[test]
    fn omitted_hard_gate_is_not_held() {
        // I8: an omitted contributor result is missing evidence, never tacit
        // agreement with another critic's held gate.
        let held = ComparisonRecordDoc::new(RecordContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(4).expect("iterations are 1-based"),
            reference_digest: digest(&"11".repeat(32)),
            observation_digest: digest(&"22".repeat(32)),
            envelope_digest: digest(&"33".repeat(32)),
            contract_version: "1".to_owned(),
            identity: ComparatorIdentity::new("critic-a".to_owned(), String::new(), String::new()),
            residuals: Vec::new(),
            hard_gate_results: vec![HardGateResult::new(
                UnitId::new("AK-0014").expect("valid unit id"),
                true,
                vec![EvidenceRef::new("evd-agg-0001").expect("valid evidence ref")],
            )],
            verdict: RecordVerdict::new(Verdict::Pass, String::new()),
        });
        let omitted = ComparisonRecordDoc::new(RecordContent {
            identity: ComparatorIdentity::new("critic-b".to_owned(), String::new(), String::new()),
            hard_gate_results: Vec::new(),
            ..held.content()
        });
        let merged = aggregate(&[held, omitted]).expect("merges");
        assert!(!merged.hard_gate_results()[0].held());
        assert_eq!(
            merged.verdict().status(),
            Verdict::Inconclusive,
            "an omitted gate contributor is unavailable evidence, never a pass"
        );
    }

    #[test]
    fn empty_and_disagreeing_inputs_are_typed_errors() {
        assert_eq!(aggregate(&[]), Err(AggregateError::InputsEmpty));
        let mut foreign = doc("critic-b", vec![], Verdict::Pass);
        let mut content = foreign.content();
        content.envelope_digest = digest(&"44".repeat(32));
        foreign = ComparisonRecordDoc::new(content);
        assert!(matches!(
            aggregate(&[doc("critic-a", vec![], Verdict::Pass), foreign]),
            Err(AggregateError::InputsDisagree {
                field: "envelope_digest",
                ..
            })
        ));
    }
}
