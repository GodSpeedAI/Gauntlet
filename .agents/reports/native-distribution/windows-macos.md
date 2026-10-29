# Slice C report — Windows/macOS packaging (builder C)

Updated: 2026-10-29
Plan: `.agents/plans/native-distribution.md` (Slice C + "Interface contracts" are binding)
Branch: `release/native-distribution`
Files owned (new, exclusive): `packaging/macos/build.sh`, `packaging/smoke-macos.sh`,
`packaging/windows/build.ps1`, `packaging/gauntlet.iss`, `packaging/smoke-windows.ps1`.

## Files created

| File | Purpose |
|---|---|
| `packaging/macos/build.sh` | arm64/universal `.pkg` (pkgbuild, `/usr/local`) + UDZO `.dmg`, honest label, post-build assertions, sha256 |
| `packaging/smoke-macos.sh` | installed-binary smoke: `--version`/`--help`/`doctor`/`init` + real-PTY workbench leg (120x40, markers, clean `q`) |
| `packaging/windows/build.ps1` | `-Version/-BinDir/-OutDir` → zip (both exes + LICENSE at root) + Inno Setup 6 installer; Inno >= 6.3 asserted |
| `packaging/gauntlet.iss` | per-user Inno 6 installer: both exes + LICENSE + release notes, HKCU PATH add/remove, honest unsigned notice |
| `packaging/smoke-windows.ps1` | pwsh smoke: CLI legs + honest workbench start leg (unstartable = FAIL with exact error) |

All five are UNSIGNED and say so everywhere they speak to a user (README-install.txt,
RELEASE-NOTES.txt / InfoBeforeFile, script headers). No capability is simulated; every skip
and downgrade is printed and recorded.

## Per-script contract adherence

### packaging/macos/build.sh `<arm64-stage-dir> [<x64-stage-dir>] <version> <out-dir>`
- 3 args → arm64-only (`-macos-arm64`); 4 args → `lipo -create` per binary pair (`-macos-universal`).
  The label follows the lipo outcome structurally: `universal` is unreachable without an x86_64
  stage dir, and the post-merge `lipo -archs` assertions re-verify both slices in BOTH binaries
  (arm64 missing → fail; universal without x86_64 → fail). An arm64-only build can never be
  labeled universal.
- Both installed-name binaries (`gauntlet`, `gauntlet-workbench`) required in every provided
  stage dir (missing → error with path).
- Stage: `bin/` + `share/doc/gauntlet/{LICENSE,README-install.txt}`; `pkgbuild --root …
  --identifier ai.godspeed.gauntlet --version <v> --install-location /usr/local` → installs to
  `/usr/local/bin` and `/usr/local/share/doc/gauntlet/`.
- `codesign --force --sign -` (ad-hoc) on every staged binary, after lipo/cp (a merge invalidates
  input signatures; arm64 requires a valid signature).
- `hdiutil create -format UDZO` → `gauntlet-<v>-macos-<label>.dmg` containing the .pkg + LICENSE
  + README-install.txt (unsigned notice with Gatekeeper/xattr guidance and SHA256SUMS/attestation
  verification hints; honest prerequisites: PostgreSQL REQUIRED via GAUNTLET_DATABASE_URL, bwrap
  optional fail-closed, bun optional for codemode, workbench env vars; uninstall instructions:
  `sudo pkgutil --forget ai.godspeed.gauntlet` + removal of both binaries and the doc dir).
- Post-build assertions: `pkgutil --expand` + PackageInfo grep for ` version="<v>"` and
  `identifier="ai.godspeed.gauntlet"`; both outputs non-empty; sha256 of both printed.
- README-install.txt is written BEFORE `pkgbuild`, so the in-pkg copy at
  `/usr/local/share/doc/gauntlet/README-install.txt` is real, and copied onto the dmg.

