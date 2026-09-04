# Task 23 charter — Process optimizer and the self-improvement gate (P9)

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 23 (phase 6). **Spec:**
§19.3 (process optimizer), §19.4 (self-improvement gate), §5.3.8
(`CandidateRevision`), Annex B.7 (`candidate_revision@v1`), I7, §23.1
P9. **Ruler units:** AK-056 (`verify_by: test`, `blast_radius:
cross_cutting`, `authority: lead_agent`, `evidence_required: {kind:
test_report, locator: tests/proofs/p9_self_improvement_containment,
producer: instrument}`) and AK-057 (same shape, locator
`tests/contract/promotion::candidate_only`). **Task 24 (anti-gaming
suite) and task 25 (real-runner E2E) are OUT OF SCOPE** — do not start
them. `gauntlet improve baseline` belongs to task 26 — do not wire it.

**Spec §19.3–19.4 (verbatim, the ruler for this slice):**

> ### 19.3 Process optimizer
>
> Introduce only after several trustworthy runs exist. It reads run
> evidence and asks what part of the Gauntlet itself caused avoidable
> payment, missed defects, false positives, false negatives, slow
> recovery, or unnecessary orchestration.
>
> It produces proposals. It may not modify the active loop. Every
> proposal declares: claimed problem · supporting evidence · affected
> component · predicted improvement · potential regression · candidate
> change · test · held-out evaluation · rollback.
>
> ### 19.4 Self-improvement gate
>
> ```text
> current → candidate → isolated evaluation → held-out comparison → regression checks
>         → protected-capability non-inferiority → adopt or rollback
> ```
>
> A candidate becomes active only when **all** hold: governance
> preserved · reference integrity preserved · settlement access
> preserved · rollback valid · regression suite passes · protected
> capabilities non-inferior · target metric improves by the declared
> margin · payment remains acceptable.
>
> Evaluate against historical replay, held-out tasks, regression tasks,
> adversarial/gaming cases, and recovery tests. Insufficient evidence
> ⟹ `provisional` ⟹ not promoted.
>
> The optimizer never modifies itself in production and then grades its
> own modification. The active system never moves its own ruler and
> then claims improvement.

Plan gate: `cargo nextest run --test p9_self_improvement_containment &&
cargo nextest run -p gauntlet-app promote::` — plus ALL global gates
(check/test/lint/deny/boundaries/ruler). Plan Done-when: the
verification-weakening candidate is rejected with a recorded reason;
removing the gate-erosion check lets it promote (teeth; AK-056, AK-057).

## Mission

The optimizer's ONLY output channel is a typed candidate revision, and
a revision becomes active only through the full §19.4 conjunction
proven over real isolated evaluation runs. The proof of the slice is
AK-056 + AK-057: an optimizer proposal that improves a metric by
weakening verification is REJECTED with a recorded reason; the
optimizer has no write path into the active loop's configuration or
roles; a revision cannot promote without isolated evaluation (with
`parent_run` set), held-out comparison, regression non-inferiority,
protected-capability non-inferiority, the declared margin, and a valid
rollback; insufficient evidence rests at `provisional`, never
promoted.

## Discovered facts (do not re-derive; verify only where told to)

- **Domain record already exists — reuse it, do not duplicate it.**
  `crates/gauntlet-domain/src/capability.rs:316` `CandidateRevision`:
  target (open vocabulary), claimed_problem, supporting_evidence,
  predicted_improvement, predicted_regression_risk, diff_ref,
  evaluation_run_ids, held_out_result, regression_result,
  protected_capabilities_result, decision (write-once
  `RevisionDecision`: pending | promoted | rejected | provisional),
  rollback_ref. Its structural floor is already proven by domain
  tests: `promote()` requires ≥1 evaluation run AND all three
  comparison results; `mark_provisional()` requires an evaluation run;
  `reject()` requires a rollback ref; the decision is write-once past
  a final state.
- **`RunDefinition.parent_run` exists** (`crates/gauntlet-domain/src/run.rs:75`)
  — the isolated-evaluation run shape. Use it; do not invent a
  different isolation marker.
