//! Renders the harness verification matrix as Markdown. Report-only: it reads definitions and
//! evidence records and never runs a harness.
//!
//! Executable discovery is injected by the caller, so the renderer stays hermetic and testable.

use crate::definition::{HarnessDefinition, PromptMode};
use crate::provenance::HERDR_PINNED_COMMIT;
use crate::verification::{EvidenceKind, VerificationRecord};
use std::collections::{BTreeMap, BTreeSet};

/// Column headers of the matrix, in output order.
pub const COLUMNS: [&str; 15] = [
    "harness",
    "catalogued",
    "executable on this machine",
    "launch args",
    "prompt mode",
    "lifecycle source",
    "interrupt",
    "session identity",
    "resume",
    "model selection",
    "structured events",
    "native delegation",
    "support level",
    "verification tiers",
    "evidence (VERIFICATION.toml)",
];

/// Ids of the adapter-owned row (no definition file). Its cells are fixed by the adapter.
pub const PRIME_ROW_ID: &str = "prime-agent";

/// One rendered matrix row: the harness id, its cells, and the tiers it reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixRow {
    pub id: String,
    pub cells: Vec<String>,
    pub tiers: BTreeSet<EvidenceKind>,
    pub discovered: bool,
}

/// Build one row per definition, then the adapter-owned prime row.
pub fn build_rows(
    defs: &[HarnessDefinition],
    records: &[VerificationRecord],
    discovered: &dyn Fn(&str) -> bool,
) -> Vec<MatrixRow> {
    let mut rows: Vec<MatrixRow> = defs
        .iter()
        .map(|def| definition_row(def, records, discovered))
        .collect();
    rows.push(prime_row(records, discovered));
    rows
}

fn definition_row(
    def: &HarnessDefinition,
    records: &[VerificationRecord],
    discovered: &dyn Fn(&str) -> bool,
) -> MatrixRow {
    let found: Vec<&String> = def.executables.iter().filter(|e| discovered(e)).collect();
    let mut tiers = tiers_for(&def.id, records);
    tiers.insert(EvidenceKind::Catalogued);

    let provenance = def
        .provenance
        .as_ref()
        .map(|p| format!("yes: Herdr {} ({})", p.commit, p.license))
        .unwrap_or_else(|| "no".to_string());

    let args = if def.launch.args.is_empty() {
        "(none)".to_string()
    } else {
        def.launch.args.join(" ")
    };
    let prompt_mode = match def.launch.prompt_mode {
        PromptMode::Flag => format!("flag {}", def.launch.prompt_flag.as_deref().unwrap_or("?")),
        other => other.to_string(),
    };
    let resume = def
        .session
        .resume_argv
        .as_ref()
        .map(|argv| argv.join(" "))
        .unwrap_or_else(|| "no".to_string());
    let model = def
        .launch
        .model_arg
        .as_ref()
        .map(|argv| argv.join(" "))
        .unwrap_or_else(|| "no".to_string());

    let cells = vec![
        format!("{} ({})", def.id, def.label),
        provenance,
        discovery_cell(&found, &def.executables),
        args,
        prompt_mode,
        def.lifecycle.source.to_string(),
        def.control.interrupt.to_string(),
        def.session.identity.to_string(),
        resume,
        model,
        yes_no(def.capabilities.structured_events),
        yes_no(def.capabilities.native_subagents),
        def.capabilities.support_level.to_string(),
        tier_list(&tiers),
        evidence_list(&def.id, records),
    ];

    MatrixRow {
        id: def.id.clone(),
        cells,
        tiers,
        discovered: !found.is_empty(),
    }
}

fn prime_row(records: &[VerificationRecord], discovered: &dyn Fn(&str) -> bool) -> MatrixRow {
    let found = discovered(PRIME_ROW_ID);
    let tiers = tiers_for(PRIME_ROW_ID, records);
    let cells = vec![
        format!("{PRIME_ROW_ID} (adapter-owned)"),
        "no: no definition file; behavior owned by gauntlet-adapter-agent-prime".to_string(),
        if found {
            format!("yes: {PRIME_ROW_ID}")
        } else {
            "no".to_string()
        },
        "--mode rpc (adapter transport)".to_string(),
        "rpc via adapter transport (not -p)".to_string(),
        "adapter result contract".to_string(),
        "adapter cancel (not in help)".to_string(),
        "adapter invocation id".to_string(),
        "not modeled".to_string(),
        "not modeled".to_string(),
        "rpc telemetry (adapter-parsed)".to_string(),
        "no".to_string(),
        "n/a (not a definition)".to_string(),
        tier_list(&tiers),
        evidence_list(PRIME_ROW_ID, records),
    ];
    MatrixRow {
        id: PRIME_ROW_ID.to_string(),
        cells,
        tiers,
        discovered: found,
    }
}

