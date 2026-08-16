//! (b) Source denylist — spec §24.3: "a denylist test asserts that provider,
//! harness, database, browser, and sandbox identifiers appear only under
//! `crates/adapters/` and configuration files."
//!
//! # Two term sets, two scopes (recritique R2/R12/R20/R34, class B2)
//!
//! The curated [`DENIED_TERMS`] list names *products*. On its own it never
//! enforced AK-002/AK-014, because ten of the eleven adapter crate names
//! contain no denied word: `"agent-mock"`, `"prime-agent"`, `"cli-agent"` all
//! passed. [`Denylist::derive`] therefore UNIONS the curated list with every
//! adapter identifier discovered in the workspace — full crate names, their
//! `gauntlet-adapter-` suffixes, the reversed segment order the spec §4.1
//! diagram uses (`fs-evidence` for `evidence-fs`), and the `_` spellings of
//! all of them.
//!
//! The two sets have deliberately different scopes, and this is the decision
//! the artifact has to make on purpose rather than under pressure later:
//!
//! | term set | scope | why |
//! |---|---|---|
//! | vendor ([`DENIED_TERMS`]) | all four core crates | spec §24.3: product identifiers belong under `adapters/` and configuration |
//! | adapter identifiers (derived) | domain, ports, application | AK-002/AK-014 and §24.3's second bullet say *"domain or application source"* |
//!
//! `gauntlet-cli` is the composition root (spec §4.1: "cli wires everything";
//! `deps` already permits it to depend on every adapter). If the adapter-name
//! rule applied there, the manifest edit Task 20 *requires* —
//! `gauntlet-adapter-state-sqlite.workspace = true` — would fail this gate,
//! and the cheapest fix would be to weaken or delete the rule. Instead the
//! composition root is scanned with [`Role::CompositionRoot`], and the carve-
//! out is a **mask, not a skip** (round-3 V5/V14): the adapter crate names are
//! replaced with a space and the REMAINDER of the line is scanned normally.
//!
//! That distinction is the whole point. The previous implementation skipped
//! the entire scan unit whenever it mentioned an adapter crate, so
//! `rusqlite::Connection::open(p)?; // gauntlet_adapter_state_sqlite`
//! laundered a bare vendor call with a trailing comment — and repeating the
//! comment per line laundered the file. With masking, the crate name
//! disappears and `rusqlite` is still reported.
//!
//! A consequence worth stating, because it constrains Task 20: only the crate
//! NAME is masked, so an adapter must export capability-named items. `use
//! gauntlet_adapter_state_sqlite::StateStore;` is legal in the composition
//! root; `use gauntlet_adapter_state_sqlite::SqliteStore;` is not, because
//! after masking the bare vendor substring `sqlite` remains. That matches spec
//! §8.4 ("the runtime names capabilities, never products") and costs the
//! composition root nothing.
//!
//! # Matching
//!
//! Source text and manifest keys use the SAME case-insensitive substring
//! matcher (recritique R24, class B1): the older word-boundary matcher let
//! `libsqlite3-sys`, `anthropic_sdk`, and `openai_api` through as dependency
//! keys. Manifests are additionally scanned as raw text, so a vendor name in
//! `[features]` or `[package.metadata]` is caught too (recritique R33), and
//! `scan_manifest` resolves `workspace = true` through the root
//! `[workspace.dependencies]` table, so a workspace-inherited rename
//! (`store = { package = "rusqlite" }` + `store.workspace = true`) reports the
//! package it actually compiles (round-3 V4).
//!
//! Matching runs over `crate::text::scan_units`, so a term split by a Rust
//! line-continuation, by `concat!("sq", "lite")`, by a `\x73` escape, or by a
//! rustfmt statement break is still found (recritique R6, round-3 V6/V11/V17).
//!
//! # Recorded residual: runtime assembly (critic-4 m2)
//!
//! Normalization closes the *splitting* family; it cannot close deliberate
//! runtime assembly. `["mo", "ck"].join("")` and `String::from("rusq") +
//! "lite"` contain no whole literal and no contiguous substring, so no text
//! scanner sees the assembled vendor name — only a parser with dataflow
//! could. The module's own sources read as if normalization closed the whole
//! evasion family; it does not, and that is now written down (the literal
//! scanner carries the same note).

use std::collections::BTreeMap;

use crate::deps::Member;

// The denylist intentionally has NO defaults-module or config carve-out:
// the spec §8.6 defaults table contains no vendor identifiers, and any
// future legitimate need is a deliberate, recorded boundary-test change —
// not a silent edit here.

