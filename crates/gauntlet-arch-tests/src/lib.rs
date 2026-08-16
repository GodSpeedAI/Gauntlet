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
//!   model ids, providers, endpoints, ports, commands, timeouts, tier
//!   policy, tool identity;
//! - [`envfile`] — `.env.example` diffed against spec Annex E.
//!
//! Run with `just boundaries` (`cargo nextest run -p gauntlet-arch-tests`).
//! This crate is test-only: not a dependency of anything, and it depends on
//! no internal crate. Its own sources necessarily contain the patterns it
//! polices, which is harmless because its scope is the four core crates —
//! this crate is outside that scope by construction.
//!
//! # THREAT MODEL — what this suite enforces, and where its authority ends
//!
//! This section exists so that a reviewer can tell the difference between a
//! gap nobody thought of and a line somebody drew. Everything below the second
//! heading is a limit accepted on purpose, not an oversight.
//!
//! ## What it enforces
//!
//! The suite polices **this repository's own declared build inputs**: every
//! workspace member manifest, the root manifest (its `[workspace.dependencies]`,
//! `[patch]` and `[replace]` tables included), every in-repo cargo
//! configuration (`.cargo/config.toml` and the legacy `.cargo/config`,
//! ANYWHERE in the tree — cargo reads them from the invocation directory
//! upward, so a config inside a crate directory is a build input too), the
//! `rust-toolchain.toml` pin, and every in-tree file under the four core
//! crates. Within that surface the aim is
//! evasion-resistance rather than tidiness: matching runs over normalized scan
//! units so a literal split by a line continuation, by `concat!`, by an escape,
//! or by a statement break is still found; membership comes from the
//! filesystem rather than from `[workspace].members`; external dependencies are
//! admitted by a (crate, source) pair against a per-layer allowlist that is
//! empty by default; and the walk's own carve-outs are measured against an
//! independent listing so a narrowed scope fails the gate instead of shrinking
//! it. Input the suite cannot read — an unreadable, over-sized, non-UTF-8, or
//! symlinked file, an `include!` whose argument is computed, a declaration too
//! long to join, a file that ends inside a string literal — is reported as a
//! violation, never skipped (invariant I8: missing evidence is not passing
//! evidence).
//!
//! ## What it does not and cannot enforce
//!
//! - **The developer's environment outside the repository.**
//!   `$CARGO_HOME/config.toml`, `RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, any other
//!   `CARGO_*` variable, and a locally modified toolchain all reach the same
//!   compiles as the in-repo config this suite reads — and none of them is a
//!   file in this repository, so no file-based instrument can observe them.
//!   Closing that channel needs a hermetic or containerized build, which is an
//!   infrastructure decision, not a test.
//! - **The CONTENTS of an admitted third-party crate.** The allowlist admits by
//!   identity and source (name + default registry); it does not audit code it
//!   never scanned. `cargo deny` covers licences and advisories for that code;
//!   nothing here reads it. The mitigation available to this suite is to keep
//!   every allowlist empty, which it does except for the one manifest parser
//!   the boundary suite itself needs.
//! - **Proc-macro and build-script code generation**, beyond the path-resolution
//!   checks already present (a build script in a core crate is a violation; an
//!   `include!` of a computed path is a violation). What a macro EXPANDS TO is
//!   decided at compile time by code, so a scanner of source text cannot see
//!   it; that would need expansion or a real Rust front end.
//! - **Runtime string assembly.** `Command::new(var)`, `["mo", "ck"].join("")`,
//!   a denied identifier concatenated from configuration at run time: the
//!   compiled program contains no literal to match. Static undecidability, not
//!   an omission.
//!
//! ## Residuals known and accepted
//!
//! Whitespace-split directive spellings beyond those normalized here; spelling
//! variants of a listed tool identity laundered through a const; UNC paths
//! (`\\\\server\\share`), which the absolute-path class does not recognize;
//! well-known ports outside the curated list; an un-prefixed secret token
//! sitting in an `.env.example` comment; an endpoint under a top-level domain
//! nobody listed; a provider or model family nobody listed. Each is a bounded
//! miss of an enumerated list rather than a hole in a channel, and each is
//! recorded beside the class it belongs to in [`literals`], [`denylist`] and
//! [`envfile`].

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
    let discovered = deps::discover_package_dirs(root);
    violations.extend(discovered.violations.clone());
    violations.extend(deps::undeclared_packages(&discovered.dirs, &members));

    let root_table = match root_text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(err) => return vec![format!("dependency-direction: workspace Cargo.toml: {err}")],
    };
    let workspace_deps = deps::workspace_dep_names(&root_table);
    if workspace_deps.is_empty() {
        violations
            .push("dependency-direction: root [workspace.dependencies] resolved to nothing".into());
    }
    let root_deps = deps::workspace_dep_records(&root_table, &members);
    violations.extend(deps::check_workspace_dependency_table(
        &root_table,
        &members,
    ));
    violations.extend(deps::check_patch_and_replace(&root_table, &members));
    // Critic-4 M5: `.cargo/config.toml` source replacement and rustflags
    // redirect what compiles without touching any manifest read above.
    violations.extend(deps::check_cargo_config(root));

    for member in &members {
        let manifest_path = format!("{}/Cargo.toml", member.dir);
        let text = std::fs::read_to_string(root.join(&manifest_path))
            .unwrap_or_else(|e| panic!("{manifest_path} is readable: {e}"));
        violations.extend(deps::check_member(member, &text, &members, &root_deps));
    }
    violations
}

/// Scope integrity: symlinks, unreadable files, `#[path]`/`include!` targets
/// that escape their crate, Cargo targets whose source root is relocated out
/// of the crate, and build scripts (classes A2/A3/A4, round-3 V1/V2/V3).
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
        let Some(crate_dir) = walk::core_crate_dir_of(file) else {
            continue;
        };
        violations.extend(escapes::scan_escapes(file, crate_dir, &text));
        violations.extend(escapes::build_script_violation(file, crate_dir));
        if file.ends_with("/Cargo.toml") {
            violations.extend(escapes::scan_cargo_target_paths(file, crate_dir, &text));
        }
    }
    violations
}

/// Read every in-scope core file once, applying `scan` to each — and guard
/// the result against vacuity BEFORE returning it.
///
/// The guard lives here, inside the shared scan path, on purpose (critic-4
/// vacuity M6): it used to be a separate call each `check_*` function could
/// silently delete while keeping its scan green. Folding it in makes deleting
/// the guard mean deleting it for every check at once, and the v15 test
/// drives this function directly so the wiring stays pinned.
fn scan_core_tree(
    root: &Path,
    label: &str,
    in_scope: fn(&str) -> bool,
    crate_dirs: &[&str],
    declared_exemptions: &[&str],
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
    report.violations.extend(assert_scan_is_not_vacuous(
        root,
        label,
        &report,
        in_scope,
        crate_dirs,
        declared_exemptions,
    ));
    report
}

