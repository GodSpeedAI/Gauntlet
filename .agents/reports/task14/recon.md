# Task 14 recon — authority gate, sandbox port, tier policies (P5, AK-044..AK-050)

Written before any task-14 implementation. Sources: plan Task 14 section, Core-spec
§6.1/§6.2/§8.5/§8.6/§17/§21.3/§22.2/Annex F, answer-key AK-044..AK-050 verbatim,
`.agents/reports/environment.md` (Task 0 backend inventory), CURRENT_STATUS.yml
handoff (task14 deferrals), and the current tree.

## What the ruler demands (AK-044..AK-050, verbatim claims)

- AK-044: irreversible is never an automatic dispatch candidate at the scheduler
  level. Locator `tests/proofs/p5_hard_gate::irreversible_never_dispatched`.
- AK-045: a high-quality result that violates one hard requirement is rejected
  rather than averaged into a pass. Locator `tests/proofs/p5_hard_gate`.
- AK-046: an agent role cannot lower a Unit's blast radius or authority; only an
  operator can raise it. Locator `tests/antigaming/g7_tier_downgrade`.
- AK-047: sandbox policy validation precedes execution; a validation failure
  escalates without falling back to unsandboxed execution. Locator
  `tests/contract/sandbox::no_unsandboxed_fallback`.
- AK-048: containment wraps the builder session from process start; no builder
  command executes before the authority and containment checks. Locator
  `tests/contract/sandbox::wraps_from_process_start`.
- AK-049: a sandbox policy file for the irreversible tier causes doctor to fail.
  Locator `tests/e2e/doctor::irreversible_policy_is_error`.
- AK-050: a runner reporting isolation none is refused for any tier above
  contained. Locator `tests/contract/sandbox::capability_mismatch`.

All seven locators name repo-root test targets that core crates cannot declare
(the Task-2 arch rule; exemption list is exactly `["crates/gauntlet-arch-tests"]`).
This is the EXISTING locator-materialization candidate class (first recorded
2026-08-17 for AK-049 itself). AK-049's claim is already satisfied since Task 5
(doctor check 5, forbidden-tier scan; reference-candidates entry with in-crate
locator re-declaration).

## Tree state (what exists, what does not)

Ports (Task 4, with shared contract suites):
- `gauntlet-ports/src/sandbox.rs`: `Sandbox` trait (`validate`/`wrap` carrying the
  validation through, `backend_identity`), `SandboxPolicy`, `PolicyValidation`
  (carries achieved `Isolation`), `SandboxError` (ValidationFailed /
  BackendUnavailable / WrapFailed / IdentityInvalid). Wrap is structurally
  validation-gated (foreign validation ⇒ typed refusal).
- `gauntlet-ports/src/policy_provider.rs`: `PolicyProvider` (`policy_for(tier,
  workspace)`, `permissions_for(role, unit)`), `NoPolicyForTier` documents that
  for `irreversible` absence is the REQUIREMENT.
- `gauntlet-ports/src/agent_runner.rs`: `RunnerCapabilities` with the
  `Isolation` ladder (None < Process < Container < Vm), `RoleInvocationSpec`
  with `sandbox_policy: Option<SandboxPolicyRef>`.
- Contract suite `contract/sandbox.rs` currently has 2 cases
  (validation_precedes_every_use, wrap_demands_its_own_validation) with
  green/broken stub teeth in `stubs.rs`/`suite_runs.rs`.

Domain: `RoleInvocation::admit` enforces §5.3.5's "sandbox_policy_ref null only
when tier=contained and backend=none" TIER half (backend half deferred to Task 14
by comment). `BlastRadius`/`Authority` lattices pinned with `blast_rank`/
`authority_rank`.

App: `control/scheduler.rs` already refuses irreversible at eligibility
(conjunct 3, §17.1 quote) and treats a tier with no concurrency entry as
unadmitted. `control/controller.rs::dispatch` (the only runner.invoke call site)
currently passes `sandbox_policy: None` always, never consults
`RunnerCapabilities`, and pins exercisable authority to `Critic` with the L1
note (beyond-critic completions surface pinned evidence
`completion_record_unrepresentable`). Doctor check 4 (`check_capabilities`) is
`Unavailable` with reason "capability-vs-tier dispatch check lands with plan
Task 14". Doctor check 5 (AK-049) is complete: forbidden-tier scan of BOTH
effective and default policy dirs (spelling/symlink-robust, unreadable ⇒ fail),
tier-file existence required only when `backend != none`.

Config: `sandbox.backend` (§8.6 default `none`, "with tier contained only; any
higher tier without a backend refuses to dispatch") and `sandbox.policy_dir`
(default `<root>/.gauntlet/policies`, anchoring precedent in config resolve).
§8.2 backend token vocabulary: `none | process | bwrap | seatbelt | mxc |
microvm`.

`.gauntlet/policies/` is EMPTY today. No Sandbox or PolicyProvider adapter
exists. No `govern` module exists (`cargo nextest run -p gauntlet-app govern::`
selects nothing yet).

## Environment (Task 0 inventory, unchanged)

bwrap 0.11.1 present and smoke-tested (`--ro-bind / / --dev /dev --proc /proc
--unshare-net --die-with-parent`). No docker/podman/qemu/firecracker. Task 0
adjudication: bwrap IS a real backend ⇒ the plan's redesign trigger ("no real
sandbox backend exists") does NOT fire; cross_cutting gets bwrap (the strongest
available real backend; the Annex-F typical-backend weakening to be stated in
docs/CONFORMANCE.md at Task 27). UNK-001 in reference.yaml tracks the general
question.

