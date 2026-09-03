# Task 21 falsifier — round 2 (blind executing falsifier)

Status: FINAL — round complete. Findings: 0 blocking / 0 major / 3 minor
(R2-F-1, R2-F-2, R2-F-3) / 2 informational (R2-I-1, R2-I-2). Tree under
test: HEAD a71eba9 + the UNCOMMITTED COR-1 correction slice
(`git diff HEAD -- crates/`). Binary: `target/debug/gauntlet` built with
`--features surfaces` under the task compile lock (default build for the
topology leg, surfaces restored after). Worlds: my own, under
`/tmp/falsifier-r2/` (fresh; round-1 scratch not reused).

Attack plan:

Part A (per correction, from `.agents/reports/task21/cor1-brief.md` — the
mechanism under test, not the builder's claims):

- C1 refused/failed §20.3 log parity: forgery via `?run=` steering,
  explicit-ticket lying, ambiguous-ticket case, missing fields, note leakage.
- C2 both queue-write-failure branches (re-entry on settled unit / fresh
  accept), real binary, chmod-locked escalations dir; wording confusion.
- C3 accept classification: real EMFILE pressure attack against a live
  `serve` (ulimit), fatal-class wording, "continuing" never printed on stop.
- C4 access-log escaping: the METHOD field (parser validates no token
  grammar), header values in any log, misleading escaped forms.
- C5 read parity: other default-serving reads, 404 message internals leak.
- C6 bind-all-loopback: multi-address hosts (localhost → 127.0.0.1 + ::1),
  IPv6 forms, v4-mapped forms, hostname resolution, first-offender naming.
- C7 shared matcher: drift-pin discrimination, case sensitivity,
  token-carrying false positives in the operator's denial view.
- C8 note validation: nested/array/null/duplicate keys, 1 MiB note on the
  ACCEPTED path (persisted? logged?), non-UTF8 bodies.
- C9 acceptance disclosure: CLI accept line, §20.3 line, API JSON.

Part B: regression sweep of round 1's confirmed mechanisms (no-bypass chain,
idempotent replay, ambiguous/corrupt refusals, refusal-path zero-write audit,
queue-write-failure window + recovery, HTTP caps/methods/identity, §21.3
isolation, feature topology, retry token D5, derivation steering, bind
policy).

Part C: over-rejection leg — one genuinely-earned PASS settlement through the
API AND one through the CLI, §20.3 lines with the full field set.

Findings appended below as they land, most severe first.

---

## Findings

### R2-F-1 — MINOR — attacker-shaped ticket-id strings reach log sinks RAW (control bytes and forged lines in the CLI's stderr, including the §20.3 audit channel)

The COR-1 C4 fix debug-escapes the access log's request TARGET, but the
ticket-id channel interpolates untrusted strings raw into two log sinks,
and the ticket-id grammar (`TicketId::new`,
`crates/gauntlet-ports/src/human_escalation.rs:24-49`) refuses only
whitespace and path separators — control bytes pass.

Locus 1 (newline → forged log lines, pre-parse):
`crates/gauntlet-cli/src/escalations_cmd.rs:213-215` prints the RAW
pre-validation argument. The §20.3 line never prints (the parse fails
before `execute_operator_action`), but the error line forges arbitrary
following lines:

```
$ gauntlet escalations resolve --ticket $'x\noperator-action: action=resolve \
  run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT' --unit AK-903 --operator op
stderr (cat -A):
gauntlet escalations resolve: x$
operator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT is not a usable ticket id$
```

A second line that reads as a §20.3 accepted record lands in the CLI's
stderr (exit 1). An operator relaying a ticket id from an escalation
message (durable, builder-influenceable text) carries the payload.

Locus 2 (raw 0x1b into the §20.3 failed line itself): a ticket id that
PASSES the grammar (`\x1b[31mred-ticket` — no whitespace, no separator)
flows through `execute_operator_action`; the queue's `RecordInvalid`
Display embeds the id; the failed arm
(`crates/gauntlet-cli/src/operator_action.rs:132-137`) prints `detail`
raw:

