//! (a) Dependency direction — spec §4.1 ("dependencies point inward") made a
//! mechanical gate per spec §24.3. Pure manifest checks live here; the tree
//! walk that feeds them is `crate::walk`.
//!
//! Layer rules, each an upper bound on *internal* (workspace-member)
//! dependencies. Dev- and build-dependencies count: a test-only leak is
//! still a leak.
//!
//! | crate               | allowed internal deps                |
//! |---------------------|--------------------------------------|
//! | gauntlet-domain     | none                                 |
//! | gauntlet-ports      | gauntlet-domain                      |
//! | gauntlet-app        | gauntlet-domain, gauntlet-ports      |
//! | gauntlet-cli        | domain, ports, app, every adapter    |
//! | crates/adapters/*   | domain, ports — never app or cli     |
//! | gauntlet-arch-tests | none (test-only)                     |
//!
//! The cli row is open-ended ("cli wires everything", spec §4.1, plan Task 2
//! step 2): every member under `crates/adapters/` is allowed, today's and
//! tomorrow's, without editing this rule — the composition root is the one
//! crate that may construct adapters.
//!
//! A member that is none of these is itself a violation: the Task 2 crate
//! list is an orchestrator decision, so a new crate must be classified here
//! deliberately rather than slip in unclassified.
//!
//! # Membership is discovered, not declared (recritique R1/R14, class A1)
//!
//! Cargo auto-includes any path dependency residing in the workspace
//! directory as a member, so `[workspace].members` is a *claim*, not the
//! truth. Two checks close that:
//!
//! - [`undeclared_packages`] walks `crates/**` for every directory holding a
//!   `Cargo.toml`; one that is not a declared member is a violation, because
//!   it can be compiled into a core crate while sitting outside every scan
//!   scope;
//! - every `path = "…"` dependency — in a member manifest or in the root
//!   `[workspace.dependencies]` table — must resolve to a known member. A
//!   `path` pointing at `../../vendor/infra` is the undeclared-infrastructure
//!   edge AK-001 forbids, and it used to be discarded as "external".

use std::collections::BTreeMap;
use std::path::Path;

use toml::Table;

/// A workspace member: its package directory (repo-relative, POSIX
/// separators) and its `[package].name`.
#[derive(Clone, Debug)]
pub struct Member {
    pub dir: String,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Domain,
    Ports,
    App,
    Cli,
    Adapter,
    BoundaryTests,
}

impl Layer {
    fn name(self) -> &'static str {
        match self {
            Layer::Domain => "domain",
            Layer::Ports => "ports",
            Layer::App => "application",
            Layer::Cli => "cli",
            Layer::Adapter => "adapter",
            Layer::BoundaryTests => "boundary-tests",
        }
    }

    /// The layer's allowed internal dependencies. The cli list is extended
    /// with every adapter crate found among the workspace members — the
    /// composition root may wire any adapter, so adapters join its allowed
    /// set by being members, not by being enumerated here.
    fn allowed_internal(self, members: &[Member]) -> Vec<&str> {
        let mut allowed: Vec<&str> = match self {
            Layer::Domain | Layer::BoundaryTests => vec![],
            Layer::Ports => vec!["gauntlet-domain"],
            Layer::App => vec!["gauntlet-domain", "gauntlet-ports"],
            Layer::Cli => vec!["gauntlet-domain", "gauntlet-ports", "gauntlet-app"],
            Layer::Adapter => vec!["gauntlet-domain", "gauntlet-ports"],
        };
        if self == Layer::Cli {
            allowed.extend(
                members
                    .iter()
                    .filter(|m| m.dir.starts_with("crates/adapters/"))
                    .map(|m| m.name.as_str()),
            );
        }
        allowed
    }
}

fn classify(member: &Member) -> Result<Layer, String> {
    if member.dir.starts_with("crates/adapters/") {
        return Ok(Layer::Adapter);
    }
    match member.name.as_str() {
        "gauntlet-domain" => Ok(Layer::Domain),
        "gauntlet-ports" => Ok(Layer::Ports),
        "gauntlet-app" => Ok(Layer::App),
        "gauntlet-cli" => Ok(Layer::Cli),
        "gauntlet-arch-tests" => Ok(Layer::BoundaryTests),
        other => Err(other.to_string()),
    }
}

