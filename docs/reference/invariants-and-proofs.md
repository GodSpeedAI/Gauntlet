# Reference: Invariants, Proofs & Anti-Gaming

This document specifies the 12 Non-Negotiable Invariants (I1–I12), the 12 Proof Obligations (P1–P12), and the 16 Anti-Gaming Defenses (G1–G16) that govern Gauntlet.

---

## The 12 Non-Negotiable Invariants (Spec §3)

| ID | Invariant Statement | Structural Enforcement Mechanism | Proof Obligation & Test Locus |
|---|---|---|---|
| **I1** | The builder never modifies the standard its work is evaluated against. | Reference normalized and hashed via SHA-256 at start; locked in `.gauntlet/reference.lock`; answer key placed outside builder workspaces; re-verified at settlement. | **P1** — [`crates/gauntlet-cli/tests/p1_reference_integrity.rs`](../../crates/gauntlet-cli/tests/p1_reference_integrity.rs) |
| **I2** | Observation is not the builder's completion claim. | Observers probe reality via instruments; builder text is isolated in `claims_facet` marked `excluded_from_verification: true`. | **P3** — [`crates/gauntlet-cli/tests/p3_real_observation.rs`](../../crates/gauntlet-cli/tests/p3_real_observation.rs) |
| **I3** | Critic $\neq$ Verifier. | Distinct ports, records, and outputs (`Residual` vs `Outcome`). Settlement requires verifier `PASS`; critic opinion alone cannot close a Unit. | **P11** — [`crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`](../../crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs) |
| **I4** | The builder cannot review itself (structural context blindness). | `EnvelopeBuilder` builds context envelopes exclusively from an allowlist of fields; builder trajectory, reasoning, identity, and attempts are omitted. | **P2** — [`crates/gauntlet-cli/tests/p2_context_isolation.rs`](../../crates/gauntlet-cli/tests/p2_context_isolation.rs) |
| **I5** | Similarity never authorizes. | Model scores and resemblance produce candidates and residuals only; settlement and authority gates read typed fields. | **P5** — [`crates/gauntlet-cli/tests/p5_hard_gate.rs`](../../crates/gauntlet-cli/tests/p5_hard_gate.rs) |
| **I6** | No false capability credit. | Capability ledger requires $N \ge 3$ distinct settlements across verified variation axes plus an autonomous recovery event; single successes write memory only. | **P10** — [`crates/gauntlet-cli/tests/p10_no_false_learning.rs`](../../crates/gauntlet-cli/tests/p10_no_false_learning.rs) |
| **I7** | Self-improvement is governed structural revision. | Optimizer produces `CandidateRevision`s; promotion requires isolated evaluation, held-out comparison, regression testing, and rollback safety. | **P9** — [`crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`](../../crates/gauntlet-cli/tests/p9_self_improvement_containment.rs) |
| **I8** | Missing evidence is not passing evidence. | `Outcome` is strictly three-state (`PASS`, `FAIL`, `INCONCLUSIVE`). Missing evidence yields `INCONCLUSIVE`, which blocks settlement. | **P12** — [`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`](../../crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs) |
| **I9** | Authority gates whether it runs; sandbox gates how contained it is. | Both checks precede builder process start, in that order; irreversible units are never auto-dispatched. | **P5** — [`crates/gauntlet-cli/tests/p5_hard_gate.rs`](../../crates/gauntlet-cli/tests/p5_hard_gate.rs) |
| **I10** | The runtime is bounded even when configuration is incomplete. | Every budget dimension has conservative defaults; absence of config never permits infinite loops; recovery reserve (15%) is never spent forward. | **P6** — [`crates/gauntlet-cli/tests/p6_budget_control.rs`](../../crates/gauntlet-cli/tests/p6_budget_control.rs) |
| **I11** | Critical state is never reconstructed from conversational memory. | Durable append-only event log (`fsync` on commit) + PostgreSQL store; resume reads disk logs, not chat transcripts. | **P7** — [`crates/gauntlet-cli/tests/p7_recovery.rs`](../../crates/gauntlet-cli/tests/p7_recovery.rs) |
| **I12** | Core control logic depends on semantic ports, never on products. | Zero compile-time dependencies on vendor SDKs in domain or app; ports provide shared contract suites; passes across $\ge 2$ runner adapters. | **P8** — [`crates/gauntlet-cli/tests/p8_adapter_substitution.rs`](../../crates/gauntlet-cli/tests/p8_adapter_substitution.rs) |

---

## The 16 Anti-Gaming Strategies (Spec §23.2)

Autonomous agents frequently attempt to "game" evaluation rubrics to report success. Gauntlet implements mechanical defenses for each known strategy:

