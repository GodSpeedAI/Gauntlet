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
L4T) against the **published release's own assets**. Native builds are not
byte-reproducible, so a record made against any other build is rejected
(sha256 mismatch against the release's `SHA256SUMS`). The flow is therefore
two-phase:

1. The release workflow publishes a **prerelease**. Its notes say
   `Jetson: NOT QUALIFIED (no real-hardware evidence)`.
2. The operator qualifies THAT release on the Jetson and uploads the record.
3. `promote-release.yml` re-aggregates every record and, only if every claimed
   platform is QUALIFIED, flips the prerelease to a release.

On the Jetson (needs `gh`, `git`, and a reachable PostgreSQL; the owner types
the sudo password when prompted):

```sh
tag=gauntlet-v<version>; d=$(mktemp -d ~/gq.XXXX)
gh release download "$tag" --repo GodSpeedAI/Gauntlet --dir "$d/dist" \
  --pattern 'gauntlet_*_arm64.deb' --pattern 'gauntlet-*-aarch64-*.tar.gz' --pattern SHA256SUMS
(cd "$d/dist" && sha256sum -c SHA256SUMS --ignore-missing)
git clone -q --depth 1 -b "$tag" https://github.com/GodSpeedAI/Gauntlet "$d/repo"
export GAUNTLET_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:5432/postgres
bash "$d/repo/packaging/qualify-local.sh" --platform linux-arm64-jetson --dist "$d/dist" \
  --version <version> --commit "$(git -C "$d/repo" rev-parse HEAD)" \
  --out "$d/qualification-linux-arm64-jetson.json"
gh release upload "$tag" "$d/qualification-linux-arm64-jetson.json" --repo GodSpeedAI/Gauntlet
```

Then run Actions -> **Promote release (qualification)** with "Use workflow
from" set to the tag. It has no inputs.

The script refuses `linux-arm64-jetson` on a non-Jetson host and on a hosted
runner. It installs the arm64 `.deb` with sudo, runs the same steps, and
uninstalls.

### Restore mode (a gauntlet .deb is already installed)

If a gauntlet `.deb` is already installed, package mode becomes restore mode.
Before changing anything the script needs a checksum-verified `.deb` whose
Version equals the installed version. In order of preference:
`--previous-deb <path>` (with `--previous-sha256 <hex>` or a `SHA256SUMS` next
to it), the `gauntlet-v<installed>` release, or the candidate `.deb` itself
when its Version equals the installed one. In the last case the script prints
that the restored binary is a newer build of the same version, and there is no
upgrade leg (NOT_RUN). If no such `.deb` exists, it refuses to start. It runs
plain `sudo`, never stores or pipes the password, and at exit (also on
failure) reinstalls the original version. The last line printed is
`qualify: RECORD: <path>`.

### No sudo: tarball mode

When `sudo` needs a password you cannot type, or you must not touch the
system package, use `--mode tarball`. It installs the tarball into a prefix
inside a scratch dir, redirects `HOME` and `XDG_*` there, records the `.deb`
install and upgrade steps as NOT_RUN, and removes everything at exit. The
record status is `QUALIFIED (tarball)`, which is shown honestly in the table
and does not make the release production-ready:

```sh
scratch=$(mktemp -d ~/gauntlet-qual.XXXX)
TMPDIR="$scratch" packaging/qualify-local.sh --mode tarball --platform linux-arm64-jetson \
  --dist <dir-with-assets> --version <version> --commit <sha> --out "$scratch/qualification-linux-arm64-jetson.json"
```

You can also run the script on any other host with the matching platform id
(`linux-arm64-generic`, `linux-amd64`, `macos-arm64`, `macos-x64`,
`windows-x64` from Git Bash). Those records are `operator-host` evidence and
never mark Jetson QUALIFIED.

## Run qualification from GitHub Actions

Actions -> Qualify (native) -> Run workflow, from a branch or a tag (the
workflow must exist on the default branch to be listed). There is
no ref input: it builds and qualifies the commit you selected. It also runs on
pull requests that change qualification paths.

## Signing

No Apple Developer ID or Authenticode signing is configured; every release is
labelled unsigned. If signing secrets are later added, the publish job logs
whether they are present (never their values); signing itself must be added to
the package stage before qualification so the qualified hashes stay valid.
