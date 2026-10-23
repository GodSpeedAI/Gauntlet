//! Template rendering: shell-word PARSING into argv, never shell
//! interpolation (spec §4.1; plan Task 16; correction RES-D8).
//!
//! `GAUNTLET_CLI_AGENT_CMD` is operator configuration, so it is UNTRUSTED
//! configuration: it is TOKENIZED into argv words by the POSIX shell-word
//! rules — single quotes, double quotes, backslash escapes, `#` comments
//! at word starts, newlines flattened into separators — and the child is
//! spawned DIRECTLY from those words. No `sh -c` wrapping of the line
//! exists anywhere in this crate, and NO expansion of any kind happens:
//! `$VAR`, `` `…` ``, `$(…)` and globs are data, never grammar.
//!
//! Placeholders — `{prompt_file}`, `{workspace}`, `{result_path}`,
//! `{model}` — are recognized in the ASSEMBLED word text (quotes only
//! group; they never hide or splice a placeholder). Two rules keep
//! substituted values from ever becoming grammar:
//!
//! 1. a placeholder must occupy the WHOLE argv word. `--model={model}` or
//!    `x{model}y` are typed errors ([`TemplateError::EmbeddedPlaceholder`]),
//!    so no substituted value can fuse with adjacent operator text;
//! 2. substituted values are substituted as SINGLE argv words, verbatim:
//!    spaces, quotes, `;`, `|`, `$(` and newlines inside a workspace path
//!    or model hint are CHARACTERS OF ONE ARGV ELEMENT — the child
//!    receives them as data because no shell ever re-splits them.
//!
//! Every malformed shape is a typed error at the parse (the doctor's
//! configuration probe) or at the render (admission) — never a silently
//! unrendered literal reaching a child: empty/whitespace template,
//! unbalanced quote, trailing backslash, an unknown `{…}` name, a
//! dangling `{`, a fused placeholder, `{model}` with no model hint, or an
//! empty argv[0] after substitution.
//!
//! A template that is only `{prompt_file}` is legal-but-odd: argv[0]
//! becomes the prompt file path, which is executably meaningless for a
//! real agent — it parses, it renders, and it is the operator's
//! configuration. Documented, not refused (RES-D8).

use core::fmt;

/// The `{prompt_file}` placeholder: the invocation's prompt file, anchored
/// under the invocation's state directory.
pub const PLACEHOLDER_PROMPT_FILE: &str = "prompt_file";
/// The `{workspace}` placeholder: the workspace directory the child starts in.
pub const PLACEHOLDER_WORKSPACE: &str = "workspace";
/// The `{result_path}` placeholder: the exact file the result contract
/// must be written to (spec §10.3 rule 2).
pub const PLACEHOLDER_RESULT_PATH: &str = "result_path";
/// The `{model}` placeholder: the dispatch's model hint, when the spec
/// carries one.
pub const PLACEHOLDER_MODEL: &str = "model";

/// The known placeholder names, in prompt order.
const KNOWN_PLACEHOLDERS: [&str; 4] = [
    PLACEHOLDER_PROMPT_FILE,
    PLACEHOLDER_WORKSPACE,
    PLACEHOLDER_RESULT_PATH,
    PLACEHOLDER_MODEL,
];

