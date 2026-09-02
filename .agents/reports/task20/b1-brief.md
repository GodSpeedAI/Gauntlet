# Task 20 / Builder B1 brief — containment & durability seams

You are Builder B1 for plan Task 20 in /home/sprime01/projects/gauntlet.
Read first, in order: `AGENTS.md` (repo root), `.agents/reports/task20/charter.md`
(decisions D1–D10 and guardrails bind you), then this brief. Your slice is
B1 only — do not touch CLI composition, the report service, or E2E (other
slices own those; editing their files will conflict).

## Goal

Close the containment and durability seams that the FIRST COMPOSED BUILDER
EXECUTION (task20's E2E) hard-triggers. Three bounded items, in this order:

### Item 1 — production WorkspaceProvider adapter (R-1b, part a)

New crate `crates/adapters/gauntlet-adapter-workspace-fs` (add to workspace
members in the root `Cargo.toml`, deps: domain + ports only, mirroring an
existing adapter crate's manifest style, e.g. gauntlet-adapter-evidence-fs).

Implement `WorkspaceProvider` (port: `crates/gauntlet-ports/src/
workspace_provider.rs:96` — `create(run_id, subject: WorkspaceSubject,
deadline) -> Workspace`, `cleanup(&WorkspaceRef, deadline)`, `hooks()`).

Requirements:
- Filesystem-backed: workspaces materialize under a configured root
  (composition passes e.g. `<state_dir>/workspaces`; the app defaults module
  already exports `WORKSPACES_DIR_NAME`).
- Per the existing contract suite (`crates/gauntlet-ports/src/contract/
  workspace_provider.rs`, entry `workspace_provider_suite(factory)`, cases
  `create_isolates_parallel_subjects` / `cleanup_releases_the_workspace`):
  distinct subjects get distinct workspaces, keys sanitized to
  `[A-Za-z0-9._-]`, cleanup succeeds and the provider stays usable. Run the
  suite against your adapter in the crate's `tests/contract_suite.rs`
  (copy the pattern from a sibling adapter crate).
- Refuse to create outside the configured root (path traversal defense,
  mirroring the roles loader's `contained_join` discipline from task17).
- Honest, typed errors (`WorkspaceProviderError`), no unwrap/expect in
  production paths.

### Item 2 — thread a fresh per-unit workspace through gate admission (R-1b, part b)

Today `GovernDeps` (crates/gauntlet-app/src/govern/gate.rs:64) holds ONE
`WorkspaceRef` for its whole lifetime, so every unit admitted through one
GovernDeps resolves to the SAME workspace-scoped directory. The controller
separately derives `run_workspace_ref(run_id)` (controller.rs:3571). The
recorded residual (CURRENT_STATUS `R-1b`) is MINOR against task14's ruler but
MAJOR under spec §17.5.1 invariant 1. Fix it structurally:

- The controller asks a `WorkspaceProvider` for a FRESH workspace per
  dispatched unit/slice before admission, and the admission (`AuthorityGate::
  admit`, `GateFacts { unit, tier, authority, runner_isolation }` at
  gate.rs:206) validates against THAT workspace — not a lifetime-scoped one.
  Choose the minimal honest shape: e.g. `GovernDeps` carries
  `Option<Arc<dyn WorkspaceProvider>>` and/or `GateFacts` gains the
  per-dispatch `WorkspaceRef`; the gate must be able to see the workspace the
  unit will actually run in. Whatever shape you pick: the old
  single-lifetime field must not remain the operative source of truth.
- `ControllerDeps` (controller.rs:215-253) gains the provider (optional, so
  existing tests construct; `None` ⇒ the controller falls back to the
  current derived workspace ref and records nothing new — do NOT silently
  degrade when the composition wired a provider).
- Update every construction site that breaks (gate.rs in-crate tests,
  crates/gauntlet-cli/tests/p5_hard_gate.rs, g7/g10/p4/p6/p7 — mechanical
  constructor updates only; do not weaken any assertion).
- TEETH (required): a test proving two units of the same run receive
  DISTINCT workspaces at admission when a provider is wired, and that the
  test FAILS (red) when the provider is bypassed/the old single-ref path is
  restored. Record the red→green evidence in your progress file.
- Also wire cleanup: after a unit's invocation terminalizes, the provider's
  `cleanup` is invoked (best-effort, typed failure recorded, never blocking
  settlement ordering — check §16 ordering before choosing the call site).

### Item 3 — write_atomic directory-entry fsync (R17-note-1)

Locate the `write_atomic` helper (grep; task17 introduced it around the
roles lock/installer). Add a directory-entry fsync after the rename (open
the parent directory with `OpenOptions` read + `fsync`), with a test that
fails if the fsync is removed (a mutation-style tooth; document in the test
doc-comment why). Keep the helper's signature and call sites unchanged.

## Boundaries

- Do NOT touch: crates/gauntlet-cli/src/**, gauntlet_app::report (doesn't
  exist yet), tests/fixtures/**, any `.wayfinder/**`, `.gauntlet/**`,
  `.agents/specs/**`. Mechanical compile fixes in existing
  crates/gauntlet-cli/tests/*.rs files are allowed and expected.
- Do NOT add dependencies without checking `deny.toml` policy; std-only
  should suffice.
- Compile discipline: `flock .agents/reports/task20/compile.lock -c '<cmd>'`
  around every cargo invocation. B2a runs in parallel with you — expect
  brief waits. Prefer `just check-package <crate>` / `just test-package
  <crate>`.

## Checkpoint protocol (mandatory)

Work from `.agents/reports/task20/b1-progress.md` (create it): declare-first
`STARTED <item>` before editing, `DONE <item>` after, with evidence lines
(commands run, test counts, red→green proof for the teeth). Your final
message may be lost — the progress file is your contract. Finish by writing
`.agents/reports/task20/b1-report.md`: what landed, file inventory, test
counts, the teeth's red→green evidence, any deviation from this brief with
its reason, and anything you deliberately did NOT do.

## Done when

`just check-package gauntlet-adapter-workspace-fs`,
`just test-package gauntlet-adapter-workspace-fs` (contract suite green),
`just check` and `just test` pass workspace-wide (expect only your changes
breaking things — anything else red, report it, do not fix outside scope),
`just boundaries` and `just lint` pass, `just ruler` passes.
