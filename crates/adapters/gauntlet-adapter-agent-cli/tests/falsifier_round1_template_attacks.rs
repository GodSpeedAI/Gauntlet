//! TEMPLATE-RENDER ATTACKS — falsifier round 1 (composed blind by the
//! falsifier agent; executed by the orchestrator after the agent's
//! usage-window death — see .agents/reports/task16/{falsifier-round1.md,
//! IN-FLIGHT.md}). Retained as the round's regression teeth: a4b pins
//! COR-F2 (a comment-only template is a typed parse refusal, so the
//! doctor's configuration probe can report it honestly). Every attack
//! here must stay REPELLED (typed error / literal argv word) — a
//! regression here is a finding.

use gauntlet_adapter_agent_cli::{TemplateError, TemplateValues, parse_template, render};

fn values<'a>(
    prompt: &'a str,
    workspace: &'a str,
    result: &'a str,
    model: Option<&'a str>,
) -> TemplateValues<'a> {
    TemplateValues {
        prompt_file: prompt,
        workspace,
        result_path: result,
        model,
    }
}

fn render_ok(source: &str) -> Vec<String> {
    let template = parse_template(source).expect("template must parse");
    render(
        &template,
        &values(
            "/state/inv/p.prompt.txt",
            "/ws",
            "/state/inv/result.json",
            Some("vendor-model-x"),
        ),
    )
    .expect("template must render")
}

// A1: unknown placeholders and case tricks must be typed refusals.
#[test]
fn a1_unknown_and_case_tricks_are_typed_refusals() {
    for source in [
        "agent {fabricated}",
        "agent {PROMPT_FILE}",
        "agent {Prompt_File}",
        "agent {}",
        "agent {model}}",
        "agent {{model}}",
        "agent {a{model}}",
        "agent {prompt_file} {nope}",
    ] {
        let error = parse_template(source).expect_err(source);
        assert!(
            matches!(
                error,
                TemplateError::UnknownPlaceholder { .. }
                    | TemplateError::DanglingBrace { .. }
                    | TemplateError::EmbeddedPlaceholder { .. }
            ),
            "{source} must refuse typed, got {error:?}"
        );
    }
}

// A2: fused placeholders (dot suffix, flag prefix, quote-fused) are
// refusals, so no substituted value can splice into operator grammar.
#[test]
fn a2_fused_placeholders_are_refusals() {
    for source in [
        "agent {result_path}.txt",
        "agent --out={result_path}",
        "agent x{model}y",
        "agent \"{prompt_file}\".txt",
        "agent {prompt_file}{result_path}",
    ] {
        let error = parse_template(source).expect_err(source);
        assert!(
            matches!(error, TemplateError::EmbeddedPlaceholder { .. }),
            "{source} must refuse as fused, got {error:?}"
        );
    }
    // An empty brace before a real placeholder is an unknown NAME, still
    // a typed refusal.
    let error = parse_template("agent '{}'{prompt_file}").expect_err("'{}' fused");
    assert!(
        matches!(error, TemplateError::UnknownPlaceholder { ref name } if name.is_empty()),
        "{error:?}"
    );
}

// A3: quoted placeholders group into the placeholder (quotes never hide
// or splice one) — they must still be recognized, not turned literal.
#[test]
fn a3_quoted_placeholders_still_bind() {
    assert_eq!(
        render_ok("agent \"{prompt_file}\" '{workspace}'"),
        vec!["agent", "/state/inv/p.prompt.txt", "/ws"]
    );
}

// A4: empty / whitespace / CRLF-only templates refuse.
#[test]
fn a4_empty_shapes_refuse() {
    for source in ["", "   ", "\n\n", " \t ", "\r\n"] {
        let error = parse_template(source).expect_err(source);
        assert!(
            matches!(error, TemplateError::Empty),
            "{source:?} must be Empty, got {error:?}"
        );
    }
}

// A4b FINDING → CORRECTED (COR-F2): a comment-only template must PARSE-
// REFUSE as Empty — a zero-word source is no program, and the doctor's
// configuration probe (parse + synthetic render) must be able to report
// it unusable instead of clean-then-refuse-at-admission.
#[test]
fn a4b_comment_only_template_is_a_typed_parse_refusal() {
    let error = parse_template("\n # only a comment\n").expect_err("comment-only refuses");
    assert!(matches!(error, TemplateError::Empty), "{error:?}");
}

