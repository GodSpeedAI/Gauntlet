# Tutorial: Your First Gauntlet Run

This hands-on tutorial guides you through executing your first transformation loop using Gauntlet's deterministic mock runner. By the end of this tutorial, you will have run preflight checks, executed a multi-unit run, inspected generated evidence, and verified settlement reports.

---

## Prerequisites

- Access to the repository checkout.
- Rust toolchain and `just` installed (or running in WSL2).
- Terminal shell opened at the repository root.

---

## Step 1: Verify Environment with `gauntlet doctor`

Before executing any run, run the preflight doctor to verify your configuration, reference files, and writable paths:

```bash
cargo run -q -p gauntlet-cli -- doctor
```

### Expected Output
```text
gauntlet doctor (spec 8.5 preflight)
  1. config                   pass
  2. reference                pass
      - manifest             pass
      - answer key           pass (70 units)
      - digest               pass
  3. runners                  pass
  4. capabilities             pass
  5. sandbox policies         pass
  6. required capabilities    pass
  7. writable paths           pass
  8. budget                   pass
exit: OK
report: .agents/reports/doctor.json
```

If any check fails, review the [Troubleshooting Guide](../troubleshooting.md).

---

## Step 2: Execute a Dry-Run Transformation

Execute a dry-run to inspect the plan and scheduled units without modifying code:

```bash
cargo run -q -p gauntlet-cli -- run --dry-run "Implement user password hashing"
```

### Expected Output
The runtime resolves the reference, calculates eligible units based on dependencies, and prints the scheduled dispatch plan without invoking any builder processes.

---

## Step 3: Run the Mock Scenario E2E

Now execute an end-to-end run using the scripted mock runner:

```bash
export GAUNTLET_AGENT_RUNNER=mock-agent
export GAUNTLET_MOCK_SCENARIO=happy_path
cargo run -q -p gauntlet-cli -- run "Implement user password hashing"
```

### What Happens During Execution
1. **Freeze**: The reference digest is locked and verified against `.gauntlet/reference.lock`.
2. **Dispatch**: The mock runner admits the task and simulates code changes in an isolated temporary worktree.
3. **Observation**: Observers inspect compilation status and test execution.
4. **Comparison**: The comparator inspects the blind comparison envelope.
5. **Verification**: Verifiers inspect test reports and certify `Outcome::PASS`.
6. **Settlement**: Preconditions hold, and the run commits settlement atomically.

---

## Step 4: Inspect Run Status and Settlement Records

Inspect the settled run state using its run ID:

```bash
cargo run -q -p gauntlet-cli -- status --run <run-id>
```

To inspect specific verification records and evidence bindings:

```bash
cargo run -q -p gauntlet-cli -- inspect --run <run-id> --verifications --settlements
```

### Expected Output
A detailed breakdown showing:
- Run status: `SETTLED`
- Settled units with `Outcome::PASS`
- Attached content-addressed `EvidenceRef`s (e.g. `sha256:...`)

---

## Step 5: Verify Evidence Directly

Run the verification command directly against the recorded evidence without invoking any models or builders:

```bash
cargo run -q -p gauntlet-cli -- verify --run <run-id>
```

### Expected Output
```text
Verifying run <run-id> against answer key:
  [PASS] AK-001: password hashing using argon2id (evidence: sha256:7f9b...)
  [PASS] AK-002: rejection of weak passwords (evidence: sha256:3a1c...)
Result: 2/2 Units PASS. Run is fully verified.
```

---

## Next Steps

Now that you have executed a basic run:
- Learn how to specify your own requirements: [Creating a Reference Tutorial](creating-a-reference.md).
- Understand how the loop works under the hood: [The Iteration Loop Workflow](../workflows/iteration-loop.md).
- Explore CLI commands and options: [CLI Reference](../reference/cli.md).
