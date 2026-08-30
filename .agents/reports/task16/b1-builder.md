# Task 16 — B1 Builder Report (Generic CLI agent adapter + P8 substitution proof)

Builder: B1 (sole builder, held the compile lock). Date: 2026-08-29. Baseline
HEAD 054176b. All 12 mid-flight design corrections (RES-D1..D12) were
integrated; where they superseded the brief, the supersession is named below.

---

## 1. Verdict per deliverable

| # | Deliverable | Verdict | Evidence |
|---|---|---|---|
| 1 | `crates/adapters/gauntlet-adapter-agent-cli`: command template from `GAUNTLET_CLI_AGENT_CMD`, `{prompt_file}` `{workspace}` `{result_path}` `{model}` placeholders, no vendor name anywhere in the crate | **DONE** | Shell-word PARSING into argv (POSIX rules; no `sh -c`, no expansion of `$VAR`/`$(…)`/globs; `#` comments at word starts; newlines flatten). Placeholders must be the WHOLE argv word; fused/unknown/dangling forms are typed errors (`template.rs`). Env var absent ⇒ typed `ConfigError::MissingCommand`; no default template exists on purpose. Whole-crate vendor scan (teeth, RES-D5) GREEN; vendor plant shown RED. |
| 2 | `poll` checks process state AND the result contract; clean exit without a valid result ⇒ `Failed{missing_result}` | **DONE** | Total order in `poll` (RES-D8a): contract read → exited-without-valid-document ⇒ `Failed{missing_result}` (before any deadline; exit-triggered final re-read closes the write/exit race; signal-death and nonzero exit collapse to the same verdict, RES-D8c) → deadline ⇒ `TimedOut` → `Running`. Exit status recorded in adapter-local forensics only (RES-D8b). Pinned by the ports' `exit_without_result_suite` (RES-D1's sibling-suite shape) against mock + cli; demonstrated RED on the TimedOut mutation. |
| 3 | Proof suite parameterized over the runner adapter; adapter selection in the test harness | **DONE** | `crates/gauntlet-cli/tests/p8_adapter_substitution.rs`: ONE loop body runs `agent_runner_suite` + `exit_without_result_suite` over every selected factory; zero per-adapter test bodies. Selection: `GAUNTLET_P8_ADAPTER=mock|cli|both` (default `both`); unknown value panics (fail-closed). DEVIATION from the plan's literal `--adapter` flag recorded below (D-dev-1). |
| 4 | `p8_adapter_substitution` runs the full suite against mock + one real adapter | **DONE** | Gate line `cargo nextest run --test p8_adapter_substitution` PASS (10-11 ms); default run asserts ≥2 adapters selected, one labeled `cli-agent-adapter` (non-mock), one `mock-agent-runner`. |
| 5 | Done-when: same suite passes against two adapters with zero test-body changes | **DONE** | Mock: 70/70 package tests (incl. both suites); cli: 21/21 (incl. both suites); both factories flow through the identical loop body in p8. |
| 6 | Done-when: vendor identifier in the adapter's public API fails `just boundaries` (teeth; AK-013/AK-014) | **DONE** | RES-D5 shape (whole-crate text scan of the ONE crate over existing `DENIED_TERMS`, zero exemptions, no new term list, anti-vacuity built in). Plant `pub const VendorDemoPlant: &str = "anthropic";` ⇒ `just boundaries` RED (test + finding verbatim in §5); byte-identical restore (sha256-verified) ⇒ 224/224 GREEN. |

Additional in-scope work delivered: ports revision (`InvocationFailure::MissingResult`
+ Display token `missing_result`); `RunnerScript::ExitWithoutResult` variant;
`exit_without_result_suite` as an EXPORTED SIBLING (not inside
`agent_runner_suite`, per RES-D1); doctor mirror for the cli runner with the
typed configuration probe kind (RES-D6); env-gated live probe (PASSED, §4);
forensic handle record, retention, SIGTERM→wait→SIGKILL cancel, non-blocking
reaping (RES-D12); envelope-body sibling + `GAUNTLET_RESULT_PATH` env channel
(RES-D2/D3).

