# Task 27 — falsifier round 3 (focused, execution report)

VERDICT: **CONFIRM — the corrected exit-code classification scheme holds.**
No state collapses anywhere: every executed shape lands on its intended
side (genuine failure ⇒ FAIL; executor/inner inability ⇒ INCONCLUSIVE;
INCONCLUSIVE-only sweeps exit 0, FAIL sweeps exit 1 — charter decision 3
intact end-to-end). Four findings, none MAJOR, each with an executed
reproduction: **R3-F1 (MINOR)** a chosen inner exit inside 129..159 is
reported "killed by signal N" — a false factual claim — and this
pass-through direction is NOT in the disclosure (disclosure-gap,
pre-adjudicated MINOR: the direction is INCONCLUSIVE-not-FAIL);
**R3-F2 (MINOR)** the signal-0 corner (a genuine inner exit 200) reports
"killed by signal 0 (the wrapper's shell encoded it as 128)" — doubly
impossible; the 200..231 class IS disclosed, the reason text is not;
**R3-F3 (COSMETIC)** the kind-arm refusal text still advertises the
bounded recorded-artifact form to log-kind authors, now test_report-only
(behavior correct, executed both directions); **R3-F4 (MINOR)** an
artifact-only sweep into a not-yet-existing report parent adjudicates
fully then dies at the report write (os error 2) — the write path creates
directories only for evidence files, and an artifact-only run has none
(fail-explicit, no dishonesty). Band boundaries, the 10s stop bound, the
daemon invariant, and the full classification table all CONFIRMED through
the REAL wrapper + REAL binary; regression matrix green (adapter 13/13 in
4.0s — round 2's 269s stall gone; cli 9/9; docs_present 7/7; ruler PASS;
DuplicateId prose clean).

Status: COMPLETE (round 3, narrow scope).

Falsifier: blind executing adversary, round 3. Attacked ONLY the corrected
classification scheme (COR-4 R2-F1/R2-F2/R2-F3, commit 9350a43) and its
immediate adjacencies. Normative: charter decision 3. Blind: b1/b2/b3
reports, progress files, .agents/agents/, .agents/prompts/ not read.

## Target map (read, normative inputs only)

- `crates/adapters/gauntlet-adapter-verify-command-exit-code/src/sweep_toolchain.rs`
  — reserved pair {86, 69} lock-level; wrapper script remaps inner
  shell-status 129..159 → 200+signal FIRST, then inner own 69→169 / 86→186;
  `classify_exit` pure table (200..231 ⇒ InnerSignal); `timeout 10 sccache
  --stop-server || true` inside the locked script; behavioral teeth.
- `crates/gauntlet-cli/src/verify_cmd.rs` — the executor: locked-arm
  classification through `classify_exit`; CannotRun for LockBusy /
  LockExecFailed / InnerSignal (naming the signal and the shell's 128+n);
  remapped InnerFailure carries an exit_note naming the remap AND the
  indistinguishability; bare-program arms own their codes; COR-3 F10
  no-code path.
- `crates/gauntlet-app/src/conformance/mod.rs` — FAIL reason =
  `command exited N[ — exit_note]`; CannotRun ⇒ INCONCLUSIVE
  `cannot_run: …`; evidence logs only for completed commands.
- `crates/gauntlet-app/src/conformance/report.rs` — DISCLOSURES (the
  honesty surface adjudicated in R3-F1/R3-F2).
- `crates/gauntlet-app/src/conformance/manifest.rs:609-656` — the kind/arm
  matrix (`bounded_artifact` = test_report-only) and its refusal text.

## Executed harness

Fixture worlds under /tmp/gauntlet-falsifier3 driven by the REAL binary
(`target/debug/gauntlet`, rebuilt from HEAD = 9350a43 under the compile
lock before attacking: fresh 0.63s, binary mtime after every touched
source) via `PATH=<stub-dir> GAUNTLET_PROJECT_ROOT=<world> gauntlet verify
--all --manifest manifest.yaml --report out/<case>.json`, cwd = the repo.
The classification world's single locked arm is `execute: {package:
gauntlet-domain, lib: true, filter: falsifier_probe}`; a stub `cargo`
first on PATH is env-driven (FALSIFIER_EXIT / FALSIFIER_SIGNAL /
FALSIFIER_SLEEP), so every case runs the REAL wrapper (`flock -w 3600 -E
86 /tmp/gauntlet-build.lock -c <generated script>`) end-to-end. Worlds
carry a fixture ruler at `.wayfinder/answer-key.md` (the strict parser
refuses any non-AK `#` line — first generator draft refused, fixed; also
learned: the manifest's frozen_locator is cross-checked against the
ruler's evidence_required locator — a drifted control world was REFUSED,
an incidental strengthening over the round-2 target map) and a detached
`.git/HEAD`. Daemon discipline per case: `sccache --start-server` +
pgrep-wait before, pgrep assert after.

