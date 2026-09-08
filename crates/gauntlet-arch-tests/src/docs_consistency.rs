//! Wave-2 docs/handoff consistency legs (task-27 charter slice B3).
//!
//! # Instrument and evidence locator
//!
//! These checks run inside the `tests/contract/docs_present` target
//! (AK-068's frozen locator), beside B2's structural legs: the address
//! file drives [`check_handoff_matches_conformance`] and
//! [`check_docs_claims_not_ahead_of_conformance`] from here.
//!
//! # What this enforces
//!
//! - **handoff ↔ conformance grouping equality.** The canonical handoff
//!   (`.agents/reports/handoff.md`, AK-067's frozen locator) must carry
//!   all six §26 categories as sections, and the set of ruler units
//!   (`AK-NNN`) listed under each category must EQUAL — counts included
//!   — the grouping `conformance.json` derives from each unit's
//!   `status_category`. No unit in two sections, every unit exactly
//!   once. This is charter tooth T4: moving a unit between sections in
//!   the handoff makes the suite RED.
//! - **docs status claims never run ahead of the conformance report.**
//!   Every `- <status_word> — AK-NNN:` claim line in the eight §25 docs
//!   is admissible only where the sweep's adjudication supports it:
//!   `implemented_and_proven` only where the json says
//!   `implemented_and_proven`; `implemented_but_provisional` where the
//!   json says proven OR provisional (claiming weaker than the evidence
//!   is safe; claiming stronger is not); the other four categories only
//!   where the json agrees exactly. A claim naming an id the json does
//!   not carry is a violation (fail-closed).
//! - **COR-B3 (falsifier F8) — laundering-proof claim binding.** The
//!   canonical line shape above stays the primary grammar, and a
//!   FALLBACK window scan binds everything it cannot see: for each
//!   line naming an `AK-NNN` id — ANY casing, canonicalized to the
//!   report's spelling (`ak-053` ⇒ `AK-053`) — every six-word §26
//!   vocabulary token (any casing, word-delimited) on that line or its
//!   immediate neighbors is a claim about that id under the same
//!   ahead-of-evidence rule. This closes the five executed evasion
//!   classes (leading capital, ALL CAPS, hyphen/colon separators
//!   instead of the em-dash, a claim split across two lines, lowercase
//!   unit ids). The window keys on the vocabulary WORDS only — verbs
//!   ("implements", "delivers") and bare status-like tokens never bind
//!   a claim, so ordinary prose beside an id stays prose.
//!
//! # Scope honesty (deliberate limits)
//!
//! - The json is read from the committed locator
//!   (`.agents/reports/conformance.json`) at TEST time and compared
//!   MECHANICALLY — no count, id, or digest is hardcoded here, so the
//!   settlement re-sweep regenerating the json cannot stale this test
//!   except by genuinely changing the grouping (which must then be
//!   reflected in the handoff — that is the point).
//! - The test does NOT pin `conformance.json`'s embedded
//!   manifest/answer-key digests against the tree: manifest-sha
//!   freshness is the sweep's own invariant, not a docs invariant.
//! - These checks do not judge whether a status is EARNED; they keep
//!   the three documents from disagreeing with the one instrument that
//!   adjudicates (the sweep).
//!
//! # Fixture overrides (RED teeth without mutating real artifacts)
//!
//! Two environment variables repoint the checkers at fixture copies —
//! the tooth procedure (orchestrator T4 and the ahead-of-evidence leg)
//! copies the real artifact to a temp dir, tampers the COPY, and runs
//! the target with the override set:
//!
//! - `GAUNTLET_CONFORMANCE_JSON_FIXTURE` — path to a fixture
//!   conformance.json (absolute, or relative to the repo root);
//! - `GAUNTLET_HANDOFF_FIXTURE` — path to a fixture handoff.md.
//!
//! The env overrides affect only the two integration legs driven from
//! `tests/contract/docs_present.rs`; the unit tests below pass explicit
//! paths (nextest runs each test in its own process, so a fixture run
//! cannot leak into a concurrent plain run of the same binary).
//!
//! # JSON parsing
//!
//! This crate depends on no internal crate (its own declared posture)
//! and carries only `toml`, so this module parses the FROZEN
//! `gauntlet.conformance@v1` report with a small fail-closed recursive
//! descent parser over the JSON grammar (strings with escapes, numbers,
//! objects, arrays, literals). Anything malformed is a violation,
//! never a skip (I8).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::docs::{CANONICAL_HANDOFF_LOCATOR, DOC_SET, STATUS_WORDS};

/// The committed conformance report's frozen locator.
pub const CONFORMANCE_LOCATOR: &str = ".agents/reports/conformance.json";

/// The schema line every report this module reads must carry.
pub const CONFORMANCE_SCHEMA: &str = "gauntlet.conformance@v1";