```
operator-action: action=resolve run_id= unit=AK-903 operator="falsifier-cli" \
  outcome=failed tickets= code=action_failed — escalation ticket ^[[31mred-ticket \
  is not a valid record: No such file or directory (os error 2)
```

(raw `1b 5b` confirmed in the stderr bytes). Round 1 verified the
OPERATOR identity lands escaped via `{:?}`; the ticket channel is a
second untrusted input that does not. Same severity class as round-1
F-4 (MINOR): CLI-side sink, needs the operator to relay the hostile
string; no durable state affected. Fix direction: debug-escape the
ticket argument at `escalations_cmd.rs:214` and the `detail` interpolation
in the §20.3 failed/refused arms (or forbid control bytes in
`TicketId::new`).

### R2-F-2 — MINOR — the access log escapes the request TARGET but still interpolates the raw METHOD: control bytes from the request line land in the server's stderr

Locus: `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:808-814`
(`access_log_line` = `surface: {method} {target:?} -> {status}`) and the
parser `:689-697`, which applies NO token grammar to the method (any
non-empty non-space bytes before the target are the method). C4's fix
debug-escapes the attacker-controlled target (executed proof: it lands
quoted) but leaves the method as raw `Display`.

Executed repro against the live wB server (`127.0.0.1:44101`):

```
raw request line: \x1b[31mFORGED\x07 /api/v1/state HTTP/1.1
→ 405 (Allow: GET) — correct refusal
server stderr, raw bytes (cat -A):
surface: ^[[31mFORGED^G "/api/v1/state" -> 405$
```

Raw ESC (0x1b, color/cursor control) and BEL (0x07) bytes from the
request line reach the operator's `serve` diagnostics — the exact
injection class F-4 described and C4 fixed for the target. No newline
injection is possible through the method (the head splits on `\r\n`
first; a bare CR can still ride the method but cannot start a new
line). Same severity as F-4: cosmetic-to-misleading diagnostics
forgery from any local process that can open a loopback connection;
no durable-state effect. Fix direction: escape or token-validate the
method in `access_log_line`/`parse_head` (the 405/404 semantics don't
depend on exotic methods).

### Correction record (Part A, per item; executed against the real binary)

- **C1 — HELD, with R2-F-1 as the residue.** Refused line now carries the
  full D7 field set (executed: foreign-run steering →
  `run_id=01ARZ…FAV … outcome=refused tickets= code=run_not_durable`;
  ambiguous case → run named, `tickets=` empty — the action addressed
  none, honest). The run named under `?run=` steering is the requested
  anchor the action addressed and the refusal names the refusal —
  attribution, not forgery. The note never reached any log in any
  executed path (400 refusals drop it; accepted path writes it to the
  ticket record only — grep over stdout+stderr clean).
