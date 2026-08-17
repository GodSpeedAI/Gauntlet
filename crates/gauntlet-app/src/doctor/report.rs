//! Doctor report — the §8.5 machine-readable report and human summary.
//!
//! Spec §8.5: "Output is a machine-readable report at
//! `$GAUNTLET_REPORT_DIR/doctor.json` plus a human summary. Exit non-zero on
//! any REQUIRED failure." Statuses are FOUR-valued (plan Task 5 decision D5):
//! `pass`, `fail`, `unavailable` — the machinery lands in a later task and
//! honesty (I8) forbids silently passing — and `skipped` — a precondition is
//! false. All eight checks always appear, each with evidence. `unavailable`
//! never blocks and never counts as a pass in prose. The report carries
//! `generated_at` for humans; the report itself is not digested this task,
//! and the field stays outside any digested content when it is.

use core::fmt;

use gauntlet_domain::Digest;

use crate::config::canonical::{CanonValue, write_canonical};

/// One check's four-valued status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Fail,
    Unavailable { reason: String },
    Skipped { reason: String },
}

impl CheckStatus {
    /// The lowercase wire token.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unavailable { .. } => "unavailable",
            Self::Skipped { .. } => "skipped",
        }
    }

    /// The reason carried by a non-pass/non-fail status, for serialization.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable { reason } | Self::Skipped { reason } => Some(reason),
            Self::Pass | Self::Fail => None,
        }
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One sub-check: a named clause inside a check, with its own status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubResult {
    pub name: String,
    pub status: CheckStatus,
    pub evidence: Vec<String>,
}

/// One of the eight §8.5 checks: overall status, evidence, sub-clauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    pub id: u8,
    pub name: &'static str,
    pub status: CheckStatus,
    pub evidence: Vec<String>,
    pub sub: Vec<SubResult>,
}

impl CheckOutcome {
    /// Combines a check's own status with its sub-clauses: a failure
    /// anywhere fails the check; otherwise an unavailable clause makes the
    /// check unavailable (never silently pass); otherwise any pass makes it
    /// pass; a check whose every clause was skipped is skipped.
    ///
    /// Rule for reading a check-level `pass`: it relies ONLY on the
    /// sub-clauses that themselves `pass`. A `skipped` sub-clause under a
    /// passing check (e.g. check 5's tier-existence clause while the sandbox
    /// backend is the conservative default) carries its recorded reason and
    /// is never relied upon for the pass — the pass rests on the clauses
    /// that genuinely ran.
    pub fn combine(
        id: u8,
        name: &'static str,
        own: CheckStatus,
        sub: Vec<SubResult>,
        evidence: Vec<String>,
    ) -> Self {
        let mut statuses: Vec<CheckStatus> =
            sub.iter().map(|clause| clause.status.clone()).collect();
        statuses.push(own);
        let combined = if statuses.contains(&CheckStatus::Fail) {
            CheckStatus::Fail
        } else if statuses
            .iter()
            .any(|status| matches!(status, CheckStatus::Unavailable { .. }))
        {
            CheckStatus::Unavailable {
                reason: "a sub-check's machinery lands with a later plan task".to_owned(),
            }
        } else if statuses.contains(&CheckStatus::Pass) {
            CheckStatus::Pass
        } else {
            CheckStatus::Skipped {
                reason: "every clause's precondition was false".to_owned(),
            }
        };
        Self {
            id,
            name,
            status: combined,
            evidence,
            sub,
        }
    }
}

/// The whole preflight report.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub checks: Vec<CheckOutcome>,
    pub config_fingerprint: Digest,
    /// Unix seconds, for humans; never part of digested content.
    pub generated_at: u64,
    /// Where the config contract was found, when it was.
    pub config_origin: Option<String>,
}

impl DoctorReport {
    /// The first REQUIRED check that failed, when any did.
    pub fn required_failure(&self) -> Option<&CheckOutcome> {
        self.checks
            .iter()
            .find(|check| check.status == CheckStatus::Fail)
    }

