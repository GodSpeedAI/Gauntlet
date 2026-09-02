# Task 20 / Builder B2 brief — CLI composition root + §20.1 surface + run

You are Builder B2 for plan Task 20 in /home/sprime01/projects/gauntlet.
Read first, in order: `AGENTS.md` (repo root), `.agents/reports/task20/charter.md`
(decisions D1–D10 + operational addenda — binding), then this brief.
B1 and B2a are DONE; their landed work is your baseline (b1-report.md,
b2a-report.md — read both; compose against the documented
`gauntlet_app::report` API, do not modify it except the one lint item below).

## Item 0 — baseline hygiene (do first, small)

One clippy error remains: `redundant_closure` at
`crates/gauntlet-app/tests/report.rs:213` (B2a's file; explicitly handed to
you). Fix mechanically; `just lint` must be green before you start Item 1.

## Item 1 — thin-library refactor of the CLI (behavior-identical)

`crates/gauntlet-cli` is bin-only. Add `src/lib.rs` exposing the entry
points (`pub fn run_cli(args: &[String]) -> std::process::ExitCode` or a
per-command API — your call, but the E2E builder will drive the BINARY, and
the tests drive the library). `main.rs` becomes a thin wrapper. Existing
doctor/reference/roles behavior must be byte-identical (`just doctor`, the
existing gates prove it). Share the root-discovery/context plumbing
(today `roles_context()`) across all commands. Update the module doc
("the full §20.1 surface lands with plan Task 20" is no longer true when
you are done) and `usage()`.

## Item 2 — `gauntlet run` + `--dry-run` (the milestone)

The full control path already exists as `RunController` (compose/
start_run/resume_run). Your job is the composition root. Best template:
`crates/gauntlet-cli/tests/p6_budget_control.rs` lines ~679-830 (a complete
`ControllerDeps` literal over real adapters). Requirements:

- Preflight: doctor must exit OK before anything runs (reuse the existing
  `measure_backend`/`measure_runner` plumbing — refactor shared, do not
  duplicate).
- Runner selection (D2): `runners.default` (GAUNTLET.md / `GAUNTLET_AGENT_
  RUNNER`) selects among THREE fronted tokens: `prime-agent`, `cli-agent`,
  and a NEW `mock-agent` token you add to the mock adapter crate
  (`pub const RUNNER_TOKEN: &str = "mock-agent";` — mirror the siblings).
  Unset/unknown ⇒ typed configuration failure, non-zero exit, no dispatch,
  with a message that names the mechanism (config key + env var). The repo's
  own GAUNTLET.md stays untouched (UNK-003 remains the operator's).
  The mock composition seam: scenario selection via the smallest honest
  mechanism (inventory the mock crate first; a documented test-hook env var
  is acceptable — record the choice; if you touch `.env.example`, keep the
  `env_example_complete` arch test green).
- Workspace provider (R-1b closure): wire B1's
  `gauntlet-adapter-workspace-fs` provider into `ControllerDeps.workspaces`,
  root = `<state_dir>/<WORKSPACES_DIR_NAME>`, so per-unit workspaces are
  real in the composed path.
- Wrap caller (D3, R-3): for the `cli-agent` token, after
  `AuthorityGate::admit`, call `wrap_builder_command` and invoke through its
  verdict; with the shipped `none` backend posture this yields
  `BuilderCommand::Unwrapped` — the composition-level proof for the wrap
  call goes in your tests. R17-F4: when a LIVE backend wraps, the runtime-
  provided result-contract directory must appear in the policy's writable
  roots — wire it and cover with the cheapest honest test you can; if the
  live-bwrap composed leg is impractical here, record it for task25
  explicitly in your report (do not silently skip).
- Deny-set wiring (R2-N1): the loaded roles' `context.deny` sets must reach
  the comparison envelope inputs at composition — inventory how the
  controller builds envelopes (ComparisonDeps / envelope.rs) and thread the
  deny sets from the roles loader through `ControllerDeps` or the
  comparison deps; add one test proving a deny-set symbol cannot appear in
  a built envelope through the composed path.
- Escalation (NEW production adapter): no production `HumanEscalation`
  exists (proof-local stubs only — p6:622-635 records this). Add
  `crates/adapters/gauntlet-adapter-escalation-fs`: durable filesystem
  escalation queue under the state dir (`escalate(item) -> Ticket` appends
  a typed record; `poll(ticket)` reads status), std-only, passing the
  existing HumanEscalation contract suite if one exists (grep
  `crates/gauntlet-ports/src/contract/`), else with crate tests proving
  durability + typed errors. Wire it into the run composition. This is also
  what `gauntlet escalations list` reads (below) — the escalation READ
  lives with the escalation adapter, NOT the StateStore (B2a's inventory
  finding).
- Dry-run (D1): `--dry-run` = doctor-clean preflight + config resolve +
  reference parse + `verify_unchanged` + scheduler plan preview; ZERO
  writes; no runner required; exit 0 with the preview. `resume` shares the
  run composition (controller.resume_run).

## Item 3 — the §20.1 surface (D8 depth, D9 write path)

Implement, per charter decision D8 (real where backing services exist;
typed not-wired failure naming the plan task where they do not):

- `gauntlet init` — scaffold a project: refuse to overwrite existing files;
  write a minimal GAUNTLET.md template, `.env.example` copy, directory
  skeleton. Honest, bounded, no policy fabrication.
- `gauntlet reference form|freeze|show|diff|verify` (digest/validate exist):
  form = formation entry (validates a candidate key via the existing
  validator when given `--path`; never fabricates; points at the
  wayfinder-gauntlet skill); freeze = standalone freeze through
  `FileReferenceProvider` into the evidence store, printing the digest;
  show = parse + summary (id/version, units by tier/authority, digest);
  diff = digest comparison of two paths; verify = `verify_unchanged`
  against `.gauntlet/reference.lock` (the typed twin of `just ruler`).
- `gauntlet status` — runs, units by tier, in-flight, budget burn, from
  store reads (compose from existing reads; B2a proved this is possible).
- `gauntlet inspect` — `--run <id>` with optional `--iteration/--invocation/
  --unit/--evidence` detail views, from the same reads.
- `gauntlet verify` — re-run ladder verification for `--unit` or a whole
  `--run` via `VerificationAuthority` + the composed ladder; prints
  three-state outcomes; NEVER settles.
- `gauntlet resume <run_id>` — controller.resume_run through the full
  composition.
- `gauntlet escalations list|resolve|retry` — list reads the escalation-fs
  adapter's records; resolve/retry go through
  `SettlementService::settle_operator_action` with an operator identity
  (D9: same atomic path; no second writer). If a listed escalation has no
  pending-settlement claim, say so truthfully.
- `gauntlet skills install|verify|list` — mirror roles: install repo-owned
  skills into the harness skill surface + `.gauntlet/skills.lock` (content
  digests, drift-relative repair semantics mirroring roles), verify =
  re-read + digest + lint, list = enumerate repo skills with provenance
  presence. Reuse the task19 lint; do not grow the REPO_OWNED_SKILLS
  semantics.
- `gauntlet improve` — parse subcommands, typed not-wired failure naming
  plan Task 23 (D8). Non-zero exit; honest help text.
- `gauntlet export` — `export_run` to stdout or `--out <path>`.

Every command: unknown arguments are usage errors, exit 2 (established
convention); secrets never printed; `key=value` stable phrasing (§20.3).

## Item 4 — run report consumption + `cli_surface` test target

- After a run (or resume), print the six-category report from
  `gauntlet_app::report` (b2a-report.md has the full API), populated from
  the run's real records + the command-manifest the CLI declares (D8:
  capability-scope rows are caller-declared — wire the CLI's real command
  table into the manifest parameter).
