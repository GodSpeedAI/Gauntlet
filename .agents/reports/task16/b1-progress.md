# b1-progress — Task 16 builder checkpoint (append-only)

- Builder dispatched 2026-08-29 (B1, compile lock held). Baseline: HEAD
  054176b, clean tree (only orchestrator's task16 IN-FLIGHT/prereg files +
  .jolli noise untracked). `just ruler` PASS at dispatch per IN-FLIGHT.md.

## Design resolutions taken (recorded BEFORE implementation, for mid-flight adjudication)

1. **Decision-3 internal tension — RESOLUTION RECORDED.** The charter says the
   shape-2 case is "registered in agent_runner_suite" AND "the prime test
   double learns the new script; its suite must stay green". These cannot both
   hold as written: the prime adapter's pinned architecture (Annex C.4 via
   prime's runner docs: liveness is NEVER a state input; poll classifies only
   from the contract document and the deadline) has NO code path to
   `Failed{missing_result}` — a prime invocation whose child never writes can
   only reach `TimedOut`. Prime's runner is out of my touch surface
   ("test-support change only").
   Resolution: `AgentRunnerFactory` gains a DEFAULTED method
   `honors_script(&self, script) -> bool` (default `true` — additive,
   zero breakage). The new case function runs its FULL pin
   (`Failed{missing_result}` on the first probe, never Completed, never
   TimedOut, collect refuses) only when the factory honors the script; for a
   factory that declares it does not (prime's suite factory will, with a
   comment citing Annex C.4), the case still binds the UNIVERSAL half — the
   dead child must never be reported `Completed` (§10.3 rule 3) — so the case
   is non-vacuous everywhere and fully toothed for mock + cli. p8 and the cli
   crate's own suite run the FULL suite over mock + cli (both honor it), so
   the rule is proven where it applies.
2. **Exhaustive-match fallout (authorized by charter decision 2):**
   `crates/gauntlet-ports/src/contract/stubs.rs` (ScriptedRunner::advance) and
   `crates/gauntlet-app/src/control/controller.rs` (dispose_failure) both
   match `RunnerScript` / `InvocationFailure` exhaustively. Ports stub arm:
   ExitWithoutResult ⇒ pin `Failed{MissingResult}` on first probe. Controller
   arm: `MissingResult` joins the §11.5 "transient (process)" row (retry with
   backoff) — the child died without producing a document; a re-dispatch can
   succeed; "never mark the unit settled" holds. The spec §11.5 table names no
   `missing_result` row; this mapping is RECORDED as a disposition decision
   for the closure record (fog: the alternative — TimedOut-row retry-once —
   was rejected as conflating a deadline verdict with an observed exit).
3. **SIGTERM deviation (charter decision 6):** std has no signal API; graceful
   SIGTERM needs `libc`/`nix` (ask-first dependency). Production stop =
   `Child::kill()` (SIGKILL) + bounded reaping wait. The seam's `stop` is
   shaped terminate-then-kill so a SIGTERM-first impl slots in without a
   runner change. RECORDED as deviation, not silently chosen.
4. **Template strictness (decision 4):** placeholders must occupy the WHOLE
   argv word (`{model}` yes, `--model={model}` and `x{model}y` ⇒ typed error)
   so substituted values can never fuse with adjacent text into grammar; ANY
   `{...}`-shaped span (known or unknown) in any word is validated; an
   unclosed `{` is a typed error. Substituted values become single argv words
   verbatim (metacharacters are data — the child is spawned directly, no
   `sh -c`).

## Progress log

- [t0] Recon done (ports, contract suite, prime, mock, arch tests, doctor,
  prereg, IN-FLIGHT). Checkpoint created. NEXT: ports revision.

## RES-D routed (2026-08-29, mid-flight) — ACKNOWLEDGED

All 12 design-review corrections received and accepted; they supersede the
corresponding brief decisions. Integration plan against work already landed:

- RES-D1: REVERTING the `agent_runner_suite` registration I had added and the
  `honors_script` trait machinery (my resolution of the decision-3 tension is
  superseded). Shape-2 case becomes the exported sibling
  `exit_without_result_suite(factory)`; run against mock + cli in their own
  contract tests and in p8. Prime keeps only the compile-only `document_for`
  arm (already landed). stubs.rs arm already landed.
- RES-D2: envelope body sibling `<leaf>.envelope.json` referenced from the
  prompt file (planned; confirmed in the build).
