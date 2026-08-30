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
use std::path::{Path, PathBuf};

use crate::deps::Member;

// The denylist intentionally has NO defaults-module or config carve-out:
// the spec §8.6 defaults table contains no vendor identifiers, and any
// future legitimate need is a deliberate, recorded boundary-test change —
// not a silent edit here.

/// Adapter LAST-name segments that are the spec's own field vocabulary and
/// therefore NOT distinguishing selection literals (plan Task 10, first
/// entry `model`: spec §5.3.5 `model:`, §6.1 `model_hint`, §8.2
/// `runners.by_role.<role>.model`). The full crate names, the hyphenated
/// and underscore suffix spellings (`compare-model`, `compare_model`),
/// and the reversed-segment spellings remain denied — only the bare
/// generic word stops being a segment term, exactly as the capability
/// first segments (`agent`, `state`, `observe`, `compare`, `evidence`)
/// already are. Pinned by test: growth is a deliberate edit that updates
/// the pin.
pub const SPEC_VOCABULARY_SEGMENTS: &[&str] = &["model", "digest", "check", "none"];

// Plan Task 14 extended the same non-distinguishing ruling to `none` (last
// segment of `gauntlet-adapter-sandbox-none`): `none` is the spec's own
// vocabulary three times over — §6.1's `isolation: none` wire token (the
// `Isolation::None` parse arm), §8.2's `sandbox.backend` enum, and §8.6's
// declared default value (the closed Task-5 defaults table spells it
// literally). It cannot SELECT an adapter on its own any more than `model`
// can: the distinguishing spellings `sandbox-none`, `none-sandbox`, and
// the full crate name ARE still derived and denied, and no configuration
// resolves a backend by the bare word (the backend token selects, and the
// token IS `none` — which is exactly why the word is field vocabulary:
// §8.2's enum, not a product name). Deriving the bare segment would make
// the AK-014 class a false-positive factory on the spec's own default.

