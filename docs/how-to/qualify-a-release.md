# How-To: Qualify a Native Release

Native releases pass three separate stages. They are kept separate on purpose,
so a green build is never mistaken for a ready release.

| Stage | Workflow | Question it answers |
|---|---|---|
| CI validation | `ci.yml` | Does the code build and pass its tests? |
| Qualification | `qualify-native.yml` | Do the packaged artifacts install, run, upgrade and uninstall on each platform? |
| Publication | `release-native.yml` | Publish, with the readiness table. It has a hard `needs:` on qualification. |

`release-native.yml` builds only the commit of the `gauntlet-v*` tag that
triggered it. Neither it nor `qualify-native.yml` accepts a ref input.

## What qualification does

Per platform, on a native hosted runner, `packaging/qualify-local.sh`:

1. **checksums**: every artifact matches `SHA256SUMS`.
2. **provenance**: `gh attestation verify` where an attestation exists. The
   attestation is created by the publish job after qualification passes, so
   this is NOT_RUN during qualification and is verified in publish.
3. **tarball_install**: the tarball (Linux) or zip (Windows) payload runs.
4. **install**: the package is installed as a user would (dpkg/apt for `.deb`,
   `installer -pkg` for macOS, Inno Setup silent install for Windows).
5. **upgrade**: the previous released version is installed first, then
   upgraded to the candidate. NOT_RUN when no previous release exists.
6. **smoke**: `gauntlet --version`, `gauntlet doctor` in the fixture project,
   `gauntlet agent doctor`, and the packaging smoke scripts.
7. **governed_run**: one governed run with the mock adapter and a status check
   that units settled. Needs a PostgreSQL server (`GAUNTLET_DATABASE_URL`).
8. **uninstall**: no binaries remain (on disk or on `PATH`), and project data,
   run state and evidence are kept, as the install guide documents.

Platforms: `windows-latest`, `ubuntu-22.04`, `ubuntu-22.04-arm`, `macos-15`
(arm64) and `macos-15-intel`. Each platform writes a JSON record; the records
are collected into `qualification.json` and `QUALIFICATION.md`.

## What each status means

Step statuses:

| Status | Meaning |
|---|---|
| PASS | The step ran and its assertions held. |
| FAIL | The step ran and an assertion failed. Any FAIL blocks publication. |
| NOT_RUN | The step could not run; the record states why. It is never counted as PASS. |

Platform statuses:

| Status | Meaning |
|---|---|
| QUALIFIED (tarball) | A no-sudo run: tarball install, smoke, governed run and uninstall PASSed; the package install and upgrade are NOT_RUN. Not release-ready. |
| QUALIFIED | No step FAILed and `checksums`, `tarball_install`, `install`, `smoke`, `governed_run` and `uninstall` all PASSed. `upgrade` and `provenance` may be NOT_RUN. |
| PARTIAL | No step FAILed, but a required step was NOT_RUN. |
| NOT QUALIFIED | A step FAILed, no record exists (for example the runner label was unavailable), or, for Jetson, there is no valid real-hardware evidence. |

The release is **production-ready only if every claimed platform is
QUALIFIED** (claims live in `packaging/qualification-platforms.json`) and the
publish run attested the tagged commit. Otherwise it is created as a
**prerelease**. All packages are unsigned.

## Read the readiness table

The release notes and `QUALIFICATION.md` contain:

- one row per claimed platform with status, evidence kind (`hosted-runner` or
  `real-hardware`), runner image, and notes. Every non-PASS step appears in the
  notes with its reason;
- a step matrix (platform by step).

`qualification.json` is the machine-readable form: each platform carries its
artifact names and sha256, the commit, each step with status and reason, and
the runner image. To check an artifact against the record, compare its sha256
with the `artifacts` entry for your platform.

## Run qualification on a Jetson

Hosted `ubuntu-22.04-arm` qualifies **linux-arm64 (generic)** only. The
Jetson row needs a record produced on real Jetson hardware (Ubuntu 22.04,
L4T).

On the Jetson, with the release assets and `SHA256SUMS` in one directory, a
checkout of this repository at the release tag, and a reachable PostgreSQL
server:

```sh
export GAUNTLET_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:5432/postgres
packaging/qualify-local.sh --platform linux-arm64-jetson \
  --dist ~/Downloads --version <version> --commit <tag-commit-sha> \
  --out qualification-linux-arm64-jetson.json
```

The script refuses `linux-arm64-jetson` on a non-Jetson host and refuses to
run it on a hosted runner. It installs the arm64 `.deb` with sudo, runs the
same steps, and uninstalls. The record is Jetson evidence only if its status is
QUALIFIED and its artifact sha256 values match the release's arm64 assets.

Attach the record to the release (a manual operator step):

```sh
gh release upload gauntlet-v<version> qualification-linux-arm64-jetson.json
```

The next publish run for that tag validates the record against that run's
artifacts: real-hardware evidence, same version, QUALIFIED, matching sha256.
A stale or mismatched record is rejected and the row stays NOT QUALIFIED. A
re-run rebuilds the artifacts, so evidence you attached earlier will not match
a rebuild. Until a valid record is attached, the release notes say
`Jetson: NOT QUALIFIED (no real-hardware evidence)`.

### With sudo on a machine that already has gauntlet installed (restore mode)

If a gauntlet `.deb` is already installed, package mode becomes restore mode.
Before changing anything the script needs a checksum-verified `.deb` of the
installed version (fetched with `gh release download gauntlet-v<installed>`,
or given with `--previous-deb <path>` plus `--previous-sha256 <hex>` or a
`SHA256SUMS` next to it) and refuses to start if it cannot get one. It then
runs plain `sudo` (you type the password; it is never stored or piped),
upgrades from the installed version to the candidate, smokes, runs the
governed mock loop, uninstalls, and finally reinstalls the originally
installed version, also when a step fails. The last line printed is
`qualify: RECORD: <path>`.

### No sudo: tarball mode

When `sudo` needs a password (or you must not touch the system package), use
`--mode tarball`. It installs the tarball into a prefix inside a scratch dir,
redirects `HOME` and `XDG_*` there, records the `.deb` install and upgrade
steps as NOT_RUN, and removes everything at exit. The record status is
`QUALIFIED (tarball)`, which is shown honestly in the table and does not make
the release production-ready:

```sh
scratch=$(mktemp -d ~/gauntlet-qual.XXXX)
TMPDIR="$scratch" packaging/qualify-local.sh --mode tarball --platform linux-arm64-jetson \
  --dist <dir-with-assets> --version <version> --commit <sha> --out "$scratch/qualification-linux-arm64-jetson.json"
```

A record may also be checked in at
`packaging/qualification/evidence/jetson/qualification-linux-arm64-jetson.json`;
qualification reads it input-free and accepts it only when its sha256 values
match that run's artifacts (builds are not byte-reproducible, so evidence is
normally attached to the release instead).

You can also run the script on any other host with the matching platform id
(`linux-arm64-generic`, `linux-amd64`, `macos-arm64`, `macos-x64`,
`windows-x64` from Git Bash). Those records are `operator-host` evidence and
never mark Jetson QUALIFIED.

## Run qualification from GitHub Actions

Actions -> Qualify (native) -> Run workflow, from a branch or a tag. There is
no ref input: it builds and qualifies the commit you selected. It also runs on
pull requests that change qualification paths.

## Signing

No Apple Developer ID or Authenticode signing is configured; every release is
labelled unsigned. If signing secrets are later added, the publish job logs
whether they are present (never their values); signing itself must be added to
the package stage before qualification so the qualified hashes stay valid.
