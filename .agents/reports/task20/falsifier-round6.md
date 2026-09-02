# Task 20 — falsifier round 6 (blind executing falsifier, fresh instance)

Scope: COR-R5's two fixes re-attacked (settlement-arm of the recovery/no-op
seam), plus a regression sweep across every arm earlier rounds closed, plus
the mandatory over-rejection leg. Every attack below was EXECUTED through the
real `gauntlet` binary (scratch worlds under `/tmp/gauntlet-f6`, fixture copy
+ state surgery via sqlite3) or through the shipped suites as shipped. No
repo file other than this report was written; probe files lived only in the
falsifier tempdir. `just ruler`: PASS before report.

## Verdict summary

**VERDICT: CONFIRM (0 blocking / 0 major / 0 minor / 3 informational).**
Every COR-R5 cell held; every prior arm held under one honest attack and one
over-rejection probe each; the over-rejection leg is clean (752/752 shipped
tests + executed kill-9/resume cells).

## Informational observations (no correction commissioned)

- **I-1 (consistency, dormant): full-path resume emits no settlement-forensic
  pin.** The `settlement_row_unrecoverable` pin lives only in the certified
  settled no-op close (controller.rs, COR-R5-2 branch). A genuinely
  UNSETTLED run resumed over below-anchor settlement-row damage on an
  already-settled unit discloses the divergence through the report's
  classification instead: executed cell (rounds=2 world, AK-901 settled,
  its row deleted out-of-band, resume) → exit 1 budget_exhausted (honest),
  NO re-dispatch of the settled unit, zero forensic pins, report prints
  `provisional 1` for the damaged unit with the basis line "verification
  record … is PASS but no settlement record commits it; the latest
  checkpoint projects settled but no settlement record exists". The damage
  is therefore never silent — the report names it — but the forensic pin and
  the report classification are two different surfaces and only the no-op
  close pins. COR-R5-2's commission scoped the pin to the certified no-op;
  recorded here so the asymmetry is a decision, not an accident.
- **I-2 (dormant predicate asymmetry): resume no-op requires all-`Settled`;
  `decide_progress` closes on `Settled | OutOfScope`.** A run whose units
  closed settled-or-out-of-scope would fail the no-op predicate on resume
  and take the full path (books/charges an empty round boundary, writes a
  checkpoint + event) instead of the zero-write no-op. Not reachable today:
  no shipped surface assigns `UnitStatus::OutOfScope` (grep over production
  code: only reads; assignments exist in scheduler tests only). Recorded
  before an out-of-scope producer ever lands and makes the asymmetry
  load-bearing.
- **I-3 (cosmetic): a ticket id spelled WITH its `.json` suffix** (e.g.
  `escalations resolve --ticket esc-X.json` where the queue file is
  `esc-X.json`) reads `esc-X.json.json`, fails typed with "not a valid
  record: No such file or directory". Honest and fail-closed, but the
  wording suggests corruption where the truth is a not-found id. Pure
  message-polish note; the stem form works and every refusal exits non-zero.

## A. COR-R5 re-attacks — all held

Executed on a settled happy run (3 units, settlements at event offsets
4/8/12, latest checkpoint offset 12) with durable-state surgery:

