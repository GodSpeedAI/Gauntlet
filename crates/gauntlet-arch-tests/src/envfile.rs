//! AK-019 instrument — `.env.example` completeness (recritique R11/R15/R27).
//!
//! Locator: `tests/contract/env_example_complete`.
//!
//! Until now nothing in the repository read `.env.example`. Its content
//! happened to be right; deleting a line or pasting a live key would not have
//! failed a single gate. This module is the missing producer: it parses
//! **Annex E** out of `.agents/specs/Core-spec.md` — the spec is the source of
//! truth, not a second copy of the list kept here — and diffs it against the
//! file.
//!
//! It asserts, per AK-019 and spec §8.1:
//!
//! 1. every Annex E variable appears in `.env.example`;
//! 2. every `.env.example` key appears in Annex E, or is a NAMED exclusion
//!    (see [`RUNTIME_INJECTED`]);
//! 3. every value is empty ("a `.env.example` with empty values is REQUIRED");
//! 4. no value is credential-shaped, so a mistake is reported as a *secret*
//!    rather than as a generic non-empty value;
//! 5. no key is declared twice;
//! 6. the parse is non-vacuous — an Annex E that yielded nothing would
//!    otherwise make every check above pass emptily.
//!
//! # `GAUNTLET_RESULT_PATH` — a named, justified exclusion
//!
//! `GAUNTLET_RESULT_PATH` is a recognized `GAUNTLET_` variable (spec §9 line
//! "the runtime creates the invocation directory and passes the path in the
//! prompt and the environment", §10.3 result contract, Annex C role prompt).
//! It is **runtime-injected into each child invocation**, not operator
//! configuration: the parent computes it per invocation, and an operator who
//! set it in `.env` would corrupt every child's result contract. It is
//! therefore correctly absent from Annex E and from `.env.example`.
//!
//! That absence is encoded, not assumed: [`check_env_example`] asserts the
//! variable is missing from BOTH. If a future spec revision adds it to Annex
//! E, this check fails and forces the decision to be made again in the open,
//! instead of the omission quietly becoming a gap.

/// Variables that are runtime-injected per invocation rather than operator
/// configuration, with the spec reference that justifies the exclusion.
pub const RUNTIME_INJECTED: &[(&str, &str)] = &[(
    "GAUNTLET_RESULT_PATH",
    "spec §10.3 + Annex C: written per child invocation by the runtime; an \
     operator-set value would corrupt the result contract",
)];

/// Fragments that mark a value as credential-shaped.
const CREDENTIAL_SHAPES: &[&str] = &[
    "sk-",
    "sk_",
    "ghp_",
    "gho_",
    "github_pat_",
    "akia",
    "asia",
    "xox",
    "bearer ",
    "-----begin",
    "aiza",
    "glpat-",
    "npm_",
    "pypi-",
];

/// Extract every `GAUNTLET_*` variable named in Annex E of the spec.
pub fn annex_e_variables(spec: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    for line in spec.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# Annex E") {
            inside = true;
            continue;
        }
        if inside && trimmed.starts_with("# Annex ") {
            break;
        }
        if !inside {
            continue;
        }
        for name in gauntlet_tokens(line) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

/// Every `GAUNTLET_[A-Z0-9_]+` token in a line.
fn gauntlet_tokens(line: &str) -> Vec<String> {
    const PREFIX: &str = "GAUNTLET_";
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(found) = line[from..].find(PREFIX) {
        let start = from + found;
        let mut end = start + PREFIX.len();
        while end < bytes.len()
            && (bytes[end].is_ascii_uppercase()
                || bytes[end] == b'_'
                || bytes[end].is_ascii_digit())
        {
            end += 1;
        }
        let name = line[start..end].trim_end_matches('_').to_string();
        if name.len() > PREFIX.len() {
            out.push(name);
        }
        from = end.max(start + 1);
    }
    out
}

/// One `.env.example` entry.
#[derive(Debug, Clone)]
pub struct EnvEntry {
    pub line: usize,
    pub key: String,
    pub value: String,
}