### packaging/smoke-macos.sh `<gauntlet-bin> [workbench-bin] <expected-version>`
- Arg forms: 2 args (CLI only) or 3 args (workbench second). Version expected is the LAST arg.
- Disposable mktemp root, trap-cleaned; disposable HOME; `GAUNTLET_REPORT_DIR` under the root;
  clears GAUNTLET_PROJECT_ROOT/GAUNTLET_CONFIG_DIR/GAUNTLET_DATABASE_URL/GAUNTLET_ENDPOINT/
  GAUNTLET_MOCK_SCENARIO so the CLI runs env-only. Writes nothing outside its own root.
- Legs: (1) `--version` exact `gauntlet <v>`; (2) `--help` exit 0 with non-empty output;
  (3) `doctor` exit 0 in a bare temp cwd + machine report present under GAUNTLET_REPORT_DIR
  (doctor does not probe PostgreSQL; none provided, none simulated); (4) `gauntlet init` in a
  temp project (asserts GAUNTLET.md, .env.example, .gauntlet/) plus the second-init refusal;
  (5) workbench leg only when a workbench binary is passed.
- Workbench PTY leg: embedded python3 harness — `pty.fork`, winsize 120x40 via TIOCSWINSZ
  (child-side before exec), TERM=xterm-256color, waits up to 30 s for COMPOSER, sends `q`,
  requires exit 0 within 15 s, requires all of NAVIGATOR/PRIMARY/INSPECTOR/COMPOSER in the
  accumulated frame, prints a frame tail on failure. Workbench cwd and
  GAUNTLET_WORKBENCH_PROJECTION_STORE are disposable; GAUNTLET_WORKBENCH_GAUNTLET_BINARY points
  at the CLI under test. Honesty contract: leg cannot run (no python3) → FAIL with a remediation
  hint unless `GAUNTLET_SMOKE_SKIP_PTY=1`, in which case a WARNING + recorded skip is printed;
  a leg that RUNS and fails is a failure, never a skip. No workbench passed → recorded NOTE.

### packaging/windows/build.ps1 `-Version <v> -BinDir <dir> -OutDir <dir>`
- Named mandatory params; semver-regex validation (sxr-proven pattern).
- Requires `gauntlet.exe` + `gauntlet-workbench.exe` + repo LICENSE.
- Generates `RELEASE-NOTES.txt` INTO BinDir (the only file it adds there) — shown as the
  installer wizard's information page and installed to `{app}`; content states what is
  installed, the unsigned status, and the honest prerequisites.
- Zip: `gauntlet-<v>-x86_64-pc-windows-msvc.zip` via temp stage + `Compress-Archive`; both exes
  and LICENSE at the archive root; stale outputs removed first.
- Inno: locates ISCC.exe (PATH → ProgramFiles(x86) → ProgramFiles); reads the Inno Setup version
  from the ARP registry; `< 6.3` → hard throw (x64compatible needs >= 6.3); undeterminable →
  loud `Write-Warning` (the ISCC compile itself fails on older Inno, as backstop). Drives ISCC
  with `/DAppVersion /DBinDir /DLicenseFile /DOutDir /DSetupBase` and `packaging/gauntlet.iss`;
  asserts `gauntlet-<v>-x86_64-pc-windows-msvc-setup.exe` exists; prints the artifact table and
  sha256 of both outputs.

### packaging/gauntlet.iss
- `PrivilegesRequired=lowest`, `DefaultDirName={localappdata}\Programs\Gauntlet`,
  `ArchitecturesAllowed=x64compatible`, `ArchitecturesInstallIn64BitMode=x64compatible`,
  `ChangesEnvironment=yes`, `Compression=lzma2`, `WizardStyle=modern`, new fixed AppId GUID
  (`{4C9A7E1F-8B2D-4E63-9F5A-2D8C0B7E1A34}`), AppVersion/AppVerName from `/DAppVersion`,
  publisher GodSpeedAI / github.com/GodSpeedAI/Gauntlet.
- [Files]: gauntlet.exe, gauntlet-workbench.exe, LICENSE, RELEASE-NOTES.txt (all `ignoreversion`).
- `InfoBeforeFile={#BinDir}\RELEASE-NOTES.txt` — the honest unsigned notice + prerequisites are
  ON the wizard, not buried.
