# Task 21 — COR-3 report (builder, FINAL class-closing sweep)

Commission: `.agents/reports/task21/cor3-brief.md`. Defect-class
specification: `.agents/reports/task21/falsifier-round3.md` (R3-F-1/2/3).
Tree: HEAD a71eba9 + staged COR-1 + unstaged COR-2; this slice's edits
land unstaged on top; nothing committed, staging untouched as found.
Changed files (the commissioned set only):
`crates/gauntlet-cli/src/escalations_cmd.rs`,
`crates/gauntlet-cli/src/operator_action.rs`,
`crates/gauntlet-cli/tests/operator_actions_no_bypass.rs` (test added).
Frozen artifacts (`.wayfinder/`, `.gauntlet/`, `.agents/specs/`) diff-clean.
No new dependencies. No behavior change beyond output escaping (exit
codes, refusal vocabularies, durable writes, JSON semantics byte-identical
— gates 1–3's frozen tests pass unchanged).

## 1. The one escaping discipline

One helper, `log_quote`, in `crates/gauntlet-cli/src/operator_action.rs:30-42`
(`pub(crate)`, imported by `escalations_cmd.rs:16`):

```rust
pub(crate) fn log_quote(value: &str) -> String {
    value.escape_debug().collect()
}
```

Contract: Rust Debug escaping WITHOUT added quoting — hostile bytes land
in their Debug escapes (`\n`, `\t`, `\u{1b}`, `\\`, `\"`), so a newline
can no longer forge a following line and a control byte can no longer
ride raw, while already-clean text passes through BYTE-IDENTICAL. That
byte-identity is load-bearing: the frozen tests pin clean forms plainly
(`tickets={id}` at `operator_actions_no_bypass.rs:717/:940`,
`resolved tickets: {id}` at `:902`) and Debug-quoted forms elsewhere
(`operator={OPERATOR:?}` at `:852`); the discipline keeps every pin green
without touching those tests. Sites already Debug-quoted at their sink
(`{:?}`: the operator identity, the list summary, the failed-arm detail,
the pre-parse refusals) are the same discipline at those sites and were
left byte-identical.

## 2. Per-interpolation audit (mechanical; every `println!`/`eprintln!`
in both files accounted for)

### `crates/gauntlet-cli/src/escalations_cmd.rs`