## 2. Gates run (all on the final tree unless noted)

| Gate | Result | Verbatim summary |
|---|---|---|
| `just check-package gauntlet-adapter-agent-cli` | PASS | `Finished dev profile` (0 warnings) |
| `just test-package gauntlet-adapter-agent-cli` | PASS | `21 tests run: 21 passed, 0 skipped` |
| `just check-package gauntlet-ports` / `just test-package gauntlet-ports`* | PASS | ports covered by workspace run below after revision |
| `just test-package gauntlet-adapter-agent-mock` | PASS | `70 tests run: 70 passed, 0 skipped` |
| `just test-package gauntlet-adapter-agent-prime` | PASS | `34 tests run: 34 passed, 0 skipped` (suite stays byte-green; double change is the compile-only `document_for` arm) |
| `just check-package gauntlet-app` + `just test-package gauntlet-app` | PASS | `350 tests run: 350 passed` (doctor evidence re-derivation holds: VersionRange kind keeps the pinned "version compatibility" phrases) |
| `just check` | PASS | `Finished dev profile` |
| `just test` | PASS | `1150 tests run: 1150 passed, 0 skipped` |
| `just lint` | PASS | clippy `-D warnings` clean; `cargo fmt --check` clean |
| `just deny` | PASS | `advisories ok, bans ok, licenses ok, sources ok` |
| `just boundaries` | PASS | `224 tests run: 224 passed, 0 skipped` (and shown RED under the plant, §5) |
| `just doctor` | PASS | `exit: OK`; report at `.agents/reports/doctor.json`; check 3/4 unchanged for the conservative posture (no `runners.default` configured — UNK-003 untouched) |
| `just ruler` | PASS | `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock` (run at start and end) |
| Gate line: `cargo nextest run --test p8_adapter_substitution` | PASS | `1 passed; 0 failed` |
| Live leg: `GAUNTLET_CLI_AGENT_LIVE_PROBE=1 … --test live_probe` | PASS | First run `0.257s`; evidence lines: `admitted: invocation 01ARZ3NDEKTSV4RRFFQ69G5FAV with session_ref Some(SessionRef { raw: "cli-child:1754090" })` (real pid), one `poll: Running {…}` then Completed, `collected: state=Completed role=Builder`, `live probe finished in 251.609665ms`. A second run with `--success-output=immediate` captured the evidence lines (also PASS — evidence capture, not a retry into green). Hygiene: temp scratch removed; `ls .gauntlet/` shows only the pre-existing `comparison-contract.yaml`, `policies`, `reference.lock`. |

\* ports/app were exercised via the workspace run; package-scoped runs were
used during development.

## 3. Touch surface (exact)

**New files**
- `crates/adapters/gauntlet-adapter-agent-cli/src/{clock,config,prompt,runner,template,transport,testing}.rs`
- `crates/adapters/gauntlet-adapter-agent-cli/tests/{contract_suite,live_probe}.rs`
- `crates/gauntlet-cli/tests/p8_adapter_substitution.rs`
- `crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs` (rewritten from the
  doc-only stub — same file, new content)

