# Task 10 — Builder brief: Comparator and the envelope builder (P2)

You are the builder. Read this file completely before touching code.
Orchestrator constraints (AGENTS.md, CURRENT_STATUS.md "Rules that outrank
whatever task you are on") bind you. Your claim of completion carries no
settlement authority — you produce code, tests, checkpoints, and a report.

## Non-negotiables

1. `just ruler` before you start and after every slice. If it fails, STOP and
   report; that outranks this task.
2. Never edit `.wayfinder/**`, `.gauntlet/reference.lock`, or
   `.agents/specs/Core-spec.md`. Apparent ruler defects → a dated entry in
   `.agents/reports/reference-candidates.md` (evidence, affected Units,
   whether new class or instance of an existing one) — never a workaround.
3. Never weaken a test to reach green; never treat INCONCLUSIVE as PASS.
4. You may NOT modify the frozen reference, comparison criteria, acceptance
   gates, immutable evidence, or prior settlement records. Task-9 observation
   modules are CONSUMED, not redesigned — behavior-preserving additions only,
   each justified in your report.
5. Build lock: you are the only compiler. Do not raise Cargo/nextest
   parallelism above the repo configuration (WSL, ~7.8 GiB RAM). Prefer
   targeted `cargo nextest run -p <crate> <filter>`; workspace-wide runs only
   at slice ends. Watch `du -sh target` (ceiling ~4 GiB, env-configured).
6. Checkpoint contract (message-loss lesson): after EVERY slice below,
   append a dated entry to `.agents/reports/task10/builder-checkpoints.md`
   (what landed, test counts, open questions). Your final message may be
   lost; the files are the contract. Final report →
   `.agents/reports/task10/builder-round1.md` (structure at the end of this
   brief).
7. Run `cargo clippy --workspace --all-targets -- -D warnings` and
   `cargo fmt` before declaring any slice done (Task-8 lesson: a dying
   builder's never-run lint gate went red).
8. Epistemic discipline in every report: bounded claims stay bounded. "No
   evasion found" means none found within the declared scope and search.

## Authoritative reading (in order, before coding)

- `.agents/specs/Core-spec.md` §9.5 (comparison contract), §12 (observation,
  esp. §12.3 claims exclusion, §12.4 digest), §13 ALL (envelope allowlist,
  review modes, dimensions, aggregation), §5.3.5 (RoleInvocation
  `context_envelope_digest`), §5.3.6, §6.2 (Comparator port), §6.3 (required
  adapter set: Comparator = model-backed + deterministic-diff), §21 (failure
  classes), Annex B.1 (role_result envelope wraps EVERY result contract),
  B.4 (comparison_record@v1).
- `.wayfinder/answer-key.md` AK-026, AK-027, AK-032, AK-033, AK-034 (also
  AK-031 for context — comparator produces residuals only; its proof is
  Task 13's, not yours).
- `.agents/plans/gauntlet-impl.md` Task 10 section.
- Code: `crates/gauntlet-ports/src/comparator.rs` (current envelope — the
  type you will widen), `crates/gauntlet-ports/src/contract/comparator.rs`
  (shared suite), `crates/gauntlet-domain/src/comparison.rs`,
  `crates/gauntlet-domain/src/role_invocation.rs`,
  `crates/gauntlet-app/src/observation/{input,digest,bundle,assembly}.rs`
  (VerificationInputs — the claims-free facet view you build on),
  `crates/gauntlet-ports/src/role_result.rs` (RolePayload),
  `crates/gauntlet-ports/src/json.rs` (JsonValue),
  `crates/gauntlet-adapter-agent-mock/src/{runner,scenario}.rs`,
  one observe adapter (e.g. `gauntlet-adapter-observe-test`) as the adapter
  pattern, `crates/gauntlet-arch-tests/src/walk.rs` (ANNEX_B_CONTRACT_PARSERS
  inventory — lines ~114–190 — and the maskable-intersection pin discipline).

## Deliverables (slices, in order; checkpoint after each)

### D1 — Domain + ports surface (the sanctioned widening)

This task is the ledgered trigger for the Task-9 deferrals "envelope
widening" (B-3c + A-M3) and "domain view drops facets" (B-8a). Widen
deliberately:

1. **Domain** `ComparisonRecord` gains a REQUIRED `envelope_digest: Digest`
   (spec §13.2: "the digest is stored on both the RoleInvocation and the
   ComparisonRecord"). Constructor change; every existing constructor
   callsite updates. Document the reconciliation: Annex B.4's example JSON
   does not spell the field, §13.2 requires it — B.4 is a minimum shape, the
   same reconciliation class as Task 9's env-fingerprint-in-evidence-doc.
2. **Ports `ComparisonEnvelope`** — new allowlist shape per §13.2:
   - keep: `run_id`, `iteration_no`, `reference_digest`, `reference_view`.
   - replace `observation: ObservationBundle` (digest+gaps only) with a
     facet-carrying observation view: `observation_digest`, `bundle_ref`,
     instrument facet summaries (kind, instrument name+version, command,
     exit_status, evidence_ref), BOTH gap arrays. Same field set as app
     `VerificationInputs` — that similarity is the point: the envelope's
     observation side IS the verification-input view. Claims remain excluded
     at the TYPE level (no claims-typed field, private construction from
     typed parts — Task-9 discipline, no runtime `if kind == claims`).
   - add: comparison contract view (version + instantiated dimensions +
     severity rubric — from the §9.5 artifact), applicable constraints,
     unresolved-but-relevant residuals (§13.2 allowlist members currently
     missing).
   - The module doc must record: this widening is the ledgered Task-10
     boundary decision; adding any further field is a boundary decision.
3. **Ports wire codec**: `comparison_record@v1` (Annex B.4, field-for-field:
   residuals with dimension/unit_refs/expected/observed/evidence_refs/
   severity/confidence/disputed/diagnosis/cheapest_discriminating_test/
   candidate_corrections; hard_gate_results; verdict) — strict decode
   (unknown fields rejected), typed errors, in a NEW ports module. Strictness
   class: settlement-adjacent.
4. **Ports `RolePayload`** gains a `Comparison(...)` variant carrying the
   B.4 document (B.1 wraps every result contract; a critic's result contract
   is a comparison_record). Strict decode as above.
5. **Shared comparator contract suite** — update for the widened envelope and
   ADD cases: (a) the returned record's `envelope_digest` equals the
   envelope's serialized digest; (b) the envelope's facet-level observation
   reality is visible to the comparator (a facet with a failing exit status
   is present in what the comparator receives — the P3 comparison-consumption
   coupling). Existing two cases keep their semantics.
6. **Arch suite**: the new codec module joins `ANNEX_B_CONTRACT_PARSERS`;
   recompute/extend the maskable-intersection pin and any key-mask tests from
   the REAL member list (Task-9 C7 discipline). Every boundary-suite edit is
   deliberate, minimal, and listed in your report for the adversarial critic.

### D2 — App `compare::envelope` + the §9.5 comparison contract artifact

1. `.gauntlet/comparison-contract.yaml` — versioned artifact (spec §9.5):
   version, instantiated dimensions (from §13.4 candidate set), severity
   rubric, what each comparator receives (the allowlist), what counts as a
   residual. Keep it small and honest.
2. App loader: strict parse → typed view; unknown version ⇒ typed error
   (`formation`/`configuration` class). Unit-tested.
3. `EnvelopeBuilder`: constructs the ports envelope from EXACTLY the
   allowlist inputs — frozen reference digest + rendered reference view,
   `VerificationInputs` (from a real `ObservationBundleDoc`), contract view,
   constraints, open residuals. Nothing else enters; there is no
   `with_extra_context` escape hatch.
4. **Digest recompute at the consumption boundary** (B-3c, MAJOR deferral —
   closing it is mandatory): the builder recomputes
   `observation::digest::observation_digest(doc)` and REFUSES (typed error,
   observation/comparison failure class) on mismatch with the doc's declared
   digest. A tampered facet or gap array can no longer flow silently into a
   comparison.
5. Canonical serialization of the whole envelope (JsonValue, documented key
   order per member) + `envelope_digest` (SHA-256 over the canonical bytes —
   same hashing idiom as `observation::digest`). The `RoleInvocation` for a
   critic dispatch carries it as `context_envelope_digest` (§5.3.5/§13.2) —
   prove that composition with a real `RoleInvocation` construction.

### D3 — App `compare::aggregate` (spec §13.5, AK-032)

Deterministic, total, model-free merge over N `comparison_record@v1`
documents:

- Keyed by `(dimension, expected, observed_locator)`; duplicates merge,
  keeping the HIGHEST severity and the UNION of evidence refs.
- Any `blocking` residual ⇒ aggregate verdict `fail`, regardless of
  everything else.
- Disagreement on the same key (differing severity or verdict intent)
  preserved as `disputed: true` — never averaged away; disputed blocking
  residuals flagged for verification/diagnosis routing.
- Determinism proven over shuffled input order: same inputs in any order ⇒
  byte-identical aggregate (canonical ordering by the merge key).
- Aggregation is arithmetic over typed records: no AgentRunner, no model, no
  clock, no randomness in the merge path. Name the test
  `deterministic_aggregation` (gate filter `compare::` must hit it).

### D4 — App `compare::review` (spec §13.3, AK-033/AK-034)

Review-mode dispatch over the §13.4 dimensions instantiated by the contract:

- `single`: one comparator across all applicable dimensions.
- `fanout`: one comparator per dimension, merged via D3.
- `adaptive` (default): deterministic probes first (the deterministic-diff
  comparator); dimensions settled CONCLUSIVELY by instruments are skipped —
  no model dispatch for them; fan out model critics only across undecided
  dimensions; merge.
- `high_assurance`: fanout across all dimensions PLUS an adversarial
  comparator pass instructed to find the strongest case the work does NOT
  meet the reference.
- Budget truncation: a typed model-dispatch budget (count of model-comparator
  dispatches); dimensions dropped because the budget ran out are recorded as
  `unavailable_evidence` entries (kind = dimension name, reason cites
  truncation) — NEVER silently omitted. Name the tests
  `adaptive_skips_settled_dimensions` and `truncation_is_recorded`.
- Full `BudgetMeter` composition is Task 12's ledgered trigger — record a
  deferral note, do not wire the port here.

### D5 — Comparator adapters (spec §6.3 REQUIRED set)

1. **`gauntlet-adapter-compare-diff`** (existing stub crate — fill it):
   deterministic diff comparator. No model, no clock dependence. Derives
   residuals from facet-level observation reality under the contract (e.g. a
   failing exit status on an instantiated dimension ⇒ residual at the
   rubric's severity; gap arrays ⇒ residuals/gap-forwarding per I8). Emits
   `comparison_record@v1` AND returns the domain `ComparisonRecord` carrying
   all three digests. Refuses unknown contract versions
   (`EnvelopeInvalid`). AK-028-class provenance: `comparator_identity`
   filled with the adapter's own identity.
2. **NEW crate `crates/adapters/gauntlet-adapter-compare-model`**:
   model-backed comparator over `dyn AgentRunner` (constructed with the
   runner — composition root injects mock or real). Flow: build critic
   `RoleInvocationSpec` (role critic, `context_envelope_digest` = envelope
   digest, result-contract path) → `invoke` → `poll`/`collect` within the
   `Deadline` → strict-parse `role_result@v1` at the path → payload must be
   the `Comparison` variant → convert to domain record. Failure taxonomy:
   timeout/no document ⇒ typed error (never a pass); malformed document ⇒
   typed error; wrong `invocation_id` ⇒ typed `contract_mismatch`-class
   error. Uncertainty is never success (§21.2).
3. **Both adapters pass the UNMODIFIED shared comparator suite** via a
   factory (suite runs live in the adapter crates' tests, Task-4/8 pattern).
4. Mock composition: for compare-model tests prefer a small in-crate
   scripted critic runner (implements `AgentRunner`, writes comparison
   payloads at the contract path, scripts the derailments) IF extending the
   pinned mock scenario dialect would disturb AK-059's eight fixtures or the
   suite pin. If you extend the dialect instead, it must be additive and
   optional with the eight fixtures byte-identical and the AK-059 pin green.
   State which path you took and why.

### D6 — Proof + evidence locators

1. `crates/gauntlet-cli/tests/p2_context_isolation.rs` (composition root,
   p3's pattern — read `p3_real_observation.rs` first):
   - Build a REAL observation of a lying-builder artifact through the real
     Task-9 adapters (the Task-8 lying fixture: declared self-check passes
     while the artifact fails) → bundle → `VerificationInputs`.
   - Build the envelope via D2; assert the serialized bytes contain NONE of
     the §13.2 deny-list, proven with canaries you inject into the
     builder-side inputs (builder reasoning text, identity, attempt counts,
     prior verdicts, operator chat, claims content) — each canary must
     appear in the builder-side source material and be absent from the
     envelope bytes.
   - Assert facet-level reality IS present (failing exit status visible).
   - Digest-validation tooth material: tamper the bundle after digesting ⇒
     envelope builder refuses with the typed error.
   - Comparator consumption: run compare-diff over the envelope; the record
     must carry a residual derived from the INSTRUMENT failure (not from
     claims); digests couple (envelope/reference/observation).
   - **Teeth variant, same file, `#[test]` named `teeth`** (AK-027's
     locator suffix `::teeth`): inject a deny-listed field
     (`"builder_reasoning"` + canary) into the envelope serialization and
     assert the blindness proof FAILS on it. This is the designed-to-fail
     assertion pair: the proof function must reject the poisoned envelope.
2. Gate-filter tests in `gauntlet-app` named exactly:
   `deterministic_aggregation`, `adaptive_skips_settled_dimensions`,
   `truncation_is_recorded` (under `compare::` modules).
3. Reference candidates: dated entries for AK-026, AK-027, AK-032, AK-033,
   AK-034 — each locator names a repo-level test path that cannot be
   materialized under the arch-suite escapes rule; cite the REAL in-crate
   tests; mark each as another instance of the existing
   locator-materialization class (first recorded 2026-08-17, AK-049).
4. Recorded acts for your report: any `ADAPTER_EXTERNAL_ALLOWLIST` growth
   (compare-model should need NO new external deps — if you think you need
   one, stop and justify first), boundary-suite inventory growth, new
   workspace member.

## Out of scope (do not do)

- Settlement, verifier, controller, BudgetMeter wiring (Tasks 11–13).
- Port-shape gaps deferred to "ports revision" (facet Optionality, §12.2
  wider facet vocabulary) — record, don't fix.
- Observer process timeouts (Task-12 trigger — do not add process spawning
  here).
- CLI surface beyond what the proof needs (no new user-facing commands).
- AK-031's settlement-side proof (Task 13).

## Gate (run all, record output in your report)

```bash
just ruler
cargo nextest run -p gauntlet-domain
cargo nextest run -p gauntlet-ports
cargo nextest run -p gauntlet-app compare::
cargo nextest run -p gauntlet-adapter-compare-diff
cargo nextest run -p gauntlet-adapter-compare-model
cargo nextest run --test p2_context_isolation   # from crates/gauntlet-cli: cargo nextest run -p gauntlet-cli --test p2_context_isolation
just check && just lint && just deny && just boundaries
cargo nextest run --workspace   # once, at the end
just ruler
```

## Final report structure (`.agents/reports/task10/builder-round1.md`)

Per slice: what landed (files, line counts), test counts (focused + gate),
decisions taken and why, spec quotes relied on, deviations from this brief
(with reasons), residuals you could not close (bounded claims), recorded
acts. Then: full gate output, `du -sh target`, and a "teeth readiness" list —
the exact edits an orchestrator should make-and-revert to prove the gates
bite (candidates: delete the digest recompute; delete the deny-list
assertion; make aggregation order-dependent; make truncation silent; drop
envelope_digest from the record).

Remember: files are the contract. Checkpoint early, checkpoint often.
