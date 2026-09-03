# Task 21 falsifier — round 3 (blind executing falsifier, final focused)

Status: IN PROGRESS. Tree under test: HEAD a71eba9 + staged COR-1 + unstaged
COR-2 (`git diff HEAD` + `git diff --cached` both visible to me; builder
reports and adjudications are NOT). Binary: `target/debug/gauntlet` built
with `--features surfaces` under the task compile lock. Worlds: mine, under
`/tmp/falsifier-r3/` (fresh; round-2 scratch not reused).

Scope (focused, per commission):

1. Re-attack COR-2's fixes at their executed loci:
   - D1: pre-parse `--ticket` refusal, `--unit` refusal, same-function
     unknown-argument arm (all now debug-escaped); §20.3 FAILED arm's
     `detail` now debug-escaped. Attack the log/non-log boundary: does any
     escaped-sink change leak into or corrupt the non-log paths (human CLI
     failure line, JSON 500 body)? Does any raw path still feed a
     LOG-shaped sink?
   - D2: access log escapes METHOD + TARGET; verify 405/404 parser
     semantics byte-identical for exotic methods.
   - D3: 50 ms named-constant backoff on the transient accept branch.
     Attack: starvation/misclassification of real transient classes; fatal
     path promptness (backoff must not delay shutdown or the fatal break);
     rate bound real under persistent EMFILE (CPU bounded, log growth
     bounded).
2. Adjudicate recorded-not-fixed loci (execute the forged-line attack):
   - the escalations dispatcher's unknown-argument line (outside the fixed
     function);
   - `escalations list` output (raw argument interpolation).
   Classify BLOCKING/MAJOR/MINOR/INFO by real operator impact.
3. Mandatory regression leg (one pass, on a world I seed):
   (a) resolve without durable PASS refuses with the SAME typed refusal
   through BOTH CLI and API, ticket open, ZERO settlement rows;
   (b) one genuinely earned PASS settles through the API exactly once, the
   ticket resolves, the §20.3 accepted line carries the full field set
   (escaped fields still greppable/human-readable);
   (c) idempotent replay commits nothing.

Findings appended below as they land.

---

## Findings

### R3-F-1 — MINOR — the §20.3 ACCEPTED arm and the CLI acceptance line still interpolate the ticket id RAW: a raw ESC rides the audit line (the D1 fix covered the FAILED arm's `detail` only)

Executed on world wD (real binary): a queue record is planted whose ticket
id is `\x1b[31mesc-ticket` (PASSES `TicketId::new` — no whitespace, no
separator), anchored to the seeded run; `escalations resolve --ticket
$'\x1b[31mesc-ticket' --unit AK-903 --operator falsifier-cli` hits the
settled unit → re-entry accept, exit 0:

```
stderr (the §20.3 audit line), raw bytes:  tickets=\x1b[31mesc-ticket
stdout (the CLI acceptance line), raw:     ticket=\x1b[31mesc-ticket …
                                           resolved tickets: \x1b[31mesc-ticket
```

Byte counts: t5.err contains one raw 0x1b, t5.out two. Loci:
`crates/gauntlet-cli/src/operator_action.rs:117-124` (accepted arm,
`tickets={}` raw join) and `crates/gauntlet-cli/src/escalations_cmd.rs:270-274`
(`ticket={ticket}` raw). COR-2's D1 escaped the FAILED arm's `detail` and
the pre-parse refusals; the ACCEPTED arm interpolates the same untrusted
channel raw. Bounded exactly like R2-F-1 locus 2: `TicketId::new` refuses
whitespace, so NO newline/forged-line is possible — raw control bytes
(color/cursor escape) only, in the §20.3 audit line. Access bar is
HIGHER than R2-F-1: the id must exist as a queue record; the queue's own
`escalate()` mints `esc-<hex>-<hex>` only, so this needs a hand-planted
queue file (durable-state write access). Same sink class round 2 scored
MINOR; scored MINOR for the sink, with the higher access bar recorded.