// Plan Task 11 extended the same non-distinguishing ruling to two more
// LAST segments, by the same argument the Round-3 V7 note makes for
// "model": `digest` (last segment of `gauntlet-adapter-verify-file-digest`)
// is the spec's own FIELD vocabulary — the `Digest` domain type (§5.2) and
// the `reference_digest`/`observation_digest`/`context_envelope_digest`
// fields (§9.4, §12.4, §13.2) — and `check` (last segment of
// `gauntlet-adapter-verify-schema-check`) is §14.2's own rung name
// ("file/digest check", "schema check") and §8.5's doctor-check vocabulary.
// Both landed as last segments of Task-11 verifier adapters while closed
// earlier tasks already spelled those words as ordinary domain literals;
// neither word can SELECT an adapter on its own (the distinguishing
// spellings `verify-file-digest`, `digest-file-verify`,
// `verify-schema-check`, `check-schema-verify` — and the full crate names —
// ARE still derived and denied; a bare `digest-file`/`check-schema` literal
// contains no derived term and passes, which is why the PIN test asserts the
// full derived spellings), so deriving the bare segment would only make the
// AK-014 class a false-positive factory on spec vocabulary.

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
            // deliberately not derived. Plan Task 10 extended the same
            // non-distinguishing ruling to a LAST segment that is the spec's
            // own FIELD vocabulary: `model` (spec §5.3.5 `model:`, §6.1
            // `model_hint`, §8.2 `runners.by_role.<role>.model`) landed as
            // the last segment of `gauntlet-adapter-compare-model` while the
            // closed Task-5 config code already spelled the settings key
            // `"model"` ten times — a word that generic is not a
            // distinguishing selection literal (the `compare-model`,
            // `compare_model`, and full crate-name spellings ARE still
            // derived and denied), so deriving it would only have made the
            // AK-014 class a false-positive factory on spec vocabulary.
            if let Some(last) = reversed.first()
                && !SPEC_VOCABULARY_SEGMENTS.contains(last)
            {
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

// ---------------------------------------------------------------------------
// Plan Task 16 (correction RES-D5): the generic CLI adapter's WHOLE-CRATE
// vendor scan.
//
// Spec §4.1 and plan Task 16 step 1 say the `cli-agent` adapter carries
// "no vendor name anywhere in the crate". The core-crate scans above
// cannot see inside `crates/adapters/**` (that is the whole §24.3 point),
// and a public-API-extraction scan would be red at birth — settled
// legitimate adapters publicize vendor-shaped names (`BwrapSandbox`,
// `SqliteStateStore`). The adjudicated shape is therefore the NARROWEST
// honest rule with the required teeth: scan EVERY file of the ONE crate
// whose contract says "no vendor name anywhere", as raw text (whole
// files, all extensions, re-exports included — there are no submodules to
// launder through), over the EXISTING curated [`DENIED_TERMS`]. No new
// term list, no masks, no exemptions, and no edits to the functions,
// Roles, or allowlists above: this is strictly additive.
// ---------------------------------------------------------------------------

/// The crate directory the whole-crate vendor scan polices (plan Task 16).
pub const GENERIC_CLI_ADAPTER_DIR: &str = "crates/adapters/gauntlet-adapter-agent-cli";

/// Scan one file of the generic CLI adapter crate as raw text: a
/// case-insensitive substring match of every curated vendor term, with
/// the file's true line numbers. Splitting/escape normalization is
/// deliberately NOT carried over from [`scan_source`] — this scan reads
/// whole files of ONE crate whose written contract is "no vendor name
/// anywhere", so the recorded residual is narrower, not wider: runtime
/// assembly (`["cla", "ude"].join("")`) and `#[path]`-escaped submodules
/// stay invisible to every text scanner (see the module-doc residual).
pub fn scan_generic_cli_adapter(path: &str, text: &str, deny: &Denylist) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lowered = line.to_lowercase();
        for term in &deny.vendor {
            if contains_term(&lowered, term) {
                let message = format!(
                    "denied-cli-adapter-identifier: {path}:{}: '{term}' — the generic CLI \
                     adapter crate must carry no vendor identifier anywhere in its surface; \
                     selection is configuration, not code (plan Task 16; spec §4.1, §24.3)",
                    index + 1
                );
                if !out.contains(&message) {
                    out.push(message);
                }
            }
        }
    }
    out
}