fn tiers_for(id: &str, records: &[VerificationRecord]) -> BTreeSet<EvidenceKind> {
    records
        .iter()
        .filter(|r| r.harness == id)
        .map(|r| r.kind)
        .collect()
}

fn yes_no(flag: bool) -> String {
    if flag { "yes" } else { "no" }.to_string()
}

fn discovery_cell(found: &[&String], executables: &[String]) -> String {
    if found.is_empty() {
        format!("no (checked {})", executables.join(", "))
    } else {
        format!(
            "yes: {}",
            found
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn tier_list(tiers: &BTreeSet<EvidenceKind>) -> String {
    tiers
        .iter()
        .map(|k| k.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn evidence_list(id: &str, records: &[VerificationRecord]) -> String {
    let mut parts: Vec<String> = records
        .iter()
        .filter(|r| r.harness == id)
        .map(|r| {
            let short = match r.kind {
                EvidenceKind::Catalogued => "catalogued",
                EvidenceKind::Fixture => "fixture",
                EvidenceKind::HelpVerified => "help",
                EvidenceKind::LiveVerified => "live",
            };
            format!("{short}:{}", r.field)
        })
        .collect();
    parts.sort();
    parts.dedup();
    if parts.is_empty() {
        "none beyond catalogue".to_string()
    } else {
        parts.join("; ")
    }
}

/// Count rows that reached each tier (a row counts once per tier it holds).
pub fn tier_counts(rows: &[MatrixRow]) -> BTreeMap<EvidenceKind, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        for tier in &row.tiers {
            *counts.entry(*tier).or_insert(0) += 1;
        }
    }
    counts
}

/// Render the whole matrix. `generated` is the generation date (YYYY-MM-DD), passed in so the
/// output is deterministic for a given input.
pub fn render_matrix(
    defs: &[HarnessDefinition],
    records: &[VerificationRecord],
    discovered: &dyn Fn(&str) -> bool,
    generated: &str,
) -> String {
    let rows = build_rows(defs, records, discovered);
    let mut out = String::new();
    out.push_str("# Harness verification matrix\n\n");
    out.push_str(
        "Generated by `harness-matrix` (report-only). Source: `definitions/*.toml`, \
         `definitions/VERIFICATION.toml`. Tier meanings are defined in \
         `definitions/CATALOG.md`.\n\n",
    );
    out.push('|');
    for col in COLUMNS {
        out.push_str(&format!(" {col} |"));
    }
    out.push('\n');
    out.push('|');
    for _ in COLUMNS {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &rows {
        out.push('|');
        for cell in &row.cells {
            out.push_str(&format!(" {} |", cell.replace('|', "\\|")));
        }
        out.push('\n');
    }

    let counts = tier_counts(&rows);
    let count = |k: EvidenceKind| counts.get(&k).copied().unwrap_or(0);
    let on_machine = rows.iter().filter(|r| r.discovered).count();
    let adapter_rows = rows.iter().filter(|r| r.id == PRIME_ROW_ID).count();

    out.push('\n');
    out.push_str("## Footer\n\n");
    out.push_str(&format!("- Generated: {generated}.\n"));
    out.push_str(&format!(
        "- Catalog pin: Herdr {HERDR_PINNED_COMMIT}. Rows: {} ({} definitions, {} adapter-owned).\n",
        rows.len(),
        rows.len() - adapter_rows,
        adapter_rows
    ));
    out.push_str(&format!(
        "- Records in VERIFICATION.toml: {}.\n",
        records.len()
    ));
    out.push_str(&format!(
        "- Tier counts (rows holding at least one record of the tier): catalogued {}, fixture {}, help-verified {}, live-verified {}.\n",
        count(EvidenceKind::Catalogued),
        count(EvidenceKind::Fixture),
        count(EvidenceKind::HelpVerified),
        count(EvidenceKind::LiveVerified),
    ));
    out.push_str(&format!(
        "- Executable found on PATH at generation: {on_machine} of {} rows. Only those rows can carry live-verified evidence from this machine.\n",
        rows.len()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{HarnessDefinition, Origin};
    use crate::verification::parse_verification;

    const ALPHA: &str = r#"
schema = 1
id = "alpha"
label = "Alpha"
executables = ['alpha']

[launch]
args = ["exec"]
model_arg = ["--model", "{model}"]
prompt_mode = "flag"
prompt_flag = "-p"

[session]
identity = "id"
resume_argv = ["--resume", "{session}"]

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = true
model_selection = true
native_subagents = false
structured_events = false
support_level = 1
"#;

    const BETA: &str = r#"
schema = 1
id = "beta"
label = "Beta"
executables = ['beta']

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "none"

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = false
model_selection = false
native_subagents = false
structured_events = false
support_level = 0
"#;

    fn defs() -> Vec<HarnessDefinition> {
        vec![
            HarnessDefinition::from_toml_str(ALPHA, Origin::BuiltIn).expect("alpha"),
            HarnessDefinition::from_toml_str(BETA, Origin::BuiltIn).expect("beta"),
        ]
    }

    fn records() -> Vec<VerificationRecord> {
        let ids: BTreeSet<String> = ["alpha", "beta", PRIME_ROW_ID]
            .iter()
            .map(|s| s.to_string())
            .collect();
        parse_verification(
            r#"
schema = 1

[[record]]
harness = "alpha"
field = "executables"
kind = "live-verified"
probe_argv = ["alpha", "--version"]
observed = "alpha 1.0.0"
harness_version = "1.0.0"
date = "2026-10-09"

[[record]]
harness = "alpha"
field = "launch.prompt_mode"
kind = "help-verified"
probe_argv = ["alpha", "--help"]
observed = "-p, --print"
harness_version = "1.0.0"
date = "2026-10-09"

[[record]]
harness = "alpha"
field = "executables"
kind = "fixture"
probe_argv = ["cargo", "test", "alpha_fixture"]
observed = "fixture"
harness_version = "n/a"
date = "2026-10-09"

[[record]]
harness = "prime-agent"
field = "executables"
kind = "fixture"
probe_argv = ["cargo", "test", "prime_fixture"]
observed = "fixture"
harness_version = "n/a"
date = "2026-10-09"
"#,
            &ids,
        )
        .expect("fixture records are valid")
    }

    fn only_alpha(name: &str) -> bool {
        name == "alpha"
    }

    #[test]
    fn one_row_per_definition_plus_prime() {
        let rows = build_rows(&defs(), &records(), &only_alpha);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta", PRIME_ROW_ID]);
        assert!(rows.iter().all(|r| r.cells.len() == COLUMNS.len()));
    }

    #[test]
    fn tiers_follow_records_and_harnesses_without_records_stay_low() {
        let rows = build_rows(&defs(), &records(), &only_alpha);
        let alpha = &rows[0];
        assert!(alpha.tiers.contains(&EvidenceKind::LiveVerified));
        assert!(alpha.tiers.contains(&EvidenceKind::HelpVerified));
        assert!(alpha.tiers.contains(&EvidenceKind::Fixture));
        assert!(alpha.tiers.contains(&EvidenceKind::Catalogued));

        let beta = &rows[1];
        assert_eq!(
            beta.tiers.iter().copied().collect::<Vec<_>>(),
            vec![EvidenceKind::Catalogued],
            "a harness without records is only catalogued"
        );
        assert!(
            beta.tiers
                .iter()
                .all(|t| matches!(t, EvidenceKind::Catalogued | EvidenceKind::Fixture))
        );
    }

    #[test]
    fn tier_counts_and_footer_are_correct() {
        let rows = build_rows(&defs(), &records(), &only_alpha);
        let counts = tier_counts(&rows);
        assert_eq!(counts.get(&EvidenceKind::Catalogued), Some(&2));
        assert_eq!(counts.get(&EvidenceKind::Fixture), Some(&2));
        assert_eq!(counts.get(&EvidenceKind::HelpVerified), Some(&1));
        assert_eq!(counts.get(&EvidenceKind::LiveVerified), Some(&1));

        let text = render_matrix(&defs(), &records(), &only_alpha, "2026-10-09");
        assert!(text.contains("- Generated: 2026-10-09."));
        assert!(text.contains("Rows: 3 (2 definitions, 1 adapter-owned)"));
        assert!(text.contains("catalogued 2, fixture 2, help-verified 1, live-verified 1"));
        assert!(text.contains("Executable found on PATH at generation: 1 of 3 rows"));
    }

    #[test]
    fn discovery_column_reflects_injected_lookup() {
        let text = render_matrix(&defs(), &records(), &only_alpha, "2026-10-09");
        assert!(text.contains("| yes: alpha |"));
        assert!(text.contains("| no (checked beta) |"));
    }

    #[test]
    fn prompt_mode_cell_names_the_flag() {
        let text = render_matrix(&defs(), &records(), &only_alpha, "2026-10-09");
        assert!(text.contains("| flag -p |"));
    }
}
