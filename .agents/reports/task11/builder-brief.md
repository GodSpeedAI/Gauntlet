# Task 11 — Builder brief (round 1)

You are the builder for plan Task 11 (`.agents/plans/gauntlet-impl.md`, §"Task 11
— Verifier and evidence binding *(phase 3 · proves P12)*"). You implement a
bounded transformation. You may NOT modify: `.wayfinder/` (frozen ruler),
`.agents/research/preregistrations/task-11.prereg.yaml` or its `.sha256`
(frozen preregistration), the spec, the plan, acceptance gates, comparison
criteria, or any prior task's report. If you believe the frozen reference is
defective, write the evidence in your report — never edit the ruler.

Read first, in order: `AGENTS.md`, `.agents/specs/Core-spec.md` §14 (all),
§16.1, §6.2 (Verifier port), §21 (error taxonomy), Annex B.5
(`verification_record@v1`), `.agents/plans/gauntlet-impl.md` Task 11, and this
brief. Relevant frozen Units: AK-005 (three-state; locator
`tests/proofs/p12_missing_is_not_passing`), AK-035 (deterministic-first),
AK-036 (evidence binding).

## The invariant this task must not violate (learned the hard way in Task 10)

No binding check may compare two values the same caller already supplied.
Task 10's architecture diagnosis
(`.agents/reports/task10/architecture-diagnosis.md`) found this as the root
cause of its round-3 residuals, and the frozen task-11 prereg predicts (P1)
that the first Task-11 implementation will decide staleness from
caller-supplied data. Prove that prediction WRONG:

- The **requirement** side of every §14.3 check must come from the frozen
  reference projection (the unit's `evidence_required`), loaded by a governed
  operation — not from the caller's say-so.
- The **evidence** side must come from durable state: a production record the
  evidence owner wrote (write-once), plus the `EvidenceStore` itself for
  resolvability.
- The **recency** decision must read an ordering fact from durable state (event
  log offset, iteration number, or invocation-record anchor) that no `verify`
  caller supplies. A test where the request LIES about recency must still be
  rejected.

The established pattern to mirror: `crates/gauntlet-app/src/compare/authority.rs`
(earned typestate chain, no public constructors, `pub(crate)` sealing,
unforgeable products). Also mirror `StateStore::bind_observation_bundle` /
`observation_binding` (write-once durable binding, migration
`0005_observation_bindings`) for whatever durable fact you introduce.

## Deliverables

### D1 — Ports: Annex B.5 codec + §14.3 typed classes

- `verification_record@v1` strict-only codec in a new ports module (mirror
  `crates/gauntlet-ports/src/comparison_record.rs`): every B.5 field, unknown
  keys refused, strict decode. `instrument.deterministic` is the model-rung
  flag carrier; `class` carries the typed INCONCLUSIVE class (`null` for
  non-INCONCLUSIVE records).
- Typed class vocabulary for §14.3 failures: `evidence_unresolvable`,
  `evidence_stale`, `evidence_producer_mismatch`. §14.3 states four conditions
  but names three classes; the locator-mismatch condition has no spec-named
  class. Do NOT invent silent collapses: pick and DOCUMENT a mapping
  (most natural: locator mismatch → `evidence_unresolvable`, evidence that is
  not the required evidence), and state it in your report as an explicit,
  critic-reviewable decision.
- Reconcile the port's error/outcome split: §14.3 says binding failures are
  INCONCLUSIVE outcomes with classes, NOT `VerifierError`s. The current
  `verifier.rs` port has `VerifierError::EvidenceMissing` — reconcile per spec
  (P12: missing evidence yields INCONCLUSIVE, not silence, not an error
  escape hatch). Port revisions are allowed; weakening the existing shared
  contract suite is not. EXTENDING the suite with new cases for new surface is
  a recorded act — list it in your report.

### D2 — Durable evidence-production binding (the recency anchor)

The `EvidenceStore` is content-addressed bytes with no metadata; producer,
locator, and recency facts therefore need a durable production record. Add a
write-once state-store binding mirroring the observation-binding discipline
(sqlite migration `0006_*`): production of evidence recorded by the producer
side, keyed so that a second conflicting record for the same evidence is
refused, never overwritten. It must capture at least: run id, evidence ref,
producer instrument identity, the locator it was produced at, and an ordering
anchor (event-log offset and/or invocation id, iteration no) sufficient to
decide "produced in this run at or after the transform it certifies".
Task-10's ledger notes the observation-owner write should be capability-gated
when Task 12 wires its production writer; record the same anticipation as a
residual for your new write.

### D3 — App: `gauntlet-app/src/verify/` (gate filter `verify::`)

- An earned verification chain (mirror `compare/authority.rs`): a governed
  open/read of durable state (run + unit requirement from the frozen reference
  projection + production records + evidence store) that mints an unforgeable
  verification context; the §14.3 checks and the §14.2 ladder live behind it.
  No public constructor for anything consequential; `pub(crate)` sealing where
  Task 10 established it.
- §14.2 ladder: cheapest-conclusive-first per the spec's ladder ordering;
  deterministic instruments preferred over model judgment when both can
  decide; the selected instrument's identity is recorded; model-backed
  verification only where no instrument can decide and it is FLAGGED in the
  record (B.5 `instrument.deterministic: false`).
- Settlement-precondition layer (bounded; settlement itself is Task 13):
  refuse settlement absent a PASS record meeting §16.1 conditions 1–2 (PASS
  record in this run + evidence resolves and is pinned). Record what you do
  NOT check (authority chain, blocking residuals, reference digest — Task 13).

### D4 — Adapters: the four deterministic verifiers + the model rung

- `crates/adapters/gauntlet-adapter-verify-command-exit-code`, `-test-report`,
  `-file-digest`, `-schema-check` (follow the `observe-*` naming/structure
  conventions; workspace members; adapter registry per the arch suite).
- Each: implements `Verifier` (`verify` + `is_deterministic() -> true`),
  reports its instrument identity, and passes the UNMODIFIED Task-4 shared
  `verifier_suite` (see how `observe-*` adapters do it).
- Model rung: mirror `gauntlet-adapter-compare-model` (AgentRunner-backed) —
  either a `verify-model` adapter crate or an app-layer rung over the
  `AgentRunner` port; `is_deterministic() -> false`; flagged in every record
  it mints. Use the mock runner in tests; no new external dependencies.
- External-dep policy: adapters are deny-by-default allowlisted
  (`gauntlet-arch-tests/src/deps.rs`); `sha2` is already allowed in the
  adapter layer. Do not add anything else.

### D5 — Proof + teeth

- `tests/proofs/p12_missing_is_not_passing.rs` at the REPO ROOT, declared as a
  `[[test]]` target of `gauntlet-cli` with `path = "../../tests/proofs/..."`
  (mirror how `gauntlet-arch-tests` declares `tests/contract/*`). This
  MATERIALIZES AK-005's declared locator exactly. The proof is the
  composed-reachability evidence: real adapters, real stores (sqlite + fs),
  at the composition root. Cover:
  - absent evidence file → INCONCLUSIVE (never PASS) at every layer: the
    verifier, the binding checks, and the settlement precondition;
  - stale evidence from a prior iteration/transform → rejected
    (`evidence_stale`);
  - a request that LIES about recency/producer/locator → still rejected (the
    decision reads durable state, not the request);
  - deterministic-first: a case where a model-backed and a deterministic
    verifier could BOTH decide one claim, asserting the deterministic
    instrument's identity is the one recorded;
  - every model-produced record is flagged non-deterministic;
  - each §14.3 class reachable and distinct (no collapsed catch-all).
- Teeth (`task11/teeth.md`, orchestrator re-runs them): neuter the staleness
  check → a named test fails (the plan's done-when tooth); neuter producer /
  locator / resolvability checks → respective tests fail; neuter the
  deterministic-first preference → the both-rung test fails. Revert every
  neutering. NOTE: teeth on untracked files revert by manual inverse edit, not
  `git checkout`.
- AK-035/AK-036 declared locators (`tests/contract/verifier::deterministic_first`,
  `tests/contract/verifier::evidence_binding`) cannot materialize under the
  cargo-target-path rule (same class as AK-049/AK-026/027/028-030/032-034 —
  `::` in the path). Write the two dated reference-candidates entries in your
  report (orchestrator lands them in
  `.agents/reports/reference-candidates.md`); cite your real tests.

### D6 — Gates + reports

- Focused: `cargo nextest run --test p12_missing_is_not_passing && cargo
  nextest run -p gauntlet-app verify::` (the plan's gate).
- Full: `just check && just lint && just deny && just boundaries && just ruler`.
  Workspace suite must stay green (baseline: 697 tests + yours; boundaries
  194 + any new pins you legitimately add).
- Reports: append to `.agents/reports/task11/builder-checkpoints.md` AFTER
  EVERY deliverable (what landed, test counts, decisions, open questions) —
  this file is your result contract if the session dies; final
  `task11/builder-round1.md` (artifact summary, decisions with spec cites,
  teeth table, bounded-claims language, deferrals with triggers).

## Constraints

- **Memory-tight host; one compile at a time.** Never raise build jobs or
  nextest concurrency. Run targeted tests during development; full suite at D6.
- `INCONCLUSIVE` is never `PASS`. No boolean conversions of `Outcome`. Typed
  errors with failure locus; no bare strings, no swallowed errors.
- `clippy -D warnings` and `cargo fmt --check` must pass.
- Bounded claims only: say what you proved, within the scope you tested.
- Model output is untrusted input: validate against the declared schema
  before it enters consequential state.
- `just ruler` before you start and after you finish. It must print PASS.
- If you hit an apparent frozen-reference defect: evidence in the report,
  ruler untouched.

## Report contract

Your final message is unreliable; the files are the contract. When done,
ensure `task11/builder-checkpoints.md` ends with a DONE line listing: gate
outputs (verbatim tail), teeth table summary, deliverables D1–D6 status, and
every decision a critic should adjudicate.
