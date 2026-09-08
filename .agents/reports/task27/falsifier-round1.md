# Task 27 — falsifier round 1 (execution report)

VERDICT: **NOT_CONFIRM — 8 findings (3 MAJOR, 5 MINOR)**, every one with an
executed reproduction; the false-PASS paths are F1 and F2, the false-FAIL
(I8-violating) path is F10. The delivered refusal matrix, three-state
adjudication, grouping teeth, and bundle pin all held under execution (see
"Attacks that held").

Status: COMPLETE (round 1).

Falsifier: blind executing adversary. Attacks run against /tmp copies,
fixture worlds, and env-override fixtures; the committed tree is never
mutated. Every finding below carries a command that was actually run.

## Target map (read, normative inputs only)

- App sweep: `crates/gauntlet-app/src/conformance/{mod,manifest,commands,predicates,report}.rs`
- CLI: `crates/gauntlet-cli/src/verify_cmd.rs` (verify_all at :383; SweepProcessExecutor :77-157)
- Adapter toolchain: `crates/adapters/gauntlet-adapter-verify-command-exit-code/src/sweep_toolchain.rs`
- Committed inputs: `.agents/reports/conformance-manifest.yaml` (70 entries, 4 bounded:true),
  `.agents/reports/conformance.json`, `.agents/reports/handoff.md`,
  `docs/[A-Z]*.md`, `tests/contract/docs_present.rs`,
  `crates/gauntlet-arch-tests/src/{docs,docs_consistency}.rs`,
  `scripts/pin-evidence-bundle.sh`, tests `conformance_cli.rs` / `conformance.rs`.
- Env knobs discovered for driving the REAL binary at a fixture world:
  `GAUNTLET_PROJECT_ROOT` (`config/resolve.rs:1739`), plus `--manifest`/`--report`
  (verify_cmd.rs:206/167) whose paths join the root (absolute paths replace it).

## Findings

Executed harness: fixture worlds under /tmp/gauntlet-falsifier driven by the REAL
binary (`target/debug/gauntlet`, bin target `src/bin/gauntlet.rs` — note:
`target/debug/gauntlet-cli` is a STALE Aug-31 artifact, not the CLI surface) via
`GAUNTLET_PROJECT_ROOT=<world> gauntlet verify --all --manifest <fixture.yaml>
--report <tmp>/report.json`, cwd = the repo so cargo arms resolve the repo
workspace. Ruler/manifest generators: /tmp/gauntlet-falsifier/gen_world.py.

### F1. MAJOR — duplicate manifest ids are accepted; the FIRST entry silently shadows any later entry for the same unit

- Executed: world-dup has AK-901 twice (first `execute: {program: /bin/true}`,
  duplicate `execute: {program: /bin/false}`); ruler carries AK-901 once.
  `GAUNTLET_PROJECT_ROOT=/tmp/gauntlet-falsifier/world-dup ./target/debug/gauntlet
  verify --all --manifest world-dup/manifest.yaml --report out/dup.json`
