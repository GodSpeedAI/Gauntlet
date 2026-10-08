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
//! # External dependencies fail closed (critic-4 C1, M5, M8)
//!
//! The layer rules above bound *internal* dependencies only. Until critic 4,
//! every other source channel was invisible: a registry version requirement
//! (`shadow-net = "0.8"`), a git source (any URL spelling), a
//! workspace-inherited external crate, a `[patch]` entry with a git source,
//! and `.cargo/config.toml` source replacement could each compile
//! attacker-chosen code into `gauntlet-domain` with every locator green. The
//! channel is closed at the source, not by enumerating vendor names:
//!
//! - a member's dependency sections (normal, dev, build, target-scoped —
//!   [`dependency_sections`]) admit only (i) workspace members and (ii)
//!   crates on that layer's allowlist ([`DOMAIN_EXTERNAL_ALLOWLIST`],
//!   [`APPLICATION_EXTERNAL_ALLOWLIST`], …). Every allowlist is EMPTY today
//!   except the boundary suite's own `toml`. The allowlist is the reviewable
//!   decision point: adding `serde = "1"` to `gauntlet-domain` fails
//!   `just boundaries` until a maintainer lists it here with a recorded
//!   justification beside the entry;
//! - an allowlist entry admits a (crate, SOURCE) pair, never a bare name
//!   (critic-5 C2). Only the default registry makes a name evidence of the
//!   code behind it, so `toml = { git = "https://…/toml-fork" }` and
//!   `toml = { version = "0.9", registry = "mirror" }` are violations even
//!   though `toml` is allowlisted — either would hand this instrument's own
//!   manifest parser to whoever controls that source;
//! - a root `[workspace.dependencies]` entry must resolve to a member by
//!   `path` or sit on the union of the layer allowlists
//!   ([`external_allowlist_union`]) — the table is inheritable by every
//!   member, `gauntlet-domain` included. A git source there is a violation
//!   outright;
//! - a `[patch]`/`[replace]` entry must resolve to a member by `path`. Any
//!   other source (git, registry) substitutes a crate across the whole
//!   workspace — including for this instrument's own `toml` dependency, the
//!   parser behind every manifest check here;
//! - in-repo cargo configuration is DISCOVERED (every `.cargo/config*` in the
//!   tree, not only the root one — cargo reads them from the invocation
//!   directory upward) and walked generically ([`check_cargo_config`]):
//!   `[source.*]` replacement, `[registries.*]`, every key in
//!   [`FORBIDDEN_CARGO_CONFIG_KEYS`] (`rustflags`, `linker`, `runner`,
//!   wrappers, `paths`), and every compiler-affecting entry of the `[env]`
//!   table ([`FORBIDDEN_ENV_KEYS`], critic-8 E2 — cargo applies `[env]`
//!   values to build scripts, rustc invocations, and cargo build/run) are
//!   violations WHEREVER they appear — under `[build]`,
//!   under `[target.'cfg(all())']`, or under any other table. All of them
//!   redirect what compiles without touching a manifest this suite reads. The
//!   toolchain pin is the same channel in another file ([`check_toolchain_file`]).
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

// ---------------------------------------------------------------------------
// External-dependency allowlists (critic-4 C1) — fail CLOSED.
//
// Spec §24.3's first bullet and AK-001 say the domain "declares zero
// dependencies on port, adapter, or infrastructure crates", and the
// dependency-direction rules above bound only workspace members. These lists
// are the other half: a dependency that is not a member is legal in a layer
// only if it is named here. Every list is EMPTY today except the boundary
// suite's own `toml`. THE EMPTY LIST IS THE DESIGN: the allowlist is the
// reviewable decision point, so adding an entry must be a recorded, justified
// act — a comment naming the spec section or plan task that permits it,
// beside the entry — not a diff buried in a refactor. `just boundaries` fails
// until the entry exists.
// ---------------------------------------------------------------------------

/// One permitted external crate: its package name and the recorded reason it
/// is permitted. The reason is not decoration — `allowlist_entries_state_a_reason`
/// fails the gate if an entry is added without one, so the allowlist cannot
/// grow silently (critic-5 anti-vacuity).
pub type ExternalAllowance = (&'static str, &'static str);

/// External crates `gauntlet-domain` may depend on. EMPTY (spec §24.3,
/// AK-001: zero dependencies on port, adapter, or infrastructure crates).
pub const DOMAIN_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[];

/// External crates `gauntlet-ports` may depend on. The envelope's canonical
/// bytes are consumed by adapters, therefore the §13.2 SHA-256 revalidation
/// lives at the port boundary rather than only in the application builder.
pub const PORTS_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[(
    "sha2",
    "canonical comparison-envelope SHA-256 recomputation at every comparator consumption \
     boundary (Task 10 correction round 2, spec §13.2); std has no SHA-256",
)];

/// External crates `gauntlet-app` may depend on. Plan Task 5 adds the one
/// entry, following the recorded-act procedure above: the §8.1
/// `config_fingerprint` digest needs SHA-256 and std has none; the same
/// crate serves plan Task 7's reference digest.
pub const APPLICATION_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[(
    "sha2",
    "config_fingerprint digest (plan Task 5); reused by reference digest (Task 7) — std has no \
     SHA-256",
)];

/// External crates `gauntlet-cli` may depend on. EMPTY today; the composition
/// root inherits its permissions from the layers it wires, and anything it
/// needs beyond them is a recorded decision made here.
pub const CLI_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[];

/// External crates an adapter under `crates/adapters/**` may depend on.
/// Plan Task 6 adds the first two, following the recorded-act procedure
/// above: the embedded-SQL state store needs the database engine, and the
/// filesystem evidence store needs SHA-256 for content addressing. Slice E1a
/// adds postgres for the canonical durable store.
pub const ADAPTER_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[
    (
        "postgres",
        "the synchronous PostgreSQL client for the state-postgres adapter (slice E1a); \
         canonical durable store replacing SQLite, sync client matches the synchronous \
         single-threaded ports contract; declared in the adapter's own manifest",
    ),
    (
        "rusqlite",
        "the embedded SQL engine behind the state-sqlite adapter's StateStore and EventLog (plan \
         Task 6; spec 18.1: a single-file embedded database is a StateStore adapter choice); \
         declared with the bundled feature in that adapter's own manifest, never in the root \
         table the denylist keeps vendor-free",
    ),
    (
        "sha2",
        "content addressing for the evidence-fs adapter's put(bytes, kind) (plan Task 6, spec \
         6.2 EvidenceStore) and the five observe adapters' sha256:<hex> environment fingerprints \
         plus the filesystem manifest's per-entry content digests (plan Task 9, spec 12.2) — std \
         has no SHA-256; the same crate the application layer already carries for the §8.1 \
         config fingerprint, inherited from the root table",
    ),
];

/// External crates `gauntlet-arch-tests` may depend on.
pub const BOUNDARY_TESTS_EXTERNAL_ALLOWLIST: &[ExternalAllowance] = &[(
    "toml",
    "manifest parsing — the one external dependency plan Task 2 permits, declared once in the \
     root [workspace.dependencies] as toml = \"0.9\"; this crate is test-only and is never a \
     dependency of another member",
)];

/// Every allowlist, in layer order, for the pins and the union.
pub const ALL_EXTERNAL_ALLOWLISTS: &[(&str, &[ExternalAllowance])] = &[
    ("domain", DOMAIN_EXTERNAL_ALLOWLIST),
    ("ports", PORTS_EXTERNAL_ALLOWLIST),
    ("application", APPLICATION_EXTERNAL_ALLOWLIST),
    ("cli", CLI_EXTERNAL_ALLOWLIST),
    ("adapter", ADAPTER_EXTERNAL_ALLOWLIST),
    ("boundary-tests", BOUNDARY_TESTS_EXTERNAL_ALLOWLIST),
];

/// The names on one allowlist, for membership tests and violation text.
pub fn allowlist_names(list: &[ExternalAllowance]) -> Vec<&'static str> {
    list.iter().map(|(name, _)| *name).collect()
}

/// Every crate any layer permits externally. The root
/// `[workspace.dependencies]` table is inheritable by all of them, so an
/// entry there must sit on this union to be legal.
pub fn external_allowlist_union() -> Vec<&'static str> {
    let mut out = Vec::new();
    for (_, list) in ALL_EXTERNAL_ALLOWLISTS {
        for (name, _) in *list {
            if !out.contains(name) {
                out.push(name);
            }
        }
    }
    out
}

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

    /// The layer's external (non-member) dependency allowlist. EMPTY for every
    /// layer except the boundary suite (critic-4 C1): the empty lists are what
    /// make the external channel fail closed instead of failing open.
    fn allowed_external(self) -> &'static [ExternalAllowance] {
        match self {
            Layer::Domain => DOMAIN_EXTERNAL_ALLOWLIST,
            Layer::Ports => PORTS_EXTERNAL_ALLOWLIST,
            Layer::App => APPLICATION_EXTERNAL_ALLOWLIST,
            Layer::Cli => CLI_EXTERNAL_ALLOWLIST,
            Layer::Adapter => ADAPTER_EXTERNAL_ALLOWLIST,
            Layer::BoundaryTests => BOUNDARY_TESTS_EXTERNAL_ALLOWLIST,
        }
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

/// How a root `[workspace.dependencies]` entry gets its code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RootDepSource {
    /// `path = "…"` resolving to this member directory.
    Member,
    /// `path = "…"`, resolved repo-relative, pointing outside the member set.
    Path,
    /// Any `git = "…"` source, any URL spelling (https, scp-like, …).
    Git,
    /// A version requirement served by the DEFAULT registry (crates.io).
    Registry,
    /// `registry = "…"` / `registry-index = "…"`: a named registry whose index
    /// is chosen elsewhere (`.cargo/config.toml` `[registries.*]`, or the
    /// developer's `$CARGO_HOME`). The name says nothing about the code
    /// (critic-5 C2).
    AlternateRegistry,
}

/// One root `[workspace.dependencies]` entry, resolved far enough to decide
/// whether any member may inherit it (critic-4 C1: the root table is the one
/// place a single line can point every member, `gauntlet-domain` included, at
/// out-of-tree code).
#[derive(Clone, Debug)]
pub struct RootDep {
    /// The crate name an inheriting member actually compiles (`package = "…"`
    /// rename included).
    pub package: String,
    /// The entry's declared source.
    pub source: RootDepSource,
}

