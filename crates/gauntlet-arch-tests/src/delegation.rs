//! (d) Delegation await-ban — the prime-agent adapter (plan Task 15;
//! correction RES-D5). Plan Task 15's done-when: "no code path awaits a
//! value from the delegation call (grep-asserted in the arch tests)."
//!
//! # Subject: Rust-side consumption of delegation outcomes
//!
//! The banned shape is ANSWER-SHAPED PARSING on the Rust side: the result
//! contract must be decoded in exactly two places — the runner's `poll`
//! and `collect` — and nowhere else; the transport (the delegation call's
//! seam) must not decode results at all; and the transport's success
//! payload must stay a four-field admission handle, so a child answer has
//! no field to occupy (spec §10.5: the `rlm` call "never returns the
//! child's answer"; §10.3: the file is the contract; Annex C.2 rule 1).
//!
//! # What the scan bans, mechanically
//!
//! - **A (call locus)**: any file in the adapter other than `runner.rs`
//!   containing the result-decoding calls `read_result_contract(` or
//!   `RoleResult::decode(`. The transport module is the critical case:
//!   the delegation seam must never derive a result from anything it
//!   observes (including the parent print run's piped stdout — capture
//!   for forensics is allowed, decoding is not, and this rule makes any
//!   decoding call in that module a failing boundary).
//! - **B (poll/collect exclusivity)**: inside `runner.rs`, those same
//!   call tokens may appear only within the bodies of `fn poll` and
//!   `fn collect` (tracked by the last-seen rustfmt-shaped `fn <name>`
//!   header). A call inside `invoke` — deriving a result from the
//!   delegation call — or inside a helper is a violation.
//! - **C (payload pin)**: `AdmissionHandle`'s struct block carries
//!   EXACTLY the four admission fields (`rlm_child_id`, `name`,
//!   `session_dir`, `model`) and no other `pub` field — an added
//!   `answer`-shaped field fails here instead of quietly becoming a
//!   smuggling channel.
//!
//! # The declared exemption (RES-D5)
//!
//! The pinned kernel template in `program.rs` legitimately contains
//! `await rlm(...)` — the upstream ADMISSION-await, whose awaited value is
//! the handle the template itself records (Annex C.1). This scan
//! deliberately keys on result-decoding CALLS, not on `await` tokens, so
//! the template does not self-flag. Any future await-shaped rule over this
//! crate MUST declare `program.rs`'s template constant as its exemption —
//! and that declaration belongs HERE, next to the negative control, not
//! in a weakened grep.
//!
//! # The negative control
//!
//! "A suite no adapter can fail is not a suite": the control fixtures
//! below plant each banned shape (a decoding call in a transport module;
//! a decoding call inside `fn invoke`; a five-field admission handle)
//! and assert the scan REPORTS each one. The fixtures live only here —
//! the real adapter sources carry none of them, which the repo test
//! asserts in the same run.

use std::collections::BTreeMap;

/// The adapter crate this scan polices.
pub const PRIME_ADAPTER_SRC: &str = "crates/adapters/gauntlet-adapter-agent-prime/src";

/// The one file allowed to hold result-decoding calls, and only in
/// `poll`/`collect`.
const RUNNER_FILE: &str = "runner.rs";

/// The result-decoding call tokens (call-shaped: with the paren).
const DECODE_CALLS: &[&str] = &["read_result_contract(", "RoleResult::decode("];

/// The admission handle's exact field inventory (RES-D5).
const HANDLE_FIELDS: &[&str] = &["rlm_child_id", "name", "session_dir", "model"];

/// The fns that may decode the result contract.
const DECODING_FNS: &[&str] = &["poll", "collect"];

