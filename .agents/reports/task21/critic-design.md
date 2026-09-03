# Task 21 — Blind Design-as-Built Critic Report

STATUS: COMPLETE.

## Header

- Commission: `.agents/reports/task21/charter.md` (D1–D12, read in full);
  plan `.agents/plans/gauntlet-impl.md` §Task 21 (lines 652–672); spec
  `.agents/specs/Core-spec.md` §16.1 (1016–1027), §16.3 (1033–1043), §20.2
  (1202–1207), §20.3 (1209–1211), §21.3 (1229–1239); answer key AK-061
  (line 425) and AK-062 (line 432), read read-only.
- Review surface: HEAD commit `a71eba9` source content — the adapter crate
  `crates/adapters/gauntlet-adapter-surface-json/**`, the CLI feature work
  (`Cargo.toml`, `lib.rs`, `operator_action.rs`, `durable_surface.rs`,
  `serve_cmd.rs`, `escalations_cmd.rs`, `compose.rs`, `report_manifest.rs`),
  `crates/gauntlet-app/src/settle/mod.rs`, the tests
  (`operator_actions_no_bypass.rs`, `cli_surface.rs`, `escalations_cli.rs`,
  `cli_help_surfaces.txt`, adapter `tests/{contract_suite,http}.rs`), and the
  workspace/manifest files. `docs/**` markdown out of scope; `.jolli/` and
  report files excluded.
- Blindness rule honored: b1-report.md, b1-progress.md, IN-FLIGHT.md, all
  `*.log`, falsifier-round1.md, closure.md were NOT read.
- Method: every claim below carries file:line evidence read from the tree at
  a71eba9. Charter clause → tree → residual. Charter deviations recorded
  in-code by the tree are treated per the charter's own protocol ("deviations
  get recorded, not smuggled") and verified for accuracy rather than counted
  as smuggling.

## What was verified clean (so the record holds the positives with evidence)

- **D1 topology.** `crates/gauntlet-cli/Cargo.toml:16-17` — `default = []`,
  `surfaces = ["dep:gauntlet-adapter-surface-json"]`; the adapter is
  `optional = true` (`crates/gauntlet-cli/Cargo.toml:27`). No other manifest
  enables `surfaces` (rg over all Cargo.toml files: only this definition).
  Adapter source usage sites are exclusively under `#[cfg(feature =
  "surfaces")]` (`crates/gauntlet-cli/src/lib.rs:41-46` for
  `durable_surface`/`serve_cmd`; `lib.rs:69-70` for the `serve` dispatch;
  `lib.rs:109-110` for the usage line). The shared action path
  (`operator_action.rs`) imports no adapter-crate item, so the default build
  does not reference the gated crate. `cli_help.txt` is absent from the
  commit's file list (byte-identical); `cli_help_surfaces.txt` is the
  surfaces snapshot; `cli_surface.rs:31-35` selects the golden per config and
  `cli_surface.rs:114-129` asserts `serve` is unknown (exit 2) in the default
  build. Workspace `Cargo.toml:42,86` registers the adapter as a plain
  workspace member/dependency — nothing default-enables it.