/// The root `[workspace.dependencies]` table with each entry's source
/// resolved. Member classification needs the member set, so it is passed in.
pub fn workspace_dep_records(
    root_manifest: &Table,
    members: &[Member],
) -> BTreeMap<String, RootDep> {
    let mut out = BTreeMap::new();
    let Some(deps) = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    else {
        return out;
    };
    for (alias, spec) in deps {
        let fields = spec.as_table();
        let package = fields
            .and_then(|f| f.get("package"))
            .and_then(|value| value.as_str())
            .unwrap_or(alias)
            .to_string();
        let source = if let Some(rel) = fields
            .and_then(|f| f.get("path"))
            .and_then(|value| value.as_str())
        {
            let target = join_posix("", rel);
            if members.iter().any(|m| m.dir == target) {
                RootDepSource::Member
            } else {
                RootDepSource::Path
            }
        } else if fields.is_some_and(|f| f.contains_key("git")) {
            RootDepSource::Git
        } else if fields.is_some_and(has_alternate_registry) {
            RootDepSource::AlternateRegistry
        } else {
            RootDepSource::Registry
        };
        out.insert(alias.clone(), RootDep { package, source });
    }
    out
}

/// Every entry in the root `[workspace.dependencies]` table (class A1 +
/// critic-4 C1): a `path` must resolve to a declared member; a git source is
/// a violation outright; anything else (a registry entry) must sit on the
/// union of the layer external allowlists, because the table is one
/// `alias.workspace = true` away from compiling into `gauntlet-domain`.
pub fn check_workspace_dependency_table(root_manifest: &Table, members: &[Member]) -> Vec<String> {
    let Some(deps) = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    else {
        return Vec::new();
    };
    let allowed = external_allowlist_union();
    let mut out = Vec::new();
    for (alias, spec) in deps {
        let fields = spec.as_table();
        if let Some(rel) = fields
            .and_then(|f| f.get("path"))
            .and_then(|value| value.as_str())
        {
            let target = join_posix("", rel);
            if !members.iter().any(|m| m.dir == target) {
                out.push(format!(
                    "dependency-direction: Cargo.toml: [workspace.dependencies] '{alias}' has \
                     path = '{rel}' (resolved: '{target}'), which is not a declared workspace member \
                     — an out-of-tree crate reachable from every member is exactly the \
                     infrastructure edge AK-001 forbids"
                ));
            }
            continue;
        }
        if fields.is_some_and(|f| f.contains_key("git")) {
            out.push(format!(
                "dependency-direction: Cargo.toml: [workspace.dependencies] '{alias}' has a git \
                 source — the entry is inheritable by every member, gauntlet-domain included, so \
                 it compiles code from outside the workspace into the domain with no scanner \
                 reading a byte (AK-001, spec §24.3)"
            ));
            continue;
        }
        if fields.is_some_and(has_alternate_registry) {
            out.push(format!(
                "dependency-direction: Cargo.toml: [workspace.dependencies] '{alias}' names an \
                 alternate registry — the index it resolves against is chosen in cargo \
                 configuration or in the developer's environment, so the NAME is no evidence of \
                 the code; only the default registry is admissible here (AK-001, spec §24.3, \
                 critic-5 C2)"
            ));
            continue;
        }
        let package = fields
            .and_then(|f| f.get("package"))
            .and_then(|value| value.as_str())
            .unwrap_or(alias);
        if !allowed.contains(&package) {
            out.push(format!(
                "dependency-direction: Cargo.toml: [workspace.dependencies] '{alias}' (resolved: \
                 '{package}') is a registry entry no layer's external allowlist permits {allowed:?} \
                 — the root table is one '{alias}.workspace = true' away from compiling it into \
                 gauntlet-domain. If a layer needs it, add it to that layer's allowlist with a \
                 recorded justification (AK-001, spec §24.3)"
            ));
        }
    }
    out
}

/// How an external dependency's source was declared, when it is neither a
/// workspace member nor a `path` that failed to resolve. The description goes
/// into the violation so the maintainer sees the channel, not just the name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExternalSource {
    /// A bare or table version requirement served by the DEFAULT registry
    /// (`shadow-net = "0.8"`).
    Registry,
    /// `registry = "…"` / `registry-index = "…"` — a named registry (critic-5
    /// C2).
    AlternateRegistry,
    /// Any `git = "…"` source, any URL spelling.
    Git,
    /// `workspace = true` inheriting a root-table entry of that kind.
    InheritedGit,
    InheritedRegistry,
    InheritedAlternateRegistry,
    InheritedPath,
    /// `workspace = true` for an alias the root table does not carry, or a
    /// spec this classifier cannot read as a source.
    Unspecified,
}

impl ExternalSource {
    fn describe(self) -> &'static str {
        match self {
            ExternalSource::Registry => "a default-registry version requirement",
            ExternalSource::AlternateRegistry => "an alternate-registry source",
            ExternalSource::Git => "a git source",
            ExternalSource::InheritedGit => "workspace inheritance of a root-table git entry",
            ExternalSource::InheritedRegistry => {
                "workspace inheritance of a root-table default-registry entry"
            }
            ExternalSource::InheritedAlternateRegistry => {
                "workspace inheritance of a root-table alternate-registry entry"
            }
            ExternalSource::InheritedPath => {
                "workspace inheritance of a root-table path outside the member set"
            }
            ExternalSource::Unspecified => "an unspecified external source",
        }
    }

    /// Whether an allowlist entry may admit a dependency from this source
    /// (critic-5 C2). The allowlist names a CRATE; only the default registry
    /// makes a name evidence of the code behind it. A git URL, an alternate
    /// registry index, or a source this classifier could not read is chosen by
    /// whoever wrote the URL — allowlisting `toml` must not admit
    /// `toml = { git = "https://…/toml-fork" }`, which would hand this
    /// instrument's own manifest parser to the author of that fork.
    fn admissible_by_allowlist(self) -> bool {
        matches!(
            self,
            ExternalSource::Registry | ExternalSource::InheritedRegistry
        )
    }
}

/// Whether a dependency spec table names a registry other than the default
/// one. `registry = "…"` resolves its index through cargo configuration, and
/// `registry-index = "…"` states the index outright; neither is the default
/// registry, so neither may be admitted by name.
fn has_alternate_registry(fields: &Table) -> bool {
    fields.contains_key("registry") || fields.contains_key("registry-index")
}

/// One dependency that is neither a workspace member nor an unresolved `path`:
/// the external channel, kept so `check_member` can enforce the layer
/// allowlist against it (critic-4 C1).
struct ExternalDep {
    section: String,
    key: String,
    name: String,
    source: ExternalSource,
}

/// One enabled FEATURE on one INTERNAL (workspace-member) dependency edge
/// (Task 11 correction round 2, F2 — recorded boundary-suite growth): the
/// section it was declared in (including target-scoped labels), the
/// manifest key, the resolved member crate, and the feature specifier.
/// Until F2 the walker read every field of a dependency spec EXCEPT
/// `features`, so "no production edge enables `gauntlet-app/composition`"
/// was true but unpinned — invisible to every check here, silently
/// regressable. This type is the visibility: feature specifiers on
/// internal edges are COLLECTED, and [`composition_seal_violations`] pins
/// the one legal edge.
#[derive(Clone, Debug)]
pub struct FeatureEdge {
    pub section: String,
    pub key: String,
    pub dep: String,
    pub feature: String,
}

/// The `features` specifier of a dependency spec table, in cargo's both
/// spellings: an array of feature names, or a single name as a bare string.
/// Anything else (absent, wrong type) is no feature specifier.
fn features_of(fields: &Table) -> Vec<String> {
    match fields.get("features") {
        Some(toml::Value::Array(list)) => list
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::to_string)
            .collect(),
        Some(toml::Value::String(single)) => vec![single.clone()],
        _ => Vec::new(),
    }
}

/// Internal (workspace-member) dependencies declared in `text`, the `path`
/// dependencies that resolve to nothing known, every external dependency
/// whatever its source kind, and the feature specifiers on the internal
/// edges (F2).
struct DeclaredDeps {
    internal: Vec<(String, String)>,
    unresolved_paths: Vec<(String, String, String, String)>,
    external: Vec<ExternalDep>,
    feature_edges: Vec<FeatureEdge>,
}

fn declared_deps(
    member_dir: &str,
    text: &str,
    members: &[Member],
    root_deps: &BTreeMap<String, RootDep>,
) -> Result<DeclaredDeps, String> {
    let manifest = parse_table(text).map_err(|e| format!("invalid TOML: {e}"))?;
    let mut internal = Vec::new();
    let mut unresolved_paths = Vec::new();
    let mut external = Vec::new();
    let mut feature_edges = Vec::new();
    for (section, table) in &dependency_sections(&manifest) {
        for (key, spec) in *table {
            let fields = spec.as_table();
            let mut dep = key.clone();
            let mut source = ExternalSource::Unspecified;
            let mut unresolved = false;
            if let Some(fields) = fields {
                if let Some(renamed) = fields.get("package").and_then(|v| v.as_str()) {
                    dep = renamed.to_string();
                }
                if fields.contains_key("git") {
                    source = ExternalSource::Git;
                } else if has_alternate_registry(fields) {
                    source = ExternalSource::AlternateRegistry;
                } else if fields.contains_key("version") {
                    source = ExternalSource::Registry;
                }
                if let Some(rel) = fields.get("path").and_then(|v| v.as_str()) {
                    let target = join_posix(member_dir, rel);
                    match members.iter().find(|m| m.dir == target) {
                        // A `path` that resolves to a member is internal,
                        // whatever else the spec table said.
                        Some(hit) => {
                            dep = hit.name.clone();
                            source = ExternalSource::Unspecified;
                        }
                        None => {
                            unresolved_paths.push((
                                section.clone(),
                                key.clone(),
                                rel.to_string(),
                                target,
                            ));
                            unresolved = true;
                        }
                    }
                }
                // Workspace inheritance wins last: with `workspace = true`
                // the root table is authoritative for what the crate is.
                if fields.get("workspace").and_then(|v| v.as_bool()) == Some(true)
                    && let Some(root) = root_deps.get(key)
                {
                    dep = root.package.clone();
                    source = match root.source {
                        RootDepSource::Member => ExternalSource::Unspecified,
                        RootDepSource::Path => ExternalSource::InheritedPath,
                        RootDepSource::Git => ExternalSource::InheritedGit,
                        RootDepSource::Registry => ExternalSource::InheritedRegistry,
                        RootDepSource::AlternateRegistry => {
                            ExternalSource::InheritedAlternateRegistry
                        }
                    };
                    // A member may narrow an inherited entry to a named
                    // registry of its own; the narrowing decides the source.
                    if has_alternate_registry(fields) {
                        source = ExternalSource::AlternateRegistry;
                    }
                }
            } else {
                // A bare string spec (`shadow-net = "0.8"`) is a registry
                // requirement and used to be invisible to every check.
                source = ExternalSource::Registry;
            }
            if members.iter().any(|m| m.name == dep) {
                internal.push((section.clone(), dep.clone()));
                // F2: the walker no longer ignores feature specifiers on
                // internal edges — the composition seal pins against them.
                // (For a `workspace = true` edge these are the member's OWN
                // additions; features inherited from the root table are
                // collected by `root_feature_edges`.)
                for feature in fields.map(features_of).unwrap_or_default() {
                    feature_edges.push(FeatureEdge {
                        section: section.clone(),
                        key: key.clone(),
                        dep: dep.clone(),
                        feature,
                    });
                }
            } else if !unresolved {
                external.push(ExternalDep {
                    section: section.clone(),
                    key: key.clone(),
                    name: dep,
                    source,
                });
            }
        }
    }
    Ok(DeclaredDeps {
        internal,
        unresolved_paths,
        external,
        feature_edges,
    })
}