| line (post-fix) | dynamic content | verdict |
|---|---|---|
| :39-42 | dispatcher unknown-subcommand: `other` = raw argv | **ESCAPED (fix, R3-F-2)** |
| :54-63 | `usage()` lines | static constants |
| :71-74 | `list` unknown-argument: `other` = raw argv | **ESCAPED (fix, R3-F-2)** |
| :81, :279 | "the configuration did not resolve: {error}" | trusted-configuration (resolved config paths/env; the operator's own configuration) |
| :89 | "no queue at {}", `root.display()` | trusted-configuration (resolved state dir) |
| :95, :109 | queue open/list errors | trusted-configuration (`RootFailed`/`ListFailed` details are the queue-root path + OS io text; never record content) |
| :118-123 | quarantined `entry.ticket` = the FILE STEM, never grammar-checked | **ESCAPED (fix, R3-F-3 locus 1)**; `entry.class.token()` static vocabulary token |
| :127 | "queue empty" | static |
| :133-147 | resolved-state build: action token (static enum token); `operator` from the record's free-form `OperatorIdentity` — argv-reachable through the sanctioned resolve | token trusted constant; **operator ESCAPED (fix, R3-F-3 locus 2)** |
| :155-166 | record line: `record.ticket` (grammar-valid — control bytes ride the grammar), `record.run_id` (ULID grammar; decode refuses non-ULIDs), `state` (built from static tokens + the escaped operator), `record.summary` (`{:?}` since COR-2) | **ticket ESCAPED (fix, R3-F-3 locus 3)**; run_id trusted-by-grammar; summary already escaped, unchanged |
| :194, :201, :228 | "--{name} given twice"/"requires a value" | trusted constants (`name`/`action_word` are call-site literals/static tokens) |
| :236 | unknown argument `{other:?}` | already escaped (COR-2), unchanged |
| :243-248 | missing-required refusal | static text + static action token |
| :256 | `{ticket:?}` pre-parse refusal | already escaped (COR-2), unchanged |
| :263 | `{unit:?}` pre-parse refusal | already escaped (COR-2), unchanged |
| :269-274 | operator-identity refusal: `OperatorIdentityError::Blank` Display | trusted constant (static text, never embeds input) |
| :304-318 | CLI acceptance line: `ticket` (argv; grammar-validated by this point — control bytes ride the grammar), `operator` `{:?}` (escaped), `acceptance.unit`/`run_id` (UnitId/RunId grammar), `resolved` (validated ids — control bytes ride the grammar) | **ticket + resolved ESCAPED (fix, R3-F-1)**; operator already escaped; unit/run trusted-by-grammar |
| :321-327 | refused human line: `refusal.message` | trusted-typed (typed refusal Display: grammar-bounded ids, configuration paths, chain vocabulary; the refusal voice, vocabulary frozen) |
| :329 | `Err(detail)` human line | **recorded NON-LOG boundary (COR-2 decision)** — stays VERBATIM; see §5 |

### `crates/gauntlet-cli/src/operator_action.rs`

| line (post-fix) | dynamic content | verdict |
|---|---|---|
| :30-42 | `log_quote` — the family helper | the discipline itself |
| :133-143 | §20.3 ACCEPTED arm: run_id (ULID grammar), unit (UnitId grammar), `operator` `{:?}` (escaped), `tickets` join (validated ids; control bytes ride the grammar) | ids trusted-by-grammar; operator already escaped; **tickets ESCAPED (fix, R3-F-1)** |
| :145-155 | §20.3 REFUSED arm: `run_field` (ULID grammar), unit (grammar), `operator` `{:?}` (escaped), `tickets_field`, `code` (static vocabulary) | **tickets_field ESCAPED (fix)**; rest trusted/escaped |
| :160-170 | §20.3 FAILED arm: `tickets_field`, `detail` `{:?}` (escaped since COR-2), `operator` `{:?}` (escaped) | **tickets_field ESCAPED (fix)**; rest already escaped, unchanged |
| detail/refusal `format!` constructors (:190s-:450s) | not output sites — reach output ONLY via the §20.3 arms (escaped) or the recorded non-log consumers | covered at the sinks |

### Brief item 3 — the API's server-side §20.3 line

The server-side §20.3 line is emitted ONLY by
`execute_operator_action` — `durable_surface.rs::perform` calls the same
shared path the CLI calls — so it is covered by this fix directly, and
COR-1's `ActionLogContext` shape already gave the refused/failed arms
their run/ticket fields. `durable_surface.rs` itself feeds NO record
field into any log sink: `perform` returns typed outcomes whose messages
reach only the JSON body (the recorded non-log consumer), and the JSON
adapter's access log method/target are already Debug-escaped (COR-2 D2).
**No `durable_surface.rs` change needed.**

Out-of-scope sibling recorded (not touched, per guardrail):
`crates/gauntlet-cli/src/serve_cmd.rs:28` interpolates the raw
unknown-argument token — the same CLASS as R3-F-2, outside the
commissioned escalations/operator-action family.

## 3. The enumeration-free invariant test — RED pre-fix, GREEN post-fix

New test `no_hostile_record_or_argument_can_escape_the_family_s_output_discipline`
in `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs` (appended;
the COR-3 block). It seeds hostile records through the queue's own
substrate plus argv — a grammar-valid ESC ticket id with a record in the
queue's own on-disk schema (the mint refuses to EMIT such an id; that
refusal is the defense, so the record is a hand-planted durable-state
write, the falsifier's executed vector and leg 5's corrupt-file
precedent), a quarantined queue file whose stem carries a newline + a
forged §20.3 line, a second open ticket minted through the queue's OWN
`escalate()`, a hostile operator identity (`op\noperator-action: …`)
delivered through the SANCTIONED resolve's argv, and hostile dispatcher
subcommand / list-argument tokens — then drives `escalations list`, a
REFUSED resolve, a FAILED resolve (queue-read failure), a fresh ACCEPT
and a re-entry ACCEPT on a settled unit, the dispatcher and
unknown-argument paths, and asserts over the FULL captured stdout+stderr
(raw bytes):