/// Dependency sections of a manifest, including target-scoped ones, as
/// `(label, table)` pairs. Target sections are walked so a dependency cannot
/// hide behind a `cfg` — the constraint is architectural, not per-target.
pub fn dependency_sections(manifest: &Table) -> Vec<(String, &Table)> {
    const SECTIONS: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
    let mut out = Vec::new();
    for section in SECTIONS {
        if let Some(table) = manifest.get(*section).and_then(|v| v.as_table()) {
            out.push((section.to_string(), table));
        }
    }
    if let Some(targets) = manifest.get("target").and_then(|v| v.as_table()) {
        for (cfg, spec) in targets {
            let Some(cfg_table) = spec.as_table() else {
                continue;
            };
            for section in SECTIONS {
                if let Some(table) = cfg_table.get(*section).and_then(|v| v.as_table()) {
                    out.push((format!("target.{cfg}.{section}"), table));
                }
            }
        }
    }
    out
}

/// Alias → resolved crate name for the root manifest's
/// `[workspace.dependencies]`.
pub fn workspace_dep_names(root_manifest: &Table) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(deps) = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    else {
        return out;
    };
    for (alias, spec) in deps {
        let resolved = spec
            .as_table()
            .and_then(|fields| fields.get("package"))
            .and_then(|value| value.as_str())
            .unwrap_or(alias);
        out.insert(alias.clone(), resolved.to_string());
    }
    out
}

/// Every `path = "…"` entry in the root `[workspace.dependencies]` table must
/// resolve to a declared member (class A1): the root table is the one place a
/// single line can point the whole workspace at an out-of-tree crate.
pub fn check_workspace_dependency_paths(root_manifest: &Table, members: &[Member]) -> Vec<String> {
    let Some(deps) = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (alias, spec) in deps {
        let Some(rel) = spec
            .as_table()
            .and_then(|fields| fields.get("path"))
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        let target = join_posix("", rel);
        if !members.iter().any(|m| m.dir == target) {
            out.push(format!(
                "dependency-direction: Cargo.toml: [workspace.dependencies] '{alias}' has \
                 path = '{rel}' (resolved: '{target}'), which is not a declared workspace member \
                 — an out-of-tree crate reachable from every member is exactly the \
                 infrastructure edge AK-001 forbids"
            ));
        }
    }
    out
}

/// Internal (workspace-member) dependencies declared in `text`, plus the
/// `path` dependencies that resolve to nothing known.
struct DeclaredDeps {
    internal: Vec<(String, String)>,
    unresolved_paths: Vec<(String, String, String, String)>,
}

fn declared_deps(
    member_dir: &str,
    text: &str,
    members: &[Member],
    workspace_deps: &BTreeMap<String, String>,
) -> Result<DeclaredDeps, String> {
    let manifest = parse_table(text).map_err(|e| format!("invalid TOML: {e}"))?;
    let mut internal = Vec::new();
    let mut unresolved_paths = Vec::new();
    for (section, table) in &dependency_sections(&manifest) {
        for (key, spec) in *table {
            let mut dep = key.clone();
            if let Some(fields) = spec.as_table() {
                if let Some(renamed) = fields.get("package").and_then(|v| v.as_str()) {
                    dep = renamed.to_string();
                }
                if let Some(rel) = fields.get("path").and_then(|v| v.as_str()) {
                    let target = join_posix(member_dir, rel);
                    match members.iter().find(|m| m.dir == target) {
                        Some(hit) => dep = hit.name.clone(),
                        None => unresolved_paths.push((
                            section.clone(),
                            key.clone(),
                            rel.to_string(),
                            target,
                        )),
                    }
                }
                // Workspace inheritance wins last: with `workspace = true`
                // the root table is authoritative for what the crate is.
                if fields.get("workspace").and_then(|v| v.as_bool()) == Some(true)
                    && let Some(resolved) = workspace_deps.get(key)
                {
                    dep = resolved.clone();
                }
            }
            if members.iter().any(|m| m.name == dep) {
                internal.push((section.clone(), dep));
            }
        }
    }
    Ok(DeclaredDeps {
        internal,
        unresolved_paths,
    })
}

