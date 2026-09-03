# COR-1 report (Task 21 — operator surfaces correction slice)

Builder: COR-1. Tree: HEAD a71eba9 (unchanged; nothing staged, nothing
committed — the tree is left dirty as commissioned). All findings from
`critic-design.md` (RES-*) and `falsifier-round1.md` (F-*) per the
adjudications in `design-adjudication.md` and `falsifier-adjudication-r1.md`.
Progress log: `cor1-progress.md` (declare-first, maintained throughout).

## Files changed (all unstaged)

- `crates/gauntlet-cli/src/operator_action.rs` (C1, C2)
- `crates/gauntlet-cli/src/escalations_cmd.rs` (C9, C10)
- `crates/gauntlet-cli/src/durable_surface.rs` (C5, C7)
- `crates/gauntlet-cli/Cargo.toml` (C10)
- `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs` (tests: C1, C2, C5, C7, C9)
- `crates/gauntlet-app/src/govern/gate.rs` (C7 shared vocabulary + pin test)
- `crates/gauntlet-app/src/govern/mod.rs` (C7 re-export)
- `crates/adapters/gauntlet-adapter-surface-json/src/http.rs` (C3, C4, C6, C8 + in-crate tests)
- `crates/adapters/gauntlet-adapter-surface-json/src/lib.rs` (C10)
- `crates/adapters/gauntlet-adapter-surface-json/tests/http.rs` (test: C8)

Guardrail verification (end of slice): `git diff --stat HEAD` over
`crates/gauntlet-cli/tests/cli_help.txt`, `cli_help_surfaces.txt`,
`.wayfinder/answer-key.md`, `.gauntlet/reference.lock`, `.agents/specs/`
is EMPTY (both snapshots byte-identical to how they started); no new
external dependencies (`Cargo.lock` untouched); nothing under
`.gauntlet/policies/` or `.wayfinder/` touched; no commits.

## Per-item record

### C1 — §20.3 action-log field parity on the refused and failed arms (RES-01, D7) — DONE

- What: `operator_action.rs:60-68` new private `ActionLogContext`
  { run_id, tickets }, threaded through `decide_operator_action`
  (signature `:153`) and set exactly where the durable reads resolve
  them (explicit-ticket read `:171-172`; run anchor `:180`; single open
  ticket `:187-189`). The refused arm (`operator_action.rs:126-128`) and
  the failed arm (`:133-136`) now log the same field set as the accepted
  arm — `action run_id unit operator outcome tickets [code]` — empty
  until known, never invented; one log shape for all outcomes. The note
  is still never logged.
- Failing-first: leg 1 RED — panics "the refused §20.3 line names the
  run: operator-action: action=resolve unit=AK-901 operator=...
  outcome=refused code=verification_not_pass" (no run_id, no tickets
  field) at `operator_actions_no_bypass.rs:712`.
- Post-last-edit gate: gate 1 GREEN 8/8 (leg 1 + the C2 test's failed-arm
  assertions: run_id= and tickets= present on the failed line).

### C2 — re-entry branch names the already-durable truth (F-1) — DONE

- What: `operator_action.rs:278-286` — the re-entry branch's queue-write
  failure now reads "the settlement is already durable; the queue write
  failed: {error}" with a comment stating the branch performed no settle
  on that call. The fresh-accept path (`:345-348`) keeps "the settle
  committed but the queue write failed".
- Test:
  `the_reentry_failure_names_the_durable_truth_the_fresh_one_names_the_commit_and_the_acceptance_names_both_ids`
  (`operator_actions_no_bypass.rs:866`): part A — CLI settle, then CLI
  replay of the same ticket: asserts the already-durable wording, asserts
  `!stderr.contains("the settle committed")`, settlement count still 1.
  Part B — escalations dir chmod 555 BEFORE the first CLI action: asserts
  the fresh wording, settlement count 1 (the commit is real), then
  restores perms and asserts the §16.3 re-entry resolves the ticket with
  no second row (recovery).