- **C2 — HELD.** Both branches driven through the real CLI on world wB:
  fresh accept with the escalations dir chmod 555 → settle committed
  (settlement rows 0→1) and the message names THAT commit ("the settle
  committed but the queue write failed: … Permission denied"); recovery
  replay resolved the still-open ticket (exit 0, still exactly one
  settlement row); second replay → re-entry branch → "the settlement is
  already durable; the queue write failed: … already resolved" with NO
  "the settle committed" text anywhere (grep = 0). Each line names the
  true durable state of its own call; exactly-once held through all
  three phases.
- **C5 — HELD.** `GET /api/v1/units/AK-903?run=01ARZ…FAV` → 404
  `not_found` with byte-identical wording to the run view ("run 01ARZ…
  is not durable in this state store"); the message embeds only the
  RunId-validated anchor — no store internals. Residue noted: a
  valid-grammar unit NOT in the frozen key on a durable run
  (`/api/v1/units/AK-904`) serves 200 `{"unit":null,…,"settled":false}`
  — `unit:null` honestly discloses the key's absence, and `settled:false`
  is a true durable read (no settlement row), so this is the disclosed
  posture, not F-5's invented `open`; recorded, no action.
- **C8 — HELD.** `{"note":{…}}`, `[…]`, `null`, `true` → typed 400
  `body_unparsable`; duplicate keys → 400 (the parser refuses duplicate
  keys outright — "ambiguous documents are not admitted"); unknown
  fields ignored (documented posture); non-UTF8 body → 400
  `body_unreadable`. A 100 KB note on the ACCEPTED path (CLI, re-entry
  accept): persisted into the ticket record (100,296-byte record,
  JSON-escaped, still machine-readable via `escalations list`), NEVER
  logged (stdout+stderr clean). Bounded by the 1 MiB body cap / OS arg
  cap; the record is the note's designed destination.
- **C9 — HELD.** Every surface that prints the pairing names the settled
  unit and the resolved ticket(s) as separate verbatim facts: the CLI
  accept line ("ticket=t-amb-2 … settled unit AK-901 of run 5BZH…;
  resolved tickets: t-amb-2" — executed with a deliberately MISMATCHED
  ticket/unit pairing; both ids verbatim, pairing disclosed not
  implied), the §20.3 accepted line (unit= and tickets= separate), and
  the API accepted JSON (`run_id`, `unit_id`, `committed_now`,
  `tickets_resolved` as distinct fields). The binding CHECK remains
  deferred per the brief (the mismatched action still succeeds) — the
  recorded decision, not a regression.
- **C3 — BROKE (R2-F-3).** Executed finding below.
- **C6 — HELD.** Executed matrix of real `serve` processes: 127.0.0.2,
  127.1, localhost, this host's own name (→127.0.1.1), [::1] all bind
  (every resolved address loopback); 0.0.0.0, [::], 169.254.1.1,
  255.255.255.255 refused with the offender NAMED in the typed message;
  `[::ffff:127.0.0.1]` (a v4-mapped address that would actually serve
  loopback) is conservatively REFUSED and named; `:99999` and unknown
  hosts get typed unparsable refusals; no refused bind opened a socket.
  Limitation: no name on this host resolves to a MIXED loopback +
  non-loopback set (needs DNS control), so the all-addresses rule over
  a mixed set is code-reviewed (`find` over the whole resolved set,
  `http.rs:131-141`), not executed — the same honest limit round 1
  recorded.
- **C7 — HELD, one informational false positive.** Executed on world wC
  with five seeded tickets: the gate's OWN AK-050 runner-isolation
  summary (no "sandbox" token) NOW APPEARS in `/api/v1/state`
  `sandbox_denials` (the RES-04 recall gap is fixed); the
  authority-class Irreversible summary does NOT; the case-variant
  "Sandbox policy …" (capital S) does NOT (matcher is case-sensitive —
  correct for "the gate's own spellings"); the old matcher's false
  positive "no sandbox needed …" is GONE. New-matcher false positive
  confirmed: a ticket summary "dispatch deferred without a backend
  decision by the operator" (carries the token "without a backend", no
  real denial) surfaces in the denial view verbatim — inherent to
  substring matching over durable ticket text (same posture round-1 F-6
  recorded; the surface narrowed). Drift-pin note: the gate-side test
  pins the FIVE containment + THREE authority variants by enumeration;
  a future containment variant added without extending the token list
  would not be caught by the pin (hand-maintained arrays, not
  compiler-enforced) — informational.

### R2-F-3 — MINOR — the transient-continue branch has no backoff: a PERSISTENT EMFILE spins the accept loop at 100% CPU flooding stderr (~3.1M "continuing" lines / 3 s)

Locus: `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:199-207`
(transient → `eprintln!("…; continuing"); continue;` — no sleep, no
rate limit). The classification itself is exactly what the brief
commissioned, and my executed attack PROVES the continue-not-break
behavior the round-1 F-7 gap lacked. But when the fd exhaustion is
persistent rather than "momentary", the continue path degenerates:

Executed (real `serve`, world wC, `RLIMIT_NOFILE=5`, one held fd beyond
stdio + listener — so `accept()` returns EMFILE forever, the pending
probe connection sitting unserved in the backlog):

```
server log: "surface: the accept loop observed Too many open files
             (os error 24); continuing"  × 3,093,911 lines in 3.0 s
process CPU: 3.00 cpu-seconds in the 3.0 s window (100% of one core)
log growth: 111,448,580 bytes (≈37 MB/s) in that window
```

The surface never serves another request while the ceiling holds, yet
announces "continuing" ~10⁶ times/second at full CPU — unbounded log
growth (disk-filling) and a busy loop, for as long as the condition
persists. §21.3's letter held (no crash; a typed message at startup
when fds are exhausted even BEFORE the loop: ceiling 4 fails closed
with "reading GAUNTLET.md: Too many open files (os error 24)"). Not
attacker-drivable through the surface itself (the serial
accept→handle→close loop holds at most one connection fd, so a
connection flood cannot exhaust the process's fds); it needs an
environmental fd ceiling or a leak elsewhere in the process. MINOR:
the degradation is confined to the surface process, but a bounded-loop
backoff (e.g. a short sleep on the transient branch) is the standard
fix and the current code has none.

---

## Part B — regression sweep (round 1's confirmed mechanisms, re-attacked)

Every leg below was executed against the real binary on my own worlds
(COR-1 tree); "held" means the executed result matched round 1's
confirmed behavior.

- **No-bypass chain, negative leg — HELD.** wD: AK-901's durable
  verification records removed, one open ticket; CLI resolve → exit 1,
  refused `verification_not_pass` with the FULL §20.3 field set
  (run_id/unit/operator/outcome/tickets/code), ticket still open, still
  exactly one AK-901 settlement row. Zero-write audit: ticket-file
  hashes + full sqlite `.dump` hash UNCHANGED across the refusal.
- **No-bypass chain, forged-substrate refusals — HELD.** wB before world
  surgery: the fresh action over a settlement-row-deleted unit whose OLD
  verification record still held the (unit, evidence, PASS) identity →
  409-class refusal `verification_record_disagrees` (the re-mint's
  second document over the same identity refused); the recency defense
  also re-fired for a stale production (AK-902 production at iteration
  2 vs durable checkpoint 3 → the mint's claim came back INCONCLUSIVE →
  refused `verification_not_pass`). Both are the round-1 F-2 defenses
  intact under COR-1.
- **Idempotent replay — HELD.** API re-POST over the settled unit →
  200 `committed_now:false`, `tickets_resolved:[]`, still exactly one
  settlement row (AK-902 leg, below).
- **Ambiguous / corrupt ticket refusals — HELD.** Two open tickets on
  one run, API action without `?run=` → 409 `ambiguous_ticket` naming
  the CLI route (wD); garbage ticket file via CLI → typed
  "not a valid record: json: unexpected byte 0x67 at offset 0", exit 1,
  file byte-unchanged (hash-proven).
- **Refusal-path zero-write — HELD** (see audit above; also the C2
  fresh-failure window: the settlement committed by the service while
  the TICKET file stayed byte-unchanged — the queue write refused, no
  partial write).
- **Queue-write-failure window + recovery — HELD** (C2 record above).
- **HTTP caps/methods/identity — HELD.** 405+`Allow` on GET-resolve,
  DELETE-state, PUT-refresh, POST-units; 404 on unknown routes, `/`,
  `/api/v2`, `/api/v1/../x`; 16 KiB-exact and 17 KiB heads → typed 400
  `request_too_large` (a raw-socket race in my first probe produced a
  client-side ECONNRESET while the server HAD answered 400 and closed —
  probe artifact, not a server defect; re-proven with a half-close);
  declared body > 1 MiB → 400 `body_too_large` before reading;
  `Content-Length: -5`, missing version, colon-less header, non-UTF-8
  head → typed 400s; missing/blank identity → 400
  `operator_identity_required`; empty `?run=` → 400 `run_unparsable`.
- **§21.3 surface-failure isolation — HELD, with an updated startup
  posture.** Store corrupted AFTER server start (valid header, trashed
  body page): reads stay quiet 200s, the action fails LOUDLY (500
  `action_failed`, "a durable store could not be opened: … database
  disk image is malformed"), and the server keeps serving (refresh and
  state 200 after). Store dead at STARTUP (header stomped, or malformed
  image): `serve` now fails CLOSED before opening any socket with a
  typed message ("a durable store could not be opened: … file is not a
  database" / "… database disk image is malformed") — a typed, honest
  startup refusal rather than round-1's lazy-open posture; recorded as
  a posture note, not a regression (nothing runs against a dead store).
  Mid-request client abort (Content-Length lie): connection closed with
  no response, typed server-side log line, server survived.
- **Feature topology — HELD** (see topology section below).
- **Retry token (D5) — HELD.** CLI retry on the settled unit with an
  open ticket → exit 0, re-entry accept (`committed_now` semantics),
  the ticket record carries `action: "retry"` + the operator identity,
  and NO new settlement row (still 1).
- **Derivation steering — HELD (read-side masking unchanged, action
  steering refused).** A forged open ticket naming fabricated run
  `7ZZZ…ZZZ` (sorts after the true run) flips the snapshot's derived
  run and masks all units to `open` — with the derivation DISCLOSED in
  `run_derivation` (the known round-1 informational posture; requires
  durable-store write access). The action under the steered derivation
  refused 409 `run_not_durable` — no settlement steering.
- **Bind policy — HELD** (C6 record above: every directly-specifiable
  non-loopback address refuses).

---

## Part C — over-rejection leg (the legitimate path, executed)

Both positive legs settled GENUINELY EARNED PASS through the real
chain: the §14.2 test-report rung re-judged the pinned locator's bytes
live, the governed re-mint produced the claim, and the ONE §16.3 commit
wrote the row.

- **CLI leg (wB, AK-903):** the C2 fresh phase IS a genuine CLI
  settlement — the settle committed (settlement rows 0→1) with the
  queue write failing by construction; after recovery the ticket
  resolved through the accepted re-entry (exit 0) with the full §20.3
  accepted line and the C9 CLI acceptance line naming
  ticket/unit/run/resolved-tickets separately.
- **API leg (wB, AK-902):** surgery produced the unset-but-earning
  unit (settlement row removed, stale production freshened to the
  durable iteration, honest locator bytes planted at the workspace
  root, old verification identity moved aside);
  `POST /api/v1/units/AK-902/resolve?run=5BZH…` → 200
  `{"action":"resolve","run_id":"5BZH…","unit_id":"AK-902",
  "committed_now":true,"tickets_resolved":[]}`, chain-minted row
  `vrf-H3BZYWYJFFY3QX6FVSYRZMHW52` exactly once, §20.3 accepted line
  with the full field set, and the empty ticket list disclosed rather
  than silent. Idempotent re-POST → `committed_now:false`, still one
  row.

No over-rejection observed: every legitimately-earning input settled,
and every illegitimate input from Part A/B was refused.

---

## Feature topology (executed)

- Surfaces build: `--help` BYTE-IDENTICAL to the frozen
  `crates/gauntlet-cli/tests/cli_help_surfaces.txt` (diff-proven); the
  restored surfaces binary serves `GET /api/v1/state` (sanity probe).
- Default build: `--help` BYTE-IDENTICAL to the frozen
  `crates/gauntlet-cli/tests/cli_help.txt`; `gauntlet serve` →
  "unknown command 'serve'", exit 2, no socket.
- `git diff HEAD` on `.wayfinder/`, `.gauntlet/`, `.agents/specs/`:
  EMPTY — no frozen artifact touched. No new external dependencies (the
  COR-1 diff touches no manifest dependency edges; only the C10 comment
  rewording in `crates/gauntlet-cli/Cargo.toml`).

## C10 — doc/comment truth — HELD (verified in the current tree)

- `lib.rs:36-43` + `:84-90`: states the actual field-order rule (run
  preamble, then §20.2 sections in priority order, disclosure last) and
  drops the false "escalated FIRST = RuntimeSnapshot field order"
  parenthetical, recording the charter tension instead.
- `escalations_cmd.rs:4-9`: names the real path
  (`execute_operator_action` / `settle_operator_action_gate` /
  `settle_operator_action_earned`).
- `Cargo.toml:12-15`: "the only feature edge THIS task adds".

## Additional numbered informational findings

### R2-I-1 — INFORMATIONAL — a valid unit id NOT in the frozen key serves `{"unit":null,…,"settled":false}` (200) on a durable run

`GET /api/v1/units/AK-904` (grammar-valid, absent from the fixture
key) on the durable derived run → 200 with `unit:null` (honest
disclosure that the key does not know the unit) and `settled:false`
(a true durable read: no settlement row). Unlike round-1 F-5, nothing
is INVENTED — the absence is disclosed — recorded for completeness
within the C5 sweep.

### R2-I-2 — INFORMATIONAL — the shared containment matcher's false-positive surface and the drift-pin's enumeration shape

Executed false positive: a durable ticket summary "dispatch deferred
without a backend decision by the operator" surfaces in the operator's
`sandbox_denials` (token "without a backend", no real denial) — the
view reflects durable ticket text verbatim, consistent with the
disclosed D6 posture; the COR-1 matcher still NARROWED the surface
(the old `"sandbox"` substring matched "no sandbox needed" — that
probe no longer false-positives). Drift-pin note: the gate-side pin
test enumerates the 5 containment + 3 authority variants by hand; a
future containment `GateRefusal` variant added without extending
`CONTAINMENT_SUMMARY_TOKENS` would not be caught by the pin (a
compiler-enforced shape — e.g. a per-variant summary→token association
— would close it).

---

## Per-correction one-line record

| Item | Record |
|---|---|
| C1 log parity | HELD (full field set on refused/failed arms, executed; residue R2-F-1 on the ticket-id channel) |
| C2 message split | HELD (both branches executed on the real binary; wording names each branch's durable truth) |
| C3 accept classification | BROKE as executed — the transient-continue path hot-spins without backoff on persistent EMFILE (R2-F-3); classification itself works as commissioned |
| C4 access-log escaping | HELD for the target; BROKE for the method field (R2-F-2) |
| C5 read parity | HELD (404 byte-parity; R2-I-1 informational residue) |
| C6 bind-all-loopback | HELD (executed matrix; mixed-set case not locally executable — code-reviewed) |
| C7 shared matcher | HELD (isolation spelling now surfaces; R2-I-2 informational residue) |
| C8 note validation | HELD (all body shapes; accepted-path note persisted to the record only, never logged) |
| C9 acceptance disclosure | HELD (CLI line, §20.3 line, API JSON all name the facts separately, executed with a mismatched pairing) |
| C10 doc truth | HELD (verified verbatim in the current tree) |

## Provenance

- Worlds: `/tmp/falsifier-r2/{wA,wB,wC,wD,wE}` — wA seeded by a REAL
  `e2e-happy` mock-loop run (run 5BZH6T9N84J16F105AJ6E65WR4, units
  AK-901/902/903 settled); wB–wE derived copies with my own forgeries
  (deleted/mutated rows, planted locator bytes digest-matched to the
  durable productions, forged ticket files in the queue's own schema).
  All probe files live under /tmp; the repository was never modified
  except this report. Frozen artifacts diff-clean vs HEAD.
- Binary: `target/debug/gauntlet` built with `--features surfaces`
  under `flock .agents/reports/task21/compile.lock` (sccache stopped
  inside the same flock'd command; the sccache client timed out at
  shutdown twice — harmless); the default-features build for the
  topology leg, then the surfaces build restored (serve existence and
  a live state probe re-verified).
- Shipped-instrument floor (not evidence, context):
  `cargo nextest run -p gauntlet-cli --features surfaces --test
  operator_actions_no_bypass` → 8/8 PASS on the COR-1 tree.
- Note: mid-session, `git status` showed the COR-1 crates/ changes
  staged in the index (unstaged at round start). I staged nothing;
  recording the observation without acting on it.

---

VERDICT: CONFIRM (0 blocking / 0 major / 3 minor / 2 informational)