## Findings

### R3-F1. MINOR — an inner command that genuinely EXITS inside 129..159 (no signal) is reported with a FALSE factual claim ("killed by signal N") and the direction is NOT disclosed

- Executed: world-classify, stub `exit 130` (a chosen exit; no signal is
  delivered anywhere). Real binary, real wrapper:
  `AK-930 INCONCLUSIVE unknown | reason: cannot_run: the sweep command was
  killed by signal 2 (the wrapper's shell encoded it as 130; the band remap
  carried the signal through)`; sweep exit 0; commands_executed 0
  (case exit130).
- Nothing was killed — the command chose 130 — yet the reason asserts a
  signal death as fact. Contrast the remapped-169/186 path, whose
  exit_note honestly says "a direct exit of N is indistinguishable from
  the remapped form (disclosed)". The 129..159 band carries no such note.
- Disclosure adjudication: the report's `_meta` lock disclosure names the
  indistinguishable pass-throughs as "169, 186, or 200..231" — the
  129..159 chosen-exit direction is NOT named; and `sweep_toolchain.rs`'s
  module doc asserts the band means "the inner command was KILLED by
  signal n-128" — false for a chosen exit.
- Severity: MINOR (pre-adjudicated: the direction is
  INCONCLUSIVE-not-FAIL — it claims nothing as an outcome and the
  honest-reporting exit 0 is charter-correct; no state collapses) — but
  the disclosure must name it and the reason must not assert a kill that
  did not happen.
- Demanded correction (disclosure-class): extend the DISCLOSURES
  pass-through sentence to "…or inside 129..159"; fix the module doc's
  "was KILLED" to "is classified as"; soften the reason to e.g. "the
  wrapper's shell status was 130, inside the signal band; classified as
  signal 2 — a genuine inner exit in 129..159 is indistinguishable from a
  signal death (disclosed)".

### R3-F2. MINOR — the signal-0 corner: a genuine inner exit 200 reports "killed by signal 0 (the wrapper's shell encoded it as 128)" — a doubly impossible claim

- Executed: stub `exit 200` → `AK-930 INCONCLUSIVE unknown | reason:
  cannot_run: the sweep command was killed by signal 0 (the wrapper's
  shell encoded it as 128; the band remap carried the signal through)`;
  sweep exit 0 (case x200).
- No process can be killed by signal 0 (it is an existence probe; no
  signal is delivered), and a shell never encodes a signal death as 128 —
  signal deaths are 129..191. This row can ONLY be a genuine inner chosen
  exit 200 passing through the band check (`classify_exit(200)` =
  InnerSignal { signal: 0 }), never a signal death. The CLASS is disclosed
  (200..231 indistinguishable, "the band side claiming nothing") and the
  direction is INCONCLUSIVE — no state collapse — but the reason asserts
  specific impossible facts.
- Demanded correction: special-case signal 0 in verify_cmd.rs's
  InnerSignal reason, e.g. "shell status 200 is the signal-band base;
  classified as signal 0 — no signal death can produce this code, a
  genuine inner exit of 200 is indistinguishable (disclosed)". Same
  sentence-in-DISCLOSURES extension as R3-F1 covers the class text.

### R3-F3. COSMETIC — the kind-arm refusal text still advertises the bounded recorded-artifact form to log-kind authors, though COR-4 R2-F4 made it test_report-only

- Executed: world-reglog (kind log + bounded:true + artifact arm) →
  REFUSED, exit 1, no report: "kind log admits execute arms only, or the
  bounded recorded-artifact form (charter decision 11) — an unbounded
  artifact arm would launder a text file into a PASS with zero commands
  executed".
- The BEHAVIOR is the delivered correction, confirmed in code
  (`manifest.rs:622-624`: `bounded_artifact` requires kind ==
  "test_report") and by execution (the bounded log shape refuses). But
  the refusal text (`manifest.rs:650`) tells a log-kind author the
  bounded form exists for them — set bounded:true and you are refused
  again with the same misleading message. Same class as round 2's R2-F6.
