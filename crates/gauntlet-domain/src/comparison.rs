//! `ComparisonRecord` and `Residual` — spec §5.3.6.
//!
//! Spec §5.3.6 domain invariant: "`ComparisonRecord` MUST carry
//! `reference_digest` and `observation_digest`; a record whose
//! `reference_digest` differs from the run's frozen digest invalidates the
//! iteration (I1)."
//!
//! Spec §13.2 adds the third digest: "The envelope is serialized, digested,
//! and the digest is stored on both the `RoleInvocation` and the
//! `ComparisonRecord`." Annex B.4's example JSON does not spell
//! `envelope_digest` — the same reconciliation class as Task 9's
//! environment-fingerprint-in-evidence-doc: the annex example is a MINIMUM
//! shape, and the normative body section's MUST wins. `envelope_digest` is
//! therefore a required constructor argument alongside the other two.
//!
//! All three digests are required constructor arguments — a record without
//! them does not exist. Cross-checking against the run's frozen digest
//! happens where the run is knowable (plan Task 10/13); the domain
//! guarantees the record always carries all three.

use core::fmt;

use crate::values::{Digest, ResidualId, Severity, Verdict};

/// `ComparisonRecord` invariant failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonError {
    /// Spec §13.5: "Any `blocking` residual makes the aggregate verdict
    /// `fail` regardless of other dimensions and regardless of weighted
    /// scores." A record whose residual set contradicts its verdict cannot
    /// exist.
    BlockingResidualRequiresFailVerdict { blocking: usize },
}

impl fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockingResidualRequiresFailVerdict { blocking } => write!(
                f,
                "{blocking} blocking residual(s) force a fail verdict; a pass or inconclusive \
                 verdict alongside a blocking residual is not a constructible comparison record \
                 (spec 13.5)"
            ),
        }
    }
}

impl std::error::Error for ComparisonError {}

/// One difference the comparator found between observation and reference
/// (spec §5.1 vocabulary: the Comparison context communicates via "residual
/// set"). Comparators produce residuals only — a comparator verdict alone
/// never settles a Unit (AK-031; enforced at settlement, spec §16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residual {
    id: ResidualId,
    severity: Severity,
    detail: String,
    dimension: Option<String>,
}

impl Residual {
    pub fn new(
        id: ResidualId,
        severity: Severity,
        detail: String,
        dimension: Option<String>,
    ) -> Self {
        Self {
            id,
            severity,
            detail,
            dimension,
        }
    }

    pub fn id(&self) -> &ResidualId {
        &self.id
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// The quality dimension this residual belongs to, when applicable.
    pub fn dimension(&self) -> Option<&str> {
        self.dimension.as_deref()
    }

    /// Spec §13.5: a blocking residual forces an aggregate fail verdict.
    pub fn is_blocking(&self) -> bool {
        self.severity == Severity::Blocking
    }
}

/// What a fresh-context comparison found (spec §5.3.6, §13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonRecord {
    reference_digest: Digest,
    observation_digest: Digest,
    envelope_digest: Digest,
    residuals: Vec<Residual>,
    verdict: Verdict,
}

impl ComparisonRecord {
    /// Records a comparison outcome. `reference_digest` is what was
    /// compared against; the caller must have checked it equals the run's
    /// frozen digest (I1) — carrying it here is what makes that check
    /// possible at every later boundary. `envelope_digest` is the digest of
    /// the serialized comparison envelope the comparison ran under (spec
    /// §13.2: stored on both the `RoleInvocation` and this record).
    ///
    /// Spec §13.5: "Any `blocking` residual makes the aggregate verdict
    /// `fail` regardless of other dimensions and regardless of weighted
    /// scores." Enforced here, at construction: a verdict of `pass` (or
    /// `inconclusive`) alongside a blocking residual is a typed error, so
    /// the residual set and the verdict can never disagree in a record that
    /// exists.
    pub fn new(
        reference_digest: Digest,
        observation_digest: Digest,
        envelope_digest: Digest,
        residuals: Vec<Residual>,
        verdict: Verdict,
    ) -> Result<Self, ComparisonError> {
        let blocking = residuals.iter().filter(|r| r.is_blocking()).count();
        if blocking > 0 && verdict != Verdict::Fail {
            return Err(ComparisonError::BlockingResidualRequiresFailVerdict { blocking });
        }
        Ok(Self {
            reference_digest,
            observation_digest,
            envelope_digest,
            residuals,
            verdict,
        })
    }

    pub fn reference_digest(&self) -> &Digest {
        &self.reference_digest
    }

