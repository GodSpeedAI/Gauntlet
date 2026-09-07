# Conformance

This document states the implementation's trust posture, sandbox posture,
profile adjudication, advisory-only enforcements, and the terminal
disposition of UNK-001, as spec §22.1 and the task-27 charter require.

**Where per-unit truth lives.** The authoritative per-unit disposition is
`.agents/reports/conformance.json` (the `gauntlet verify --all` sweep
output) and `.agents/reports/handoff.md` (the six-category handoff).
Nothing in this file overrides either artifact, and no line here claims a
status ahead of what those artifacts record. Unit-level mentions below use
the six status categories of spec §26 and defer to the sweep.

## Trust posture

- **The agent kernel is a control environment, not a security sandbox**
  (spec Annex C.2). The Prime-Agent kernel — and any runner that executes
  model-generated code — does so with the worker's OS permissions unless
  externally contained. An adapter reports `isolation: none` unless it is
  externally wrapped; containment is the runtime's responsibility, never
  the harness's promise.
- **Authority gates whether something runs; containment gates how contained
  it is if it does.** Both precede builder process start (spec §17.1,
  §17.5.4). Neither substitutes for the other.
- **Honest capability reporting.** Runners and backends report measured
  capability, never aspirational capability: the bwrap adapter probes the
  real binary before judging any policy; `gauntlet doctor` measures the
  configured runners and the sandbox rung instead of declaring them; the
  Prime-Agent adapter reports `isolation: none` because nothing wraps it.
  Where enforcement is prompt-only it is recorded as
  `enforcement: advisory` (see the advisory section below).
- **Environment stance.** This composition targets a trusted,
  single-operator development environment. The conservative default
  posture (below) is the honest posture for that environment. It is not a
  hardened posture for hostile tenants; the recorded boundaries in the
  advisory section state exactly where hardening would have to land.

## Sandbox backends and tier mapping

Blast radius maps to a containment tier (spec §17.3):

| Tier | Posture | Automatic? |
|---|---|---|
| `contained` | Ephemeral workspace, deny-by-default network, read-write limited to the workspace | yes |
| `local` | As `contained`, plus explicit deny of credential paths and the repository git directory | yes |
| `cross_cutting` | Tightest available real backend; host-mutating fallbacks explicitly refused; runner must report isolation ≥ process | only with `authority` satisfied — never `critic` for this tier |
| `irreversible` | **No policy exists.** The action never reaches the sandbox port unattended; a policy file for this tier is a `doctor` failure | no |

- **Backends shipped:** `sandbox-none` (the conservative token — it refuses
  every tier above `contained`, it does not degrade) and `sandbox-bwrap`
  (bubblewrap). On this host **bwrap 0.11.1 is the only real backend**
  (no docker/podman/qemu/firecracker; `.agents/reports/environment.md`).
- **The backend is an artifact-declared, substitutable choice.**
  `.gauntlet/policies/<tier>.json` names `"backend": "bwrap"` as data; the
  policy adapter decodes the token strictly and the bwrap adapter refuses a
  policy naming a backend other than its own identity. Changing backends is
  a versioned artifact edit plus a composition-root swap, not a code
  change.
- **The shipped default posture refuses rather than degrades.** The
  declared default is `sandbox.backend = "none"` (spec §8.6). Under it the
  authority gate refuses every tier above `contained`
  (`NoBackendForTier`); a sandbox validation failure escalates and never
  falls back to unsandboxed execution. A default-configured runtime does
  not run `cross_cutting` under bwrap — it does not run `cross_cutting` at
  all.