/// Identifiers that name products rather than capabilities. In every scanned
/// text the match is a case-insensitive substring (compound identifiers
/// included). If a term starts producing false positives on legitimate domain
/// vocabulary, narrow the scope of the match to imports and string literals
/// (plan Task 2 redesign trigger) — do not delete the term.
///
/// Deliberately absent, because substring matching makes them false-positive
/// factories in ordinary prose: `phi` (`graphics`), `cursor` (iteration),
/// `vertex` (graph vocabulary — the product is spelled `vertex-ai` here).
pub const DENIED_TERMS: &[&str] = &[
    // databases and persistence products
    "sqlite",
    "rusqlite",
    "libsql",
    "sqlx",
    "duckdb",
    "sled",
    "diesel",
    "postgres",
    "mysql",
    "redis",
    "mongodb",
    // http clients and servers
    "reqwest",
    "ureq",
    "hyper",
    // containerization and sandbox products (spec §8.2 backend enum, §4.1)
    "docker",
    "podman",
    "bwrap",
    "bubblewrap",
    "firejail",
    "seatbelt",
    "microvm",
    "mxc",
    "nsjail",
    "gvisor",
    // model providers and harness vendors
    "claude",
    "anthropic",
    "openai",
    "chatgpt",
    "gemini",
    "copilot",
    "deepseek",
    "mistral",
    "qwen",
    "llama",
    "grok",
    "groq",
    "moonshot",
    "cohere",
    "openrouter",
    "ollama",
    "bedrock",
    "vertex-ai",
    "vertex_ai",
    "vertexai",
    // browser engines and automation products
    "firefox",
    "chromium",
    "webkit",
    "gecko",
    "playwright",
    "selenium",
    "puppeteer",
    "chrome",
    "safari",
    // telemetry products (the capability is "log sink", not the product)
    "openobserve",
];

/// Which rule set applies to a file's crate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Domain, ports, application: vendor terms AND adapter identifiers.
    Inner,
    /// `gauntlet-cli`: vendor terms only, with adapter identifiers masked out
    /// first. See the module docs for why.
    CompositionRoot,
}

/// The effective denylist for a workspace: the curated vendor list plus the
/// adapter identifiers derived from the members.
#[derive(Clone, Debug, Default)]
pub struct Denylist {
    pub vendor: Vec<String>,
    /// Adapter identifiers, longest first so masking is greedy.
    pub adapter: Vec<String>,
    /// Full adapter crate names (both spellings), longest first. These are
    /// what the composition root is allowed to name, and what is masked out
    /// of its text before the vendor scan.
    pub adapter_crate_names: Vec<String>,
    /// The distinguishing (last) segment of each adapter suffix — `prime`,
    /// `mock`, `cli`, `sqlite`, `fs`, `git`, `diff`, … These are matched as
    /// WHOLE string literals only, never as substrings, because as substrings
    /// they collide head-on with ordinary domain vocabulary. See
    /// [`Denylist::segment_literal`].
    pub adapter_segments: Vec<String>,
}

impl Denylist {
    /// Derive the effective denylist from the workspace members.
    pub fn derive(members: &[Member]) -> Self {
        let mut adapter: Vec<String> = Vec::new();
        let mut crate_names: Vec<String> = Vec::new();
        let mut segments: Vec<String> = Vec::new();
        for member in members
            .iter()
            .filter(|m| m.dir.starts_with("crates/adapters/"))
        {
            let name = member.name.to_lowercase();
            for spelling in [name.clone(), name.replace('-', "_")] {
                push_unique(&mut crate_names, spelling.clone());
                push_unique(&mut adapter, spelling);
            }
            let Some(suffix) = name.strip_prefix("gauntlet-adapter-") else {
                continue;
            };
            push_unique(&mut adapter, suffix.to_string());
            push_unique(&mut adapter, suffix.replace('-', "_"));
            // The §4.1 diagram names the same adapters with their segments
            // the other way round (`prime-agent`, `fs-evidence`,
            // `mock-agent`), and that is exactly the spelling a match arm
            // would use.
            let reversed: Vec<&str> = suffix.split('-').rev().collect();
            if reversed.len() > 1 {
                let joined = reversed.join("-");
                push_unique(&mut adapter, joined.replace('-', "_"));
                push_unique(&mut adapter, joined);
            }
            // Round-3 V7: `match role { "builder" => "prime", … }` selects an
            // adapter by its shortest distinguishing segment and matched none
            // of the spellings above. The LAST segment is the distinguishing
            // one; the first (`agent`, `state`, `observe`, `compare`,
            // `evidence`) is the capability the spec itself uses, so it is
            // deliberately not derived.
            if let Some(last) = reversed.first() {
                push_unique(&mut segments, (*last).to_string());
            }
        }
        adapter.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        crate_names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        segments.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        Denylist {
            vendor: DENIED_TERMS.iter().map(|t| t.to_string()).collect(),
            adapter,
            adapter_crate_names: crate_names,
            adapter_segments: segments,
        }
    }

