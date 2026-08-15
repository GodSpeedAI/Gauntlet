//! Architecture boundary tests (spec §24.3, plan Task 2): the layering of
//! spec §4.1 enforced mechanically, not by review.
//!
//! # Instruments and their evidence locators
//!
//! The frozen answer key addresses this work at `tests/contract/…`. Those
//! locators are real cargo test targets, declared in this crate's manifest
//! and living at the repo-root `tests/contract/` directory, each calling the
//! `check_*` function beside it here (recritique R13/R16/R28):
//!
//! | locator | function | Unit |
//! |---|---|---|
//! | `tests/contract/architecture_boundaries` | [`check_dependency_direction`], [`check_scope_integrity`] | AK-001 |
//! | `tests/contract/architecture_boundaries::denylist` | [`check_denylist`] | AK-002 |
//! | `tests/contract/adapter_registry::selection_is_config_driven` | [`check_adapter_selection`] | AK-014 |
//! | `tests/contract/no_hardcoded_literals` | [`check_no_hardcoded_literals`] | AK-016 |
//! | `tests/contract/env_example_complete` | [`check_env_example`] | AK-019 |
//!
//! # Mechanisms
//!
//! - [`deps`] — workspace dependency direction (§4.1 "dependencies point
//!   inward"), over a member set DISCOVERED from the filesystem rather than
//!   trusted from `[workspace].members`;
//! - [`walk`] + [`escapes`] — scope integrity: what gets scanned must equal
//!   what compiles. Symlinks, unreadable/oversized/non-UTF-8 files, and
//!   `#[path]`/`include!` targets that escape a crate directory are all
//!   violations, never skips (I8: missing evidence is not passing evidence);
//! - [`denylist`] — provider/harness/database/browser/sandbox identifiers, and
//!   adapter identifiers derived from the workspace itself, confined to
//!   `crates/adapters/**` and configuration;
//! - [`literals`] — no environment hardcoded in core sources: paths, URLs,
//!   model ids, endpoints, ports, commands, timeouts, tier policy, tool
//!   identity;
//! - [`envfile`] — `.env.example` diffed against spec Annex E.
//!
//! Run with `just boundaries` (`cargo nextest run -p gauntlet-arch-tests`).
//! This crate is test-only: not a dependency of anything, and it depends on
//! no internal crate. Its own sources necessarily contain the patterns it
//! polices, which is harmless because its scope is the four core crates —
//! this crate is outside that scope by construction.

pub mod denylist;
pub mod deps;
pub mod envfile;
pub mod escapes;
pub mod literals;
pub mod text;
pub mod walk;

use std::path::Path;

pub use walk::repo_root;

/// Render a violation list as one assertion message. Each entry already
/// carries its own typed prefix, file, and line.
pub fn summarize(kind: &str, violations: &[String]) -> String {
    if violations.is_empty() {
        return format!("{kind}: no violations");
    }
    let mut message = format!("{kind}: {} violation(s)", violations.len());
    for violation in violations {
        message.push_str("\n  - ");
        message.push_str(violation);
    }
    message
}

/// What a repo-level source scan actually read, so its callers can assert on
/// the POST-filter set rather than on the pre-filter file list (recritique
/// R30, class C1).
pub struct ScanReport {
    /// Files that passed the scope filter AND were read successfully.
    pub scanned: Vec<String>,
    /// Files that passed the scope filter and could NOT be read. Every one of
    /// them is also a violation; the count exists so callers can assert it is
    /// zero without inspecting message text.
    pub skipped: usize,
    pub violations: Vec<String>,
}

fn read_root_manifest(root: &Path) -> String {
    std::fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest is readable")
}

fn members_of(root: &Path) -> Vec<deps::Member> {
    let text = read_root_manifest(root);
    deps::read_members(root, &text).unwrap_or_else(|e| panic!("workspace members resolve: {e}"))
}

/// (a) AK-001 — dependency direction, over a discovered member set.
pub fn check_dependency_direction(root: &Path) -> Vec<String> {
    let root_text = read_root_manifest(root);
    let members = match deps::read_members(root, &root_text) {
        Ok(members) => members,
        Err(err) => return vec![format!("dependency-direction: {err}")],
    };
    let mut violations = Vec::new();

    // Anti-vacuity: every layer named in the Task 2 crate set must be
    // present — a glob that silently matched nothing would otherwise make
    // this check pass emptily.
    for expected in [
        "gauntlet-domain",
        "gauntlet-ports",
        "gauntlet-app",
        "gauntlet-cli",
        "gauntlet-arch-tests",
    ] {
        if !members.iter().any(|m| m.name == expected) {
            violations.push(format!(
                "dependency-direction: expected workspace member {expected} is missing"
            ));
        }
    }
    if !members
        .iter()
        .any(|m| m.dir.starts_with("crates/adapters/"))
    {
        violations.push("dependency-direction: no adapter crates found in the workspace".into());
    }

    // Class A1: the filesystem, not the members array, decides what exists.
    violations.extend(deps::undeclared_packages(
        &deps::discover_package_dirs(root),
        &members,
    ));

    let root_table = match root_text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(err) => return vec![format!("dependency-direction: workspace Cargo.toml: {err}")],
    };
    let workspace_deps = deps::workspace_dep_names(&root_table);
    if workspace_deps.is_empty() {
        violations
            .push("dependency-direction: root [workspace.dependencies] resolved to nothing".into());
    }
    violations.extend(deps::check_workspace_dependency_paths(
        &root_table,
        &members,
    ));

    for member in &members {
        let manifest_path = format!("{}/Cargo.toml", member.dir);
        let text = std::fs::read_to_string(root.join(&manifest_path))
            .unwrap_or_else(|e| panic!("{manifest_path} is readable: {e}"));
        violations.extend(deps::check_member(member, &text, &members, &workspace_deps));
    }
    violations
}

