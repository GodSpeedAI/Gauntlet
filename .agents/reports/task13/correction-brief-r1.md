# Task 13 / Correction brief round 1 — COR-1..COR-7 (early-probe findings)

You are the CORRECTION BUILDER. Read `AGENTS.md` first, then:

1. `.agents/reports/task13/early-adversarial-probe.md` (the findings),
2. `.agents/reports/task13/early-probe-adjudication.md` (YOUR SCOPE —
   COR-1..COR-7 fix-now, each with its fail-first obligation; M4/m3/m4/m5
   are later-scope, do NOT touch them),
3. `.agents/reports/task13/builder-b1.md` + `builder-b2.md` (what you are
   correcting),
4. `crates/gauntlet-ports/src/producer_authority.rs` (the pattern COR-1
   follows).

Complete each COR item exactly as adjudicated: mechanism + fail-first
evidence (RED observation from the probe's attack scenario → fix → GREEN),
one item at a time, `correction-checkpoints.md` STARTED/DONE per item
(append after EACH item — never batch). Specific design notes:

- **COR-1**: the recording authority validates durable run + iteration
  anchor, mints `AdmittedVerificationRecord`; the trait's
  `record_verification` consumes it. `VerifiedClaim`'s consumer path (the
  app's `ensure_verification_recorded`) earns through the authority.
  crash_helper/test fixtures earn too. `apply_settlement_row` (adapter)
  becomes test-gated. Fail-first: today's ungated write sequence (raw doc →
  record_verification → admit → commit — crash_helper's exact shape minus
  its earning) must be refused or non-compiling after.
- **COR-2**: `SettleRequest` drops the `readiness` field; the service
  calls `settlement_precondition` internally; explicit claim-outcome==Pass
  entry check. Fail-first: probe M3's FAIL-claim + stale-PASS-readiness
  attack through the service → refusal with ZERO durable writes (assert no
  events, no rows).
- **COR-3**: `VerificationRecordDoc` gains the verification-ref field
  (strict codec round-trip + evidenced golden update; the domain record
  already carries the id — Annex-B samples are illustrative). `admit`
  requires the durable PASS record's ref == the commit's verification ref.
  Fail-first: the N-distinct-refs attack → after the first commit, each
  further commit refuses with NO duplicate settlement event.
- **COR-4**: `AlreadyCommitted` reads the stored row; unit+evidence+
  verification agreement or typed `AlreadyCommittedDivergence` refusal.
  Fail-first: the E2 evidence-divergence shape.
- **COR-5**: `admit` compares `record.run_id()` to the commit's run
  (fake-store divergent test — the ports fake that ignores run scoping is
  the discriminator).
- **COR-6**: escape the operator identity through the shared
  `escape_json_text`; injection test (quotes + backslashes in identity;
  pinned record parses as intended JSON).
- **COR-7**: trait read defaults (`settlements`, `verification_records`)
  fail closed with the typed not-implemented refusal (mirror write
  defaults); conforming stubs unaffected; fail-first with a write-only
  stub.

Watch for ripple: COR-2 changes `SettleRequest`'s shape (B2 tests +
controller call site); COR-3 changes the doc schema (goldens, crash_helper,
export pins — evidenced updates only). Never weaken an assertion; proof
adjustments only where the assertion itself is unchanged, each a recorded
deviation.

Gates (run personally, verbatim tails to the checkpoints):
```bash
just ruler
just check-package gauntlet-ports && just test-package gauntlet-ports
just check-package gauntlet-app && just test-package gauntlet-app
cargo nextest run --test p4_corrective_loop --test p6_budget_control --test p12_missing_is_not_passing --test p2_context_isolation
just lint && just boundaries
```
Result contract: `.agents/reports/task13/correction-round1.md` (items,
fail-first evidence, deviations, gate tails, files touched).

Hard constraints (unchanged): never touch `.wayfinder/`,
`.gauntlet/reference.lock`, `deny.toml`, nextest config, `AGENTS.md`,
`.jolli/`, `.tmp/`, `.agents/prompts/`, applied migrations (a NEW
migration only if genuinely required — COR-3 needs NONE: verification
records are rows, re-derive from the schema); no sub-agents; no
parallelism increases; `just ruler` before and after.

---

## FINISHER ADDENDUM (2026-08-27, post-recovery — operative for the COR-2..7 slice)

COR-1 is DONE (recovered by the orchestrator, verified green: ports 98/98,
sqlite 46/46, app 324/324, proofs 31/31, lint 0, ruler PASS, boundaries
215/215 — see correction-checkpoints.md RECOVERED-AND-VERIFIED). Your scope
is COR-2 through COR-7 ONLY, plus ONE re-derivation: the builder's COR-1
fail-first evidence is LOST — re-derive it (the probe's C1 attack sequence,
raw doc → ungated record → admit → commit, must now be refused/non-compiling
through the landed authority; record as recovered evidence). Checkpoints
append to correction-checkpoints.md; result contract still
correction-round1.md (write it: COR-1 recovered+verified summary + your
COR-2..7 items + the re-derived COR-1 evidence + gate tails).
