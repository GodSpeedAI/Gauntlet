# Task 27 — falsifier round 2 (execution report)

VERDICT: **NOT_CONFIRM — the corrections did not fully hold.** 6 new findings
(3 MAJOR, 2 MINOR, 1 COSMETIC), every one with an executed reproduction:
R2-F1 inner-exit-86 misclassified as lock-busy (a genuine failure becomes an
INCONCLUSIVE the plan gate exits 0 on); R2-F2 an inner signal kill (the
realistic OOM victim) is still FAIL-by-default with a fabricated 128+n exit
code; R2-F3 the wrapper's unconditional `sccache --stop-server` stalls ~2m14s
per command, lock-held, visible as a ~134.8s floor across the committed
sweep's own evidence. R2-F4 the delivered kind/arm matrix is wider than the
correction's own contract (log × bounded-artifact); R2-F5 homoglyph laundering
evades both docs legs; R2-F6 a mangled refusal string in the F1 correction.
The F1/F2/F10/F3/COR-B3 corrections HELD on every other executed shape (see
the held list); all regression gates green (app 25/25, cli 9/9, adapter 9/9,
docs_present 7/7, ruler PASS, bundle --check verified).

Status: COMPLETE (round 2).

Falsifier: blind executing adversary, round 2. Re-attacks the COR-3 corrected
surfaces (F1 duplicate-id refusal, F2 kind/arm matrix, F10 signal classification,
F3 handoff predicate, F7/F13 disclosures, COR-B3 docs-claim binding) and their
adjacencies, through /tmp fixture worlds driven by the REAL binary, never
mutating the tracked tree.

## Target map (read, normative inputs only)

- `crates/gauntlet-app/src/conformance/manifest.rs` — COR-3 F1 DuplicateId
  (byte-equal compare at :762), COR-3 F2 matrix (:617-649).
- `crates/gauntlet-app/src/conformance/mod.rs` — adjudication, set equality.
- `crates/gauntlet-app/src/conformance/predicates.rs` — F3 heading grammar.
- `crates/gauntlet-app/src/conformance/report.rs` — DISCLOSURES, assembly.
- `crates/gauntlet-cli/src/verify_cmd.rs` — F10 signal handling (:130-142),
  reserved-code classification (:143-162).
- `crates/adapters/gauntlet-adapter-verify-command-exit-code/src/sweep_toolchain.rs`
  — reserved pair {86, 69}, remap 69→169 ONLY (:147-151).
- `crates/gauntlet-arch-tests/src/{docs,docs_consistency}.rs` — COR-B3 checkers.
- Committed inputs: `.agents/reports/conformance-manifest.yaml`,
  `conformance.json`, `handoff.md`, `evidence-bundle.sha256`, `docs/[A-Z]*.md`.

## Executed harness

Fixture worlds under /tmp/gauntlet-falsifier2 driven by the REAL binary
(`target/debug/gauntlet`, rebuilt from HEAD under the compile lock before
attacking: "Finished dev profile in 0.73s", binary mtime after every touched
source) via `GAUNTLET_PROJECT_ROOT=<world> gauntlet verify --all --manifest
fixture-manifest.yaml --report out/<case>.json`. Worlds carry a fixture ruler
at `.wayfinder/answer-key.md` and a detached `.git/HEAD` (head_commit reads
it). `--manifest`/`--report` paths join the project root. F8 shapes run
through a /tmp driver crate path-depending on `gauntlet-arch-tests` and
calling the REAL checkers. Compiles serialized under /tmp/gauntlet-build.lock.

## Findings

### R2-F1. MAJOR — a locked-arm command that exits 86 ON ITS OWN MERITS is recorded INCONCLUSIVE "the repository compile lock stayed busy" — a fabricated environmental excuse for a genuine failure

- Executed: world-f10, AK-930 = `execute: {package: gauntlet-domain, lib: true,
  filter: zzz_no_such_xyz}` (a LOCKED nextest arm). A stub `cargo` was placed
  first on PATH: `#!/bin/sh` / `exit 86`. The lock was FREE (fuser empty; bare
  `flock -c /bin/true` on /tmp/gauntlet-build.lock: 0.009s).
  `PATH=/tmp/gauntlet-falsifier2/stub:$PATH GAUNTLET_PROJECT_ROOT=…/world-f10
  ./target/debug/gauntlet verify --all --manifest fixture-manifest.yaml
  --report out/f10-a.json`