/// Anti-vacuity for a repo-level scan (class C1): the post-filter set must
/// cover every file under `crate_dirs` except the `declared_exemptions`, must
/// include each of those crates' manifests and at least one source, and must
/// have skipped nothing.
///
/// `crate_dirs` is the universe the instrument claims to police —
/// [`walk::CORE_CRATES`] for the denylist and literal scans,
/// [`walk::INNER_CRATES`] for the adapter-identifier scan. Passing it in is
/// what lets AK-014's instrument carry the same guard as the other two
/// (round-3 V13/V15): an added exclusion inside the layers it polices must
/// fail the gate rather than silently shrink it.
///
/// The universe itself comes from [`walk::unfiltered_listing`], NOT from the
/// walker (critic-4 vacuity M1): the guard used to compute its `policed` set
/// out of the same walker's output, so a carve-out added to the walker shrank
/// both sides in lockstep. Files the walker reports as integrity violations
/// (symlinks, unreadables) are excused — the gate is already red on those.
fn assert_scan_is_not_vacuous(
    root: &Path,
    label: &str,
    report: &ScanReport,
    in_scope: fn(&str) -> bool,
    crate_dirs: &[&str],
    declared_exemptions: &[&str],
) -> Vec<String> {
    let tree = walk::core_tree(root);
    let mut violations = Vec::new();

    // The call-site guard (critic-5 vacuity M-1): the exemption list is an
    // ARGUMENT, and until now an argument nothing checked. Adding an entry at
    // a call site looked exactly like the sanctioned `DEFAULTS_MODULE`
    // pattern and quietly removed a file from a check. An exemption is now
    // only honoured if it is declared, with its reason, in
    // `walk::DECLARED_SCAN_EXEMPTIONS`.
    for exemption in declared_exemptions {
        if !walk::DECLARED_SCAN_EXEMPTIONS
            .iter()
            .any(|(declared, _)| declared == exemption)
        {
            violations.push(format!(
                "{label}: '{exemption}' is passed to the scan as an exemption but is not one of \
                 the declared exemptions {:?} — an exemption is a recorded decision with a stated \
                 reason, not a call-site argument (spec §24.3)",
                walk::DECLARED_SCAN_EXEMPTIONS
                    .iter()
                    .map(|(path, _)| *path)
                    .collect::<Vec<_>>()
            ));
        }
    }

    // The walker-level guard: a file that exists on disk under the policed
    // directories must be listed by the walker or reported by it as an
    // integrity violation. Anything else is a walker carve-out — the first,
    // unpoliced filter in the chain (critic-4 vacuity M1).
    let listed = walk::unfiltered_listing(root, crate_dirs);
    for file in &listed {
        if tree.files.contains(file) {
            continue;
        }
        if tree.violations.iter().any(|v| v.contains(file.as_str())) {
            continue;
        }
        violations.push(format!(
            "{label}: '{file}' exists on disk under {crate_dirs:?} but the walker neither lists \
             nor reports it — a walker-level carve-out is invisible to every guard that measures \
             the walk with the walk"
        ));
    }

    let policed: Vec<&String> = listed.iter().filter(|f| tree.files.contains(f)).collect();
    let expected: Vec<&&String> = policed.iter().filter(|f| in_scope(f)).collect();
    if expected.len() != report.scanned.len() {
        violations.push(format!(
            "{label}: scanned {} of {} in-scope files — the scope predicate and the scan disagree",
            report.scanned.len(),
            expected.len()
        ));
    }
    // The scope predicate itself must not have grown an exclusion: every file
    // under the policed crates is in scope except the declared exemptions.
    for file in &policed {
        if !in_scope(file) && !declared_exemptions.contains(&file.as_str()) {
            violations.push(format!(
                "{label}: '{file}' is a policed file that the scope predicate excludes, and it is \
                 not a declared exemption {declared_exemptions:?} — no subtree may be carved out \
                 of the scan"
            ));
        }
    }
    // The floor is derived from the crate set the instrument polices, not a
    // constant that happens to equal today's count.
    for dir in crate_dirs {
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

/// The root manifest's alias → package map, used to resolve
/// `dep.workspace = true` to the crate it actually compiles. Fails CLOSED on
/// an unparseable root manifest (critic-4 vacuity m1): the old
/// `unwrap_or_default` silently resolved every inherited rename to nothing,
/// and only call order kept that unreachable.
fn workspace_deps_of(
    root: &Path,
    label: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    read_root_manifest(root)
        .parse::<toml::Table>()
        .map(|table| deps::workspace_dep_names(&table))
        .map_err(|err| {
            format!(
                "{label}: the workspace manifest does not parse ({err}) — the alias map must fail \
                 closed, or an unparseable root silently resolves every workspace-inherited \
                 rename to nothing"
            )
        })
}

/// (b) AK-002 — the source denylist over the core crates, plus the one root
/// table core crates inherit from.
pub fn check_denylist(root: &Path) -> Vec<String> {
    let deny = denylist::Denylist::derive(&members_of(root));
    if deny.adapter.is_empty() {
        return vec![
            "denied-identifier: no adapter identifiers were derived from the workspace — the \
             AK-002/AK-014 half of the denylist would be vacuous"
                .to_string(),
        ];
    }
    let workspace_deps = match workspace_deps_of(root, "denied-identifier") {
        Ok(map) => map,
        Err(violation) => return vec![violation],
    };
    let report = scan_core_tree(
        root,
        "denied-identifier",
        walk::in_denylist_scope,
        walk::CORE_CRATES,
        &[],
        |file, text, role| {
            let mut out = denylist::scan_source(file, text, &deny, role);
            if file.ends_with("/Cargo.toml") {
                out.extend(denylist::scan_manifest(
                    file,
                    text,
                    &deny,
                    role,
                    &workspace_deps,
                ));
            }
            out
        },
    );
    let mut violations = report.violations;
    // Round-3 V4: the root `[workspace.dependencies]` table is the one table
    // outside the core crates whose contents compile into them, through a
    // single `dep.workspace = true`. It is scanned as text with the
    // composition-root mask, so the workspace's own adapter entries are legal
    // and a vendor package name is not.
    violations.extend(denylist::scan_source(
        "Cargo.toml",
        &denylist::workspace_dependencies_text(&read_root_manifest(root)),
        &deny,
        denylist::Role::CompositionRoot,
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
        adapter_segments: full.adapter_segments,
    };
    let workspace_deps = match workspace_deps_of(root, "adapter-selection") {
        Ok(map) => map,
        Err(violation) => return vec![violation],
    };
    let report = scan_core_tree(
        root,
        "adapter-selection",
        walk::in_adapter_identifier_scope,
        walk::INNER_CRATES,
        &[],
        |file, text, _| {
            let mut out = denylist::scan_source(file, text, &adapters_only, denylist::Role::Inner);
            if file.ends_with("/Cargo.toml") {
                out.extend(denylist::scan_manifest(
                    file,
                    text,
                    &adapters_only,
                    denylist::Role::Inner,
                    &workspace_deps,
                ));
            }
            out
        },
    );
    let mut violations = report.violations;
    // Round-3 V13/V15 note: the anti-vacuity guard for this scan now lives
    // inside `scan_core_tree` above, like for the other two checks; the
    // universe is INNER_CRATES because excluding the composition root is the
    // declared scope decision (see `walk::COMPOSITION_ROOT`), not a
    // carve-out.
    //
    // The other half of that declared scope decision: the composition root is
    // outside this rule and must stay inside the other two, so the exclusion
    // cannot quietly become an unpoliced region.
    for file in walk::core_tree(root).files {
        if !file.starts_with(&format!("{}/", walk::COMPOSITION_ROOT)) {
            continue;
        }
        if !walk::in_denylist_scope(&file) || !walk::in_literal_scope(&file) {
            violations.push(format!(
                "adapter-selection: '{file}' is outside the adapter-identifier rule as the \
                 composition root, but it has also left the vendor denylist or the literal scan — \
                 the carve-out would make it an unpoliced region (spec §4.1, §24.3)"
            ));
        }
    }
    violations
}

/// (c) AK-016 — the hardcoded-literal scan over the core crates.
pub fn check_no_hardcoded_literals(root: &Path) -> Vec<String> {
    scan_core_tree(
        root,
        "hardcoded-literal",
        walk::in_literal_scope,
        walk::CORE_CRATES,
        &[walk::DEFAULTS_MODULE],
        |file, text, _| literals::scan_literals(file, text),
    )
    .violations
}

/// No exemption may be silently vacuous (class C2, recritique R31, critic-4
/// vacuity M2): the declared defaults module must either exist AND be the
/// spec §8.6 defaults table, or not exist — in which case the exemption is
/// inactive and nothing in the tree is exempt. The vendor denylist applies to
/// it either way.
///
/// The literal-scan carve-out is pinned UNCONDITIONALLY now: every walked file
/// the two scopes treat differently must be exactly [`walk::DEFAULTS_MODULE`],
/// whether or not that file exists. The old cross-check ran only while the
/// module was absent, so the moment it appeared, a sibling (`config/
/// tables.rs`) could be exempted from the literal scan with no warrant check
/// at all (critic-4 vacuity M2b).
///
/// The warrant itself is declaration-shaped, not mention-shaped: the file
/// must carry `const <SETTING>` (case-insensitive) for each §8.6 setting. A
/// doc comment that names the six settings and then hardcodes literals is not
/// a defaults table, and no longer passes (critic-4 vacuity M2a).
pub fn check_defaults_exemption(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    if !walk::in_denylist_scope(walk::DEFAULTS_MODULE) {
        violations.push(format!(
            "vacuous-exemption: {} is exempt from the denylist as well as the literal scan — the \
             §8.6 defaults table holds values, never vendor identifiers",
            walk::DEFAULTS_MODULE
        ));
    }
    // The literal-scan exemption is exactly the declared defaults module,
    // existent or not. Any other file the two scopes treat differently is an
    // unwarranted exemption.
    for file in walk::core_tree(root).files {
        if walk::in_denylist_scope(&file) && !walk::in_literal_scope(&file) {
            if file != walk::DEFAULTS_MODULE {
                violations.push(format!(
                    "vacuous-exemption: '{file}' is exempt from the literal scan but is not the \
                     declared defaults module {} — an exemption needs a warrant, and the only one \
                     on file is the §8.6 defaults table",
                    walk::DEFAULTS_MODULE
                ));
            }
        } else if walk::in_literal_scope(&file) && !walk::in_denylist_scope(&file) {
            violations.push(format!(
                "vacuous-exemption: '{file}' is inside the literal scope but outside the \
                 denylist scope — the denylist must never be narrower than the literal scan"
            ));
        }
    }
    let path = root.join(walk::DEFAULTS_MODULE);
    if path.is_file() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lowered = text.to_lowercase();
        for setting in walk::DEFAULTS_TABLE_SETTINGS {
            if !lowered.contains(&format!("const {setting}")) {
                violations.push(format!(
                    "vacuous-exemption: {} claims the spec §8.6 defaults-table exemption but \
                     does not DECLARE '{setting}' (`const {setting}`) — mentioning the table in \
                     prose is not being the table",
                    walk::DEFAULTS_MODULE
                ));
            }
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

/// End-to-end regression tests for round 3's four critical evasions, driven
/// through the real `check_*` entry points rather than through the scanner
/// functions directly.
///
/// Each critical was a hole in the WIRING as much as in a matcher — a check
/// that was never called, a parameter that was never passed, a file the walk
/// never reached. Unit tests on the scanners cannot see that class of defect,
/// so these build a synthetic workspace, apply the critic's own evasion to it,
/// and assert the corresponding locator fails.
#[cfg(test)]
mod evasion_tests {
    use super::*;
    use std::path::PathBuf;

    struct Repo {
        dir: PathBuf,
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    impl Repo {
        /// A minimal but complete workspace: the four core crates, one
        /// adapter, and the boundary suite, all clean.
        fn clean(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "gauntlet-arch-tests-evasion-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            let repo = Repo { dir };
            repo.write(
                "Cargo.toml",
                "[workspace]\nresolver = \"3\"\nmembers = [\n  \
                 \"crates/gauntlet-domain\",\n  \"crates/gauntlet-ports\",\n  \
                 \"crates/gauntlet-app\",\n  \"crates/gauntlet-cli\",\n  \
                 \"crates/adapters/gauntlet-adapter-state-sqlite\",\n  \
                 \"crates/gauntlet-arch-tests\",\n]\n\n[workspace.dependencies]\ngauntlet-domain \
                 = { path = \"crates/gauntlet-domain\" }\n",
            );
            for (dir, name) in [
                ("crates/gauntlet-domain", "gauntlet-domain"),
                ("crates/gauntlet-ports", "gauntlet-ports"),
                ("crates/gauntlet-app", "gauntlet-app"),
                ("crates/gauntlet-cli", "gauntlet-cli"),
                (
                    "crates/adapters/gauntlet-adapter-state-sqlite",
                    "gauntlet-adapter-state-sqlite",
                ),
                ("crates/gauntlet-arch-tests", "gauntlet-arch-tests"),
            ] {
                repo.write(
                    &format!("{dir}/Cargo.toml"),
                    &format!("[package]\nname = \"{name}\"\nedition = \"2024\"\n"),
                );
                repo.write(&format!("{dir}/src/lib.rs"), "//! Layer documentation.\n");
            }
            repo
        }

        fn write(&self, rel: &str, contents: &str) {
            let path = self.dir.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent directory"))
                .expect("fixture directory is creatable");
            std::fs::write(&path, contents).expect("fixture file is writable");
        }

        fn append(&self, rel: &str, contents: &str) {
            let existing = std::fs::read_to_string(self.dir.join(rel)).unwrap_or_default();
            self.write(rel, &format!("{existing}{contents}"));
        }

        fn root(&self) -> &std::path::Path {
            &self.dir
        }
    }

    /// The fixtures must be clean before an evasion is applied, or "the check
    /// fails" proves nothing.
    #[test]
    fn the_clean_fixture_workspace_passes_every_check() {
        let repo = Repo::clean("baseline");
        for (label, violations) in [
            (
                "dependency-direction",
                check_dependency_direction(repo.root()),
            ),
            ("scope-integrity", check_scope_integrity(repo.root())),
            ("denied-identifier", check_denylist(repo.root())),
            ("adapter-selection", check_adapter_selection(repo.root())),
            (
                "hardcoded-literal",
                check_no_hardcoded_literals(repo.root()),
            ),
        ] {
            assert!(violations.is_empty(), "{}", summarize(label, &violations));
        }
    }

    #[test]
    fn v1_a_cargo_target_path_leaving_the_crate_fails_the_gate() {
        // Round-3 V1: `[lib] path` points the crate ROOT outside the crate, so
        // the walk scans a file that is not compiled and the compiler compiles
        // a file nothing scans.
        let repo = Repo::clean("v1");
        repo.append(
            "crates/gauntlet-domain/Cargo.toml",
            "\n[lib]\npath = \"../../shared/core.rs\"\n",
        );
        repo.write(
            "shared/core.rs",
            "use rusqlite::Connection;\npub const ROOT: &str = \"/home/agent/state\";\n",
        );
        let violations = check_scope_integrity(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("[lib]") && v.contains("shared/core.rs")),
            "{}",
            summarize("scope-integrity", &violations)
        );
    }

    #[test]
    fn v2_a_cfg_attr_path_attribute_fails_the_gate() {
        // Round-3 V2: `all()` is unconditionally true, so rustc compiles
        // `<repo>/vendor/store.rs` as `gauntlet_domain::store`.
        let repo = Repo::clean("v2");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "#[cfg_attr(all(), path = \"../../../vendor/store.rs\")]\nmod store;\n",
        );
        let violations = check_scope_integrity(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("vendor/store.rs")),
            "{}",
            summarize("scope-integrity", &violations)
        );

        // And the whitespace spelling of the same attribute.
        let repo = Repo::clean("v2b");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "#[ path = \"../../../vendor/store.rs\"] mod store;\n",
        );
        assert!(!check_scope_integrity(repo.root()).is_empty());
    }

    #[test]
    fn v3_out_dir_codegen_and_build_scripts_fail_the_gate() {
        // Round-3 V3: the resolver used to grab `OUT_DIR`, join it to the
        // crate directory, and decide "inside".
        let repo = Repo::clean("v3a");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "include!(concat!(env!(\"OUT_DIR\"), \"/wiring.rs\"));\n",
        );
        let violations = check_scope_integrity(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("not a single plain string literal")),
            "{}",
            summarize("scope-integrity", &violations)
        );

        // The build script that fills OUT_DIR is a violation in its own right,
        // with or without a manifest key.
        let repo = Repo::clean("v3b");
        repo.write(
            "crates/gauntlet-app/build.rs",
            "fn main() {\n    let out = std::env::var(\"OUT_DIR\").unwrap();\n    let _ = out;\n}\n",
        );
        let violations = check_scope_integrity(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("build script")),
            "{}",
            summarize("scope-integrity", &violations)
        );
    }

    #[test]
    fn v4_a_workspace_inherited_rename_fails_the_gate() {
        // Round-3 V4: the alias carries no denied term, and the root table
        // where `rusqlite` is spelled out is not a core file.
        let repo = Repo::clean("v4");
        repo.append(
            "Cargo.toml",
            "store = { package = \"rusqlite\", version = \"0.32\" }\n",
        );
        repo.append(
            "crates/gauntlet-domain/Cargo.toml",
            "\n[dependencies]\nstore.workspace = true\n",
        );
        let violations = check_denylist(repo.root());
        assert!(
            violations.iter().any(
                |v| v.contains("crates/gauntlet-domain/Cargo.toml") && v.contains("'rusqlite'")
            ),
            "the inherited rename was not resolved: {}",
            summarize("denied-identifier", &violations)
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("denied-identifier: Cargo.toml:")),
            "the root [workspace.dependencies] table is not in denylist scope: {}",
            summarize("denied-identifier", &violations)
        );
    }

    #[test]
    fn v5_an_adapter_comment_no_longer_launders_a_vendor_call() {
        // Round-3 V5/V14: the carve-out skipped the whole scan unit.
        let repo = Repo::clean("v5");
        repo.write(
            "crates/gauntlet-cli/src/lib.rs",
            "pub fn open(p: &str) {\n    let _c = rusqlite::Connection::open(p); // \
             gauntlet_adapter_state_sqlite\n}\n",
        );
        let violations = check_denylist(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("'rusqlite'")),
            "{}",
            summarize("denied-identifier", &violations)
        );
    }

    /// Round-3 V13/V15, reworked for critic-4 vacuity M6: the anti-vacuity
    /// guard now lives INSIDE `scan_core_tree`, so this test drives the shared
    /// scan path with exactly the critic's narrowed predicate. Deleting the
    /// guard wiring now means deleting it for every check at once — and this
    /// test fails.
    #[test]
    fn v15_a_narrowed_adapter_identifier_scope_fails_the_gate() {
        fn narrowed(rel: &str) -> bool {
            walk::in_adapter_identifier_scope(rel) && !rel.contains("/registry/")
        }

        let repo = Repo::clean("v15");
        repo.write(
            "crates/gauntlet-app/src/registry/select.rs",
            "pub fn store_for(mode: &str) -> &'static str {\n    match mode { \"durable\" => \
             \"state-sqlite\", _ => \"sqlite-state\" }\n}\n",
        );

        // With the real predicate the subtree is scanned and the selection is
        // reported, so the fixture is a genuine evasion target.
        assert!(!check_adapter_selection(repo.root()).is_empty());

        // With the narrowed predicate the scan says nothing — and the guard
        // inside the shared path must turn that silence into a failure.
        let report = scan_core_tree(
            repo.root(),
            "adapter-selection",
            narrowed,
            walk::INNER_CRATES,
            &[],
            |_, _, _| Vec::new(),
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-app/src/registry/select.rs")
                    && v.contains("no subtree may be carved out")),
            "{}",
            summarize("adapter-selection", &report.violations)
        );
    }

    /// The same guard wiring for the other two repo-level scans: a carve-out
    /// of a policed subtree in the denylist or literal scope must fail through
    /// the shared scan path, whatever the check around it would say.
    #[test]
    fn a_narrowed_denylist_or_literal_scope_fails_through_the_shared_scan_path() {
        fn narrowed(rel: &str) -> bool {
            walk::in_denylist_scope(rel) && !rel.contains("/generated/")
        }

        let repo = Repo::clean("narrowed");
        repo.write(
            "crates/gauntlet-app/src/generated/wiring.rs",
            "pub const STORE: &str = \"sqlite\";\npub const ROOT: &str = \"/tmp/gauntlet/state\";\n",
        );

        // The real predicates report the file (vendor term, absolute path).
        assert!(!check_denylist(repo.root()).is_empty());
        assert!(!check_no_hardcoded_literals(repo.root()).is_empty());

        let report = scan_core_tree(
            repo.root(),
            "denied-identifier",
            narrowed,
            walk::CORE_CRATES,
            &[],
            |_, _, _| Vec::new(),
        );
        assert!(
            report.violations.iter().any(|v| v
                .contains("crates/gauntlet-app/src/generated/wiring.rs")
                && v.contains("no subtree may be carved out")),
            "{}",
            summarize("denied-identifier", &report.violations)
        );
    }

    /// Critic-4 vacuity M1: a carve-out in the WALKER (not the predicate) used
    /// to be invisible to every guard, because the guard measured the walk
    /// with the walk. The guard's universe is now an independent listing, so a
    /// file the walker silently drops fails every repo-level check.
    #[test]
    fn a_walker_level_carve_out_fails_the_gate() {
        // This cannot edit the walker, so it simulates the critic's edit the
        // only way an outside test can: a policed file that the walker's
        // output no longer contains. The independent listing still sees it on
        // disk, and the guard reports the disagreement.
        let repo = Repo::clean("walkercarve");
        repo.write(
            "crates/gauntlet-app/src/generated/wiring.rs",
            "pub const STORE: &str = \"sqlite\";\n",
        );

        // Ground truth: with the real walker the file is scanned and flagged.
        assert!(!check_denylist(repo.root()).is_empty());

        // A scan fed a walker output that silently excludes the file reports
        // nothing itself — the guard must still fire, from the independent
        // listing, not from the (compromised) walk.
        fn excluding_walker_scope(rel: &str) -> bool {
            walk::in_denylist_scope(rel) && !rel.contains("/generated/")
        }
        let report = scan_core_tree(
            repo.root(),
            "denied-identifier",
            excluding_walker_scope,
            walk::CORE_CRATES,
            &[],
            |_, _, _| Vec::new(),
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-app/src/generated/wiring.rs")),
            "{}",
            summarize("denied-identifier", &report.violations)
        );
        // And the listing itself is the difference: it contains the file the
        // walker-equivalent scope would drop.
        let listed = walk::unfiltered_listing(repo.root(), walk::CORE_CRATES);
        assert!(listed.contains(&"crates/gauntlet-app/src/generated/wiring.rs".to_string()));
    }

    #[test]
    fn majors_that_defeated_a_class_fail_the_gate() {
        // V16: a shell metacharacter used to disable the command class.
        let repo = Repo::clean("v16");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const BUILD_CMD: &str = \"cargo build --workspace 2>&1\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-command")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );

        // V17: a rustfmt line break used to split identifier from value.
        let repo = Repo::clean("v17");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const GAUNTLET_BUILDER_INVOCATION_STALL_TIMEOUT_MILLISECONDS: u64 =\n    \
             900_000;\npub static REVIEW_MODE: &str =\n    \"adversarial\";\n",
        );
        let violations = check_no_hardcoded_literals(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // V6: a single escaped character used to hide a denied term.
        let repo = Repo::clean("v6");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "pub const STORE: &str = \"\\x73qlite\";\npub const ROOT: &str = \
             \"/\\x74mp/gauntlet/state\";\n",
        );
        assert!(
            check_denylist(repo.root())
                .iter()
                .any(|v| v.contains("'sqlite'")),
            "{:?}",
            check_denylist(repo.root())
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );

        // V7: adapter selection by the shortest distinguishing segment.
        let repo = Repo::clean("v7");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub fn runner_for(role: &str) -> &'static str {\n    match role { \"builder\" => \
             \"sqlite\", _ => \"sqlite\" }\n}\n",
        );
        assert!(
            !check_adapter_selection(repo.root()).is_empty(),
            "AK-014's own locator must fail on adapter selection by literal"
        );

        // V9: a `[patch]` entry substituting an out-of-tree crate.
        let repo = Repo::clean("v9");
        repo.append(
            "Cargo.toml",
            "\n[patch.crates-io]\nserde = { path = \"../gauntlet-infra/serde-shim\" }\n",
        );
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("[patch.crates-io]")),
            "{:?}",
            check_dependency_direction(repo.root())
        );

        // V19: an absolute path rooted outside the enumerated list.
        let repo = Repo::clean("v19");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const CONFIG: &str = \"/app/config.toml\";\npub const S: &str = \
             \"/./home/agent/state\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );
    }

    /// Critic-4 C1, all three evasion shapes, through the real entry point:
    /// an external crate compiling into `gauntlet-domain` must fail the gate.
    #[test]
    fn c1_external_dependencies_compiling_into_the_domain_fail_the_gate() {
        // (a) A registry version requirement — the two-line edit that used to
        // pass every locator.
        let repo = Repo::clean("c1a");
        repo.append(
            "crates/gauntlet-domain/Cargo.toml",
            "\n[dependencies]\nserde = \"1\"\n",
        );
        let violations = check_dependency_direction(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-domain/Cargo.toml")
                    && v.contains("'serde'")
                    && v.contains("AK-001")),
            "registry dep in the domain not flagged: {}",
            summarize("dependency-direction", &violations)
        );

        // (b) A git source, scp-like spelling (no `://` for the URL class).
        let repo = Repo::clean("c1b");
        repo.append(
            "crates/gauntlet-domain/Cargo.toml",
            "\n[dependencies]\ninfra = { git = \"git@github.com:attacker/gauntlet-infra\" }\n",
        );
        let violations = check_dependency_direction(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-domain/Cargo.toml")
                    && v.contains("'infra'")
                    && v.contains("git")),
            "git dep in the domain not flagged: {}",
            summarize("dependency-direction", &violations)
        );

        // (c) Workspace inheritance: the member manifest carries only an
        // alias; the root table carries the external crate. The fixture root
        // manifest already opens [workspace.dependencies], so the entries are
        // appended into it.
        let repo = Repo::clean("c1c");
        repo.append("Cargo.toml", "\nserde = \"1\"\n");
        repo.append(
            "crates/gauntlet-domain/Cargo.toml",
            "\n[dependencies]\nserde.workspace = true\n",
        );
        let violations = check_dependency_direction(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("[workspace.dependencies] 'serde'")),
            "unallowlisted root registry entry not flagged: {}",
            summarize("dependency-direction", &violations)
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-domain/Cargo.toml")
                    && v.contains("workspace inheritance")),
            "inherited external dep in the domain not flagged: {}",
            summarize("dependency-direction", &violations)
        );

        // And a git entry in the root table is a violation before anyone
        // even inherits it.
        let repo = Repo::clean("c1d");
        repo.append(
            "Cargo.toml",
            "\ninfra = { git = \"git@github.com:attacker/x\" }\n",
        );
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("[workspace.dependencies] 'infra'") && v.contains("git")),
            "root git entry not flagged"
        );
    }

    /// Critic-4 M5: the substitution channels that reach even the
    /// instrument's own `toml` dependency.
    #[test]
    fn m5_patch_git_and_cargo_config_fail_the_gate() {
        let repo = Repo::clean("m5a");
        repo.append(
            "Cargo.toml",
            "\n[patch.crates-io]\ntoml = { git = \"https://github.com/attacker/toml-fork\" }\n",
        );
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("[patch.crates-io] 'toml'") && v.contains("git")),
            "git patch not flagged"
        );

        let repo = Repo::clean("m5b");
        repo.write(
            ".cargo/config.toml",
            "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[source.vendored-sources]\ndirectory = \"vendor\"\n\n[build]\nrustflags = [\"--cfg\", \"injected\"]\n",
        );
        let violations = check_dependency_direction(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("[source.crates-io]")),
            "source replacement not flagged: {}",
            summarize("dependency-direction", &violations)
        );
        assert!(
            violations.iter().any(|v| v.contains("rustflags")),
            "rustflags not flagged: {}",
            summarize("dependency-direction", &violations)
        );
    }

    /// Critic-4 vacuity M5: the two `deps::` clauses whose wiring inside
    /// `check_dependency_direction` had only direct unit tests — deleting the
    /// calls used to leave the suite green.
    #[test]
    fn v5_undeclared_packages_and_root_paths_fail_through_the_entry_point() {
        // An undeclared package directory under crates/**.
        let repo = Repo::clean("v5a");
        repo.write(
            "crates/gauntlet-infra/Cargo.toml",
            "[package]\nname = \"gauntlet-infra\"\nedition = \"2024\"\n",
        );
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("crates/gauntlet-infra") && v.contains("[workspace].members")),
            "undeclared package directory not reported through the entry point"
        );

        // A root [workspace.dependencies] entry pointing outside the member
        // set. The fixture root manifest already opens that table.
        let repo = Repo::clean("v5b");
        repo.append("Cargo.toml", "\ninfra = { path = \"vendor/infra\" }\n");
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("[workspace.dependencies] 'infra'")
                    && v.contains("vendor/infra")),
            "root path entry not reported through the entry point"
        );
    }

    /// Critic-4 vacuity M2: the defaults exemption needs a
    /// declaration-shaped warrant, and a sibling of the defaults module stays
    /// fully policed.
    #[test]
    fn v2_the_defaults_warrant_requires_declarations_not_mentions() {
        // The critic's fixture: names the six settings in prose, hardcodes
        // three literal classes, passes the old mentions-test.
        let repo = Repo::clean("v2a");
        repo.write(
            "crates/gauntlet-app/src/config/defaults.rs",
            "//! max_rounds max_parallel max_depth max_wall_seconds recovery_reserve_pct \
             diagnostic_budget\npub const API_BIND: &str = \"localhost:8080\";\npub const STALL_MS: \
             u64 = 900_000;\n",
        );
        let violations = check_defaults_exemption(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("does not DECLARE 'max_rounds'")),
            "mention-shaped warrant still accepted: {}",
            summarize("vacuous-exemption", &violations)
        );

        // A genuine §8.6-shaped table warrants the exemption, and a sibling
        // module is still policed by the literal scan.
        let repo = Repo::clean("v2b");
        repo.write(
            "crates/gauntlet-app/src/config/defaults.rs",
            "pub const MAX_ROUNDS: usize = 12;\npub const MAX_PARALLEL: usize = 4;\npub const \
             MAX_DEPTH: usize = 3;\npub const MAX_WALL_SECONDS: u64 = 3600;\npub const \
             RECOVERY_RESERVE_PCT: u8 = 20;\npub const DIAGNOSTIC_BUDGET: u64 = 1000;\n",
        );
        assert!(
            check_defaults_exemption(repo.root()).is_empty(),
            "{}",
            summarize("vacuous-exemption", &check_defaults_exemption(repo.root()))
        );
        assert!(
            check_no_hardcoded_literals(repo.root()).is_empty(),
            "the warranted defaults module is the one exempt file: {}",
            summarize(
                "hardcoded-literal",
                &check_no_hardcoded_literals(repo.root())
            )
        );
        repo.write(
            "crates/gauntlet-app/src/config/tables.rs",
            "pub const ROOT: &str = \"/tmp/gauntlet/state\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path")
                    && v.contains("config/tables.rs")),
            "a sibling of the defaults module must stay policed: {:?}",
            check_no_hardcoded_literals(repo.root())
        );
    }

    /// Critic-5 C1, through the real entry point: rustflags (and the rest of
    /// the compiler-input channel) in the spellings the old two-key probe
    /// never looked at. The fixture writes the config into a TEMPORARY repo —
    /// the channel is tested, not committed.
    #[test]
    fn c1_target_scoped_cargo_config_channels_fail_the_gate() {
        for (label, config, needle) in [
            (
                "cfg-all rustflags",
                "[target.'cfg(all())']\nrustflags = [\"--extern\", \
                 \"shadow=vendor/libshadow.rlib\"]\n",
                "rustflags",
            ),
            (
                "triple rustflags",
                "[target.x86_64-unknown-linux-gnu]\nrustflags = [\"--cfg\", \"injected\"]\n",
                "rustflags",
            ),
            (
                "target linker",
                "[target.'cfg(all())']\nlinker = \"/opt/evil/ld\"\n",
                "linker",
            ),
            (
                "alternate registry index",
                "[registries.mirror]\nindex = \"sparse+https://attacker.example/x\"\n",
                "[registries.mirror]",
            ),
        ] {
            let repo = Repo::clean("c1cfg");
            repo.write(".cargo/config.toml", config);
            let violations = check_dependency_direction(repo.root());
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{label}: {}",
                summarize("dependency-direction", &violations)
            );
        }
    }

    /// Critic-5 C2, through the real entry point: the allowlist admits a
    /// (crate, source) pair. The instrument's own parser is the target.
    #[test]
    fn c2_an_allowlisted_name_from_a_hostile_source_fails_the_gate() {
        for (label, spec) in [
            (
                "git fork",
                "toml = { git = \"https://github.com/attacker/toml-fork\", branch = \"main\" }",
            ),
            (
                "alternate registry",
                "toml = { version = \"0.9\", registry = \"mirror\" }",
            ),
        ] {
            let repo = Repo::clean("c2");
            repo.append(
                "crates/gauntlet-arch-tests/Cargo.toml",
                &format!("\n[dependencies]\n{spec}\n"),
            );
            let violations = check_dependency_direction(repo.root());
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains("'toml'")
                        && v.contains("crates/gauntlet-arch-tests/Cargo.toml")),
                "{label}: {}",
                summarize("dependency-direction", &violations)
            );
        }

        // And through the substitution table, which reaches the same parser
        // without any member manifest changing.
        let repo = Repo::clean("c2patch");
        repo.append(
            "Cargo.toml",
            "\n[patch.crates-io]\ntoml = { version = \"0.9\", registry = \"mirror\" }\n",
        );
        assert!(
            check_dependency_direction(repo.root())
                .iter()
                .any(|v| v.contains("[patch.crates-io] 'toml'")
                    && v.contains("alternate-registry source"))
        );
    }

    /// Critic-5 C3, through the real entry point: a `]` that is data inside an
    /// attribute used to truncate the body before the `path` key.
    #[test]
    fn c3_a_bracket_in_a_literal_no_longer_hides_a_path_escape() {
        let repo = Repo::clean("c3");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "#[cfg_attr(not(doc = \"]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
        );
        let violations = check_scope_integrity(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("vendor/evil.rs")),
            "{}",
            summarize("scope-integrity", &violations)
        );

        // Unscannable input is a violation in its own right, never a skip.
        let repo = Repo::clean("c3b");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "pub const S: &str = \"unterminated\nmod evil;\n",
        );
        assert!(
            check_scope_integrity(repo.root())
                .iter()
                .any(|v| v.contains("never closed")),
            "{:?}",
            check_scope_integrity(repo.root())
        );
    }

    /// Critic-5 M1/M2 through the real locator entry point: the split beyond
    /// any fixed window, the scheme-less endpoint, and the extension-less
    /// relative path.
    #[test]
    fn critic5_literal_evasions_fail_the_gate() {
        // M1: 40 filler lines — past the old 24-line join window.
        let repo = Repo::clean("c5m1");
        let filler = "    //\n".repeat(40);
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            &format!(
                "pub const STALL_TIMEOUT_MS: u64 =\n{filler}    900_000;\npub static REVIEW_MODE: \
                 &str =\n{filler}    \"adversarial\";\n"
            ),
        );
        let violations = check_no_hardcoded_literals(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // M2: a provider endpoint with neither scheme nor port.
        let repo = Repo::clean("c5m2");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const PROVIDER_BASE: &str = \"api.deepinfra.com/v1\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-provider-endpoint")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );

        // M2 (path half): a relative path with no extension.
        let repo = Repo::clean("c5m2b");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "pub const STATE: &str = \"run/state\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-relative-path")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );
    }

    /// Critic-5 vacuity M-1: the exemption list a check hands to the shared
    /// scan is an argument, and an argument nothing checked. Only declared
    /// exemptions, with recorded reasons, are honoured now.
    #[test]
    fn an_undeclared_call_site_exemption_fails_the_gate() {
        let repo = Repo::clean("exempt");
        repo.write(
            "crates/gauntlet-app/src/wiring.rs",
            "pub const ROOT: &str = \"/tmp/gauntlet/state\";\n",
        );

        // Ground truth: the file is policed by the real check.
        assert!(!check_no_hardcoded_literals(repo.root()).is_empty());

        // The critic's edit: exempt it at the call site. The scan itself now
        // says nothing about the file — and the guard must report the
        // undeclared exemption.
        let report = scan_core_tree(
            repo.root(),
            "hardcoded-literal",
            walk::in_literal_scope,
            walk::CORE_CRATES,
            &[walk::DEFAULTS_MODULE, "crates/gauntlet-app/src/wiring.rs"],
            |_, _, _| Vec::new(),
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.contains("crates/gauntlet-app/src/wiring.rs")
                    && v.contains("not one of the declared exemptions")),
            "{}",
            summarize("hardcoded-literal", &report.violations)
        );

        // Negative: the one declared exemption is still honoured, so the guard
        // is a warrant check and not a ban.
        let clean = Repo::clean("exemptok");
        let report = scan_core_tree(
            clean.root(),
            "hardcoded-literal",
            walk::in_literal_scope,
            walk::CORE_CRATES,
            &[walk::DEFAULTS_MODULE],
            |_, _, _| Vec::new(),
        );
        assert!(
            report
                .violations
                .iter()
                .all(|v| !v.contains("declared exemptions")),
            "{}",
            summarize("hardcoded-literal", &report.violations)
        );
    }

    /// Critic-5 vacuity M-2: AK-019's root-based entry point had no
    /// adversarial fixture through it, so the wiring layer where the other
    /// checks' criticals lived was untested for this one.
    #[test]
    fn env_example_gaps_fail_through_the_root_entry_point() {
        let spec = "# Annex E — Environment variable reference\n\n| var | default |\n|---|---|\n| \
                    `GAUNTLET_MODEL` | — |\n| `GAUNTLET_MAX_ROUNDS` | 12 |\n";
        let repo = Repo::clean("env");
        repo.write(".agents/specs/Core-spec.md", spec);

        // A variable the spec names and the file has lost.
        repo.write(".env.example", "GAUNTLET_MODEL=\n");
        let violations = check_env_example(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("GAUNTLET_MAX_ROUNDS")),
            "{}",
            summarize("env-example", &violations)
        );

        // A value that is present — `.env.example` carries names, never values.
        repo.write(
            ".env.example",
            "GAUNTLET_MODEL=some-model-id\nGAUNTLET_MAX_ROUNDS=\n",
        );
        assert!(
            !check_env_example(repo.root()).is_empty(),
            "a non-empty value must fail through the root entry point"
        );

        // A missing file is a violation, not a skip (I8).
        let repo = Repo::clean("envmissing");
        repo.write(".agents/specs/Core-spec.md", spec);
        assert!(
            check_env_example(repo.root())
                .iter()
                .any(|v| v.contains(".env.example"))
        );

        // Negative: the complete pair passes, so the fixture is a real target.
        let repo = Repo::clean("envok");
        repo.write(".agents/specs/Core-spec.md", spec);
        repo.write(".env.example", "GAUNTLET_MODEL=\nGAUNTLET_MAX_ROUNDS=\n");
        assert!(
            check_env_example(repo.root()).is_empty(),
            "{}",
            summarize("env-example", &check_env_example(repo.root()))
        );
    }

    /// Critic-4 M1–M4 through the real locator entry point, not just the
    /// scanner unit tests.
    #[test]
    fn critic4_literal_evasions_fail_the_gate() {
        // M1: the split before the `=`.
        let repo = Repo::clean("m1");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const STALL_TIMEOUT_MS: u64\n    = 900_000;\npub static REVIEW_MODE: &str\n    \
             = \"adversarial\";\n",
        );
        let violations = check_no_hardcoded_literals(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // M2: single-token command head, laundered through a const.
        let repo = Repo::clean("m2");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const SHELL: &str = \"bash\";\npub fn shell() -> Command { \
             std::process::Command::new(SHELL) }\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-tool-identity") && v.contains("'bash'")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );

        // M3: a provider identity with no matcher.
        let repo = Repo::clean("m3");
        repo.write(
            "crates/gauntlet-app/src/lib.rs",
            "pub const PROVIDER: &str = \"deepinfra\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-provider")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );

        // M4: relative path literals.
        let repo = Repo::clean("m4");
        repo.write(
            "crates/gauntlet-domain/src/lib.rs",
            "pub const STATE_DB: &str = \"state/gauntlet.db\";\npub const DATA: &str = \
             \"./data\";\n",
        );
        assert!(
            check_no_hardcoded_literals(repo.root())
                .iter()
                .any(|v| v.starts_with("hardcoded-relative-path")),
            "{:?}",
            check_no_hardcoded_literals(repo.root())
        );
    }
}

