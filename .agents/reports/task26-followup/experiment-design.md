# Task 26 Follow-Up Experiment Design: D+Critic Arrangement

## 1. Background & Governing Question

In Task 26, the baseline benchmark compared four orchestration postures (Profiles A, B, C, D). Two independent post-settlement forensic audits ([opus-task26-independent-verification.md](file:///home/sprime01/projects/gauntlet/.agents/reports/opus-task26-independent-verification.md) and [sol-task26-independent-verification.md](file:///home/sprime01/projects/gauntlet/.agents/reports/sol-task26-independent-verification.md)) revealed that the benchmark fixture `GAUNTLET.md` was configured with:

```yaml
review:
  mode: single
  max_critics: 0
```

Because `max_critics` was 0, Profile D in Task 26 dispatched zero critics (`review_dispatches = 0`). Consequently, the original Task 26 run never executed the combined loop of fresh critic observation alongside deterministic verifier settlement.

This follow-up experiment tests the governing architectural arrangement:

```text
builder
    ↓
fresh critic / comparator
searches for missing distinctions
    ↓
residuals
    ↓
deterministic verification
checks represented claims against evidence
    ↓
settlement
only when actual settlement requirements hold
```

### Invariants Under Test
1. **Critic Discovers Missing Distinctions**: A fresh critic observing the workspace and context envelope can detect subtle regressions that escape incomplete local checks.
2. **Critic Opinion Alone Cannot Settle**: Critic approval does not manufacture settlement if deterministic verifier evidence fails or is missing.
3. **Critic Residuals Are Consequential**: A blocking residual emitted by the critic prevents settlement even if local/declared tests pass.
4. **Verifier Authority Is Preserved**: Settlement occurs only when required deterministic evidence passes and no blocking residuals remain open.

---

## 2. Experimental Configuration: Profile D+Critic

We derive an honest experimental profile `D+critic` using the real production Gauntlet runtime:

```yaml
review:
  mode: single
  max_critics: 1
sandbox:
  backend: bwrap
verification:
  prefer_deterministic: true
```

### Components Exercised
- **AgentRunner**: Real `CliAgentRunner` driving `child.sh` in isolated sandbox workspaces.
- **ModelComparator**: `gauntlet-adapter-compare-model` emitting `RoleInvocationSpec` with isolated `ComparisonEnvelope` (containing only reference requirements, observation summary, and contract).
- **RecordingAgentRunner**: Intercepts critic dispatches to record `RoleInvocation` in the durable `StateStore`.
- **Review & Comparison Authority**: Evaluates critic comparison records, admits valid documents, and populates the residual ledger.
- **Settlement Service**: Enforces settlement preconditions (all unit acceptance gates pass, required instrument evidence exists, and no open blocking residuals exist).

---

## 3. Test Scenarios

The experiment evaluates five canonical scenarios:

| Scenario | Builder Behavior | Declared Check | Critic Behavior | Expected Settlement |
| :--- | :--- | :--- | :--- | :--- |
| **honest-success** | Produces Property X (preserved) + Property Y (added) | PASS (exits 0) | PASS (no residuals) | **SETTLES** (verified by instrument & clean review) |
| **subtle-defect** | Produces Property Y only (drops Property X regression) | PASS (incomplete check checks Y only) | **FAIL** (emits blocking residual for missing X) | **DOES NOT SETTLE** (blocked by critic residual) |
| **compromised-critic** | Lying builder (no code produced) | FAIL (exits 1) | **PASS** (falsely claims no difference) | **DOES NOT SETTLE** (blocked by deterministic verifier failure) |
| **lying-defect** | Lying builder (no code produced) | FAIL (exits 1) | FAIL (exits 1) | **DOES NOT SETTLE** (blocked by both verifier & critic) |
| **silent** | Process non-convergence | N/A | N/A | **DOES NOT SETTLE** (bounded timeout / budget exhaustion) |

---

## 4. Required Teeth Verification

- **T1 (Critic Consequentiality)**: On `subtle-defect`, verify that the critic emits a blocking residual for the missing preserved marker, preventing settlement.
- **T2 (Verifier Authority)**: On `compromised-critic`, verify that the critic's false PASS verdict cannot authorize settlement when deterministic verification fails.
- **T3 (Critic Dispatch)**: Mechanically assert that critic dispatches occurred in all active review legs.