/// Env override repointing the conformance read at a fixture copy.
pub const CONFORMANCE_FIXTURE_ENV: &str = "GAUNTLET_CONFORMANCE_JSON_FIXTURE";

/// Env override repointing the handoff read at a fixture copy.
pub const HANDOFF_FIXTURE_ENV: &str = "GAUNTLET_HANDOFF_FIXTURE";

/// Where the conformance report is read from: the fixture override if
/// set, else the committed locator under `root`.
pub fn conformance_path(root: &Path) -> PathBuf {
    resolve_override(root, CONFORMANCE_FIXTURE_ENV, CONFORMANCE_LOCATOR)
}

/// Where the canonical handoff is read from: the fixture override if
/// set, else the frozen locator under `root`.
pub fn handoff_path(root: &Path) -> PathBuf {
    resolve_override(root, HANDOFF_FIXTURE_ENV, CANONICAL_HANDOFF_LOCATOR)
}

fn resolve_override(root: &Path, env_name: &str, default: &str) -> PathBuf {
    match std::env::var_os(env_name) {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        }
        None => root.join(default),
    }
}

/// Leg (f): the canonical handoff's category sections equal the json's
/// grouping, counts included (charter tooth T4).
pub fn check_handoff_matches_conformance(root: &Path) -> Vec<String> {
    check_handoff_matches_conformance_at(&handoff_path(root), &conformance_path(root))
}

/// Leg (g): the eight docs' status claim lines never claim ahead of
/// the conformance report.
pub fn check_docs_claims_not_ahead_of_conformance(root: &Path) -> Vec<String> {
    check_docs_claims_not_ahead_at(&root.join("docs"), &conformance_path(root))
}

// -------------------------------------------------------------------------
// shared extraction
// -------------------------------------------------------------------------

/// `AK-` followed by one or more ASCII digits, and nothing else.
fn is_unit_id(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("AK-") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

/// Every `AK-NNN` id on the line, word-delimited on both sides
/// (underscores are word characters; hyphens delimit, so `UNK-003` and
/// the prose literal `AK-NNN` never match). CASE-INSENSITIVE and
/// CANONICALIZING (COR-B3, falsifier F8's class 5): `ak-053`, `Ak-053`,
/// and `AK-053` all yield the canonical `AK-053` spelling the
/// conformance report keys on.
fn unit_ids_on(line: &str) -> Vec<String> {
    let lowered = line.to_lowercase();
    let bytes = lowered.as_bytes();
    let mut ids = Vec::new();
    let mut at = 0;
    while let Some(found) = lowered[at..].find("ak-") {
        let start = at + found;
        let mut end = start + 3;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let has_digits = end > start + 3;
        let boundary_before = start == 0
            || !lowered[..start]
                .chars()
                .next_back()
                .is_some_and(is_word_char);
        let boundary_after = !lowered[end..].chars().next().is_some_and(is_word_char);
        if has_digits && boundary_before && boundary_after {
            ids.push(format!("AK-{}", &lowered[start + 3..end]));
        }
        at = end;
    }
    ids
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The unit → category grouping from a `gauntlet.conformance@v1`
/// document: category → sorted unit ids. Fail-closed: any structural
/// deviation from the frozen schema is an error, never a guess.
fn conformance_grouping(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let root = mini_json::parse(text).map_err(|err| format!("malformed json: {err}"))?;
    let schema = root
        .get("schema")
        .and_then(mini_json::Json::as_str)
        .ok_or("the schema field is missing")?;
    if schema != CONFORMANCE_SCHEMA {
        return Err(format!("schema {schema:?} is not {CONFORMANCE_SCHEMA:?}"));
    }
    let units = root
        .get("units")
        .and_then(mini_json::Json::as_arr)
        .ok_or("the units array is missing")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut grouping: BTreeMap<String, Vec<String>> = STATUS_WORDS
        .iter()
        .map(|word| ((*word).to_owned(), Vec::new()))
        .collect();
    for (index, unit) in units.iter().enumerate() {
        let id = unit
            .get("id")
            .and_then(mini_json::Json::as_str)
            .ok_or_else(|| format!("units[{index}] carries no id"))?;
        if !is_unit_id(id) {
            return Err(format!("units[{index}] id {id:?} is not an AK-NNN id"));
        }
        if !seen.insert(id.to_owned()) {
            return Err(format!("unit {id} appears twice in the units array"));
        }
        let category = unit
            .get("status_category")
            .and_then(mini_json::Json::as_str)
            .ok_or_else(|| format!("unit {id} carries no status_category"))?;
        let bucket = grouping.get_mut(category).ok_or_else(|| {
            format!("unit {id} status_category {category:?} is not one of the six §26 words")
        })?;
        bucket.push(id.to_owned());
    }
    for bucket in grouping.values_mut() {
        bucket.sort();
    }
    Ok(grouping)
}

// -------------------------------------------------------------------------
// leg (f) — handoff sections vs json grouping
// -------------------------------------------------------------------------

/// One `## <category> — N units` section of the canonical handoff.
#[derive(Debug)]
struct HandoffSection {
    category: String,
    declared_count: usize,
    ids: Vec<String>,
}

/// Parse the category sections out of the handoff text. A header is a
/// category section iff the text after `## ` starts with one of the six
/// §26 words followed by ` — `; the declared count is the number before
/// `unit`/`units`. Any category header that does not parse refuses.
fn handoff_sections(text: &str) -> Result<Vec<HandoffSection>, String> {
    let mut sections = Vec::new();
    let mut current: Option<HandoffSection> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            let matched = STATUS_WORDS.iter().find_map(|word| {
                rest.strip_prefix(word)
                    .and_then(|after| after.strip_prefix(" — "))
                    .map(|count_text| (*word, count_text))
            });
            if let Some((word, count_text)) = matched {
                let digits: String = count_text
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                let declared = if digits.is_empty() {
                    return Err(format!("the {word} header carries no `N unit(s)` count"));
                } else if !count_text[digits.len()..].starts_with(" unit") {
                    return Err(format!(
                        "the {word} header count is not followed by `unit(s)`"
                    ));
                } else {
                    digits
                        .parse::<usize>()
                        .map_err(|err| format!("the {word} header count does not parse: {err}"))?
                };
                current = Some(HandoffSection {
                    category: word.to_owned(),
                    declared_count: declared,
                    ids: Vec::new(),
                });
            }
        } else if let Some(section) = current.as_mut()
            && line.starts_with("- ")
        {
            section.ids.extend(unit_ids_on(line));
        }
    }
    if let Some(section) = current.take() {
        sections.push(section);
    }
    Ok(sections)
}

