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
//!
//! # Recorded residuals (critic-4)
//!
//! - [`RUNTIME_INJECTED`] is pinned to exactly its one documented entry by
//!   test. Growth converts a spec-recognized variable into an unlisted one
//!   with the AK-019 locator green (vacuity M4), so it must be a deliberate
//!   edit that updates the pin.
//! - The derived non-vacuity floor counts `GAUNTLET_` tokens named ANYWHERE
//!   in the spec. A variable named ONLY in Annex E (the Promotion and
//!   Test-hook groups today) shrinks the floor too when it leaves the annex,
//!   so an annex + `.env.example` lockstep deletion reads as clean (vacuity
//!   m2). Deleting spec text is moving the ruler, not evading the suite —
//!   `just ruler` catches it — but the floor is honestly §8.3-scoped, and
//!   that is now written down rather than implied.

/// Variables that are runtime-injected per invocation rather than operator
/// configuration, with the spec reference that justifies the exclusion.
pub const RUNTIME_INJECTED: &[(&str, &str)] = &[(
    "GAUNTLET_RESULT_PATH",
    "spec §10.3 + Annex C: written per child invocation by the runtime; an \
     operator-set value would corrupt the result contract",
)];

/// Fragments that mark a value as credential-shaped. `eyj` covers committed
/// JWTs, whose header segment is the base64 of `{"alg":…}` and matches no
/// keyed prefix at all (critic-4 m1).
const CREDENTIAL_SHAPES: &[&str] = &[
    "sk-",
    "sk_",
    "ghp_",
    "gho_",
    "github_pat_",
    "akia",
    "asia",
    "eyj",
    "xox",
    "bearer ",
    "-----begin",
    "aiza",
    "glpat-",
    "npm_",
    "pypi-",
];

/// The first credential shape occurring at a non-alphanumeric boundary.
///
/// The boundary requirement is what makes the shape list safe to run over raw
/// text rather than only over parsed values: `GAUNTLET_TASK_TIMEOUT` contains
/// `sk_`, and `disk-usage` contains `sk-`, but neither starts one.
fn credential_shape_at_boundary(lowered: &str) -> Option<&'static str> {
    let bytes = lowered.as_bytes();
    for shape in CREDENTIAL_SHAPES {
        let mut from = 0usize;
        while let Some(found) = lowered[from..].find(shape) {
            let at = from + found;
            if at == 0 || !bytes[at - 1].is_ascii_alphanumeric() {
                return Some(shape);
            }
            from = at + 1;
        }
    }
    None
}

/// Credential-shaped material anywhere in the RAW file text, comments
/// included (round-3 V10).
///
/// `parse_env_example` drops every `#` line before any check runs, so
/// `# GAUNTLET_ANTHROPIC_KEY=sk-ant-api03-REAL` committed a live key with the
/// gate green: the line produced no entry, so `CREDENTIAL_SHAPES` was never
/// applied to it, and it was not an "unknown key" either, because commented
/// lines did not exist to this instrument. AK-019's "no credentials" clause is
/// about the FILE, not about the parsed values.
pub fn raw_credentials(env: &str) -> Vec<String> {
    env.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let shape = credential_shape_at_boundary(&line.to_lowercase())?;
            Some(format!(
                "env-example: .env.example:{}: the line carries credential-shaped material \
                 (matches '{shape}') — generated files never contain credentials (spec §8.1), and \
                 commenting a key out does not un-commit it (AK-019)",
                index + 1
            ))
        })
        .collect()
}

/// Commented lines shaped like an entry (`# GAUNTLET_KEY=…`) that carry a
/// NON-EMPTY value (critic-4 m1).
///
/// The shape-list scan above catches prefixed tokens; it cannot catch a live
/// JWT (no known prefix) or a bare hex/base64 token. But a comment that names
/// an environment key and assigns it a value is not prose — it is an entry in
/// the comment channel, and generated files carry no values there either. A
/// commented entry with an EMPTY value stays clean.
fn commented_entry_values(env: &str) -> Vec<String> {
    env.lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let trimmed = raw.trim_start();
            let body = trimmed.strip_prefix('#')?.trim_start();
            if body.starts_with(|b: char| !b.is_ascii_uppercase()) {
                return None;
            }
            let (key, value) = body.split_once('=')?;
            let key_is_shaped = !key.is_empty()
                && key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            if !key_is_shaped || value.trim().is_empty() {
                return None;
            }
            Some(format!(
                "env-example: .env.example:{}: the commented entry '{}=…' carries a value — \
                 commenting an entry out does not un-commit it, and generated files carry no \
                 values, commented or not (AK-019, spec §8.1)",
                index + 1,
                key
            ))
        })
        .collect()
}

