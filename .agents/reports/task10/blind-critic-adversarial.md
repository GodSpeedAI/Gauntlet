# Task 10 blind adversarial critic

**Scope:** frozen Core-spec, answer key AK-026–AK-034, Task 10 plan, and current Task-10 implementation/tests only. Read-only review; no compilation was run.

## Findings

### C1 — Critical: the supposedly frozen reference view is neither provenance-bound nor blind-value checked

**Evidence:** `crates/gauntlet-app/src/compare/envelope.rs:86-105` accepts independent caller-controlled `reference_digest: Digest` and `reference_view: String`; `:192-212` copies both into the envelope and hashes them without resolving a frozen reference artifact or proving that the view projects the bytes named by the digest. `assert_blind` at `:298-304` searches only for literal JSON *keys*. Consequently a caller can provide the genuine frozen digest while putting builder reasoning/identity (or escaped JSON containing it) in `reference_view`, `constraints`, or an open-residual detail. The result hashes and passes P2 because the leak is a value under an allowlisted key, not a denylisted top-level key.

**Impact:** violates I4/§13.2 structural blindness and I1/§9.4 provenance simultaneously; a comparator can receive builder context disguised as reference material.

### M1 — Major: model comparison payload digests are accepted without binding them to the dispatched envelope

**Evidence:** `crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:224-252` validates the outer role-result invocation ID and outer context digest, then returns the inner `ComparisonRecordDoc` unchanged. `:293-305` constructs a domain record from the document's own `reference_digest`, `observation_digest`, and `envelope_digest`; it never compares any of them (nor `run_id`, iteration, or contract version) to `envelope`. `crates/gauntlet-app/src/compare/review.rs:321-327` perpetuates those record digest values into the aggregate input. Aggregation only checks agreement *among supplied records* (`aggregate.rs:260-304`), so a single forged/stale model record passes.

**Impact:** an outer contract-correct child can submit a `comparison_record@v1` for a different reference/observation/envelope and have it admitted as this review's result. This defeats the digest cross-wiring protection required by §5.3.6, §10.3(6), and §13.2.

### M2 — Major: timeout path abandons the critic instead of cancelling it

**Evidence:** `ModelComparator::drive_to_terminal` (`gauntlet-adapter-compare-model/src/lib.rs:255-273`) returns `TimedOut` after 24 running polls. `comparison_document` maps that to an error (`:185-210`) but calls no `AgentRunner::cancel`; the only `cancel` implementation visible is test scaffolding at `:629-636`. Spec §10.3(5) requires the runtime to cancel a child when no result arrives by deadline.

**Impact:** timed-out critics may continue with access to their workspace/result path and later write a stale result; resource/budget and forensic state are also not brought to a terminal cancellation path.

### M3 — Major: `high_assurance` silently degrades to ordinary fanout if no adversarial source is supplied

**Evidence:** in `review.rs:226-240`, the adversarial pass runs only under `if let Some(adversarial)`; `None` adds neither an error nor `unavailable_evidence`. The existing all-modes test deliberately passes `adversarial: None` (`:699-703`) and accepts `HighAssurance` (`:705-734`). Spec §13.3 requires high assurance to fan out across all dimensions **plus** an adversarial comparator.

**Impact:** configuration or wiring omission yields a successful-looking high-assurance review without its defining protection.

### M4 — Major: budget-truncated review can produce a passing comparison record while gaps live only in a side channel

**Evidence:** truncation appends `UnavailableEvidence` separately (`review.rs:206-208`, `:213-215`, `:264-266`), while `:281-289` aggregates only dispatched records and returns the gaps outside `ComparisonRecordDoc`. Thus with fanout budget 1, one clean model record and one omitted dimension, `aggregate` can produce `Verdict::Pass` (`aggregate.rs:350-370`). The AK-034 test asserts only that `outcome.unavailable_evidence()` has a gap (`review.rs:604-624`), not that the returned record is inconclusive or that any persisted comparison record carries the gap.

**Impact:** consumers that persist/use `ReviewOutcome::record()`—the consequential comparison artifact—can treat incomplete review as pass, contrary to I8 and AK-034.

### M5 — Major: aggregation discards an already-recorded dispute

**Evidence:** `ResidualDoc` has a decoded `disputed` field (`comparison_record.rs:285-287`, `:396-409`), but `MergedResidual::absorb` in `aggregate.rs:119-166` never reads it. Its new dispute calculation (`:168-170`) uses only local severity and verdict-intent sets. Therefore aggregating one previously aggregated record (or any record whose comparator already marked a same-key disagreement) converts `disputed: true` to false when its single residual has one severity/intent.

**Impact:** material comparator disagreement can disappear before required verification/diagnosis, violating §13.5's preservation rule.

### M6 — Major integration gap: no Task-10 comparison path is reachable from the intended CLI

**Evidence:** `crates/gauntlet-cli/src/main.rs:19-40` accepts only `doctor` and `reference digest`; no branch constructs an envelope, comparator source, review, or either comparator adapter. `crates/gauntlet-cli/Cargo.toml` lists only `gauntlet-adapter-compare-diff` as a **dev-dependency** (`:34-38`) and no compare-model adapter. The file explicitly says production wiring lands later (`main.rs:11`; Cargo comment `:32-38`).

**Impact:** current tests exercise isolated/library composition, but an operator cannot run the Task-10 comparison mechanism via the reference CLI. There is no production path to establish fresh-context dispatch, artifact freezing, or result binding end-to-end.

### M7 — Medium: “canonical” envelope bytes remain input-order dependent

**Evidence:** `build_envelope` transfers caller vectors directly (`envelope.rs:205-207`); `ComparisonEnvelope::to_json` emits `constraints` and `open_residuals` in their provided order (`comparator.rs:553-565`). Facet/gap vectors similarly retain bundle order (`envelope.rs:153-190`). No sort/canonicalization occurs before the digest at `envelope.rs:211-212`.

**Impact:** equivalent sets with different collection order generate different envelope digests, weakening reproducibility and making digest identity sensitive to incidental observer/caller ordering despite the Task 10 requirement to serialize canonically.
