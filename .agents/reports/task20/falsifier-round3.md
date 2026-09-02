# Task 20 — blind executing falsifier, round 3

Fresh instance. Target: the round-2 corrections' mechanisms (COR-R2-1 unified
production key surface, COR-R2-2 escalation resolve cross-wiring defense,
COR-R2-3 strict skills-lock reader) plus the mandatory over-rejection leg.
Blindness rules honored: no *report*.md read except adjudication-r2 (and the
charter); no progress files, no critic-design, no round-1/round-2 falsifier
reports, no logs.

STATUS: COMPLETE — round finished by a resumed instance after a network cut.
Every commissioned attack in the plan below is now either a finding or a
recorded failed attack; nothing was dropped. Final counts at the verdict line.

## Attack plan

- M1 (COR-R2-1): worlds crossing; partial production rows (produce records
  ONE row per completion — first facet — while the recovery loops ALL bundle
  facets); crash between produce and checkpoint; forged/missing production
  row on a settled run; settled run with a lost evidence object; concurrent
  double resume.
- M2 (COR-R2-2): exact-match lying record; unicode/case ticket spellings;
  record whose ticket names the VICTIM while requested is the lying id;
  resolve-after-refusal; quarantine interaction.
- M3 (COR-R2-3): shape-valid locks lying about CONTENT; duplicate-but-equal
  entries; reordered arrays vs the repo's own roles.lock discipline;
  verify-after-source-revert.
- Over-rejection: honest settle in both produce worlds; parallel carry
  (max_parallel=2); kill -9 mid-run + first/second resume; corroborated-world
  pin; shipped E2E suite green as shipped.

## Code-map (what the corrections built)

- `crates/gauntlet-app/src/control/controller.rs` `ProducePolicy::
  production_recorded` (one key surface: facet-ref row for
  MaterializeObservation, locator-keyed row set for BindLocatorContent),
  consumed by `SettlementService::complete_pending_after_checkpoint`
  (`crates/gauntlet-app/src/settle/mod.rs`), threaded from
  `RunController::resume_run` (`self.produce`).
- Produce step `produce_unit_evidence` records ONE production per completion
  over the FIRST facet's ref (default world) or the locator artifact's
  content ref (produced world, with an `already_durable` idempotent skip).
- The CLI composition (`crates/gauntlet-cli/src/compose.rs`) is FIXED to
  `compose_binding_produced_artifacts` for both `run` and `resume` — the
  default world is reachable only through in-app compositions.
- `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`: `record()`
  refuses a decoded record whose `ticket` field differs from the requested
  id (typed `TicketIdentityMismatch`); `resolve()` writes back through the
  REQUESTED id; `poll()` applies the same identity binding; `list()`
  quarantines a lying record as `record-invalid`.
- `crates/gauntlet-cli/src/skills_cmd.rs` `read_lock`: closed root field
  set, exact schema token, count equality, closed entry field set, no
  duplicate names, exact owned-set membership. `skills verify` re-digests
  and re-lints the INSTALLED tree against the lock only.

## Findings

(appended as attacks execute — most severe first when finalized)

### F-R3-1 (MAJOR) — M1: damaged/missing production rows on a SETTLED produced-world run: false forensic evidence pinned + dishonest `budget_exhausted` terminal; the same damage is silently masked in the partial case

COR-R2-1's unified key surface made the honest settled no-op hold in both
produce worlds (re-proven below), but the resume-completion recovery that
consumes it is dishonest the moment a production row is missing while the run
is otherwise settled. Executed through the real binary (mock `e2e-happy`,
settled 3/3, then `evidence_productions` rows deleted from the state DB; the
`EvidenceProduced` events and the store bytes all left in place — LESS damage
than any real crash, which the A5 window explicitly names recoverable):

