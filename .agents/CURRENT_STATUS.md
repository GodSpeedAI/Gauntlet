# Current status — HANDOFF DOCUMENT

**Last updated:** 2026-08-25 — **Tasks 2–11 CLOSED (records below). Task 12
(controller: selection, budget, diagnosis — P4, P6) is IN FLIGHT, stopped
mid-correction-round by operator direction; full resume state in
`.agents/reports/task12/` (read IN-FLIGHT.md first, then
correction-checkpoints.md).** Task-12 progress: prereg frozen before
implementation (`just prereg-check 12` PASS); four builder slices landed
(B1 domain `Spent`/`Budget::restore` + ports owner-write capability gating
(`producer_authority.rs` minting authorities, following AdmittedComparison)
+ `gauntlet-adapter-meter-memory` (renamed from budget-meter — denylist);
B2 `control::{scheduler,select,diagnosis}` + domain `correction.rs` +
durable `record_correction_selection` (migration 0008); B3 controller
composition + §11.5 disposition table + observer `run_bounded` deadlines in
all four command adapters; B4 proofs p4/p6 + teeth T1–T6); two blind
critics returned **1 CRITICAL + 8 MAJOR (A) and 8 ESCAPED areas (B)** —
prereg **R1 FIRED** as predicted (composition certifies its own admission:
rounds dead in shipped meter, degenerate composed diagnosis, non-convergence
unreachable composed, vacuous selection record); adjudication C1–C9 fix-now
/ L1–L4 later-scope; correction round 1 **completed C1, C2, C4, C3**
(fail-first evidenced) before the builder was cancelled — tree verified
green at the stop point (control:: 58/58, p4+p6 6/6, ruler PASS, boundaries
197/197 as of B4). **Everything is UNCOMMITTED in the working tree**
(one-task-one-commit lands only at closure). Remaining to close Task 12:
correction C5–C9, full gates + teeth re-record, correction-round1.md
contract, fresh blind confirmation, orchestrator gate/teeth re-verification,
reference-candidates entries (AK-037/038/039/051/052), task-12.outcome.yaml,
closure record here, single commit with explicit pathspecs. Three more
interruption instances this task (12th–14th; two mid-builder usage-limit
deaths recovered by documented finisher procedure, one
admission-refusal; one operator-directed stop). Operator events: `dbe529a`
committed the previously-uncommitted AGENTS.md/justfile/.gitignore work;
an uncommitted AGENTS.md edit renaming CURRENT_STATUS.md → current_status.yml
references (operator's, mtime 2026-08-24 19:28) is left untouched — the .md
remains the status file until the operator lands that migration.

**CLOSED (2026-08-21, Task 10 — fresh blind confirmation round 7: CONFIRM):** An architecture diagnosis
(`architecture-diagnosis.md`) established that the round-3 residuals were **one
root invariant** — Task 10 modelled provenance as value validation at each
boundary (every check compared two values the caller already held) rather than
as authority conferred by a governed operation — plus one independent bounded
defect and one honestly-ledgered later-scope half (durable adversarial
attestation has no §5.3.5/Annex-B carrier and is claimed by no Task-10 plan step
or AK). Correction round 7 landed the earned `RunComparisonAuthority`, the
application-owned consuming admission chain, `pub(crate)` sealing of
`compare::{envelope,review,aggregate}`, and deletion of
`StateStore::record_comparison`. Fresh confirmation round 4 was NOT CONFIRM
(two compiled public-API bypasses).

**Correction round 8** (`correction-round8.md`) completed the §5 architecture
with the two operator-required corrections:

- **Observation provenance now terminates in durable state.** New write-once
  `StateStore::bind_observation_bundle` / `observation_binding` (sqlite
  migration `0005_observation_bindings`, PK `(run_id, iteration_no)`);
  `RunComparisonAuthority::open` performs BOTH durable reads; `GovernedRun::issue`
  takes **no observation argument at all**, so substituting a second valid,
  correctly hashed, pinned bundle is unrepresentable rather than detected.
- **Builder-originated TEXT is not contraband.** The diagnosis's proposed T3 was
  withdrawn, not implemented: Task 9's structural claims exclusion governs, and a
  tooth asserts the *opposite* — the exact executed command reaches the
  comparator and its text is byte-identical to the builder's own declared
  self-check.
- `RunComparisonAuthority` is documented as **durable provenance authority, not
  actor-permission authority**.
- Round-4 C1–C4 closed: `accept_comparison` takes an unforgeable
  `AdmittedComparison`, minted only by acceptance, which takes a
  `ComparisonProduct` minted only by dispatch; `assemble` /
  `from_frozen_projection` / `ObservationView::new` are `pub(crate)` behind a
  single `TrustedComparisonContext::issue` that projects the view from the bound
  bytes and runs the P2 blindness assertion (moved into the port); the
  adversarial doc claims only what is true; `dispatch_context()` deleted; the
  model adapter no longer accepts its own output.

Gates green: workspace **691/691**, boundaries **194/194**,
check/lint/deny/ruler. Five teeth proven to bite by reverting the corresponding
production check.

**Correction rounds 9 and 10 closed every blocking finding, and fresh blind
confirmation round 7 returned CONFIRM** (`correction-round9.md`,
`correction-round10.md`, `final-confirmation-round7.md`).

- **Round 9** deleted `TrustedComparisonContext::contract_fixture` — the ports
  contract suite was a publicly reachable second minting path (round-5 F1).
  The suite now earns its capability through `open` + `issue` against an
  in-suite store, leaving `issue` as the only minting path in existence.
- **Round 10** closed three MAJOR findings from confirmation round 6:
  acceptance now anchors the comparator's reported `context_envelope` to the
  envelope the capability actually issued (previously both sides of that check
  were comparator-supplied); the §12.4 observation digest moved into ports and
  is re-derived from the bundle's bytes rather than believed; the durable
  binding now records the observation's CONTENT digest, not just its evidence
  ref, so bytes that are not the recorded observation are refused whoever
  serves them; and `TrustedComparisonContext::authority()` is `pub(crate)`, so
  the capability no longer leaks out of every comparator callback.
- **Round 7 confirmation** compiled an external attack crate: direct
  construction, struct literals, and capability retention are all compile
  errors; the forged-envelope, misdeclared-digest, and swapped-bytes attacks
  are all refused at runtime. All remaining findings are MINOR.

Two toothless checks the round-7 critic found were fixed in the same round:
`verify_envelope_digest` now has a biting in-crate tooth, and the CLI test
formerly named `p2_adapter_refuses_a_stale_envelope_digest` — which asserted
`is_ok()` and proved no refusal — is renamed to what it actually proves.

Recorded MINOR residuals: the public `comparator_suite` lends a fixture
capability to dependent crates; the observation-owner write
(`bind_observation_bundle`) is ungated and has no production writer yet — when
the Task-12 controller wires it, that write should be capability-gated;
blindness is field-name-only; and `assert_blind` inside `issue` is
unfalsifiable by construction.

Gates: workspace **697/697**, boundaries **194/194**,
check/lint/deny/ruler. No frozen Wayfinder material was modified and no
reference candidate was created or promoted.

Evidence: `.agents/reports/task10/{architecture-diagnosis,correction-round7,
final-confirmation-round4,correction-round8,final-confirmation-round5,
correction-round9,correction-round10,final-confirmation-round7}.md`.

**Task 9's interruption record (seventh instance of the class):** the
round-1 builder died from a **usage limit** mid-D4 (D1–D3 green and
checkpointed; D4 code landed never-executed; D5/D6 unstarted). Recovery per
`.agents/lessons/session-interruption-recovery.md`: reconcile `git status`
against the checkpoint file, dispatch a finisher that proves-what-landed
instead of re-dispatching. The finisher also surfaced two items the critics
then adjudicated: (1) `DeterministicClock` interior `Cell`→`AtomicU64`
(closed-Task-8 crate; composition-root `Arc<dyn Clock>` requirement — the
"intended production/composition path" a closed-task green had failed to
prove reachable); (2) a deliberate, pinned boundary-suite growth —
`ANNEX_B_CONTRACT_PARSERS` (4 codec modules) + the line-initial
array-element mask shape — same adjudicated class as Task 4's B.2
exemption; adversarially attacked (5a) and ledgered as D4 below.**

**Read this file first.** Then `AGENTS.md`, `.agents/specs/Core-spec.md`,
`.agents/plans/gauntlet-impl.md` — in that order.

---

## TL;DR for the next agent

Tasks **0, 1 complete** (ruler frozen — never edit `.wayfinder/`). **Task 2 CLOSED**
(`1578dde`): 183/183 arch tests, gates exit 0, residuals are recorded deferrals. **Task 3
CLOSED** (`cb2f4ac`): the full §5.2/§5.3 domain model, zero dependencies, 76/76 tests,
invalid states unrepresentable or typed-error-rejected; confirmation blind critic found
**0 criticals**. **Task 4 CLOSED** (`b27b152`): all 17 ports as traits with typed error
enums, `role_result@v1` envelope + Annex-B payloads with strict-only decoding, 17 shared
contract suites with proven teeth, gates 322/322 workspace; three blind critics, 0
criticals, correction round closed 5 residuals; the adversarial critic found **no
AK-014 evasion within its bounded search** of the Annex-B wire-key exemption. **Task 5
CLOSED** (this session):
config resolution (defaults < GAUNTLET.md < env < flags, `.env` under process env),
§8.6 defaults module, canonical serialization + `config_fingerprint`, all eight §8.5
doctor checks, repo `GAUNTLET.md`, `gauntlet doctor` CLI. Critic loop found the round's
critical — **a UTF-8 BOM silently discarded the entire config layer while doctor claimed
"contract found and parsed"** — plus AK-049 spelling/location evasions, io-error
conflation, flow-mapping duplicate-key fingerprint collision, and an explicit-`--config`
NotFound fallback; all closed with teeth, confirmation critic verified 10/11 then the
remainder. Workspace 385/385.

The history is the argument for the whole method — a green gate was wrong three times
before the blind-critic loop caught it. Keep the loop: **a task is done when its gate is
green AND a blind critic that never saw the builder's reasoning fails to break it**, with
the round-5 threat-model rule deciding what reopens vs. defers.

| Round | What happened |
|---|---|
| 1 | First cut. All five gates green. |
| 2 | Three blind critics → 34 residuals, 4 critical. Correction; committed `b69c095` (provisional). |
| 3 | Two blind critics → 19 residuals, 4 critical (what gets scanned ≠ what gets compiled). Correction; 113 tests. |
| 4 | Four blind critics → 4 critical (C-A…C-D), 18 major, 13 minor. |
| 5 | Correction closed C-A…C-D + threat-model deliverable; 159 tests. Two blind critics → 5 critical (all in the `text.rs` lexer). Work landed as commit `f85c925` — see deviation note below. |
| 6 | One real lexing pass (`text::regions`), setting-scoped defaults exemption, I8-honest declared inputs, pinned inventories. Two blind critics (critic8-evasion, critic9-vacuity) → **2 critical (E1, E2), 9 major, 13 minor**. |
| 6.1 | Closed E1 (comment tokens inside directives), E2 (`[env] RUSTFLAGS` cargo-config channel), F2/E7 (declaration-granular defaults exemption). **183 tests. 0 critical open.** CLOSED. |

### Task 4 closure record (this session, commit `b27b152`)

Recovery + layered closure: a previous session's builder left ~4.4k lines of Task-4 work
uncommitted and uncompiling (3 breaks: `evidence_store.rs:178` delimiter, missing
`contract/stubs.rs` + `suite_runs.rs`, `json.rs` keys signature, `role_result.rs` u64→u32).
Procedure per deviation 4: verify partial state compiles → recon gap reports (two explore
agents) → finisher brief → blind critics → correction round → final doc slice. Workspace
322/322 tests, arch 190/190, ports 56/56; lint/check/deny/ruler all 0/PASS.

**Adjudication worth preserving:** the arch suite's whole-literal adapter-segment rule
(= AK-014's "adapter selection by literal" tripwire) false-fired on the Annex-B.2 wire key
`"command"` in `role_result.rs` (spec-mandated JSON vocabulary, not adapter selection).
Resolution: module+key+position-gated exemption (`ANNEX_B_CONTRACT_PARSER` +
`ANNEX_B_WIRE_KEYS`, key-access positions only), fail-closed across escaped/byte/raw/
padded spellings, corners pinned; **the adversarial blind critic found no evasion
within its declared bounded search**. The exemption survived that bounded adversarial
battery; its recorded residuals: receiver invisibility incl. module-path callees
(`helpers::get("k")`), array-purpose masking inside
the one module, leading-space spellings all masked. Any future widening is a deliberate
edit to walk.rs's audited inventory.

**E4/F1/F3 re-litigation (ledger-mandated on arch-suite touch): ALL CLOSED**, each with
failing-before evidence (see commit message): E4 timeout class accepts
`return`/block-value positions; F1 stopword exemption yields to command SHAPE (flag token
or shell metachar); F3 `=`-less `.env.example` lines are findings, not skips (the last
fail-open parse). Recorded false-positive shapes: F1 — a prose string citing a flag
(`"git push --force is dangerous"`) flags; E4 — a count returned from a
duration-hint-named fn (`fn timeout_retry_limit() -> u64 { return 5; }`) flags. Both err
toward flagging, by design, documented in literals.rs docs.

**Accepted-with-doc (critic adjudications, 0 criticals):** `IdGenerator` has no error
enum (infallible per §6.2; entropy failure is an adapter concern) · `invoke` has no
deadline parameter (admission is non-blocking by contract; `collect`/`cancel` carry
deadlines) · `RoleInvocationSpec.budget_slice: Budget` (whole budget carried; meter
derives slices) · no vacuous getter cases added for last_offset/hooks/backend_identity ·
8-draw id-collision sampling · probe budget default instant for in-repo teeth runs (real
adapters pass caller-supplied patience). **Deferred to later tasks:** EventLog
fsync/durability + crash-injection (Task 6, per plan); runner capability-vs-tier
dispatch check (Task 14, the `Isolation` ladder is in place).

### Task 6 closure record (this session)

- **Interruption recovery (fifth instance of the class):** the round-1 builder
  completed code + checkpoints; the session died before its final report, the
  teeth runs, and the critic loop. Recovered per the documented procedure:
  reconstruct from files (never memory), verify gates personally (413/413),
  run the missing loop steps. The pattern is now lessoned:
  `.agents/lessons/session-interruption-recovery.md` (+ AGENTS.md failure-log
  rule: reconcile `git status` vs this file before treating any task as
  unstarted; IN-FLIGHT line at dispatch time — first use was this session).
- **Artifact:** rusqlite(bundled) EventLog+StateStore (INSERT/SELECT only,
  `synchronous=FULL`, in-repo append-only migrations, typed `Damaged`),
  content-addressed write-once FsEvidenceStore (fsync+rename, pins, no
  overwrite/delete API), app-layer canonical `state::export` service, real
  process-abort crash helper. Recorded acts: `ADAPTER_EXTERNAL_ALLOWLIST`
  += rusqlite+sha2 with pin-test update; **deviation:** rusqlite lives in the
  adapter manifest only (the root-table form is denylist-denied); deny.toml
  needed no edit.
- **Teeth (orchestrator-run, all reverted):** export ORDER BY removal →
  golden byte-diff; §16.3 reorder in the crash script → half-settle
  assertion; rusqlite in gauntlet-app → boundaries fails twice (allowlist +
  vendor-identifier). Honest limit: `synchronous=OFF` is not demonstrable via
  process abort (page cache survives process death) — pragma configuration is
  the mechanism, recorded, echoing critic-A's fsync-proof observation.
- **Critic yield:** A (spec): 2 major (commit_settlement zero callers;
  non-zero-spend resume impossible), 14 minor. B (adversarial): 18 BITE /
  1 EVADE-major (same zero-caller finding, convergent) / 11 EVADE-minor.
  Correction round (13 items, fail-first ledger in
  `builder-checkpoints.md`): commit_settlement direct coverage + packaged-
  primitive crash script; negative timestamps → typed damage (4 sites);
  settlement-row offsets cross-validated against the log (this also makes a
  naive §16.3 swap inside commit_settlement structurally refused — the row
  would cite an offset with no event); half-settled error names the durable
  offset; Damaged→Corrupt through the store surface; offset-gap detection;
  record_run full-identity compare; get() re-hashes against the address;
  symlinked object paths refused; kind-sidecar conflict refusal + unique
  temps; `at` fields type-checked; **the composed export stack test** (real
  adapter bytes through canonical re-export, at the CLI composition root —
  capability-named re-exports `EmbeddedStateStore`/`EmbeddedEventLog` per the
  denylist's own prescription); trivia. Confirmation critic C: **CONFIRM**,
  13/13 CLOSED, 0 new critical/major, 2 new minors recorded.
- **Final gates (orchestrator-run):** state-sqlite 31/31, evidence-fs 8/8,
  gauntlet-app state:: 8/8, gauntlet-cli 2/2, check/lint/deny 0, boundaries
  190/190, ruler PASS, workspace **433/433**.
- **Reference candidate (recorded-act):** AK-054's locator
  `tests/contract/state::deterministic_export` cannot be materialized under
  the escapes rule → second entry in
  `.agents/reports/reference-candidates.md`, citing the three real tests
  (app filter test, adapter golden, CLI stack test).

### Task 9 closure record (this session, 2026-08-20)

- **Interruption recovery (seventh instance of the class, new subclass —
  usage limit):** the round-1 builder completed D1–D3 with checkpoints
  (ports codec 7/7, five adapters 20/20, app `observation::` 23/23) and
  died mid-D4; the two proof files + cli dev-deps landed NEVER EXECUTED.
  Recovered per the documented procedure by a finisher: prove-what-landed
  (p3/g6 first-ever runs), D5 recorded acts, D6 gates — which surfaced a
  **red boundaries gate** the dying builder never ran (92 hits: B.3 wire
  key `"command"`, kind tokens in tests, one adapter name in a ports doc).
- **P3 scope:** the plan assigns Task 9 the real-observation, gap-recording,
  and structural claims-exclusion slice. P3's authoritative predicate also
  requires that comparison operate on the recorded instrument failure. Task 9
  closes the observation/acquisition portion; Task 10 must compose the
  comparison-consumption portion through its envelope (D3), carrying
  facet-level observation reality rather than only digest/gap projections.
- **Artifact:** ports `facet_evidence@v1` strict codec (per-facet env
  fingerprint lives in the evidence document — the §12.2-vs-B.3
  reconciliation, documented); five facet adapters (`filesystem` bounded
  walk with named caps + `\xNN` manifest determinism, `git`→Versioning
  with the documented nonzero-exit exception, `build`, `test`, generic
  `command`) — all passing the UNMODIFIED Task-4 shared contract suite,
  each with an AK-028 provenance test; `gauntlet-app observation::{bundle,
  digest, input, assembly}` (typed B.3 mirror with strict decode, §12.4
  digest over timestamp-stripped normalized bytes, structural claims
  exclusion — `ClaimsFacet` has no path into `InstrumentFacets`/
  `VerificationInputs`, single §21.3 gap-population site feeding BOTH
  arrays, provenance cross-check); composition-root proofs
  `p3_real_observation` (P3 observation half; the lying builder's declared
  self-check re-run as a real `FacetKind::Test` instrument → facet exit 1 +
  failing evidence while the claims facet asserts success; claims canary over verification
  inputs; digest stability/sensitivity) and `g6_hide_failed_observations`
  (spawn-failure and store-write-failure → both gap arrays exactly once,
  digest covers gap entries, all-healthy = explicitly empty arrays).
- **Teeth (orchestrator-run, all reverted, `task9/teeth.md`):** T1 delete
  the `unavailable_evidence` population → 4 tests fail (app + g6); T2 add
  a "helpful" `claims_context` field to `VerificationInputs` → both canary
  tests + the p3 PROOF fail; T3 empty env fingerprint in the codec →
  round-trip (strict decode refuses) + adapter provenance fail; T4 include
  `observed_at` in the digest → both determinism tests fail. Note: teeth
  on untracked files revert by manual inverse edit, not `git checkout`.
- **Critic yield:** A (spec): 0 critical, 3 major (AK-028 locator —
  already dispositioned by the reference-candidates entry, which A
  independently confirmed without seeing the ledger; subprocess
  unboundedness; P3's comparison half uncomposed), 10 minor. B
  (adversarial): 0 critical, 5 major (weak cross-check; env-fingerprint
  semantics; symlinked-root/`..`-ref containment absence; no process
  timeout), 10 minor. Within its static, typed scopes, B found no break in
  claims-facet content exclusion (not builder-authored invocation/failure-detail
  strings), both-array population for returned `ObserverError`s (not hangs,
  panics, or assembly aborts), or normalized-byte canonicalization (not digest
  recomputation at consumption). Correction C1–C10 (fail-first
  per item, `task9/correction-adjudication.md`): workspace containment in
  all five adapters; mismatch→both arrays; cross-check identity/command/
  exit/window agreement incl. the None-bypass; three typed plan refusals
  (empty plan, fingerprint shape, empty revision); manifest error-line
  cap; non-UTF-8 escape determinism; **the maskable-intersection pin**
  (`segments ∩ ANNEX_B_WIRE_KEYS == ["command"]` computed from the real
  member list — a colliding adapter name now fails a pin); spawn-failure
  taxonomy unified to `Unavailable`; the split-argument mask residual
  pinned-as-masking; the verdict-carrier doc note. Confirmation critic:
  **CONFIRM**, 10/10 CLOSED, 0 new critical/major, 2 minors (MINOR-1 doc
  accuracy — closed by orchestrator micro-fix; MINOR-2 informational).
- **Final gates (orchestrator-run, post-correction):** p3 2/2, g6 3/3, app
  `observation::` 29/29 (filter non-empty), ports observer 9/9, adapters
  29/29, arch 193/193, workspace **621/621**, check/lint/deny 0,
  boundaries **193/193**, ruler PASS.
- **Recorded acts:** adapter-layer `sha2` reason extended (env
  fingerprints + manifest digests); three reference-candidates entries
  (AK-028/029/030, locator-materialization class); boundary-suite growth
  (4-module parser list + line-initial element shape + 3 new pins/mask
  tests — adversarially reviewed, residual ledgered as D4).

### Task 8 closure record (this session)

- **Interruption recovery (sixth instance of the class):** the round-1
  builder completed all four code slices with checkpoints (61/61 per its
  last checkpoint) but died before D6 gates, its final report, teeth, and
  the critic loop. Recovered per the documented procedure: ruler PASS →
  gates run personally (61/61 + 3/3 + check/deny/boundaries green, **lint
  RED** — 2 clippy errors + fmt drift the never-run D6 would have caught)
  → missing report slice written from the checkpoint contract → loop
  resumed at lint closure, not re-dispatched.
- **Artifact:** `gauntlet-adapter-agent-mock` — `DeterministicClock`
  (virtual monotonic-max time), `SeededIdGenerator` (FNV-1a → splitmix64 →
  Crockford windows, derivation documented), Annex-E hook selection
  (fail-closed `UnknownClockToken`; pure selectors, no env mutation —
  edition-2024 `unsafe` avoided by design), strict YAML-subset scenario
  parser (position contract on one convention; leftover lines typed;
  **`MAX_SCENARIO_DEPTH = 64`** after the critic round — the fail-first
  run reproduced a live stack overflow, the recorded Task-7 defect class),
  Annex-B document renderer with five derailments, `MockAgentRunner`
  (completion only through `read_result_contract`; honest capabilities:
  `isolation: None`, `max_depth: 0`, File-delivery emulation with
  `written_document` forensics; duplicate admission typed-refused),
  `MockRunnerFactory` (total `RunnerScript` mapping), the eight REQUIRED
  fixtures at AK-059's exact locator spelling (child behavior only), the
  AK-059 suite pin, nine playback tests each asserting a distinct state.
- **Teeth (orchestrator-run, all reverted, `task8/teeth.md`):** T1
  failure-class neutering → suite pin + playback fail (plus a documented
  nextest per-binary-cancellation observation); T2 REQUIRED fixture
  removed → inventory pin fails typed; T3 isolation inflated → honesty
  test fails. **Honest observation recorded:** the shared suite does NOT
  pin adapter capability honesty — Task-14 deferral class.
- **Critic yield:** A (spec): 1 major (stall+effects dialect/runner
  divergence), 7 minor. B (adversarial): 2 major (duplicate-invoke
  terminal resurrection; parser depth/memory bound), 6 minor; could NOT
  break classification, determinism, or termination (bounded static
  battery). Correction round C1–C6 (fail-first per item) + confirmation
  critic: **CONFIRM** — all CLOSED but C5-with-residual N1 (write-knob
  off-by-one at cap), N1 then closed by micro-fix
  (`WRITE_PROBE_CEILING = PROBE_CEILING - 1`).
- **Final gates (orchestrator-run):** mock **68/68** (~35 MiB peak RSS),
  ports agent_runner 3/3, workspace **547/547**, check/lint/deny 0,
  boundaries **190/190**, ruler PASS.
- **Plan corrected (recorded):** Task-8 gate filter
  `contract::agent_runner` matched zero tests — the Task-4 suite lives at
  `contract::suite_runs::agent_runner_*`; plan now spells the working
  substring filter.

### Task 7 closure record (this session)

- **The journey matters:** round-1 + finisher sessions died repeatedly to the
  ESC-002 host-OOM spawn deaths (write-only finisher-2 landed D4R–D8 + report,
  zero execution). Orchestrator gates then exposed a **runaway-memory defect**
  the never-run test suites had been holding: `parse_manifest`'s tree
  traversal was written against two conflicting position conventions
  (`parse_tree` passed `pos` AT the key line, `mapping` AFTER it); the
  nested-mapping arm returned an empty map consuming zero lines, so
  `parse_tree`'s `while pos < lines.len()` pushed `(key.to_owned(), node)`
  forever — the operator's instrumented diagnosis (1.7→4.9 GiB RSS, worker
  thread committing pages) matched exactly. **Root cause + fix + full report:
  `.agents/reports/task7/runaway-memory-defect.md`.** D3's manifest tests and
  D4's provider tests had never executed even once — "landed+compiling" was
  never "passing". Post-fix: the contract-suite test runs in 0.02 s at ~30 MiB
  peak RSS.
- **Artifact:** `gauntlet-app reference::{normalize,parse,manifest,provider}`
  (script-exact normalization with pinned-output tests; strict AK-block
  parser with per-block isolation; manifest dialect parser — traversal
  rebuilt on one position contract; production `FileReferenceProvider` over
  the frozen `.wayfinder/` pair with the freeze tripwire, Unavailable ≠
  ReferenceMutated, and the F4 mid-flight guard); `gauntlet reference digest`
  CLI; doctor check 2 fully closed (strict lock read, 70/70 unit schema,
  digest-vs-lock on **both** ruler halves — key AND manifest); composed P1
  proof at the composition root (real FsEvidenceStore + EmbeddedStateStore);
  reference-candidates entries for AK-021, AK-022/023/024, AK-025.
- **Teeth (all bite, all reverted, full re-verify green):** boundary-call
  removal → `digest_rechecked` fails; per-line trim → both pinned normalize
  tests fail AND the script-vs-CLI diff gate **disagrees** on a synthetic
  trailing-space fixture (`5518ad7a…` vs `e49c81e2…`) — the composed gate is
  now extended beyond the frozen key (which carries no trailing-whitespace
  lines) by diffing a `printf 'alpha \nbeta\n'` fixture too; F4 freeze-guard
  removal → freeze tripwire test fails; doctor digest-clause neutering →
  moved-key test fails; manifest-half compare neutering → moved-manifest test
  fails.
- **Critic yield:** A (spec): 4 major (AK-025 unrecorded; doctor manifest
  clause shallow; manifest_digest unverified; teeth harness-only), 5 minor.
  B (adversarial): **1 critical** (reachable `unreachable!` panic via
  `">= 0 settlements"`), 2 major (manifest_digest — convergent; dead lex tab
  guard), 14 minor/bounded. B could NOT break normalize byte-parity against
  the script (bounded battery: BOM, lone CR, whitespace tails, NUL, U+2028)
  nor construct a non-terminating traversal post-fix.
- **Correction round (C1–C7, each with a discriminating test):**
  reliability `< 1` refused at parse (`ReliabilityBelowOne`) — the panic path
  is structurally closed; `manifest_digest` required + verified in check 2
  (both halves fail `REFERENCE MOVED`); the tab guard made live (tab anywhere
  in the space/tab run; NBSP not admitted as indentation — pinned); AK-025
  candidates entry; empty flow entries refused; P1 confinement assertion tied
  to the evidence-store root; provider header made accurate. One never-run
  test pin corrected with evidence (`UnknownKey` flavour `line: 2`→`3`;
  sibling test pins the same physical line at 3). Confirmation critic:
  **CONFIRM**, 0 open critical/major, 3 minors (all closed: NBSP fix + pin,
  doc comment, this ledger).
- **Final gates (orchestrator-run):** gauntlet-app 112/112 (manifest 12,
  doctor 25, reference:: 38), p1 4/4, workspace **479/479**, check/lint/deny
  0, boundaries **190/190**, ruler PASS, `just doctor` exit 0, both digest
  gates AGREE (frozen key == lock `sha256:ccc7c99b…`).
- **Deviation recorded:** orchestrator-implementer for the defect fix and the
  correction round per ESC-002's standing operator authorization (delegation
  spawn-deaths continued; critics still ran blind via explore agents). The
  operator's untracked `.cargo/config.toml` was reduced to the
  suite-sanctioned `[build] jobs=1` + `[profile.dev]` form by operator choice
  after `just boundaries` correctly flagged its `[target]` linker/rustflags
  section (AK-001 compile-channel rule).

### Task 5 closure record (previous session)

- Builder round 1 + K-correction + M1/M2 micro-correction. Reports:
  `.agents/reports/task5/{builder-round1.md, builder-checkpoints.md, critic-a-result.md,
  critic-b-result.md, critic-c-result.md}` (critic1-spec.md is a partial pre-interruption
  evidence file, superseded).
- **Interruption recovery:** the round-1 builder completed; the session was interrupted
  before critics ran; the M1/M2 agent was cancelled mid-work (code landed, proof missing)
  — recovered by a finisher that only added tests/teeth/report. Both recoveries followed
  the documented pattern: reconstruct state from files, never from conversational memory.
- **Critic yield:** A (spec): 1 major (AK-049 single-spelling probe), 2 minor. B
  (adversarial): 1 critical (BOM → silent empty config layer), 2 major (io-kind
  conflation; flow-mapping duplicate keys → fingerprint collision), 5 minor. C
  (confirmation, post-correction): 10/11 CLOSED, no regressions; the B-R2 remainder
  (explicit `--config` NotFound → silent defaults) closed by M1; unreadable policy dir
  never-clean closed by M2 (both with live CLI teeth and bite-proven tests).
- **Adjudicated, not defects:** skipped-sub-under-check-pass documented in `report.rs`
  (a check's pass relies only on its pass sub-clauses); policy-stem contract matches
  exactly `irreversible` (case-insensitive, any extension, both effective and default
  dirs, `symlink_metadata`) — `irreversible.json.bak`/`.irreversible` are out of contract
  by decision; `max_cost_usd` Money kind (Int|Str) accepts the spec §8.2 example.
- **Recorded-act edits:** `sha2` added to `APPLICATION_EXTERNAL_ALLOWLIST` (config
  fingerprint; reused by Task 7's reference digest) with the pin test updated to the new
  pinned state. AK-049's answer-key evidence locator (`tests/e2e/doctor::…`) cannot exist
  under the escapes rule → first entry in `.agents/reports/reference-candidates.md`
  (dated, cites the real in-crate tests). **The ruler was not touched** — `just ruler`
  PASS verified after every round.
- **Honest unavailables (in doctor.json with reasons):** check 2 deep unit-schema +
  digest-vs-lock (Task 7 parser/`verify_unchanged`); check 3 runner resolution (Tasks
  15–16); check 4 capability-vs-tier (Task 14); check 6 tool binding (Tasks 15–16).
- **Orchestrator verification (independent):** full gate exit 0; focused 62/62;
  workspace 385/385; `just doctor` exit 0 on the repo; BOM tooth, `irreversible.yaml`
  tooth, explicit-missing-`--config` tooth, chmod-000 policy-dir tooth all re-observed
  from the transcripts and spot-probed live.

### Task 3 closure record (this session, commit `cb2f4ac`)

Two-phase build (first builder hit a usage limit mid-task with unwired modules —
"compiles clean" was hollow until lib.rs declared the modules; the finisher caught that,
wired everything, fixed 7 latent defects, added run/iteration/capability). Round 2 closed
both criticals — `RoleId::Custom` grammar bypass and the foreign-run settlement hole
(§5.3.2 "in this run"), found independently by both critics — plus four majors
(§13.5 blocking-forces-fail, AK-055 duplicate-RunId dedup, widened no-bool guard,
irreversible sandbox probe). A micro-fix added three missing public re-exports found by
the orchestrator's outside-the-crate probe, plus a permanent public-API settle test.
Confirmation critic (criticC): **CONFIRM, 0 critical / 2 major / 7 minor.**

**Orchestrator verification:** gates green (`gauntlet-domain` 76/76; check/lint/deny/
boundaries 183/183/ruler all 0); inverted outside-the-crate probe asserting the forbidden
`Unit{irreversible, critic}` constructs FAILED with `Err(IrreversibleRequiresHuman{..})`.

**Task-3 deferral ledger:** evidence locators for AK-003…007 resolve at Task 7 (parser
task) — same provisional-status class as AK-001/002/016/019; `SettlementRecord`
store-binding at Task 13 (SettlementService enforces §16.1; the domain type cannot see
the store); 7 minors in `.agents/reports/task3/criticC-confirmation.md` (settle
from-status table, `DraftUnit` evidence drop, duplicate variation axes counting toward
promotion, `attach_iteration` ordering, source-scan guard evasion routes — `into_bool`/
`fn ok` needles, `pub struct …(pub String)` opacity escape, `*self = Self{…}` run-pin
escape — and settle doc under-listing §16.1 deferrals). Close the guard-evasion minors
opportunistically at the next domain-crate touch.

### Round-6/6.1 closure record

- Builder reports: `.agents/reports/task2/builder-round6.md`, `builder-round6.1.md`.
  Critic reports: `critic8-evasion.md`, `critic9-vacuity.md` (raw, unfiltered).
- **Orchestrator verification (independent of builders' own runs):** full gate
  `just check && just lint && just deny && just boundaries && just ruler` exit 0
  (183/183); repo-level teeth, each failing `just boundaries` with the suite's own
  violation message then reverting byte-identical: round-5 set (denied dep key; plain
  registry dep into domain; crate-depth `.cargo/config.toml` `--extern` rustflags;
  `cfg_attr` path escape; model-id literal) and round-6.1 set (E1 comment-in-directive
  payload; E2 `[env] RUSTFLAGS` payload — flagged as "the `[env]` spelling of the
  rustflags key this check already denies (AK-001, spec §24.3)"; F2 `localhost:8080`
  endpoint on a setting line inside a fully-warranted `defaults.rs`).
- Known-gaps the 6.1 builder declared honestly: cross-line directive heads with trailing
  comments (`include! /*c*/`⏎`("…")`) still defeat head-join (declared in lib.rs
  residual list); `[env]` names reaching the compiler only transitively (`PATH`,
  `LD_LIBRARY_PATH`) not denied — recorded policy call, not taken.

### Deferral ledger (recorded, never silent — from critics 8/9, post-6.1)

**Task-11 deferrals (same upgrade rule — a deferred finding blocks the
moment a task walks into its trigger):**

- **Ungated evidence-production write + declared-iteration validation at
  write time** (critic-b MAJOR-3, adjudicated C7): any in-process holder of
  the state store can record a production for any store-resolvable bytes;
  write-once per (run, ref) holds, but the write itself is not
  capability-gated and the declared iteration is not checked against the
  run's durable iteration. Task 10's `bind_observation_bundle` carries the
  identical recorded residual. Trigger: **Task 12** — capability-gate BOTH
  writes when the controller wires their production writers (evidence-owner
  and observation-owner roles).
- **Chain-read → instrument-read race** (round-2 residual, honestly
  documented in-code, §22.2): the file at the locator could change between
  the chain's pre-dispatch digest read and the instrument's read. Trigger:
  any requirement for race-freedom — the recorded closure is the option-(b)
  Verifier port revision (instruments consume store-served bytes).
- **Workspace-root pairing is convention-bound at the library seam**
  (confirmation round 2 NOTE-2): `for_run` accepts any root; nothing in the
  library forces it to equal the root the adapters were constructed with.
  Same §7.1 composition-root trust position as the ladder. Trigger:
  **Tasks 12/20** production wiring must keep them one fact (the composed
  fixture already does).
- **Production-row crash-window replay** (critic-a unimplemented-3): the
  window between the durable `EvidenceProduced` event append and the row
  insert is named in the error string; no replay code exists. Trigger:
  **Task 13** recovery composition (same class as the settlement
  commit-window discipline).
- **Ladder rungs 5–9 and 11** (broad test, static analysis, benchmark,
  runtime probe, visual diff, human): §6.3 RECOMMENDED; visual-diff-shaped
  evidence currently falls to the model rung. Trigger: **ports revision**
  or the task that needs them.
- **Locator "contract-defined" matching for non-deterministic kinds**
  (critic-a MINOR-4): exact-only today (stricter, not looser). Trigger:
  **ports revision**.
- **`produced_at` integer vs Annex-B string placeholder** (critic-a
  MINOR-6): annex samples use `""` placeholders; the codec is stricter.
  Trigger: **docs/schemas** (Task 17/27).
- **Containment is path-string level, not inode level** (confirmation
  round 2 NOTE-1): a locator symlink resolves outside the workspace root
  for BOTH readers identically; no false outcome is mintable through it;
  requires workspace write access (§22.2). Recorded so nobody reads
  "escape-refused" as inode-level containment.

**Task-9 deferrals (added this session; same upgrade rule — a deferred finding
blocks the moment a task walks into its trigger):**

- **Observer process timeouts + subprocess output-size caps** (B-7a/A-M2,
  MAJOR): `observe()` blocks unboundedly; a lying builder declaring
  `sleep infinity` as its self-check hangs the run instead of producing a
  gap record. Extends the standing Task-8 "deadlines noted, not consulted"
  convention to observers explicitly. Trigger: **Task 12** (controller
  deadline composition — §18.4 hang disposal needs the controller anyway).
- **Env fingerprint vs actual child environment** (B-3b core, MAJOR):
  fingerprints snapshot the observer's construction-time env; children
  inherit the live process env; `PATH` (binary resolution) is outside the
  fingerprint; `from_parts` declares rather than captures. Shape now
  validated at assemble (C4). Trigger: **Tasks 15–16** (tool binding).
- **Digest recompute at the consumption boundary + envelope widening**
  (B-3c + A-M3, MAJOR): no production caller recomputes
  `observation_digest`; `ComparisonEnvelope` carries the domain bundle
  (digest + gaps, NO facets) so a comparator wired today could not see the
  recorded instrument failure — P3's "comparison operates on it" half
  composes in Task 10. Trigger: **Task 10** (its own plan section).
- **Masked selection-shaped spellings in the four codec modules** (B-5a,
  MINOR–MAJOR): split fn-arg, purpose-blind array element,
  receiver-invisible `.get` mask inside `ANNEX_B_CONTRACT_PARSERS`; now
  PINNED-as-masking (C9) with the maskable intersection pinned (C7).
  Trigger: **next arch-suite touch** — re-litigate alongside E4/F1/F3.
- **Domain view drops facets/identity/revision** (B-8a): `as_domain_view`
  carries digest + gap arrays only. Trigger: **Task 10 / ports revision**.
- **Port-shape gaps** (A-m7/m9/m10): `Option` facet fields permit
  incomplete facets; `FacetKind` vocabulary narrower than §12.2's wider
  class list (lint/static, runtime_probe, logs, metrics, security_scan,
  external unrepresentable); command adapter's kind is caller-bound with
  no type restraint. Trigger: **ports revision** (facet Optionality) and
  **production wiring** (kind↔command binding, Task 12/15).
- **Command adapter lacks an in-crate spawn-classification test** (confirm
  nano; property discriminated cross-crate by g6 face 1). Trigger: next
  observe-command touch.
- **Evidence timestamps digest-covered only transitively** via
  content-addressed refs (A-m8) — inherent, documented, no trigger.

**Task-8 deferrals (added this session; same upgrade rule — a deferred finding
blocks the moment a task walks into its trigger):**

- **Shared suite does not pin adapter capability honesty** (T3 teeth
  observation): `the_mock_runner_passes_the_unmodified_shared_contract_suite`
  PASSED with `isolation` inflated to `Process`; only the adapter's own
  in-crate honesty test caught the lie. Trigger: **Task 14** (the
  capability-vs-tier dispatch check is already a Task-14 ledger item —
  compose them).
- **Symlink pre-placement under a test-provided workspace root** (B-m3):
  both effect guards are lexical; a hostile filesystem under the root
  defeats them. Outside the adapter's documented threat model; §17.5
  placement is the primary guard (same accepted class as Task-6's fanout
  symlink residual). Trigger: mock effects ever resolving against a
  non-test-provided root.
- **`documents` map keyed by contract path** (B-m6): two invocations
  sharing one result-contract path clobber each other's raw text
  (forensics only; classification holds per-entry). Trigger: a second
  consumer of `written_document` or Task-9+ forensic needs.
- **Runner deadlines modeled, not enforced** (A-m2): probe counts stand in
  for §10.3 rule 5 deadlines — documented time-free design, consistent
  with the port-wide "deadlines noted, not consulted" convention. Trigger:
  any task composing real deadline behavior (Task 12 controller).

**Task-7 deferrals (added this session; same upgrade rule — a deferred finding
blocks the moment a task walks into its trigger):**

- **Doctor check 2's manifest clause stays shape-only** (critic-A M2 / B-F11):
  the strict `parse_manifest` exists and runs on the provider path, but check
  2's manifest clause does not deep-validate (and its `top_level_mapping_keys`
  passes an all-indented file as "0 keys"). The doctor Scratch fixture's
  manifest would fail deep parse. Trigger: **next doctor touch or Task 12**
  (config/reference surface work) — deepen the clause and promote the fixture
  to a full manifest.
- **Iteration-boundary / settlement teeth are harness-only** (critic-A M4; the
  plan's own done-when is not dischargeable as written until then): no
  production `verify_unchanged` caller exists; both digests live in the typed
  error but are not persisted as evidence. Trigger: **Tasks 12–13** (existing
  substrate-level ledger entries for AK-023/AK-024 carry this).
- **`ReferenceVersion` lacks the spec's `evidence_ref` field** (critic-A m1):
  linkage is by the `sha256:<hex>` convention. Trigger: **Task 13** settlement
  persistence or the ports revision.
- **`Unavailable` vs `ReferenceMutated` boundary behavior for broken artifacts
  is undefined at the controller** (critic-A m3): the classification is
  deliberate and pinned; what the controller DOES with `Unavailable` is not.
  Trigger: **Task 12** controller error policy.
- **Manifest dialect leniencies (B-F4/F5/F7/F8/F9/F19):** `key:` values keep
  extra leading whitespace; `digest:` bare-null spelling refused while the
  header allows it; `depends_on` existence/acyclicity unchecked (F7 belongs
  to the **Task 12** scheduler); `#`-leading folded-body lines dropped;
  bare-dash error locus misleading; `version: +1` accepted and duplicate-root
  message mislabels. Trigger: **next manifest/parse touch** — batch-close.
- **Doctor/lock message nits (B-F10/F12/F13):** "carries no unit_count" for
  malformed (not absent) counts; lock path fields whitelisted but
  unverified; CLI `--path --path` takes the second flag as a value. Trigger:
  **next doctor/CLI touch.**
- **Provider-level empty-ruler legality (B-F18):** `parse_answer_key("")` →
  zero units, freeze pins sha256(empty). Compensating control is real (doctor
  count clause + zero-blocks schema clause). Trigger: **Task 12** if the
  controller ever trusts `verify_unchanged` solo.
- **Boundary-scanner residuals demonstrated by this round's shape fixes
  (B-F16, recorded for the next arch-suite re-litigation):** tier vocabulary
  in fn-expression position passes the declaration-only tier class
  (`field_keys()`); `format!("{}/{}", a, b)` construction passes the
  relative-path literal class (production doctor code already used the
  shape). Not widened by this task; the class docs' recorded residual
  stands.

**Task-6 deferrals (added this session; same upgrade rule — a deferred finding
blocks the moment a task walks into its trigger):**

- **Non-zero `budget_spent` cannot resume** (critic-A MAJOR-2): domain `Spent`
  has no value-carrying constructor, so once `Budget::charge` runs and a
  checkpoint is written, `resume` refuses `Corrupt` (loudly, data preserved
  and exported). Trigger: **Task 12** (first production `charge` caller) —
  needs a domain reconstruction slice (constructor + tests), not an adapter
  patch.
- **Intra-call §16.3 swap discrimination** (residual of the round's reopening
  major): a naive statement swap inside `commit_settlement` is now
  structurally refused (the row would cite an offset with no event — the A3.4
  cross-validation bites), but a rewrite that also defeats the cross-check
  needs mid-function abort injection. Trigger: **Task 13** P7
  (`p7_recovery::commit_order`, AK-041's declared locator).
- **Adapter allowlist granularity** (B-A5.2): entries pin names, not
  version/features — any adapter may declare `rusqlite` with any features
  (e.g. `load_extension`). Trigger: **next arch-suite touch** (re-litigate
  alongside E4/F1/F3 per the standing rule).
- **Checkpoint-only run projection indistinguishable** (A MINOR-3): callers
  can only string-match `PROJECTION_INTENT`; frozen suite forces the
  projection to exist. Trigger: **ports revision** (enum/type surface is
  frozen this task).
- **Replay reads the whole log from offset 0** (A MINOR-4): §18.3 names the
  checkpoint offset as the start; the superset replay is idempotent so the
  outcome is safe. Trigger: **Task 13** resume composition.
- **Evidence-fs crash injection absent** (A MINOR-8): temp+fsync+rename
  durability is mechanism-only, no kill test. Trigger: **Task 25** recovery
  E2E (or next evidence-fs touch).
- **Settled_at is caller-attested** (confirm-critic R1) and **non-timestamp
  integer coercions survive as `unwrap_or(0)`** (R3, notably a tampered
  `settlements.event_log_offset` reads 0, not Corrupt) — minors, close
  opportunistically at the next state-sqlite touch.
- **Fanout-directory symlink containment** (B-A1.4 remainder, accepted):
  defense-in-depth only; §17.5 placement (evidence root outside every
  builder-writable path) is the primary guard. **Deadlines noted, not
  consulted** (A MINOR-5): documented port-wide convention. **OR IGNORE
  widening hazard** (B-A3.3 note): any future constraint added to
  `settlements` widens what OR IGNORE swallows — pinned nowhere; revisit if
  the schema ever grows a constraint.

**Majors (7):** E3 layer classification trusts `[package].name` while scanners trust
directories (adversarial) · E5 UNC/verbatim Windows paths mis-resolve to "inside"
(adversarial; escape-resolver half undeclared) · E6 `rust-toolchain*` checked only at repo
root (adversarial) · F4 evidence-locator pin checks substrings, not which `check_*` a
locator calls (accidental+adversarial).
**Closed since round 6.1:** E4 (timeout class), F1 (command class), F3 (env
fail-open parse) — all closed during Task 4's arch-suite touch (see Task-4 closure).
**New record (Task 4):** F1/E4 false-positive shapes (prose string citing a flag;
count from a duration-hint-named function) documented in `literals.rs` docs — known,
erring toward flagging.

**Minors (13):** E8–E12 (cargo-config discovery skips `target`/`.git` dirs and symlinked
dirs; `terminates_statement` has no char-literal case; dotted `[workspace.dependencies.x]`
header blanked; locator bodies unpinned; byte-string literal assembly) · F5–F12 (timeout
hint-word narrowing; tier class const/static-only; enumerated executable heads; root-table
header exact-match; walker and "independent" listing flatten IO errors in lockstep;
adapter selection by first segment; scope-agreement loops true by construction;
repo-level greens currently evidence over doc-only stub crates).

Rule for these: a deferred finding **upgrades to blocking** the moment a task walks into
its trigger (e.g. F2 was deferred-class until Task 5's defaults module made it live — it
is now closed). Re-litigate E4/F1/F3 first time the arch suite is touched for any reason.

### Deviations recorded this session (for the operator)

1. **`f85c925` was created by the environment's Jolli auto-capture tooling**, not by the
   orchestrator: non-conventional message, no gate/teeth output, and `.jolli/` files in
   the tree against the never-stage-`.jolli` discipline. Left unamended because the Jolli
   system keys its capture records on the SHA. Gate+teeth evidence for round 5 lives in
   this file and the round-6 commit message instead.
2. **`.agents/plans/gauntlet-impl.md:578` carries an uncommitted edit** (upstream
   wayfinder clone URL `github.com/mattpocock/skills`, plus a `../projects/` clone hint)
   made by neither the orchestrator nor the round-6/6.1 builders. Possibly the operator's.
   Left uncommitted and unverified; confirm or drop it before Task 18.
3. The previous harness's staged scratchpad scripts (`teeth3.sh`, `task3-wf.js`) no longer
   exist in this environment; their intent was reconstructed from this file and the plan.
4. **A sub-agent usage limit killed the first Task-3 builder mid-task** (reset 02:39 next
   day). Delegation itself remained available to fresh agents. Recovery was a finisher
   brief; no orchestrator implementation was needed, so no ESC entry. If it recurs, follow
   the "Next action" note on finisher dispatch.
5. **Task 4 started as an orphaned partial state** — a previous session's builder left
   ~4.4k lines of uncompiling ports work uncommitted (3 breaks + 11 missing suites).
   Recovered by the documented procedure (verify partial state → recon gap reports →
   finisher). Lesson learned: check `git status` before assuming a task is unstarted.
6. **Sub-agent final messages were lost FOUR times this session** (3 empty, 1 truncated —
   two `general` builders and two `general` critics). Recovery pattern that worked:
   checkpoint instructions ("append progress to a file after each item") + `explore`-type
   agents for read-only work when `general` keeps losing its tail. The correction builder
   still lost its progress notes; verification had to be re-done from the tree. See
   `.agents/lessons/subagent-message-loss.md`.
7. **(2026-08-17) The session itself was interrupted twice** (harness suspension between
   tool calls). Recovery both times: reconstruct from `git log`/`git status`/report files,
   re-verify the gate personally, then resume the loop at the missing step (critics →
   correction → confirmation). No work was redone that files had already recorded.
8. **(2026-08-17) A cancelled M1/M2 agent left implemented-but-unproven code** (fresh
   mtimes, no report entries). The finisher-brief pattern (prove only, change no
   behavior) closed it in one bounded slice. Checkpoint/report files are the contract;
   the agent's return message is not.
9. **(2026-08-17) The Task-6 round-1 builder's session was interrupted after the work
   but before report/teeth/critics** — the fifth instance of the class. Recovered by
   the documented procedure; the recurring pattern is now lessoned
   (`.agents/lessons/session-interruption-recovery.md`) with an AGENTS.md failure-log
   rule and the IN-FLIGHT-line practice. Task 6 closed in the same session.
10. **(2026-08-21..23) Task 11: THREE builder sessions died to usage limits**
    (round-1 after D4; correction-1 mid-edit; correction-2 mid-edit — the
    ninth, tenth, and eleventh instances; both correction deaths left
    non-compiling trees and unfulfilled checkpoint contracts). All three
    recovered by the documented procedure (reconcile footprint → finisher
    completes → regenerate fail-first evidence). The mid-edit subclass
    recurs often enough that the correction-brief template now says
    "append to checkpoints after EACH item" — keep enforcing it.
11. **(2026-08-22) The correction-1 finisher committed mid-loop** (before the
    confirmation critic) and included three operator-owned files (AGENTS.md,
    justfile, .agents/prompts/) against the one-task-one-commit discipline.
    Repaired by soft-reset + selective recommit (a507d08); operator files
    left uncommitted for the operator. The confirmation critic subsequently
    returned NOT CONFIRM, validating the rule the premature commit broke.
12. **(2026-08-21) The operator updated the toolchain mid-task**
    (rustc 1.92.0 → 1.98.0, "tooling optimizations") and separately landed
    AGENTS.md/justfile recipe work driven by a `.agents/prompts/` recipe-
    integration prompt. The toolchain change surfaced one pre-existing lint
    (walk.rs unused binding) + three more 1.98 lints — four compensating
    fixes recorded, no logic changes. Operator files remain uncommitted.

**Before you touch anything:** run `just ruler`. It must print PASS. If it does not,
that is a higher-priority problem than any task — stop, diagnose, and report it in
`.agents/reports/` before doing anything else.

**Build-lock discipline (operator instruction):** only one agent may compile at a time,
and `target/` must be watched (`du -sh target`; `cargo clean` if it grows past ~4 GiB).
Blind critics are read-only and non-compiling; builders hold the lock exclusively.

---

## Completed work

| Task | State | Commit |
|---|---|---|
| 0 — Recon, toolchain baseline, delegation spike | **done** | `76e491a` |
| 1 — Freeze the ruler | **done** | `bf09a30` |
| — toolchain install bookkeeping | done | `ac0f033`, `66eebc0` |
| 2 — Workspace skeleton + justfile + gates + boundary test | **done** — closed at round 6.1, residuals recorded above | `b69c095`, `f85c925`, round-6 commit (this session) |
| 3 — Domain types | **done** — closed at round 2 + confirmation, residuals recorded above | `cb2f4ac` |
| 4 — Ports + shared contract suites | **done** — closed at `b27b152`, closure record above | `b27b152` |
| 5 — Configuration, defaults, doctor | **done** — closed after 3-critic loop, closure record above | this session |
| 6 — State store, evidence store, event log | **done** — closed after interruption recovery + 2 critics + correction + confirmation, closure record above | this session |
| 7 — ReferenceProvider: normalize, digest, freeze, verify | **done** — closed after runaway-memory defect fix + 4+1 teeth + 2 blind critics + C1–C7 correction + confirmation CONFIRM, closure record above | this session |
| 8 — Mock AgentRunner + fixtures | **done** — closed after interruption recovery + lint closure + 3 teeth + 2 blind critics + C1–C6 correction + confirmation CONFIRM + N1 micro-fix, closure record above | this session |
| 9 — Observers + observation bundle | **done** — closed after usage-limit interruption recovery + finisher + orchestrator gates + 4 teeth + 2 blind critics (0 critical) + C1–C10 correction + confirmation CONFIRM + N1 doc micro-fix, closure record above | this session |
| 10 — Comparator and envelope builder | **done** — closed after architecture diagnosis + correction rounds 7–10 + fresh blind confirmation round 7 CONFIRM (compiled external attack crate), closure record above; committed after interruption recovery (gates re-verified personally) | `1dab05c` |
| 11 — Verifier and evidence binding | **done** — closed after 2 blind critics + denylist supplement + correction rounds 1–2 + fresh confirmation round 2 CONFIRM (executed evidence); prereg R1 fired and resolved by the preregistered A5 class | `a507d08`, `793d115` |
| 12 — Controller: selection, budget, diagnosis (P4, P6) | **IN FLIGHT** — B1–B4 + critics + adjudication + correction C1/C2/C4/C3 landed (UNCOMMITTED); resume per the Task-12 section below | — |
| 13–27 | not started | — |

### Task 2 — final shape

16-crate workspace (`gauntlet-{domain,ports,app,cli}`, 11 adapters,
`gauntlet-arch-tests`), ten-recipe `justfile` (`ruler` untouched), `.env.example`
(92/92 Annex E vars), `deny.toml` (real policy), 183-test boundary suite: dependency
direction incl. every manifest/config/toolchain channel, per-layer external-dep policy
deny-by-default, source denylist, 13 literal classes over one lexing pass
(`text::regions`: strings/chars/raw/byte/nested comments/escapes), scope-escape
detection (`#[path]`/`include!`/target paths, comment-tolerant), setting-scoped §8.6
defaults exemption at declaration granularity, pinned inventories everywhere, I8-honest
declared build inputs, and the four AK evidence locators under `tests/contract/`.

### Design decisions taken during Task 2 — preserve these

- **Composition-root carve-out.** The adapter-identifier rule (AK-002, §8.1) applies to
  `gauntlet-domain`, `-ports`, `-app` — **not** to `gauntlet-cli`, which spec §4.1 makes the
  composition root and which must be free to name and declare adapter crates. `gauntlet-cli`
  remains subject to the hardcoded-literal scan.
- **`GAUNTLET_RESULT_PATH` is deliberately absent from `.env.example`** (runtime-injected
  per child invocation; listing it would invite operator-stale redirection of a result
  contract). Encoded as a named, justified exclusion in `env_example_complete`.
- **Evidence-locator obligation met:** the four AK-001/002/016/019 locators under
  `tests/contract/` resolve to runnable test targets (`the_manifest_declares_exactly_the_four_answer_key_locators`
  pins them); Task 27 can resolve evidence mechanically.
- **The `versioning` rename is applied** (env vars `GAUNTLET_TOOL_VERSIONING`,
  `GAUNTLET_VERSIONING_STRATEGY`; capability/facet `versioning`). `.env.example` and Annex E
  must always change together (`env_example_complete` diffs them). The spec's §8.6 settings
  table still says `git_strategy` — that matches the frozen spec; do not "fix" one side.
  `crates/adapters/gauntlet-adapter-observe-git` deliberately keeps the product name (§24.3).
  Task 4's `Observer` port uses `Versioning` as the facet-kind variant, never `Git`.

### Operator edits to the spec, absorbed

The operator renamed the `git` tool capability and observation facet to `versioning`
(`Core-spec.md:573`, `:886`). Annex A untouched, `just ruler` unaffected. Task 4 (ports)
and Task 9 (observers) must use `versioning`, not `git`.

### Task 0 outcomes (do not re-derive)

- `.agents/reports/environment.json` + `.md` — full environment fingerprint.
- `.agents/reports/delegation-spike.md` — delegation is admission-based (handle returns
  synchronously, result arrives later); **build Task 4 on the admission-based design as
  specified** (`invoke -> handle`, `poll`, `collect`, result-contract file). Do not re-spike.
- Directory skeleton per spec §25 created; `crates/…` layout owned by Task 2 (done).

### Task 1 outcomes — THE FROZEN RULER

- `.wayfinder/answer-key.md` — spec Annex A verbatim, 70 Units, AK-001…AK-070.
- `.wayfinder/reference.yaml` — manifest wrapper; `.gauntlet/reference.lock` — freeze record
  (answer key `sha256:ccc7c99b…`, manifest `sha256:95331d3e…`).
- `scripts/reference-digest.sh`, `scripts/verify-ruler.sh`, `justfile` (`just ruler`).
- **Design decision Task 7 depends on:** the digest script normalizes CRLF→LF and collapses
  end-of-file newline drift, **nothing else** (per-line trailing whitespace is NOT stripped —
  ruler sensitivity beats the plan's looser wording). Task 7's Rust `normalize` must
  reproduce these exact rules; the two implementations agreeing is the Task 7 gate.

---

## Environment — do not re-derive this

| Thing | State |
|---|---|
| Host | WSL2, kernel 6.18.35.2, x86_64, **6 cores, 7.8 GiB RAM (tight — ~1.6 GiB free)** |
| Rust | rustc/cargo **1.98.0** (updated from 1.92.0 mid-Task-11 by the operator's tooling optimization, 2026-08-21; four compensating lint fixes recorded — no logic changes; 1.92.0 still installed), rustup 1.29.0, rust-analyzer 1.92.0 |
| `just` | 1.58.0 |
| `cargo-nextest` | 0.9.143 ✅ installed |
| `cargo-deny` | 0.20.2 ✅ installed |
| `cargo-mutants` | 27.1.0 ✅ installed |
| Other | git 2.53.0, sqlite3 3.46.1, python3 3.14.4, node 24, bun 1.3.14, uv 0.12.1, gh 2.46.0 |
| **Sandbox** | **`bwrap` 0.11.1 only** (smoke-tested OK). **No docker, podman, qemu, firecracker.** |

**Memory is the real constraint.** Keep `max_parallel` low (spec default 2 is right here);
do not fan out concurrent Rust-compiling builders or you will OOM.

**Containment consequence (recorded, don't re-litigate):** `contained` and `local` tiers
get genuine bwrap containment; `cross_cutting` also runs under bwrap as the strongest
*available* real backend — weaker than spec Annex F's typical container/microVM, honestly
recorded for `docs/CONFORMANCE.md` at Task 27. Not a Task 14 redesign trigger.
`irreversible` gets **no** policy file, by design.

---

## Rules that outrank whatever task you are on

1. **Never edit `.wayfinder/answer-key.md`.** A Unit that seems wrong becomes a dated,
   evidenced entry in `.agents/reports/reference-candidates.md` (create when first needed).
2. **The spec outranks the plan.** On conflict, fix the plan; never the spec.
3. **`just ruler` before and after every task.**
4. **Delegate; do not implement directly** (AGENTS.md §"Delegation", spec §10.7). Every
   builder slice gets an independent **blind** critic: only the artifact + relevant
   answer-key units — never builder reasoning, transcript, or attempt count.
5. **Every task needs its gate at exit 0 AND a demonstrated teeth check** (break the
   mechanism, watch the gate fail, revert, record in the commit message).
6. **Parallel slices get separate git worktrees**, never a shared checkout.
7. **`INCONCLUSIVE` is never `PASS`**, at any layer, ever.
8. **One task, one commit**, with gate + teeth output in the message.
9. Irreversible-tier work, human sign-off, or a genuine blocker → record in
   `.agents/reports/escalations.md`, skip to the next unblocked task, continue.
   **Never downgrade a tier or authority to keep moving.**
10. **Task-2 termination rule (round 5, stands):** a surviving critical whose trigger lies
    outside the stated threat model (lib.rs) is a recorded deferral; inside it reopens —
    except that criticals confined to identified subsystems with purely adversarial
    triggers may be recorded as accepted residuals when the budget rule bites. Accidental-
    trigger criticals always reopen.

---

## Answer-key unit status (spec §26 categories)

- **implemented and proven: none.** No Unit has settled (verification machinery is Task 11).
- **implemented but provisional (27):** the derived, Unit-by-Unit set (each
  counted once) is **AK-001**, **AK-002**, **AK-004**, **AK-005**,
  **AK-015**, **AK-016**,
  **AK-017**, **AK-018**, **AK-019**, **AK-021**, **AK-022**, **AK-023**,
  **AK-024**, **AK-025**, **AK-028**, **AK-029**, **AK-030**, **AK-035**,
  **AK-036**, **AK-037**, **AK-038**, **AK-039**, **AK-049**,
  **AK-051**, **AK-052**, **AK-054**, **AK-059**. **Task 12 added
  AK-037** (durable §15.1 selections on both loop paths, non-empty
  rejected alternatives with true numbers — the domain refuses vacuous
  records; teeth T5), **AK-038** (injected defect closed by a ROUTED
  corrective iteration through the real diff comparator over the real
  workspace), **AK-039** (three real-comparator recurrences under the
  §13.5 semantic key route into bounded per-phase diagnosis ending
  INCONCLUSIVE + quarantine), **AK-051** (budget_exhausted at the exact
  forward rounds ceiling, reserve unspent on every dimension, durable
  checkpoint arithmetic; teeth T1), **AK-052** (meter binding on every
  dispatch the shipped controller makes — builder leg and, after F4,
  the critic fan-out; reservations meter-minted only, released at
  terminal, refusals recorded as typed truncations; teeth T4 + T7) —
  same provisional class: composed proofs with teeth over real adapters
  through two blind confirmation rounds, no VerificationRecord yet; all
  five locators have dated reference-candidates entries
  (materialization class). **Task 11 added AK-005** (three-state Outcome;
  INCONCLUSIVE never satisfies settlement — the composed p12 proof asserts
  NotPass{Inconclusive} through the real chain for absent/stale/wrong
  evidence at every layer; the no-boolean-conversion half rests on the
  pre-existing domain source-scan test outside the proof's locator — the
  split is stated in the reference-candidates entry), **AK-035**
  (deterministic-first: both-rung scenario at unit AND composed level with a
  zero-model-dispatch counter; neutering the ascending sort fails the test),
  **AK-036** (three binding classes reachable and pairwise distinct;
  lying-request, producer, locator, stale, absent, and content-divergence
  scenarios each typed, with 23 bite-proven teeth) — same provisional
  class: instrument-level proof with teeth, no settlement yet; all three
  locators have dated reference-candidates entries (materialization class).
  **AK-001**, **AK-002**, **AK-016** — mechanisms
  enforced by `just boundaries`, teeth
  demonstrated, blind-critic-hardened through seven rounds; provisional because no
  `VerificationRecord` exists yet and deferred majors remain on record. **AK-019** —
  `env_example_complete` instrument landed and green;
  same provisional status. (Task 4 did not move these — the new wire-key exemption and
  E4/F1/F3 closures are boundary-suite changes, not Unit settlements.) **Task 5 added
  AK-015** (config precedence, all four layers test-proven), **AK-017** (unknown
  keys/vars collected and reported), **AK-018** (secrets present-but-never-printed,
  canary-proven on every surface), **AK-049** (no irreversible policy, any spelling,
  both dirs, broken symlinks, unreadable dirs) — same provisional class: teeth-proven
  at the instrument level, no VerificationRecord yet; critic A's "partial-here"
  reflects Task-7/14/15-16 dependencies, not open defects. **Task 6 added AK-054**
  (deterministic export, byte-identical at all three layers: adapter golden, app
  canonical re-export under reordered input, composed stack at the CLI root) — same
  provisional class; its declared locator has a recorded reference candidate. **Task 7
  added AK-004** (strict `wire_enum!` parsing rejects unknown enum values) — same
  provisional class. AK-041/AK-042/AK-053 gained substrate-level support
  (commit-order primitive + exactly-once keyed replay + full §18.2 checkpoint
  durability, real process-kill proven) but their
  declared locators are Task-13 proofs — not claimed. **Task 7 added AK-021**
  (script-exact typed normalization, pinned-output determinism + sensitivity tests,
  both digest implementations agreeing on the frozen key AND a trailing-space
  fixture), **AK-022** (frozen copy outside workspaces at the content address,
  SUT-rooted confinement asserts, composed with real stores), **AK-025's
  anti-adoption half** (any mid-run ruler move aborts with both digests; candidate
  recording is Task 22/24) — same provisional class; **AK-023/AK-024 remain substrate-level provisional mechanisms**
  (tripwire primitive proven at the composition root; the production
  boundary/settlement callsites are Tasks 12–13). All five Units'
  declared locators have dated reference-candidates entries (materialization
  class). **Task 8 added AK-059** (the mock replays all eight scripted
  shapes — malformed, timeout, reference-mutation attempt among them — with
  each driving a distinct asserted runtime state, and the mock passes the
  UNMODIFIED Task-4 shared contract suite through `MockRunnerFactory`;
  teeth on classification, inventory, and capability honesty) — same
  provisional class: instrument-level proof with teeth, no
  `VerificationRecord` yet. **Task 9 added AK-028, AK-029, AK-030** —
  **AK-028** (every facet records instrument identity, exact command, env
  fingerprint in the evidence document, exit status, timing, and a
  content-addressed evidence ref; per-adapter provenance tests + the
  assembler cross-check, which after C3 compares identity/command/exit/
  window agreement and refuses mismatches into both gap arrays),
  **AK-029** (claims live only in a `ClaimsFacet` with no type-level path
  into `VerificationInputs`; proven at the composition root by p3: the
  lying builder's claim diverges from the instrument-observed reality
  inside the bundle, and a canary sweep shows zero claims text in the
  verification inputs; the T2 tooth — a "helpful" claims field — fails
  the proof), **AK-030** (both gap arrays REQUIRED and populated at the
  single §21.3 site on every observer failure and provenance mismatch;
  g6 proves spawn-failure, store-failure, and explicitly-empty faces,
  with the T1 tooth deleting the population failing four tests) — same
  provisional class: instrument-level proof with teeth, no
  `VerificationRecord` yet; all three locators have dated
  reference-candidates entries (materialization class).
- **proposed (48):** every other Unit, including AK-014 (vacuously true; no adapter
  selection exists yet).
- **blocked: none. · unknown: none.** The arithmetic is **0 proven + 22 provisional +
  48 proposed + 0 blocked + 0 unknown = 70 frozen Units**.

---

## Open unknowns (recorded in `.wayfinder/reference.yaml`, not silently resolved)

- **UNK-001** — which containment backend `cross_cutting` runs under in real use, beyond
  this host's bwrap-only environment.
- **UNK-002** — should this repo carry a LICENSE, and which? Not evidenced; not invented.
- **UNK-003** — which concrete `AgentRunner` is `runners.default` in `GAUNTLET.md`.
  Deferred to Task 5 / Tasks 15–16.

## Escalations

**ESC-001 CLOSED** (2026-08-15) — delegation capacity exhausted mid-round; Task 3 was
deliberately NOT implemented inline. The escalation's stated risk was correct and held.
Full context in `.agents/reports/escalations.md`. No escalations currently open.

---

## Next action, concretely

**Tasks 2–10 are CLOSED** (Task 10's artifact was left uncommitted by a session
interruption after its confirmation round; recovered per the documented
procedure — gates re-verified personally: check/lint/deny 0, workspace 697/697,
boundaries 194/194, ruler PASS — then committed). Its MINOR residuals are
recorded in the Task-10 entry above; the observation-owner write gate is the
one that hands work to the Task-12 controller when that task opens.

**Task 11 — Verifier and evidence binding (proves P12) — CLOSED** (commits
`a507d08` round 1 + correction 1; `793d115` correction 2; fresh blind
confirmation round 2 **CONFIRM** with executed evidence). The loop: round-1
builder (died to a usage limit after D4 — ninth interruption instance;
finisher proved-what-landed and landed D5/D6) → two blind critics + a
denylist supplement (0 critical; 7 major code findings; prereg **R1 FIRED**
exactly as preregistered — the caller-supplies-both-sides seam reappeared at
the ladder and the precondition store) → correction round 1 (C1–C6+C8a; its
builder also died mid-edit — tenth instance; finisher recovered) → fresh
confirmation round 1 **NOT CONFIRM** (new MAJOR F1: the content-digest
discipline protected only the store side; a doctored workspace file at the
locator rode the real adapter to a PASS over pinned FAILING bytes — executed
attack) → correction round 2 (F1 three-way digest equality `locator ==
production.content == store` at the governed open with the earned workspace
root; F2 composition-seal arch pin, boundaries 194→197; F3 symmetric gating;
its builder died mid-edit too — eleventh instance; finisher recovered) →
fresh blind confirmation round 2 **CONFIRM** (15-scenario executed probe
incl. the re-executed attack → refused; crafted-manifest attacks on the
seal; 4×E0599 constructor-absence probe; four non-blocking NOTEs recorded).

**Final gates (orchestrator-run, on the committed tree):** the plan's focused
gate `--test p12_missing_is_not_passing` **12/12** + `-p gauntlet-app
verify::` **44/44**; workspace **783/783**; boundaries **197/197**;
check/lint/deny 0; ruler PASS; `just prereg-check 11` PASS. Prereg outcome
observations recorded (`task-11.outcome.yaml`: P1/P2 falsified, P3/P4
supported; R1 fired and was resolved by the preregistered A5 intervention
class — earned authority + unforgeable values — at three seams; **no
representation change**; R2/R3 did not fire).

**Task 12 — Controller: selection, budget, diagnosis (proves P4, P6) —
CLOSED** (single closure commit; fresh blind confirmation round 2
**CONFIRM** with executed gates). The loop: four builder slices (B1
ports `BudgetMeter`/`MeteredBudget` + owner-write gating + producer
authority; B2 scheduler/selector/diagnosis in-crate; B3 controller
composition + observer deadline enforcement; B4 proofs p4/p6 + teeth) —
every slice interrupted at least once by usage limits or operator stops
(12th–15th interruption-class instances; each resume via the documented
reconciliation, never redoing recorded work) → two blind critics
(convergent: rounds dead in the shipped meter, degenerate composed
diagnosis, non-convergence unreachable composed, vacuous selection
record; prereg **R1/P1/P3 (+P2 partial) FIRED** as preregistered) →
correction round 1 C1–C9 + N1–N6, re-sliced mid-round into finishers
F1 (C5 verify-and-record, C6, C7, N6), F2 (C8.1–8.3, C9.2–9.4, N3–N5,
+ ITEM 0), F3 (N1, N2, full gates) after an operator stop at C4 →
**fresh blind confirmation round 1 NOT CONFIRM** (one MAJOR: AK-052's
critic-fan-out leg unmetered — the review path's own doc had deferred
the wiring to "Task 12", a deferral that never reached this ledger; two
MINORs: overstated `mint` doc, §21.3 bare-Err residue) → correction F4
(the run meter REQUIRED in `ReviewDeps`, consulted at every model
admission with truncation-with-typed-cause records and slot release at
fan-in; mint doc-truthed) — F4's builder died at the limit having
landed the work unrecorded (the C5-incident shape, caught by
`reconcile-checkpoints.sh`); the orchestrator verified-and-recorded per
`unrecorded-landed-work.md`, re-deriving all three fail-first legs →
**fresh blind confirmation round 2 CONFIRM** (six MINOR findings, all
dispositioned below; executed gates green).

Two research-relevant events of the round, recorded in
`task-12.outcome.yaml`: the **C7 mis-specification** (a critic finding
encoded an attack no shipped surface can express — the parser's field
set is frozen and typed-refusing; second instance after L4 cohorts;
reachability must be checked against the tree, not assumed from prose),
and the **C5 incident** (work landed past the last checkpoint write;
the record was present, well-formed, and confidently wrong — new
interruption subclass, mitigated by the declare-first rule +
`scripts/reconcile-checkpoints.sh` + the tree-outranks-record rule in
`.agents/lessons/unrecorded-landed-work.md`).

**Final gates (orchestrator-run, on the final tree):** the plan's
focused gate `--test p4_corrective_loop --test p6_budget_control`
**6/6** + `-p gauntlet-app control::` **75/75** (compare:: 36/36, p2
13/13, ports 88/88, sqlite 41/41, meter-memory 7/7); workspace
**914/914**; boundaries **197/197** (no growth); check/lint/deny 0;
ruler PASS; `just doctor` exit OK (check 2 now deep-parse-backed);
`just prereg-check 12` PASS. Prereg outcome recorded
(`task-12.outcome.yaml`: P1/P3/P4 supported, P2 partial; R1 fired,
resolved by the preregistered A3/A5 class; **no representation
change**; R2/R3 did not fire). **Seven teeth** bite-proven on the final
tree (T1/T4 re-expressed post-C2; T7 new on the F4 mechanism), every
revert cmp-verified byte-identical (`task12/teeth.md`).
Reference-candidates entries landed for AK-037/038/039/051/052
(materialization class); N1's governed stop did NOT fire (the real
reference.yaml deep-parses).

**Residuals carried forward:** L1 authority-gate + tier posture
(Task 14); L2 full §18.3 resume + retry-ledger persistence beyond
attempts (Task 13 P7); L3 settlement crash-window replay (Task 13);
L4 cohort non-separability (spec-awaiting; Task 20 fixture); payment
unification — critic costs into run `budget_spent` (Task 15/20);
`BudgetMeter::mint` type-level seal (next ports revision); §21.3
non-integrity mid-run failures — answer-key read failure + runner poll
transport error (Task 13 recovery composition / next §21.3 touch);
model-rung dispatch unmetered (latent, unreachable today; Task 20);
evidence production binds first-facet bytes per attempted unit (Task
13/20); p6's module-doc mischaracterization of p4's mechanism
(comment-only); T3 arch-suite minors (standing); the deferral-ledger
process rule (deferrals live in CURRENT_STATUS, not task-local files).

**Next action, concretely:** Task 13 — Settlement, atomic commit,
single-writer discipline (proves P7, P11) per `.agents/plans/
gauntlet-impl.md`: `SettlementService` (§16.1 five preconditions),
idempotent `(run_id, unit_id, verification_ref)`-keyed commit replay,
answer-key single-writer confinement, proofs `p7_recovery` +
`p11_critic_is_not_verifier`; its inherited surfaces are L2/L3 above
plus the settlement-side sqlite coercions (`settled_at`,
non-checkpoint `unwrap_or(0)` legs) named in
`task12/residual-readjudication.md`.

Evidence: `.agents/reports/task12/` (recon, residual + critic
adjudications, briefs B1–B4 + finishers F1–F4, checkpoints, critic
results, confirmation rounds 1–2 + adjudication, teeth ×7, IN-FLIGHT),
`task-12.outcome.yaml`, and the reference-candidates entries.

**Task-9 dependency carried into Task 10 (D3):** Task 9 closed P3's
observation/acquisition portion: it records instrument reality, explicit gaps, and
structurally excludes claims from verification inputs. P3 also requires comparison
to operate on that recorded instrument failure. The present `ComparisonEnvelope`
carried only digest/gap projections, not facets or exit statuses. Task 10 closed
this: the envelope's observation is PROJECTED from the bound bundle's §5.3.6 wire
bytes (facets, exit statuses, both gap arrays) inside the port, and
`observation_digest` is re-derived at that consumption boundary and cross-checked
against the durable binding. D1/D2 and D4
retain their recorded later-task triggers in the deferral ledger.