/// Scans adapter source files (repo-relative path → text) for the banned
/// delegation-consumption shapes. Every violation names its rule, file,
/// and line.
pub fn scan_delegation_ban(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut violations = Vec::new();
    let (runner_present, handle_present) = (
        files.keys().any(|p| p.ends_with(RUNNER_FILE)),
        files.keys().any(|p| p.ends_with("transport.rs")),
    );
    if files.is_empty() || !runner_present || !handle_present {
        violations.push(format!(
            "delegation-ban: {PRIME_ADAPTER_SRC}: the adapter crate's runner/transport sources \
             are missing — the scan has nothing to police and must not pass silently"
        ));
        return violations;
    }
    for (path, text) in files {
        let is_runner = path.ends_with(RUNNER_FILE);
        // Rule A: decoding calls exist ONLY in runner.rs.
        if !is_runner {
            for token in DECODE_CALLS {
                for (index, line) in text.lines().enumerate() {
                    if line.contains(token) {
                        violations.push(format!(
                            "delegation-ban (rule A): {path}:{}: result-decoding call {token:?} \
                             outside {RUNNER_FILE} — only the runner's poll/collect may derive \
                             a result from the contract (plan Task 15, RES-D5, Annex C.2)",
                            index + 1
                        ));
                    }
                }
            }
        }
        // Rule C: the admission handle's field inventory is exact.
        // Line-based and indentation-tolerant: scan from the struct
        // header to the first closing-brace line.
        if path.ends_with("transport.rs") {
            let header = "pub struct AdmissionHandle {";
            if let Some(start) = text.find(header) {
                for (index, line) in text[start..].lines().enumerate() {
                    let trimmed = line.trim_start();
                    if index > 0 && trimmed.starts_with('}') {
                        break;
                    }
                    if trimmed.starts_with("pub ") && trimmed.contains(':') {
                        let field = trimmed
                            .split(':')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_start_matches("pub ")
                            .trim();
                        if !HANDLE_FIELDS.contains(&field) && !field.is_empty() {
                            violations.push(format!(
                                "delegation-ban (rule C): {path}:{}: AdmissionHandle carries \
                                 non-admission field {field:?} — the transport's success payload \
                                 is EXACTLY the four admission fields; an answer has no field to \
                                 occupy (RES-D5, spec 10.5)",
                                text[..start].lines().count() + index
                            ));
                        }
                    }
                }
            }
        }
        // Rule B: within runner.rs, decoding calls live only in poll/collect.
        if is_runner {
            let mut current_fn = "<top>";
            for (index, line) in text.lines().enumerate() {
                if let Some(name) = fn_header(line) {
                    current_fn = name;
                }
                if DECODING_FNS.contains(&current_fn) {
                    continue;
                }
                for token in DECODE_CALLS {
                    if line.contains(token) {
                        violations.push(format!(
                            "delegation-ban (rule B): {path}:{}: result-decoding call {token:?} \
                             inside fn {current_fn} — the contract is decoded ONLY in fn poll \
                             and fn collect (no code path may await or derive a value from the \
                             delegation call; plan Task 15, spec 6.1/10.3)",
                            index + 1
                        ));
                    }
                }
            }
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

/// The name of the fn whose header this line opens, if any
/// (rustfmt-shaped: `fn name(` or `pub fn name(` at any indent).
fn fn_header(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))?;
    let name_end = rest.find('(')?;
    let name = &rest[..name_end];
    let well_formed =
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    well_formed.then_some(name)
}

#[cfg(test)]
mod delegation_tests {
    use std::path::Path;

    use super::*;

    /// Loads the real adapter sources for the repo scan.
    fn real_adapter_sources() -> BTreeMap<String, String> {
        let root = crate::walk::repo_root();
        let src = root.join(PRIME_ADAPTER_SRC);
        let mut files = BTreeMap::new();
        collect_rs(&src, &root, &mut files);
        files
    }

    fn collect_rs(dir: &Path, root: &Path, files: &mut BTreeMap<String, String>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs(&path, root, files);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.insert(rel, text);
            }
        }
    }

    /// The repo scan: the real prime-agent adapter carries NO banned shape.
    #[test]
    fn the_prime_adapter_derives_no_result_from_the_delegation_call() {
        let files = real_adapter_sources();
        assert!(
            files.len() >= 6,
            "the adapter crate's sources must be loadable: {:?}",
            files.keys()
        );
        let violations = scan_delegation_ban(&files);
        assert!(
            violations.is_empty(),
            "the prime-agent adapter must never derive a result from the delegation call:\n{}",
            violations.join("\n")
        );
    }

    // ---------------------------------------------------------------------------
    // Negative control: the scan must REPORT each banned shape. The fixtures
    // are planted here only — asserted and never committed to the adapter.
    // ---------------------------------------------------------------------------

    /// A fake transport module that decodes a result from what it observed —
    /// rule A must fire.
    const VIOLATING_TRANSPORT: &str = "\
    use gauntlet_ports::role_result::{RoleResult, read_result_contract};
    pub fn sneak(program: &str) -> Option<RoleResult> {
        let decoded = read_result_contract(program, &todo!(), &todo!());
        decoded.ok()
    }
    ";

    /// A fake runner whose `invoke` (not poll/collect) decodes the contract —
    /// rule B must fire.
    const VIOLATING_RUNNER: &str = "\
    use gauntlet_ports::role_result::read_result_contract;
    pub struct Runner;
    impl Runner {
        pub fn invoke(&self, spec: &str) -> u32 {
            let _stolen = read_result_contract(spec, &todo!(), &todo!());
            0
        }
        pub fn poll(&self, handle: &str) -> u32 {
            let _allowed = read_result_contract(handle, &todo!(), &todo!());
            1
        }
        pub fn collect(&self, handle: &str) -> u32 {
            let _allowed = read_result_contract(handle, &todo!(), &todo!());
            2
        }
    }
    ";

    /// A transport carrying an answer-shaped field on the admission handle —
    /// rule C must fire.
    const VIOLATING_HANDLE: &str = "\
    pub struct AdmissionHandle {
        pub rlm_child_id: String,
        pub name: String,
        pub session_dir: String,
        pub model: String,
        pub answer: String,
    }
    ";

    fn fixture_map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    #[test]
    fn the_control_proves_rule_a_has_teeth() {
        let files = fixture_map(&[
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/transport.rs",
                VIOLATING_TRANSPORT,
            ),
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/runner.rs",
                "pub struct Runner;\n",
            ),
        ]);
        let violations = scan_delegation_ban(&files);
        assert!(
            violations.iter().any(|v| v.contains("rule A")
                && v.contains("transport.rs")
                && v.contains("read_result_contract")),
            "a decoding call in the transport module must be REPORTED: {violations:?}"
        );
    }

    #[test]
    fn the_control_proves_rule_b_has_teeth() {
        let files = fixture_map(&[
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/transport.rs",
                "pub struct Transport;\n",
            ),
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/runner.rs",
                VIOLATING_RUNNER,
            ),
        ]);
        let violations = scan_delegation_ban(&files);
        let invoke_hit = violations
            .iter()
            .any(|v| v.contains("rule B") && v.contains("inside fn invoke"));
        let poll_clean = !violations.iter().any(|v| {
            v.contains("rule B")
                && (v.contains("inside fn poll") || v.contains("inside fn collect"))
        });
        assert!(
            invoke_hit,
            "a decoding call inside fn invoke must be REPORTED: {violations:?}"
        );
        assert!(
            poll_clean,
            "the poll/collect exemption must hold: {violations:?}"
        );
    }

    #[test]
    fn the_control_proves_rule_c_has_teeth() {
        let files = fixture_map(&[
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/transport.rs",
                VIOLATING_HANDLE,
            ),
            (
                "crates/adapters/gauntlet-adapter-agent-prime/src/runner.rs",
                "pub struct Runner;\n",
            ),
        ]);
        let violations = scan_delegation_ban(&files);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("rule C") && v.contains("answer")),
            "an answer-shaped field on the admission handle must be REPORTED: {violations:?}"
        );
    }

    #[test]
    fn the_control_proves_a_missing_crate_cannot_pass_silently() {
        let violations = scan_delegation_ban(&BTreeMap::new());
        assert!(
            violations.iter().any(|v| v.contains("missing")),
            "an empty source set must be a violation, not a pass: {violations:?}"
        );
    }
}
