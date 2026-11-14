# How-To: Install Precompiled Gauntlet Binaries

This guide installs a released `gauntlet` CLI (the `surfaces` build) and the
`gauntlet-workbench` operator TUI from precompiled native binaries — no Rust
toolchain and no Bun runtime required on the target machine. It also covers the
NVIDIA Jetson AGX Orin (Ubuntu 22.04, arm64, glibc 2.35), which is the oldest
Linux ABI the release supports.

Every release carries these assets on the GitHub Release tagged
`gauntlet-v<version>`:

| Platform | Assets |
|---|---|
| Linux amd64 | `gauntlet_<version>_amd64.deb`, `gauntlet-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 | `gauntlet_<version>_arm64.deb`, `gauntlet-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| macOS | `gauntlet-<version>-macos-universal.pkg` / `.dmg` (or `-macos-arm64` when the x86_64 CLI slice could not be built; the label describes the CLI — the workbench inside is arm64-only in every shape) |
| Windows x64 | `gauntlet-<version>-x86_64-pc-windows-msvc-setup.exe`, `.zip` |
| All | `SHA256SUMS`, `built-from-<os>.txt` (the exact commit each artifact was built from) |

Every Linux package contains both binaries (`gauntlet` and
`gauntlet-workbench`), and that workbench is real-PTY-qualified on its native
Linux build runner. Everywhere else, the package shape follows what the
runner actually qualified:

- **macOS** — the CLI inside the pkg is a universal (arm64 + x86_64) lipo or
  arm64-only; the **workbench inside the pkg is arm64-only in every shape**
  and is real-PTY-qualified on the arm64 runner. A cross-compiled Intel
  (`bun-darwin-x64`) workbench is never placed inside the pkg; when one was
  built it is published only as the separate `pkg-macos-workbench-unqualified`
  CI artifact with an explanatory README.
- **Windows** — two honest shapes. Qualified: the zip and setup.exe carry
  `gauntlet-workbench.exe`, built and launch-qualified on the Windows runner.
  Fallback (when PTY qualification is unavailable on the Windows runner): the
  zip and setup.exe are **CLI-only** — they do NOT contain the workbench —
  and the unqualified binary is published separately as
  `pkg-windows-workbench-unqualified` with a README stating exactly what was
  and was not validated. No runner in this pipeline PTY-qualifies
  `bun-windows-x64`.

An unqualified artifact is never presented as qualified; TUI launch
qualification never transfers between runners.

Packages are **unsigned**: there is no Apple Developer ID and no Authenticode
certificate. Verify the checksum before installing.

---

## Get the artifacts

Formal release assets (tagged releases):

```sh
gh release download gauntlet-v<version> --repo GodSpeedAI/Gauntlet
```

CI artifact channel (release candidates from a `package.yml` run; each
artifact group is one platform, e.g. `pkg-linux-arm64`):

```sh
gh run download <run-id> --repo GodSpeedAI/Gauntlet -n pkg-linux-arm64
```

Unqualified workbench artifacts (`pkg-macos-workbench-unqualified`,
`pkg-windows-workbench-unqualified`) appear only on CI runs — never as
release assets — and their READMEs state exactly what was and was not
validated.

---

## Runtime prerequisites

**PostgreSQL is REQUIRED.** `gauntlet run/status/inspect/verify/export/serve`
need the state store. Two things to know, stated honestly:

- `gauntlet doctor` does **not** probe PostgreSQL. A missing server is
  reported as such — never simulated — so a green `doctor` does not mean the
  database is reachable.
- The built-in default URL,
  `postgresql://postgres:postgres@127.0.0.1:54329/postgres`, is a **developer
  instance** on port 54329 that will not exist on your machine. Always set
  `GAUNTLET_DATABASE_URL` explicitly.

Point it at a real server, for example:

```sh
export GAUNTLET_DATABASE_URL=postgresql://gauntlet:secret@localhost:5432/gauntlet
```

or set `GAUNTLET_DATABASE_URL` in the project `.env` (`gauntlet init`
scaffolds `.env.example` as the template).

Optional capabilities — reported honestly by `gauntlet doctor`, never faked:

- **bubblewrap**: enables non-`none` sandbox backends. The default backend is
  `none`; requesting another backend without bwrap is a typed refusal, never a
  silent fallback.
- **bun**: needed only by the codemode tool-program host. The workbench
  executable is self-contained (Bun compiled standalone) and does not need it.
- **prime-agent**: the delegation harness; when absent, `doctor` reports it and
  delegation stays unavailable.

Commands resolve the Gauntlet project from the current working directory (the
directory holding `GAUNTLET.md`; `GAUNTLET_PROJECT_ROOT` overrides it).
`gauntlet init` scaffolds a fresh project.

---

## Verify the download