### R3-F-2 — MINOR — the escalations dispatcher's unknown-subcommand line and the `escalations list` unknown-argument line still interpolate the raw argument: the forged-line attack executes against both (recorded-not-fixed locus 1)

Both outside the fixed `operator_action` function, both executed on the
real binary (wD):

```
$ gauntlet escalations $'bogus\noperator-action: action=resolve run_id=FORGEDRUN \
  outcome=accepted tickets=FAKE-TKT'      (exit 2)
stderr, cat -A:
gauntlet escalations: unknown subcommand 'bogus$
operator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT'$
```

and identically for `escalations list $'x\noperator-action: …'`
(`gauntlet escalations list: unknown argument 'x$` + the forged line).
Loci: `crates/gauntlet-cli/src/escalations_cmd.rs:34` and `:59`.
The forged second line is §20.3-SHAPED and lands in stderr; the closing
quote rides its tail (a tell-tale, but the line reads as an accepted
action record). Access bar identical to the FIXED R2-F-1 locus 1 (the
pre-parse `--ticket` line): the string rides the command line — a
workflow that relays untrusted text into argv carries the payload; the
same-function unknown-argument arm at `:202` got the escape, these two
sibling dispatchers did not. MINOR (same class and same access as the
locus COR-2 fixed).

### R3-F-3 — MINOR — `escalations list` output forges lines THREE ways: quarantined file stems, the record's free-form operator field (argv-reachable through the sanctioned resolve), and raw ticket ids

Executed on wD (real binary, exit 0 list):

1. Quarantined stem: a corrupt file named
   `x\noperator-action: …tickets=FAKE-TKT.json` in the queue dir →
   `escalations: ticket=x` + forged line `operator-action: action=resolve
   run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT state=QUARANTINED
   reason=record-invalid — …` (stem printed raw; the stem is the file
   name, never grammar-checked — `queue.rs` `list()` takes any `.json`
   stem).
2. Operator field: `OperatorIdentity::new` accepts ANY non-blank string —
   INCLUDING newlines (`crates/gauntlet-app/src/settle/*.rs:188-200`).
   The SANCTIONED CLI path itself lands the payload:
   `escalations resolve --ticket t-r3-opwrite --unit AK-902 --operator
   $'op\noperator-action: …'` exits 0, writes the identity into the
   durable ticket record, and the NEXT plain `escalations list` prints
   `… operator=op` + the forged §20.3-shaped line — argv-reachable, NO
   filesystem planting needed (the t8b/t9 probes used planting only to
   isolate the locus; t9 used the sanctioned path end to end).
3. Ticket ids print raw in list lines (`ticket={record.ticket}`), so a
   hand-planted ESC id colorizes the inbox view raw (no newline
   possible — grammar).

Loci: `crates/gauntlet-cli/src/escalations_cmd.rs:98-105` (quarantined
stem raw), `:114-133` (`record.ticket` raw, `operator.as_str()` raw at
the resolved-state build `:118-128`; only `summary` is escaped
`{:?}`). Sinks: the operator's §20.2 inbox stdout — the queue view
operators act on. Severity MINOR: forged LINES are possible here (the
operator field channel), the forged lines are §20.3-shaped, and one
vector needs only argv relay through the sanctioned resolve; bounded by
impact (the inbox is a derived view — an attacker with queue-dir write
access can already forge the substrate itself, and no durable state is
affected by the display forgery).

---

## Per-fix record

### D1 — ticket-id strings never reach log sinks raw — HELD (with R3-F-1 as the accepted-arm residue)

Executed against the real binary (`--features surfaces`, COR-2 tree), wD:

- Pre-parse `--ticket` refusal (escalations_cmd.rs:222): newline payload
  `x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted
  tickets=FAKE-TKT` → ONE stderr line, the argument in its escaped form
  `"x\noperator-action: …"`, exit 1; zero `^operator-action:` lines
  (byte-verified). The forged-line attack that defined R2-F-1 locus 1 no
  longer executes.
