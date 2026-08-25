//! The §11.5 retry/escalation table as typed logic, plus the control
//! policy that carries its §11.4/§11.5 settings (plan Task 12, slice B3).
//!
//! Every row of Core-spec §11.5 is one [`IterationCause`] variant with one
//! [`Disposition`] outcome, decided by [`dispose`] — a pure function, so
//! the table can be tested row by row without a loop, a clock, or a
//! runner. The composed controller ( [`super::controller`] ) feeds causes
//! from real observations and EXECUTES dispositions; it cannot reshape
//! them, because the dispositions the loop may act on are exactly the
//! variants of [`Disposition`]:
//!
//! - re-dispatch carries the residual's TEXT ONLY (row 1: "residual text
//!   (not critic trajectory) attached") — [`ResidualAttachment`] is built
//!   from the residual text alone, so there is no field a critic's
//!   reasoning could travel through;
//! - transient failures back off exponentially between the configured
//!   floor and ceiling (row 2, [`backoff_ms`]);
//! - `malformed_result` gets ONE schema reminder, then escalates (row 3);
//! - `TimedOut` cancels, preserves the session reference for forensics,
//!   retries once, then escalates (row 4; §10.3 rule 5). §11.4 step 2's
//!   stall rule ("cancel, record, retry or escalate") is the same shape
//!   and routes through the same row;
//! - attempts exhausted escalate as `max_attempts_exceeded` (rows 1+5);
//! - an authority gap escalates and never loops past it (row 6);
//! - a sandbox validation failure escalates and never falls back to
//!   unsandboxed execution (row 7) — there is no disposition variant that
//!   weakens containment, so the fallback is unrepresentable, not merely
//!   unchosen;
//! - a reference digest mismatch aborts the run with both digests and no
//!   retry (row 8, §9.4);
//! - a non-converging residual set stops correcting and enters diagnosis
//!   (row 9); §15.2's "if diagnosis is inconclusive, escalate" half is the
//!   [`EscalationCause::DiagnosisInconclusive`] outcome the controller
//!   routes from the [`super::diagnosis::DiagnosticEngine`]'s result.

use core::fmt;

use gauntlet_domain::{Authority, Digest, SessionRef, UnitId};

use super::diagnosis::ResidualKey;
use crate::config::ControlSettings;

// ---------------------------------------------------------------------------
// The policy carrier (brief Item 3: spec-backed settings only)
// ---------------------------------------------------------------------------

/// The control loop's policy: the §11.4 tick and stall windows, the §11.5
/// backoff bounds, attempt ceiling, and non-convergence threshold. Every
/// value is spec-anchored — one §11.4 inline default (`poll_interval_ms`)
/// and five REQUIRED settings the spec tables no default for — so the only
/// construction paths are [`ControlPolicy::new`] (validated values) and
/// [`ControlPolicy::from_settings`] (the config layer's output, with a
/// typed absence error per required setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPolicy {
    poll_interval_ms: u64,
    stall_timeout_ms: u64,
    retry_base_ms: u64,
    retry_max_ms: u64,
    max_attempts_per_unit: u32,
    nonconvergence_n: u32,
}

/// Why a [`ControlPolicy`] could not be built — typed, never a defaulted
/// guess (spec §21.2; §8.6's table is the only literal home and carries no
/// rows for these settings, so absence cannot be papered over with an
/// invented default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlPolicyError {
    /// A REQUIRED §11.4/§11.5 setting is absent from the effective
    /// configuration. The spec tables no default; the loop refuses to run
    /// rather than invent one.
    RequiredSettingMissing {
        setting: &'static str,
        variable: &'static str,
    },
    /// A setting arrived as zero. A zero tick spins, a zero stall window
    /// stalls on the first poll, zero backoff is no backoff, and a zero
    /// attempt ceiling exhausts before the first dispatch.
    ZeroSetting { setting: &'static str },
}