    /// Whether the run may proceed: no required check failed.
    pub fn exit_ok(&self) -> bool {
        self.required_failure().is_none()
    }

    /// The report as a canonical tree: sorted keys, the same writer as the
    /// effective configuration. Evidence and reasons are strings already
    /// vetted to carry no secret values.
    pub fn to_canonical_value(&self) -> CanonValue {
        let mut checks = Vec::new();
        for check in &self.checks {
            let mut entry = std::collections::BTreeMap::new();
            entry.insert(
                "evidence".to_owned(),
                CanonValue::Seq(
                    check
                        .evidence
                        .iter()
                        .map(|line| CanonValue::Str(line.clone()))
                        .collect(),
                ),
            );
            entry.insert("id".to_owned(), CanonValue::Uint(u64::from(check.id)));
            entry.insert("name".to_owned(), CanonValue::Str(check.name.to_owned()));
            if let Some(reason) = check.status.reason() {
                entry.insert("reason".to_owned(), CanonValue::Str(reason.to_owned()));
            }
            entry.insert(
                "status".to_owned(),
                CanonValue::Str(check.status.as_str().to_owned()),
            );
            let mut subs = Vec::new();
            for clause in &check.sub {
                let mut sub = std::collections::BTreeMap::new();
                sub.insert(
                    "evidence".to_owned(),
                    CanonValue::Seq(
                        clause
                            .evidence
                            .iter()
                            .map(|line| CanonValue::Str(line.clone()))
                            .collect(),
                    ),
                );
                sub.insert("name".to_owned(), CanonValue::Str(clause.name.clone()));
                if let Some(reason) = clause.status.reason() {
                    sub.insert("reason".to_owned(), CanonValue::Str(reason.to_owned()));
                }
                sub.insert(
                    "status".to_owned(),
                    CanonValue::Str(clause.status.as_str().to_owned()),
                );
                subs.push(CanonValue::Object(sub));
            }
            entry.insert("sub".to_owned(), CanonValue::Seq(subs));
            checks.push(CanonValue::Object(entry));
        }
        let mut root = std::collections::BTreeMap::new();
        root.insert("checks".to_owned(), CanonValue::Seq(checks));
        root.insert(
            "config_fingerprint".to_owned(),
            CanonValue::Str(self.config_fingerprint.to_string()),
        );
        root.insert(
            "config_origin".to_owned(),
            self.config_origin
                .clone()
                .map_or(CanonValue::Null, CanonValue::Str),
        );
        root.insert(
            "exit_code".to_owned(),
            CanonValue::Uint(if self.exit_ok() { 0 } else { 1 }),
        );
        root.insert(
            "generated_at".to_owned(),
            CanonValue::Uint(self.generated_at),
        );
        CanonValue::Object(root)
    }

    /// The canonical JSON bytes of the report.
    pub fn to_json(&self) -> String {
        write_canonical(&self.to_canonical_value())
    }

    /// The human summary: one line per check with status, sub-clauses
    /// indented, evidence further indented. Secret values never reach this
    /// text — evidence strings carry variable NAMES only.
    pub fn human_summary(&self) -> String {
        let mut out = String::from("gauntlet doctor (spec 8.5 preflight)\n");
        for check in &self.checks {
            out.push_str(&format!(
                "  {}. {:<24} {}\n",
                check.id, check.name, check.status
            ));
            for line in &check.evidence {
                out.push_str(&format!("      {line}\n"));
            }
            for clause in &check.sub {
                out.push_str(&format!("      - {:<20} {}\n", clause.name, clause.status));
                for line in &clause.evidence {
                    out.push_str(&format!("          {line}\n"));
                }
            }
        }
        match self.required_failure() {
            Some(failed) => out.push_str(&format!(
                "exit: FAIL (check {}: {})\n",
                failed.id, failed.name
            )),
            None => out.push_str("exit: OK\n"),
        }
        out
    }
}