- `--unit` refusal (escalations_cmd.rs:229): `\x1b[31mAK-901\n…` payload →
  ONE line, escaped (`"\u{1b}[31mAK-901\n…"`), zero raw ESC, zero forged
  lines, exit 1.
- Same-function unknown-argument arm (escalations_cmd.rs:202): newline
  payload → ONE line, escaped, exit 2, zero forged lines.
- §20.3 FAILED arm (operator_action.rs:139-144): grammar-valid ESC ticket
  `\x1b[31mred-ticket` → queue read failure flows into the failed line
  with the detail DEBUG-ESCAPED (`"escalation ticket \u{1b}[31mred-ticket
  is not a valid record: …"`); the §20.3 line contains NO raw 0x1b
  (byte-verified) and DOES contain the escaped form. The raw-ESC
  R2-F-1-locus-2 attack no longer executes against the FAILED arm.
- Boundary holds in BOTH directions (the commission's leak question):
  - Log→non-log leak: NONE. The human CLI failure line
    (escalations_cmd.rs:286) prints the same detail VERBATIM — raw ESC
    byte-confirmed present there and only there — and exits 1 exactly as
    before. The escape did not corrupt the non-log path.
  - Raw→log leak: the ACCEPTED arm still feeds the ticket id raw into
    the §20.3 line (R3-F-1) — the one remaining raw→log path found.
  - JSON 500 body verbatim: executed via the wB server (queue dir
    read-only; resolve of an already-settled unit) — the 500 body
    carries the detail verbatim (`action_failed`), and the server-side
    §20.3 line for the SAME event carries it escaped. Boundary holds.

### D2 — access log escapes method and target; parser semantics untouched — HELD

Executed against the live wA server (127.0.0.1:44701), raw sockets:

- Hostile request line `\x1b[31mFORGED\x07 /api/v1/state HTTP/1.1` → 405
  + `Allow: GET` (correct refusal), and the access log lands fully
  escaped: `surface: "\u{1b}[31mFORGED\u{7}" "/api/v1/state" -> 405` —
  zero raw 0x1b/0x07 in the server stderr (cat -A: no `^[`, no `^G`).
  The R2-F-2 attack no longer executes.
- 405/404 parity matrix (parser untouched — verified identical to the
  round-2-recorded semantics): POST on the GET-resolve route → 405
  Allow GET; DELETE /refresh → 405 Allow POST; PUT /units/AK-901 → 405
  Allow GET; exotic `BREW` on a known route → 405 Allow GET; exotic
  method on an unknown route → 404; `GE\rT` (bare CR inside the method)
  → 405; missing version / missing target / empty method → typed 400s;
  `/` and `/api/v1/../x` → 404. Plain requests log as
  `surface: "GET" "/api/v1/state" -> 200` — quoted like the target,
  still greppable. No refused-or-escaped probe altered routing
  behavior; the escape is log-sink-only as commissioned.

### D3 — bounded backoff on the transient accept branch — HELD

Executed against a real `serve` (wA) under `prlimit --nofile=5:1024`
with a backlog probe holding the listener busy so every accept returns
EMFILE (the R2-F-3 repro conditions):

- Rate bound is REAL. 5 s window under persistent EMFILE: 100
  "continuing" lines (19.9/s — exactly the 50 ms constant), 8,000 log
  bytes (1.6 KB/s), CPU 1 jiffies = 0.2% of one core. Round 2's
  executed baseline: ~3,093,911 lines / 3 s, ~37 MB/s, 100% of a core —
  five orders of magnitude on every axis.
- Classification unchanged and honest. `accept_error_is_transient` is
  byte-identical in the COR-2 diff (the sleep is the only behavioral
  addition). Executed: the real EMFILE class continued for the whole
  window (never misreported as fatal, never broke); after the soft
  limit was raised under the running process (`prlimit --pid
  --nofile=1024:1024`), the spin stopped at the same bounded count
  (47→47) and the server accepted and SERVED a real probe
  (`HTTP/1.1 200 OK`, after serially draining the earlier silent
  backlog socket through its typed 10 s read timeout — per-connection
  isolation intact). No starvation: the transient class keeps the loop
  alive and recovers the moment the pressure lifts; the backoff does
  not wedge or delay the loop's recovery.
- Fatal path and shutdown prompt. The break arm carries NO sleep (the
  backoff exists only on the transient branch; the shipped
  injected-sequence test consumes exactly through the EBADF fatal —
  instrument context). Executed shutdown: SIGINT during the EMFILE
  spin → process exit latency 0.01 s. The backoff delays nothing on
  the way down.
- Instrument context (not evidence): the shipped adapter suite runs
  11/11 on the COR-2 tree, including the new
  `consecutive_transient_accept_errors_cannot_hot_spin_the_loop`
  timing assertion (0.207 s — well above its 120 ms floor, no flake
  observed).

## Regression leg record (the task's core invariant, one pass)

World seeded by a REAL `e2e-happy` mock-loop run: run
`B0GNRTVQKQSVXC2RTH2WJP9VT1`, units AK-901/902/903 settled (terminal=settled,
exit 0), then surgical copies. All three legs executed against the real
binary; the negative leg through BOTH the CLI and a live `serve`.

**(a) Refusal without durable PASS — HELD (wB, AK-901).** Surgery: AK-901's
settlement row and verification_record row deleted; its production
freshened to the durable iteration 3 and the honest locator bytes planted
at the workspace root — the world was left earning everywhere EXCEPT the
durable PASS record (the D11 sensor shape: a bypassed gate would have had
nothing left to refuse on). One open ticket `t-r3-neg` planted in the
queue's own schema.
- CLI: `escalations resolve --ticket t-r3-neg --unit AK-901 --operator
  falsifier-cli` → exit 1; §20.3 refused line with the FULL field set
  (`action=resolve run_id=B0GN… unit=AK-901 operator="falsifier-cli"
  outcome=refused tickets=t-r3-neg code=verification_not_pass`); human
  refusal "no PASS verification record exists for unit AK-901 of run
  B0GN… — settlement requires one (spec 16.1 condition 1): the run holds
  no verification record for unit AK-901 at all".
- API: `POST /api/v1/units/AK-901/resolve?run=B0GN…` with
  `X-Gauntlet-Operator: falsifier-api` → 409 Conflict
  `{"error":{"code":"verification_not_pass","message":…}}` — the message
  BYTE-IDENTICAL to the CLI's refusal text (programmatic comparison:
  True). The SAME typed refusal through BOTH channels.
