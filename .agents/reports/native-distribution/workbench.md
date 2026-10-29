# Slice W — Workbench multi-target release builder (report)

Builder: W. Date: 2026-10-29. Branch: `release/native-distribution` (from main @ 1543cbf).
Plan: `.agents/plans/native-distribution.md` (Slice W). File scope honored:
`workbench/src/**` + `scripts/tui-preflight.sh` (E6 only). No cargo run; no commits.

## Files changed

| File | Change |
|---|---|
| `workbench/src/release-manifest.ts` | Multi-target release builder: host-derived target, override, target-qualified artifact path, honest two-shape launch qualification, verifier extensions, target-aware support facts, legacy-artifact cleanup. |
| `workbench/src/release-manifest.test.ts` | Constant pins replaced by derivation/mapping unit pins (all 5 host triples + fail-closed cases + `.exe` naming); new contract tests for both launch shapes, target/artifact consistency, unknown-target rejection, geometry pin, digest-drift detection. |
| `workbench/src/bun-native-smoke.test.ts` | Attack 1 `ADMITTED_PROD` extended with `quickjs-wasi` (justified inline; attack strength kept). |
| `scripts/tui-preflight.sh` | E6 row only: follows the manifest-recorded target-qualified artifact path (no hard-coded `dist/gauntlet-workbench`), probe now distinguishes `MANIFEST_OK` / `MANIFEST_UNQUALIFIED` / `MANIFEST_INVALID`; fails closed on unqualified artifacts. |
| `.agents/reports/gauntlet-tui-prime-workbench/evidence/T20/frames/release-artifact-120x40.txt` | Refreshed by the build (existing T20 behavior, unchanged path). |

## Target derivation (pure function, unit-pinned)

`deriveReleaseTarget(platform, arch)` — fails closed (throws) for anything else:

| Host triple | Target | Artifact (repo-relative) |
|---|---|---|
| linux / x64 | `bun-linux-x64` | `dist/gauntlet-workbench-bun-linux-x64` |
| linux / arm64 | `bun-linux-arm64` | `dist/gauntlet-workbench-bun-linux-arm64` |
| darwin / arm64 | `bun-darwin-arm64` | `dist/gauntlet-workbench-bun-darwin-arm64` |
| darwin / x64 | `bun-darwin-x64` | `dist/gauntlet-workbench-bun-darwin-x64` |
| win32 / x64 | `bun-windows-x64` | `dist/gauntlet-workbench-bun-windows-x64.exe` (Bun appends `.exe`; verified against the pinned Bun 1.4.0 toolchain) |

- `resolveReleaseTarget(override, platform, arch)`: `GAUNTLET_WORKBENCH_RELEASE_TARGET`
  override, validated against `KNOWN_RELEASE_TARGETS` (the 5 host-derived targets plus
  override-only `bun-linux-x64-musl` / `bun-linux-arm64-musl`); unknown values fail closed
  before any build. `cross_build` is true iff the resolved target differs from the host target.
- musl is NOT auto-derived (no reliable detection from this builder); the glibc assumption is
  recorded as a `support_facts` entry on every `bun-linux-*` manifest, and musl targets are
  reachable only through the explicit override.

## Manifest contract (extended, never weakened)

All existing fields kept (`artifact`, `artifact_sha256`, `artifact_bytes`, `entry`, `target`,
`bun_version`, `lockfile`, `lockfile_sha256`, `dependencies`, `dev_dependencies`,
`build_form: "bun-compile-standalone"`, `launch_qualification`, `support_facts`,
`built_at_utc`). Additions/changes:

- `artifact` is now the target-qualified path; `verifyReleaseManifest` enforces
  `artifact === releaseArtifactPath(target)` and that `target` is a known target.
- `launch_qualification` is a union. Host-executable targets keep the REAL record:
  `{transport: "real-pty", geometry: "120x40", rendered_markers: [...], exited_cleanly: true,
  exit_code: 0}` (geometry now checked, not just carried).
