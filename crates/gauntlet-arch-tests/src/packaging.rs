//! Packaging boundary (WP-63): Prime Agent is an OPTIONAL dependency that
//! Gauntlet never bundles and never fetches while packaging.
//!
//! Two instruments:
//!
//! 1. a SOURCE scan: no packaging script, installer definition or release
//!    workflow may name the harness or its distribution host. A packaging
//!    path that mentions it is one edit from fetching or copying it;
//! 2. a SCRIPT test: `packaging/assert-no-prime-agent.sh`, which the
//!    package workflow runs on every staged directory and built archive,
//!    fails on a directory and on a tarball that carry the harness and
//!    passes on a clean one (so the CI assertion is not vacuous).

use std::path::Path;

/// Packaging inputs the scan covers (repo-relative; directories are walked).
pub const PACKAGING_PATHS: &[&str] = &[
    "packaging",
    ".github/workflows/package.yml",
    ".github/workflows/release-native.yml",
];

/// The one file allowed to spell the harness name: the assertion itself.
pub const ASSERTION_SCRIPT: &str = "packaging/assert-no-prime-agent.sh";

/// How the package workflow refers to the assertion; removed before the scan.
const ASSERTION_SCRIPT_NAME: &str = "assert-no-prime-agent.sh";

/// Lowercase needles that mean "the harness or where it is distributed".
pub const FORBIDDEN: &[&str] = &[
    "prime-agent",
    "prime_agent",
    "primeintellect",
    "prime-intellect",
];

/// Violations in one file's text (one per forbidden needle found).
pub fn scan_text(rel: &str, text: &str) -> Vec<String> {
    let lowered = text.to_ascii_lowercase().replace(ASSERTION_SCRIPT_NAME, "");
    FORBIDDEN
        .iter()
        .filter(|needle| lowered.contains(*needle))
        .map(|needle| format!("packaging: {rel} mentions '{needle}' (Prime Agent must never be packaged or fetched)"))
        .collect()
}

fn collect(root: &Path, rel: &str, out: &mut Vec<String>, errors: &mut Vec<String>) {
    let path = root.join(rel);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) => {
            errors.push(format!("packaging: {rel} cannot be inspected: {error}"));
            return;
        }
    };
    if metadata.is_dir() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) => {
                errors.push(format!("packaging: {rel} cannot be listed: {error}"));
                return;
            }
        };
        let mut names: Vec<String> = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => names.push(entry.file_name().to_string_lossy().into_owned()),
                Err(error) => errors.push(format!("packaging: {rel} entry unreadable: {error}")),
            }
        }
        names.sort();
        for name in names {
            collect(root, &format!("{rel}/{name}"), out, errors);
        }
    } else if metadata.is_file() {
        out.push(rel.to_owned());
    }
}

/// Every packaging file, repo-relative. The assertion script is excluded.
pub fn packaging_files(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for rel in PACKAGING_PATHS {
        collect(root, rel, &mut files, &mut errors);
    }
    files.retain(|rel| rel != ASSERTION_SCRIPT);
    (files, errors)
}

/// The whole source scan.
pub fn check_packaging_excludes_prime_agent(root: &Path) -> Vec<String> {
    let (files, mut violations) = packaging_files(root);
    for rel in files {
        match std::fs::read(root.join(&rel)) {
            Ok(bytes) => violations.extend(scan_text(&rel, &String::from_utf8_lossy(&bytes))),
            Err(error) => violations.push(format!("packaging: {rel} unreadable: {error}")),
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk::repo_root;
    use std::process::Command;

    #[test]
    fn no_packaging_input_names_or_fetches_prime_agent() {
        let root = repo_root();
        let (files, errors) = packaging_files(&root);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        // Anti-vacuity: the scan must actually see the packaging inputs.
        for expected in [
            "packaging/gauntlet.iss",
            "packaging/deb/build.sh",
            "packaging/linux/build-tarball.sh",
            "packaging/macos/build.sh",
            "packaging/windows/build.ps1",
            ".github/workflows/package.yml",
        ] {
            assert!(
                files.iter().any(|f| f == expected),
                "scan misses {expected}"
            );
        }
        let violations = check_packaging_excludes_prime_agent(&root);
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    #[test]
    fn the_scan_has_teeth() {
        assert!(
            !scan_text(
                "x.sh",
                "curl -fsSL https://app.primeintellect.ai/prime-agent/install.sh"
            )
            .is_empty()
        );
        assert!(!scan_text("x.yml", "cp ~/.local/bin/Prime-Agent stage/").is_empty());
        assert!(scan_text("x.yml", "run: sh packaging/assert-no-prime-agent.sh stage").is_empty());
    }

    #[test]
    fn the_package_workflow_runs_the_assertion_on_every_platform() {
        let text = std::fs::read_to_string(repo_root().join(".github/workflows/package.yml"))
            .expect("package.yml reads");
        assert_eq!(
            text.matches(ASSERTION_SCRIPT_NAME).count(),
            3,
            "linux, macos and windows jobs each run the assertion"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_assertion_script_fails_on_a_bundled_harness_and_passes_when_clean() {
        let root = repo_root();
        let script = root.join(ASSERTION_SCRIPT);
        let scratch =
            std::env::temp_dir().join(format!("gauntlet-pkg-assert-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let clean = scratch.join("clean");
        let dirty = scratch.join("dirty");
        std::fs::create_dir_all(clean.join("bin")).unwrap();
        std::fs::create_dir_all(dirty.join("bin")).unwrap();
        std::fs::write(clean.join("bin/gauntlet"), b"x").unwrap();
        std::fs::write(dirty.join("bin/gauntlet"), b"x").unwrap();
        std::fs::write(dirty.join("bin/prime-agent"), b"x").unwrap();
        let run = |args: &[&Path]| {
            Command::new("sh")
                .arg(&script)
                .args(args)
                .output()
                .expect("sh runs")
                .status
                .success()
        };
        assert!(run(&[&clean]), "a clean stage passes");
        assert!(!run(&[&dirty]), "a stage carrying the harness fails");
        assert!(!run(&[&clean, &dirty]), "any dirty input fails the run");
        let tar_of = |dir: &Path, name: &str| {
            let out = scratch.join(name);
            let status = Command::new("tar")
                .arg("-czf")
                .arg(&out)
                .arg("-C")
                .arg(dir)
                .arg(".")
                .status()
                .expect("tar runs");
            assert!(status.success());
            out
        };
        assert!(
            run(&[&tar_of(&clean, "clean.tar.gz")]),
            "a clean tarball passes"
        );
        assert!(
            !run(&[&tar_of(&dirty, "dirty.tar.gz")]),
            "a tarball carrying the harness fails"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