/// Parse `.env.example` into its entries, ignoring blank lines and comments.
pub fn parse_env_example(text: &str) -> Vec<EnvEntry> {
    text.lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some(EnvEntry {
                line: index + 1,
                key: key.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

/// Diff Annex E against `.env.example` (AK-019). `spec` and `env` are the two
/// files' full texts.
pub fn check_env_example(spec: &str, env: &str) -> Vec<String> {
    let mut out = Vec::new();
    let annex = annex_e_variables(spec);
    let entries = parse_env_example(env);

    // Non-vacuity first: an empty parse on either side would make every
    // difference check below pass by saying nothing.
    if annex.len() < 50 {
        out.push(format!(
            "env-example: .agents/specs/Core-spec.md: Annex E parsed to only {} variables — the \
             annex parser is broken, and an empty expectation is not a passing one",
            annex.len()
        ));
        return out;
    }
    if entries.len() < 50 {
        out.push(format!(
            "env-example: .env.example: only {} entries parsed — the file or the parser is \
             broken",
            entries.len()
        ));
        return out;
    }

    for (name, justification) in RUNTIME_INJECTED {
        if annex.iter().any(|v| v == name) {
            out.push(format!(
                "env-example: .agents/specs/Core-spec.md: '{name}' is now listed in Annex E, but \
                 this instrument excludes it as runtime-injected ({justification}). The exclusion \
                 must be re-decided in the open rather than left stale"
            ));
        }
        if let Some(entry) = entries.iter().find(|e| e.key == *name) {
            out.push(format!(
                "env-example: .env.example:{}: '{name}' is runtime-injected, not operator \
                 configuration ({justification}) — an operator-set value would corrupt the result \
                 contract",
                entry.line
            ));
        }
    }

    for expected in &annex {
        if !entries.iter().any(|e| e.key == *expected) {
            out.push(format!(
                "env-example: .env.example: missing '{expected}', which spec Annex E lists as a \
                 recognized GAUNTLET_ variable (AK-019)"
            ));
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    for entry in &entries {
        if !entry.key.starts_with("GAUNTLET_") {
            out.push(format!(
                "env-example: .env.example:{}: '{}' is not a GAUNTLET_ variable — this file \
                 documents exactly the runtime's own surface",
                entry.line, entry.key
            ));
            continue;
        }
        if seen.contains(&entry.key.as_str()) {
            out.push(format!(
                "env-example: .env.example:{}: '{}' is declared twice",
                entry.line, entry.key
            ));
        }
        seen.push(entry.key.as_str());

        let is_known = annex.contains(&entry.key)
            || RUNTIME_INJECTED.iter().any(|(name, _)| *name == entry.key);
        if !is_known {
            out.push(format!(
                "env-example: .env.example:{}: '{}' is not listed in spec Annex E — either the \
                 annex or this file is stale, and AK-019 requires them to agree",
                entry.line, entry.key
            ));
        }

        if !entry.value.is_empty() {
            let lowered = entry.value.to_lowercase();
            let credential = CREDENTIAL_SHAPES
                .iter()
                .find(|shape| lowered.contains(**shape));
            match credential {
                Some(shape) => out.push(format!(
                    "env-example: .env.example:{}: '{}' carries a credential-shaped value \
                     (matches '{shape}') — generated files never contain credentials (spec §8.1)",
                    entry.line, entry.key
                )),
                None => out.push(format!(
                    "env-example: .env.example:{}: '{}' has a non-empty value — AK-019 and spec \
                     §8.1 require '.env.example' to list every variable with EMPTY values",
                    entry.line, entry.key
                )),
            }
        }
    }
    out
}

#[cfg(test)]
mod repo {
    #[test]
    fn env_example_matches_annex_e() {
        let violations = crate::check_env_example(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("env-example", &violations)
        );
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    /// A miniature Annex E with enough variables to clear the non-vacuity
    /// floor, so the difference checks are what the fixtures exercise.
    fn spec_fixture() -> String {
        let mut names: Vec<String> = (0..60).map(|i| format!("`GAUNTLET_VAR_{i}`")).collect();
        names.push("`GAUNTLET_OTEL_ENDPOINT`".to_string());
        format!(
            "# Annex D — something else\n\n`GAUNTLET_NOT_IN_ANNEX_E`\n\n# Annex E — Environment \
             variable reference\n\n| Group | Variables |\n|---|---|\n| All | {} |\n\n# Annex F — \
             later\n\n`GAUNTLET_ALSO_NOT_IN_ANNEX_E`\n",
            names.join(" ")
        )
    }

    fn env_fixture() -> String {
        let mut out = String::from("# comment line\n\n");
        for i in 0..60 {
            out.push_str(&format!("GAUNTLET_VAR_{i}=\n"));
        }
        out.push_str("GAUNTLET_OTEL_ENDPOINT=\n");
        out
    }

    #[test]
    fn annex_e_is_parsed_and_bounded_to_the_annex() {
        let vars = annex_e_variables(&spec_fixture());
        assert_eq!(vars.len(), 61, "{vars:?}");
        assert!(vars.contains(&"GAUNTLET_OTEL_ENDPOINT".to_string()));
        assert!(
            !vars.contains(&"GAUNTLET_NOT_IN_ANNEX_E".to_string()),
            "the parser must not read past the annex boundaries"
        );
        assert!(!vars.contains(&"GAUNTLET_ALSO_NOT_IN_ANNEX_E".to_string()));
    }

    #[test]
    fn a_matching_pair_is_clean() {
        let violations = check_env_example(&spec_fixture(), &env_fixture());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_deleted_variable_is_flagged() {
        // Recritique R11/R15/R27's exact evasion.
        let env = env_fixture().replace("GAUNTLET_OTEL_ENDPOINT=\n", "");
        let violations = check_env_example(&spec_fixture(), &env);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("missing 'GAUNTLET_OTEL_ENDPOINT'"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn a_credential_value_is_flagged_as_a_credential() {
        // Recritique R11/R15's exact evasion.
        for (value, needle) in [
            ("sk-live-abc123", "credential-shaped"),
            ("sk-ant-api03-REAL-KEY", "credential-shaped"),
            ("worktree", "non-empty value"),
        ] {
            let env = env_fixture().replace(
                "GAUNTLET_OTEL_ENDPOINT=\n",
                &format!("GAUNTLET_OTEL_ENDPOINT={value}\n"),
            );
            let violations = check_env_example(&spec_fixture(), &env);
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{value} -> {violations:?}"
            );
        }
    }

    #[test]
    fn an_unknown_or_duplicated_key_is_flagged() {
        let mut env = env_fixture();
        env.push_str("GAUNTLET_INVENTED=\nGAUNTLET_VAR_0=\n");
        let violations = check_env_example(&spec_fixture(), &env);
        assert!(
            violations.iter().any(|v| v.contains("GAUNTLET_INVENTED")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("declared twice")),
            "{violations:?}"
        );
    }

    #[test]
    fn the_runtime_injected_exclusion_is_named_and_enforced_both_ways() {
        // Present in .env.example: a violation, with the justification.
        let mut env = env_fixture();
        env.push_str("GAUNTLET_RESULT_PATH=\n");
        let violations = check_env_example(&spec_fixture(), &env);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("GAUNTLET_RESULT_PATH") && v.contains("runtime-injected")),
            "{violations:?}"
        );

        // Added to Annex E: also a violation, so the exclusion cannot go
        // stale silently.
        let spec = spec_fixture().replace("| All | ", "| All | `GAUNTLET_RESULT_PATH` ");
        let violations = check_env_example(&spec, &env_fixture());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("now listed in Annex E")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_vacuous_parse_is_a_violation() {
        let violations = check_env_example("# Annex E — nothing here\n", &env_fixture());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("Annex E parsed to only"),
            "{violations:?}"
        );
    }
}