- **`irreversible` has no policy file by design**; its presence is a
  configuration error the doctor fails on (spec §17.3, AK-049's class).

## Profile adjudication

Spec §24.1 defines three profiles. Item verdicts use met / partial /
absent; each names where its evidence lives. These are the working
ledger's verdicts — the per-unit backing is the sweep's, not this table's.

### Core (REQUIRED) — met

| Obligation | Verdict | Evidence |
|---|---|---|
| §3 invariants I1–I12 | met | frozen ruler + `just ruler` green every task; typed domain (`crates/gauntlet-domain`) |
| Ports §6.1–6.3 incl. required adapters | met | `crates/gauntlet-ports/src/contract/` shared suites; task 4/8 records |
| Config §8 (precedence, doctor, defaults) | met | `crates/gauntlet-app/src/config/`, `src/doctor/`; task 5 |
| Reference freeze/integrity §9.4 | met | `crates/gauntlet-cli/tests/p1_reference_integrity.rs`; task 7 |
| Result contract §10.3 | met | `role_result@v1` codec + runner contract suites; tasks 4, 15, 16 |
| Orchestration §11 | met | `crates/gauntlet-app/src/control/`; task 12 |
| Observation §12 | met | five facet adapters + `src/observation/`; task 9 |
| Comparison §13 | met | blind envelope builder + `compare-diff`; task 10 |
| Verification §14 | met | `src/verify/` + four verify adapters; task 11 |
| Settlement §16 | met | `src/settle/` (five-precondition service, keyed atomic commit); task 13 |
| Governance §17 | met | `src/govern/` (gate, posture) + sandbox adapters; task 14 |
| Persistence/recovery §18 | met | `src/state/` + sqlite/fs adapters; tasks 6, 20 |
| CLI §20.1 | met | `crates/gauntlet-cli/tests/cli_surface.rs`; task 20 |
| Failure model §21 | met | typed failure classes through the app/adapter layers |
| Proofs P1–P12 | met | `crates/gauntlet-cli/tests/p1_*` … `p12_*`; per-proof rows below |
| Anti-gaming G1–G16 | met (G9 advisory — see below) | `crates/gauntlet-cli/tests/antigaming.rs` (19 tests) + `.agents/reports/mutation.json` (73 examined / 63 caught / 10 unviable / 0 missed, task 24) |

Proof-by-proof: P1 reference integrity (task 7), P2 context isolation with
teeth (task 10), P3 real observation with the claims facet excluded
structurally (task 9), P4 corrective loop (task 12), P5 hard gates +
irreversible-never-dispatched (task 14; re-evidenced on the real binary,
task 25), P6 budget control with the reserve intact (task 12; real-binary
E2E task 25), P7 recovery exactly-once (task 13; real-binary recovery legs
task 25), P8 adapter substitution mock+CLI with zero test-body changes
(task 16), P9 self-improvement containment (task 23), P10 no false
learning (task 22), P11 critic-is-not-verifier (task 13), P12 missing ≠
passing at every layer including the operator resolve action (task 11).

### Extended — partial

| Item | Verdict | Evidence |
|---|---|---|
| Formation via `wayfinder-gauntlet` (§9.2) | met | `.agents/skills/wayfinder-gauntlet/` (derived from the real upstream, task 18); fog/out-of-scope invariants deny-by-default (`gauntlet_app::formation`) |
| Critic fan-out `adaptive`/`high_assurance` (§13.3) | partial | review-mode machinery and the adaptive default are exercised at instrument and composed level (tasks 10, 26); no dedicated `high_assurance` composed run is recorded |
| Memory + promotion (§19.1–19.2) | met | `CapabilityLedger` port + sqlite migration, `gauntlet_app::memory` (task 22) |
| JSON API (§20.2) | met (feature-gated) | `crates/adapters/gauntlet-adapter-surface-json` behind the `surfaces` feature; the default build is byte-identical CLI-only (task 21) |
| TUI / dashboard (§20.2) | absent | not built by this plan; the operator's separate workstream (`.agents/specs/gauntlet-tui-prime-workbench.*`) is out of scope and makes no conformance claim here |
| Git-worktree workspaces | absent | the config vocabulary exists but the shipped `workspace-fs` provider is directory-based; no worktree-backed adapter exists |
| Observability adapter | absent | logging is structured `key=value` with configured sinks (§20.3); no external telemetry adapter exists in the tree |

### Self-improving — partial

| Item | Verdict | Evidence |
|---|---|---|
| Optimizer (§19.3) | met | `gauntlet improve propose|evaluate|promote|rollback`; proposals-only enforced (task 23) |
| Self-improvement gate (§19.4) | met | all eight conditions adjudicated over durable four-arm evaluation records; insufficient evidence rests at provisional (task 23) |
| Baselines (§23.3) | met | `.agents/reports/baselines.json` + `.agents/reports/baselines.md`, A/B/C/D on the bounded gadget-task fixture, the D-does-not-win cases recorded (task 26) |
| Dogfooding (§19.5) | partial | the loop drives itself in composed fixtures — mock E2E (task 20), real-runner E2E with live bwrap (task 25), the baselines D legs through the real composed loop (task 26) — but this repository's own tasks were orchestrated by the Prime-Agent harness, not by `gauntlet run`; the repo root deliberately names no default runner (UNK-003) |

## Advisory-only enforcement

Every enforcement that is prompt/recording-only rather than mechanical, with
its recorded trigger:

1. **G9 — critic write-scope is advisory.** No mechanical critic write-scope
   enforcement exists; the boundary is role-context discipline plus the
   envelope allowlist. A `CandidateRevision` ("runtime write-scope
   enforcement for role contexts") is recorded for the ruler owner in the
   task-24 closure and the g9 test header/manifest. Trigger: ruler-owner
   decision.
2. **F6 — cli-agent run_id echo is informational.** The generic CLI adapter
   admits a result whose `run_id` does not echo the dispatch as `Completed`;
   `invocation_id` IS enforced. Recorded task-26 residual; trigger: next
   adapter touch.
3. **Executed none-posture judge-forgery boundary (task 26).** Under the
   `none` posture a hostile same-user child can forge judge material — the
   wrapper, the snapshots, the manifest; the shipped defenses (sha256 pin,
   nonce-named snapshots, facet-count cross-check) are tamper *evidence*,
   not prevention; tamper detection is a race, not an invariant. The
   baselines record is scoped to the declared FIXED-CHILDREN threat model;
   a privileged judge channel (uid or sandbox separation) is the structural
   prerequisite for any hostile-child claim. Recorded in
   `.agents/reports/reference-candidates.md` entries 6–7 (2026-09-07);
   trigger: any future hostile-child threat model, or a runtime/sandbox
   revision.
4. **Instrument-run denominators are not comparable across baseline
   profiles** (task 26, reference-candidates entry 7): D counts durable
   verification records, B/C count declared-check re-runs; a comparable
   verification-burden metric is a ruler-owner candidate before any payment
   comparison is promoted to a tracked dimension.
5. **Roles harness write-scope for generic-cli harnesses is advisory**
   (task 17): the semantic harness registry records the enforcement grade
   per harness; file-based harness state the runtime cannot lock is
   reported, not prevented.
6. **Task-14 escalation R-4 (recorded, operator's):** inside the sandbox the
   whole host filesystem is readable via the baseline read-only bind —
   literally spec-conformant (credential *denial* is `local`-tier and
   writability is what §17.5.5 bounds) but a builder that can read the
   frozen ruler can target its assertions. Escalated to the operator as a
   specification CandidateRevision-class question; not closed by this plan.

## UNK-001 — terminal disposition

**cross_cutting containment — a declared substitution.** Spec Annex F's
typical `cross_cutting` backend is a microVM or the strongest available
container tier. This implementation ships neither. The strongest real
backend adjudicated on the build host is bubblewrap, a user-namespace
mount sandbox whose honest rung is `Isolation::Process`.
`.gauntlet/policies/cross_cutting.json` therefore names `bwrap`, and the
achieved isolation for `cross_cutting` on this host is **process grade,
not kernel or VM grade** — a weakening relative to Annex F, stated here
rather than concealed behind an unqualified "it is sandboxed" claim.

Which backend `cross_cutting` runs under in any deployment other than this
build host is **unknown by design**: no production deployment target is
named anywhere in the tree, so the question has no observable referent to
answer it. It is a deployment-configuration fact, resolved by editing the
versioned tier artifact and composing the corresponding adapter. The
shipped default (`sandbox.backend = "none"`) does not run `cross_cutting`
at all — the authority gate refuses every tier above `contained` when no
backend is composed, and a validation failure escalates rather than
degrading.

Evidence anchor: `.agents/reports/task14/structural-review-unk001.md`
(the PARTIALLY-RESOLVED adjudication and the four-proposition table this
disposition carries forward). The frozen manifest
(`.wayfinder/reference.yaml`) is never edited to record any of this; its
UNK-001 note already routed the disposition here.

## Proven vs provisional

Status categories are spec §26's six. Where a line names a ruler unit, the
status word is the working ledger's claim ("55 units provisional through
Task 26", CURRENT_STATUS observations); the sweep's conformance.json is
authoritative and this section defers to it.

- implemented_but_provisional — AK-005: `Outcome` stays three-state end to
  end and missing evidence never satisfies a settlement requirement
  (`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`, task 11).
- implemented_but_provisional — AK-058: each of the sixteen §23.2 gaming
  strategies has a test that goes RED when its defense is removed
  (`crates/gauntlet-cli/tests/antigaming.rs` + the task-24 mutation run:
  73 examined / 63 caught / 10 unviable / 0 missed; G9's defense is the
  recorded advisory above, judged honest by the task-24 falsifier).
- implemented_but_provisional — AK-066: the composed E2E drives the real
  binary, the real generic CLI adapter, and live bwrap on the seeded
  per-tier fixture; the `cross_cutting` unit routes its closure to the
  operator per the frozen authority semantics (task 25; the AK-066 locator
  divergence and the settleability tension are recorded in
  reference-candidates 2026-09-06).
- implemented_but_provisional — AK-069: baselines A/B/C/D executed on the
  bounded gadget-task fixture and recorded at
  `.agents/reports/baselines.json`, including the cases where D does not
  win (task 26).
- implemented_but_provisional — AK-070: the recorded task-24 mutation
  artifact `.agents/reports/mutation.json` is the evidence basis — a dated
  recording pinned at settlement, not a fresh execution (task-27 charter
  decision 11).