- (a) zero bytes < 0x20 except `\n` (0x0A) and `\t` (0x09);
- (b) the count of stderr lines starting with `operator-action:` equals
  the number of actions actually driven (4) — forged §20.3 lines break
  the count wherever they were interpolated from;
- (c) every hostile payload lands in its Rust-Debug-escaped form
  (greppable, human-readable) wherever it legitimately surfaces.

All violations are collected and reported in ONE assertion, so a single
RED run names every live locus.

### RED — pre-fix tree (verbatim from `.agents/reports/task21/cor3-red-run.log`)

```
FAIL [   0.059s] (1/1) gauntlet-cli::operator_actions_no_bypass no_hostile_record_or_argument_can_escape_the_family_s_output_discipline
thread 'no_hostile_record_or_argument_can_escape_the_family_s_output_discipline' (3818107) panicked at crates/gauntlet-cli/tests/operator_actions_no_bypass.rs:1754:5:
THE FAMILY OUTPUT INVARIANT IS BROKEN (11 violation(s)):
(a) raw control byte 0x1b at offset 20 in the stdout of [accepted-esc]
(a) raw control byte 0x1b at offset 157 in the stdout of [accepted-esc]
(a) raw control byte 0x1b at offset 132 in the stderr of [accepted-esc]
(a) raw control byte 0x1b at offset 234 in the stdout of [list]
(b) 6 `operator-action:` stderr lines for 4 driven actions — a dynamic string forged a §20.3 line:
  [dispatcher] operator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT'
  [list-unknown-arg] operator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT'
  [failed] operator-action: action=resolve run_id= unit=AK-901 operator="operator-cor3-fail" outcome=failed tickets= code=action_failed — "escalation ticket t-cor3-norecord is not a valid record: No such file or directory (os error 2)"
  [refused] operator-action: action=resolve run_id=01JDY0C0RDD5ESCSTEST0RVN01 unit=AK-904 operator="op\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT" outcome=refused tickets=esc-6a98fecc-0 code=verification_not_pass — the ticket stays open
  [accepted-esc] operator-action: action=resolve run_id=01JDY0C0RDD5ESCSTEST0RVN01 unit=AK-901 operator="operator-cor3-esc" outcome=accepted tickets=esc-ticket
  [accepted-operator] operator-action: action=resolve run_id=01JDY0C0RDD5ESCSTEST0RVN01 unit=AK-901 operator="op\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT" outcome=accepted tickets=esc-6a98fecc-1

(c) the hostile subcommand is not in its escaped form in [dispatcher]: "gauntlet escalations: unknown subcommand 'bogus\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT'\n"
(c) the hostile list argument is not in its escaped form in [list-unknown-arg]: "gauntlet escalations list: unknown argument 'x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT'\n"
(c) the failed §20.3 line lost its plain dynamic field in [failed]: "operator-action: action=resolve run_id= unit=AK-901 operator=\"operator-cor3-fail\" outcome=failed tickets= code=action_failed — \"escalation ticket t-cor3-norecord is not a valid record: No such file or directory (os error 2)\"\ngauntlet escalations resolve: escalation ticket t-cor3-norecord is not a valid record: No such file or directory (os error 2)\n"
(c) the hostile ticket id is not escaped in the accepted §20.3 line in [accepted-esc]: "operator-action: action=resolve run_id=01JDY0C0RDD5ESCSTEST0RVN01 unit=AK-901 operator=\"operator-cor3-esc\" outcome=accepted tickets=\u{1b}[31mesc-ticket\n"
(c) the hostile ticket id is not escaped in the CLI acceptance line in [accepted-esc]: "escalations: ticket=\u{1b}[31mesc-ticket action=resolve operator=\"operator-cor3-esc\" — settled unit AK-901 of run 01JDY0C0RDD5ESCSTEST0RVN01; resolved tickets: \u{1b}[31mesc-ticket\n"
(c) the queue listing is not fully escaped in [list]: "escalations: ticket=x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT state=QUARANTINED reason=record-invalid — the file is not a valid ticket record; actions on it are refused\nescalations: ticket=\u{1b}[31mesc-ticket run=01JDY0C0RDD5ESCSTEST0RVN01 state=resolved action=resolve operator=operator-cor3-esc summary=\"cor3 hostile esc seed\"\nescalations: ticket=esc-6a98fecc-0 run=01JDY0C0RDD5ESCSTEST0RVN01 state=open summary=\"stalled past the quiet window (surface world)\"\nescalations: ticket=esc-6a98fecc-1 run=01JDY0C0RDD5ESCSTEST0RVN01 state=resolved action=resolve operator=op\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT summary=\"cor3 re-entry seed\"\n"
error: test run failed
```