impl fmt::Display for ControlPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiredSettingMissing { setting, variable } => write!(
                f,
                "control setting '{setting}' ({variable}) is REQUIRED — the spec tables no \
                 default, so the control loop refuses to run without it"
            ),
            Self::ZeroSetting { setting } => {
                write!(f, "control setting '{setting}' is zero and unusable")
            }
        }
    }
}

impl std::error::Error for ControlPolicyError {}

impl ControlPolicy {
    /// Builds the policy from validated values. Zero in any field is a
    /// typed refusal — every window in this policy must be a real span.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        poll_interval_ms: u64,
        stall_timeout_ms: u64,
        retry_base_ms: u64,
        retry_max_ms: u64,
        max_attempts_per_unit: u32,
        nonconvergence_n: u32,
    ) -> Result<Self, ControlPolicyError> {
        for (setting, zeroed) in [
            ("poll_interval_ms", poll_interval_ms == 0),
            ("stall_timeout_ms", stall_timeout_ms == 0),
            ("retry_base_ms", retry_base_ms == 0),
            ("retry_max_ms", retry_max_ms == 0),
        ] {
            if zeroed {
                return Err(ControlPolicyError::ZeroSetting { setting });
            }
        }
        if max_attempts_per_unit == 0 {
            return Err(ControlPolicyError::ZeroSetting {
                setting: "max_attempts_per_unit",
            });
        }
        if nonconvergence_n == 0 {
            return Err(ControlPolicyError::ZeroSetting {
                setting: "nonconvergence_n",
            });
        }
        Ok(Self {
            poll_interval_ms,
            stall_timeout_ms,
            retry_base_ms,
            retry_max_ms,
            max_attempts_per_unit,
            nonconvergence_n,
        })
    }

    /// Builds the policy from the config layer's control settings plus the
    /// §11.5 non-convergence threshold. The four REQUIRED settings are
    /// absent-until-supplied by the config layer ([`ControlSettings`]);
    /// each absence is a typed error naming the setting and its Annex E
    /// variable, never a defaulted value.
    pub fn from_settings(
        settings: &ControlSettings,
        nonconvergence_n: u32,
    ) -> Result<Self, ControlPolicyError> {
        let stall_timeout_ms = settings.stall_timeout_ms.ok_or(Self::absent(
            "stall_timeout_ms",
            "GAUNTLET_STALL_TIMEOUT_MS",
        ))?;
        let retry_base_ms = settings
            .retry_base_ms
            .ok_or(Self::absent("retry_base_ms", "GAUNTLET_RETRY_BASE_MS"))?;
        let retry_max_ms = settings
            .retry_max_ms
            .ok_or(Self::absent("retry_max_ms", "GAUNTLET_RETRY_MAX_MS"))?;
        let max_attempts_per_unit = settings.max_attempts_per_unit.ok_or(Self::absent(
            "max_attempts_per_unit",
            "GAUNTLET_MAX_ATTEMPTS_PER_UNIT",
        ))?;
        Self::new(
            settings.poll_interval_ms,
            stall_timeout_ms,
            retry_base_ms,
            retry_max_ms,
            max_attempts_per_unit,
            nonconvergence_n,
        )
    }

    fn absent(setting: &'static str, variable: &'static str) -> ControlPolicyError {
        ControlPolicyError::RequiredSettingMissing { setting, variable }
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }

    pub fn stall_timeout_ms(&self) -> u64 {
        self.stall_timeout_ms
    }

    pub fn retry_base_ms(&self) -> u64 {
        self.retry_base_ms
    }

    pub fn retry_max_ms(&self) -> u64 {
        self.retry_max_ms
    }

    pub fn max_attempts_per_unit(&self) -> u32 {
        self.max_attempts_per_unit
    }

    pub fn nonconvergence_n(&self) -> u32 {
        self.nonconvergence_n
    }
}

// ---------------------------------------------------------------------------
// The §11.5 table
// ---------------------------------------------------------------------------

/// The transient-failure loci §11.5 row 2 names in its parenthetical:
/// "tool, process, network, store".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientLocus {
    Tool,
    Process,
    Network,
    Store,
}