/// Template failures — typed, carrying the offending text (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The template is empty or whitespace-only.
    Empty,
    /// A quote opened and never closed.
    UnterminatedQuote { quote: char, offset: usize },
    /// The template ends in a lone backslash.
    TrailingBackslash { offset: usize },
    /// A `{…}` span names something outside the placeholder vocabulary.
    UnknownPlaceholder { name: String },
    /// A known placeholder is fused with other text in one word — refused
    /// so a substituted value can never splice into operator grammar.
    EmbeddedPlaceholder { word: String, name: String },
    /// A `{` with no closing `}` in its word.
    DanglingBrace { word: String },
    /// `{model}` is in the template but the dispatch carries no model
    /// hint. Fail-closed at admission: a silently empty argument would
    /// hand the child a lie.
    MissingValue { name: String },
    /// argv[0] is the empty word after substitution.
    EmptyProgram,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the command template is empty"),
            Self::UnterminatedQuote { quote, offset } => {
                write!(f, "a {quote} quote at byte {offset} is never closed")
            }
            Self::TrailingBackslash { offset } => {
                write!(f, "the template ends in a lone backslash at byte {offset}")
            }
            Self::UnknownPlaceholder { name } => {
                write!(
                    f,
                    "unknown placeholder {{{name}}} — the vocabulary is \
                     {{{PLACEHOLDER_PROMPT_FILE}}}, {{{PLACEHOLDER_WORKSPACE}}}, \
                     {{{PLACEHOLDER_RESULT_PATH}}}, {{{PLACEHOLDER_MODEL}}}"
                )
            }
            Self::EmbeddedPlaceholder { word, name } => write!(
                f,
                "the placeholder {{{name}}} must be the whole argv word, but {word:?} fuses \
                 it with other text — substituted values must stay single whole words"
            ),
            Self::DanglingBrace { word } => {
                write!(f, "the word {word:?} opens a '{{' with no closing '}}'")
            }
            Self::MissingValue { name } => write!(
                f,
                "the template uses {{{name}}} but this dispatch carries no value for it — \
                 refusing rather than substituting an empty argument"
            ),
            Self::EmptyProgram => {
                write!(
                    f,
                    "argv[0] is empty after substitution — there is no program to run"
                )
            }
        }
    }
}

impl std::error::Error for TemplateError {}

/// One parsed argv word: either operator-literal text or a single
/// whole-word placeholder. Multi-segment words cannot exist — a fused
/// placeholder is refused at the parse.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Word {
    Literal(String),
    Placeholder(String),
}

/// A parsed command template: argv words with placeholders resolved but
/// values not yet substituted. Values are substituted per invocation by
/// [`render`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    words: Vec<Word>,
}

impl Template {
    /// Whether the template uses `{model}` — the honest basis for the
    /// `supports_model_selection` capability report.
    pub fn uses_model(&self) -> bool {
        self.words
            .iter()
            .any(|word| matches!(word, Word::Placeholder(name) if name == PLACEHOLDER_MODEL))
    }