- Demanded correction: when kind is log, say "…admits execute arms only
  (the bounded recorded-artifact form is test_report-only)".

### R3-F4. MINOR — an artifact-only sweep into a not-yet-existing report parent adjudicates fully, then fails at the report write

- Executed: world-regctrl (bounded test_report + artifact arm) with no
  `out/` directory and `--report out/regctrl.json`: the sweep ran, every
  unit adjudicated (`commands_executed: 0`), then
  `gauntlet verify --all: writing …/out/regctrl.json: No such file or
  directory (os error 2)`, exit 1, no report.
- Mechanism: verify_cmd.rs creates directories only for EVIDENCE file
  parents; an artifact-only manifest produces zero evidence files (and no
  evidence MANIFEST — mod.rs skips it when no command ran), so the
  report's parent is never created before the report write. The
  production default (`.agents/reports/`) always exists, so the committed
  sweep is unaffected; any fresh world or custom `--report` into a new
  directory with an artifact-only manifest hits it. Fail-explicit (typed
  IO error, exit 1, no half-report), so no honesty concern — but the
  recorded-posture arm itself cannot land in a fresh world without a
  pre-made directory.
- Demanded correction: `create_dir_all(report_path.parent())` before the
  report write (or always emit the evidence MANIFEST, even empty).

## Attacks that held (executed; the corrected surface survived)

### Attack 1 — the signal band's boundaries are exact (all four edges)

- `kill -1` (SIGHUP) → shell 129 → `INCONCLUSIVE cannot_run: the sweep
  command was killed by signal 1 (the wrapper's shell encoded it as 129;
  the band remap carried the signal through)`; sweep exit 0; no evidence
  log (commands_executed 0 — nothing fabricated). (case sig1)
- `kill -31` (SIGSYS) → shell 159 (band top) → signal 31, encoded 159,
  same shape. (case sig31)
- `kill -34` (SIGRTMIN) → shell 162, OUTSIDE 129..159 → **FAIL** `command
  exited 162`, status `blocked`, sweep exit 1, evidence log records the
  shell's true status. The disclosed RT-signal residual stays FAIL
  exactly as documented — never CannotRun. (case rt34)
- `kill -40` → shell 168 → **FAIL** `command exited 168`. (case rt40)

### Attack 3 — the daemon-stop bound is load-bearing and exact

- State sensitivity established: a CLEAN no-daemon `sccache --stop-server`
  fails fast (0.104s, exit 2); a DIRTY no-daemon state makes the client
  print "Stopping sccache server…" and wait out a connect ETIMEDOUT —
  REPRODUCED LIVE THIS ROUND, unbounded (os error 110, the ~2m14s-class
  R2-F3 stall). Which state applies depends on how the previous
  daemon/stop ended; the wrapper cannot rely on the fast path.
- The delivered bound caps the stall at exactly 10.0s in every parent
  context: bash-direct 10.017s, `sh -c` 10.025s, flock-sh 10.023s. With
  an inner `sleep 2` the exact wrapper shape = 12.050s / 12.056s (two
  runs). Through the REAL sweep (case sleep2, FALSIFIER_SLEEP=2
  FALSIFIER_EXIT=0, no daemon): sweep wall 12.04s, evidence MANIFEST
  duration=12025ms, unit PASS, sweep exit 0 — inner runtime + ≤10s +
  overhead, NEVER +134s. This round's opening build also hit the bound
  against a stale/hung daemon: 10.99s wall = 0.63s build + ~10.3s stop.
- With a live clean daemon the stop is fast: every daemon-refreshed case
  run completed in 0.11–0.15s total.

### Attack 4 — the sccache-alive invariant held on every executed run

- Explicit form (case alive): daemon started (pgrep pid confirmed), one
  full sweep through the REAL wrapper; a 20ms-interval pgrep watcher
  observed the daemon die t+220ms — DURING the locked run; sweep exit 0,
  AK-930 PASS, `pgrep sccache` empty at end.