/// Check one member manifest against its layer rule — the internal upper
/// bound AND the external allowlist.
pub fn check_member(
    member: &Member,
    text: &str,
    members: &[Member],
    root_deps: &BTreeMap<String, RootDep>,
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
    match declared_deps(&member.dir, text, members, root_deps) {
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
            // Critic-4 C1/M8: anything that is not a member and not a failed
            // `path` is the external channel. It is legal only by name, on
            // this layer's allowlist; the empty allowlists are what make the
            // channel fail closed rather than fail open.
            let allow = layer.allowed_external();
            let allow_names = allowlist_names(allow);
            for dep in declared.external {
                let listed = allow_names.contains(&dep.name.as_str());
                if listed && dep.source.admissible_by_allowlist() {
                    continue;
                }
                let citation = match layer {
                    Layer::Domain => "AK-001",
                    Layer::App => "AK-002",
                    _ => "spec §4.1, §24.3",
                };
                // Critic-5 C2: an allowlisted NAME is not an allowlisted
                // CRATE. The allowlist admits a (name, source) pair, and only
                // the default registry makes the name evidence of the code.
                if listed {
                    violations.push(format!(
                        "dependency-direction: {manifest}: crate '{}' ({}) declares {} on '{}' \
                         (resolved: '{}') through {} — '{}' is on this layer's external allowlist \
                         {:?}, but an allowlist admits a (crate, source) PAIR: only the default \
                         registry makes the name evidence of the code behind it. A git URL or a \
                         named registry index is chosen by whoever wrote it, so it can substitute \
                         arbitrary code — including for this suite's own manifest parser \
                         ({citation}, spec §24.3)",
                        member.name,
                        layer.name(),
                        dep.section,
                        dep.key,
                        dep.name,
                        dep.source.describe(),
                        dep.name,
                        allow_names
                    ));
                    continue;
                }
                violations.push(format!(
                    "dependency-direction: {manifest}: crate '{}' ({}) declares {} on '{}' \
                     (resolved: '{}', {}), which is neither a workspace member nor on the layer's \
                     external allowlist {:?} — {} forbids the channel, not just the enumerated \
                     vendor names; permit it deliberately by listing it with a justification, or \
                     not at all",
                    member.name,
                    layer.name(),
                    dep.section,
                    dep.key,
                    dep.name,
                    dep.source.describe(),
                    allow_names,
                    citation
                ));
            }
        }
    }
    violations
}

// ---------------------------------------------------------------------------
// Feature specifiers on internal edges (Task 11 correction round 2, F2 —
// recorded boundary-suite growth). Until F2 this crate read every field of
// a dependency spec except `features`, so the application layer's
// `composition` cargo feature — the audited §7.1 seam correction round 1
// (C1/B-M1) opened for ladder/rung construction — was sealed by nothing
// mechanical: "no production edge enables it" was true and unpinned. The
// three items below are deliberately NARROW: collect feature specifiers on
// internal edges (member manifests and the inheritable root table), and
// pin exactly one seal. Recorded act for the next adversarial review:
// this is a new class of manifest evidence, not a widening of any
// existing check.
// ---------------------------------------------------------------------------

/// Feature specifiers on the INTERNAL (workspace-member) dependency edges
/// of one member manifest, one edge per enabled feature. Sections are
/// labeled as [`dependency_sections`] labels them (including
/// `target.<cfg>.<section>` spellings, so a feature cannot hide behind a
/// target scope).
pub fn member_feature_edges(
    member_dir: &str,
    text: &str,
    members: &[Member],
    root_deps: &BTreeMap<String, RootDep>,
) -> Result<Vec<FeatureEdge>, String> {
    declared_deps(member_dir, text, members, root_deps).map(|declared| declared.feature_edges)
}

/// Feature specifiers on the ROOT `[workspace.dependencies]` table's
/// member entries. That table is inheritable by EVERY member,
/// `gauntlet-domain` included, so a feature enabled there is the widest
/// possible opening of any seam — it can never be the dev-only edge.
pub fn root_feature_edges(root_manifest: &Table, members: &[Member]) -> Vec<FeatureEdge> {
    let mut out = Vec::new();
    let Some(deps) = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    else {
        return out;
    };
    for (alias, spec) in deps {
        let Some(fields) = spec.as_table() else {
            continue;
        };
        let dep = fields
            .get("package")
            .and_then(|value| value.as_str())
            .unwrap_or(alias)
            .to_string();
        if !members.iter().any(|m| m.name == dep) {
            continue;
        }
        for feature in features_of(fields) {
            out.push(FeatureEdge {
                section: "workspace.dependencies".to_owned(),
                key: alias.clone(),
                dep: dep.clone(),
                feature,
            });
        }
    }
    out
}

/// The composition feature seal (F2, widened by plan Task 20 with the
/// production edge recorded at that task): the ONLY edges in the workspace
/// permitted to enable `gauntlet-app/composition` are `gauntlet-cli`'s
/// unscoped `[dev-dependencies]` edge — the one correction round 1
/// recorded as the audited §7.1 composition seam — AND, from plan Task 20,
/// `gauntlet-cli`'s unscoped `[dependencies]` edge: the run composition is
/// the production caller the Task 11 note anticipated ("the production
/// wiring of Tasks 12/20"), and it constructs the §14.2 rung set
/// (`Ladder::new`) in a normal build of the composition root. Every other
/// spelling is a violation: any production section of any OTHER member,
/// any target-scoped or build-dependency section, and any root
/// `[workspace.dependencies]` entry (`owner: None`, inheritable by
/// every member). Narrow by design: this pins the TWO composition seams;
/// a general feature-policy language is not being invented here.
pub fn composition_seal_violations(
    manifest: &str,
    owner: Option<&str>,
    edges: &[FeatureEdge],
) -> Vec<String> {
    let mut out = Vec::new();
    for edge in edges {
        if edge.dep != "gauntlet-app" || edge.feature != "composition" {
            continue;
        }
        let legal = owner == Some("gauntlet-cli")
            && (edge.section == "dev-dependencies" || edge.section == "dependencies");
        if !legal {
            out.push(format!(
                "composition-seal: {manifest}: {} enables feature 'composition' on '{}' via \
                 [{}] — the audited §7.1 seam (Task 11 correction round 1, C1) is enabled \
                 ONLY by gauntlet-cli's unscoped [dev-dependencies] edge, plus (plan Task \
                 20) gauntlet-cli's unscoped [dependencies] edge: the run composition is \
                 the recorded production caller; any other edge puts the sealed ladder/rung \
                 constructors into a normal build of a crate that must not have them (Task \
                 11 correction round 2, F2)",
                owner.unwrap_or("the root [workspace.dependencies] table"),
                edge.dep,
                edge.section
            ));
        }
    }
    out
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

/// What the filesystem says about packages under `crates/**`: the directories
/// holding a `Cargo.toml`, and the places the walk could not look.
#[derive(Default)]
pub struct PackageDiscovery {
    /// Repo-relative package directories, sorted.
    pub dirs: Vec<String>,
    /// Discovery failures. Round-3 V18: these used to be swallowed, so an
    /// unreadable subtree under `crates/` read as "no undeclared packages".
    /// Missing evidence is not passing evidence (I8).
    pub violations: Vec<String>,
}

/// Every directory under `crates/**` that holds a `Cargo.toml`, repo-relative
/// and sorted. This is the filesystem's answer to "what packages exist",
/// against which `[workspace].members` is checked.
///
/// No directory name is excluded. The previous version skipped anything named
/// `target` unconditionally, so a package at `crates/target/Cargo.toml` was
/// invisible to the discovery check (round-3 V18); the workspace's build
/// directory is at the repo root, not under `crates/`, so nothing is gained by
/// the exclusion.
pub fn discover_package_dirs(root: &Path) -> PackageDiscovery {
    let mut out = PackageDiscovery::default();
    collect_packages(root, &root.join("crates"), &mut out);
    out.dirs.sort();
    out.violations.sort();
    out
}

fn rel_of(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_packages(root: &Path, dir: &Path, out: &mut PackageDiscovery) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            if dir.exists() {
                out.violations.push(format!(
                    "dependency-direction: {}: directory is not readable ({err}) — a package \
                     hiding under an unreadable subtree of crates/ would be auto-included by \
                     Cargo and reported by nothing (I8, AK-001)",
                    rel_of(root, dir)
                ));
            }
            return;
        }
    };
    if dir.join("Cargo.toml").is_file() {
        out.dirs.push(rel_of(root, dir));
    }
    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                out.violations.push(format!(
                    "dependency-direction: {}: file type is not readable ({err}) — an entry the \
                     discovery walk cannot classify is missing evidence, not a pass (I8)",
                    rel_of(root, &entry.path())
                ));
                continue;
            }
        };
        // `is_dir()` on the entry's own type: a symlinked directory is not
        // descended into here either — `crate::walk` reports those.
        if file_type.is_dir() {
            collect_packages(root, &entry.path(), out);
        }
    }
}