    /// The placeholder names the template uses, in word order (diagnostics).
    pub fn placeholder_names(&self) -> Vec<&str> {
        self.words
            .iter()
            .filter_map(|word| {
                if let Word::Placeholder(name) = word {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Is this character an unquoted word separator (POSIX whitespace)?
fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0B}' | '\u{0C}')
}

/// The value a backslash escapes inside double quotes: POSIX keeps the
/// backslash literal before anything else (no expansion exists here, so
/// `$` and backtick have no special meaning to preserve).
fn double_quote_escape(c: char) -> bool {
    matches!(c, '"' | '\\' | '$' | '`' | '\n')
}

/// Parses the raw template into argv words (POSIX shell-word rules), then
/// validates every `{…}` span. All refusal shapes are typed; nothing is
/// ever silently left unrendered.
pub fn parse_template(source: &str) -> Result<Template, TemplateError> {
    if source.trim().is_empty() {
        return Err(TemplateError::Empty);
    }
    let bytes: Vec<char> = source.chars().collect();
    let mut words: Vec<Word> = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        // Skip separators.
        while index < bytes.len() && is_separator(bytes[index]) {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        // A `#` at word start, outside quotes, begins a comment: the rest
        // of the LINE is skipped (the newline acts as a separator).
        if bytes[index] == '#' {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            continue;
        }
        // Assemble one word's raw text.
        let mut text = String::new();
        let mut word_started = false;
        while index < bytes.len() {
            let c = bytes[index];
            if c == '\'' {
                word_started = true;
                index += 1;
                let open = index;
                while index < bytes.len() && bytes[index] != '\'' {
                    text.push(bytes[index]);
                    index += 1;
                }
                if index >= bytes.len() {
                    return Err(TemplateError::UnterminatedQuote {
                        quote: '\'',
                        offset: open,
                    });
                }
                index += 1; // closing quote
            } else if c == '"' {
                word_started = true;
                index += 1;
                let open = index;
                let mut closed = false;
                while index < bytes.len() {
                    let inner = bytes[index];
                    if inner == '\\' && index + 1 < bytes.len() {
                        let next = bytes[index + 1];
                        if double_quote_escape(next) {
                            if next != '\n' {
                                text.push(next);
                            }
                            index += 2;
                            continue;
                        }
                    }
                    if inner == '"' {
                        closed = true;
                        index += 1;
                        break;
                    }
                    text.push(inner);
                    index += 1;
                }
                if !closed {
                    return Err(TemplateError::UnterminatedQuote {
                        quote: '"',
                        offset: open,
                    });
                }
            } else if c == '\\' {
                let Some(&next) = bytes.get(index + 1) else {
                    return Err(TemplateError::TrailingBackslash { offset: index });
                };
                if next != '\n' {
                    text.push(next);
                }
                word_started = true;
                index += 2;
            } else if is_separator(c) {
                if word_started {
                    break;
                }
                index += 1;
            } else {
                text.push(c);
                word_started = true;
                index += 1;
            }
        }
        words.push(classify_word(text)?);
    }
    // COR-F2: a source whose words are ALL comments yields no program —
    // the same defect as a whitespace-only template, so it refuses the
    // same way, at parse time where the doctor's configuration probe can
    // report it honestly (never clean-here, refused-at-admission).
    if words.is_empty() {
        return Err(TemplateError::Empty);
    }
    Ok(Template { words })
}

/// The doctor-usable configuration probe (COR-F2): a command template is
/// usable only if it PARSES and RENDERS against every declared
/// placeholder with synthetic values. This is the whole shape check — a
/// template that parses but cannot render (empty program, missing value)
/// is reported unusable here, never clean-here-refused-at-admission.
pub fn probe_template(source: &str) -> Result<(), TemplateError> {
    let template = parse_template(source)?;
    let synthetic = TemplateValues {
        prompt_file: "/probe/prompt",
        workspace: "/probe/workspace",
        result_path: "/probe/result",
        model: Some("probe-model"),
    };
    let _ = render(&template, &synthetic)?;
    Ok(())
}

/// Classifies one assembled word: a single whole-word known placeholder,
/// pure literal text, or a typed refusal.
fn classify_word(text: String) -> Result<Word, TemplateError> {
    let open = text.find('{');
    let Some(open) = open else {
        return Ok(Word::Literal(text));
    };
    let close = text[open..].find('}').map(|at| open + at);
    let Some(close) = close else {
        return Err(TemplateError::DanglingBrace { word: text });
    };
    let name = text[open + 1..close].to_owned();
    if !KNOWN_PLACEHOLDERS.contains(&name.as_str()) {
        return Err(TemplateError::UnknownPlaceholder { name });
    }
    if open != 0 || close + 1 != text.len() {
        return Err(TemplateError::EmbeddedPlaceholder { word: text, name });
    }
    Ok(Word::Placeholder(name))
}

/// The per-invocation values substituted for the placeholders. Paths are
/// passed as strings and become single argv words verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateValues<'a> {
    pub prompt_file: &'a str,
    pub workspace: &'a str,
    pub result_path: &'a str,
    pub model: Option<&'a str>,
}

/// Renders the template into argv for one dispatch. Placeholders become
/// their values as SINGLE argv words; anything malformed is a typed error
/// at admission, never a silently unrendered literal.
pub fn render(template: &Template, values: &TemplateValues) -> Result<Vec<String>, TemplateError> {
    let mut argv = Vec::with_capacity(template.words.len());
    for word in &template.words {
        match word {
            Word::Literal(text) => argv.push(text.clone()),
            Word::Placeholder(name) => {
                let value = match name.as_str() {
                    PLACEHOLDER_PROMPT_FILE => values.prompt_file,
                    PLACEHOLDER_WORKSPACE => values.workspace,
                    PLACEHOLDER_RESULT_PATH => values.result_path,
                    PLACEHOLDER_MODEL => values.model.ok_or(TemplateError::MissingValue {
                        name: PLACEHOLDER_MODEL.to_owned(),
                    })?,
                    // Unreachable: the parse refuses unknown names.
                    other => {
                        return Err(TemplateError::UnknownPlaceholder {
                            name: other.to_owned(),
                        });
                    }
                };
                argv.push(value.to_owned());
            }
        }
    }
    if argv.first().is_none_or(String::is_empty) {
        return Err(TemplateError::EmptyProgram);
    }
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn words(source: &str) -> Vec<String> {
        parse_template(source)
            .and_then(|t| render(&t, &values("/p", "/w", "/r", Some("m"))))
            .expect("template renders")
    }

    #[test]
    fn plain_words_quotes_and_placeholders_parse_into_argv() {
        let argv = words("agent --prompt {prompt_file} --dir \"{workspace}\" --out {result_path}");
        assert_eq!(
            argv,
            ["agent", "--prompt", "/p", "--dir", "/w", "--out", "/r"]
        );
        // Single quotes group exactly like double quotes.
        assert_eq!(words("sh -c 'two words' x"), ["sh", "-c", "two words", "x"]);
        // Backslash escapes outside quotes; POSIX double-quote escapes.
        assert_eq!(
            words(r#"a\ b \"quoted\" \$keep"#),
            [r#"a b"#, "\"quoted\"", "$keep"]
        );
        // A quoted empty word stays an (empty) argument.
        assert_eq!(words("x '' y"), ["x", "", "y"]);
    }

    #[test]
    fn comments_and_newlines_flatten_and_hash_only_comments_at_word_start() {
        assert_eq!(
            words("# a comment line\nagent {prompt_file}"),
            ["agent", "/p"]
        );
        // `#` mid-word is an ordinary character.
        assert_eq!(words("a#b"), ["a#b"]);
        // Newlines separate words; a line-end backslash is a POSIX
        // continuation (removed), which still leaves two words here
        // because the space before it already ended the first word.
        assert_eq!(words("one\ntwo\nthree"), ["one", "two", "three"]);
        assert_eq!(words("one\\\ntwo"), ["onetwo"]);
    }

    #[test]
    fn no_expansion_ever_happens() {
        // $VAR, $(…), globs and backticks are DATA.
        assert_eq!(
            words("$HOME $(rm -rf /) *.txt `x`"),
            ["$HOME", "$(rm", "-rf", "/)", "*.txt", "`x`"]
        );
    }

    #[test]
    fn substituted_values_stay_single_verbatim_words() {
        // A hostile workspace path full of metacharacters is one argv word.
        let template = parse_template("agent --dir {workspace}").expect("parses");
        let hostile = "/w s;|rm -rf ~&$(x)`y\"'";
        let argv = render(&template, &values("/p", hostile, "/r", None)).expect("renders");
        assert_eq!(argv, ["agent", "--dir", hostile]);
        assert_eq!(argv.len(), 3, "no value ever splits into grammar");
    }

    #[test]
    fn every_refusal_shape_is_typed() {
        for (source, expected) in [
            ("   ", TemplateError::Empty),
            (
                "'never closed",
                TemplateError::UnterminatedQuote {
                    quote: '\'',
                    offset: 1,
                },
            ),
            ("trailing\\", TemplateError::TrailingBackslash { offset: 8 }),
            (
                "agent {fabricated}",
                TemplateError::UnknownPlaceholder {
                    name: "fabricated".to_owned(),
                },
            ),
            (
                "agent {model",
                TemplateError::DanglingBrace {
                    word: "{model".to_owned(),
                },
            ),
            (
                "agent --m=x{model}y",
                TemplateError::EmbeddedPlaceholder {
                    word: "--m=x{model}y".to_owned(),
                    name: "model".to_owned(),
                },
            ),
        ] {
            assert_eq!(parse_template(source).unwrap_err(), expected, "{source}");
        }
        // {model} with no hint refuses at RENDER (admission), not parse.
        let template = parse_template("agent --model {model}").expect("parses");
        assert_eq!(
            render(&template, &values("/p", "/w", "/r", None)).unwrap_err(),
            TemplateError::MissingValue {
                name: "model".to_owned()
            }
        );
        // An empty argv[0] after substitution refuses: there is no program.
        let template = parse_template("'' {prompt_file}").expect("parses");
        assert_eq!(
            render(&template, &values("/p", "/w", "/r", None)).unwrap_err(),
            TemplateError::EmptyProgram
        );
    }

    #[test]
    fn legal_but_odd_bare_prompt_template_parses_and_is_documented() {
        // A template that is only {prompt_file} is legal-but-odd: argv[0]
        // becomes the prompt file path. Documented, not refused (RES-D8).
        let template = parse_template("{prompt_file}").expect("parses");
        let argv = render(&template, &values("/p", "/w", "/r", None)).expect("renders");
        assert_eq!(argv, ["/p"]);
        assert!(!template.uses_model());
        assert_eq!(
            parse_template("agent {model}")
                .expect("parses")
                .placeholder_names(),
            ["model"]
        );
    }
}