The RED run maps 1:1 onto the falsifier's findings: (a) = R3-F-1 (raw ESC
through the accepted §20.3 arm and the CLI acceptance line) + R3-F-3
locus 3 (raw ESC in the listing); (b) = R3-F-2 (forged lines from the
dispatcher and list-unknown-argument paths); (c) = all of the above plus
R3-F-3 loci 1–2 (the quarantined stem forgery and the operator-field
forgery in the listing).

### GREEN — post-fix (`.agents/reports/task21/cor3-green-run.log`)

```
PASS [   0.028s] (1/1) gauntlet-cli::operator_actions_no_bypass no_hostile_record_or_argument_can_escape_the_family_s_output_discipline
Summary [   0.028s] 1 test run: 1 passed, 12 skipped
```

The test also verifies the legs' behavior UNDER the discipline (exit
codes 2/2/1/1/0/0/0, the typed refusal vocabulary, the refused leg
leaving the seeded ticket open, the queue-read failure surfacing) and
that the hostile operator identity genuinely traveled the SANCTIONED
resolve into the durable ticket record (record-level assert) — the
invariant held at every sink that record surfaced from.

## 4. Gates (green on the final tree; post-last-edit; one compiler at a
time; each call wrapped `flock .agents/reports/task21/compile.lock -c
'<cmd> && sccache --stop-server'`)

| gate | command | result | log |
|---|---|---|---|
| 1 | `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass` | **13/13 PASS** (12 prior + the invariant test) | `cor3-gate1.log` |
| 2 | `cargo nextest run -p gauntlet-cli --test cli_surface --test escalations_cli` | **7/7 PASS** | `cor3-gate2.log` |
| 3 | `cargo nextest run -p gauntlet-adapter-surface-json` | **11/11 PASS** | `cor3-gate3.log` |
| 4 | `cargo build -p gauntlet-cli --no-default-features` | **Finished** (exit 0) | `cor3-gate4.log` |
| 5 | `just lint && just boundaries && just ruler` | clippy `-D warnings` clean + `fmt --check` clean; arch-tests **228/228**; ruler **PASS** ("answer key and manifest digests match the frozen lock") | `cor3-gate5.log` |

`just test` NOT run — reserved for the orchestrator's closure battery,
per the brief.

## 5. Recorded boundary — the non-log consumers

Per COR-2's recorded decision (re-affirmed by the commission): the CLI's
human failure detail line (`escalations_cmd.rs:329`) and the JSON 500
body (`ActionFailure.message`) carry the store/queue error detail
VERBATIM. The invariant accommodates them by leg design: the FAIL leg's
queue-read failure detail is control-byte-free (a plain-ASCII id), so
(a) holds over the full output while the boundary stays untouched. A
control-byte-carrying ticket id WOULD surface verbatim in that human
line — that is the recorded non-log posture, not an open defect of this
slice; the §20.3 log line for the same event carries the same detail
Debug-escaped.

