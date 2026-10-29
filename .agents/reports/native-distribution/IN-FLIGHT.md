# IN-FLIGHT — Native distribution (Rust kernel + operator workbench)

Updated: 2026-10-29 (correction round 1 integrated; Package run 2 in flight)
Plan: `.agents/plans/native-distribution.md`
Branch: `release/native-distribution` — commits: fdcbb11 (R), b538e46 (W), 3a148bb (B/C/E), 7b5d0f4 (.agents), 83059ca (correction round 1)
Mission: operator assignment 2026-10-29 — native binary distribution for the `gauntlet` Rust CLI
(surfaces build) and the `gauntlet-workbench` Bun/OpenTUI standalone executable; immediate
settlement target is a verified downloadable ARM64 package for the NVIDIA Jetson AGX Orin 64GB.

## State

- Slices R/W/B/C/E landed and integrated; `just ci` battery green on the integrated tree;
  TUI gates green (tui-check, tui-test 1073→1074/0, tui-lint no new findings, preflight
  bootstrap PASS after the shell-quote lockfile fix, tui-build release PASS with real-PTY
  qualification).
- PR #7 open; Package (native) run 1 (40934599697) RED — diagnosed by independent adversarial
  review (Agent F, verdict DO-NOT-SHIP, findings F1-F9); CI gates run 1 (40934599549) GREEN.
- Correction round 1 committed (83059ca): F1 smoke doctor legs reworked to the real fail-closed
  contract (+ GAUNTLET_SMOKE_PROJECT interface, plumbed into the workflow), F2 all external
  spawns guarded in release-manifest.ts (tmux ENOENT killed the macos job), F3 sum job fails
  on partial platform success, F4 Zone.Identifier file renamed (windows checkout was dead),
  F5 macOS pkg never carries an x86_64 workbench slice (+ pkg-macos-workbench-unqualified),
  F6 install doc honesty, F7 marker-set verification, F8 annotated-tag peel, F9 tarball pipefail.
- Package run 2 (40944168892) + CI run 2 (40944168905) in flight on 83059ca.

## Acceptance snapshot (from plan)

1. Branch CI run green on linux amd64+arm64 / windows / macos with install tests.
2. `gauntlet --version` additive, gates green.
3. Workbench multi-target builder with tui gates green.
4. `just ci` green on the integrated tree.
5. Downloadable artifacts + SHA-256 table + Jetson install script delivered.

## Next moves

- Watch Package run 2; fix any residual failures; collect per-platform SHA256SUMS +
  built-from bindings from the green run; deliver the Jetson install script and the final
  verification matrix in the handoff report.

## Slice checkpoint (builder C, 2026-10-29)

- Slice C complete: packaging/macos/build.sh, packaging/smoke-macos.sh, packaging/windows/build.ps1, packaging/gauntlet.iss, packaging/smoke-windows.ps1 written (no commits). Report: .agents/reports/native-distribution/windows-macos.md. Smoke harness locally validated end-to-end with fake binaries (happy + 4 negative + skip paths); pkgbuild/ISCC paths are desk-checked CI-only.
- Correction round (builder C): windows packaging made composition-honest — gauntlet-workbench.exe optional in build.ps1 (CLI-only zip/installer, /DIncludeWorkbench gate, recorded Write-Warning), gauntlet.iss workbench entry gated on #ifdef IncludeWorkbench (AppId fixed), smoke-windows.ps1 verified already optional. Details: windows-macos.md correction-round section.
- Correction round 2 (builder C, F5): packaging/macos/build.sh never ships an x86_64 workbench slice — CLI lipo'd universal from both stages, workbench OPTIONAL from arm64 stage only installed as-is, loud refusal on any x64-stage/fat workbench, README Composition section (Apple-Silicon-only note), shellcheck clean, fake-tool harness validated 3 compositions + 3 refusals. Details: windows-macos.md correction-round-2 section.

## Slice checkpoint (builder correction, 2026-10-29, windows gauntlet-cli portability)

- Package run 2's `package (windows x86_64)` job diagnosed: 8 errors, all unix-only fs
  permissions in gauntlet-cli src. Correction gated 5 production/const sites (fabric_compose
  `is_executable_file` + `EXECUTE_BITS`; improve_cmd baseline-instrument, judge-wrapper, and
  `make_scripts_executable` chmods) plus 3 more found by proactive scan (2 test chmods in
  improve_cmd, verify_cmd signal-kill test now `#[cfg(unix)]`). Unix arms byte-identical;
  non-unix arms are warning-free no-ops. Windows-target check of the CI lib+bin surface
  (`--features surfaces --locked`) GREEN; unix gates green (check --all-targets, clippy
  -D warnings, fmt, nextest cli_surface 5/5, arch-tests 261/261). Residual: unix-only
  constructs remain in `crates/gauntlet-cli/tests/*.rs` (8 files) — outside touch scope,
  do not block the bin-only package build; listed in rust-windows-portability.md.