// A5: unbalanced quotes and trailing backslash refuse.
#[test]
fn a5_unbalanced_refuse() {
    for source in ["agent \"open", "agent 'open", "agent trailing\\"] {
        assert!(parse_template(source).is_err(), "{source} must refuse");
    }
}

// A6: shell grammar in the TEMPLATE stays data: $VAR, $(...), backticks,
// globs, ; && | are argv characters, never executed grammar.
#[test]
fn a6_template_grammar_stays_data() {
    let argv = render_ok("$AGENT $(rm -rf /) `id` *.txt x;y && z | w");
    assert_eq!(
        argv,
        vec![
            "$AGENT", "$(rm", "-rf", "/)", "`id`", "*.txt", "x;y", "&&", "z", "|", "w"
        ]
    );
}

// A7: hostile substituted VALUES (spaces, ;, &&, newlines, quotes, $(),
// backticks in workspace / model / prompt / result) are SINGLE verbatim
// argv words — never split into grammar.
#[test]
fn a7_hostile_values_stay_single_words() {
    let template =
        parse_template("agent {workspace} {model} {prompt_file} {result_path}").expect("parses");
    let hostile_ws = "/w s;rm -rf ~&$(boom)`x\"'|\n/etc";
    let hostile_model = "m;|&&$(x)`y\"' \n z";
    let argv = render(
        &template,
        &values("/p", hostile_ws, "/r", Some(hostile_model)),
    )
    .expect("renders");
    assert_eq!(argv.len(), 5, "no value ever splits: {argv:?}");
    assert_eq!(argv[1], hostile_ws);
    assert_eq!(argv[2], hostile_model);
}

// A8: the legal-but-odd bare {prompt_file} template, and {workspace} as
// argv[0] — both render; only an EMPTY argv[0] refuses.
#[test]
fn a8_odd_argv0_shapes() {
    assert_eq!(render_ok("{prompt_file}"), vec!["/state/inv/p.prompt.txt"]);
    assert_eq!(
        render_ok("{workspace} {prompt_file}"),
        vec!["/ws", "/state/inv/p.prompt.txt"]
    );
    let template = parse_template("'' {prompt_file}").expect("parses");
    let error = render(&template, &values("/p", "/w", "/r", None)).unwrap_err();
    assert!(matches!(error, TemplateError::EmptyProgram), "{error:?}");
}

// A9: {model} with no hint refuses at RENDER (never a silently empty arg).
#[test]
fn a9_model_without_hint_refuses_at_render() {
    let template = parse_template("agent --model {model}").expect("parses");
    let error = render(&template, &values("/p", "/w", "/r", None)).unwrap_err();
    assert!(
        matches!(error, TemplateError::MissingValue { ref name } if name == "model"),
        "{error:?}"
    );
}

// A10: duplicated placeholders substitute the same value twice.
#[test]
fn a10_duplicated_placeholders_bind_twice() {
    assert_eq!(
        render_ok("agent {result_path} {result_path}"),
        vec!["agent", "/state/inv/result.json", "/state/inv/result.json"]
    );
}

// A11: CRLF line endings are separators (no \r leaks into words).
#[test]
fn a11_crlf_is_separator() {
    assert_eq!(
        render_ok("agent\r\n--flag\r\n{prompt_file}"),
        vec!["agent", "--flag", "/state/inv/p.prompt.txt"]
    );
}

// A12: the double-quote escape vocabulary is POSIX-narrow; a backslash
// before a non-escape char inside quotes stays literal (both chars).
#[test]
fn a12_backslash_inside_quotes_stays_literal() {
    let source = "agent \"\\a\\b\" \\x \\\\ end\\\"";
    let argv = render_ok(source);
    assert_eq!(argv, vec!["agent", "\\a\\b", "x", "\\", "end\""]);
}

// A13: a # mid-word is an ordinary character; a # at word start comments
// out the REST OF THE LINE only (a later line still renders).
#[test]
fn a13_hash_rules() {
    assert_eq!(
        render_ok("a#b {prompt_file}"),
        vec!["a#b", "/state/inv/p.prompt.txt"]
    );
    assert_eq!(
        render_ok("agent # {prompt_file}\nsecond {result_path}"),
        vec!["agent", "second", "/state/inv/result.json"]
    );
}