- Honest cross record shape (built + hashed, never launch-qualified):
  `{transport: "not-qualifiable-on-this-host", reason: "<why>..."}` — reason must be a
  non-empty string. `verifyReleaseManifest` returns a new `launch_qualified: boolean` that is
  true ONLY for a well-formed real-pty 120x40 clean-exit record; an honest not-qualifiable
  manifest verifies as a manifest but can never be presented as qualified (E6 fails closed on
  it). Fake transports (`"assumed"`, etc.) still fail as before.
- Digest re-check over the on-disk artifact kept (drift test added).
- `main()` removes the superseded unsuffixed `dist/gauntlet-workbench` after a successful build.

## quickjs-wasi smoke fix (before/after)

- BEFORE (recorded): `bun test src/bun-native-smoke.test.ts` → 6 pass / 1 fail —
  Attack 1 failed: `Expected to contain: "quickjs-wasi"` in `ADMITTED_PROD`
  (`["@opentui/core","@opentui/react","react","react-reconciler"]`).
- AFTER: Attack 1 passes; full default suite green. Justification: `quickjs-wasi` 3.6.2 is a
  real prod dependency (package.json since 914c538) imported by
  `workbench/src/codemode/sandbox-worker.ts` (`QuickJS`, `JSValueHandle`, `quickjs.wasm`).
  The attack's strength is preserved: every prod dependency in package.json must still appear
  in the list, so any future undeclared dependency fails until explicitly qualified.

## Gate outputs (all run from repo root)

| Gate | Result |
|---|---|
| `just tui-check` | PASS (`tsc --noEmit`, no output). |
| `just tui-test` (default suite) | **1073 pass / 0 fail / 32 skip** (env-gated batteries) across 97 files. Before the quickjs-wasi fix the suite was red (Attack 1). |
| `just tui-lint` | exit 0 — 89 warnings / 616 infos. Baseline was 89 warnings / 638 infos: my touched files went to ZERO findings (they carried ~23 pre-existing `useLiteralKeys` infos at HEAD); no new findings anywhere. The 6 baseline noConsole warnings remain, all in untouched `src/bun-substrate.bench.ts`. |
| `just tui-preflight bootstrap` | **PASS (all 6 rows green, exit 0)** after the correction-round lockfile bump (see below). Before it, the row failed on a NEW critical advisory (GHSA-pqg4-j6r4-53mv) that pre-existed my changes (deps byte-identical to HEAD). |
| `just tui-build release` | PASS, verbatim: `tui-build release: bun 1.4.0, target bun-linux-x64 (host-derived)` / `built dist/gauntlet-workbench-bun-linux-x64` / `OK — dist/gauntlet-workbench-bun-linux-x64 sha256=1e64791f98678bab… launch 120x40 exit 0, manifest dist/release-manifest.json` — re-run after the lockfile bump with the identical artifact digest (shell-quote is not bundled) and the refreshed `lockfile_sha256`. |
| `bun test src/release-manifest.test.ts src/production-entry.test.ts` (focused) | 15 pass / 0 fail (post-build manifest asserts `valid` AND `launch_qualified`). |
| `bun test src/release-manifest.test.ts src/bun-native-smoke.test.ts` (focused) | 21 pass / 0 fail. |

## Correction round (orchestrator dispatch: clear the bootstrap bun-audit failure)

- BEFORE: `bun audit` → 1 vulnerability (1 critical): `shell-quote@1.10.0` via
  `@opentui/react > react-devtools-core`, GHSA-pqg4-j6r4-53mv, affected `>=1.8.4 <1.11.0`
  (saved: `/tmp/audit-before.txt` if still present; verbatim in the previous revision of this
  report). Deps were byte-identical to HEAD — pre-existing, environmental.
