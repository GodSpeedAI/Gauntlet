# Task 23 closure — Process optimizer and the self-improvement gate (P9)

**Date:** 2026-09-04. **Plan:** `.agents/plans/gauntlet-impl.md`
§Task 23. **Spec:** §19.3–19.4 (verbatim in charter.md), §5.3.8,
Annex B.7, I7, §23.1 P9. **Ruler units:** AK-056 + AK-057. **Commits:**
the task23 closure commit + the status commit (one task one commit;
explicit pathspecs).

## What shipped

The governed self-improvement loop, end to end, over real adapters:

- **Port** `crates/gauntlet-ports/src/revision_ledger.rs`: the
  `RevisionLedger` port — durable candidate-revision proposals
  (content-derived identity, replay idempotency independent of the
  clock), evaluation records carrying all four arms' reference digests
  and their declared metric/protected provenance inside the
  idempotency agreement, decision records write-once past final states
  (claimed-state validated against the derived state), and append-only
  restore records (the rolled-back fact is row existence; no status
  column anywhere). Shared contract suite `contract/revision_ledger.rs`
  with green/broken/caps legs.
- **Codec**: `candidate_revision@v1` payload validator in
  `role_result.rs` — §19.3's nine declared fields, strict; a payload
  claiming a decision or evaluation evidence is mechanically refused
  (the optimizer proposes; it never decides).
- **SQLite adapter** over migration `0010_revision_ledger.sql`
  (INSERT-only), passing the suite unmodified; reopen durability
  proven.
- **App module** `gauntlet_app::promote`: proposal admission (the
  service struct holds ONLY ledger+clock; the dispatched `propose`
  path goes through the real AgentRunner invocation with an allowlisted
  envelope), the evaluation harness (control + treatment arms through
  the injected executor seam; treatment runs carry `parent_run`, and
  the harness VERIFIES the durable run record's parent fact before
  anything is recorded; identical metric derivation for both arms;
  per-run developmental import through the task-22 service after
  terminal states), the §19.4 gate (all EIGHT conditions, typed
  per-condition refusals; FAILED ⇒ Rejected with every reason + the
  rollback ref; evidence gaps ⇒ Provisional, never promoted, never
  rejected — an incomplete arm, a moved ruler on any arm, or an
  underivable figure is a GAP; infra failures abort typed, never
  judge), and the rollback/restore surface (one-way, ledger-validated,
  durable across reopen).
