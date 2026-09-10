# T25 — Whole-system independent conformance and capability settlement (builder evidence)

Task: gauntlet-tui-prime-workbench-T25 (P3, independent_adversarial). Date: 2026-09-09.
Preregistration: `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T25.prereg.yaml` — `just prereg-check gauntlet-tui-prime-workbench-T25`: PASS (frozen hash `74e86008f0b76275b7f1ee64aed7bbca9a944992b63214fe830e61f116d66f87`), never modified.
Scope: implement the two long-NOT_IMPLEMENTED modes for real (`just tui-test all`, `just tui-preflight conformance`); mechanically recompute requirement/DSN coverage; run the full final proof matrix; consolidate settled/trigger-record state for the independent verifier. All 24 predecessor tasks were settled before T25 dispatch.

Files owned by this task: `workbench/src/plan-traceability.ts` (new), `workbench/src/tui-test-all.ts` (new), `workbench/src/coverage-recomputation.test.ts` (new), `justfile` (`tui-test all`, `tui-bench large-case|long-horizon|all-required`), `scripts/tui-preflight.sh` (conformance mode), `workbench/src/long-horizon.bench.ts` (comment-only header sweep), `evidence/T00/settlement-record.md` + `evidence/T00A/settlement-record.md` (append-only consolidations, see §2.3), this document. No new dependencies. No test weakened; no plan/spec/DESIGN/prereg byte touched.

## 1. The two long-NOT_IMPLEMENTED modes, implemented for real

### 1.1 `just tui-test all` — full normative conformance run

The NOT_IMPLEMENTED stub is removed. The mode now:

1. Builds the normative suite registry MECHANICALLY from the frozen plan (`workbench/src/plan-traceability.ts` parses `.agents/plans/gauntlet-tui-prime-workbench.plan.yaml` with a small pure-TS YAML-subset scanner; no dependency added). **Inclusion rule (explicit, mechanical):** a suite is a REGISTERED NORMATIVE SUITE iff it resolves a `just tui-test <selector>` gate entry declared by a plan task block T00A..T24 (T25's own gate is this run — self-referential, excluded). Consequences decided by the rule, not taste: `index.test.ts` (T00's non-vacuity smoke), `src/testing/*` helpers (`canonical-surface-stub.ts`, `cross-surface-matrix.ts`), the gated battery file `operator-journeys-pty.test.ts`, and the other instrument suites (`payment-budgets`, `planner-swap`, `release-manifest`, `substrate-inventory`, `worker-folds`, `inspector-lens-wiring`) are NOT registry rows because no plan gate names them; `canonical-boundary` IS a registry row because the settled P3 T01 gate names it — excluding a plan-declared gate selector would move the ruler.
2. Resolves every selector to `src/<selector>.test.ts` or to a RECORDED adjudication (below). An unresolvable selector aborts with exit 2: a missing normative suite is a conformance failure.
3. Runs each suite as its own `bun test` invocation (verbatim output shown), prints a per-suite table (pass/fail/skip/exit + owning plan gates), and FAILS (exit 1) if any normative suite fails or selects zero tests.
4. Skips stay visible per-suite by design; their named, evidence-backed adjudication is `tui-preflight conformance`.

**Plan-gate selector divergence adjudication (carried, not papered over).** Two plan gate selectors match no file; the frozen plan text is NOT edited:

* `kernel-serialization` → `src/prime-kernel.test.ts` — orchestrator adjudication at T24 settlement (current-status.yaml; T24 evidence honest-gap 1): T05's suite is `prime-kernel.test.ts`, semantic content identical (serialization without interleaving; liveness != success).
* `kernel-object-lifecycle` → `src/prime-kernel.test.ts` — same divergence class, carried to T05's second plan selector in this task's registry build: T05's own settled evidence (evidence/T05/prime-kernel.md gate table) executed `just tui-test prime-kernel` (5/5 PASS) as its gate, and `prime-kernel.test.ts` contains the object-lifecycle content ("transitions object lifecycle: ephemeral -> pinned -> artifactized"). Recorded here and in `ADJUDICATED_SUITE_ALIASES`; flagged for the verifier's review.

Registry: **49 normative suites** from the plan gates; 2 adjudicated selectors. Observed run: **529 pass / 0 fail / 9 skip** — the 9 skips are exactly the gated/recorded capabilities adjudicated in §1.2 (1 T13 real-PTY, 7 T24 real-substrate, 1 T20 real-sshd). Adjudicated suites are marked `PASS*` in the table.

### 1.2 `just tui-preflight conformance` — runtime qualification (REQ-CONFIG-010/011/012)

