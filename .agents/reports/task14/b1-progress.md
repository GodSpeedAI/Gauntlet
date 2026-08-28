# Task 14 slice B1 — builder progress checkpoint

Working log; appended after each numbered item, before running the next gate.

## Start

- 2026-08-28: charter read (`.agents/agents/builder.agent.md`), recon read
  (`.agents/reports/task14/recon.md`), port files read (sandbox.rs,
  policy_provider.rs, contract/sandbox.rs, contract/stubs.rs, suite_runs.rs,
  json.rs, deadline.rs, agent_runner.rs Isolation, domain values.rs wire_enum +
  blast_rank). Adapter precedents read: observe-command (spawn/deadline
  discipline), meter-memory (Cargo.toml shape).
- `just ruler` before start: **PASS**.
- bwrap present at /usr/bin/bwrap, 0.11.1 (matches environment report).

## Item 4 — sandbox contract suite (done)

- `crates/gauntlet-ports/src/contract/sandbox.rs`: +3 cases
  (`no_unsandboxed_fallback`, `wraps_from_process_start`,
  `capability_mismatch`), existing 2 kept by name/rule; suite wires 5.
- DEVIATION (recorded, minimal): the shared `policy()` fixture helper is
  now factory-aware (names the factory's OWN `backend_identity()`), and the
  fixture roots are absolute (`/suite-writable-root`); the existing cases'
  names, rules, and docstring anchors are untouched. Why: both new
  adapters are mandated to refuse policies naming another backend
  (cross-wiring defense) and relative roots — with the old fixed
  `"local-rung"`/relative-root fixtures NO real adapter could pass the
  suite, contradicting the brief's own requirement that the none/bwrap
  factories run it clean. Also the `validation_precedes_every_use`
  exact-equality assertion is now rung-aware: none-rung identity wraps must
  still return the exact text; transforming wraps (rung > none, mandated by
  AK-048) must CARRY the invocation inside the rendered command (probe is
  quote-free so escaping cannot alter it). Exact equality and AK-048's
  transform are mutually exclusive for rung > none backends; the ruler
  (AK-048) wins, the older fixture accommodates.
- `contract/stubs.rs`: SandboxGreen is now an honest NONE-rung backend
  (refuses above-contained tiers at validate; wrap refuses foreign /
  findings-carrying / rung-mismatched validations); LaxSandbox defect
  unchanged. +2 broken variants, one defect each: `SandboxBrokenFallback`
  (wraps on a findings-carrying validation) and
  `SandboxBrokenProcessStart` (claims Process rung, identity wrap).
- `contract/suite_runs.rs`: +2 teeth (`sandbox_fallback_broken_is_caught`
  intended case `no_unsandboxed_fallback`;
  `sandbox_process_start_broken_is_caught` intended case
  `wraps_from_process_start`).
- Gates: `just check-package gauntlet-ports` PASS (2m18s);
  `just test-package gauntlet-ports` PASS 107/107 (was 105; +2 teeth),
  sandbox teeth all green: `sandbox_green_is_clean`,
  `sandbox_broken_is_caught`, `sandbox_fallback_broken_is_caught`,
  `sandbox_process_start_broken_is_caught`.

## Item 1 — none adapter (code written, gates after registration)

- Empirical bwrap facts (verified on host before designing the renderer):
  `--tmpfs <existing-dir>` masks (empty dir); `--tmpfs <existing-FILE>`
  ABORTS bwrap ("Can't mkdir ... Not a directory"); `--ro-bind /dev/null
  <file>` is a real read-denial (Permission denied on read);
  `--tmpfs <nonexistent>` fails closed at startup when the parent is ro.


## Recovery record (orchestrator, 2026-08-28 ~22:30 — heal+continue)

The builder session died to the 5-hour sub-agent usage window (21st recorded
instance; window resets 2026-08-29 01:44) AFTER landing Items 1–4 code but
BEFORE its final report. Tree-vs-ledger reconciliation found all three crates,
the Cargo.toml registration, the policy artifacts, and the ports suite landed;
`just ruler` PASS before and after. Per the standing heal+continue instruction
the orchestrator completed the slice personally:

- Fixed policy-fs test compile errors (12): `.replace()` String→&str borrows
  at 7 sites; `Scratch` (Drop) partial moves at 4 sites (`.path.clone()`).
- Fixed policy-fs `bound()`: fixed-epoch deadline was always-expired against
  the adapter's wall-clock read → now `now + BOUND_SPAN` (bwrap's `live_bound`
  was already correct; the policy-fs helper was the bug — 6 tests failed).
- Fixed ports `contract::sandbox::bound()` the same way (fixed epoch → live
  wall-clock bound; suite is test-support surface, same fact the adapter
  reads). Removed the now-unused BASE_INSTANT.
- Fixed bwrap `an_expired_deadline_refuses_without_spawning`: `at_epoch(now)`
  is not expired (expiry is strict `now > at`) → `now - 1` with the comment.
- Clippy: collapsed two if-let ladders in the new suite cases.
- Boundary failures (4 tests) fixed the sanctioned way:
  - `'/suite-writable-root'`/`'/suite-denied-path'` absolute-path literals in
    the suite fixtures → `Path::new("/").join(SEGMENT)` construction (the
    `contract::stubs` workspace-root precedent; lone `/` is not flagged).
  - adapter last-segment `none` tripped AK-014 on PRE-EXISTING spec-vocabulary
    literals (defaults.rs SANDBOX_BACKEND, Isolation token arms) → grew
    SPEC_VOCABULARY_SEGMENTS to `["model","digest","check","none"]` with the
    recorded justification + pin-test update (the sanctioned growth path).

## Final gates (orchestrator-run)

- `just ruler` PASS; `just prereg-check 14` PASS (hash 0ba5ce79…).
- ports 107/107 · sandbox-none 12/12 · sandbox-bwrap 12/12 (incl. live
  confinement EXECUTED under real bwrap: in-root write OK, out-of-root write
  denied, denied-file read denied via /dev/null mask, denied-dir emptied via
  tmpfs) · policy-fs 11/11 (incl. shipped-repo-artifacts decode).
- `just check` clean; `just test` workspace **1034/1034**; `just lint` clean;
  `just boundaries` **215/215**; `just fmt-check` clean; `just deny` ok.