**Modified (surgical)**
- `crates/gauntlet-ports/src/agent_runner.rs` — `InvocationFailure::MissingResult` + Display arm + doc (only)
- `crates/gauntlet-ports/src/contract/agent_runner.rs` — PURELY ADDITIVE (git diff: zero removed lines): `RunnerScript::ExitWithoutResult`, `exit_without_result_suite` (exported sibling, NOT registered in `agent_runner_suite` per RES-D1), `STABILITY_POLLS` const
- `crates/gauntlet-ports/src/contract/stubs.rs` — `ScriptedRunner::advance` arm for the new variant (compile-functional, RES-D1's consumer list) + import line reflow
- `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs` — `ChildBehavior::ExitWithoutResult` (variant, token `exit-without-result`, parser arm refusing every writing key AND the deadline knob) — "scenario vocabulary" per RES-D1
- `crates/adapters/gauntlet-adapter-agent-mock/src/runner.rs` — script-table arm + `advance` arm pinning `Failed{MissingResult}` on first probe
- `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs` — added `the_mock_runner_passes_the_exit_without_result_suite` (RES-D1: the sibling suite runs in each satisfier's own contract test)
- `crates/adapters/gauntlet-adapter-agent-prime/tests/contract_suite.rs` — TESTS-ONLY compile arm: `document_for` merges `ExitWithoutResult` into the never-writes arm
- `crates/gauntlet-app/src/control/controller.rs` — RES-D4: `MissingResult` joins `OrphanedByRestart | Execution` in the existing §11.5 row-2 backoff-retry arm, with the adjudicated rationale comment (file added to touch surface for this arm ONLY, per RES-D4)
- `crates/gauntlet-app/src/doctor/checks.rs` — RES-D6: typed `RunnerProbeKind` (`VersionRange` | `Configuration`); `RunnerDescriptor.kind`; the two evidence sites (+ the check-3 pass site) render the kind via `clean_phrase`/`failed_phrase`/`capability_phrase`; doc re-derived honestly (a configuration probe claims no version measurement)
- `crates/gauntlet-app/src/doctor/mod.rs` — the three test descriptor constructions gain `kind: VersionRange` (behavior-identical; no expectation weakened)
- `crates/gauntlet-cli/src/main.rs` — `measure_runner` gains the cli leg: token match on `RUNNER_TOKEN`, env read of `COMMAND_ENV_VAR`, typed `parse_template` probe, descriptor kind `Configuration` (RES-D6/D9); capability-named items only
- `crates/gauntlet-cli/Cargo.toml` — production dep `gauntlet-adapter-agent-cli.workspace = true` (doctor measures at runtime; prime precedent)
- `crates/gauntlet-arch-tests/src/denylist.rs` — RES-D5, strictly additive: `GENERIC_CLI_ADAPTER_DIR`, `scan_generic_cli_adapter`, `check_generic_cli_adapter_sources`, `use std::path::{Path, PathBuf}`, plus new tests (`the_generic_cli_adapter_crate_carries_no_vendor_identifiers`, `every_denied_term_is_flagged_in_the_cli_adapter_scan`, `the_cli_adapter_scan_is_case_insensitive_and_whole_file`, `a_vacuous_cli_adapter_scan_fails_rather_than_passing`)
- `crates/gauntlet-arch-tests/src/lib.rs` — `pub use denylist::check_generic_cli_adapter_sources;` + doc (additive wiring)
- `Cargo.lock` — regenerated by cargo (no new external dependency; path-dep metadata only)

**Not touched** (verified): `.wayfinder/**`, `.gauntlet/reference.lock`, answer
key, existing `agent_runner_suite` case bodies (zero removed lines in the
file), existing arch scanners/Roles/masks/allowlists, prime's runner,
governance types, `wrap_builder_command`, controller wiring.

## 4. Recorded deviations, fog, and decisions

- **D-dev-1 (`--adapter` flag):** the plan's literal test-harness flag has no
  precedent in this repo's nextest usage (no test parses CLI args; the gate
  line must work unchanged). Implemented env-var selection
  `GAUNTLET_P8_ADAPTER=mock|cli|both` (default `both`; unknown value FAILS).
  This was pre-authorized by the brief (decision 10) and by RES-D10's
  framing ("`GAUNTLET_P8_ADAPTER` selects the battery only"). The plan's
  `--adapter` wording remains a locator-class divergence for the closure
  record.
- **D-dev-2 (SIGTERM leg, RES-D12):** std exposes no signal API; the
  graceful leg sends SIGTERM via a FIXED-ARGV helper
  (`sh -c 'kill -TERM "$1"' sh <pid>` — pid as an argv argument, no
  operator data in the command, no interpolation), bounded wait, then
  `Child::kill()` (SIGKILL) and a bounded reap. A `libc`/`nix` dependency
  would be the clean way and remains an ask-first decision for the
  orchestrator. Kill-after-exit maps to success; a second cancel never
  reaches the seam (typed no-op at the runner).
- **D-dev-3 (shape-2 + prime):** resolved per RES-D1 — the shape-2 rule is
  NOT universal: prime's one-input poll (contract file + deadline, Annex
  C.4) cannot observe an exit, so the case lives in the exported sibling
  suite run against mock + cli only; prime's double needed only a
  compile-only arm. My earlier `honors_script` machinery was removed when
  RES-D1 landed.