/// `[patch.*]` and `[replace]` entries in the root manifest (round-3 V9,
/// critic-4 M5).
///
/// Both tables silently substitute an out-of-tree crate for a declared
/// dependency across the WHOLE workspace, including `gauntlet-domain`. Neither
/// appears in any dependency section, so `dependency_sections` never saw them
/// and `discover_package_dirs` never walks outside `crates/`.
///
/// Every entry must resolve to a workspace member by `path`. A git or
/// registry patch is a violation on its own (M5): `[patch.crates-io] toml =
/// { git = "…" }` swaps the parser behind every manifest check this suite
/// performs, and there is no allowlist for that channel — a patch that is not
/// a member is not a patch, it is an injection.
pub fn check_patch_and_replace(root_manifest: &Table, members: &[Member]) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |label: String, spec: &toml::Value| {
        let fields = spec.as_table();
        let Some(rel) = fields
            .and_then(|f| f.get("path"))
            .and_then(|value| value.as_str())
        else {
            let kind = if fields.is_some_and(|f| f.contains_key("git")) {
                "a git source"
            } else if fields.is_some_and(has_alternate_registry) {
                "an alternate-registry source"
            } else {
                "a non-path source"
            };
            out.push(format!(
                "dependency-direction: Cargo.toml: {label} uses {kind} — a patch or replace \
                 entry substitutes that crate for every member that depends on it, including \
                 gauntlet-domain and this suite's own toml dependency, and it is invisible to \
                 every source scanner (AK-001, spec §24.3)"
            ));
            return;
        };
        let target = join_posix("", rel);
        if !members.iter().any(|m| m.dir == target) {
            out.push(format!(
                "dependency-direction: Cargo.toml: {label} has path = '{rel}' (resolved: \
                 '{target}'), which is not a declared workspace member — a patch or replace entry \
                 substitutes that crate for every member that depends on it, including \
                 gauntlet-domain, and it is invisible to every source scanner (AK-001)"
            ));
        }
    };
    if let Some(patch) = root_manifest.get("patch").and_then(|v| v.as_table()) {
        for (registry, entries) in patch {
            let Some(entries) = entries.as_table() else {
                continue;
            };
            for (name, spec) in entries {
                check(format!("[patch.{registry}] '{name}'"), spec);
            }
        }
    }
    if let Some(replace) = root_manifest.get("replace").and_then(|v| v.as_table()) {
        for (name, spec) in replace {
            check(format!("[replace] '{name}'"), spec);
        }
    }
    out
}

/// Cargo-configuration keys that decide what the compiler is handed, wherever
/// they appear. Each carries the reason it is forbidden; the pin
/// `forbidden_cargo_config_keys_state_a_reason` fails if an entry loses one.
///
/// Critic-5 C1: the previous version probed exactly two hard-coded locations
/// (`[source.*]` and `[build].rustflags`), and cargo honours rustflags under
/// target-scoped tables too — `[target.'cfg(all())']` matches EVERY compile.
/// `rustflags = ["--extern", "shadow=vendor/libshadow.rlib"]` there puts an
/// arbitrary rlib in the extern prelude of every crate in the workspace, so
/// `pub use shadow::backdoor;` compiles in `gauntlet-domain` with its manifest
/// still declaring nothing. The walk below is therefore generic: the key name
/// is what is forbidden, at any depth, under any table.
pub const FORBIDDEN_CARGO_CONFIG_KEYS: &[(&str, &str)] = &[
    (
        "rustflags",
        "flags are merged into every matching rustc invocation, and `--extern` puts an arbitrary \
         rlib in the extern prelude of every core crate while `--cfg` switches unscanned code in \
         — cargo honours this key under [build] AND under any [target.<cfg-or-triple>] table",
    ),
    (
        "rustdocflags",
        "the same channel for rustdoc invocations, which compile the same sources",
    ),
    (
        "rustc",
        "replaces the compiler binary itself, so nothing about what compiles is knowable from \
         this repository",
    ),
    (
        "rustc-wrapper",
        "wraps every rustc invocation and may add flags or substitute inputs before rustc sees \
         them",
    ),
    (
        "rustc-workspace-wrapper",
        "the same wrapper channel, applied to workspace members specifically",
    ),
    (
        "linker",
        "controls the final link step, where an arbitrary object file can join the binary",
    ),
    (
        "runner",
        "replaces the program that actually executes the test targets this gate reports on, so a \
         green gate would prove nothing",
    ),
    (
        "paths",
        "path overrides substitute local directories for declared dependencies workspace-wide, \
         outside every manifest this suite reads",
    ),
];

/// Environment-variable names denied inside a cargo configuration `[env]`
/// table (critic-8 E2). Cargo's own documentation: "The `[env]` section
/// allows you to set additional environment variables for build scripts,
/// rustc invocations, cargo run and cargo build" (The Cargo Book,
/// reference/config.html) — so an `[env]` entry is a direct line to the
/// processes that compile and run this repository's code, and every name
/// below is one of the channels [`FORBIDDEN_CARGO_CONFIG_KEYS`] closes,
/// spelled as an environment variable instead of a config key: cargo's
/// environment-variables page documents each as "flags to pass to all
/// compiler invocations" or as the wrapper/compiler substitution itself.
/// `[env] RUSTFLAGS = { value = "--extern shadow=…", force = true }` is the
/// exact payload the `rustflags` key ban exists to stop, delivered under a
/// child key the case-sensitive key walk never matched.
///
/// Matched ASCII-case-INSENSITIVELY, unlike the generic key walk:
/// environment variable names are looked up case-insensitively on Windows
/// (a host this suite acknowledges elsewhere, in its drive-letter path
/// rules) and case-sensitively nowhere that would make `RustFlags` a
/// DIFFERENT benign variable — the channel is denied, not one spelling of
/// it. Cargo's config KEYS, by contrast, are exact-match TOML lookups,
/// which is why [`walk_cargo_config`] stays case-sensitive.
///
/// The `[env]` table itself is NOT denied: benign entries (a `GAUNTLET_*`
/// variable, an `OPENSSL_DIR` for a vendored build) are ordinary
/// configuration. Only the names that reach the compiler are.
pub const FORBIDDEN_ENV_KEYS: &[(&str, &str)] = &[
    (
        "rustflags",
        "a space-separated list of flags passed to all compiler invocations cargo performs — \
         the [env] spelling of the rustflags key this check already denies",
    ),
    (
        "cargo_encoded_rustflags",
        "the 0x1f-separated encoding of the same all-compiler-invocations flag list, so the \
         payload survives any space in it",
    ),
    (
        "cargo_build_rustflags",
        "the environment spelling of build.rustflags, the key this check already denies at \
         any table depth",
    ),
    (
        "rustdocflags",
        "the same flag channel for rustdoc invocations, which compile these sources just as \
         rustc does",
    ),
    (
        "cargo_encoded_rustdocflags",
        "the 0x1f-separated encoding of the rustdoc flag channel, exactly like its rustc \
         counterpart",
    ),
    (
        "cargo_build_rustdocflags",
        "the environment spelling of build.rustdocflags, the key this check already denies",
    ),
    (
        "rustc",
        "cargo executes this binary instead of rustc, so nothing about what compiles is \
         knowable from this repository",
    ),
    (
        "cargo_build_rustc",
        "the environment spelling of build.rustc, the compiler-substitution key",
    ),
    (
        "rustc_wrapper",
        "cargo executes this wrapper around every rustc invocation, and a wrapper may add \
         flags or substitute inputs before rustc sees them",
    ),
    (
        "cargo_build_rustc_wrapper",
        "the environment spelling of build.rustc-wrapper, the wrapper key this check already \
         denies",
    ),
    (
        "rustc_workspace_wrapper",
        "the same wrapper channel applied to workspace members specifically, naming these \
         crates rather than the toolchain",
    ),
    (
        "cargo_build_rustc_workspace_wrapper",
        "the environment spelling of build.rustc-workspace-wrapper, the workspace-wrapper key",
    ),
    (
        "ld_preload",
        "the dynamic loader injects this library into every process cargo spawns — rustc, the \
         linker, and the test binaries this gate reports on alike — the channel the runner \
         ban exists to close",
    ),
    (
        "dyld_insert_libraries",
        "the macOS spelling of the dynamic-loader injection channel, with the same reach \
         into every spawned compiler process",
    ),
];

/// Whether an `[env]` entry name reaches the compiler (critic-8 E2), and the
/// recorded reason when it does: an exact [`FORBIDDEN_ENV_KEYS`] hit, or the
/// per-target flag family `CARGO_TARGET_<triple>_RUSTFLAGS`, whose name
/// embeds the target triple and so cannot be listed exactly (The Cargo Book,
/// environment-variables.html: "extra rustc flags for a target"). Matching is
/// ASCII-case-insensitive; see [`FORBIDDEN_ENV_KEYS`] for why.
fn forbidden_env_reason(name: &str) -> Option<&'static str> {
    if let Some((_, reason)) = FORBIDDEN_ENV_KEYS
        .iter()
        .find(|(forbidden, _)| forbidden.eq_ignore_ascii_case(name))
    {
        return Some(reason);
    }
    let lowered = name.to_ascii_lowercase();
    (lowered.starts_with("cargo_target_") && lowered.ends_with("_rustflags")).then_some(
        "cargo's per-target spelling of the rustflags channel: CARGO_TARGET_<triple>_RUSTFLAGS \
         passes these flags to every rustc invocation building that target, so it is the \
         rustflags key under a name that embeds the triple",
    )
}

/// The `[env]`-table half of the cargo-configuration check (critic-8 E2),
/// separate from [`walk_cargo_config`] because its matching rule differs: the
/// `[env]` table's CHILD keys are environment-variable names, so a
/// compiler-affecting name is denied wherever its case is spelled, while the
/// generic walk matches cargo's exact-match config keys. Cargo reads `[env]`
/// at the configuration root only — a nested `env` table under some other
/// section is not cargo's environment table and is not consulted here.
fn env_table_violations(table: &Table, rel: &str) -> Vec<String> {
    let Some(env) = table.get("env").and_then(|value| value.as_table()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in env.keys() {
        if let Some(reason) = forbidden_env_reason(name) {
            out.push(format!(
                "dependency-direction: {rel}: [env] {name} — {reason} (AK-001, spec §24.3)"
            ));
        }
    }
    out
}

/// `.cargo/config.toml` (and the legacy `.cargo/config`) can redirect what
/// compiles without touching any manifest this suite reads (critic-4 M5,
/// critic-5 C1/C2): `[source]` replacement sends every registry dependency to
/// a local tree outside every walk root; `[registries.*]` defines the index an
/// alternate-registry dependency resolves against; every key in
/// [`FORBIDDEN_CARGO_CONFIG_KEYS`] hands the compiler something this repository
/// does not describe; and since critic-8 E2 the `[env]` table's
/// compiler-affecting entries ([`FORBIDDEN_ENV_KEYS`]) are denied too — cargo
/// applies them to build scripts, rustc invocations, and cargo build/run, so
/// `[env] RUSTFLAGS = …` is the `rustflags` channel under another spelling.
/// All are violations; a legitimate future need is a
/// recorded decision that widens this function, not a silent config file.
///
/// Scope note (threat model, see the crate docs): this reads the IN-REPO cargo
/// configuration only. `$CARGO_HOME/config.toml` and the `RUSTFLAGS` /
/// `CARGO_*` environment variables are the same channel outside the
/// repository, and no file-based instrument can see them.
pub fn check_cargo_config(root: &Path) -> Vec<String> {
    let discovered = discover_cargo_configs(root);
    let mut out = discovered.violations;
    for rel in discovered.files {
        let rel = rel.as_str();
        let path = root.join(rel);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                // Critic-6 M1 / critic-7 MJ1: this used to be `continue`, the
                // one silent skip in a function whose whole thesis is I8. An
                // in-repo cargo config this suite cannot read is missing
                // evidence, not "no forbidden keys".
                out.push(format!(
                    "dependency-direction: {rel}: not readable ({err}) — cargo reads this file \
                     to decide what compiles, and a config the suite cannot read is missing \
                     evidence, not a pass (I8, spec §24.3)"
                ));
                continue;
            }
        };
        let table = match parse_table(&text) {
            Ok(table) => table,
            Err(err) => {
                out.push(format!(
                    "dependency-direction: {rel}: malformed TOML ({err}) — cargo reads it and \
                     this suite cannot, so the file is missing evidence, not a pass (I8)"
                ));
                continue;
            }
        };
        if let Some(source) = table.get("source").and_then(|value| value.as_table()) {
            for name in source.keys() {
                out.push(format!(
                    "dependency-direction: {rel}: [source.{name}] — source replacement redirects \
                     what compiles without touching any manifest the boundary suite reads; the \
                     replaced code compiles into core crates unscanned (AK-001, spec §24.3)"
                ));
            }
        }
        if let Some(registries) = table.get("registries").and_then(|value| value.as_table()) {
            for name in registries.keys() {
                out.push(format!(
                    "dependency-direction: {rel}: [registries.{name}] — a named registry index is \
                     what turns `registry = \"{name}\"` in a manifest into actual code, and the \
                     name is no evidence of that code; the boundary suite admits the default \
                     registry only (AK-001, spec §24.3, critic-5 C2)"
                ));
            }
        }
        // Critic-8 E2: the `[env]` table's values reach build scripts, rustc
        // invocations, and the programs cargo build/run execute, so a
        // compiler-affecting NAME there is the same channel as the forbidden
        // keys — `[env] RUSTFLAGS = …` was the exact payload of the
        // `rustflags` ban, delivered under a child key the walk never matched.
        out.extend(env_table_violations(&table, rel));
        walk_cargo_config(&table, "", rel, &mut out);
    }
    out.extend(check_toolchain_file(root));
    out
}

