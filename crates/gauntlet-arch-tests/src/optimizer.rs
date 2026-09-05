//! (e) Optimizer containment — the promote module (plan Task 23, charter
//! D8a/D10). The optimizer's ONLY output channel is a typed candidate
//! revision; the §19.3/§19.4 services in `gauntlet-app/src/promote` may
//! not modify the active loop's configuration or its installed roles, and
//! this scan enforces that mechanically over the module's source, in the
//! delegation await-ban's scan class.
//!
//! # What the scan bans, mechanically
//!
//! Any file under `crates/gauntlet-app/src/promote/` containing:
//!
//! - **Rule A (config write)**: the configuration-write surface's tokens
//!   — `set_whole(`, `set_toggle(`, `set_list(`, `set_text(`, the
//!   `FlagOverrides` constructor surface, or the four-layer application
//!   entry (`apply_env_layer`/`apply_env_value`) — PLUS the raw write /
//!   env-mutation / process-spawn classes (`std::fs::write` /
//!   `fs::write(`, `File::create` / `OpenOptions`, `set_var(`,
//!   `Command::new`; COR-2, falsifier F-6 probe H: the raw classes
//!   reached the same active-loop surfaces while reporting zero
//!   violations). Task 24 E-1 extends the raw classes to the remaining
//!   file-mutation spellings — `fs::rename`, `fs::copy`, `File::options`,
//!   `fs::hard_link`, `fs::remove_file` — and adds the `write!` macro as
//!   a TARGET rule (a `write!` to any non-formatter sink is banned; the
//!   Display/Error formatter shape `write!(f, …)` is the documented
//!   exemption). The promote module may
//!   READ the config vocabulary ([`crate::optimizer`]'s subject is the
//!   candidate DELTA, which validates keys against it); it may never
//!   WRITE effective configuration — through the typed surface or around
//!   it.
//! - **Rule B (roles install)**: the roles-install surface's tokens —
//!   `InstallInput`, `InstallReport`, `RolesLock`, or a `::install` /
//!   `install::` path. The optimizer never installs roles.
//!
//! # The negative controls
//!
//! "A suite no adapter can fail is not a suite": the controls plant each
//! banned shape in a fixture map and assert the scan REPORTS it. The real
//! promote module carries none of them, which the repo test asserts in
//! the same run — and the planting builder proved the bite once by
//! committing a real violation to a scratch copy and watching this scan
//! (and `just boundaries`) go red before reverting.
//!
//! # Scope
//!
//! The scan is over `crates/gauntlet-app/src/promote/**` only: it is the
//! optimizer's module (the app crate's other modules legitimately write
//! state, and the CLI's improve COMMAND legitimately applies a candidate
//! delta through the composition — the candidate becomes active only
//! through an operator-run evaluation, never by the optimizer writing
//! configuration itself).

use std::collections::BTreeMap;

/// The promote module this scan polices.
pub const PROMOTE_MODULE_SRC: &str = "crates/gauntlet-app/src/promote";

/// The configuration-write surface's tokens (rule A). Beyond the typed
/// config-write surface, the RAW write / env-mutation / process-spawn
/// classes are banned too (COR-2, falsifier F-6 probe H): a promote module
/// that bypasses the typed configuration surface entirely — rewriting
/// GAUNTLET.md or the roles dir through raw `fs`, mutating the loop's
/// environment, or shelling out to copy files — violates §19.3's "It may
/// not modify the active loop" exactly as much as one that calls
/// `set_whole(`, and the original token list let it through with ZERO
/// violations.
const CONFIG_WRITE_TOKENS: &[&str] = &[
    "set_whole(",
    "set_toggle(",
    "set_list(",
    "set_text(",
    "FlagOverrides",
    "apply_env_layer",
    "apply_env_value",
    // Raw file writes (std and glob-imported spellings).
    "std::fs::write",
    "fs::write(",
    "File::create",
    "OpenOptions",
    // Task 24 E-1 (the remaining raw file-mutation classes): a file can
    // be created, replaced, linked, or destroyed without ever spelling
    // `fs::write` — rename, copy, hard-link, options-opened handles, and
    // removal each reach the same active-loop surfaces the raw-write ban
    // exists for. Each class carries its std-spelling token (which also
    // matches `use` imports) and its glob-imported call spelling, exactly
    // as the original write classes do.
    "std::fs::rename",
    "fs::rename(",
    "std::fs::copy",
    "fs::copy(",
    "File::options",
    "std::fs::hard_link",
    "fs::hard_link(",
    "std::fs::remove_file",
    "fs::remove_file(",
    // Environment mutation.
    "set_var(",
    // Process spawn.
    "Command::new",
];

