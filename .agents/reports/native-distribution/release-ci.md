# Slice E report — CI release engineering (native distribution)

Builder: E. Branch: `release/native-distribution`. Date: 2026-10-29.
Plan: `.agents/plans/native-distribution.md` (Slice E, Interface contracts, Artifact matrix — binding).
No commits, no pushes, no local cargo/bun runs (CI-only qualification, per dispatch).

## Files created/edited

| File | Status |
|---|---|
| `.github/workflows/package.yml` | new (reusable package workflow) |
| `.github/workflows/release-native.yml` | new (tag-validate → package → publish) |
| `docs/how-to/install-native-binaries.md` | new (Diátaxis how-to, adapted from sxr) |
| `README.md` | edited — one "## Install" section (18 added lines, 0 removed), pointer to the how-to + both release channels |

No other file was touched. `git status` confirms the other builders' modified files are untouched.

## Workflow structure summary

### package.yml (reusable)

- Triggers: `workflow_call` {`ref` (string, ''), `upload` (boolean, false)}; `workflow_dispatch` (same inputs); `pull_request` with the exact paths filter from the dispatch (packaging/**, both workflow files, Cargo.toml/lock, crates/gauntlet-cli/**, workbench/src/**, workbench/package.json, workbench/bun.lock, the install doc). No `push:` trigger (sxr rationale kept).
- `permissions: contents: read` (top). `persist-credentials: false` on every checkout; checkout `ref: inputs.ref || ''`.
- Concurrency: `package-${{ inputs.upload && 'release' || github.event_name }}-${{ inputs.ref || github.ref }}`; `cancel-in-progress: ${{ inputs.upload != true }}` — release builds (upload=true) get their own group and are never cancelled mid-run; candidates are event-scoped and may supersede each other.
- Header comments: honesty rule for workbench qualification + ci.yml-style SHA-pin supply-chain note.
- Jobs:
  - `linux` matrix: {ubuntu-22.04 / amd64 / x86_64-unknown-linux-gnu / bun-linux-x64}, {ubuntu-22.04-arm / arm64 / aarch64-unknown-linux-gnu / bun-linux-arm64} (native arm, never cross). rust stable (dtolnay), bun 1.4.0 (oven-sh), dpkg-dev+binutils apt install. CLI built in the frozen form; `--version` asserted. Workbench: `bun install --frozen-lockfile` + `bun run src/release-manifest.ts` (real-PTY qualified on both hosts; artifact + manifest existence asserted). Stage under installed names; ABI checks on both binaries; deb + tarball built; PostgreSQL provisioned (apt install, systemctl start, pg_isready wait, ALTER USER postgres PASSWORD 'postgres'); `install-test-deb.sh` run with `GAUNTLET_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:5432/postgres` and `GAUNTLET_REPO_ROOT=$GITHUB_WORKSPACE` (covers installed smoke, e2e-happy mock loop, serve + workbench connect, clean removal); tarball smoke via `smoke-gauntlet.sh` with `GAUNTLET_EXPECTED_VERSION`; `SHA256SUMS-linux-<arch>` + `built-from-linux-<arch>.txt`; upload `pkg-linux-<arch>` (error / 30d).
  - `macos` (macos-14): both darwin rust targets; aarch64 + x86_64 CLI slices built (x86_64 hard step per dispatch); arm64 workbench built + real-PTY qualified via release-manifest.ts; darwin-x64 workbench cross attempt (`bun install --frozen-lockfile` + `bun build --compile --target=bun-darwin-x64 ...`, continue-on-error, may fail because a host-arch bun install does not lay down @opentui/core-darwin-x64); variant step: x64 stage assembled only when the cross build succeeded → `universal`, else arm64-only → `arm64` label (`::warning` + honest job summary; the label always follows the actual stage content via packaging/macos/build.sh arg count); `sudo installer -pkg` + lipo records + `smoke-macos.sh` on both installed binaries (workbench leg included); always-cleanup (rm + pkgutil --forget); `SHA256SUMS-macos` + built-from; upload `pkg-macos`.
  - `windows` (windows-latest): frozen CLI build; workbench attempt via release-manifest.ts (host target bun-windows-x64; continue-on-error); on failure an honest fallback (`bun build --compile --target=bun-windows-x64 ... --outfile dist/gauntlet-workbench-bun-windows-x64.exe`, with a defensive normalize if Bun appends a second `.exe`, `::warning`, separate `pkg-windows-workbench-unqualified` upload incl. an UNQUALIFIED-README.txt); Inno Setup 6.3+ ensured (version-checked choco install/upgrade — build.ps1 asserts >= 6.3 and never upgrades in place); both exes staged under installed names; `build.ps1 -Version -BinDir -OutDir`; silent per-user install (`/VERYSILENT /SUPPRESSMSGBOXES /CURRENTUSER /LOG=`), file presence, fresh-pwsh PATH+version check, `smoke-windows.ps1` (workbench leg), silent uninstall + assertions that BOTH exes are gone and the user PATH entry is removed (with a 20s settle window for Inno's uninstaller handoff); `SHA256SUMS-windows` + built-from; uploads `pkg-windows` (+ `pkg-windows-workbench-unqualified` only on fallback); honest qualification record in the job summary.
  - `sum` (needs [linux, macos, windows], `if: always()`): download `pkg-*` (merge-multiple); verify every `built-from-*.txt` equals the built commit (github.sha for default-ref runs; the checked-out commit when `inputs.ref` is set — see assumptions); `sha256sum -c --strict` each platform SUMS; compgen-G presence assertions per successful platform (both debs, both tarballs, windows zip+setup.exe, macos pkg+dmg with either label); dpkg -c / tar -tzf workbench-inside-package checks for each linux deb/tarball; combined `SHA256SUMS` regenerated NUL-safely (`find -printf '%P\0' | sort -z | xargs -0 sha256sum`, written to a sibling then renamed — see deviations); artifact table (bytes + sha256) + CI-artifact download note in the job summary and in `SUMMARY.md`; explicit failure when zero platforms succeeded; upload `release-meta` (SHA256SUMS + built-from-*.txt + SUMMARY.md, 90d).

### release-native.yml

- `on`: push tags `gauntlet-v*`; workflow_dispatch with required `tag` input. Top `permissions: contents: read`; ci.yml-style SHA-pin header note; concurrency per ref, never cancelled.
- `validate`: resolves the tag (push → GITHUB_REF_NAME; dispatch → input, required; grammar gauntlet-v* enforced), checks out the tag, `sh packaging/version.sh "$TAG"` (exit 1 on drift) + explicit `test "$TAG" = "gauntlet-v${version}"`; outputs tag, version, tag_sha.
- `package`: `uses: ./.github/workflows/package.yml` with `ref: <tag>`, `upload: true`.
- `publish` (needs [validate, package]; `contents: write`, `id-token: write`, `attestations: write`): downloads `pkg-*` (merge-multiple) + `release-meta`; re-verifies every built-from file against the TAG COMMIT (>= 3 records required) and `sha256sum -c --strict SHA256SUMS`; presence-asserts all 8 asset classes; assembles notes.md (install pointer to docs/how-to/install-native-binaries.md, verification commands, attestation status, unsigned notice, artifact table + sha256s from SUMMARY.md); `actions/attest-build-provenance@v2` over the package files ONLY when `tag_sha == github.sha`, with an honest `::notice` skip on re-attach runs; `gh release create --verify-tag` (tag must already exist — tags are never created or moved here) or `gh release upload --clobber` when the release exists; final `if: failure()` recovery-notice step (idempotent re-run, --clobber, never move tags, cut a new tag instead).

## Interface cross-check (every invocation vs the actual scripts read)

| Workflow invocation | Script signature (as read) | Match |
|---|---|---|
| `sh packaging/version.sh` | `[expected]` → x.y.z; exit 1 non-numeric/mismatch | OK |
| `sh packaging/version.sh "$TAG"` (validate) | accepts `gauntlet-v<v>` / `v<v>` / `<v>` | OK |
| `cargo build --release --locked -p gauntlet-cli --features surfaces --bin gauntlet` (+ `--target aarch64|x86_64-apple-darwin` on macos) | frozen build form | OK |
| `cd workbench && bun install --frozen-lockfile && bun run src/release-manifest.ts` | writes `dist/gauntlet-workbench-<target>` (+`.exe` for windows, appended by Bun) + `dist/release-manifest.json`; exits nonzero when artifact/qualification missing for a host-executable target | OK (artifact names taken from `deriveReleaseTarget`/`releaseArtifactPath`, not guessed) |
| `bun build --compile --target=bun-darwin-x64 src/main.ts --outfile dist/gauntlet-workbench-bun-darwin-x64` | dispatch-mandated honest cross attempt | OK |
| `bun build --compile --target=bun-windows-x64 src/main.ts --outfile dist/gauntlet-workbench-bun-windows-x64.exe` | dispatch-mandated fallback; + defensive `.exe.exe` normalize/assert | OK |
| `sh packaging/check-linux-binary.sh stage/gauntlet "$ARCH" rust` | `<bin> <amd64|arm64> [rust|bun]` | OK |
| `sh packaging/check-workbench-binary.sh stage/gauntlet-workbench "$ARCH"` | `<bin> <amd64|arm64>` | OK |
| `sh packaging/deb/build.sh stage "$VERSION" "$ARCH" dist` | `<stage-dir> <version> <amd64|arm64> <out-dir>`; stage has gauntlet + gauntlet-workbench (installed names) | OK |
| `sh packaging/linux/build-tarball.sh stage "$VERSION" "$TARGET" dist` | `<stage-dir> <version> <triple> <out-dir>`; triples match the script's allowlist | OK |
| `sh packaging/install-test-deb.sh "dist/gauntlet_${VERSION}_${ARCH}.deb" "$RUNNER_TEMP/install-test"` | `<deb> <work-root>`; caller obligations honored: postgres provisioned + reachable BEFORE the script (it only probes), `GAUNTLET_DATABASE_URL` exported, `GAUNTLET_REPO_ROOT=$GITHUB_WORKSPACE`, curl/python3 present on runners | OK |
| `GAUNTLET_EXPECTED_VERSION=... sh packaging/smoke-gauntlet.sh <extracted>/gauntlet <extracted>/gauntlet-workbench` | `<gauntlet> [workbench] [expected-version]`; env `GAUNTLET_EXPECTED_VERSION` accepted (arg 3 alternative) | OK (env form used, per dispatch) |
| `sh packaging/macos/build.sh stage-arm64 ["stage-x64"] "$VERSION" dist` | `<arm64-stage> [<x64-stage>] <version> <out-dir>`; every stage dir must contain BOTH installed-name binaries — x64 stage is assembled only when the cross workbench build succeeded | OK |
| `sh packaging/smoke-macos.sh /usr/local/bin/gauntlet /usr/local/bin/gauntlet-workbench "$VERSION"` | `<gauntlet-bin> [workbench-bin] <expected-version>` (3-arg form) | OK |
| `packaging/windows/build.ps1 -Version $env:VERSION -BinDir "$PWD\stage" -OutDir "$PWD\dist"` | `-Version -BinDir -OutDir`; BinDir MUST contain gauntlet.exe AND gauntlet-workbench.exe | OK |
| `pwsh -NoProfile -File packaging/windows/smoke-windows.ps1 -Gauntlet ... -Workbench ... -ExpectedVersion $env:VERSION` | `param(Gauntlet, ExpectedVersion, Workbench)` | OK |
| `sudo installer -pkg ... -target /`, `pkgutil --pkg-info/--forget ai.godspeed.gauntlet` | identifier `ai.godspeed.gauntlet` taken verbatim from packaging/macos/build.sh | OK |

Artifact-name consistency between package.yml and release-native.yml: `pkg-linux-amd64`, `pkg-linux-arm64`, `pkg-macos`, `pkg-windows`, `pkg-windows-workbench-unqualified`, `release-meta` — download patterns/paths match the upload names exactly; `needs.validate.outputs.{tag,version,tag_sha}` are declared and consumed consistently.

YAML parse: `python3 -c "import yaml; yaml.safe_load(open(f))"` → **OK** for both workflows.

Additional empirical checks run locally (no cargo/bun): `set -e` counter idiom (`[ x = y ] && ok=$((ok+1))`) verified safe under `set -euo pipefail`; the combined-SUMS pipeline verified on dummy files (strict verify passes, no self-reference); grep-based extraction of every packaging invocation cross-checked against the scripts.

## Assumptions and CI-only items

1. **Windows installer packages physically carry the workbench even when unqualified** (surfaced contradiction, not silently resolved): the dispatch says the unqualified workbench must be "never inside the installer packages", but builder C's `packaging/windows/build.ps1` hard-requires `gauntlet-workbench.exe` in BinDir and `packaging/gauntlet.iss` installs it — a CLI-only windows package cannot be produced without editing files I do not own. Resolution implemented: build.ps1 invoked as contracted (packages carry both exes), AND the raw artifact is uploaded separately as `pkg-windows-workbench-unqualified` with an UNQUALIFIED-README.txt, AND the summary/warning/notes state explicitly that the installer binaries are packaging-contract, not a qualification claim. The plan's "never as qualified" is honored in presentation; honoring it physically needs a build.ps1 change (orchestrator decision). **[Superseded 2026-10-29 by the correction round below: the physical split is now implemented in package.yml against C's optional-workbench build.ps1 contract.]**
2. **sum's built-from == github.sha check** uses github.sha for default-ref runs and the checked-out commit when `inputs.ref` is set. Reason: for a manual dispatch against a tag (the re-attach path), github.sha names the dispatch trigger commit, not the artifacts' commit; comparing against github.sha there would fail honestly-built artifacts. The primary paths (PR, tag push via release-native) compare against github.sha exactly as dispatched. publish always binds to the tag commit.
3. **Combined SHA256SUMS written to a sibling file then renamed** instead of redirecting straight onto SHA256SUMS: the redirect creates the (empty) target before find(1) runs, so the file would hash itself into its own listing and `sha256sum -c --strict` would fail. Same command shape, same NUL-safety and LC_ALL=C ordering.
4. macOS x86_64 CLI slice is a hard step (dispatch: "build CLI twice"), while only the darwin-x64 workbench cross build is continue-on-error. If the CLI cross build ever fails, the macos job fails honestly (sxr treats it as best-effort; the dispatch for gauntlet does not).
5. `ubuntu-22.04` / `ubuntu-22.04-arm` / `macos-14` / `windows-latest` runner images: PostgreSQL preinstalled-but-stopped and pg_isready availability assumed (the workflow apt-installs + starts + waits anyway, so both image states work). Inno Setup >= 6.3 assumed provisionable via choco (checked, upgraded when older — mirrors sxr's proven step).
6. `gh release create --verify-tag` requires a gh version with that flag (present in current gh on runners); release-native publish does NOT pass `--target`, so it can never create a tag or move one.
7. Attestation (`attest-build-provenance@v2`) runs without a checkout in the publish job — it attests the downloaded files against the run context; this is the documented usage.
8. README "Install" section is 18 lines (dispatch: ~15 max) — kept as one cohesive block; trimmable if the orchestrator wants exactly 15.
9. CI-only (not verifiable locally): all runner behaviors — apt/postgres/systemctl, PTY legs, installer(8), ISCC, choco, artifact upload/download, OIDC attestation, gh release. Local validation was limited to YAML parsing, script-interface cross-checks, and shell-idiom dry runs.
10. Version currently resolves to 0.1.0 (numeric x.y.z — passes version.sh's prerelease gate). Tag grammar `gauntlet-v0.1.0` per the frozen naming.

## Residuals

- Residual 1 (from assumption 1): physical "never inside the installer packages" for the unqualified windows workbench requires a build.ps1/.iss change (builder C / orchestrator decision). Currently mitigated by labeling, not by exclusion. **[Closed 2026-10-29 by the correction round below.]**
- Residual 2: sum branches presence assertions on per-JOB results; a linux MATRIX partial failure (one arch green, one red) marks `needs.linux.result = failure`, so the green arch's presence assertion is skipped (its artifact is still checksum+commit-verified). Per-matrix-leg branching would need per-leg outputs, which matrix jobs cannot merge reliably.
- Residual 3: the zip payload of the windows package is not expanded-and-executed in CI (not in the dispatch list; the setup.exe path is fully install-tested and build.ps1 assembles the zip from the same staged exes).
- Residual 4: no cargo/bun caching in package.yml (not dispatched; first builds are the qualifying builds anyway).
- Residual 5: real CI qualification is pending — the orchestrator's dispatch/dispatch-watch loop is the actual gate; everything here is desk-checked against the landed scripts.

---

## Correction round (orchestrator, 2026-10-29)

Dispatch: split the windows fallback shape so the UNQUALIFIED workbench binary can never physically ride inside the installer packages — closing assumption 1 / Residual 1 above by changing `package.yml` (this slice's file), not the ruler or acceptance gates. Builder C concurrently makes `packaging/windows/build.ps1` + `packaging/gauntlet.iss` treat `gauntlet-workbench.exe` in BinDir as OPTIONAL (CLI-only zip/setup.exe when absent; workbench included when present, via a `/DIncludeWorkbench` define). Only `.github/workflows/package.yml` and this report were touched.

### C/E contract as implemented

BinDir CONTENT is the qualification signal: package.yml stages `gauntlet-workbench.exe` into BinDir only when `release-manifest.ts` qualification succeeded, and stages `gauntlet.exe` ONLY otherwise. package.yml never passes `/DIncludeWorkbench` itself — C's build.ps1/.iss derive the define from the exe's presence. LICENSE needs no staging (build.ps1 takes it from the repo root, confirmed by reading the script) and RELEASE-NOTES.txt is written into BinDir by build.ps1 itself, so the fallback BinDir is exactly `gauntlet.exe`. The unqualified `workbench/dist/gauntlet-workbench-bun-windows-x64.exe` is staged into a separate `wbu/` directory and uploaded only as `pkg-windows-workbench-unqualified` (+ rewritten UNQUALIFIED-README.txt) — it cannot enter the zip or setup.exe. When qualification succeeds: previous behavior (both exes in BinDir, installer carries the qualified workbench, no separate artifact).

### Changes in package.yml

1. Header honesty rule updated: fallback shape = CLI-only installers + separate artifact (stated physically, not just presentationally).
2. Fallback build step `::warning` text states the CLI-only outcome; new staging step (renamed honestly, branches on `steps.wbqual.outcome`) emits a `::notice` on the CLI-only path; job summary states BOTH branches explicitly (qualified: installer carries the qualified workbench, no separate artifact; fallback: CLI-only installers, unqualified binary only as `pkg-windows-workbench-unqualified`).
3. UNQUALIFIED-README.txt rewritten to state exactly what WAS validated (binary starts/runs under redirected pipes via smoke-windows.ps1's workbench start leg against the installed gauntlet.exe; the installed gauntlet.exe passed the full functional smoke) and what was NOT (real-PTY 120x40 qualification; no runner in this pipeline PTY-qualifies bun-windows-x64).
4. Install-test step is shape-aware: file-presence assertions include `gauntlet-workbench.exe` only when qualified, and the CLI-only shape additionally FAILS if `gauntlet-workbench.exe` is installed (contract-violation tripwire against C's .iss); the smoke workbench leg runs against the installed workbench when qualified and against the UNQUALIFIED dist binary (composed with the installed gauntlet.exe) when not — which is what earns the README's "starts" claim.
5. sum job windows assertions accept both shapes: unqualified shape = assert BOTH `dist/gauntlet-workbench-bun-windows-x64.exe` and `dist/UNQUALIFIED-README.txt` present (a partial artifact errors) and `unzip -l` must show NO `gauntlet-workbench.exe` in the installer zip; qualified shape = no unqualified artifact and `unzip -l` MUST show `gauntlet-workbench.exe`. Checksum verification untouched (the unqualified exe/README land in dist via the `pkg-*` merge-multiple download and are covered by the combined SHA256SUMS + artifact table as before).
6. Adjacent bug found while editing and fixed: the install-test step invoked `packaging\windows\smoke-windows.ps1`, but the script lives at `packaging\smoke-windows.ps1` — every windows run (either shape) would have failed at that step. Both call sites now use the real path.

### Validation performed (no cargo/bun)

- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/package.yml'))"` → OK (caught and fixed one invalid step name containing `: ` in an unquoted plain scalar).
- Artifact names re-verified mechanically: uploads are `pkg-linux-${{ matrix.arch }}` (amd64/arm64), `pkg-macos`, `pkg-windows`, `pkg-windows-workbench-unqualified` (conditioned on `steps.wbqual.outcome != 'success'`, paths `wbu/gauntlet-workbench-bun-windows-x64.exe` + `wbu/UNQUALIFIED-README.txt`), `release-meta`; sum downloads `pkg-*` merge-multiple so the unqualified shape is detectable file-level as `dist/gauntlet-workbench-bun-windows-x64.exe` + `dist/UNQUALIFIED-README.txt` (no name collision with any other artifact's contents). release-native.yml's publish presence assertions and asset globs pass in both shapes unchanged.
- `bash -n` on the extracted sum inline scripts → OK; the `flag=0; have X && flag=1` counter idiom is the same shape proven safe under `set -euo pipefail` earlier in this report.
- Functional dry-run of the sum windows block against five synthetic fixtures: both valid shapes pass, and all three violations fail (partial unqualified artifact; workbench inside the zip while the unqualified artifact exists; no artifact while the zip lacks the workbench).

### Residuals / adjacent notes for this round

- The split depends on C's optional-workbench build.ps1/.iss landing; until then the old hard-requiring build.ps1 fails the windows job (or sum's unzip check) honestly instead of shipping the unqualified binary inside the installer.
- unzip(1) on ubuntu-latest reads the pwsh 7 Compress-Archive zip (forward-slash entry names); the assertion matches the archive-root basename, so the check is layout-independent.
- Adjacent (NOT touched, owner: release-native.yml): on the fallback shape the GitHub Release would attach a CLI-only zip/setup.exe without attaching the unqualified workbench binary or stating the shape in the release notes (publish's notes assembly reads SUMMARY.md, which does include the artifact table rows) — follow-up for that file's owner if the fallback shape should be more visible on the release page.
- Real CI qualification still pending (Residual 5 above); this round is desk-checked + locally dry-run only.
