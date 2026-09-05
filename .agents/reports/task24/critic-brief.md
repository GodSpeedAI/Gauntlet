# Task 24 — blind design-as-built critic brief

You are the BLIND design-as-built critic for task 24 (anti-gaming suite
G1–G16 + mutation teeth) in /home/sprime01/projects/gauntlet. Fresh
context, READ-ONLY: you modify no tracked file. Running tests and
inspection commands is allowed (compile discipline below). You have no
builder history by design — judge only the delivered tree.

**Blindness rule:** do NOT open `.agents/reports/task24/b1-report.md`,
`b1-progress.md`, `mutation-scratch/`, or `mutation-run.log` — those
carry the builder's reasoning. Everything else under
`.agents/reports/task24/` (charter, this brief) is your contract.

## Your inputs

1. The charter: `.agents/reports/task24/charter.md` (mission,
   deliverables, honest-boundary rule, out-of-scope list).
2. Spec `.agents/specs/Core-spec.md` §23.1 (teeth rule, verbatim:
   "Each proof MUST have a **teeth check**: disabling the defense makes
   the proof fail."), §23.2 (the sixteen strategies), §16 (settlement),
   §19.4 (self-improvement gate), §13.5 (aggregation).
3. Ruler units: `.wayfinder/answer-key.md` AK-058 + AK-070 (read their
   claims verbatim; the ruler itself is frozen — never edit).
4. The delivered tree:
   - `crates/gauntlet-cli/tests/antigaming.rs` (the suite)
   - `.agents/reports/defended-mechanisms.txt` (the teeth manifest)
   - `scripts/run-mutation.sh` + `.agents/reports/mutation.json`
   - `git diff` on `crates/gauntlet-app/src/promote/gate.rs` (a
     re-adjudication change) and
     `crates/gauntlet-arch-tests/src/optimizer.rs` (token extension),
     plus `.gitignore`.
   - Context sources for the defended mechanisms: the named
     `source_location` files themselves.

## What to adjudicate (compare design-as-built against the contract)

1. **AK-058 honesty, per strategy G1–G16:** does each test actually
   bind to the manifest's named mechanism — would neutralizing that
   mechanism (a plausible mutation) turn THAT test RED? Flag any test
   that would stay green with its defense removed (the §23.1 failure
   mode), any G whose "defense" is really a tautology, and any G whose
   test overclaims depth it does not have (suite-level tests may be
   thinner than the deep proofs they cite, but must still bite).
2. **The G9 advisory disposition:** the honest-boundary rule says an
   advisory defense must never pass as proven. Judge whether the g9
   test + manifest entry are an honest recorded-weakness posture or a
   disguised fake.
3. **The promotion-gate change** (gate.rs): the §23.2 G16 strategy
   ("Re-ask a model until it returns `pass`") realized at the §19.4
   layer. Verify RED-first plausibility (does the pre-change shape
   actually adjudicate the re-rolled record?), that the sweep's
   semantics preserve the §19.4 contract (evidence gaps ⟹ provisional,
   never promoted; failures ⟹ rejected; the reasons vector stays
   in spec-order; the honest road — a gap superseded by a fresher
   complete record — stays open), and that it is the smallest typed
   change. Read the g16 tests against the implementation.
4. **The manifest:** format validity, all sixteen G-IDs covered, every
   `source_location` existing and actually containing the described
   mechanism, the exclusion justifications at the top (are the claimed
   equivalent-mutant / out-of-scope / fail-safe arguments TRUE — check
   each against the source), and whether any entry's mutation scope is
   silently narrower than its claim.
5. **The mutation pipeline:** `scripts/run-mutation.sh` — does it
   actually scope cargo-mutants to the manifest's loci, does the
   produced `mutation.json` honestly represent outcomes (missed == 0
   asserted), is `--baseline skip` legitimate given the tree's gates,
   is the per-mutant test invocation actually executing the 18-test
   suite (a zero-test "caught" is bogus evidence), and are the three
   recorded `-E` exclusions applied as documented? AK-070's claim is
   about suites detecting defects in the mechanisms they guard —
   judge whether the delivered evidence proves that claim or a weaker
   one.
6. **Plan-gate shape:** the plan's literal gate lines vs what actually
   runs. Any divergence must be honest, documented, and of the
   recorded-divergence class (tool-vs-plan spelling), never a silent
   weakening.
7. **Scope discipline:** no settled proof files (g6/g7/g10/p*.rs),
   ruler, or out-of-scope surfaces touched; no new dependencies; the
   arch-scan extension consistent with its negative-control style.

## Verdict

Write `.agents/reports/task24/critic-design.md` FIRST (full findings,
each classified BLOCKING / MAJOR / MINOR / INFORMATIONAL, with
file:line evidence), then return a one-paragraph pointer. Verdict is
one of CONFIRM / NOT_CONFIRM. NOT_CONFIRM requires at least one
BLOCKING or MAJOR finding. Static analysis + running the shipped tests
is your instrument set; you do NOT perform mutation neutralization
(the executing falsifier's territory) — but you MAY use
`cargo mutants --list`-style inspection read-only.

Compile discipline (binding): every cargo/just invocation is
`flock .agents/reports/task24/compile.lock -c '<command>'` — flock
NEVER nested inside another flock's -c string — and run
`sccache --stop-server` inside the same flock'd command afterward.
`just ruler` must pass; if it ever fails, stop and report immediately.
