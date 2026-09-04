# Task 23 — IN-FLIGHT ledger

One line per dispatch, updated at every phase transition. The tree
outranks this file; reconcile with `git status` before trusting it.

- 2026-09-03 orchestrator: task23 DISPATCHED — builder B1 chartered
  (charter.md; P9, AK-056 + AK-057; optimizer surface + revision
  ledger + evaluation harness + §19.4 gate + improve CLI wiring + p9
  proof + promotion::candidate_only). No code landed yet at dispatch.
- 2026-09-04 orchestrator: B1 LANDED before an orchestrator-side
  interruption; report + 38 file digests verified against the tree;
  reference-candidates.md truncation verified purely additive (+24).
  PERSONAL GATE BATTERY GREEN: check clean, test 1487/1487, lint
  clean, deny ok, boundaries 233/233, ruler PASS, p9 4/4, promote::
  30/30, candidate_only 1/1 (corrected invocation: `cargo nextest run
  -p gauntlet-app --test promotion candidate_only` — the
  `promotion::candidate_only` substring filter matches zero tests;
  reference-candidates entry line corrected by the orchestrator).
  Settlement-path audit: zero diff over settle/ + producer_authority.
- 2026-09-04 orchestrator (OWN FINDING, held for the correction round,
  falsifier not steered): promote/gate.rs condition 7 — when
  `metric_figures` is None (an arm settled nothing), the `(0,0)`
  fallback still feeds `margin_holds`, which pushes `MarginNotMet` as
  a FAILED condition, so an evidence gap UPGRADES to a Rejection,
  contradicting the module's documented contract and §19.4's
  "insufficient evidence ⟹ provisional". Expected RED-first fix: a
  control arm that settles nothing must adjudicate Provisional.
  Blind critic + falsifier dispatched; this folds into COR-1 (or is
  independently found — either way the correction slice carries it).
- 2026-09-04 orchestrator: blind design critic round 1 = NOT_CONFIRM
  (critic-design.md): 1 BLOCKING (u64::MAX sentinel for incomplete
  arms breaks the verdict contract BOTH directions — promotion over an
  incomplete control arm DEMONSTRATED on the real binary; no-op delta
  over two incomplete arms durably REJECTED; p9 legs 2/3 built on the
  defect) + 4 major (M1 moving-clock admission idempotency, M2
  parent_run never verified as evidence, M3 condition-4 fail-open on
  store read error, M4 capability-ledger infra failure becomes final
  Rejection) + 6 minor + 6 informational. The orchestrator's own
  (0,0)-fallback finding is subsumed by B1's latent-site paragraph.
  COR-1 DISPATCHED (cor1-brief.md, C1–C11, RED-first) to a fresh
  builder. Falsifier r1 will run FRESH on the corrected tree.
