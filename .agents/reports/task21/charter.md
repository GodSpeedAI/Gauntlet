# Task 21 charter — Operator surfaces (feature-gated)

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 21 (phase 5, optional for
Core). **Spec:** §20.2 (additional surfaces), §16.1/§16.3 (settlement +
commit), §20.3 (logging). **Ruler units:** AK-062 (both halves; the
POSITIVE half lands here — task20 honestly shipped only the refusal
posture). **Predecessor:** task20 settled 2026-09-01; its closure.md and
escalations_cli.rs header document the exact seam this task closes.

## Mission

A JSON API and a status view that add observability and action WITHOUT
becoming load-bearing: the default build stays CLI-only and completes a
full loop (AK-061); the surfaces are an opt-in Cargo feature; the API's
action endpoints perform the SAME atomic commit sequence as automatic
settlement (§16.3) and can never settle a Unit lacking PASS evidence
(AK-062). The escalations success path — a service-ACCEPTED operator
action resolving its ticket — becomes reachable end-to-end for the first
time.

## Discovered facts (do not re-derive; verify only where told to)

- The `OperatorSurface` port EXISTS: `crates/gauntlet-ports/src/operator_surface.rs`
  — `RuntimeSnapshot` (fields already in §20.2 priority order: escalated,
  in_flight, units by tier, budget, sandbox_denials, open_residuals),
  `OperatorAction::{Refresh, ResolveUnit, RetryUnit}`, and the shared
  contract suite `crates/gauntlet-ports/src/contract/operator_surface.rs`
  (`operator_surface_suite`: snapshot echo + actions-cover-the-§20.2-
  endpoints). NO adapter implements it yet.
