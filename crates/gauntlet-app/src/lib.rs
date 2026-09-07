//! Application layer (spec §4.1): `RunController`, `IterationEngine`,
//! `UnitScheduler`, `CorrectionSelector`, `SettlementService`,
//! `PromotionService`, `ReferenceFreezer`, `EnvelopeBuilder`,
//! `DiagnosticEngine`.
//!
//! Operates on the domain through ports only. Depends on no adapter — if
//! replacing a provider required editing this layer, the boundary would be
//! wrong (spec §24.3), and `just boundaries` would fail before any reviewer
//! could miss it.
//!
//! Plan Task 5 lands the first services here: configuration resolution
//! ([`config`], spec §8) and the preflight checks ([`doctor`], spec §8.5).

pub mod baseline;
pub mod compare;
pub mod config;
pub mod conformance;
pub mod control;
pub mod doctor;
pub mod formation;
pub mod govern;
pub mod memory;
pub mod observation;
pub mod promote;
pub mod reference;
pub mod report;
pub mod roles;
pub mod settle;
pub mod skills;
pub mod state;
pub mod verify;

pub use config::defaults::{
    DIAGNOSTIC_BUDGET, DIGEST_ALGO, GIT_STRATEGY, INTERACTIVE, InteractiveDefault, MAX_COST_USD,
    MAX_CRITICS, MAX_DEPTH, MAX_PARALLEL, MAX_ROUNDS, MAX_TOKENS, MAX_WALL_SECONDS,
    NONCONVERGENCE_N, PREFER_DETERMINISTIC, RECOVERY_RESERVE_PCT, REVIEW_MODE, SANDBOX_BACKEND,
};
pub use config::{
    CanonValue, ConfigError, ConfigKey, EffectiveConfig, EnvLayer, FlagOverrides, ResolvedConfig,
    config_fingerprint, resolve,
};
pub use doctor::{CheckOutcome, CheckStatus, DoctorInput, DoctorReport, SubResult, run_doctor};
pub use state::{ExportError, export_run};
