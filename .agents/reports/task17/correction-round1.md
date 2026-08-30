# Task 17 — falsifier round-1 adjudication + correction contract (round: falsifier-round1.md)

Round-1 verdict NOT_CONFIRM (2 major / 3 minor / 0 blocking; every executed attack,
five mutation teeth RED→GREEN proven, scratch removed, ruler PASS at start and end).
All checkable claims were verified against the tree before routing. Disposition:

| Id | Severity | Disposition |
|---|---|---|
| FIND-1 lock-forgery source anchor (drift invisibility) | major | **ACCEPT → COR-R1-1** |
| FIND-2 resolved $VAR value leaks into errors/stderr | major | **ACCEPT → COR-R1-2** |
| FIND-3 duplicate-entry lock accepted | minor | **ACCEPT → COR-R1-3** |
| FIND-4 repo artifacts need unadvertised RESULT_CONTRACT_DIR | minor | **DEFER → R17-F4** (trigger task20 CLI surface) |
| FIND-5 repair flag lock-relative not drift-relative | minor | **ACCEPT → COR-R1-4** |

## COR-R1-1 (FIND-1) — lock-recorded paths are DERIVED, never trusted

Invariant to restore: install.rs's tamper story — drift detection anchors to the
REVIEWED source tree. The falsified claim ("the only way to hide that drift is to
edit the reviewed source file itself") becomes true again structurally.

- `verify` (and any consumer of a lock) must DERIVE each entry's expected paths from
  the values it already trusts: the configured roles dir (B2's D02 anchored
  resolution), the configured state root, the harness label, the enforcement level,
  and the role stem. Concretely:
  - `source:` must equal `<configured roles_dir>/<stem>.md` (exact, resolved,
    absolute) — a lock naming any other in-project path (`rogue-src/builder.md`,
    A-3) is a typed `LockTampered` refusal naming the entry and the divergence.
  - `installed:` must equal the derived `<state_root>/roles/installed/<harness>/
    <stem>.md` (or `injectable/<harness>/` for advisory enforcement) — a lock
    pointing at a fabricated harness dir (A-1) is refused the same way.
  - The `harness:` label must be a REGISTERED harness id (refuse unregistered,
    fail-closed, naming the label).
- Same pattern as COR-D03's resolved-shape write-path validation: derived-shape
  checks precede digest comparisons; a digest-valid but shape-invalid lock is a
  tamper refusal, never a drift verdict.
- Teeth required (new tests beside the existing reforge test):
  `full_lock_source_redirect_reforge_cannot_bless_drift` (the falsifier's A-3 as a
  refusal test) and `cross_harness_relabeled_relocated_lock_is_tamper` (A-1 as a
  refusal test).

## COR-R1-2 (FIND-2) — a resolved indirection VALUE never reaches error text

Invariant: definition.rs's own documented claim — the typed error names the
VARIABLE and the locus, and the VALUE is never echoed (spec §8.1).

- Audit every typed error Display in `roles/definition.rs` and `roles/install.rs`
  that interpolates an already-resolved value (mode/network/tool token refusals,
  `WritePathEscape`/`WritePathProtected`, any locus message carrying a resolved
  path). Replace with the unresolved token or the variable name + locus. Path
  refusals name the SHAPE ("escapes the state root", "outside the project"), never
  the resolved value.
- Existing tests that assert the DEFECTIVE behavior (assertions requiring a
  resolved value inside a message) are updated to assert the opposite: the value
  is absent and the variable/locus is present. The literal-unresolved-token
  assertions (e.g. forged `../escape.md` tests) are unchanged — an unresolved
  literal in the file is not a secret.
- Teeth required: `resolved_indirection_value_never_echoes_into_error_text` pair
  (mode refusal + write-path escape), each asserting the canary appears NOWHERE in
  the error Display and the variable name does.
- Red discipline: each new tooth is shown RED on the pre-correction tree (the
  falsifier's e1a/e1b demonstrations are the RED evidence; re-derive at least one
  RED personally before relying on GREEN).

## COR-R1-3 (FIND-3) — duplicate-entry lock refused

`RolesLock::parse` refuses a repeated role stem with a typed `LockTampered` naming
the stem (COR-D01's logic extended one shape further: refuse any lock shape install
would never write). Tooth: `duplicate_entry_lock_is_tamper` (the falsifier's A-2
inverted).

## COR-R1-4 (FIND-5) — the repair flag is drift-relative

`repaired` reports TRUE when installed bytes were actually rewritten during the
reinstall (compare before/after per role), not merely when lock bytes differ. The
CLI message stays honest in both directions (never over-reports). Tooth: a
reinstall over out-of-band installed drift reports the repair.

## Constraints (builder)

- Touch surface: `crates/gauntlet-app/src/roles/{definition,install}.rs`,
  `crates/gauntlet-app/tests/roles.rs`, and only if a message shape requires it
  `crates/gauntlet-cli/src/main.rs` (stderr stays `{error}`; no new CLI surface).
  No arch-gate edits, no `.wayfinder/**`, no `.gauntlet/**`, no preregistration,
  no CURRENT_STATUS, no firewalled reports. Do NOT read
  `.agents/reports/task17/falsifier-round1.md`, critic-design.md, b2-*.md,
  IN-FLIGHT.md, or `.agents/reports/task15|16/**` — this file is your complete
  contract.
- RED-first per tooth where practical; byte-identical restore discipline with
  sha256 proof for any mutation; ruler PASS at start and end; package battery
  (`just check-package gauntlet-app`, `just test-package gauntlet-app`) green
  before reporting; checkpoint `.agents/reports/task17/b3-progress.md` as you go
  and finish with `.agents/reports/task17/b3-builder.md` (exact touch surface,
  per-correction evidence, deviations).
- Recording deviations is mandatory, never optional. A claim of completeness
  carries no settlement authority; a fresh blind round 2 will re-attack the
  corrected surface.