/// The roles-install surface's tokens (rule B).
const ROLES_INSTALL_TOKENS: &[&str] = &[
    "InstallInput",
    "InstallReport",
    "RolesLock",
    "::install",
    "install::",
];

/// Task 24 E-1: the `write!` macro is a file/sink-write vector (`write!(
/// file_handle, amended)` rewrites a file through an already-opened
/// handle) — but the SAME macro spells every Display/Error impl's
/// formatter call (`write!(f, …)`), which the promote module legitimately
/// carries. The rule is therefore the TARGET, not the macro: a `write!`
/// whose first argument is the std fmt convention's formatter binding `f`
/// is the formatting shape; a `write!` targeting ANY other expression is
/// a sink write and is banned. The target is read past the paren, joining
/// one line break so the wrapped-call shape (`write!(\n f, …)`) is not a
/// hole.
fn write_macro_sink_lines(text: &str) -> Vec<(usize, String)> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let mut search_from = 0;
        while let Some(offset) = line[search_from..].find("write!(") {
            let paren = search_from + offset + "write!(".len();
            // The target: the text after the paren up to the first comma,
            // spilling onto the NEXT line when this line ends first.
            let rest = &line[paren..];
            let target = match rest.split_once(',') {
                Some((target, _)) => target.trim().to_owned(),
                None => {
                    let next = lines.get(index + 1).copied().unwrap_or("");
                    match next.split_once(',') {
                        Some((target, _)) => target.trim().to_owned(),
                        None => next.trim().to_owned(),
                    }
                }
            };
            if target != "f" {
                violations.push((index + 1, target));
            }
            search_from = paren;
        }
    }
    violations
}

