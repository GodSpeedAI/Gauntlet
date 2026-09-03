# Task 21 falsifier — round 1 (blind executing falsifier)

Status: FINAL — round complete. Findings: 0 blocking / 0 major / 3 minor
(F-1, F-4, F-5) / 4 informational (F-2, F-3, F-6, F-7).
Tree under test: HEAD a71eba9 (task21 source committed; `git diff HEAD` on
crates/ and Cargo manifests empty). Binary: `target/debug/gauntlet` with
`--features surfaces` (and the default-features build for the topology leg).

Attack plan (breadth-first, per commission):

1. No-bypass chain: forged/duplicate/stale/foreign-run PASS records in the
   durable store, through CLI `escalations resolve|retry` AND
   `POST /api/v1/units/{id}/resolve|retry`; re-mint integrity; `?run=` steering.
2. Ticket/queue integrity: replay, two-open-ticket ambiguity, mid-flow
   corruption, refused-action write window, accepted-action + queue-failure
   honesty.
3. HTTP surface: bind policy, caps, methods, identity validation, body
   validation, header/log injection, surface-failure isolation.
4. Exactly-once across both surfaces.
5. Feature topology: default build, help snapshots, adapter deps.
6. Read-surface honesty: run_derivation, sandbox_denials, stale/invented data.
7. Over-rejection: legitimate positive path via API and CLI.

Findings will be appended below as they land, most severe first.

---

## Findings

### F-1 — MINOR — "the settle committed but the queue write failed" fires on the re-entry path where NOTHING committed

Locus: `crates/gauntlet-cli/src/operator_action.rs:245-261` (re-entry branch)
vs `:286-293` (the message's home), reachable through
`escalations resolve --ticket <resolved-ticket>` on an already-settled unit.

Repro (world `w1b`, seeded below; server on 127.0.0.1:43613):
1. Positive leg settled AK-903 through the API (200, committed_now=true,
   exactly one new settlement row `vrf-QVBP5QM7QPNWPTXSA5WGVZB383`,
   ticket `esc-falsifier-forge1` resolved with the operator identity).
2. Replayed the CLI action on the SAME ticket:

```
gauntlet escalations resolve --ticket esc-falsifier-forge1 --unit AK-903 --operator falsifier-cli
→ operator-action: ... outcome=failed code=action_failed — the settle committed
  but the queue write failed: escalation ticket esc-falsifier-forge1 is already
  resolved — a queue record resolves exactly once    (exit=1)
```

The queue's own typed refusal is correct, and the durable state stays
exactly-once (still one AK-903 row; nothing written by the replay). But the
diagnostic says "the settle committed" when the re-entry branch
(`operator_action.rs:241-261`) performs NO settle at all — it finds the
existing row and goes straight to the queue write. An operator reading this
cannot tell whether their invocation just settled the unit (it did not; the
unit was settled by the earlier call). Misleading failure locus (§21.2
"typed errors carrying the failure locus"); the durable facts are honest,
so MINOR, not MAJOR.

Evidence of the honest durable state after the replay: settlements for
AK-903 still exactly 1; ticket file unchanged (still resolved, first
operator identity intact).

### F-2 — INFORMATIONAL — forged durable PASS records cannot settle failing evidence; only fully staged genuinely-passing evidence settles (the honest boundary)

World w2 (real `e2e-lying` mock loop: run HKP4T5JQ300PGEY6EZJ8XSD9H3,
bounded non-settlement, ZERO durable verification records, one production
binding the LYING report `{"tests": 1, "failures": 1}` at
`evidence/marker-check-report.json`, digest `sha256:08c3dff5…`).

Attack ladder, all through the real API (`POST /api/v1/units/AK-901/resolve`,
`X-Gauntlet-Operator: falsifier-op`), server on 127.0.0.1:43447:

1. FORGED a durable PASS verification record for AK-901 into w2's SQLite
   (w1's genuine AK-901 payload, run_id rewritten). With the workspace
   locator file absent: 409 `verification_not_pass` — "the settling claim's
   own outcome is INCONCLUSIVE". The mint ran (the gate passed on the
   forged record) and returned an INCONCLUSIVE claim; the settle refused
   at its own entry check. No settlement row.
2. Planted the lying report bytes at the canonical workspace root
   (`<state>/workspaces/evidence/marker-check-report.json`): still 409 —
   INCONCLUSIVE. The production is at iteration 1; the run's durable
   checkpoint stands at iteration 3; the mint anchors at the durable
   iteration and a prior-iteration production is STALE (recency defense).
3. FORGED the production row to iteration 3 (fresh): 409
   `verification_not_pass` — "the settling claim's own outcome is FAIL".
   Binding passed (bytes digest-match the production both in the store and
   at the workspace root) and the §14.2 test-report rung JUDGED the lying
   report FAIL live. No settlement row.
4. Planted the genuinely PASSING report bytes (digest `sha256:98cbdd08…`)
   over the lying ones, keeping the production's digest at `08c3dff5…`:
   409 `verification_refused` — "the workspace file … hashes to
   sha256:98cbdd08… but the durable production record names sha256:08c3dff5…
   — the bytes the instrument would examine are not the pinned bytes
   (refused before dispatch, whoever wrote the file)". Post-freeze artifact
   poisoning cannot ride the locator.
5. FULL forge (store object + pin file + production row + workspace bytes
   all name the genuinely passing bytes; forged gate record still naming
   them): 409 `verification_record_disagrees` — my own forged PASS record
   collided with the re-mint's identity (unit, evidence, outcome) and the
   integrity check refused a second document.
6. Detached the forged substrate's evidence ref (removing the identity
   collision): 200 `committed_now:true`, settlement row
   `vrf-KNTW15SC6S0322BVQW4X4B5QZ4`, ticket resolved.

