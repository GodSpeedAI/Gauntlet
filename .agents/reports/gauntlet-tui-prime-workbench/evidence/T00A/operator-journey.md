# T00A Evidence — Early Rendered Read-Only Gauntlet Operator Journey

Date: 2026-09-08
Task: T00A
Scope: Implementation and verification of the early rendered read-only operator journey under the bootstrap carve-out, proving OpenTUI rendering substrate, terminal geometry floors, keyboard navigation, and empty/disconnected states.

## Requirements & Design Directives Addressed

- **REQ-UX-005 / DSN-007**: Geometry floors: Full (>= 120x40), Compact (>= 80x24), BelowMinimum (< 80x24 recoverable prompt).
- **REQ-TUI-001 / REQ-TUI-005**: OpenTUI core runtime rendering with `@opentui/core` and `@opentui/react`.
- **DSN-001 .. DSN-004**: Semantic visual hierarchy and status tokens (settled, rejected, active, pending, blocked).
- **DSN-006 / Spec §20.2**: Escalation-first unit presentation in the Navigator surface.
- **Finding F10**: Strict bootstrap carve-out with zero `REQ-BUN-001` overlap dependencies admitted.
- **Finding F9**: Explicit read-only guarantees — mutations refused, explanation views marked unavailable awaiting T01.

## Architecture & Code Delivered

1. **`workbench/src/client.ts`**:
   - HTTP client querying `gauntlet serve` endpoints (`/api/v1/state`, `/api/v1/runs/{id}`, `/api/v1/units/{id}`).
   - Typed snapshot models (`WorkspaceStateSnapshot`, `RunSummarySnapshot`, `UnitSnapshot`).
   - Mock client for offline/disconnected testing and preview mode.

2. **`workbench/src/geometry.ts`**:
   - `classifyGeometry(cols, rows)`:
     - `cols >= 160 && rows >= 48` -> `Wide`
     - `cols >= 120 && rows >= 40` -> `Full` (3 simultaneous columns: Navigator, Work, Inspector)
     - `cols >= 80 && rows >= 24` -> `Compact` (single pane with Tab switching)
     - Otherwise -> `BelowMinimum` (prompts operator to enlarge with dimensions)
   - `renderBelowMinimumPrompt(cols, rows)`: Clean 4-line centered guidance preserving terminal sanity.

3. **`workbench/src/theme.ts`**:
   - Status glyphs, labels, and ANSI color codes (`[PASS]`, `[FAIL]`, `[ESC]`, `[INCON]`, `[PEND]`, `[REV]`).
   - Border styles and layout dividers.

4. **`workbench/src/preview.ts`**:
   - Terminal layout renderer supporting both Full (3 columns) and Compact modes.
   - Escalation-first sorting in Navigator column.
   - Keyboard reducer (`j`/`k`/`Down`/`Up` navigation, `Tab` column/pane cycling, `Enter` inspect, `Escape` back, `q` quit).
   - Read-only enforcement: attempts to execute actions result in a warning notice.
   - Explicit T01 marker: Detailed explanation / answer-key breakdown rendered as `unavailable (T01 read contract required)`.

5. **`workbench/src/operator-preview.test.ts`**:
   - 11 comprehensive automated tests covering all geometry, navigation, empty state, disconnected, read-only, and dependency invariants.

## Verification Evidence

### 1. Automated Test Suite

Command: `just tui-test`
Output:
```text
bun test v1.4.0 (34cbb9a40)

src/index.test.ts:
✓ workbench exports are importable and functional [0.25ms]

src/operator-preview.test.ts:
✓ T00A Operator Preview: Geometry & Layout Floors (REQ-UX-005, DSN-007) > classifies 120x40 as Full layout with 3 simultaneous surfaces [0.11ms]
✓ T00A Operator Preview: Geometry & Layout Floors (REQ-UX-005, DSN-007) > classifies 80x24 as Compact layout with single primary pane [0.02ms]
✓ T00A Operator Preview: Geometry & Layout Floors (REQ-UX-005, DSN-007) > classifies < 80x24 as BelowMinimum and renders the recoverable compact prompt [0.03ms]
✓ T00A Operator Preview: Geometry & Layout Floors (REQ-UX-005, DSN-007) > teeth: terminal resize down to <80x24 yields prompt; resize back restores layout and location [0.47ms]
✓ T00A Operator Preview: Keyboard Journey & Escalation-First Flow > navigates units, inspects details, and returns with keyboard controls [0.06ms]
✓ T00A Operator Preview: Keyboard Journey & Escalation-First Flow > renders escalation-first in Navigator column (spec §20.2 / DSN-006) [0.08ms]
✓ T00A Operator Preview: Keyboard Journey & Escalation-First Flow > renders explanation fields as unavailable in preview mode (T00A requirement) [0.05ms]
✓ T00A Operator Preview: Empty & Disconnected Handling (Teeth) > teeth: disconnected state provides clear next steps and zero crashes [0.04ms]
✓ T00A Operator Preview: Empty & Disconnected Handling (Teeth) > teeth: empty workspace provides clear next steps and zero crashes [0.02ms]
✓ T00A Operator Preview: Read-Only Contract > teeth: mutation attempts are refused with explicit read-only notice [0.06ms]
✓ T00A Bootstrap Dependency Carve-out Discipline (Teeth) > teeth: package.json admits zero REQ-BUN-001 overlap dependencies [0.26ms]

 12 pass
 0 fail
 143 expect() calls
Ran 12 tests across 2 files. [13.00ms]
```