- PascalScript `AddAppToUserPath`/`RemoveAppFromUserPath` (HKCU\Environment; idempotent add at
  ssPostInstall, clean removal at usPostUninstall) — verbatim from sxr's proven realitytrace.iss.

### packaging/smoke-windows.ps1 `-Gauntlet <path> [-Workbench <path>] -ExpectedVersion <v>`
- CLI legs mirror macOS: exact `--version`; `--help` exit 0 + non-empty; `doctor` exit 0 with
  temp `GAUNTLET_REPORT_DIR` + doctor.json assert; `init` scaffold asserts + second-init
  refusal. Config env (GAUNTLET_PROJECT_ROOT/CONFIG_DIR/DATABASE_URL/ENDPOINT/MOCK_SCENARIO)
  cleared; every CLI invocation runs with the temp root or a subdir as cwd.
- Workbench leg (only with `-Workbench`): `[System.Diagnostics.Process]` with redirected
  stdin/stdout/stderr, disposable cwd + `GAUNTLET_WORKBENCH_PROJECTION_STORE`,
  `GAUNTLET_WORKBENCH_GAUNTLET_BINARY` set. Cannot start → FAIL with the exact exception
  (`Win32Exception` message surfaces verbatim). Exits non-zero immediately → FAIL with captured
  stdout/stderr (an unstartable/broken binary must never ship). Exits 0 → marker probe on the
  captured pipe; markers present → full pass, else printed downgrade ("render qualification
  happens on the macOS/Linux legs — recorded"). Still running after 15 s → writes `q` to stdin;
  clean exit → pass; else kill + printed started-only downgrade. `$started` guard prevents a
  never-started `Process.HasExited` throw from masking a failure. Uninstall assertions are the
  workflow's (out of scope per contract).

## sxr deltas (what changed from the proven originals, and why)

- **Argument order (macos/build.sh, build.ps1):** frozen Gauntlet contract
  (`<arm64> [<x64>] <version> <out-dir>`; named `-Version/-BinDir/-OutDir`) differs from sxr's
  positional `<version> <arm64dir> <x86dir> <outdir>`.
- **Two binaries everywhere:** sea-rs multi-binary pattern applied to sxr's single-binary flow;
  both stage dirs must carry both installed-name binaries.
- **Doc payload:** `share/doc/gauntlet/{LICENSE,README-install.txt}` (task contract) instead of
  sxr's `copyright`; README also shipped on the dmg (sxr did not put the README in the pkg).
- **New post-build assertions (macos):** sxr had none; contract requires version/identity
  verification and sha256 printing. `pkg --info` was replaced by `pkgutil --expand` + PackageInfo
  after research showed `pkg --info` is not a dependable macOS tool (two web sources agree;
  `pkg` is the BSD package manager).
- **New lipo -archs verification (macos):** enforces the honest universal/arm64 label beyond
  arg-shape.
- **smoke-macos.sh is new:** sxr has no macOS smoke; env hygiene adapted from sxr's
  smoke-realitytrace.sh, product legs from Gauntlet's real surface, PTY leg mirrors the
  workbench release-manifest launch qualification (same markers, 120x40, q-exit) — with the
  task's GAUNTLET_SMOKE_SKIP_PTY loud-skip contract added.
- **No auto `choco upgrade` in build.ps1:** the plan's staging rule forbids network fetches at
  package time; the task offered "assert >= 6.3 … or document that the CI workflow pre-installs".
  Chosen: hard assert + loud warning when undeterminable + CI-pre-install documentation. Delta
  recorded here deliberately.
- **Release notes without new defines:** the frozen define list (AppVersion/BinDir/LicenseFile/
  OutDir/SetupBase) has no notes slot, so build.ps1 generates `RELEASE-NOTES.txt` into BinDir and
  the .iss requires it (compile fails loudly if someone drives ISCC directly without build.ps1).
- **gauntlet.iss adds InfoBeforeFile** (honest unsigned notice ON the wizard) — new vs sxr; PATH
  PascalScript kept byte-identical to the proven original.

## Validation evidence (what actually ran)

- `sh -n` clean for both shell scripts.
- **End-to-end execution of smoke-macos.sh on this Linux host** with a fake `gauntlet`
  (POSIX-sh implementation of the four CLI legs against the same observable contract) and fake
  workbenches (python3 raw-mode pty children): happy path (both binaries) ALL PASS; CLI-only
  run ALL PASS with recorded NOTE/SKIP; wrong expected version → FAIL; marker-less workbench →
  FAIL with frame-tail diagnostics; workbench exiting non-zero on `q` → FAIL;
  `GAUNTLET_SMOKE_SKIP_PTY=1` → loud recorded skip and pass; no leaked temp roots. The PTY
  harness machinery (pty.fork, ioctl winsize, marker wait, q, exit status) is therefore really
  exercised, not desk-checked.
- build.ps1 / gauntlet.iss / smoke-windows.ps1: **desk-check only** — no pwsh or Inno on this
  host (checked: `command -v pwsh` empty). Covered: here-string terminator placement, backtick
  continuations (no trailing whitespace after backticks), brace balance (47/47),
  `${env:ProgramFiles(x86)}` quoting, `Process.HasExited`-on-never-started guard, LF line
  endings matching the proven sxr originals, .iss/AppId `{{` escaping, PascalScript verbatim
  from a known-compiling original.

## Assumptions and CI-only items

1. `lipo` runtime validity for the Bun standalone workbench: lipo merges complete Mach-O slices,
   but whether the merged Bun executable actually RUNS as universal must be qualified in CI —
   the smoke's PTY leg (post-install) is that qualifier. If Bun universal merging proves broken,
   the supported fallback is arm64-only packages (pass only the arm64 stage); never mislabeled.
2. `doctor` exits 0 in a bare disposable dir (no GAUNTLET.md, §8.6 conservative defaults, no
   postgres probe). Contract-frozen and consistent with `doctor_cmd.rs` /
   `gauntlet-app/src/doctor/mod.rs` (report path `$GAUNTLET_REPORT_DIR/doctor.json` confirmed in
   source); not locally executable because the CLI binary is not built here (builder R owns
   cargo). If CI disagrees, that is a cross-slice contract question, not a script bug.
3. `--version` support comes from slice R (parallel); the smokes assert the frozen exact output
   `gauntlet <semver>`.
4. `pkgutil --expand` works on unsigned component pkgs (expansion is signature-independent).
5. `lipo -archs` prints space-separated arch names on both thin and fat inputs (standard).
6. macOS runners provide python3 (macos-14 images do) and the standard tool set used here
   (pkgbuild, codesign, lipo, hdiutil, pkgutil, shasum).
7. choco-installed Inno Setup registers in the ARP registry (it uses the native installer);
   portable Inno copies may not be detectable → loud warning, ISCC compile as backstop.
8. pwsh (PowerShell 7) runs both .ps1 files (contract); scripts are UTF-8 without BOM — pwsh 7
   reads that correctly; Windows PowerShell 5.1 is not targeted.
9. Stage dirs contain binaries under installed names (plan staging rule); BinDir receives only
   the additive RELEASE-NOTES.txt from build.ps1, and the zip content is controlled explicitly,
   so nothing leaks into the zip.
10. `spctl` is deliberately NOT asserted: an unsigned pkg is expected to fail it, and asserting
    rejection is brittle; "size sanity" here means non-empty-output asserts (`test -s` /
    `Test-Path`). Documented in README-install.txt instead.

## Residuals / fog

- Execution of pkgbuild/codesign/lipo/hdiutil/pkgbuild assertions, ISCC, and Compress-Archive is
  CI-only on this host (WSL); those paths are desk-checked against sxr originals that are
  known-good in CI, plus the deltas listed above.
- This script REQUIRES the workbench for macOS packages. If the workbench cannot be qualified on
  macOS in CI, a CLI-only macOS artifact needs an explicit contract change (the plan allows
  shipping it separately/labeled) — I did not silently add an "optional workbench" mode because
  it could produce a full-looking package that is quietly missing the TUI.
  [CLOSED by correction round 2 (F5): build.sh now takes the workbench as OPTIONAL from the
  arm64 stage and installs it as-is, refusing only an x86_64 workbench slice; see the
  correction-round-2 section below.]
- The universal label honestly depends on lipo verification only; it cannot prove the merged
  binary launches (residual 1 covers this).
- Em-dash characters appear in some smoke output strings (UTF-8). POSIX sh and pwsh 7 pass them
  through; cosmetic only.

## State bookkeeping

- Per the dispatch's exclusive file ownership, this report is my completion record;
  `.agents/CURRENT_STATUS.yml` was not structurally edited (parallel builders edit shared state
  concurrently; the orchestrator reconciles from slice reports per IN-FLIGHT.md "Next moves").
  A single appended checkpoint line was added to `.agents/reports/native-distribution/IN-FLIGHT.md`.
- No commits, no pushes. No cargo/bun invocations.

## Correction round (2026-10-29, orchestrator integration decision)

Trigger: Slice E surfaced a contract contradiction — the original build.ps1 hard-required
`gauntlet-workbench.exe`, so an UNQUALIFIED Windows workbench binary would physically ride
inside the zip and setup.exe, violating "do not ship an unqualified workbench as though it
passed release validation". Implemented in owned files only; `.github/**` and
`packaging/macos/**` untouched.

1. `packaging/windows/build.ps1`
   - `gauntlet.exe` + LICENSE remain REQUIRED; `gauntlet-workbench.exe` is now OPTIONAL
     (`$workbenchStaged = Test-Path ...`).
   - The decision is recorded on the build log either way: `Write-Host` when staged ("packaging
     CLI + workbench"), `Write-Warning` when absent ("packaging CLI-ONLY ... must not be
     presented as Windows-qualified"). Never silent.
   - Zip payload is conditional: `gauntlet.exe` + LICENSE always, `gauntlet-workbench.exe` only
     when staged.
   - RELEASE-NOTES.txt is composition-honest: a `Composition:` line plus a conditional install
     list; the CLI-only variant states explicitly "THIS PACKAGE DOES NOT INCLUDE
     gauntlet-workbench.exe: ... not shipped here as though it passed release validation", and
     the workbench env-var prerequisite bullet appears only in the full variant.
   - ISCC is invoked with an extra `/DIncludeWorkbench` define ONLY when the exe was staged
     (define list built as an array and splatted: `& $iscc @issDefines $issFile`).
2. `packaging/gauntlet.iss`
   - The workbench `[Files]` entry is gated on `#ifdef IncludeWorkbench`. Inno derives the
     uninstall record from `[Files]`, so there are no separate uninstall entries to gate — the
     gate is complete. AppId is UNCHANGED, so CLI-only and full installs share one product
     identity for upgrade/uninstall. Header comments and the BinDir `#error` text document the
     define.
3. `packaging/smoke-windows.ps1` — VERIFIED, no change needed: `-Workbench` was already an
   optional parameter (`[Parameter()][string]$Workbench`), with the start leg gated on
   `$WorkbenchBin` and a recorded `NOTE ... skipped: not requested` when absent.

Re-checked after the edit: brace balance 31/31, no trailing whitespace after backtick
continuations, here-string terminators intact, the .iss gate appears exactly once.

New residuals from this round:
- The frozen artifact FILE NAMES (`gauntlet-<v>-x86_64-pc-windows-msvc.zip` / `-setup.exe`) do
  not encode the composition; the composition is recorded in RELEASE-NOTES.txt (wizard-visible),
  the build log, and Slice E's `built-from-<os>.txt`. If the orchestrator wants composition in
  the filename (e.g. a `-cli` suffix), that is an artifact-matrix change and needs a decision.
- The same composition question exists on macOS but with a different frozen mechanism
  (honest universal/arm64 label; and build.sh still REQUIRES the workbench). Per instruction,
  `packaging/macos/**` was not touched in this round. If the workbench cannot be qualified on
  macOS, build.sh fails loudly — the CLI-only-macos-package path remains a recorded contract
  decision, not a silent fallback.
  [SUPERSEDED by correction round 2 (F5): the macOS composition contract is now explicit —
  see below.]

## Correction round 2 (2026-10-29, F5 — adversarial review + orchestrator decision)

Trigger (F5, MAJOR): the original macos/build.sh treated the x64 stage as "workbench included",
so a successful darwin-x64 cross build would have lipo'd an UNQUALIFIED x64 workbench slice into
a `universal`-labeled pkg — a false capability claim. Decision (matching the Windows contract):
the macOS pkg NEVER contains an x86_64 workbench slice.

Implemented in `packaging/macos/build.sh` (signature unchanged:
`build.sh <arm64-stage> [<x64-stage>] <version> <out-dir>`):

- CLI (`gauntlet`): required in the arm64 stage and in the x64 stage when provided; lipo-merged
  into a universal slice when both stages exist (label `universal`), else arm64-only (label
  `arm64`).
- Workbench (`gauntlet-workbench`): OPTIONAL, taken ONLY from the arm64 stage and installed
  AS-IS — never lipo'd. If the x64 stage contains a `gauntlet-workbench` binary (any file,
  `-e` check), build.sh REFUSES with a loud four-line error: a darwin-x64 workbench may only
  ship as a separate unqualified workflow-handled artifact.
- New arch assertion: a staged workbench must be a THIN arm64 binary — `lipo -archs` must show
  arm64 and must NOT show x86_64 (a fat workbench staged into the arm dir is refused).
- README-install.txt gains a `Composition` section stating exactly which binaries are inside
  and their arch(es). When the CLI is universal but the workbench is arm64-only, it states
  explicitly: "The included gauntlet-workbench runs on Apple Silicon only; Intel Macs get the
  CLI only. No x86_64 workbench slice is ever shipped in this package." CLI-only packages state
  that the workbench is not included and was not staged/qualified. The Workbench usage section
  appears only when the workbench is actually in the payload; the Install line mentions
  gauntlet-workbench only when present.
- Header comment rewritten to document the composition contract.

`packaging/smoke-macos.sh`: UNCHANGED — verified it never assumed a lipo'd workbench (it PTY
tests whatever workbench binary is passed; an arm64-only installed workbench is handled the
same). Only change in this round: `CDPATH=` → `CDPATH=''` (two lines) to satisfy shellcheck
SC1007; behavior identical.

