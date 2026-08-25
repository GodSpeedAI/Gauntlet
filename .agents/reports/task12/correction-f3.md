# Task 12 — correction FINISHER slice F3 — result contract

Scope: `correction-brief-f3.md` items 1–3. Single-compiler build lock
held throughout; no concurrent cargo/nextest; no sub-agents. No
commits. No edits to `.wayfinder/**`, `.agents/specs/**`,
`.agents/plans/**`, `AGENTS.md`, `justfile`, `deny.toml`,
`.agents/CURRENT_STATUS.md`, the frozen prereg files,
`.agents/prompts/**`, `.tmp/**`, `.jolli/**`.

`./scripts/reconcile-checkpoints.sh 12` was run FIRST ("OK — no
task-surface file was modified after the last checkpoint write").
Per-item fail-first evidence, designs, and gate outputs are in
`.agents/reports/task12/correction-checkpoints.md` (entries
"Correction round 1 — FINISHER slice F3"). This file is the contract
summary.

## Status by item

| Item | Subject | State |
|---|---|---|
| 1 | N1 — doctor check 2's manifest clause non-vacuous + strict parse + fixture promotion | **DONE** |
| 2 | N2 — in-crate spawn-classification test (observe-command) | **DONE** (flip-discriminated) |
| 3a | Gate repair — 18 arch-scanner violations in the round's own `controller.rs` | **DONE** (found by item 3; fixed without weakening anything) |
| 3 | Full workspace gate sweep | **DONE — ALL TEN GATES GREEN** |

No STOPPED items; no assertion, gate definition, scanner rule, or
frozen surface was weakened. The N1 **governed stop condition did NOT
fire** (evidence below).

## Item 1 — N1: the manifest clause is no longer vacuous

Two defects, two fail-first observations (both `left: Pass /
right: Fail` against the shipped clause — verbatim in the
checkpoints):

1. **Vacuity:** `top_level_mapping_keys` returned `Ok(0)` for an
   all-indented file and the clause reported PASS ("top level is a
   mapping (0 keys)"). Now `Ok(0)` is a FAIL whose evidence names the
   vacuity ("zero keys is not a pass").
2. **No strict parse:** the clause never ran the strict
   `parse_manifest` the provider path uses (`reference/provider.rs`).
   Now, after the shape gate passes, it runs the same parser; an
   `Err` is a FAIL carrying the reference class and the typed
   `ManifestError`'s line and reason; a PASS reports the parsed
   identity (id, version) so success evidence is non-vacuous too.

**Fixture promoted:** `doctor/mod.rs::scratch_manifest` (documented
"shallow on purpose") is now a minimal frozen-shape manifest
(`scratch-ref-01`) that deep-parses; the scratch lock hashes the same
bytes, so it followed automatically. One knock-on moved WITH the
semantics (F2's precedent, not a weakening):
`a_moved_manifest_fails_check_two_as_a_reference_class_failure`
rewrote the manifest to a shallow one-liner and asserted the manifest
clause still passes — under the deep parse that rewrite is itself a
manifest failure, so the moved bytes are now a deep-parseable prose
rewrite; the test's assertions (manifest clause PASSES, digest clause
alone FAILS) are unchanged and keep their original meaning.

**Stop condition, closed with evidence:** the real
`.wayfinder/reference.yaml` deep-parses — pinned in-tree by
`reference/manifest.rs::the_frozen_manifest_parses_with_its_fields_intact`
BEFORE this slice, and now verified through the REAL doctor path at
item 3's `just doctor`: green, evidence line "deep-parses under the
frozen reference shape (id gauntlet-core-bootstrap, version 1)". No
`reference-candidates.md` entry; the parser was never touched.

## Item 2 — N2: in-crate spawn-classification test

The typed classification was read, not assumed:
`ObserverError::Unavailable { detail }`, `detail` naming the spawn
failure (`run_bounded`'s `map_err` arm — "C8: a program that cannot
spawn 'could not run at all'"). New in-crate test in the adapter's own
`mod tests` (beside the deadline/ceiling tests):
`a_program_that_cannot_spawn_is_unavailable_not_a_failed_instrument`,
driving the REAL `CommandObserver::observe` end-to-end over a live
scratch workspace, real `WallClock`, future deadline, `NullStore`
(never reached), and a `./`-relative absent program name so PATH is
bypassed and the spawn failure is deterministic.

Fail-first here is DISCRIMINATION, observed both ways:

- **Green** against the shipped classification:
  `10 tests run: 10 passed`.
- **Flip** (`Unavailable` → `InstrumentFailed` at the spawn arm,
  nothing else): the test failed exactly on the classification —
  `panicked … a non-spawnable program is an unavailable instrument,
  got Err(InstrumentFailed { kind: Test, detail: "spawning the
  command instrument failed: No such file or directory (os error 2)" })`.
- **Revert byte-identical** (sha256 equal before/after, `diff` clean
  against a pre-flip snapshot), **green again**: `10 tests run:
  10 passed`. Zero production-code change stands.

Honest scope note: the property is also discriminated in the package's
integration target (`tests/shared_contract.rs`, Task 9) and composed
in p4's C4 proof — both link the adapter externally. The in-crate
unit tooth the brief specified did not exist within the scope searched
(`src/lib.rs`'s `mod tests`); it now exists and is flip-discriminated.

## Item 3a — gate repair found by the sweep (recorded, in-brief)

The sweep's `just test` ran RED:
`core_crates_contain_no_hardcoded_literals`, **18 violations, all in
`crates/gauntlet-app/src/control/controller.rs`** — a file absent
from HEAD, i.e. entirely this correction round's code (F1/F2),
uncaught because F1/F2 ran package-scoped gates + `just lint` only;
the arch scanner runs workspace-wide. No F3 file was implicated.
Decomposition (verified against the scanner's own rules):

- 15x endpoint flags `'core-spec.md:835'` — root: TWO comment lines
  (F1's C6.4 records) citing `` (`Core-spec.md:835`) ``; the endpoint
  rule lowercases units and matches dotted-host:digits, and statement
  unit-joining multiplies the two citations into 15 flags. Repo-wide,
  `Core-spec.md:NNN` occurs in scanned source nowhere else (bounded
  search declared).
- 1x relative-path literal — F2's N4 test assertion needle
  `'../leak-ctrl-9101.json'`.
- 2x timeout-shaped numbers — F2's C9.4 test (comment "= 30" beside
  "stall"; `const … = 32` beside "poll").

Repair, all mechanical conformance, **no assertion's meaning, gate
definition, scanner rule, exemption list, or frozen file touched**:
citations reworded to `` §11.3 of `Core-spec.md` `` (section number
stays); the N4 needle BUILT not spelled per the repository's own
established idiom (the doctor suite's AK-016 needle comment) with the
asserted value byte-identical; the derivable "= 30" dropped from a
comment; the spin budget derived as `DERIVED_DRAIN_PASS_BOUND as u32
+ 2` (the same 32, now expressing its load-bearing relation "two past
the bound"). The arch suite itself was NOT modified — the
`residual-readjudication.md` deferral of the T3 arch-suite residuals
stands; this is the round's own code conforming to the frozen
scanner.

`just lint` then flagged ONE fmt drift — F3's own new adapter test
line; `cargo fmt` touched exactly that one file (verified against the
pre-run `--check` list, F1's discipline), `just lint` green after.

## Files touched

- `crates/gauntlet-app/src/doctor/checks.rs` (item 1)
- `crates/gauntlet-app/src/doctor/mod.rs` (item 1: fixture promotion,
  two new tests, one moved fixture)
- `crates/adapters/gauntlet-adapter-observe-command/src/lib.rs`
  (item 2: one new test; production code unchanged — flip reverted
  byte-identical; plus a fmt wrap on the new test)
- `crates/gauntlet-app/src/control/controller.rs` (item 3a: two
  comment rewordings + two built test figures; zero production-logic
  change)
- `.agents/reports/task12/correction-checkpoints.md`, this file

## New and changed tests

| Test | Item |
|---|---|
| `doctor::tests::an_all_indented_manifest_is_not_a_mapping_and_fails_check_two` | 1 (new) |
| `doctor::tests::a_manifest_that_cannot_deep_parse_fails_check_two_as_a_reference_class_failure` | 1 (new) |
| `a_moved_manifest_fails_check_two_as_a_reference_class_failure` | 1 (fixture moved with the semantics; assertions unchanged) |
| `gauntlet-adapter-observe-command tests::a_program_that_cannot_spawn_is_unavailable_not_a_failed_instrument` | 2 (new, flip-discriminated) |

Suite growth: doctor 25 → 27; observe-command package 9 → 10;
workspace 909 → 911.

## Gates — the full sweep, verbatim (all run personally, sequentially, on the final tree)

```
$ just check
cargo check --workspace --all-targets
    Checking gauntlet-adapter-compare-model v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-model)
    Checking gauntlet-adapter-compare-diff v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-diff)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Checking gauntlet-adapter-agent-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-agent-cli)
    Checking gauntlet-adapter-agent-prime v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-agent-prime)
    Checking gauntlet-ports v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-ports)
    Checking gauntlet-domain v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-domain)
    Finished `dev` profile [unoptimized] target(s) in 42.98s
EXIT=0

$ just test
     Summary [  48.954s] 911 tests run: 911 passed, 0 skipped
EXIT=0
(first run RED — the item 3a arch violations, repaired; final re-run
on the final tree after the fmt wrap:
     Summary [  47.950s] 911 tests run: 911 passed, 0 skipped — EXIT=0)

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-adapter-observe-command v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-observe-command)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 9.79s
cargo fmt --check
EXIT=0
(first run RED on one fmt drift — F3's new adapter test line; cargo
fmt touched only that file; this is the green re-run)

$ just deny
advisories ok, bans ok, licenses ok, sources ok
EXIT=0

$ just boundaries
     Summary [  41.718s] 197 tests run: 197 passed, 0 skipped
EXIT=0

$ just ruler
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
EXIT=0

$ just doctor
  2. reference                pass
      - manifest             pass
          manifest exists; top level is a mapping (1 keys); deep-parses under the frozen reference shape (id gauntlet-core-bootstrap, version 1)
      - answer key           pass
          answer key exists, non-empty, 70 units (frozen count 70)
      - unit schema          pass
          70/70 units well-formed under the 5.3.2 schema (tier/authority coupling included)
exit: OK
report: /home/sprime01/projects/gauntlet/.agents/reports/doctor.json
EXIT=0

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   1.284s] 6 tests run: 6 passed, 0 skipped
EXIT=0

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.453s] 73 tests run: 73 passed, 227 skipped
EXIT=0

$ just prereg-check 12
./scripts/prereg-check.sh
prereg-check.sh: PASS — task 12 preregistration matches its frozen hash (402e444ae08a86ca7e4ba2dd4dd7f1b0161608fe201a78dd5e7b096ba8ec8486)
EXIT=0
```

- **Boundaries: 197/197 — exactly the expected count, NO growth** (F3
  added no arch-suite test; the new tests are crate unit tests).
- **Ruler: PASS.** **prereg-check 12: PASS.**
- **Workspace test total: 911 tests run: 911 passed, 0 skipped.**

## Deliberately left alone

- The three doctor/lock **message** nits (B-F10/F12/F13) — per brief;
  N1 touched `doctor/checks.rs`'s manifest clause only.
- The T3 arch-suite guard-evasion minors — the standing deferral; the
  arch suite was not modified (item 3a conformed the round's code to
  the frozen scanner instead).
- Everything F1/F2 recorded as deliberately left alone (tier posture /
  Task 14, §21.3 degradation policy, completion-path zero ask,
  `exercisable_authority` ceiling, critic-A m4's vacuous p4 assertion,
  Task-13 settlement surfaces, L2 retry-ledger persistence).

## Teeth note (for the orchestrator's closure re-run)

- No tooth's break site was touched by F3 (controller.rs edits are
  comments and test figures; T3's `reread_answer_key` body and T2's
  `boundary_verify` are unchanged).
- The arch-scanner repair (item 3a) means any tooth scripted against
  the OLD comment text or the spelled `../leak-ctrl-9101.json` literal
  needs its break re-expressed; `teeth.md` names none of those lines.
- Doctor check 2's manifest clause is now deep-parse-backed: any
  future scratch fixture with a shallow manifest will fail it (that is
  the point of N1).

F3 slice complete; the correction round's gate sweep is green.