    /// The terms that apply to a file in the given role.
    fn terms_for(&self, role: Role) -> Vec<(&str, &'static str)> {
        let mut out: Vec<(&str, &'static str)> = self
            .vendor
            .iter()
            .map(|t| (t.as_str(), "provider/harness/database/browser/sandbox"))
            .collect();
        if role == Role::Inner {
            out.extend(self.adapter.iter().map(|t| (t.as_str(), "adapter")));
        }
        out
    }

    /// Mask every known adapter crate name out of `lowered`, replacing each
    /// with a space, and return the remainder.
    ///
    /// This is the composition root's carve-out, and it is a MASK rather than
    /// a skip on purpose (round-3 V5/V14). In the composition root the crate
    /// name IS the wiring — `gauntlet-adapter-state-sqlite.workspace = true`,
    /// `use gauntlet_adapter_state_sqlite::StateStore;` — so the vendor
    /// substring inside the crate name is not a leak. Everything else on the
    /// line still is: `rusqlite::Connection::open(p)?; //
    /// gauntlet_adapter_state_sqlite` masks to `rusqlite::connection::open(p)?;
    /// // ` and reports `rusqlite`.
    ///
    /// The replacement is a SPACE, not the empty string, so masking can never
    /// splice two halves of a term together (`chro` + name + `mium`).
    pub fn mask_adapter_crate_names(&self, lowered: &str) -> String {
        // `adapter_crate_names` is sorted longest-first, so the greedy
        // spelling is removed before any shorter one it contains.
        self.adapter_crate_names
            .iter()
            .fold(lowered.to_string(), |text, name| {
                text.replace(name.as_str(), " ")
            })
    }

    /// The adapter segment a string literal is EXACTLY equal to, if any.
    ///
    /// Whole-literal equality is the documented narrowing (plan Task 2's
    /// redesign trigger: "narrow to imports and string literals"). As a
    /// substring, `state`/`build`/`test`/`command` are ordinary domain
    /// vocabulary and the class would be a false-positive factory; as a whole
    /// string literal, `"prime"` in `match role { "builder" => "prime" }` is
    /// adapter selection by literal, which is exactly what AK-014 forbids.
    pub fn segment_literal(&self, literal: &str) -> Option<&str> {
        let value = literal.trim().to_lowercase();
        self.adapter_segments
            .iter()
            .find(|segment| **segment == value)
            .map(|s| s.as_str())
    }
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !value.is_empty() && !list.contains(&value) {
        list.push(value);
    }
}

/// Case-insensitive substring occurrence check. Word boundaries are
/// deliberately NOT required: a vendor name embedded in a compound identifier
/// (`SqliteConn`, `sqlite_conn`, `REQWEST_CLIENT`, `libsqlite3-sys`) still
/// couples the core to that vendor.
pub fn contains_term(haystack_lower: &str, term: &str) -> bool {
    haystack_lower.contains(term)
}

/// Scan one text file's content (`.rs`, `.toml`, or any other extension).
/// Reports every (line, term) hit so a fix removes all of them.
pub fn scan_source(path: &str, text: &str, deny: &Denylist, role: Role) -> Vec<String> {
    let terms = deny.terms_for(role);
    let mut out: Vec<String> = Vec::new();
    for unit in crate::text::scan_units(text) {
        let lowered = unit.text.to_lowercase();
        // The composition root's carve-out: mask the adapter crate names out
        // and scan what is left (round-3 V5/V14). Never skip the unit.
        let scanned = match role {
            Role::CompositionRoot => deny.mask_adapter_crate_names(&lowered),
            Role::Inner => lowered,
        };
        for (term, kind) in &terms {
            if contains_term(&scanned, term) {
                let message = format!(
                    "denied-identifier: {path}:{}: '{term}' — {kind} names belong only under \
                     crates/adapters/** (spec §24.3)",
                    unit.line
                );
                if !out.contains(&message) {
                    out.push(message);
                }
            }
        }
        // AK-014: adapter selection by the shortest distinguishing segment,
        // as a whole string literal (round-3 V7). The extraction is the ONE
        // lexing pass (`crate::text::string_literals`) since round 6, so a
        // `'"'` char literal earlier on the line no longer deletes the
        // literals after it from this rule (critic-6 C1).
        if role == Role::Inner {
            for literal in crate::text::string_literals(&unit.text) {
                if let Some(segment) = deny.segment_literal(&literal) {
                    let message = format!(
                        "denied-identifier: {path}:{}: '{segment}' — the string literal \
                         '{literal}' is an adapter's distinguishing segment; adapter selection \
                         resolves from configuration only (AK-014, spec §24.3)",
                        unit.line
                    );
                    if !out.contains(&message) {
                        out.push(message);
                    }
                }
            }
        }
    }
    out
}

/// Scan one `Cargo.toml`'s dependency keys, `package = "..."` rename values,
/// and workspace-inherited aliases (all dependency sections, including
/// target-scoped ones). The match is the same substring matcher source text
/// gets (recritique R24): a word-boundary matcher let `libsqlite3-sys` and
/// `anthropic_sdk` through.
///
/// `workspace_deps` is the root manifest's alias → package map. Without it a
/// workspace-inherited rename compiles a denied vendor crate into a core layer
/// with nothing reported anywhere: the member manifest carries only the alias,
/// and the root manifest — where the real package name lives — is not a core
/// file (round-3 V4).
pub fn scan_manifest(
    path: &str,
    text: &str,
    deny: &Denylist,
    role: Role,
    workspace_deps: &BTreeMap<String, String>,
) -> Vec<String> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return vec![format!(
            "denied-identifier: {path}: malformed Cargo.toml — dependency keys cannot be checked \
             (the dependency-direction check reports the parse error)"
        )];
    };
    let terms = deny.terms_for(role);
    let mut out: Vec<String> = Vec::new();
    for (section, entries) in crate::deps::dependency_sections(&table) {
        for (key, spec) in entries {
            let mut candidates: Vec<(&str, &str)> = vec![("dependency key", key.as_str())];
            let fields = spec.as_table();
            if let Some(renamed) = fields
                .and_then(|fields| fields.get("package"))
                .and_then(|value| value.as_str())
            {
                candidates.push(("renamed package", renamed));
            }
            // `store.workspace = true` inherits whatever the root table says
            // `store` is. Both the dotted spelling and the table spelling are
            // resolved, because both compile the same crate.
            let inherits = fields
                .and_then(|fields| fields.get("workspace"))
                .and_then(|value| value.as_bool())
                == Some(true);
            if inherits && let Some(resolved) = workspace_deps.get(key) {
                candidates.push(("workspace-inherited package", resolved.as_str()));
            }
            for (what, candidate) in candidates {
                let lowered = candidate.to_lowercase();
                let lowered = match role {
                    Role::CompositionRoot => deny.mask_adapter_crate_names(&lowered),
                    Role::Inner => lowered,
                };
                for (term, kind) in &terms {
                    if !contains_term(&lowered, term) {
                        continue;
                    }
                    let line = text
                        .lines()
                        .position(|l| l.to_lowercase().contains(candidate))
                        .map(|i| i + 1);
                    let location = match line {
                        Some(n) => format!("{path}:{n}"),
                        None => path.to_string(),
                    };
                    let message = format!(
                        "denied-identifier: {location}: '{term}' (as {what} '{candidate}' in \
                         [{section}]) — {kind} names belong only under crates/adapters/** \
                         (spec §24.3)"
                    );
                    if !out.contains(&message) {
                        out.push(message);
                    }
                }
            }
        }
    }
    out
}

