# Task 22 charter — Developmental memory and the capability ledger (P10)

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 22 (phase 6). **Spec:**
§19.1 (two separate stores), §19.2 (promotion criteria, verbatim below),
I6 (no false capability credit), §6 (ports), §24.2 profile. **Ruler
unit:** AK-055 (`verify_by: test`, `blast_radius: local`,
`authority: critic`, `evidence_required: {kind: test_report, locator:
tests/proofs/p10_no_false_learning, producer: instrument}`). **Task
23 depends on this task; do NOT build any of task23** (no optimizer, no
self-improvement gate, no `improve` wiring — `GAUNTLET_PROMOTION_MARGIN`
stays unconsumed).

**Spec §19.1–19.2 (verbatim, the ruler for this slice):**

> | **Developmental memory** | Every attempt, failure, correction, comparison, payment, settlement, recovery. Raw. | Immediate, append-only |
> | **Capability ledger** | Promoted skills, shortcuts, routes, prompts, model-routing rules. | Governed promotion only |
>
> Tool access is not capability. One successful run is not capability.
> Capability is repeated reliable settlement under relevant variation,
> with recovery and decreasing orchestration burden.
>
> Promote a capability only with: ≥ `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
> (default 3) settlements across recorded variation axes; ≥1 recovery
> event handled; non-increasing failure rate; **no gate erosion** (the
> settlements did not become easier because verification became weaker);
> stable or decreasing orchestration burden.
>
> Tracked dimensions: performance · variation · transfer · recovery ·
> orchestration burden · settlement reliability · failure rate · gate
> erosion · stability.
>
> **A faster workflow that became faster by skipping verification is a
> regression, not learning.** This is the single most important sentence
> in this section.

Plan gate: `cargo nextest run --test p10_no_false_learning && cargo
nextest run -p gauntlet-app memory::` — plus ALL global gates
(check/test/lint/deny/boundaries/ruler). Plan Done-when: one success does
not promote; three settlements with *identical* variation axes also do
not promote (variation must be real — teeth; AK-055).

## Mission

Two stores with opposite write policies, both durable: developmental
memory that accumulates every run fact immediately and without gating,
and a capability ledger whose only status-raising writer is a promotion
service that refuses everything §19.2 does not prove. The proof of the
slice is AK-055: a single successful run leaves a capability `observed`;
three settlements under identical variation axes are refused promotion
with recorded reasons; the positive path (≥N settlements across real
variation + a handled recovery + non-increasing failure rate + no gate
erosion + non-increasing orchestration burden) is the ONLY road to
`promoted`, and it is typed end to end.

## Discovered facts (do not re-derive; verify only where told to)

- **Ports:** `crates/gauntlet-ports/src/` — one file per port with typed
  errors; shared contract suites in `src/contract/` parameterized over a
  `*Factory` trait, run from `src/contract/suite_runs.rs` with
  green/broken/caps legs (see `agent_runner_{green,broken,caps}_is_caught`).
  A new port needs all three legs.
- **Durable substrate already recording run facts (per-run, via
  `StateStore`):** `record_role_invocation` (attempts;
  NO invocation reader exists on the port — do not invent one),
  `record_correction_selection`/`correction_selections`,
  `accept_comparison`, `record_verification`/`verification_records`
  (typed outcomes incl. FAIL/INCONCLUSIVE), `commit_settlement`/
  `settlements` (durable `SettlementRecord` rows), `checkpoint`/`resume`
  (carries `budget_spent` — the durable payment fact),
  `record_evidence_production`, `append_event` (ordinary kinds only;
  settlement kind REFUSED on both generic append surfaces — task13 COR3),
  `export_jsonl` (deterministic). `complete_pending_settlements` is the
  §18.3 recovery completion; it returns a count and writes NO distinct
  durable "recovery happened" marker.
- **NO run enumeration exists on `StateStore`** (task21 charter:
  "the state store names NO run enumeration — do not invent one").
  Cross-run promotion evidence therefore cannot be queried from the
  per-run substrate; it must accumulate in the ledger store's own
  observation log (see D2/D3).
- **Authority pattern precedent:** earned writes are minted through
  authority types with no public constructors (`AdmittedSettlement`,
  `VerifiedClaim`, `GateAdmission`). Follow the house pattern wherever a
  write must be earned.
- **Config wiring pattern:** `crates/gauntlet-app/src/config/resolve.rs`
  — `ConfigKey` enum → env mapping (`GAUNTLET_NONCONVERGENCE_N` at
  :334) → `defaults` const (`NONCONVERGENCE_N: u32 = 3` in
  `config/defaults.rs:148`, the ONLY literals module the boundary test
  whitelists) → effective-config field → canonical serialization
  (fingerprint coverage). `GAUNTLET_PROMOTION_MIN_SETTLEMENTS` and
  `GAUNTLET_PROMOTION_MARGIN` are already in the Annex E vocabulary
  (`.env.example:119-120`, `spec_vocab.rs:105-106`); neither is consumed
  by any logic today.
- **CLI:** `gauntlet improve` is HONESTLY NOT WIRED naming task23
  (`crates/gauntlet-cli/src/improve_cmd.rs`) — leave it exactly so. The
  §20.1 command set has no capabilities command; adding CLI surface is
  OUT OF SCOPE for this task.
- **Domain:** `gauntlet-domain` is zero-dependency; fixed-value-set
  fields are enums; `Outcome` is three-state with no boolean conversion.
- **Proof location convention:** composed proofs live as integration
  tests in `crates/gauntlet-cli/tests/` (`p1`…`p12`, `e2e_full_loop_mock`).
  AK-055's frozen locator `tests/proofs/p10_no_false_learning` matches no
  file — same recorded locator-divergence class as AK-060/061/062/063
  (record it in `.agents/reports/reference-candidates.md`; NEVER touch
  the ruler).
- **Battery baseline (task21 settlement):** 1392/1392 workspace,
  boundaries 228/228, lint/deny 0, ruler PASS, e2e+cli_surface green in
  both feature configs.
- **Environment:** WSL2, ~1.6 GiB free memory, one compile at a time
  (flock build-lock; `sccache --stop-server` INSIDE the same flock'd
  command). Package-scoped checks first (`just check-package
  <crate>` / `just test-package <crate>`).

## Contract decisions (binding; deviations get recorded, not smuggled)

- **D1 — New port `CapabilityLedger`, its own file.**
  `crates/gauntlet-ports/src/capability_ledger.rs`: typed record types +
  typed errors + the trait, with a shared contract suite
  `src/contract/capability_ledger.rs` (factory-parameterized) wired into
  `suite_runs.rs` green/broken/caps. The port carries BOTH §19.1 stores'
  durable substrate: (a) an append-only **observation log** — the
  developmental-memory row type covering every class in the §19.1 left
  column (attempt, failure, correction, comparison, payment, settlement,
  recovery) + the run/run-fact provenance — appended freely, ungated,
  idempotent on a declared observation identity; and (b) the
  **capability register**: `CapabilityId` → status in
  `{Observed, Provisional, Promoted, Revoked}` + tracked-dimension
  facts, with status transitions as FIRST-CLASS typed records carrying
  reasons and evidence refs. Idempotency: re-appending the same
  observation identity is a no-op; disagreeing re-append is a typed
  error (the `record_run` precedent).
- **D2 — One writer per store.** Observation appends: any caller, never
  gated on outcome (§19.1 "Immediate, append-only"; a failed attempt is
  still memory). Status transitions toward `Provisional`/`Promoted`/
  `Revoked`: ONLY through the app-layer promotion/revoke services (D4);
  the port must be shaped so a bare `set_status`-style escape hatch does
  not exist (earn it through a typed transition record the service
  composes — the critic will check for smuggled elevation paths).
- **D3 — Developmental memory mechanism: import, not dual-write.**
  `gauntlet_app::memory::developmental` projects a run's durable
  substrate (the typed `StateStore` reads listed above) into observation
  rows and appends the ones the ledger store does not yet have,
  idempotently. Invocation rows have NO port reader — attempts/failures
  at the invocation level are projected from what the port CAN read
  (verification outcomes, correction selections, comparison records,
  settlements, checkpoints' `budget_spent`); the observation row type
  records exactly the facts available — never invented ones. The import
  is UNGATED: it records failures, INCONCLUSIVEs, and empty results with
  the same eagerness as successes. Production call sites: before any
  promotion evaluation, and after a run reaches a terminal state in the
  report path — both append-only, neither touches the settlement path.
- **D4 — `gauntlet_app::memory::promotion` — the §19.2 gate, all five
  conditions, typed refusals.** `PromotionService::evaluate(capability)`
  reads the capability's backing observations and returns a durable
  decision: `Promote{decision_record}` or `Refuse{per-condition
  reasons}`. A refusal writes `Provisional` + reasons (spec §19.4's
  "insufficient evidence ⟹ provisional ⟹ not promoted"); the capability
  that was never evaluated stays `Observed`. Conditions, mechanically:
  1. **Min settlements:** ≥ configured N (default 3) SETTLEMENT-class
     observations for the capability.
  2. **Recorded variation:** the N settlements must differ on at least
     one CONTENT axis. Typed axes, from durable facts only: unit_id,
     reference_digest, blast_radius/tier, evidence kind/locator class.
     `run_id` and timestamps are EXPLICITLY NOT axes (they always
     differ; that is the false-variation trap AK-055 names). Identical
     axes across all N ⟹ refuse with `InsufficientVariation`.
  3. **Recovery handled:** ≥1 recovery-class observation. The recovery
     observation is appended (ungated) by the recovery completion call
     site after `complete_pending_settlements` returns >0 completed —
     an append-only observation, NEVER a settlement-path change. The
     proof may drive it through that seam.
  4. **Non-increasing failure rate:** computed over the capability's
     observation window from durable outcome facts; typed rule, recorded
     in the module docs; a window with no attempts reads as
     insufficient evidence, never as a pass.
  5. **No gate erosion + stable/decreasing orchestration burden:** the
     settlements' verification facts must not weaken across the window
     (later settlements verified by strictly weaker instrument classes
     than earlier ones ⟹ `GateErosion` refuse), and per-settlement
     orchestration cost (iterations/selections per settlement) must not
     increase. These are typed proxies over recorded instruments — say
     so honestly in the module docs; they are not semantic proofs.
- **D5 — Status semantics.** `Observed` = has observations, never
  evaluated. `Provisional` = evaluated and refused, reasons durable
  (also the resting state for evaluation with zero settlements — never
  promoted). `Promoted` = full §19.2 satisfaction, decision record with
  evidence refs durable. `Revoked` = explicit revoke (service method,
  operator/optimizer-facing; task23 consumes it) — sticky: re-promotion
  must re-run the FULL evaluation, not merely flip a bit. Domain gets
  the four-state enum (`gauntlet-domain`), the port the record types,
  the app the services.
- **D6 — Config.** `PROMOTION_MIN_SETTLEMENTS: u32 = 3` in
  `config/defaults.rs`; full `ConfigKey` wiring following
  `NonconvergenceN` (env `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`, GAUNTLET.md
  key `promotion.min_settlements`, effective-config field, canonical
  serialization + fingerprint). A configured 0 is a typed configuration
  error (I6 cannot hold below 1; refuse at resolution). `GAUNTLET_PROMOTION_MARGIN`
  stays vocabulary-only (task23's declared-margin check).
- **D7 — SQLite adapter.** Implement the port in
  `gauntlet-adapter-state-sqlite` (new tables via the existing migrations
  pattern; cross-run by design — the ledger deliberately outlives runs).
  It must pass the new contract suite unmodified.
- **D8 — Proof `crates/gauntlet-cli/tests/p10_no_false_learning.rs`.**
  Composed over the real sqlite ledger + deterministic clock/id adapters
  (house style of p4/p7): (a) one successful run → import → status stays
  `Observed`; (b) N≥3 settlements with identical content axes → refuse,
  `InsufficientVariation` recorded, NOT promoted (the plan's teeth); (c)
  the positive path → `Promoted` with a durable decision record naming
  evidence; (d) revoke → sticky semantics proven. Teeth declarations
  in-module: each anti-false-learning assertion names the single
  mechanism whose removal defeats it (the falsifier will mutate them).
- **D9 — App module tests under `memory::`.** Transition legality
  (including revoke stickiness and no bare-set-status escape),
  condition-by-condition promotion coverage, import idempotency +
  ungated-ness (failures recorded as eagerly as successes), config
  resolution incl. the 0-refusal.
- **D10 — Boundaries and layering.** domain types in `gauntlet-domain`
  (zero deps), port + suite in `gauntlet-ports`, adapter in
  `state-sqlite`, services in `gauntlet-app`, composition in the proof.
  NO new crate. NO new CLI surface. NO doctor/report changes. NO
  settlement-path modification (rg-audit in the closure: the only
  settlement writers remain `commit_settlement` + the earned authority).
- **D11 — `improve` untouched.** `improve_cmd.rs` keeps naming task23.

## Acceptance criteria

1. Plan gate green: `cargo nextest run --test p10_no_false_learning`
   and `cargo nextest run -p gauntlet-app memory::` (bare names; nextest
   filters are space-joined).
2. Plan Done-when teeth: the single-success refusal AND the
   identical-variation refusal are both asserted against the REAL
   service over the REAL adapter — not against a test double.
3. Global gates green: `just check && just test && just lint && just
   deny && just boundaries && just ruler`.
4. Contract suite: green on the sqlite adapter, RED on a deliberately
   broken stub (suite_runs legs).
5. `just ruler` before and after every phase (the ruler is NEVER
   touched; AK-055 locator divergence → reference-candidates.md only).

## Working discipline (non-negotiable)

- Write `progress.md` in `.agents/reports/task22/` as you go (checkpoint
  every milestone: files touched, gates run, digest of your own edited
  files); a final `b1-report.md` when done. Sub-agent deaths must not
  lose work.
- Build-lock: one compile at a time — `flock` the build; run
  `sccache --stop-server` inside the same flock'd command. Prefer
  `just check-package` / `just test-package` loops.
- NEVER edit `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.wayfinder/reference.yaml`. Never weaken an assertion to reach green.
  Report provisional things as provisional.
- Do not stage/commit; the orchestrator owns integration and commits.
- Known trap: workspace has no serde — hand-rolled canonical JSON
  writers only (see `gauntlet_app::report`).
