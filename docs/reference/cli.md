# Reference: Command-Line Interface (`gauntlet`)

This document provides the complete syntax, arguments, flags, environment variables, exit codes, and output formats for the `gauntlet` CLI binary.

---

## Command Overview

```text
gauntlet <command> [options]
```

### Global Exit Codes

| Code | Meaning | Description |
|---|---|---|
| `0` | Success | The command completed successfully with all gates/preconditions satisfied. |
| `1` | Operational Failure | Preflight check failed, unit verification failed, or budget was exhausted. |
| `2` | Usage / Syntax Error | Unknown command, missing required arguments, or invalid option syntax. |

---

## Subcommands

### `gauntlet doctor`
Executes the 8 preflight checks specified in `Core-spec.md` §8.5.

```text
gauntlet doctor [--config <path>]
```

- **Options**:
  - `--config <path>`: Optional path to an alternate `GAUNTLET.md` configuration file.
- **Side Effects**: Writes `.agents/reports/doctor.json` and prints a formatted summary to stdout.
- **Exit Codes**:
  - `0`: All 8 checks passed.
  - `1`: One or more required checks failed.
- **Implementation**: [`crates/gauntlet-cli/src/doctor_cmd.rs`](../../crates/gauntlet-cli/src/doctor_cmd.rs).

---

### `gauntlet init`
Initializes a repository with Gauntlet's standard governance directories and templates.

```text
gauntlet init
```

- **Side Effects**: Creates `.wayfinder/reference.yaml`, `.wayfinder/answer-key.md`, `.gauntlet/policies/`, and `GAUNTLET.md` if they do not exist.
- **Exit Codes**: `0` on success, `1` if files cannot be created.
- **Implementation**: [`crates/gauntlet-cli/src/init_cmd.rs`](../../crates/gauntlet-cli/src/init_cmd.rs).

---

### `gauntlet reference`
Manages the reference lifecycle: validation, hashing, freezing, and diffing.

```text
gauntlet reference digest --path <path>
gauntlet reference validate --path <path>
gauntlet reference form [--path <path>]
gauntlet reference freeze [--run <run-id>]
gauntlet reference show
gauntlet reference diff --path <path> --against <path>
gauntlet reference verify
```

- **Subcommands**:
  - `digest`: Computes normalized SHA-256 digest of the specified reference file.
  - `validate`: Validates Unit schemas in `answer-key.md` against spec §5.3.2.
  - `form`: Invokes formation assistance for incomplete references.
  - `freeze`: Locks the current reference into `.gauntlet/reference.lock`.
  - `show`: Prints the currently active Reference and Unit inventory.
  - `diff`: Compares two reference versions and outputs semantic differences.
  - `verify`: Verifies that disk reference files match `.gauntlet/reference.lock`.
- **Implementation**: [`crates/gauntlet-cli/src/reference_cmd.rs`](../../crates/gauntlet-cli/src/reference_cmd.rs).

---

### `gauntlet run`
Executes an autonomous or guided transformation loop against the frozen reference.

```text
gauntlet run [--dry-run] [--config <path>] "<intent>"
```

- **Arguments**:
  - `"<intent>"`: High-level natural language description of the desired change.
- **Options**:
  - `--dry-run`: Resolves the reference and schedules units without invoking builders or modifying files.
  - `--config <path>`: Uses an alternate configuration file.
- **Exit Codes**:
  - `0`: Run successfully settled (`TerminalState::Settled`).
  - `1`: Run ended in `Blocked`, `BudgetExhausted`, `Escalated`, `Aborted`, or `Inconclusive`.
- **Implementation**: [`crates/gauntlet-cli/src/run_cmd.rs`](../../crates/gauntlet-cli/src/run_cmd.rs).

---

### `gauntlet status`
Displays the current execution status and metrics for a run.

```text
gauntlet status --run <run-id>
```

- **Options**:
  - `--run <run-id>`: The ULID identifier of the run.
- **Output**: Total iterations, spent budget (tokens/USD), and counts of settled, open, and blocked units.
- **Implementation**: [`crates/gauntlet-cli/src/state_cmd.rs`](../../crates/gauntlet-cli/src/state_cmd.rs).

---

### `gauntlet inspect`
Deeply inspects runtime entities for a specific run.

```text
gauntlet inspect --run <run-id> [--iteration <n>] [--unit <unit-id>] [--verifications] [--settlements]
```

- **Options**:
  - `--iteration <n>`: Inspects observations and residuals for iteration $n$.
  - `--unit <unit-id>`: Inspects verification history and residual status for a Unit.
  - `--verifications`: Lists all verification records.
  - `--settlements`: Displays settlement transaction metadata and evidence bindings.
- **Implementation**: [`crates/gauntlet-cli/src/state_cmd.rs`](../../crates/gauntlet-cli/src/state_cmd.rs).

---

### `gauntlet verify`
Re-evaluates all units against existing evidence artifacts without invoking builders.

```text
gauntlet verify --run <run-id> [--unit <unit-id>]
```

- **Implementation**: [`crates/gauntlet-cli/src/verify_cmd.rs`](../../crates/gauntlet-cli/src/verify_cmd.rs).

---

### `gauntlet resume`
Replays the event log to recover and resume an interrupted or failed run.

```text
gauntlet resume <run-id>
```

- **Implementation**: [`crates/gauntlet-cli/src/run_cmd.rs`](../../crates/gauntlet-cli/src/run_cmd.rs).

---

### `gauntlet escalations`
Manages the operator escalation queue.

```text
gauntlet escalations list
gauntlet escalations resolve <ticket-id> --resolution <approved|rejected> [--evidence <path>]
gauntlet escalations retry <ticket-id>
```

- **Implementation**: [`crates/gauntlet-cli/src/escalations_cmd.rs`](../../crates/gauntlet-cli/src/escalations_cmd.rs).

---

### `gauntlet roles`
Manages role definitions and installs them into configured agent harnesses.

```text
gauntlet roles install --harness <id>
gauntlet roles verify
gauntlet roles list
```

- **Implementation**: [`crates/gauntlet-cli/src/roles_cmd.rs`](../../crates/gauntlet-cli/src/roles_cmd.rs).

---

### `gauntlet skills`
Installs and verifies companion skills.

```text
gauntlet skills install
gauntlet skills verify
gauntlet skills list
```

- **Implementation**: [`crates/gauntlet-cli/src/skills_cmd.rs`](../../crates/gauntlet-cli/src/skills_cmd.rs).

---

### `gauntlet export`
Exports the complete deterministic JSONL event log for a run.

```text
gauntlet export --run <run-id> [--out <path>]
```

- **Implementation**: [`crates/gauntlet-cli/src/state_cmd.rs`](../../crates/gauntlet-cli/src/state_cmd.rs).

---

### `gauntlet serve` *(feature: `surfaces`)*
Launches the HTTP JSON API operator surface.

```text
gauntlet serve
```

- **Environment**: Configured via `GAUNTLET_API_BIND` (default: `127.0.0.1:8080`).
- **Implementation**: [`crates/gauntlet-cli/src/serve_cmd.rs`](../../crates/gauntlet-cli/src/serve_cmd.rs).