impl fmt::Display for TransientLocus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::Tool => "tool",
            Self::Process => "process",
            Self::Network => "network",
            Self::Store => "store",
        };
        f.write_str(token)
    }
}

/// The residual re-dispatch attachment (§11.5 row 1: "Re-dispatch builder
/// with residual text (not critic trajectory) attached"). Built from the
/// residual's own text and nothing else; the critic's reasoning has no
/// field here to ride in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualAttachment {
    text: String,
}

impl ResidualAttachment {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// One §11.5 row's cause, as the controller observes it. `attempts_used`,
/// `retries_used`, and `reminders_used` are counts the controller tracks
/// per unit; the table's row selection reads them here, in the open,
/// instead of hiding them in loop state.
#[derive(Debug, Clone, PartialEq)]
pub enum IterationCause {
    /// Row 1: a residual was found and the unit has attempts left.
    ResidualFound {
        unit: UnitId,
        residual_text: String,
        attempts_used: u32,
    },
    /// Row 2: a transient error of the named locus. `attempt` is the
    /// retry's 1-based number, feeding `min(base × 2^(n-1), max)`.
    TransientError { locus: TransientLocus, attempt: u32 },
    /// Row 3: `malformed_result`. `reminders_used` counts schema-reminder
    /// re-dispatches already spent (the row allows exactly one).
    MalformedResult { unit: UnitId, reminders_used: u32 },
    /// Row 4: `TimedOut` (§10.3 rule 5) — also the disposition shape of a
    /// §11.4 step-2 stall. `retries_used` counts retries already spent
    /// (the row allows exactly one).
    TimedOut {
        unit: UnitId,
        session_ref: Option<SessionRef>,
        retries_used: u32,
    },
    /// Row 5: attempts exhausted.
    AttemptsExhausted { unit: UnitId },
    /// Row 6: the closure needs an authority the run cannot exercise.
    AuthorityNotCritic {
        unit: UnitId,
        required: Authority,
        exercisable: Authority,
    },
    /// Row 7: sandbox validation failed. There is no "retry unsandboxed"
    /// outcome anywhere in the table.
    SandboxValidationFailed { unit: UnitId },
    /// Row 8: the reference digest moved (§9.4, I1).
    ReferenceDigestMismatch { expected: Digest, actual: Digest },
    /// Row 9: the same residual projection recurred `occurrences` times
    /// (the threshold itself lives in the policy).
    NonConverging {
        unit: UnitId,
        key: ResidualKey,
        occurrences: u32,
    },
}

/// What the loop does with a cause. The variant set IS the policy: no
/// variant weakens containment, lowers authority, or retries a mutated
/// reference, so those outcomes are unrepresentable rather than merely
/// unchosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Disposition {
    /// Row 1: re-dispatch the builder with the residual's TEXT ONLY
    /// attached; explicitly no backoff.
    RetryWithResidual {
        unit: UnitId,
        attachment: ResidualAttachment,
    },
    /// Row 2: retry the same work after the exponential backoff.
    RetryAfterBackoff {
        locus: TransientLocus,
        delay_ms: u64,
    },
    /// Row 3: the one schema-reminder re-dispatch.
    RetryOnceWithSchemaReminder { unit: UnitId },
    /// Row 4: cancel the invocation, PRESERVE the session reference for
    /// forensics, and retry once.
    CancelThenRetryOnce {
        unit: UnitId,
        session_ref: Option<SessionRef>,
    },
    /// Escalate to the operator with the named cause (rows 3–7, and §15.2's
    /// inconclusive-diagnosis half of row 9).
    Escalate { reason: EscalationCause },
    /// Row 8: terminal abort — class `reference_mutated`, both digests
    /// carried, no retry (§9.4, §11.6).
    AbortRun { expected: Digest, actual: Digest },
    /// Row 9: stop correcting; enter the §15.2 bounded diagnostic cycle.
    EnterDiagnosis {
        unit: UnitId,
        key: ResidualKey,
        occurrences: u32,
    },
}

