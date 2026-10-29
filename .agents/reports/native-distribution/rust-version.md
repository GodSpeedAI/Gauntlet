# Slice R report — CLI root `--version` (builder R)

Status: COMPLETE. Branch `release/native-distribution`, source commit at dispatch 1543cbf (main).
Scope held: only `crates/gauntlet-cli/**` touched; no commits, no pushes; other builders' worktree
changes (workbench/, scripts/tui-preflight.sh, packaging/, .agents/) untouched.

## Files changed (all additive; 44 insertions, 0 deletions)

1. `crates/gauntlet-cli/src/lib.rs`
   - Lines 65–72: new root arm in `run_cli`'s dispatch, placed immediately after the
     `--help`/`-h` arm and before subcommand dispatch: `Some("--version") | Some("-V")` prints
     `gauntlet <version>` to stdout via `println!("gauntlet {}", env!("CARGO_PKG_VERSION"))`
     and returns `ExitCode::SUCCESS`. The version comes from compile-time Cargo metadata
     (workspace `[workspace.package] version = 0.1.0` inherited by `gauntlet-cli` via
     `version.workspace = true`); no hardcoded version string exists anywhere. The arm is
     NOT feature-gated, so it behaves identically in the default build and the `surfaces`
     build (same cfg treatment as `--help`/`-h`, which is likewise ungated).
   - Line 150: one line appended to `usage()`: `println!("       gauntlet --version  Print the CLI version");`
2. `crates/gauntlet-cli/tests/cli_help.txt` — line 33 appended (the same usage line), deliberate.
3. `crates/gauntlet-cli/tests/cli_help_surfaces.txt` — line 34 appended (the same usage line), deliberate.
4. `crates/gauntlet-cli/tests/cli_surface.rs` — lines 100–132: new regression test
   `root_version_flags_print_the_cargo_version_and_exit_zero`: for each of `--version` and `-V`,
   runs the REAL binary via the existing `binary()` helper (`CARGO_BIN_EXE_gauntlet`), asserts
   exit 0, asserts stdout is EXACTLY `format!("gauntlet {}\n", env!("CARGO_PKG_VERSION"))`
   (derived from Cargo metadata in the test crate, which shares the binary's package version —
   the test cannot drift from the version authority), and asserts stderr is empty.

No other test file, golden, manifest, or source file changed. No new dependencies
(`gauntlet-cli` external allowlist stays EMPTY); no `unsafe`; no literals that trip the
arch-tests scanners (verified by the boundary suite run below).

## Usage-golden decision (deliberate, recorded per the golden-file procedure)

DECISION: added the `--version  Print the CLI version` line to `usage()` and updated BOTH
golden help files deliberately.

Reasoning: the slice contract allows either path; the dispatch instruction prefers the smallest
COMPLETE change when all agreement tests stay green. Verified before choosing:
- `COMMAND_TABLE_LEN` (`crates/gauntlet-cli/src/report_manifest.rs` line 140) counts COMMANDS
  only (`COMMAND_TABLE.len() + surfaces_extra_rows()`); root flags were never part of that
  contract — `--help`/`-h` are undocumented in usage() and uncounted, so documenting
  `--version` cannot desynchronize the dispatch table or the manifest.
- `usage_dispatch_and_manifest_name_the_same_command_set` checks each command via
  `stdout.contains("gauntlet {command}")`; the new line names no command, so no collision.
- `the_whole_tree_help_snapshot_holds` is byte-exact per configuration; both goldens were
  updated with the identical line usage() emits, so the snapshot holds in BOTH builds.

Value of the line: `--version` becomes discoverable in `--help`; the packaging smoke contract
(Slice B: `smoke-gauntlet.sh` asserts `--version` exact) targets an operator-visible flag, so an
undocumented flag would be the incomplete change. No assertion was weakened; no golden was
updated because of drift — the drift IS the recorded change.

## Gates run (from repo root; all PASS)

| Gate | Command | Result |
|---|---|---|
| Type check | `cargo check -p gauntlet-cli --all-targets` | PASS (finished, 3m01s) |
| CLI surface, DEFAULT build | `cargo nextest run -p gauntlet-cli --test cli_surface` | PASS — 5/5 (incl. new `root_version_flags_print_the_cargo_version_and_exit_zero`, `the_whole_tree_help_snapshot_holds`, `usage_dispatch_and_manifest_name_the_same_command_set`) |
| CLI surface, SURFACES build | `cargo nextest run -p gauntlet-cli --features surfaces --test cli_surface` | PASS — 5/5 (surfaces golden byte-exact; both feature configurations covered) |
| Lint | `cargo clippy -p gauntlet-cli --all-targets -- -D warnings` | PASS (zero warnings) |
| Boundaries | `cargo nextest run -p gauntlet-arch-tests` | PASS — 261/261 (incl. `core_crates_contain_no_hardcoded_literals`, denylist, scope integrity, deps seal) |
| Ruler | `./scripts/verify-ruler.sh` | PASS — "answer key and manifest digests match the frozen lock" |

Direct binary smoke (outside the test suite): `target/debug/gauntlet --version` and `-V` both
print `gauntlet 0.1.0` to stdout, exit 0, stderr empty.

The cli_surface tests require no database (they only exercise root flags and usage refusals);
the DB-backed gate battery remains the orchestrator's integration step.

## Residuals

- None blocking. Notes for the record:
  - `usage()`'s doc comment still reads "one line per surface"; it now also carries one root
    flag line. Left as-is (comment accuracy is not a gate; widening the doc was out of scope).
  - Slice B/C smoke scripts must assert the exact string `gauntlet <workspace-version>`; if the
    workspace version ever bumps, no test literal needs editing (both the dispatcher and the
    test derive from `env!("CARGO_PKG_VERSION")`).
  - `cargo check --all-targets` covers default features; the surfaces configuration is compiled
    by the surfaces-config nextest run above (and by `just test-surfaces` at integration).
