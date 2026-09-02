# Task 20 — falsifier round 5 (blind executing falsifier, fresh instance)

Round scope: the three COR-R4 mechanisms only — (1) the settled no-op
predicate (`resume_run`, controller.rs), (2) skills raw-byte digesting
(`digest_raw` / `skills verify`), (3) the writer-lock Held refusal
(hedged message + bounded reread) — plus the mandatory over-rejection
leg. Blindness held: no report/progress/design files beyond the
adjudication r1–r4 set was opened. All probes ran in
`/tmp/falsifier-r5/**`; no tracked file was modified; the repo's
`.gauntlet/`, `.wayfinder/`, `.agents/` trees were never written by a
probe (probe worlds carry their own copies under /tmp).

Executed through the real composed binary (`target/debug/gauntlet`,
current build) against the shipped `tests/fixtures/target` fixture with
the e2e leg's environment recipe, unless a cell names a shipped test.

## Findings (most severe first)

### F-R5-1 (MAJOR) — the settlement arm of the recovery-completed shape
still books an iteration over a fully settled run, and with the shipped
budget posture reports a dishonest `budget_exhausted` terminal

COR-R4-1's contract sentence is general: after a recovery that COMPLETES
on an otherwise-all-settled run, the resume must "close as an honest
settled no-op (terminal=settled, exit 0, zero additional iterations) —
never book a new iteration round for a run whose durable projection is
fully settled." The shipped fix implements this for the PRODUCTION arm
(re-verified below, cell C). The SETTLEMENT arm — `completion.
settlements_completed > 0`, the case the predicate's own doc comment
deliberately routes to the full path ("the normal path's reconciliation
checkpoint below must record it") — re-enters `drive`, where the fresh
iteration's round booking is REFUSED by the recovery-reserve rule, the
refused booking sets `terminal=BudgetExhausted`, and `decide_progress`'s
all-closed branch (which would have closed the run honestly as settled)
is skipped because the loop only calls it `if state.terminal.is_none()`
(controller.rs, the §11.1 iterate block around `book_iteration_round`).
Result: a run whose every unit is SETTLED, whose report prints
`implemented-and-proven 3`, exits 1 with `terminal=budget_exhausted`.

Reproduction through the real binary (cell D,
`/tmp/falsifier-r5/world-cellD`, fixture budget: rounds=4,
recovery_reserve_pct=15; the close-out checkpoint restores rounds_spent=3
so the reserve refuses a 4th round):

- happy run settles 3/3 (iterations=3);
- surgery (the exact compound shape the shipped COR-R4-1 leg uses as
  given damage): delete the keyed settlement row of AK-901 AND regress
  the latest checkpoint's `event_log_offset` to 3 (below the settlement
  event at offset 4);
- `gauntlet resume` → **exit 1, `terminal=budget_exhausted
  iterations=4`** while printing `implemented-and-proven 3`. The
  recovery half ran honestly (the settlement row was re-inserted;
  `settlements` count back to 3; a reconciliation checkpoint at
  iteration 4/offset 14 landed) — the loop half burned the round into
  the refused booking and misreported the terminal.

Two aggravating observations:

- With rounds headroom (`GAUNTLET_MAX_ROUNDS=8`, cell D-budget): the
  same shape exits 0 `terminal=settled iterations=4` — one BURNED,
  zero-dispatch iteration on a fully settled run, the exact
  "books a new iteration round for a run whose durable projection is
  fully settled" outcome the correction's contract sentence forbids.
- The shape is reachable without any anchor forgery by an ordinary
  §16.3/A5 crash (cell D-natural): kill the run between the last
  unit's produce/settlement events and the close-out checkpoint (the
  natural end-state: latest boundary checkpoint anchor at offset 8,
  below the settlement event at 12; keyed row missing). Simulated by
  deleting ONLY the close-out checkpoint row (the crashed process
  never wrote it) and the AK-903 settlement row — resume → exit 0
  `terminal=settled iterations=4`: the honest terminal, but still the
  extra booked round. The exit-1 variant additionally needs the
  close-out checkpoint present with the anchor regressed (the same
  given-damage posture the commissioned COR-R4-1 leg itself uses for
  the production twin — and the production twin under IDENTICAL given
  damage closes as the honest settled no-op; the settlement twin does
  not).