Interpretation: every shortcut short of REAL passing bytes was refused —
stale recency, live instrument judgment (FAIL over the lying report),
store/locator digest binding, and record-identity disagreement each caught
a different forgery stage. Settlement only commits over bytes an instrument
just judged PASS at a pinned, digest-bound locator. Step 6 shows the
boundary honestly: an attacker with raw write access to the durable store,
the evidence store, the pins, AND the canonical workspace can stage
genuinely passing evidence and the chain will honestly verify it. That is
outside the surface threat model (the durable store is the runtime's sole
write domain, §16.4; builder writes are a containment question, §16.2) —
recorded as the mechanism's honest limit, not a bypass.

### F-3 — INFORMATIONAL — the COR-F1 (producer_uncorroborated) world cannot be ridden by the operator path

World w3 (real `e2e-forged-report` mock loop: forged PASSING reports over
marker-less source; the loop's COR-F1 settlement gate refuses with
`producer_uncorroborated`, bounded non-settlement). Inspected the durable
store after the run: ZERO verification records — the loop records the
verification document only inside the §16.3 commit
(`ensure_verification_recorded`), so a COR-F1 refusal leaves no durable
PASS record for the operator gate to ride, and `gauntlet verify` writes
nothing (verify_cmd.rs:1-7). The operator path therefore cannot override
COR-F1 without ALSO forging store rows (covered by F-2's boundary).
Defense in depth holds: the gate is exactly the barrier that makes the
controller-side corroboration refusal stick.

### F-4 — MINOR — raw ANSI escape bytes land in the surface's access log (log injection via request target)

Locus: `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:212-215`
(`eprintln!("surface: {} {} -> {}", request.method, request.target, …)` —
raw `Display` of the attacker-controlled request target).

Repro (server on 127.0.0.1:43613):

```
GET /api/v1/\x1b[31mred-alert HTTP/1.1   →  404 (correct)
server stderr, raw bytes:  surface: GET /api/v1/^[[31mred-alert -> 404
                           (raw 0x1b confirmed in the stderr file)
```

The §20.2 JSON error body escapes it correctly (`\u001b`), and the §20.3
action-log line escapes via `{:?}` (verified: identity `op\x01erator` logs
as `operator="op\u{1}erator"`), but the access line interpolates the raw
target: a local process can rewrite/forge the appearance of the operator's
`serve` diagnostics with terminal escape sequences (no newline injection —
`\r\n` splits the head — but cursor/color/erase attacks and fake
status-line suffixes land). Narrow defect: escape or debug-format the
target like the action log does.

### F-5 — MINOR — `GET /api/v1/units/{id}?run=<non-durable-run>` serves 200 with an invented-looking `status:"open"`; the run view 404s the same run

Locus: `crates/gauntlet-cli/src/durable_surface.rs:381-427` (`unit_view`
never checks run durability; `units(None)` defaults missing-checkpoint
units to `UnitStatus::Open`, `:104-133`) vs `:299-309` (`run_view` refuses
non-durable runs with `NotFound`).

Repro (w1b server):

```
GET /api/v1/runs/01ARZ3NDEKTSV4RRFFQ69G5FAV
  → 404 {"error":{"code":"not_found","message":"run … is not durable in this state store"}}
GET /api/v1/units/AK-901?run=01ARZ3NDEKTSV4RRFFQ69G5FAV
  → 200 {"unit_id":"AK-901","unit":{"id":"AK-901","status":"open","tier":"contained"},
         "verifications":[],"settled":false}
```

The unit-scoped read answers a valid-grammar ULID that never existed with
`status:"open", settled:false` — a stale or foreign run anchor is
indistinguishable from a durable run where the unit is genuinely open,
and it contradicts the run-scoped read's own 404 for the same anchor. D6
requires unit reads to be run-anchored and absence honest; the empty
records/settlements are honest, the `open` status is a default, not a
durable read. Action steering is NOT possible this way (the action path's
`settle_operator_action_gate` refuses non-durable runs: 409
`run_not_durable`, verified). Fix shape: 404 (or a `run_durable:false`
disclosure) when `resume`/`load_run` names the run absent.

### F-6 — INFORMATIONAL — coarse denial heuristic and silent unit-view degradation, both consistent with the established read posture

- `sandbox_denials` is a substring match (`record.summary.contains("sandbox")`,
  `durable_surface.rs:236-241`) over ALL queue records (open AND resolved).
  A forged ticket whose summary says "sandbox policy bwrap failed validation…"
  surfaced verbatim as a denial in `/api/v1/state`. In production summaries
  are runtime-generated (the governance gate's own vocabulary), so this is
  the disclosed D6 derivation over durable facts, but the heuristic is
  coarse (case-sensitive; "no sandbox needed" would also surface) and
  reflects whatever durable ticket text holds.
- With the frozen answer key unreadable (probe: chmod 000 on the SCRATCH
  world's key, restored byte-identically afterwards, sha256 proven),
  `/api/v1/state` serves 200 with `"units": []` — the completion view
  silently vanishes. The CLI `status` in the same state also exits 0
  without units-by-tier, so the surface matches the established read
  posture ("the same reads status uses"); noted as honest-absence
  ambiguity (empty units = "no units" OR "key unreadable"), not a new
  divergence introduced by task 21.

### Confirmed-working mechanisms (executed evidence, not praise)

No-bypass chain (worlds w1/w1b/w2/w3, all real-binary):
- Positive leg end-to-end through the REAL API against my own seeded world
  (real mock-loop world w1, one settlement row removed by forgery to create
  the unset-but-passing unit, locator artifact planted at the canonical
  workspace root, one forged open ticket): `POST /api/v1/units/AK-903/resolve`
  → 200 `committed_now:true`, chain-minted settlement row exactly once,
  ticket resolved with the recorded identity, §20.3 log line present.
- Idempotent re-POST (`?run=` anchored): 200 `committed_now:false`,
  `tickets_resolved:[]`, still exactly one settlement row.
- `verification_record_disagrees` fire-honesty: with the gate's durable PASS
  record naming the SAME evidence identity the re-mint produces (a forged
  half-applied §16.3 state), the action refuses 409 BEFORE any write —
  settlements unchanged, ticket open (the operator path never mints a second
  document over an already-recorded identity).
- A probe bug of mine (Content-Length 26 > actual 23 body bytes) made the
  server block on the body read and hit its socket deadline: connection
  closed with NO response, server survived, logged
  `surface: reading the request failed: Resource temporarily unavailable`
  and kept serving — per-connection failure did not take the surface down.

Ticket/queue integrity (w1b):
- Two open tickets on one run, API action without `?run=`: 409
  `ambiguous_ticket` naming the CLI `--ticket` route; ticket file hashes
  byte-unchanged through the refusal.
- Corrupt ticket via CLI: typed `not a valid record` failure, exit 1, file
  untouched (garbage still garbage).
- Queue-write-failure window (accepted/re-entry action, escalations dir
  chmod 555): 500 `action_failed` naming the durable truth
  ("the settle committed but the queue write failed: … Permission denied");
  settlement rows unchanged in the re-entry case, ticket byte-unchanged;
  after restoring the dir the re-POST resolved the ticket (honest
  recovery). The settled-unit+open-ticket state is exactly what the code
  claims it is, and it is recoverable — but see F-1: in the re-entry case
  "the settle committed" is wrong (nothing committed on THAT call).
- Refusal-path write audit across every refusal above (gate refusals,
  ambiguous, corrupt, foreign run, identity): zero queue writes, zero
  settlement-row changes, zero event writes observed (file hashes + sqlite
  diffs before/after).
- Read/write race on a ticket file (swapper flipping open/lying/garbage at
  1 ms while the CLI resolved): outcomes were only the typed
  invalid-record refusal or a clean resolve; the cross-wire record (ticket
  field naming another ticket) can never win because `resolve` re-reads
  and binds the record's own ticket field to the requested id
  (queue.rs:287-293).

HTTP surface (w1b, raw sockets):
- 405+`Allow` on GET-resolve / DELETE-state / PUT-refresh / POST-units(2
  segs); 404 on unknown routes, `/`, `/api/v2/*`, extra segments,
  `/api/v1/../x`; `//state` tolerated (200; harmless); `units//resolve`
  405s as GET (cosmetic mislabel for an empty unit segment).
- Caps: 17 KiB head → 400 `request_too_large`; declared body > 1 MiB →
  400 `body_too_large` (refused before reading the body); unparsable
  Content-Length (`-5`) → 400 `content_length_unparsable`; missing version
  / colon-less header / non-UTF-8 head → typed 400s.
- Identity: missing and whitespace-only headers → 400
  `operator_identity_required`; a `\x01`-carrying identity passes
  `OperatorIdentity::new` (blank-only refusal, per its documented
  contract) but lands escaped in every sink (`{:?}` in the §20.3 line,
  JSON escaping in bodies/records) — no injection path found.
- Bodies: non-object JSON → 400 `body_unparsable`; malformed JSON → 400
  with the parser's locus; a NON-STRING `note` (e.g. `{"note":123}`) is
  silently dropped (empty note) and the action proceeds — lax but the
  note is non-consequential bookkeeping; unknown fields ignored; a 900 KB
  note on a refused action is neither logged nor persisted (refusal).
- Empty `?run=` → 400 `run_unparsable`; `?run=<valid ULID never durable>`
  → 409 `run_not_durable` on actions (no steering onto foreign runs);
  query keys match case-insensitively; no percent-decoding (encoded
  traversal stays inert).
- Corruption/failure isolation: with the SQLite store corrupted (header
  overwritten in the scratch copy), reads serve a quiet all-empty snapshot
  (see F-6's ambiguity note) while the action fails LOUDLY (500
  `action_failed`, "a durable store could not be opened"); the server
  stayed up through all of it (§21.3). `POST /refresh` over the healthy
  store changed ZERO state files (byte+mtime snapshot diff).

Feature topology:
- Default build (`cargo build -p gauntlet-cli`): `gauntlet serve` →
  "unknown command 'serve'", exit 2; `--help` BYTE-IDENTICAL to the frozen
  `crates/gauntlet-cli/tests/cli_help.txt` (diff-proven).
- Surfaces build: `--help` BYTE-IDENTICAL to `cli_help_surfaces.txt`, and
  it lists `serve`.
- `crates/adapters/gauntlet-adapter-surface-json/Cargo.toml`: dependencies
  are `gauntlet-domain` + `gauntlet-ports` ONLY (no app dependency); the
  CLI gates it behind `surfaces = ["dep:gauntlet-adapter-surface-json"]`
  with `default = []`; `git diff HEAD` on `Cargo.toml`/`Cargo.lock` is
  empty — no new external dependencies anywhere.

Retry semantics (D5): `POST /api/v1/units/AK-901/retry?run=…` on the
settled unit → 200 `committed_now:false`, and the queue record carries
`action:"retry"` with the operator identity — the token distinguishes the
action exactly as D5 specifies; it settled nothing new.

Derivation steering (read side only): a forged open ticket naming a
FABRICATED greater-ULID run flipped the snapshot's derived run — units all
read "open", budget zeros (the real run's view was masked), but the
derivation string DISCLOSED both run ids, and an action under the steered
derivation refused 409 `run_not_durable`. View-masking DoS needs durable-
store write access (already outside the surface threat model); settlement
steering is refused.

Bind policy (each a real `gauntlet serve` process): unset / empty /
whitespace / garbage / `0.0.0.0:43113` / `255.255.255.255:80` / `::` /
`:99999` / unknown-host all exit 1 with typed
`BindUnparsable`/`BindRefused` messages and open NO socket; `[::1]:0`
binds. `serve extra-arg` exits 2. One code-level residue I could NOT
trigger locally (no DNS control): `SurfaceHttp::bind` checks ANY resolved
address for loopback but BINDS THE FIRST (http.rs:121-136) — a name whose
first address is non-loopback with a loopback second would open a
non-loopback listener despite the any-loopback check. Noted for the
record; loopback-only intent holds for every directly-specifiable address
I could test.

### F-7 — INFORMATIONAL — the serve loop's own words promise "continuing" while it breaks

Locus: `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:178-181`
(`Err(error) => { eprintln!("… the accept loop observed {error}; continuing"); break; }`)
against the module doc (`http.rs:5-7`: "a per-connection failure never takes
the surface down: it is reported and the loop continues"). If `accept()`
ever returns an error (EMFILE, ECONNABORTED-class), the surface ends —
printing a line that says "continuing". I could not trigger an accept
error empirically (connection-RST attempts were absorbed by the kernel
backlog; only the listener dying stops the loop), so this stays a
code-level contradiction between the loop's self-description and its
behavior, not an executed failure. §21.3's run-isolation held in every
executed case.

## Provenance

- All worlds are under `/tmp/falsifier-t21/` (w1 happy settled loop;
  w1b w1-derived with my forgeries; w2 lying loop; w3 forged-report loop);
  the repo was never modified except this report file. The scratch
  answer-key chmod probe restored byte-identically (sha256 verified).
- Shipped instrument run as the floor: `cargo nextest run -p gauntlet-cli
  --features surfaces --test operator_actions_no_bypass` → 5/5 PASS.
- Binary: `cargo build -p gauntlet-cli --features surfaces` (and the
  default-features build for the topology leg), each under the task
  compile lock.

---

VERDICT: NOT_CONFIRM (0 blocking / 0 major / 3 minor / 4 informational)