- **D-dev-4 (controller disposition):** spec §11.5 names no `missing_result`
  row; the adjudicated RES-D4 mapping (row-2 transient, backoff retry) is
  implemented with the rationale comment. If the operator wants a distinct
  disposition (e.g. TimedOut-row retry-once), that is a one-arm change in
  `dispose_failure`.
- **D-dev-5 (scanner self-catch):** during integration, the EXISTING
  AK-002/AK-014 derived-identifier scan flagged my own new ports doc
  comments spelling the adapter identifier (`cli-agent`) — evidence the
  settled instruments have teeth. Fixed by rewording the new ports docs to
  "the §4.1 command-template runner" (the rule was not touched).
- **D-dev-6 (instrument friction, recorded not worked around):** the
  literals scanner's statement-join units flag any value-position numeric
  in the same region as poll/deadline vocabulary; the new suite's stability
  loop therefore uses a named constant and the strings avoid
  comma-introduced numerics. Same class as the suite's existing
  numeric-literal-on-its-own-line posture.
- **Fog carried (not implemented, per scope fences):** no controller wiring,
  no WorkspaceProvider (the runner takes the workspace root injected; R-1b
  stays open), no `runners.default` selection (UNK-003), R-2/R-3 untouched
  (this adapter consumes a TEMPLATE, never a wrapped `SandboxedCommand` —
  prereg D3), T15-R1..R5 untouched, environment credential pruning of the
  child env is task20's (RES-D3 posture documented in `transport.rs`).
- **AK-013 locator divergence** (`tests/proofs/p8_adapter_substitution` vs
  delivered `crates/gauntlet-cli/tests/p8_adapter_substitution.rs`):
  handled by the prereg's D5 — a reference-candidates entry at closure,
  which per RES-D11 is the ORCHESTRATOR's closure work, not mine.

## 5. Teeth demonstrations (both RED before GREEN, final tree clean)

**(a) Vendor identifier in the cli crate's public API ⇒ `just boundaries` RED.**
Plant (appended to `crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs`):
`pub const VendorDemoPlant: &str = "anthropic";`
- pre-plant sha256: `c1bc1d53c30520d769d19938345bb3a1407de91788b57d93e0a46c13cea1a8e4`
- RED: `just boundaries` failed — test
  `denylist::repo::the_generic_cli_adapter_crate_carries_no_vendor_identifiers`,
  finding verbatim:
  `denied-cli-adapter-identifier: crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs:100: 'anthropic' — the generic CLI adapter crate must carry no vendor identifier anywhere in its surface; selection is configuration, not code (plan Task 16; spec §4.1, §24.3)`
  (`Summary: 13/224 tests run: 12 passed, 1 failed, 0 skipped`)
- restore: sha256 after restore matches pre-plant exactly (`c1bc1d53…`)
- GREEN: `just boundaries` → `224 tests run: 224 passed, 0 skipped`

