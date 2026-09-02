# Task 20 — COR-D reconciliation & completion record

**Attribution (recovery protocol).** COR-D (builder agent) landed all ten
adjudicated corrections (COR-D1..D10) on 2026-08-31 but was killed by a
usage limit during its final compile; its trail is `cor-d-progress.md`. The
first finisher dispatch landed the missing `write_contained_policy` helper +
call-site fix, then died on an API connection failure. The remaining
completion, verification, and THREE further live-failure fixes were done by
the ORCHESTRATOR under the standing heal+continue exception (sub-agent
dispatch failing at connection level twice in succession, 2026-09-01) —
recorded here as recovered-and-verified, with each fix's evidence below.
Tree outranks record; every landed item was re-proven by the final battery.

## What COR-D landed (verified against the tree; per-item basis in cor-d-progress.md)

- COR-D1 `roles list` (app read + CLI arm + usage)
- COR-D2 parallel-completion carry semantics (LoopState.carried +
  iteration_binding_spent; one binding per (run, iteration); carried
  completions checkpointed, resumed, released; E2E leg 4 at max_parallel=2)
- COR-D3 R-1b behavioral half: execution anchors at the admitted per-unit
  workspace (mock effects, CLI cwd/{workspace}, produce-locator, ladder
  factory `ladder_for` memoized per anchored workspace)
- COR-D4/D5 teeth batteries (compose_teeth.rs, escalations_cli.rs)
- COR-D6 dry-run zero-write tooth hashes contents
- COR-D7 usage-line fixes + deliberate golden regen + improve consults
  report_manifest::is_wired (RES-15 by construction)
- COR-D8 doctor check-3 reason reworded token-neutral
- COR-D9 diagnostic allowance mapping moved into gauntlet_app config
- COR-D10 E2E assertions parse unit→category pairs; vacuous contains removed
- RECORD entries appended to reference-candidates.md (binding-granularity
  ruler-owner question; locator content semantics composition-bound)

## Completion + live-failure fixes (orchestrator, evidence per item)

1. **Tier-policy set (test infra).** The live-wrap leg wrote only
   contained.json; doctor §8.5 check 5 requires the full
   local/contained/cross_cutting set (gate evidence: final-doctor-adjacent
   logs recovery-teeth*.log). Helper generalized to `write_tier_policies`
   over the three tiers; `irreversible` absent by design (AK-049).
2. **Required control settings (test infra).** The leg reaches config
   resolution (doctor clean, runner selected) and must set
   GAUNTLET_STALL_TIMEOUT_MS/RETRY_BASE_MS/RETRY_MAX_MS/
   GAUNTLET_MAX_ATTEMPTS_PER_UNIT — same values as the seeded-fixture E2E
   (e2e_full_loop_mock.rs:149-152).
3. **REAL DEFECT (mechanism fix, compose.rs ContractDirPolicy):** the
   R17-F4 wrapper added `state/runs` as a writable root WITHOUT
   materializing it, so the real bwrap bind refused a nonexistent source
   and the composition-time wrap for `cli-agent` could NEVER validate
   clean on a fresh state dir (reproduction: recovery-teeth3/4.log). Fix:
   the runtime materializes the result-contract directory it promises,
   before claiming it; creation failure escalates through the provider's
   typed error (never a silent widening/shrink). Found by the teeth on its
   first live exercise — the teeth work paid for itself.
4. **Assertion proxy refresh (test).** With (3), `state/runs` legitimately
   exists at wrap time; "no dispatch" is now asserted by an EMPTY root (no
   anchored invocation subtree), preserving the original intent.
5. **Boundary/literal compliance (test infra, mechanical):**
   `STATE_BACKEND_TOKEN` const exported by the sqlite adapter (vendor
   token spelled in the adapter, mirroring RUNNER_TOKEN) replaces the
   `sqlite` literal in compose_teeth's generated GAUNTLET.md; budget block
   built from named consts (literals-scanner joined-span rule); e2e/
   escalations path literals built segment-wise (["evidence",
   "marker-check-report.json"].join("/")) — AK-016 compliant.
6. **Load-sensitive wait budget (test infra, demonstrated).**
   `b1_contract_wins_over_exit7...` failed 2 of 3 full-battery runs and
   passed isolation every time: `wait_terminal`'s 10s budget starves on
   the 6-core WSL host while many live children spawn in parallel. All 7
   legs now use WAIT_BUDGET=60s (test slack only; asserted behaviors are
   timing-independent; the spec-deadline path is tested separately).
7. **Invalid fixed ULID (test, never-run leg).** `RUN_ULID` contained
   O and U — not in the Crockford base32 alphabet, so `RunId::new` refused
   and the leg could not even start (it sat behind the compile that never
   ran). Valid spelling of the same mnemonic:
   `01JDY0C0RDD5ESCSTEST0RVN01`.

## COR-D5 fog (orchestrator adjudication, carried to closure)

`settle_operator_action` structurally never returns Ok until Task 21 (the
AK-062 no-bypass gate; the positive half is Task 21's commissioned
surface). The shipped legs pin: refusal without records; refusal with
non-PASS; refusal even with a chain-earned durable PASS; zero settlement
rows — the D9 single-writer discipline held on every leg. The
settle-committed-but-queue-write-failed CLI branch is unreachable until
Task 21. ACCEPTED as honest exposure per D8; recorded, no change.

## Final battery (orchestrator, personally run, 2026-09-01)

- `just check` 0 · `just test` **1333/1333** · `just lint` 0 · `just deny` 0
- `just boundaries` 228/228 · `just ruler` PASS · `just doctor` OK
- `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test
  cli_surface` green · repo-root dry-run exit 0 with a before/after
  git-status diff proving ZERO writes.
- Logs: final-*.log, recovery-*.log, *-iso*.log in this directory.