- RES-D3: child env = parent env + `GAUNTLET_RESULT_PATH` (constant exported
  from the adapter; inheritance posture documented; pruning is task20's).
- RES-D4: controller arm ALREADY LANDED with the row-2 group; aligning the
  comment with the adjudicated rationale (finer class, same transient
  disposition).
- RES-D5: arch teeth RE-SCOPED to a whole-crate text scan of
  `crates/adapters/gauntlet-adapter-agent-cli/**` over the existing
  DENIED_TERMS (no public-API extraction, no new term list, no masks).
- RES-D6: `RunnerDescriptor` gains a typed what-was-measured dimension;
  the two evidence-string sites render the kind; cli reports the
  configuration kind; construction sites updated honestly.
- RES-D7: no doctor refusal logic; honest capabilities only.
- RES-D8: total-order classification pinned; exit status in adapter-local
  forensics only; signal-death collapses to missing_result (documented);
  template refusal set + no-expansion tokenization + `#` comment rule +
  newline flattening implemented as specified.
- RES-D9: injected Clock only (planned).
- RES-D10: p8 cli factory template is a constructor-injected test literal;
  `GAUNTLET_P8_ADAPTER` selects the battery only (planned).
- RES-D11: touch-surface discipline held; prime tests-only.
- RES-D12: forensic sibling `<leaf>.handle.json` (pid + identity);
  invocation dir retained post-terminal; cancel = SIGTERM -> bounded wait ->
  SIGKILL (std-only implementation: fixed-argv `sh -c 'kill -TERM "$1"'
  helper for the TERM leg, pid passed as an argv argument, no operator data
  in the command — RECORDED: a libc/nix dependency would be the clean way
  and remains ask-first); kill-after-exit maps to success; reaping via
  try_wait in poll (non-blocking).

- [t1] RES-D rework landed: suite registration REVERTED; shape-2 case is the
  exported `exit_without_result_suite` (ports contract module); honors_script
  machinery removed; prime double reduced to the compile-only document_for
  arm; mock learned ExitWithoutResult (behavior + script table + advance) and
  its contract test runs both suites; controller joins MissingResult into the
  row-2 arm with the adjudicated rationale; RunnerProbeKind added to
  RunnerDescriptor with kind-rendered evidence (RES-D6); doctor test
  construction sites updated (kind: VersionRange).
- [t2] gauntlet-adapter-agent-cli implemented: template parser (RES-D8
  refusal set, no expansion, # comments, whole-word placeholders),
  ProcessTransport seam + ProcessChildTransport (env channel RES-D3,
  SIGTERM→wait→SIGKILL stop, try_wait reaping), prompt file
  (gauntlet-cli-agent-prompt@1, RES-D2 envelope sibling reference), runner
  (total-order classification RES-D8a/b/c, forensic handle file RES-D12,
  retention, injected clock RES-D9, honest capabilities RES-D7),
  testing.rs (script doubles parse the REAL prompt file; CliRunnerFactory
  with injected template literal RES-D10). Package check green; 18 package
  tests.
- NEXT: cli crate integration tests (contract suite + shape-2 suite), mock
  + prime package tests, doctor mirror, arch scan, p8, live probe, ladder.

- [t3] Full ladder GREEN: `just check` PASS; `just test` 1150/1150; `just
  lint` PASS (clippy -D warnings + fmt clean); `just deny` PASS; `just
  boundaries` 224/224 (incl. the new whole-crate cli-adapter vendor scan +
  vacuity controls); `just doctor` exit OK; `just ruler` PASS; gate line
  `cargo nextest run --test p8_adapter_substitution` PASS (both factories,
  shared suite + shape-2 sibling, zero per-adapter test bodies).
- [t4] Live probe leg (GAUNTLET_CLI_AGENT_LIVE_PROBE=1): PASS first run,
  0.257s — one real `sh` child via ProcessChildTransport (session_ref
  `cli-child:1754090`, a real pid), one Running poll then Completed decided
  by the real contract file, collect returned the matching document,
  idempotent cancel x2, stranger refused. Hygiene: scratch removed, nothing
  under .gauntlet/. (A second run with --success-output=immediate captured
  the evidence lines verbatim; both runs passed — not a retry into green.)
- NEXT: teeth demonstrations (vendor plant -> boundaries RED -> byte-identical
  restore; TimedOut mutation -> shape-2 suite RED -> byte-identical restore),
  then final report.

- [t5] Teeth demonstrated RED→restore→GREEN, sha256-verified byte-identical:
  (a) `pub const VendorDemoPlant: &str = "anthropic";` planted in the cli
  crate public API ⇒ `just boundaries` RED
  (`the_generic_cli_adapter_crate_carries_no_vendor_identifiers`, finding
  names lib.rs:100 'anthropic') ⇒ restore (sha c1bc1d53…) ⇒ 224/224 GREEN.
  (b) classification mutation (clean-exit-no-result ⇒ TimedOut) ⇒
  `the_cli_agent_runner_passes_the_exit_without_result_suite` RED with 3
  findings (wrong verdict + 2 stability) ⇒ restore (sha abf77d91…) ⇒ cli
  package 21/21 GREEN.
- [t6] Final gates re-verified on the clean tree: p8 gate line PASS; ruler
  PASS. Final report written to `b1-builder.md` (deliverable verdicts, gate
  table, touch surface, deviations/fog, teeth, REBIRTH per-item notes,
  not-done list). B1 COMPLETE — awaiting independent verification.
