# Slice B report — Linux packaging (builder B)

Branch: `release/native-distribution`. Date: 2026-10-29. Source commit at dispatch: 1543cbf (main).
No commits made, nothing pushed, no cargo/bun runs (per dispatch constraints).

## Files created (all NEW, all inside `packaging/`)

| File | Role |
|---|---|
| `packaging/version.sh` | x.y.z from root Cargo.toml `[workspace.package] version` (awk, section-tracked, no network); refuses non-numeric x.y.z; with an argument refuses mismatch (accepts `<v>`, `v<v>`, `gauntlet-v<v>`) |
| `packaging/check-linux-binary.sh` | readelf ABI gate: ELF64, machine (X86-64/AArch64), interpreter (`/lib64/ld-linux-x86-64.so.2` / `/lib/ld-linux-aarch64.so.1`), EXEC/DYN, no RPATH/RUNPATH, no `/nix/store`, NEEDED allowlist = glibc family (+ `libresolv.so.2` for kind=bun only), rejects GLIBCXX_/CXXABI_, max GLIBC_ <= 2.35 (Jetson Ubuntu 22.04) |
| `packaging/check-workbench-binary.sh` | thin wrapper: `exec check-linux-binary.sh <bin> <arch> bun`; kept as its own entrypoint because the plan contract and CI invoke exactly this name |
| `packaging/deb/build.sh` | `gauntlet_<v>_<arch>.deb` via `dpkg-deb --build --root-owner-group` from an assembled control tree; no cargo-deb/nfpm/fpm |
| `packaging/deb/control` | control-file skeleton; build.sh substitutes `@VERSION@ @ARCH@ @INSTALLED_SIZE@ @DEPENDS@` |
| `packaging/deb/README.md` | packaged-runtime notes source; installed verbatim as `/usr/share/doc/gauntlet/README` |
| `packaging/linux/build-tarball.sh` | reproducible `gauntlet-<v>-<triple>.tar.gz` (`--sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner | gzip -9n`); rebuilds and byte-compares before declaring success |
| `packaging/smoke-gauntlet.sh` | out-of-checkout smoke: version/help/doctor/init legs + workbench real-PTY legs (embedded python3 helper, explicit winsize) |
| `packaging/install-test-deb.sh` | full install/exercise/remove acceptance cycle (CI-qualified; see below) |

## Contract adherence notes

- Argument orders are the frozen interface: `deb/build.sh <stage-dir> <version> <amd64|arm64> <out-dir>`;
  `build-tarball.sh <stage-dir> <version> <triple> <out-dir>`; `smoke-gauntlet.sh <gauntlet> [workbench] [expected-version]`.
- `smoke-gauntlet.sh` expected version: argument 3 or env `GAUNTLET_EXPECTED_VERSION` (documented in
  header and in the typed usage error; the script runs outside the checkout so it cannot read Cargo.toml).
- Repo root resolution: `GAUNTLET_REPO_ROOT` when set and non-empty, else derived from the script's own
  path (`packaging/deb/build.sh` -> `../../`). Documented in the script header.
- Deb: package `gauntlet`, Section utils, Priority optional, Architecture amd64|arm64, `Depends`
  computed with `dpkg-shlibdeps -O` (union across both staged binaries, `libc6` presence asserted),
  `Recommends: postgresql, bubblewrap`, `Installed-Size` present. Installs `/usr/bin/gauntlet`
  (+ `/usr/bin/gauntlet-workbench` when staged) 0755 root:root, `/usr/share/doc/gauntlet/copyright`
  (repo-root LICENSE), `/usr/share/doc/gauntlet/README`. NO postinst, NO systemd units, no service
  enablement, no conffiles.