## 6. Deviations

1. **Test-expectation correction mid-slice (one edit to my own new
   test):** the FAIL leg's (c) check originally required
   `tickets=t-cor3-norecord` in the §20.3 FAILED line; the run proves the
   field is honestly EMPTY — the queue read failed before any durable
   read named the ticket, and COR-1's shape is "the log never invents a
   run or a ticket" (visible in the RED run's own (b) listing). The check
   now requires `outcome=failed` + `tickets=` + `unit=AK-901` + the
   ticket id via the read-failure's detail — the (c) intent (dynamic
   fields present and honest) preserved, one field strengthened. No
   source behavior implied; no frozen test touched.
2. **(b) counts STDERR only, per the brief's wording** ("stderr lines
   starting with `operator-action:`"); `escalations list` forgeries ride
   STDOUT (the listing prints to stdout) and are caught by (a) + (c)
   instead. The three sub-invariants jointly cover every sink.
3. **The ESC record is a hand-planted file in the queue's own on-disk
   schema** — `escalate()` cannot mint it BY CONSTRUCTION (the
   `esc-<hex>-<hex>` mint is the defense), and the quarantined stem is by
   definition outside the queue's APIs. "Through the queue's own APIs
   plus argv" holds literally for the operator-identity vector (sanctioned
   resolve argv → durable record via the queue's own `resolve`) and the
   second ticket (`escalate()`).
4. **Wrapper exit codes 2 on gates 1/3:** the `sccache --stop-server`
   tail inside the flock'd command could not connect (server already
   stopped by a prior gate's stop) — the known-harmless pattern recorded
   in the falsifier round-3 provenance. The gate commands themselves
   passed (Summary lines in §4). The RED run used `;` (not `&&`) before
   the stop so the expected-failing exit could not skip the stop; the
   stop always executed inside the lock.
5. **No commit; staging untouched:** COR-1 remains staged, COR-2 + this
   slice unstaged, exactly as found.

## 7. Provenance

- Compile discipline: every cargo/just call ran as
  `flock .agents/reports/task21/compile.lock -c '… && sccache
  --stop-server'` (stop inside the lock; `;` on the expected-failing RED
  run); default parallelism throughout; no compiler parallelism
  overrides.
- Evidence files: `cor3-red-run.log`, `cor3-green-run.log`,
  `cor3-gate1.log` … `cor3-gate5.log` under `.agents/reports/task21/`.
- Files touched: `escalations_cmd.rs` (+78/−16 region),
  `operator_action.rs` (+39/−16 region),
  `tests/operator_actions_no_bypass.rs` (+517, the COR-3 block). Nothing
  else.

---

## r4 ADDENDUM — the boundary tooth (tests-only; closes falsifier r4's
## single minor)

Finding accepted: the invariant accommodated the recorded COR-2 boundary
(the human CLI failure detail line, `escalations_cmd.rs:329`, prints
queue-error Display verbatim — embedding grammar-valid hostile ticket ids
RAW) by AVOIDANCE — its drive set never took a hostile-id record whose
READ fails, so no shipped tooth observed the boundary, and a future
widening (a newline-capable string entering an error Display) would have
been invisible to both invariant clauses.

Fix, in `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs` ONLY
(no source change; nothing else touched, no commit):