pub fn scan_optimizer_containment(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut violations = Vec::new();
    if files.is_empty() {
        violations.push(format!(
            "optimizer-containment: {PROMOTE_MODULE_SRC}: the promote module's sources are \
             missing — the scan has nothing to police and must not pass silently"
        ));
        return violations;
    }
    for (path, text) in files {
        if !path.starts_with(PROMOTE_MODULE_SRC) {
            continue;
        }
        for (rule, tokens) in [
            ("rule A", CONFIG_WRITE_TOKENS),
            ("rule B", ROLES_INSTALL_TOKENS),
        ] {
            for token in tokens {
                for (index, line) in text.lines().enumerate() {
                    if line.contains(token) {
                        violations.push(format!(
                            "optimizer-containment ({rule}): {path}:{}: the promote module \
                             references the write/install token {token:?} — the optimizer has \
                             no write path into the active loop's configuration or roles \
                             (plan Task 23 D8a/D10, spec 19.3: \"It may not modify the active \
                             loop\")",
                            index + 1
                        ));
                    }
                }
            }
        }
        // Task 24 E-1: the `write!` sink-write rule (rule A's macro
        // class) — a `write!` targeting anything but the fmt formatter is
        // a file/sink write into the active loop's surfaces.
        for (line, target) in write_macro_sink_lines(text) {
            violations.push(format!(
                "optimizer-containment (rule A): {path}:{line}: the promote module writes \
                 through the write! macro to a non-formatter target {target:?} — the \
                 optimizer has no write path into the active loop's configuration or roles \
                 (plan Task 23 D8a/D10, spec 19.3: \"It may not modify the active loop\")"
            ));
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

/// The repo scan: the REAL promote module carries no banned shape.
#[cfg(test)]
mod optimizer_containment_tests {
    use super::*;

    fn real_promote_sources() -> BTreeMap<String, String> {
        let root = crate::walk::repo_root();
        let src = root.join(PROMOTE_MODULE_SRC);
        let mut files = BTreeMap::new();
        collect_rs(&src, &root, &mut files);
        files
    }

    fn collect_rs(
        dir: &std::path::Path,
        root: &std::path::Path,
        files: &mut BTreeMap<String, String>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs(&path, root, files);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.insert(rel, text);
            }
        }
    }

    #[test]
    fn the_promote_module_writes_no_configuration_and_installs_no_roles() {
        let files = real_promote_sources();
        assert!(
            files.len() >= 6,
            "the promote module's sources must be loadable: {:?}",
            files.keys()
        );
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations.is_empty(),
            "the optimizer's module must never reference a config-write or roles-install \
             surface:\n{}",
            violations.join("\n")
        );
    }

    // ---------------------------------------------------------------------------
    // Negative controls: the scan must REPORT each banned shape. The
    // fixtures are planted here only — asserted and never committed to the
    // promote module.
    // ---------------------------------------------------------------------------

    fn fixture_map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    const VIOLATING_CONFIG_WRITE: &str = "\
    use crate::config::resolve::{FlagOverrides, FlagValue};
    fn sneak() -> u32 {
        let mut flags = FlagOverrides::new();
        flags.set_whole_verification(6);
        6
    }
    fn set_whole(basis_points: u64) {
        let _ = basis_points;
    }
    ";

    const VIOLATING_ROLES_INSTALL: &str = "\
    use gauntlet_app::roles::install::{InstallInput, InstallReport};
    fn install_roles(input: &InstallInput<'_>) -> InstallReport {
        gauntlet_app::roles::install::install(input).expect(&format!(\"{:?}\", \"never\"))
    }
    ";

    #[test]
    fn the_control_proves_rule_a_has_teeth() {
        let files = fixture_map(&[
            (
                "crates/gauntlet-app/src/promote/proposal.rs",
                VIOLATING_CONFIG_WRITE,
            ),
            (
                "crates/gauntlet-app/src/promote/gate.rs",
                "pub struct Gate;\n",
            ),
        ]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rule A") && v.contains("FlagOverrides")),
            "a config-write reference in the promote module must be REPORTED: {violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("set_whole(")),
            "the set_whole token must be REPORTED: {violations:?}"
        );
    }

    #[test]
    fn the_control_proves_rule_b_has_teeth() {
        let files = fixture_map(&[
            (
                "crates/gauntlet-app/src/promote/rollback.rs",
                VIOLATING_ROLES_INSTALL,
            ),
            (
                "crates/gauntlet-app/src/promote/gate.rs",
                "pub struct Gate;\n",
            ),
        ]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rule B") && v.contains("InstallInput")),
            "a roles-install reference in the promote module must be REPORTED: {violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("::install")),
            "the ::install path token must be REPORTED: {violations:?}"
        );
    }

    /// The falsifier's probe-H fixture shape (COR-2, F-6): a promote
    /// module that rewrites GAUNTLET.md and the roles dir through the RAW
    /// fs / env-mutation / process-spawn surfaces — passing the original
    /// token list with ZERO violations while violating "the optimizer may
    /// not modify the active loop". Planted here only — asserted and
    /// never committed to the promote module.
    const VIOLATING_RAW_WRITE: &str = "\
    use std::fs;
    use std::process::Command;
    fn rewrite(project: &std::path::Path) {
        std::fs::write(project.join(\"GAUNTLET.md\"), b\"amended\").unwrap();
        std::fs::write(project.join(\".agents/roles/critic.md\"), b\"weaker\").unwrap();
        std::env::set_var(\"GAUNTLET_REVIEW_MODE\", \"single\");
        Command::new(\"cp\").args([\"-r\", \"roles\", \"project\"]).status().unwrap();
        let _ = fs::metadata(project);
    }
    ";

    #[test]
    fn the_control_proves_the_raw_write_classes_have_teeth() {
        // COR-2 (F-6, falsifier probe H verbatim): the raw write /
        // env-mutation / process-spawn classes were outside the scan's
        // token list — this fixture reported ZERO violations. Every class
        // it carries must be REPORTED.
        let files = fixture_map(&[
            (
                "crates/gauntlet-app/src/promote/proposal.rs",
                VIOLATING_RAW_WRITE,
            ),
            (
                "crates/gauntlet-app/src/promote/gate.rs",
                "pub struct Gate;\n",
            ),
        ]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            !violations.is_empty(),
            "a promote module rewriting the active loop through raw fs/env/process must be \
             REPORTED: {violations:?}"
        );
        for class in ["fs::write(", "set_var(", "Command::new"] {
            assert!(
                violations.iter().any(|violation| violation.contains(class)),
                "the raw {class:?} shape in the promote module must be REPORTED: {violations:?}"
            );
        }
    }

    #[test]
    fn every_raw_write_env_and_spawn_token_is_reported_individually() {
        // COR-2 (F-6): each added rule-A token has its own planted shape —
        // a scan token no fixture exercises is decoration, not containment.
        const SHAPES: &[(&str, &str)] = &[
            (
                "fs::write(",
                "    std::fs::write(path, b\"bytes\").unwrap();",
            ),
            (
                "File::create",
                "    let handle = std::fs::File::create(path).unwrap();",
            ),
            (
                "OpenOptions",
                "    let opened = std::fs::OpenOptions::new().write(true).open(path);",
            ),
            (
                "set_var(",
                "    std::env::set_var(\"GAUNTLET_REVIEW_MODE\", \"single\");",
            ),
            (
                "Command::new",
                "    std::process::Command::new(\"cp\").status().unwrap();",
            ),
        ];
        for (token, line) in SHAPES {
            let source = format!("fn sneak(project: &std::path::Path) {{\n{line}\n}}\n");
            let files = fixture_map(&[(
                "crates/gauntlet-app/src/promote/metrics.rs",
                source.as_str(),
            )]);
            let violations = scan_optimizer_containment(&files);
            assert!(
                violations.iter().any(|violation| violation.contains(token)),
                "the raw write token {token:?} must be REPORTED in the promote module: \
                 {violations:?}"
            );
        }
    }

    #[test]
    fn every_task24_file_mutation_token_is_reported_individually() {
        // Task 24 E-1: each NEW raw file-mutation class has its own
        // planted shape — rename/copy/hard-link rewrite the active loop's
        // files without ever spelling `fs::write`; an options-opened
        // handle is a write path `File::create` does not cover; removal
        // destroys state. A token no fixture exercises is decoration.
        const SHAPES: &[(&str, &str)] = &[
            ("fs::rename(", "    std::fs::rename(from, to).unwrap();"),
            ("fs::copy(", "    std::fs::copy(from, to).unwrap();"),
            (
                "File::options",
                "    let handle = std::fs::File::options().append(true).open(p).unwrap();",
            ),
            (
                "fs::hard_link(",
                "    std::fs::hard_link(target, link).unwrap();",
            ),
            (
                "fs::remove_file(",
                "    std::fs::remove_file(path).unwrap();",
            ),
        ];
        for (token, line) in SHAPES {
            let source = format!("fn sneak(project: &std::path::Path) {{\n{line}\n}}\n");
            let files = fixture_map(&[(
                "crates/gauntlet-app/src/promote/metrics.rs",
                source.as_str(),
            )]);
            let violations = scan_optimizer_containment(&files);
            assert!(
                violations.iter().any(|violation| violation.contains(token)),
                "the file-mutation token {token:?} must be REPORTED in the promote module: \
                 {violations:?}"
            );
        }
    }

    #[test]
    fn a_write_macro_to_a_sink_is_reported_and_the_formatter_shape_is_not() {
        // Task 24 E-1: the write! rule is the TARGET, not the macro. A
        // sink write (`write!(file, …)` — any non-formatter target) is
        // REPORTED; the Display/Error formatter call (`write!(f, …)`,
        // same line AND wrapped onto the next line — the module's real
        // shapes) is legitimate and must stay unflagged, or the scan
        // would outlaw the module's own error rendering.
        let sink = "\
        fn sneak(path: &std::path::Path) {\n\
        \x20   let mut file = std::fs::File::create(path).unwrap();\n\
        \x20   write!(file, \"amended criteria\").unwrap();\n\
        }\n";
        let files = fixture_map(&[("crates/gauntlet-app/src/promote/metrics.rs", sink)]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("write!")),
            "a write! to a file handle must be REPORTED: {violations:?}"
        );
        // The wrapped sink shape: target on the next line.
        let wrapped_sink = "\
        fn sneak(path: &std::path::Path) {\n\
        \x20   let mut out = std::fs::File::create(path).unwrap();\n\
        \x20   write!(\n\
        \x20       out,\n\
        \x20       \"amended\"\n\
        \x20   )\n\
        \x20   .unwrap();\n\
        }\n";
        let files = fixture_map(&[("crates/gauntlet-app/src/promote/metrics.rs", wrapped_sink)]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("write!")),
            "a wrapped write! to a sink must be REPORTED too: {violations:?}"
        );
        // The formatter shapes are NOT violations: same line and wrapped.
        let formatter = "\
        impl core::fmt::Display for Error {\n\
        \x20   fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {\n\
        \x20       match self {\n\
        \x20           Self::One => write!(f, \"one\"),\n\
        \x20           Self::Two => write!(\n\
        \x20               f,\n\
        \x20               \"two\"\n\
        \x20           ),\n\
        \x20       }\n\
        \x20   }\n\
        }\n";
        let files = fixture_map(&[("crates/gauntlet-app/src/promote/metrics.rs", formatter)]);
        let violations = scan_optimizer_containment(&files);
        assert!(
            violations
                .iter()
                .all(|violation| !violation.contains("write!")),
            "the Display formatter shape is legitimate and must not be REPORTED: {violations:?}"
        );
    }

    #[test]
    fn the_control_proves_a_missing_module_cannot_pass_silently() {
        let violations = scan_optimizer_containment(&BTreeMap::new());
        assert!(
            violations.iter().any(|v| v.contains("missing")),
            "an empty source set must be a violation, not a pass: {violations:?}"
        );
    }

    #[test]
    fn the_scan_ignores_files_outside_the_promote_module() {
        // The roles module ITSELF names InstallInput — that is the
        // surface, not a violation of it. Only the promote module is
        // policed.
        let files = fixture_map(&[(
            "crates/gauntlet-app/src/roles/install.rs",
            "pub struct InstallInput<'a>;\n",
        )]);
        assert!(scan_optimizer_containment(&files).is_empty());
    }
}
