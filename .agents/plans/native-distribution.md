# Plan: Gauntlet Native Distribution (Rust kernel + operator workbench)

Mission: make Gauntlet independently installable from precompiled native distributions
(`gauntlet` Rust CLI with the `surfaces` operator surface, and the `gauntlet-workbench`
Bun/OpenTUI standalone executable), per the operator assignment of 2026-10-29.

Source of truth for this effort: the operator's mission text (pasted 2026-10-29) + root
AGENTS.md + crates/AGENTS.md + workbench/AGENTS.md. Version authority: root Cargo.toml
`[workspace.package] version` (currently 0.1.0). Source commit at dispatch: 1543cbf (main).

Non-negotiables (mission §3/§8/§10):
- No architectural change: domain/ports/app/adapters/cli layering and `just boundaries` stay intact.
- No weakening of the ruler, gates, evidence, PASS/FAIL/INCONCLUSIVE semantics.
- Packaging must not silently install or simulate missing capabilities; fail-closed stays fail-closed.
- bwrap sandbox backend stays an honest typed refusal when unavailable (no fallback to `none`).
- Workbench stays a router/projection; the installed workbench connects to the operator surface,
  never bypasses the gateway.

## Environment facts (recon-established)

- Build form: `cargo build --release --locked -p gauntlet-cli --features surfaces --bin gauntlet`.
  Feature `surfaces` = dep gauntlet-adapter-surface-json (serve + JSON operator surface).
  Default build stays CLI-only (charter D1) — do not change feature defaults.
- CLI dispatcher is hand-written (no clap). Root flags: only --help/-h today. Golden help snapshots:
  crates/gauntlet-cli/tests/cli_surface.rs + tests/goldens/cli_help{,_surfaces}.txt (byte-exact).
  A new root `--version` arm is safe if usage() text is unchanged; usage text changes require
  deliberate, recorded golden updates plus the usage-lines/dispatch-table agreement tests.
- State store: PostgreSQL is REQUIRED for run/status/inspect/verify/export/serve/fabric
  (GAUNTLET_DATABASE_URL; dev default postgresql://postgres:postgres@127.0.0.1:54329/postgres —
  that default is a dev instance; packaged installs must point GAUNTLET_DATABASE_URL at a real server).
  Doctor does NOT probe postgres (never has) — do not fake one; document the prerequisite honestly.
- bwrap: optional (default backend `none`). Missing binary + non-`none` backend = typed refusal.
- Codemode tool-program host = `bun run codemode-host` in a `<dir>/workbench`-shaped tree
  (package.json script `codemode-host` -> `bun src/codemode/cli.ts`); CWD-relative probe in doctor;
  `fabric serve --program-host-dir` selects it explicitly. Decision (mission §7, smallest viable):
  codemode remains an optional Bun-runtime capability, reported truthfully by doctor; NOT bundled.
- Workbench: Bun >= 1.4.0, `@opentui/core` 0.5.11 ships per-OS/arch native packages via
  optionalDependencies — all 8 platform packages are in bun.lock, so per-OS native builds are
  feasible (build ON each target OS runner; no x64->arm64 copying). `bun:sqlite` is in-runtime
  (no external sqlite). Existing release builder: workbench/src/release-manifest.ts (x64-pinned,
  120x40 real-PTY launch qualification, manifest with sha256/build form/support facts).