- ALL rows deleted: `gauntlet resume` exits 1 reporting
  `terminal=budget_exhausted iterations=4` — a fully settled run misreported
  with the exact dishonest terminal F-R2-3/COR-F6 eliminated — after pinning
  TWO false limitation records into the evidence store:
  `evidence_production_orphan_unrecoverable` / "no frozen locator agrees with
  the orphan EvidenceProduced event for sha256:821b… — its locator is not
  durably recoverable". Both halves of that statement are false: (1) no
  `EvidenceProduced` event exists for those refs at all (the durable events
  name the ARTIFACT content refs, not the facet refs the recovery mints);
  (2) the referenced bytes ARE in the store. The recovery minted the FACET
  refs (the default world's key), found no matching orphan event, and
  recorded "unrecoverable" instead of the truth ("this world's production
  rows are missing; their events name artifact refs").
- ONE row deleted (other locators intact): the no-op HOLDS with zero writes
  and the missing row is never repaired or mentioned — the recovery's
  `applied` flag is facet-wide across units, so any one intact locator masks
  a sibling locator's missing row.

Net: the same damage class produces opposite, both-dishonest outcomes
(false "unrecoverable" forensic evidence + misreported terminal vs silent
blindness). Reachability has TWO stories, both real: (a) external state
damage (as executed — no honest run loses rows on its own); (b) the store's
own A5 crash window — `record_evidence_production` appends the
`EvidenceProduced` event and inserts the row as two adjacent autocommit
statements, so a kill between them leaves event-without-row, the exact
window the store's own error text says "replay recovery should complete".
In the DEFAULT world that window IS completed by the recovery (facet-ref
mint matches the facet-ref event); in the PRODUCED world it is NOT — the
recovery mints the facet ref while the durable orphan event names the
ARTIFACT content ref, so the completion can never match and the honest
crash path also ends in the false "unrecoverable" pin (the run itself can
still settle on a later re-production, which re-appends a second event and
lands the row). The mint half of the recovery is still facet-keyed in BOTH
worlds — the half COR-R2-1 did not unify.

### Failed attack — M2 (COR-R2-2): every commissioned cross-wiring shape refused typed; victims untouched

Executed through the real binary against a durable (bounded, unsettled) run
with hand-crafted ticket files under `state/escalations/`:

- Exact clobber: file `esc-A.json` whose decoded `ticket` field names the
  VICTIM `esc-B`; `gauntlet escalations resolve --ticket esc-A …` exits 1 with
  the typed cross-wiring refusal ("the record's own ticket field names esc-B
  …"); `esc-B.json` byte-identical after (sha256 before/after), `esc-A.json`
  untouched. The write-back never fires (the read refuses first) and is bound
  to the REQUESTED id by construction (`queue.resolve` writes through
  `contained_ticket_path(requested)`).
- Reverse direction: `resolve --ticket esc-B` over the victim's honest file
  reaches the D9 settlement-service gate and refuses there (no PASS
  verification record; AK-062); the queue record stays open, byte-identical.
- Case spelling: record field `ESC-c` vs requested/file `esc-c` → typed
  identity-mismatch refusal (exact string compare, no normalization).
  Unicode lookalike (`еsc-u`, Cyrillic е) in the field vs ASCII stem → the
  listing quarantines it; a consistently-spelled unicode id is a valid
  opaque id for its OWN file (`еsc-u.json`), which does not alias the ASCII
  file on this filesystem.
- Resolve-after-refusal: the lying ticket refuses again, no state movement;
  the quarantined corrupt file (`esc-z`) refuses at decode, coarse JSON error
  only.
- Quarantine interaction: `escalations list` shows the liar, the case
  variant, the lookalike, and the corrupt file as `QUARANTINED
  reason=record-invalid` (coarse class, no content echoed) while honest
  records list normally; exit 0.
- poll side (`queue.poll` on a lying file) refuses on the same identity
  binding in code (`StateReadFailed`, cross-wiring defense); the adapter's
  shipped contract suite covers the read path.

No over-rejection observed: honest tickets list, and honest resolves refuse
only at the settlement-service gate with the queue untouched.

### Failed attack — M1: lost evidence object / forged production row / resume xN / concurrent double resume on a settled run

- Settled run with the artifact OBJECT deleted from the evidence store:
  `resume` holds the settled no-op, zero writes, exit 0. Honest (settlement
  is already durable; the loss surfaces at `verify`/`inspect`, not at §18.3
  recovery).
- FORGED production row (foreign ref/instrument/iteration inserted at an
  honest locator): the locator-keyed read cannot distinguish it and the
  no-op holds; nothing on the settled path consumes the forgery.
- Resume xN: three consecutive resumes are identical no-ops.
- Concurrent double resume (two processes, same run): settled world — both
  exit 0 settled no-ops, settlements stay 3, no corruption; unsettled
  (killed-run) world — both drivers interleave on the same durable run
  (both report `iterations=6`), no store corruption, settlements stay 0.
  Recorded as M-R3-1 below: there is no single-writer admission on
  run/resume; durable invariants (write-once rows, budget restore) held
  under this attack, but two concurrent drivers each meter against their own
  restored budget and would double-dispatch real builders.

### M-R3-1 (MINOR) — no single-writer guard on `gauntlet run` / `gauntlet resume`

Two concurrent resume processes over one run are both admitted; the durable
stores' write-once rules kept every invariant green under the executed
attack, but nothing prevents two drivers interleaving dispatches and
double-spending the run's budget (each process meters from its own restored
snapshot). Pre-existing surface (not a COR-R2 regression); the mandated
"concurrent double resume" leg exposed it honestly: bounded, no corruption,
but unguarded.

### F-R3-2 (MAJOR) — M3 (COR-R2-3): a shape-valid lock that lies about CONTENT verifies clean — `skills verify`'s only anchor is the lock itself; the mirrored roles surface refuses the identical attack

The strict READER landed (closed field sets, exact token, count equality,
duplicate + owned-set membership — all re-proven below), but the verify it
feeds re-digests the INSTALLED tree against the LOCK only. It never consults
the provenance source tree (`.agents/skills/`, the repo-owned reviewed
copies), and the skills lock has no canonical-form closure. Executed through
the real binary in a scratch world (repo-owned skill sources copied in,
`skills install` → `skills verify` baseline clean):

- CONTENT LIE SUCCEEDS: rewrote ONE installed `SKILL.md` body with different
  lint-clean procedural prose, updated its `digest` in the lock (plain
  sha256 over the file bytes — accepted), and `gauntlet skills verify`
  exits 0: "verified 8 skill(s) — no drift, lint clean". A shape-valid lock
  whose digests are internally consistent but bind TAMPERED bodies is
  blessed.
- SOURCE-REVERT BLINDNESS SUCCEEDS: edited a skill's SOURCE file after
  install (installed body + lock untouched) — `skills verify` exits 0. The
  mirrored roles command's own contract names exactly this class a typed
  failure ("source edit since install").
- Mirror proof, same world, same adversary: `roles install --harness
  prime-agent`, then tampered the installed role body AND re-forged
  `roles.lock` consistently (entry digest + `lock_digest` self-digest
  recomputed) — `gauntlet roles verify` FAILS typed ("source file modified
  since install") because the source tree anchors it; then a plain
  post-install source edit also FAILS typed. The skills surface passes both.
- Reordered-but-honest lock arrays VERIFY clean (the roles lock's
  canonical-form closure refuses any hand-reordered byte form as tamper).
  Duplicate-but-equal entries and every strict shape from the round-2
  corrections are still refused typed (see the failed attacks).

Mitigations, recorded honestly: the verify path re-runs the task19 lint over
the tampered body (my injected prose passed; the lint blocks the blatant
control-logic vocabulary class, not arbitrary procedure text), and the
exploit requires write access to the state dir — exactly the adversary the
roles surface's LockTampered discipline treats as in-model ("locks are
runtime-written documents, never hand-edited ones"). Charter D8 commissioned
skills to MIRROR roles install with a lock; COR-R2-3's word was "the repo's
standard strict-codec discipline". The anchor half of that discipline is
missing, and the mandated round-3 attack ("digest fields consistent but
pointing at swapped/tampered skill bodies") walks straight through it.

### Failed attack — M3: the strict reader's own shapes and the honest round trip

Executed through the real binary against a freshly installed lock: a
duplicate-but-equal entry (count adjusted to match) is refused typed
("records a duplicated skill entry"). The round-2 corrections' shapes (wrong
schema token, lying count, narrowed entry set over a consistent count,
unknown root/entry fields) remain covered by the shipped strict-shapes tooth
in `crates/gauntlet-cli/tests/skills_roundtrip.rs`. The honest install →
verify round trip stays byte-for-byte green.

### Failed attack — M1: honest settled resume in the produced world (the correction's own contract)

Baseline world (mock `e2e-happy`, 3/3 settled): FIRST `resume` exits 0,
`terminal=settled iterations=3` (unchanged), ZERO file changes over the whole
scratch (sha256 before/after over state DB, evidence store, pins). The
COR-R2-1 mechanism holds for every honest settled run I could construct.

### Failed attack — M1: two-facet bundles do not break the produced-world no-op

The produced-world recovery's locator-keyed read is facet-INsensitive, so the
2-facet iteration-3 bundles the composed world really produces (filesystem +
test facets, read from the durable bundle) never trip the per-facet recovery.
Latent asymmetry recorded under F-R3-1's analysis: in the DEFAULT world a
multi-facet bundle WOULD trip per-facet limitations (the produce step records
only the first facet's ref there) — no shipped composition fronts >1 observer
in the default world (verified: p4/p5/p6/p7/p11/g7/g10 test compositions and
the in-app fixtures all use single-observer vecs; the CLI is fixed to the
produced world), so this stays latent, not blocking.

## Spot-verify of F-R3-1's central claims (code read; no scope extension)

All three load-bearing claims re-confirmed at the source, cheaply:

- The recovery's mint half is facet-keyed in BOTH worlds:
  `complete_pending_after_checkpoint` mints with the facet's OWN
  `evidence_ref` (`crates/gauntlet-app/src/settle/mod.rs:933`); the threaded
  `produce` policy decides only the `applied_here` read
  (`ProducePolicy::production_recorded`, settle/mod.rs:914), never the mint.
  The produce step in the produced world records the ARTIFACT content ref
  (`crates/gauntlet-app/src/control/controller.rs:3477`-3552), so an orphan
  `EvidenceProduced` event (artifact ref) can never be completed by a
  facet-ref mint — the un-unified half, exactly as drafted.
- Event + row are two adjacent autocommit statements:
  `append_event` (`crates/adapters/gauntlet-adapter-state-sqlite/
  src/state_store.rs:2200`) then the `INSERT INTO evidence_productions`
  (:2214), with the store's own error text "replay recovery should complete
  the row before re-recording" (:2234) — the window F-R3-1 names.
- The pinned limitation text quoted in F-R3-1 is verbatim the fallback at
  settle/mod.rs:996, and the `applied` flag is facet-wide across units
  (:901-902, :993) — one intact locator masks a sibling locator's missing
  row.

## Failed attack — over-rejection: honest recovery through a REAL kill -9 mid-run, first/second resume (the round's mandated leg)

Executed through the real binary in a fresh scratch world (fixture `e2e-happy`,
produced world, same env shape as the shipped E2E). The whole honest run
completes in ~50 ms under the deterministic clock, so the kill was raced
across fine-grained delays until it landed mid-work — and it landed in a
PRECISE crash window, which makes this a stronger recovery proof than
planned:

- Kill state (20 ms): process group SIGKILLed with `settled=0`, no report;
  the durable log holds `checkpoint(iteration 1)`, `invocation_admitted`,
  `evidence_produced` (offset 3, row present — no production orphan in THIS
  kill), and `settlement_committed` (offset 4) while the settlements table
  holds ZERO rows — the event-without-row settlement window the store's own
  comments name recoverable.
- FIRST resume: exit 0, `terminal=settled iterations=3` — the orphan
  settlement completed from its durable offset-anchored event, the remaining
  units dispatched and settled honestly (3 settlement rows, 3 production
  rows at the end), and NO limitation pin of any kind (`unrecoverable`
  grep over the whole evidence tree: empty). No dishonest
  `budget_exhausted`; the budget reported honestly (`rounds=2` post-resume).
- SECOND resume: exit 0, `terminal=settled iterations=3`, and ZERO byte
  writes across the entire state+evidence tree (sha256 over every file
  before/after, identical). THIRD resume: identical no-op.

This is the honest counterpart of F-R3-1: where the orphan event and the
recovery's mint agree on the ref surface (settlement events are
offset-anchored, not ref-keyed), the crash-window machinery recovers
exactly as designed. F-R3-1's defect remains confined to the
production-orphan half in the produced world (the ref-surface mismatch);
this kill did not happen to straddle THAT window and the attack was not
extended to force it.

## Failed attack — over-rejection: the shipped E2E suite green AS SHIPPED (happy + corroborated pin + parallel carry + teeth)

`just test-package gauntlet-cli` under the compile lock: **84 tests run,
84 passed, 0 skipped, exit 0** — unmodified, as shipped. The mandated legs
are all in that run:

- Happy path, produced world, CORROBORATED: `the_composed_loop_settles_
  every_unit_and_pins_the_artifacts` — the fixture fronts the real `test`
  facet (GAUNTLET_TOOL_TEST binds the project's own check.sh, which the
  instrument actually executes), and every unit settles. This is the
  corroborated-world over-rejection pin: honest builders settle exactly
  when the executed instrument corroborates them.
- Parallel carry: `parallel_completions_carry_and_never_collide` —
  `max_parallel: 2` over the multi-unit fixture (COR-D2), honest carry, no
  collision.
- Settled no-op: `the_first_resume_of_a_fully_settled_run_is_a_settled_
  noop` — COR-R2-1's own shipped pin, green.
- Discrimination teeth: `the_lying_builder_is_never_settled`,
  `the_report_forging_liar_is_never_settled` (the round-1 blocking attack
  as a shipped tooth), `the_dry_run_preflight_writes_nothing`.
- BOTH produce worlds settle honest builders: the produced world through
  the shipped E2E legs above and the kill -9 recovery leg; the DEFAULT
  world through the shipped in-app proofs in the same green run
  (p1-p12/g6/g7/g10 decide through `compose()`, including the p4 corrective
  loop and p7 recovery). Skills/roles strict shapes
  (`skills_roundtrip`) green.

## Severity record (final)

- **F-R3-1 — MAJOR (kept, with reason).** Not blocking: every HONEST path
  holds — the settled no-op in both worlds (proven), and now a real kill -9
  crash recovery through the real binary (proven above). The defect fires
  only on state damage or the narrow production-row crash window. It stays
  MAJOR because when it fires it pins FALSE forensic evidence into the
  durable store ("no frozen locator agrees … not durably recoverable" over
  bytes that ARE in the store, citing events that do not exist for those
  refs) and misreports a fully settled run with the dishonest
  `budget_exhausted` terminal — dishonesty in pinned evidence, not merely a
  wrong exit code — and the same damage is silently masked in the
  one-row-missing case. Code-confirmed root: the recovery's mint half is
  still facet-keyed in both worlds; COR-R2-1 unified the read, not the mint.
- **F-R3-2 — MAJOR (kept, with reason).** The mandated round-3 M3 attack —
  a shape-valid lock whose digests are internally consistent over TAMPERED
  bodies — verifies clean through the real binary, and verify-after-
  source-revert is blind, while the mirrored roles surface refuses the
  identical adversary typed (re-proven in the same world). COR-R2-3's word
  was "the repo's standard strict-codec discipline"; the strict-shape half
  landed, the anchor half (verify against the provenance source, canonical
  lock form) did not. Not blocking: the adversary needs state-dir write
  access and the lint re-run blocks the control-logic vocabulary class —
  but the drift tooth the surface exists to provide is silently defeated
  for everything else.
- **M-R3-1 — MINOR (kept).** No single-writer admission on run/resume;
  bounded and corruption-free under the executed attack, but two drivers
  would double-dispatch real builders against independently restored
  budgets. Pre-existing surface, not a round-2-correction regression.

## Operational note (recurrence of the known sccache class, new variant)

The charter's protocol (stop-server INSIDE the flock'd command) was
followed, but this time the `sccache --stop-server` invocation itself hung
while holding the inherited lock fd (~30 s, previously the daemon held it
after shell exit). Recovery that worked: bounded wait, then kill the
stop-server process; the lock released and no waiter was stranded. The
in-test battery itself completed green (84/84, exit recorded in the run
log) before the hang.

## Verdict

Blindness held for the whole round: no builder/critic history, no progress
files, no round-1/2 reports, no logs; only the charter, the two
adjudications, and this round's own partial report were read. No test,
fixture, or source file was modified; all probe material lived in
/tmp/gauntlet-f-r3-finisher and no tracked file was touched.

VERDICT: NOT_CONFIRM (0 blocking / 2 major / 1 minor / 0 informational)
— F-R3-1 (major), F-R3-2 (major), M-R3-1 (minor).
