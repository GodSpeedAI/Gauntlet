//! Scope escapes — the second half of "what gets scanned must equal what
//! compiles" (recritique R4/R17/R25, class A3).
//!
//! `crate::walk` guarantees every regular file *inside* a core crate reaches
//! a scanner. Rust has four ways to compile a file from *outside* the crate
//! directory anyway, none of which the walk can see:
//!
//! ```text
//! #[path = "../../../shared/wiring.rs"] mod wiring;
//! include!("../../../shared/wiring.rs");
//! const KEY: &str = include_str!("../../../shared/model-id.txt");
//! const BLOB: &[u8] = include_bytes!("../../../shared/blob.bin");
//! ```
//!
//! Each carries only a relative path, which no literal class flags, and the
//! target lives outside every scan scope. This module resolves each such
//! target against the file that declares it and reports any that escapes the
//! declaring crate's own directory.
//!
//! False-positive posture: an in-crate `#[path = "sub/mod.rs"]` or
//! `include_str!("fixture.txt")` resolves inside the crate directory and is
//! not reported — those files are already walked and scanned. Only escapes
//! are violations.

/// Directives whose string argument names a file the compiler will read.
const DIRECTIVES: &[&str] = &["#[path", "include!", "include_str!", "include_bytes!"];

/// Report every `#[path]`/`include*!` target in `text` that resolves outside
/// `crate_dir`. `path` is the repo-relative path of the declaring file and
/// `crate_dir` its crate's repo-relative directory.
pub fn scan_escapes(path: &str, crate_dir: &str, text: &str) -> Vec<String> {
    let declaring_dir = parent_dir(path);
    let mut out = Vec::new();
    for unit in crate::text::scan_units(text) {
        let lowered = unit.text.to_lowercase();
        for directive in DIRECTIVES {
            let mut from = 0usize;
            while let Some(found) = lowered[from..].find(directive) {
                let at = from + found;
                from = at + directive.len();
                let Some(target) = first_string_after(&unit.text, from) else {
                    continue;
                };
                if target.is_empty() {
                    continue;
                }
                let resolved = normalize_join(&declaring_dir, &target);
                let inside = resolved == crate_dir
                    || resolved.starts_with(&format!("{crate_dir}/"))
                    || is_absolute(&target);
                if is_absolute(&target) {
                    out.push(format!(
                        "scope-escape: {path}:{}: `{}` names the absolute path '{target}' — the \
                         compiler reads it, no scanner does (spec §24.3)",
                        unit.line,
                        directive.trim_start_matches('#')
                    ));
                } else if !inside {
                    out.push(format!(
                        "scope-escape: {path}:{}: `{}` pulls '{target}' (resolved: '{resolved}') \
                         into {crate_dir} from outside the crate directory — it compiles into a \
                         core crate but no boundary scanner ever reads it (spec §24.3)",
                        unit.line,
                        directive.trim_start_matches('#')
                    ));
                }
            }
        }
    }
    out.dedup();
    out
}

/// The first double-quoted string starting at or after `from`, unescaped
/// only for `\\` and `\"` (enough for a path).
fn first_string_after(text: &str, from: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = from.min(bytes.len());
    while i < bytes.len() && bytes[i] != b'"' {
        // Stop at a statement boundary so a later, unrelated string is not
        // attributed to this directive.
        if bytes[i] == b';' || bytes[i] == b'{' {
            return None;
        }
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1;
    let mut out = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(out),
            b'\\' if i + 1 < bytes.len() => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            other => {
                out.push(other as char);
                i += 1;
            }
        }
    }
    None
}

fn is_absolute(target: &str) -> bool {
    target.starts_with('/')
        || target
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic())
            && target.as_bytes().get(1) == Some(&b':')
            && matches!(target.as_bytes().get(2), Some(b'\\') | Some(b'/'))
}

fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(at) => path[..at].to_string(),
        None => String::new(),
    }
}

/// Join `rel` onto `base` (both POSIX, repo-relative) and resolve `.`/`..`.
/// A `..` that walks above the repo root yields a path starting with `..`,
/// which can never be inside a crate directory.
fn normalize_join(base: &str, rel: &str) -> String {
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
mod tests {
    use super::*;

    const APP: &str = "crates/gauntlet-app";
    const APP_LIB: &str = "crates/gauntlet-app/src/lib.rs";

    #[test]
    fn path_attribute_escaping_the_crate_is_flagged() {
        // Recritique R4's exact evasion.
        let fixture = "#[path = \"../../../shared/wiring.rs\"]\nmod wiring;\n";
        let violations = scan_escapes(APP_LIB, APP, fixture);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("scope-escape: crates/gauntlet-app/src/lib.rs:1:"),
            "{}",
            violations[0]
        );
        assert!(violations[0].contains("shared/wiring.rs"), "{violations:?}");
    }

    #[test]
    fn include_forms_escaping_the_crate_are_flagged() {
        // Recritique R4/R25: include!, include_str!, include_bytes!.
        for fixture in [
            "include!(\"../../../shared/wiring.rs\");",
            "const KEY: &str = include_str!(\"../../../shared/model-id.txt\");",
            "const BLOB: &[u8] = include_bytes!(\"../../../vendor/blob.bin\");",
            "#[path = \"../../../vendor/leak.rs\"] mod leak;",
            "#[path = \"../../../vendor/store.rs\"] mod store;",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("scope-escape: "),
                "{}",
                violations[0]
            );
        }
    }

    #[test]
    fn absolute_include_targets_are_flagged() {
        let violations = scan_escapes(APP_LIB, APP, "include!(\"/opt/shared/wiring.rs\");");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("absolute path"), "{violations:?}");
    }

    #[test]
    fn escapes_split_across_lines_are_flagged() {
        // The continuation form of the same evasion (recritique R6 shape).
        let fixture = "#[path = \"../../../ven\\\n    dor/leak.rs\"]\nmod leak;\n";
        let violations = scan_escapes(APP_LIB, APP, fixture);
        assert!(
            violations.iter().any(|v| v.contains("vendor/leak.rs")),
            "{violations:?}"
        );
    }

    #[test]
    fn in_crate_paths_and_includes_are_not_flagged() {
        for fixture in [
            "#[path = \"engine/mod.rs\"]\nmod engine;\n",
            "const T: &str = include_str!(\"../fixtures/plan.txt\");\n",
            "include!(\"generated/table.rs\");\n",
            "//! A doc comment mentioning include! without a target.\n",
            "let s = format!(\"{}\", x);\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    #[test]
    fn normalize_join_resolves_dot_segments() {
        assert_eq!(
            normalize_join("crates/gauntlet-app/src", "../../../shared/w.rs"),
            "shared/w.rs"
        );
        assert_eq!(
            normalize_join("crates/gauntlet-app/src", "./engine/mod.rs"),
            "crates/gauntlet-app/src/engine/mod.rs"
        );
        assert_eq!(normalize_join("a", "../../../../b"), "../../../b");
    }
}

#[cfg(test)]
mod repo {
    #[test]
    fn core_crates_have_no_scope_escapes_or_unscannable_files() {
        let violations = crate::check_scope_integrity(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("scope-integrity", &violations)
        );
    }
}
