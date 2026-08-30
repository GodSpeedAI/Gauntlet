//! PROBE-TEMPLATE HONESTY — permanent teeth for the doctor's
//! parse-AND-render configuration probe (COR-R2-1, round-2 finding F1;
//! report: .agents/reports/task16/correction-round2.md).
//!
//! `probe_template` is the mechanism COR-F2 relies on: a command template
//! is usable only if it PARSES and RENDERS against every declared
//! placeholder with synthetic values, so the doctor reports a
//! parse-ok-but-render-impossible template unusable HERE instead of
//! clean-here-refused-at-admission. Round-2 mutation M1 proved the
//! regression class: gutting the probe to parse-only left every shipped
//! suite green (58/58 adapter in-tree, 62/62 gauntlet-cli incl. P8) — a
//! parse-only probe would report `'' {prompt_file}` (a template that can
//! NEVER render: argv[0] is the empty word) as usable. These tests pin
//! the render half:
//!
//! - the discriminating leg: a parse-ok / render-impossible template is
//!   REFUSED with the typed render error `EmptyProgram` — under a
//!   parse-only gutting this returns `Ok(())` and this file goes RED;
//! - a valid template (all four placeholders bound) probes Ok — the
//!   probe must not over-fail;
//! - an unknown placeholder is refused with the typed parse error.

use gauntlet_adapter_agent_cli::{TemplateError, probe_template};

// Leg 1 (discriminating): parse-ok / render-impossible refuses with the
// typed render error — never a clean probe for a template that cannot
// render at admission (COR-F2 honesty, permanently pinned).
#[test]
fn parse_ok_render_impossible_template_is_refused() {
    let error = probe_template("'' {prompt_file}").expect_err(
        "a template whose argv[0] is the empty word must be REFUSED by the probe — \
         an Ok here means the probe has regressed to parse-only (round-2 mutation M1)",
    );
    assert!(
        matches!(error, TemplateError::EmptyProgram),
        "the refusal must be the typed render error EmptyProgram, got {error:?}"
    );
}

// Leg 2: a valid template with all four placeholders bound probes Ok —
// the probe renders against every declared placeholder and must not
// over-fail a usable configuration.
#[test]
fn valid_template_with_all_placeholders_probes_ok() {
    probe_template(
        "agent --prompt {prompt_file} --dir {workspace} --out {result_path} \
         --model {model}",
    )
    .expect("a fully bindable template must probe usable");
}

// Leg 3: an unknown placeholder is refused with the typed parse error —
// the probe surfaces the parse half of the vocabulary check too.
#[test]
fn unknown_placeholder_is_refused() {
    let error = probe_template("agent {fabricated}").expect_err("unknown placeholder refuses");
    assert!(
        matches!(
            error,
            TemplateError::UnknownPlaceholder { ref name } if name == "fabricated"
        ),
        "the refusal must be the typed UnknownPlaceholder error, got {error:?}"
    );
}
