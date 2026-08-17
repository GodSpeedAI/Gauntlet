//! Environment layer — spec §8.3 + Annex E, the `.env`-under-process fold
//! (plan Task 5 decision D9).
//!
//! Spec §8.1: "`built-in defaults` < `GAUNTLET.md` front matter < `.env` /
//! process environment < CLI flags. Later wins." This module is the third
//! layer's inside fold: a `.env` file at the project root is read first and
//! the process environment writes over it, so a process export always beats
//! the file. Values are raw strings — typed conversion happens in
//! [`crate::config::resolve`] against the known-key table.
//!
//! Unknown `GAUNTLET_*` names are never silent and never fatal: Annex E is a
//! closed vocabulary ("Full table in Annex E"), so a `GAUNTLET_*` name
//! outside it is collected with its source and reported by `gauntlet doctor`
//! exactly like an unknown front-matter key — a typo surface, not a crash.
//! The namespace check is case-insensitive: a lowercase `gauntlet_*` name
//! resolves nothing (the canonical table is uppercase) and is reported as an
//! unknown name rather than silently ignored.
//!
//! The recognized-name vocabulary is SUPPLIED BY THE CALLER (normally the
//! composition root) rather than spelled here: the boundary suite derives
//! adapter identifiers from the workspace members and denies their
//! spellings as substrings in this layer's crates, and Annex E contains one
//! variable name that collides with a derived spelling. Vocabulary lives
//! where naming adapters is the declared job; the application layer only
//! consumes it.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::defaults;

/// Where a winning environment value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvSource {
    /// The `.env` file at the project root.
    DotEnv,
    /// The process environment (wins over the file).
    Process,
}

/// A `GAUNTLET_*` name outside Annex E, with where it was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEnv {
    pub name: String,
    pub source: EnvSource,
}

/// The merged third layer: process environment over `.env`. Values are kept
/// verbatim, including empties; which names are empty is the resolve
/// layer's business (an empty override is "not set", an empty secret
/// variable is an honest empty).
#[derive(Debug, Clone, Default)]
pub struct EnvLayer {
    merged: BTreeMap<String, String>,
    sources: BTreeMap<String, EnvSource>,
    unknown: Vec<UnknownEnv>,
}

impl EnvLayer {
    /// Builds the layer from its two sources and the recognized-name
    /// vocabulary (Annex E; supplied by the composition root). `process`
    /// wins over the dotenv text for the same name.
    pub fn from_parts(
        dotenv_text: &str,
        process_vars: &[(String, String)],
        recognized: &[&str],
    ) -> Self {
        let mut layer = Self::default();
        for (name, value) in parse_dotenv(dotenv_text) {
            layer.merged.insert(name.clone(), value);
            layer.sources.insert(name.clone(), EnvSource::DotEnv);
            if is_gauntlet_name(&name) && !recognized.contains(&name.as_str()) {
                layer.push_unknown(name, EnvSource::DotEnv);
            }
        }
        for (name, value) in process_vars {
            if name.is_empty() {
                continue;
            }
            layer.merged.insert(name.clone(), value.clone());
            layer.sources.insert(name.clone(), EnvSource::Process);
            if is_gauntlet_name(name) && !recognized.contains(&name.as_str()) {
                layer.push_unknown(name.clone(), EnvSource::Process);
            }
        }
        layer
    }

    /// Collects the layer from the real host: `.env` at `project_root` (a
    /// missing or unreadable file is an absent dotenv, not an error — the
    /// file is optional) under the process environment.
    pub fn collect(project_root: &Path, recognized: &[&str]) -> Self {
        let dotenv_text =
            std::fs::read_to_string(project_root.join(defaults::ENV_FILE_NAME)).unwrap_or_default();
        let process: Vec<(String, String)> = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        Self::from_parts(&dotenv_text, &process, recognized)
    }