- Jetson guard: a computed `libc6 (>= X)` constraint above 2.35 fails the build ("build on Ubuntu
  22.04") — dpkg-shlibdeps on a newer host would otherwise emit an uninstallable-on-Jetson Depends.
- Post-build assertions against the ARTIFACT (not the tree): `dpkg-deb --field` name/version/arch/
  Installed-Size; `dpkg-deb --contents` lists each binary with `-rwxr-xr-x root/root` and docs with
  `-rw-r--r-- root/root`.
- Tarball: top dir `gauntlet-<v>-<triple>/` with staged binaries 0755, LICENSE, generated README.txt
  (install to ~/.local/bin or /usr/local/bin; GAUNTLET_DATABASE_URL REQUIRED with example; bwrap/bun
  optional with honest typed-refusal note; GAUNTLET_ENDPOINT + `gauntlet serve`/GAUNTLET_API_BIND
  workbench notes; project-root/cwd + GAUNTLET_REPORT_DIR semantics). No network at package time
  anywhere.
- Smoke legs: (1) `--version` prints exactly `gauntlet <expected>`; (2) `--help` exit 0 + `^usage:`;
  (3) `doctor` exit 0 in an env-only dir, prints `codemode (optional):` and `report: <path>`, report
  file exists and is non-empty; (4) `init` syntax probed via `init --help` (init refuses arguments and
  prints `usage: gauntlet init` — verified in crates/gauntlet-cli/src/init_cmd.rs), then plain `init`
  scaffolds GAUNTLET.md + .env.example + `.gauntlet/`; (5) workbench real PTY 120x40 (TERM=xterm-256color,
  winsize set explicitly via TIOCSWINSZ — CI has no controlling terminal): NAVIGATOR/PRIMARY/INSPECTOR/
  COMPOSER within 30s, bare `q`, exit 0 within 15s; 80x24 leg: alive after 3s (clean start) + clean
  `q` exit 0, layout not over-asserted. Failure path dumps the last ~1500 chars of PTY output. The
  helper never kills the TUI before the quit deadline (mirror of workbench/src/release-manifest.ts
  launch qualification; its helper uses Bun's terminal API, mine python3 pty — the runner's python3).
- install-test-deb.sh: typed fail-fast when caller obligations are missing (GAUNTLET_DATABASE_URL /
  GAUNTLET_REPO_ROOT / sudo / curl / python3) — it provisions nothing itself; postgres URL parse +
  TCP reachability probe before any dpkg work; `dpkg -i` with `apt-get install -y <deb>` fallback;
  `dpkg -L` assertions (binaries present+executable, docs present, no world-writable regular files,
  Installed-Size); smoke vs `/usr/bin/gauntlet` (+ workbench when the deb carries it) with
  GAUNTLET_EXPECTED_VERSION taken from the deb's own Version field; functional leg mirrors the proven
  e2e mock harness env exactly (crates/gauntlet-cli/tests/e2e_full_loop_mock.rs: GAUNTLET_STATE_DIR,
  GAUNTLET_ARTIFACT_DIR, GAUNTLET_MOCK_SCENARIO=e2e-happy, GAUNTLET_ID_SEED, GAUNTLET_CLOCK=
  deterministic, stall/retry bounds, GAUNTLET_TOOL_TEST=<project>/scripts/check.sh chmod 0755) over a
  `cp -a` of tests/fixtures/target; run -> `run: run_id=` extraction -> `status --run` with
  `settled=[1-9][0-9]*` assertion; `serve` on a free loopback port via GAUNTLET_API_BIND (exec'd so
  $! is the server), curl `/api/v1/capabilities` (body must contain `"capabilities"`) and
  `/api/v1/state` with 30s deadline; workbench connect leg in real PTY 120x40 with
  GAUNTLET_ENDPOINT + 3s dwell, then the connection proof: a `surface: "GET" "/api/v1/…" -> 200`
  access-log line (format verified at
  crates/adapters/gauntlet-adapter-surface-json/src/http.rs:962; client fetch paths verified at
  workbench/src/client.ts:131,301) strictly AFTER the curl legs' log offset; serve stopped via SIGINT
  (the documented "serving until interrupted" path), TERM/KILL escalation, process-gone assertion;
  `apt-get remove -y` (+ `dpkg -r` fallback, `dpkg -P` when still registered), then binaries and
  `/usr/share/doc/gauntlet` gone and `dpkg -s gauntlet` fails.

## Runtime facts verified in source (not assumed)

- Version authority: root `Cargo.toml` line 51 (`[workspace.package] version = "0.1.0"`).
- `GAUNTLET_API_BIND` required by `gauntlet serve` (crates/gauntlet-cli/src/serve_cmd.rs:23, loopback-only).
- `GAUNTLET_REPORT_DIR` honoured by doctor without GAUNTLET.md (crates/gauntlet-app/src/doctor/mod.rs:507);
  doctor prints `codemode (optional):` and `report: <path>` (crates/gauntlet-cli/src/doctor_cmd.rs:61-64).
- `init` refuses any argument, scaffolds GAUNTLET.md/.env.example/.gauntlet, refuses overwrite (init_cmd.rs).
- `GAUNTLET_PROJECT_ROOT` is a real config key (crates/gauntlet-app/src/config/resolve.rs:1737) — README
  claims it honestly.
- Workbench endpoint env: `GAUNTLET_WORKBENCH_ENDPOINT` ?? `GAUNTLET_ENDPOINT` (workbench/src/config.ts:238).
- Status output carries `settled=N` (asserted in e2e_full_loop_mock.rs:476-480 for the fixture: 3).

## Local validation evidence (WSL2 Ubuntu-ish host; mechanics only)

All commands run from the repo root; every script passed `shellcheck` (clean) and `dash -n`.

- `./packaging/version.sh` -> `0.1.0`; `v0.1.0` / `gauntlet-v0.1.0` / `0.1.0` -> `0.1.0`;
  `9.9.9` -> `error: requested release '9.9.9' does not name the workspace version '0.1.0'`, exit 1.
- ABI gate: `/bin/true amd64` (rust and bun kinds) PASS (`max GLIBC_2.34`, NEEDED libc.so.6);
  `/bin/true arm64` FAIL on machine; `/bin/bash amd64` FAIL on `libtinfo.so.6` (outside allowlist);
  kind `go` and arch `riscv64` -> usage-class exit 2.
- Deb build (fake stage: `/bin/true` as both installed names): built `gauntlet_0.1.0_amd64.deb`;
  `dpkg-deb --field` Package/Version/Architecture/Installed-Size all asserted; `dpkg -c` shows
  `-rwxr-xr-x root/root` binaries and `-rw-r--r-- root/root` docs; Depends `libc6 (>= 2.34)`;
  Recommends `postgresql, bubblewrap`. Single-binary (no workbench) variant assembles; arm64 machine
  gate correctly refuses an amd64 fake (exit 1, fired from inside deb/build.sh); `0.1.0-rc1` and
  `riscv64` refused with exit 2.
- Tarball: dual-binary and single-binary variants built and listed correctly; reproducibility proven
  two ways — in-process double build with `cmp`, and identical sha256 across two separate invocations
  (`b6848978…ba4d1` for the fake-content x86_64 archive).
- Smoke harness: fake `gauntlet` (version/help/doctor/init legs) + fake raw-mode TUI (markers + quit
  on bare `q`) -> ALL CHECKS PASSED including both PTY legs. Negative paths: version mismatch -> FAIL
  exit 1; missing expected version -> exit 2; TUI exiting 3 on `q` -> FAIL with dump; TUI without
  markers -> FAIL after the 30s deadline with 1500-char dump; temp roots cleaned (no `/tmp/gauntlet-smoke.*`
  left). One honest catch during validation: the first fake TUI read in canonical mode and could not
  quit on a bare `q` — the helper correctly timed out and killed it; the fake was fixed to raw mode
  (real TUIs quit on bare `q` per release-manifest.ts:173).
- install-test-deb.sh fail-fast (no sudo touched): missing args -> exit 2; missing GAUNTLET_DATABASE_URL
  -> typed FAIL exit 1; URL-parse probe rejects non-postgres schemes; reachability probe fails in ~5s
  against a dead 127.0.0.1:54329; `pty_connect.py` extracted from its heredoc, `ast.parse`-clean, and
  passes against the fake TUI with GAUNTLET_ENDPOINT set.

## CI-only items (desk-checked, not locally executable)

- `packaging/install-test-deb.sh` full cycle: needs sudo dpkg/apt, the real installed deb, and a live
  PostgreSQL — CI-qualified only. Its every assertion was desk-checked against the CLI's actual output
  formats (`run: run_id=`, `settled=N`, access-log line shape) and its fail-fast paths were exercised
  locally where possible.
- `smoke-gauntlet.sh` / PTY legs against a REAL gauntlet + gauntlet-workbench binary (none built here
  by design; the harness itself was qualified against fakes).
- `libresolv.so.2` admission for a real Bun standalone (fake binaries link libc only) and the
  GLIBC > 2.35 rejection (no >2.35 binary available locally; logic is sxr-proven `sort -V`).
- The `libc6` constraint the REAL gauntlet binaries produce (fake stage yields >= 2.34; CI builds on
  ubuntu-22.04 where the guard is expected to hold).

## Residuals

1. `.agents/CURRENT_STATUS.yml` was NOT updated by this slice: the dispatch scoped my ownership to new
   `packaging/` files plus this report, and the file is concurrently modified by other builders —
   orchestrator reconciliation per AGENTS.md §4 applies.
2. `settled=` assertion in install-test-deb.sh requires a POSITIVE count (the canonical fixture
   currently settles 3 — pinned by e2e_full_loop_mock.rs); a stricter `settled=3` was deliberately
   not hardcoded so fixture evolution does not false-fail packaging CI. If the orchestrator wants the
   stricter pin, it is a one-line change.
3. The workbench-connection proof depends on the workbench issuing GET /api/v1/state|capabilities at
   startup (verified in workbench/src/client.ts + canonical-surface-stream-port.ts today). If the
   workbench ever renders without contacting the surface, this assertion fails honestly — that is the
   intended tooth, not a bug.
4. `packaging/deb/control` and `packaging/deb/README.md` hardcode the homepage
   `https://github.com/GodSpeedAI/Gauntlet` (derived from the origin remote). If the repo moves, update
   both plus the shlibs probe inside deb/build.sh.
5. version.sh accepts `v`- and `gauntlet-v`-prefixed arguments (superset of the frozen "<version>"
   contract) to serve the release-native tag validation (Slice E validates tags `gauntlet-v*`).
6. The smoke's doctor leg unsets caller Gauntlet env (DATABASE_URL, PROJECT_ROOT, etc.) to guarantee
   the nothing-configured env-only scenario; if a future doctor treats "nothing configured" as
   non-zero, CI will fail honestly — that would be a product-behavior question, not a packaging one.