Validation this round:
- `sh -n` clean; `shellcheck` CLEAN on both shell scripts (previously 3 SC1007 warnings).
- Fake-tool harness (fake lipo/codesign/pkgbuild/pkgutil/hdiutil/shasum honoring the real
  tools' observable contracts) exercising the REAL build.sh on this Linux host:
  - universal CLI + arm64 workbench → PASS; README Composition states universal CLI +
    arm64-only workbench + Apple-Silicon/Intel sentence; Workbench section present.
  - universal CLI-only → PASS; Composition states "workbench: not included" + not-qualified
    honesty line; no Workbench section.
  - arm64-only CLI + workbench → PASS; `-macos-arm64` names; Composition "both arm64".
  - x64 stage carrying a workbench → refused rc=1 with the loud error.
  - fat (arm64+x86_64) workbench staged in the arm dir → refused rc=1.
  - missing x64 CLI → refused rc=1.
- Harness noise notes (not build.sh issues): the fake shasum mishandles `-a 256` argument
  order (prints a stray error line while still emitting the correct hash — the real macOS
  shasum handles it); the first harness run's `test -s` failures were the fake pkgbuild writing
  zero-byte pkgs, which actually demonstrated the non-empty-output assertion firing correctly.

Residual unchanged: the fake-tool harness proves control flow, not real pkgbuild/lipo/codesign
output — macOS CI remains the real qualifier (same re-run as the tmux fix, CI run 40934599697).