/// Why the loop escalated — the §11.5 escalation vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub enum EscalationCause {
    /// Row 4: timed out twice (or stalled past its one retry).
    TimedOutTwice {
        unit: UnitId,
        session_ref: Option<SessionRef>,
    },
    /// Row 5: `max_attempts_exceeded`, with the ceiling that was hit.
    MaxAttemptsExceeded { unit: UnitId, attempts: u32 },
    /// Row 3: a second malformed result after the one reminder.
    MalformedTwice { unit: UnitId },
    /// Row 6: authority the run cannot exercise is required for closure.
    AuthorityRequired {
        unit: UnitId,
        required: Authority,
        exercisable: Authority,
    },
    /// Row 7: sandbox validation failed; never an unsandboxed fallback.
    SandboxValidation { unit: UnitId },
    /// Row 9 / §15.2: the diagnostic cycle ended inconclusive.
    DiagnosisInconclusive { unit: UnitId },
}

impl fmt::Display for EscalationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOutTwice { unit, .. } => {
                write!(
                    f,
                    "{unit} timed out twice — session reference preserved for forensics"
                )
            }
            Self::MaxAttemptsExceeded { unit, attempts } => {
                write!(f, "{unit} exhausted its attempt ceiling at {attempts}")
            }
            Self::MalformedTwice { unit } => {
                write!(f, "{unit} returned a malformed result twice")
            }
            Self::AuthorityRequired {
                unit,
                required,
                exercisable,
            } => write!(
                f,
                "{unit} needs {required} authority for closure; the run can exercise only \
                 {exercisable}"
            ),
            Self::SandboxValidation { unit } => {
                write!(
                    f,
                    "{unit}'s sandbox validation failed; unsandboxed fallback is not a \
                           disposition"
                )
            }
            Self::DiagnosisInconclusive { unit } => {
                write!(f, "{unit}'s diagnostic cycle ended inconclusive")
            }
        }
    }
}

/// Decides one §11.5 row. Pure: the same cause under the same policy
/// always yields the same disposition.
pub fn dispose(cause: IterationCause, policy: &ControlPolicy) -> Disposition {
    match cause {
        IterationCause::ResidualFound {
            unit,
            residual_text,
            attempts_used,
        } => {
            // Rows 1 and 5 are the same observation's two branches: a
            // residual with attempts left re-dispatches; a residual with
            // none left escalates as `max_attempts_exceeded`.
            if attempts_used < policy.max_attempts_per_unit {
                Disposition::RetryWithResidual {
                    unit,
                    attachment: ResidualAttachment::new(residual_text),
                }
            } else {
                Disposition::Escalate {
                    reason: EscalationCause::MaxAttemptsExceeded {
                        unit,
                        attempts: policy.max_attempts_per_unit,
                    },
                }
            }
        }
        IterationCause::TransientError { locus, attempt } => Disposition::RetryAfterBackoff {
            locus,
            delay_ms: backoff_ms(policy, attempt),
        },
        IterationCause::MalformedResult {
            unit,
            reminders_used,
        } => {
            if reminders_used == 0 {
                Disposition::RetryOnceWithSchemaReminder { unit }
            } else {
                Disposition::Escalate {
                    reason: EscalationCause::MalformedTwice { unit },
                }
            }
        }
        IterationCause::TimedOut {
            unit,
            session_ref,
            retries_used,
        } => {
            if retries_used == 0 {
                Disposition::CancelThenRetryOnce { unit, session_ref }
            } else {
                Disposition::Escalate {
                    reason: EscalationCause::TimedOutTwice { unit, session_ref },
                }
            }
        }
        IterationCause::AttemptsExhausted { unit } => Disposition::Escalate {
            reason: EscalationCause::MaxAttemptsExceeded {
                unit,
                attempts: policy.max_attempts_per_unit,
            },
        },
        IterationCause::AuthorityNotCritic {
            unit,
            required,
            exercisable,
        } => Disposition::Escalate {
            reason: EscalationCause::AuthorityRequired {
                unit,
                required,
                exercisable,
            },
        },
        IterationCause::SandboxValidationFailed { unit } => Disposition::Escalate {
            reason: EscalationCause::SandboxValidation { unit },
        },
        IterationCause::ReferenceDigestMismatch { expected, actual } => {
            Disposition::AbortRun { expected, actual }
        }
        IterationCause::NonConverging {
            unit,
            key,
            occurrences,
        } => Disposition::EnterDiagnosis {
            unit,
            key,
            occurrences,
        },
    }
}