- **D2 adapter.** `crates/adapters/gauntlet-adapter-surface-json/Cargo.toml:10-13`
  — dependencies are `gauntlet-domain` + `gauntlet-ports` ONLY (no
  dev-dependencies, no app). The commit does not touch `crates/gauntlet-ports/**`
  (verified via `git show --name-only a71eba9`), so the contract suite runs
  UNMODIFIED (`tests/contract_suite.rs:20-32` asserts empty findings over the
  suite's own factory). Route table `http.rs:232-256` is exactly §20.2:
  GET state/runs/{id}/units/{id}/escalated, POST refresh and
  units/{id}/resolve|retry; 405 carries `Allow` (`http.rs:679-689`); 404 and
  400 use `error_json` (`http.rs:196-204` in lib.rs; `http.rs:661-677`).
  Loopback-only bind (`http.rs:108-137`, refuses when NO resolved address is
  loopback), bounded head (16 KiB, `http.rs:33`) and body (1 MiB,
  `http.rs:37`), per-request socket timeouts + a port Deadline
  (`http.rs:42,755-766`). Every action body field is validated before use:
  unit via `UnitId::new` (`http.rs:331`), operator non-blank at the HTTP edge
  (`http.rs:337-346`) and via `OperatorIdentity::new` in the composition root
  (`durable_surface.rs:446-454`), `?run=` via `RunId::new` (`http.rs:379-390`);
  the body contributes ONLY an optional note string (`http.rs:347-378`) — no
  claim or readiness can enter through the request (D4's request-shape rule).
- **D3 one action path.** `execute_operator_action`
  (`crates/gauntlet-cli/src/operator_action.rs:71-119`) is the single
  execution body; `escalations_cmd.rs:236-245` (CLI) and
  `durable_surface.rs:467-476` (API handler trait impl) both call it. The
  adapter seam (`action.rs:134-136`) speaks only
  accepted/refused/failed — no settlement type crosses it (verified by
  reading every item exported from the adapter).
- **D4 positive arm.** `settle_operator_action_earned`
  (`crates/gauntlet-app/src/settle/mod.rs`, new method) runs
  `settle_operator_action_gate` FIRST, inside the same method, then calls the
  ONE `settle` with `ClosingActor::Operator { identity }` and
  `tracked_residuals: &[]`. The gate keeps the SAME `VerificationNotPass` and
  `OperatorIdentityRequired` errors (diff of the gate body preserves both
  arms verbatim). The mint (`operator_action.rs:318-395`) reproduces the
  `verify_cmd.rs` pattern exactly (compare `verify_cmd.rs:104-169`):
  resume-derived latest iteration (or 1), `VerificationAuthority::new` →
  `for_run` → `verify_claim` over the FROZEN unit's own `evidence_required`.
  `VerifiedClaim` has no public constructor and the request path mints only
  through this chain. Service-level teeth tests exist:
  `the_earned_arm_refuses_past_the_gate_even_holding_a_chain_minted_pass_claim`,
  `the_earned_arm_settles_through_the_same_commit_with_the_operator_identity`,
  `the_earned_arm_keeps_the_identity_requirement_inside_the_gate` (settle
  tests module, added by this commit).
- **D5 retry.** Retry is the same function with
  `OperatorAction::Retry`; the token is what is recorded on the ticket
  (`operator_action.rs:288`, `queue.resolve(..., action)`); no builder
  re-dispatch exists anywhere in the path.
- **D6 reads.** `durable_surface.rs` builds every snapshot field from the
  allowed durable reads only: open tickets (`queue.list()`), run derivation
  over open-ticket run ids (`:93-101`), checkpoint via `resume` (`:141`),
  unit/tier facts from the FROZEN key (`:109-133`), budget headroom from
  `load_run` limits minus checkpoint spend (`:172-225`), run-scoped facts via
  `load_run`/`verification_records`/`settlements` (`:299-378`). No new
  StateStore method was added (the StateStore trait in
  `crates/gauntlet-ports/src/state_store.rs` is untouched). The derivation is
  disclosed via `SnapshotDisclosure::run_derivation` (`durable_surface.rs:270-296`)
  and served with every snapshot (`http.rs:265`, `lib.rs:136-139`). `GET
  /api/v1/units/{id}` anchors across `?run=` or the derived run
  (`http.rs:308-325, 431-448`). The StateStore port names NO residual read
  (method list verified), so the always-empty `open_residuals`
  (`durable_surface.rs:242-244`) is honest absence within the charter's
  allowed read set.
- **D8 refresh.** `("POST", ["refresh"]) => self.snapshot_response()`
  (`http.rs:234`) — read-side only; nothing writes.
- **D9 serve.** `serve_cmd.rs:34-54` typed fail-closed on unset/empty/unreadable
  `GAUNTLET_API_BIND`; non-loopback refused in `SurfaceHttp::bind`; serves
  until interrupted (`serve_cmd.rs:96`); default build treats `serve` as an
  unknown command (asserted, `cli_surface.rs:114-129`).
- **D10 legs.** All five charter legs exist in
  `operator_actions_no_bypass.rs` driving the REAL binary over a real
  loopback socket against a seeded durable world: leg 1
  (`:632-710`, refusal code+message parity with the CLI in the SAME world,
  ticket open, zero settlement rows), leg 2 (`:719-845`, committed_now,
  exactly one row, ticket resolved with operator identity, idempotent
  re-POST commits nothing, CLI replay hits the queue's own "already
  resolved" refusal), leg 3 (`:854-883`, see RES-07), leg 4 (`:891-986`),
  leg 5 (`:994-1029`, corrupt ticket refuses, file never rewritten, nothing
  settles).
- **D11 teeth (structural).** Leg 1's world is seeded so the pinned evidence
  genuinely re-verifies PASS (leg 2 proves the mint succeeds in the identical
  world shape + gate substrate). The charter's one-line revert-class mutation
  (dropping the gate line inside `settle_operator_action_earned`) makes the
  settle's conditions re-derive over the chain-minted claim, which passes in
  leg 1's world, so the settle commits, `settlement_count()` becomes 1 and
  the 409/code/zero-rows assertions fail — the leg is RED under the mutation
  and the zero-settlement-rows assertions are the no-second-writer sensors as
  commissioned. Personal re-verification of teeth at settlement remains the
  orchestrator's job (not performed or claimed here).
- **No second writer.** Production `commit_settlement` call sites: ONLY
  `crates/gauntlet-app/src/settle/mod.rs:739` (inside `SettlementService::settle`).
  All other hits are ports contract suites, the recovery/completion paths
  (`producer_authority.rs`, adapter `complete_pending_settlements`, the
  pre-existing crash_helper bin). Production `queue.resolve` call sites: ONLY
  `operator_action.rs:248,288`, both strictly after the service accepted (or
  after the idempotent already-durable check). `VerificationRecordAuthority`
  appears in CLI production code nowhere (tests only). The adapter crate has
  no state access at all.
- **D12 dogfooding.** Recorded as fog, not dropped: charter §D12 states the
  block explicitly; `.agents/CURRENT_STATUS.yml` `next_action` names task21
  "the FIRST dogfooded task" with the UNK-003 operator decision context.
  Acknowledged in the commissioned records.

## Residuals

### RES-01 — MINOR — §20.3 action log is incomplete on the refused and failed arms

- Evidence: `crates/gauntlet-cli/src/operator_action.rs:96-117`. The accepted
  arm logs `action= run_id= unit= operator= outcome=accepted tickets=`;
  the REFUSED arm (`:104-110`) logs only `action= unit= operator=
  outcome=refused code=` — no `run_id`, no `ticket`; the FAILED arm
  (`:111-116`) likewise. `Refusal` (`:50-53`) carries only `code`/`message`,
  so the run id and tickets known at refusal time (resolved at
  `:142-192`, before the gate at `:230`) are structurally unavailable to the
  log line.
- Commission vs tree: charter D7 — "EVERY action — accepted AND refused —
  logs a §20.3 key=value line (action, run_id, unit_id, operator, outcome,
  ticket)". The mechanism (log on every outcome) exists; two commissioned
  fields are missing on exactly the arms an operator most needs to audit.
  Spec §20.3 lists `run_id` among the required context fields where
  applicable — it is applicable and known here.
- What the record must say: the refused/failed log lines omit run_id and
  ticket; the charter's D7 field list is met in full only on the accepted
  arm.
- Correction direction: carry `run_id` (and the resolved tickets) on
  `ActionDecision::Refused` (or log inside `decide_operator_action` where
  both are in scope) so the refused line carries the same field set;
  add a log-shape assertion to leg 1 of the no-bypass test.
- Cheapest discriminating test: serve a refusing action and assert the
  stderr line contains `run_id=<run>` and `ticket=<id>` — RED today.

### RES-02 — MINOR — an explicit-ticket action settles ANY unit and still resolves that ticket (no ticket↔unit relationship check)

- Evidence: `crates/gauntlet-cli/src/escalations_cmd.rs:203-210, 236-244`
  take `--ticket` and `--unit` as independent inputs;
  `operator_action.rs:142-150` derives only the run from the ticket record
  and pairs it with the operator-given unit; the settlement then addresses
  `request.unit_id` (`:220-224`) while `queue.resolve(ticket, ...)` closes
  the given ticket (`:248, 288`). `EscalationRecord` carries NO unit field
  (`crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs:131-140`),
  so the tree cannot bind them from the record alone. The same pairing applies
  in the idempotent re-entry branch (`:241-261`).
- Commission vs tree: charter D3's input vocabulary (action kind, unit id,
  operator identity, note — plus the CLI's ticket) does not commission a
  binding check, and the ticket record predates this task without a unit
  field, so this is not a bypass and not a commissioned violation. But with
  the write path now LIVE, `escalations resolve --ticket T --unit Y` settles
  unit Y (if Y's evidence earns PASS) and resolves ticket T — the durable
  record then shows T resolved by an action that never addressed the unit T
  escalated for. The acceptance output
  (`escalations_cmd.rs:246-253`) prints ticket and "settled" as one sentence,
  which is misleading when the ids are mismatched.
- What the record must say: the one action path does not (and structurally
  cannot, given the record shape) verify that the resolved ticket relates to
  the settled unit; a mismatched operator input resolves a ticket with a
  settle of a different unit.
- Correction direction (data only): record the gap at settlement; the
  principled fix is a unit binding on `EscalationRecord` (ports revision) or,
  cheaper, a typed warning line on the CLI acceptance naming both the settled
  unit and the resolved ticket so the pairing is disclosed rather than
  implied. Do not silently "fix" by matching free-text summaries.
- Cheapest discriminating test: in a world with PASS records for two units
  and one open ticket, resolve `--ticket T --unit OTHER` — today it succeeds
  and closes T; a binding check would refuse.

### RES-03 — MINOR — the serve loop terminates on transient accept errors, against its own "serves until interrupted" posture

- Evidence: `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:169-184`
  — `serve_while` breaks out of the loop on ANY `listener.accept()` error
  (`:178-181`), after one `eprintln`. `serve` (`:153-156`) then returns and
  `serve_cmd.rs:96-97` exits SUCCESS.
- Commission vs tree: charter D9 — "serves until interrupted"; §21.3 —
  "Surface/log failure → never crash the run". A run-state crash does not
  occur (the serve process is separate from any run), so this is not
  blocking; but transient accept errors of the EINTR / ECONNABORTED /
  EMFILE class — classic retry-and-continue conditions — end the surface.
  The module doc (`http.rs:5-7`) claims "a connection error is reported and
  the serve loop continues", which is true only for per-connection faults,
  not for the listener loop the same paragraph describes.
- What the record must say: one transient accept error stops the JSON
  surface until an operator restarts it; the doc overstates the loop's
  liveness.
- Correction direction (data only): classify accept errors — continue on
  transient classes (EINTR/ECONNABORTED/EMFILE), break on listener-fatal
  ones (EBADF after shutdown), keeping the §21.3 report-and-continue posture.
- Cheapest discriminating test: hard to drive portably through the real
  socket; a unit test over an injected accept-failure sequence (or a
  documented accept-failure posture) is the narrow proof.

### RES-04 — MINOR — sandbox_denials matches the substring "sandbox" only; the module doc claims the runner-isolation class too

- Evidence: `crates/gauntlet-cli/src/durable_surface.rs:236-241` —
  `record.summary.contains("sandbox")`; the module doc (`:17-21`) and the
  inline comment (`:226-227`) claim coverage of "the sandbox vocabulary the
  runtime's own governance gate escalates with ('sandbox policy ... failed
  validation', runner isolation below the tier's rung)".
- Commission vs tree: charter D6 — "sandbox_denials derives from queue
  tickets whose records name sandbox/containment escalation classes". A
  containment-class ticket whose summary names runner isolation without the
  literal word "sandbox" is silently absent from the operator's denial view,
  while the doc asserts it is covered. Misleading self-description over a
  real (narrow) recall gap.
- What the record must say: the denial view's matcher is the single
  substring "sandbox"; containment-class summaries without that token are
  not shown, contrary to the in-file claim.
- Correction direction (data only): either widen the matcher to the
  runtime's actual escalation summary vocabulary (and pin it to the
  governance gate's own summary constructor, so the two cannot drift), or
  narrow the doc to the implemented matcher. A shared vocabulary constant
  used by both the escalation site and this filter is the drift-proof shape.
- Cheapest discriminating test: seed a ticket whose summary names isolation
  without "sandbox" and assert the view's behavior is the documented one.

### RES-05 — MINOR — the "escalated FIRST" field-order claim is not what the wire serves, and the in-code justification misstates the port's own field order

- Evidence: `lib.rs:80-84` claims "canonical field order = the port's §20.2
  priority order (escalation queue FIRST — `RuntimeSnapshot`'s field
  order)", but `snapshot_json` (`lib.rs:85-141`) serializes `run` FIRST and
  `escalated` SECOND, matching `RuntimeSnapshot`'s actual declaration order
  (`crates/gauntlet-ports/src/operator_surface.rs:53-69`, where `run` is the
  first field). The tests pin the served order honestly
  (`tests/http.rs:409-429` asserts run < escalated < ...; `:197-226` asserts
  escalated before the later §20.2 sections).
- Commission vs tree: the charter conflicts with itself — D2 says
  "canonical field order = the port's §20.2 priority order, escalated FIRST"
  while D6 says "assembles the port's `RuntimeSnapshot` in its field order";
  the port's field order puts `run` first (it is preamble, not a §20.2
  priority section). The tree took the defensible side (D6/port order,
  escalated first among the §20.2 sections) and pinned it in tests — but the
  doc comment asserts both "escalated FIRST" and "RuntimeSnapshot's field
  order" as if they agreed, and they do not; an auditor reading the comment
  against the charter's D2 letter will conclude compliance that the wire
  does not literally show.
- What the record must say: `/api/v1/state` serves `run` before `escalated`;
  "escalated first" holds only among the §20.2 priority sections; the
  lib.rs comment's parenthetical is inaccurate.
- Correction direction (data only): one doc-line fix — state the actual rule
  ("run id preamble, then §20.2 sections in priority order, disclosure
  last") and note the recorded charter tension; no wire change needed unless
  the ruler's owner rules otherwise.
- Cheapest discriminating test: none needed beyond the existing order test —
  this is a documentation-accuracy residual.

### RES-06 — INFORMATIONAL — open_residuals is permanently empty and the wire does not disclose that

- Evidence: `durable_surface.rs:242-244` (always `Vec::new()`, in-code
  disclosure "no durable read in the read surface names residuals"); the
  StateStore port (`crates/gauntlet-ports/src/state_store.rs`, method list)
  names no residual read; charter D6's allowed-read list excludes residuals.
- Analysis: charter-compliant honest absence; but on the wire
  `open_residuals: []` is indistinguishable from "none open", while §20.2
  lists recent residuals as a snapshot section. The in-code comment
  discloses; the response does not (unlike run_derivation, which is
  disclosed).
- Record/correction direction: note at settlement that the residuals section
  is structurally empty until a typed durable residual read exists (already
  a recorded ports-revision deferral); optionally add a disclosure field
  alongside run_derivation when that read lands.

### RES-07 — INFORMATIONAL — charter leg 3's "authority forbids operator closure" refusal is genuinely unreachable; the in-test recorded deviation is accurate

- Evidence: `crates/gauntlet-app/src/settle/mod.rs` `check_authority`
  (pre-existing; untouched by a71eba9): `(Authority::Critic, _) => true`,
  `(Authority::LeadAgent, ClosingActor::Operator { identity }) => identity.is_some()`,
  `(Authority::Human, ClosingActor::Operator { identity }) => identity.is_some()`
  — an IDENTIFIED operator is permitted at every authority level; the only
  operator refusals are identity-None. The test doc
  (`operator_actions_no_bypass.rs:17-23`) records this deviation and leg 3
  asserts the closest service-refused leg (`unit_not_frozen` through the
  chain, `:854-883`).
- Analysis: the charter's leg-3 premise does not hold against the service's
  pre-existing authority vocabulary; manufacturing it would require changing
  the service or the frozen ruler (both out of scope). The deviation is
  recorded in-file per the charter's own protocol, and this critic verified
  the recording is true.
- Record/correction direction: the settlement record should restate the
  deviation so the operator sees that AK-062's "authority" refusal surface
  does not exist for identified operators as a class.

### RES-08 — INFORMATIONAL — escalations_cmd.rs header still names the retired entry point

- Evidence: `crates/gauntlet-cli/src/escalations_cmd.rs:5-6` — "`resolve` /
  `retry` go through `SettlementService::settle_operator_action`" — while the
  as-built path is `settle_operator_action_gate` +
  `settle_operator_action_earned` via `execute_operator_action` (the
  function-level doc at `:136-141` IS updated). The bare
  `settle_operator_action` now has no production callers (rg: tests and doc
  comments only), which is the intended structural posture.
- Record/correction direction: one header-line update at the next touch of
  this file; no behavior claim is affected.

### RES-09 — INFORMATIONAL — AK-062's frozen locator still names no materializable target; the tree follows the plan-gate name as commissioned

- Evidence: answer key line 437 freezes
  `tests/contract/operator_actions::no_bypass`; the as-built target is
  `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs` (charter D10
  explicitly commissioned that name so the plan gate `--test
  operator_actions_no_bypass` matches). The divergence is already recorded
  in `.agents/reports/reference-candidates.md` (2026-08-28 entry, "AK-062
  evidence locator names an unmaterializable test target").
- Record/correction direction: none for this task; the ruler's owner owns
  the locator. Settlement should reference the existing candidates entry
  rather than editing anything frozen.

### RES-10 — INFORMATIONAL — small untrusted-input honesty nits in the action request shape

- Evidence: `crates/adapters/gauntlet-adapter-surface-json/src/action.rs:38-52`
  records the `run` field as a RECORDED deviation from D3's exact field list
  (in-code, acceptable per the charter's protocol);
  `http.rs:353-361` — a body `{"note": <non-string>}` silently yields an
  empty note (`unwrap_or_default`) instead of a typed 400, and any additional
  body fields are silently ignored (they can never reach the settle path —
  only `note` is extracted — so this is a disclosure nit, not an admission
  risk).
- Record/correction direction: reject non-string `note` values with
  `body_unparsable`, or document the coercion; keep the ignore-unknown-fields
  posture explicit.

### RES-11 — INFORMATIONAL — the new Cargo.toml comment overstates "the ONLY feature edge in the workspace"

- Evidence: `crates/gauntlet-cli/Cargo.toml:12-15` claims "`surfaces` is the
  ONLY feature edge in the workspace", while `gauntlet-app`'s audited
  `composition` feature exists and is enabled by gauntlet-cli in the default
  build (`crates/gauntlet-cli/Cargo.toml:34`, predating this task).
- Analysis: `surfaces` is the only NEW edge this task adds; the comment as
  written is false and could mislead a later topology audit (the same
  comment appears at the workspace level, `Cargo.toml:84-85`, in the
  accurate "only edge that enables it [the adapter]" form).
- Record/correction direction: reword to "the only feature edge THIS task
  adds" at the next manifest touch.

## Charter tensions resolved by the tree (recorded, neither smuggled nor blocking)

1. D2 "escalated FIRST" vs D6 "the port's field order" — resolved as
   port-field-order with `run` as preamble (RES-05).
2. D3's exact field list vs the `?run=` anchor on actions — recorded
   in-code at `action.rs:38-44` (RES-10); the extension is disclosed and
   serves D6's run-anchoring.
3. D10 leg 3's authority-refusal premise vs the service's pre-existing
   authority vocabulary — recorded in-test (RES-07), verified accurate.
4. D7's log-field list vs the `Refusal` type's field set — NOT resolved
   (RES-01, minor).

## Verdict

The commissioned invariants hold as built: the default build is CLI-only with
the surfaces feature as the sole new edge; the adapter depends on ports +
domain only and passes the unmodified shared suite; one operator-action
execution path serves CLI and API with no settlement type crossing the seam;
the durable-PASS gate and identity requirement stand inside the earned arm
that pairs them with the ONE settlement commit; the no-bypass legs drive the
real binary over a real socket with zero-settlement sensors and structural
teeth; reads trace to named durable reads with the derivation disclosed; and
no second settlement or queue write path exists anywhere in the production
tree. The residuals are narrow: an incomplete refusal-log field set, an
unbindable ticket↔unit pairing now that the write is live, a transient
accept-error liveness gap, a narrower denial matcher than its doc claims,
and a self-inaccurate field-order comment.

VERDICT: CONFIRM (0 blocking / 0 major / 5 minor / 6 informational)