The NOT_IMPLEMENTED stub is replaced with the real mode. It first re-verifies document integrity WITHOUT rebinding (same checks as bootstrap: package skeleton, exact version pins, `bun.lock` frozen-lockfile sync, `bun audit`, and the plan's pinned spec/DESIGN sha256 hashes), then qualifies every declared runtime capability under the prereg-D2 rule — a REQUIRED integration is either

* **EXECUTED** here, live, and passing: bun >= 1.4.0; `bun:sqlite` open/write/read; Bun PTY facility attach; OpenTUI/React/reconciler installed at exact pinned versions; `prime-agent --version` == 0.8.0 matching the T24-qualified substrate inventory (drift FAILS with a requalification instruction); release artifact present with `dist/release-manifest.json` digest + required fields verified (`verifyReleaseManifest`); tmux present; every one of the 27 settled tasks' evidence dirs carrying a `Confirmation:` record; plus an **unbacked-gate sweep**: every boolean env gate (`GAUNTLET_T… === "1"`) declared anywhere in the test tree must have a declared conformance row — an env-gated battery with no row FAILS as an unbacked skip.
* **GATED** on an opt-in env var, printed with the exact variable and checked against its backing evidence: `GAUNTLET_T13_REAL_PTY=1` (T13 real-PTY battery), `GAUNTLET_T20_REAL_PTY=1` (T20 real-PTY operator journeys), `GAUNTLET_T24_REAL_PRIME=1` (T24 real-substrate lifecycle battery), and the real-`sshd` SSH transport leg — environment-gated; if the probe EVER succeeds, the mode runs `src/ssh-target.test.ts` right there instead of accepting the skip; today sshd is unreachable (connection timeout) and the skip is backed by T20's recorded evidence ("The real-`sshd` SSH leg did not run").
* **UNQUALIFIED** — honestly recorded limitation, printed with owning task + trigger-record and evidence-checked: T14B `role_binding` declaration fixture (no local `.sea` fixture emits one on the slice0b grammar; `SXR_LIVE_QUALIFICATION.role_binding_declaration_fixture_real: false` re-verified in source); T24 `schedule list`/`cancel` non-default daemon socket (validator rejects what `add` accepts; ADD ran for real); T24 `send --steer`/`--follow-up` HELP/IMPLEMENTATION drift (`documented_but_refused`); the release entry's cold-start host loop (no live `gauntlet serve` binding; embedded OpenTUI React viewport remains recorded later-task wiring — REQ-UX-007 live-endpoint breadth).

Exit 0 only when nothing is silently missing; every GATED/UNQUALIFIED row is visible, named, and evidence-backed, which prereg D2 treats as conforming. During development the mode's own sweeps caught and I fixed: a stderr/stdout capture bug in the prime version check, an unbacked-marker mismatch, and a sweep false-positive on the non-gate control var `GAUNTLET_T24_CAPTURE_DIR` (the sweep now matches the boolean gate idiom only).

### 1.3 T00/T00A evidence consolidation (append-only, disclosed)

The first-wave evidence artifacts predate the `Confirmation:`-line convention; their settlement verdicts live in the orchestrator's status anchor. To make the mechanical settled-evidence index see them without touching any existing file, `evidence/T00/settlement-record.md` and `evidence/T00A/settlement-record.md` were APPENDED — each quotes its `latest_evidence.T00/A.confirmation` entry verbatim, names its source, and states that no new verification is performed. Flagged for the verifier.

## 2. Mechanical coverage recomputation (prereg D1/P1)

`workbench/src/coverage-recomputation.test.ts` + `workbench/src/plan-traceability.ts`:

* Source: `traceability.requirement_to_tasks` and `verification.design_clause_owners` are parsed DIRECTLY from the frozen plan file (no duplicated mapping anywhere). Self-checks pin the parser to the plan's own declared counts (104/104, 15/15, unmapped = []), so a silently rotting parser fails loudly.
* Rule: every REQ and DSN owner must be a plan task whose evidence dir contains an artifact with a `Confirmation:` line. DESIGN.md and the bound spec must still match the plan's pinned hashes. DSN-015 delegates to the existing guards: `renderer-boundary` + `opentui-shell` must be registered normative suites, and the live shell registry scan must refuse every roadmap id (`assertNoRoadmapSurfaces`, `registerShellView` on `harness`).
* Negative controls pin the checker itself (missing dir / Confirmation-less artifact / proper artifact), so it cannot pass vacuously.
* **Honest RED-first:** until this task's own evidence file existed, the suite failed on exactly `DSN-015 → T25` and the six `REQ-* → T25` mappings — the checker demonstrably fails when evidence is genuinely missing. With this document present it is green.

**Coverage summary: 104/104 REQs mapped and settled; 15/15 DSN clauses mapped and settled; 0 unmapped; bound hashes verified.** One exception honestly listed: `DSN-015` (owner T25) is settled by THIS document — the verifier should weigh that self-reference (the DSN-015 technical checks themselves are the independent renderer-boundary/registry/shell-scan suites cited in the table).

### 2.1 Requirement-to-evidence index (104 REQs, mechanically recomputed)

| REQ | Owning tasks | Settled evidence artifact(s) |
| :--- | :--- | :--- |
| `REQ-AFF-001` | T01, T09, T23 | T01: T01/canonical-boundary.md<br>T09: T09/operation-gateway.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-AFF-002` | T09, T11, T23 | T09: T09/operation-gateway.md<br>T11: T11/kernel-affordances.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-AFF-003` | T09, T23 | T09: T09/operation-gateway.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-AFF-004` | T08, T09 | T08: T08/focus-horizon.md<br>T09: T09/operation-gateway.md |
| `REQ-AFF-005` | T08, T09 | T08: T08/focus-horizon.md<br>T09: T09/operation-gateway.md |
| `REQ-BUN-001` | T03 | T03: T03/dependency-admission-ledger.md |
| `REQ-BUN-002` | T03 | T03: T03/dependency-admission-ledger.md |
| `REQ-BUN-003` | T06 | T06: T06/sqlite-projection-store.md |
| `REQ-BUN-004` | T13 | T13: T13/embedded-terminal.md |
| `REQ-BUN-005` | T20 | T20: T20/release-qualification.md |
| `REQ-BUN-006` | T17, T21 | T17: T17/horizon-payment.md<br>T21: T21/payment-benchmark.md |
| `REQ-CONFIG-001` | T02 | T02: T02/config-preflight.md |
| `REQ-CONFIG-002` | T02 | T02: T02/config-preflight.md |
| `REQ-CONFIG-003` | T02 | T02: T02/config-preflight.md |
| `REQ-CONFIG-004` | T03 | T03: T03/dependency-admission-ledger.md |
| `REQ-CONFIG-005` | T02, T20 | T02: T02/config-preflight.md<br>T20: T20/release-qualification.md |
| `REQ-CONFIG-010` | T02 | T02: T02/config-preflight.md |
| `REQ-CONFIG-011` | T09 | T09: T09/operation-gateway.md |
| `REQ-CONFIG-012` | T02 | T02: T02/config-preflight.md |
| `REQ-DATA-001` | T07 | T07: T07/workbench-model.md |
| `REQ-DATA-002` | T06, T16 | T06: T06/sqlite-projection-store.md<br>T16: T16/live-streams.md |
| `REQ-DATA-003` | T06, T16 | T06: T06/sqlite-projection-store.md<br>T16: T16/live-streams.md |
| `REQ-DATA-004` | T06, T16 | T06: T06/sqlite-projection-store.md<br>T16: T16/live-streams.md |
| `REQ-DATA-005` | T07, T12 | T07: T07/workbench-model.md<br>T12: T12/semantic-shell.md |
| `REQ-DATA-006` | T07 | T07: T07/workbench-model.md |
| `REQ-DATA-007` | T06 | T06: T06/sqlite-projection-store.md |
| `REQ-DOM-001` | T14 | T14: T14/domainforge-adapter.md |
| `REQ-DOM-002` | T14 | T14: T14/domainforge-adapter.md |
| `REQ-DOM-003` | T14 | T14: T14/domainforge-adapter.md |
| `REQ-DOM-004` | T00, T14, T14B | T00: T00/settlement-record.md<br>T14: T14/domainforge-adapter.md<br>T14B: T14B/sxr-evidence.md |
| `REQ-DOM-005` | T14, T14B | T14: T14/domainforge-adapter.md<br>T14B: T14B/sxr-evidence.md |
| `REQ-FOCUS-001` | T08 | T08: T08/focus-horizon.md |
| `REQ-FOCUS-002` | T08 | T08: T08/focus-horizon.md |
| `REQ-FOCUS-003` | T08, T17, T21 | T08: T08/focus-horizon.md<br>T17: T17/horizon-payment.md<br>T21: T21/payment-benchmark.md |
| `REQ-FOCUS-004` | T08, T17, T21 | T08: T08/focus-horizon.md<br>T17: T17/horizon-payment.md<br>T21: T21/payment-benchmark.md |
| `REQ-FOCUS-005` | T08 | T08: T08/focus-horizon.md |
| `REQ-GOAL-001` | T08, T12, T25 | T08: T08/focus-horizon.md<br>T12: T12/semantic-shell.md<br>T25: MISSING |
| `REQ-GOAL-002` | T01, T25 | T01: T01/canonical-boundary.md<br>T25: MISSING |
| `REQ-GOAL-003` | T04, T10, T25 | T04: T04/prime-host-topology.md<br>T10: T10/prime-recursion.md<br>T25: MISSING |
| `REQ-GOAL-004` | T03, T25 | T03: T03/dependency-admission-ledger.md<br>T25: MISSING |
| `REQ-GOAL-005` | T07, T12, T19, T25 | T07: T07/workbench-model.md<br>T12: T12/semantic-shell.md<br>T19: T19/headless-equivalence.md<br>T25: MISSING |
| `REQ-GOAL-006` | T08, T17, T21, T25 | T08: T08/focus-horizon.md<br>T17: T17/horizon-payment.md<br>T21: T21/payment-benchmark.md<br>T25: MISSING |
| `REQ-OPS-001` | T01, T04, T10, T16, T24 | T01: T01/canonical-boundary.md<br>T04: T04/prime-host-topology.md<br>T10: T10/prime-recursion.md<br>T16: T16/live-streams.md<br>T24: T24/prime-fresh-target.md |
| `REQ-OPS-002` | T01, T09, T16, T23 | T01: T01/canonical-boundary.md<br>T09: T09/operation-gateway.md<br>T16: T16/live-streams.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-OPS-003` | T01, T09 | T01: T01/canonical-boundary.md<br>T09: T09/operation-gateway.md |
| `REQ-OPS-004` | T08, T10, T23 | T08: T08/focus-horizon.md<br>T10: T10/prime-recursion.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-OUT-001` | T12, T25 | T12: T12/semantic-shell.md<br>T25: MISSING |
| `REQ-OUT-002` | T06, T25 | T06: T06/sqlite-projection-store.md<br>T25: MISSING |
| `REQ-OUT-003` | T16, T20, T25 | T16: T16/live-streams.md<br>T20: T20/release-qualification.md<br>T25: MISSING |
| `REQ-OUT-004` | T09, T11, T25 | T09: T09/operation-gateway.md<br>T11: T11/kernel-affordances.md<br>T25: MISSING |
| `REQ-PLUGIN-001` | T15 | T15: T15/plugins.md |
| `REQ-PLUGIN-002` | T15 | T15: T15/plugins.md |
| `REQ-PLUGIN-003` | T15 | T15: T15/plugins.md |
| `REQ-PLUGIN-004` | T15 | T15: T15/plugins.md |
| `REQ-PRIME-001` | T04 | T04: T04/prime-host-topology.md |
| `REQ-PRIME-002` | T04 | T04: T04/prime-host-topology.md |
| `REQ-PRIME-003` | T04, T10, T24 | T04: T04/prime-host-topology.md<br>T10: T10/prime-recursion.md<br>T24: T24/prime-fresh-target.md |
| `REQ-PRIME-004` | T05, T10, T24 | T05: T05/prime-kernel.md<br>T10: T10/prime-recursion.md<br>T24: T24/prime-fresh-target.md |
| `REQ-PRIME-005` | T04, T10, T24 | T04: T04/prime-host-topology.md<br>T10: T10/prime-recursion.md<br>T24: T24/prime-fresh-target.md |
| `REQ-PRIME-006` | T05, T10, T11 | T05: T05/prime-kernel.md<br>T10: T10/prime-recursion.md<br>T11: T11/kernel-affordances.md |
| `REQ-PRIME-007` | T05, T11 | T05: T05/prime-kernel.md<br>T11: T11/kernel-affordances.md |
| `REQ-PRIME-008` | T10 | T10: T10/prime-recursion.md |
| `REQ-PRIME-009` | T10, T16, T24 | T10: T10/prime-recursion.md<br>T16: T16/live-streams.md<br>T24: T24/prime-fresh-target.md |
| `REQ-PRIME-010` | T10, T16, T24 | T10: T10/prime-recursion.md<br>T16: T16/live-streams.md<br>T24: T24/prime-fresh-target.md |
| `REQ-SAFE-001` | T18, T19, T22 | T18: T18/security-hardening.md<br>T19: T19/headless-equivalence.md<br>T22: T22/recovery-matrix.md |
| `REQ-SAFE-002` | T09, T18 | T09: T09/operation-gateway.md<br>T18: T18/security-hardening.md |
| `REQ-SAFE-003` | T02, T18 | T02: T02/config-preflight.md<br>T18: T18/security-hardening.md |
| `REQ-SAFE-004` | T05, T18 | T05: T05/prime-kernel.md<br>T18: T18/security-hardening.md |
| `REQ-SAFE-005` | T16, T22 | T16: T16/live-streams.md<br>T22: T22/recovery-matrix.md |
| `REQ-SEC-001` | T05, T18 | T05: T05/prime-kernel.md<br>T18: T18/security-hardening.md |
| `REQ-SEC-002` | T09, T18, T23 | T09: T09/operation-gateway.md<br>T18: T18/security-hardening.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-SEC-003` | T02, T18 | T02: T02/config-preflight.md<br>T18: T18/security-hardening.md |
| `REQ-SEC-004` | T15, T18 | T15: T15/plugins.md<br>T18: T18/security-hardening.md |
| `REQ-SEC-005` | T13, T18 | T13: T13/embedded-terminal.md<br>T18: T18/security-hardening.md |
| `REQ-STATE-001` | T01 | T01: T01/canonical-boundary.md |
| `REQ-STATE-002` | T06 | T06: T06/sqlite-projection-store.md |
| `REQ-STATE-003` | T06 | T06: T06/sqlite-projection-store.md |
| `REQ-STATE-004` | T19 | T19: T19/headless-equivalence.md |
| `REQ-STATE-005` | T01 | T01: T01/canonical-boundary.md |
| `REQ-SXR-001` | T14B | T14B: T14B/sxr-evidence.md |
| `REQ-SXR-002` | T14B, T23 | T14B: T14B/sxr-evidence.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-SXR-003` | T14B, T18 | T14B: T14B/sxr-evidence.md<br>T18: T18/security-hardening.md |
| `REQ-SXR-004` | T14B, T20, T22, T25 | T14B: T14B/sxr-evidence.md<br>T20: T20/release-qualification.md<br>T22: T22/recovery-matrix.md<br>T25: MISSING |
| `REQ-SYNC-001` | T01, T06, T16, T22 | T01: T01/canonical-boundary.md<br>T06: T06/sqlite-projection-store.md<br>T16: T16/live-streams.md<br>T22: T22/recovery-matrix.md |
| `REQ-SYNC-002` | T06, T16, T21 | T06: T06/sqlite-projection-store.md<br>T16: T16/live-streams.md<br>T21: T21/payment-benchmark.md |
| `REQ-SYNC-003` | T06, T11, T14B, T22 | T06: T06/sqlite-projection-store.md<br>T11: T11/kernel-affordances.md<br>T14B: T14B/sxr-evidence.md<br>T22: T22/recovery-matrix.md |
| `REQ-TUI-001` | T07, T12 | T07: T07/workbench-model.md<br>T12: T12/semantic-shell.md |
| `REQ-TUI-002` | T12 | T12: T12/semantic-shell.md |
| `REQ-TUI-003` | T12 | T12: T12/semantic-shell.md |
| `REQ-TUI-004` | T12 | T12: T12/semantic-shell.md |
| `REQ-TUI-005` | T19 | T19: T19/headless-equivalence.md |
| `REQ-UX-001` | T12, T25 | T12: T12/semantic-shell.md<br>T25: MISSING |
| `REQ-UX-002` | T08, T12, T25 | T08: T08/focus-horizon.md<br>T12: T12/semantic-shell.md<br>T25: MISSING |
| `REQ-UX-003` | T09, T12, T23 | T09: T09/operation-gateway.md<br>T12: T12/semantic-shell.md<br>T23: T23/cross-surface-isomorphism.md |
| `REQ-UX-004` | T05, T12, T16 | T05: T05/prime-kernel.md<br>T12: T12/semantic-shell.md<br>T16: T16/live-streams.md |
| `REQ-UX-005` | T12, T13, T20 | T12: T12/semantic-shell.md<br>T13: T13/embedded-terminal.md<br>T20: T20/release-qualification.md |
| `REQ-UX-006` | T02, T12, T13, T18 | T02: T02/config-preflight.md<br>T12: T12/semantic-shell.md<br>T13: T13/embedded-terminal.md<br>T18: T18/security-hardening.md |
| `REQ-UX-007` | T00A, T12, T20, T21, T25 | T00A: T00A/settlement-record.md<br>T12: T12/semantic-shell.md<br>T20: T20/release-qualification.md<br>T21: T21/payment-benchmark.md<br>T25: MISSING |
| `REQ-VERIFY-001` | T06, T22, T25 | T06: T06/sqlite-projection-store.md<br>T22: T22/recovery-matrix.md<br>T25: MISSING |
| `REQ-VERIFY-002` | T19, T22, T25 | T19: T19/headless-equivalence.md<br>T22: T22/recovery-matrix.md<br>T25: MISSING |
| `REQ-VERIFY-003` | T09, T23, T25 | T09: T09/operation-gateway.md<br>T23: T23/cross-surface-isomorphism.md<br>T25: MISSING |
| `REQ-VERIFY-004` | T09, T23, T25 | T09: T09/operation-gateway.md<br>T23: T23/cross-surface-isomorphism.md<br>T25: MISSING |
| `REQ-VERIFY-005` | T16, T22, T24, T25 | T16: T16/live-streams.md<br>T22: T22/recovery-matrix.md<br>T24: T24/prime-fresh-target.md<br>T25: MISSING |
| `REQ-VERIFY-006` | T16, T22, T25 | T16: T16/live-streams.md<br>T22: T22/recovery-matrix.md<br>T25: MISSING |

### 2.2 Design-clause coverage (15 DSNs)

| DSN | Clause (DESIGN.md summary) | Owners | Settled evidence |
| :--- | :--- | :--- | :--- |
| DSN-001 | Semantic theme tokens; raw colour literals only in token adapters | T12 | T12/semantic-shell.md |
| DSN-002 | Consequential state legible under colour degradation (truecolor/256/mono) | T12, T20 | T12/semantic-shell.md, T20/release-qualification.md |
| DSN-003 | `brand.gold` only for proof/confirmation/authority/decision/authoritative route | T12 | T12/semantic-shell.md |
| DSN-004 | Authority/execution/evidence/settlement/readiness/integrity/capability stay distinct token families and labels | T12 | T12/semantic-shell.md |
| DSN-005 | No component-local authority; affordance resolver owns existence/enablement | T09, T12 | T09/operation-gateway.md, T12/semantic-shell.md |
| DSN-006 | Every Focus occurrence is a placement of one FocusRef; no forking | T08, T12 | T08/focus-horizon.md, T12/semantic-shell.md |
| DSN-007 | Geometry floors: 120x40 full, 80x24 one primary pane, recoverable prompt below | T12, T20 | T12/semantic-shell.md, T20/release-qualification.md |
| DSN-008 | Evidence/authority render as inspectable record objects; supersession never erases | T12 | T12/semantic-shell.md |
| DSN-009 | Blocked affordances materialize with gate + next step; impossible actions discoverable | T09, T12 | T09/operation-gateway.md, T12/semantic-shell.md |
| DSN-010 | Chat is one View; Trace projects execution history, never takes Chat as input | T07, T12 | T07/workbench-model.md, T12/semantic-shell.md |
| DSN-011 | No canonical truth in components; any View renderable by another frontend | T07, T19 | T07/workbench-model.md, T19/headless-equivalence.md |
| DSN-012 | Plugin surfaces register/unregister cleanly; unload returns to baseline | T15 | T15/plugins.md |
| DSN-013 | Reduced-motion mode; motion suppression leaves interface understandable | T12 | T12/semantic-shell.md |
| DSN-014 | Untrusted text renders as text; control sequences never escape the deliberate PTY viewport | T13, T18 | T13/embedded-terminal.md, T18/security-hardening.md |
| DSN-015 | No roadmap surface built (no Harness View, no rich semantic overlay, no second production renderer) | T25 | T25/whole-system-conformance.md (this document; checks delegated to renderer-boundary + opentui-shell registry/shell scans, see §2) |

### 2.3 T00/T00A note

Covered by §1.3: T00 and T00A evidence dirs now carry their quoted settlement records (append-only).

## 3. Final proof matrix — gate results (verbatim)

All runs on the final tree unless marked. `<wb> = cd workbench`.

| # | Gate / command | Result (verbatim essentials) |
| :--- | :--- | :--- |
| 1 | `just prereg-check gauntlet-tui-prime-workbench-T25` | `prereg-check.sh: PASS — task gauntlet-tui-prime-workbench-T25 preregistration matches its frozen hash (74e86008f0b76275b7f1ee64aed7bbca9a944992b63214fe830e61f116d66f87)` |
| 2 | `just tui-test all` | exit 0 — `registry: 49 normative suites.` … `suites: 49, tests: 529 pass / 0 fail / 9 skip.` … `tui-test all: PASS — every registered normative suite ran green.` (full per-suite table in §5) |
| 3 | `just tui-bench all-required` (runs all 7 required selectors) | `tui-bench all-required: PASS — all 7 required benchmarks exited 0`. Individual verdicts: bun-substrate `Substrate Meets All Performance Floors` (bun:sqlite insert 10k rows 9.10ms / 1,099,479 ops/s); sqlite-replay `Replay 10000 events: 136.95ms (73,018 events/sec)`; focus-latency `ALL BUDGETS PASS`; projection-worker `ALL BUDGETS PASS`; terminal-path complete (strip 8 MiB median 83.65ms; projection p50 0.260ms; real-child round trip p50 3.05ms); large-case `ALL BUDGETS PASS` (HC1 cold start p99 2352ms <= 60000ms; host facts matched: 6 cores/7.8 GiB/Bun 1.4.0); long-horizon `ALL SHAPE CHECKS PASS` (hot set flat medium→large at subscriptions 2/60 <= 128 while eager whole-world grows 11800→28600 and violates the same ceiling at every size) |
| 4 | `just tui-preflight bootstrap` | exit 0 — `tui-preflight.sh: bootstrap: PASS` (skeleton, exact pins, lockfile sync, `bun audit` clean, spec + DESIGN hashes match) |
| 5 | `just tui-preflight conformance` | exit 0 — `tui-preflight.sh: conformance: PASS — every declared runtime capability is executed-and-passing, or visibly GATED/UNQUALIFIED with its settled evidence record; no REQUIRED integration is silently missing` (rows enumerated in §1.2) |
| 6 | `just tui-check` | exit 0 — `tsc --noEmit -p tsconfig.json` clean |
| 7 | `just tui-lint` | exit 0 — `Checked 117 files … Found 78 warnings. Found 533 infos.` (warn/infos only; noConsole on the runner/benches, same class as the pre-existing baseline) |
| 8 | `just tui-build release` | exit 0 — `tui-build release: OK — dist/gauntlet-workbench sha256=59f282229799a679… launch 120x40 exit 0, manifest dist/release-manifest.json`. Rebuilt twice consecutively: byte-identical `59f282229799a679…` (determinism re-proven on the current tree). Digest differs from T20's recorded `ec110d78…` because the release entry's import graph legitimately changed with settled T21 COR-1/COR-2 model-layer corrections; the manifest was regenerated by the build and `verifyReleaseManifest` passes |
| 9 | `<wb> bun test` (full tree, default env) | exit 0 — `605 pass / 12 skip / 0 fail`, `7509 expect() calls`, `Ran 617 tests across 58 files` — includes the new coverage-recomputation suite (12 tests, green now that this document exists); the 12 skips are the adjudicated gated/recorded capabilities of §1.2 |
| 10 | `GAUNTLET_T13_REAL_PTY=1 just tui-test embedded-terminal` | exit 0 — `15 pass / 0 fail` (real PTY spawn/write/resize/cancel battery) |
| 11 | `GAUNTLET_T20_REAL_PTY=1 just tui-test operator-journeys-pty` | exit 0 — `3 pass / 0 fail` (real PTY 120x40 / 80x24 / below-minimum keyboard journeys) |
| 12 | `GAUNTLET_T20_REAL_PTY=1 <wb> bun test` (full tree) | pre-evidence run: `606 pass / 9 skip / 2 fail` — the only 2 failures were this task's own coverage suite awaiting THIS document (DSN-015/T25 self-reference), diagnosed and expected; final re-run: exit 0 — `608 pass / 9 skip / 0 fail`. Zero pre-existing tests regressed under the gate |
| 13 | `GAUNTLET_T24_REAL_PRIME=1 just tui-test prime-fresh-target` | exit 0 — `11 pass / 0 fail` in 164.05s (real prime-agent 0.8.0: child admission, same-kernel serialization, misleading-completion liveness, TUI SIGKILL survival, cache-deleted reattachment, cross-process resume, schedule add round-trip) |
| 14 | `just verify` (Rust closure: check, test, lint, deny, boundaries, ruler) | exit 0 — `Summary [155.963s] 1710 tests run: 1710 passed (2 slow), 0 skipped`; surfaces selection `17 tests run: 17 passed`; boundaries `261 tests run: 261 passed, 0 skipped`; clippy/fmt/deny clean (chain exited 0) |
| 15 | `just ruler` | exit 0 — `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)` |

### 3.1 Final confirmation pass (re-run on the final tree after this document landed)

Prereg transfer L1 (replay): after the last correction, the full matrix re-ran on the final tree with stable results — `just tui-test all` exit 0 (49 suites, 529 pass / 0 fail / 9 skip; per-suite table in §5); `just tui-preflight bootstrap` exit 0; `just tui-preflight conformance` exit 0; `just tui-check` exit 0; `just tui-lint` exit 0 (78 warnings, warn-level only); `bun test` exit 0 — `605 pass / 12 skip / 0 fail` across `617 tests across 58 files`; `GAUNTLET_T20_REAL_PTY=1 bun test` exit 0 — `608 pass / 9 skip / 0 fail` (the row-12 pre-evidence failures cleared by this document's existence, exactly as diagnosed); `just ruler` exit 0 (verbatim above).

## 4. Final E2E Case composition (prereg D2) — referenced, not re-run

The plan's final case is constituted by the settled evidence below; T25 re-executed the gated legs it owns (#10–#13) and references the rest:

| D2 element | Settled proof (task, evidence) | Re-run by T25 |
| :--- | :--- | :--- |
| At least one real Prime child | T24 real battery — real `-p goal+autonomous` child admission (`real-child-admission.json`); T10 recursion + T24 fresh-target | Yes (#13, 11/0) |
| One kernel computation | T05 `prime-kernel` serialization + T24 same-kernel serialization (turns never interleave) | Yes (#2 suite, #13 real) |
| One evidence-producing operation | T11 evidence-promotion (kernel artifact promotion) + T14B real SXR evidence lifecycle (13-record sxr 0.1.0 capture) | Yes (#2 suites) |
| Attention/composer interaction | T12 composer-takeover (queued attention, stale-answer purge on admission, keyboard safety) | Yes (#2 suite) |
| One disconnect/reconnect | T16 detach-reattach/stale-frame/reconnect-races; T22 matrix real SIGKILL under load; T24 TUI SIGKILL survival + cache-deleted reattachment from authoritative records | Yes (#2 suites, #13 real) |
| Final Gauntlet consequence | T22 recovery matrix (7 cases, T19 digest referee, failed rounds preserved); T23 8-route isomorphism settlement (two-host idempotency race = 1 settlement); T09 no-bypass gateway; workbench never canonical (canonical-boundary, renderer-boundary, DSN-015 scans) | Yes (#2 suites) |
| VAR-001..006 / RECOV-001..006 applicable cases | T22 evidence (all 7 cases pass incl. real SIGKILL under concurrent load); T20 transport equivalence (8 pass); T19 replay determinism across processes | Via #2/#12 |

### 4.1 The three final teeth (prereg D3) — pinned by settled suites, cited

1. **Delete Bun SQLite + restart before replaying the final recorded Case** → equivalent consequential state reconstructs from authoritative sources. Pinned: `cold-rebuild.test.ts` (T22 teeth/falsifiers 1–2; cursor/epoch validation, quarantine rebuild digest-equal under the T19 referee) + `recovery-matrix.test.ts` (7 cases incl. real SIGKILL under concurrent load) + `headless-replay.test.ts` deletion-order attack (c): "deleting the SQLite file loses nothing". Evidence: T22/recovery-matrix.md, T19/headless-equivalence.md.
2. **Select a denied operation through the most permissive-looking projection while Prime children and plugins are active** → denial holds, no side effect. Pinned: `cross-surface-equivalence.test.ts` drives the DENIED representative (post-render authority revocation) through ALL routes of the 8-route registry — `gateway-http/shim`, `direct-api/cli`, `kernel-callable`, `plugin-projection`, `prime-delegation`, `shell-effect` — with one typed denial and zero side effects, byte-identical shapes, plus racing-denial and least-obvious-path cases; `deny-bypass.test.ts` (T09/T14B/T18/T23) deep-pins the gateway family with a negative control. Evidence: T23/cross-surface-isomorphism.md, T09/operation-gateway.md.
3. **Final Case without DomainForge enrichment, then with a real bound SEA/DomainForge model** → core semantics correct in both; enrichment adds source-identified relations without identity substitution. Pinned: `domainforge-adapter.test.ts` (absent/present/invalid-producer modes; fail-closed on invalid carrier) + `semantic-evidence-composition.test.ts` (real producer/consumer/workbench composition; "both lenses compose beside each other with DISTINCT source identities (no identity collapse)"; real CAS content_ref carriage through the pinned binary when its digest matches). Evidence: T14/domainforge-adapter.md, T14B/sxr-evidence.md. No gap remained, so no new integrative test was added.

## 5. `just tui-test all` per-suite table (final confirmation pass)

```text
=== tui-test all: per-suite conformance table ===
RESULT      PASS   FAIL   SKIP  EXIT SUITE
PASS          11      0      0     0 src/affordance-resolver.test.ts  [T09]
PASS           7      0      0     0 src/bun-native-smoke.test.ts  [T03]
PASS           4      0      0     0 src/canonical-boundary.test.ts  [T01]
PASS           3      0      0     0 src/cold-rebuild.test.ts  [T22]
PASS          11      0      0     0 src/composer-takeover.test.ts  [T12]
PASS           7      0      0     0 src/config-preflight.test.ts  [T02]
PASS          13      0      0     0 src/cross-surface-equivalence.test.ts  [T23]
PASS          16      0      0     0 src/deny-bypass.test.ts  [T09/T14B/T18/T23]
PASS           3      0      0     0 src/detach-reattach.test.ts  [T16/T22/T24]
PASS          26      0      0     0 src/domainforge-adapter.test.ts  [T14]
PASS          14      0      1     0 src/embedded-terminal.test.ts  [T13]
PASS           6      0      0     0 src/evidence-promotion.test.ts  [T11]
PASS           6      0      0     0 src/focus-identity.test.ts  [T08]
PASS          16      0      0     0 src/headless-replay.test.ts  [T19]
PASS           6      0      0     0 src/horizon-hoist.test.ts  [T08]
PASS           8      0      0     0 src/horizon-materialization.test.ts  [T08/T17]
PASS          12      0      0     0 src/kernel-affordances.test.ts  [T11]
PASS           7      0      0     0 src/keyboard-accessibility.test.ts  [T12]
PASS          12      0      0     0 src/opentui-shell.test.ts  [T12]
PASS           6      0      0     0 src/operation-isomorphism.test.ts  [T09]
PASS          10      0      0     0 src/operator-journeys.test.ts  [T12/T20]
PASS          11      0      0     0 src/operator-preview.test.ts  [T00A]
PASS          17      0      0     0 src/plugin-lifecycle.test.ts  [T15]
PASS          14      0      0     0 src/plugin-trust.test.ts  [T15]
PASS          10      0      0     0 src/prime-context-envelope.test.ts  [T10]
PASS           6      0      7     0 src/prime-fresh-target.test.ts  [T24]
PASS           2      0      0     0 src/prime-host.test.ts  [T04]
PASS*          5      0      0     0 src/prime-kernel.test.ts  [T05/T24]
PASS           8      0      0     0 src/prime-long-running.test.ts  [T10]
PASS          12      0      0     0 src/prime-recursion.test.ts  [T10]
PASS           1      0      0     0 src/prime-topology.test.ts  [T04]
PASS           2      0      0     0 src/projection-fanout.test.ts  [T07]
PASS           2      0      0     0 src/projection-generation.test.ts  [T06]
PASS           1      0      0     0 src/projection-rebuild.test.ts  [T06]
PASS           4      0      0     0 src/reconnect-races.test.ts  [T16]
PASS          20      0      0     0 src/recovery-matrix.test.ts  [T22]
PASS           9      0      0     0 src/renderer-boundary.test.ts  [T19]
PASS          15      0      0     0 src/secret-leakage.test.ts  [T18]
PASS          26      0      0     0 src/security-boundaries.test.ts  [T18]
PASS          10      0      0     0 src/semantic-evidence-composition.test.ts  [T14B/T20]
PASS           6      0      0     0 src/semantic-identity.test.ts  [T14]
PASS           7      0      0     0 src/semantic-protocol-compatibility.test.ts  [T14]
PASS           5      0      1     0 src/ssh-target.test.ts  [T20]
PASS           5      0      0     0 src/stale-frame.test.ts  [T16]
PASS          57      0      0     0 src/sxr-evidence-adapter.test.ts  [T14B]
PASS           8      0      0     0 src/transport-equivalence.test.ts  [T20]
PASS          41      0      0     0 src/unsafe-input-boundaries.test.ts  [T13]
PASS          19      0      0     0 src/work-chat-trace.test.ts  [T12]
PASS           2      0      0     0 src/workbench-model.test.ts  [T07]
```

suites: 49, tests: 529 pass / 0 fail / 9 skip. `PASS*` = suite reached through a recorded plan-selector adjudication (kernel-serialization → prime-kernel).
Skips are gated/recorded capabilities and stay visible by design; their named, evidence-backed adjudication is `just tui-preflight conformance`.

```text
tui-test all: PASS — every registered normative suite ran green.
```

## 6. Support matrix

| Dimension | Qualified on this target | Recorded gap |
| :--- | :--- | :--- |
| Host | Linux WSL2 Ubuntu-26.04 x86_64, 6 cores / 7.8 GiB, Bun 1.4.0 | (machine facts; not a portability claim) |
| Runtime substrate | Bun >= 1.4 (sqlite, PTY, workers, compile); python3 present; prime-agent 0.8.0 build 8d7deea-dirty (T24 inventory) | Substrate drift FAILS preflight E5 with requalification instruction |
| Renderer | OpenTUI/React 0.5.11 / 19.2.8 pinned and importable; release artifact renders the structural ShellSession tree | Embedded OpenTUI React viewport in the release entry is recorded later-task wiring (T13 deferral) |
| Terminals | Local PTY (real, gated + executed); tmux 3.6 legs real | real-sshd SSH transport leg UNQUALIFIED (no sshd on this host; executes unchanged via `src/ssh-target.test.ts` once reachable) |
| Semantics | DomainForge/SEA pinned-parser adapter; real sxr 0.1.0 binary legs digest-gated (currently matching: 57 pass) | `role_binding` declaration fixture not real (structural projection only) |
| Remote/Prime quirks | send --steer/--follow-up `documented_but_refused`; schedule list/cancel non-default socket — recorded, fail-visible | T24 trigger-records |
| Release form | bun-compile standalone bun-linux-x64, ~83 MB, launches in real PTY 120x40, rebuild-deterministic | No frozen size budget (recorded fact) |

## 7. Unsettled / recorded items (consolidated trigger-records; none re-proven here)

**Discharged during the plan (for the record):** T10 MAJOR substrate-inventory staleness → discharged by T24 requalification; T14 MAJOR both-mode carrier → discharged by T14B requalification; T17 planner-swap production wiring → discharged at T20; T14B Inspector lens wiring → discharged at T19/T20.

**Open recorded minors/trigger-records (owned boundaries unchanged):**

* T08: governanceScope whole-Case fallback for non-WorkItem roots; unescaped `:` id grammar if source/kind become free text.
* T09: accepted-record re-drive shape (anchored vs unanchored); unanchored committed-truth audit-UX; observe head-of-line bounded stall; non-crypto 64-bit payload digest; compromised-transport forged-accepted exposure via ledger + fresh observation; composition-level deadline for T13's perform-hang stall.
* T11: `transitionTo` latent ordering hazard (dead defensive branch); `unknownOrStaleOffer` reason-fidelity nit.
* T12: DSN-004 latent label collisions in unused vocabulary (named T20; not visibly addressed in T20 evidence — carried); RenderedRecord.kind taxonomy stretch at next registry change.
* T13: non-idempotent cancel reason overwrite on real PTYs; 128+signum exitCode provenance fidelity; untyped TypeError on garbage admission shapes (fail-closed records).
* T16: degradedReasons transition-only staleness; nested-reconnect handshake desync (self-healing); forged-but-plausible persisted cursors validate `valid` (reporting level); orphaned buffered frames bounded/inert.
* T17: readmission-measurement bookkeeping; single-sample RSS; B3 endpoint-vs-per-cycle wording; REQ-GOAL-006 current-actions residual (re-measured by T21).
* T18: `redactDeep` Map/Set handling; real-PTY echoed paste.
* T19: `normalize*` accept any WorkbenchSnapshot (convention, not type enforcement) — narrower parameter type at next touch.
* T20: presence-only lens render gate (typed guard before trust boundary); planner `refreshInFlight` clearing micro-waste; bracketed IPv6 classifies `remote` (T02 support fact).
* T21: `Object.freeze(view)`-throws + `isFrozen`-false divergences (disclosed, zero observers) — the stale bench-header part of this trigger-record was T25's to sweep and is DONE (§ header note in `long-horizon.bench.ts`).
* T22: retention-expired cursor false-positive direction; quarantine-evidence hostile repair.
* T23: textual implementor pin (structural pins cover); prereg frozen_at wall-clock cosmetic.
* T24: `--steer/--follow-up` documented-but-refused (drift captured); schedule list/cancel socket binding; RLM RuntimeError loop quirk; attach surface-probed; dual session-identity spaces; load-sensitivity.
* T14B: deadline-kill vs grandchild pipe holders; offer_id-keyed admission (cross-unit dedupe is canonical-API-owned); relations.ts `valid:false` filter (T14-carried).
* T25 (new, this task): the `kernel-object-lifecycle` selector adjudication (§1.1) — recorded here first; T00/T00A settlement-record consolidations (§1.3); release-artifact digest movement vs T20 explained in §3 row 8.

**Roadmap (explicitly NOT proven, seams only):** production libvaxis fallback; rich cross-model semantic overlays; second production renderer / GUI / mobile; harness-metabolization UI; REQ-UX-007 live-endpoint breadth in the release entry; real-sshd transport on this host.

## 8. Rollback / recovery instructions

* **Repository gates red after a future change:** `just verify` (Rust) and the four `just tui-*` gates are independent; fix the failing layer, never weaken a gate. `just tui-test all` exit 2 means the plan registry broke (selector unresolvable) — reconcile the plan with `ADJUDICATED_SUITE_ALIASES`, never edit the frozen plan.
* **Release artifact:** regenerate with `just tui-build release` (writes `dist/release-manifest.json`, launches the artifact in a real PTY 120x40). `tui-preflight conformance` E6 fails if the artifact and manifest disagree — re-run the build, do not hand-edit the manifest.
* **Substrate drift:** if `prime-agent --version` != 0.8.0, preflight conformance FAILS by design; the recovery is the T24-shaped requalification (re-run `GAUNTLET_T24_REAL_PRIME=1 just tui-test prime-fresh-target` plus `substrate-inventory` re-observation), not a gate edit.
* **Projection loss (the product's own recovery):** deleting Bun SQLite / projection cache is safe by construction — `cold-rebuild.test.ts` + `recovery-matrix.test.ts` prove reconstruction from authoritative sources with the T19 digest referee. Quarantined rows are detected via cursor/epoch validation and never trusted.
* **Gated batteries:** `GAUNTLET_T13_REAL_PTY=1`, `GAUNTLET_T20_REAL_PTY=1`, `GAUNTLET_T24_REAL_PRIME=1` enable the real batteries; each is evidence-backed in §1.2 and each was re-executed green in §3 (#10–#13).
* **Coverage checker:** if `coverage-recomputation.test.ts` fails, the plan/evidence changed — find the named REQ/DSN in the failure list and restore its settled evidence; the negative-control tests distinguish checker rot from real gaps.

## 9. Handoff (per plan handoff_contract)

* **Settled capabilities/requirements:** 104/104 REQs, 15/15 DSN clauses with settled evidence (§2); both long-NOT_IMPLEMENTED modes real (§1); the operator journey — enter/resume a Case, focus/hoist with progressive materialization, inspect the locally coherent world, invoke/delegate through isomorphic surfaces over one canonical admission path, observe Prime/Python execution live, detach/reattach, reconstruct after cold loss, plugin and SXR composition, release form on real hardware — each settled by its owning task and re-observed here.
* **Explicitly unsettled/deferred:** §7 (recorded trigger-records + roadmap seams). None of these silently passed a gate; the conformance preflight names them with their evidence.
* **Current operational system state:** all gates green on the final tree (§3); workbench 617 tests across 58 files (default env), 49 normative suites green under `tui-test all`; Rust closure 1710+17+261 green; release artifact builds deterministically and launches in a real PTY.
* **Requirement-to-evidence index:** §2.1/§2.2 (mechanically recomputed by `coverage-recomputation.test.ts` on every run).
* **Known limitations:** §6 support-matrix right column; §1.1 skip accounting (9 skips, all adjudicated).
* **Rollback/recovery:** §8.
* **Important decisions/discoveries that changed the execution model:** the selector-divergence adjudications (§1.1); the append-only T00/T00A settlement consolidations (§1.3); the release digest movement explanation (§3 row 8); the unbacked-gate sweep added to the conformance mode (structural prevention of future silently-gated skips).
* **Exact next executable action:** dispatch the fresh independent adversarial verifier with the frozen requirements + comparison contract + this evidence bundle (builder reasoning/history excluded) to answer the bounded confirmation question and issue `CONFIRM_BOUNDED_CONFORMANCE` or a revision verdict. Only after that verdict and the orchestrator's own total-evidence review may the plan record `none — plan settled`.

Confirmation: BUILDER evidence pending independent adversarial confirmation.

## Settlement findings (independent adversarial verification, orchestrator-recorded)

VERDICT: CONFIRM_BOUNDED_CONFORMANCE (2026-09-09, fresh independent adversarial verifier; 0 BLOCKING / 0 MAJOR). The verifier independently re-ran all ten declared gates, traced 15 REQs + 4 DSNs to load-bearing test assertions beyond the coverage suite, re-executed two of the three final teeth with its own probes (SQLite-deletion digest replay; denied-op through a live plugin projection with a running Prime child), verified the release build byte-identical, and confirmed DSN-015. Two MINORs recorded here as standing trigger-records:

1. **Unbacked-gate sweep idiom scope** (defense-in-depth): the conformance sweep matches only the literal `GAUNTLET_T…"] === "1"` idiom; a functionally identical gate written `!== "1"` would evade it. No current gate uses another idiom (verified by the verifier's grep and re-verified at settlement). Follow-up: widen the sweep to any `GAUNTLET_T<n>_*` literal appearing in a comparison; soften the script header's "structural prevention" framing to name the idiom scope. Owning scope: next touch of scripts/tui-preflight.sh.
2. **Presentation lag in this document's §2.1**: T25-owned index rows read "T25: MISSING" — an accurate snapshot of the disclosed RED-first state, superseded in the same document once this evidence file existed. The live mechanical coverage suite is authoritative (104/104 + 15/15, negative controls green).

Settlement recorded by the orchestrator after total-evidence review: every task T00-T25 carries settled evidence with its declared confirmation mode and level (ladder history preserved: T09 two adversarial rounds, T15/T16/T21 one each, plus peer/adversarial corrections at T08/T12/T13/T14/T22/T23/T24 — all RED-first, all focused-reconfirmed); coverage 104/104 REQs + 15/15 DSNs mechanically recomputed and independently traced; all global gates green on the final tree in the orchestrator's hands; failed rounds and trigger-records preserved throughout; no roadmap surface built.