1. **The drive set grew by three hostile-id READ-failure shapes** (the
   r4 repro's exact postures): `resolve --ticket $'\u{1b}[31mmissing'`
   (no record — `RecordInvalid` embeds the requested ESC id), a
   cross-wired record (file `\u{1b}[31mwire-stem.json` decoding cleanly
   but claiming a DIFFERENT hostile id `\u{1b}[31mwire-claimed`, so
   `TicketIdentityMismatch` fires and the detail embeds BOTH ESC ids),
   and a corrupt record under an ESC stem (`\u{1b}[31mrot-stem.json`
   over garbage). Seeded through the queue's own substrate: the
   cross-wired record in the queue's own on-disk schema (the identity
   mismatch is only reachable with a lying file), the corrupt file by
   direct write (leg-5 precedent).
2. **The boundary is now pinned BY CONSTRUCTION, not avoided.** The
   three boundary captures sit OUTSIDE the blanket invariant's capture
   set and under a dedicated tooth asserting, over each drive's full
   captured stdout+stderr: exactly two stderr lines (the escaped §20.3
   failed line, then the human failure line); raw control bytes confined
   to that ONE human line (line-index scan) and absent from stdout
   entirely; the hostile ids present VERBATIM in the human line (the
   boundary is genuinely observed, never vacuous) and in their escaped,
   greppable form in the §20.3 line beside it; exit code still 1. The
   §20.3 line-count invariant (b) now spans the boundary drives too
   (7 driven actions, 7 lines). Any widening — a newline-capable or
   control-byte-carrying string entering an error Display (line split →
   line count / confinement fails), a forged second line (count +
   confinement fail), a raw byte anywhere else (confinement fails) —
   MUST fail this test.
3. **The carve-out is declared in the invariant test's doc comment:**
   clause (a) now reads "on every driven path EXCEPT the human CLI
   failure line — the recorded COR-2 decision (log sinks escaped, the
   human failure detail verbatim), which the r4 boundary tooth pins by
   construction", followed by a dedicated THE RECORDED CARVE-OUT (r4)
   paragraph. The blanket phrasing stays only with the carve-out stated;
   the wording now matches reality.

### RED-first evidence

The extended drives were run BEFORE the assertions were fixed — added to
the blanket capture set, under the invariant's original phrasing
(`.agents/reports/task21/cor3-r4-red-run.log`, verbatim):

```
FAIL [   0.054s] (1/1) gauntlet-cli::operator_actions_no_bypass no_hostile_record_or_argument_can_escape_the_family_s_output_discipline
THE FAMILY OUTPUT INVARIANT IS BROKEN (… violation(s)):
(a) raw control byte 0x1b at offset 275 in the stderr of [boundary-missing]
(a) raw control byte 0x1b at offset 398 in the stderr of [boundary-crosswired]
(a) raw control byte 0x1b at offset 469 in the stderr of [boundary-crosswired]
(a) raw control byte 0x1b at offset 271 in the stderr of [boundary-corrupt]
error: test run failed
```

Exactly the finding: the boundary drives trip the blanket invariant on
the human lines' raw bytes (two in the cross-wired capture — requested +
claimed ids), while the §20.3 count held (7 == 7) — proving the drives
genuinely observe the boundary and that the blanket phrasing could not
survive them unchanged.

### GREEN + gates (final tree, post-last-edit incl. the fmt fix)

```
PASS — invariant test incl. the r4 boundary tooth
      (.agents/reports/task21/cor3-r4-green-run.log: 1 passed, 12 skipped)
```

| gate | command | result | log |
|---|---|---|---|
| r4-1 | `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass` | **13/13 PASS** | `cor3-r4-gate1.log` |
| r4-2 | `cargo nextest run -p gauntlet-cli --test cli_surface --test escalations_cli` | **7/7 PASS** | `cor3-r4-gate2.log` |
| r4-3 | `just lint` (clippy `-D warnings` + `fmt --check`) | **PASS** | `cor3-r4-gate3.log` |

Gates r4-1/r4-2 were re-run AFTER the final `cargo fmt` whitespace fix
(whitespace-only in the test file) so every gate is green on the exact
final tree. The r4-2 wrapper exit 2 is again only the `sccache
--stop-server` connect tail (known-harmless; Summary lines cited).
`cargo fmt` touched ONLY the test file (verified: no other file's git
status changed).

Addendum deviation: none beyond the noted `sccache` tail. No source
file, no frozen artifact, no other test changed.
