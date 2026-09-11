# TUI runtime repair follow-up

Status: handoff-ready, 2026-09-10.

The production repair is implemented in the workbench tree. The next operator should close the environment-dependent gates below without changing the frozen spec, answer key, or acceptance criteria.

## Completed

- Replaced the ANSI-only launcher with configured `WorkbenchHost` and native OpenTUI rendering.
- Connected canonical state, inspection, capabilities, polling, reconnect/disconnect handling, and gateway-routed resolve/retry operations.
- Preserved Navigator, Primary, Inspector, attention, palette, composer, contextual help, and semantic styling.
- Corrected 120x40, 80x24, and below-minimum geometry; the 60x18 prompt keeps `press q to quit` and admitted-work status visible.
- Added Escape, Shift-Tab, resize, bracketed paste, and grapheme-safe Unicode editing.
- Added focused host, renderer, composer, and release-entry PTY/screen-emulator tests.

## Remaining

1. Run `just tui-build release` from an environment that can write the declared T20 evidence path under `.agents/reports/gauntlet-tui-prime-workbench/evidence/T20/frames/`; this session built the executable but the managed filesystem rejected that evidence write. Confirm the release manifest and digest checks.
2. Run `just tui-test` and `just tui-test all` with loopback listen and tmux permitted. The complete suite passed before the final prompt assertion correction; focused regressions pass after it. Record exact final totals.
3. Run `just verify` if full Rust closure is required. No Rust source changed.
4. Decide whether the bounded lack of live Prime/kernel/message transports and resumable event streams remains intended. The host reports these paths unsupported and never fabricates delivery.
5. Update the repository-level `.agents/CURRENT_STATUS.yml` and `.agents/plan/tui-runtime-repair-followup.md` (requested handoff targets) with this plan, exact gate totals, skips, and residuals once those paths are writable. Then update `workbench/repair/IN-FLIGHT.md`, result contracts, run `graft build`, and inspect the final diff. In this session the managed filesystem rejected both repository-level writes and `graft build` graph output; the exact failures are recorded here rather than hidden.

Suggested status entry once the parent path is writable:

```yaml
tui_runtime_repair:
  status: handoff_ready
  updated: "2026-09-10"
  scope: Fix seven production composition, rendering, input, and verification findings.
  task_ledger: workbench/repair/IN-FLIGHT.md
  plan: .agents/plan/tui-runtime-repair-followup.md
  next_action: >-
    Run release evidence and complete TUI suites with loopback/tmux permissions,
    rerun graft build, then settle or record the bounded transport residuals.
```

## Evidence

- `workbench/TUI-INSPECTION.md` records the seven original findings and red reproduction.
- `workbench/repair/host-result.json`, `renderer-result.json`, and `production_tests-result.json` record builder evidence.
- `bun test src/workbench-host.test.ts src/production-entry.test.ts`: 3 passed, 0 failed, 41 assertions.
- `bun test src/ssh-target.test.ts`: 5 passed, 1 expected SSH skip, 0 failed.
- Focused geometry/renderer tests: 12 passed, 154 assertions.
- `just tui-preflight bootstrap`, `just tui-check`, `just tui-lint`, and `just ruler` passed with required runtime/network access. Sandbox-only preflight failed at npm advisory DNS resolution.
- `just tui-build release` compiled `dist/gauntlet-workbench` but could not write the existing T20 evidence frame under `.agents/reports/.../frames/` because that repository path is read-only in this managed session; the artifact was launched successfully in non-TTY mode.
- `graft build` parsed the complete tree but could not write `graft/.graph/wiring.json.10.tmp` for the same managed filesystem boundary.
