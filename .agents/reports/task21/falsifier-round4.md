# Falsifier Round 4 (FINAL, minimal) — Task 21 operator surfaces

Blind executing falsifier, round 4. Scope: re-execute round 3's three attack
families against the COR-3 tree (UNSTAGED on top of staged COR-1, HEAD
a71eba9), then attack the new enumeration-free invariant test's own teeth.
No re-run of rounds 1–3's full sweeps.

Tree under test: HEAD a71eba9 + the full correction stack visible in
`git diff HEAD` / `git diff --cached`. Binary: `target/debug/gauntlet`,
`--features surfaces`, built under
`flock .agents/reports/task21/compile.lock` (cargo: Finished, nothing to
rebuild — the orchestrator battery had it current; the `sccache
--stop-server` client timed out inside the flock'd commands, the
known-harmless round-3 pattern; the lock was held and released). Worlds:
`/tmp/falsifier-r4/{wA,wB}` — wA seeded by a REAL `e2e-happy` mock-loop run
through the real binary (run `MGA2MKJTE80AZ87B2R69XPWTM3`,
`GAUNTLET_ID_SEED=falsifier-r4-seed`, units AK-901/902/903 settled,
terminal=settled, exit 0); wB a copy for the leg-4 attacks. Byte-scan
instrument: `/tmp/falsifier-r4/scan.py` (invariant (a)'s exact predicate:
flags any byte < 0x20 except 0x0A and 0x09). Every §20.3-line count below
is `grep -c '^operator-action:'` on the captured stderr. All captures kept
raw under `/tmp/falsifier-r4/out-*`.

## Leg 1 — R3-F-1 re-attack (ESC ticket id, `resolve --ticket` re-entry ACCEPT) — HELD

Seed: a hand-planted queue record in the queue's own on-disk schema whose
ticket id is `\x1b[31mesc-ticket` (ESC passes `TicketId::new` — no
whitespace, no separator), file `\x1b[31mesc-ticket.json` planted with a
raw ESC byte in the name, anchored to the seeded run.

Drive (real binary, wA): `escalations resolve --ticket $'\x1b[31mesc-ticket'
--unit AK-903 --operator falsifier-cli` — AK-903 is durably settled, so this
is the idempotent re-entry accept. **Exit 0.** Raw captures:

```
stderr (the §20.3 line), cat -A:
operator-action: action=resolve run_id=MGA2MKJTE80AZ87B2R69XPWTM3 unit=AK-903 \
  operator="falsifier-cli" outcome=accepted tickets=\u{1b}[31mesc-ticket$
stdout (the CLI acceptance line), cat -A:
escalations: ticket=\u{1b}[31mesc-ticket action=resolve operator="falsifier-cli" \
  — settled unit AK-903 of run MGA2MKJTE80AZ87B2R69XPWTM3; \
  resolved tickets: \u{1b}[31mesc-ticket$
```

- Byte-scan (invariant (a) predicate): **stderr clean, stdout clean** — ZERO
  raw control bytes in the §20.3 line AND the CLI acceptance line (round 3
  measured one raw 0x1b in stderr and two in stdout; the attack no longer
  executes).
- The escaped form `\u{1b}[31mesc-ticket` is present in BOTH lines
  (grep-confirmed) — greppable, human-readable.
- Exactly ONE `operator-action:` stderr line for the one action.

**Legitimate-line corruption check — none.** A PLAIN ticket
(`t-r4-plain.json`, planted the same way, plain id) driven the same way
(`resolve --ticket t-r4-plain --unit AK-901 --operator falsifier-cli`,
re-entry accept, exit 0) prints `ticket=t-r4-plain` and
`tickets=t-r4-plain` VERBATIM in both lines (grep-confirmed),
byte-scan clean — `log_quote` is the identity on clean text; the escape
did not corrupt the legitimate line, which stays greppable.

## Leg 2 — R3-F-2 re-attack (newline payloads through the dispatcher paths) — HELD

wA, real binary, payload
`bogus\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT`
(and `x\n…` for the list argument):

- `escalations $'bogus\n…'` → **exit 2**; stderr is **exactly ONE line**,
  the payload escaped inside the quoting:
  `gauntlet escalations: unknown subcommand 'bogus\noperator-action: …tickets=FAKE-TKT'`
  (cat -A: a literal backslash-n; one line terminator total);
  **zero `^operator-action:` lines**; byte-scan clean (stdout carries only
  the static usage).
- `escalations list $'x\n…'` → **exit 2**; stderr **exactly ONE line**,
  escaped (`gauntlet escalations list: unknown argument 'x\noperator-action: …'`);
  zero forged lines; byte-scan clean.