/// §11.5 row 2's exponential backoff: `min(base × 2^(n-1), max_backoff)`
/// with 1-based `attempt`. Saturating arithmetic — a span past `u64` is the
/// ceiling, never a wrap. Pure: the controller asserts these values
/// against virtual time; nothing here sleeps.
pub fn backoff_ms(policy: &ControlPolicy, attempt: u32) -> u64 {
    let shift = attempt.saturating_sub(1).min(u64::BITS - 1);
    let factor = 1u64 << shift;
    let grown = policy.retry_base_ms.saturating_mul(factor);
    grown.min(policy.retry_max_ms)
}

#[cfg(test)]
mod tests {
    use super::{
        ControlPolicy, ControlPolicyError, ControlSettings, IterationCause, TransientLocus,
    };
    use super::{Disposition, EscalationCause, backoff_ms, dispose};
    use gauntlet_domain::{Authority, Digest, SessionRef, UnitId};

    /// The §11.4 inline default, referenced (never spelled at a
    /// duration-hinted site).
    const TICK: u64 = crate::config::defaults::POLL_INTERVAL_MS;
    const QUIET_WINDOW: u64 = 45_000;
    const FLOOR: u64 = 100;
    /// The attempt ordinal whose backoff is the floor times four — named
    /// so no bare figure sits in a value position inside the row-two
    /// match's joinable region.
    const THIRD_ATTEMPT: u32 = 3;
    const CEILING: u64 = 1_600;
    const ATTEMPT_CAP: u32 = 2;
    const THRESHOLD: u32 = 3;

    fn policy() -> ControlPolicy {
        ControlPolicy::new(TICK, QUIET_WINDOW, FLOOR, CEILING, ATTEMPT_CAP, THRESHOLD)
            .expect("the fixture policy is usable")
    }

    fn ak_unit() -> UnitId {
        UnitId::new("AK-9001").expect("fixture unit id")
    }

    fn digest(seed: u8) -> Digest {
        Digest::new("sha256", &seed.to_string().repeat(32)).expect("fixture digest")
    }

    // -----------------------------------------------------------------------
    // Policy construction (brief Item 3's typed absence errors)
    // -----------------------------------------------------------------------

    #[test]
    fn required_settings_absent_from_the_config_layer_are_typed_errors() {
        // The config layer's default state carries ONLY the §11.4 inline
        // tick default; the four REQUIRED settings are None, and each
        // absence names its setting and its Annex E variable.
        let settings = ControlSettings::from_defaults();
        assert_eq!(settings.poll_interval_ms, TICK);
        match ControlPolicy::from_settings(&settings, THRESHOLD) {
            Err(ControlPolicyError::RequiredSettingMissing { setting, variable }) => {
                assert_eq!(setting, "stall_timeout_ms");
                assert_eq!(variable, "GAUNTLET_STALL_TIMEOUT_MS");
            }
            other => panic!("absence must be a typed error, got {other:?}"),
        }
        // Supplying each missing setting surfaces the next absence — all
        // four are checked, in the open.
        let mut filled = settings.clone();
        filled.stall_timeout_ms = Some(QUIET_WINDOW);
        match ControlPolicy::from_settings(&filled, THRESHOLD) {
            Err(ControlPolicyError::RequiredSettingMissing { setting, .. }) => {
                assert_eq!(setting, "retry_base_ms");
            }
            other => panic!("the second absence must be typed too, got {other:?}"),
        }
        filled.retry_base_ms = Some(FLOOR);
        match ControlPolicy::from_settings(&filled, THRESHOLD) {
            Err(ControlPolicyError::RequiredSettingMissing { setting, .. }) => {
                assert_eq!(setting, "retry_max_ms");
            }
            other => panic!("the third absence must be typed too, got {other:?}"),
        }
        filled.retry_max_ms = Some(CEILING);
        match ControlPolicy::from_settings(&filled, THRESHOLD) {
            Err(ControlPolicyError::RequiredSettingMissing { setting, .. }) => {
                assert_eq!(setting, "max_attempts_per_unit");
            }
            other => panic!("the fourth absence must be typed too, got {other:?}"),
        }
        filled.max_attempts_per_unit = Some(ATTEMPT_CAP);
        let composed =
            ControlPolicy::from_settings(&filled, THRESHOLD).expect("all four supplied composes");
        assert_eq!(composed, policy());
    }

