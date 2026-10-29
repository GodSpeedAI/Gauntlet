# IN-FLIGHT — Native distribution (Rust kernel + operator workbench)

Updated: 2026-10-29 (dispatch)
Plan: `.agents/plans/native-distribution.md`
Branch: `release/native-distribution` (from main @ 1543cbf)
Mission: operator assignment 2026-10-29 — native binary distribution for the `gauntlet` Rust CLI
(surfaces build) and the `gauntlet-workbench` Bun/OpenTUI standalone executable; immediate
settlement target is a verified downloadable ARM64 package for the NVIDIA Jetson AGX Orin 64GB.

## State

- Recon complete (three read-only agents): CLI runtime facts, sibling packaging patterns
  (sxr/domainforge/sea-rs), workbench release-builder facts — summarized in the plan.
- Five builder slices dispatched in parallel with frozen contracts and exclusive file ownership:
  - R — CLI `--version` (crates/gauntlet-cli)
  - W — workbench multi-target release builder (workbench/src, tui gates)
  - B — Linux packaging scripts (packaging/*: deb, tarball, ABI checks, smoke, install test)
  - C — Windows/macOS packaging scripts (packaging/*: Inno, pkgbuild/dmg, smokes)
  - E — CI release engineering (.github/workflows/package.yml, release-native.yml, install docs)
- No commits yet; integration + gate battery + CI qualification are the orchestrator's.

## Acceptance snapshot (from plan)

1. Branch CI run green on linux amd64+arm64 / windows / macos with install tests.
2. `gauntlet --version` additive, gates green.
3. Workbench multi-target builder with tui gates green.
4. `just ci` green on the integrated tree.
5. Downloadable artifacts + SHA-256 table + Jetson install script delivered.

## Next moves

- Collect slice reports (`.agents/reports/native-distribution/<slice>.md`), review diffs,
  run the Rust + TUI gate batteries, commit, push, dispatch `package.yml` via workflow_dispatch,
  watch CI, correct, and re-run. Then adversarial review (Agent F) and the final handoff report.

## Slice checkpoint (builder C, 2026-10-29)

- Slice C complete: packaging/macos/build.sh, packaging/smoke-macos.sh, packaging/windows/build.ps1, packaging/gauntlet.iss, packaging/smoke-windows.ps1 written (no commits). Report: .agents/reports/native-distribution/windows-macos.md. Smoke harness locally validated end-to-end with fake binaries (happy + 4 negative + skip paths); pkgbuild/ISCC paths are desk-checked CI-only.
- Correction round (builder C): windows packaging made composition-honest — gauntlet-workbench.exe optional in build.ps1 (CLI-only zip/installer, /DIncludeWorkbench gate, recorded Write-Warning), gauntlet.iss workbench entry gated on #ifdef IncludeWorkbench (AppId fixed), smoke-windows.ps1 verified already optional. Details: windows-macos.md correction-round section.
