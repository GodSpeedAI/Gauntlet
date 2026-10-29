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
- Correction round 3 (builder C, CI 40949652982): smoke-macos.sh + smoke-windows.ps1 doctor legs reworked to the observed fail-closed contract (env-only = typed nonzero + exit: FAIL + codemode + report line + non-empty doctor.json; exit 0 now FAILS) and a NEW required GAUNTLET_SMOKE_PROJECT in-project leg (doctor exits 0 in a copied doctor-clean project; typed exit-2 refusals). Validated: shellcheck clean + 8-case fake harness incl. both fail-closed teeth. CI must export GAUNTLET_SMOKE_PROJECT for both smokes.

## SETTLED (orchestrator, 2026-10-29 evening)

- **Package run 8 (40959948193) fully GREEN** — linux amd64 + linux arm64 (native
  ubuntu-22.04-arm, glibc ≤ 2.35 ABI gate), macos-14 (UNIVERSAL CLI pkg/dmg after the
  darwin-x64 optional-dep install attempt succeeded; arm64-qualified workbench inside;
  darwin-x64 cross build shipped only as pkg-macos-workbench-unqualified), windows-latest
  (zip + Inno setup.exe carrying a real-PTY-qualified workbench), sum (built-from == commit
  50355780, strict checksum verification, presence assertions, combined SHA256SUMS).
- Install acceptance proven per platform (mock-runner loop, serve + workbench connect,
  clean removal on linux; installer + uninstall on macos/windows).
- `just ci` green on the integrated tree; TUI gates green (1074/0/32-skip, preflight bootstrap
  PASS, tui-build release real-PTY PASS). CI-gates runs green on every round.
- Adversarial review F1-F9 all closed (commits 83059ca..bf058c7). SHA-256s + commit bindings
  in the release-meta artifact and the PR #7 evidence comment. aarch64 tarball byte-identical
  across runs 7/8 (bb7c0ad2…).
- Formal release flow (release-native.yml, gauntlet-v* tags) exists but was NOT executed —
  tag cutting is an operator decision. Artifacts downloadable from run 40959948193.
- Residuals recorded: test-only windows compile residuals (crates/gauntlet-cli/tests);
  projections.db re-mutation hermeticity quirk (restored each time, dde57792…); universal pkg
  workbench is arm64-only (disclosed); shell-quote fixed in-lockfile only.