## Spec posture facts that fix the design

- §17.1 (the authority gate, before dispatch): irreversible never dispatches;
  `authority ≠ critic and closure needed` escalates BEFORE any transform;
  operator-identity lack escalates. "A tier may be raised by an operator; it is
  never lowered by an agent."
- §17.3 tier table: `backend: none` permitted ONLY at contained; local = same
  shape + explicit deny of credential paths and the repo git dir; cross_cutting
  = tightest available real backend, host-mutating fallbacks refused, "Runner
  must report `isolation ≥ process`"; irreversible = no policy exists.
- §17.5.4: every builder invocation above contained is wrapped by the tier's
  sandbox FROM PROCESS START.
- §22.2: a harness adapter must report `isolation: none` unless it is itself
  wrapped by a Sandbox adapter — the mock runner already pins this honestly
  (isolation none), which is what lets the Task-14 tier check refuse it.
- §8.5 check 4: "Runner capabilities satisfy configured tiers (§17.3);
  otherwise `sandbox_capability_mismatch`."
- §21.3 row "Sandbox validation failure → Escalate; never fall back to
  unsandboxed".

## Consequences for the standing battery (load-bearing)

Tier→required-isolation posture implied by §17.3/§8.6/AK-050: contained → None
(backend none permitted); local → ≥ Process; cross_cutting → ≥ Process; and the
runner's REPORTED isolation is the decisive fact (AK-050, plan step 4). The mock
runner reports `Isolation::None`. Therefore, once the capability-vs-tier check
is wired into the controller dispatch path, units at local/cross_cutting tiers
can no longer dispatch through the mock runner — correct per the ruler, but it
changes behavior the standing composed proofs currently rely on:

- `gauntlet-cli/tests/p4_corrective_loop.rs`: fixture unit `blast_radius: local`
  (authority critic). P4's subject (defect exposed → compared → routed
  correction) is tier-agnostic.
- `gauntlet-cli/tests/p6_budget_control.rs`: fixture `blast_radius: local`.
  P6's subject is budget control.
- `gauntlet-cli/tests/p12_missing_is_not_passing.rs`: fixture `blast_radius:
  local` (3 spellings + 1 dynamic). P12's subject is evidence completeness.
- g10/p11/p7 fixtures are `contained` — unaffected.

There is no honest way to elevate the in-process mock to process isolation
(§22.2 honesty: nothing is contained because nothing spawns). The deliberate,
evidenced resolution is to move those three fixtures' units to `contained`
(tier is not the property under proof in those files) and to prove the
above-contained posture in the NEW p5_hard_gate proof against the real bwrap
backend plus the gate's capability facts. This is a fixture-input change with a
recorded reason, not a golden-output re-pin; the falsifier should attack it.

## Locator plan (existing candidate class)

- p5_hard_gate: `crates/gauntlet-cli/tests/p5_hard_gate.rs` (test
  `irreversible_never_dispatched` included by name).
- g7_tier_downgrade: `crates/gauntlet-cli/tests/g7_tier_downgrade.rs`.
- sandbox contract cases `no_unsandboxed_fallback`, `wraps_from_process_start`,
  `capability_mismatch`: new cases in `gauntlet-ports/src/contract/sandbox.rs`
  (run by the none + bwrap adapter factories and the green/broken stub teeth).
- AK-049: already satisfied (Task 5); unchanged.
- New reference-candidates entries at closure: the AK-044..AK-048/AK-050
  locator spellings (locator-materialization class, instances not amendments).

## Module/crate plan

- New `gauntlet-app/src/govern/`: tier posture (required rung per tier),
  `AuthorityGate` admission (§17.1 lines + AK-050 capability conjunct + AK-046
  agent-originated downgrade rejection with governance event), wired into
  `controller.rs::dispatch` BEFORE `runner.invoke`; refusals route to the
  existing escalation recording; admission carries the resolved policy +
  validation (+ wrapped command for above-contained) so the durable record
  proves order (admission event precedes invocation).
- New adapters: `gauntlet-adapter-sandbox-none` (contained-only; identity wrap),
  `gauntlet-adapter-sandbox-bwrap` (real backend: binary probe at validate,
  ro-bind root, writable roots rw-bound, denied paths tmpfs-masked,
  --unshare-net for DenyByDefault; Allowlist posture is an honest validation
  FINDING on this backend — no host-allowlist capability; achieved rung
  Process), `gauntlet-adapter-policy-fs` (reads `<policy_dir>/<tier>.json`,
  strict decode, irreversible refused structurally, $VAR-style root anchoring
  consistent with the config layer's indirection discipline).
- In-repo versioned tier policies: `.gauntlet/policies/{contained,local,
  cross_cutting}.json` (no irreversible file — by design).
- Doctor check 4 becomes real: backend rung vs the tiers the configuration can
  dispatch (`sandbox_capability_mismatch` on failure); under the conservative
  `none` backend the dispatchable set is {contained} — doctor stays green on
  default posture while above-contained dispatch refuses at the gate (runtime
  governance, not doctor failure); declared-but-absent backend binaries fail.
- `permissions_for` artifact shape is NOT specified by the frozen ruler (only
  per-tier sandbox policies are, §25 layout); scope it honestly rather than
  inventing an artifact (fog/reference-candidate at closure if it stays open).