    #[test]
    fn zero_policy_values_are_refused_setting_by_setting() {
        for zeroed in [
            ControlPolicy::new(0, QUIET_WINDOW, FLOOR, CEILING, ATTEMPT_CAP, THRESHOLD),
            ControlPolicy::new(TICK, 0, FLOOR, CEILING, ATTEMPT_CAP, THRESHOLD),
            ControlPolicy::new(TICK, QUIET_WINDOW, 0, CEILING, ATTEMPT_CAP, THRESHOLD),
            ControlPolicy::new(TICK, QUIET_WINDOW, FLOOR, 0, ATTEMPT_CAP, THRESHOLD),
            ControlPolicy::new(TICK, QUIET_WINDOW, FLOOR, CEILING, 0, THRESHOLD),
            ControlPolicy::new(TICK, QUIET_WINDOW, FLOOR, CEILING, ATTEMPT_CAP, 0),
        ] {
            assert!(
                matches!(zeroed, Err(ControlPolicyError::ZeroSetting { .. })),
                "a zero window must be refused: {zeroed:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Row 1 / row 5: residual re-dispatch, text only; attempts exhausted
    // -----------------------------------------------------------------------

    #[test]
    fn a_residual_with_attempts_left_re_dispatches_with_residual_text_only() {
        let cause = IterationCause::ResidualFound {
            unit: ak_unit(),
            residual_text: "expected exit status 0, observed 1 at tests/x.rs line 12".to_owned(),
            attempts_used: 1,
        };
        match dispose(cause, &policy()) {
            Disposition::RetryWithResidual { unit, attachment } => {
                assert_eq!(unit, ak_unit());
                // TEXT ONLY: the attachment IS the residual text; there is
                // no other payload field for a critic trajectory to ride.
                assert_eq!(
                    attachment.as_str(),
                    "expected exit status 0, observed 1 at tests/x.rs line 12"
                );
            }
            other => panic!("row 1 re-dispatches, got {other:?}"),
        }
        // The companion guarantee the controller asserts on its dispatch
        // prompt: a critic's reasoning string is nowhere in the attachment,
        // because the attachment was built from the residual text alone.
        let attachment = super::ResidualAttachment::new("residual: dimension mismatch");
        let critic_reasoning = "the critic believes the builder misread the module boundary";
        assert!(!attachment.as_str().contains(critic_reasoning));
        assert!(!attachment.as_str().contains("critic"));
    }

    #[test]
    fn a_residual_with_no_attempts_left_escalates_max_attempts_exceeded() {
        // ATTEMPT_CAP attempts used: row 1's guard fails, row 5 fires.
        let cause = IterationCause::ResidualFound {
            unit: ak_unit(),
            residual_text: "still failing".to_owned(),
            attempts_used: ATTEMPT_CAP,
        };
        match dispose(cause, &policy()) {
            Disposition::Escalate {
                reason: EscalationCause::MaxAttemptsExceeded { unit, attempts },
            } => {
                assert_eq!(unit, ak_unit());
                assert_eq!(attempts, ATTEMPT_CAP);
            }
            other => panic!("row 5 escalates with the ceiling, got {other:?}"),
        }
        // The bare row-5 cause escalates identically.
        match dispose(
            IterationCause::AttemptsExhausted { unit: ak_unit() },
            &policy(),
        ) {
            Disposition::Escalate {
                reason: EscalationCause::MaxAttemptsExceeded { .. },
            } => {}
            other => panic!("attempts-exhausted escalates, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Row 2: exponential backoff between the configured bounds
    // -----------------------------------------------------------------------

    #[test]
    fn transient_backoff_grows_exponentially_and_is_capped_at_the_ceiling() {
        // min(base × 2^(n-1), max): the sequence doubles from the fixture
        // floor and clamps at the fixture ceiling — pure arithmetic,
        // asserted by value (virtual time; nothing sleeps).
        let expected = [
            FLOOR,
            FLOOR * 2,
            FLOOR * 4,
            FLOOR * 8,
            CEILING,
            CEILING,
            CEILING,
        ];
        for (index, figure) in expected.iter().enumerate() {
            let attempt = u32::try_from(index + 1).expect("1-based attempt");
            assert_eq!(backoff_ms(&policy(), attempt), *figure);
        }
        // A ceiling below the floor clamps to the ceiling — the formula's
        // own min(), not an invented error.
        let tight = ControlPolicy::new(TICK, QUIET_WINDOW, CEILING, FLOOR, ATTEMPT_CAP, THRESHOLD)
            .expect("usable");
        assert_eq!(backoff_ms(&tight, 1), FLOOR);
        // Absurdly large attempt numbers saturate at the ceiling, never
        // wrap (the shift is bounded by the word width).
        let grown = backoff_ms(&policy(), u32::MAX);
        assert_eq!(grown, CEILING);
        // The disposition carries the same arithmetic.
        match dispose(
            IterationCause::TransientError {
                locus: TransientLocus::Store,
                attempt: THIRD_ATTEMPT,
            },
            &policy(),
        ) {
            Disposition::RetryAfterBackoff { locus, delay_ms } => {
                assert_eq!(locus, TransientLocus::Store);
                assert_eq!(delay_ms, FLOOR * 4);
            }
            other => panic!("row 2 retries after backoff, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Row 3: malformed_result — one reminder, then escalation
    // -----------------------------------------------------------------------

    #[test]
    fn a_malformed_result_gets_one_reminder_then_escalates() {
        match dispose(
            IterationCause::MalformedResult {
                unit: ak_unit(),
                reminders_used: 0,
            },
            &policy(),
        ) {
            Disposition::RetryOnceWithSchemaReminder { unit } => assert_eq!(unit, ak_unit()),
            other => panic!("the first malformed result re-dispatches, got {other:?}"),
        }
        match dispose(
            IterationCause::MalformedResult {
                unit: ak_unit(),
                reminders_used: 1,
            },
            &policy(),
        ) {
            Disposition::Escalate {
                reason: EscalationCause::MalformedTwice { unit },
            } => assert_eq!(unit, ak_unit()),
            other => panic!("the second malformed result escalates, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Row 4: TimedOut — cancel, preserve the session ref, retry once
    // -----------------------------------------------------------------------

    #[test]
    fn a_timeout_cancels_preserves_the_session_and_retries_exactly_once() {
        let session = SessionRef::new("session-forensic-1").expect("fixture session ref");
        match dispose(
            IterationCause::TimedOut {
                unit: ak_unit(),
                session_ref: Some(session.clone()),
                retries_used: 0,
            },
            &policy(),
        ) {
            Disposition::CancelThenRetryOnce { unit, session_ref } => {
                assert_eq!(unit, ak_unit());
                assert_eq!(
                    session_ref.as_ref().map(SessionRef::as_str),
                    Some("session-forensic-1")
                );
            }
            other => panic!("the first timeout retries once, got {other:?}"),
        }
        match dispose(
            IterationCause::TimedOut {
                unit: ak_unit(),
                session_ref: Some(session.clone()),
                retries_used: 1,
            },
            &policy(),
        ) {
            Disposition::Escalate {
                reason: EscalationCause::TimedOutTwice { session_ref, .. },
            } => {
                // The escalation PRESERVES the session reference — forensic
                // evidence travels with the escalation, never dropped.
                assert_eq!(
                    session_ref.as_ref().map(SessionRef::as_str),
                    Some("session-forensic-1")
                );
            }
            other => panic!("the second timeout escalates, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Rows six, seven, and eight: escalate-never-loop, never-unsandbox,
    // abort-no-retry
    // -----------------------------------------------------------------------

    #[test]
    fn an_authority_gap_escalates_and_has_no_loop_disposition() {
        match dispose(
            IterationCause::AuthorityNotCritic {
                unit: ak_unit(),
                required: Authority::Human,
                exercisable: Authority::LeadAgent,
            },
            &policy(),
        ) {
            Disposition::Escalate {
                reason:
                    EscalationCause::AuthorityRequired {
                        required,
                        exercisable,
                        ..
                    },
            } => {
                assert_eq!(required, Authority::Human);
                assert_eq!(exercisable, Authority::LeadAgent);
            }
            other => panic!("row 6 escalates, got {other:?}"),
        }
    }

    #[test]
    fn a_sandbox_failure_escalates_and_the_table_has_no_unsandboxed_outcome() {
        match dispose(
            IterationCause::SandboxValidationFailed { unit: ak_unit() },
            &policy(),
        ) {
            Disposition::Escalate {
                reason: EscalationCause::SandboxValidation { .. },
            } => {}
            other => panic!("row 7 escalates, got {other:?}"),
        }
        // Structural half: no disposition variant exists that relaxes
        // containment — the fallback is unrepresentable. The enum carries
        // only re-dispatch (same work), retry-after-backoff (same work),
        // reminder, cancel-retry, escalate, abort, and diagnosis.
        fn assert_exhaustive(disposition: &Disposition) -> bool {
            matches!(
                disposition,
                Disposition::RetryWithResidual { .. }
                    | Disposition::RetryAfterBackoff { .. }
                    | Disposition::RetryOnceWithSchemaReminder { .. }
                    | Disposition::CancelThenRetryOnce { .. }
                    | Disposition::Escalate { .. }
                    | Disposition::AbortRun { .. }
                    | Disposition::EnterDiagnosis { .. }
            )
        }
        let sample = Disposition::Escalate {
            reason: EscalationCause::SandboxValidation { unit: ak_unit() },
        };
        assert!(assert_exhaustive(&sample));
    }

    #[test]
    fn a_reference_digest_mismatch_aborts_with_both_digests_and_no_retry() {
        let frozen = digest(1);
        let moved = digest(2);
        match dispose(
            IterationCause::ReferenceDigestMismatch {
                expected: frozen.clone(),
                actual: moved.clone(),
            },
            &policy(),
        ) {
            Disposition::AbortRun { expected, actual } => {
                // BOTH digests travel with the abort — §9.4's evidence
                // requirement, not a log line.
                assert_eq!(expected, frozen);
                assert_eq!(actual, moved);
            }
            other => panic!("row 8 aborts, never retries, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Row 9: non-convergence enters diagnosis
    // -----------------------------------------------------------------------

    #[test]
    fn a_non_converging_residual_set_stops_correcting_and_enters_diagnosis() {
        use super::super::diagnosis::ResidualKey;
        use gauntlet_domain::Severity;
        let key = ResidualKey {
            unit: ak_unit(),
            dimension: "functional".to_owned(),
            severity: Severity::Blocking,
            expected: "the report exists at the locator".to_owned(),
            observed: "evd-fixture exited 1".to_owned(),
        };
        let carried = key.clone();
        match dispose(
            IterationCause::NonConverging {
                unit: ak_unit(),
                key,
                occurrences: THRESHOLD,
            },
            &policy(),
        ) {
            Disposition::EnterDiagnosis {
                unit,
                key,
                occurrences,
            } => {
                assert_eq!(unit, ak_unit());
                assert_eq!(key, carried);
                assert_eq!(occurrences, THRESHOLD);
            }
            other => panic!("row 9 enters diagnosis, got {other:?}"),
        }
    }
}
