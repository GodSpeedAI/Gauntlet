# Task 16 — falsifier round 1 (EXECUTING)

**VERDICT: NOT_CONFIRM** (4 findings stood: F1 major, F2 major, F3 minor,
F4 major — all corrected in `correction-round1.md`; every corrected tooth
now GREEN).

## Round provenance (honest record)

- The round was dispatched as a fresh BLIND executing falsifier
  (charter verbatim, firewalled from all builder/design reports).
- The falsifier agent DIED at ~17 minutes (sub-agent 5-hour usage window
  exhausted; reset 2026-08-30T12:39). Before dying it had: read the full
  surface + spec anchors (§7.3, §10.3, §17.3, §18.3, Annex B; AK-013/014),
  run the cli package suite and the p8 gate (both PASS), and COMPOSED two
  executable attack files — `falsifier_tmp_template.rs` (A1–A13) and
  `falsifier_tmp_live.rs` (B/C/D/F/H/J legs) — the latter written but
  NEVER compiled or run.
- Per the operator's standing heal+continue rule, the ORCHESTRATOR
  completed the round: recovered both files, repaired their harness bugs
  (single-poll race after invoke; `with_extension` replaces `.json`
  instead of suffixing; one wrong expectation in B4 that contradicted the
  pinned shape-2 order the falsifier itself attacked — an exited
  wrong-path child is `Failed{missing_result}` before any deadline, not
  `TimedOut`), executed all attacks, adjudicated findings, and implemented
  the corrections. BLINDNESS IS DEGRADED for the orchestrator-executed
  portion (it had read the builder/adjudication reports); mitigations:
  the attacks themselves were composed in blindness by the fallen agent,
  and the orchestrator added fresh attacks from the declared floor (p8
  fail-closed, arch-scan evasion payloads, doctor honesty legs) rather
  than replaying known findings. Recorded per the task13/15
  interruption-recovery discipline.
- The recovered attack files are RETAINED in-tree as the round's
  regression teeth: `tests/falsifier_round1_template_attacks.rs`,
  `tests/falsifier_round1_live_attacks.rs`.

## Executed attacks and outcomes

Template render (real parser, 14 tests — all REPELLED except A4b):

| Id | Attack | Outcome |
|---|---|---|
| A1 | unknown placeholders + case tricks (`{fabricated}`, `{PROMPT_FILE}`, `{}`, `{model}}`, `{{model}}`, `{a{model}}`) | REPELLED — typed UnknownPlaceholder/DanglingBrace/EmbeddedPlaceholder |
| A2 | fused placeholders (`{result_path}.txt`, `--out={result_path}`, `x{model}y`, quote-fused, adjacent) | REPELLED — EmbeddedPlaceholder refusals; no value can splice into operator grammar |
| A3 | quoted placeholders | REPELLED — quotes group into the placeholder; values bind as whole words |
| A4 | empty / whitespace / CRLF-only templates | REPELLED — `TemplateError::Empty` |
| A4b | comment-only template (zero words) | **BROKE (F2)** — parsed clean while every admission refuses (`EmptyProgram`): doctor's configuration probe reported usable for a template that can never admit |
| A5 | unbalanced quotes, trailing backslash | REPELLED |
| A6 | shell grammar in template (`$AGENT`, `$(rm -rf /)`, backticks, globs, `;` `&&` `\|`) | REPELLED — argv DATA, never grammar |
| A7 | hostile substituted VALUES (spaces, `;`, `&&`, `$()`, backticks, quotes, newlines in workspace/model) | REPELLED — single verbatim argv words, `argv.len() == 5` |
| A8 | legal-but-odd shapes (`{prompt_file}` alone; `{workspace}` as argv[0]; `'' {prompt_file}`) | REPELLED (renders/refuses typed) — but see F2's class: parse-only doctor passes `'' {prompt_file}` too; closed by the same render-level probe |
| A9 | `{model}` with no hint | REPELLED — `MissingValue` at render, never a silent empty arg |
| A10 | duplicated placeholders | REPELLED — same value twice |
| A11 | CRLF separators | REPELLED — no `\r` leaks into words |
| A12 | backslash inside double quotes (POSIX-narrow escapes) | REPELLED — literal, both chars kept |
| A13 | `#` mid-word vs word-start (comment to end of LINE only) | REPELLED |

