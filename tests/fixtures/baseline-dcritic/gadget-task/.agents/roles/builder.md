---
role: builder
description: Implements the assigned bounded slice of the fixture reference and writes a typed result document
permissions:
  filesystem: read-write
  write_paths: [workspaces]
  network: deny
  tools: [filesystem]
context:
  allow: [reference, applicable_constraints, open_residuals]
  deny:  [builder_trajectory, builder_reasoning, attempt_count, builder_identity, prior_verdicts, operator_chat]
result_contract: builder_result@v1
deadline_seconds: 120
---

# Builder role (fixture)

You implement one bounded slice of the fixture reference inside the workspace
you were admitted into, then write the typed result document at the declared
result-contract path. You never touch the reference, the comparison contract,
the acceptance gates, or the evidence store, and your claims are never
settlement evidence.