- Zero-write audit: state-store file sha256 unchanged across the CLI
  refusal (pre/post hash compare: True); ticket file hash unchanged;
  ticket still open; settlement rows still 2 with AK-901 at ZERO.

**(b) One genuinely earned PASS settles through the API exactly once —
HELD (wC, AK-902).** Surgery: AK-902's settlement row deleted; its old
verification identity moved aside (the record's content-addressed
`evidence_ref` re-pointed to `evd-r3-moved-aside`, keeping a durable PASS
record for the gate that cannot collide with the chain-minted document);
production freshened to iteration 3; the honest locator bytes planted at
the canonical workspace root (`state/workspaces/evidence/check-report.json`
— note: my first planting went to the wrong root and the loop answered
409 `verification_not_pass` with "the settling claim's own outcome is
INCONCLUSIVE" — the honest unearned-read refusal, zero writes, ticket
open; the discrimination worked before the world was correctly seeded).
Then: `POST /api/v1/units/AK-902/resolve?run=B0GN…` → 200
`{"action":"resolve","run_id":"B0GN…","unit_id":"AK-902",
"committed_now":true,"tickets_resolved":["t-r3-pos"]}` —
- the chain-minted settlement row `vrf-XTEJYRRXZ5Q44VS7CPYXNC11T1`
  written EXACTLY ONCE (AK-902 rows = 1; total rows 2→3);
