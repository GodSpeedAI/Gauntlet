# Task 24 — blind EXECUTING falsifier brief

You are the BLIND EXECUTING falsifier for task 24 (anti-gaming suite
G1–G16 + mutation teeth) in /home/sprime01/projects/gauntlet. Fresh
context: you are blind to how the tree came to be — never open
`.agents/reports/task24/{b1-report,b1-progress,critic-design,critic-brief,cor1-report,cor1-progress,cor1-brief}.md`.
Your inputs are the charter
(`.agents/reports/task24/charter.md`), this brief, the spec
(`.agents/specs/Core-spec.md` §13.5/§16/§19.4/§23.1/§23.2), the ruler
(`.wayfinder/answer-key.md` AK-058/AK-070 — read-only), and the
delivered tree. You EXECUTE attacks: temporarily neutralize defenses
and watch the named tests go RED, then restore byte-identically. You
never leave a tracked file modified.

**Your mission:** find a way the delivered anti-gaming suite is NOT
what it claims. Every attack you execute that does NOT go red is a
finding. Every attack that goes red is a repelled attack (record it).
Restore after every neutralization (sha256 before/after in your log).

## Delivered artifacts under attack

- `crates/gauntlet-cli/tests/antigaming.rs` (18 tests, one per
  strategy G1–G16 plus a two-leg G16 and a manifest validator)
- `.agents/reports/defended-mechanisms.txt` (teeth manifest: test_id |
  G-ID | mechanism | source_location; exclusion justifications at
  top)
- `scripts/run-mutation.sh` + `.agents/reports/mutation.json`
- Production diffs: `crates/gauntlet-app/src/promote/gate.rs`
  (re-adjudication sweep), `crates/gauntlet-arch-tests/src/optimizer.rs`
  (E-1 token extension), `.gitignore`

## Attack families (execute at least these; extend as you see holes)

**T1 — per-strategy teeth (AK-058's core claim).** For EACH of the
sixteen strategies: neutralize the manifest's named mechanism at its
named source_location (a minimal hand mutation — delete an arm, flip a
comparison, return early) and run the named test. The test MUST go
RED. If any test stays GREEN with its defense neutralized, that is a
CRITICAL finding (the §23.1 failure mode: a proof that passes with its
mechanism removed). Restore byte-identically after each (sha256).
Prioritize the strategies whose defense is NOT protected by the
mutation run's examined scope — but sample all sixteen at least once;
for G9 (advisory) attack the ADVISORY CLAIM instead: try to widen the
FilesystemMode vocabulary or forge an enforcement that does not exist,
and judge whether the g9 test still honestly describes reality.

**T2 — manifest attacks.** Corrupt the manifest in each direction and
predict what the validator + script do: a wrong line number; a missing
G-ID; an entry whose test_id belongs to a different target; a
justified exclusion whose justification is false (verify each recorded
exclusion's equivalence claim yourself against the source); a
source_location that exists but contains something else. Any direction
that passes silently is a finding.

**T3 — pipeline attacks (AK-070's claim).** Attack the mutation
evidence itself: (a) does `scripts/run-mutation.sh` fail loudly if a
mutant survives? (instrument this safely — e.g., run one shard with a
deliberately broken test expectation in a THROWAWAY copy of the tree,
not in place; or neutralize a defense in place, run the script scoped
to that shard, observe MISSED ≠ 0, restore); (b) is any "caught" the
bogus zero-test class (Test phase that runs nothing)? (c) does the
script's merge assert honestly count unviable/timeout outcomes? (d)
does mutation.json's manifest_sha256 actually pin the delivered
manifest?

**T4 — the promotion-gate sweep (the G16 defense).** Adversarial
shapes against `earlier_record_failures` + the adjudication loop:
failing-then-passing laundering (must REJECT); gap-then-complete
honest road (must still PROMOTE); a poisoned earlier record (unparseable
metric, missing arm digests); re-evaluation AFTER a final decision
(must hit AlreadyFinal); duplicated evaluation records; a stale
record's margin/payment figures vs the fresh record's; the
two-sweep-shard boundary (280:292 vs 842:912) — is any line between
the shards mutable-without-detection? Also: does the sweep respect
"failed ⟹ Rejected, gaps ⟹ Provisional" for EVERY condition, or does
an earlier-record failure ever leak into a Provisional?

**T5 — regression sweep.** After all your attacks and restores: full
`cargo nextest run --test-threads 4 --test antigaming` (18/18),
`cargo nextest run -p gauntlet-app promote::` (46/46), `cargo nextest
run -p gauntlet-cli --test improve_promote_gates` (1/1),
`cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment`
(4/4), `just boundaries` (237/237), `just lint`, `just ruler` — all
green on the restored tree, and `git status` shows no tracked
modification you made.

## Discipline (binding)

- EVERY neutralization is temporary and restored byte-identically with
  sha256 proof in your log. `posture.rs` currently hashes
  `560e0ebc0dfff36c593355f14f07f458bea9f6f9f5edf4d9ba1b56fa7e17e3ee`;
  record the same for every file you touch before first touch.
- Every cargo/just invocation:
  `flock .agents/reports/task24/compile.lock -c '<command>'` — flock
  NEVER nested inside another flock's -c string (self-deadlock);
  `sccache --stop-server || true` INSIDE the same flock'd command.
  Exception: `./scripts/run-mutation.sh` takes the lock itself via its
  own top-level `exec 9>lock; flock 9` — invoke it DIRECTLY, never
  under another flock.
- cargo-mutants `-j` at most 2; measure with `--list` before any run;
  budget your mutation runs (one scoped shard to answer T3a is enough
  — do not re-run the full pipeline unless a finding requires it).
- `just ruler` at start and end; if it ever fails, STOP and report
  immediately (that outranks everything).
- No new dependencies; no edits to justfile, settled proof files
  (g6/g7/g10/p*.rs — reading them is fine and encouraged for T1
  context), ruler files, or any file outside temporary neutralizations.
- Probe hygiene: anything you plant under `.gauntlet/policies/` or in
  scratch dirs must be removed and proven removed (`ls`) before you
  finish.

## Deliverable

Write `.agents/reports/task24/falsifier-round1.md` FIRST: the executed
attack log (per attack: neutralization, sha256 before/after, test
result RED/GREEN, file:line evidence), findings classified
CRITICAL / MAJOR / MINOR / INFORMATIONAL, the regression-sweep
results, and your verdict: CONFIRM (no finding ≥ MAJOR) or
NOT_CONFIRM (≥1 MAJOR/CRITICAL). Final message: pointer + verdict +
counts.
