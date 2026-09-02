# Task 20 — falsifier round 4 (blind executing falsifier, COR-R3 mechanisms)

Round 4 target: the COR-R3-1/2/3 mechanisms as landed, plus the mandatory
over-rejection leg. Blind: no builder/critic reports read (adjudications
r1–r3 and charter excepted). All cargo/just under
`flock .agents/reports/task20/compile.lock` with `sccache --stop-server`
in the same command. Probe artifacts live only under a /tmp tempdir.

Status: IN PROGRESS — findings appended below as they land.

## Findings (most severe first)

### F-R4-3 (MAJOR) — COR-R3-1(d) violated on its own headline scenario: a
recovery-COMPLETED production on an otherwise-settled run burns the resume to
a dishonest `budget_exhausted` terminal (reachability-caveated)

Reproduction (pristine scratch world `/tmp/falsifier-r4/iso-world`, real
binary, shipped E2E fixture project, `e2e-happy`):

1. `gauntlet run` settles all 3 units (terminal=settled, iterations=3,
   exit 0). Durable state: settlements AK-901@4, AK-902@8, AK-903@12;
   close-out checkpoint (seq 4, iteration 3) all-settled, rounds_spent=3;
   three `evidence_produced` events at offsets 3/7/11 with rows.
2. Damage: DELETE the `evidence/marker-check-report.json` production row and
   roll the last checkpoint's `event_log_offset` 12 → 2 (below the produce
   event at 3) — the exact durable state the A5 crash window leaves (event
   durable above the anchor, row missing).
3. `gauntlet resume` → **exit 1, `terminal=budget_exhausted iterations=4`**
   while the printed report records 3/3 units implemented-and-proven and the
   terminal close-out checkpoint (seq 6) holds all three units SETTLED with
   rounds stuck at 3. The recovery half WORKS: the row was re-minted at the
   event's own ref/offset/timestamp (verified byte-level against the event
   log). The LOOP half then burns: the completed production defeats the
   settled no-op predicate (only `limitations` are forgiven,
   controller.rs:1197-1203), the full path runs one zero-dispatch
   reconciliation iteration, `book_iteration_round`'s round-4 booking is
   REFUSED (forward allowance = floor(max_rounds 4 × (100% − 15% reserve)) =
   3, already spent), and the refused booking sets `budget_exhausted`
   (controller.rs:1489-1499) BEFORE `decide_progress` — which would have
   returned `settled` — is ever consulted.

Contrast that nails the mechanism: the identical damage + `GAUNTLET_MAX_ROUNDS=5`
(forward allowance 4) → the same resume books round 4, `decide_progress` sees
all-settled, and the run reports `terminal=settled iterations=4`, exit 0.