- `SettlementService::settle_operator_action`
  (`crates/gauntlet-app/src/settle/mod.rs:784`) currently: loads the run;
  refuses WITHOUT durable PASS records with the SAME `VerificationNotPass`
  error the automatic path's entry check produces; requires
  `OperatorIdentity`; then ALWAYS returns
  `OperatorActionRequiresEarnedReadiness` — the deliberate task20 posture
  (escalations_cli.rs header lines 12–24: "the post-accept queue write is
  unreachable end-to-end until Task 21 composes an acceptance path").
- `SettlementService::settle` (`settle/mod.rs:673`) is the ONE settlement
  path: re-derives conditions 1–2 over the chain-minted `VerifiedClaim`
  itself (COR-2), adjudicates condition 3 (actor vs the frozen unit's
  authority), checks condition 4 (tracked then durable residuals) and
  condition 5 (settle-time reference re-verify), then the §16.3 commit
  (`commit_authority.admit` → `state.commit_settlement`), in-memory
  advance LAST. It cross-checks durable records (`VerificationRecordDisagrees`).
- `VerifiedClaim` has NO public constructor
  (`crates/gauntlet-app/src/verify/authority.rs:920`) — only the governed
  verification chain mints it. `gauntlet verify`
  (`crates/gauntlet-cli/src/verify_cmd.rs`) shows the composition pattern:
  `VerificationAuthority::new(state, evidence, ids)` → `for_run(...)` →
  `verify_claim(&request)` over the FROZEN unit's own `evidence_required`.
- The CLI's operator actions (`crates/gauntlet-cli/src/escalations_cmd.rs`)
  already build the service over `open_operator_surface` + `FsEvidenceStore`
  + `FileReferenceProvider` + `SystemClock`, require `--operator`, call
  `settle_operator_action`, and resolve the queue ticket ONLY on `Ok`
  (COR-R2-2 requested-id discipline; `FsEscalationQueue::resolve(ticket,
  operator, note, action)`).
- Read models exist: `state_cmd.rs` (`load_run`, `resume` → `Checkpoint`
  with unit_states/in_flight/budget_spent, `verification_records`,
  `settlements`, tier facts from the FROZEN answer key) and
  `gauntlet_app::state::export`. The state store names NO run enumeration
  (`status` requires `--run`, state_cmd.rs:48-58) — do not invent one.
- `Remaining` (budget headroom) lives in `gauntlet_ports::budget_meter`.
- Config: `operator.surfaces` is a typed config key already
  (`crates/gauntlet-app/src/config/resolve.rs:469`); `GAUNTLET_API_BIND`
  is in Annex E, `.env.example:113`, and `spec_vocab.rs:101`.
- JSON precedent: hand-rolled writers only (`gauntlet_app::report`'s
  `to_json`, report_manifest.rs). The workspace is std-only; NO serde.
- Gate syntax: nextest filters are space-joined — use bare test names.
- Battery baseline (task20 settlement): 1377/1377 workspace, boundaries
  228/228, lint/deny 0, ruler PASS, e2e_full_loop_mock + cli_surface 16/16.

## Contract decisions (binding; deviations get recorded, not smuggled)

- **D1 — feature topology.** `crates/gauntlet-cli` gains
  `[features] surfaces = []` and `default = []`. The DEFAULT build is
  byte-for-byte today's CLI-only build (AK-061): the new adapter crate is
  a NON-default, feature-gated dependency of gauntlet-cli (target-scoped
  `optional = true` + `features = ["surfaces"]` enabling it, or the
  equivalent minimal Cargo mechanics — builder's choice; keep it the ONLY
  feature edge). `cargo build -p gauntlet-cli --no-default-features` must
  succeed and the mock E2E must pass in BOTH configurations.
- **D2 — the surfaces adapter.** New crate
  `crates/adapters/gauntlet-adapter-surface-json` (std-only; deps:
  gauntlet-ports + gauntlet-domain ONLY — it must NOT depend on
  gauntlet-app). It implements the `OperatorSurface` port and a minimal
  HTTP/1.1 server over `std::net::TcpListener` (loopback binds; bounded
  request size; per-request deadline; no new external dependencies ever).
  Routes (§20.2, exact): `GET /api/v1/state`, `GET /api/v1/runs/{id}`,
  `GET /api/v1/units/{id}` (unit-scoped reads are run-anchored — see D6),
  `GET /api/v1/escalated`; `POST /api/v1/refresh`,
  `POST /api/v1/units/{id}/resolve`, `POST /api/v1/units/{id}/retry`.
  Errors are `{"error":{"code":"...","message":"..."}}`; unsupported
  methods return 405 (+ `Allow`); unknown routes 404; malformed requests
  400 — all in the §20.2 error body shape. It MUST pass the UNMODIFIED
  `operator_surface_suite` (factory-parameterized, the P8 pattern: the
  suite's seeded-snapshot constructor is injectable so the echo case runs
  against a synthetic source, and the REAL source is exercised by the
  integration tests). JSON encoding follows the in-tree hand-rolled
  precedent (string escaping, canonical field order = the port's §20.2
  priority order, escalated FIRST).
- **D3 — one operator-action path (D9 continuity).** Extract the
  resolve/retry execution body from `escalations_cmd::operator_action`
  into ONE shared composition-root function used by BOTH the CLI commands
  and the API's action handler. The adapter exposes a narrow action
  handler trait (input: action kind, unit id, operator identity, note;
  output: typed outcome/refusal — NO settlement types cross it); the
  composition root implements it over `SettlementService`. No second
  write path may exist; the plan's redesign trigger (enqueue, never a
  second writer) is satisfied by construction.