- Known drift to fix: workbench/src/bun-native-smoke.test.ts ADMITTED_PROD list predates
  `quickjs-wasi` (package.json dep since 914c538); workbench/dist/* artifacts are stale
  (gitignored, on-disk only) — rebuild via `just tui-build release`.
- Sibling proven patterns (copy, do not invent): ~/projects/sxr (.github/workflows/package.yml,
  release-please.yml, clean-install.yml; packaging/{version.sh,check-linux-binary.sh,deb/build.sh,
  linux/build-tarball.sh,macos/build.sh,windows/build.ps1,*.iss,smoke-*}), domainforge (same +
  macos-intel fallback job), sea-rs (multi-binary deb/zip layout; Nix-detachment lessons).
- Repo is PUBLIC: `ubuntu-22.04-arm` GitHub-hosted runners are free — build ARM64 natively
  (glibc 2.35 == Jetson AGX Orin Ubuntu 22.04). NEVER cross-compile for the Jetson target.
- `.env.example` is diffed against spec Annex E (AK-019) — do NOT add env vars to it.
- `workbench/dist/` is gitignored; packaging stages from a fresh build, never commits artifacts.

## Artifact matrix (naming is a frozen contract)

| Target | Artifacts |
|---|---|
| linux amd64 | gauntlet_<v>_amd64.deb, gauntlet-<v>-x86_64-unknown-linux-gnu.tar.gz |
| linux arm64 | gauntlet_<v>_arm64.deb, gauntlet-<v>-aarch64-unknown-linux-gnu.tar.gz |
| windows x64 | gauntlet-<v>-x86_64-pc-windows-msvc.zip, gauntlet-<v>-x86_64-pc-windows-msvc-setup.exe |
| macOS (universal if qualified, else arm64, labeled honestly) | gauntlet-<v>-macos-<label>.pkg, .dmg |

Each package carries BOTH binaries (`gauntlet` + `gauntlet-workbench`) when the workbench binary
for that platform is built AND launch-qualified on its native runner; if the workbench cannot be
qualified on a platform, ship the CLI package and any workbench artifact separately and explicitly
labeled (never as qualified). Per-platform SHA256SUMS + built-from-<os>.txt (commit) accompany
every artifact group.

## Slice contracts (file ownership is exclusive; no competing edits)

### Slice R — CLI `--version` (owner: builder R; files: crates/gauntlet-cli/**)
- Additive root arg handling: `--version` and `-V` print `gauntlet <env!("CARGO_PKG_VERSION")>`
  to stdout, exit 0, in BOTH default and surfaces builds (before subcommand dispatch).
- If (and only if) a usage line is added, update BOTH golden help files deliberately and keep the
  usage-lines / dispatch-table / COMMAND_TABLE_LEN agreement tests green.
- New regression tests in the cli_surface test file (or a sibling): --version and -V exit 0 with
  the exact workspace version string, in both feature configurations.
- Gates: cargo check -p gauntlet-cli --all-targets; cargo nextest run -p gauntlet-cli --test cli_surface;
  cargo clippy -p gauntlet-cli --all-targets -- -D warnings; cargo nextest run -p gauntlet-arch-tests.
- No external dependencies. No other behavior change.

### Slice W — Workbench multi-target release builder (owner: builder W; files: workbench/src/**,
scripts/tui-preflight.sh only if E6 rows must change)
- Extend workbench/src/release-manifest.ts: derive the compile target from the HOST
  (process.platform/process.arch -> bun-linux-x64 | bun-linux-arm64 | bun-darwin-arm64 |
  bun-darwin-x64 | bun-windows-x64), overridable by GAUNTLET_WORKBENCH_RELEASE_TARGET for
  cross builds (cross targets get honest not-qualified-on-this-host launch records, never fake ones).
- Artifact: dist/gauntlet-workbench-<target> (update constant pins in release-manifest.test.ts
  deliberately); manifest carries per-target artifact path/sha256/bytes/bun version/lockfile digest/
  dependencies/launch qualification/support facts; keep build_form "bun-compile-standalone".
- Launch qualification stays REAL PTY 120x40 + clean `q` exit for host-executable targets; keep the
  T20 frame evidence write (create the directory if missing).
- Fix bun-native-smoke.test.ts ADMITTED_PROD to include quickjs-wasi (verify it is actually failing
  first; fix honestly, keep the attack's strength).
- Rebuild workbench/dist via `just tui-build release`; confirm scripts/tui-preflight.sh bootstrap
  and the E6 conformance row still hold.
- Gates: just tui-check; just tui-test (default suite); just tui-lint (no NEW findings);
  just tui-preflight bootstrap. Do not touch .github/ or packaging/.

### Slice B — Linux packaging (owner: builder B; new files only)
packaging/version.sh, packaging/check-linux-binary.sh, packaging/check-workbench-binary.sh,
packaging/deb/build.sh, packaging/linux/build-tarball.sh, packaging/smoke-gauntlet.sh,
packaging/install-test-deb.sh — per the interface contract below. Adapt sxr/domainforge scripts.
Deb: package name `gauntlet`, both binaries in /usr/bin, dpkg-shlibdeps-computed Depends (must
include libc6 >= 2.35 target compatibility), Recommends: postgresql, bubblewrap; NO postinst, no
services, /usr/share/doc/gauntlet/{copyright,README}. Tarball: reproducible flags, README.txt with
install + GAUNTLET_DATABASE_URL + optional-capability notes.
smoke-gauntlet.sh must run OUTSIDE the checkout on installed/staged binaries: --version exact,
--help, doctor exit 0 in an env-only dir (disposable GAUNTLET_REPORT_DIR), workbench PTY 120x40
markers + clean q exit (when a workbench binary is passed), 80x24 leg where supported.
install-test-deb.sh: apt install the deb, dpkg -L assertions, smoke with a real postgres (caller
provisions), a full mock-runner loop (GAUNTLET_MOCK_SCENARIO=e2e-happy over a copy of
tests/fixtures/target), serve + workbench connect leg, apt remove, binaries-gone assertions.
Local validation opportunistic (dpkg-deb exists on WSL); CI is the real qualifier.

### Slice C — Windows/macOS packaging (owner: builder C; new files only)
packaging/macos/build.sh, packaging/smoke-macos.sh, packaging/windows/build.ps1,
packaging/gauntlet.iss, packaging/smoke-windows.ps1 — per the interface contract below, adapted
from sxr (pkgbuild /usr/local, ad-hoc codesign, UDZO dmg, honest universal-vs-arm64 label from
lipo outcome; Inno 6 per-user, ChangesEnvironment=yes, HKCU PATH add/remove, x64compatible).
Smoke scripts assert installed identity + doctor-equivalent behavior; workbench launch legs
best-effort with honest skips (recorded, never silent).

### Slice E — CI release engineering (owner: builder E; files: .github/workflows/package.yml,
.github/workflows/release-native.yml, docs/how-to/install-native-binaries.md, README.md)
- package.yml: reusable workflow_call {ref, upload} + workflow_dispatch + PR-paths trigger;
  permissions contents: read; jobs: linux matrix (ubuntu-22.04/amd64 + ubuntu-22.04-arm/arm64),
  macos (macos-14), windows (windows-latest), sum (combined SHA256SUMS + built-from == GITHUB_SHA
  verification + asset presence assertions). Each platform job: rust stable toolchain, build the CLI
  per the frozen build form, bun 1.4.0 + bun install --frozen-lockfile + bun run src/release-manifest.ts
  (native host), ABI check scripts, stage both binaries (installed names), version.sh,
  deb+tarball (linux) / pkg+dmg (macos) / zip+setup.exe (windows), INSTALL tests (deb incl.
  postgres + mock-runner loop + serve/workbench connect; macos installer -pkg + smoke; windows
  silent per-user install + uninstall), per-platform SHA256SUMS + built-from-<os>.txt,
  upload-artifact (if-no-files-found: error).
- release-native.yml: tag push gauntlet-v* -> validate tag == version.sh output -> call package.yml
  (ref=tag, upload=true) -> publish job (download pkg-*, verify built-from commit == tag commit,
  sha256sum -c --strict, required-asset presence incl. both .deb arches, attest-build-provenance
  gated on commit equality, gh release upload --clobber, explicit failure-recovery notice).
  Formal release flow is minimal and GitHub-native (no Release Please).
- Pin style: floating major tags like sxr (@v4 etc.) + persist-credentials: false on every checkout;
  permissions: contents: read everywhere except the publish job (contents: write, id-token: write,
  attestations: write).
- docs/how-to/install-native-binaries.md: artifact matrix, SHA256SUMS verification, Linux deb/tarball
  installs, a dedicated NVIDIA Jetson AGX Orin (Ubuntu 22.04, arm64, glibc 2.35) section, macOS
  Gatekeeper note, Windows per-user note, runtime prerequisites (PostgreSQL REQUIRED with
  GAUNTLET_DATABASE_URL, bwrap optional, bun optional for codemode), uninstall/rollback, troubleshooting.

## Interface contracts (frozen; CI invokes exactly these)

- packaging/version.sh [expected]        -> stdout: x.y.z from root Cargo.toml; exit 1 on
                                           non-numeric version or mismatch with $1.
- packaging/check-linux-binary.sh <bin> <amd64|arm64> [rust|bun]
                                         -> ELF machine/class/interpreter gates, no RPATH,
                                            no /nix/store, NEEDED allowlist, max GLIBC <= 2.35.
- packaging/deb/build.sh <stage-dir> <version> <amd64|arm64> <out-dir>
                                         -> stage-dir has gauntlet (required) +
                                            gauntlet-workbench (optional); builds
                                            gauntlet_<v>_<arch>.deb.
- packaging/linux/build-tarball.sh <stage-dir> <version> <triple> <out-dir>
                                         -> gauntlet-<v>-<triple>.tar.gz (reproducible).
- packaging/smoke-gauntlet.sh <gauntlet> [workbench]
                                         -> exit 0 only on real behavior; disposable dirs;
                                            never writes outside its own temp roots.
- packaging/install-test-deb.sh <deb> <work-root> -> full install/exercise/remove cycle;
                                            caller guarantees postgres reachable via
                                            GAUNTLET_DATABASE_URL.
- packaging/macos/build.sh <arm64-stage> [<x64-stage>] <version> <out-dir>
                                         -> pkg + dmg, label universal|arm64 chosen honestly.
- packaging/windows/build.ps1 -Version <v> -BinDir <dir> -OutDir <dir>
                                         -> zip + setup.exe.

Staging rule: stage dirs contain binaries under their INSTALLED names (gauntlet,
gauntlet-workbench). The workbench artifact from dist/gauntlet-workbench-<target> is renamed at
stage time. Nothing under packaging/ may fetch from the network at package time.

## Dispatch sequencing and shared-resource discipline

- R, W, B, C, E dispatched in parallel on the shared branch release/native-distribution (single
  worktree). Ownership boundaries above are exclusive. Only R runs cargo; only W runs bun tests;
  B/C/E run no toolchain builds locally (CI qualifies). Integration, gate battery, commit, push,
  CI watching, and correction dispatches are the orchestrator's job.
- Every slice writes a report at .agents/reports/native-distribution/<slice>.md before messaging
  completion (evidence-first; a completion claim without a report is not a completion).

## Acceptance (definition of done for this plan)

1. Branch CI run green: linux amd64+arm64 (deb+tarball, install-tested, mock-loop, serve/workbench
   connect), windows (zip+setup.exe, install/uninstall-tested), macos (pkg+dmg, install-tested),
   per-platform checksums + commit binding.
2. `gauntlet --version` additive change merged into the same branch with gates green
   (check/lint/boundaries + cli_surface suite incl. goldens).
3. Workbench multi-target builder merged with tui gates green (check/test/lint/preflight bootstrap)
   and a rebuilt dist artifact.
4. `just ci` green on the integrated tree (orchestrator).
5. Downloadable CI artifacts (run URL + SHA-256 table) for the Jetson handoff; exact install script
   delivered in the final report; formal-release readiness documented (no tag cut without approval).
