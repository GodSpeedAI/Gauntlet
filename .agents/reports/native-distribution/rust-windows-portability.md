# Report — Rust Windows portability correction (CLI round 2 follow-up)

Branch: `release/native-distribution`. Date: 2026-10-29. Builder correction task.
Dispatch: Package run 40944168892, job `package (windows x86_64)`, step "Build the gauntlet
CLI (frozen release form)" — `cargo build --release --locked -p gauntlet-cli --features
surfaces --bin gauntlet` RED with 8 errors, all unix-only fs-permission code in gauntlet-cli.
Touch scope honored: only `crates/gauntlet-cli/src/**` and this report directory. No commits.

## Change principle

Every unix cfg arm keeps the exact pre-existing code (byte-identical logic and error
messages verbatim). Non-unix arms are new, minimal, and contain no dead code: where the
surrounding function would otherwise leave parameters unused on non-unix, the arm consumes
them with `let _ = (...)` (verified clippy-clean; see gates). No new dependencies; no
signature, behavior, or test changes on unix.

## Gated sites in `crates/gauntlet-cli/src/` (CI-named)

1. `fabric_compose.rs` `is_executable_file` — `use std::os::unix::fs::PermissionsExt;` +
   `mode() & EXECUTE_BITS` moved into a `#[cfg(unix)]` arm (exact expression preserved).
   Non-unix arm: `fs::metadata(path).is_ok_and(|meta| meta.is_file())` — a regular file on
   the search path; PATHEXT resolution is the loader's concern.
2. `fabric_compose.rs` `EXECUTE_BITS` const — now `#[cfg(unix)]`: its only use is the unix
   arm above, so leaving it ungated would be dead code on non-unix.
3. `improve_cmd.rs` ~1357 (baseline instruments) — chmod of `child_wrapper`/`shim` is a
   `#[cfg(unix)]` block (messages verbatim); `#[cfg(not(unix))]` arm is a no-op
   (`let _ = (&child_wrapper, &shim);`). Both variables are still used later on all cfgs.
4. `improve_cmd.rs` ~2662 (judge wrapper) — same shape; `#[cfg(not(unix))]` arm
   `let _ = &judge_wrapper;` (the wrapper is still read and digest-pinned below).
5. `improve_cmd.rs` `make_scripts_executable` — whole body split: unix arm keeps the
   read_dir walk + `from_mode(0o755)` chmod verbatim (including the absent-dir
   `return Ok(())`); non-unix arm is `let _ = (dir, refused); Ok(())` — nothing to set.

## Additional sites found by proactive scan, gated in the same pass (beyond the CI list)

Scan patterns: `std::os::unix`, `std::os::fd`, `PermissionsExt`, `from_mode`, `set_permissions`,
`pre_exec`, `libc::`, `termios`, `unistd`, `CommandExt`, `ExitStatusExt`, `symlink_metadata`,
raw-fd traits, mode literals, `cfg(target_os/windows/unix)`.

6. `improve_cmd.rs` test chmod ~3471 (`fixture_project` held-out instrument) — `#[cfg(unix)]`
   block; on non-unix the fixture simply sets no exec mode (the variable is still used by the
   preceding `fs::write`, so no arm is needed for warnings).
7. `improve_cmd.rs` test chmod ~3926 (fallback-test leg copy) — `#[cfg(unix)]` block; `leg`
   is used elsewhere, so no non-unix arm is needed.
8. `verify_cmd.rs` `a_signal_killed_command_is_the_typed_environmental_refusal` test —
   whole test now `#[cfg(unix)]` (matches the existing `#[cfg(unix)] #[test]` precedent on
   `fabric_compose.rs`'s symlink test): signal semantics are unix-only (the non-unix
   `ExitStatusExt` gate at verify_cmd.rs ~131-134 fabricates signal 0, so the test's
   "signal 9" assertion could never hold on windows), and its chmod block goes with it.

Already gated before this pass (no change): `verify_cmd.rs:131-134` (`ExitStatusExt::signal`
with a `#[cfg(not(unix))]` fallback), `fabric_compose.rs` `#[cfg(unix)] #[test]` symlink test
(~1916), `std::fs::symlink_metadata` uses (cross-platform). The only other match was a doc
comment (`writer_lock.rs` SIGSTOP mention) and `tests/p2_context_isolation.rs:1062`
(`from_model` substring-matching the `from_mode` pattern — not a real site).

## Residuals (recorded, NOT touched — outside the dispatch touch scope)

`crates/gauntlet-cli/tests/*.rs` integration tests carry unix-only constructs that rustc
rejects on the windows target under `--all-targets` (the failing CI step builds `--bin
gauntlet` only, so these do NOT block the package job):

- `tests/fabric_acceptance.rs:580` — `std::os::unix::fs::symlink` (confirmed compile error
  in the windows `--all-targets` check).
- `tests/e2e_full_loop_real_runner.rs:182-183`, `tests/e2e_recovery.rs:207-208`,
  `tests/e2e_full_loop_mock.rs:228-229`, `tests/e2e_budget.rs:240-241`,
  `tests/e2e_deliver.rs:150-151` — `PermissionsExt` + `from_mode(0o755)` chmod helpers.
- `tests/operator_actions_no_bypass.rs:953-958, 986-991` — `PermissionsExt` chmods.
- `tests/improve_baseline.rs:884-892` — `PermissionsExt::mode()` exec-bit assertion.

These need the same cfg treatment (or unix-gating of the affected tests) in a follow-up
allowed to touch `crates/gauntlet-cli/tests/**` if windows `cargo test`/`--all-targets` is
ever run in CI; the package build job does not compile tests.

## Gates (all run on linux x86_64; windows target added via `rustup target add`)

| Gate | Result |
|---|---|
| `cargo check -p gauntlet-cli --all-targets` | GREEN |
| `cargo check -p gauntlet-cli --features surfaces --locked --target x86_64-pc-windows-msvc` (mirrors the failing CI step's lib+bin surface; `cargo check` needs no MSVC linker) | GREEN |
| `cargo clippy -p gauntlet-cli --all-targets -- -D warnings` | GREEN |
| `cargo fmt --check` | CLEAN |
| `cargo nextest run -p gauntlet-cli --test cli_surface` | 5/5 PASS |
| `cargo nextest run -p gauntlet-arch-tests` | 261/261 PASS (literal scans green — the cfg arms add no new literals) |

Windows `--all-targets` (tests included) is RED only on the residual tests/ sites listed
above; the lib, unit tests, and bins compile for the windows target.

The `let _ = (refs)` no-op pattern was clippy-verified in a scratch crate before use
(default clippy, `-D warnings`), and the `#[cfg] let`/block-arm shapes compile and lint
clean on the unix cfg; the windows cfg is validated by the windows-target check above.
