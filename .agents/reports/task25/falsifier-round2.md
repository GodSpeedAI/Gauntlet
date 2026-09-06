# Task 25 — blind EXECUTING falsifier, round 2 (focused): report

Falsifier: round-2 blind executing falsifier. Fresh context. Blindness held:
no b1-/cor1-/cor2-/critic-/IN-FLIGHT/falsifier-round1 files opened. Inputs:
`charter.md`, `falsifier2-brief.md` (binding brief), the spec, the ruler,
the delivered tree.

`just ruler` at session start: **PASS** (`verify-ruler.sh: PASS — answer key
and manifest digests match the frozen lock`), 2026-09-05.

Baseline sha256 of every file this round may mutate (restore proofs reference
these):

```
b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0  crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs
9f7197e42f0edd9df6df0fb270fe6f51093c7b75367f7b9e5e0bb7d9b55de8e7  crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs
f30fcc847db4c63b5c6595c4376bdfd95189dde0bc2033545b965480af9e2d66  crates/adapters/gauntlet-adapter-agent-cli/tests/cor2_seam_and_recovery_attacks.rs
493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181  crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs
40a4a5757b0dd0a27978c3e4d6295a94fed97863ca1542bfa68aa4ebc1dfc170  crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs
0ea3ae3d973138f54db48642c5db98b4d3d83c4206d8d78d3b1c6bba18cebc9b  crates/gauntlet-app/src/report/mod.rs
fddf3f5e6b1d809965adfc99bd8ddfc71d56158223d25d26c0bb772a07e07e91  crates/gauntlet-app/src/control/controller.rs
6d0e610688dee468e08a19a88229c97b8ade0f9e752f4a3edb706d4b33c73b72  crates/gauntlet-app/tests/report.rs
075363abd55bbd9f64eda9f7c0e1c8256caaf3973f128ed6fdfe1e5cdc2f3ba8  crates/gauntlet-ports/src/state_store.rs
94b79743e3cf95a1d4c24942305b4413f6465a0f0b929973ecff876c6eb1ea7b  crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs
88a84f3a811fd745e2b3f47eeb4e021ce3ab2060e40fd5f05393fe9d497a743b  crates/adapters/gauntlet-adapter-state-sqlite/src/encode.rs
```

## Baseline battery (run 1 of the required two per E2E target)