/// Scope integrity: symlinks, unreadable files, and `#[path]`/`include!`
/// targets that escape their crate (classes A2/A3/A4).
pub fn check_scope_integrity(root: &Path) -> Vec<String> {
    let tree = walk::core_tree(root);
    let mut violations = tree.violations;
    for file in &tree.files {
        if !walk::in_denylist_scope(file) {
            continue;
        }
        let text = match walk::read_scannable(root, file) {
            Ok(text) => text,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        if let Some(crate_dir) = walk::core_crate_dir_of(file) {
            violations.extend(escapes::scan_escapes(file, crate_dir, &text));
        }
    }
    violations
}

/// Read every in-scope core file once, applying `scan` to each.
fn scan_core_tree(
    root: &Path,
    in_scope: fn(&str) -> bool,
    mut scan: impl FnMut(&str, &str, denylist::Role) -> Vec<String>,
) -> ScanReport {
    let tree = walk::core_tree(root);
    let mut report = ScanReport {
        scanned: Vec::new(),
        skipped: 0,
        violations: tree.violations,
    };
    for file in &tree.files {
        if !in_scope(file) {
            continue;
        }
        match walk::read_scannable(root, file) {
            Ok(text) => {
                let role = if walk::in_adapter_identifier_scope(file) {
                    denylist::Role::Inner
                } else {
                    denylist::Role::CompositionRoot
                };
                report.violations.extend(scan(file, &text, role));
                report.scanned.push(file.clone());
            }
            Err(violation) => {
                report.skipped += 1;
                report.violations.push(violation);
            }
        }
    }
    report
}

/// Anti-vacuity for a repo-level scan (class C1): the post-filter set must
/// cover every core file except the declared exemptions, must include each
/// core crate's manifest and at least one source, and must have skipped
/// nothing.
fn assert_scan_is_not_vacuous(
    root: &Path,
    label: &str,
    report: &ScanReport,
    in_scope: fn(&str) -> bool,
) -> Vec<String> {
    let tree = walk::core_tree(root);
    let mut violations = Vec::new();

    let core_files: Vec<&String> = tree
        .files
        .iter()
        .filter(|f| walk::is_core_path(f))
        .collect();
    let expected: Vec<&&String> = core_files.iter().filter(|f| in_scope(f)).collect();
    if expected.len() != report.scanned.len() {
        violations.push(format!(
            "{label}: scanned {} of {} in-scope core files — the scope predicate and the scan \
             disagree",
            report.scanned.len(),
            expected.len()
        ));
    }
    // The scope predicate itself must not have grown an exclusion: every core
    // file is in scope except the declared exemptions.
    for file in &core_files {
        if !in_scope(file) && **file != walk::DEFAULTS_MODULE {
            violations.push(format!(
                "{label}: '{file}' is a core file that the scope predicate excludes, and it is \
                 not the declared defaults module — no subtree may be carved out of the scan"
            ));
        }
    }
    // The floor is derived from the discovered crate set, not a constant that
    // happens to equal today's count.
    for dir in walk::CORE_CRATES {
        let manifest = format!("{dir}/Cargo.toml");
        if in_scope(&manifest) && !report.scanned.contains(&manifest) {
            violations.push(format!("{label}: scope is missing {manifest}"));
        }
        let prefix = format!("{dir}/");
        if !report
            .scanned
            .iter()
            .any(|f| f.starts_with(&prefix) && f.ends_with(".rs"))
        {
            violations.push(format!(
                "{label}: no Rust source under {dir} was scanned — the walker or crate layout is \
                 broken"
            ));
        }
    }
    if report.skipped != 0 {
        violations.push(format!(
            "{label}: {} in-scope file(s) were not readable — missing evidence is not passing \
             evidence (I8)",
            report.skipped
        ));
    }
    violations
}

/// (b) AK-002 — the source denylist over the core crates.
pub fn check_denylist(root: &Path) -> Vec<String> {
    let deny = denylist::Denylist::derive(&members_of(root));
    if deny.adapter.is_empty() {
        return vec![
            "denied-identifier: no adapter identifiers were derived from the workspace — the \
             AK-002/AK-014 half of the denylist would be vacuous"
                .to_string(),
        ];
    }
    let report = scan_core_tree(root, walk::in_denylist_scope, |file, text, role| {
        let mut out = denylist::scan_source(file, text, &deny, role);
        if file.ends_with("/Cargo.toml") {
            out.extend(denylist::scan_manifest(file, text, &deny, role));
        }
        out
    });
    let mut violations = report.violations.clone();
    violations.extend(assert_scan_is_not_vacuous(
        root,
        "denied-identifier",
        &report,
        walk::in_denylist_scope,
    ));
    violations
}

/// AK-014 — adapter selection resolves from configuration only: no adapter
/// identifier is a literal in domain or application code.
pub fn check_adapter_selection(root: &Path) -> Vec<String> {
    let full = denylist::Denylist::derive(&members_of(root));
    if full.adapter.is_empty() {
        return vec![
            "adapter-selection: no adapter identifiers were derived from the workspace — this \
             check would be vacuous"
                .to_string(),
        ];
    }
    let adapters_only = denylist::Denylist {
        vendor: Vec::new(),
        adapter: full.adapter,
        adapter_crate_names: full.adapter_crate_names,
    };
    let report = scan_core_tree(root, walk::in_adapter_identifier_scope, |file, text, _| {
        let mut out = denylist::scan_source(file, text, &adapters_only, denylist::Role::Inner);
        if file.ends_with("/Cargo.toml") {
            out.extend(denylist::scan_manifest(
                file,
                text,
                &adapters_only,
                denylist::Role::Inner,
            ));
        }
        out
    });
    let mut violations = report.violations.clone();
    for dir in walk::INNER_CRATES {
        let manifest = format!("{dir}/Cargo.toml");
        if !report.scanned.contains(&manifest) {
            violations.push(format!("adapter-selection: scope is missing {manifest}"));
        }
    }
    if report.skipped != 0 {
        violations.push(format!(
            "adapter-selection: {} in-scope file(s) were not readable",
            report.skipped
        ));
    }
    violations
}

/// (c) AK-016 — the hardcoded-literal scan over the core crates.
pub fn check_no_hardcoded_literals(root: &Path) -> Vec<String> {
    let report = scan_core_tree(root, walk::in_literal_scope, |file, text, _| {
        literals::scan_literals(file, text)
    });
    let mut violations = report.violations.clone();
    violations.extend(assert_scan_is_not_vacuous(
        root,
        "hardcoded-literal",
        &report,
        walk::in_literal_scope,
    ));
    violations
}

/// No exemption may be silently vacuous (class C2, recritique R31): the
/// declared defaults module must either exist AND be the spec §8.6 defaults
/// table, or not exist — in which case the exemption is inactive and nothing
/// in the tree is exempt. The vendor denylist applies to it either way.
pub fn check_defaults_exemption(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    if !walk::in_denylist_scope(walk::DEFAULTS_MODULE) {
        violations.push(format!(
            "vacuous-exemption: {} is exempt from the denylist as well as the literal scan — the \
             §8.6 defaults table holds values, never vendor identifiers",
            walk::DEFAULTS_MODULE
        ));
    }
    let path = root.join(walk::DEFAULTS_MODULE);
    if path.is_file() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lowered = text.to_lowercase();
        for setting in walk::DEFAULTS_TABLE_SETTINGS {
            if !lowered.contains(setting) {
                violations.push(format!(
                    "vacuous-exemption: {} claims the spec §8.6 defaults-table exemption but does \
                     not mention '{setting}' — the exemption must name what it exempts",
                    walk::DEFAULTS_MODULE
                ));
            }
        }
        return violations;
    }
    // The file does not exist, so the exemption must currently be inactive:
    // no walked file may be treated differently by the two scopes.
    for file in walk::core_tree(root).files {
        if walk::in_denylist_scope(&file) != walk::in_literal_scope(&file) {
            violations.push(format!(
                "vacuous-exemption: '{file}' is exempt from the literal scan while the declared \
                 defaults module {} does not exist — an exemption that points at nothing must \
                 exempt nothing",
                walk::DEFAULTS_MODULE
            ));
        }
    }
    violations
}

/// AK-019 — `.env.example` against spec Annex E.
pub fn check_env_example(root: &Path) -> Vec<String> {
    let spec_path = root.join(".agents/specs/Core-spec.md");
    let env_path = root.join(".env.example");
    let spec = match std::fs::read_to_string(&spec_path) {
        Ok(text) => text,
        Err(err) => {
            return vec![format!(
                "env-example: .agents/specs/Core-spec.md is not readable ({err}) — Annex E is the \
                 source of truth for this check"
            )];
        }
    };
    let env = match std::fs::read_to_string(&env_path) {
        Ok(text) => text,
        Err(err) => {
            return vec![format!(
                "env-example: .env.example is not readable ({err}) — AK-019 requires the file to \
                 exist"
            )];
        }
    };
    envfile::check_env_example(&spec, &env)
}