What the predicate decided is correct in every cell: with
`settlements_completed=1` durable state HAD moved and the no-op
correctly refused to short-circuit. The defect is the full path's
closing behavior: a reconciliation checkpoint does not require booking
a fresh iteration round, and a refused booking must not preempt
`decide_progress`'s all-closed settled close on an all-settled run.
This is the same dishonesty class F-R4-3 named ("reporting a
`budget_exhausted` terminal that misrepresents a fully settled run"),
one recovery arm over.

Suggested shape (not prescribed): after §18.3 step-1 completes and the
event-ordered projection is all-settled with nothing in flight, close
after recording the reconciliation checkpoint (settled, exit 0, zero
booked rounds) — i.e., re-assess the terminal projection after the
completion for BOTH completion arms, or let `decide_progress`'s
all-closed branch run even when the round booking was refused.

### F-R5-2 (MINOR) — below-anchor settlement-row damage is recovered by
the production arm with per-locator forensic pins but silently by the
settlement arm

Cell D2 (`world-cellD2`): delete a keyed settlement row while leaving
the close-out anchor intact (the event sits below the anchor). The
resume closes as the settled no-op (correct: the event-ordered
projection says settled, and the event is the durable truth), but the
keyed row stays missing with NO forensic record, and the run's own
`export`/`inspect --settlements` now show 2 of 3 settlements while the
report still prints `implemented-and-proven 3`. The production arm pins
exactly this shape (missing row, anchor-bounded "no EvidenceProduced
event at or after the resume anchor" records — shipped leg 7). The
settlement arm's replay is anchor-bounded by §18.3 design, so nothing
false is asserted — but the asymmetry leaves durable keyed-state damage
invisible to every recovery surface, on a run that was just certified
by an honest settled no-op. Recording, not prescribing: a cross-check
(all-settled projection ∧ missing keyed rows ⇒ pin) would close it
without touching the anchor rule.

### F-R5-3 (COSMETIC, no action required) — wording imprecision on an
unreadable-as-text installed skill

`skills verify` reports a non-UTF-8 installed SKILL.md as "'name' is
MISSING from the installed tree: stream did not contain valid UTF-8"
(the file exists; the text read failed). Fail-closed and drift-counted
(cell T8), but the token says MISSING for an unreadable file. The same
wording classes existed pre-COR-R4-2; noted for the failure-locus
discipline only.

## Fixed mechanisms — attacks executed and refused (fail-closed verified)

### 1. Settled no-op predicate — recovery × settled-projection matrix

All through the real binary, produced-artifact world (the composition's
world):

| cell | damage | projection | recovery outcome | observed resume |
|---|---|---|---|---|
| A | none | all settled | nothing to do | exit 0, `settled`, iterations=3 unchanged; whole world (70 files incl. state DB) byte-identical across resume 1 AND resume 2 |
| C | 1 production row deleted + anchor regressed below the produce event | all settled | production COMPLETED (re-mint) | exit 0, `settled`, iterations=3, zero burned; row restored on the event's identity (shipped COR-R4-1 leg independently re-verified) |
| D | settlement row deleted + anchor regressed below settlement event | all settled | settlement COMPLETED | **exit 1 `budget_exhausted` iterations=4 — F-R5-1** |
| D-budget | same, rounds=8 | all settled | settlement COMPLETED | exit 0 `settled` iterations=4 (burned round — F-R5-1) |
| D-natural | close-out checkpoint absent + settlement row missing (real A5 crash end-state, no forgery) | all settled (via event-ordered upgrade) | settlement COMPLETED | exit 0 `settled` iterations=4 (burned round — F-R5-1) |
| D2 | settlement row deleted below INTACT anchor | all settled | nothing (anchor-bounded) | exit 0 `settled` iterations=3; row silently missing — F-R5-2 |
| G | anchor regressed only, rows intact | all settled | applied-checks pass, nothing to mint | exit 0 `settled` iterations=3; no pins needed, no iteration; logical durable state untouched |
| H5 | production row deleted + evidence object bytes deleted + anchor regressed | all settled | recovery LIMITS OUT | exit 0 `settled` iterations=3; one honest forensic pin (`evidence_production_orphan_unrecoverable … does not resolve in the evidence store`); no fabricated digest, no burned iteration |

False no-op hunt: every constructible state in which the predicate held
carried a genuinely all-settled EVENT-ORDERED projection. A genuinely
unfinished unit never passed: cells with an Open unit in the projection
(D-natural pre-upgrade, K-A2 below) took the full path; a missing
projection entry or an empty frozen membership fails the predicate
conservatively; the only route that hides unfinished work behind a
settled projection remains the recorded forged-checkpoint
projection class, which this round left untouched per the
adjudication's scope note. The settlement conjunct (`==
0`) demonstrably gates correctly (cells D*: the predicate refused to
no-op exactly when the recovery had completed durable state).

### 2. Skills raw-byte digesting — reforged and partial-edit shapes

Probe world `/tmp/falsifier-r5/skills-probe` (repo-owned sources copied;
real binary via `GAUNTLET_PROJECT_ROOT`):

- Honest round trip: `install` → `verify` clean; re-install repairs
  byte-exactly (T1–T8 all end with a clean re-install+verify).
- T1 trailing-LF-only added to an installed body, honest lock →
  `DRIFTED`, both RAW digests named, no clean-verify lie (the old
  normalizer erases exactly this shape).
- T2 lone-CR re-spelling of the installed body (normalizer-erased
  shape), honest lock → refused (typed lint-half refusal; digest half
  would refuse identically).
- T3 CRLF + trailing-blank body, lock REFORGED over the swapped bytes'
  RAW digest → `SOURCE DRIFTED` naming the forged lock digest and the
  source's raw digest — the canonical-source anchor catches the
  internally consistent forgery.
- T4 the normalized-reforge fixpoint: lock re-forged with the
  NORMALIZED digest of the swapped body — equal to the honest lock
  digest (the source is normalization-canonical), so the "reforge"
  cannot land; the drift is still refused at the installed half,
  naming both digests, never a clean verify over non-canonical bytes.
- T5 partial-line edit (one word), honest lock → `DRIFTED`.
- T6 partial-line edit + lock re-forged over the edited bytes →
  `SOURCE DRIFTED` (self-consistent tamper refused by the source
  anchor).
- T7 BOM-prefixed installed body → refused (lint: BOM typed refusal;
  raw digest would refuse identically).
- T8 invalid UTF-8 in the installed body → refused, drift-counted
  (wording note: F-R5-3).

Roles/skills identical discrimination (T9, probe world
`/tmp/falsifier-r5/roles-probe`, harness prime-agent): the SAME
CRLF+trailing-blank installed-body swap under roles → `drift detected …
installed artifact modified out-of-band` with both raw digests
(refused); with the roles.lock digest re-forged over the swapped bytes
→ refused by the lock's own body self-digest. Both surfaces call every
shape drift; neither prints a clean verify over tampered bytes
(skills' source anchor and roles' lock self-digest are different
refusal mechanisms for the same forgery — discrimination outcome
identical).

### 3. Writer-lock refusal — race, blanks, stale recovery, both surfaces

All through the real binary (`world-wl`):

- W1a rapid double acquire ×3 rounds: every round admits EXACTLY one
  driver (the honest settled no-op) and refuses the other typed.
- W1b 8-way burst: 1 admitted (settled no-op), 7 typed refusals, zero
  non-typed stderr lines, durable record byte-identical across all
  waves, no lock debris.
- W6 live-holder pid (the driving shell itself): `gauntlet resume` AND
  `gauntlet run` both refuse with the hedged "stale or unreadable
  writer lock (… names pid …)" — never a present-tense holding claim;
  `another writer holds` appears nowhere; manual recovery + no-takeover
  named; exit 1 both surfaces.
- W2 blank lock (the race loser's view): refusal after the bounded
  reread window — measured 117 ms wall (window ≈ 10×10 ms) vs 6 ms when
  the read errors — reported as `<unreadable>`, no blank where the pid
  belongs, lock left for the operator.
- W3 non-UTF-8 garbage lock: typed refusal plus "(the lock file could
  not be read: stream did not contain valid UTF-8)".
- W4 the lock path replaced by a DIRECTORY: typed Held refusal with the
  read error ("Is a directory") — fail-closed, no IO panic.
- W5 stale-lock manual recovery end-to-end: planted dead-pid lock →
  refusal naming the pid, the manual removal, and the no-takeover
  posture, lock preserved → operator removes → resume proceeds to the
  honest settled no-op → no lock left behind.
- Crash interaction (K-A2): a kill -9'd run leaves its writer.lock;
  the next resume refuses with the hedged stale-lock message exactly
  as designed (fail-closed over crash debris), and proceeds only after
  the operator's removal.

## Over-rejection leg — honest builders settle in both produce worlds

- Happy path, produced world (the binary): cell A's run — exit 0,
  `terminal=settled iterations=3`, `implemented-and-proven 3`.
- Happy path, default world: shipped controller-level suites
  (p4_corrective_loop, p5_hard_gate, p6_budget_control, p7_recovery,
  g7_tier_downgrade exercise `RunController::compose`) — green in the
  battery below.
- Parallel carry (`GAUNTLET_MAX_PARALLEL=2` through the binary):
  exit 0 `terminal=settled iterations=3`; three distinct observation
  bundles (one per iteration, never a shared binding); first resume
  after the parallel run is the settled no-op.
- Kill -9 mid-run + first/second resume (produced world, K-A2): crash
  during iteration-2 drain (AK-902/AK-903 open, one admitted dispatch
  orphaned) → typed stale-lock refusal (crash debris) → operator
  removal → FIRST resume re-dispatches exactly the interrupted units
  (invocation admissions 2 → 4; already-settled AK-901 NOT
  re-dispatched), exit 0 `terminal=settled iterations=4`,
  `implemented-and-proven 3` → SECOND resume is the settled no-op at
  iterations=4. The predicate correctly did NOT no-op the crashed run.
- Corroborated pin: the settled legs' PASS decisions are the
  instrument-corroborated shape (executed passing test facets over the
  evidence workspace); the uncorroborated side is pinned by the
  shipped liar legs (`the_lying_builder_is_never_settled`,
  `the_report_forging_liar_is_never_settled`).
- Shipped E2E suite AS SHIPPED, counted:
  `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock` →
  **10/10 passed** (happy+pin, lying builder, forging liar, dry-run,
  parallel carry, settled no-op, rows-deleted honest, damaged-locator,
  COR-R4-1 recovery-completed no-op, single-writer guard).
- Full battery: `just test` (workspace) — result recorded at the end
  of this file.

## Disciplines held

- Every cargo/just invocation ran inside
  `flock .agents/reports/task20/compile.lock -c '…; sccache --stop-server'`;
  long runs backgrounded and polled.
- No shipped test, fixture, or source weakened, deleted, or "fixed";
  the only repo-tree write is this report. Probe surgery ran on probe
  worlds under /tmp/falsifier-r5 (including SQLite state surgery and
  evidence-object deletion on those copies).
- `just ruler` PASS before the round (answer-key
  ccc7c99be3d3b60488930eed9975dc6d67378a84bf3b13910b48511e9c228859,
  manifest 95331d3e345fa799e9840dbdb02b60b7c41c73be84c96fb7ecc1068775ba5e94);
  re-run after the battery (result below).

## Battery and closure

- Shipped E2E suite as shipped: `cargo nextest run -p gauntlet-cli
  --test e2e_full_loop_mock` → 10/10 passed (counted above).
- Full battery: `just test` (workspace, unmodified) → **1371/1371
  passed, 0 skipped, rc=0**.
- `just ruler`: PASS before and PASS after; frozen digests identical
  (answer key `ccc7c99b…`, manifest `95331d3e…`).
- Repo cleanliness: no tracked file was modified by this round (the
  working tree's tracked modifications are the orchestrator's landed
  COR-R4 state, untouched); probe artifacts live only under
  `/tmp/falsifier-r5`; the only repo-tree write is this report file.

## VERDICT

NOT_CONFIRM (0 blocking / 1 major / 1 minor).

- F-R5-1 (MAJOR): the settlement arm of the recovery-completed shape
  books a fresh iteration round over a fully settled run and — under
  the shipped budget posture (4 rounds, 15% reserve) — closes
  `terminal=budget_exhausted` exit 1 over a run whose every unit is
  settled, via `decide_progress` being skipped after the refused
  booking. The fix's own contract sentence ("never book a new
  iteration round for a run whose durable projection is fully
  settled") is violated by this arm; the dishonest-terminal variant
  reproduces under the same given-damage posture the commissioned
  COR-R4-1 leg uses for the production twin, and the burned-round
  variant reproduces from an ordinary §16.3/A5 crash with no forgery.
- F-R5-2 (MINOR): below-anchor settlement-row damage closes silently
  (no forensic pin, export/inspect show 2 of 3 settlements) where the
  production arm pins the identical shape.
- F-R5-3 is a note, not counted: pre-existing wording imprecision
  ("MISSING" for an unreadable-as-text installed skill), fail-closed,
  outside the three fixed mechanisms' contracts.

Everything else attacked held: the no-op predicate never held for a
genuinely unfinished unit in any constructible cell; skills
raw-byte digesting refused every reforged/partial/BOM/non-UTF-8 shape
with roles discriminating identically; the writer-lock refusal was
typed, hedged, and fail-closed on every path including the crash-debris
and 8-way race; and the over-rejection leg is clean (honest builders in
both produce worlds, parallel carry, kill-9 + first/second resume
honest, corroborated pins, shipped E2E 10/10 as shipped, full battery
1371/1371).
