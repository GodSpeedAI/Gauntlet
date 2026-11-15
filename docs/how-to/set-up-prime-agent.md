# Set up Prime Agent (optional)

Prime Agent is the recursive delegation harness behind the `prime-agent`
runner. It is an **optional** dependency:

- Gauntlet never bundles it. No package (`.deb`, tarball, macOS `.pkg`/`.dmg`,
  Windows zip or setup.exe) contains it, and CI fails if one does.
- Gauntlet never installs it silently, and never downloads it on its own.
- Without it, every other adapter (`mock-agent`, `cli-agent`, `tty-agent`)
  and every non-delegation feature works unchanged.

## Check what you have

```sh
gauntlet agent doctor
```

One block per adapter. For Prime Agent it reports one of:

| State | Meaning |
|---|---|
| `ready` | Found; shows the version, the location and how it was found. |
| `not installed (optional)` | Not found. Nothing is wrong; the message says how to set it up. |
| `PROBLEM` | Found but unusable, or its version is outside the supported range. The message names the found and the supported versions. The command exits non-zero. |

`gauntlet doctor` also prints one `prime-agent (optional)` line. It never
changes that command's exit status.

## How Gauntlet finds it

In order:

1. `PRIME_AGENT_BIN`, the full path of the binary (the harness's own
   variable namespace; nothing under `GAUNTLET_*`).
2. The first `prime-agent` on `PATH`.

If `PRIME_AGENT_BIN` is set but names no file, detection fails with that
fact. It does **not** quietly fall back to `PATH`: an explicit path means
exactly that binary.

The located binary is asked for `--version` and must sit in the supported
range. The range lives in one place, `HARNESS_VERSION_MIN` and
`HARNESS_VERSION_LIMIT` in `crates/gauntlet-app/src/config/defaults.rs`
(currently `0.8.0` inclusive to `1.0.0` exclusive); `gauntlet doctor`
additionally runs the harness's capability probe. A run that selects
`prime-agent` (`GAUNTLET_AGENT_RUNNER=prime-agent` or `runners.default`)
while Prime Agent is absent or incompatible refuses before any dispatch,
with the same message.

## Install it (opt in)

```sh
gauntlet agent setup prime
```

Prints the exact manual steps and changes nothing. Gauntlet does not
download Prime Agent itself: upstream publishes a hosted installer script on
a moving release channel, so there is no artifact whose bytes Gauntlet could
pin. The installer verifies the release tarball against the channel's
`SHA256SUMS` once it runs; that is upstream's check, not a pinned one of
Gauntlet's.

To have Gauntlet run an installer you obtained and reviewed (Linux and macOS):

```sh
gauntlet agent setup prime --installer ./install.sh --sha256 <digest-you-recorded>
```

Gauntlet then

1. checks the file's SHA-256 against `--sha256` and refuses on a mismatch;
2. shows what it will do (pinned version, user-scoped prefix `~/.local`);
3. asks you to type `yes`, or accepts `--yes` for non-interactive use. Without
   either, it exits with status 2 and runs nothing;
4. runs `sh <installer>` with `PRIME_AGENT_VERSION` set to the pinned version
   (`HARNESS_VERSION_PINNED` in the same defaults file) and
   `PRIME_AGENT_RUST_PREFIX` set to `~/.local`.

The installer itself uses the network and, per its own documentation, takes
over an existing TypeScript install of the same product and stops its daemon.
Read it before you confirm. On Windows, follow the manual steps printed by
`setup`.

Verify with `gauntlet agent doctor`. If `prime-agent` is not on `PATH`, set
`PRIME_AGENT_BIN`.

## Moving the supported range

Raising `HARNESS_VERSION_LIMIT` or the pin is a deliberate re-verification of
the delegation surface against the new harness, made in
`crates/gauntlet-app/src/config/defaults.rs`. The pin must stay inside the
range (a unit test enforces it).
