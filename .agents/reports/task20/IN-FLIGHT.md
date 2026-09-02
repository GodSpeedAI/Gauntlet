# Task 20 — IN-FLIGHT (declare-first)

Started: 2026-08-31, orchestrator session (ZCode). Record: task19 settled
(b29da05); task20 is the sole ready task in `.agents/CURRENT_STATUS.yml`.

Task: **CLI surface + first full loop on the mock** (plan Task 20, phase 5,
self-hosting milestone). Plan: `.agents/plans/gauntlet-impl.md` §Task 20.
Ruler units answered: AK-060, AK-061. Residuals triggered here: R-1b, R-2
re-adjudication, R-3, R17-F4, R17-note-1, R2-N1, UNK-003 (forced, stays
operator's), task18 formation residuals (first composed map/key use).

## Slice ledger (declare before edit, mark DONE after; the tree is the contract)

- [DONE 2026-08-31] B1 — containment/durability seams: adapter crate landed,
  R-1b threading landed, cleanup wired, teeth THREE-STATE proofs recorded
  (R-1b distinct-workspaces tooth + install.rs dir-fsync tooth, hashes in
  b1-progress.md), battery green (1303/1303, boundaries 228/228, ruler PASS).
  Report: b1-report.md. (Two builder deaths + one pseudo-death recovered;
  no work lost.)
- [DONE 2026-08-31] B2a — gauntlet_app::report service: landed, 458/458
  package tests incl. teeth (settlement-record removal), lint clean in
  module, API documented in b2a-report.md. Known follow-up handed to B2:
  one clippy redundant_closure at crates/gauntlet-app/tests/report.rs:213
  (out of B1's scope, recorded in b1-report.md).
- [DONE 2026-08-31] B2 — CLI composition root + §20.1 surface + run/dry-run
  landed and independently re-verified (b2-report.md + recovery addendum):
  1319/1319 at its close, dry-run zero-writes, cli_surface snapshot green,
  deviations recorded (id-entropy adapter, arch composition-seal widening,
  [[bin]] name = "gauntlet", D5 resolved in-crate with reference-candidates
  entry).
- [DONE 2026-08-31] B3 — E2E fixture project LANDED and validated (doctor OK,
  dry-run 0 zero-writes, parser OK: 3 units, real check script verified both
  ways; plus two new scenario fixtures e2e-happy/e2e-lying; .gitignore
  un-ignore line for tests/fixtures/target/). E2E TEST TARGET BLOCKED on a
  genuine composition defect, recorded with case analysis in b3-report.md:
  (1) facet evidence written AT evidence_required.locator collides with the
  composed ladder rung's artifact at the same path (RungFailed on first
  dispatch; related to the pre-adjudicated "evidence production binding
  first-facet bytes" task20 deferral), (2) compose.rs never consults the
  Annex-E GAUNTLET_CLOCK hook (mock's fixed-2023 epoch stalls under wall
  clock), (3) max_critics>=1 fails closed for the isolation-none mock as
  model critic (fixture uses 0 — acceptable). No CLI source touched; test
  target deliberately not created while the defect stands.
- [STARTED 2026-08-31] COR-B3a — composition correction: evidence-production
  path collision + clock hook wiring; then complete e2e_full_loop_mock legs
  + discrimination tooth. Result contract:
  `.agents/reports/task20/cor-b3a-report.md`.
  [DONE 2026-08-31] COR-B3a landed (ProducePolicy composition decision,
  clock hook, E2E 3 legs + tooth; 1322/1322).
- [DONE 2026-09-01] COR-D — design-critic correction round (COR-D1..D10
  per design-adjudication.md). Builder was interrupted by a usage limit
  AFTER landing all ten corrections (cor-d-progress.md is the trail);
  recovered-and-verified by the orchestrator 2026-09-01 (cor-d-report.md):
  one real mechanism defect found and fixed by the new teeth
  (ContractDirPolicy contract-dir materialization — the composition-time
  wrap for cli-agent could never validate clean on a fresh state dir),
  plus test-infrastructure completions, all attributed in cor-d-report.md.
  Final battery: 1333/1333, boundaries 228/228, lint/deny 0, ruler PASS,
  doctor OK, e2e+cli_surface green, dry-run zero-writes proven by a
  before/after git-status diff.
- [STARTED 2026-09-01] Falsifier round 1 (blind, fresh context; commission:
  falsifier-round1-brief.md). Result contract:
  `.agents/reports/task20/falsifier-round1.md`.
- [DONE 2026-09-01] Falsifier round 1: NOT_CONFIRM (1 blocking / 1 major /
  4 minor / 1 informational) — report falsifier-round1.md; adjudicated
  falsifier-adjudication-r1.md.
- [DONE 2026-09-01] COR-F — falsifier corrections COR-F1..F6 landed
  (cor-f-report.md): producer-corroboration settlement gate in the composed
  world (blocking F-1 closed; forging attack shipped as e2e-forged-report +
  E2E non-settlement leg), stem containment, TicketId grammar + queue
  containment, escalation-list quarantine, skills lock round-trip fix,
  settled-resume no-op. 1349/1349, boundaries 228/228, dry-run zero-writes.
- [STARTED 2026-09-01] Falsifier round 2 (fresh blind instance; attacks the
  corrections themselves). Result contract:
  `.agents/reports/task20/falsifier-round2.md`.
- [STARTED 2026-08-31] COR-R4 — falsifier round 4 corrections (work order:
  falsifier-adjudication-r4.md; round 4 returned NOT_CONFIRM — 0 blocking /
  1 major / 2 minor; COR-R1..R3 rounds before it are recorded in their own
  report files): COR-R4-1 recovery-COMPLETED production on an
  otherwise-settled run must close as the honest settled no-op (never book
  an iteration for a fully-settled durable projection); COR-R4-2 skills
  verify digests RAW source bytes exactly as roles does; COR-R4-3
  writer-lock Held refusal — bounded retry/reread closes the blank-pid
  window, hedged "stale or unreadable" wording. Result contract:
  `.agents/reports/task20/cor-r4-report.md`. Progress:
  cor-r4-progress.md.
- [ ] Closure: closure.md, CURRENT_STATUS.yml update, commit(s).

## Loop steps still ahead after any interruption

Reconcile `git status` + this ledger + slice progress files; classify and
resume at the missing step. Wave order B1∥B2a → B2 → B3 → critic →
falsifier → gates. Never settle on builder say-so. Compile discipline:
`flock .agents/reports/task20/compile.lock` around every cargo invocation.