EXECUTED 2026-09-05 (flock'd, default threading):

```
cargo nextest run --test e2e_full_loop_real_runner  → 2 passed (10.7s, 15.7s)
cargo nextest run --test e2e_recovery               → 3 passed (15.7s, 15.7s, 18.8s)
cargo nextest run --test e2e_budget                 → 2 passed (9.5s, 15.5s)
```

7/7 green. (The invocation's overall exit was 2 from the trailing
`sccache --stop-server` with no server running — benign; every test line
PASS.)

## Attack group A4 — escalation dedup (F-R5, `queue.rs` + suites)

### A4-M1 — cross-run dedup mutant: **SURVIVING MUTANT = FINDING F2-1 (MAJOR)**

Mutation (EXACT, in
`crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`, the
`escalate` dedup scan): the dedup predicate

```rust
.find(|record| {
    record.is_open() && record.run_id == item.run_id && record.summary == item.summary
})
```

was mutated to

```rust
.find(|record| {
    record.is_open() && record.summary == item.summary
})
```

(dropping the per-run conjunct — the exact one-clause regression that
would merge two runs' facts into one ticket). Executed battery under the
mutant, ALL GREEN:

```
cargo nextest run -p gauntlet-adapter-escalation-fs                          → 15 passed
cargo nextest run -p gauntlet-cli --test e2e_budget --test escalations_cli   → 5 passed
```

The brief's invariant — "dedup across runs (must NEVER — try the same
summary in two run ids)" — has NO executed teeth anywhere in the
delivered battery: no suite ever escalates the same summary under two
run ids (the contract suite's dedup test `the_same_open_fact_
reaffirms_one_ticket_and_a_resolved_one_never_dedups` uses one
`fixture_run()`; e2e_budget's composed re-affirm leg is single-run too —
a resume continues the SAME run id, so the composed surface cannot
distinguish this mutant either). Under the mutant, run B's genuinely new
hard residual is silently re-affirmed onto run A's open ticket and never
reaches the operator's queue as its own fact (G8-adjacent: a dropped
hard residual). Severity MAJOR: the mutant is a one-clause deletion on
the newest code, and nothing in the delivered tree would catch it.

Positive probe (executed, under CORRECT code, via a temporary probe
`crates/adapters/gauntlet-adapter-escalation-fs/tests/
zz_fals2_tmp_probe.rs` — ADDED then DELETED, tree back to
delivered state): the same summary under run `…G5FAV` and run `…G5FAW`
mints TWO tickets, listing shows 2 — the invariant HOLDS in the
delivered code. Re-applying the mutant with the probe present: the probe
goes RED (`the_same_summary_under_two_different_runs_never_dedups ...
FAILED`) while all 15 delivered tests stay green — the mutant is
demonstrably live and demonstrably unwatched.

Restore: queue.rs reversed-edit, sha256 both sides:

```
mutant:   (predicate without the run_id conjunct, as quoted above)
restored: 493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181
baseline: 493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181  (identical)
```

### A4-M2 — resolve-then-reobserve dedup mutant: KILLED (teeth exist)

Mutation: the same predicate with `record.is_open() &&` dropped
(a re-observation AFTER the operator resolved would silently re-affirm
the RESOLVED ticket instead of minting anew — the run would poll
`Resolved` and proceed on the earlier action). Executed:

```
cargo nextest run -p gauntlet-adapter-escalation-fs     → 1 FAILED:
  the_same_open_fact_reaffirms_one_ticket_and_a_resolved_one_never_dedups
  (assert_ne!(first.id(), again.id()) — "a resolved ticket never dedups")
cargo nextest run -p gauntlet-cli --test e2e_budget     → 2 passed
```

KILLED at the queue contract level. Noted for completeness: the composed
e2e_budget leg alone does NOT discriminate this mutant (it stayed green
under it) — the queue-level teeth are the only guardian of
resolve-then-reobserve. Restore sha256: `493a1606…` = baseline
(identical, reversed edit).

### A4-P1 — false-dedup collision shape: **EXECUTED COLLISION = FINDING F2-2 (MINOR+, mechanism live; producer reachability is the caveat)**

Executed probe (temporary test, added then deleted): ONE run, TWO
genuinely different facts — different evidence context refs
(`evd-unit-A-lattice-refusal` vs `evd-unit-B-lattice-refusal`) — whose
summaries are byte-identical. Observed verbatim:

```
P1 RESULT: first=TicketId { ticket: "esc-6a9d0b0a-0" } second=TicketId { ticket: "esc-6a9d0b0a-0" } records=1 second_context_dropped=true
P1 CONFIRMED COLLISION: fact B re-affirmed fact A's ticket; fact B's context ref is recorded nowhere in the queue (dropped residual)
```

This is the designed semantics ("byte-identical summaries are the same
fact"), so it is only a defect if a shipped producer can emit
byte-identical summaries for different units. One can:
`crates/gauntlet-app/src/govern/gate.rs` `GateRefusal::LatticeMovementRefused`
—— the ONLY `GateRefusal::summary()` variant that does NOT embed the
unit — renders `"lattice movement refused: {rule}"` (gate.rs:473-478).
Two units refused by the same lattice rule in one run would mint
byte-identical ticket summaries via
`submit_ticket(..., format!("dispatch refused by the governance gate: {}", refusal.summary()))`
(controller.rs:2407-2414) and the second fact would be suppressed
exactly as P1 shows. Every OTHER producer (scheduler `Ineligible`
Display, all other `GateRefusal` variants) embeds the unit id and cannot
collide. Caveat recorded honestly: LatticeMovementRefused requires the
dispatch facts to rank below the recorded ruler facts — a defense-in-depth
arm whose composed-loop reachability I could not execute within this
round's budget (the frozen ruler parse feeds both fact sources, so they
normally agree). The collision MECHANISM is executed live; the producer
surface is identified statically.

### A4-P2 — concurrent escalate race: **EXECUTED RACE = FINDING F2-3 (MINOR)**

8 threads escalating the SAME (run, summary) fact through one
`FsEscalationQueue` — observed verbatim:

```
P2 RESULT: 8 concurrent escalators of one fact -> distinct tickets returned=2, files on disk=2
```

The dedup is a read-then-write scan with no serialization between the
scan and the mint, so concurrent submits mint duplicate tickets for one
fact. Direction of failure is NOISE, not suppression (the operator sees
two tickets for one fact; no residual is lost) — hence MINOR. Composed
reachability requires parallel dispatch with max_parallel > 1 in the
same iteration window.

### A4-P3 — quarantine vs the dedup scan: defense CORRECT (no finding)

A corrupt ticket file planted beside a valid open ticket: the dedup scan
neither blocks on it nor mistakes it for a dedup partner; the valid fact
re-affirms its own ticket; the corrupt file quarantines (1 quarantined,
1 record). The corrupt-open-ticket direction fails closed.

Checkpoint: A4 complete. Queue restored byte-identically
(`493a1606…`, verified after each mutation), temporary probe file
deleted. Tree state: only the delivered modifications remain.

## Attack group A1 — the F-R1 no-seam refusal teeth (`runner.rs` + `cor2_seam_and_recovery_attacks.rs`)

Target: the `(None, Some(policy_ref))` arm at
`crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs:552-560` and
its teeth test `fr1_a_policy_carrying_dispatch_without_the_seam_is_a_
typed_refusal_that_spawns_nothing`. Three mutants, each EXECUTED against
the teeth target; each restored byte-identically before the next.

### A1-M1 — the disabling mutant (arm runs the dispatch unwrapped): KILLED

Mutation: the arm replaced by `(None, Some(_)) => argv,` (the exact
round-1 disabling mutant). Observed RED verbatim:

```
FAIL gauntlet-adapter-agent-cli::cor2_seam_and_recovery_attacks
     fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing
panicked at crates/adapters/gauntlet-adapter-agent-cli/tests/cor2_seam_and_recovery_attacks.rs:153:10
Summary: 2 tests run: 1 passed, 1 failed
```

(the dispatch returned `Ok`, so `.expect_err(...)` panicked — the typed
refusal never fired). fr3 stayed green. Killed.

### A1-M2 — refuse-with-spawn (refusal returned AFTER the fs effects and the spawn): KILLED

Mutation: the arm fell through to `argv`, the full admission ran
(invocation dir, envelope, prompt, and the CHILD SPAWN — verbatim argv),
and the byte-identical typed refusal was returned only after the spawn.
The error class and detail were IDENTICAL to the honest refusal — only
the order was wrong. Observed RED verbatim:

```
panicked at crates/adapters/gauntlet-adapter-agent-cli/tests/cor2_seam_and_recovery_attacks.rs:168:5
"a refused policy-carrying dispatch spawns nothing, wrapped or otherwise"
Summary: 2 tests run: 1 passed, 1 failed
```

Killed by the SPAWN-COUNTER assertion — the load-bearing discriminator
the teeth test was built around. A weaker test asserting only the error
would have survived this mutant.

### A1-M3 — refuse-with-workspace-effect (refusal AFTER the invocation artifacts, BEFORE the spawn): KILLED

Mutation: same fall-through, the refusal returned after the invocation
directory/envelope/prompt writes but before `transport.spawn`. Observed
RED verbatim:

```
panicked at crates/adapters/gauntlet-adapter-agent-cli/tests/cor2_seam_and_recovery_attacks.rs:178:5
"a refused dispatch anchors no invocation directory"
Summary: 2 tests run: 1 passed, 1 failed
```

Killed by the artifact-level assertion (spawn counter empty, but the
invocation tree was anchored). Each of the three teeth assertions is
individually load-bearing: A1-M1 dies on the typed refusal, A1-M2 on
the spawn counter, A1-M3 on the artifact check.

### A1-M4 — the teeth-deleted coverage probe: EXECUTED — the teeth test is the SOLE guardian

Setup: the A1-M1 disabling mutant RE-APPLIED and the fr1 teeth test
`#[ignore]`d (the "if the teeth test is deleted" question). Executed
battery under that state — ALL GREEN:

```
cargo nextest run -p gauntlet-adapter-agent-cli                                        → 61 passed, 1 skipped, 0 failed
cargo nextest run -p gauntlet-cli --test compose_teeth --test p8_adapter_substitution \
    --test e2e_full_loop_real_runner                                                   → 7 passed
```

Answer to the brief's question: NO other suite keeps the arm honest.
The composed run always installs the containment seam
(`with_containment`), so no composition-level test ever drives a
policy-carrying dispatch into a seam-less runner — the arm is reachable
only through the exact shape the fr1 teeth test binds. Recorded as an
OBSERVATION (not a defect): the AK-047 no-seam invariant hangs on that
one test's existence; its deletion would be silent.

Restores (sha256 both sides, after every mutant):

```
runner.rs  mutant-states as quoted above
           restored: b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0  = baseline (identical)
cor2_seam_and_recovery_attacks.rs
           restored: f30fcc847db4c63b5c6595c4376bdfd95189dde0bc2033545b965480af9e2d66  = baseline (identical)
```

## Attack group A3 — group-wide child retirement (F-R3, `transport.rs`)

Target: `crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs` —
`process_group(0)` at spawn and the two group-addressed stop legs.
Probes were executed through the REAL `ProcessChildTransport` via a
temporary probe target
(`crates/adapters/gauntlet-adapter-agent-cli/tests/zz_fals2_transport_
probe.rs` — ADDED then DELETED; the delivered tests were never edited in
this group).

### A3-L1 — the spawned child IS its own process-group leader: CONFIRMED (no finding)

Observed: child pgid == child pid, and != the test process's pgid. The
group stop's blast radius cannot reach the run itself.

### A3-L2 — a hostile child that group-kills on TERM cannot reach the run: CONFIRMED (no finding)

A child trapping TERM and answering with `kill -KILL -- -$pgid` retires
only its own tree; the run's group is distinct and the stop stays
bounded. (This is the brief's "child that inspects/kills group members"
probe, executed.)

### A3-L3 — the RECORDED boundary is TRUE (no finding; doc honesty verified)

A TERM-ignoring grandchild (`trap '' TERM`) outlives its driver's own
TERM death: the stop returns at the driver's death WITHOUT escalating,
the grandchild survives — exactly what the module docs claim is beyond
the bounded claim ("the driver's own death then ends the observable
handle"). A later group KILL empties it. The recorded boundary and the
behavior agree. Note: the survivor is LIVE (a leak within the recorded
boundary), and it inherits every open fd of its spawner — see F2-6.

### A3-L4 — TERM-ignoring driver + TERM-ignoring grandchild (WITHIN the documented claim): CONFIRMED under correct code

`stop` escalates to the group SIGKILL and the WHOLE tree dies — no LIVE
member of the group remains (4/4 deterministic passes, ~3.8s each).
Method note (recorded because it changed the probe's authority): a
`kill -0 -- -PGID` liveness check is NOT authoritative here — orphaned,
unreaped CORPSES (zombies) still count as group members for kill(2),
and reparented-zombie reaping latency on this host is seconds. The
probe therefore reads liveness from `/proc/<pid>/stat` (state != Z, pgid
match). The first L4 revision (kill -0 based) produced 3 deterministic
FALSE alarms; the /proc-based revision is the one these conclusions
rest on.

### A3-M1 — `process_group(0)` removed: KILLED

fr3 went RED with the exact leak diagnostic:

```
the post-spawn refusal left the wrapped tree alive: driver pid 1161744 alive=false,
wrapped payload pid Some(1161754) alive=true, process group still has members=false
— the cancel-for-recovery arm leaked the child
Summary: 22 tests run: 21 passed, 1 failed
```

The process-group line is load-bearing and pinned by the delivered
teeth.

### A3-M2 — TERM leg addressed to the DIRECT pid instead of the group: KILLED

Mutation: the shared `pid` binding changed from `-<pid>` to `<pid>`
(TERM leg — and the same binding serves the KILL leg, so this run was
the compound direct-only stop). fr3 RED (the wrapped payload survived;
the delivered teeth catch the TERM leg's group addressing).

### A3-M3 — KILL leg addressed to the DIRECT pid only: **SURVIVING MUTANT = FINDING F2-4 (MAJOR)**

Mutation (isolated to the escalation leg): the KILL leg's argv target
changed from the group pid (`-<pid>`) to the direct child pid. Under
this mutant the ENTIRE DELIVERED BATTERY STAYS GREEN:

```
cargo nextest run -p gauntlet-adapter-agent-cli   → all delivered tests PASS
```

Only my temporary probe discriminates it — RED with LIVE survivors:

```
the escalation leg's group SIGKILL must retire the WHOLE tree — live members survived: [1075063, 1075367]
```

Analysis: the correction's headline claim — "BOTH stop legs address the
WHOLE GROUP" — is only HALF-pinned by the delivered battery. fr3 pins
the TERM leg's group addressing (A3-M2), but no delivered test drives
the KILL leg into a group that TERM did not already retire: the
delivered TERM-ignoring-child test uses a DIRECT child (retired by the
`inner.kill()` backstop) and hygiene-kills its own `sleep` grandchild
itself. A one-line regression that silently reverts the KILL leg to
round-1 direct-pid semantics — recreating the exact leak shape F-R3
corrected, for every TERM-ignoring wrapped tree — would land green.
The SHIPPED code is correct (A3-L4 proves the real behavior); the gap
is executed evidence for the claim. Same class as F2-1.

### A3-F2-5 — (observation, MINOR) zombie-corpse reaping can mask a stop's completeness signal

The delivered fr3 teeth and the escalation-queue reading share a latent
measurement hazard this round hit empirically: `kill -0 -- -PGID`
reports SUCCESS while an unreaped orphan corpse (zombie) remains a group
member, so "group empty" can be false-negative for up to the reaper's
latency (observed > 5 s on this host). No delivered assertion failed on
it in the clean runs (fr3's payload zombie was reaped within its 10 s
poll), but the delivered group-liveness idiom is reaper-latency-sensitive.
Recorded as a robustness observation, not a defect executed against a
specific claim.

### A3-F2-6 — (EXECUTED, ops hazard, MINOR-to-MAJOR): a leaked spawned child INHERITS AND HOLDS the repository compile lock

During the M-A3c mutant run, the surviving TERM-ignoring grandchildren
(from the L4 probe failures under the mutant) were found holding fd 3
OPEN on `.agents/reports/task25/compile.lock` — inherited across the
spawn chain (flock → nextest → test binary → transport spawn, no
close-on-exec on that fd path). With two such orphans alive, the NEXT
flock'd invocation blocked in `locks_lock_inode_wait` for 28+ minutes —
the EXACT task23 wedge class, reproduced live and diagnosed precisely:

```
HOLDER pid=1074715 cmd=sh /tmp/gauntlet-fals2-transport-l4-1074712-0/inner.sh  fd=/proc/1074715/fd/3
HOLDER pid=1075063 cmd=sh /tmp/gauntlet-fals2-transport-l4-1075060-0/inner.sh  fd=/proc/1075063/fd/3
HOLDER pid=1077395 cmd=flock .agents/reports/task25/compile.lock -c '...'   (waiting, locks_lock_inode_wait)
```

Killing the orphans released the lock instantly (verified). The shipped
code's defense against this is precisely the group retirement (F-R3) —
which is why the surviving KILL-leg mutation (F2-4) is dangerous beyond
its own leg: it recreates the leak shape that wedges the whole
repository's compile loop. Under CORRECT code, the same inheritance
applies to the A3-L3 recorded-boundary survivor (a grandchild that
outlives its driver's TERM death keeps the run's open fds — including
any lock files — alive). The transport's own docs scope credential
pruning to the gate/sandbox composition; fd discipline (CLOEXEC on
non-contract fds in the spawn path) is nobody's recorded invariant.
Graded MINOR-to-MAJOR: no invariant is crossed under correct code, but
the hazard is one leak away from a self-deadlocked repository.

Clean-code regression after all restores (probes + delivered suites
together): `42 tests run: 42 passed, 0 skipped`. Probe file DELETED;
`transport.rs` sha256 restored byte-identically
(`9f7197e42f0edd9df6df0fb270fe6f51093c7b75367f7b9e5e0bb7d9b55de8e7` =
baseline); leaked probe children swept to zero (`ps` count 0).

## Attack group A2 — run-lifecycle facts (F-R4/F-R5: ports, sqlite, controller, report)

Method: a persistent scratch world (`/tmp/fals2/world1`) driven by the
REAL `target/debug/gauntlet` binary on a copy of `tests/fixtures/
target-real`; run id `F1B8H9YCD9JGFC0AJJ9DYV8ZK5`, honest terminal
`budget_exhausted`, ticket `esc-6a9d233f-0` for AK-953.

### A2-P1 — export determinism (AK-054) with the new events present: CONFIRMED (no finding)

```
gauntlet export --run F1B8H9YCD9JGFC0AJJ9DYV8ZK5  (twice, separate files)
export1.jsonl sha256 5ee52298c05fb1e748970a10008982cae77c2f648c9fb4670272bb0d3e1f7b57
export2.jsonl sha256 5ee52298c05fb1e748970a10008982cae77c2f648c9fb4670272bb0d3e1f7b57
cmp → BYTE-IDENTICAL
```

The export carries the new events (1 `run_terminal`, 3 `unit_dispatch`)
and is a pure function of the records — AK-054 holds real-world with
the F-R4 machinery in place.

### A2-P2 — corrupt lifecycle row: fail-closed CONFIRMED (no finding)

`UPDATE event_log SET payload='{broken json'` on the first
`unit_dispatch` row, then the real binary:

```
gauntlet export …  → exit 1:
  "stored state is corrupt: the event log is damaged: offset 3: payload is not
   valid JSON: json: unexpected byte 0x62 at offset 1"
gauntlet resume …  → the pass re-terminalized honestly (iterations=4), then:
  exit 1, "gauntlet: the run report could not be built from the records:
  the report's store read failed: stored state is corrupt: … offset 3 …"
```

A damaged lifecycle record never becomes a basis: the read is typed
`Corrupt` through the strict codec, the export refuses, and the report
REFUSES TO RENDER rather than classify from a damaged log. (The strict
decode scan is over the whole log, so even another run's damaged row
blinds this run's export — over-broad but in the fail-closed direction;
recorded as the observed behavior.)

### A2-P3 — kill windows AROUND the new event appends: exactly-once CONFIRMED, 3/3 legs (no finding)

Three fresh worlds; the real binary SIGKILLed at the OBSERVED durable
fact "first `unit_dispatch` row present" (bounded 5 ms sqlite poll);
then the DESIGNED recovery and resume:

```
LEG world2a: kill landed at unit_dispatch=1 invocation_terminal=0 settlement=0
LEG world2b: kill landed at unit_dispatch=1 invocation_terminal=0 settlement=0
LEG world2c: kill landed at unit_dispatch=1 invocation_terminal=0 settlement=0
```

All three kills landed INSIDE the F-R4 window (the unit-attributed
dispatch fact durable, no terminal, no settlement — the exact shape the
brief warned must not become a second settlement/recovery hole).
Observed after resume in every leg:

- the SIGKILLed run left a stale `writer.lock`; resume REFUSED typed
  ("stale or unreadable writer lock … never guesses liveness"); after
  confirming the holder pid dead and removing the lock, resume
  completed (the designed operator recovery works through the real
  binary);
- exactly-once: `settlements[AK-951]=1 …[AK-954]=1`, zero units with
  duplicate settlement records — the dead pass's durable dispatch fact
  did NOT produce a second settlement;
- honest terminal `budget_exhausted` with report head
  `terminal_state":"budget_exhausted"`, AK-952 `provisional`, AK-953
  `configured-but-untested`;
- the append-only record shows the dead pass's dispatch beside the
  resumed pass's (4 `unit_dispatch` events; the transition is visible,
  never a rewrite).

### A2-M1 — attempt-summary blinded (`attempt_summary` filter → false): KILLED

```
gauntlet-app --test report:  an_attempted_unit_is_provisional_with_the_record_derived_attempt_basis FAILED
e2e_budget:                  the_non_converging_silent_unit_terminates_cleanly… FAILED
```

The "attempted unit rendered never-attempted" lie is caught at BOTH the
unit-report level and the composed level. Restored:
`0ea3ae3d973138f54db48642c5db98b4d3d83c4206d8d78d3b1c6bba18cebc9b` =
baseline.

### A2-M2 — stalest-terminal mutant (`run_terminals.last()` → `.first()`): KILLED (composed level only)

```
gauntlet-app --test report: 10 passed  (the latest-of-many rule has NO unit-level teeth:
                            every unit-level fixture carries a SINGLE terminal)
e2e_budget:                 the_escalated_resume_re_affirms… FAILED
```

Only the composed resume leg (first pass `escalated`, resumed pass
`settled`) discriminates the mutant — the report rendered the stale
terminal and the leg's record-derived assertion fired. A resumed run's
lifecycle facts being stale IS caught, but solely by the e2e; the same
single-guardian pattern as F2-1/F2-4. Restored: sha = baseline.

### A2-M3 — terminal never sealed (`seal_terminal` → `Ok(())`): KILLED (composed level only)

```
gauntlet-app (whole crate): 566 passed  (NO unit-level test pins the durable RunTerminal append)
e2e_budget:                 BOTH legs FAILED ("the report carries the run's real terminal,
                             not a null")
```

A terminal that ends without a sealed `RunTerminal` event anywhere
leaves the report's `terminal_state` NULL — and the composed legs catch
it. Two structural facts recorded: (1) NO caller of the domain
aggregate's `terminate()` exists anywhere, so the report's
`or_else(aggregate)` fallback is dead code in every shipped composition
— the sealed event is the ONLY terminal source (the fallback would
matter only for a future store that replays terminals into the
aggregate); (2) the entire 566-test gauntlet-app unit battery is blind
to terminal sealing — the e2e is the sole guardian. Relatedly,
`improve propose --from-run` projects `terminal={:?}` from
`load_run(…).terminal_state()` — which is ALWAYS `None` for every run
in every shipped store — so the optimizer's run-history projection
renders `terminal` empty while the record holds a real terminal. A
stale operator-adjacent display (recorded as an OBSERVATION, adjacent
scope, not fixed).

Restores: `controller.rs fddf3f5e6b1d809965adfc99bd8ddfc71d56158223d25d26c0bb772a07e07e91`
and `report/mod.rs 0ea3ae3d973138f54db48642c5db98b4d3d83c4206d8d78d3b1c6bba18cebc9b`
— both = baselines (identical).

## Attack group A7 — the `improve` composition's fail-closed posture (r1 not-attacked, EXECUTED)

Executed through the REAL binary in a fresh cli-agent+bwrap scratch
world (`/tmp/fals2/world3`, optimizer role copied into the SCRATCH
world's own `.agents/roles/` — nothing written under the repo):

```
gauntlet improve propose   (exit 1)
  "the optimizer invocation was not admitted: admission refused: the result-contract
   token "results/QBH22Y6P80QQ8HS5PFPP023XY0/candidate_revision.json" must be a single
   leaf component; the runtime anchors it under runs/<run_id>/inv/<invocation_id>/"
post-state: state/runs ABSENT, zero workspace allocations, zero evidence objects,
zero spawn effects
```

Findings, stated precisely:

1. The dispatch is IMPOSSIBLE there and FAILS CLOSED — executed, not
   static: the only runner the improve composition builds is
   `runner_for(&token, context, &clock, None)` (the seam-less shape),
   the plan carries `sandbox_policy: None` by construction, and the
   one executed attempt to dispatch was refused TYPED at admission
   before any filesystem effect or spawn. A policy-carrying dispatch
   into that shape is the exact fr1 teeth case (group A1) — three
   mutants executed there prove the refusal has teeth.
2. OBSERVATION (adjacent scope, task-23 surface, MINOR, fail-closed
   direction): the `improve propose` MODEL-dispatch leg is currently
   DOA in the delivered tree — the ProposalService's result-contract
   token (`results/<revision>/candidate_revision.json`) is refused by
   the contract-path grammar on EVERY attempt, so the optimizer can
   never actually be invoked through the composed path (only
   `--from-result` works). The failure is honest and contained, but the
   wired capability is not reachable end-to-end. Recorded for the
   operator; not fixed (adjacent scope).

## Attack group A8 — the DESIGNED operator flow end to end (r1 not-attacked, EXECUTED TWICE)

Cycle, executed twice (world1 run `F1B8H9YCD9JGFC0AJJ9DYV8ZK5`; world4
run `VHEMRP47VQGBYAZ47BNVE5FKD0`), through the real binary:

1. `gauntlet escalations list` → two open tickets (AK-953 irreversible,
   AK-952 lead_agent closure), correct run attribution.
2. `gauntlet escalations resolve --ticket … --unit …` → **REFUSED
   typed** on BOTH tickets in BOTH cycles:
   `code=verification_not_pass — no PASS verification record exists for
   unit …; settlement requires one (spec 16.1 condition 1)`. The
   tickets stay open; exit non-zero.
3. `gauntlet escalations retry --ticket …` (world1) → refused by the
   SAME gate, same code.
4. `gauntlet resume <run-id>` → the run CONTINUES: re-plans, re-spends a
   booked round honestly, re-affirms the SAME open tickets (F-R5 dedup:
   still exactly 2 open), re-terminalizes `budget_exhausted` with an
   honest record-derived report (`AK-951/AK-954 implemented-and-proven,
   AK-952 provisional, AK-953 configured-but-untested`), exactly-once
   settlements preserved (`AK-951=1, AK-954=1`, zero duplicates).

Judgment (the brief: the flow MUST work — breaking it is a finding):

- The RESUME half of the flow works and is honest in every executed
  respect (liveness, identity, exactly-once, append-only transition,
  record-derived report, honest non-success exit).
- **FINDING F2-7 (MAJOR, design-flow gap — executed):** the operator
  RESOLVE half cannot ever complete for the tickets this fixture world
  actually mints. Both minted ticket classes (§11.5-row-6 lead_agent
  closure; irreversible never-dispatched) carry NO PASS verification
  record — AK-952's pipeline never mints one (its proof would only
  materialize at a settlement, and settlement is exactly what is routed
  to the operator), and AK-953 may never dispatch at all. The operator
  surface's only two actions BOTH demand that record as a precondition,
  so every resolve/retry is refused, the tickets are unresolvable
  through the designed surface, and the run can never reach `settled`
  in this world. Every refusal is typed and honest (the operator
  bypass is correctly refused — AK-062's class), but the designed flow
  as a flow dead-ends: there is no reachable operator path that closes
  an operator-routed ticket. Whether the frozen design intends closure
  of such tickets to live OUTSIDE the run is a ruler-owner question;
  within every surface this repo ships, the deadlock is real and was
  executed twice with identical results.

## Attack group A5 — child retirement on the remaining disposition paths (r1 not-attacked, EXECUTED with /proc sweeps)

Real-binary worlds, each followed by a /proc sweep for surviving
`child.sh`/`bwrap` processes of the world (none found on any path):

- **Reference-mutated abort** (world5b): mid-run mutation of a claim
  word inside a frozen AK-951 body line (parse stays valid, digest
  changes — the first attempt appended a stray heading and was rightly
  refused at RESOLUTION time, before freeze: the strict parser is a
  second confirmed defense). Observed:
  `run: terminal=aborted`, `reference_breach=Mutated { expected: …,
  actual: … }` — the §9.4 abort fired at the boundary; sweep CLEAN
  (no child outlived the abort).
- **Governance refusal mid-flight** (world5c): `cross_cutting.json`'s
  backend switched to the no-backend token AFTER the first dispatch was
  durable; the next dispatch-time validation refused; observed
  `run: terminal=blocked`; sweep CLEAN.
- **Stall/non-convergence escalation** (world5d, `silent` child, 4 s
  stall): `run: terminal=escalated`; sweep CLEAN.
- **Diagnostic exhaustion (`inconclusive`, §15.2/row 9): configured-but
  -untested at the real-binary level within this round's budget** —
  reaching it composed requires the non-convergence detector's
  threshold + diagnosis-engine phases; the disposition IS executed
  composed at the controller level by the delivered in-crate test
  (`a_surviving_hypothesis_ends_inconclusive_with_quarantine_composed`),
  but no real-binary leg drove it this round, and the retirement sweep
  on that path was therefore not executed against the real binary.
  Recorded honestly rather than claimed.

## Attack group A6 — cancel-during-retry window race (r1 not-attacked, EXECUTED)

world6, silent child, retry backoff widened to 3000 ms; the run
SIGKILLed at the OBSERVED backoff boundary (1 `invocation_terminal`
durable, 1 `unit_dispatch`, retry NOT yet dispatched — 20 ms poll):

```
A6: killed in backoff window (terminal=1 dispatch=1)
designed recovery: stale lock removed
resume → terminal=escalated iterations=4
exactly-once: OK (0 settlements — correct for a non-converging run)
invocation_terminal events: 3 (attempt accounting carried honestly across the kill)
/proc sweep: CLEAN
```

The retry window race is safe: the killed pass's attempt facts stayed
durable, the resumed pass continued the attempt sequence (never
re-ran the same attempt twice), and the terminal is honest. No
double-dispatch, no settlement hole.

## Probe hygiene and tree integrity (closing proof)

- Repo `.gauntlet/policies/`: exactly `contained.json`,
  `cross_cutting.json`, `local.json` — nothing ever planted (the
  policy-breaking legs mutated SCRATCH copies only).
- `tests/fixtures/`: git shows only the delivered untracked
  `?? tests/fixtures/target-real/` (the delivered fixture, uncommitted
  by design); zero modifications to any fixture file — every leg ran in
  `/tmp/fals2/world*` copies.
- Temporary probe targets
  (`zz_fals2_tmp_probe.rs`, `zz_fals2_transport_probe.rs`): DELETED;
  neither tests dir contains them.
- All 11 touched files sha256-verified BYTE-IDENTICAL to the baseline
  list at the top of this report
  (`diff baseline-sha256.txt end-sha256.txt` → empty).
- Leftover probe processes: 0 (swept; the F2-6 orphans were killed and
  their lock released).
- All scratch under `/tmp/fals2/`. Nothing staged, nothing committed.
- `just ruler` PASS at start AND at end (`RULER_EXIT=0`).

## Executed-attacks table

| # | Attack | Target | Executed evidence | Outcome |
|---|--------|--------|-------------------|---------|
| A4-M1 | cross-run dedup predicate dropped (mutant) | `queue.rs` escalate | full battery green under mutant; probe RED; restore sha equal | **F2-1 MAJOR — surviving mutant** |
| A4-M2 | `is_open()` dropped (mutant) | `queue.rs` escalate | contract-suite dedup test RED | killed |
| A4-P1 | two facts, byte-identical summaries, one run | queue + `gate.rs` producer scan | `P1 CONFIRMED COLLISION … context ref is recorded nowhere` | **F2-2 MINOR+ (mechanism live; producer reachability caveat)** |
| A4-P2 | 8 concurrent escalators, one fact | `FsEscalationQueue` | `distinct tickets returned=2, files on disk=2` | **F2-3 MINOR** |
| A4-P3 | corrupt file vs dedup scan | queue | quarantine correct, no false dedup | defense confirmed |
| A1-M1 | no-seam arm disabled (mutant) | `runner.rs` | fr1 RED at `expect_err` (line 153) | killed |
| A1-M2 | refuse-with-spawn (mutant) | `runner.rs` | fr1 RED at spawn counter (line 168) | killed |
| A1-M3 | refuse-with-workspace-effect (mutant) | `runner.rs` | fr1 RED at artifact check (line 178) | killed |
| A1-M4 | arm disabled + teeth test ignored | whole battery | 61+1 skipped green; 7 composition tests green | observation: teeth test is the SOLE guardian |
| A3-L1 | child is own group leader | transport (live) | pgid == pid != run's pgid | defense confirmed |
| A3-L2 | hostile child group-kills on TERM | transport (live) | run unreachable; tree retired | defense confirmed |
| A3-L3 | TERM-ignoring grandchild vs driver TERM death | transport (live) | grandchild survives, stop returns — doc boundary TRUE | recorded boundary confirmed |
| A3-L4 | TERM-ignoring driver+grandchild, escalation leg | transport (live, 4×) | no LIVE member survives; ~3.8 s | defense confirmed |
| A3-M1 | `process_group(0)` removed (mutant) | `transport.rs` | fr3 RED with leak diagnostic | killed |
| A3-M2 | TERM leg direct-pid (mutant) | `transport.rs` | fr3 RED | killed |
| A3-M3 | KILL leg direct-pid only (mutant) | `transport.rs` | delivered battery GREEN; probe RED with live pids | **F2-4 MAJOR — surviving mutant** |
| A3-F2-5 | zombie-corpse group-membership | measurement idiom | `kill -0` false-membership observed | observation (MINOR) |
| A3-F2-6 | leaked child holds compile.lock fd | spawn-chain fds | 2 orphans held the lock 28 min; `locks_lock_inode_wait`; kill → released | **F2-6 MINOR(+), executed ops hazard** |
| A2-P1 | export determinism, new events present | real binary | two exports sha-equal (`5ee52298…`) | AK-054 confirmed |
| A2-P2 | corrupt `unit_dispatch` payload | sqlite + real binary | export exit 1 typed Corrupt; report refuses to render | fail-closed confirmed |
| A2-P3 | SIGKILL in the `unit_dispatch` window, ×3 | real binary + sqlite poll | exactly-once 3/3; designed stale-lock recovery works; append-only transition visible | defense confirmed |
| A2-M1 | attempt-summary blinded (mutant) | `report/mod.rs` | report test RED + e2e_budget RED | killed |
| A2-M2 | stalest terminal (mutant) | `report/mod.rs` | unit tests blind; e2e_budget resume leg RED | killed (sole guardian: e2e) |
| A2-M3 | terminal never sealed (mutant) | `controller.rs` | 566 unit tests blind; BOTH e2e_budget legs RED | killed (sole guardian: e2e) |
| A5 | retirement sweeps: reference-mutated abort / governance mid-flight / stall escalation | real binary worlds | `aborted` w/ breach digest; `blocked`; `escalated`; sweeps CLEAN | confirmed |
| A6 | cancel-during-retry window SIGKILL | real binary world | killed at terminal=1/dispatch=1; resume honest; exactly-once; sweep CLEAN | confirmed |
| A7 | improve-composition fail-closed | real binary | dispatch refused typed, zero spawn; `sandbox_policy: None` by construction; fr1 teeth bind the shape | confirmed + DOA-propose observation |
| A8 | designed operator flow, ×2 | real binary | resolve REFUSED both ticket classes twice; resume honest, exactly-once, tickets re-affirmed | **F2-7 MAJOR — resolve leg dead-ends** |

## Findings, counts per severity

**VERDICT: CONFIRM**

- **MAJOR: 3**
  - F2-1 — surviving mutant: the escalation dedup's per-run conjunct
    can be deleted with the ENTIRE delivered battery staying green;
    dedup across runs (an invariant the brief names MUST NEVER) has no
    executed teeth. Correct code verified correct; the evidence is
    missing.
  - F2-4 — surviving mutant: the stop's KILL leg can silently revert to
    direct-pid addressing (the round-1 leak shape, for TERM-ignoring
    trees) with the ENTIRE delivered battery staying green; the
    correction's "BOTH stop legs address the WHOLE GROUP" claim is only
    half-pinned (the TERM leg is pinned by fr3).
  - F2-7 — the designed operator-resolve flow dead-ends: both real
    ticket classes the composed run mints (§11.5-row-6 lead_agent
    closure; irreversible never-dispatched) are unresolvable through
    the operator surface (resolve AND retry both demand a PASS
    verification record that no reachable path produces for
    operator-routed units), executed twice. The resume half is honest;
    the run can never reach `settled` in this world.
- **MINOR: 3** — F2-2 (false-dedup collision mechanism executed live;
  shipped unit-free summary producer identified —
  `LatticeMovementRefused` — with composed reachability unproven);
  F2-3 (concurrent-escalate dedup race — duplicate tickets for one fact
  observed live); F2-6 (leaked spawned children inherit and hold the
  compile-lock fd — a 28-minute self-deadlock reproduced and released;
  MINOR-to-MAJOR because F2-4 is what recreates the leak that wedges
  it).
- **Observations (no severity):** F2-5 (the `kill -0 -- -PGID` group
  liveness idiom counts unreaped corpses — reaper-latency-sensitive);
  A1-M4 (the fr1 teeth test is the SOLE guardian of the no-seam
  invariant — its deletion would be silent); A2 notes (the report's
  aggregate-terminal fallback is dead code — no `terminate()` caller
  exists anywhere; the 566-test gauntlet-app unit battery is blind to
  terminal sealing; `improve propose --from-run` always projects
  `terminal` empty); A7-2 (the `improve propose` model-dispatch leg is
  DOA — the contract-path grammar refuses every attempt; fail-closed
  but not reachable).

## Not-attacked (honest residue)

1. The `inconclusive` diagnostic-exhaustion disposition was not driven
   through the real binary (its retirement sweep therefore not executed
   there); it is executed only by the delivered controller-level test.
2. The composed-loop reachability of `LatticeMovementRefused` (the
   F2-2 producer) — the two fact sources normally agree, so no executed
   leg fires it.
3. setsid-escaping grandchildren (the transport's other recorded
   boundary) — not re-probed this round; the boundary text was taken as
   given after A3-L3 verified the adjacent TERM-death boundary live.
4. Budget-reserve-breach dispositions' retirement sweeps (only
   aborted/blocked/escalated were swept).
5. Corruption in the OTHER record feeds of the report
   (`verification_records`, `settlements` rows) — only the event-log
   corruption was executed; those reads share the strict codec but were
   not individually fired.
6. Export determinism under a CONCURRENTLY appending store (two
   sequential exports were proven byte-identical; concurrent
   export-vs-write races were not attempted).

`just ruler`: PASS at start and at end. Nothing staged or committed.
The tree is exactly as delivered (11/11 touched files byte-identical,
temp probes deleted, scratch confined to /tmp/fals2).

---

(Checkpointed after each executed attack per the outage discipline; final
VERDICT block at the end.)