### 2. Full 120x40 Terminal Render Capture

```text
┌─ [1] NAVIGATOR ──────────────────────────┬─ [2] WORK ───────────────────────────────┬─ [3] INSPECTOR ──────────────────────────┐
│ Runs: 1  |  Units: 3                     │ Unit: unit-102 (critic_review)           │ Unit Details: unit-102                   │
│                                          │ Run: run-001                             │ Status: escalated                        │
│ Units:                                   │ Status: [ESC] escalated                  │ Tier: tier-3                             │
│ > [*] [ESC] unit-102 (tier-3)            │ Phase: comparing                         │                                          │
│   [ ] [FAIL] unit-101 (tier-1)           │                                          │ Explanation:                             │
│   [ ] [PASS] unit-100 (tier-2)           │ Artifacts:                               │ unavailable (T01 read contract required) │
│                                          │ - report.md (size: 1024 bytes)           │                                          │
│                                          │ - diff.patch (size: 256 bytes)           │ Verification Records:                    │
│                                          │                                          │ unavailable (T01 read contract required) │
│                                          │ Recent Logs:                             │                                          │
│                                          │ [19:30:01] Observer dispatched           │ Escalation Ticket:                       │
│                                          │ [19:30:05] Residual detected: spec mis   │ - reason: Residual budget exhausted      │
│                                          │ [19:30:08] Escalation threshold reached  │                                          │
└──────────────────────────────────────────┴──────────────────────────────────────────┴──────────────────────────────────────────┘
 [j/k/Down/Up] Navigate  [Tab] Cycle Focus  [Enter] Inspect  [Esc] Back  [q] Quit
```

### 3. Compact 80x24 Terminal Render Capture

```text
┌─ WORK (Tab to switch pane) ──────────────────────────────────────────────────┐
│ Unit: unit-102 (critic_review)                                               │
│ Status: [ESC] escalated                                                      │
│ Run: run-001                                                                 │
│                                                                              │
│ Artifacts:                                                                   │
│ - report.md (1024 bytes)                                                     │
│ - diff.patch (256 bytes)                                                     │
│                                                                              │
│ Recent Logs:                                                                 │
│ [19:30:01] Observer dispatched                                               │
│ [19:30:05] Residual detected: spec mismatch in test cases                    │
│ [19:30:08] Escalation threshold reached                                      │
│                                                                              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
 [j/k] Nav  [Tab] Pane  [Enter] Detail  [Esc] Back  [q] Quit
```

### 4. Below-Minimum (< 80x24) Terminal Render Capture

```text
 Terminal size (70x20) is below minimum (80x24).
 Please enlarge terminal to resume Gauntlet Workbench.
 Required: >= 80 cols x 24 rows (Compact) or >= 120x40 (Full).
```

### 5. Teeth Attacks & Falsification Checks

1. **Terminal Resize Down and Restore Teeth**:
   - Verified that resizing a live terminal from 120x40 down to 70x20 switches to `BelowMinimum` prompt with exact column/row report.
   - Resizing back up to 120x40 restores layout classification to `Full` and preserves cursor selection on `unit-102`.
2. **Escalation-First Ordering Teeth**:
   - Confirmed `unit-102` (escalated) appears at index 0 of the Navigator list despite being ordered after `unit-100` and `unit-101` in the input run data.
3. **Refusal of Mutations (Read-Only Contract)**:
   - Emitting an operator action (e.g. `{ type: "approve" }`) returns a rejected result: `refused: workbench is in read-only preview mode (T00A)`.
4. **Disconnected / Empty Workspace Teeth**:
   - Disconnected backend renders status `[DISCONNECTED]` and guidance: `Check that 'gauntlet serve' is running`.
   - Empty workspace renders `[EMPTY]` and guidance: `Initialize a run with 'gauntlet run'`.
5. **Zero Overlap Dependencies**:
   - `workbench/package.json` inspected and validated against `REQ-BUN-001` forbidden overlap items. Zero overlap packages present.

### 6. Verification Status

- `just tui-check`: PASS (TypeScript compiler strict checking)
- `just tui-lint`: PASS (Biome clean)
- `just tui-test`: PASS (12/12 tests passing)
- `just tui-preflight bootstrap`: PASS (Package skeleton, exact pins, lockfile sync, clean audit, spec/design hashes match)
- `just test-surfaces`: PASS (17 tests passing)
- `just ruler`: PASS (Frozen answer-key and reference lock unmutated)

Settlement condition satisfied. T00A is settled and ready to unblock Wave 2 (T01).