- **D4 — the positive acceptance path (AK-062 positive half).**
  `settle_operator_action`'s durable-PASS gate and identity requirement
  STAND (same `VerificationNotPass` / `OperatorIdentityRequired` errors).
  When a durable PASS record exists AND the identity is present, the
  shared path re-mints the readiness THROUGH THE GOVERNED CHAIN — compose
  `VerificationAuthority` over the SAME state/evidence stores, open
  `for_run` at the run's latest durable iteration, `verify_claim` with the
  FROZEN unit's own `evidence_required` (the verify_cmd pattern) — and
  then calls `SettlementService::settle(SettleRequest{ claim, actor:
  ClosingActor::Operator{ identity }, tracked_residuals: &[] })`: the
  SAME settle the automatic path uses, with conditions 1–5 re-derived
  inside and the §16.3 commit performed by the service. The operator
  request shape still carries NO claim and NO readiness. ANY service
  refusal (`VerificationNotPass`, authority condition-3 refusals,
  `BlockingResidualOpen`, reference condition-5, `VerificationRecordDisagrees`,
  domain-transition errors) leaves the ticket open and surfaces as the
  typed refusal. If the chain's re-derivation needs a service-side change
  (e.g. a `settle_operator_action` positive arm taking the minted claim),
  that change lives INSIDE the existing service method's contract — the
  no-bypass gate at the service level is never relaxed or duplicated.
- **D5 — retry semantics.** Retry walks the SAME earned-PASS settlement
  path; its recorded meaning is "re-run verification and settle only on a
  fresh earned PASS" — the action token on the ticket distinguishes it.
  It never re-dispatches a builder and never settles without earned PASS
  (AK-062 names resolve and retry identically). A loop-consumed
  re-dispatch retry is NOT invented here; the orchestrator records it as
  fog for the operator.
- **D6 — reads without invented ports.** Read endpoints reuse the
  existing durable reads only (`open_operator_surface`, `queue.list()`,
  `load_run`, `resume`, `verification_records`, `settlements`, frozen-key
  tier facts). `/api/v1/state` assembles the port's `RuntimeSnapshot` in
  its field order; the "active run" is DERIVED from durable artifacts
  (open escalation tickets carry run ids; the newest run's checkpoint
  anchors the rest) and the derivation is DISCLOSED in the response
  (`state.run_derivation` string) — never a silent guess, never a new
  StateStore enumeration method. `/api/v1/units/{id}` resolves the unit
  across the derived (or `?run=`-given) run. `sandbox_denials` derives
  from queue tickets whose records name sandbox/containment escalation
  classes; when nothing durable names a denial the field is EMPTY (honest
  absence — no new denial log is invented).
- **D7 — operator identity + logging.** CLI keeps `--operator` (required).
  The API requires the identity on every action (builder's choice of
  required `X-Gauntlet-Operator` header or body field, validated through
  `OperatorIdentity::new`); missing/blank identity is a typed 4xx. EVERY
  action — accepted AND refused — logs a §20.3 key=value line (action,
  run_id, unit_id, operator, outcome, ticket) to the command's stderr;
  large payloads by evidence ref; secrets never printed.
- **D8 — refresh.** `POST /api/v1/refresh` re-assembles and returns the
  snapshot. A read-side action; it writes nothing.
- **D9 — serve command.** The CLI gains `gauntlet serve` (compiled only
  under `surfaces`): reads the bind from `GAUNTLET_API_BIND` (typed
  fail-closed configuration error when unset/empty or unparsable; refuses
  non-loopback binds), serves until interrupted, never crashes the run
  state (§21.3: surface failure ≠ run failure). DEFAULT-build behavior is
  unchanged: `serve` is an unknown command there (exit 2), and the frozen
  `cli_help.txt` snapshot stays BYTE-IDENTICAL. The surfaces build's help
  gets its own frozen snapshot (or equivalent mechanical proof) so
  `cli_surface` passes under `--features surfaces` too.
