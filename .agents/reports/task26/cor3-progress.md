# COR-3 progress — task 26 correction round 3

Dispatched 2026-09-06. Work items (charter adjudications, binding):
B1 token-free app disclosures; B2 literal compliance across the seven
task-26 files (mechanical, gate rules learned first); B3 single-writer
inventory entries (reason+guard, no rule changes); m1 null reliability on
zero-completion legs (RED-first); m2 `_meta` D-surface disclosure +
per-leg `held_out_basis`; m3 run-id scope disclosure in `_meta`; m4
monotonic in-harness wall measurement (ms). M4 (frozen locator +
narrative) is the orchestrator's settlement act — NOT mine.

## Log

- Read charter.md, critic-design.md, IN-FLIGHT.md.
- Read arch gate sources first (per adjudication): `walk.rs` (scope:
  four core crates incl. tests/**; no file exemptions),
  `literals.rs` (classes: relative-path literals are structural —
  multi-segment filename-shaped literals flag even on const lines;
  `/`-rooted literals and `/root` mentions flag line-wide; timeout
  class = numeral >=2 in value position on a line carrying a
  duration-hint identifier), `denylist.rs` (adapter identifiers denied
  in inner layers; cli = composition root, masked-not-skipped),
  `single_writer.rs` (Rule C: (file, token) pairs must be pinned in
  WRITE_API_INVENTORY with reason+guard; guard needle must stay in the
  file's production text; pin test `the_pinned_inventories_are_pinned`
  must be updated in lockstep — documented lockstep edit, not a rule
  change).
- Captured the full violation list under flock: 5 denylist + 290
  literal + 25 single-writer (kept at /tmp/cor3-arch-baseline.txt).
- Empirical scanner probe (throwaway /tmp crate against the
  arch-tests lib, under flock): verified the compliance shapes —
  runtime assembly from named pieces (stem+suffix consts, dirname
  chains, temp_dir, join chains) passes; `.sh`/`.absent`/`gadget.rs`
  as whole pieces pass; bare `sh`, `check.sh`, shebang `/bin`, and
  `2>/dev/null` flag; const NAMES must not carry duration-hint
  fragments; numerals >=2 never share a unit with a hint identifier;
  ANY first literal of a unit containing `Command::new(` flags, so
  spawn statements stay literal-free. Decision: no `\n`-escape
  reliance (the scanner under-decodes escapes — exploiting that would
  be laundering, not compliance).
- RED-FIRST confirmed under flock: `a_zero_completion_leg_carries_no_
  reliability_figure` and `a_sub_second_leg_records_a_nonzero_monotonic_
  wall` both FAIL against the pre-fix code (Some(0) vs None; wall 0 vs
  >0).
- App module edits landed: HeldOutBasis vocabulary (metrics.rs),
  LegResult.held_out_basis, harness m1 (no denominator no figure) +
  m4 (monotonic Instant wall; the Clock removed from BaselineHarness —
  it had no remaining reader), report m2/m3 `_meta` disclosures +
  held_out_basis field, B1 token-free wording (mod.rs + report.rs),
  literal compliance across baseline/{harness,metrics,report,task}.rs.
- `just check-package gauntlet-app` exit 0 (lib + all targets).
- CLI improve_cmd.rs in progress: interpreter-baked instruments (SHELL
  from the environment as the materialized scripts' shebangs — the
  interpreter path is host configuration), child wrapper + judge shim
  materialized once per process, sentinel recursion guard, template
  adoption, script-name pieces, spawn units kept literal-free.
- DISCIPLINE NOTE (self-caught): I briefly wrote a nested `flock -c
  true` inside a flock'd command string (the AGENTS.md failure-log
  shape). Killed the task and re-ran as its own top-level flock. No
  compile interleaved with the kill (the lock was held throughout).


## Completion

- All residuals landed (B1/B2/B3, m1-m4). M4 (frozen locator +
  narrative) left to the orchestrator.
- Final batteries: arch 237/237 (exit 0); app check exit 0 + 620/620;
  cli check exit 0 + 139/139; gate-shape run exit 0 with ALL-PASS
  python verification; verify-ruler.sh PASS; fmt + clippy clean
  (0 warnings) on the three touched packages.
- Completion contract: `cor3-report.md` (deviations + inventory entries
  + battery evidence). NO settlement authority.
