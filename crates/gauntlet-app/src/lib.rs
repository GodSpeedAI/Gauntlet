//! Application layer (spec §4.1): `RunController`, `IterationEngine`,
//! `UnitScheduler`, `CorrectionSelector`, `SettlementService`,
//! `PromotionService`, `ReferenceFreezer`, `EnvelopeBuilder`,
//! `DiagnosticEngine`.
//!
//! Operates on the domain through ports only. Depends on no adapter — if
//! replacing a provider required editing this layer, the boundary would be
//! wrong (spec §24.3), and `just boundaries` would fail before any reviewer
//! could miss it.