- Failing-first: the test RED at the C9 assertion first (below); with
  C9's line fixed the C2 assertions were exercised against the old
  message in the same failing run family — the old wording ("the settle
  committed but the queue write failed: ... already resolved") was the
  falsifier's F-1 executed repro, and the test's
  `!contains("the settle committed")` assertion discriminates old vs new
  directly. Post-fix: GREEN.
- Post-last-edit gate: gate 1 GREEN 8/8.

### C3 — accept-error classification (RES-03, F-7) — DONE

- What: `http.rs:500-517` private `AcceptSource` seam (`TcpListener` is
  the production impl; `serve_while` keeps its public signature and
  delegates to `serve_accept_loop(&dyn AcceptSource, ...)`,
  `http.rs:186-215`). `accept_error_is_transient` (`http.rs:520-535`):
  continue on Interrupted, ConnectionAborted, ConnectionReset,
  BrokenPipe, WouldBlock, TimedOut, OutOfMemory, StorageFull, plus a
  raw-errno fallback for ENFILE(23)/EMFILE(24) — this toolchain maps
  EMFILE/ENFILE to `ErrorKind::TooManyOpenFiles`, which is UNSTABLE to
  name (E0658 probe recorded in cor1-progress.md); break on everything
  else (EBADF→Uncategorized, EINVAL→InvalidInput, NotFound — including a
  NotFound after a deliberate close). The transient arm prints "...;
  continuing" and continues; the fatal arm prints "the listener is no
  longer usable; the surface stops" and breaks — the "continuing"
  eprintln structurally cannot print on the break. Module doc
  (`http.rs:1-11`) rewritten to describe the real classification.
  std-only; no new dependency.
- Tests (in-crate `mod accept_and_log`, `http.rs:861+`):
  `transient_accept_errors_continue_and_listener_fatal_ones_break`
  (classification table over raw errnos and constructed kinds);
  `the_serve_loop_continues_past_transient_errors_and_stops_on_a_listener_fatal_one`
  (injected sequence: Interrupted, ConnectionAborted, a REAL connected
  pair served through `handle`, then EBADF, then an unreachable step;
  asserts the flag never fell, exactly-through-fatal consumption, and
  the accepted step answered after the transient errors).
- Failing-first: the seam and classifier did not exist pre-fix — package
  RED by compilation (E0425 `access_log_line`, E0425
  `accept_error_is_transient`, E0405/E0422 `AcceptSource`), captured.
  The pre-fix behavior itself (any accept error ends the loop while
  printing "continuing") is the critic's/falsifier's code-level finding;
  a continue-past-transient test is not expressible without the seam.
- Post-last-edit gate: gate 2 GREEN 10/10.

### C4 — escape the request target in the access log (F-4) — DONE

- What: `http.rs:812-817` `access_log_line(method, target, status)` —
  the attacker-controlled target lands `{:?}`-escaped, the same posture
  as the §20.3 action log's identity escaping; `handle` logs through it.
- Test: `the_access_log_line_escapes_the_request_target` — a target with
  a raw 0x1b byte produces no raw 0x1b in the line and shows the
  `\u{1b}` escaped form; a plain target reads as itself.
- Failing-first: same compile RED as C3 (helper did not exist); the
  pre-fix raw-byte behavior is the falsifier's executed F-4 repro.
- Post-last-edit gate: gate 2 GREEN 10/10.

### C5 — unit read on a non-durable run refuses (F-5) — DONE

- What: `durable_surface.rs:395-410` — `unit_view` performs the run
  view's OWN `load_run` durability check first and returns
  `RunScopedError::NotFound` with the run view's exact wording ("run
  {run} is not durable in this state store", cf. `:315`); http.rs maps
  NotFound → 404 `not_found` (existing mapping). The check lives in the
  port seam (one place); the derived-run path (no `?run=`) is unaffected
  in honest worlds because its run ids come from open tickets — a
  derived-but-undurable id (forged durable ticket) now refuses instead
  of serving default `open` facts, which is strictly more honest.
- Test: `a_unit_read_on_a_non_durable_run_404s_with_the_run_view_s_own_wording`
  (`operator_actions_no_bypass.rs:1209`): 404 + code `not_found` + the
  run view's wording on `GET /api/v1/units/AK-901?run=<valid ULID never
  durable>`; run-view parity (same 404 + wording); derived-path control
  (no `?run=` still 200).
- Failing-first: RED — "the unit read refuses a non-durable anchor:
  HTTP/1.1 200 OK" with body `{"unit":{"status":"open",...}}`.
- Post-last-edit gate: gate 1 GREEN 8/8.

### C6 — bind requires ALL resolved addresses loopback (falsifier bind residue) — DONE (with recorded test limitation)

- What: `http.rs:131-141` — the any-loopback check became
  find-first-OFFENDER: any resolved address that is not loopback is a
  typed `BindRefused` naming that address, before any socket is opened.
  Existing typed refusals kept; message shape preserved.
- Evidence: existing bind tests stay GREEN in gate 2
  (`the_bind_refuses_non_loopback_hosts_before_opening_a_socket`); every
  directly-specifiable non-loopback address still refuses.
- Deviation (necessity, recorded): no NEW discriminating test — a mixed
  (non-loopback, loopback) multi-address resolution needs DNS control
  this host does not offer (the falsifier hit the same wall). The
  predicate change is one line and covered on the single-address side by
  the existing tests.

### C7 — sandbox_denials pinned to the governance gate's own vocabulary (RES-04, F-6 matcher half) — DONE

- Shape taken: the SHARED-CONSTANT shape (the brief's preferred), viable
  because the CLI already depends on gauntlet-app — no new dependency
  edge.
  - `gate.rs:511-545`: `pub const CONTAINMENT_SUMMARY_TOKENS` — one
    token per containment arm of `GateRefusal::summary` ("sandbox
    policy", "sandbox backend", "runner reports isolation", "without a
    backend", "policy could not be resolved") — and
    `pub fn containment_summary_matches(&str)`. Doc names the arm→token
    mapping. Re-exported at `govern/mod.rs:19`.
  - Drift pin (gate side): new test
    `every_containment_summary_carries_the_shared_vocabulary_and_authority_ones_do_not`
    constructs every containment variant directly and asserts its own
    `summary()` matches, and the three authority-class variants do not —
    changing either side alone goes RED.
  - `durable_surface.rs:246`: the filter calls the shared matcher;
    module doc (`:18-27`) and inline comment (`:238-240`) narrowed to
    describe exactly what matches and that the vocabulary is shared.
- Test (behavior, real binary): 
  `the_denial_view_names_the_governance_gate_s_containment_vocabulary`
  (`operator_actions_no_bypass.rs:1264`) escalates a ticket whose
  summary is the gate's OWN `CapabilityMismatch` summary (built via the
  gate constructor — no copied string; carries no "sandbox" token) and
  asserts it appears in `/api/v1/state`'s `sandbox_denials`, and that
  the non-containment ticket still does not appear.
- Failing-first: RED — "the runner-isolation escalation is IN the denial
  view: []".
- Post-last-edit gates: gate 1 GREEN 8/8; gate 3 pin test GREEN.
- Recorded: one test-side fix during bring-up — the pin test's fixture
  unit id "AK-TOKENS" violated the `UnitId` pattern (assertion on the
  constructor, not on any assertion being weakened); changed to
  "AK-902". No assertion weakened anywhere.

### C8 — non-string note is a typed 400 (RES-10) — DONE

- What: `http.rs:380-408` — a body `{"note": <non-string>}` now returns
  `400 body_unparsable` ("the action body's 'note' must be a JSON
  string"), never a silent coercion to empty (null included). Unknown
  fields stay ignored, with the ignore-posture now documented inline
  (the body contributes only an optional note; no other field can reach
  the action path).
- Test: extension of `malformed_requests_are_typed_400s`
  (`tests/http.rs:311-331`) — `{"note": 123}` must be 400
  `body_unparsable`.
- Failing-first (honest sequence): with all other fixes in place the fix
  hunk was temporarily reverted to the old coercion; the package run
  showed the C8 test RED at exactly the new assertion ("a non-string
  note is 400: 409" — the old path proceeded to the scripted 409) while
  C3/C4 were already GREEN; the fix was re-applied and the package went
  GREEN 10/10.
- Post-last-edit gate: gate 2 GREEN 10/10.

### C9 — acceptance discloses settled unit and resolved tickets as separate facts (RES-02 disclosure half) — DONE

- What: `escalations_cmd.rs:249-265` — the CLI acceptance line now reads
  `escalations: ticket=<requested> action=<w> operator=<op> — settled
  unit <unit> of run <run>; resolved tickets: <list|(none)>`: an
  explicit-ticket action names BOTH the requested ticket id and the
  settled unit id verbatim, and the actually-resolved list is a separate
  named fact (non-implied pairing). The §20.3 accepted log line already
  carried `unit=` and `tickets=` separately (verified; unchanged). No
  ticket↔unit binding check added — that is the deferred ports revision
  (adjudication: TRIGGER-RECORD).
- Failing-first: RED — "the acceptance names the settled unit:
  escalations: ticket=esc-6a98caed-0 ... — settled through the
  settlement service's operator path" (no settled unit named).
- Post-last-edit gate: gate 1 GREEN 8/8 (part A asserts both ids
  verbatim on the new line).

### C10 — doc/comment truth (RES-05, RES-08, RES-11) — DONE

- `lib.rs:34-40` (module doc) and `lib.rs:82-88` (`snapshot_json` doc):
  the actual rule stated — run preamble first, then the §20.2 sections
  in priority order, disclosure last — with the D2 charter tension
  recorded ("escalated FIRST holds among the §20.2 sections, not over
  the whole body"); the false "escalated FIRST = RuntimeSnapshot field
  order" parenthetical dropped. (Both spots carried the false claim; the
  brief named the fn doc, the module doc was the same claim and is fixed
  too.) Tests already pin the served order; unchanged and GREEN.
- `escalations_cmd.rs:1-10`: the header now names the real path
  (`execute_operator_action` / `settle_operator_action_gate` /
  `settle_operator_action_earned`) instead of the retired
  `SettlementService::settle_operator_action`.
- `crates/gauntlet-cli/Cargo.toml:11-15`: "the only feature edge THIS
  task adds" (was "the ONLY feature edge in the workspace").
- No behavioral test (doc-only); evidenced by gate 5 compilation, clippy
  `-D warnings`, and `just ruler` PASS.

## Gates — all green on the FINAL tree (post last edit)

The last source edits were the rustfmt pass over the three touched
packages (diffs exclusively in COR-1's new code; `cargo fmt --check`
clean). Every gate below was run after that edit, one compiler at a time
under `flock .agents/reports/task21/compile.lock -c '<cmd> &&
sccache --stop-server'`:

1. `cargo nextest run -p gauntlet-cli --features surfaces --test
   operator_actions_no_bypass` → 8/8 PASS (5 commissioned legs + 3
   COR-1 tests).
2. `cargo nextest run -p gauntlet-adapter-surface-json` → 10/10 PASS
   (unmodified shared contract suite included).
3. `cargo nextest run -p gauntlet-app settle::` → 30/30 PASS (earned-arm
   tests included); bare-name run
   `every_containment_summary_carries_the_shared_vocabulary...` → 1/1
   PASS.
4. `cargo build -p gauntlet-cli --no-default-features` → PASS.
5. `just check && just test && just lint && just deny && just boundaries
   && just ruler` → check clean; 1391/1391 tests PASS (includes the
   golden `cli_help.txt`/`cli_help_surfaces.txt` snapshot tests and the
   arch/boundary suites); clippy `--workspace --all-targets --
   -D warnings` clean; deny "advisories ok, bans ok, licenses ok,
   sources ok"; boundaries 228/228 PASS; ruler PASS (frozen lock
   digests match).