/// The root manifest's `[workspace.dependencies]` table, as text, with every
/// other line blanked so line numbers stay true.
///
/// Round-3 V4: the root `Cargo.toml` is not a core file, so the string
/// `rusqlite` in `store = { package = "rusqlite" }` lived outside every scan
/// scope. This is the table core crates INHERIT from — the one root table
/// whose contents can compile into `gauntlet-domain` through a single
/// `store.workspace = true`. It is therefore inside the denylist, scanned with
/// [`Role::CompositionRoot`] so the workspace's own adapter path entries mask
/// out. An adapter's vendor dependency belongs in that adapter's own manifest,
/// not in the root table.
pub fn workspace_dependencies_text(root_manifest: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in root_manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == "[workspace.dependencies]";
            out.push('\n');
            continue;
        }
        if inside {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod repo {

    #[test]
    fn core_crates_contain_no_denied_identifiers() {
        let violations = crate::check_denylist(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("denied-identifier", &violations)
        );
    }

    #[test]
    fn adapter_selection_is_config_driven() {
        let violations = crate::check_adapter_selection(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("adapter-selection", &violations)
        );
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    fn adapters() -> Vec<Member> {
        [
            "gauntlet-adapter-agent-mock",
            "gauntlet-adapter-agent-prime",
            "gauntlet-adapter-agent-cli",
            "gauntlet-adapter-state-sqlite",
            "gauntlet-adapter-evidence-fs",
            "gauntlet-adapter-observe-git",
            "gauntlet-adapter-compare-diff",
        ]
        .iter()
        .map(|name| Member {
            dir: format!("crates/adapters/{name}"),
            name: name.to_string(),
        })
        .collect()
    }

    fn deny() -> Denylist {
        Denylist::derive(&adapters())
    }

    fn no_workspace_deps() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn every_denied_term_is_flagged_in_source() {
        let deny = deny();
        for term in DENIED_TERMS {
            let fixture = format!("let binding = \"call {term} now\";\n");
            let violations = scan_source("fixture.rs", &fixture, &deny, Role::Inner);
            assert!(
                violations.iter().any(|v| v.contains(&format!("'{term}'"))),
                "term {term} not flagged: {violations:?}"
            );
            assert!(
                violations[0].contains("fixture.rs:1"),
                "violation must name file and line: {}",
                violations[0]
            );
        }
    }

    /// The term inventory is pinned to its exact contents (critic-4 vacuity
    /// M3): `every_denied_term_is_flagged_in_source` iterates the list, so it
    /// shrinks in lockstep with a deleted term, and the module's "do not
    /// delete the term" instruction was prose, not a gate. Deleting or
    /// silently editing a term now fails this test; widening is a deliberate
    /// edit that updates the pin.
    #[test]
    fn denied_terms_inventory_is_pinned() {
        assert_eq!(
            DENIED_TERMS,
            &[
                // databases and persistence products
                "sqlite",
                "rusqlite",
                "libsql",
                "sqlx",
                "duckdb",
                "sled",
                "diesel",
                "postgres",
                "mysql",
                "redis",
                "mongodb",
                // http clients and servers
                "reqwest",
                "ureq",
                "hyper",
                // containerization and sandbox products (spec §8.2 backend enum, §4.1)
                "docker",
                "podman",
                "bwrap",
                "bubblewrap",
                "firejail",
                "seatbelt",
                "microvm",
                "mxc",
                "nsjail",
                "gvisor",
                // model providers and harness vendors
                "claude",
                "anthropic",
                "openai",
                "chatgpt",
                "gemini",
                "copilot",
                "deepseek",
                "mistral",
                "qwen",
                "llama",
                "grok",
                "groq",
                "moonshot",
                "cohere",
                "openrouter",
                "ollama",
                "bedrock",
                "vertex-ai",
                "vertex_ai",
                "vertexai",
                // browser engines and automation products
                "firefox",
                "chromium",
                "webkit",
                "gecko",
                "playwright",
                "selenium",
                "puppeteer",
                "chrome",
                "safari",
                // telemetry products (the capability is "log sink", not the product)
                "openobserve",
            ]
        );
    }

    #[test]
    fn matching_is_case_insensitive_and_substring_based() {
        let deny = deny();
        let violations = scan_source("f.rs", "let v = RusQLite::open(p)?;\n", &deny, Role::Inner);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations.iter().any(|v| v.contains("'rusqlite'")));
        assert!(violations.iter().any(|v| v.contains("'sqlite'")));

        for compound in ["SqliteConn", "sqlite_conn", "REQWEST_CLIENT"] {
            let violations = scan_source(
                "f.rs",
                &format!("let x = {compound};\n"),
                &deny,
                Role::Inner,
            );
            assert_eq!(violations.len(), 1, "{compound} -> {violations:?}");
        }

        let clean = scan_source(
            "f.rs",
            "let chromatic = chromatically(x);\n",
            &deny,
            Role::Inner,
        );
        assert!(clean.is_empty(), "{clean:?}");
    }

    #[test]
    fn derived_adapter_identifiers_are_flagged_in_inner_layers() {
        // Recritique R2's exact evasion: none of these contains a curated
        // vendor term, and every one of them selects an adapter by literal.
        let deny = deny();
        let fixture = "\
pub fn runner_for(role: &str) -> &'static str {
    match role { \"builder\" => \"prime-agent\", \"critic\" => \"agent-mock\", _ => \"cli-agent\" }
}
pub const OBSERVER: &str = \"gauntlet-adapter-observe-git\";
pub const COMPARE: &str = \"gauntlet-adapter-compare-diff\";
pub const EVIDENCE: &str = \"fs-evidence\";
";
        let violations = scan_source(
            "crates/gauntlet-app/src/lib.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        for needle in [
            "'prime-agent'",
            "'agent-mock'",
            "'cli-agent'",
            "'gauntlet-adapter-observe-git'",
            "'gauntlet-adapter-compare-diff'",
            "'fs-evidence'",
        ] {
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{needle} not flagged: {violations:?}"
            );
        }
        // And the underscore spelling a Rust `use` would produce.
        let uses = scan_source(
            "crates/gauntlet-app/src/lib.rs",
            "use gauntlet_adapter_state_sqlite::SqliteStore;\n",
            &deny,
            Role::Inner,
        );
        assert!(
            uses.iter()
                .any(|v| v.contains("'gauntlet_adapter_state_sqlite'")),
            "{uses:?}"
        );
    }

    #[test]
    fn composition_root_may_name_adapters_but_not_bare_vendors() {
        // Recritique R12/R20/R34: the gate must not make Task 20 unreachable.
        let deny = deny();
        let manifest = "\
[dependencies]
gauntlet-adapter-state-sqlite.workspace = true
gauntlet-adapter-agent-prime.workspace = true
";
        assert!(
            scan_manifest(
                "crates/gauntlet-cli/Cargo.toml",
                manifest,
                &deny,
                Role::CompositionRoot,
                &no_workspace_deps(),
            )
            .is_empty(),
            "the composition root must be able to declare adapter crates"
        );
        assert!(
            scan_source(
                "crates/gauntlet-cli/src/main.rs",
                "use gauntlet_adapter_state_sqlite::StateStore;\n",
                &deny,
                Role::CompositionRoot
            )
            .is_empty(),
            "the composition root must be able to name adapter crates"
        );

        // But it is still a core crate for vendor identifiers.
        let bare = scan_source(
            "crates/gauntlet-cli/src/main.rs",
            "let conn = rusqlite::Connection::open(p)?;\n",
            &deny,
            Role::CompositionRoot,
        );
        assert!(bare.iter().any(|v| v.contains("'rusqlite'")), "{bare:?}");

        // And the same manifest inside the application layer is a violation.
        let inner = scan_manifest(
            "crates/gauntlet-app/Cargo.toml",
            manifest,
            &deny,
            Role::Inner,
            &no_workspace_deps(),
        );
        assert!(
            !inner.is_empty(),
            "adapter deps are illegal in the app layer"
        );
    }

    #[test]
    fn the_composition_root_carve_out_masks_rather_than_skips() {
        // Round-3 V5/V14's exact evasion: a trailing comment naming an
        // adapter crate used to launder the whole line, and repeating it per
        // line laundered the whole file.
        let deny = deny();
        for fixture in [
            "let conn = rusqlite::Connection::open(p)?; // gauntlet_adapter_state_sqlite\n",
            "let conn = rusqlite::Connection::open(cfg.path)?; // gauntlet-adapter-state-sqlite\n",
            "let img = docker_run(spec); // gauntlet-adapter-agent-cli\n",
            "let c = chromium::launch(); // gauntlet-adapter-observe-git\n",
        ] {
            let violations = scan_source(
                "crates/gauntlet-cli/src/main.rs",
                fixture,
                &deny,
                Role::CompositionRoot,
            );
            assert!(
                !violations.is_empty(),
                "the adapter comment laundered the line: {fixture} -> {violations:?}"
            );
        }

        // The manifest form: a non-dependency table laundered by a comment.
        let metadata = "[package.metadata.gauntlet]\nstate_backend = \"sqlite\"  # \
                        gauntlet-adapter-state-sqlite\n";
        let violations = scan_source(
            "crates/gauntlet-cli/Cargo.toml",
            metadata,
            &deny,
            Role::CompositionRoot,
        );
        assert!(
            violations.iter().any(|v| v.contains("'sqlite'")),
            "{violations:?}"
        );

        // Masking must not splice a term together out of two halves.
        assert!(
            deny.mask_adapter_crate_names("chrogauntlet-adapter-agent-climium")
                .contains(' ')
        );
        assert!(
            scan_source(
                "crates/gauntlet-cli/src/main.rs",
                "let x = chrogauntlet-adapter-agent-climium;\n",
                &deny,
                Role::CompositionRoot,
            )
            .is_empty(),
            "masking with a space must not create 'chromium' out of two halves"
        );
    }

    #[test]
    fn workspace_inherited_renames_are_flagged_in_a_core_manifest() {
        // Round-3 V4's exact evasion: the alias carries no denied term, and
        // the root table where `rusqlite` is spelled out is not a core file.
        let deny = deny();
        let root = "\
[workspace.dependencies]
store = { package = \"rusqlite\", version = \"0.32\" }
";
        let workspace_deps = crate::deps::workspace_dep_names(
            &root.parse::<toml::Table>().expect("fixture root parses"),
        );
        let member = "[dependencies]\nstore.workspace = true\n";
        let violations = scan_manifest(
            "crates/gauntlet-domain/Cargo.toml",
            member,
            &deny,
            Role::Inner,
            &workspace_deps,
        );
        assert!(
            violations.iter().any(|v| v.contains("'rusqlite'")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("workspace-inherited package")),
            "{violations:?}"
        );

        // The table spelling inherits identically.
        let table_form = "[dependencies]\nstore = { workspace = true }\n";
        let violations = scan_manifest(
            "crates/gauntlet-domain/Cargo.toml",
            table_form,
            &deny,
            Role::Inner,
            &workspace_deps,
        );
        assert!(
            violations.iter().any(|v| v.contains("'rusqlite'")),
            "{violations:?}"
        );

        // And the root table itself is now text-scanned, at its true line.
        let text = workspace_dependencies_text(root);
        let violations = scan_source("Cargo.toml", &text, &deny, Role::CompositionRoot);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("Cargo.toml:2") && v.contains("'rusqlite'")),
            "{violations:?}"
        );
    }

    #[test]
    fn workspace_dependencies_text_keeps_only_that_table() {
        let root = "\
[workspace]
members = [\"crates/adapters/gauntlet-adapter-state-sqlite\"]

[workspace.dependencies]
store = { package = \"rusqlite\" }

[profile.dev]
debug = \"line-tables-only\"
";
        let text = workspace_dependencies_text(root);
        assert!(text.contains("rusqlite"), "{text:?}");
        assert!(!text.contains("line-tables-only"), "{text:?}");
        assert!(!text.contains("members"), "{text:?}");
        assert_eq!(text.lines().count(), root.lines().count());
    }

    #[test]
    fn adapter_segments_are_flagged_as_whole_string_literals_only() {
        // Round-3 V7's exact evasion: selection by the shortest
        // distinguishing segment matched none of the derived spellings.
        let deny = deny();
        let fixture = "\
pub fn runner_for(role: &str) -> &'static str {
    match role { \"builder\" => \"prime\", \"critic\" => \"mock\", _ => \"cli\" }
}
";
        let violations = scan_source(
            "crates/gauntlet-app/src/lib.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        for needle in ["'prime'", "'mock'", "'cli'"] {
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{needle} not flagged: {violations:?}"
            );
        }

        // The documented narrowing: the same words as ordinary identifiers,
        // prose, or substrings are NOT flagged, so the class does not become a
        // false-positive factory on legitimate vocabulary.
        for clean in [
            "let primary = primary_runner(role);\n",
            "//! The mock is described in the spec's testing section.\n",
            "pub fn clip(v: u8) -> u8 { v }\n",
            "let s = \"primer\";\n",
        ] {
            let violations =
                scan_source("crates/gauntlet-app/src/lib.rs", clean, &deny, Role::Inner);
            assert!(violations.is_empty(), "{clean} -> {violations:?}");
        }

        // Capability segments the spec itself uses are deliberately NOT
        // derived: `agent`, `state`, `observe`, `compare`, `evidence`.
        for capability in ["agent", "state", "observe", "compare", "evidence"] {
            assert!(
                !deny.adapter_segments.iter().any(|s| s == capability),
                "'{capability}' is spec vocabulary and must not be a segment term"
            );
        }
    }

    #[test]
    fn manifest_keys_use_the_substring_matcher() {
        // Recritique R24: a word-boundary matcher let all three through.
        let deny = deny();
        for (key, term) in [
            ("libsqlite3-sys", "sqlite"),
            ("anthropic_sdk", "anthropic"),
            ("openai_api", "openai"),
        ] {
            let fixture = format!("[dependencies]\n{key} = \"1\"\n");
            let violations = scan_manifest(
                "crates/gauntlet-domain/Cargo.toml",
                &fixture,
                &deny,
                Role::Inner,
                &no_workspace_deps(),
            );
            assert!(
                violations.iter().any(|v| v.contains(&format!("'{term}'"))),
                "{key} not flagged: {violations:?}"
            );
        }
    }

    #[test]
    fn line_continuations_and_concat_fragments_are_flagged() {
        // Recritique R6's exact evasion.
        let deny = deny();
        let split = "pub const MODEL: &str = \"claud\\\n    e-3-5-sonnet\";\n";
        let violations = scan_source("f.rs", split, &deny, Role::Inner);
        assert!(
            violations.iter().any(|v| v.contains("'claude'")),
            "{violations:?}"
        );
        assert!(violations[0].contains("f.rs:1"), "{violations:?}");

        let concatenated = "pub const STORE: &str = concat!(\"sq\", \"lite\");\n";
        let violations = scan_source("f.rs", concatenated, &deny, Role::Inner);
        assert!(
            violations.iter().any(|v| v.contains("'sqlite'")),
            "{violations:?}"
        );
    }

    #[test]
    fn spec_named_backends_are_flagged() {
        // Recritique R8: sandbox and database identifiers the spec prints.
        let deny = deny();
        for term in ["seatbelt", "mxc", "microvm", "duckdb", "libsql", "gvisor"] {
            let fixture = format!("pub const BACKEND: &str = \"{term}\";\n");
            let violations = scan_source("f.rs", &fixture, &deny, Role::Inner);
            assert!(
                violations.iter().any(|v| v.contains(&format!("'{term}'"))),
                "{term} not flagged: {violations:?}"
            );
        }
    }

    #[test]
    fn clean_source_produces_no_flags() {
        let fixture = "\
//! Documentation mentioning capability names only.
pub fn settle(residual: &Residual, evidence: &EvidenceRef) -> Outcome {
    verify(residual).collapse_to(evidence) // terminology from the spec
}
";
        assert!(scan_source("clean.rs", fixture, &deny(), Role::Inner).is_empty());
    }

    #[test]
    fn non_rust_text_files_are_scanned_like_source() {
        let deny = deny();
        let txt = scan_source(
            "fixture/notes.txt",
            "remember to rotate the redis token\n",
            &deny,
            Role::Inner,
        );
        assert_eq!(txt.len(), 1, "{txt:?}");
        assert!(txt[0].contains("fixture/notes.txt:1"), "{}", txt[0]);

        let json = scan_source(
            "fixture/wiring.json",
            "{\"store\": \"sqlite\"}\n",
            &deny,
            Role::Inner,
        );
        // Both the vendor term and the whole-literal adapter segment fire.
        assert_eq!(json.len(), 2, "{json:?}");
        assert!(
            json.iter().all(|v| v.contains("fixture/wiring.json:1")),
            "{json:?}"
        );
    }

    #[test]
    fn manifest_keys_and_renames_are_flagged() {
        let fixture = "\
[dependencies]
rusqlite = \"0.3\"

[dev-dependencies]
storage = { package = \"sqlite-tools\", version = \"1\" }
";
        let violations = scan_manifest(
            "f/Cargo.toml",
            fixture,
            &deny(),
            Role::Inner,
            &no_workspace_deps(),
        );
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|v| v.contains("f/Cargo.toml:2") && v.contains("'rusqlite'")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("f/Cargo.toml:5") && v.contains("'sqlite'")),
            "{violations:?}"
        );
    }

    /// Critic-6 C1, denylist side: the `'"'` char literal in
    /// `trim_matches('"')` used to shift the quote pairing of the whole unit,
    /// so the AK-014 whole-literal segment rule — the ONLY thing that catches
    /// bare `"mock"`/`"prime"` — saw garbage instead of the literals. The
    /// extraction is the one lexing pass now.
    #[test]
    fn adapter_segments_survive_a_char_literal_quote() {
        let deny = deny();
        let fixture = "/// Resolve the agent runner. `spec` may arrive quoted from the CLI.\n\
                       pub fn agent_for(spec: &str) -> &'static str {\n    match \
                       spec.trim_matches('\"') { \"fast\" => \"mock\", _ => \"prime\" }\n}\n";
        let violations = scan_source(
            "crates/gauntlet-app/src/lib.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        assert!(
            violations.iter().any(|v| v.contains("'mock'")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("'prime'")),
            "{violations:?}"
        );
    }

    #[test]
    fn clean_manifest_produces_no_flags() {
        let fixture = "\
[dependencies]
gauntlet-domain.workspace = true
serde_json = { workspace = true }
";
        assert!(
            scan_manifest(
                "f/Cargo.toml",
                fixture,
                &deny(),
                Role::Inner,
                &no_workspace_deps()
            )
            .is_empty()
        );
    }

    #[test]
    fn malformed_manifest_is_flagged_not_swallowed() {
        let violations = scan_manifest(
            "f/Cargo.toml",
            "[dependencies",
            &deny(),
            Role::Inner,
            &no_workspace_deps(),
        );
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("malformed"));
    }

    #[test]
    fn manifest_tables_outside_dependency_sections_are_scanned_as_text() {
        // Recritique R33: [features] and [package.metadata] were unscanned.
        // The repo check runs scan_source over manifests as well, which is
        // what catches these.
        let deny = deny();
        let features = "[features]\ndefault = [\"sqlite\"]\nsqlite = []\n";
        let violations = scan_source(
            "crates/gauntlet-app/Cargo.toml",
            features,
            &deny,
            Role::Inner,
        );
        assert!(
            violations.iter().any(|v| v.contains("'sqlite'")),
            "{violations:?}"
        );

        let metadata = "[package.metadata.gauntlet]\nstate_backend = \"sqlite\"\n";
        let violations = scan_source(
            "crates/gauntlet-app/Cargo.toml",
            metadata,
            &deny,
            Role::Inner,
        );
        assert!(
            violations.iter().any(|v| v.contains("'sqlite'")),
            "{violations:?}"
        );
    }
}