    /// The winning raw value for `name`, if the layer carries one.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.merged.get(name).map(|value| value.as_str())
    }

    /// Which source the winning value for `name` came from.
    pub fn source_of(&self, name: &str) -> Option<EnvSource> {
        self.sources.get(name).copied()
    }

    /// Every `GAUNTLET_*` name this layer carries, sorted — evidence text
    /// names VARIABLES, never values (§8.1 secrets rule).
    pub fn gauntlet_names(&self) -> Vec<&str> {
        self.merged
            .keys()
            .map(|name| name.as_str())
            .filter(|name| is_gauntlet_name(name))
            .collect()
    }

    /// The collected unknown `GAUNTLET_*` names (outside Annex E).
    pub fn unknown(&self) -> &[UnknownEnv] {
        &self.unknown
    }

    fn push_unknown(&mut self, name: String, source: EnvSource) {
        if !self
            .unknown
            .iter()
            .any(|existing| existing.name == name && existing.source == source)
        {
            self.unknown.push(UnknownEnv { name, source });
        }
    }
}

/// Whether a name belongs to the `GAUNTLET_*` namespace Annex E governs.
/// The prefix is matched case-insensitively: the canonical table is
/// uppercase, so a lowercase `gauntlet_*` spelling resolves nothing — and it
/// is REPORTED as an unknown name here instead of passing through silently.
fn is_gauntlet_name(name: &str) -> bool {
    const PREFIX: &str = "GAUNTLET_";
    name.len() >= PREFIX.len()
        && name.is_char_boundary(PREFIX.len())
        && name[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
}

/// Parses dotenv text: `KEY=VALUE` lines (with the standard `export `
/// prefix accepted), `#` full-line comments, no interpolation, matching
/// surrounding quotes stripped, and — in UNQUOTED values only — whitespace
/// followed by `#` starting a trailing comment that is stripped. Lines that
/// are blank, comments, or carry no `=` / an empty key are skipped — `.env`
/// is a loose host convention, not a typed contract; the typed contract is
/// GAUNTLET.md (§8.2) and this layer's unknown-name report.
pub fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, value)) = body.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.bytes().all(is_env_byte) {
            continue;
        }
        out.push((key.to_owned(), dotenv_value(value)));
    }
    out
}

fn is_env_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// One dotenv value. A quoted value keeps `#` and whitespace literally and
/// ends at its closing quote; an unquoted value ends at a `#` preceded by
/// whitespace (`KEY=x # note` is `x`, not a value carrying the note), and a
/// `#` glued to text is part of the value.
fn dotenv_value(raw: &str) -> String {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if matches!(bytes.first(), Some(b'"') | Some(b'\'')) {
        let quote = char::from(bytes[0]);
        if let Some(close) = trimmed[1..].find(quote) {
            return trimmed[1..1 + close].to_owned();
        }
        // Unterminated quote: verbatim — `.env` is a loose convention.
        return trimmed.to_owned();
    }
    let cut = inline_comment_start(trimmed);
    trimmed[..cut].trim_end().to_owned()
}

