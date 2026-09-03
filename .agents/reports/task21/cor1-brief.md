# Task 21 — COR-1 correction brief (builder)

You are building the single correction slice for plan Task 21 (operator
surfaces) at /home/sprime01/projects/gauntlet, tree HEAD a71eba9. Both
confirmation rounds returned 0 blocking / 0 major; this slice lands the
adjudicated narrow residuals. Findings + adjudications:
`.agents/reports/task21/critic-design.md` (RES-*),
`.agents/reports/task21/falsifier-round1.md` (F-*),
`.agents/reports/task21/design-adjudication.md`,
`.agents/reports/task21/falsifier-adjudication-r1.md`. Read those four
first. The tree may have moved beyond a71eba9 by trivial docs-only
commits — `git log --oneline -3` and reconcile before editing.

## Contract decisions (binding; deviations get recorded, not smuggled)

- **C1 (RES-01, D7 parity).** The §20.3 operator-action log line on the
  REFUSED and FAILED arms must carry the same field set as the accepted
  arm: action, run_id, unit_id, operator, outcome, ticket(s). Today
  (`crates/gauntlet-cli/src/operator_action.rs:96-117`) the refused arm
  omits run_id + ticket. Carry the data on the decision type or log
  where both are in scope; keep one log shape for all outcomes. The
  note is still never logged.
- **C2 (F-1).** The re-entry branch (`operator_action.rs:245-261`) must
  NOT say "the settle committed but the queue write failed" — nothing
  committed on that call. Message names the durable truth, e.g. "the
  settlement is already durable; the queue write failed: {error}". The
  fresh-accept path (:286-293) keeps its (now accurate) message. Assert
  the distinction in a test.
- **C3 (RES-03 + F-7).** The accept loop
  (`crates/adapters/gauntlet-adapter-surface-json/src/http.rs:169-184`)
  classifies accept errors: continue on transient classes
  (EINTR/ECONNABORTED/ECONNRESET/EMFILE/ENFILE/ENOBUFS/EPIPE-style —
  map via errno kinds available in std::io::ErrorKind), break on
  listener-fatal ones (EBADF/INVAL-class, or NotFound after a deliberate
  close). The "continuing" eprintln must never print when the loop
  breaks; the module doc (`http.rs:5-7`) is updated to describe the
  real classification. Prove with a unit test over an injected
  accept-failure sequence (refactor the accept source behind a small
  trait/closure seam if needed — std-only, no new deps).
- **C4 (F-4).** The access log (`http.rs:212-215`) must not interpolate
  the raw attacker-controlled request target — escape or debug-format
  it (`{:?}`-style, matching how the §20.3 action log escapes identity).
  Test: a target with a raw 0x1b byte produces no raw 0x1b in the
  captured stderr line.
- **C5 (F-5).** `GET /api/v1/units/{id}?run=<non-durable-run>` refuses
  with 404 `not_found` using the run view's own wording
  ("run … is not durable in this state store"), matching
  `durable_surface.rs:299-309`. The derived-run path (no `?run=`) is
  unaffected (its run ids come from open tickets and are durable).
  Test in `tests/http.rs` or the no-bypass suite.
- **C6 (bind residue).** `SurfaceHttp::bind`
  (`http.rs:121-136`) requires ALL resolved addresses of the host to be
  loopback (today: any, then binds the first). Keep the existing typed
  refusals; existing bind tests must stay green.
- **C7 (RES-04).** `sandbox_denials` (`durable_surface.rs:236-241`):
  preferred shape is pinning the matcher to the governance gate's own
  escalation-summary vocabulary (find where the runtime constructs
  those summaries; share a constant so the two cannot drift). If that
  constant lives where the adapter/CLI cannot import it without a new
  dependency edge, instead widen the matcher to the runtime's actual
  summary spellings AND narrow the module doc + inline comment to
  describe exactly what matches. Record which shape you took and why.
- **C8 (RES-10).** A body `{"note": <non-string>}` is a typed 400
  `body_unparsable` (never a silent coercion to empty). Unknown fields
  stay ignored (documented posture).
- **C9 (RES-02 disclosure half).** The CLI acceptance output and the §20.3
  accepted log line disclose the settled unit and the resolved
  ticket(s) as separate named facts (they already do, mostly — verify
  and make the pairing non-implied where the ids could diverge; e.g. an
  explicit-ticket action names BOTH ids verbatim). Do NOT add a
  ticket↔unit binding check — that is a ports revision, deferred.
- **C10 (RES-05, RES-08, RES-11 — doc/comment truth).**
  `crates/adapters/gauntlet-adapter-surface-json/src/lib.rs:80-84`
  states the actual field-order rule (run preamble, then §20.2 sections
  in priority order, disclosure last) and drops the false "escalated
  FIRST = RuntimeSnapshot field order" parenthetical;
  `crates/gauntlet-cli/src/escalations_cmd.rs:5-6` names the real path
  (`execute_operator_action` / `settle_operator_action_gate` /
  `settle_operator_action_earned`); `crates/gauntlet-cli/Cargo.toml:12-15`
  says "the only feature edge THIS task adds".

## Tests first, teeth honest

For each behavioral item (C1-C6, C8): add or extend the failing test
FIRST, see it fail for the expected reason, then fix. Do not weaken any
existing assertion; if a test is wrong, demonstrate why in your report
before changing it. New tests live beside their kin (no-bypass suite,
adapter `tests/http.rs`, in-crate modules).

## Gates (all must be green on your final tree; one compiler at a time)

```
flock .agents/reports/task21/compile.lock -c '<cmd> && sccache --stop-server'
```

1. `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass`
2. `cargo nextest run -p gauntlet-adapter-surface-json`
3. `cargo nextest run -p gauntlet-app settle::` filter → bare name runs of
   the earned-arm tests (space-joined filter discipline: bare test names)
4. `cargo build -p gauntlet-cli --no-default-features`
5. `just check && just test && just lint && just deny && just boundaries && just ruler`
6. Mock E2E both configs: `cargo nextest run -p gauntlet-cli [--features surfaces] --test e2e_full_loop_mock`

## Guardrails

- Never touch `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.agents/specs/`, frozen snapshots (`cli_help.txt`,
  `cli_help_surfaces.txt` may only change if the surfaces help text
  legitimately changes — it should NOT; `serve` usage is unchanged).
  If `cli_help_surfaces.txt` would change, you have made a surface
  mistake — stop and record.
- No new external dependencies. No vendor names in capability surfaces.
- Builders do not commit. Do not stage anything; leave the tree dirty.
- Probe hygiene: nothing under `.gauntlet/policies/` or `.wayfinder/`.
- Model output / request bodies are untrusted input — your fixes must
  keep the parse-validate-narrow posture.

## Result contract

`.agents/reports/task21/cor1-report.md` — per item C1-C10: what changed
(file:line), the failing-first evidence, the gate evidence (post-last-
edit), and any deviation. Also write `cor1-progress.md` declare-first
(STARTED/DONE per item) as you go. A builder claim of completion
carries no settlement authority; write the report honestly and
completely — it is the only thing I read.