- `cli_surface` test target: snapshot of `--help` for the WHOLE command
  tree (golden file, committed; the update procedure is: regenerate
  deliberately with the change recorded — never because output drifted) +
  per-command unknown-arg/missing-arg exit-code smoke. Placement per D5:
  check `crates/gauntlet-arch-tests/src/escapes.rs` first; if hosting at
  the frozen locator `tests/e2e/cli_surface` would require changing
  arch-suite SEMANTICS (not just adding a path to an existing locator-
  anchored list), host in `crates/gauntlet-cli/tests/cli_surface.rs` and
  record the divergence in `.agents/reports/reference-candidates.md`
  (AK-063/AK-065 class). Never edit the ruler; never widen the arch suite's
  standing re-litigation trigger without recording it.

## Boundaries

- Do NOT touch: `.wayfinder/**`, `.gauntlet/**` contents (except writing
  `skills.lock`/workspace state at RUN time through the adapters, never
  editing locks by hand), `.agents/specs/**`, `tests/fixtures/**` (B3 owns
  the fixture project), B1's adapter crate, the controller's control logic.
  CLI + new adapter crates + the mock token const are yours.
- `just ruler` before and after your slice; `just doctor` stays green.
- Compile discipline per charter addenda: flock + `sccache --stop-server`
  inside the lock; background+poll cadence; declare-first checkpoints.

## Checkpoint protocol (mandatory)

`.agents/reports/task20/b2-progress.md`: STARTED/DONE lines per item with
evidence. Finish with `.agents/reports/task20/b2-report.md`: what landed,
the exact command table (command → backing service → real/not-wired), the
CLI library API surface (B3 composes the BINARY, but the design critic
needs this), test counts, teeth evidence, decisions taken where this brief
left you a choice (scenario mechanism, cli_surface placement, escalation
contract suite), deviations, and anything deliberately not done.

## Done when

`just check`; `just test`; `just lint`; `just deny`; `just boundaries`;
`just ruler`; `just doctor` all pass; `cargo run -q -p gauntlet-cli --
run --dry-run "seeded bounded task"` exits 0 from the repo root (D1: no
writes — verify with `git status --porcelain` before/after showing no new
artifacts outside your report files); the `cli_surface` snapshot test
passes. Workspace test count should grow by your slice's tests; any
pre-existing red outside your scope: report, don't fix.