The R3-F-2 forged-second-line attack no longer executes against either
dispatcher locus.

## Leg 3 — R3-F-3 re-attack — HELD (both vectors)

**(a) Quarantined queue file with a hostile stem.** The queue dir holds a
corrupt file literally named
`x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted
tickets=FAKE-TKT.json` (content `not a ticket record`). A PLAIN
`escalations list` (exit 0) prints the quarantined entry as ONE physical
line with the stem ESCAPED:

```
escalations: ticket=x\noperator-action: action=resolve run_id=FORGEDRUN \
  outcome=accepted tickets=FAKE-TKT state=QUARANTINED reason=record-invalid \
  — the file is not a valid ticket record; actions on it are refused$
```

Byte-scan: stdout clean, stderr clean; zero §20.3-shaped lines anywhere in
the listing. The record lines beside it are clean too (the planted ESC id
lists as `\u{1b}[31mesc-ticket`, escaped).

**(b) The argv-reachable durable operator vector.** The open ticket
`t-r4-opwrite` (planted plain) driven through the SANCTIONED path:
`escalations resolve --ticket t-r4-opwrite --unit AK-902 --operator
$'op\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted
tickets=FAKE'` → re-entry accept over the durably-settled AK-902, **exit
0**. Both action lines carry the operator Debug-quoted
(`operator="op\noperator-action: …"`) — escaped, one physical line,
byte-scan clean. The runtime durably wrote the identity into the ticket
record (record format out of scope — the JSON file carries
`"operator":"op\n…"`; the contract is the display). The subsequent PLAIN
`escalations list` (exit 0) shows the resolved record as ONE line:

```
escalations: ticket=t-r4-opwrite run=MGA2MKJTE80AZ87B2R69XPWTM3 \
  state=resolved action=resolve \
  operator=op\noperator-action: action=resolve run_id=FORGEDRUN \
  outcome=accepted tickets=FAKE summary="r4 operator vector ticket"$
```

Byte-scan clean; the forged §20.3-shaped line does NOT appear (zero
`^operator-action:` lines in the listing stdout). Round 3's list-forgery
attack — quarantined stem, operator field, ESC id — no longer executes.

## Leg 4 — the invariant test's own teeth

Target: `no_hostile_record_or_argument_can_escape_the_family_s_output_discipline`
(`crates/gauntlet-cli/tests/operator_actions_no_bypass.rs:1512`). Its drive
set: dispatcher, list-unknown-arg, failed (CLEAN missing id
`t-cor3-norecord`), refused (newline operator, outside ruler), accepted-esc
(ESC id with a VALID planted record), accepted-operator, list. Its
invariant over the full captured stdout+stderr of those drives:
(a) zero raw bytes < 0x20 except \n and \t; (b) `^operator-action:` stderr
line count == actions driven; (c) escaped forms present.

**Defeat attempt — hostile records through channels/shapes its seeding and
drives do not produce** (wB; all executed on the real binary):

- **4.1 Valid record with a hostile SUMMARY** (`t-r4-hsum.json`: plain id,
  summary = `line-one\noperator-action: … tickets=FAKE-TKT \u{1b}[31mred` —
  newline + forged §20.3 line + ESC; the test's planted record has a clean
  summary). Plain `escalations list`: the summary lands Debug-quoted in ONE
  physical line (`summary="line-one\noperator-action: … \u{1b}[31mred"`),
  byte-scan clean, zero forged lines. **The discipline holds on this
  channel** (positive control).
- **4.2 Cross-wired record** (`t-r4-liar.json`: stem `t-r4-liar`, record's
  `ticket` field `\u{1b}[31mliar` — decodes valid, fails the queue's
  identity binding; a shape neither round 3 nor the invariant seeds).
  `resolve --ticket t-r4-liar --unit AK-901` → exit 1. The §20.3 FAILED
  line carries the detail ESCAPED (`…names \u{1b}[31mliar…`) — and the
  HUMAN failure line beneath it prints the same detail VERBATIM:
  `cat -A` shows `^[[31mliar`. **Byte-scan: 1 RAW CONTROL BYTE (0x1b,
  offset 443) in the captured stderr.**
- **4.3 Grammar-valid ESC id with NO record** (`resolve --ticket
  $'\x1b[31mmissing'`) → exit 1; §20.3 line escaped
  (`\u{1b}[31mmissing is not a valid record: No such file or directory`),
  human line `^[[31mmissing` — **1 RAW 0x1b (offset 272)**.