/// The grouping check over explicit paths (the unit tests' seam).
pub fn check_handoff_matches_conformance_at(handoff: &Path, conformance: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let json_text = match std::fs::read_to_string(conformance) {
        Ok(text) => text,
        Err(err) => {
            return vec![format!(
                "handoff-consistency: conformance report unreadable at {}: {err}",
                conformance.display()
            )];
        }
    };
    let grouping = match conformance_grouping(&json_text) {
        Ok(grouping) => grouping,
        Err(err) => {
            return vec![format!(
                "handoff-consistency: conformance report rejected at {}: {err}",
                conformance.display()
            )];
        }
    };
    let handoff_text = match std::fs::read_to_string(handoff) {
        Ok(text) => text,
        Err(err) => {
            return vec![format!(
                "handoff-consistency: handoff unreadable at {}: {err}",
                handoff.display()
            )];
        }
    };
    let sections = match handoff_sections(&handoff_text) {
        Ok(sections) => sections,
        Err(err) => {
            return vec![format!(
                "handoff-consistency: handoff rejected at {}: {err}",
                handoff.display()
            )];
        }
    };

    // Each category exactly once.
    for word in STATUS_WORDS {
        let count = sections.iter().filter(|s| s.category == word).count();
        match count {
            0 => violations.push(format!(
                "handoff-consistency: the handoff has no `{word}` section"
            )),
            1 => {}
            more => violations.push(format!(
                "handoff-consistency: the handoff has {more} `{word}` sections (exactly one \
                 required)"
            )),
        }
    }
    // A unit in two sections can never be right, whatever the json says.
    let mut where_seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for section in &sections {
        for id in &section.ids {
            where_seen.entry(id).or_default().push(&section.category);
        }
    }
    for (id, categories) in &where_seen {
        if categories.len() > 1 {
            violations.push(format!(
                "handoff-consistency: unit {id} appears in {} sections ({}) — every unit \
                 exactly once",
                categories.len(),
                categories.join(", ")
            ));
        }
    }
    // Set and count equality per category.
    for word in STATUS_WORDS {
        let Some(section) = sections.iter().find(|s| s.category == word) else {
            continue;
        };
        let json_ids: Vec<&str> = grouping[word].iter().map(String::as_str).collect();
        let section_ids: Vec<&str> = section.ids.iter().map(String::as_str).collect();
        if section.declared_count != json_ids.len() {
            violations.push(format!(
                "handoff-consistency: the `{word}` header declares {} unit(s) but \
                 conformance.json groups {} there",
                section.declared_count,
                json_ids.len()
            ));
        }
        if section.ids.len() != section.declared_count {
            violations.push(format!(
                "handoff-consistency: the `{word}` section lists {} unit bullet(s) under a \
                 header declaring {}",
                section.ids.len(),
                section.declared_count
            ));
        }
        let missing: Vec<&str> = json_ids
            .iter()
            .copied()
            .filter(|id| !section_ids.contains(id))
            .collect();
        if !missing.is_empty() {
            violations.push(format!(
                "handoff-consistency: the `{word}` section is missing {} unit(s) the json \
                 groups there: {}",
                missing.len(),
                missing.join(", ")
            ));
        }
        let extra: Vec<&str> = section_ids
            .iter()
            .copied()
            .filter(|id| !json_ids.contains(id))
            .collect();
        if !extra.is_empty() {
            violations.push(format!(
                "handoff-consistency: the `{word}` section lists unit(s) the json does not \
                 group there: {}",
                extra.join(", ")
            ));
        }
    }
    violations
}