- **Config**: `GAUNTLET_PROMOTION_MARGIN` / `promotion.margin` fully
  wired (bps; default 0 = the strict-improvement floor; >10 000 bps is
  a typed error at resolution AND at the gate constructor — the
  falsifier's silent-clamp finding is closed at both layers).
- **CLI**: `gauntlet improve propose|evaluate|promote|rollback` wired
  (§20.1); `evaluate` names held-out + regression task sets, refuses an
  unappliable delta entry before any arm runs (C7), and refuses a
  moved-ruler task set at evaluate time (R1); `promote` aborts typed on
  an evidence-store read error (C5). Manifest row wired; help snapshots
  updated deliberately. No `baseline` subcommand (task 26's).
- **Proofs**: `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`
  — leg 1 (AK-056): the verification-weakening candidate is REJECTED
  over the real composed loop with `settlement_access_weakened`
  durably recorded; leg 2: the honest candidate promotes through the
  full eight-condition conjunction; leg 3: rollback durable across a
  ledger reopen; leg 4: no evaluation ⇒ Provisional, never promoted.
  `crates/gauntlet-app/tests/promotion.rs::candidate_only` (AK-057's
  locator verbatim): behavioral containment (GAUNTLET.md bytes and
  installed-roles state byte-identical after a real admission; the
  revision rests PENDING) + the arch containment scan
  (`gauntlet-arch-tests/src/optimizer.rs`) with negative controls.

## Confirmation history

1. **Builder B1** landed the slice (orchestrator-side interruption hit
   the dispatch; the work landed; 38 reported digests verified against
   the tree).
2. **Blind design-as-built critic round 1: NOT_CONFIRM**
   (`critic-design.md`) — 1 BLOCKING (the `u64::MAX` sentinel for
   incomplete arms broke the verdict contract in BOTH directions; a
   promotion over an incomplete control arm was DEMONSTRATED on the
   real binary, and the shipped honest-road legs were built on the
   defect) + 4 majors + 6 minors. The critic also independently
   confirmed the orchestrator's own (0,0)-fallback finding.
3. **COR-1** (`cor1-brief.md`, C1–C11, RED-first): incomplete arms are
   evidence gaps (sentinel deleted); the p9 honest-road legs moved to
   the gate-level composition over REAL StateStore earned authorities
   (the loop-economics finding: NO inert candidate key can improve
   per-settlement economics on the current loop — write-once
   per-(run,iteration) binding + carry pins iterations/settlement at
   N/N; recorded in reference-candidates.md, the binding re-key is the
   ruler owner's); admission idempotency off the clock; parent_run
   verified as durable evidence; condition-4 fail-open closed;
   capability-ledger infra failure = typed abort; no silently
   unapplied delta entries; docs/comment honesty (C8/C10); dead
   contract variant removed; deviations recorded. Operational incident
   disclosed in the reports: a predecessor self-deadlocked the compile
   lock with a nested flock (orchestrator unwedged), and a continuation
   dispatch overlapped the recovered predecessor — both builders
   disclosed; the interleaved tree FAILED the orchestrator's battery
   (arch-scanner + literal + fmt violations in the p9 file, which
   neither builder's slice-scoped battery covered); COR-1-SWEEP + a
   formatter pass reconciled it.
4. **Blind EXECUTING falsifier round 1: NOT_CONFIRM**
   (`falsifier-round1.md`) — F-1 MAJOR executed through the real
   binary (condition 2 structurally saw 2 of 4 evaluation arms; a
   moved regression ruler evaluated cleanly and adjudicated past
   condition 2) + F-2 MAJOR executed over real adapters (the
   instrument series paired positionally; an extra deterministic
   settlement shielded a model-rung-weakened unit and the gate
   PROMOTED) + F-3..F-6 (margin >10 000 bps silent clamp;
   evaluation-record idempotency omitted the declared metric/protected;
   case-sensitive governance-target classification; the containment
   scan missed raw write classes). Every declared tooth bit; all
   mutations restored byte-identically.
5. **COR-2** (`cor2-brief.md`, R1–R6, RED-first): the evaluation
   record carries ALL FOUR arms' digests through port/adapter/suite/
   harness, condition 2 checks all four, and the harness refuses a
   moved-ruler task set at evaluate time (typed, before any record —
   the lock is RE-VERIFIED, not trusted); the instrument-series check
   joins BY UNIT; the margin ceiling is a typed refusal at both
   layers; `facts_agree` covers metric+protected; the governance
   classification is case-normalized; the scan reports the raw write
   classes. (A builder death mid-R1 was recovered by a continuation
   builder that reconciled orphaned state and fixed the
   never-run-gates fallout: clippy `result_large_err` box-sizing,
   literal-scan fixture spellings, fmt.)
6. **Blind EXECUTING falsifier round 2 (fresh): CONFIRM**
   (`falsifier-round2.md`) — all six round-1 holes re-attacked and
   CLOSED (the round-1 binary reproduction verbatim now dies at
   evaluate time, nothing recorded; the revision rests at the
   provisional floor; the gate independently re-checks all four arms
   against a re-verified lock and catches a post-evaluation ruler
   tamper; the shield shape rejects by-unit; the ceiling arithmetic is
   exact at 10 000 bps; re-declaration conflicts; case-normalization
   holds; every declared raw-write class is reported). Extension
   attacks (moved held-out ruler, crafted/fifth-arm/digest-missing
   records, shared directories, duplicate settlement rows,
   cross-connection double-adjudication, stale claims, CLI refusal
   shapes, full four-arm pipeline) found NO new hole — two MINOR
   residuals, trigger-recorded below. All seven declared teeth went
   RED under neutralization with 1:1 attribution; restores
   byte-identical; post-restore sweep green.

## Orchestrator personal verification (never settled on sub-agent say-so)

- Gate batteries at every phase transition; the closure battery on the
  final tree run TWICE: `just test` 1507/1507 both runs, `just lint`
  clean, `just deny` ok (one transient first-run failure re-verified
  exit 0 — warnings only, pre-existing), `just boundaries` 235/235
  both runs, `just ruler` PASS at every transition and ×3 at closure,
  plan gates p9 4/4 / promote:: 46/46 / candidate_only 1/1, ports
  suite legs 3/3, sqlite contract suite 5/5, arch scans 235/235.
- **Done-when tooth personally executed:** neutralized
  `preserves_verification` (the delta classification) → p9 leg 1
  (`the_verification_weakening_candidate_is_rejected_over_the_real_loop`)
  RED in my own run, other legs green → restored byte-identical
  (sha256 `4276075e…` verified OK against the pre-mutation digest) →
  all green. The falsifier additionally proved (T1) that the same
  neutralization lets the weakened candidate PROMOTE through the
  isolation leg's mechanism — the promotion-by-weakening shape is
  real exactly one gate mechanism away, and the gate is what stops it.
- Settlement-path audit at closure: zero diff over
  `crates/gauntlet-app/src/settle/` and `producer_authority.rs`; the
  promote module holds no config-write or roles-install surface
  (arch scan with negative controls, now covering the raw write
  classes).

## Honest boundaries and residuals (all trigger-recorded)

- **The §19.4 positive path is gate-level-composition, not
  loop-level-economics** (charter D7 deviation, recorded): on the
  current loop, NO inert candidate key can improve per-settlement
  economics between complete arms (the observation-binding
  granularity pins iterations/settlement at N/N; mock payment is
  zero) — p9 legs 2/3 compose the promotion through the REAL harness,
  ledgers, gate, and rollback over run facts earned through the REAL
  StateStore authorities (the p10 pattern), with the runner-cost
  economy declared as proof-authored seeds. Leg 1 (AK-056, the plan's
  Done-when) remains fully on the real composed loop. The loop-level
  promotion demonstration awaits the ruler owner's binding re-key
  (reference-candidates 2026-09-04 entry, and the earlier
  binding-granularity entry).
- **E-1 (minor, falsifier r2):** the containment scan's token list
  still misses `fs::rename`, `fs::copy`, `File::options`,
  `fs::hard_link`, `fs::remove_file`, `write!` spellings (one
  spelling generation behind; the behavioral AK-057 leg and the
  ledger+clock-only service structs are the real containment).
  Trigger: next arch-suite touch or task24.
- **E-2 (minor, falsifier r2):** the by-unit instrument join takes
  the FIRST treatment row per unit — order-sensitive for duplicated
  unit rows; unreachable through the shipped loop (settlement commits
  are idempotent by run+unit). Trigger: next gate touch.
- **Falsifier r1 informational leads carried:** re-evaluation
  cherry-picking (`evaluations.last()` — task24 territory); the
  governance-target prefix check is advisory against adversarial
  spellings; the optimizer dispatch's budget is the whole configured
  run budget; `runners.by_role.<role>.model` classified Inert is a
  latent policy question (unreachable through the binary today —
  `validate_delta` refuses by_role keys; ruler owner before any
  composition learns to apply them).
- **AK-056 locator divergence:** `tests/proofs/p9_self_improvement_containment`
  materializes as `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`
  (the AK-049/p4/p7/p10 class; reference-candidates 2026-09-03).
  AK-057's `tests/contract/promotion::candidate_only` materializes as
  the `candidate_only` test of the `promotion` target — the
  space-joined nextest invocation note is in the same entry.
- **Operational lessons (for AGENTS.md):** a self-nested `flock`
  command deadlocks the repository compile lock (killed two builder
  sessions today); resuming a "dead" agent via message CAN revive it —
  verify liveness before re-dispatching a replacement or two builders
  will interleave on one tree; slice-scoped batteries never run the
  repo-wide scans — an orchestrator full battery between rounds is
  what catches cross-file interleaving damage.

## Answer-key unit status

49 → **51 provisional through Task 23** (AK-056 delivered: the
verification-weakening candidate is rejected over the real loop with
the reason durably recorded, and the tooth is the recorded reason
itself; AK-057 delivered: the optimizer's only output is a candidate
revision — behaviorally and structurally contained, evaluation-gated,
rollback-valid). 37 proposed, 0 blocked.

## Battery (final tree, orchestrator's own runs)

check clean · test 1507/1507 ×2 consecutive · lint clean · deny ok ·
boundaries 235/235 ×2 · ruler PASS ×3 · p9 4/4 · promote:: 46/46 ·
candidate_only 1/1 · ports suite legs 3/3 · sqlite contract suite 5/5 ·
settle-path audit zero diff · Done-when tooth RED → byte-identical
restore (sha256-verified) → GREEN.
