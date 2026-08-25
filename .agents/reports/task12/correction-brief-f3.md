# Task 12 — correction FINISHER F3: activated-trigger closes + full gates

You are a correction finisher in `/home/sprime01/projects/gauntlet`.
**You are slice 3 of 3.** F1 (C5 verify/record, C6, C7, N6) and F2 (C8, C9,
N3, N4, N5) have landed — read both result contracts first.

Your two code items are small and live in surfaces the other slices did not
touch. Your third job is the **full workspace gate sweep** that closes the
correction round.

## Read first

1. `AGENTS.md`
2. `.agents/reports/task12/residual-readjudication.md` — N1 and N2 are
   composed there, with the reason each trigger fired
3. `.agents/reports/task12/correction-f1.md`, `correction-f2.md`, and the F1/F2
   entries in `correction-checkpoints.md`

## Method

Fail-first per item. **Declare-first checkpointing** to
`correction-checkpoints.md`: append `## <item> — STARTED <timestamp>` with your
intended design *before* editing, and the full `## <item> — DONE <timestamp>`
entry after — see `.agents/lessons/unrecorded-landed-work.md` for why
write-after-completion loses a whole item when a session dies. Before
implementing, run `./scripts/reconcile-checkpoints.sh 12` and read what it
flags: anything listed was modified after the last checkpoint write, so the
record cannot account for it. Result contract
`.agents/reports/task12/correction-f3.md`. Bounded claims only.

## Items

### 1. N1 — doctor check 2's manifest clause is vacuous

**Why this is in scope now** (it was deferred pre-implementation on a reason
the tree has since falsified — see `residual-readjudication.md`): the ledger
entry's trigger is "next doctor touch **or Task 12 (config/reference surface
work)**", and Task 12 widened the config canonical/fingerprinted form
(`config/canonical.rs`, six new `ConfigKey` variants, new Annex-E env names)
and the reference provider (`reference/provider.rs` — new `pub fn read_text`,
`describe` made public). The trigger fired on its own terms.

Two defects, both in `crates/gauntlet-app/src/doctor/checks.rs`:

- `top_level_mapping_keys` returns `Ok(0)` for a file whose every line is
  indented — and the clause reports that as **PASS** ("top level is a mapping
  (0 keys)"). A mapping with zero top-level keys is not a mapping.
- The clause never runs the strict `parse_manifest` that Task 7 built and that
  the provider path already uses, so a manifest that would fail deep parse
  passes doctor check 2.

Deliverable: make the clause non-vacuous and run the strict parse, reporting a
typed `reference`-class failure; then promote the doctor scratch fixture
(`doctor/mod.rs::scratch_manifest`, currently "shallow on purpose") to a
manifest that deep-parses. Fail-first on both legs.

**GOVERNED STOP CONDITION — read before you start.** If the *real*
`.wayfinder/reference.yaml` fails the deep parse, that is a finding about the
frozen reference, not a licence to loosen the parser or the clause. STOP,
record an evidenced entry in `.agents/reports/reference-candidates.md` naming
the affected Units/tasks and whether it is a new defect class or another
instance of an existing candidate class, and report it. `just doctor` must be
green at the end **without** weakening anything; if it cannot be, that is the
stop condition, not a target to force.

### 2. N2 — observe-command spawn-classification test

Ledger trigger: "next observe-command touch" — fired. The adapter gained
`run_bounded`, `drain_capped` and an output ceiling this task (+~305 lines),
and Task 12's own C4 proof *leans on* this classification ("the REAL command
observer over an absent program").

The adapter documents that a program which cannot spawn "could not run at all"
→ `Unavailable`, but its own `mod tests` never exercises it — the property is
discriminated only cross-crate, from a CLI proof. Add one in-crate test
pinning `ObserverError::Unavailable` (or whatever the typed classification
actually is — read it, do not assume) for a non-spawnable program, in the same
`mod tests` that now holds the deadline/ceiling tests.

Fail-first here means: confirm the test *would* catch a misclassification —
briefly flip the classification, observe the new test fail, revert
byte-identical, re-run green. Record both observations.

### 3. Full gate sweep

Run every gate, one at a time, and record each output verbatim in
`correction-f3.md`:

```
just check
just test
just lint
just deny
just boundaries
just ruler
just doctor
cargo nextest run --test p4_corrective_loop --test p6_budget_control
cargo nextest run -p gauntlet-app control::
just prereg-check 12
```

Expected: `boundaries` **197/197** (or grown only through a recorded act — if
it grew, say exactly which act and why); `ruler` **PASS**; `prereg-check 12`
PASS. Report the workspace test total.

**A red gate at session end is reported loudly and never weakened.** If a gate
is red and the fix would require changing an assertion, a gate definition, or
the frozen reference, STOP and record.

## Hard rules

Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
`AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, the frozen
prereg files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`. **No commits.**

You hold the single-compiler build lock: never run two cargo/nextest commands
concurrently. Memory-constrained WSL — do not raise build or test
parallelism. If a test is OOM-killed, diagnose the failing process; do not
retry with more parallelism.

Do not fix items outside this brief. In particular leave alone the three
doctor/lock **message** nits (B-F10/F12/F13) — `residual-readjudication.md`
records them as later-scope because their loci are in CLI/doctor surfaces this
task does not modify, and N1 does not reach them.
