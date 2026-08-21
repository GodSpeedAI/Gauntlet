
---

## 2026-08-20 — D1: Domain + ports surface (the sanctioned widening) — LANDED, GREEN

What landed:

- `crates/gauntlet-domain/src/comparison.rs`: `ComparisonRecord` gains the
  REQUIRED `envelope_digest: Digest` (constructor arg + accessor; §13.2
  "stored on both the RoleInvocation and the ComparisonRecord"). Doc records
  the B.4-minimum-shape reconciliation (annex example lacks the field;
  normative §13.2 wins — same class as Task 9's env-fingerprint).
- `crates/gauntlet-ports/src/comparator.rs` (rewritten): widened
  `ComparisonEnvelope` — keeps run_id/iteration_no/reference_digest/
  reference_view; `observation: ObservationView` replaces the domain bundle
  (digest + bundle_ref + `FacetSummary` list + BOTH gap arrays — same field
  set as app `VerificationInputs`); adds `contract: ContractView`
  (version + dimensions + severity rubric), `constraints:
  Vec<ApplicableConstraint>`, `open_residuals: Vec<Residual>`, and
  `envelope_digest: Digest`. Canonical `to_json()` (documented key order;
  self-referential digest member EXCLUDED, documented). Claims exclusion is
  TYPE-level: no claims-typed member; `FacetSummary::new` keys on the typed
  `FacetKind` (claims is not a variant — no runtime `if kind == claims`).
  Module doc records the ledgered Task-10 boundary decision.
  `ComparatorError` gains `TimedOut` and `ContractMismatch` (D5 taxonomy).
- `crates/gauntlet-ports/src/comparison_record.rs` (NEW, ~950 lines with
  tests): strict `comparison_record@v1` codec field-for-field per Annex B.4
  (+ the §13.2-required `envelope_digest`), unknown-field rejection at
  every level, typed `ComparisonDocError`; standalone + payload forms
  (B.1 convention: payload omits the schema member); 5 in-module tests.
- `crates/gauntlet-ports/src/role_result.rs`: `RolePayload::Comparison(
  Box<ComparisonRecordDoc>)` variant (B.1 wraps every result contract);
  new round-trip test through a critic `role_result@v1` envelope.
- Shared suite `contract/comparator.rs`: existing two cases keep semantics
  (updated to the widened shape); case 1 extended to assert envelope_digest
  coupling; NEW case `facet_reality_is_visible_to_the_comparator`
  (failing facet present in canonical bytes; comparison over it never a
  clean pass). `contract/stubs.rs`: green stub now honors facet reality;
  NEW broken stub `ComparatorFacetBlind` + suite-run test proving the new
  case bites.
- Arch suite (deliberate, minimal, listed for the adversarial critic):
  `ANNEX_B_CONTRACT_PARSERS` += comparison_record.rs + comparator.rs (the
  envelope canonically serializes the B.3 facet field vocabulary,
  including `command`); its content pin updated. NO `ANNEX_B_WIRE_KEYS`
  growth in the end — see the boundary decision below.

Boundary decision (fog recorded, spec-literal reading chosen): creating
`gauntlet-adapter-compare-model` makes the bare word `model` a derived
adapter SEGMENT, which immediately flagged 10 existing `"model"` literals
in closed Task-5 config code (spec §8.2 `runners.by_role.<role>.model`
settings vocabulary). Two materially different resolutions existed: (a) add
`model` to `ANNEX_B_WIRE_KEYS` + grow the maskable-intersection pin —
blocked: the 10 config literals are NOT in parser modules, so the boundary
gate stays red under (b2) absent config refactors that would amount to
obfuscating spec vocabulary; (b) rule `model` non-distinguishing at segment
derivation — the exact rationale the suite already uses for capability
first segments (`agent`, `state`, `observe`, `compare`, `evidence`: "the
capability the spec itself uses ... deliberately not derived"). Chose (b):
new pinned `SPEC_VOCABULARY_SEGMENTS = ["model"]` in denylist.rs with the
rationale in place; the distinguishing spellings (`compare-model`,
`compare_model`, `model-compare`, full crate name) REMAIN denied (pinned by
the new test `spec_vocabulary_segments_are_pinned_and_the_spellings_still_deny`);
the maskable-intersection pin stays `["command"]` (recomputed from the real
member list, message updated). Recorded for the adversarial critic.

Recorded acts: new workspace member
`crates/adapters/gauntlet-adapter-compare-model` created in D1 (skeleton
manifest + lib doc; filled in D5) so the intersection pin computes from the
real member list at every slice boundary; boundary-suite inventory growth
(parser list 4→6 + pin, SPEC_VOCABULARY_SEGMENTS + pin test); NO
ADAPTER_EXTERNAL_ALLOWLIST growth (compare-model needs no external deps).

Tests: domain+ports 146/146; arch 194/194; `cargo fmt --check` clean;
`cargo clippy` -D warnings clean on domain/ports/compare-model; `just
ruler` PASS; `du -sh target` 1.8G.

Open questions: none blocking. The p2 proof's canary strategy and the
review-mode budget shape are D4/D6 concerns.

---

## 2026-08-20 — D2: compare::envelope + the §9.5 artifact — LANDED, GREEN

What landed:

- `.gauntlet/comparison-contract.yaml` (NEW): the §9.5 versioned artifact —
  version, five instantiated §13.4 dimensions (functional behavior,
  correctness, performance, security, evidence completeness — mapped to
  this repo's instrument vocabulary), the four-severity rubric (instrument
  failure ⇒ blocking; recorded gaps ⇒ major + inconclusive, never pass),
  `comparator_receives` (the §13.2 allowlist, declared), and
  `residual_definition`. Frozen-with-the-reference note in the header.
- `crates/gauntlet-app/src/compare/contract.rs` (NEW, ~430 lines): strict
  YAML-subset loader → typed `ComparisonContract` (version + dimensions +
  rubric → ports `ContractView`). Unknown version ⇒ typed
  `VersionUnsupported` (configuration class, never partially adopted);
  unknown sections, duplicate/missing sections, malformed rubric items,
  unknown severity tokens, missing severities, empty/zero dimensions — all
  typed with line loci. Empty `comparator_receives`/`residual_definition`
  sections are missing declarations, not vacuous passes (I8). 5 tests,
  including a parse of the REPOSITORY's own artifact.
- `crates/gauntlet-app/src/compare/envelope.rs` (NEW, ~530 lines):
  `EnvelopeInputs` = EXACTLY the §13.2 allowlist (no `with_extra_context`
  hatch); `build_envelope`:
  1. **B-3c closed** — recomputes `observation::digest::observation_digest`
     over the bundle doc and REFUSES on mismatch (typed
     `ObservationDigestMismatch`); tampered facets AND tampered gap arrays
     both refused (tested).
  2. Reduces the bundle via `VerificationInputs::of_bundle` → ports
     `ObservationView` (claims excluded structurally; unknown kind tokens
     refused typed).
  3. Canonical serialization (ports `to_json`, digest member excluded) +
     `envelope_digest` = SHA-256 over the bytes (same idiom as
     observation::digest); `recompute_envelope_digest` re-verifies at any
     boundary.
  4. P2 machinery: `DENY_LISTED_FIELD_NAMES` (12 §13.2 spellings) +
     `assert_blind(bytes)` — the AK-026 proof function; the unit-level
     AK-027 pair injects every deny-listed name and asserts failure.
  5. `RoleInvocation` composition proven (critic admission carries exactly
     the envelope digest, §5.3.5/§13.2).
- `crates/gauntlet-app/src/compare/mod.rs` + lib.rs wiring.
- Test-string lesson applied twice (line-continuation `\` eats indentation):
  contract fixtures written as plain multi-line strings.

Decision recorded: the facet COMMAND is provenance (AK-028, Task-9 C10
discipline) — the envelope test's canary asserts the claims STORY, not
command text that legitimately spells the re-run instrument invocation.

Tests: gauntlet-app 152/152 (compare:: 11/11); clippy -D warnings clean;
`cargo fmt --check` clean; `just ruler` PASS; target 1.8G.

Open questions: none blocking. D3 next.

---

## 2026-08-20 — D3: compare::aggregate (spec §13.5, AK-032) — LANDED, GREEN

What landed: `crates/gauntlet-app/src/compare/aggregate.rs` (~640 lines
with tests) — `aggregate(&[ComparisonRecordDoc]) ->
Result<ComparisonRecordDoc, AggregateError>`:

- Keyed merge by `(dimension, expected, observed)` (BTreeMap — canonical
  key order IS the output order); duplicates merge HIGHEST severity +
  UNION of evidence refs (sorted, deduped).
- `disputed` = contributors of the same key carried differing severities
  OR differing verdict intents (one fail, one not); computed from kept
  facts, so a single contributor can never dispute with itself.
- Any blocking residual ⇒ aggregate verdict `fail` regardless of
  everything else; disputed blocking residuals stay disputed (rationale
  text counts them for verification/diagnosis routing).
- Verdict rule (documented, deterministic, total): blocking ⇒ fail; else
  any disputed or any inconclusive input ⇒ inconclusive; else any fail
  input ⇒ fail; else pass. Missing evidence never becomes a pass (I8).
- Hard gates keyed by unit; held = AND of contributors; evidence union.
- Deterministic text merges (diagnosis/cheapest test: higher severity
  wins, ties lexicographic; candidate corrections: sorted union).
- Model-free by construction: no AgentRunner/clock/randomness imports;
  identity runner `gauntlet-app::compare::aggregate`, model field empty.
- Empty input ⇒ typed `InputsEmpty` (an empty merge would be a fabricated
  pass); incoherent coupling fields ⇒ typed `InputsDisagree` naming the
  field.

Tests (6): `deterministic_aggregation` (the exact gate-filter name — all
six input permutations byte-identical, plus merged-shape assertions),
`any_blocking_residual_forces_fail_regardless_of_everything_else`,
`verdict_intents_flow_deterministically` (incl. equal-severity intent
dispute), `hard_gates_hold_only_when_every_contributor_holds`,
`empty_and_incoherent_inputs_are_typed_errors`.

Gates: gauntlet-app 157/157 (compare:: 16/16); clippy -D warnings clean;
fmt clean; `just ruler` PASS; target 1.8G.

Open questions: none. D4 next.

---

## 2026-08-20 — D4: compare::review (spec §13.3, AK-033/AK-034) — LANDED, GREEN

What landed: `crates/gauntlet-app/src/compare/review.rs` (~700 lines with
tests):

- `ComparatorSource` trait (across_dimensions / for_dimension): routing is
  answered by the REVIEW MODE; sources only supply comparators. The
  dimension travels in the comparator's construction, NOT in the §13.2
  envelope (the envelope is the shared digested context; the question is
  not context) — so every dispatch shares one envelope digest and the
  aggregate's coherence check holds naturally.
- `review(envelope, deps, mode, budget, deadline) -> ReviewOutcome`:
  - `single`: one model comparator across all dimensions (1 budget unit).
  - `fanout`: one model comparator per instantiated dimension, merged via
    D3.
  - `adaptive`: deterministic probe per dimension FIRST; a dimension with
    a probe residual is SETTLED CONCLUSIVELY (its probe record merges; no
    model dispatch); clean probes settle nothing (absence of a diff is not
    evidence of conformance) ⇒ model fanout only over undecided
    dimensions.
  - `high_assurance`: full model fanout + adversarial pass (adversarial
    source in `ReviewDeps`).
- `ModelDispatchBudget(u32)`: count of model dispatches; truncation drops
  dimensions in CANONICAL (sorted) order and records each as an
  `UnavailableEvidence{kind: dimension, reason cites truncation}` entry —
  never silently omitted. A dropped adversarial pass records kind
  "adversarial-pass". A FULLY truncated review yields `record: None` +
  the gaps — no fabricated record (I8).
- `ReviewOutcome.record` is `Option` exactly when nothing ran; comparator
  failures are typed `ReviewError` (uncertainty never success).
- Domain→wire rebox (`to_doc`) anchors run identity + contract version
  from the envelope; digests come from the record (the coupling aggregate
  checks).
- **Deferral note recorded in the module doc**: full `BudgetMeter` port
  composition is Task 12's ledgered trigger; not wired here.

Tests (3): `adaptive_skips_settled_dimensions` (exact gate-filter name —
settled dimension gets NO model dispatch and its instrument residual
survives the merge; control: clean envelope ⇒ both dispatched),
`truncation_is_recorded` (budget-1 fanout drops canonically-last dimension
with recorded gap; budget-0 single mode records every dimension with
`record: None`; high-assurance budget-2 records the truncated adversarial
pass), `every_mode_merges_into_one_coupled_record` (all four modes couple
the envelope's three digests; only adaptive sees the instrument failure
through the scripted clean-model source — per-mode expectations kept
honest).

Gates: gauntlet-app 160/160 (compare:: 19/19); clippy -D warnings clean;
fmt clean; `just ruler` PASS; target 1.8G.

Open questions: none. D5 next.

---

## 2026-08-20 — D5: comparator adapters (spec §6.3 REQUIRED set) — LANDED, GREEN

What landed:

1. `crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs` (filled the
   stub, ~470 lines with tests): deterministic `DiffComparator`
   (`across_dimensions()` / `for_dimension(d)`). No model, no clock, no
   randomness (byte-identical output pinned; residual ids are FNV-1a over
   the §13.5 merge key). Derives residuals from facet-level reality under
   the frozen contract: non-zero exit status on an instantiated dimension
   ⇒ blocking residual citing the facet's evidence ref; both gap arrays ⇒
   major residuals, gap-bearing verdict inconclusive (never pass);
   uninstantiated-dimension facets are out of scope (documented: the frozen
   contract declares applicability; moving it mid-run is ruler movement).
   Documented kind→dimension table mapping ALL 8 FacetKind variants (test/
   build/browser/http → functional behavior; bench → performance; scanner
   → security; filesystem/versioning → evidence completeness). Emits the
   full `comparison_record@v1` document (`comparison_document`) AND the
   domain record via `compare` — all three digests coupled. Unknown
   contract version ⇒ EnvelopeInvalid. `comparator_identity.runner` =
   "gauntlet-adapter-compare-diff", model field empty (AK-028-class).
   5 tests including the UNMODIFIED shared suite via factory.
2. `crates/adapters/gauntlet-adapter-compare-model/src/lib.rs` (~780 lines
   with tests): `ModelComparator` over `Arc<dyn AgentRunner>` +
   `Arc<dyn IdGenerator>` + result root. Flow: version check BEFORE
   dispatch → critic RoleInvocationSpec (role critic;
   ContextEnvelope{canonical bytes, envelope digest}; result-contract path
   derived from the invocation id; critic/adversarial prompts; dimension
   scoped via the instruction, not the envelope) → invoke → bounded poll
   (`PROBE_CEILING = 24`, Task-8 convention documented; collect carries
   the real deadline) → read the raw document at the path → strict
   `read_result_contract` (invocation id + envelope digest validation) →
   payload must be `Comparison` → doc→domain conversion enforcing the
   §13.5 verdict coupling (an inconsistent critic record is refused).
   Taxonomy: EnvelopeInvalid / TimedOut (no terminal, runner TimedOut) /
   ComparisonFailed (no document, malformed, Failed/Cancelled) /
   ContractMismatch (wrong invocation id, wrong envelope digest, wrong
   payload variant — via `RoleResultError::failure_class`). Uncertainty
   never success. 4 tests: the composition coupling test (the scripted
   critic genuinely parses the context envelope and derives its record
   from facet reality; dispatch carries the canonical bytes + digest),
   the full derailment battery, the probe pin, and the UNMODIFIED shared
   suite via factory.
3. **Mock-composition decision (D5.4), stated:** took the IN-CRATE
   scripted critic runner path. Extending the pinned mock scenario
   dialect with a comparison-payload document kind would touch the AK-059
   pinned renderer vocabulary and risk the eight fixtures' byte-identity;
   a test-local `AgentRunner` implementation that writes
   comparison_record payloads at the contract path and scripts six
   derailments (missing doc, malformed, wrong payload, foreign
   invocation, hang, timeout) is bounded to this crate's tests and
   disturbs nothing pinned. The scripted critic is a genuine envelope
   READER (parses the canonical JSON, derives fail/pass from facet exit
   statuses, echoes the dispatch identity), so the suite's
   facet-visibility case is non-vacuous through the model adapter.
4. Codec fix in passing: `residuals.confidence` decode now accepts any
   JSON number (`as_f64`) — the canonical serializer renders whole floats
   without a decimal point (1.0 → "1"), which re-parses as Uint; the
   strict Float-only match broke the diff adapter's round-trip.

Recorded acts: NO `ADAPTER_EXTERNAL_ALLOWLIST` growth — compare-model
declares only gauntlet-domain + gauntlet-ports, exactly as forecast.

Gates: compare-diff 5/5, compare-model 4/4, ports 79/79; clippy -D
warnings clean on both adapters; fmt clean; `just ruler` PASS; target
1.8G.

Open questions: none. D6 next (p2 proof at the CLI composition root).


---

## 2026-08-20 — D6: P2 proof + evidence locators — PRESENT; focused recovery result recorded, not settlement

This checkpoint was written by the documentation finisher after interruption
recovery. It changes no implementation, test, ruler, reference, contract, or
status file.

File evidence inspected:

- `crates/gauntlet-cli/tests/p2_context_isolation.rs` currently exists as an
  untracked 511-line composition-root integration test with two tests:
  (the initial documentation inspection saw 491 lines; a concurrent
  boundary-gate correction subsequently replaced hard-coded relative paths.)
  `the_envelope_is_structurally_blind_and_carries_instrument_reality` and
  `teeth`.
- The first test composes the named mock runner, command/filesystem observers,
  filesystem evidence store, observation assembler, `EnvelopeBuilder`, and
  `DiffComparator`. It asserts six builder-side canaries are present before
  envelope construction and absent from the serialized envelope; calls
  `assert_blind`; asserts `"exit_status":1` and the failing evidence ref are
  visible; expects `EnvelopeError::ObservationDigestMismatch` after tampering;
  and asserts the diff comparison is a blocking functional-behavior residual
  citing the failing instrument, with reference/observation/envelope digest
  coupling.
- `teeth` first accepts the honest bytes, then injects
  `builder_reasoning` plus its canary and expects `assert_blind` to reject it;
  it repeats that rejection check for every configured deny-listed field name.

Focused execution evidence available to this finisher is the recovery result
communicated with this task: `cargo nextest run -p gauntlet-cli --test
p2_context_isolation` was green. Raw nextest stdout, elapsed time, and an
independently captured count were not placed in the Task-10 report directory;
the static file establishes two `#[test]` functions, but not a substitute for
that execution record. This is not a settlement claim.

Spec/brief basis: Core-spec §13.2's allowlist and digest binding; §5.3.5's
critic context-envelope digest; Task-10 brief D6 and AK-026/AK-027 locator
requirements. The test comment documents the composition-root locator as an
instance of the existing locator-materialization class.

Open residuals: the final mandatory gate transcript remains incomplete. The
parent recovery session reported a required `just boundaries` failure on eight
hardcoded-relative-path findings (including this P2 test), so no full-gate
PASS is claimed here. D1–D5 checkpoint claims and the recovery's focused gate
outcomes are recorded evidence, not settlement authority.

Recorded acts for D6: none by this finisher. No new workspace member,
`ADAPTER_EXTERNAL_ALLOWLIST` growth, boundary inventory change, reference
candidate change, or source/test change was made by this checkpoint.


---

## 2026-08-20 — Pre-critic gate correction: hardcoded relative test paths — LANDED, GREEN

Diagnosis: `just boundaries` was red solely because its literal scanner found
8 hardcoded-relative-path occurrences introduced by Task 10 test code: the
comparison-contract repository-artifact test used a manifest-relative runtime
path (reported twice by the scanner's logical-span accounting), and the P2
composition proof repeated manifest-relative repository and frozen-artifact
paths.

Correction: no artifacts or criteria changed. The app contract test now finds
the repository root by manifest ancestors and composes its comparison-contract
location from the declared `RUNTIME_DIR_NAME` and `COMPARISON_CONTRACT_NAME`
defaults. The P2 proof now has one manifest-ancestor `repo_root` helper and
named frozen-answer-key/comparison-contract helpers composed from the declared
layout constants (`WAYFINDER_DIR_NAME`, `ANSWER_KEY_NAME`,
`RUNTIME_DIR_NAME`, and `COMPARISON_CONTRACT_NAME`). Test inputs, read-only
access, and assertions are otherwise unchanged.

Fail-first evidence: `just boundaries` exited 100 before the correction, with
exactly the reported eight relative-path violations (2 in
`gauntlet-app/src/compare/contract.rs`, 6 in
`gauntlet-cli/tests/p2_context_isolation.rs`).

Green evidence, sequential: focused app artifact test (1/1); P2 composition
proof (2/2); `cargo fmt --check`; `just boundaries` (194/194). No open
residual within this correction's bounded scope. Target cache: 1.8G.


## 2026-08-20 — correction round 1

- Reconciled both blind critic reports and static audits against Core-spec §§9.5, 10.3, 13.2–13.5, answer-key AK-026/027/032/033/034, and D1–D5 brief wording. No frozen file, lock, criteria artifact, candidate, or status file changed.
- Fail-first then green: nested model-record cross-wire (`every_derailment_is_a_typed_error_and_never_a_pass`); carried dispute (`preserves_an_already_disputed_residual`); absent high-assurance adversary (`high_assurance_requires_an_adversarial_source`); truncated pass (`truncation_is_recorded`); deny token inside an allowlisted value (`a_deny_listed_name_hidden_in_an_allowlisted_value_fails_blindness`).
- Landed: generated fresh critic workspace + envelope-only permissions and process-or-stronger isolation guard; deadline is carried into invoke/collect/cancel and no-terminal/TimedOut calls cancel; nested B.4 run/iteration/reference/observation/envelope/contract plus requested-dimension binding; high assurance refuses without adversary; truncation turns record inconclusive with named gap kinds; full B.4 documents now flow directly to aggregation; inherited disputes survive; canonical serializer sorts semantic sets; declared contract lists are retained; B.4 `observed` explicitly reconciled as §13.5 observed_locator.
- Bounded blockers recorded in `correction-round1.md`: the current synchronous `Comparator` port cannot actually fan out in parallel; no frozen reference/contract digest binding can be added without changing the prohibited frozen lock/reference model; textual provenance cannot prove arbitrary reference-view content came from frozen bytes.
- Focused gates green: ports 71; app compare 22; model adapter 4; diff adapter 5; P2 CLI 2; fmt; workspace clippy -D warnings; ruler; boundaries 194.


## 2026-08-20 — correction round 2 checkpoint

Result contract: `.agents/reports/task10/correction-round2.md`.

Landed bounded corrections: port-level canonical envelope SHA-256 verification
at review, diff (`compare` and document), and model (pre-admission); P2 stale
mutation teeth; adaptive only treats non-inconclusive dimension evidence as
settling and preserves inconclusive probe evidence while dispatching model
critics; hard-gate contributor omission fails closed; build invokes the P2
structural-key check while allowing legitimate frozen prose; immediate
`ReviewOutcome` contributor B.4 provenance; model completion now requires its
declared result path and successful validated `collect`, never local-file
bypass. SHA-256 was explicitly admitted at the ports boundary and the
allowlist pin updated.

Fail-first evidence is recorded in the round-2 report: adaptive log (exit 101),
hard-gate omission (exit 101), P2 tooth with the direct diff check temporarily
removed (exit 101), and dependency allowlist pin (boundaries exit 100).
Final green: app compare 26, diff 6, model 5, P2 3, workspace check/clippy,
deny, boundaries 194/194, ruler PASS.

Open blockers explicitly recorded, not deferred silently: frozen contract and
reference-projection provenance need a candidate/schema revision; true parallel
fanout/admission and typed adversarial identity need a Comparator/controller
port revision; durable invocation persistence needs runtime composition; CLI
production reachability belongs to Task 20 (with controller ownership Task 12).
No frozen/ruler/contract/candidate/CURRENT_STATUS artifact changed.


## 2026-08-20 — correction round 3 provenance-persistence checkpoint

Result contract: `.agents/reports/task10/correction-round3.md` (section C).

Landed migration 0004: SQLite stores `runs.reference_evidence_ref`, rehydrates
fresh frozen versions via `ReferenceVersion::with_evidence`, includes the ref
in immutable identity comparison and canonical export, and reports legacy NULL
records as typed `StateStoreError::Unavailable` rather than inventing a ref.
P1 now proves record → checkpoint → adapter reopen → frozen projection from
reopened filesystem evidence. Negative teeth cover missing evidence and a
wrong stored/offered evidence binding. The application export schema accepts
and re-emits the new canonical run field.

Fail-first: focused P1 reopen failed exit 100 before the correction, showing
the legacy unbound marker. The first post-migration workspace run found the
strict export parser omission; it was corrected before final verification.

Final evidence: state SQLite 33/33; P1 4/4; P2 4/4; domain 76/76; ports 71/71;
app compare 30; diff 6; model 5; workspace 677/677; `just check`, `just lint`,
`just deny`, `just boundaries` (194/194), and `just ruler` all pass.

No frozen Wayfinder/Core/lock material, CURRENT_STATUS, or reference-candidates
artifact was changed. Task 10 remains IN-FLIGHT/BLOCKED; this checkpoint makes
no closure or settlement claim.


---

## 2026-08-20 — Round 4 residual-evidence teeth — LANDED, GREEN

- Changed `PendingComparison::collect` to retain its pending handle on an
  error. Review now actively cancels the failed primary as well as siblings;
  only cancellation refusal quarantines the lease and returns
  `RecoveryRequired`. Added deterministic forever-running-primary teeth for
  confirmed cancellation/release and cancellation-refusal/quarantine, plus a
  model-adapter bounded-hang tooth that observes direct primary cancellation.
- Moved normal/adversarial comparator-source capabilities to semantic ports.
  `ModelComparatorSource` is normal; adapter-owned
  `ModelAdversarialSource::from_model` invokes
  `ModelComparator::adversarial()` and is the distinct capability admitted by
  application `ReviewDeps`. The P2 composition construction proof wires that
  capability without introducing an app-to-adapter dependency; CLI production
  construction remains Task 20.
- Added a P2 acceptance-refusal tooth. It observes the required pin-before-state
  ordering, returns non-success on state refusal, and proves the durable export
  has the role invocation but no comparison. The pinned evidence is explicitly
  described as an orphan, not accepted evidence.

Evidence: focused app review 14/14; model adapter 7/7; P2 5/5; workspace
nextest 684/684; `just check`, `just lint`, `just deny` (known warnings only),
`just boundaries` 194/194, and `just ruler` all passed. No closure claim.


---

## 2026-08-20 — Round 9 deadline enforcement checkpoint

Landed absolute-deadline enforcement in the model comparator's pending
collection path. `ModelComparator::with_clock` accepts the semantic Clock port;
the ordinary production constructor supplies its infrastructure system clock.
`PendingModelComparison::collect` checks expiry before its first probe, before
each later probe, and before result collection. On expiry it owns and issues
cancellation before returning `TimedOut`; cancellation refusal remains
`RecoveryRequired`, allowing review's existing lease quarantine path to retain
capacity rather than release a still-live critic. The direct API's cleanup sees
the completed cancellation and does not double-cancel.

The new fail-first tooth uses a clock already beyond the deadline and proves one
admission, **zero polls**, and one cancellation. Existing deterministic tests
now inject the existing mock deterministic clock rather than relying on host
wall time.

Focused evidence: model adapter 8/8; P2 composition 5/5. Full gates after the
change: `just test` 685/685; `just check`; `just lint`; `just deny`; `just
boundaries` 194/194; and `just ruler` all passed. No frozen/reference/CURRENT
STATUS/reference-candidate material changed. This is only a checkpoint: Task
10 remains IN-FLIGHT and no closure or settlement is claimed.