- FIX: `bun update shell-quote` in `workbench/` → `shell-quote 1.10.0 -> 1.12.0` (outside the
  affected range, still satisfying `react-devtools-core`'s `^1.6.1`). **`package.json`
  unchanged** (verified: empty diff; all pins remain exact), only `bun.lock` moved. The
  preflight pinned-exact rule still passes. No gate was weakened; a patched version existed
  upstream so no audit-row change was needed.
- AFTER: `bun audit` → `No vulnerabilities found (checked 62 packages)`, exit 0.
  `bun install --frozen-lockfile --dry-run` still in sync.
- Gate re-runs after the lockfile change: `just tui-preflight bootstrap` **PASS**;
  `just tui-check` PASS; `just tui-test` **identical: 1073 pass / 32 skip / 0 fail** (no
  behavioral perturbation); `just tui-lint` unchanged (89 warnings / 616 infos);
  `just tui-build release` re-run so the manifest's `lockfile_sha256` matches the updated
  `bun.lock` (verified YES; artifact sha256 byte-identical `1e64791f…` — shell-quote is a
  transitive of react-devtools-core and is not imported by the release entry, so it is not in
  the standalone bundle). Focused suites re-run: 0 fail.

Rebuilt artifact: `workbench/dist/gauntlet-workbench-bun-linux-x64`, sha256
`1e64791f98678bab90cc99053487899441d2ba472cbb40e0391d054cfea0d32c`, 100,406,472 bytes.
Manifest dependency list matches package.json exactly (`@opentui/core`, `@opentui/react`,
`quickjs-wasi` 3.6.2, `react`, `react-reconciler`). Lockfile digest re-recorded
(`e218c883…`). Frame evidence refreshed (2026-10-29 14:01). Old unsuffixed
`dist/gauntlet-workbench` removed by the build. E6 probe verified inline: `MANIFEST_OK`
(E6 itself is a conformance row; conformance was not run, per task scope).

## Cross-target behavior (probed on this host, recorded honestly)

- `GAUNTLET_WORKBENCH_RELEASE_TARGET=made-up-target` → fails closed before any build:
  "not a known Bun standalone-compile target (known targets: …)". No artifact/manifest touched.
- `GAUNTLET_WORKBENCH_RELEASE_TARGET=bun-windows-x64` and `=bun-linux-arm64` → the wiring
  works (banner "env-override, cross build — no PTY qualification on this host") but
  `bun build --compile` fails closed: `Could not resolve: "@opentui/core-win32-x64"` /
  `"@opentui/core-linux-arm64(-musl)"` — a host only installs its own platform package of
  @opentui/core 0.5.11 (optionalDependencies; all 8 in bun.lock). No artifact, no manifest,
  previous dist state preserved. This matches the plan's decision to build ON native runners
  (CI jobs); the not-qualified record shape is exercised by unit tests.

## Residuals / adjacent issues

1. ~~`just tui-preflight bootstrap` fails at its `bun audit` row~~ RESOLVED in the correction
   round: `bun update shell-quote` moved the transitive to 1.12.0 (patched), `package.json`
   untouched, bootstrap now fully PASS. No gate text was changed for this.
2. Full cross-target bundling of the workbench entry requires the TARGET platform's
   `@opentui/core-<os>-<arch>` optional dependency to be installed (or the build to run on a
   native runner). CI (Slice E) builds natively per platform, so this is documentation-grade;
   noted so nobody expects x64→win/arm64 cross builds to succeed on a bare host.
3. `justfile` comment for `tui-build release` still says "pinned target (bun-linux-x64)";
   behavior is unchanged and the recipe needed no edit (out of my file scope).
4. `workbench/.gauntlet/projections.db` shows a content diff — side effect of running the
   required `just tui-test` battery against the real sqlite projection store (runtime state,
   not a source change); left as-is for orchestrator reconciliation.
5. Windows-target artifact naming (.exe) was verified via a throwaway hello-world cross build
   in /tmp (Bun appends `.exe` even when `--outfile` omits it); a full windows artifact could
   not be produced here (Residual 2).