Classification / process lifecycle / channels (real `ProcessChildTransport`,
18 tests — REPELLED except F1, F4; F3 was a doc finding exposed by the F leg):

| Id | Attack | Outcome |
|---|---|---|
| B1 | contract wins over a hostile exit 7; process state never consulted on this path (forensics exit_status stays None); collect works | REPELLED |
| B2/B3 | garbage and EMPTY documents | REPELLED — `Failed{malformed_result}` on first terminal poll; never collectible |
| B4 | child writes to a DIFFERENT path, then exits | REPELLED — `Failed{missing_result}` BEFORE the deadline (the falsifier's own scripted expectation of TimedOut was corrected to the pinned shape-2 order) |
| B4b | child RUNNING and silent past the deadline | REPELLED — `TimedOut` + the timeout path STOPS the child (pid verified dead); post-terminal stable; `payment_so_far` zero |
| B5 | foreign invocation_id; stale envelope digest | REPELLED — `Failed{contract_mismatch}` through the real transport |
| B7 | a DIRECTORY at the contract path | REPELLED — typed observation error, never coerced Running; persists; no terminal pinned |
| B8 | valid matching contract written AFTER the missing_result verdict | REPELLED — late writer cannot resurrect (3 stability polls) |
| B14 | spawn failure (nonexistent program) | REPELLED — typed `Transport` at admission |
| B15 | deadline boundary exactness (injected stepping clock) | REPELLED — now == deadline still Running (strict), next instant TimedOut |
| C(a-d) | cancel lifecycle with REAL children: live cancel stops the child under the caller's bound; second cancel = typed AlreadyTerminal; stranger handle = UnknownHandle; cancel on an already-exited unpolled child succeeds then polls Cancelled | REPELLED |
| C-sig | child that `trap '' TERM`-ignores SIGTERM | **BROKE (F1)** — SIGKILL landed but the escalation leg returned `CancelFailed{"not reaped within the bound"}`: the graceful leg had consumed the whole caller bound, giving the reap leg zero time; the Cancelled verdict was at risk of being replaced by missing_result on the retry poll (a deliberately cancelled run re-classified as a transient agent failure) |
| C-hang | child writes the contract then HANGS | **BROKE (F4)** — Completed pinned correctly, but the live child leaked forever; cancel unreachable (AlreadyTerminal) |
| C-2 | two invocations on ONE runner: duplicate admission refused, original intact; no cross-wiring (each collects its OWN document) | REPELLED |
| C-forged | forged handle poll/collect/cancel | REPELLED — UnknownHandle everywhere |
| D | §10.3 rule-1 channels executed: `GAUNTLET_RESULT_PATH` in the child env == declared path; prompt carries RESULT-PATH; `MODEL:` empty (not a lie) with no hint; envelope BODY sibling byte-exact with the runtime-carried body | REPELLED |
| F | lib.rs doc claims the contract is read "in exactly two places — poll and collect"; executed: collect serves the VALIDATED poll snapshot and never re-reads (post-completion tamper cannot change collection — the BEHAVIOR is right) | **BROKE (F3, minor, doc-only)** |
| H | §18.3 orphan story: forensic handle file survives runtime death, parseable, cites the REAL child pid; a fresh runner instance refuses the stale handle (no faked knowledge) | REPELLED |
| J | capabilities honesty with a `{model}` template: supports_model_selection exactly when configured; isolation none; delivery file; no native subagents | REPELLED |

Fresh orchestrator-executed attacks (completing the declared floor):

| Id | Attack | Outcome |
|---|---|---|
| P8-fc | `GAUNTLET_P8_ADAPTER=bogus` | REPELLED — the proof FAILS loudly (never a silently narrowed battery); `=cli` and default `both` PASS |
| ARCH-1 | vendor terms planted in a cli SUBMODULE file, in UPPERCASE, and string-concatenated (`"op" "enai"`) | REPELLED — whole-crate scan flagged all 3 (`claude`, `anthropic`, `openai` at transport.rs:412); byte-identical restore → 224/224 |
| ARCH-2 | CONTROL: the same plant in ANOTHER adapter's crate (mock) | REPELLED — no false positive (the scan is scoped to the cli crate only) |
| DOC-1 | doctor with a GOOD template + `GAUNTLET_AGENT_RUNNER=cli-agent` | CONFIRMED honest — "the typed configuration probe measured clean (… configuration only, no binary probed and no version claimed)" |
| DOC-2 | doctor with the F2 template (comment-only) | CONFIRMED CORRECTED — check 3 FAILS honestly ("the command template is empty"), never a green pass for an unadmittable runner |
| DOC-3 | doctor with `'' {prompt_file}` (parses, render-refuses) | CONFIRMED CORRECTED — check 3 FAILS (the probe renders, closing the whole class) |

## Findings

- **F1 (major, security/lifecycle — corrected COR-F1):** cancel escalation
  returned `CancelFailed` for a landed SIGKILL with a pending reap (bound
  consumed by the graceful leg), risking the deliberate Cancelled verdict
  being re-classified `Failed{missing_result}` (a cancelled run treated as
  a retryable agent failure). Locus: `transport.rs` escalation reap loop.
- **F2 (major, evidence/honesty — corrected COR-F2):** a zero-word
  (comment-only) template parsed clean, so doctor's configuration probe
  reported a usable runner that can never admit; `'' {prompt_file}`
  shared the class. Locus: `template.rs parse_template`, doctor probe.
- **F3 (minor, evidence/doc — corrected COR-F3):** lib.rs misdescribed the
  contract-read sites ("two places — poll and collect"); the actual
  collect-serves-validated-snapshot behavior is correct and tamper-proof.
  Locus: `lib.rs` module doc.
- **F4 (major, resource/lifecycle — corrected COR-F4):** a terminal pin
  (Completed/malformed/mismatch) left a LIVE child running forever with
  cancel unreachable; no invocation must outlive its verdict with
  authority. Locus: `runner.rs` pin paths.

## Teeth

- The builder's two teeth were re-demonstrated this round in the ARCH-1
  leg (vendor plant → `just boundaries` RED naming file:line →
  byte-identical restore, sha256-verified → 224/224 GREEN).
- The corrections' teeth are inherent: a4b, c_sigterm, and c_hang were
  executed RED on the pre-correction tree (29/32 with exactly those three
  failing) and GREEN after the corrections landed — the tests catch the
  defect classes they pin, in the direction that matters.

## Not attacked (honest scope)

- No full mutation run (task 24's instrument); no daemonized-grandchild /
  process-group exotica (recorded blindspot); no real agent binary or
  model (explicit non-claim); no dispatch-gate composition (R-1b fence);
  BudgetMeter/observer/comparator surfaces (untouched by this task);
  concurrency >2 invocations (two-instance leg only).

## Hygiene

- All scratch children killed (sweeps for `sleep 121`/probe processes:
  clean); /tmp scratch dirs from failed test batches removed at cleanup;
  planted arch payloads restored byte-identical (sha256 -c OK);
  `ls .gauntlet/` shows only `comparison-contract.yaml`, `policies`,
  `reference.lock`; `just ruler` PASS post-round.
- NOTE recorded for the next hygiene pass: stale `/tmp/gauntlet-falsifier-*`
  scratch dirs from a PREVIOUS task (net-escape/file-smuggle/
  forged-admission/irreversible/share-net/flag-smuggle) were found and
  left for a dated cleanup decision (not this task's surface; nothing
  under `.gauntlet/`).