- **4.4 ESC-stem corrupt record** (`resolve --ticket $'\x1b[31mcorrupt'`,
  file `\x1b[31mcorrupt.json` holding `garbage`) → exit 1; §20.3 line
  escaped, human line `^[[31mcorrupt` — **1 RAW 0x1b (offset 272)**.
- In ALL three failed shapes: exactly ONE §20.3 line (the count invariant
  holds — no forged line; the detail's embedded ids are grammar-checked and
  cannot carry newlines), `run_id=`/`tickets=` honestly empty (the durable
  reads never resolved them — the log never invents).
- **4.5 Hostile `--note`** (newline + forged §20.3 line) through a
  sanctioned re-entry resolve (exit 0): the note never surfaces in ANY
  output line (§20.3, acceptance, or listing — all byte-clean, zero forged
  lines); it travels into the durable record only (out of scope). The last
  free-form argv channel holds.
- **Instrument context (not evidence):** the shipped invariant test run on
  this tree, filtered to itself —
  `cargo nextest run -p gauntlet-cli --features surfaces --test
  operator_actions_no_bypass no_hostile_record_or_argument_can_escape_the_family_s_output_discipline`
  → **1 passed, 12 skipped** — GREEN while the raw bytes above survive
  through drives it never takes.

**Assessment.** The invariant itself was not defeated where it bites: every
newline-capable channel I could reach (argv remnants, summary, operator,
stems, note) lands escaped, and the §20.3 count held everywhere. What the
attack exposed is a coverage hole in the invariant's own drive set — R4-F-1
below. The one surviving raw-control-byte sink is the RECORDED COR-2
sanctioned boundary (the human CLI failure detail line at
`crates/gauntlet-cli/src/escalations_cmd.rs:329`, printing the queue error
`Display` verbatim — `RecordInvalid`/`TicketIdentityMismatch` embed the
grammar-valid ticket id RAW, `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs:74-79`
and `:88-93`), so this is not a class regression — but no shipped tooth
observes that boundary, and the invariant accommodates it only by not
driving it.

## Findings

### R4-F-1 — MINOR — the enumeration-free invariant's "zero raw control bytes across captured stdout+stderr" is a property of its drive SUBSET, not of the family: raw ESC demonstrably survives in family stderr through three executed shapes its drive set never takes, and it has no explicit accommodation for the sanctioned verbatim sink it would red-flag

Executed repro (wB, real binary; captures `/tmp/falsifier-r4/out-l4-*`):

```
$ gauntlet escalations resolve --ticket $'\x1b[31mmissing' --unit AK-901 --operator falsifier-cli
stderr line 1 (§20.3): operator-action: … outcome=failed tickets= code=action_failed \
  — "escalation ticket \u{1b}[31mmissing is not a valid record: …"   (escaped, clean)
stderr line 2 (human): gauntlet escalations resolve: escalation ticket ^[[31mmissing \
  is not a valid record: …                                            (RAW 0x1b survives)
```

Byte-identical posture for the cross-wired claimed id (`t-r4-liar` →
`^[[31mliar`, offset 443) and the ESC-stem corrupt record (`^[[31mcorrupt`).
The shipped invariant test is GREEN on this tree (1/1 executed) because its
drive set omits every one of these shapes: its `failed` drive uses a CLEAN
missing id, and no drive addresses a hostile-id record whose READ fails.
The COR-2-era test that does drive the ESC-id failed shape
(`a_control_byte_ticket_id_lands_escaped_in_the_failed_action_log_line`,
`operator_actions_no_bypass.rs:1327`) scopes its assertions to the §20.3
LINE only — so across all shipped teeth, the full capture of a hostile-id
failed action is asserted NOWHERE, and the raw byte in the sanctioned
human line is observed by nothing.

Why this is a teeth finding and not just the recorded boundary:

1. The COR-3 contract (`.agents/reports/task21/cor3-brief.md` item 1) says
   the invariant "must accommodate" the verbatim non-log consumers. It
   accommodates them by AVOIDANCE, not by construction — no carve-out
   exists and no test records the boundary. Adding the natural drive
   (resolve on a hostile-id record) would RED the invariant on a SANCTIONED
   sink — a false positive against the recorded decision; the teeth cannot
   be extended to full coverage without first writing that accommodation.
2. No shipped tooth guards the boundary against WIDENING: if a future
   queue/store error Display embedded a newline-capable string, a forged
   line through the human sink would be invisible to clauses (a) (newline
   is allowed) and (b) (never captured) until the drive set is extended.
   Today that embedding is unreachable (every error-embedded id is
   grammar-checked whitespace-free; the summary/operator/note never enter
   error Displays) — so no forged §20.3 line is reachable through it.