/// Byte offset where an unquoted value's trailing comment begins: a `#` at
/// the start or preceded by a space or tab.
fn inline_comment_start(value: &str) -> usize {
    let bytes = value.as_bytes();
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && (offset == 0 || matches!(bytes[offset - 1], b' ' | b'\t')) {
            return offset;
        }
    }
    value.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOCAB: &[&str] = &[
        "GAUNTLET_MAX_ROUNDS",
        "GAUNTLET_ANSWER_KEY_PATH",
        "GAUNTLET_CLOCK",
        "GAUNTLET_ID_SEED",
    ];

    #[test]
    fn the_process_environment_wins_over_the_dotenv_file() {
        let dotenv = "GAUNTLET_MAX_ROUNDS=5\nGAUNTLET_ANSWER_KEY_PATH=from-file\n";
        let process = vec![("GAUNTLET_MAX_ROUNDS".to_owned(), "7".to_owned())];
        let layer = EnvLayer::from_parts(dotenv, &process, VOCAB);
        assert_eq!(layer.value("GAUNTLET_MAX_ROUNDS"), Some("7"));
        assert_eq!(
            layer.source_of("GAUNTLET_MAX_ROUNDS"),
            Some(EnvSource::Process)
        );
        assert_eq!(layer.value("GAUNTLET_ANSWER_KEY_PATH"), Some("from-file"));
        assert_eq!(
            layer.source_of("GAUNTLET_ANSWER_KEY_PATH"),
            Some(EnvSource::DotEnv)
        );
    }

    #[test]
    fn unknown_gauntlet_names_are_collected_with_their_source() {
        let dotenv = "GAUNTLET_TYPO_ROUNDZ=4\n";
        let process = vec![
            ("GAUNTLET_MAX_ROUNDS".to_owned(), "9".to_owned()),
            ("GAUNTLET_MIS SPELT".to_owned(), String::new()),
            ("GAUNTLET_NOT_IN_ANNEX_E".to_owned(), String::new()),
        ];
        let layer = EnvLayer::from_parts(dotenv, &process, VOCAB);
        let names: Vec<&str> = layer
            .unknown()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert!(names.contains(&"GAUNTLET_TYPO_ROUNDZ"));
        assert!(names.contains(&"GAUNTLET_NOT_IN_ANNEX_E"));
        // Recognized names — including the test hooks — are not unknown.
        assert!(!names.contains(&"GAUNTLET_MAX_ROUNDS"));
        assert!(!layer.gauntlet_names().contains(&"GAUNTLET_CLOCK"));
        let dotenv_hit = layer
            .unknown()
            .iter()
            .find(|entry| entry.name == "GAUNTLET_TYPO_ROUNDZ")
            .expect("collected");
        assert_eq!(dotenv_hit.source, EnvSource::DotEnv);
    }

    #[test]
    fn dotenv_parsing_strips_quotes_and_skips_comments() {
        let text = "# a comment\n\nQUOTED=\"with spaces\"\nSINGLE='kept'\nBARE=plain\n\
                    EMPTY=\nNO_EQUALS_LINE\n=NONAME\nSPACED KEY = x\n";
        let pairs = parse_dotenv(text);
        let lookup = |key: &str| {
            pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(lookup("QUOTED"), Some("with spaces"));
        assert_eq!(lookup("SINGLE"), Some("kept"));
        assert_eq!(lookup("BARE"), Some("plain"));
        assert_eq!(lookup("EMPTY"), Some(""));
        // Malformed lines are skipped, not parsed into nonsense.
        assert_eq!(pairs.len(), 4);
    }

    #[test]
    fn export_prefixes_apply_and_inline_comments_strip_from_unquoted_values() {
        let text = "export GAUNTLET_MAX_ROUNDS=5\n\
                    export GAUNTLET_ANSWER_KEY_PATH=from-file # trailing note\n\
                    NOTE=kept # a comment\nGLUED=note#kept\nQUOTED=\"a # b\" # after\n";
        let layer = EnvLayer::from_parts(text, &[], VOCAB);
        // The `export ` prefix is the standard dotenv spelling: it applies.
        assert_eq!(layer.value("GAUNTLET_MAX_ROUNDS"), Some("5"));
        assert_eq!(
            layer.source_of("GAUNTLET_MAX_ROUNDS"),
            Some(EnvSource::DotEnv)
        );
        // Unquoted values end at a whitespace-preceded `#`...
        assert_eq!(layer.value("GAUNTLET_ANSWER_KEY_PATH"), Some("from-file"));
        assert_eq!(layer.value("NOTE"), Some("kept"));
        // ...a `#` glued to text is part of the value...
        assert_eq!(layer.value("GLUED"), Some("note#kept"));
        // ...and quoted values keep `#` literally.
        assert_eq!(layer.value("QUOTED"), Some("a # b"));
        // A key literally named `export` is not a prefix problem.
        let pairs = parse_dotenv("export=plain");
        assert_eq!(pairs, vec![("export".to_owned(), "plain".to_owned())]);
    }

    #[test]
    fn lowercase_gauntlet_names_are_reported_not_silently_inert() {
        let text = "gauntlet_max_rounds=9\n";
        let layer = EnvLayer::from_parts(text, &[], VOCAB);
        // It resolves nothing (the canonical table is uppercase)...
        assert_eq!(
            layer.value("GAUNTLET_MAX_ROUNDS"),
            None,
            "a lowercase name must not address an Annex E variable"
        );
        // ...and it is collected into the unknown set with its source.
        let names: Vec<&str> = layer
            .unknown()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert!(names.contains(&"gauntlet_max_rounds"), "{names:?}");
        let hit = layer
            .unknown()
            .iter()
            .find(|entry| entry.name == "gauntlet_max_rounds")
            .expect("collected");
        assert_eq!(hit.source, EnvSource::DotEnv);
    }
}