Why this is the correction's own contract: the adjudication's (d) clause —
"an otherwise-settled run never reports `budget_exhausted` from recovery
bookkeeping" — and the shipped no-op predicate's own doc comment
(controller.rs:1178-1186) name exactly this burn ("re-entering the loop from
an all-settled fact set can only plan zero dispatches and burn empty
iterations until a budget dimension refused — reporting a `budget_exhausted`
terminal that misrepresents a fully settled run") as the thing the predicate
exists to prevent. COR-R3-1's mint half (completing orphan rows) is what
defeats the predicate: the gate forgives `limitations` but not
`productions_completed`. The two halves of the same correction contradict
each other here.

Reachability caveat, stated honestly: on a NATURALLY crashed run this state
is not producible — a pending produce event above the resume anchor implies
the crash happened inside that iteration, so that iteration's settlements
never happened and the run cannot be fully settled (on such runs the same
burn is the honest budget story: open units remain and the rounds are truly
gone). Reaching the dishonest terminal requires the fully-settled run PLUS a
pending orphan above the anchor, which I produced by out-of-band surgery that
includes ROLLING A CHECKPOINT OFFSET BACKWARD — state forgery against the
runtime-owned store, not mere row deletion (the shipped E2E's damage model).
The store DETECTS event-log holes (a deleted middle event is a typed
`Corrupt` refusal — I verified: "the event log is damaged: expected offset 7
but found 8") but does NOT validate checkpoint offsets against the log head,
which is what let the forged anchor through. Two honest fix directions for
the orchestrator to weigh: (a) widen the settled no-op predicate to treat a
completed production as compatible with the settled terminal when the
event-ordered projection is all-settled (the reconciliation it needs is
already durable by the predicate's own reasoning about pins), or (b) have
the store refuse a checkpoint whose offset regresses below its
already-recorded log head, closing the forgery precondition.

Not blocking: no dishonest settlement, no fabricated evidence; every durable
fact stayed truthful; the NEXT resume is the honest settled no-op (verified:
exit 0, `terminal=settled`, zero new checkpoints), so the wrong terminal is
not sticky.

### F-R4-2 (MINOR) — writer-lock `Held` refusal message: two honest-claim
wrinkles (behavior fail-closed in both; refusal never weakened)

Executed against the real binary, scratch fixture world
(`/tmp/falsifier-r4/run-world`):

1. **Pid-reuse attribution.** A stale `writer.lock` was planted recording the
   pid of a LIVE unrelated process (`sleep 300`, pid 1294703). The next
   `gauntlet run` refuses: `another writer holds the state dir
   (/tmp/…/writer.lock holds pid 1294703) — … If the holder pid is dead,
   remove the lock file manually and retry: this guard never guesses liveness
   and never takes over automatically`. The first clause asserts holding as a
   present-tense fact; with pid reuse it can be false (the named process is
   not a writer at all). The message's own instruction (confirm the pid is
   dead first) counteracts misattribution, and the guard never ACTS on
   liveness — the risk is confined to an operator who kills the named pid on
   the first clause alone. A wording like "a writer lock file names pid …"
   would remove the residual. Judged minor: fail-closed behavior intact, the
   manual recovery is named, no automatic action exists to mis-fire.
2. **Blank-pid window.** Freezing the holder deterministically between
   `create_new` and the pid `writeln` (SIGSTOP on lock-file appearance) makes
   a concurrent second `run`/`resume` read an EMPTY lock file: the refusal
   fires, exit 1, but prints `holds pid ` with a blank where the pid belongs
   (`holder_pid` is `Some("")`, not `None`, so the `<unreadable>` fallback
   does not trigger). Narrow window, purely cosmetic, refusal semantics
   intact — but the adjudication's "naming the holder pid" property is
   vacuous in exactly this race the guard exists to arbitrate.

Both shapes live in `crates/gauntlet-cli/src/writer_lock.rs`
(`WriterLockError::report` / `acquire`).

### F-R4-1 (MINOR) — skills verify's content anchor is a NORMALIZED digest; a
reforged lock over a normalization-equivalent (non-byte-identical) installed
body passes where roles refuses the same shape

Reproduction (scratch world `/tmp/falsifier-r4/skills-world/proj`, real
`gauntlet` binary):

1. `gauntlet skills install` — honest round trip, verify clean (8 skills).
2. Rewrite the INSTALLED `.gauntlet/skills/neatcode/SKILL.md` with
   byte-different but normalization-equivalent content (every `\n` → `\r\n`,
   plus extra trailing blank lines), and reforge `.gauntlet/skills.lock`'s
   `neatcode` digest to `sha256` over the NORMALIZED swapped bytes (count,
   schema, name set all consistent).
3. `gauntlet skills verify` → **exit 0**, `skills: verified 8 skill(s) — no
   drift, lint clean` — even though `installed bytes != source bytes`
   (asserted by the harness before verify ran).

The same attack against `roles verify` in a clean world
(`/tmp/falsifier-r4/roles-world/proj`, prime-agent harness, state dir pinned
via `GAUNTLET_STATE_DIR`): installed `builder.md` swapped the same way, lock
digest reforged to the verbatim digest of the swapped bytes (self-digest
recomputed) → **exit 1**, `[builder] source file modified since install
(recorded sha256:d618…, actual sha256:db3a…)`.

Cause: `crates/gauntlet-cli/src/skills_cmd.rs` digests through
`gauntlet_app::reference::normalize::digest_of` (CRLF→LF fold +
trailing-blank-line collapse) for BOTH the source and installed halves, while
roles (`crates/gauntlet-app/src/roles/install.rs`) digests VERBATIM bytes
(`sha256_hex(bytes)` at install and in both verify halves). Under a reforged
lock, the skills source anchor compares normalized-to-normalized, so any
installed body inside the normalizer's fixed point class (line-ending
spellings, trailing blank lines) verifies "no drift".

Severity argument, honest both ways: the normalizer's fixed-point class
cannot carry a semantic change (a skill's instructions are identical modulo
line endings and trailing blank lines), so no content survives that the
canonical source does not already state — this is not a path to smuggle new
skill text past the reviewed source. It is (a) an asymmetry against the roles
surface this correction was explicitly mirror-shaped after (roles refuses the
same forgery), and (b) a false claim on the happy path ("no drift" printed
over bytes that are not the canonical bytes). Fix direction (for the
orchestrator to weigh): digest the installed half byte-exactly (the source
half may keep normalization if the lock schema requires it — but see that
install itself writes the source bytes verbatim, so verbatim on both halves
round-trips today).

Not blocking: the COR-R3-2 attack shapes the correction targets (semantic
swaps, source-revert, dupes, deleted source) all refuse (below).

## Failed attacks (mechanism 2 — skills content-verify)

All in the scratch world, real binary, honest baseline first:

- Honest round trip: install → verify exit 0, 8 skills, lock is canonical
  JSON with the exact owned set. Reinstall after each attack repairs.
- **Swapped body + REFORGED lock (digest consistent with swapped bytes):**
  REFUSED, exit 1, `'grilling' SOURCE DRIFTED from the lock — lock
  sha256:8ae5…, source sha256:9eac…` — names the skill and BOTH digests
  (exactly COR-R3-2's required shape).
- **Source edit after install (stale-install revert):** REFUSED, exit 1,
  `'research' SOURCE DRIFTED …` naming both digests; restoring the source
  byte-identically (sha256-checked against the pre-attack baseline) returns
  verify to exit 0.
- **Duplicate entries (equal name + equal digest, count updated to match):**
  REFUSED, exit 1, `records a duplicated skill entry`.
- **Source file deleted:** REFUSED, exit 1, `'prototype' SOURCE is MISSING
  from the provenance tree (…)` — names the missing canonical path; verify
  is not "clean with fewer skills".

## Failed attacks (mechanism 3 — writer lock)

Real binary, fixture world, every refusal typed exit 1:

- **Mutual exclusion at startup (deterministic).** Holder frozen with SIGSTOP
  the moment `writer.lock` appears → a concurrent `gauntlet run` AND a
  concurrent `gauntlet resume` both refuse typed, naming the holder pid and
  the manual stale-lock recovery; the frozen holder then CONTinues and
  SETTLES its run; the lock file is REMOVED by the holder's exit (clean
  release on the normal path).
- **Kill while holding (SIGTERM delivered to the frozen holder, then CONT —
  process dies by default disposition, no handlers installed):** the lock
  file REMAINS; the next run refuses typed naming the dead pid and the
  manual recovery; removing the file by hand restores run capability
  (fresh-seed run settles, exit 0). Exactly the documented fail-closed
  stale-lock story — an honest manual recovery is genuinely needed and is
  printed in the refusal itself.
- **Dry-run is lock-free and zero-write.** With a stale `writer.lock`
  planted, `gauntlet run --dry-run` exits 0 (no refusal), the planted lock
  content is untouched, and a before/after whole-world file-set+content
  hash diff shows zero program-written changes (charter D1 holds under an
  adversarial lock state).
- **Placement.** The lock is created only under the resolved state dir
  (`$GAUNTLET_STATE_DIR/writer.lock`); no writer.lock appears at CWD or at
  the repository root after any of the above.

## Failed attacks (mechanism 1 — orphan recovery / COR-R3-1)

Worlds: `/tmp/falsifier-r4/rec-world` (settled-run damage surgery) and
`/tmp/falsifier-r4/kill-world` (real kill -9 mid-run). Ground truth read from
the sqlite stores; every pin's claims were checked against the events table,
the productions table, and the content-addressed objects.

- **A5 window self-completion (mint half, produced world).** Row deleted +
  last checkpoint's anchor rolled below the produce event (the A5 end-state):
  resume re-minted the row at the EVENT'S OWN identity — evidence_ref equals
  the event payload's ref byte-for-byte, `event_log_offset` = the event's
  offset (3), `produced_at` = the event's timestamp (10) — exactly what
  COR-R3-1 requires. (The re-minted row's `iteration_no` carries the
  recovery's durable iteration, not the original's — the event payload
  records no iteration; noted, not a false claim.)
- **Per-locator granularity (mixed damage, natural anchor).** Two rows
  deleted, one locator's intact row untouched → resume pins EXACTLY ONE
  limitation naming only the damaged locator; the intact locator is silent;
  no cross-masking.
- **Event-log holes are refused, not Papered over.** Deleting a MIDDLE event
  is a typed store refusal (`stored state is corrupt: the event log is
  damaged: expected offset 7 but found 8 — rows were removed out of band; an
  append-only log has no holes`); the recovery never runs on the corrupt
  log. (Tail truncation — a legal crash shape — is accepted.)
- **Forensics content truthfulness.** The bytes-absent pin read
  `{"class":"evidence_production_orphan_unrecoverable","detail":"the durable
  orphan event's evidence sha256:98cbdd… (offset 3) does not resolve in the
  evidence store"}` — the named event/offset exist in the log and the named
  bytes were genuinely absent when the pin was written (the object was
  moved aside by the harness). The below-anchor pin read "…no
  EvidenceProduced event at or after the resume anchor (offset 0) names this
  locator…" — TRUE (the event was truncated). No false "no event exists" or
  byte-presence claims observed in any pin.
- **Forensics silent on open units (unsettled run).** kill -9 mid-run
  (mid-iteration-3: AK-903 open, unproduced, no event, no row): after the
  kill and after the recovery resume, ZERO `orphan_unrecoverable` pins — the
  open unit's missing row is correctly not treated as damage.
- **Settled no-op with pins only (COR-R3-1(d), pins half).** Row-deletion
  damage with the natural (above-events) anchor: resume → honest per-locator
  pins, `terminal=settled`, exit 0, no iteration burned. Byte-absent variant
  (event above anchor via rolled anchor, object removed): pin fires, row
  stays absent (no fabrication), `terminal=settled`, exit 0. Both exactly
  the pins-don't-defeat-the-no-op contract.
- **Second resume is always an honest settled no-op.** After every
  full-path resume (including the F-R4-3 burn), a further resume reports
  `terminal=settled`, exit 0, with byte-identical zero writes (DB + evidence
  tree hashed before/after).

## Over-rejection leg

Executed against the real binary and the shipped suites; nothing weakened,
nothing skipped:

- **Shipped E2E suite green AS SHIPPED:** `just test-package gauntlet-cli`
  → **90 tests run: 90 passed, 0 skipped**, including the nine
  `e2e_full_loop_mock` legs (happy loop settles every unit; lying builder
  never settled; forged-report liar refused with the typed
  `producer_uncorroborated` pin — the CORROBORATED-PIN tooth; dry-run
  zero-write; parallel carry; settled no-op; all-rows-deleted honesty;
  per-locator damage naming; single-writer double-resume), the full p1–p12 /
  g6/g7/g10 proof set, `skills_roundtrip`'s five COR-R3-2 teeth, and the
  `cli_surface` locator suite.
- **Honest builders, kill -9 mid-run + first/second resume (real binary,
  produced world, fresh world).** kill -9 at mid-iteration-3 (AK-903 still
  open, unproduced): resume #1 → exit 0, `terminal=settled`, all 3 rows +
  settlements durably landed, ZERO spurious forensic pins; resume #2 →
  `terminal=settled`, exit 0, ZERO writes (sha256 of the state DB and of the
  whole evidence tree identical before/after); no writer.lock left by
  either. (After the kill itself the stale writer.lock is present and the
  first resume attempt is refused typed with the manual recovery — the
  documented fail-closed story; removing the file per that message restores
  service.)
- **Default-world honest-settler proofs:** the produced-world CLI
  composition is the only binary surface (compose_run); the default world's
  honest-settler, settled-no-op-both-worlds
  (`resuming_an_all_settled_run_is_an_honest_settled_no_op` /
  `…_in_the_produced_world`), and parallel-carry proofs ran in the app
  battery: `just test-package gauntlet-app` → **471 tests run: 471 passed,
  0 skipped**.
- **Guardrails:** `just ruler` PASS before and after the round (exit 0, the
  frozen lock's digests match). No tracked repository file was read-modified
  or written by this round; all probe worlds, damage surgery, and forged
  locks live under `/tmp/falsifier-r4/` (skills-world, roles-world,
  run-world, rec-world, iso-world, kill-world). Every source file
  temporarily altered inside a scratch world was restored sha256-verified
  where the round trip required it.

## Verdict

NOT_CONFIRM — 0 blocking / 1 major (F-R4-3) / 2 minor (F-R4-1, F-R4-2).

The three COR-R3 mechanisms hold on their mandated shapes: skills
content-verify refuses every adjudicated forgery shape (F-R4-1 records the
normalized-digest residue roles does not have); the writer lock excludes
concurrent writers, fails closed on kills with the documented manual
recovery, and never touches dry-runs (F-R4-2 records two message-wording
wrinkles); the orphan recovery mints at the event's own ref, pins honestly
and per-locator, stays silent on open units, and never lets pins burn a
settled run (F-R4-3 records the one path where a COMPLETED production still
does). Over-rejection leg fully clean: 90/90 CLI-package (shipped E2E suite
as shipped) and 471/471 app-package tests green, kill -9 + first/second
resume honest and zero-write on re-resume.