- Observed: NO refusal (exit 1 only because AK-902's honest arm fails); report
  has exactly 2 rows; AK-901 = PASS with basis "FORGED first entry (true)".
  The shadowed honest entry appears nowhere — not in the report, not in stderr.
- Expected: charter decision 2 says "One entry per ruler unit"; set equality is
  checked as id SETS, so a duplicate slips the both-directions check
  (`mod.rs:265-279`); `by_id` (`mod.rs:262`) returns the first match, so the
  first entry decides the disposition and the second is dead input the sweep
  conceals.
- Mechanism: `crates/gauntlet-app/src/conformance/manifest.rs`
  (parse_manifest_with never compares entry ids) + `mod.rs:262,274`.
- Minimal correction: refuse duplicate ids in parse_manifest_with (typed
  ManifestError::DuplicateId { id, line }) — set equality is on multisets, not
  sets.

### F2. MAJOR — a `test_report` unit may carry an ARTIFACT arm and PASS with zero commands executed (evidence-kind laundering)

- Executed: world-main AK-906: `kind: test_report` (ruler) + manifest arm
  `artifact: {path: fixture-artifacts/handoff-six-words.md, predicate:
  handoff_six_categories}`. Real binary run (see harness above).
- Observed: AK-906 = PASS, `status_category=implemented_and_proven`,
  `evidence_kind=test_report`, evidence_ref = the artifact file. No test ever
  ran; the report row CLAIMS the test-report evidence kind while the actual
  evidence is a text file. `_meta.commands_executed` counts nothing for it.
- Expected: the charter's shape rule restricts file/metric kinds from execute
  arms (`manifest.rs:547-560`) but NOT the inverse — test_report is admitted
  with artifact/none arms. The report's evidence_kind then misdescribes the
  evidence actually used. G12-adjacent (claim without the declared evidence
  class).
- Mechanism: `manifest.rs:549` (`built.kind == "file" || built.kind == "metric"`)
  and adjudication `mod.rs:430` (artifact arm evaluated regardless of kind).
- Minimal correction: admit only the kind/arm pairs (test_report→execute,
  file/metric→artifact|none) and refuse the rest.

### F3. MINOR — `handoff_six_categories` Holds on a content-free artifact

- Executed: world-main AK-904's artifact is ONE line containing the six §26
  words and nothing else (no sections, no units). Real binary run.
- Observed: PASS (implemented_and_proven for the file-kind unit).
- Expected (adversary's reading): a predicate that only `contains()` six words
  cannot distinguish a real handoff from a legend. Defense-in-depth exists —
  docs_present leg (f) parses `## <category> — N units` sections and would be
  RED against the real conformance.json — so the committed gate catches the
  lie one layer up; but the sweep's own AK-067 verdict (and the plan gate's
  python asserts) would call it proven.
- Mechanism: `predicates.rs:55-84`.
- Minimal correction: require the six `## <category> — N units` section
  headers (the same grammar the consistency leg parses) in the predicate.

### F4. MINOR — `mutation_record_valid` accepts a stale record (date not checked against anything)

- Executed: world-main AK-912: otherwise-valid mutation record with
  `generated_at: 2020-01-01`. Observed: PASS. The 64-hex `manifest_sha256` is
  shape-checked but never compared to anything.
- Context: charter decision 11 deliberately adopts the recorded-artifact
  posture, so staleness is disclosed policy, not an accident; recorded as a
  held observation with the residual that the pin is unverifiable inside the
  sweep.

### F5. Confirmed-held refusal matrix (all exit 1, NO report written, typed locus)

r1 unknown id; r2 missing id (both directions); r3 kind mismatch; r4 frozen
locator drift; r5 `../escape.md`; r6 `/etc/passwd`; r8 filter `a;b`;
r9 `features: [default]`; r10 relative program; r11 program-with-args
(whitespace smuggling); r12 file-kind+execute arm. Exact transcripts in the
session log; each REFUSED line names the unit/line. Cosmetic: close_entry
errors report "line 0" (`manifest.rs:530-559`).

### F6. Other executed dispositions that held (world-main)

`/bin/false` → FAIL blocked; exit-4 stand-in → FAIL "command exited 4" (the
nextest zero-tests class); empty artifact → predicate Refutes → FAIL; directory
artifact → INCONCLUSIVE `unreadable_artifact`; absent artifact → INCONCLUSIVE
`no_artifact`; grouping of identical program arms ran once (3 commands for 13
units); evidence logs + MANIFEST with per-log sha256 written beside the report;
INCONCLUSIVE-only hint `none` maps to `unknown` as documented.

### F7. MINOR — evidence_ref resolution anchor depends on the arm, and breaks under a non-default --report

- Executed: world-main run with `--report /tmp/gauntlet-falsifier/out/main.json`:
  execute-PASS rows cite `conformance-evidence/<stamp>/NN.log` which resolves
  against the REPORT'S PARENT (`verify_cmd.rs:459-460`), while artifact-PASS
  rows cite workspace-root-relative paths. With the report outside the repo,
  neither spelling resolves from the repo root — the plan's "resolvable
  evidence ref" is only true at the default location. The committed report
  uses the default location, so the shipped artifact is fine; recorded as a
  latent wart for any future non-default run.

### F8. MINOR — docs status-claim laundering: five evasion classes invisible to BOTH mechanical legs (executed)

- Executed: /tmp driver crate (`gauntlet-arch-tests` path dependency) calling the
  REAL `docs::check_status_claim_vocabulary` and
  `docs_consistency::check_docs_claims_not_ahead_at` against fixture docs and
  the REAL committed conformance.json (where AK-053 is
  implemented_but_provisional, so any proven claim is "ahead of evidence").
- EVADE both legs (0 violations each):
  1. `- Implemented_and_proven — AK-053: …` (leading capital);
  2. `- IMPLEMENTED_AND_PROVEN — AK-053: …` (all caps);
  3. `- implemented_and_proven - AK-053: …` (hyphen instead of em-dash);
  4. claim split across two lines (`- implemented_and_proven:` / newline / `AK-053 — …`);
  5. lowercase id (`— ak-053:`).
- CAUGHT as designed: canonical ahead claim (leg g, "ahead of evidence");
  AK-0053 leading zeros (leg g, "carries no such unit"); bare `- proven — AK-053`
  (leg e, non-canonical token); prose `AK-053 is proven and delivered` (leg e).
  `implemented and proven` with spaces is caught by leg e too (bare tokens
  "proven, implemented").
- Context: `contains_word`/`parse_claim` are case-sensitive and shape-exact
  (`docs.rs:219-233`, `docs_consistency.rs:410-425`); the module docs promise
  "mixed-up near-misses … fail closed", but case variants are not near-miss
  tokens. Committed docs are honest today (docs_present 7/7 GREEN on the real
  tree). This is a deterrence gap in the anti-laundering teeth, not a live lie.
- Minimal correction: match the status tokens/canonical words case-insensitively,
  and add a fallback scan: any line naming AK-NNN that contains "proven"-
  family tokens in ANY casing without a canonical word (any casing) is a
  violation.

### F9. Confirmed-held: handoff↔conformance grouping teeth (T4 class, executed via the documented fixture env overrides)

Ran `cargo nextest run -p gauntlet-arch-tests --test docs_present
handoff_matches_conformance_grouping` with `GAUNTLET_HANDOFF_FIXTURE` /
`GAUNTLET_CONFORMANCE_JSON_FIXTURE` against tampered COPIES:
- canonical copies → GREEN (harness attribution control);
- unit moved proven→blocked → RED (2 violations, unit named);
- header count inflated 65→66 → RED (declares-vs-groups + lists-vs-declares);
- unit duplicated into a second section → RED ("appears in 2 sections");
- unit deleted from handoff (json still groups it) → RED ("missing … AK-005");
- bogus AK-999 added → RED ("lists unit(s) the json does not group there");
- json side recategorized under the handoff → RED (4 violations).
Every attack in menu item C fails loudly with the offending unit named.

### F10. MAJOR — a signal-killed sweep command (the OOM-killer shape) is recorded as FAIL "command exited -1", never INCONCLUSIVE

- Executed: held `/tmp/gauntlet-build.lock` (`flock -w 120 … -c 'sleep 20'`),
  launched the REAL binary over world-lock (one nextest arm:
  `cargo nextest run -p gauntlet-domain --lib zzz_no_such_test_xyz`, which
  blocks on the held lock inside the wrapper's `flock -w 3600`), observed the
  wrapper child (`flock -w 3600 -E 86 /tmp/gauntlet-build.lock -c cargo
  nextest …`) in the process tree, then `kill -9` on it.
- Observed: sweep completes; report row AK-930 = **FAIL**, `status_category:
  blocked`, `reason: "command exited -1"`, evidence log `exit: -1`; sweep exit
  code FAILURE. No such exit code exists — the child died on signal 9 and the
  code fabricated `-1`.
- Expected: an executor that cannot run a command (environmental) must be the
  typed INCONCLUSIVE (`cannot_run: …`), per charter decision 3 and the module's
  own honesty rules. This workspace is memory-tight WSL; an OOM kill of a
  waiting wrapper is a realistic event and would both fail the plan gate
  dishonestly and record `blocked` for an innocent unit — the exact
  FAIL-by-default the design forbids.
- Mechanism: `crates/gauntlet-cli/src/verify_cmd.rs:123`
  (`status.status.code().unwrap_or(-1)`) — the signal case is collapsed into a
  fake exit code before classification; only {86, 69} are remapped.
- Minimal correction: when `status.code()` is `None`, emit
  `ExecuteError::CannotRun { reason: format!("the sweep command was killed by
  signal {}", status.signal()) }`, and record the signal in the evidence log
  instead of `exit: -1`.

### F11. Held: the honest busy-lock path (executed at the adapter's behavioral tooth)

The adapter's own behavioral teeth (real `flock` over a temp lock, real holder
process, real inner executables) run inside
`gauntlet-adapter-verify-command-exit-code` lib tests
(`sweep_toolchain.rs:327-393`): wrapper executes inner under real flock;
inner exit preserved across the sccache stop; inner own-69 remapped to 169;
held lock reports LOCK_BUSY_EXIT(86) within a bounded wait. Executed output in
the session log. The CLI classifies 86/69 → `CannotRun` → INCONCLUSIVE
(`verify_cmd.rs:124-143`); the full end-to-end busy-lock through the CLI was
NOT executed because the production wait is fixed at 3600 s
(`LOCK_WAIT_SECONDS`), which is also why no honest lock-busy INCONCLUSIVE row
exists in any committed evidence log.

### F12. Held — bundle pin (executed in a scratch world, real script)

`scripts/pin-evidence-bundle.sh` in /tmp/gauntlet-falsifier/pin-world:
pin writes 6 entries; `--check` verifies from the repo root AND a foreign cwd;
one flipped byte in a pinned evidence log → `FAILED` + exit 1; tampered
conformance.json → exit 1; missing pinned input → pin mode refuses (typed
message, exit 1); the REAL repo bundle `--check` verifies (50 files). Scope
note: a file ADDED to the pinned evidence dir after pinning is invisible to
`--check` (the pin is a digest list, not directory closure; git visibility
remains) — executed and confirmed, recorded as a scope note, not a finding.



### F13. MINOR — the COMMITTED manifest's lib filters are loose substrings; AK-040's PASS is over-determined by 25+ unrelated tests

- Executed: `cargo nextest list -p gauntlet-app --lib settle` against the repo:
  the bare nextest filter token is a SUBSTRING match; `settle` matches 25+ tests
  across baseline::, compare::, control::controller::, memory::, promote:: —
  most of which have nothing to do with AK-040's claim ("the settlement
  precondition matrix"). AK-040's committed arm is exactly
  `execute: {package: gauntlet-app, lib: true, filter: settle}`
  (`conformance-manifest.yaml:358`).
- Consequence (attacker's reading): AK-040 stays PASS as long as ANY settle-
  substring test passes — even if every real settlement-precondition test
  failed; conversely the row FAILS if an unrelated settle-adjacent test
  regresses. Same-shape demonstration through the REAL binary: world-nextest
  AK-921 (frozen_locator unrelated to any real test) PASSed purely on
  `filter: role_id` (the four role_id lib tests).
- Context: the closed vocabulary validates only token SHAPE (`commands.rs:140`),
  not specificity; the committed manifest is sha256-pinned and its bases are
  honest, so this is authored-input looseness, not a live lie. Other loose
  committed filters: `doctor` (used twice), `precedence`, `reference::parse`
  (used deliberately by 3 units as a grouped suite), `compare::`.
- Minimal correction: disclose substring semantics in the codec doc comment and
  prefer exact-match filter operators (or require the sweep to assert via
  `cargo nextest list` that the filter matches a test whose name contains the
  frozen locator's last segment).

### F14. Held observation — no delivered test covers the F1/F2/F10 gaps

App conformance 22/22, CLI conformance 9/9, adapter lib 9/9, docs_present 7/7
all GREEN on the tree. None of them covers: duplicate manifest ids (F1),
test_report+artifact arm laundering (F2), or signal-death classification
(F10) — each was found only by attacking outside the delivered suites.

## Attacks that held (the executed surface that survived)

- Refusal matrix (F5): every structural manifest lie I could spell inside the
  codec — unknown id, missing id (both directions), kind mismatch, frozen-
  locator drift, `../` and absolute artifact paths, shell metacharacter in a
  filter, the `default` feature edge, relative program, program-with-args
  smuggling, file-kind+execute arm — REFUSES with a typed locus and writes NO
  report. Executed through the real binary.
- Three-state discipline: `/bin/false` → FAIL; exit-4 stand-in (the
  zero-tests-run class) → FAIL "command exited 4"; absent artifact →
  INCONCLUSIVE `no_artifact`; directory artifact → INCONCLUSIVE
  `unreadable_artifact`; empty artifact → predicate Refutes → FAIL;
  `missed > 0` mutation record → FAIL; missing shard array → FAIL. No
  environmental or evidence gap ever became PASS; no honest-FAIL became
  INCONCLUSIVE. Executed through the real binary (world-main/world-nextest).
- Command grouping: identical arms run once, fan out to every citing unit
  (13 units / 3 commands; 5 units / 5 commands). Evidence logs + MANIFEST with
  per-log sha256 land beside the report; PASS rows cite existing logs only.
- Plan-gate python asserts hold by construction (one row per ruler unit,
  three-state vocabulary, PASS ⇒ non-empty repo-relative evidence_ref on
  disk).
- COR-1 lock wrapper: options-before-file operand pinned by a test; inner exit
  preserved across the sccache stop; inner own-69 remapped to 169; a held temp
  lock reports 86 within a bounded wait (9/9 adapter lib tests, executed —
  note: two wrapper tests took ~270 s under load, consistent with the repo's
  recorded load-flakiness watch item).
- COR-2 feature edges: a surfaces-gated target WITHOUT the feature runs zero
  tests → nextest exit 4 → FAIL (real binary, AK-923); a feature edge the
  package does not declare → cargo exit 101 → FAIL (AK-922).
- handoff↔conformance grouping teeth (F9): all six tamper classes RED with
  named units, via the documented fixture env overrides.
- Docs claim teeth partially: canonical ahead-of-evidence claims, unknown ids,
  and bare-token claims are caught (F8 control cases).
- Bundle pin (F12): tamper → exit 1; missing input → refusal; foreign-cwd
  --check OK; the committed bundle verifies.
- The committed handoff/conformance pair is internally consistent right now
  (docs_present 7/7 GREEN on the real tree; AK-067 honestly INCONCLUSIVE in
  the evidence-of-the-moment window, settlement re-sweep owed).

## Suspicion list (code-cited, NOT executed repros)

- `reason` on a PASS row is not refused by `assemble_report`
  (`report.rs:178` checks only non-PASS); `run_sweep` never produces one, so
  only a direct `assemble_report` caller could smuggle it. Latent.
- `flock -c` script interpolates structural values unquoted
  (`sweep_toolchain.rs:147-151`): safe under the current closed vocabulary
  (plain tokens), but the invariant lives in the validator, not here — any
  future widening of `plain_token` becomes shell-relevant silently.
- Post-pin additions to the pinned evidence dir are invisible to
  `pin-evidence-bundle.sh --check` (executed; recorded in F12 as a scope note).
- A wrapper whose lock file cannot be OPENED (ENOSPC on /tmp, permissions)
  makes flock exit 1 → FAIL rather than an environmental INCONCLUSIVE (same
  defect class as F10; not executed against the real lock to avoid sabotaging
  shared state).

## Not attacked (honest scope)

- The full 70-unit real sweep end-to-end (~75 min; the orchestrator's
  settlement re-sweep covers it — my real-binary legs exercised every
  adjudication branch over fixture worlds plus 4 REAL nextest arms).
- The honest busy-lock INCONCLUSIVE end-to-end through the CLI (production
  wait is fixed at 3600 s; F11 executed the 86 path at the adapter level).
- `gauntlet verify --run` (pre-existing surface, unchanged by task 27) and the
  operator TUI workstream (out of scope per charter).
- `gauntlet export` (no durable repo-level run exists; the bundle IS the
  export per charter decision 10 — the pin itself was executed, F12).
- Fuzzing the mini_json reader or the answer-key parser beyond the malformed
  fixtures the consistency legs already refuse (I8 spot checks only).
- Memory/parallelism behavior of the real 45-command sweep under load.

## Environment discipline record

- No tracked file was modified, staged, or restored; every attack ran in
  /tmp/gauntlet-falsifier or through env-override fixtures the tests provide.
- One compile at a time under /tmp/gauntlet-build.lock throughout.
- Incident, disclosed: my FIRST ddriver invocation placed
  `sccache --stop-server` BEFORE cargo inside the flock, which let the freshly
  spawned sccache daemon inherit the lock fd and hold the compile lock after
  the command ended — wedging my own three subsequent flocks (silent 60 s
  timeouts). Recovered with `sccache --stop-server`; lock verified free
  (fuser) before continuing. This is exactly the task-20 lesson; the delivered
  wrapper's order (cargo first, stop-server after, inside the same locked
  command) is the correct one and my incident re-confirms its necessity.
- /tmp cleanup at end of round: /tmp/gauntlet-falsifier removed in full
  (fixture worlds, ddriver, pin-world, fixture copies, out/). Nothing else
  touched.