/// Check one member manifest against its layer rule.
pub fn check_member(
    member: &Member,
    text: &str,
    members: &[Member],
    workspace_deps: &BTreeMap<String, String>,
) -> Vec<String> {
    let manifest = format!("{}/Cargo.toml", member.dir);
    let layer = match classify(member) {
        Ok(layer) => layer,
        Err(name) => {
            return vec![format!(
                "dependency-direction: {manifest}: workspace member '{name}' is not one of the \
                 Task 2 crate classes (domain, ports, application, cli, adapter, boundary-tests); \
                 classify it in crates/gauntlet-arch-tests/src/deps.rs or remove it"
            )];
        }
    };
    let mut violations = Vec::new();
    match declared_deps(&member.dir, text, members, workspace_deps) {
        Err(err) => violations.push(format!("dependency-direction: {manifest}: {err}")),
        Ok(declared) => {
            for (section, key, rel, target) in declared.unresolved_paths {
                violations.push(format!(
                    "dependency-direction: {manifest}: crate '{}' ({}) declares {} on '{key}' by \
                     path = '{rel}' (resolved: '{target}'), which is not a declared workspace \
                     member — a path dependency outside the member set is undeclared \
                     infrastructure (AK-001), and it is invisible to every source scanner",
                    member.name,
                    layer.name(),
                    section
                ));
            }
            let allowed = layer.allowed_internal(members);
            for (section, dep) in declared.internal {
                if !allowed.contains(&dep.as_str()) {
                    let extra = if layer == Layer::Adapter
                        && (dep == "gauntlet-app" || dep == "gauntlet-cli")
                    {
                        " — adapters must never depend on the application or composition layers"
                    } else {
                        ""
                    };
                    violations.push(format!(
                        "dependency-direction: {manifest}: crate '{}' ({}) declares {} on '{}', \
                         which is not among its allowed internal dependencies {:?}{extra}",
                        member.name,
                        layer.name(),
                        section,
                        dep,
                        allowed
                    ));
                }
            }
        }
    }
    violations
}

/// Read `[workspace].members` from the root manifest and each member's
/// `[package].name`. Any missing or malformed manifest is an error — silence
/// is never agreement.
pub fn read_members(root: &Path, root_manifest: &str) -> Result<Vec<Member>, String> {
    let table = parse_table(root_manifest).map_err(|e| format!("workspace Cargo.toml: {e}"))?;
    let list = table
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .ok_or_else(|| {
            "workspace Cargo.toml: [workspace].members missing or not an array".to_string()
        })?;
    let mut out = Vec::new();
    for entry in list {
        let dir = entry
            .as_str()
            .ok_or_else(|| "workspace Cargo.toml: members must be path strings".to_string())?
            .replace('\\', "/");
        let manifest_path = format!("{dir}/Cargo.toml");
        let text = std::fs::read_to_string(root.join(&manifest_path))
            .map_err(|e| format!("{manifest_path}: {e}"))?;
        let member_table = parse_table(&text).map_err(|e| format!("{manifest_path}: {e}"))?;
        let name = member_table
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("{manifest_path}: [package].name missing"))?
            .to_string();
        out.push(Member { dir, name });
    }
    Ok(out)
}

/// Every directory under `crates/**` that holds a `Cargo.toml`, repo-relative
/// and sorted. This is the filesystem's answer to "what packages exist",
/// against which `[workspace].members` is checked.
pub fn discover_package_dirs(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_packages(root, &root.join("crates"), &mut out);
    out.sort();
    out
}