6. Mock E2E both configs: `cargo nextest run -p gauntlet-cli --test
   e2e_full_loop_mock` → 12/12 PASS; `cargo nextest run -p gauntlet-cli
   --features surfaces --test e2e_full_loop_mock` → 12/12 PASS.

## Deviations and records for the orchestrator

1. C6 has no NEW test: a mixed multi-address set needs DNS control the
   host does not provide. The one-line predicate strengthening is
   covered on every directly-specifiable address by the existing bind
   tests (GREEN). 
2. C3/C4's failing-first evidence is a compile RED (the seam, the
   classifier, and the log helper did not exist before the fix); the
   pre-fix runtime behaviors are the falsifier's code-level findings
   (F-4 executed; RES-03/F-7 code-level). C8 got a true executed
   failing-first run (fix temporarily reverted).
3. Classification limit recorded: ENOBUFS has no nameable ErrorKind on
   this toolchain (maps to Uncategorized, which also holds EBADF) and
   therefore lands in the BREAK class. The brief's transient list
   includes ENOBUFS; closing that gap would require either an unstable
   kind name or hardcoded errno tables beyond the EMFILE/ENFILE pair —
   taken as far as std's stable kinds allow, documented at
   `http.rs:520-535`.
4. C5's durability check lives in `unit_view` (the port seam), so a
   derived-but-undurable run id (only reachable via forged durable
   tickets, the falsifier's steering scenario) now also refuses instead
   of serving default `open` facts — read-parity made total, strictly
   more honest than the brief's minimum.
5. C7 shape: shared-constant (preferred) via `gauntlet_app::govern`,
   drift-pinned by a gate-side test. Queue summaries are NOT rewritten
   (no durable-record format change): the matcher is contains-based
   over the exported tokens, so the controller's
   "dispatch refused by the governance gate: {summary}" prefix is
   transparent to it.
6. During gate 3 bring-up the new pin test's own fixture unit id
   ("AK-TOKENS") violated the domain's UnitId pattern; the fixture
   string changed to "AK-902". No assertion was weakened; no existing
   test was changed at all in this slice (extensions and additions
   only).
7. `.agents/CURRENT_STATUS.yml`, `.agents/reports/task21/IN-FLIGHT.md`
   and `.jolli/*` show as modified/untracked in `git status` — those
   changes predate/outside this builder's scope; this builder touched
   none of them. Builder claim of completion carries no settlement
   authority.