- Observed: `AK-930 INCONCLUSIVE unknown | reason: cannot_run: the repository
  compile lock stayed busy past the wrapper's bounded wait`. The evidence log
  records `exit=86` for `command=cargo nextest run -p gauntlet-domain --lib
  zzz_no_such_xyz`. EXECUTED end-to-end consequence: a world containing ONLY
  AK-930 runs `units=1 pass=0 inconclusive=1 fail=0` and the sweep EXITS 0 —
  the plan gate passes on a command that genuinely failed
  (world-f10single).
- Expected: 86 is one half of the RESERVED pair {86, 69}; an inner command's
  own 86 must surface as a FAIL ("command exited 86"), exactly as an inner
  own-69 surfaces as FAIL "command exited 169" (executed: case b69 → FAIL,
  "command exited 169"). The disambiguation is HALF-BUILT: the wrapper script
  remaps only 69→169 (`sweep_toolchain.rs:147-151`, `-eq 69`), never 86, so
  every inner exit-86 is misattributed to the lock layer by
  `verify_cmd.rs:143`.
- The code's own comment lies: verify_cmd.rs:145-147 — "The wrapper script
  remaps the INNER command's own reserved code, so these two are
  unambiguously the lock layer speaking" — is FALSE for 86; and the report
  `_meta` disclosure ("the wrapper remapping an inner command's own reserved
  exit code so the lock-level pair stays unambiguous") inherits the same
  overclaim. A command exiting 169 directly would ALSO surface as 169
  (indistinguishable from remapped 69) — same ambiguity class.
- Mechanism: `sweep_toolchain.rs` script remap covers only 69;
  `verify_cmd.rs:143` classifies both reserved codes as lock-level.
- Minimal correction: remap the inner command's own 86 in the wrapper script
  (the 69→169 precedent, e.g. `__sweep_status=186`), extend the
  `INNER_EXIT_REMAP` contract, and pin it with the behavioral tooth (the
  inner_exiting(86) stub). Fix the verify_cmd comment and the `_meta`
  disclosure to name BOTH remaps.

### R2-F2. MAJOR — a signal-killed INNER command (the realistic OOM victim) is still FAIL-by-default with a fabricated exit code; COR-3 F10 covers only a kill of the wrapper itself

- Executed (same world, stub `cargo`): `kill -15 $$` → AK-930 = FAIL,
  "command exited 143", evidence log `exit=143`; `kill -9 $$` → FAIL,
  "command exited 137", evidence log `exit=137`. No such exit codes existed —
  the child died on signal 15/9 and the wrapper's shell encoded 128+n.
- Observed disposition: FAIL (`blocked`), reason carries no signal fact, the
  evidence log records a code the process never had.
- Expected: round-1 F10's demanded correction — "an executor that cannot run
  a command (environmental, typed) ⇒ INCONCLUSIVE" — applies a fortiori to
  the INNER command: in this memory-tight WSL workspace the OOM-killer
  targets the memory-hungry test process (the inner command), not the flock
  wrapper. The correction as delivered classifies the wrapper-level kill
  (code() == None) but leaves the inner kill landing as an honest-looking
  FAIL with a fabricated code — the exact FAIL-by-default shape charter
  decision 3 forbids, one shell layer down.
- Mechanism: the wrapper script's `__sweep_status=$?` destroys the signal
  fact (sh encodes 128+n); `verify_cmd.rs` then sees Some(143/137) and
  classifies a plain non-zero exit.
- Minimal correction: have the wrapper script detect the 128..191 band it
  itself can only mean "child died on signal (n-128)" OR document the
  ambiguity explicitly; remap/emit a distinguishable marker for it (e.g.
  exit 200+signal-128) and classify it as CannotRun naming the signal — or,
  minimally, add an honest `_meta` disclosure that inner signal deaths
  surface as 128+n FAILs, and record the band in the evidence log.

### R2-F3. MAJOR (payment) — the delivered wrapper's unconditional `sccache --stop-server` stalls ~2m14s per command WHENEVER no sccache server is live, while HOLDING the repository compile lock; the committed sweep's own evidence shows the ~134s floor on ~45 commands

- Executed, direct: `time sccache --stop-server` twice with NO server running
  (pgrep sccache: empty) → 2m14.044s, 2m14.487s (pure wait, user 0.057s).
  `time flock -c /bin/true` on the same lock → 0.009s. My locked stub arms
  (stub exits in 0ms) each took ~135s wall — the wrapper's stop-server line
  IS the cost.
- Corroborated by the committed good sweep:
  `.agents/reports/conformance-evidence/20260907T200600Z/MANIFEST` — many
  unrelated commands sit just above a ~134.8s floor (03: 135167ms, 05:
  134860ms, 07: 134215ms, 08: 135591ms, 27: 139953ms …). The 45-command
  sweep paid roughly 45 × ~135s ≈ 100 minutes, lock-held. Round-1's
  "two wrapper tests took ~270 s under load" is two stalls, not load — and
  the recorded `b1_contract_wins_over_exit7` load-flakiness watch item plausibly
  shares this cause.
- Mechanism: `sweep_toolchain.rs:148` runs `sccache --stop-server`
  unconditionally inside every wrapped command; with no live server the
  client waits out a connect timeout (~2m14s on this box) while the flock
  fd is held, serializing all repo work behind the stall.
- Minimal correction: bound the stop (`timeout 10s sccache --stop-server`)
  or stop only when a server is actually live; keep the task-20 invariant
  (a daemon never outlives the locked command) with the bounded form.

### R2-F4. MINOR — the delivered matrix admits (test_report|log) × BOUNDED-artifact, wider than the correction's own contract; a `log`-kind unit can PASS (provisional) with zero commands executed on a text artifact

- Executed: world-f2d, AK-911 kind `log` (the ruler's real log-kind shape,
  cf. AK-061) + `bounded: true` + `artifact:
  {path: fixture-artifacts/handoff-six.md, predicate: handoff_six_categories}`.
  Real binary: ACCEPTED; AK-911 PASS, `implemented_but_provisional`,
  `evidence_kind=log`, evidence_ref = the text file; sweep exit 0; zero
  commands executed for it.
- Expected per the correction's own summary ("test_report|log=>execute
  only; file|metric=>artifact|none only") and charter decision 11, which
  names exactly ONE recorded-artifact instance (AK-070, kind test_report).
  The code (`manifest.rs:617-621`) generalizes the bounded exception to the
  `log` kind. The dishonest PROVEN form is closed (unbounded log+artifact
  refuses, executed: world-f2a), and the bounded form lands in a disclosed
  provisional category — residual: dead latitude a future manifest can use
  without a recorded change, and the row's `evidence_kind=log` still
  misdescribes a text artifact.
- Minimal correction: restrict the bounded-artifact exception to
  `test_report` (the charter's single named instance) or record the log-kind
  widening deliberately (comment + the matrix refusal text).

### R2-F5. MINOR — homoglyph laundering evades BOTH docs legs (a new evasion class outside the five closed)

- Executed (driver crate over the REAL checkers, fixture json where AK-053 is
  implemented_but_provisional):
  `- implemented_and_proven — АК-053: prose` (Cyrillic А, U+0410) → ZERO
  violations: `unit_ids_on` scans for the ASCII `ak-` prefix after
  lowercasing, so the line names no unit; neither leg sees a claim.
  `- implemеnted_and_proven — AK-053: prose` (Cyrillic е, U+0435 in the
  word) → ZERO violations: the status word is not the canonical byte string,
  and no STATUS_LIKE_TOKEN matches, so nothing binds. A doc can claim
  `implemented_and_proven` for a provisional unit in a form a reader sees as
  identical while both mechanical legs are blind.
- Context: same class as round-1 F8 (deterrence gap on authored input, not a
  live lie — the committed docs are honest, docs_present 7/7 on the real
  tree re-run below). The COR-B3 hardening closed casing/separator/split
  shapes; homoglyphs are the adjacent class it did not claim.
- Minimal correction (cheap): normalize the doc lines with a Unicode
  confusable/ASCII-fold pass before scanning, or fold the specific
  look-alikes (Cyrillic А/Е/О/С/Р and the unicode dash family) in
  `unit_ids_on`/`contains_word` inputs; at minimum record the class as
  disclosed-limited in the module docs.

### R2-F6. COSMETIC — the F1 correction's refusal text carries a mangled run of spaces

- manifest.rs:282: "…first seen on line {first_line}) — one
  `                    ` entry per ruler unit…" — executed refusal output:
  `unit AK-901 appears again (first seen on line 3) — one                  entry
  per ruler unit`. Typo in the delivered correction's message.

## Attacks that held (executed; the corrected surface survived)

### F1 corrections held — every near-duplicate shape refuses, no report, exit 1

- Byte-equal duplicate where the SECOND entry is the honest one (first
  `execute: /bin/true` forged, duplicate `execute: /bin/false`): REFUSED —
  `DuplicateId` names BOTH loci ("line 12: unit AK-901 appears again (first
  seen on line 3)"); the honest-later-entry shadowing of round 1 is dead.
  (world-f1a)
- Whitespace-padded twin (`AK-901` + `AK-901 ` with trailing spaces):
  REFUSED as DuplicateId — `split_field` trims, so padding canonicalizes into
  the byte-equal check. (world-f1c) Same for a trailing NBSP twin
  (`AK-901\u{00a0}`): REFUSED as DuplicateId. (world-f1e)
- Duplicate id where the twin arm is ARTIFACT against the execute original:
  REFUSED as DuplicateId (the first entry's matrix check runs at
  close_entry, then the dup check fires). (world-f1d)
- Case-variant twin (`AK-901` + `ak-901`): NOT caught by DuplicateId (the
  check is byte-equal), but refused by the ruler set-equality — "the
  manifest names units the ruler does not carry: ak-901", exit 1, no report.
  Same for a leading-zero twin (`AK-0901`: "the manifest names units the
  ruler does not carry: AK-0901"). No false PASS exists on any near-dup
  path; the residual is only that the canonicalized twins report as
  unknown-id rather than as a duplicate pairing. (world-f1b, world-f1f)

### F2 corrections held — the dishonest matrix shapes refuse; the directory shape cannot PASS

- kind `log` + UNBOUNDED artifact arm: REFUSED — "kind log admits execute
  arms only, or the bounded recorded-artifact form (charter decision 11) —
  an unbounded artifact arm would launder a text file into a PASS with zero
  commands executed". (world-f2a)
- kind `file` + execute arm: REFUSED ("kind file admits execute arms
  only…"). (world-f2b)
- Alien kind (`screenshot`) + any arm: REFUSED — "kind screenshot is outside
  the sweep's kind-arm matrix … widening it is a recorded change".
  (world-f2e)
- bounded test_report + artifact arm pointing at an EXISTING DIRECTORY:
  the sweep runs, and the row is INCONCLUSIVE `unreadable_artifact: … Is a
  directory` — implemented_but_provisional is NOT reachable through a
  directory artifact. (world-f2c)
- Round-1 F2's dishonest PROVEN form stays dead: the only artifact route for
  a command-kind unit is the bounded recorded-artifact posture, which lands
  in a disclosed provisional category (see R2-F4 for its log-kind latitude).

### F10 correction held — direct kills of the swept command name THEIR signal

- Program arms killed by SIGTERM (15), SIGSEGV (11), SIGKILL (9): each
  INCONCLUSIVE `cannot_run: the sweep command was killed by signal 15/11/9
  (no exit code exists)`; no evidence log is written for them (no fabricated
  `exit: -1` anywhere). (world-f10, rows AK-931/932/933)
- Bare program arms keep ownership of their codes: direct `exit 86` → FAIL
  "command exited 86"; direct `exit 69` → FAIL "command exited 69" (only
  LOCKED arms interpret the reserved pair). (world-f10, rows AK-934/935)
- The inner-69 remap works as documented: locked arm whose inner command
  exits 69 → surfaces 169 → FAIL "command exited 169". (world-f10 case b69)

### F3 correction held — the handoff predicate demands real `## ` section headings

Executed through the real binary (world-f3, one artifact per shape):
- six `## <category>` headings with ZERO content: Holds (PASS) — see
  residual R2-O1;
- `### implemented_and_proven` (three hashes): Refutes → FAIL;
- `## implemented_and_provens` (near-miss plural): Refutes → FAIL;
- `## implemented_and_proven:` (colon suffix): Refutes → FAIL;
- `## implemented_and_proven — 65 units` (count suffix): Holds — the
  documented trailing-suffix form.

### COR-B3 docs-claim binding held — six of the eight round-2 evasion shapes are caught by the window fallback

Executed through a /tmp driver over the REAL checkers (fixture json where
AK-053 is `implemented_but_provisional`, AK-001 proven):
- id-first reversed order (`- AK-053 — implemented_and_proven: prose`):
  reported, "claims implemented_and_proven for AK-053 … (ahead of
  evidence)";
- id inside backticks / inside parentheses: reported;
- en-dash separator (`–`) and unspaced em-dash (`—` with no spaces):
  reported;
- trailing punctuation on the word (`implemented_and_proven,` / `.`):
  reported;
- reversed-BEHIND claim (`AK-001` called provisional): admissible —
  claiming weaker stays safe under variant shapes;
- over-rejection invariant re-pinned: verb-beside-id prose with no
  vocabulary word binds nothing (0 violations); an id two lines away from a
  vocabulary word binds nothing (window is strictly ±1).
- handoff leg (f) grammar variants all fail closed: a category header with
  a missing dash → "has no section"; a missing count → typed rejection
  ("carries no `N unit(s)` count"); a duplicated section → "2 … sections"
  plus per-unit "appears in 2 sections"; asterisk bullets → declared/listed
  mismatch. No silent acceptance anywhere.
- REAL-TREE truth leg: `check_docs_claims_not_ahead_at(real docs,
  committed conformance.json)` → 0 violations.

### Regression sweep (all executed on the committed tree, sequential under the lock)

- `just ruler` → PASS (digests match the frozen lock).
- `bash scripts/pin-evidence-bundle.sh --check` → "bundle verified", exit 0
  (50 pinned inputs).
- `cargo nextest run -p gauntlet-app --test conformance` → 25/25.
- `cargo nextest run -p gauntlet-cli --test conformance_cli` → 9/9
  (includes `the_committed_manifest_agrees_with_the_frozen_ruler`, :917 —
  the committed manifest still cross-validates against the committed ruler).
- `cargo nextest run -p gauntlet-adapter-verify-command-exit-code --lib` →
  9/9. Note: the two behavioral wrapper tests took 269.234s / 269.226s —
  exactly 2 × the ~134.7s sccache-stop stall of R2-F3, i.e. deterministic,
  not load.
- `cargo nextest run -p gauntlet-arch-tests --test docs_present` → 7/7 on
  the real tree.
- Rebuild of `gauntlet-cli` from HEAD before attacking: clean, 0.73s (fresh).

## Residual observations (executed, judged not to be new defects — recorded for the record)

- **R2-O1 — the AK-067 predicate remains a first-layer form check.** A
  handoff consisting of the six `## <category>` headings and NOTHING else
  still makes the sweep's `handoff_six_categories` HOLD (world-f3 AK-921 =
  PASS). The round-1 demanded correction (require the section headings) is
  delivered and works against legends/word-salads; content truth stays with
  the second layer (`docs_present` leg (f) reconciles the same file against
  conformance.json). Fail-closed composition; noted so nobody cites AK-067's
  predicate alone as proof of a truthful handoff.
- **R2-O2 — window over-binding can misattribute, exactly as disclosed.**
  An honest prose line naming `ak-053` adjacent (±1) to another unit's
  `implemented_and_proven` claim line is reported as that claim's owner:
  "ARCHITECTURE.md:2 claims implemented_and_proven for AK-053 …". The
  direction is fail-closed (over-pairing never launders), the behavior is
  documented in `docs_consistency.rs:457-466`, and the committed docs keep
  the required spacing (real-tree leg: 0 violations). Recorded because the
  failure message names the WRONG pairing's line — a future debugging
  trap.
- **R2-O3 — the committed conformance.json is the pre-correction sweep's
  output (the recorded lifecycle, not a defect).** It was generated
  2026-09-07T21:19Z at tree a98b624 (wave 1): it carries 7 disclosures (the
  corrected code emits 9 — the F7 anchoring and F13 filter disclosures are
  absent), its embedded `manifest.sha256` does NOT match the committed
  manifest (B3 phase-1 re-pinned the manifest after the sweep ran), and its
  AK-067 row is INCONCLUSIVE `no_artifact` although handoff.md now exists at
  the locator (a re-sweep will PASS it through the predicate, per B3's
  disclosed 3/67 recount). Charter decision 12 requires the settlement
  re-sweep + bundle re-pin to land before closure; until then the committed
  report and the corrected code deliberately disagree.
- **R2-O4 — disclosed environment event: tracked/untracked operator files
  changed DURING this round's window.** `DESIGN.md` (tracked) carries
  uncommitted modifications; `justfile` became modified and `workbench/`
  appeared untracked mid-round. Not by the falsifier — no command in this
  round writes any of them (`just ruler` only reads the justfile); they
  belong to the operator's concurrent TUI workstream (whose untracked
  plan/spec files were already present at round start). Recorded so the
  reconciliation battery attributes them correctly.

## Suspicion list (code-cited, NOT executed)

- Carry-over from round 1: `assemble_report` does not refuse a `reason` on
  a PASS row; `run_sweep` cannot produce one (latent, unchanged).
- Carry-over from round 1: the wrapper script interpolates structural values
  unquoted; the invariant still lives entirely in the closed vocabulary
  validator (unchanged).
- The inner-exit band 128..191 is ambiguous in POSIX sh: a program that
  REALLY exits 143 is indistinguishable from a SIGTERM death inside the
  wrapper. R2-F2's correction cannot be made perfectly signal-precise
  without the wrapper cooperating (e.g. running the inner under `sh -c '…
  && exit 0 || case $? in …'` with trap bookkeeping); any fix must disclose
  the residual ambiguity.
- An inner command that genuinely exits 169 is indistinguishable from a
  remapped inner-69 (folded into R2-F1's contract fix).
- Lock-file OPEN failures (permissions/ENOSPC → flock exit 1 → FAIL) remain
  unexecuted to avoid sabotaging shared state (round-1 carry-over).

## Not attacked (honest scope)

- The full 70-unit real sweep end-to-end (~45 commands × up to 2.5 min under
  R2-F3's stall; the orchestrator's settlement re-sweep covers it — and
  R2-F3 gives it a direct interest).
- The honest busy-lock INCONCLUSIVE end-to-end through the CLI (production
  wait fixed at 3600 s; round 1 executed the 86 path at the adapter level,
  and this round re-proved the adapter suite 9/9).
- `gauntlet verify --run`, `gauntlet export`, the operator TUI workstream
  (out of scope per charter).
- Fuzzing `mini_json` / the answer-key parser beyond the I8 spot checks the
  consistency legs already carry.
- The docs-claim legs against NON-ASCII whitespace twins of the em-dash
  beyond the executed en-dash/unspaced forms (the window makes separator
  variants unreachable as a class — any byte-shape of separator still puts
  the word and id in the same window; only homoglyph WORD/ID substitutions
  evade, recorded as R2-F5).

## Environment discipline record

- No tracked file was modified, staged, or restored by this round; every
  attack ran in /tmp/gauntlet-falsifier2 (fixture worlds, stubs, the driver
  crate) or through the checkers' env-override design. The only repo write
  is this report file (untracked, the mandated deliverable).
- Every `flock` was its OWN top-level command; one compile at a time under
  /tmp/gauntlet-build.lock; `sccache --stop-server` inside the same flock'd
  command (and — per R2-F3 — each such stop cost this round ~2m14s of
  lock-held waiting, the finding's own subject).
- Never raised build/test parallelism; package-scoped suites before any
  broader run; no workspace-wide run was needed.
- /tmp cleanup at end of round: /tmp/gauntlet-falsifier2 removed in full.

