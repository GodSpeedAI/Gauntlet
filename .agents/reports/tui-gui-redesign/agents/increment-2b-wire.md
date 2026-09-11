# Increment 2b — wire-shape honesty between the workbench and the real Rust surface

Date: 2026-09-11. Branch: `task26-dcritic-apparatus-repair`. Not committed (per slice
contract). Owner files: `workbench/src/operation-gateway.ts`,
`workbench/src/affordance.ts`, `workbench/src/testing/canonical-surface-stub.ts`,
new `workbench/src/wire-shape.test.ts`.

Rust wire anchors (verified in-source this slice, not taken from the recon on faith):

- `crates/adapters/gauntlet-adapter-surface-json/src/lib.rs:205-213` — `error_json`:
  EVERY error body the surface serves is `{"error":{"code","message"}}` (nested).
- `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:494-541` — the action
  body contract: empty/whitespace body → note `""`; a non-empty body must parse as a
  JSON object whose `note` member (when present) is a string; otherwise a typed 400
  `body_unparsable` (non-object: "the action body must be a JSON object";
  non-string note: "the action body's 'note' must be a JSON string"; unparsable:
  the parser's own error text). Auth checks (401/403) precede body parsing.
- `crates/adapters/gauntlet-adapter-surface-json/src/lib.rs:94-150` — `snapshot_json`:
  `/state` serves `run`, `escalated` (BARE ticket ids, no unit binding), `in_flight`,
  `units {id,status,tier}`, `budget` (rounds/parallel/depth/wall_seconds/tokens/
  cost_usd_micro, `budget_json` :169-199), `sandbox_denials`, `open_residuals`,
  `run_derivation`. It serves NO `open_escalations`, `budget_remaining`,
  `governance_denied_units`, or `authority_revoked_units`.

## 1. Deliverables and exact behavior changes

### W1 — nested error body (operation-gateway.ts, `HttpOperatorActionRoute.perform`)

- Error parsing now reads the nested `body.error.{code,message}` shape; a flat
  top-level `{code,message}` remains tolerated (resilience for non-Rust stand-ins);
  **nested wins** when both are present. Previously every real 4xx/409/5xx collapsed
  to code `surface_error` with an empty message, breaking the gateway's "refusal
  code preserved" claim (operation-gateway.ts:31-34). Preserved refusal/failure codes
  and messages now flow through exactly into the `refused`/`failed` GatewayOutcome.
- Class doc updated to the real wire (`accepted → 200 {action, run_id, unit_id,
  committed_now, tickets_resolved}` unchanged; all errors nested).

### W2 — JSON-object note body (operation-gateway.ts, `perform`)

- The note rides as `JSON.stringify({note})` with `Content-Type: application/json`;
  an EMPTY note sends NO body at all (`body: undefined`, no Content-Type). The
  unconditional `Content-Type: text/plain` + raw-note body is gone. Free-text
  composer drafts are no longer refused 400 `body_unparsable` by the real parser.

### W3 — honest escalation surfacing (operation-gateway.ts `observe` + affordance.ts docs)

- `observe()` maps ONLY a served `open_escalations` array. The fabrication fallback
  that minted `{ticket_id, unit_id: null}` tickets from the real `/state`'s bare
  `escalated` id list is DELETED: an absent (or empty) field now yields
  `open_escalations: []` — honest absence. Because a null-unit ticket matches every
  unit in the resolver, the old fabrication made `unit.resolve` resolve
  `requires_attention` on every unit whenever any ticket existed; against the real
  surface it now resolves `unavailable` with reason `no_open_ticket` (fail-closed).
- Served entries — including genuinely unbound `unit_id: null` ones — still map
  verbatim; the resolver's null-matching rule (affordance.ts:265-267, 295-300
  pre-change) is KEPT for tickets the observation explicitly carried. affordance.ts
  changes are documentation-only: the honesty rule is now stated on
  `AffordanceEscalationObservation.unit_id` and at both null-matching sites (gate and
  attention-item), each citing the Rust wire. No resolver behavior changed (verified:
  the served-bound and served-unbound contrast legs pass identically before and
  after).
- The real `/state` budget fact is served under `budget` with a different member set;
  it is deliberately NOT conflated into `budget_remaining` — absent stays absent
  (comment cites `budget_json`). `governance_denied_units`/`authority_revoked_units`
  mappings unchanged (absent → `[]`, honest absence of a read).

### W4 — the stub speaks the real wire (testing/canonical-surface-stub.ts)

- Every stub error body is now the nested `{"error":{"code","message"}}` shape via a
  new `errorResponse()` helper (404/400/401/403/409/500 alike).
