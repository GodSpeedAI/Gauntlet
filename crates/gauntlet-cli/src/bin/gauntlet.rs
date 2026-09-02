//! The `gauntlet` binary (spec §20.1's canonical command name): a thin
//! wrapper over the `gauntlet_cli` library's [`gauntlet_cli::run_cli`]
//! dispatcher (plan Task 20's thin-library refactor — the composition
//! root is the library; the binary only forwards `std::env::args`).
//! Auto-discovered from `src/bin/gauntlet.rs` — no manifest path key.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    gauntlet_cli::run_cli(&args)
}
