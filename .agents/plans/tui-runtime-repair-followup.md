# TUI runtime repair follow-up — settled 2026-09-10

Source: `workbench/repair/HANDOFF-PLAN.md` (handoff-ready) + `workbench/TUI-INSPECTION.md`
(seven production composition, rendering, input, and verification findings).
Ledger: `workbench/repair/IN-FLIGHT.md`. Builder evidence: `workbench/repair/host-result.json`,
`renderer-result.json`, `production_tests-result.json`.

Note: the handoff named this plan `.agents/plan/tui-runtime-repair-followup.md`
(singular); the canonical plans directory is `.agents/plans/`, so it lands here.

## Completed (this session)

1. Release evidence: `just tui-build release` — OK. Artifact
   `dist/gauntlet-workbench` sha256
   `1cb5a1b3ef7d88d0ab7c746025208471e93a40fae67ff8f2a357b62b74230bdd`
   (100160712 bytes), launch qualification real-PTY 120x40 with all markers
   (Gauntlet, NAVIGATOR, PRIMARY, INSPECTOR, COMPOSER), exit 0. Manifest
   `dist/release-manifest.json` verifies against the artifact on disk.
   Evidence frame written to
   `.agents/reports/gauntlet-tui-prime-workbench/evidence/T20/frames/release-artifact-120x40.txt`.
   (Prior session was blocked by a read-only managed-filesystem boundary; writable here.)
2. Full suites with loopback/tmux permitted:
   - `just tui-test`: 611 pass, 12 skip, 0 fail, 7576 assertions, 623 tests across 62 files.
   - `just tui-test all`: 49 suites, 530 pass / 0 fail / 9 skip — PASS.
   - Focused regressions re-confirmed after the final prompt correction:
     `workbench-host + production-entry` 3 pass / 41 assertions;
     `operator-preview + terminal-renderer` 12 pass / 154 assertions;
     `ssh-target` 5 pass, 1 expected SSH skip, 26 assertions.
3. Rust closure: NOT run — no Rust source changed
   (`git diff --name-only HEAD -- crates/` empty). Per workbench gate decoupling,
   the Rust closure gate does not cover `workbench/` changes.
4. `graft build`: PASS (12696 nodes, 19175 edges, 432 cards). Prior graph-write
   block is gone in this environment.
5. `just tui-check` PASS; `just tui-lint` PASS (exit 0; 78 warnings + 543 infos,
   pre-existing informational); `just tui-preflight bootstrap` PASS; `just ruler` PASS.

## Residuals

Superseded by `.agents/plans/workbench-intent-closure.md`, settled 2026-09-10 (phases
A-D, F; see `.agents/CURRENT_STATUS.yml#workbench_intent_closure`). That plan closed
every residual recorded here at settlement time: Prime/kernel/message submission is now
bound in production (Phase C2/D, `RealPrimeHost`, gated real-substrate batteries green
against the installed binary); the `poll-N` cursor was deleted and replaced with a
declared, generation-reconciled read path over a real canonical HTTP surface (Phase B);
the `ssh-target` false-green defect was repaired (Phase A) — the real-sshd leg remains an
honest gated skip on this host (no sshd installed), unchanged in kind, not in cause.

Only one residual survives that plan, and it is carried here by reference rather than
restated: `.agents/plans/workbench-intent-closure.md` §6, "The residual that survives
this plan" — the canonical Gauntlet surface exposes snapshots, not an ordered durable
event stream. See that section for the full statement and its tripwires.

## Acceptance

Actual release entry connects configured canonical reads, routes supported actions
through the gateway, renders all shell content with bounded terminal geometry/styles,
handles keyboard/Unicode/Escape, restores the terminal, and has production
composition regression evidence. No new dependency or authority semantics.
Status: settled 2026-09-10.