/// Directories the configuration walk never descends into: build output and
/// version-control internals hold no cargo configuration cargo would read for
/// a build of this workspace, and both are large enough to make the walk
/// expensive.
const CONFIG_WALK_SKIPPED_DIRS: &[&str] = &["target", ".git"];

/// Every in-repo cargo configuration file, and the places the walk could not
/// look.
struct CargoConfigDiscovery {
    files: Vec<String>,
    violations: Vec<String>,
}

/// Cargo reads `.cargo/config.toml` from the invocation directory UPWARD, not
/// only from the workspace root. A config committed at
/// `crates/gauntlet-domain/.cargo/config.toml` therefore applies to every
/// `cargo build` run from inside that crate — an ordinary thing to do — while
/// a root-only probe reports nothing.
///
/// This was not on any critic's list; it is the same channel as critic-5 C1 in
/// a different location, and a check that closed the channel only at the root
/// would be claiming more than it enforces.
fn discover_cargo_configs(root: &Path) -> CargoConfigDiscovery {
    let mut discovery = CargoConfigDiscovery {
        files: Vec::new(),
        violations: Vec::new(),
    };
    collect_cargo_configs(root, root, &mut discovery);
    discovery.files.sort();
    discovery.violations.sort();
    discovery
}

fn collect_cargo_configs(root: &Path, dir: &Path, out: &mut CargoConfigDiscovery) {
    for name in ["config.toml", "config"] {
        let candidate = dir.join(".cargo").join(name);
        if candidate.is_file() {
            out.files.push(rel_of(root, &candidate));
        }
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            out.violations.push(format!(
                "dependency-direction: {}: directory is not readable ({err}) — a cargo config \
                 hiding under an unreadable subtree still applies to every build started there, \
                 and missing evidence is not passing evidence (I8)",
                rel_of(root, dir)
            ));
            return;
        }
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".cargo" || CONFIG_WALK_SKIPPED_DIRS.contains(&name.as_str()) {
            continue;
        }
        collect_cargo_configs(root, &entry.path(), out);
    }
}

/// `rust-toolchain.toml` / `rust-toolchain` is a declared build input of this
/// repository, and `[toolchain] path = "…"` points rustup at an arbitrary
/// toolchain directory — the `rustc` substitution channel of
/// [`FORBIDDEN_CARGO_CONFIG_KEYS`], spelled in a different file.
///
/// A named `channel` is not flagged: pinning a released toolchain is ordinary,
/// reproducible practice, and the channel name resolves through rustup's own
/// distribution rather than through a path this repository chose.
///
/// Unreadable or unparseable input is a VIOLATION, never a skip (critic-6 M1
/// / critic-7 MJ1): a file that exists, that cargo reads to choose the
/// compiler, and that this suite cannot interpret is missing evidence. The
/// one documented non-TOML form is the legacy extensionless `rust-toolchain`
/// carrying a bare channel name (`stable`, `1.92.0`, `nightly-2026-01-01`),
/// which carries no `path` key by construction; anything else that does not
/// parse is reported.
pub fn check_toolchain_file(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for rel in ["rust-toolchain.toml", "rust-toolchain"] {
        let path = root.join(rel);
        if !path.is_file() {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                out.push(format!(
                    "dependency-direction: {rel}: not readable ({err}) — rustup reads this file \
                     to choose the compiler, and a pin the suite cannot read is missing \
                     evidence, not a pass (I8, spec §24.3)"
                ));
                continue;
            }
        };
        let table = match parse_table(&text) {
            Ok(table) => table,
            Err(err) => {
                if rel == "rust-toolchain.toml" || !is_bare_channel_name(&text) {
                    out.push(format!(
                        "dependency-direction: {rel}: unparseable ({err}) and not the one \
                         documented non-TOML form (a bare channel name in the legacy \
                         extensionless file) — cargo still reads it to choose the compiler, so \
                         input this suite cannot interpret is missing evidence, not a pass (I8, \
                         spec §24.3)"
                    ));
                }
                continue;
            }
        };
        if let Some(toolchain) = table.get("toolchain").and_then(|value| value.as_table())
            && let Some(path) = toolchain.get("path").and_then(|value| value.as_str())
        {
            out.push(format!(
                "dependency-direction: {rel}: [toolchain] path = '{path}' — this replaces the \
                 compiler for every build of this workspace, so nothing about what actually \
                 compiles is knowable from the manifests and sources this suite reads (AK-001, \
                 spec §24.3)"
            ));
        }
    }
    out
}