// -------------------------------------------------------------------------
// leg (g) — docs claim lines vs json adjudication
// -------------------------------------------------------------------------

/// A parsed `- <status_word> — AK-NNN:` claim line.
#[derive(Debug, PartialEq)]
struct ClaimLine<'a> {
    status: &'a str,
    id: &'a str,
}

fn parse_claim(line: &str) -> Option<ClaimLine<'_>> {
    let rest = line.strip_prefix("- ")?;
    for word in STATUS_WORDS {
        if let Some(after_word) = rest.strip_prefix(word)
            && let Some(tail) = after_word.strip_prefix(" — ")
            && let Some(colon) = tail.find(':')
            && is_unit_id(&tail[..colon])
        {
            return Some(ClaimLine {
                status: word,
                id: &tail[..colon],
            });
        }
    }
    None
}

/// COR-B3 fallback (falsifier F8's classes 1-4): the line-anchored
/// canonical grammar stays the PRIMARY parse, and this scan binds what
/// it cannot see. For each line naming an `AK-NNN` id (any casing,
/// canonicalized), every six-word §26 vocabulary token (any casing,
/// word-delimited) on that line or its immediate neighbors is a CLAIM
/// about that id, subject to the same ahead-of-evidence rule. This
/// closes the leading-capital, ALL-CAPS, separator-variant
/// (`—`/`-`/`–`/`:`), and split-two-line shapes without inventing a
/// second grammar.
///
/// Two deliberate invariants:
/// - the window keys on the six VOCABULARY WORDS, never on verbs or
///   bare status-like tokens — "the runtime implements X" beside an id
///   is prose until a category word actually appears;
/// - the window is conservative by design: a vocabulary token on an
///   ADJACENT line binds the neighboring id too (adjacent claims read
///   as claims about each other's units) — over-pairing can only ever
///   surface as an ahead-of-evidence violation, which the canonical
///   line shape resolves by keeping claims at least one non-claim line
///   apart, as the committed docs do.
fn window_claims(lines: &[&str]) -> Vec<(usize, &'static str, String)> {
    let mut claims = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let ids = unit_ids_on(line);
        if ids.is_empty() {
            continue;
        }
        let lo = index.saturating_sub(1);
        let hi = (index + 1).min(lines.len() - 1);
        for neighbor in &lines[lo..=hi] {
            let lowered = neighbor.to_lowercase();
            for word in STATUS_WORDS {
                if crate::docs::contains_word(&lowered, word) {
                    for id in &ids {
                        claims.push((index, word, id.clone()));
                    }
                }
            }
        }
    }
    claims
}

/// The admissibility matrix: may a doc claim `status` for a unit the
/// json adjudicates `json_category`? Claiming WEAKER than the evidence
/// is safe; claiming STRONGER is not.
fn claim_admissible(status: &str, json_category: &str) -> bool {
    match status {
        "implemented_and_proven" => json_category == "implemented_and_proven",
        "implemented_but_provisional" => {
            matches!(
                json_category,
                "implemented_and_proven" | "implemented_but_provisional"
            )
        }
        "configured_but_untested" | "proposed" | "blocked" | "unknown" => status == json_category,
        _ => false,
    }
}