**(b) Clean-exit-without-result mis-classified ⇒ the shape-2 suite RED.**
Mutation (in `crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs`, the
`Exited` branch of `poll`): replaced the
`Failed{MissingResult}` pin with `Self::pin(entry, InvocationStatus::TimedOut)`.
- pre-mutation sha256: `abf77d91e0e3a587ccf075f7aa03e3e0bf36f0c6dd290fe90e8579e5e9fed21e`
- RED: test
  `gauntlet-adapter-agent-cli::contract_suite the_cli_agent_runner_passes_the_exit_without_result_suite`
  failed with THREE findings, including verbatim:
  `[agent_runner::exit_without_result_is_failed_missing_result] … an exited child was reported TimedOut — the adapter polled a dead child to the deadline instead of classifying the observed exit (a dead child can never write)`
  plus the two stability findings ("poll after the missing_result verdict reported TimedOut on attempt …").
- restore: sha256 after restore matches pre-mutation exactly (`abf77d91…`)
- GREEN: `just test-package gauntlet-adapter-agent-cli` → `21 tests run: 21 passed, 0 skipped`

Additionally, the mock + p8 legs of the shape-2 rule were exercised on every
run of `just test` (mock's own suite test + p8 over both factories).

## 6. Arch-suite standing trigger (REBIRTH) — per-item notes

The arch suite carries a standing trigger (CURRENT_STATUS unresolved_summary:
"Arch-suite standing trigger REBIRTHS at the next arch-suite touch"). My
arch-suite change is strictly ADDITIVE (two new scanner functions + one
re-export + new tests; zero modifications to existing functions, Roles,
masks, allowlists, or pre-existing tests — verified by review and by the
green unchanged pins). Per-item disposition:

- **E4/F1/F3** (prior arch-suite finding classes): untouched — no scanner
  matching logic, region lexing, or normalization was modified; their pinned
  tests re-ran GREEN unchanged in this task's `just boundaries` runs.
- **B.5a masked spellings** (single_writer masked-spelling residuals):
  untouched — `single_writer.rs` not opened; `check_answer_key_single_writer`
  GREEN unchanged.
- **A5.2 allowlist granularity** (deps layer external-allowlist decisions):
  untouched — `deps.rs` not opened; no allowlist entry added or removed;
  `check_dependency_direction` GREEN unchanged. (The cli adapter gained NO
  external dependencies; `Cargo.lock` changed only by path-dep metadata.)
- **B-F16 shapes** (Annex-B wire-key mask byte-shapes and the D4
  split-argument residual): untouched — `ANNEX_B_CONTRACT_PARSERS`,
  `ANNEX_B_WIRE_KEYS`, the mask gates, and their pin tests
  (`annex_b_*`) re-ran GREEN unchanged.

## 7. Honest statement of what was NOT done

- The live probe ran against a trivial `sh` probe child — NOT against any
  real agent binary or model (explicit non-claim; "the mechanism travels",
  never "every CLI works").
- No real builder executed anywhere (R-1b intact): the batteries run over
  in-memory process doubles; the only real processes are the live probe
  child and three small in-crate transport unit tests (`sh -c 'sleep 30'`,
  `exit 3`, and the RESULT_PATH observer — bounded, no gate).
- Doctor's cli leg was exercised via `just doctor`'s green run (no
  `runners.default` configured, so `measure_runner`'s cli branch is
  composition-tested only through the code path + package tests; no doctor
  unit test pins the Configuration-kind evidence text — the
  `RunnerProbeKind` phrases are covered by the app package tests for the
  VersionRange kind, and the Configuration phrases are compile-wired. This
  is a residual: a doctor unit test asserting the Configuration-kind
  evidence wording would strengthen RES-D6's guarantee.)
- Process-tree exotica (daemonized grandchildren surviving kill,
  process-group signaling) are out of scope — recorded as the prereg's
  known blindspot; the stop covers the direct child only.
- The prereg's P1/P2 predictions (first-render interpolates; first
  classification wrong) did NOT fire as predicted — the template parser and
  total order were designed against those failure classes from the start;
  the falsifier should treat the pre-built refusal set as the attack
  surface, not evidence of absence.

## 8. Checkpoint cross-reference

Progress log: `.agents/reports/task16/b1-progress.md` (appended at each
milestone, including the RES-D routed acknowledgment). This file is the
result contract; the final message to the orchestrator is a signal only.
