# Task 10 — correction round 2

**Scope.** Reconciled the two confirmation critics against Core-spec §§5.3.5,
9.5, 13.2–13.5 and Task-10 steps 1–6. No frozen Wayfinder/Core material,
reference lock, comparison-contract artifact, reference candidate, or
`CURRENT_STATUS` file was changed.

## Corrected residuals

| Residual | Disposition |
|---|---|
| Stale envelope digest (both adapters and review) | **Corrected.** `gauntlet-ports` now computes/verifies the canonical SHA-256 at the shared port boundary. Diff verifies on both `compare` and B.4-document paths; model verifies before admission; review verifies before source selection. All reject `ComparatorError::ContractMismatch`. P2, both adapters, and review have mutation teeth. |
| Model completion bypassed `collect` | **Corrected within adapter boundary.** Model now requires `Completed.result_ref == declared result-contract path`, requires successful deadline-bounded `collect`, validates collected dispatch identity/state, and consumes the collected B.4 payload. It no longer reopens a local file after collection failure. A scripted collection-failure tooth is included. Durable `RoleInvocation` persistence remains blocked below. |
| Adaptive treats gaps/inconclusive probe as settled | **Corrected.** A deterministic record settles only when it addresses the dimension and is not `Inconclusive`. Gap-bearing probe evidence is retained and the model receives that dimension if budget permits; budget exhaustion remains explicit unavailable evidence. |
| Hard-gate omission fails open | **Corrected.** Gate merge tracks distinct contributing records; a gate is held only when every contributor reported it held. Omission yields `held=false` and an `Inconclusive` aggregate verdict, never held/pass. |
| Build-boundary P2 check test-only | **Corrected.** `build_envelope` invokes `assert_blind` before returning. The check now parses JSON object keys rather than scanning prose values: legitimate frozen text may quote a denied term, but an injected denied field is refused. This is intentionally only field-shape blindness, not source provenance. |
| Aggregation contributor provenance | **Partially corrected without B.4 mutation.** `ReviewOutcome` retains immutable contributor B.4 documents for immediate verification/diagnosis, including each comparator identity. Aggregate-wire durable contributor provenance is not added: `comparison_record@v1` has no such field and strict decoding rejects unknown fields. A durable field requires an explicit B.4 candidate/schema revision. |

## Fail-first evidence

1. `adaptive_inconclusive_probe_is_not_a_settlement` initially failed: model
   dispatch log was `[]`, not both dimensions. It passes after the conclusive
   decision distinction.
2. `omitted_hard_gate_is_not_held` initially failed because true-plus-omission
   yielded held. After contributor tracking, its strengthened verdict assertion
   also failed first (`Pass` vs required `Inconclusive`); it now passes.
3. The P2 stale-envelope tooth was run with only the diff `compare` boundary
   check temporarily removed: exit 101 (mutated bytes compared). The exact
   check was restored; the final P2 run is green.
4. The dependency-policy pin initially failed after the necessary port-level
   SHA-256 dependency (exit 100); its explicit ports allowlist/pin was then
   added and architecture boundaries are green.

## Green evidence

- `cargo test -p gauntlet-app compare::` — 26 passed.
- `cargo test -p gauntlet-adapter-compare-diff` — 6 passed.
- `cargo test -p gauntlet-adapter-compare-model` — 5 passed.
- `cargo test -p gauntlet-cli --test p2_context_isolation` — 3 passed.
- `cargo check --workspace --all-targets` — pass.
- `cargo clippy --workspace --all-targets -- -D warnings` — pass.
- `cargo deny check` — pass (pre-existing informational warnings only).
- `cargo nextest run -p gauntlet-arch-tests` — 194/194 pass.
- `bash scripts/verify-ruler.sh` — pass.

## Explicit blockers / no deferred-compliance claim

1. **Frozen contract and projection provenance (critics C1/C2):**
   `EnvelopeInputs` still receives separately supplied `reference_digest` and
   rendered `reference_view`; the current freeze lock records no
   comparison-contract content digest/evidence ref bound to the reference.
   §9.5/§13.2 require that meaning, but making it durable across invocation,
   B.4 record, aggregation, and settlement needs a frozen-contract/reference
   projection type plus versioned record fields. That is a candidate
   revision/schema change, not a Task-10 silent wire change.
2. **Parallel/admission fanout (M2/M4):** Core §13.3 requires parallel fanout,
   while `Comparator::compare` and `ComparatorSource` are synchronous
   request/response APIs with no admission handle, executor, cancellation
   join, or concurrent budget reservation. Current review remains serial and
   makes no parallel-compliance claim. A port/controller revision is needed.
3. **Durable invocation state (M1):** this correction fixes collection and
   dispatch/result coupling, but `ModelComparator` has no `StateStore`,
   `EvidenceStore`, or `EventLog` dependency/composition owner to persist a
   `RoleInvocation`. The controller/runtime composition must ledger it before
   production settlement; do not infer durability from transient specs.
4. **Production CLI composition:** Task-10's P2 test is a dev-only composed
   proof. The CLI has no production review loop (plan Task 20 is the named
   full CLI/mock-E2E milestone; Task 12 owns controller/budget/diagnosis).
   No production reachability is claimed.
5. **Typed adversarial source:** high assurance still requires a distinct
   provided source, but `ComparatorSource` has no adversarial-role identity
   capability. Enforcing that property requires the same dispatch/source-port
   revision as parallel admission; it was not relabeled as achieved.