/// The repo-level check: read EVERY file under
/// [`GENERIC_CLI_ADAPTER_DIR`] (any extension — the crate's whole
/// surface, Cargo.toml included) and run
/// [`scan_generic_cli_adapter`] over each. Unreadable files are
/// violations, never skips (I8). Anti-vacuity is built in: a missing
/// crate directory, an empty walk, or a scan that saw no Rust source or
/// no manifest all fail the check — a glob that matched nothing must not
/// pass emptily.
pub fn check_generic_cli_adapter_sources(root: &Path) -> Vec<String> {
    let deny = Denylist::derive(&[]); // only the curated vendor terms are used
    let crate_dir = root.join(GENERIC_CLI_ADAPTER_DIR);
    let mut violations = Vec::new();
    if !crate_dir.is_dir() {
        return vec![format!(
            "denied-cli-adapter-identifier: {GENERIC_CLI_ADAPTER_DIR} does not exist — the \
             whole-crate vendor scan would be vacuous"
        )];
    }
    let mut scanned = Vec::new();
    let mut walk = |dir: &Path, out: &mut Vec<PathBuf>| {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            let entries = match std::fs::read_dir(&current) {
                Ok(entries) => entries,
                Err(error) => {
                    violations.push(format!(
                        "denied-cli-adapter-identifier: {} is not readable ({error}) — missing \
                         evidence is not passing evidence (I8)",
                        current.display()
                    ));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path);
                }
            }
        }
    };
    let mut files = Vec::new();
    walk(&crate_dir, &mut files);
    files.sort();
    for path in &files {
        let relative = path
            .strip_prefix(root)
            .map(|stripped| stripped.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                violations.push(format!(
                    "denied-cli-adapter-identifier: {relative} is not readable ({error}) — \
                     missing evidence is not passing evidence (I8)"
                ));
                continue;
            }
        };
        scanned.push(relative.clone());
        violations.extend(scan_generic_cli_adapter(&relative, &text, &deny));
    }
    if files.is_empty() {
        violations.push(
            "denied-cli-adapter-identifier: the whole-crate scan walked zero files — the \
             scan is vacuous"
                .to_owned(),
        );
    }
    if !scanned.iter().any(|file| file.ends_with(".rs")) {
        violations.push(
            "denied-cli-adapter-identifier: no Rust source under the generic CLI adapter \
             crate was scanned — the walk or the crate layout is broken"
                .to_owned(),
        );
    }
    if !scanned.iter().any(|file| file.ends_with("Cargo.toml")) {
        violations.push(
            "denied-cli-adapter-identifier: the generic CLI adapter crate's manifest was not \
             scanned — the crate surface is incomplete"
                .to_owned(),
        );
    }
    violations
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
    // The file's physical lines by number (for the wire-key mask's
    // array-element shape: a rustfmt-formatted multi-line array puts each
    // element at the start of its own line, so the opening `[` or `,`
    // lives on the PREVIOUS line).
    let physical: std::collections::BTreeMap<usize, String> = crate::text::physical_lines(text)
        .into_iter()
        .map(|unit| (unit.line, unit.text))
        .collect();
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
        // lexing pass (`crate::text::string_literal_spans`) since round 6, so
        // a `'"'` char literal earlier on the line no longer deletes the
        // literals after it from this rule (critic-6 C1).
        if role == Role::Inner {
            for (start, end, literal) in crate::text::string_literal_spans(&unit.text) {
                if let Some(segment) = deny.segment_literal(&literal) {
                    let previous: &str = physical
                        .get(&unit.line.saturating_sub(1))
                        .map(String::as_str)
                        .unwrap_or("");
                    if annex_b_wire_key_access(path, &unit.text, previous, start, end, segment) {
                        continue;
                    }
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

/// The Annex-B wire-vocabulary mask: a whole-literal adapter-segment hit is
/// consumed ONLY inside one of [`crate::walk::ANNEX_B_CONTRACT_PARSERS`]
/// (the B.2 decoder, plan Task 9's B.3-family codec modules, and plan Task
/// 10's B.4-generation modules), only when
/// the literal is a declared [`crate::walk::ANNEX_B_WIRE_KEYS`] key, and
/// only in a key-ACCESS position — because matching the wire schema is not
/// adapter selection (AK-014). Position is a byte-level shape, not a parse
/// tree: a `.get("k")` argument (`get` as a whole identifier ended by `(`)
/// or an `&[…"k"…]` array element (bracketed by `[`/`,` before and
/// `,`/`]` after — either on the same line, or line-initial with the
/// opening `[`/`,` on the previous physical line, which is how
/// rustfmt-formatted schema tables spell their elements). Every other use
/// of the same literal in those modules reports exactly as before; see the
/// [`crate::walk::ANNEX_B_WIRE_KEYS`] doc for the recorded residual of the
/// shape.
fn annex_b_wire_key_access(
    path: &str,
    unit: &str,
    previous: &str,
    start: usize,
    end: usize,
    segment: &str,
) -> bool {
    if !crate::walk::ANNEX_B_CONTRACT_PARSERS.contains(&path)
        || !crate::walk::ANNEX_B_WIRE_KEYS
            .iter()
            .any(|(key, _)| *key == segment)
    {
        return false;
    }
    let before = unit[..start].trim_end();
    let after = unit[end..].trim_start();
    let get_call = before
        .strip_suffix('(')
        .and_then(|rest| rest.strip_suffix("get"))
        .is_some_and(|rest| {
            rest.chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        });
    let same_line_element = before
        .chars()
        .next_back()
        .is_some_and(|c| c == '[' || c == ',')
        && after.chars().next().is_some_and(|c| c == ',' || c == ']');
    // A rustfmt-formatted array puts each element at the start of its own
    // line, with the opening `[` (or the preceding element's `,`) on the
    // previous physical line. Line-initial position + previous line ending
    // in `[`/`,` + same-line `,`/`]` (or line end) is that element shape —
    // recorded residual: the shape cannot see the array's purpose, exactly
    // like the same-line form, and a line-initial literal following ANY
    // line ending in `[`/`,` (a split call argument list, say) is masked
    // in these codec modules.
    let line_initial_element = before.is_empty()
        && previous.trim_end().ends_with(['[', ','])
        && after.chars().next().is_none_or(|c| c == ',' || c == ']');
    get_call || same_line_element || line_initial_element
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

    /// Plan Task 16's teeth (correction RES-D5): the generic CLI adapter
    /// crate carries NO vendor identifier anywhere in its surface — the
    /// whole crate, every file, as raw text. A vendor identifier planted
    /// anywhere in this crate (public API included) fails this check, so
    /// `just boundaries` goes red before the substitution story can
    /// become vendor-coupled.
    #[test]
    fn the_generic_cli_adapter_crate_carries_no_vendor_identifiers() {
        let violations = crate::check_generic_cli_adapter_sources(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("denied-cli-adapter-identifier", &violations)
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

        // The whole-word segment stays denied when the word is NOT spec
        // vocabulary: the fixture's adapters() list carries the real
        // distinguishing segments, and a fresh one still flags.
        let fixture = "let pick = \"diff\";\n";
        let violations = scan_source(
            "crates/gauntlet-app/src/lib.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        assert!(
            violations.iter().any(|v| v.contains("'diff'")),
            "a distinguishing segment still flags: {violations:?}"
        );
    }

    /// Plan Task 10: `model` — the spec's own field word — landed as the
    /// last segment of `gauntlet-adapter-compare-model`. It is pinned here
    /// as NOT a segment term (the closed Task-5 config code spells the
    /// §8.2 settings key `model` ten times), while the distinguishing
    /// spellings of the same adapter ARE still denied: a match-arm or
    /// const selecting the comparator by `compare-model`, by
    /// `compare_model`, or by the full crate name reports.
    #[test]
    fn spec_vocabulary_segments_are_pinned_and_the_spellings_still_deny() {
        let mut members = adapters();
        members.push(Member {
            dir: "crates/adapters/gauntlet-adapter-compare-model".to_string(),
            name: "gauntlet-adapter-compare-model".to_string(),
        });
        members.push(Member {
            dir: "crates/adapters/gauntlet-adapter-verify-file-digest".to_string(),
            name: "gauntlet-adapter-verify-file-digest".to_string(),
        });
        members.push(Member {
            dir: "crates/adapters/gauntlet-adapter-verify-schema-check".to_string(),
            name: "gauntlet-adapter-verify-schema-check".to_string(),
        });
        members.push(Member {
            dir: "crates/adapters/gauntlet-adapter-sandbox-none".to_string(),
            name: "gauntlet-adapter-sandbox-none".to_string(),
        });
        let deny = Denylist::derive(&members);
        // Plan Task 11 extended the list (see the const's doc): `digest` is
        // the §5.2 Digest field vocabulary, `check` is §14.2's own rung
        // name — both landed as last segments of the Task-11 verifier
        // adapters while closed earlier tasks already spelled those words
        // as ordinary domain literals. Plan Task 14 added `none` (§6.1
        // isolation token, §8.2 backend enum, §8.6 default) for
        // `gauntlet-adapter-sandbox-none`.
        assert_eq!(
            SPEC_VOCABULARY_SEGMENTS,
            &["model", "digest", "check", "none"]
        );
        assert!(
            !deny
                .adapter_segments
                .iter()
                .any(|s| s == "model" || s == "digest" || s == "check" || s == "none"),
            "spec field vocabulary must not be a segment term"
        );
        for spelling in [
            "gauntlet-adapter-compare-model",
            "compare-model",
            "compare_model",
            "model-compare",
            "model_compare",
            "gauntlet-adapter-verify-file-digest",
            "verify-file-digest",
            "verify_file_digest",
            "digest-file-verify",
            "digest_file_verify",
            "gauntlet-adapter-verify-schema-check",
            "verify-schema-check",
            "verify_schema_check",
            "check-schema-verify",
            "check_schema_verify",
            "gauntlet-adapter-sandbox-none",
            "sandbox-none",
            "sandbox_none",
            "none-sandbox",
            "none_sandbox",
        ] {
            let fixture = format!("const PICK: &str = {spelling:?};\n");
            let violations = scan_source(
                "crates/gauntlet-app/src/lib.rs",
                &fixture,
                &deny,
                Role::Inner,
            );
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains(&format!("'{spelling}'"))),
                "the distinguishing spelling {spelling:?} must still be denied: {violations:?}"
            );
        }
        // And the bare words stay legal inner-layer vocabulary.
        for word in ["model", "digest", "check", "none"] {
            let clean = scan_source(
                "crates/gauntlet-app/src/lib.rs",
                &format!("let word = {word:?};\n"),
                &deny,
                Role::Inner,
            );
            assert!(clean.is_empty(), "{word:?}: {clean:?}");
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

    /// The shared `adapters()` fixture predates
    /// `gauntlet-adapter-observe-command`, so the Annex-B.2 wire key
    /// `command` is not a derived segment there — and a mask test needs the
    /// masked literal to be a segment. This is the fixture the Annex-B.2
    /// tests scan with.
    fn annex_deny() -> Denylist {
        let mut members = adapters();
        members.push(Member {
            dir: "crates/adapters/gauntlet-adapter-observe-command".to_string(),
            name: "gauntlet-adapter-observe-command".to_string(),
        });
        Denylist::derive(&members)
    }

    #[test]
    fn annex_b_wire_keys_are_masked_only_in_the_parser_in_key_access() {
        let deny = annex_deny();
        let fixture = "\
reject_unknown_fields(BUILDER_RESULT_SCHEMA, object, &[\"command\", \"exit_code\"])?;
let command = match object.get(\"command\") {
    Some(JsonValue::Str(s)) => s.clone(),
    None => return Err(payload_missing(\"self_checks.command\")),
};
let prime = match object.get(\"prime\") {
    None => return Err(payload_missing(\"self_checks.prime\")),
};
";
        let parser = scan_source(
            crate::walk::ANNEX_B_CONTRACT_PARSERS[0],
            fixture,
            &deny,
            Role::Inner,
        );
        assert!(
            !parser.iter().any(|v| v.contains("'command'")),
            "a declared wire key in key-access position must be masked in the parser: \
             {parser:?}"
        );
        assert!(
            parser.iter().any(|v| v.contains("'prime'")),
            "a non-wire segment must still report at the same position in the parser: \
             {parser:?}"
        );

        let elsewhere = scan_source(
            "crates/gauntlet-ports/src/unrelated.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        let command_hits: Vec<_> = elsewhere
            .iter()
            .filter(|v| v.contains("'command'"))
            .collect();
        assert_eq!(
            command_hits.len(),
            2,
            "outside the parser module every key-access use must report: {elsewhere:?}"
        );
    }

    #[test]
    fn annex_b_wire_keys_are_masked_in_the_task9_codec_modules_too() {
        // Plan Task 9's growth of the parser list (see the pinned const):
        // the B.3-family codec modules get the SAME mask under the SAME
        // gates — declared key, key-access position — and core files
        // outside the list still report every spelling.
        let deny = annex_deny();
        let fixture = "\
let command = match object.get(\"command\") {
    Some(JsonValue::Str(s)) => s.clone(),
    None => return Err(missing(&[\"command\"])),
};
let prime = match object.get(\"prime\") {
    None => return Err(missing(\"prime\")),
};
";
        for parser in &crate::walk::ANNEX_B_CONTRACT_PARSERS[1..] {
            let violations = scan_source(parser, fixture, &deny, Role::Inner);
            assert!(
                !violations.iter().any(|v| v.contains("'command'")),
                "a declared wire key in key-access position must be masked in {parser}: \
                 {violations:?}"
            );
            assert!(
                violations.iter().any(|v| v.contains("'prime'")),
                "a non-wire segment must still report at the same position in {parser}: \
                 {violations:?}"
            );
        }
        let outside = scan_source(
            "crates/gauntlet-app/src/observation/digest.rs",
            fixture,
            &deny,
            Role::Inner,
        );
        assert!(
            outside.iter().filter(|v| v.contains("'command'")).count() >= 2,
            "outside the parser list every key-access use must report: {outside:?}"
        );
    }

    #[test]
    fn annex_b_wire_key_outside_key_access_still_reports_in_the_parser() {
        let deny = annex_deny();
        for fixture in [
            "let role = \"command\";\n",
            "match role { \"command\" => run(), _ => skip() }\n",
            "const DEFAULT_ROLE: &str = \"command\";\n",
            "budget(\"command\");\n",
            "get (\"command\");\n",
            "getter(\"command\");\n",
        ] {
            let violations = scan_source(
                crate::walk::ANNEX_B_CONTRACT_PARSERS[0],
                fixture,
                &deny,
                Role::Inner,
            );
            assert!(
                violations.iter().any(|v| v.contains("'command'")),
                "non-key-access 'command' must report inside the parser: {fixture:?} -> \
                 {violations:?}"
            );
        }
    }

    #[test]
    fn annex_b_mask_corners_are_pinned_to_the_documented_residual() {
        let deny = annex_deny();
        // Escaped and padded spellings decode to the same key value the
        // parser sees (segment literals are trimmed at both ends, so a
        // leading space pads too), so a key-access position masks them all.
        for fixture in [
            "object.get(\"com\\u{6d}and\");\n",
            "object.get(\"command \");\n",
            "object.get(\" command\");\n",
        ] {
            let violations = scan_source(
                crate::walk::ANNEX_B_CONTRACT_PARSERS[0],
                fixture,
                &deny,
                Role::Inner,
            );
            assert!(
                !violations.iter().any(|v| v.contains("'command'")),
                "{fixture:?} -> {violations:?}"
            );
        }
        // The byte-level shapes cannot see the array's purpose or the `.get`
        // receiver — recorded residuals, not a silent widening.
        for fixture in ["other(&[\"command\"]);\n", "commands.get(\"command\");\n"] {
            let violations = scan_source(
                crate::walk::ANNEX_B_CONTRACT_PARSERS[0],
                fixture,
                &deny,
                Role::Inner,
            );
            assert!(
                !violations.iter().any(|v| v.contains("'command'")),
                "{fixture:?} -> {violations:?}"
            );
        }
        // RECORDED RESIDUAL (critic-B BIT-5a, correction C9 — pinned
        // visibility, not acceptance): a SPLIT call argument —
        // `registry.select("builder",\n    "command",\n)` — is
        // selection-shaped, and it MASKS today in these codec modules: the
        // literal is line-initial, its previous line ends in `,`, and its
        // same-line `after` starts with `,`, so the line-initial element
        // shape (and the joined-unit same-line shape) both fire. This is
        // ledger entry D4 of the Task-9 correction adjudication: the
        // accepted masking class for these four modules, whose
        // re-litigation trigger is the next arch-suite touch (alongside
        // E4/F1/F3 per the standing rule). The assertion pins that we SEE
        // it mask; the outside-the-module control below pins that the
        // fixture is genuinely detectable, so this can never quietly pass
        // vacuously.
        let split_argument = "let observer = registry.select(\"builder\",\n    \"command\",\n);\n";
        for parser in crate::walk::ANNEX_B_CONTRACT_PARSERS {
            let violations = scan_source(parser, split_argument, &deny, Role::Inner);
            assert!(
                !violations.iter().any(|v| v.contains("'command'")),
                "the recorded split-argument residual masks in {parser} (D4): {violations:?}"
            );
        }
        let outside = scan_source(
            "crates/gauntlet-app/src/observation/digest.rs",
            split_argument,
            &deny,
            Role::Inner,
        );
        assert!(
            outside.iter().any(|v| v.contains("'command'")),
            "outside the codec modules the same split argument reports: {outside:?}"
        );
        // Plan Task 9's line-initial element form: a rustfmt-formatted
        // schema table masks its line-initial declared keys (previous line
        // ends in `[`/`,`), while the same line-initial spelling WITHOUT
        // that predecessor still reports.
        let multi_line_table = "const FIELDS: &[&str] = &[\n    \"command\",\n];\n";
        for parser in crate::walk::ANNEX_B_CONTRACT_PARSERS {
            let violations = scan_source(parser, multi_line_table, &deny, Role::Inner);
            assert!(
                !violations.iter().any(|v| v.contains("'command'")),
                "a line-initial declared key in a schema table must be masked in {parser}: \
                 {violations:?}"
            );
        }
        let orphan_line_initial = "let role =\n    \"command\";\n";
        let violations = scan_source(
            crate::walk::ANNEX_B_CONTRACT_PARSERS[0],
            orphan_line_initial,
            &deny,
            Role::Inner,
        );
        assert!(
            violations.iter().any(|v| v.contains("'command'")),
            "a line-initial literal whose previous line ends in `=` still reports: {violations:?}"
        );
    }

    /// C7 (critic-B BIT-5b'): the maskable vocabulary — what the Annex-B
    /// wire-key mask can ever hide at a key-access position in the four
    /// codec modules — is the intersection of the derived adapter
    /// SEGMENTS with the DECLARED wire keys. This pin computes that
    /// intersection from the REAL workspace member list
    /// ([`Denylist::derive`] over [`crate::deps::read_members`], exactly
    /// what the repo scan uses), so landing an adapter whose last name
    /// segment collides with a declared wire key (say
    /// `gauntlet-adapter-observe-path` or `-claims`) FAILS here and
    /// forces a deliberate, recorded decision — widening the pin and the
    /// mask's documented residual together — instead of silently
    /// widening what the mask can hide.
    #[test]
    fn the_maskable_wire_key_intersection_is_pinned() {
        let root = crate::walk::repo_root();
        let manifest =
            std::fs::read_to_string(root.join("Cargo.toml")).expect("the manifest is readable");
        let members =
            crate::deps::read_members(&root, &manifest).expect("the workspace members resolve");
        let deny = Denylist::derive(&members);
        assert_eq!(
            maskable_wire_keys(&deny).as_slice(),
            ["command"],
            "an adapter segment now collides with a declared Annex-B wire key: growing the \
             maskable set is a deliberate, recorded decision — update this pin and the mask's \
             documented residual together (plan Task 10 recomputed this pin from the real \
             member list when gauntlet-adapter-compare-model landed; it stayed [command] \
             because `model` is excluded at segment derivation — see \
             SPEC_VOCABULARY_SEGMENTS)"
        );

        // The control that keeps the pin honest: the intersection is
        // COMPUTED from the member list, not a constant — a hypothetical
        // member whose last segment is a declared wire key widens it, which
        // is exactly the failure this pin must produce on a real edit.
        let mut grown = members.clone();
        grown.push(Member {
            dir: "crates/adapters/gauntlet-adapter-observe-path".to_string(),
            name: "gauntlet-adapter-observe-path".to_string(),
        });
        let widened = Denylist::derive(&grown);
        assert!(
            maskable_wire_keys(&widened).as_slice().contains(&"path"),
            "the control member must widen the intersection"
        );
    }

    /// The adapter segments that are ALSO declared Annex-B wire keys — the
    /// whole vocabulary the parser mask can hide, in the segment sort
    /// order (length, then alphabetical). Same inputs and same predicate
    /// as the mask's own module+key gates, so it cannot drift from them.
    fn maskable_wire_keys(deny: &Denylist) -> Vec<&str> {
        deny.adapter_segments
            .iter()
            .map(String::as_str)
            .filter(|segment| {
                crate::walk::ANNEX_B_WIRE_KEYS
                    .iter()
                    .any(|(key, _)| key == segment)
            })
            .collect()
    }

    // -------------------------------------------------------------------
    // Plan Task 16 (correction RES-D5): the generic CLI adapter's
    // whole-crate vendor scan — its own scanner tests, so the rule has
    // demonstrated teeth of its own shape (raw text, every curated term,
    // true line numbers) rather than borrowing the core scan's.
    // -------------------------------------------------------------------

    /// Every curated term is flagged by the whole-crate scanner, at the
    /// true line, with the plan-Task-16 message.
    #[test]
    fn every_denied_term_is_flagged_in_the_cli_adapter_scan() {
        let deny = deny();
        for term in DENIED_TERMS {
            let fixture = format!("let binding = \"call {term} now\";\nlet ok = 1;\n");
            let violations = scan_generic_cli_adapter("fixture.rs", &fixture, &deny);
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains(&format!("'{term}'")) && v.contains("fixture.rs:1")),
                "term {term} not flagged at line 1: {violations:?}"
            );
        }
    }

    /// The scan is case-insensitive, reads whole files (comment prose
    /// included — a vendor name in a doc comment is still a vendor name
    /// in this crate), and reports NOTHING on clean text or on the
    /// adapter's own capability vocabulary.
    #[test]
    fn the_cli_adapter_scan_is_case_insensitive_and_whole_file() {
        let deny = deny();
        let prose = "//! This module fronts any headless agent.\n// The configuration is \
                     named OpenAI-shaped by the operator? That sentence itself is a \
                     violation.\n";
        let violations = scan_generic_cli_adapter("lib.rs", prose, &deny);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("lib.rs:2") && v.contains("'openai'")),
            "prose must not launder the crate's vendor coupling: {violations:?}"
        );
        let clean = scan_generic_cli_adapter(
            "lib.rs",
            "//! Generic CLI agent adapter: selection is configuration, not code.\npub const \
             RUNNER_TOKEN: &str = \"cli-agent\";\n",
            &deny,
        );
        assert!(clean.is_empty(), "{clean:?}");
    }

    /// The negative control for the repo-level check's anti-vacuity: a
    /// missing crate directory, an empty walk, or a walk without Rust
    /// source or manifest all FAIL rather than pass emptily.
    #[test]
    fn a_vacuous_cli_adapter_scan_fails_rather_than_passing() {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-cli-scan-vacuous-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        // (a) A missing crate directory is a violation.
        let missing = check_generic_cli_adapter_sources(&dir);
        assert!(
            missing.iter().any(|v| v.contains("does not exist")),
            "{missing:?}"
        );
        // (b) A crate directory with only a manifest is a violation (no
        // Rust source scanned). The check looks under the fixed relative
        // crate path, so the scratch mirrors that layout.
        let crate_dir = dir
            .join("crates")
            .join("adapters")
            .join("gauntlet-adapter-agent-cli");
        std::fs::create_dir_all(&crate_dir).expect("scratch creatable");
        std::fs::write(crate_dir.join("Cargo.toml"), "[package]\nname = \"x\"\n")
            .expect("manifest writable");
        let manifest_only = check_generic_cli_adapter_sources(&dir);
        assert!(
            manifest_only.iter().any(|v| v.contains("no Rust source")),
            "{manifest_only:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