- 2026-09-04 orchestrator (C2 DECISION, mid-COR-1): the builder
  empirically proved the prescribed max_parallel mechanism is a
  BEHAVIORAL NO-OP on the current loop (write-once per-(run,iteration)
  observation binding + carry ⇒ iterations/settlement pinned at N/N
  for every complete arm; mock payment zero ⇒ both metrics
  scale-invariant). Recorded finding: NO inert candidate key can
  improve per-settlement economics on this loop — the §19.4 positive
  path is loop-unreachable until the ruler-owner's binding re-key
  (reference-candidates 1358 class). DECISION: (1) leg 1 (AK-056, the
  plan's Done-when) STAYS over the REAL composed loop — builder first
  tests whether a verification-bearing delta (review.mode) genuinely
  moves mock correction-round economics; if yes, strongest form (real
  improvement, real rejection); if no, complete-armed rejection on
  condition 3 with the header stating the containment form honestly.
  (2) Legs 2/3 promote at the GATE level: REAL harness + REAL sqlite
  ledgers + REAL gate, executor seam proof-authored over the REAL
  StateStore EARNED authorities (the p10 composition pattern),
  parent_run on the treatment run's durable record and C4's
  verification exercised; improvement mechanism honestly modeled as
  runner-cost (inert runners.default delta; proof-authored per-
  invocation costs — declared in the header, same authoring class as
  p10's substrate seeds). (3) The loop-economics finding is recorded
  (reference-candidates append + closure + cor1-report) as a DEVIATION
  from charter D7's all-real-loop composition, with the reason.
  (4) No relaxation of C1 — legs 2/3 must not rest on incompleteness.
- 2026-09-04 orchestrator: COR-1 LANDED — with a disclosed concurrency
  incident: the predecessor (fb4e0d63) recovered after the orchestrator
  unwedged its self-nested flock, while a fresh continuation builder
  (75ee8e82, dispatched before the recovery was visible) also worked
  the slice; the peer detected the collision, stayed read-only on
  shared files until its partition question, and completed; one Write
  was safely refused by the Edit stale-read guard; both disclosed in
  cor1-report.md. All C1–C11 landed (C2 per the recorded decision).
  The interleaved tree FAILED the orchestrator's battery (3 arch
  scanner violations + 15 literal violations in the p9 file, fmt
  hunks in 5 files) — the builders' slice-scoped batteries never ran
  the repo-wide scans on the COMBINED tree. COR-1-SWEEP builder fixed
  the p9 file (embedded-SQL respell, lifted consts, port-shape
  respell); orchestrator applied cargo fmt to the residual 4 files.
  RECONCILED TREE PERSONALLY GREEN: test 1498/1498, lint clean, deny
  ok, boundaries 233/233, ruler PASS, p9 4/4, promote:: 40/40,
  candidate_only 1/1, ports 3/3, sqlite 5/5, arch 5/5, C5 teeth 1/1.
  FRESH blind EXECUTING falsifier r1 dispatched (falsifier-brief.md).
- 2026-09-04 orchestrator: falsifier r1 = NOT_CONFIRM (falsifier-round1.md):
  F-1 MAJOR executed through the real binary (condition 2 saw 2 of 4
  arms — a moved regression ruler evaluated and adjudicated cleanly) +
  F-2 MAJOR executed (instrument series paired positionally — an extra
  settlement shielded a weakened unit; real adapters PROMOTED) + F-3..F-6
  (margin >10000bps silent clamp; evaluation-record idempotency omitted
  metric/protected; case-sensitive governance-target classification;
  containment scan missed raw write classes). Every declared tooth bit;
  restores sha256-proven. COR-2 dispatched (cor2-brief.md R1–R6
  RED-first); a builder death mid-R1 was recovered by a continuation
  builder that reconciled orphaned state, verified instead of rewrote,
  and fixed the never-run-gates fallout (clippy box-sizing, literal
  scans, fmt). ORCHESTRATOR BATTERY on the COR-2 tree: 1507/1507,
  lint clean, deny ok (transient first-run failure, re-verified exit 0),
  boundaries 235/235, ruler PASS, p9 4/4, promote:: 46/46,
  candidate_only 1/1.
- 2026-09-04 orchestrator: falsifier r2 (FRESH, blind, executing;
  round-1 ledger in hand) = **CONFIRM** (falsifier-round2.md): all six
  round-1 holes re-attacked and CLOSED (the moved-ruler binary
  reproduction now dies at evaluate time typed, nothing recorded;
  the shield shape rejects by-unit; margin ceiling typed at both
  layers; re-declaration conflicts; case-normalized classification;
  raw-write classes reported by the scan); extension attacks found
  only 2 MINOR residuals (E-1 scan-token spelling escapes —
  fs::rename/copy/hard_link/remove_file/File::options/write!;
  E-2 by-unit join first-row order sensitivity — unreachable through
  the real loop, settlement commits idempotent by run+unit). All 7
  teeth RED under neutralization, 1:1 attribution; restores
  byte-identical; post-restore sweep green.
- 2026-09-04 orchestrator: CLOSURE. Done-when tooth personally
  executed: preserves_verification neutralized → p9 leg 1 RED in my
  own run → byte-identical restore (sha256 4276075e… verified OK) →
  GREEN. Closure battery personally run TWICE: 1507/1507 ×2, lint
  clean ×2, deny ok, boundaries 235/235 ×2, ruler PASS ×3, p9 4/4,
  promote:: 46/46, candidate_only 1/1, settle-path audit zero diff.
  task23 SETTLED. Ready set recomputed: task24 + task25.

