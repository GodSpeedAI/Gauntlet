# How-To: Configure Runtime Budgets & Safety Ceilings

This guide explains how to configure resource limits, execution timeouts, financial spend ceilings, and recovery reserves to prevent runaway loops.

---

## Goal

Configure runtime execution bounds across iterations, wall-clock time, tokens, USD cost, and recovery reserves using `GAUNTLET.md` and environment variables.

---

## The Budget Hierarchy

Gauntlet resolves configuration in four layers with strict precedence:

```text
Defaults Table  ──►  GAUNTLET.md Front Matter  ──►  Environment Variables  ──►  CLI Flags
(Lowest)                                                                        (Highest)
```

If a setting is omitted in higher layers, conservative defaults apply automatically (Invariant I10: *The runtime is bounded even when configuration is incomplete*).

---

## Procedure

### Step 1: Configure `GAUNTLET.md`

Open `GAUNTLET.md` at the project root and edit the `budget` section in the YAML front matter:

```yaml
---
budget:
  max_rounds: 10               # Maximum corrective iterations (default: 8)
  max_parallel: 2             # Maximum concurrent builder subagents (default: 2)
  max_depth: 1                # Maximum sub-agent recursion depth (default: 1)
  max_wall_seconds: 1800      # Wall-clock timeout in seconds (default: 3600 = 1 hour)
  max_tokens: 1500000         # Maximum combined LLM tokens (default: 2,000,000)
  max_cost_usd: 15            # Maximum financial spend ceiling (default: 25 USD)
  recovery_reserve_pct: 20    # Quarantined budget for rollback & recovery (default: 15%)
  diagnostic_budget: 3        # Number of bounded diagnostic test cycles (default: 2)
---
```

### Step 2: Override via Environment Variables (Optional)

In automated CI/CD pipelines or constrained environments, override any setting using standard Annex E environment variables:

```bash
# Reduce execution time to 15 minutes in CI
export GAUNTLET_BUDGET_MAX_WALL_SECONDS=900

# Set a strict financial spend limit of $5.00
export GAUNTLET_BUDGET_MAX_COST_USD=5

# Increase recovery reserve to 25% for high-risk refactors
export GAUNTLET_RECOVERY_RESERVE_PCT=25
```

### Step 3: Override via CLI Flags (Optional)

For ad-hoc runs, specify overrides on the command line:

```bash
gauntlet run --config custom-config.yaml "Refactor database migrations"
```

---

## Understanding the Recovery Reserve

The `recovery_reserve_pct` setting (default: 15%) safeguards against stranded states:
- **How it works**: If `max_cost_usd = 20` and `recovery_reserve_pct = 15`, then $3.00 is quarantined.
- **Forward Threshold**: When forward execution spend reaches $17.00, `would_breach_reserve()` returns true.
- **Enforcement**: The runtime terminates forward iterations immediately with `TerminalState::BudgetExhausted`.
- **Reserve Use**: The remaining $3.00 is preserved exclusively for rolling back uncommitted git changes, writing diagnostic reports, and committing the state database.

---

## Validation

Run `gauntlet doctor` to verify the effective budget calculations:

```bash
cargo run -q -p gauntlet-cli -- doctor
```

Inspect Section 8 in the output:
```text
  8. budget                   pass
      domain Limits constructed from the effective budget
      recovery reserve 20 percent, below one hundred
      diagnostic budget 3
      - ceilings             pass
          rounds 10 parallel 2 depth 1 wall 1800 tokens 1500000 micro-usd 15000000
```

---

## Common Failure Symptoms

| Symptom | Cause | Fix |
|---|---|---|
| `gauntlet doctor` fails check 8: `recovery reserve >= 100` | `recovery_reserve_pct` set to 100 or higher. | Set recovery reserve between 1% and 99% (recommended: 15–20%). |
| Run terminates prematurely with `BudgetExhausted` | Forward work breached the recovery reserve. | Increase `max_cost_usd` or `max_tokens` in `GAUNTLET.md`. |
| Run times out before finishing iteration 1 | `max_wall_seconds` set too low for large test suite. | Increase `max_wall_seconds` to accommodate test execution. |