- **A1 mixed restore+pin cell (the commission's hardest shape).** Deleted
  AK-901 AND AK-902's keyed rows, regressed the close-out anchor to 6
  (strictly below AK-902's event, strictly above AK-901's). One resume:
  exit 0, `terminal=settled iterations=3` (zero booked rounds — the close
  fired before/regardless of booking, COR-R5-1), the anchored replay
  re-completed AK-902's row at the EVENT'S OWN identity (offset 8,
  settled_at 20 — the event's stored timestamp, never the recovery's
  clock), AK-901 pinned exactly ONCE (`settlement_row_unrecoverable`,
  naming only AK-901, stating the anchor bound and the
  checked-against-the-store posture), report `implemented-and-proven 2,
  provisional 1` with an honest per-unit basis. Export delta: exactly the
  one re-completed settlement record; no new events, checkpoints,
  invocations. Second resume: byte-identical export, NO duplicate pin
  (content-addressed pin idempotence).
- **A2 restore-arm-idle cell.** Row deleted, anchor regressed to 6, only
  AK-902's row (re-covered by the replay) missing: replay restored it
  idempotently, zero pins, zero booked rounds, exit 0 settled — the delta
  against the pre-resume export was exactly my own surgery (the forged
  checkpoint line + the deleted row), i.e. the loop wrote nothing to the
  run record. Recovered damage is not damage; damage the replay cannot
  re-complete is pinned, never absorbed and never fabricated.
- **A3 event+row both destroyed below the anchor.** Still honest: the
  no-op close fires on the event-ordered projection's certification, the
  pin fires (row missing), and the pin's text stays truthful — it claims
  the durable PROJECTION certifies the unit Settled, never that the event
  exists. Report downgrades the unit to provisional.
- **A4 false-pin hunt (over-rejection probe for COR-R5-2).** Anchor
  regression ALONE (no row damage): zero pins of either class, all three
  rows intact (the replay is idempotent, adds 0), report 3/3 proven, exit
  0. A run with no settlement damage pins nothing new. FAILED attack.
- **A5 false no-op hunt (BLOCKING-class attempt).** Tried every ordinary
  opening: budget-interrupted runs (AK-901 settled, others open) fail the
  predicate and take the full path with no settled-work re-dispatch;
  in-flight non-empty defeats the predicate by construction. The in-loop
  analogue (booking refusal over an all-settled projection via a carried
  completion) is STRUCTURALLY unreachable: `rounds_exhausted` is one
  predicate shared by plan-time gating and booking refusal, and
  `decide_progress` runs after booking every iteration, so any carry that
  could be absorbed at iteration k's top had a round affordable at k−1's
  end; when rounds are exhausted the carry is stranded with its unit
  honestly OPEN and the terminal is `budget_exhausted` with the unit owed.
  The only remaining openings are checkpoint-offset/unit-state forgery —
  the recorded "checkpoint() forged-row projection class (planning-layer)"
  deferral, which the code honors explicitly (it keys on the event-ordered
  projection and never validates offsets). No false no-op found.
- **A6 forensics vs open units.** Unsettled-run resumes, clean and
  damaged (cells above and the rounds=2 world): zero pins of any class in
  every cell — the recovery never pinned healthy open state, never fired
  on open units, never lied. FAILED attack.
- **A7 mixed production+settlement damage.** Settlement row (AK-901) and a
  production row (AK-903's locator) deleted together, anchor intact: both
  pin classes fired in one resume, non-masking — the settlement pin names
  only AK-901; the production pin names exactly the damaged locator
  ("produced world: the durable production row for locator … is missing
  and no EvidenceProduced event at or after the resume anchor …"), exit 0
  settled, report honest. The R3-1(d) per-locator granularity holds under
  compound damage.

## B. Regression sweep — every prior arm attacked and probed

- **Corroboration settlement gate (COR-F1).** Attack: `e2e-forged-report`
  scenario through the binary — exit 1, bounded terminal, ZERO settlements,
  zero proven, exactly one `producer_uncorroborated` pin naming unit+locator
  ("no executed passing test facet … covers the evidence workspace"). The
  gate compares the bundle's `run_id`+`iteration_no` against the settling
  loop's own facts (cross-iteration/cross-run bundles corroborate nothing)
  and the (run, iteration) binding is write-once (substitution refused,
  p2 teeth). Over-rejection: the honest happy run settles 3/3 with the
  fixture's own check actually executing.
- **Per-unit workspace anchoring + traversal/escape.** The
  `falsifier_round1_live_attacks` battery (live child processes, SIGTERM
  escalation, template attacks) passed in the 655-test batch; the e2e
  pinned-observation-manifest assertions prove work landed inside the
  admitted per-unit workspaces; p2 context-isolation teeth green.
- **load_scenario stem containment.** `GAUNTLET_MOCK_SCENARIO=../../src/lib`
  and `=e2e-happy/deep` both refused typed BEFORE any path join ("must be a
  simple fixture name (no path separators) under the scenario fixtures
  dir"), exit 1. Over-rejection: `e2e-happy` loads and runs.
- **TicketId grammar + queue read/write containment + resolve
  cross-wiring (COR-R2-2/F-3).** `--ticket ../../evil` → "not a usable
  ticket id" pre-fs, exit 1. Forged file `esc-forged-0.json` whose body
  claims `esc-6a97a6ad-0` → "esc-forged-0 is not this record: the record's
  own ticket field names … (cross-wiring defense)", exit 1, VICTIM record
  untouched (`state: open`). Queue paths derive from grammar-checked stem
  ids only.
- **Escalation list quarantine (COR-F4).** Planted a corrupt ticket: `list`
  exits 0 showing it as `QUARANTINED reason=record-invalid` (coarse class,
  no content echoed; the forged cross-wired file quarantined the same way),
  and any ACTION on a quarantined/invalid ticket fails closed. Valid
  tickets list normally (over-rejection probe).
- **Skills raw-byte digesting + lock strictness (COR-R3-2/R4-2/F-5/R2-3).**
  Shipped legs green as shipped: normalization-equivalent body under a
  reforged lock refuses byte-drift; self-consistent lock over tampered
  bodies is drift against the canonical source; forged/nested lock shapes
  refuse; install→verify round-trips; out-of-band edit named and reinstall
  repairs.
- **Writer-lock refusal + race window (COR-R3-3/R4-3).** Executed the
  REAL stale-lock case: a kill -9'd run leaves `writer.lock`; the next
  resume refuses typed naming the killed pid, the manual recovery named in
  the message works (remove → clean resume). Blank pid file and
  binary-garbage pid file both produce the hedged "stale or unreadable
  writer lock (… names pid <unreadable>)" refusal — never a live-holder
  assertion. The create-vs-pid-write race regression leg is green in the
  shipped batch; an honest single run leaves no lock behind.
- **Dry-run zero-write (D1).** `run --dry-run` in a fresh world: exit 0,
  plan preview printed (3 dispatches, units/tiers/authorities), reference
  verified against the lock; full-tree sha256 before/after IDENTICAL —
  zero files created, zero modified.
- **D2 runner fail-closed.** `runners.default` deleted → typed
  configuration failure naming both knobs (GAUNTLET.md key + env), exit 1,
  no dispatch, nothing durable written. Unknown token → typed failure
  listing the fronted tokens (`prime-agent, cli-agent, mock-agent`).
- **§20.1 argument discipline (sample).** `status/inspect/export` with
  traversal or garbage run ids → "not a valid run id (ULID grammar)",
  exit 1; unknown flags and unknown subcommands across
  run/verify/escalations/improve → exit 2 usage; `improve
  propose|evaluate|promote|rollback` → typed NOT-WIRED exit 1 naming plan
  Task 23 (never a fake success, never a panic); `init` refuses to
  overwrite an existing project; `roles list` honestly fails on a missing
  lock; `doctor` passes; `reference show` works. No command echoed
  unsanitized input into a path.

## C. Over-rejection leg — honest builders settle everywhere

- **Happy path, produced world:** fresh world, `e2e-happy` through the real
  binary: exit 0, `terminal=settled iterations=3`, implemented-and-proven 3,
  corroborated by the executed passing test facet (the happy leg IS the
  corroborated-pin probe).
- **Default produce world:** the p4/p7 proof rungs (facet-document
  contracts) exercise `MaterializeObservation` through the composed
  controller — green in the shipped batch (p7 commit_order + three crash
  windows + anchored-replay re-resume).
- **Parallel carry (max_parallel=2):** shipped
  `parallel_completions_carry_and_never_collide` green as shipped.
- **kill -9 mid-run + first/second resume (executed through the binary):**
  killed the run process at 20 ms — partial durable state (1 of 3
  settlements, run row, checkpoint iteration 2). The stale writer.lock was
  refused typed (above); after manual removal the FIRST resume settled all
  remaining units: exit 0, `terminal=settled iterations=4`, exactly 3
  settlement rows (the pre-kill settlement counted once — no settled work
  re-run), zero forensic pins. The SECOND resume was the settled no-op:
  exit 0, byte-identical export, no lock left behind.
- **Shipped suites AS SHIPPED (run, counted):** `cargo nextest run -p
  gauntlet-cli` → 97/97 (includes e2e_full_loop_mock legs 1–8 incl. 7c/7d,
  cli_surface, p1–p12, skills_roundtrip, export, escalations);
  `cargo nextest run -p gauntlet-adapter-agent-mock -p gauntlet-app -p
  gauntlet-adapter-agent-cli -p gauntlet-adapter-state-sqlite` → 655/655
  (includes the mock contract suite + scenario playback, app skills/roles/
  report/compare_deny/formation, the round-1 live and template attack
  batteries, the sqlite store suite). Total 752/752, zero skipped.
  `just ruler` PASS.

## Constraint conformance

All compile/test invocations ran under
`flock .agents/reports/task20/compile.lock` with `sccache --stop-server`
inside the same command; long runs executed in background with polling.
Probe files were created only under `/tmp/gauntlet-f6`; no `.gauntlet/` or
`.wayfinder/` file in the REPOSITORY was touched (fixture edits happened
only on copies under the tempdir); no test, fixture, or source file was
weakened, deleted, or fixed. Blindness held: no `*report*.md`,
`*progress*.md`, `critic-design.md`, `design-adjudication.md`,
`falsifier-roundN.md`, or `*.log` under `.agents/reports/task20/` was read —
only the charter and adjudication files r1–r5.

VERDICT: CONFIRM (0 blocking / 0 major / 0 minor / 3 informational)