Linux and Windows (Git Bash / WSL):

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

macOS (`sha256sum` is not part of a stock install):

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

Each `built-from-<os>.txt` names the exact commit the artifacts were built
from; compare it with the release notes. Where GitHub CLI is available, you
may also check the build provenance attached at publish time:

```sh
gh attestation verify <asset> --repo GodSpeedAI/Gauntlet
```

This only succeeds when the release carries provenance. Provenance is attached
when the publishing run itself built the tagged commit; a release whose assets
were re-attached later (a workflow_dispatch against an existing tag) honestly
has none. `SHA256SUMS` is always present and is the baseline check.

---

## Linux — .deb (recommended)

```sh
sudo apt-get install -y ./gauntlet_<version>_amd64.deb   # or _arm64.deb
gauntlet --version                # expect: gauntlet <version>
```

The package installs `/usr/bin/gauntlet`, `/usr/bin/gauntlet-workbench`, and
the documentation under `/usr/share/doc/gauntlet/`. It has **no postinst**,
no services, and enables nothing. It *Recommends* `postgresql` and
`bubblewrap`, so a default `apt-get install` pulls those packages in — that
installs the software, not the configuration: you still point
`GAUNTLET_DATABASE_URL` at a reachable server yourself (see
[Runtime prerequisites](#runtime-prerequisites)).

To keep the install minimal:

```sh
sudo apt-get install -y --no-install-recommends ./gauntlet_<version>_amd64.deb
```

## Linux — tarball (no package manager)

```sh
tar -xzf gauntlet-<version>-x86_64-unknown-linux-gnu.tar.gz
install -m 0755 gauntlet-<version>-x86_64-unknown-linux-gnu/gauntlet ~/.local/bin/
install -m 0755 gauntlet-<version>-x86_64-unknown-linux-gnu/gauntlet-workbench ~/.local/bin/
gauntlet --version
```

The archive carries a `README.txt` with the same install and
prerequisites notes. Both binaries are glibc-linked and need glibc 2.35 or
newer.

### Launching the operator surface

```sh
export GAUNTLET_DATABASE_URL=postgresql://...      # required, see above
export GAUNTLET_API_BIND=127.0.0.1:4000            # the §20.2 operator surface, loopback only
gauntlet serve                                     # in a project directory
```

Then connect the workbench (another terminal):

```sh
export GAUNTLET_ENDPOINT=http://127.0.0.1:4000
gauntlet-workbench                                 # in a writable working directory
```

The workbench is a router/projection over the operator surface: it connects
through the gateway, never bypasses it. It needs a writable working directory
for its projection store (or set `GAUNTLET_WORKBENCH_PROJECTION_STORE`) and
honors `GAUNTLET_WORKBENCH_GAUNTLET_BINARY` to point at your CLI explicitly.

---

## NVIDIA Jetson AGX Orin (Ubuntu 22.04, arm64, glibc 2.35)

The arm64 artifacts are built **natively** on Ubuntu 22.04 arm64 runners —
never cross-compiled — and are gated on `GLIBC_2.35`, which is exactly the
Jetson's Ubuntu 22.04 baseline. They need no cross-environment setup on the
Jetson.

### Install

```sh
cd ~/Downloads
sha256sum -c SHA256SUMS --ignore-missing
sudo apt-get install -y ./gauntlet_<version>_arm64.deb
gauntlet --version                # expect: gauntlet <version>
gauntlet doctor
```

The tarball is equivalent if you prefer no package manager:

```sh
tar -xzf gauntlet-<version>-aarch64-unknown-linux-gnu.tar.gz
install -m 0755 gauntlet-<version>-aarch64-unknown-linux-gnu/gauntlet ~/.local/bin/
install -m 0755 gauntlet-<version>-aarch64-unknown-linux-gnu/gauntlet-workbench ~/.local/bin/
```

### PostgreSQL (required on the Jetson too)

```sh
sudo apt-get install -y postgresql
sudo -u postgres psql -c "ALTER USER postgres PASSWORD 'postgres';"
sudo -u postgres createdb gauntlet
export GAUNTLET_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:5432/gauntlet
gauntlet doctor   # reports capabilities honestly; it does NOT probe PostgreSQL
```

### bubblewrap and the user-namespace caveat

bwrap is optional (the default sandbox backend is `none`):

```sh
bwrap --version && bwrap --ro-bind / / true && echo "bwrap usable"
gauntlet doctor    # reports the sandbox backend honestly
```

Tegra/L4T kernels sometimes restrict unprivileged user namespaces, which bwrap
needs. If the probe above fails, either enable unprivileged user namespaces
(`sysctl kernel.unprivileged_userns_clone=1`, where the kernel exposes it) or
stay on the default `none` backend. Configuring a non-`none` backend without a
working bwrap fails closed with a typed refusal — Gauntlet never falls back
silently.

### Rollback

```sh
sudo apt-get remove -y gauntlet        # or purge; keeps your project data
sudo apt-get install -y ./gauntlet_<previous-version>_arm64.deb
```

Removing the package never deletes run state or evidence you created; those
live under your project and `GAUNTLET_STATE_DIR`/`GAUNTLET_ARTIFACT_DIR`
roots, not in `/usr`.

---

## macOS

```sh
sudo installer -pkg gauntlet-<version>-macos-<universal|arm64>.pkg -target /
/usr/local/bin/gauntlet --version
```

Installs to `/usr/local/bin/gauntlet` and `/usr/local/bin/gauntlet-workbench`
plus `/usr/local/share/doc/gauntlet/`. The package is unsigned and not
notarized.

Composition note: the `universal` label describes the CLI (a lipo of the
arm64 and x86_64 slices). The `gauntlet-workbench` installed by any macOS pkg
is arm64-only and real-PTY-qualified; on Intel Macs the CLI runs natively but
that arm64-only workbench binary does not execute (macOS has no
arm64-to-x86_64 translator). An Intel (`bun-darwin-x64`) workbench appears
only as the separate `pkg-macos-workbench-unqualified` CI artifact —
unqualified, never inside the pkg.

If Gatekeeper refuses it, Control-click the package and choose
Open, or remove the quarantine flag after download:

```sh
xattr -d com.apple.quarantine gauntlet-<version>-macos-<label>.pkg
```

Uninstall:

```sh
sudo rm -f /usr/local/bin/gauntlet /usr/local/bin/gauntlet-workbench
sudo rm -rf /usr/local/share/doc/gauntlet
sudo pkgutil --forget ai.godspeed.gauntlet
```

---

## Windows

Run `gauntlet-<version>-x86_64-pc-windows-msvc-setup.exe`. It installs per
user (no administrator rights) into `%LOCALAPPDATA%\Programs\Gauntlet`, adds
that directory to your **user** `PATH` (a new terminal picks it up), and
removes both on uninstall. The installer is unsigned; SmartScreen may show
"Windows protected your PC" — verify `SHA256SUMS` first, then choose
More info -> Run anyway.

```powershell
gauntlet --version
gauntlet doctor
```

The `.zip` is a portable alternative: extract it anywhere and add that
directory to your `PATH`. In the qualified shape it contains `gauntlet.exe`
and `gauntlet-workbench.exe`; in the CLI-only fallback shape it contains
`gauntlet.exe` only (see the package shapes at the top of this guide).

Uninstall: Settings -> Apps -> Gauntlet, or silently from a terminal:

```powershell
& "$env:LOCALAPPDATA\Programs\Gauntlet\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES
```

Uninstalling removes the installed exes, the release notes, and the user
`PATH` entry.

---

## Uninstall and rollback (summary)

| Platform | Uninstall | Rollback |
|---|---|---|
| Linux (.deb) | `sudo apt-get remove -y gauntlet` | install the previous `.deb` |
| Linux (tarball) | delete the installed binaries | extract and install the previous tarball |
| macOS | `rm` the binaries + `pkgutil --forget ai.godspeed.gauntlet` | install the previous `.pkg` |
| Windows | Apps settings or `unins000.exe /VERYSILENT` | run the previous `setup.exe` (it upgrades in place) |

Re-download previous assets from their `gauntlet-v<version>` release. Never
re-point an existing tag to rebuild.

---

## Troubleshooting

| Symptom | Cause | Action |
|---|---|---|
| `GLIBC_2.3x' not found` | Binary newer than the host glibc | Use the artifact for your distribution; releases target glibc 2.35+ (Ubuntu 22.04+) |
| `run`/`serve` fail with a connection error | `GAUNTLET_DATABASE_URL` unset or unreachable | Point it at a real PostgreSQL server; the default `127.0.0.1:54329` is a dev instance that does not exist on your machine |
| `doctor` is green but `run` still fails | `doctor` does not probe PostgreSQL | Verify the database reachability yourself (e.g. `pg_isready`) |
| bwrap backend refused | bwrap missing or user namespaces disabled | Install bubblewrap / enable unprivileged user namespaces, or keep the default `none` backend |
| `codemode` reported unavailable | bun not installed | Optional capability; install bun only if you use tool programs |
| Gatekeeper / SmartScreen warning | Unsigned package | Verify `SHA256SUMS`, then allow explicitly |
| Workbench renders no panels | Not connected to the surface | Start `gauntlet serve` and set `GAUNTLET_ENDPOINT` (or `GAUNTLET_WORKBENCH_ENDPOINT`) before launching |
| `command not found` after Windows install | Current terminal predates the PATH edit | Open a new terminal (the installer edits the user PATH via the registry) |