/// Whether the legacy extensionless `rust-toolchain` content is a bare
/// channel name: one line, non-empty, only the bytes a channel name uses.
fn is_bare_channel_name(text: &str) -> bool {
    let trimmed = text.trim();
    let mut lines = trimmed.lines();
    let Some(only) = lines.next() else {
        return false;
    };
    lines.next().is_none()
        && !only.is_empty()
        && only
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// Every table of a parsed cargo config, reporting a forbidden key wherever it
/// occurs. Walking generically is the point (critic-5 C1): probing two known
/// spellings leaves every other spelling of the same channel open.
fn walk_cargo_config(table: &Table, prefix: &str, rel: &str, out: &mut Vec<String>) {
    for (key, value) in table {
        if let Some((_, reason)) = FORBIDDEN_CARGO_CONFIG_KEYS
            .iter()
            .find(|(forbidden, _)| *forbidden == key.as_str())
        {
            let table_label = if prefix.is_empty() {
                "<root>".to_string()
            } else {
                prefix.to_string()
            };
            out.push(format!(
                "dependency-direction: {rel}: [{table_label}] {key} — {reason} (AK-001, spec \
                 §24.3)"
            ));
        }
        if let Some(child) = value.as_table() {
            let nested = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            walk_cargo_config(child, &nested, rel, out);
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

    /// F2 (Task 11 correction round 2): the composition feature seal is
    /// PINNED, not merely true today. "No production (non-dev) edge enables
    /// `gauntlet-app/composition`" held at commit a507d08 with no test or
    /// arch rule asserting it — the manifest walker ignored feature
    /// specifiers entirely. The pin walks every member manifest AND the
    /// root `[workspace.dependencies]` table (inheritable by every member)
    /// through the new feature-edge surface and asserts the only
    /// composition-enabling edge in the workspace is gauntlet-cli's
    /// dev-dependency. Anti-vacuity: a walker that saw nothing would pass
    /// emptily, so the pin also asserts it SEES the one legal edge.
    #[test]
    fn the_composition_feature_seal_holds() {
        let root = crate::repo_root();
        let root_text =
            std::fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest reads");
        let members = super::read_members(&root, &root_text)
            .unwrap_or_else(|e| panic!("workspace members resolve: {e}"));
        let root_table = root_text
            .parse::<toml::Table>()
            .expect("the root manifest parses");
        let root_deps = super::workspace_dep_records(&root_table, &members);
        let mut violations = Vec::new();
        let mut saw_the_legal_edge = false;
        violations.extend(super::composition_seal_violations(
            "Cargo.toml",
            None,
            &super::root_feature_edges(&root_table, &members),
        ));
        for member in &members {
            let manifest_path = format!("{}/Cargo.toml", member.dir);
            let text = std::fs::read_to_string(root.join(&manifest_path))
                .unwrap_or_else(|e| panic!("{manifest_path} is readable: {e}"));
            let edges = super::member_feature_edges(&member.dir, &text, &members, &root_deps)
                .unwrap_or_else(|e| panic!("{manifest_path} parses: {e}"));
            if edges.iter().any(|edge| {
                member.name == "gauntlet-cli"
                    && edge.dep == "gauntlet-app"
                    && edge.feature == "composition"
                    && edge.section == "dev-dependencies"
            }) {
                saw_the_legal_edge = true;
            }
            violations.extend(super::composition_seal_violations(
                &manifest_path,
                Some(&member.name),
                &edges,
            ));
        }
        assert!(
            saw_the_legal_edge,
            "the walker must SEE gauntlet-cli's dev-dependency composition edge — a blind \
             walker would pass this pin vacuously (F2 anti-vacuity)"
        );
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("composition-seal", &violations)
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

    fn no_root_deps() -> BTreeMap<String, RootDep> {
        BTreeMap::new()
    }

    fn root_deps_from(text: &str) -> BTreeMap<String, RootDep> {
        let table = parse_table(text).expect("fixture root manifest parses");
        workspace_dep_records(&table, &fixture_members())
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
            &no_root_deps(),
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
                &no_root_deps()
            )
            .is_empty()
        );
        assert!(
            check_member(
                &member("crates/gauntlet-app", "gauntlet-app"),
                app,
                &members,
                &no_root_deps()
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
            &no_root_deps(),
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
            &no_root_deps(),
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
                &no_root_deps()
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
            &no_root_deps(),
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
            &no_root_deps(),
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
            &no_root_deps(),
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
            &no_root_deps(),
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
            &root_deps_from(root_manifest),
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
            &no_root_deps(),
        );
        assert_eq!(unknown.len(), 1, "{unknown:?}");
        assert!(unknown[0].contains("gauntlet-mystery"));

        let malformed = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            "[dependencies\nbroken",
            &fixture_members(),
            &no_root_deps(),
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
            &no_root_deps(),
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
            &no_root_deps(),
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
        let violations = check_workspace_dependency_table(&table, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("vendor/infra"), "{violations:?}");
    }

    // ------------------------------------------------------------------
    // Critic-4 C1: the external-dependency channel fails closed.

    #[test]
    fn the_external_allowlists_are_pinned() {
        // The empty lists ARE the mechanism (C1): pre-populating one silently
        // would reopen the channel the whole design closes. Growth is a
        // deliberate edit that must also update this pin.
        assert!(
            DOMAIN_EXTERNAL_ALLOWLIST.is_empty(),
            "the domain allowlist is empty (AK-001)"
        );
        // Task 10 correction round 2 adds the same mature SHA-256 crate at
        // the port boundary, where both adapter consumption paths can
        // recompute canonical envelope bytes. The pin keeps that growth
        // explicit rather than reopening ports to arbitrary dependencies.
        assert_eq!(
            allowlist_names(PORTS_EXTERNAL_ALLOWLIST),
            vec!["sha2"],
            "the ports allowlist carries only the §13.2 envelope digest crate"
        );
        // Plan Task 5 (spec §8.1 config_fingerprint) added the application
        // layer's one recorded dependency; AK-002's "ports only" default
        // fails closed around exactly this entry.
        assert_eq!(
            allowlist_names(APPLICATION_EXTERNAL_ALLOWLIST),
            vec!["sha2"],
            "the application allowlist carries only the Task 5 digest crate"
        );
        assert!(CLI_EXTERNAL_ALLOWLIST.is_empty());
        // Plan Task 6 (spec §18.1 embedded SQL, §6.2 content-addressed
        // evidence) added the adapter layer's two recorded dependencies; slice E1a
        // adds postgres for the canonical durable store. The empty list was the
        // design until the first adapter needed a vendor, and growth stays a
        // deliberate edit that updates this pin.
        assert_eq!(
            allowlist_names(ADAPTER_EXTERNAL_ALLOWLIST),
            vec!["postgres", "rusqlite", "sha2"],
            "the adapter allowlist carries postgres (E1a), rusqlite (Task 6), and digest crates"
        );
        assert_eq!(
            allowlist_names(BOUNDARY_TESTS_EXTERNAL_ALLOWLIST),
            vec!["toml"],
            "the boundary suite's one permitted dependency is the manifest parser"
        );
        assert_eq!(external_allowlist_union(), vec!["sha2", "postgres", "rusqlite", "toml"]);
        assert_eq!(
            ALL_EXTERNAL_ALLOWLISTS.len(),
            6,
            "every layer classified by `classify` needs an allowlist in the inventory, or a layer \
             is policed by nothing"
        );
    }

    /// Anti-vacuity for the allowlist itself: an entry may only be added
    /// together with the reason it was added. A one-word placeholder is not a
    /// reason, so the bar is a sentence naming the purpose.
    #[test]
    fn allowlist_entries_state_a_reason() {
        for (layer, list) in ALL_EXTERNAL_ALLOWLISTS {
            for (crate_name, reason) in *list {
                assert!(
                    reason.split_whitespace().count() >= 6,
                    "the {layer} allowlist admits '{crate_name}' with the reason {reason:?} — an \
                     allowlist entry is a recorded decision; state what the crate is for and what \
                     permits it (spec §24.3), or do not list it"
                );
            }
        }
    }

    /// The same bar for the cargo-configuration key inventory: a key may not
    /// join the forbidden list without the channel it closes being written down,
    /// and the list may not shrink below the spellings critic-5 C1 demonstrated.
    #[test]
    fn forbidden_cargo_config_keys_state_a_reason() {
        for (key, reason) in FORBIDDEN_CARGO_CONFIG_KEYS {
            assert!(
                reason.split_whitespace().count() >= 6,
                "the forbidden cargo-config key '{key}' carries the reason {reason:?} — say which \
                 channel it opens"
            );
        }
        for required in ["rustflags", "linker", "rustc-wrapper", "runner"] {
            assert!(
                FORBIDDEN_CARGO_CONFIG_KEYS
                    .iter()
                    .any(|(key, _)| *key == required),
                "'{required}' hands the compiler something no manifest in this repository \
                 describes; removing it from the inventory reopens critic-5 C1"
            );
        }
    }

    /// The same bar for the `[env]` inventory (critic-8 E2): a name may not
    /// join without its channel written down, and the spellings that reach
    /// rustc may not quietly shrink.
    #[test]
    fn forbidden_env_keys_state_a_reason() {
        for (key, reason) in FORBIDDEN_ENV_KEYS {
            assert!(
                reason.split_whitespace().count() >= 6,
                "the forbidden [env] key '{key}' carries the reason {reason:?} — say which \
                 channel it opens"
            );
        }
        for required in [
            "rustflags",
            "cargo_encoded_rustflags",
            "rustc",
            "rustc_wrapper",
            "rustc_workspace_wrapper",
        ] {
            assert!(
                FORBIDDEN_ENV_KEYS.iter().any(|(key, _)| *key == required),
                "'{required}' reaches the build scripts, rustc invocations, and cargo \
                 build/run processes the [env] table applies to; removing it from the \
                 inventory reopens critic-8 E2"
            );
        }
    }

    /// Critic-8 E2: the `[env]` table delivers compiler flags under a CHILD
    /// key the generic walk never matched — the child key is the environment
    /// variable name, uppercase, and the case-sensitive key walk compared it
    /// against lowercase config keys. Matching is case-insensitive here, and
    /// benign entries stay clean: the channel to the compiler is denied, not
    /// the whole table.
    #[test]
    fn compiler_affecting_env_entries_are_flagged_and_benign_ones_are_not() {
        for (label, env_section) in [
            // The critic's exact payload.
            (
                "the critic's payload",
                "[env]\nRUSTFLAGS = { value = \"--extern \
                 shadow=/home/u/vendor/libshadow.rlib --cfg injected\", force = true }\n",
            ),
            // Case smuggling: the channel, not the spelling, is denied.
            (
                "lowercase smuggling",
                "[env]\nrustflags = \"--cfg injected\"\n",
            ),
            ("mixed case", "[env]\nRustFlags = \"--cfg injected\"\n"),
            (
                "encoded flags",
                "[env]\nCARGO_ENCODED_RUSTFLAGS = \"--extern\\u001Fshadow=/home/u/s.rlib\"\n",
            ),
            (
                "build-section env spelling",
                "[env]\nCARGO_BUILD_RUSTFLAGS = \"--cfg injected\"\n",
            ),
            (
                "compiler substitution",
                "[env]\nRUSTC = \"/opt/shadow-rustc/bin/rustc\"\n",
            ),
            (
                "wrapper",
                "[env]\nRUSTC_WRAPPER = \"/opt/evil/sccache-wrapper\"\n",
            ),
            (
                "workspace wrapper",
                "[env]\nRUSTC_WORKSPACE_WRAPPER = \"/opt/evil/workspace-wrapper\"\n",
            ),
            (
                "rustdoc flags",
                "[env]\nRustDocFlags = \"--persist-doctests /opt/evil\"\n",
            ),
            // The per-target family, whose name embeds the triple.
            (
                "per-target rustflags",
                "[env]\nCARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = \"--cfg injected\"\n",
            ),
            // The dynamic-loader channels the critic named beside `runner`.
            (
                "dynamic loader",
                "[env]\nLD_PRELOAD = \"/opt/evil/hook.so\"\n",
            ),
            (
                "macOS dynamic loader",
                "[env]\nDYLD_INSERT_LIBRARIES = \"/opt/evil/hook.dylib\"\n",
            ),
        ] {
            let table = parse_table(env_section).expect("fixture parses");
            let violations = env_table_violations(&table, ".cargo/config.toml");
            assert!(
                !violations.is_empty(),
                "{label}: the [env] channel to the compiler must be denied: {violations:?}"
            );
            assert!(
                violations.iter().all(|v| v.contains("[env]")),
                "{label}: {violations:?}"
            );
        }

        // Benign entries stay clean — the Cargo Book's own [env] examples are
        // configuration, not compiler inputs.
        let benign = "[env]\nGAUNTLET_LOG = { value = \"debug\", force = true }\nOPENSSL_DIR = \
                      \"/opt/openssl\"\nSOME_TOOL_HOME = { value = \"tools\", relative = true }\n";
        let table = parse_table(benign).expect("fixture parses");
        assert!(
            env_table_violations(&table, ".cargo/config.toml").is_empty(),
            "benign [env] entries must stay clean: {:?}",
            env_table_violations(&table, ".cargo/config.toml")
        );

        // And a config with no [env] table at all reports nothing here.
        let plain = "[build]\njobs = 4\n";
        let table = parse_table(plain).expect("fixture parses");
        assert!(env_table_violations(&table, ".cargo/config.toml").is_empty());
    }

    #[test]
    fn a_registry_dependency_compiling_into_the_domain_is_flagged() {
        // Critic-4 C1 evasion (a): `shadow-net = "0.8"` used to be invisible —
        // no `path`, no member match, no denied term.
        let text = "[dependencies]\nshadow-net = \"0.8\"\n";
        let violations = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            text,
            &fixture_members(),
            &no_root_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("crates/gauntlet-domain/Cargo.toml"),
            "{}",
            violations[0]
        );
        assert!(violations[0].contains("'shadow-net'"), "{violations:?}");
        assert!(
            violations[0].contains("registry version requirement"),
            "{}",
            violations[0]
        );
        assert!(violations[0].contains("AK-001"), "{}", violations[0]);
    }

    #[test]
    fn a_git_dependency_is_flagged_in_any_url_spelling() {
        // Critic-4 C1 evasion (b): the scp-like form has no `://`, so not even
        // the incidental URL catch fired.
        for git_spec in [
            "git@github.com:attacker/gauntlet-infra",
            "https://example.com/x/y",
        ] {
            let text = format!("[dependencies]\ninfra = {{ git = \"{git_spec}\" }}\n");
            let violations = check_member(
                &member("crates/gauntlet-domain", "gauntlet-domain"),
                &text,
                &fixture_members(),
                &no_root_deps(),
            );
            assert_eq!(violations.len(), 1, "{text} -> {violations:?}");
            assert!(violations[0].contains("'infra'"), "{violations:?}");
            assert!(violations[0].contains("git source"), "{violations:?}");
        }
    }

    #[test]
    fn an_external_dependency_is_flagged_in_every_section_kind() {
        // dev-, build-, and target-scoped tables carry the same channel.
        let text = "[dev-dependencies]\nserde = \"1\"\n";
        let violations = check_member(
            &member("crates/gauntlet-app", "gauntlet-app"),
            text,
            &fixture_members(),
            &no_root_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("dev-dependencies"),
            "{}",
            violations[0]
        );
        assert!(violations[0].contains("AK-002"), "{}", violations[0]);
    }

    #[test]
    fn a_workspace_inherited_external_dependency_is_flagged() {
        // Critic-4 C1 evasion (c): the member manifest carries only an alias.
        let root_manifest = "[workspace.dependencies]\nserde = \"1\"\n";
        let text = "[dependencies]\nserde.workspace = true\n";
        let violations = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            text,
            &fixture_members(),
            &root_deps_from(root_manifest),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("workspace inheritance of a root-table default-registry entry"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn an_allowlisted_external_dependency_is_accepted_only_in_its_layer() {
        let root_manifest = "[workspace.dependencies]\ntoml = \"0.9\"\n";
        let root_deps = root_deps_from(root_manifest);
        let text = "[dependencies]\ntoml.workspace = true\n";
        assert!(
            check_member(
                &member("crates/gauntlet-arch-tests", "gauntlet-arch-tests"),
                text,
                &fixture_members(),
                &root_deps,
            )
            .is_empty(),
            "the boundary suite may keep its manifest parser"
        );
        let violations = check_member(
            &member("crates/gauntlet-domain", "gauntlet-domain"),
            text,
            &fixture_members(),
            &root_deps,
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("AK-001"), "{}", violations[0]);
    }

    #[test]
    fn root_workspace_git_and_unallowlisted_registry_entries_are_flagged() {
        let root_manifest = r#"
[workspace.dependencies]
gauntlet-domain = { path = "crates/gauntlet-domain" }
toml = "0.9"
serde = "1"
infra = { git = "git@github.com:attacker/gauntlet-infra" }
"#;
        let table = parse_table(root_manifest).expect("fixture parses");
        let violations = check_workspace_dependency_table(&table, &fixture_members());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|v| v.contains("'infra'") && v.contains("git source")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("'serde'") && v.contains("registry entry")),
            "{violations:?}"
        );
        // The member path and the allowlisted `toml` stay legal.
        assert!(
            !violations.iter().any(|v| v.contains("'gauntlet-domain'")),
            "{violations:?}"
        );
        assert!(
            !violations.iter().any(|v| v.contains("'toml'")),
            "{violations:?}"
        );
    }

    #[test]
    fn patch_entries_with_git_or_missing_sources_are_flagged() {
        // Critic-4 M5: the substituted crate can be this suite's own toml.
        let root_manifest =
            "[patch.crates-io]\ntoml = { git = \"https://github.com/attacker/toml-fork\" }\n";
        let table = parse_table(root_manifest).expect("fixture parses");
        let violations = check_patch_and_replace(&table, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("[patch.crates-io] 'toml'") && violations[0].contains("git"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn cargo_config_source_replacement_and_rustflags_are_flagged() {
        // Critic-4 M5: source replacement redirects every registry dependency
        // to a tree no walk covers; rustflags inject into every compile.
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-cargoconfig-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".cargo")).expect("fixture .cargo is creatable");
        std::fs::write(
            dir.join(".cargo/config.toml"),
            "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[source.vendored-sources]\ndirectory = \"vendor\"\n\n[build]\nrustflags = [\"--cfg\", \"injected\"]\n",
        )
        .expect("fixture config is written");

        let violations = check_cargo_config(&dir);
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|v| v.contains("[source.crates-io]") && v.contains("source replacement")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("[source.vendored-sources]")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("rustflags")),
            "{violations:?}"
        );

        // Absent config is clean; malformed config is a violation, never a skip.
        let empty = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-emptycfg-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&empty).ok();
        std::fs::create_dir_all(&empty).expect("fixture dir is creatable");
        assert!(check_cargo_config(&empty).is_empty());
        std::fs::create_dir_all(empty.join(".cargo")).expect("fixture .cargo is creatable");
        std::fs::write(empty.join(".cargo/config"), "[source\nbroken").expect("written");
        assert_eq!(check_cargo_config(&empty).len(), 1);

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&empty).ok();
    }

    /// Critic-5 C1: the same channel in the spellings the old two-key probe
    /// never looked at. `[target.'cfg(all())']` matches every compile, so
    /// `--extern` there puts an arbitrary rlib in every crate's extern prelude.
    #[test]
    fn target_scoped_rustflags_and_other_compiler_channels_are_flagged() {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-cargocfg-target-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        for (label, config, needle) in [
            (
                "cfg-all",
                "[target.'cfg(all())']\nrustflags = [\"--extern\", \
                 \"shadow=vendor/libshadow.rlib\"]\n",
                "rustflags",
            ),
            (
                "triple",
                "[target.x86_64-unknown-linux-gnu]\nrustflags = [\"--cfg\", \"injected\"]\n",
                "rustflags",
            ),
            (
                "linker",
                "[target.'cfg(all())']\nlinker = \"/opt/evil/ld\"\n",
                "linker",
            ),
            (
                "wrapper",
                "[build]\nrustc-wrapper = \"sccache\"\n",
                "rustc-wrapper",
            ),
            (
                "runner",
                "[target.'cfg(all())']\nrunner = \"./harness\"\n",
                "runner",
            ),
            ("paths", "paths = [\"vendor/toml\"]\n", "paths"),
            (
                "registries",
                "[registries.mirror]\nindex = \"sparse+https://attacker.example/x\"\n",
                "[registries.mirror]",
            ),
        ] {
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(dir.join(".cargo")).expect("fixture .cargo is creatable");
            std::fs::write(dir.join(".cargo/config.toml"), config).expect("fixture is written");
            let violations = check_cargo_config(&dir);
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{label}: {config} -> {violations:?}"
            );
        }

        // A cargo config in a CRATE directory applies to every build started
        // there. Found by this builder, not by a critic: cargo reads
        // `.cargo/config.toml` from the invocation directory upward, so a
        // root-only probe closes the channel only for builds started at the
        // root.
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("crates/gauntlet-domain/.cargo"))
            .expect("fixture crate .cargo is creatable");
        std::fs::write(
            dir.join("crates/gauntlet-domain/.cargo/config.toml"),
            "[build]\nrustflags = [\"--extern\", \"shadow=vendor/libshadow.rlib\"]\n",
        )
        .expect("fixture is written");
        let violations = check_cargo_config(&dir);
        assert!(
            violations.iter().any(|v| v.contains("rustflags")
                && v.contains("crates/gauntlet-domain/.cargo/config.toml")),
            "a cargo config inside a crate directory is unread: {violations:?}"
        );

        // The toolchain file is the same channel in another declared build
        // input: a path toolchain replaces the compiler outright.
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("fixture dir is creatable");
        std::fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\npath = \"/opt/evil/toolchain\"\n",
        )
        .expect("fixture is written");
        assert!(
            check_cargo_config(&dir)
                .iter()
                .any(|v| v.contains("[toolchain] path")),
            "{:?}",
            check_cargo_config(&dir)
        );
        // A pinned channel is ordinary practice and stays clean.
        std::fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.92\"\ncomponents = [\"clippy\"]\n",
        )
        .expect("fixture is written");
        assert!(
            check_cargo_config(&dir).is_empty(),
            "{:?}",
            check_cargo_config(&dir)
        );

        // Negative: an in-repo cargo config that touches none of these keys is
        // not flagged, so the check is a channel rule and not a file ban.
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".cargo")).expect("fixture .cargo is creatable");
        std::fs::write(
            dir.join(".cargo/config.toml"),
            "[net]\nretry = 3\n\n[term]\nverbose = false\n",
        )
        .expect("fixture is written");
        assert!(
            check_cargo_config(&dir).is_empty(),
            "{:?}",
            check_cargo_config(&dir)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The external policy covers every section cargo compiles from, not just
    /// `[dependencies]`: a build script's dependency and a target-scoped one
    /// are compiled code in the same crate (critic-4 C1 completeness).
    #[test]
    fn the_external_policy_covers_build_and_target_scoped_sections() {
        for (section, label) in [
            ("[build-dependencies]", "build-dependencies"),
            (
                "[target.'cfg(unix)'.dev-dependencies]",
                "target.cfg(unix).dev-dependencies",
            ),
            (
                "[target.x86_64-unknown-linux-gnu.dependencies]",
                "target.x86_64-unknown-linux-gnu.dependencies",
            ),
        ] {
            let text = format!("{section}\nshadow-net = \"0.8\"\n");
            let violations = check_member(
                &member("crates/gauntlet-domain", "gauntlet-domain"),
                &text,
                &fixture_members(),
                &no_root_deps(),
            );
            assert_eq!(violations.len(), 1, "{section} -> {violations:?}");
            assert!(
                violations[0].contains(label) && violations[0].contains("'shadow-net'"),
                "{}",
                violations[0]
            );
        }
    }

    /// Critic-5 C2: the allowlist admits a (crate, source) pair. The one
    /// non-empty allowlist entry is the instrument's own parser, so admitting
    /// it by name alone hands the parser to whoever controls the source.
    #[test]
    fn an_allowlisted_name_from_a_non_default_source_is_flagged() {
        let boundary = member("crates/gauntlet-arch-tests", "gauntlet-arch-tests");
        for (label, spec) in [
            (
                "git",
                "toml = { git = \"https://github.com/attacker/toml-fork\", branch = \"main\" }",
            ),
            (
                "alternate registry",
                "toml = { version = \"0.9\", registry = \"mirror\" }",
            ),
            (
                "alternate registry index",
                "toml = { version = \"0.9\", registry-index = \"sparse+https://attacker/x\" }",
            ),
        ] {
            let text = format!("[dependencies]\n{spec}\n");
            let violations = check_member(&boundary, &text, &fixture_members(), &no_root_deps());
            assert_eq!(violations.len(), 1, "{label}: {violations:?}");
            assert!(
                violations[0].contains("'toml'") && violations[0].contains("(crate, source) PAIR"),
                "{label}: {}",
                violations[0]
            );
        }

        // Negative: the real spelling — the default registry, inherited from
        // the root table — stays admissible, or the suite could not build.
        let mut root = BTreeMap::new();
        root.insert(
            "toml".to_string(),
            RootDep {
                package: "toml".to_string(),
                source: RootDepSource::Registry,
            },
        );
        assert!(
            check_member(
                &boundary,
                "[dependencies]\ntoml.workspace = true\n",
                &fixture_members(),
                &root,
            )
            .is_empty()
        );
        assert!(
            check_member(
                &boundary,
                "[dependencies]\ntoml = \"0.9\"\n",
                &fixture_members(),
                &no_root_deps(),
            )
            .is_empty()
        );
        // And the allowlist is a per-layer permission: the domain never gets it.
        assert!(
            !check_member(
                &member("crates/gauntlet-domain", "gauntlet-domain"),
                "[dependencies]\ntoml = \"0.9\"\n",
                &fixture_members(),
                &no_root_deps(),
            )
            .is_empty()
        );
    }

    /// The root table and the substitution tables carry the same source rule.
    #[test]
    fn root_table_and_patch_entries_reject_alternate_registry_sources() {
        let root_manifest = "[workspace]\nmembers = []\n\n[workspace.dependencies]\ntoml = { \
                             version = \"0.9\", registry = \"mirror\" }\n";
        let table = parse_table(root_manifest).expect("fixture parses");
        let violations = check_workspace_dependency_table(&table, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("alternate registry"),
            "{}",
            violations[0]
        );

        let patched = "[patch.crates-io]\ntoml = { version = \"0.9\", registry = \"mirror\" }\n";
        let table = parse_table(patched).expect("fixture parses");
        let violations = check_patch_and_replace(&table, &fixture_members());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("alternate-registry source"),
            "{}",
            violations[0]
        );
    }

    /// Critic-6 M1 / critic-7 MJ1: an unreadable `.cargo/config*` and an
    /// unparseable `rust-toolchain*` used to be silent skips — the one
    /// fail-open branch in a channel whose whole posture is I8. Both are
    /// violations now, and the one documented non-TOML form (a bare channel
    /// name in the legacy extensionless file) stays clean.
    #[test]
    fn an_unreadable_or_unparseable_declared_build_input_is_a_violation() {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-buildinput-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();

        // A non-UTF-8 cargo config: is_file() accepts it, read rejects it.
        std::fs::create_dir_all(dir.join(".cargo")).expect("fixture .cargo is creatable");
        std::fs::write(dir.join(".cargo/config.toml"), [0xff_u8, 0xfe, 0x28, 0x5b])
            .expect("non-UTF-8 config is written");
        let violations = check_cargo_config(&dir);
        assert!(
            violations
                .iter()
                .any(|v| v.contains(".cargo/config.toml") && v.contains("not readable")),
            "{violations:?}"
        );

        // A malformed rust-toolchain.toml.
        std::fs::write(dir.join("rust-toolchain.toml"), "[toolchain\nbroken")
            .expect("fixture is written");
        let violations = check_cargo_config(&dir);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rust-toolchain.toml") && v.contains("unparseable")),
            "{violations:?}"
        );

        // A non-UTF-8 legacy rust-toolchain.
        std::fs::remove_file(dir.join("rust-toolchain.toml")).expect("fixture is removable");
        std::fs::write(dir.join("rust-toolchain"), [0xff_u8, 0xfe, 0x01]).expect("written");
        let violations = check_cargo_config(&dir);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rust-toolchain") && v.contains("not readable")),
            "{violations:?}"
        );

        // The legacy file carrying something that is neither TOML nor a
        // channel name.
        std::fs::write(dir.join("rust-toolchain"), "??? [[[\n").expect("written");
        let violations = check_cargo_config(&dir);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rust-toolchain") && v.contains("unparseable")),
            "{violations:?}"
        );

        // Negative: the documented non-TOML form — a bare channel name — and
        // a pinned channel in the TOML form stay clean.
        std::fs::write(dir.join("rust-toolchain"), "stable\n").expect("written");
        let toolchain_only = check_toolchain_file(&dir);
        assert!(toolchain_only.is_empty(), "{toolchain_only:?}");
        std::fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.92\"\n",
        )
        .expect("written");
        std::fs::remove_file(dir.join("rust-toolchain")).expect("fixture is removable");
        assert!(
            check_toolchain_file(&dir).is_empty(),
            "{:?}",
            check_toolchain_file(&dir)
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // ------------------------------------------------------------------
    // F2 (Task 11 correction round 2): feature specifiers on internal
    // edges are visible, and the composition seal admits exactly one edge.
    // ------------------------------------------------------------------

    #[test]
    fn feature_specifiers_on_internal_edges_are_visible_in_both_spellings() {
        // The walker used to ignore the `features` key entirely (F2's
        // finding). Array and bare-string spellings, target-scoped labels,
        // and renamed packages must all surface.
        let members = fixture_members();
        let text = r#"
[dependencies]
gauntlet-app = { workspace = true, features = ["composition", "unrelated"] }

[target.'cfg(unix)'.dev-dependencies]
core = { package = "gauntlet-domain", features = "serde" }
"#;
        let edges = member_feature_edges("crates/gauntlet-cli", text, &members, &no_root_deps())
            .expect("fixture parses");
        let mut seen: Vec<(String, String, String)> = edges
            .iter()
            .map(|edge| (edge.section.clone(), edge.dep.clone(), edge.feature.clone()))
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            vec![
                (
                    "dependencies".to_owned(),
                    "gauntlet-app".to_owned(),
                    "composition".to_owned()
                ),
                (
                    "dependencies".to_owned(),
                    "gauntlet-app".to_owned(),
                    "unrelated".to_owned()
                ),
                (
                    "target.cfg(unix).dev-dependencies".to_owned(),
                    "gauntlet-domain".to_owned(),
                    "serde".to_owned()
                ),
            ],
            "{edges:?}"
        );
        // External deps carry no internal feature edges.
        let external = member_feature_edges(
            "crates/gauntlet-domain",
            "[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\n",
            &members,
            &no_root_deps(),
        )
        .expect("fixture parses");
        assert!(external.is_empty(), "{external:?}");
    }

    #[test]
    fn the_composition_seal_accepts_only_the_recorded_cli_edges() {
        let members = fixture_members();
        let root_deps = no_root_deps();
        let cli = "crates/gauntlet-cli/Cargo.toml";

        // The legal edges: gauntlet-cli's unscoped dev-dependency (Task 11)
        // and, since plan Task 20, its unscoped production dependency — the
        // run composition is the recorded production caller of the sealed
        // rung constructors.
        let legal = "[dev-dependencies]\ngauntlet-app = { workspace = true, features = \
                     [\"composition\"] }\n";
        let edges = member_feature_edges("crates/gauntlet-cli", legal, &members, &root_deps)
            .expect("parses");
        assert!(
            composition_seal_violations(cli, Some("gauntlet-cli"), &edges).is_empty(),
            "{edges:?}"
        );
        let legal_production = "[dependencies]\ngauntlet-app = { workspace = true, features = \
             [\"composition\"] }\n";
        let edges = member_feature_edges(
            "crates/gauntlet-cli",
            legal_production,
            &members,
            &root_deps,
        )
        .expect("parses");
        assert!(
            composition_seal_violations(cli, Some("gauntlet-cli"), &edges).is_empty(),
            "the plan Task 20 production composition edge is a recorded seam: {edges:?}"
        );

        // FAIL-FIRST SHAPE (demonstrated against the live repo before the
        // pin landed, and re-demonstrated when plan Task 20 widened the
        // seal to admit the cli production edge): the SAME production edge
        // on any OTHER member — the exact regression the seal exists to
        // catch.
        let production =
            "[dependencies]\ngauntlet-app = { workspace = true, features = [\"composition\"] }\n";
        let edges = member_feature_edges(
            "crates/adapters/gauntlet-adapter-agent-mock",
            production,
            &members,
            &root_deps,
        )
        .expect("parses");
        let violations = composition_seal_violations(
            "crates/adapters/gauntlet-adapter-agent-mock/Cargo.toml",
            Some("gauntlet-adapter-agent-mock"),
            &edges,
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("[dependencies]") && violations[0].contains("composition-seal"),
            "{}",
            violations[0]
        );

        // The bare-string spelling is equally caught (on the one member the
        // seal does not admit a production edge for).
        let string_form = "[dependencies]\ngauntlet-app = { workspace = true, features = \
                           \"composition\" }\n";
        let edges = member_feature_edges(
            "crates/adapters/gauntlet-adapter-agent-mock",
            string_form,
            &members,
            &root_deps,
        )
        .expect("parses");
        assert_eq!(
            composition_seal_violations(
                "crates/adapters/gauntlet-adapter-agent-mock/Cargo.toml",
                Some("gauntlet-adapter-agent-mock"),
                &edges
            )
            .len(),
            1,
            "{edges:?}"
        );

        // Any OTHER member's dev edge — even a dev edge — is refused.
        let foreign = "[dev-dependencies]\ngauntlet-app = { workspace = true, features = \
                       [\"composition\"] }\n";
        let edges = member_feature_edges(
            "crates/adapters/gauntlet-adapter-agent-mock",
            foreign,
            &members,
            &root_deps,
        )
        .expect("parses");
        assert_eq!(
            composition_seal_violations(
                "crates/adapters/gauntlet-adapter-agent-mock/Cargo.toml",
                Some("gauntlet-adapter-agent-mock"),
                &edges
            )
            .len(),
            1,
            "{edges:?}"
        );

        // A target-scoped dev edge on the cli itself: dev-only under a
        // cfg, still not the one recorded seam spelling.
        let scoped = "[target.'cfg(unix)'.dev-dependencies]\ngauntlet-app = { workspace = true, \
                      features = [\"composition\"] }\n";
        let edges = member_feature_edges("crates/gauntlet-cli", scoped, &members, &root_deps)
            .expect("parses");
        assert_eq!(
            composition_seal_violations(cli, Some("gauntlet-cli"), &edges).len(),
            1,
            "{edges:?}"
        );

        // The root table: inheritable by EVERY member — never legal.
        let root_manifest = "[workspace.dependencies]\ngauntlet-app = { path = \
                             \"crates/gauntlet-app\", features = [\"composition\"] }\n";
        let table = parse_table(root_manifest).expect("parses");
        let edges = root_feature_edges(&table, &members);
        assert_eq!(edges.len(), 1, "{edges:?}");
        let violations = composition_seal_violations("Cargo.toml", None, &edges);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("root [workspace.dependencies] table"),
            "{}",
            violations[0]
        );

        // Unrelated features on internal edges stay clean: the seal is one
        // seam, not a feature-policy language.
        let unrelated =
            "[dependencies]\ngauntlet-domain = { workspace = true, features = [\"serde\"] }\n";
        let edges = member_feature_edges("crates/gauntlet-cli", unrelated, &members, &root_deps)
            .expect("parses");
        assert!(
            composition_seal_violations(cli, Some("gauntlet-cli"), &edges).is_empty(),
            "{edges:?}"
        );
    }
}
