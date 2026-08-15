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
//! composition root is scanned with [`Role::CompositionRoot`]: a line that
//! NAMES a known adapter crate is the wiring itself and is exempt from the
//! vendor terms it necessarily carries; every other line is scanned normally.
//! So `use gauntlet_adapter_state_sqlite::SqliteStore;` in `main.rs` is legal
//! there, while a bare `rusqlite::Connection` is not. The cli also
//! stays fully inside the hardcoded-literal scan (`crate::literals`): no
//! model id, URL, absolute path, port, command, or timeout may be compiled
//! into it either.
//!
//! # Matching
//!
//! Source text and manifest keys use the SAME case-insensitive substring
//! matcher (recritique R24, class B1): the older word-boundary matcher let
//! `libsqlite3-sys`, `anthropic_sdk`, and `openai_api` through as dependency
//! keys. Manifests are additionally scanned as raw text, so a vendor name in
//! `[features]` or `[package.metadata]` is caught too (recritique R33).
//!
//! Matching runs over `crate::text::scan_units`, so a term split by a Rust
//! line-continuation or by `concat!("sq", "lite")` is still found
//! (recritique R6).

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
}

impl Denylist {
    /// Derive the effective denylist from the workspace members.
    pub fn derive(members: &[Member]) -> Self {
        let mut adapter: Vec<String> = Vec::new();
        let mut crate_names: Vec<String> = Vec::new();
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
        }
        adapter.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        crate_names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        Denylist {
            vendor: DENIED_TERMS.iter().map(|t| t.to_string()).collect(),
            adapter,
            adapter_crate_names: crate_names,
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

    /// Whether `lowered` names a known adapter crate. In the composition root
    /// such a line IS the wiring — `gauntlet-adapter-state-sqlite.workspace =
    /// true`, `use gauntlet_adapter_state_sqlite::SqliteStore;` — and the
    /// vendor substrings it necessarily carries (in the crate name and in the
    /// adapter's own exported type names) are not a leak. Every OTHER line in
    /// the composition root is scanned normally, so a bare
    /// `rusqlite::Connection` there still fails.
    pub fn names_an_adapter_crate(&self, lowered: &str) -> bool {
        self.adapter_crate_names
            .iter()
            .any(|name| lowered.contains(name.as_str()))
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
        if role == Role::CompositionRoot && deny.names_an_adapter_crate(&lowered) {
            continue;
        }
        for (term, kind) in &terms {
            if contains_term(&lowered, term) {
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
    }
    out
}

/// Scan one `Cargo.toml`'s dependency keys and `package = "..."` rename
/// values (all dependency sections, including target-scoped ones). The match
/// is the same substring matcher source text gets (recritique R24): a
/// word-boundary matcher let `libsqlite3-sys` and `anthropic_sdk` through.
pub fn scan_manifest(path: &str, text: &str, deny: &Denylist, role: Role) -> Vec<String> {
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
            if let Some(renamed) = spec
                .as_table()
                .and_then(|fields| fields.get("package"))
                .and_then(|value| value.as_str())
            {
                candidates.push(("renamed package", renamed));
            }
            for (what, candidate) in candidates {
                let lowered = candidate.to_lowercase();
                if role == Role::CompositionRoot && deny.names_an_adapter_crate(&lowered) {
                    continue;
                }
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
                Role::CompositionRoot
            )
            .is_empty(),
            "the composition root must be able to declare adapter crates"
        );
        assert!(
            scan_source(
                "crates/gauntlet-cli/src/main.rs",
                "use gauntlet_adapter_state_sqlite::SqliteStore;\n",
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
        );
        assert!(
            !inner.is_empty(),
            "adapter deps are illegal in the app layer"
        );
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
        assert_eq!(json.len(), 1, "{json:?}");
        assert!(json[0].contains("fixture/wiring.json:1"), "{}", json[0]);
    }

    #[test]
    fn manifest_keys_and_renames_are_flagged() {
        let fixture = "\
[dependencies]
rusqlite = \"0.3\"

[dev-dependencies]
storage = { package = \"sqlite-tools\", version = \"1\" }
";
        let violations = scan_manifest("f/Cargo.toml", fixture, &deny(), Role::Inner);
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

    #[test]
    fn clean_manifest_produces_no_flags() {
        let fixture = "\
[dependencies]
gauntlet-domain.workspace = true
serde_json = { workspace = true }
";
        assert!(scan_manifest("f/Cargo.toml", fixture, &deny(), Role::Inner).is_empty());
    }

    #[test]
    fn malformed_manifest_is_flagged_not_swallowed() {
        let violations = scan_manifest("f/Cargo.toml", "[dependencies", &deny(), Role::Inner);
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