- **D10 — the no-bypass proof.** New integration test
  `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs`
  (`#![cfg(feature = "surfaces")]`, named so the plan gate's
  `--test operator_actions_no_bypass` matches), driving the REAL binary's
  serve over a real loopback socket against a seeded durable world (the
  escalations_cli.rs `World` pattern). Required legs at minimum:
  1. resolve on a unit with NO/non-PASS verification records ⇒ the SAME
     typed refusal code+message as the automatic/CLI path, ticket stays
     open, ZERO settlement rows;
  2. the positive leg: a unit whose pinned evidence re-verifies PASS ⇒
     settlement row + event appear through the SAME commit (exactly once;
     idempotent re-POST), the ticket resolves with the operator identity;
  3. an authority-condition refusal leg (an operator action on a unit
     whose frozen authority forbids operator closure is refused by the
     service, ticket open);
  4. 405 on wrong methods; 404 unknown route; missing identity 4xx with
     the error body shape;
  5. the order leg: a refused action never touches the queue; the queue
     write happens only after the service accepts (corruption/replay of
     the ticket stays the queue's own typed refusal).
- **D11 — teeth.** The refusal-parity leg must FAIL (RED) when the
  positive path is wired to bypass the durable-PASS gate (one-line
  revert-class mutation), and the no-bypass test must fail if any second
  write path appears (the zero-settlement-rows assertions are the
  sensors). The orchestrator re-verifies teeth personally at settlement.
- **D12 — dogfooding.** Plan Task 20 step 6 makes this the first
  dogfooded task, but `runners.default` is UNSET BY DESIGN (UNK-003 — the
  operator's decision) and the mock cannot author code. The task runs
  through the established delegation loop instead; the orchestrator
  records the dogfood block as fog, never silently.

## Builder steps

1. Read this charter + `escalations_cmd.rs`, `settle/mod.rs` (the
   `settle_operator_action` / `settle` pair + errors), `verify_cmd.rs`,
   `state_cmd.rs`, the port + suite files, `queue.rs`, `context.rs`,
   `compose.rs` (`open_operator_surface`), `escalations_cli.rs`,
   `cli_surface.rs`/`cli_help.txt`, workspace + arch-tests manifests.
2. Write the failing-shape tests first where practical (the no-bypass
   legs, the contract-suite passage for the adapter); confirm they fail
   for the expected reason before implementing.
3. Land the adapter crate (D2) with its contract-suite passage + unit
   tests; `just check-package gauntlet-adapter-surface-json` +
   `just test-package gauntlet-adapter-surface-json` + `just boundaries`.
4. Land the CLI feature work (D1, D3–D5, D7, D9) + the no-bypass test
   (D10). Package-scoped checks first; workspace gates at the end.
5. Run the plan gate: `cargo nextest run -p gauntlet-cli --features
   surfaces --test operator_actions_no_bypass` (space-joined filter
   discipline: the file name IS the test target) and
   `cargo build -p gauntlet-cli --no-default-features`. Then the mock E2E
   under BOTH feature configurations.
6. Write `.agents/reports/task21/b1-progress.md` (declare-first
   checkpointing: STARTED/DONE lines per item) and `b1-report.md` (what
   landed, what diverged from the charter, gate evidence, honest
   unknowns). The report file is the result contract: completion means a
   schema-honest `b1-report.md` exists. A builder claim of completion
   carries no settlement authority.

## Guardrails (all roles; AGENTS.md binds regardless)

- Never edit `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.agents/specs/`, or any frozen/prereg artifact. `just ruler` before and
  after every phase.
- Builders never weaken assertions, gates, or the service's no-bypass
  posture to make a test pass; a wrong test is demonstrated wrong, then
  changed separately.
- NO new external dependencies; the workspace stays std-only. NO vendor
  names in capability surfaces (boundary suite enforces).
- One compiler at a time:
  `flock .agents/reports/task21/compile.lock -c '<command> && sccache --stop-server'`.
  Prefer `just check-package`/`just test-package` during development; keep
  tool-call cadence high (background + poll, never one synchronous call
  over ~3 minutes).
- Builders do not commit. One task, one commit (orchestrator). Never stage
  `.jolli/`, `.tmp/`, `.agents/prompts/`, operator files.
- Model output is untrusted: the API's request bodies are untrusted input
  — parse, validate, narrow; never admit into consequential state before
  validation.