/// The evidence-locator declarations in this crate's manifest are what give
/// the answer-key locators (`tests/contract/…`) compiled existence: the repo
/// root is a virtual manifest and `tests/contract/` sits outside the package
/// directory, so those files compile ONLY through the explicit `[[test]]`
/// blocks. Nothing read that manifest, so deleting one block silently rotted
/// a locator with the suite green (critic-4 vacuity M7). This module pins the
/// contract the answer key depends on.
#[cfg(test)]
mod evidence_locators {
    use std::path::Path;

    /// Exactly the four named targets, at exactly the expected paths, each
    /// with a real test file behind it.
    #[test]
    fn the_manifest_declares_exactly_the_four_answer_key_locators() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest_path = manifest_dir.join("Cargo.toml");
        let text =
            std::fs::read_to_string(&manifest_path).expect("this crate's manifest is readable");
        let table = text
            .parse::<toml::Table>()
            .expect("this crate's manifest parses");
        let tests = table
            .get("test")
            .and_then(|value| value.as_array())
            .expect("the [[test]] blocks exist");
        let mut seen: Vec<(String, String)> = tests
            .iter()
            .filter_map(|entry| {
                let name = entry.get("name")?.as_str()?.to_string();
                let path = entry.get("path")?.as_str()?.to_string();
                Some((name, path))
            })
            .collect();
        seen.sort();
        let mut expected = vec![
            (
                "adapter_registry".to_string(),
                "../../tests/contract/adapter_registry.rs".to_string(),
            ),
            (
                "architecture_boundaries".to_string(),
                "../../tests/contract/architecture_boundaries.rs".to_string(),
            ),
            (
                "env_example_complete".to_string(),
                "../../tests/contract/env_example_complete.rs".to_string(),
            ),
            (
                "no_hardcoded_literals".to_string(),
                "../../tests/contract/no_hardcoded_literals.rs".to_string(),
            ),
        ];
        expected.sort();
        assert_eq!(
            seen, expected,
            "the evidence locators the frozen answer key addresses are declared as cargo test \
             targets; deleting or renaming a block rots an AK locator with the suite green"
        );
        for (name, rel) in &expected {
            let path = manifest_dir.join(rel);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("locator {name} at {} is readable: {err}", path.display())
            });
            assert!(
                text.contains("#[test]"),
                "locator {name} carries at least one test"
            );
            assert!(
                text.contains("check_"),
                "locator {name} drives a check_* entry point"
            );
        }
    }
}

