# Task 10 adversarial confirmation critic

**Verdict: DO NOT CONFIRM.** This fresh, read-only review found major defects in the Task-10 comparison path. No compilation or test execution was performed. Scope was limited to the frozen Core spec and answer key, Task-10 plan, and current source/tests; prior Task-10 reports, status, and history were not consulted.

## Contract checked

- Core spec §13.2 requires an envelope made from an allowlist, fresh-context comparison, and a serialized digest stored on invocation and comparison record (I4/P2).
- §13.3 requires adaptive critics only for dimensions still undecided after deterministic probes; `fanout` is parallel; `high_assurance` includes an adversarial pass.
- §13.5 requires deterministic typed aggregation; I8 says missing evidence never passes.
- AK-026/027, AK-032/033/034 and the Task-10 plan are the acceptance anchors.

## Major findings

### M1 — Context blindness is bypassable through raw allowlisted strings (**TRUE PORT BLOCKER**)

`EnvelopeInputs` exposes `reference_view: String` (`crates/gauntlet-app/src/compare/envelope.rs:84-106`), while `ComparisonEnvelope` publicly exposes all fields (`crates/gauntlet-ports/src/comparator.rs:508-530`). There is no type or validation binding `reference_view` to the frozen reference digest/projection. Thus a caller can retain the real frozen `reference_digest` but put builder trajectory into `reference_view` (or raw constraint/residual text). For example, a value such as `"The author retried the failing path seven times before this review."` contains no `DENY_LISTED_FIELD_NAMES` token, so `assert_blind` passes although it leaks attempt/reasoning information to the critic.

`assert_blind` is only a substring scan (`envelope.rs:286-292`) and is not called by envelope construction or either comparator. The P2 test only poisons JSON with known field spellings (`crates/gauntlet-cli/tests/p2_context_isolation.rs:486-511`); it does not prove source provenance or this value-level bypass. This defeats the claimed structural property in §13.2/AK-026. The port needs a frozen/projection-backed reference value (and constrained rendering provenance), not caller-provided arbitrary `String`s plus a lexical deny list.

### M2 — Envelope digest is never revalidated at a comparator/review consumption boundary (**TRUE PORT BLOCKER**)

`recompute_envelope_digest` exists (`envelope.rs:236-246`) but has no call site outside its own file/tests. A caller can mutate any public envelope field after build, retaining its old digest. `ModelComparator` dispatches the mutated `envelope.to_json()` paired with the old digest (`crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:159-178`); its result checks merely require the child to echo that same stale digest (`:352-402`). `DiffComparator` likewise copies the declared digest without recomputing it (`crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs:112-126`).

Concrete cross-wire: build E1, change `reference_view` or a facet in its public envelope, then dispatch it. The child sees E2 but every record and contract check can still name E1's digest. This violates §13.2's content/digest coupling and makes stale-result protections attest a label rather than serialized bytes. Recompute and refuse before every dispatch/compare (with a comparison/contract-mismatch error), and make invalid mutation unrepresentable where practical.

### M3 — Adaptive mode treats unavailable/failed instrument evidence as conclusively settled

`review` marks a dimension settled whenever a deterministic probe has *any* residual (`crates/gauntlet-app/src/compare/review.rs:263-270`). The deterministic diff comparator emits a Major residual and `Verdict::Inconclusive` for `unavailable_evidence` and `observation_failures` (`crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs:212-286`). Consequently an unavailable test/security/etc. probe suppresses the model critic instead of being an undecided dimension; no review-level truncation gap is added either.

That is directly contrary to §13.3 and AK-033: only dimensions probes settled conclusively may be skipped. It also conflicts with I8. The existing adaptive test uses a residual as its sole definition of settled and misses the inconclusive-gap case (`review.rs:~650`). The decision must distinguish conclusive deterministic evidence (for example, a blocking instrument failure) from `Inconclusive`/gap results, and tests must cover both unavailable and observer-failure probes.

### M4 — `fanout` / `high_assurance` dispatches serially, not in parallel (**TRUE PORT BLOCKER**)

The `ReviewMode::Fanout | HighAssurance` branch performs synchronous `compare_to_doc` calls in a `for` loop (`review.rs:219-247`). Adaptive's model calls are serial too (`:257-286`). This does not implement the §13.3 requirement that fanout runs one comparator per dimension *in parallel*. The current `Comparator` port is synchronous and lacks `Send`/async constraints (`gauntlet-ports/src/comparator.rs:640-658`), so the application API cannot safely promise the required concurrent dispatch. This can consume the shared deadline dimension-by-dimension and prevents the specified bounded fan-out behavior.

### M5 — Aggregate hard-gate merge fails open when a contributor omits a gate

The aggregate documentation says a hard gate is held only if **every contributor** held it, but the implementation only iterates reported gates (`crates/gauntlet-app/src/compare/aggregate.rs:337-362`). With records A=`gate(U, held=true)` and B=`[]`, output is `gate(U, held=true)`: B's missing evidence is treated as agreement. The test covers only A=true/B=false (`aggregate.rs:~796-831`), not omission. This is a material I8 fail-open in the Task-10 aggregate record. Track all contributors per gate and make absence false/inconclusive as the record semantics require.

## Production-composition boundary

**TRUE SPEC/PORT BLOCKER TO ANY PRODUCTION-COMPOSITION CLAIM (not evidence that Task 10's library-only gate itself failed):** there is no current executable runtime path that invokes `build_envelope`, `review`, `ModelComparator::new`, or `DiffComparator` outside unit/integration tests. Repository source search found those constructions only in their implementation/test files; `crates/gauntlet-cli/src/main.rs` exposes only `doctor` and `reference digest` and explicitly defers the full surface to Task 20. Therefore the P2 integration test proves a hand-composed test chain, not reachability through an intended production control path. Do not claim Task-10 production composition until a runtime composition path exists and exercises these adapters.

## Bounded positive checks

- Canonical envelope serialization sorts the listed semantic collections and excludes the self-referential digest (`gauntlet-ports/src/comparator.rs:535-638`); no ordering instability was found in that bounded inspection.
- Aggregation uses typed `BTreeMap`/`BTreeSet`, checks peer records' six coupling fields, preserves dispute state, and forces a blocking residual to `Fail` (`aggregate.rs:271-422`).
- The model adapter does strictly bind its nested comparison document to its *declared* envelope/run/dimension and rejects mismatched role-result identity/digest (`gauntlet-adapter-compare-model/src/lib.rs:231-278, 352-402`). That protection is insufficient for M2 because the declared digest itself is not recomputed from dispatched bytes.
- Comparator failures propagate as `ReviewError`; no serial-mode error-to-pass conversion was found.

## Required confirmation tests after correction

1. P2 value-smuggling test: builder canary placed in a raw reference/constraint value must be rejected before critic dispatch, not merely detected by an identifier substring.
2. Build E1, mutate serialized content while retaining E1 digest, and prove both adapters/review refuse it as a typed contract mismatch.
3. Adaptive tests for unavailable and observer-failure probe facets must dispatch the model critic (or persist explicit unavailable evidence if budget prevents it).
4. A concurrency/overlap test must demonstrate dimension fanout is parallel under the declared bound.
5. Aggregate `gate=true` plus a contributor omitting that gate must not yield `held=true`/pass.
6. An intended production composition test must invoke the real runtime route, verify generated envelope bytes/digest, dispatch a critic via its result contract, aggregate, and preserve unavailable evidence.