fn collect_packages(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    if dir.join("Cargo.toml").is_file() {
        out.push(
            dir.strip_prefix(root)
                .unwrap_or(dir)
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // `is_dir()` on the entry's own type: a symlinked directory is not
        // descended into here either — `crate::walk` reports those.
        if file_type.is_dir() && entry.file_name() != "target" {
            collect_packages(root, &entry.path(), out);
        }
    }
}

/// Package directories that exist on disk but are not declared members
/// (class A1). Cargo would happily compile such a package into a core crate
/// through a `path` dependency; nothing else in the gate would see it.
pub fn undeclared_packages(package_dirs: &[String], members: &[Member]) -> Vec<String> {
    package_dirs
        .iter()
        .filter(|dir| !members.iter().any(|m| m.dir == **dir))
        .map(|dir| {
            format!(
                "dependency-direction: {dir}/Cargo.toml: package directory '{dir}' exists but is \
                 not declared in [workspace].members — Cargo auto-includes in-tree path \
                 dependencies as members, so an undeclared package can compile into a core crate \
                 while sitting outside every scan scope (AK-001, spec §24.3)"
            )
        })
        .collect()
}

fn parse_table(text: &str) -> Result<Table, String> {
    text.parse::<Table>().map_err(|e| format!("{e}"))
}

/// Join a member-relative manifest `path` dependency onto `base` and resolve
/// `.`/`..` segments, so `path = "../gauntlet-ports"` maps to a member
/// directory.
fn join_posix(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    let normalized = rel.replace('\\', "/");
    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod repo {

    #[test]
    fn dependency_direction_holds() {
        let violations = crate::check_dependency_direction(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("dependency-direction", &violations)
        );
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    fn member(dir: &str, name: &str) -> Member {
        Member {
            dir: dir.to_string(),
            name: name.to_string(),
        }
    }

    fn fixture_members() -> Vec<Member> {
        vec![
            member("crates/gauntlet-domain", "gauntlet-domain"),
            member("crates/gauntlet-ports", "gauntlet-ports"),
            member("crates/gauntlet-app", "gauntlet-app"),
            member("crates/gauntlet-cli", "gauntlet-cli"),
            member(
                "crates/adapters/gauntlet-adapter-agent-mock",
                "gauntlet-adapter-agent-mock",
            ),
            member("crates/gauntlet-arch-tests", "gauntlet-arch-tests"),
        ]
    }

    fn empty_workspace_deps() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn workspace_deps_from(text: &str) -> BTreeMap<String, String> {
        let table = parse_table(text).expect("fixture root manifest parses");
        workspace_dep_names(&table)
    }

    #[test]
    fn domain_with_any_internal_dep_is_flagged() {
        let text = r#"
[package]
name = "gauntlet-domain"

[dependencies]
gauntlet-ports.workspace = true
"#;
        let violations = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            text,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-domain"));
        assert!(violations[0].contains("gauntlet-ports"));
        assert!(violations[0].contains("crates/gauntlet-domain/Cargo.toml"));
    }

    #[test]
    fn ports_and_app_layering_is_accepted() {
        let ports = r#"
[dependencies]
gauntlet-domain.workspace = true
"#;
        let app = r#"
[dependencies]
gauntlet-domain.workspace = true
gauntlet-ports.workspace = true

[dev-dependencies]
gauntlet-domain.workspace = true
"#;
        let members = fixture_members();
        assert!(
            check_member(
                &member("crates/gauntlet-ports", "gauntlet-ports"),
                ports,
                &members,
                &empty_workspace_deps()
            )
            .is_empty()
        );
        assert!(
            check_member(
                &member("crates/gauntlet-app", "gauntlet-app"),
                app,
                &members,
                &empty_workspace_deps()
            )
            .is_empty()
        );
    }

    #[test]
    fn adapter_depending_on_app_or_cli_is_flagged() {
        let text = r#"
[dev-dependencies]
gauntlet-app.workspace = true
"#;
        let violations = check_member(
            &member(
                "crates/adapters/gauntlet-adapter-agent-mock",
                "gauntlet-adapter-agent-mock",
            ),
            text,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-adapter-agent-mock"));
        assert!(violations[0].contains("gauntlet-app"));
        assert!(violations[0].contains("never depend on the application"));
    }

    #[test]
    fn adapter_depending_on_domain_and_ports_is_accepted() {
        let text = r#"
[dependencies]
gauntlet-domain.workspace = true
gauntlet-ports.workspace = true
"#;
        let violations = check_member(
            &member(
                "crates/adapters/gauntlet-adapter-agent-mock",
                "gauntlet-adapter-agent-mock",
            ),
            text,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn cli_wiring_of_adapters_is_accepted_and_the_reverse_is_flagged() {
        let members = fixture_members();
        let cli = r#"
[dependencies]
gauntlet-adapter-agent-mock.workspace = true
"#;
        assert!(
            check_member(
                &member("crates/gauntlet-cli", "gauntlet-cli"),
                cli,
                &members,
                &empty_workspace_deps()
            )
            .is_empty()
        );

        let adapter = r#"
[dependencies]
gauntlet-cli.workspace = true
"#;
        let violations = check_member(
            &member(
                "crates/adapters/gauntlet-adapter-agent-mock",
                "gauntlet-adapter-agent-mock",
            ),
            adapter,
            &members,
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-cli"));
        assert!(violations[0].contains("never depend on the application"));
    }

    #[test]
    fn arch_tests_with_an_internal_dep_is_flagged() {
        let text = r#"
[dependencies]
gauntlet-domain.workspace = true
"#;
        let violations = check_member(
            &member("crates/gauntlet-arch-tests", "gauntlet-arch-tests"),
            text,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-arch-tests"));
    }

    #[test]
    fn renamed_and_path_dependencies_are_resolved() {
        let renamed = r#"
[dependencies]
core = { package = "gauntlet-app", version = "0.1" }
"#;
        let violations = check_member(
            &member("crates/gauntlet-ports", "gauntlet-ports"),
            renamed,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-app"));

        let by_path = r#"
[target.'cfg(unix)'.dependencies]
sneaky = { path = "../gauntlet-app" }
"#;
        let violations = check_member(
            &member("crates/gauntlet-ports", "gauntlet-ports"),
            by_path,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("gauntlet-app"));
        assert!(violations[0].contains("target.cfg(unix).dependencies"));
    }

    #[test]
    fn workspace_inherited_renames_resolve_to_the_real_crate() {
        let root_manifest = r#"
[workspace.dependencies]
ren = { package = "gauntlet-app" }
"#;
        let text = r#"
[dependencies]
ren.workspace = true
"#;
        let violations = check_member(
            &member("crates/gauntlet-ports", "gauntlet-ports"),
            text,
            &fixture_members(),
            &workspace_deps_from(root_manifest),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("on 'gauntlet-app'"),
            "{}",
            violations[0]
        );
        assert!(
            !violations[0].contains("'ren'"),
            "the alias must not stand in for the resolved crate name: {}",
            violations[0]
        );
    }

    #[test]
    fn unclassified_member_and_malformed_manifest_are_flagged() {
        let unknown = check_member(
            &member("crates/mystery", "gauntlet-mystery"),
            "[package]\nname = \"gauntlet-mystery\"\n",
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(unknown.len(), 1, "{unknown:?}");
        assert!(unknown[0].contains("gauntlet-mystery"));

        let malformed = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            "[dependencies\nbroken",
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(malformed.len(), 1, "{malformed:?}");
        assert!(malformed[0].contains("invalid TOML"));
    }

    #[test]
    fn path_dependency_outside_the_member_set_is_flagged() {
        // Recritique R1's evasion: `infra = { path = "../gauntlet-infra" }`
        // in the domain, with gauntlet-infra absent from [workspace].members.
        let text = r#"
[dependencies]
infra = { path = "../gauntlet-infra" }
"#;
        let violations = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            text,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("undeclared infrastructure"),
            "{}",
            violations[0]
        );
        assert!(
            violations[0].contains("crates/gauntlet-infra"),
            "{violations:?}"
        );

        // Recritique R14's variant: a path outside the workspace directory.
        let vendored = r#"
[dependencies]
store = { path = "../../vendor/rusqlite-store" }
"#;
        let violations = check_member(
            &member("crates/gauntlet-app", "gauntlet-app"),
            vendored,
            &fixture_members(),
            &empty_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("vendor/rusqlite-store"),
            "{violations:?}"
        );
    }

    #[test]
    fn undeclared_package_directory_is_flagged() {
        // Recritique R1: crates/gauntlet-infra exists on disk but is not a
        // declared member, so Cargo auto-includes it while the gate does not.
        let discovered = vec![
            "crates/gauntlet-domain".to_string(),
            "crates/gauntlet-infra".to_string(),
        ];
        let violations = undeclared_packages(&discovered, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("crates/gauntlet-infra"),
            "{violations:?}"
        );
        assert!(
            violations[0].contains("[workspace].members"),
            "{violations:?}"
        );
    }

    #[test]
    fn workspace_dependency_path_outside_the_member_set_is_flagged() {
        let root_manifest = r#"
[workspace.dependencies]
gauntlet-domain = { path = "crates/gauntlet-domain" }
infra = { path = "vendor/infra" }
toml = "0.9"
"#;
        let table = parse_table(root_manifest).expect("fixture parses");
        let violations = check_workspace_dependency_paths(&table, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("vendor/infra"), "{violations:?}");
    }
}
