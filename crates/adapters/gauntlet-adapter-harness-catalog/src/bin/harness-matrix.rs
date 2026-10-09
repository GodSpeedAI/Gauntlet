//! Report-only generator for the harness verification matrix.
//!
//! Usage: `harness-matrix [--write <path>]`
//!
//! Prints the Markdown matrix to stdout, or writes it to `<path>`. It reads the built-in
//! definitions and `definitions/VERIFICATION.toml`, looks up each executable on `PATH` (a
//! lookup only; no harness is run), and exits 0. It exits 2 when the catalog or the evidence
//! store cannot be loaded.

use gauntlet_adapter_harness_catalog::builtin_with_evidence;
use gauntlet_adapter_harness_catalog::matrix::render_matrix;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut write_path: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--write" => match args.next() {
                Some(path) => write_path = Some(PathBuf::from(path)),
                None => {
                    eprintln!("harness-matrix: --write needs a path");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("harness-matrix: unknown argument {other:?}");
                return ExitCode::from(2);
            }
        }
    }

    let (defs, records) = match builtin_with_evidence() {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("cannot load harness catalog or evidence: {e}");
            return ExitCode::from(2);
        }
    };

    let today = today_utc();
    let text = render_matrix(
        &defs,
        &records,
        &|name| find_on_path(name).is_some(),
        &today,
    );

    match write_path {
        None => print!("{text}"),
        Some(path) => {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            if let Some(parent) = parent
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("cannot create {}: {e}", parent.display());
                return ExitCode::from(2);
            }
            if let Err(e) = std::fs::write(&path, text) {
                eprintln!("cannot write {}: {e}", path.display());
                return ExitCode::from(2);
            }
            println!("wrote {}", path.display());
        }
    }
    ExitCode::SUCCESS
}

/// First executable file named `name` on `PATH`, if any. Lookup only; nothing is executed.
fn find_on_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') || name.is_empty() {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = Path::new(&dir).join(name);
        let meta = std::fs::metadata(&candidate).ok()?;
        (meta.is_file() && meta.permissions().mode() & 0o111 != 0).then_some(candidate)
    })
}

/// Today's UTC date as YYYY-MM-DD.
fn today_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn civil_dates_are_correct() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
        // 2026-10-09 is 20_735 days after the epoch.
        assert_eq!(civil_from_days(20_735), (2026, 10, 9));
    }
}