- the ticket `t-r3-pos` resolved with `operator: falsifier-api`,
  `action: resolve` in its durable record;
- the §20.3 ACCEPTED line in the server's stderr carries the full field
  set — `action=resolve run_id=B0GN… unit=AK-902
  operator="falsifier-api" outcome=accepted tickets=t-r3-pos` — the
  escaped-field format (operator Debug-quoted) left the line greppable
  and human-readable; the plain fields stay plain.

**(c) Idempotent replay commits nothing — HELD (wC).** Re-POST over the
settled unit → 200 `{"action":"resolve",…,"committed_now":false,
"tickets_resolved":[]}` — the empty ticket list HONESTLY discloses the
already-resolved ticket; AK-902 still exactly one settlement row; the
replay's own §20.3 line shows `outcome=accepted tickets=` (the action
succeeded; nothing new committed or resolved).

## Recorded-not-fixed adjudication

Executed, classified, recorded as R3-F-2 (the escalations dispatcher's
unknown-subcommand line + the `escalations list` unknown-argument line)
and R3-F-3 (the `escalations list` output: quarantined file stems, the
free-form record operator field, raw ticket ids). Severity verdict:
MINOR, both loci — the forged-line attack EXECUTES against all of them
(with R3-F-3's operator-field vector reachable through the SANCTIONED
resolve path, no fs planting needed), but the class is the one rounds 1–2
calibrated MINOR: CLI-side sinks, argv-relay or queue-dir-write access,
no durable-state effect, no settlement steering. Notably, the
same-function unknown-argument arm COR-2 fixed sits in the middle of
these still-raw siblings (`escalations_cmd.rs:202` fixed; `:34`, `:59`,
`:98-105`, `:114-133` raw) — the sweep stopped at the function boundary.

## Provenance

- Worlds: `/tmp/falsifier-r3/{wA,wB,wC,wD}` — wA seeded by a REAL
  `e2e-happy` mock-loop run through the real binary (run
  `B0GNRTVQKQSVXC2RTH2WJP9VT1`, units AK-901/902/903 settled,
  `GAUNTLET_ID_SEED=falsifier-r3-seed`); wB/wC/wD copies with my own
  surgery (deleted rows, re-pointed evidence_ref, freshened productions,
  locator bytes digest-matched from the content-addressed store, ticket
  files in the queue's own schema). All probe files live under /tmp; the
  repository was never modified except this report.
- Frozen artifacts diff-clean vs HEAD (`.wayfinder/`, `.gauntlet/`,
  `.agents/specs/` — empty status, re-verified at round end).
- Binary: `target/debug/gauntlet` built with `--features surfaces` under
  `flock .agents/reports/task21/compile.lock` (the tree was already
  built by the orchestrator battery — cargo reported Finished with
  nothing to rebuild; no source newer than the binary, surfaces help
  snapshot diff-proven). The `sccache --stop-server` client timed out
  twice inside the flock'd commands — the known-harmless pattern from
  round 2; the locks were held and released correctly.
- Servers: four real `serve` processes (wA D2 probes :44701; wA D3 EMFILE
  legs :44702/:44703/:44704 under `prlimit --nofile`; wB negative leg
  :44705; wC positive leg :44706), all stopped after their legs.
- Blindness: builder/orchestrator reports (`cor*-report/progress`,
  `b1-*`, `IN-FLIGHT.md`, adjudications, `*.log`, `closure.md`) were
  never opened; the round-2 findings document, the COR-2 brief, the
  design/critic/charter documents, the spec, and all code/tests/fixtures
  were the only task21 inputs.
- Shipped-instrument floor (context, not evidence):
  `cargo nextest run -p gauntlet-adapter-surface-json` → 11/11 PASS on
  the COR-2 tree, including the new D3 timing assertion (0.207 s).

---

VERDICT: CONFIRM (0 blocking / 0 major / 3 minor / 0 new informational)