    pub fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }

    /// The digest of the serialized envelope this comparison ran under
    /// (spec §13.2).
    pub fn envelope_digest(&self) -> &Digest {
        &self.envelope_digest
    }

    pub fn residuals(&self) -> &[Residual] {
        &self.residuals
    }

    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// The blocking subset (spec §13.5: "any blocking residual forces an
    /// aggregate fail verdict").
    pub fn blocking_residuals(&self) -> impl Iterator<Item = &Residual> {
        self.residuals.iter().filter(|r| r.is_blocking())
    }
}

impl fmt::Display for ComparisonRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "comparison {} against reference {}: {} ({} residuals)",
            self.observation_digest,
            self.reference_digest,
            self.verdict,
            self.residuals.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::Digest;

    fn reference_digest() -> Digest {
        Digest::new("sha256", "a1b2c3d4").expect("valid parts")
    }

    fn observation_digest() -> Digest {
        Digest::new("sha256", "e5f6a7b8").expect("valid parts")
    }

    fn envelope_digest() -> Digest {
        Digest::new("sha256", "c3d4e5f6").expect("valid parts")
    }

    #[test]
    fn a_comparison_record_carries_all_three_digests() {
        // Spec §5.3.6: "`ComparisonRecord` MUST carry `reference_digest` and
        // `observation_digest`; a record whose `reference_digest` differs
        // from the run's frozen digest invalidates the iteration (I1)."
        // Spec §13.2: the envelope digest "is stored on both the
        // RoleInvocation and the ComparisonRecord".
        let record = ComparisonRecord::new(
            reference_digest(),
            observation_digest(),
            envelope_digest(),
            Vec::new(),
            Verdict::Pass,
        )
        .expect("no residuals, no contradiction");
        assert_eq!(record.reference_digest(), &reference_digest());
        assert_eq!(record.observation_digest(), &observation_digest());
        assert_eq!(record.envelope_digest(), &envelope_digest());
        // Carrying the digests is what makes the later I1/§13.2 checks
        // possible at every boundary; distinct digests stay distinct.
        assert_ne!(record.reference_digest(), record.observation_digest());
        assert_ne!(record.envelope_digest(), record.reference_digest());
        assert_ne!(record.envelope_digest(), record.observation_digest());
    }

    #[test]
    fn a_blocking_residual_forces_a_fail_verdict_at_construction() {
        // Spec §13.5: "Any `blocking` residual makes the aggregate verdict
        // `fail` regardless of other dimensions and regardless of weighted
        // scores." The rule is enforced by the constructor, not asserted
        // back from a fixture: a record whose verdict contradicts its
        // residual set is a typed error and never exists.
        let residual = |id: &str, severity| {
            Residual::new(
                ResidualId::new(id).expect("valid id"),
                severity,
                "a difference".to_owned(),
                None,
            )
        };
        // Pass + blocking: refused.
        assert!(matches!(
            ComparisonRecord::new(
                reference_digest(),
                observation_digest(),
                envelope_digest(),
                vec![residual("res-000001", Severity::Blocking)],
                Verdict::Pass,
            ),
            Err(ComparisonError::BlockingResidualRequiresFailVerdict { blocking: 1 })
        ));
        // Inconclusive + blocking: refused — blocking leaves no room.
        assert!(matches!(
            ComparisonRecord::new(
                reference_digest(),
                observation_digest(),
                envelope_digest(),
                vec![residual("res-000001", Severity::Blocking)],
                Verdict::Inconclusive,
            ),
            Err(ComparisonError::BlockingResidualRequiresFailVerdict { blocking: 1 })
        ));
        // Fail + blocking: the legal pairing, and the blocking subset stays
        // selectable (spec §13.5 residual set semantics).
        let record = ComparisonRecord::new(
            reference_digest(),
            observation_digest(),
            envelope_digest(),
            vec![
                residual("res-000001", Severity::Blocking),
                residual("res-000002", Severity::Minor),
                residual("res-000003", Severity::Blocking),
            ],
            Verdict::Fail,
        )
        .expect("fail with blocking residuals is the consistent shape");
        assert_eq!(record.verdict(), Verdict::Fail);
        let blocking: Vec<&Residual> = record.blocking_residuals().collect();
        assert_eq!(blocking.len(), 2);
        assert!(blocking.iter().all(|r| r.is_blocking()));
        assert!(record.residuals().iter().any(Residual::is_blocking));
        // Pass + non-blocking residuals only: legal.
        let record = ComparisonRecord::new(
            reference_digest(),
            observation_digest(),
            envelope_digest(),
            vec![
                residual("res-000004", Severity::Minor),
                residual("res-000005", Severity::Informational),
            ],
            Verdict::Pass,
        )
        .expect("non-blocking residuals do not force a fail");
        assert_eq!(record.verdict(), Verdict::Pass);
        assert_eq!(record.blocking_residuals().count(), 0);
    }
}