| Strategy | Gaming Attempt | Mechanical Defense | Test Locus |
|---|---|---|---|
| **G1** | Fake pass without running tests | `SettlementService` requires an `evidence_ref` pointing to a verifiable test report artifact in the store. | `p12_missing_is_not_passing.rs` |
| **G2** | Rewriting requirements mid-run | `verify_unchanged` verifies reference SHA-256 digest before settlement; aborts run on mismatch. | `p1_reference_integrity.rs` |
| **G3** | Critic grading its own builder attempt | Critic envelope builder strips author identity and trajectory; fresh context per comparison. | `p2_context_isolation.rs` |
| **G4** | Prompt injection via observation | Observation facets are structured JSON; builder text is isolated to `claims_facet` excluded from prompts. | `p3_real_observation.rs` |
| **G5** | Leaking builder reasoning to critic | `EnvelopeBuilder` allowlists only reference, diffs, and test exits; reasoning is physically dropped. | `p2_context_isolation.rs` |
| **G6** | Hiding failed observations | Observers must report all executed probes; missing probe output is flagged in `observation_failures`. | `g6_hide_failed_observations.rs` |
| **G7** | Tier downgrade by agent | `AuthorityGate` rejects any agent-originated reduction in `blast_radius` or `authority`. | `g7_tier_downgrade.rs` |
| **G8** | Premature capability promotion | `PromotionService` requires $N \ge 3$ settlements under measured variation with a recovery event. | `p10_no_false_learning.rs` |
| **G9** | Moving ruler during self-improvement | Optimizer revisions cannot modify active answer keys or weaken verification methods. | `p9_self_improvement_containment.rs` |
| **G10** | Builder writing to answer key | Answer key mounted read-only outside builder workspaces; single-writer arch scanner checks code paths. | `g10_builder_writes_answer_key.rs` |
| **G11** | Silent unwrap of sandboxing | `none` sandbox refuses execution for tiers above `contained`; never falls back silently. | `p5_hard_gate.rs` |
| **G12** | Boolean conversion of Outcome | `gauntlet-arch-tests` asserts no `impl From<Outcome> for bool` exists in the workspace. | `gauntlet-arch-tests/denylist.rs` |
| **G13** | Infinite recursive self-spawning | Strict `max_depth` (default: 1) and `max_rounds` (default: 8) bounded by `BudgetMeter`. | `p6_budget_control.rs` |
| **G14** | Reconstructing state from chat | Controller resumes only from `events.jsonl` and PostgreSQL; never accepts prompt state injections. | `p7_recovery.rs` |
| **G15** | Averaging away hard requirements | Hard-gate units cannot be satisfied by weighted quality dimension averages. | `p5_hard_gate.rs` |
| **G16** | Leaking vendor APIs into core | `gauntlet-arch-tests` denylist scans domain and app crates, failing on vendor identifiers. | `gauntlet-arch-tests/denylist.rs` |

---

## Source Trail

- [`crates/gauntlet-cli/tests/p1_reference_integrity.rs`](../../crates/gauntlet-cli/tests/p1_reference_integrity.rs) — P1 proof.
- [`crates/gauntlet-cli/tests/p2_context_isolation.rs`](../../crates/gauntlet-cli/tests/p2_context_isolation.rs) — P2 proof.
- [`crates/gauntlet-cli/tests/p3_real_observation.rs`](../../crates/gauntlet-cli/tests/p3_real_observation.rs) — P3 proof.
- [`crates/gauntlet-cli/tests/p4_corrective_loop.rs`](../../crates/gauntlet-cli/tests/p4_corrective_loop.rs) — P4 proof.
- [`crates/gauntlet-cli/tests/p5_hard_gate.rs`](../../crates/gauntlet-cli/tests/p5_hard_gate.rs) — P5 proof.
- [`crates/gauntlet-cli/tests/p6_budget_control.rs`](../../crates/gauntlet-cli/tests/p6_budget_control.rs) — P6 proof.
- [`crates/gauntlet-cli/tests/p7_recovery.rs`](../../crates/gauntlet-cli/tests/p7_recovery.rs) — P7 proof.
- [`crates/gauntlet-cli/tests/p8_adapter_substitution.rs`](../../crates/gauntlet-cli/tests/p8_adapter_substitution.rs) — P8 proof.
- [`crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`](../../crates/gauntlet-cli/tests/p9_self_improvement_containment.rs) — P9 proof.
- [`crates/gauntlet-cli/tests/p10_no_false_learning.rs`](../../crates/gauntlet-cli/tests/p10_no_false_learning.rs) — P10 proof.
- [`crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`](../../crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs) — P11 proof.
- [`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`](../../crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs) — P12 proof.