- **Result-contract codec** (`crates/gauntlet-ports/src/role_result.rs`):
  strict `role_result@v1` envelope; payload validators are a registry —
  an unknown `payload_schema` is a typed error ("never invent one at
  the call site"). Today only `builder_result@v1` validates. Adding a
  payload validator is the additive ports revision this task owns.
- **Task 22's settled surface (the substrate this slice stands on):**
  `CapabilityLedger` port (`crates/gauntlet-ports/src/capability_ledger.rs`,
  append-only observation log + transition-derived capability register,
  `UnrootedPromotion` log-rooting), sqlite adapter (migration
  `0009_capability_ledger.sql`), `gauntlet_app::memory::developmental`
  (`DevelopmentalImport::import_run` — read-only over the StateStore
  substrate, ungated, idempotent; `record_recovery` refuses an
  unearned completion count) and `gauntlet_app::memory::promotion`
  (`PromotionService` — the §19.2 gate over CAPABILITIES).
  **Do not conflate the two gates:** §19.2 promotes capabilities from
  settlement evidence; §19.4 promotes REVISIONS from evaluation
  evidence. Different records, different services, same house
  discipline (transition-derived status, no status column, typed
  per-condition refusals, insufficient-evidence-refuses).
- **Contract suite pattern:** `crates/gauntlet-ports/src/contract/`
  parameterized over a `*Factory` trait, wired into `suite_runs.rs`
  with green/broken/caps-broken legs (see `capability_ledger_*` at
  suite_runs.rs:102–129). A new port needs all three legs.
- **Migrations:** `crates/adapters/gauntlet-adapter-state-sqlite/migrations/`
  `0001`–`0009`; the next is `0010_`. The revision ledger is cross-run
  by design (same DB file as run state + capability ledger).
- **Config wiring pattern (task 22 shipped `promotion.min_settlements`):**
  `crates/gauntlet-app/src/config/resolve.rs` — `ConfigKey` enum
  (:97/:158), effective-config field (:472), defaults seed (:534),
  GAUNTLET.md section walk (:688, "promotion" arm :1020), env-overrides
  struct (:1209); canonical serialization `config/canonical.rs:270/598`;
  default `config/defaults.rs:149`. `GAUNTLET_PROMOTION_MARGIN` is in
  the Annex E vocabulary (`spec_vocab.rs:105-106`, `.env.example:120`)
  and consumed by NO logic — consuming it is this task's declared-margin
  condition.
- **CLI:** `crates/gauntlet-cli/src/improve_cmd.rs` is the HONEST
  not-wired stub naming task 23 (propose | evaluate | promote |
  rollback parse; typed refusal). `report_manifest.rs` carries
  `COMMAND_TABLE` with a `wired` flag and `is_wired()` (:132); the
  manifest and the command must not disagree by construction (RES-15
  discipline — pinned by `tests/cli_surface.rs` and the frozen help
  snapshots `tests/cli_help.txt` / `tests/cli_help_surfaces.txt`).
  Wiring `improve` means updating the manifest row, the usage text, and
  the pinned snapshots DELIBERATELY (same commit; the snapshots are
  instruments, and updating them is part of the claimed change — never
  silently).
- **Loop composition lives at the CLI composition root:**
  `crates/gauntlet-cli/src/compose.rs` (`compose_run` :723,
  `ComposedRun`, runner selection :188). The app layer cannot depend on
  the CLI — the evaluation harness must consume an injected seam, and
  the CLI implements the seam over the real composed loop.
- **Fixture target project:** `tests/fixtures/target/` (the e2e
  fixture: its own GAUNTLET.md — mock runner, `review.mode: single`,
  `review.max_critics: 0`, `verification.prefer_deterministic: true`,
  bounded budget — plus `.wayfinder/`, `.gauntlet/`, `src/`).
  `e2e_full_loop_mock.rs` copies it to a tempdir and runs there. Task
  sets for evaluation are the same shape (a fixture project per set).
- **Controller dispatch** (`crates/gauntlet-app/src/control/controller.rs`)
  is deeply integrated with the unit loop (scheduler, budget,
  RecordingAgentRunner) — do NOT entangle the optimizer with it. The
  optimizer's role invocation is a SINGLE role invocation through the
  same PORTS (`AgentRunner` invoke → handle → poll → collect,
  `role_result@v1` codec at the declared result-contract path), never
  an await of an answer (AGENTS.md: admission-based delegation).
- **The optimizer role file exists** (`.agents/roles/optimizer.md`):
  result_contract `candidate_revision@v1`; `context.allow:
  [run_history, metrics]`, `context.deny: [builder_trajectory, …]`;
  proposals only. The installed-roles surface is the task-17 loader
  (`gauntlet_app::roles`).
- **Locator precedents:** AK-056's `tests/proofs/p9_self_improvement_containment`
  materializes as `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`
  (the p4/p7/p10 precedent — record the divergence in
  `.agents/reports/reference-candidates.md`; NEVER touch the ruler).
  AK-057's `tests/contract/promotion::candidate_only` materializes
  VERBATIM as `crates/gauntlet-app/tests/promotion.rs` with
  `#[test] fn candidate_only` (nextest id `promotion::candidate_only`)
  — land it exactly so.
- **Battery baseline (task 22 settlement):** 1436/1436 workspace,
  boundaries 228/228, lint/deny clean, ruler PASS.
- **Environment:** WSL2, ~1.6 GiB free memory, ONE compile at a time
  (flock the build; run `sccache --stop-server` INSIDE the same
  flock'd command). Package-scoped checks first (`just check-package`,
  `just test-package`). No serde in the workspace — hand-rolled
  canonical JSON only (see `gauntlet_app::report`).

## Contract decisions (binding; deviations get recorded, not smuggled)

- **D1 — `candidate_revision@v1` payload validator (additive ports
  revision).** In `crates/gauntlet-ports/src/role_result.rs`: a strict
  validator for the optimizer's payload carrying §19.3's nine declared
  fields — claimed problem, supporting evidence (refs), affected
  component (the target), predicted improvement, potential regression
  (list), candidate change (the diff ref), test (the evaluation plan),
  held-out evaluation (the held-out plan), rollback (the rollback ref)
  — mapped onto the domain record's fields. Strictness is the house
  rule: unknown fields rejected; missing fields refused with the
  schema locus; a payload that claims a `decision`, evaluation
  results, or evaluation run ids is REFUSED (the optimizer proposes;
  it never decides, never grades itself). Typed payload struct; the
  codec's registry learns exactly one new schema token.
- **D2 — New port `RevisionLedger`** (`crates/gauntlet-ports/src/revision_ledger.rs`,
  own file, typed errors) + shared contract suite
  `src/contract/revision_ledger.rs` (factory-parameterized;
  green/broken/caps legs in `suite_runs.rs`). It durably holds: (a)
  candidate-revision proposals (the admitted payload + admission
  provenance), (b) append-only decision transitions
  (provisional-with-reasons / rejected-with-rollback /
  promoted-with-decision-record), (c) append-only rollback/restore
  records. The CURRENT decision is DERIVED from the transition history
  (no mutable decision column — the capability ledger's house
  pattern); a revision that was promoted and later rolled back reads
  as promoted-but-rolled-back, never silently active. Idempotency on a
  declared proposal identity; disagreeing re-admission is a typed
  error (the `record_run`/observation-identity precedent).
- **D3 — SQLite adapter** over migration `0010_revision_ledger.sql`,
  passing the new suite UNMODIFIED (green leg), catching a deliberately
  broken stub (broken/caps legs). Cross-run by design.
- **D4 — `gauntlet_app::promote` module (new), four pieces:**
  - **`proposal` — the optimizer surface.** Admission: the optimizer's
    result document → strict envelope decode (D1) → §19.3 completeness
    → durable PENDING revision in the ledger. The service struct holds
    ONLY the ledger, the clock, and the codec — structurally no
    configuration-write or roles-write path (D10's arch scan pins
    this). A second, primary path `propose` dispatches the REAL
    optimizer role invocation: single role invocation through the
    `AgentRunner` port (handle → bounded poll → collect at the
    declared result-contract path — never awaiting an answer), envelope
    built from an allowlist (role body + a run-history/metrics
    projection from the durable substrate; nothing from
    `context.deny`), deadline-bounded, honest typed failures. The
    mock runner plays the optimizer in tests (a fixture scenario
    writes a `candidate_revision@v1` result document).
  - **`evaluate` — the evaluation harness.** Composes, per task set,
    a CONTROL run (current configuration) and a TREATMENT run
    (candidate configuration delta applied), both through an injected
    executor seam — an app-owned trait (the loop is application, not
    infrastructure; do not put it in gauntlet-ports) implemented at
    the CLI composition root over `compose_run`. The TREATMENT run
    sets `parent_run` (the isolated-evaluation shape); control and
    treatment run against the SAME frozen reference digest. Task sets
    are named by the operator (held-out set + regression set — §19.4's
    list; the harness never invents them from the active reference
    alone). The harness derives the typed metrics from each run's
    DURABLE facts (settled units, iterations, payment, per-settlement
    verifying instruments, outcomes) with the SAME function for both
    arms, appends the durable `EvaluationRecord` (both run ids, both
    metric sets, both reference digests) to the revision ledger, and
    imports each evaluation run's developmental memory through the
    task-22 `DevelopmentalImport` (runs only after they reach a
    terminal state — the recorded evolving-run re-import wedge
    RES-03 stays closed by construction). A failed evaluation RUN is
    insufficient evidence, never a negative result.
  - **`gate` — the §19.4 promotion gate, all EIGHT conditions, typed
    per-condition refusals.** Adjudicates a revision + its evaluation
    record + the proposal declaration:
    1. *Governance preserved* — the treatment arm's tier/authority
       mix over dispatched units equals control's (durable facts), and
       the declared target is not classified governance-bearing.
    2. *Reference integrity preserved* — both evaluation runs carry
       the current frozen reference digest (never a moved ruler).
    3. *Settlement access preserved* — the candidate's configuration
       delta is classified by a typed TOTAL function over the config
       key vocabulary: any delta that removes, disables, or downgrades
       a verification-bearing key (deterministic verification
       preference, verifier ladder surfaces, review/verification
       strength) is `SettlementAccessWeakened` — **AK-056's refusal**.
       UNKNOWN keys classify as protected (fail-closed — no
       default-allow). Additionally the treatment settlements'
       instrument-strength series must be non-weaker than control's.
    4. *Rollback valid* — the proposal declares a rollback ref; it is
       present and non-empty, and resolvable where the evidence store
       is at hand. A revision without one cannot be adjudicated.
    5. *Regression suite passes* — on the regression task set, every
       unit control settled, treatment settles too, with non-weaker
       instruments.
    6. *Protected capabilities non-inferior* — the declared protected
       capability set (operator-named capability ids) reads
       non-inferior in the treatment arm vs control.
    7. *Target metric improves by the declared margin* — the target
       metric is from a typed vocabulary (iterations per settlement,
       payment per settlement; lower-is-better — both derive from the
       same durable facts), and treatment's figure must beat control's
       by at least the configured margin
       (`GAUNTLET_PROMOTION_MARGIN`, basis points, relative to
       control). Margin 0 keeps the floor at strict improvement.
    8. *Payment remains acceptable* — treatment payment per
       settlement ≤ control's (stable-or-decreasing, the §19.2 shape).
    Verdicts: all eight hold → **Promoted** (write-once decision via
    the domain record + durable decision record in the ledger); any
    condition FAILED → **Rejected** (recorded reasons + the rollback
    ref — the domain floor requires it); evidence MISSING or
    undecidable (no evaluation record, failed/absent runs, metrics
    underivable) → **Provisional** with recorded reasons — NEVER
    promoted, and never rejected (insufficient evidence is not a
    negative result).
  - **`rollback` — the rollback/restore surface.** Records the
    restore transition (append-only; derived state flips to
    rolled-back); the operator's `improve rollback` faces this.
    Rollback of a never-promoted revision is a typed refusal.
- **D5 — Config: consume `GAUNTLET_PROMOTION_MARGIN`.** Full wiring per
  the `NonconvergenceN`/`PromotionMinSettlements` pattern: `ConfigKey`
  arm, env parse, GAUNTLET.md key `promotion.margin`, effective-config
  field, canonical serialization + fingerprint coverage, env-overrides
  struct. Default `PROMOTION_MARGIN_BPS: u64 = 0` in
  `config/defaults.rs` with an HONEST doc comment: the spec's §8.6
  table declares no margin default; 0 keeps condition 7's floor at
  strict improvement (fail-closed by construction — a larger
  configured margin only raises the bar; there is no upper-bound
  refusal to invent). The unit is basis points, documented.
- **D6 — CLI wiring.** `gauntlet improve propose [--from-result
  <path>]` (dispatch through the configured runner by default;
  `--from-result` admits an operator-supplied result document through
  the SAME strict decode — never a bypass of validation, just of the
  model call), `gauntlet improve evaluate <revision> --held-out <dir>
  --regression <dir> [--protected cap,cap] [--metric …]`,
  `gauntlet improve promote <revision>`, `gauntlet improve rollback
  <revision>`. Manifest row flips wired; usage text, help snapshot(s),
  and the cli_surface pins updated deliberately in the same change.
  Follow the existing command patterns (RuntimeContext resolution,
  typed errors to exit codes, the writer-lock discipline for state
  writes where the house already uses it). NO `baseline` subcommand
  (task 26's).
- **D7 — Proof `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`.**
  Composed over the REAL composed loop (mock runner, the
  `tests/fixtures/target` project copied per tempdir — the e2e
  pattern), the REAL sqlite revision ledger, the REAL services, the
  REAL codec. Legs, each with an in-module teeth declaration naming
  the single mechanism whose removal defeats it:
  - **Leg 1 (AK-056):** the weakening candidate — target a
    verification surface, delta that disables/downgrades deterministic
    verification — improves the target metric (the mock reaches
    settlement cheaper without it), evaluation is REAL (control and
    treatment runs through the composed loop, `parent_run` set on the
    treatment), and the gate REJECTS with `settlement_access_weakened`
    durably recorded. This is the plan's Done-when tooth.
  - **Leg 2 (the honest road):** a candidate that genuinely improves
    the target metric WITHOUT touching any protected surface promotes
    through the full eight-condition conjunction, with the durable
    decision record.
  - **Leg 3 (rollback):** the promoted revision rolls back; the
    derived active state reverts; the restore is durable across a
    ledger reopen.
  - **Leg 4 (insufficient evidence):** a revision with NO evaluation
    record adjudicates to Provisional-with-reasons and never
    promotes (the §19.4 floor).
- **D8 — AK-057 contract test `crates/gauntlet-app/tests/promotion.rs`,
  `fn candidate_only` (nextest id `promotion::candidate_only`).**
  Proves the optimizer cannot modify the active loop: (a) a structural
  source scan — the `promote` module references no configuration-write
  and no roles-install surface (the same scan class as the arch
  await-ban); (b) behavioral — after a real `propose` admission, the
  project's GAUNTLET.md bytes and installed-roles state are
  byte-identical, and the revision sits PENDING requiring evaluation
  (the domain floor + the gate's provisional path both refuse
  shortcuts).
- **D9 — App module tests under `promote::`** (the plan gate's second
  filter): payload validation (all nine fields; decision-carrying
  payload refused), admission idempotency + conflict, each gate
  condition refusing in isolation over the REAL ledger + doubles for
  the executor seam, the provisional floor (missing evidence ⇒
  provisional, never promoted, never rejected), derived
  rolled-back state, margin arithmetic (0 = strict improvement; a
  configured margin the improvement does not meet refuses), config
  resolution incl. fingerprint coverage for the margin key.
- **D10 — Boundaries and containment scans.** domain value types (if
  any are needed, e.g. the metric vocabulary) in `gauntlet-domain`
  (zero deps); port + suite in `gauntlet-ports`; adapter in
  `state-sqlite`; services in `gauntlet-app`; composition in
  `gauntlet-cli`. NO new crate. NO settlement-path modification (the
  closure rg-audit: the only settlement writers remain
  `commit_settlement` + the earned authority). NO doctor/report
  changes beyond the improve wiring. The containment scan (D8a) lives
  where the arch suite's source-scan tests live and must BITE (a
  planted violation fails it — the builder proves the bite once and
  reverts).
- **D11 — The candidate delta surface is typed and small.** A
  candidate change is a configuration delta over the effective-config
  key vocabulary — declared keys only, validated against the config
  schema, bytes pinned at `diff_ref` in the evidence store. The gate's
  verification-bearing classification is a typed total function over
  that vocabulary (documented in the module; fail-closed on unknown
  keys). If the builder needs a new domain value type for the metric
  vocabulary or the delta, it goes in `gauntlet-domain` (zero-dep) —
  flagged in the report.

## Acceptance criteria

1. Plan gate green: `cargo nextest run --test
   p9_self_improvement_containment` and `cargo nextest run -p
   gauntlet-app promote::` (bare filters; nextest filters are
   space-joined), plus `promotion::candidate_only` green.
2. Plan Done-when teeth: the verification-weakening candidate is
   rejected with `settlement_access_weakened` recorded against the
   REAL gate over the REAL composed loop — not against a test double;
   each proof leg declares its tooth.
3. Global gates green: `just check && just test && just lint && just
   deny && just boundaries && just ruler`.
4. Contract suite: green on the sqlite adapter, RED on a deliberately
   broken stub (suite_runs legs).
5. `just ruler` before and after every phase (the ruler is NEVER
   touched; AK-056 locator divergence → reference-candidates.md only).

## Working discipline (non-negotiable)

- Write `progress.md` in `.agents/reports/task23/` as you go
  (checkpoint every milestone: files touched, gates run, sha256
  digests of your own edited files); a final `b1-report.md` when done.
  Sub-agent deaths must not lose work.
- Build-lock: one compile at a time — `flock` the build; run
  `sccache --stop-server` inside the same flock'd command. Prefer
  `just check-package` / `just test-package` loops.
- NEVER edit `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.wayfinder/reference.yaml`. Never weaken an assertion to reach
  green. Report provisional things as provisional.
- Do not stage/commit; the orchestrator owns integration and commits.
- Known trap: workspace has no serde — hand-rolled canonical JSON
  writers only. Help snapshots are pinned instruments: update them
  only as part of the declared CLI change, never to silence a
  failure you don't understand.