#[cfg(test)]
mod role_tests {
    use super::*;

    /// The scope predicates are unit-tested in `walk`; the code that CONSUMES
    /// them was not, so inverting the two branches of the role assignment left
    /// the whole suite green (round-3, critic 2's "untested wiring"). This
    /// pins the mapping itself.
    #[test]
    fn every_scanned_file_gets_the_role_its_layer_requires() {
        let root = repo_root();
        let seen = std::cell::RefCell::new(Vec::<(String, denylist::Role)>::new());
        let report = scan_core_tree(
            &root,
            "role-probe",
            walk::in_denylist_scope,
            walk::CORE_CRATES,
            &[],
            |file, _text, role| {
                seen.borrow_mut().push((file.to_string(), role));
                Vec::new()
            },
        );
        assert_eq!(report.skipped, 0);
        let seen = seen.into_inner();
        assert!(!seen.is_empty(), "the walk produced no files to role");

        let mut inner = 0usize;
        let mut composition = 0usize;
        for (file, role) in &seen {
            let expected = if file.starts_with("crates/gauntlet-cli/") {
                composition += 1;
                denylist::Role::CompositionRoot
            } else {
                inner += 1;
                denylist::Role::Inner
            };
            assert_eq!(
                *role, expected,
                "{file} was scanned as {role:?} — the composition root gets \
                 Role::CompositionRoot and the domain/ports/application layers get Role::Inner; \
                 inverting them silently disables the adapter-identifier rule for the inner \
                 layers"
            );
        }
        assert!(inner > 0 && composition > 0, "{seen:?}");
    }

    /// Role::Inner and Role::CompositionRoot must not be interchangeable: the
    /// inner role carries the adapter identifiers and the composition root
    /// does not. If this ever passes with the roles swapped, the distinction
    /// has been lost.
    #[test]
    fn the_two_roles_enforce_different_term_sets() {
        let deny = denylist::Denylist::derive(&members_of(&repo_root()));
        let wiring = "use gauntlet_adapter_agent_mock::MockRunner;\n";
        assert!(
            !denylist::scan_source("f.rs", wiring, &deny, denylist::Role::Inner).is_empty(),
            "an adapter identifier must be a violation in the inner layers"
        );
        let bare_vendor = "let c = rusqlite::Connection::open(p)?;\n";
        assert!(
            !denylist::scan_source("f.rs", bare_vendor, &deny, denylist::Role::CompositionRoot)
                .is_empty(),
            "a bare vendor identifier must be a violation in the composition root"
        );
    }
}