- The action endpoint parses the body exactly as http.rs:494-541 does (auth before
  body parse, matching Rust's order): empty/whitespace → note `""`; non-object JSON →
  400 "the action body must be a JSON object"; non-string `note` → 400 "the action
  body's 'note' must be a JSON string"; unparsable → 400 with the parser's error
  text. The 400s fire BEFORE admission (`admission_attempts` unmoved), like Rust.
- New `lastActionRequest` wire record (`{body_text, content_type}`) so suites can
  assert WHAT a transport sent, not just what admission decided.
- The /state gate-field superset (`open_escalations`/`budget_remaining`/
  `governance_denied_units`/`authority_revoked_units`) is KEPT and now explicitly
  documented as the read-visibility KNOB CARRIER the deny-bypass/cross-surface
  matrices require (removing it would gut their read-exposure teeth);
  wire-shape.test.ts pins the gateway against the real `/state` shape directly.

## 2. Test-first evidence (wire-shape.test.ts)

Written RED first; run against the pre-change code confirmed 6 fail / 1 pass with
each failure at the exact defect signature:

- W1 409 nested → received `surface_error` (expected `authority_not_permitted`);
- W1 503 nested → received `surface_error` (expected `surface_overloaded`);
- W1 precedence → flat won (expected nested to win);
- W4 parser → raw free-text body answered 200 (expected typed 400);
- W2 → `lastActionRequest` absent (no wire recording; JSON body unpinned);
- W3 → observation carried a fabricated `{ticket_id, unit_id: null}` ticket from the
  bare `escalated` id (expected `[]`), and the resolver then resolved
  `requires_attention` (expected `unavailable`/`no_open_ticket`).

The one pre-pass test (W3 contrast: served tickets still surface, served unbound
tickets keep null-matching) is a preserved-behavior pin, documented as such.

Post-fix: **7 pass / 0 fail** (48 expect() calls). The real-wire servers in the test
are hand-rolled FROM the Rust sources (field names, nesting, status codes), so the
gateway is pinned against the surface as built, independent of the stub's dialect.
One test-leg correction during the RED round, disclosed: the raw free-text leg
asserts only the `body_unparsable` CODE (it is unparsable JSON — parse-error text is
parser-specific), while the exact "must be a JSON object" message is pinned by the
valid-JSON-non-object leg; both arms match Rust's http.rs branches.

## 3. Existing test updates

**None required.** deny-bypass.test.ts and cross-surface-equivalence.test.ts pass
unchanged (16 and 14 tests, 0 fail): the stub's nested wire is now parsed by the
fixed gateway, the gateway's JSON note body is accepted by the fixed stub parser,
and C8c's hand-rolled FLAT 503 server still works through the W1 flat-tolerance leg
(teeth intact — the tolerance is pinned by wire-shape.test.ts, which also pins that
nested WINS). The cross-surface static pins on the stub file
(`return this.decide(` ×2, `settlements_committed += 1` ×1, `admission_attempts
+= 1` ×2) and on the gateway (`/api/v1/units/` ×1) all held.

## 4. Verification

- `bun test src/wire-shape.test.ts src/deny-bypass.test.ts
  src/cross-surface-equivalence.test.ts src/affordance-resolver.test.ts`
  (the task's `src/affordance.test.ts` does not exist; the affordance suite is
  `src/affordance-resolver.test.ts`): **48 pass / 0 fail** (7 + 16 + 14 + 11).
- All other suites consuming the shared stub or HttpOperatorActionRoute (the stub
  change touches them all): evidence-promotion, kernel-affordances,
  operation-isomorphism, plugin-lifecycle, prime-context-envelope,
  prime-delegation-real, prime-fresh-target, prime-recursion, reconnect-races,
  semantic-evidence-composition, transport-equivalence, canonical-boundary:
  **133 pass / 7 gated skips / 0 fail**; prime-delegation-real-battery: 4 gated
  skips (GAUNTLET_C2_REAL_PRIME=1, expected).
- `bun run check` (tsc): zero errors in every file this slice touched. The command
  exits 1 solely on `src/shell-trace.test.ts` — an UNTRACKED in-flight file of a
  CONCURRENT agent's slice (references a `handleKey` export and `ShellState.trace`
  that do not exist yet). Proven not mine: all 19 tsc errors are in that one file.
- `bun x biome check --write` then verify: exit 0 on all four owned files (remaining
  diagnostics are info-level useLiteralKeys suggestions, predominantly on
  pre-existing lines; no errors/warnings).
- `graft build`: PASS (23 parsed, 436 replayed from cache).
- RED round preserved: see section 2; the pre-change failure signatures were
  captured in the run outputs before any fix landed.

## 5. Out-of-scope needs (recorded, NOT fixed here)

1. **client.ts `mutateUnit` carries the same W1/W2 twin defects** (flat
   `body.code`/`body.message` read at client.ts:~441-446; raw `text/plain` note
   body at :~413-437). Recon §8 already flagged it as a duplicate of the gateway
   route ("the prod host uses only the latter"). With the stub now speaking the
   real wire, this twin defect is EXPOSED as 3 failing assertions in suites outside
   this slice's ownership (proven by controlled experiment: HEAD versions of the
   three owned files → those suites pass 45/45; my versions → the failures below):
   - `src/security-boundaries.test.ts` "transport.client.rpc" (~:957):
     `client.mutateUnit` flat-parses the stub's now-nested 401 → `code` undefined
     (expected `actor_authentication_required` / `invalid_session_credentials`).
   - `src/security-boundaries.test.ts` "boundary inventory coverage contract":
     cascade of the above (`recordAttack("transport.client.rpc")` unreached).
   - `src/secret-leakage.test.ts` (~:542): same root cause — the token-echo tooth
     itself still passes; only the code assertion fails.
   Minimal follow-up fix (needs an owner for client.ts + those two test files):
   parse the nested error shape in `mutateUnit` and send the note as
   `{"note":...}`/empty body — or better, delete the duplication and route through
   `HttpOperatorActionRoute.perform`. I deliberately did NOT weaken the stub back
   to the flat wire to hide this: surfacing the twin defect is W4 doing its job.
2. **Concurrent agent work in the same tree** (not mine, untouched):
   modified `command-registry.ts`, `config.ts`, `geometry.ts`, `main.ts`,
   `shell.ts`, `operator-journeys-pty.test.ts`, `palette-grouping-confirm.test.ts`;
   untracked `theme-capability.test.ts`, `shell-trace.test.ts` (the last causing
   the tsc exit 1 — mid-flight, references APIs that do not exist yet).
3. `.agents/CURRENT_STATUS.yml` still lists W1-W4 under `tui_gui_redesign.residuals`
   as open; this slice closes them (orchestrator-owned edit, not mine to make).

## 6. Next move

Slice complete and uncommitted. The orchestrator should review the diff, decide the
client.ts follow-up (out-of-scope item 1), and settle.
