# Task 14 slice B3 — result contract (orchestrator-implemented under heal+continue)

**Provenance:** sub-agent window still exhausted; orchestrator-implemented per
the standing heal+continue instruction, personally verified.

## What landed

| File | Purpose |
|---|---|
| `crates/gauntlet-cli/tests/p5_hard_gate.rs` | The P5 proof, cargo test-target name exactly `p5_hard_gate`: 4 legs — `irreversible_never_dispatched` (AK-044), `a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged` (AK-045 + AK-050 through the composed loop, with the contained-tier control dispatching under the §8.6 conservative posture), `a_sandbox_validation_failure_escalates_without_falling_back` (AK-047 over the REAL backend with a decodes-but-fails policy + the §8.6 NoBackendForTier arm + the loop-composed refusal), `containment_wraps_from_process_start_and_confines_the_session` (AK-048 EXECUTED: real bwrap, in-root write OK, out-of-root denied, denied path masked, backend-first process, admission carries the clean validation; plus the SHIPPED `.gauntlet/policies/*.json` artifacts composed through the REAL provider with the top tier structurally refused). |
| `crates/gauntlet-cli/tests/g7_tier_downgrade.rs` | The AK-046 proof, cargo test-target name exactly `g7_tier_downgrade`: 3 legs — the lattice review over every direction×origin (agent moves refused in EVERY direction incl. raises; operator raises only), the dispatch-gate defense (forged downgraded facts refused with the AK-046 rule in the recorded summary; coincident facts pass), and the composed frozen-ruler defense (local-tier refusal → attacker downgrades the tier in the answer key between runs → `resume_run` re-freeze-verifies FIRST → `aborted{Mutated}`, the lowered tier NEVER dispatches). |

## Composition inventory

Both proofs compose the REAL embedded-SQL state store (behind a delegating
invocation counter — "never dispatched" is a count of DURABLE invocation
rows), the REAL fs evidence store, the REAL file reference provider, the
REQUIRED lying-builder mock scenario (an honest §22.2 none-isolation
in-process runner — exactly the capability fact AK-050 consumes), the REAL
govern gate, and — in the gate-composed legs — the REAL process-isolation
backend adapter and the REAL fs policy provider with the backend token
derived at runtime (never spelled; the boundary suite denies it outside
adapters). The p5 wraps leg EXECUTES real bwrap confinement.

## Teeth (proven on the FINAL tree, RED → byte-identical restore → GREEN)

1. **AK-050 capability conjunct**: `if false && facts.runner_isolation < required` →
   `a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
   FAILED (the local dispatch would proceed); restore → PASS.
2. **AK-044 double line**: `false &&` on BOTH the scheduler's conjunct 3 and
   the gate's conjunct 1 → `irreversible_never_dispatched` FAILED (an
   invocation was minted); byte-identical restore → PASS. (Single-line
   deletion alone does NOT break the proof — the two lines are defense in
   depth, which is the design.)
3. AK-047/AK-048 adapter-level teeth: the shared-suite broken stubs from B1
   (`SandboxBrokenFallback`, `SandboxBrokenProcessStart`) fire on the
   intended cases by name; the bwrap adapter's own adversarial-quoting and
   live-confinement tests carry the execution teeth.

## Gate results (final battery)

`cargo nextest run --test p5_hard_gate` **4/4** · `cargo nextest run -p
gauntlet-app govern::` **11/11** · `just test` workspace **1056/1056** ·
`just lint` clean · `just fmt-check` clean · `just boundaries` **215/215** ·
`just doctor` exit 0 · `just deny` ok · `just ruler` PASS · `just
prereg-check 14` PASS.

## Notable implementation facts (for the critic)

- The literals scanner's statement joiner (24 physical lines) taught the
  hard lesson recorded inline in the proof: a string CONTINUATION inside a
  const/format arm creates a joined unit that can swallow a spawn site
  below it, and the `command::new(` branch then flags the unit's FIRST
  literal. Fixes: single-line consts (no continuations), `render_policy`
  built through `JsonValue` with short `;`-terminated statements, and the
  shell spawn isolated from literals (`shell()`/`shell_from_env()` split;
  the shell's NAME is configuration from the environment, never spelled).
- The composed-loop control leg uses the §8.6 conservative posture (the
  PRODUCTION contained-dispatch shape): a wall-clock-reading backend adapter
  inside the loop's pinned deterministic clock would be an artificial
  composition (the adapter would see a 2023 deadline against a 2026 wall
  clock); the backend-admission half is proven at gate level in the wraps
  leg. This seam (deterministic-clock loops vs wall-clock adapters) is
  recorded for Tasks 15-16, whose process runners will face the same issue.
- The g7 file-side leg deliberately proves the frozen-ruler defense through
  `resume_run` (re-freeze-verify FIRST): the in-flight cancellation window
  (p4's hook idiom) was not duplicated; the g7-specific claim is the
  never-dispatch-at-lowered-tier fact, which the abort proves.
