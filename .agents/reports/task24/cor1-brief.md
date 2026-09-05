# Task 24 — COR-1 brief (correction round 1)

You are the COR-1 builder for task 24 (anti-gaming suite G1–G16 +
mutation teeth) in /home/sprime01/projects/gauntlet. The blind
design-as-built critic returned NOT_CONFIRM
(`.agents/reports/task24/critic-design.md` — read it in full; its
findings are your work order). Apply the smallest honest correction
for each finding, RED-first where a test changes. Do NOT re-litigate
confirmed-sound findings; do NOT touch anything outside the correction
scope.

## C1 (F-1, MAJOR — the G7 exclusion is wider than true equivalence)

The manifest's G7 exclusion (`.agents/reports/defended-mechanisms.txt:31-35`)
claims the `<` mutants at `posture.rs:136:42` and `:139:50` are
equivalent, and `scripts/run-mutation.sh:99`'s `-E` regex excludes ALL
mutants at those positions — but only the two `<=` mutants are
genuinely equivalent. The `==` (blast arm), `>` (both arms), and `==`
(authority arm) mutants are non-equivalent; three are provably caught
by the shipped g7 test (critic's analysis), and `==` at `:139:50`
(operator authority decrease) has NO shipped leg.

Repair (all three parts):
1. Strengthen `antigaming::g7_an_agent_role_cannot_downgrade_a_units_tier`
   with an operator AUTHORITY-decrease leg (operator, decrease on the
   authority axis only → refused; mirror the existing blast-decrease
   leg AK-9505) so the `==`-at-139:50 mutant is caught. RED-first:
   demonstrate the new leg goes RED against the pre-fix `==` behavior
   before trusting it green (a temporary local mutation, restored
   byte-identically, sha256-verified — the established teeth drill).
2. Shrink the `-E` regex to exactly the two genuinely equivalent `<=`
   mutants (check `cargo mutants --list` for the real mutant-name
   format and match the replacement, not just the position).
3. Correct the manifest comment to state the true justification: only
   `<=` is equivalent (ranks differ whenever the arm evaluates); name
   each excluded mutant precisely.

## C2 (F-2, MINOR — no delivered working recipe for the suite gate)

The plan gate's literal `cargo nextest run --test-threads 4
antigaming::` matches zero tests (nextest parses `::`-positionals as
package selectors; verified exit 4 — fails closed). The manifest
header's sentence claiming the filter selects these tests is FALSE.
Repair: correct the manifest header (and the suite header if it
repeats the claim) to state the honest shape: test ids are
`antigaming::<fn>`, the plan's positional spelling is vacuous on this
nextest (recorded divergence, fails closed), and the working gate is
`cargo nextest run --test-threads 4 --test antigaming`. Keep it
short — this is a documentation honesty fix, not prose.

## C3 (F-3, MINOR — mutation.json must certify the delivered tree)

After C1/C2/C4/C5 land and the suite is green, re-run
`./scripts/run-mutation.sh` so `.agents/reports/mutation.json` is
canonical for the final tree (suite-green-first ordering; `--baseline
skip` is legitimate only inside that ordering). Assert missed == 0.
Expected delta vs the prior run: the G7 shard gains the four
non-equivalent mutants (three already-caught + one newly caught by the
C1 leg), the widened C4 spans add the interior mutants. Any survivor
is a suite defect: strengthen the named test RED-first until caught,
or record a TRUE exclusion justification (equivalence proven against
the source, not asserted).

## C4 (F-4, MINOR — unexamined catchable line-level mutants)

Widen the mutation spans: G16's `earlier_record_failures` full fn body
(gate.rs ~877–905 — includes `888:7` `&&`→`||`, `888:10` delete `!`,
`900:7` `&&`→`||`, `900:20` `>`→`==`/`<`/`>=`) and G1's version check
(contract.rs ~307:20 `!=`→`==`). Add both to the manifest's
source_location set (the G16 entry may name the fn's span; keep the
format valid for the validator test). If widened spans surface
survivors, triage honestly per C3.

## C5 (I-2a sub-note, smallest honest repair)

`earlier_record_failures`' early `return failed` on an unparseable
earlier-record metric (gate.rs ~887-889) also skips that record's
payment re-derivation, which does not depend on the declared metric.
Restructure so each check derives independently (e.g., per-check
guards instead of the early return) — smallest typed change, existing
g16 tests must stay green, and the widened C4 span now examines it.
If you instead judge the early return defensible, record the exact
reason in your report; do not silently keep it.

## Discipline (binding)

- RED-first for every test strengthening; temporary mutations restored
  byte-identically with sha256 verification recorded in your report.
- Every cargo/just invocation: `flock .agents/reports/task24/compile.lock
  -c '<command>'` — flock never nested inside another flock's -c
  string; `sccache --stop-server` inside the same flock'd command.
- `just ruler` before and after; if it fails, STOP.
- Memory: cargo-mutants `-j` at most 2; measure `--list` first.
- Checkpoint to `.agents/reports/task24/cor1-progress.md` as you go;
  completion contract = `.agents/reports/task24/cor1-report.md` (files
  touched + sha256, RED-first evidence, battery outcomes, mutation
  summary before/after, deviations, open findings).
- Do NOT touch: settled proof files (g6/g7/g10/p*.rs other than
  reading), ruler files, CURRENT_STATUS.yml, justfile, dependencies,
  anything outside the C1–C5 scope.
- Slice battery before reporting: `just ruler`; `cargo nextest run
  --test-threads 4 --test antigaming` 18/18 (or 19/19 with the new
  leg — count and report); `just lint`; `just boundaries`;
  `./scripts/run-mutation.sh` + the python assert; `just check-package
  gauntlet-cli` and `just check-package gauntlet-app`.
