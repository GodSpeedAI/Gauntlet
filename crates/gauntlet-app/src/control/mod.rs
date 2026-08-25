//! The control core (spec §4.1 application row: `UnitScheduler` ·
//! `CorrectionSelector` · `DiagnosticEngine`; plan Task 12, slice B2).
//!
//! Pure decision logic over injected facts: no clock, no store reads, no
//! runner calls inside. The composed controller (slice B3) supplies FRESH
//! facts at every tick — §11.4's reconciliation steps (poll, re-read the
//! answer key, re-verify the reference digest, re-check budget) are the
//! composition's wiring duty, and each engine here documents that contract
//! on its type.
//!
//! - [`scheduler`]: §11.2 eligibility, §11.2 deterministic ordering, §11.3
//!   slice inheritance, and the depends_on graph validation (Task-7 F7,
//!   adjudicated into Task 12).
//! - [`select`]: §15.1 correction selection — governance-block rejection
//!   first, affordability from the meter, an explainable lexicographic
//!   priority over the ten §15.1 dimensions, and the durable
//!   `CorrectionSelection` record (AK-037).
//! - [`diagnosis`]: §11.5 non-convergence triggering and the §15.2 bounded
//!   diagnostic cycle with the sixteen failure loci (AK-039).

pub mod controller;
pub mod diagnosis;
pub mod disposition;
pub mod scheduler;
pub mod select;
