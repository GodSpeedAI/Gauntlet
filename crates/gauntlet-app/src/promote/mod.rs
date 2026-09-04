//! The process optimizer and the §19.4 self-improvement gate (plan Task
//! 23; spec §19.3–§19.4).
//!
//! > It produces proposals. It may not modify the active loop. …
//! > A candidate becomes active only when **all** hold: governance
//! > preserved · reference integrity preserved · settlement access
//! > preserved · rollback valid · regression suite passes · protected
//! > capabilities non-inferior · target metric improves by the declared
//! > margin · payment remains acceptable.
//! >
//! > Insufficient evidence ⟹ `provisional` ⟹ not promoted.
//!
//! The optimizer's ONLY output channel is a typed `candidate_revision@v1`
//! result document (spec Annex B via the strict codec); a revision
//! becomes active only through the FULL eight-condition conjunction
//! proven over REAL isolated evaluation runs. The pieces:
//!
//! - [`proposal`] — the optimizer surface. Strict envelope decode →
//!   §19.3 completeness → durable PENDING revision in the revision
//!   ledger. The admission service struct holds ONLY the ledger and the
//!   clock — structurally no configuration-write or roles-install path
//!   (the arch suite's containment scan pins this). The `propose` path
//!   dispatches the REAL optimizer role invocation through the
//!   `AgentRunner` port: invoke → handle → bounded poll → collect at the
//!   declared result-contract path — never an await of an answer.
//! - [`delta`] — the candidate change: a configuration delta over the
//!   effective-config key vocabulary, declared keys only, with the
//!   verification-bearing classification as a typed TOTAL function
//!   (fail-closed on unknown keys — no default-allow).
//! - [`metrics`] — the typed target-metric vocabulary and the derivation
//!   of a run's [`RunMetrics`](gauntlet_ports::revision_ledger::RunMetrics)
//!   from that run's DURABLE facts, the same function for both arms.
//! - [`evaluate`] — the evaluation harness: CONTROL and TREATMENT runs
//!   through an injected executor seam (implemented at the CLI composition
//!   root over the real composed loop), `parent_run` set on the treatment
//!   arm, both arms against the same frozen reference, per-run memory
//!   imported through the §19.1 developmental import, and the durable
//!   [`EvaluationRecord`](gauntlet_ports::revision_ledger::EvaluationRecord).
//! - [`gate`] — the §19.4 gate: ALL EIGHT conditions, typed per-condition
//!   refusals; any FAILED condition ⇒ Rejected with recorded reasons and
//!   the rollback ref; evidence MISSING or undecidable ⇒ Provisional —
//!   NEVER promoted, and never rejected (insufficient evidence is not a
//!   negative result).
//! - [`rollback`] — the restore surface: append-only restore records; the
//!   derived state flips to promoted-but-rolled-back. A rollback of a
//!   never-promoted revision is a typed refusal.
//!
//! Do not conflate the two promotion gates: §19.2
//! ([`crate::memory::promotion`]) promotes CAPABILITIES from settlement
//! evidence; §19.4 (this module) promotes REVISIONS from evaluation
//! evidence.

pub mod delta;
pub mod evaluate;
pub mod gate;
pub mod metrics;
pub mod proposal;
pub mod rollback;

#[cfg(test)]
mod tests;