Severity MINOR (not INFO): the residue sits on the round's load-bearing
deliverable with an executed 1-command repro and a concrete failure mode
(brittle-under-extension + unobserved sanctioned sink). Not MAJOR: the
surviving raw byte IS the recorded COR-2 decision behaving as recorded, no
forgery is reachable, and every invariant clause holds over everything the
test actually captures. Minimal closing shape: drive the hostile-id failed
shape in the invariant and assert the PAIRING (the human line's detail is
exactly the unescaped content of the §20.3 line's `{:?}`-quoted detail, and
nothing else precedes/follows them) — or carve the sanctioned line out of
(a) explicitly and say so in the test.

### Informational — out of scope, not attacked

Usage errors of OTHER subcommands (e.g. `gauntlet status --<hostile>`) were
not attacked, per the commission's boundary. The durable record's own
FORMAT (the hostile operator stored in the ticket JSON) was not scored —
display, not storage, is this slice's contract.

## Per-fix record

- **R3-F-1 (accepted-arm raw ESC) — FIXED, attack does not execute.** Leg 1:
  zero raw control bytes in the §20.3 accepted line and the CLI acceptance
  line; escaped forms present in both; plain ids byte-identical and
  greppable. Loci confirmed in the diff:
  `crates/gauntlet-cli/src/operator_action.rs:133-144` (`log_quote` on the
  resolved-tickets join), `crates/gauntlet-cli/src/escalations_cmd.rs:304-317`
  (`log_quote` on the requested ticket and the resolved list).
- **R3-F-2 (dispatcher forged lines) — FIXED, attack does not execute.**
  Leg 2: one escaped stderr line each, exit 2, zero forged lines. Loci:
  `escalations_cmd.rs:39-42` (unknown subcommand), `:71-74` (list unknown
  argument), both through `log_quote`.
- **R3-F-3 (list output forgery: quarantined stem, operator field, ESC ids)
  — FIXED, attack does not execute.** Leg 3: quarantined stem escaped; the
  argv-reachable operator vector lands the identity into the durable record
  via the sanctioned resolve (exit 0, format out of scope) but the plain
  listing prints it escaped with zero §20.3-shaped lines; ESC record ids
  list escaped. Loci: `escalations_cmd.rs:118-123` (stem),
  `:137-167` (operator + ticket via `log_quote`, summary `{:?}` already),
  with the §20.3 refused/failed arms carrying the new `run_id=`/`tickets=`
  fields through `log_quote` (`operator_action.rs:145-168`).

## Provenance

- Worlds: `/tmp/falsifier-r4/wA` (seeded by the REAL `e2e-happy` mock-loop
  run through the real binary: run `MGA2MKJTE80AZ87B2R69XPWTM3`, exit 0,
  `terminal=settled`; env per the e2e harness recipe —
  `GAUNTLET_STATE_DIR`, `GAUNTLET_ARTIFACT_DIR`, `GAUNTLET_MOCK_SCENARIO`,
  `GAUNTLET_ID_SEED=falsifier-r4-seed`, `GAUNTLET_CLOCK=deterministic`,
  stall/retry controls, `GAUNTLET_TOOL_TEST` = the fixture's own check
  script) and `/tmp/falsifier-r4/wB` (a copy). Queue seeds were planted as
  files in the queue's own on-disk schema (`TICKET_SCHEMA`
  `gauntlet-escalation-ticket@v1`, exact field set), including raw-ESC
  filenames and a hostile-stem quarantine file. All probes under /tmp; the
  repository was modified by NOTHING except this report (verified:
  `git status` — no tracked-file changes, `.wayfinder/`, `.gauntlet/`,
  `.agents/specs/` untouched).
- Binary: `target/debug/gauntlet`, `--features surfaces`, all compilations
  and the instrument test run under
  `flock .agents/reports/task21/compile.lock -c '… && sccache --stop-server'`
  (the sccache client timed out inside the lock each time — the known
  harmless pattern; locks held and released).
- Blindness: builder/orchestrator reports (`cor*-report/progress`, `b1-*`,
  `IN-FLIGHT.md`, any `*.log`, any `*adjudication*`, `closure.md`) were
  never opened. Inputs: `falsifier-round3.md` (attack spec), `cor3-brief.md`
  (contract), `charter.md`, the spec, and all code/tests/fixtures.

---

VERDICT: CONFIRM (0 blocking / 0 major / 1 minor / 1 informational)
