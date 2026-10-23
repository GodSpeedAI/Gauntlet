//! Report-only drift check of the built-in harness catalog against a Herdr donor checkout.
//!
//! Usage: `sync-harness-catalog [DONOR_DIR]` (default `.tmp/donor/herdr`).
//! Exits 0 when in sync, 1 when drift is found, 2 when the catalog cannot be loaded.
//! It reads the donor and the catalog only; it never writes any file.

use gauntlet_adapter_harness_catalog::{HERDR_PINNED_COMMIT, builtin, check_against_donor};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let donor: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".tmp/donor/herdr"));

    let defs = match builtin() {
        Ok(defs) => defs,
        Err(e) => {
            eprintln!("cannot load built-in catalog: {e}");
            return ExitCode::from(2);
        }
    };

    let head = donor_head(&donor);
    let report = check_against_donor(&defs, &donor, head.as_deref());

    println!("harness catalog sync (report-only)");
    println!("donor: {}", donor.display());
    println!(
        "donor HEAD: {}",
        head.as_deref().unwrap_or("unknown (no readable .git HEAD)")
    );
    println!(
        "pinned commit: {HERDR_PINNED_COMMIT} ({})",
        match head.as_deref() {
            Some(h) if h.starts_with(HERDR_PINNED_COMMIT) => "match",
            Some(_) => "MISMATCH",
            None => "unverified",
        }
    );
    println!(
        "definitions: {}, refs checked: {}, refs ok: {}",
        report.definitions, report.refs_checked, report.refs_ok
    );
    if report.drift.is_empty() {
        println!("status: in sync");
    } else {
        println!("status: DRIFT ({} findings)", report.drift.len());
        for line in &report.drift {
            println!("  - {line}");
        }
    }
    println!("no files were modified");

    if report.drift.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Resolve HEAD by reading `.git` directly, so no git process is spawned.
fn donor_head(donor: &Path) -> Option<String> {
    let dot_git = donor.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let text = fs::read_to_string(&dot_git).ok()?;
        let rel = text.strip_prefix("gitdir: ")?.trim();
        donor.join(rel)
    };
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        return Some(head.to_string());
    };
    if let Ok(sha) = fs::read_to_string(git_dir.join(reference)) {
        return Some(sha.trim().to_string());
    }
    let packed = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    packed.lines().find_map(|line| {
        let (sha, name) = line.split_once(' ')?;
        (name == reference).then(|| sha.to_string())
    })
}