/// The claim-line check over explicit paths (the unit tests' seam).
pub fn check_docs_claims_not_ahead_at(docs_root: &Path, conformance: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let json_text = match std::fs::read_to_string(conformance) {
        Ok(text) => text,
        Err(err) => {
            return vec![format!(
                "docs-claims: conformance report unreadable at {}: {err}",
                conformance.display()
            )];
        }
    };
    let grouping = match conformance_grouping(&json_text) {
        Ok(grouping) => grouping,
        Err(err) => {
            return vec![format!(
                "docs-claims: conformance report rejected at {}: {err}",
                conformance.display()
            )];
        }
    };
    let json_category = |id: &str| -> Option<&str> {
        STATUS_WORDS.iter().copied().find(|word| {
            grouping
                .get(*word)
                .is_some_and(|bucket| bucket.iter().any(|unit| unit == id))
        })
    };
    for name in DOC_SET {
        let text = match std::fs::read_to_string(docs_root.join(name)) {
            Ok(text) => text,
            Err(err) => {
                violations.push(format!("docs-claims: docs/{name} unreadable: {err}"));
                continue;
            }
        };
        let lines: Vec<&str> = text.lines().collect();
        // The claims of this document: the canonical line-anchored parse
        // first (exact line attribution), then the COR-B3 window
        // fallback for every variant shape (casing, separators,
        // split lines). A (status, id) pairing is judged once — the
        // primary occurrence wins the line number.
        let mut claims: Vec<(usize, &str, String)> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if let Some(claim) = parse_claim(line) {
                claims.push((index, claim.status, claim.id.to_owned()));
            }
        }
        claims.extend(window_claims(&lines));
        let mut judged: BTreeSet<(String, String)> = BTreeSet::new();
        for (index, status, id) in claims {
            if !judged.insert((status.to_owned(), id.clone())) {
                continue;
            }
            match json_category(&id) {
                None => violations.push(format!(
                    "docs-claims: docs/{name}:{} claims {status} for {id} but the conformance \
                     report carries no such unit",
                    index + 1
                )),
                Some(category) => {
                    if !claim_admissible(status, category) {
                        violations.push(format!(
                            "docs-claims: docs/{name}:{} claims {status} for {id} but \
                             conformance.json adjudicates it {category} (ahead of evidence)",
                            index + 1
                        ));
                    }
                }
            }
        }
    }
    violations
}

// -------------------------------------------------------------------------
// a fail-closed JSON reader for the frozen report shape
// -------------------------------------------------------------------------

