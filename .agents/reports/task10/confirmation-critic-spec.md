# Task 10 confirmation critic — specification review

## Verdict: NOT CONFIRM

Fresh read-only review of the frozen Core spec, frozen answer key, Task 10 plan, and current implementation/tests. No prior reports, status material, history, or compilation were consulted. Critical and major residuals remain.

## Critical residuals

### C1 — Comparison-contract freeze is represented only by a mutable version string
**Evidence.** Core spec §9.5 (lines 669–672) requires the separate comparison contract to be frozen with the reference and its version recorded on every record; changing the rubric mid-run is ruler movement. `ComparisonContract` carries only parsed fields and `version` (`crates/gauntlet-app/src/compare/contract.rs:114–123`); `EnvelopeInputs` accepts an arbitrary `&ComparisonContract` and a separate digest/string reference view (`compare/envelope.rs:86–105`). `build_envelope` copies only version/dimensions/rubric (`:205`), and model dispatch accepts any envelope whose version is literal `"1"` (`gauntlet-adapter-compare-model/src/lib.rs:142–150`). No contract digest, frozen-contract evidence reference, or binding of contract contents to the frozen `ReferenceVersion` is carried or checked.

**Impact.** A changed same-version rubric/contract can be parsed and used mid-run while all comparison records still say version `1`; the purported freeze cannot detect ruler movement.

**Disposition.** **Fixable code/design:** introduce a frozen contract value with content digest/evidence ref, bind it to the run's frozen reference at formation, carry/check that identity at envelope, invocation, record, aggregation, and settlement boundaries. A new wire-field/version is a **reference-candidate/schema revision**, rather than silently relaxing the existing ruler.

### C2 — P2 bytes have no provenance binding to the frozen reference or to permitted sources
**Evidence.** `EnvelopeInputs.reference_view` is an unconstrained `String` and `reference_digest` is a separately supplied `Digest` (`compare/envelope.rs:86–105`); the builder never derives the view from frozen bytes nor verifies the view against the digest (`:141–216`). Likewise `constraints` and `open_residuals` accept arbitrary strings/records. The P2 proof is only `bytes.contains()` over eleven spelling tokens (`:255–304`), and the integration test supplies the placeholder `"the frozen reference's answer key, rendered view"`, not a rendered frozen artifact (`p2_context_isolation.rs:315–334`).

**Impact.** A caller can put builder rationale/identity/trajectory into `reference_view` (or another allowlisted text field) without a deny-list label, pair it with the real reference digest, and pass both `assert_blind` and the current test. Conversely, legitimate frozen text containing a denied spelling produces a false violation. The serialized bytes prove neither the claimed source provenance nor blindness of their contents.

**Disposition.** **Fixable code/design:** make the builder accept a typed frozen-reference projection and typed provenance-bearing constraint/residual inputs; derive the rendered projection inside the trusted freeze boundary and bind it to its digest. Parse structural JSON and enforce field/source provenance rather than substring matching. The needed meaning is already required by §13.2; no ruler relaxation is justified.

## Major residuals

### M1 — Model comparison creates no durable `RoleInvocation` record and ignores collection failure
**Evidence.** The plan requires the envelope digest recorded on both invocation and comparison record (Task 10 step 2, plan lines 378–380); Core §5.3.5 defines the provenance-bearing `RoleInvocation` and §13.2 requires its digest coupling. `ModelComparator` constructs only transient `RoleInvocationSpec` (`gauntlet-adapter-compare-model/src/lib.rs:169–186`) and has no state/evidence/event-log dependency. After `Completed`, it calls `collect` but discards its `Result`, then directly reads a path built from its local root (`:199–249`). No returned `result_ref`, durable invocation, actual runner/model/session provenance, or collection failure is retained.

**Impact.** The record may echo an envelope digest, but there is no durable invocation-side evidence to which it is coupled. A failed validated collection is bypassed by a separately readable file, defeating the port's completion boundary and forensic provenance.

**Disposition.** **Fixable code:** route dispatch through the run/state composition layer; persist the admitted invocation and its exact dispatch digest/runner/model/session/result path, require successful collection/contract validation, and verify the completed result ref is the declared path.

### M2 — `fanout` is serial; high-assurance does not establish an adversarial pass
**Evidence.** Core §10.4/§13.3 specifies per-dimension fan-out “in parallel” (spec lines 757–769, 926–933). `review()` loops and executes `compare_to_doc` synchronously for every dimension (`compare/review.rs:221–235`), so a model call completes before the next is admitted. For high assurance it merely requires `Some(adversarial)` and calls `across_dimensions` (`:186–194,236–250`); `ComparatorSource` has no adversarial role/prompt/identity property (`:68–84`). The test explicitly passes the same ordinary `Sources` as both model and adversarial source (`:719–727`), so no adversarial instruction is proven.

**Impact.** Fan-out latency and failure isolation do not meet the required dispatch behavior, and a normal comparator can be relabeled adversarial while the system claims high assurance.

**Disposition.** **Fixable code:** admit all selected model critics before fan-in/polling, preserve per-invocation deadlines/cancellation, and use a typed adversarial comparator/source whose dispatched prompt and identity are checked. Do not downgrade the spec.

### M3 — Aggregation discards contributing comparator provenance
**Evidence.** Input records contain `ComparatorIdentity` (`ports/comparison_record.rs:135–165`), but aggregation emits only identity `gauntlet-app::compare::aggregate` with empty model/dimension (`compare/aggregate.rs:411–425`). It retains no input-record refs or contributor identities. `ReviewOutcome` exposes only that aggregate and gaps, discarding its local `records` vector (`compare/review.rs:291–321`).

**Impact.** After fan-in, a disputed residual retains a boolean and evidence-ref union but cannot identify which critic/model/dimension supplied each conflicting finding. This prevents the specified route to verification/diagnosis from being provenance-auditable and defeats the record-level provenance declared in Annex B.4.

**Disposition.** **Fixable code plus schema candidate revision:** preserve immutable individual comparison-record evidence refs and/or contributor identities in an aggregate provenance structure, then make dispute routing consume them. Because current strict `comparison_record@v1` rejects unknown fields, adding durable aggregate provenance requires an explicit candidate schema/reference revision, not an unannounced wire change.

## Scope notes

The deterministic keyed merge, highest-severity rule, blocking-fail rule, and basic record-coupling checks are present in source. They do not resolve the residuals above. No claim of passing tests is made because compilation/execution was outside this review's instruction.