/// Every `GAUNTLET_*` token named ANYWHERE in the spec.
///
/// Round-3 V12: the AK-019 non-vacuity floor used to be the constant 50
/// against an actual 92, so 42 variables could be deleted from Annex E and
/// `.env.example` in lockstep with the gate still green. The floor is now
/// derived from this set — the spec names its own configuration surface in
/// §8.2–§8.6 as well as in the annex, so deleting a group from the annex
/// leaves the floor where it was and the deletion fails.
pub fn spec_gauntlet_variables(spec: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in spec.lines() {
        for name in gauntlet_tokens(line) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

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

    // Credential shapes are checked over the RAW file, comments included, and
    // before anything else: a key committed in a comment is committed
    // (round-3 V10). Commented entries carrying values are the un-prefixed
    // half of the same channel (critic-4 m1).
    out.extend(raw_credentials(env));
    out.extend(commented_entry_values(env));

    // Non-vacuity first, against a DERIVED floor: the number of distinct
    // `GAUNTLET_` variables the spec names anywhere, minus the ones this
    // instrument excludes by name as runtime-injected (round-3 V12). An empty
    // parse on either side would otherwise make every difference check below
    // pass by saying nothing — and so would deleting a group from both files
    // in lockstep.
    let spec_wide = spec_gauntlet_variables(spec);
    if spec_wide.is_empty() {
        out.push(
            "env-example: .agents/specs/Core-spec.md: the spec names no GAUNTLET_ variables at \
             all — the parser is broken, and an empty expectation is not a passing one"
                .to_string(),
        );
        return out;
    }
    let expected: Vec<&String> = spec_wide
        .iter()
        .filter(|name| {
            !RUNTIME_INJECTED
                .iter()
                .any(|(excluded, _)| excluded == name)
        })
        .collect();
    let floor = expected.len();
    let missing_from_annex: Vec<&&String> = expected
        .iter()
        .filter(|name| !annex.contains(**name))
        .collect();
    if !missing_from_annex.is_empty() {
        out.push(format!(
            "env-example: .agents/specs/Core-spec.md: Annex E lists {} variables but the spec \
             names {floor}; missing from the annex: {:?} — Annex E is what AK-019 diffs \
             .env.example against, so a variable the annex has lost is a variable nothing checks, \
             and a shrunken expectation is not a passing one",
            annex.len(),
            missing_from_annex
        ));
        return out;
    }
    // On the `.env.example` side the completeness guarantee is per-variable
    // (every annex name must appear, below), so the only vacuity that check
    // cannot see is an empty parse.
    if entries.is_empty() {
        out.push(format!(
            "env-example: .env.example: no entries parsed against a spec surface of {floor} \
             variables — the file or the parser is broken, and an empty file cannot satisfy \
             AK-019"
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

    /// A miniature spec whose whole `GAUNTLET_` surface lives in Annex E, so
    /// the derived floor is satisfied and the difference checks are what the
    /// fixtures exercise.
    fn spec_fixture() -> String {
        let mut names: Vec<String> = (0..60).map(|i| format!("`GAUNTLET_VAR_{i}`")).collect();
        names.push("`GAUNTLET_OTEL_ENDPOINT`".to_string());
        format!(
            "# Annex D — something else\n\nno variables here\n\n# Annex E — Environment \
             variable reference\n\n| Group | Variables |\n|---|---|\n| All | {} |\n\n# Annex F — \
             later\n\nnor here\n",
            names.join(" ")
        )
    }

    /// The same shape, but with `GAUNTLET_` tokens on BOTH sides of the annex
    /// boundary — the fixture for the parser's bounds, not for the diff.
    fn boundary_spec_fixture() -> String {
        spec_fixture()
            .replace("no variables here", "`GAUNTLET_NOT_IN_ANNEX_E`")
            .replace("nor here", "`GAUNTLET_ALSO_NOT_IN_ANNEX_E`")
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
        let vars = annex_e_variables(&boundary_spec_fixture());
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
            violations[0].contains("names no GAUNTLET_ variables at all"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_commented_out_credential_is_flagged() {
        // Round-3 V10's exact evasion: `parse_env_example` drops every `#`
        // line before any credential check, so a live key committed as a
        // commented example satisfied AK-019's "no credentials" clause.
        for comment in [
            "# Example (works today):\n# GAUNTLET_ANTHROPIC_KEY=sk-ant-api03-REAL-KEY-MATERIAL\n",
            "# GAUNTLET_GH_TOKEN=ghp_deadbeefdeadbeefdeadbeef\n",
            "# authorization: Bearer abc123\n",
            "# -----BEGIN OPENSSH PRIVATE KEY-----\n",
        ] {
            let mut env = env_fixture();
            env.push_str(comment);
            let violations = check_env_example(&spec_fixture(), &env);
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains("credential-shaped material")),
                "{comment} -> {violations:?}"
            );
        }
    }

    #[test]
    fn variable_names_are_not_mistaken_for_credentials() {
        // The boundary rule is what makes the raw-text scan safe to run over
        // comments and keys: `GAUNTLET_TASK_…` contains `sk_` and `disk-`
        // contains `sk-`, and neither is a credential.
        assert!(
            raw_credentials(
                "# GAUNTLET_TASK_TIMEOUT is the per-task budget\nGAUNTLET_TASK_TIMEOUT=\n# \
                 disk-usage notes\n# region asia\n"
            )
            .iter()
            .filter(|v| !v.contains("'asia'"))
            .count()
                == 0
        );
        assert!(check_env_example(&spec_fixture(), &env_fixture()).is_empty());
    }

    #[test]
    fn jwt_and_unprefixed_tokens_in_comments_are_flagged() {
        // Critic-4 m1's exact evasions: a committed JWT matches no keyed
        // prefix; a bare hex token matches none either — but both live on a
        // commented line shaped like an entry with a value.
        for comment in [
            "# GAUNTLET_API_TOKEN=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.SflKxwRJSMeK\n",
            "# GAUNTLET_LEGACY_KEY=8f14e45fceea167a5a36dedd4bea2543\n",
        ] {
            let mut env = env_fixture();
            env.push_str(comment);
            let violations = check_env_example(&spec_fixture(), &env);
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains("credential-shaped material")
                        || v.contains("commented entry")),
                "{comment} -> {violations:?}"
            );
        }

        // A commented entry with an EMPTY value is documentation and stays
        // clean, as does ordinary prose with an equals sign.
        for comment in [
            "# GAUNTLET_OPTIONAL_EXAMPLE=\n",
            "# precedence is defaults < GAUNTLET.md < env < flags\n",
            "# see §8.2 for the resolution order\n",
        ] {
            let mut env = env_fixture();
            env.push_str(comment);
            let violations = check_env_example(&spec_fixture(), &env);
            assert!(
                violations
                    .iter()
                    .all(|v| !v.contains("commented entry")
                        && !v.contains("credential-shaped material")),
                "{comment} -> {violations:?}"
            );
        }
    }

    /// Critic-4 vacuity M4: the exclusion list is growable and nothing pinned
    /// it, so adding an entry silently unlisted a spec-recognized variable.
    #[test]
    fn runtime_injected_is_exactly_the_documented_singleton() {
        assert_eq!(
            RUNTIME_INJECTED.len(),
            1,
            "growth converts a spec-recognized variable into an unlisted one with the locator \
             green; it must be a deliberate edit that updates this pin"
        );
        assert_eq!(RUNTIME_INJECTED[0].0, "GAUNTLET_RESULT_PATH");
        assert!(
            RUNTIME_INJECTED[0].1.contains("§10.3"),
            "the exclusion must keep citing its justification"
        );
    }

    #[test]
    fn the_non_vacuity_floor_is_derived_from_the_spec_not_a_constant() {
        // Round-3 V12's exact evasion: with a constant floor of 50 against an
        // actual 92, whole groups could be deleted from Annex E and
        // .env.example in lockstep and the check still reported clean. The
        // floor now comes from the variables the SPEC names anywhere, which a
        // deletion from the annex does not move.
        let deleted_annex = spec_fixture()
            .replace("`GAUNTLET_VAR_58` ", "")
            .replace("`GAUNTLET_VAR_59` ", "")
            .replace(
                "no variables here",
                "§8.3 also reads `GAUNTLET_VAR_58` and `GAUNTLET_VAR_59`",
            );
        let deleted_env = env_fixture()
            .replace("GAUNTLET_VAR_58=\n", "")
            .replace("GAUNTLET_VAR_59=\n", "");
        let violations = check_env_example(&deleted_annex, &deleted_env);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing from the annex")
                    && v.contains("GAUNTLET_VAR_58")
                    && v.contains("GAUNTLET_VAR_59")),
            "deleting a variable from BOTH files in lockstep must still fail: {violations:?}"
        );
    }
}