mod mini_json {
    //! A minimal, strict JSON reader (objects, arrays, strings with
    //! escapes, numbers, literals) — enough to read the frozen
    //! `gauntlet.conformance@v1` report without adding a dependency to
    //! this deliberately dependency-light crate. Anything malformed is
    //! an error with the byte offset; there is no lenient mode.

    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        pub fn get(&self, key: &str) -> Option<&Json> {
            let Json::Obj(pairs) = self else {
                return None;
            };
            pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Json::Str(text) => Some(text),
                _ => None,
            }
        }

        pub fn as_arr(&self) -> Option<&[Json]> {
            match self {
                Json::Arr(items) => Some(items),
                _ => None,
            }
        }
    }

    pub fn parse(text: &str) -> Result<Json, String> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value()?;
        parser.whitespace();
        if parser.at != parser.bytes.len() {
            return Err(format!("trailing input at byte {}", parser.at));
        }
        Ok(value)
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        at: usize,
    }

    impl<'a> Parser<'a> {
        fn peek(&self) -> Option<u8> {
            self.bytes.get(self.at).copied()
        }

        fn whitespace(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.at += 1;
            }
        }

        fn expect(&mut self, byte: u8) -> Result<(), String> {
            if self.peek() == Some(byte) {
                self.at += 1;
                Ok(())
            } else {
                Err(format!("expected {:?} at byte {}", byte as char, self.at))
            }
        }

        fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
            if self.bytes[self.at..].starts_with(word.as_bytes()) {
                self.at += word.len();
                Ok(value)
            } else {
                Err(format!("bad literal at byte {}", self.at))
            }
        }

        fn value(&mut self) -> Result<Json, String> {
            self.whitespace();
            match self.peek() {
                None => Err("unexpected end of input".to_owned()),
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => Ok(Json::Str(self.string()?)),
                Some(b't') => self.literal("true", Json::Bool(true)),
                Some(b'f') => self.literal("false", Json::Bool(false)),
                Some(b'n') => self.literal("null", Json::Null),
                Some(_) => self.number(),
            }
        }

        fn object(&mut self) -> Result<Json, String> {
            self.expect(b'{')?;
            let mut pairs = Vec::new();
            self.whitespace();
            if self.peek() == Some(b'}') {
                self.at += 1;
                return Ok(Json::Obj(pairs));
            }
            loop {
                self.whitespace();
                let key = self.string()?;
                self.whitespace();
                self.expect(b':')?;
                let value = self.value()?;
                pairs.push((key, value));
                self.whitespace();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b'}') => {
                        self.at += 1;
                        return Ok(Json::Obj(pairs));
                    }
                    _ => return Err(format!("expected ',' or '}}' at byte {}", self.at)),
                }
            }
        }

        fn array(&mut self) -> Result<Json, String> {
            self.expect(b'[')?;
            let mut items = Vec::new();
            self.whitespace();
            if self.peek() == Some(b']') {
                self.at += 1;
                return Ok(Json::Arr(items));
            }
            loop {
                items.push(self.value()?);
                self.whitespace();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b']') => {
                        self.at += 1;
                        return Ok(Json::Arr(items));
                    }
                    _ => return Err(format!("expected ',' or ']' at byte {}", self.at)),
                }
            }
        }

        fn string(&mut self) -> Result<String, String> {
            self.expect(b'"')?;
            let mut out = String::new();
            loop {
                let byte = self.peek().ok_or("unterminated string")?;
                match byte {
                    b'"' => {
                        self.at += 1;
                        return Ok(out);
                    }
                    b'\\' => {
                        self.at += 1;
                        let escape = self.peek().ok_or("unterminated escape")?;
                        self.at += 1;
                        match escape {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{0008}'),
                            b'f' => out.push('\u{000C}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let hex = self
                                    .bytes
                                    .get(self.at..self.at + 4)
                                    .ok_or("truncated \\u escape")?;
                                let text = std::str::from_utf8(hex)
                                    .map_err(|_| "non-utf8 \\u escape".to_owned())?;
                                let code = u32::from_str_radix(text, 16)
                                    .map_err(|_| "bad \\u escape".to_owned())?;
                                self.at += 4;
                                out.push(char::from_u32(code).ok_or("bad \\u code point")?);
                            }
                            other => {
                                return Err(format!(
                                    "bad escape {:?} at byte {}",
                                    other as char, self.at
                                ));
                            }
                        }
                    }
                    0x00..=0x1F => {
                        return Err(format!(
                            "raw control byte {byte:#04x} at offset {}",
                            self.at
                        ));
                    }
                    _ => {
                        // Copy one UTF-8 scalar wholesale.
                        let start = self.at;
                        let len = utf8_len(byte);
                        let end = start + len;
                        let slice = self
                            .bytes
                            .get(start..end)
                            .ok_or("truncated utf-8 sequence")?;
                        let text = std::str::from_utf8(slice)
                            .map_err(|_| "invalid utf-8 in string".to_owned())?;
                        out.push_str(text);
                        self.at = end;
                    }
                }
            }
        }

        fn number(&mut self) -> Result<Json, String> {
            let start = self.at;
            if self.peek() == Some(b'-') {
                self.at += 1;
            }
            while matches!(
                self.peek(),
                Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
            ) {
                self.at += 1;
            }
            let slice = &self.bytes[start..self.at];
            let text = std::str::from_utf8(slice).map_err(|_| "non-utf8 number")?;
            text.parse::<f64>()
                .map(Json::Num)
                .map_err(|_| format!("bad number {text:?} at byte {start}"))
        }
    }

    fn utf8_len(first: u8) -> usize {
        match first {
            0x00..=0x7F => 1,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            _ => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("gauntlet-arch-tests-docs-consistency");
        std::fs::create_dir_all(&dir).expect("create tmp dir");
        let path = dir.join(name);
        std::fs::write(&path, text).expect("write tmp file");
        path
    }

    const FIXTURE_JSON: &str = r#"{"schema":"gauntlet.conformance@v1","units":[
        {"id":"AK-001","outcome":"PASS","status_category":"implemented_and_proven"},
        {"id":"AK-002","outcome":"PASS","status_category":"implemented_and_proven"},
        {"id":"AK-053","outcome":"PASS","status_category":"implemented_but_provisional"},
        {"id":"AK-067","outcome":"INCONCLUSIVE","status_category":"proposed"}
    ]}"#;

    fn fixture_handoff() -> String {
        "# handoff\n\n\
         ## implemented_and_proven — 2 units\n\
         - AK-001 — the boundary suite\n\
         - AK-002 — the denylist case\n\
         \n\
         ## implemented_but_provisional — 1 unit\n\
         - AK-053 — bounded resume support\n\
         \n\
         ## configured_but_untested — 0 units\n\
         None.\n\
         \n\
         ## proposed — 1 unit\n\
         - AK-067 — INCONCLUSIVE (no_artifact)\n\
         \n\
         ## blocked — 0 units\n\
         None.\n\
         \n\
         ## unknown — 0 units\n\
         No ruler unit landed here. UNK-002 and UNK-003 are fog, not units.\n"
            .to_owned()
    }

    #[test]
    fn the_mini_json_reader_parses_and_refuses() {
        let value =
            mini_json::parse(r#"{"a":[1,-2.5e3],"b":"x\nyA","c":true,"d":null,"e":{"k":"A"}}"#)
                .expect("well-formed document parses");
        assert_eq!(
            value.get("b").and_then(mini_json::Json::as_str),
            Some("x\nyA")
        );
        assert!(mini_json::parse("{\"a\":1} extra").is_err());
        assert!(mini_json::parse("{\"a\":1").is_err());
        assert!(mini_json::parse("{\"a\":[1,]}").is_err());
        assert!(mini_json::parse(r#"{"a":"\q"}"#).is_err());
    }

    #[test]
    fn a_matching_handoff_has_no_violations() {
        let handoff = write_temp("ok-handoff.md", &fixture_handoff());
        let json = write_temp("ok.json", FIXTURE_JSON);
        assert!(
            check_handoff_matches_conformance_at(&handoff, &json).is_empty(),
            "the faithful fixture handoff must be clean"
        );
    }

    #[test]
    fn a_unit_moved_between_sections_is_a_violation() {
        // Tooth T4: move AK-002 from proven to provisional in a COPY.
        let tampered = fixture_handoff().replace(
            "## implemented_and_proven — 2 units\n- AK-001 — the boundary suite\n- AK-002 — the \
             denylist case",
            "## implemented_and_proven — 2 units\n- AK-001 — the boundary suite",
        );
        let tampered = tampered.replace(
            "## implemented_but_provisional — 1 unit\n- AK-053",
            "## implemented_but_provisional — 1 unit\n- AK-002\n- AK-053",
        );
        let handoff = write_temp("moved-handoff.md", &tampered);
        let json = write_temp("moved.json", FIXTURE_JSON);
        let violations = check_handoff_matches_conformance_at(&handoff, &json);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("implemented_and_proven") && v.contains("AK-002")),
            "the moved-out unit must be reported missing: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("implemented_but_provisional") && v.contains("AK-002")),
            "the moved-in unit must be reported extra: {violations:?}"
        );
    }

    #[test]
    fn a_wrong_declared_count_is_a_violation() {
        let tampered = fixture_handoff().replace(
            "## implemented_and_proven — 2 units",
            "## implemented_and_proven — 3 units",
        );
        let handoff = write_temp("count-handoff.md", &tampered);
        let json = write_temp("count.json", FIXTURE_JSON);
        assert!(
            check_handoff_matches_conformance_at(&handoff, &json)
                .iter()
                .any(|v| v.contains("declares 3 unit(s)")),
            "the inflated header count must be reported"
        );
    }

    #[test]
    fn a_missing_or_malformed_input_is_a_violation_never_a_pass() {
        let missing = check_handoff_matches_conformance_at(
            std::path::Path::new("/nonexistent/handoff.md"),
            std::path::Path::new("/nonexistent/conformance.json"),
        );
        assert!(
            missing.len() == 1 && missing[0].contains("conformance report unreadable"),
            "fail-closed on missing json: {missing:?}"
        );
        let handoff = write_temp("ok2-handoff.md", &fixture_handoff());
        let bad_json = write_temp("bad.json", "{\"schema\":\"other@v1\"}");
        assert!(
            check_handoff_matches_conformance_at(&handoff, &bad_json)
                .iter()
                .any(|v| v.contains("rejected") && v.contains("other@v1")),
            "fail-closed on a foreign schema"
        );
    }

    fn claims_dir_with(line: &str) -> std::path::PathBuf {
        claims_dir_with_text(&format!("- {line}\n"))
    }

    /// A throwaway docs tree. Each call gets its OWN directory (pid +
    /// nanos): nextest runs each test in a separate process, so a shared
    /// fixture path would let concurrent tests overwrite each other's
    /// fixture docs mid-check.
    fn claims_dir_with_text(text: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("past the epoch")
            .as_nanos();
        let dir = std::env::temp_dir()
            .join(format!(
                "gauntlet-arch-tests-docs-consistency-{}-{nanos}",
                std::process::id()
            ))
            .join("docs");
        std::fs::create_dir_all(&dir).expect("create tmp docs dir");
        for name in DOC_SET {
            std::fs::write(dir.join(name), "## Proven vs provisional\n").expect("write doc");
        }
        std::fs::write(dir.join("ARCHITECTURE.md"), text).expect("write doc");
        dir
    }

    #[test]
    fn a_claim_ahead_of_the_conformance_report_is_a_violation() {
        let json = write_temp("ahead.json", FIXTURE_JSON);
        // AK-053 is provisional in the json: claiming it proven is ahead.
        let ahead = claims_dir_with("implemented_and_proven — AK-053: prose");
        assert!(
            check_docs_claims_not_ahead_at(&ahead, &json)
                .iter()
                .any(|v| v.contains("ahead of evidence") && v.contains("AK-053")),
            "the ahead claim must be reported"
        );
        // AK-001 is proven: claiming it provisional is BEHIND — safe.
        let behind = claims_dir_with("implemented_but_provisional — AK-001: prose");
        assert!(
            check_docs_claims_not_ahead_at(&behind, &json).is_empty(),
            "a behind-evidence claim is admissible"
        );
        // AK-067 is proposed in the json: only `proposed` claims it.
        let exact = claims_dir_with("proposed — AK-067: prose");
        assert!(check_docs_claims_not_ahead_at(&exact, &json).is_empty());
        let wrong = claims_dir_with("configured_but_untested — AK-067: prose");
        assert!(
            check_docs_claims_not_ahead_at(&wrong, &json)
                .iter()
                .any(|v| v.contains("AK-067") && v.contains("ahead of evidence"))
        );
        // A unit the json does not carry: fail closed.
        let unknown = claims_dir_with("implemented_and_proven — AK-099: prose");
        assert!(
            check_docs_claims_not_ahead_at(&unknown, &json)
                .iter()
                .any(|v| v.contains("AK-099") && v.contains("carries no such unit"))
        );
    }

    /// COR-B3 (falsifier F8): the five executed evasion shapes — leading
    /// capital, ALL CAPS, hyphen separator, claim split across two lines,
    /// lowercase unit id — are ALL claims, subject to the same
    /// ahead-of-evidence rule as the canonical line shape. Each fixture
    /// claims AK-053 `implemented_and_proven` while the fixture json
    /// holds it `implemented_but_provisional`: every shape must report.
    #[test]
    fn the_five_executed_evasion_shapes_are_claims() {
        let json = write_temp("evasion.json", FIXTURE_JSON);
        let evasions = [
            (
                "- Implemented_and_proven — AK-053: prose\n",
                "leading capital",
            ),
            (
                "- IMPLEMENTED_AND_PROVEN — AK-053: prose\n",
                "all-caps status word",
            ),
            (
                "- implemented_and_proven - AK-053: prose\n",
                "hyphen separator",
            ),
            (
                "- implemented_and_proven:\nAK-053 — prose continuation\n",
                "claim split across two lines",
            ),
            (
                "- implemented_and_proven — ak-053: prose\n",
                "lowercase unit id",
            ),
        ];
        for (text, label) in evasions {
            let dir = claims_dir_with_text(text);
            assert!(
                check_docs_claims_not_ahead_at(&dir, &json)
                    .iter()
                    .any(|v| v.contains("AK-053") && v.contains("ahead of evidence")),
                "the {label} laundering shape must be reported ahead of evidence"
            );
        }
        // The behind-evidence direction stays admissible under variant
        // spellings too: claiming the PROVEN AK-001 provisional is safe.
        let behind = claims_dir_with_text("- IMPLEMENTED_BUT_PROVISIONAL — ak-001: prose\n");
        assert!(
            check_docs_claims_not_ahead_at(&behind, &json).is_empty(),
            "a behind-evidence claim in variant casing/shape is admissible"
        );
    }

    /// COR-B3 over-rejection guard: the fallback window keys on the six
    /// §26 vocabulary WORDS, never on verbs — a unit id sitting beside
    /// "implements"/"delivered"/bare "implemented" with no vocabulary
    /// token in the window is prose, not a claim.
    #[test]
    fn a_unit_id_near_a_verb_is_not_a_claim_without_a_vocabulary_word() {
        let json = write_temp("verbs.json", FIXTURE_JSON);
        let dir = claims_dir_with_text(
            "The runtime implements the precedence merge and delivers it — AK-001: prose\n\
             AK-002 sits in ordinary prose nearby, configured and operating\n\
             implemented (bare token, no category word) — AK-053: prose\n",
        );
        assert!(
            check_docs_claims_not_ahead_at(&dir, &json).is_empty(),
            "verbs and bare status-like tokens never bind a claim; only the six §26 \
             vocabulary words do"
        );
    }

    #[test]
    fn the_claim_line_parser_takes_exactly_the_established_shape() {
        assert_eq!(
            parse_claim("- implemented_but_provisional — AK-060: prose"),
            Some(ClaimLine {
                status: "implemented_but_provisional",
                id: "AK-060"
            })
        );
        // Not a claim line: no leading bullet, no colon, no em-dash.
        assert_eq!(
            parse_claim("implemented_but_provisional — AK-060: prose"),
            None
        );
        assert_eq!(
            parse_claim("- implemented_but_provisional - AK-060: prose"),
            None
        );
        assert_eq!(parse_claim("- proposed — AK-NNN: template"), None);
        // The id must be the whole token before the colon.
        assert_eq!(parse_claim("- proposed — AK-067 extra: prose"), None);
    }

    #[test]
    fn unit_id_scanning_is_word_delimited() {
        assert_eq!(
            unit_ids_on("- AK-001 — AK-002: two"),
            vec!["AK-001", "AK-002"]
        );
        assert!(unit_ids_on("UNK-003 stays a fog item").is_empty());
        assert!(unit_ids_on("the AK-NNN template literal").is_empty());
        assert_eq!(unit_ids_on("see (AK-067)."), vec!["AK-067"]);
    }
}