- Corroborating form: every classification case used the same
  start-daemon → sweep → pgrep-assert cycle; every run ended "daemon
  stopped after run" (~15 runs). The stop sits inside the locked script
  (structure pinned by the adapter's shape test), so the stopping moment
  is in-lock by construction and the end-state was verified behaviorally
  every time.

### Attack 5 — the classification table is exact on the whole reserved-set fuzz

Executed codes {68, 70, 85, 87, 168, 169, 186, 187, 199, 200, 232, 233},
one real-wrapper sweep each:

| inner exit | observed outcome |
| --- | --- |
| 68, 70, 85, 87 | FAIL `command exited N`, blocked, sweep exit 1 |
| 168 | FAIL `command exited 168` (RT band stays FAIL, per disclosure) |
| 169 | FAIL `command exited 169 — the wrapper remaps an inner exit of 69; a direct exit of 169 is indistinguishable from the remapped form (disclosed)` |
| 186 | FAIL `command exited 186 — … inner exit of 86; … (disclosed)` |
| 187, 199 | FAIL `command exited N` |
| 200 | INCONCLUSIVE `cannot_run: … killed by signal 0 …` (see R3-F2) |
| 232, 233 | FAIL `command exited N` (band edge exclusive above 231) |

Every state direction correct: no reserved lock code is reachable from an
inner failure, no inner failure hides as environmental, no environmental
death reports as FAIL. Evidence logs record the shell's true status for
FAILs; CannotRun rows write no log.

### Attack 6 — regression matrix all green

- COR-4 R2-F4: kind log + bounded + artifact REFUSED, exit 1, no report
  (world-reglog; message text defect recorded as R3-F3). Control: kind
  test_report + bounded + artifact (matched locator, existing six-heading
  artifact) → PASS `implemented_but_provisional`, exit 0 (world-regctrl,
  after mkdir — see R3-F4).
- Duplicate ids: REFUSED — "line 12: unit AK-901 appears again (first
  seen on line 3) — one entry per ruler unit; the first entry must never
  shadow a later one" — R2-F6's mangled-spaces prose is FIXED, clean
  text. (world-regdup)
- Adapter suite: `cargo nextest run -p gauntlet-adapter-verify-command-
  exit-code --lib` → 13/13 in 4.038s (round 2: 9/9 with 269s of
  behavioral-test stall — the R2-F3 payment fix is visible in the teeth
  themselves).
- Committed-manifest tooth: `cargo nextest run -p gauntlet-cli --test
  conformance_cli` → 9/9 (includes
  `the_committed_manifest_agrees_with_the_frozen_ruler`).
- docs_present: 7/7. `just ruler` → PASS (digests match the frozen lock).

## Suspicion list (code-cited, NOT executed)

- Carry-over: `assemble_report` accepts a `reason` on a PASS row (latent,
  unchanged, unreachable from run_sweep).
- Carry-over: the wrapper script interpolates structural values unquoted;
  the invariant still lives in the closed vocabulary validator.
- The dirty-vs-clean sccache state machine (what exactly flips the client
  into the ETIMEDOUT wait) was characterized behaviorally, not from
  sccache source — the bound makes the distinction non-load-bearing for
  the sweep (10s worst case either way).
- Whether cargo/nextest can really exit 200 (R3-F2's corner) in practice
  was not surveyed; the corner is reachable through the closed vocabulary
  whenever the inner command returns it, which is the only claim made.

## Not attacked (honest scope)

- The honest busy-lock INCONCLUSIVE end-to-end through the CLI (3600s
  production wait; the 86/69 lock-level classification is covered by the
  adapter suite 13/13 and round-1/2 executions).
- RT-signal deaths beyond 34/40 (the 160..192 residual band sampled at
  two points, both FAIL as disclosed).
- The full 70-unit real sweep; `gauntlet verify --run`; `gauntlet export`;
  the operator TUI workstream (out of scope per charter).
- b1/b2/b3 report contents (blind), .agents/agents/, .agents/prompts/.
- Lock-file OPEN-failure paths (permissions/ENOSPC) — sabotage risk,
  round-1 carry-over.

## Environment discipline record

- No tracked file was modified, staged, or restored by this round; every
  attack ran in /tmp/gauntlet-falsifier3 (fixture worlds, stub, runner)
  or through env-override worlds. The only repo write is this report
  file (untracked, the mandated deliverable).
- Every `flock` was its OWN top-level command; one compile at a time
  under /tmp/gauntlet-build.lock; `timeout 10 sccache --stop-server
  || true` inside every flock'd command that compiled; no flock was ever
  nested inside another flock's command string.
- Never raised build/test parallelism; package-scoped suites only; no
  workspace-wide run needed.
- /tmp cleanup at end of round: /tmp/gauntlet-falsifier3 removed in full.
