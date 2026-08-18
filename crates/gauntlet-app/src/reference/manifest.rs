//! The strict manifest reader (plan Task 7 step 3) — a narrow parser for
//! the frozen `.wayfinder/reference.yaml` shape: ONE root `reference:`
//! mapping carrying scalars, `>` folded scalars, `- ` block sequences of
//! scalars and of mappings, and flow sequences (`[a, b]`, `[]`).
//!
//! This is NOT a YAML implementation. It is a frozen-artifact reader: the
//! manifest is the ruler's narrative half, its shape is known, and drift
//! fails loudly with the line — an unknown key, an unexpected shape, a
//! duplicate key, or a value outside the domain's vocabulary is a typed
//! error, never a skipped field (spec §25: no competing sources of truth).
//!
//! ## Documented deferrals (do not invent the ruler's semantics)
//!
//! Two manifest sections name evidence as bare locator strings, while the
//! domain types require full evidence requirements (kind + locator +
//! producer). Inventing kind/producer mappings would be inventing the
//! ruler's semantics, so neither is materialized at this task:
//!
//! - `quality_dimensions` — parsed for shape, counted, and NOT materialized
//!   into the domain `Reference`; the constructed `ReferenceDefinition`
//!   carries an empty `quality_dimensions`. Materialization waits for the
//!   comparison-contract task to define the mapping.
//! - `settlement.required_evidence` — prose locator text, carried verbatim
//!   as [`ManifestDefinition::required_evidence_locators`]; the constructed
//!   `SettlementCriteria` carries an empty `required_evidence`.
//!
//! `minimum_reliability` is folded PROSE in the manifest ("spec §19.2
//! default: >= 3 settlements …"). The domain wants a pass count, so the
//! narrow extraction is the `>= N settlements` pattern inside that prose —
//! present and numeric, or a typed error. The manifest names no other
//! number; if its prose moves, this parse fails loudly instead of silently
//! keeping a stale count.
//!
//! `success_criteria` are carried as strings verbatim; `hard_units` is
//! empty (the manifest names none). The manifest's own `digest` key must
//! be `null` or absent — a self-referential digest cannot be consistent
//! (the manifest's own header says so); the freeze record that carries it
//! is `.gauntlet/reference.lock`, which is not this module's business.

use core::fmt;

use gauntlet_domain::{
    Assumption, Digest, ReferenceDefinition, ReferenceId, SettlementCriteria, TerminalState, Unit,
    Unknown,
};

/// Manifest failures — typed, each carrying the 1-based line of the drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// A root line that is not a `key:` mapping entry.
    NotAMapping { line: usize },
    /// A root key other than `reference`.
    UnknownRootKey { line: usize, key: String },
    /// A key outside the frozen shape of the mapping it sits in.
    UnknownKey { line: usize, key: String },
    /// A key appears twice in one mapping.
    DuplicateKey { line: usize, key: String },
    /// A required key never appeared.
    MissingKey { key: &'static str },
    /// A value is not the shape the frozen manifest carries.
    BadShape {
        line: usize,
        key: String,
        expected: &'static str,
        got: &'static str,
    },
    /// `version` is not a plain unsigned integer.
    NotAnInteger { line: usize, value: String },
    /// `id` is not a valid `ReferenceId`.
    InvalidId { line: usize, value: String },
    /// The manifest's own `digest` key carries a value; it must be null.
    DigestNotNull { line: usize, value: String },
    /// `units` carries entries; the answer key is the single ruler for Units.
    UnitsNotEmpty { line: usize, count: usize },
    /// A folded scalar with no continuation content.
    EmptyFolded { line: usize, key: String },
    /// `terminal_conditions` carries a value outside the §5.2 vocabulary.
    UnknownTerminal { line: usize, value: String },
    /// The reliability prose carries no `>= N settlements` statement.
    ReliabilityUnparseable { line: usize, text: String },
    /// The reliability prose names `>= 0 settlements`: a minimum of zero
    /// reliability passes is below the §19.2 floor of one and is refused at
    /// parse time, so `build_definition`'s `SettlementCriteria::new` cannot
    /// fail (untrusted input must never reach a panic path).
    ReliabilityBelowOne { line: usize },
    /// A block sequence item that is neither a scalar nor a mapping, or an
    /// inconsistent-indentation line the shape cannot place.
    MalformedItem { line: usize, detail: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAMapping { line } => {
                write!(f, "line {line}: the manifest root is not a `key:` mapping")
            }
            Self::UnknownRootKey { line, key } => write!(
                f,
                "line {line}: root key {key:?} is outside the frozen shape (one `reference` \
                 mapping only)"
            ),
            Self::UnknownKey { line, key } => {
                write!(
                    f,
                    "line {line}: key {key:?} is outside the frozen manifest shape"
                )
            }
            Self::DuplicateKey { line, key } => {
                write!(f, "line {line}: key {key:?} appears twice in one mapping")
            }
            Self::MissingKey { key } => {
                write!(f, "the manifest is missing its required key {key:?}")
            }
            Self::BadShape {
                line,
                key,
                expected,
                got,
            } => write!(f, "line {line}: key {key:?} must be {expected}, got {got}"),
            Self::NotAnInteger { line, value } => {
                write!(
                    f,
                    "line {line}: version {value:?} is not a plain unsigned integer"
                )
            }
            Self::InvalidId { line, value } => {
                write!(f, "line {line}: id {value:?} is not a valid reference id")
            }
            Self::DigestNotNull { line, value } => write!(
                f,
                "line {line}: the manifest's own digest key carries {value:?}; it must be null \
                 — a self-referential digest cannot be consistent, and the lock is the freeze \
                 record"
            ),
            Self::UnitsNotEmpty { line, count } => write!(
                f,
                "line {line}: the manifest carries {count} units entries; the answer key is the \
                 single ruler for Units — two copies of the same claims are the competing \
                 source of truth spec 25 forbids"
            ),
            Self::EmptyFolded { line, key } => {
                write!(f, "line {line}: folded scalar {key:?} carries no content")
            }
            Self::UnknownTerminal { line, value } => write!(
                f,
                "line {line}: terminal condition {value:?} is not in the section 5.2 vocabulary"
            ),
            Self::ReliabilityUnparseable { line, text } => write!(
                f,
                "line {line}: the reliability prose carries no `>= N settlements` statement: \
                 {text:?}"
            ),
            Self::ReliabilityBelowOne { line } => write!(
                f,
                "line {line}: the reliability prose names `>= 0 settlements` — below the \
                 section 19.2 floor of one"
            ),
            Self::MalformedItem { line, detail } => {
                write!(f, "line {line}: a malformed sequence item: {detail}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

// ---------------------------------------------------------------------------
// The tree: a strict YAML-subset value
// ---------------------------------------------------------------------------

/// One physical line, pre-lexed.
struct PhysLine {
    number: usize,
    indent: usize,
    text: String,
}

/// A parsed value in the subset.
enum Node {
    /// A plain scalar, `null` and booleans included.
    Scalar { line: usize, value: String },
    /// A `>` folded scalar, folded per the module header.
    Folded { line: usize, text: String },
    /// A block or flow sequence.
    Seq { line: usize, items: Vec<Node> },
    /// A nested mapping.
    Map {
        line: usize,
        entries: Vec<(String, Node)>,
    },
}

impl Node {
    fn line(&self) -> usize {
        match self {
            Self::Scalar { line, .. } | Self::Folded { line, .. } => *line,
            Self::Seq { line, .. } | Self::Map { line, .. } => *line,
        }
    }

    fn shape(&self) -> &'static str {
        match self {
            Self::Scalar { .. } => "a scalar",
            Self::Folded { .. } => "a folded scalar",
            Self::Seq { .. } => "a sequence",
            Self::Map { .. } => "a mapping",
        }
    }
}

/// Lexes the text into physical lines. Full-line comments and blank lines
/// are dropped (the frozen manifest carries comments only at line start);
/// inline `#` is content, not a comment — this reader never guesses where a
/// value ends. A tab in the indentation is drift, not style.
fn lex(text: &str) -> Result<Vec<PhysLine>, ManifestError> {
    let mut out = Vec::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        // The leading run of spaces and tabs ONLY: a tab anywhere in it is
        // drift, not style (the guard must see the whole run, not a
        // spaces-only prefix), while exotic whitespace (NBSP, U+2028) is not
        // indentation at all — it stays in the body and fails loudly
        // downstream, exactly as before this guard existed.
        let body_start = line.len() - line.trim_start_matches([' ', '\t']).len();
        if line[..body_start].contains('\t') {
            return Err(ManifestError::MalformedItem {
                line: index + 1,
                detail: "a tab in the indentation".to_owned(),
            });
        }
        let indent = body_start;
        let body = &line[indent..];
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        out.push(PhysLine {
            number: index + 1,
            indent,
            text: body.to_owned(),
        });
    }
    Ok(out)
}

/// Parses the whole document: exactly one root mapping with exactly one
/// key, `reference`.
fn parse_tree(text: &str) -> Result<Node, ManifestError> {
    let lines = lex(text)?;
    let mut pos = 0;
    let mut entries = Vec::new();
    while pos < lines.len() {
        let (key, rest, number) = key_line(&lines[pos])?;
        if key != "reference" {
            return Err(ManifestError::UnknownRootKey { line: number, key });
        }
        let node = value_of(&lines, &mut pos, rest, number, &key)?;
        entries.push((key, node));
    }
    match entries.len() {
        1 => Ok(entries.remove(0).1),
        _ => Err(ManifestError::NotAMapping { line: 1 }),
    }
}

/// Splits `key: rest` at the first colon, strictly: the colon is followed
/// by nothing (nested value) or by exactly one space.
fn key_line(line: &PhysLine) -> Result<(String, String, usize), ManifestError> {
    let Some((key, rest)) = line.text.split_once(':') else {
        return Err(ManifestError::NotAMapping { line: line.number });
    };
    if key.is_empty() || key.contains(' ') {
        return Err(ManifestError::NotAMapping { line: line.number });
    }
    let rest = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
    Ok((key.to_owned(), rest, line.number))
}

/// Parses the value for the key line at `pos`: an inline scalar or flow
/// sequence, a `>` folded block, a nested mapping, or a block sequence.
///
/// Position contract (the whole traversal depends on it): `pos` sits ON the
/// key line on entry and every return path leaves it PAST the key line and
/// the key's entire value block — a caller's `while` over `lines` can rely
/// on progress. `number` is the key line's own physical line number, so
/// block-level errors (e.g. `EmptyFolded`) name the key, not a body line.
fn value_of(
    lines: &[PhysLine],
    pos: &mut usize,
    rest: String,
    number: usize,
    key: &str,
) -> Result<Node, ManifestError> {
    let key_indent = lines[*pos].indent;
    // The folded-block marker travels in the key line's remainder
    // (`purpose: >`); its body is the run of deeper-indented lines after it.
    if rest == ">" {
        *pos += 1;
        return folded(lines, pos, key_indent, number, key);
    }
    if !rest.is_empty() {
        *pos += 1;
        if let Some(node) = flow_value(&rest, number)? {
            return Ok(node);
        }
        return Ok(Node::Scalar {
            line: number,
            value: rest,
        });
    }
    // The value is on following lines, or the key maps to nothing at all
    // (an explicit empty).
    match lines.get(*pos + 1) {
        Some(next) if next.indent > key_indent => {
            *pos += 1;
            if next.text.starts_with("- ") || next.text == "-" {
                return block_sequence(lines, pos);
            }
            mapping(lines, pos, next.indent)
        }
        _ => {
            *pos += 1;
            Ok(Node::Seq {
                line: number,
                items: Vec::new(),
            })
        }
    }
}

/// A folded scalar: entered with `pos` on the first body line, it consumes
/// the run of lines strictly more indented than the key, trims each, and
/// joins the non-empty ones with single spaces. (Body lines are never empty
/// — `lex` drops blank lines.) Zero content is an error — an empty narrative
/// is not a narrative.
fn folded(
    lines: &[PhysLine],
    pos: &mut usize,
    base_indent: usize,
    number: usize,
    key: &str,
) -> Result<Node, ManifestError> {
    let mut parts: Vec<String> = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent <= base_indent {
            break;
        }
        parts.push(line.text.trim().to_owned());
        *pos += 1;
    }
    if parts.is_empty() {
        return Err(ManifestError::EmptyFolded {
            line: number,
            key: key.to_owned(),
        });
    }
    Ok(Node::Folded {
        line: number,
        text: parts.join(" "),
    })
}

/// A block sequence: `- item` lines at one indent. An item is a scalar, or
/// a mapping whose first key sits in the item line itself and whose further
/// keys continue exactly two columns deeper (the frozen shape).
fn block_sequence(lines: &[PhysLine], pos: &mut usize) -> Result<Node, ManifestError> {
    let item_indent = lines[*pos].indent;
    let line_number = lines[*pos].number;
    let mut items = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent != item_indent {
            break;
        }
        let Some(rest) = line.text.strip_prefix("- ") else {
            break;
        };
        if rest.is_empty() {
            // `- ` alone: the item is the nested block on the next lines.
            *pos += 1;
            match lines.get(*pos) {
                Some(next) if next.indent > item_indent => {
                    items.push(mapping(lines, pos, next.indent)?);
                }
                _ => {
                    return Err(ManifestError::MalformedItem {
                        line: line.number,
                        detail: "a bare dash with no item".to_owned(),
                    });
                }
            }
            continue;
        }
        if rest.contains(": ") || rest.ends_with(':') {
            // A mapping item: the first key starts at item_indent + 2. The
            // item's whole subtree (further keys at key_indent, and the
            // deeper bodies of folded values) is collected with its physical
            // indents — mapping/folded enforce the shape from there.
            let key_indent = item_indent + 2;
            let mut sub: Vec<PhysLine> = vec![PhysLine {
                number: line.number,
                indent: key_indent,
                text: rest.to_owned(),
            }];
            *pos += 1;
            while let Some(next) = lines.get(*pos) {
                if next.indent <= item_indent {
                    break;
                }
                sub.push(PhysLine {
                    number: next.number,
                    indent: next.indent,
                    text: next.text.clone(),
                });
                *pos += 1;
            }
            let mut sub_pos = 0;
            items.push(mapping(&sub, &mut sub_pos, key_indent)?);
            continue;
        }
        // A plain scalar item may wrap onto deeper-indented lines (the
        // frozen manifest wraps several); the continuation folds with
        // single spaces, exactly as folded scalars do. A dash at that
        // depth is structural, not text — refused loudly.
        let mut parts = vec![rest.to_owned()];
        *pos += 1;
        while let Some(next) = lines.get(*pos) {
            if next.indent <= item_indent {
                break;
            }
            if next.text == "-" || next.text.starts_with("- ") {
                return Err(ManifestError::MalformedItem {
                    line: next.number,
                    detail: "a nested dash inside a scalar item".to_owned(),
                });
            }
            parts.push(next.text.trim().to_owned());
            *pos += 1;
        }
        items.push(Node::Scalar {
            line: line.number,
            value: parts.join(" "),
        });
    }
    Ok(Node::Seq {
        line: line_number,
        items,
    })
}

/// A nested mapping at one indent.
fn mapping(lines: &[PhysLine], pos: &mut usize, indent: usize) -> Result<Node, ManifestError> {
    let line_number = lines[*pos].number;
    let mut entries: Vec<(String, Node)> = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ManifestError::MalformedItem {
                line: line.number,
                detail: format!(
                    "line sits deeper ({}) than its mapping's column {indent}",
                    line.indent
                ),
            });
        }
        let (key, rest, number) = key_line(line)?;
        // `value_of` is entered with `pos` on the key line and consumes the
        // key line plus its whole value block (its position contract).
        let node = value_of(lines, pos, rest, number, &key)?;
        entries.push((key, node));
    }
    Ok(Node::Map {
        line: line_number,
        entries,
    })
}

/// A flow value: `[a, b]` (a sequence of scalars) or `[]`. Anything else is
/// not a flow value.
fn flow_value(text: &str, line: usize) -> Result<Option<Node>, ManifestError> {
    let Some(body) = text.strip_prefix('[') else {
        return Ok(None);
    };
    let Some(body) = body.strip_suffix(']') else {
        return Err(ManifestError::MalformedItem {
            line,
            detail: format!("flow sequence {text:?} does not close its bracket"),
        });
    };
    let mut items = Vec::new();
    if !body.trim().is_empty() {
        for entry in body.split(',') {
            let value = entry.trim();
            // An empty entry (`[a,,b]`, `[,]`) is drift, not a silently
            // admitted blank value — block sequences refuse empties and so
            // does the answer key's depends_on (critic-B F6).
            if value.is_empty() {
                return Err(ManifestError::MalformedItem {
                    line,
                    detail: format!("flow sequence {text:?} carries an empty entry"),
                });
            }
            items.push(Node::Scalar {
                line,
                value: value.to_owned(),
            });
        }
    }
    Ok(Some(Node::Seq { line, items }))
}

// ---------------------------------------------------------------------------
// The tree, interpreted against the frozen shape
// ---------------------------------------------------------------------------

/// What the manifest contributes to a `ReferenceDefinition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDefinition {
    pub id: ReferenceId,
    pub version: u32,
    pub source_refs: Vec<String>,
    pub purpose: String,
    pub desired_state: String,
    /// `{id, statement}` pairs rendered `"<id>: <statement>"` so the
    /// invariant's id survives into the definition (I1, I8, …).
    pub invariants: Vec<String>,
    pub constraints: Vec<String>,
    pub anti_goals: Vec<String>,
    pub unknowns: Vec<Unknown>,
    pub assumptions: Vec<Assumption>,
    pub success_criteria: Vec<String>,
    /// `settlement.required_evidence` as written — prose locator text,
    /// carried verbatim and deliberately NOT materialized into
    /// `EvidenceKind`s (see the module header's deferrals).
    pub required_evidence_locators: Vec<String>,
    pub minimum_reliability_passes: u32,
    pub terminal_conditions: Vec<TerminalState>,
    /// Parsed for shape, counted, not materialized (same deferral).
    pub quality_dimension_count: usize,
}

/// The keys the `reference` mapping may carry.
const REFERENCE_KEYS: &[&str] = &[
    "id",
    "version",
    "digest",
    "source_refs",
    "purpose",
    "desired_state",
    "invariants",
    "constraints",
    "anti_goals",
    "units",
    "quality_dimensions",
    "settlement",
    "unknowns",
    "assumptions",
];

/// The keys each `quality_dimensions` entry may carry.
const QUALITY_KEYS: &[&str] = &[
    "id",
    "description",
    "desired_condition",
    "evidence_required",
    "comparison_method",
    "threshold",
    "weight",
    "blocking",
];

/// The keys `settlement` may carry.
const SETTLEMENT_KEYS: &[&str] = &[
    "success_criteria",
    "required_evidence",
    "minimum_reliability",
    "terminal_conditions",
];

/// The keys each `unknowns` entry may carry.
const UNKNOWN_KEYS: &[&str] = &["id", "question", "status", "recorded_at", "note"];

/// Parses the frozen manifest shape into a [`ManifestDefinition`].
pub fn parse_manifest(text: &str) -> Result<ManifestDefinition, ManifestError> {
    let entries = match parse_tree(text)? {
        Node::Map { entries, .. } => entries,
        _ => return Err(ManifestError::NotAMapping { line: 1 }),
    };
    let entry = |name: &str| entries.iter().find(|(key, _)| key == name);
    for (key, node) in &entries {
        if !REFERENCE_KEYS.contains(&key.as_str()) {
            return Err(ManifestError::UnknownKey {
                line: node.line(),
                key: key.clone(),
            });
        }
    }
    duplicates(&entries)?;

    let id = scalar_of(entry("id"), "id")?;
    let id = ReferenceId::new(&id.value).map_err(|_| ManifestError::InvalidId {
        line: id.line,
        value: id.value.clone(),
    })?;
    let version = scalar_of(entry("version"), "version")?;
    let version: u32 = version
        .value
        .parse()
        .map_err(|_| ManifestError::NotAnInteger {
            line: version.line,
            value: version.value.clone(),
        })?;
    if let Some((_, node)) = entry("digest") {
        let Node::Scalar { line, value } = node else {
            return Err(ManifestError::BadShape {
                line: node.line(),
                key: "digest".to_owned(),
                expected: "the scalar null",
                got: node.shape(),
            });
        };
        if value != "null" {
            return Err(ManifestError::DigestNotNull {
                line: *line,
                value: value.clone(),
            });
        }
    }
    let source_refs = scalar_sequence(entry("source_refs"), "source_refs")?
        .into_iter()
        .map(|at| at.value)
        .collect::<Vec<_>>();
    let purpose = narrative(entry("purpose"), "purpose")?;
    let desired_state = narrative(entry("desired_state"), "desired_state")?;
    let invariants = invariants_of(entry("invariants"))?;
    let constraints = scalar_sequence(entry("constraints"), "constraints")?
        .into_iter()
        .map(|at| at.value)
        .collect::<Vec<_>>();
    let anti_goals = scalar_sequence(entry("anti_goals"), "anti_goals")?
        .into_iter()
        .map(|at| at.value)
        .collect::<Vec<_>>();
    if let Some((_, node)) = entry("units") {
        let Node::Seq { line, items } = node else {
            return Err(ManifestError::BadShape {
                line: node.line(),
                key: "units".to_owned(),
                expected: "an empty sequence",
                got: node.shape(),
            });
        };
        if !items.is_empty() {
            return Err(ManifestError::UnitsNotEmpty {
                line: *line,
                count: items.len(),
            });
        }
    }
    let quality_dimension_count = quality_dimensions_of(entry("quality_dimensions"))?;
    let settlement = settlement_of(entry("settlement"))?;
    let unknowns = unknowns_of(entry("unknowns"))?;
    let assumptions = scalar_sequence(entry("assumptions"), "assumptions")?
        .into_iter()
        .map(|text| Assumption::new(text.value))
        .collect::<Vec<_>>();

    Ok(ManifestDefinition {
        id,
        version,
        source_refs,
        purpose,
        desired_state,
        invariants,
        constraints,
        anti_goals,
        unknowns,
        assumptions,
        success_criteria: settlement.success_criteria,
        required_evidence_locators: settlement.required_evidence,
        minimum_reliability_passes: settlement.minimum_reliability,
        terminal_conditions: settlement.terminal_conditions,
        quality_dimension_count,
    })
}

/// One required entry, checked present before use.
fn required<'a>(
    entry: Option<&'a (String, Node)>,
    key: &'static str,
) -> Result<&'a (String, Node), ManifestError> {
    entry.ok_or(ManifestError::MissingKey { key })
}

/// A located scalar.
struct At {
    line: usize,
    value: String,
}

/// The scalar of a required entry.
fn scalar_of(entry: Option<&(String, Node)>, key: &'static str) -> Result<At, ManifestError> {
    let (_, node) = required(entry, key)?;
    match node {
        Node::Scalar { line, value } => Ok(At {
            line: *line,
            value: value.clone(),
        }),
        other => Err(ManifestError::BadShape {
            line: other.line(),
            key: key.to_owned(),
            expected: "a scalar",
            got: other.shape(),
        }),
    }
}

/// A narrative field: a folded scalar or a plain scalar, non-empty either way.
fn narrative(entry: Option<&(String, Node)>, key: &'static str) -> Result<String, ManifestError> {
    let (_, node) = required(entry, key)?;
    let text = match node {
        Node::Folded { text, .. } => text.clone(),
        Node::Scalar { value, .. } => value.clone(),
        other => {
            return Err(ManifestError::BadShape {
                line: other.line(),
                key: key.to_owned(),
                expected: "a (folded) scalar",
                got: other.shape(),
            });
        }
    };
    if text.trim().is_empty() {
        return Err(ManifestError::EmptyFolded {
            line: node.line(),
            key: key.to_owned(),
        });
    }
    Ok(text)
}

/// The values of a scalar sequence (absent key = empty sequence).
fn scalar_sequence(
    entry: Option<&(String, Node)>,
    key: &'static str,
) -> Result<Vec<At>, ManifestError> {
    match entry {
        None => Ok(Vec::new()),
        Some((_, Node::Seq { items, .. })) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Node::Scalar { line, value } => out.push(At {
                        line: *line,
                        value: value.clone(),
                    }),
                    other => {
                        return Err(ManifestError::BadShape {
                            line: other.line(),
                            key: key.to_owned(),
                            expected: "a sequence of scalars",
                            got: other.shape(),
                        });
                    }
                }
            }
            Ok(out)
        }
        Some((_, other)) => Err(ManifestError::BadShape {
            line: other.line(),
            key: key.to_owned(),
            expected: "a sequence of scalars",
            got: other.shape(),
        }),
    }
}

/// Duplicate keys inside one mapping are drift, not last-wins.
fn duplicates(entries: &[(String, Node)]) -> Result<(), ManifestError> {
    for (index, (key, _)) in entries.iter().enumerate() {
        if entries[index + 1..].iter().any(|(other, _)| other == key) {
            let later = entries[index + 1..]
                .iter()
                .find(|(other, _)| other == key)
                .expect("the any() above found this duplicate");
            return Err(ManifestError::DuplicateKey {
                line: later.1.line(),
                key: key.clone(),
            });
        }
    }
    Ok(())
}

/// `invariants`: a sequence of `{id, statement}` mappings, rendered
/// `"<id>: <statement>"`.
fn invariants_of(entry: Option<&(String, Node)>) -> Result<Vec<String>, ManifestError> {
    let Some((_, Node::Seq { items, .. })) = entry else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Node::Map { entries, .. } = item else {
            return Err(ManifestError::BadShape {
                line: item.line(),
                key: "invariants".to_owned(),
                expected: "a sequence of id/statement mappings",
                got: item.shape(),
            });
        };
        duplicates(entries)?;
        for (key, node) in entries {
            if !["id", "statement"].contains(&key.as_str()) {
                return Err(ManifestError::UnknownKey {
                    line: node.line(),
                    key: key.clone(),
                });
            }
        }
        let id = scalar_of(entries.iter().find(|(key, _)| key == "id"), "invariants.id")?;
        let statement = scalar_of(
            entries.iter().find(|(key, _)| key == "statement"),
            "invariants.statement",
        )?;
        out.push(format!("{}: {}", id.value, statement.value));
    }
    Ok(out)
}

/// `quality_dimensions`: shape-checked, counted, not materialized.
fn quality_dimensions_of(entry: Option<&(String, Node)>) -> Result<usize, ManifestError> {
    let Some((_, Node::Seq { items, .. })) = entry else {
        return Ok(0);
    };
    for item in items {
        let Node::Map { entries, .. } = item else {
            return Err(ManifestError::BadShape {
                line: item.line(),
                key: "quality_dimensions".to_owned(),
                expected: "a sequence of mappings",
                got: item.shape(),
            });
        };
        duplicates(entries)?;
        for (key, node) in entries {
            if !QUALITY_KEYS.contains(&key.as_str()) {
                return Err(ManifestError::UnknownKey {
                    line: node.line(),
                    key: key.clone(),
                });
            }
            let shaped = match node {
                Node::Scalar { .. } | Node::Folded { .. } => true,
                Node::Seq { items, .. } => items
                    .iter()
                    .all(|inner| matches!(inner, Node::Scalar { .. })),
                Node::Map { .. } => false,
            };
            if !shaped {
                return Err(ManifestError::BadShape {
                    line: node.line(),
                    key: key.clone(),
                    expected: "a scalar or a sequence of scalars",
                    got: node.shape(),
                });
            }
        }
    }
    Ok(items.len())
}

/// `settlement`: the narrative fields the domain `SettlementCriteria` needs.
struct Settlement {
    success_criteria: Vec<String>,
    required_evidence: Vec<String>,
    minimum_reliability: u32,
    terminal_conditions: Vec<TerminalState>,
}

fn settlement_of(entry: Option<&(String, Node)>) -> Result<Settlement, ManifestError> {
    let entries = match &required(entry, "settlement")?.1 {
        Node::Map { entries, .. } => entries,
        other => {
            return Err(ManifestError::BadShape {
                line: other.line(),
                key: "settlement".to_owned(),
                expected: "a mapping",
                got: other.shape(),
            });
        }
    };
    for (key, node) in entries {
        if !SETTLEMENT_KEYS.contains(&key.as_str()) {
            return Err(ManifestError::UnknownKey {
                line: node.line(),
                key: key.clone(),
            });
        }
    }
    duplicates(entries)?;
    let success_criteria = scalar_sequence(
        entries.iter().find(|(key, _)| key == "success_criteria"),
        "success_criteria",
    )?
    .into_iter()
    .map(|at| at.value)
    .collect::<Vec<_>>();
    let required_evidence = scalar_sequence(
        entries.iter().find(|(key, _)| key == "required_evidence"),
        "required_evidence",
    )?
    .into_iter()
    .map(|at| at.value)
    .collect::<Vec<_>>();
    let reliability_at = narrative(
        entries.iter().find(|(key, _)| key == "minimum_reliability"),
        "minimum_reliability",
    )?;
    let reliability_node = entries
        .iter()
        .find(|(key, _)| key == "minimum_reliability")
        .expect("narrative checked presence above");
    let minimum_reliability =
        reliability_from(&reliability_at).ok_or(ManifestError::ReliabilityUnparseable {
            line: reliability_node.1.line(),
            text: reliability_at,
        })?;
    // Critic-B F1: zero is parseable prose but an illegal minimum — refuse
    // it HERE, at admission, so `build_definition`'s `SettlementCriteria`
    // construction can never fail on untrusted input (no panic paths).
    if minimum_reliability == 0 {
        return Err(ManifestError::ReliabilityBelowOne {
            line: reliability_node.1.line(),
        });
    }
    let mut terminal_conditions = Vec::new();
    if let Some((_, node)) = entries.iter().find(|(key, _)| key == "terminal_conditions") {
        let Node::Seq { items, .. } = node else {
            return Err(ManifestError::BadShape {
                line: node.line(),
                key: "terminal_conditions".to_owned(),
                expected: "a flow sequence of section 5.2 tokens",
                got: node.shape(),
            });
        };
        for item in items {
            let Node::Scalar { line, value } = item else {
                return Err(ManifestError::BadShape {
                    line: item.line(),
                    key: "terminal_conditions".to_owned(),
                    expected: "a flow sequence of section 5.2 tokens",
                    got: item.shape(),
                });
            };
            terminal_conditions.push(TerminalState::parse(value).map_err(|_| {
                ManifestError::UnknownTerminal {
                    line: *line,
                    value: value.clone(),
                }
            })?);
        }
    }
    Ok(Settlement {
        success_criteria,
        required_evidence,
        minimum_reliability,
        terminal_conditions,
    })
}

/// `>= N settlements` — the one number the frozen prose names.
fn reliability_from(text: &str) -> Option<u32> {
    let marker = text.find(">=")?;
    let rest = text[marker + 2..].trim_start();
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after = rest[digits.len()..].trim_start();
    if !after.starts_with("settlements") {
        return None;
    }
    digits.parse().ok()
}

/// `unknowns`: `{id, question[, status, recorded_at, note]}` — name is the
/// id, statement is the question text (spec §5.3.1: named, not silently
/// resolved).
fn unknowns_of(entry: Option<&(String, Node)>) -> Result<Vec<Unknown>, ManifestError> {
    let Some((_, Node::Seq { items, .. })) = entry else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Node::Map { entries, .. } = item else {
            return Err(ManifestError::BadShape {
                line: item.line(),
                key: "unknowns".to_owned(),
                expected: "a sequence of mappings",
                got: item.shape(),
            });
        };
        duplicates(entries)?;
        for (key, node) in entries {
            if !UNKNOWN_KEYS.contains(&key.as_str()) {
                return Err(ManifestError::UnknownKey {
                    line: node.line(),
                    key: key.clone(),
                });
            }
        }
        let id = scalar_of(entries.iter().find(|(key, _)| key == "id"), "unknowns.id")?;
        let question = narrative(
            entries.iter().find(|(key, _)| key == "question"),
            "unknowns.question",
        )?;
        out.push(Unknown::new(id.value, question));
    }
    Ok(out)
}

/// Assembles the domain `ReferenceDefinition`: the manifest's narrative,
/// the answer key's Units, and the ANSWER KEY's digest as the definition
/// digest (the Units' ruler — the manifest's own digest stays the lock's
/// business). The deferral decisions land here, visibly: empty
/// `quality_dimensions`, empty `required_evidence`, empty `hard_units`.
pub fn build_definition(
    manifest: &ManifestDefinition,
    units: Vec<Unit>,
    key_digest: Digest,
) -> ReferenceDefinition {
    ReferenceDefinition {
        id: manifest.id.clone(),
        version: manifest.version,
        digest: key_digest,
        source_refs: manifest.source_refs.clone(),
        purpose: manifest.purpose.clone(),
        desired_state: manifest.desired_state.clone(),
        invariants: manifest.invariants.clone(),
        constraints: manifest.constraints.clone(),
        anti_goals: manifest.anti_goals.clone(),
        units,
        quality_dimensions: Vec::new(),
        settlement: SettlementCriteria::new(
            manifest.success_criteria.clone(),
            Vec::new(),
            manifest.minimum_reliability_passes,
            manifest.terminal_conditions.clone(),
            Vec::new(),
        )
        .unwrap_or_else(|err| {
            unreachable!(
                "settlement_of refuses a reliability below one at parse time, and the \
                 success criteria and terminal conditions were validated there too: {err}"
            )
        }),
        unknowns: manifest.unknowns.clone(),
        assumptions: manifest.assumptions.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal manifest in the frozen shape, for mutation per case.
    fn minimal() -> String {
        [
            "reference:",
            "  id: ref-suite-01",
            "  version: 1",
            "  purpose: >",
            "    One paragraph.",
            "  desired_state: >",
            "    The destination.",
            "  settlement:",
            "    success_criteria:",
            "      - the destination is reached",
            "    minimum_reliability: >",
            "      spec 19.2 default: >= 1 settlements before promotion.",
            "    terminal_conditions: [settled, aborted]",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn a_minimal_manifest_parses_its_fields() {
        let parsed = parse_manifest(&minimal()).expect("parses");
        assert_eq!(parsed.id.as_str(), "ref-suite-01");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.purpose, "One paragraph.");
        assert_eq!(parsed.desired_state, "The destination.");
        assert_eq!(parsed.minimum_reliability_passes, 1);
        assert_eq!(
            parsed.terminal_conditions,
            vec![TerminalState::Settled, TerminalState::Aborted]
        );
        assert_eq!(parsed.success_criteria.len(), 1);
        assert!(parsed.invariants.is_empty());
        assert!(parsed.unknowns.is_empty());
        assert_eq!(parsed.quality_dimension_count, 0);
    }

    #[test]
    fn the_frozen_manifest_parses_with_its_fields_intact() {
        // The frozen ruler's narrative half, read-only. Every extracted
        // count is asserted so drift in EITHER the manifest or this parser
        // fails here rather than silently downstream.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("the app crate sits two levels under the repository root");
        let path = repo_root.join(".wayfinder").join("reference.yaml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("the frozen manifest is readable: {err}"));
        let parsed = parse_manifest(&text).expect("the frozen manifest parses");
        assert_eq!(parsed.id.as_str(), "gauntlet-core-bootstrap");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.source_refs.len(), 1);
        assert!(
            parsed
                .purpose
                .starts_with("Build a reference-anchored Gauntlet runtime")
        );
        assert!(
            parsed
                .desired_state
                .starts_with("An operator can enter this repository")
        );
        assert_eq!(parsed.invariants.len(), 12);
        assert_eq!(
            parsed.invariants[0],
            "I1: The builder never modifies the standard its work is evaluated against."
        );
        assert!(
            parsed
                .invariants
                .iter()
                .all(|rendered| rendered.split_once(": ").is_some()),
            "every invariant's id survives the rendering"
        );
        assert_eq!(parsed.constraints.len(), 7);
        assert_eq!(parsed.anti_goals.len(), 6);
        assert_eq!(parsed.success_criteria.len(), 3);
        assert_eq!(parsed.required_evidence_locators.len(), 3);
        assert_eq!(parsed.minimum_reliability_passes, 3);
        assert_eq!(parsed.terminal_conditions.len(), 6);
        assert_eq!(
            parsed.terminal_conditions[0],
            TerminalState::Settled,
            "the section 5.2 wire tokens parse in manifest order"
        );
        assert_eq!(parsed.unknowns.len(), 3);
        assert_eq!(parsed.unknowns[0].name(), "UNK-001");
        assert!(
            parsed.unknowns[0]
                .statement()
                .contains("containment backend")
        );
        assert_eq!(parsed.assumptions.len(), 3);
        assert_eq!(parsed.quality_dimension_count, 4);
    }

    #[test]
    fn unknown_keys_and_roots_are_typed_errors_naming_the_line() {
        let drifted = minimal().replace("  version: 1", "  flavour: 1");
        let error = parse_manifest(&drifted).expect_err("drift is refused");
        assert!(
            matches!(&error, ManifestError::UnknownKey { line: 3, key } if key == "flavour"),
            "{error}"
        );
        let bad_root = format!("other:\n  id: x\n\n{}", minimal());
        let error = parse_manifest(&bad_root).expect_err("a second root is refused");
        assert!(
            matches!(&error, ManifestError::UnknownRootKey { key, .. } if key == "other"),
            "{error}"
        );
    }

    #[test]
    fn missing_required_keys_and_bad_values_are_typed_errors() {
        for (needle, replacement, wanted) in [
            (
                "  version: 1\n",
                "",
                ManifestError::MissingKey { key: "version" },
            ),
            (
                "  version: 1",
                "  version: one",
                ManifestError::NotAnInteger {
                    line: 3,
                    value: "one".to_owned(),
                },
            ),
            (
                "  id: ref-suite-01",
                "  id: not an id",
                ManifestError::InvalidId {
                    line: 2,
                    value: "not an id".to_owned(),
                },
            ),
            (
                "    One paragraph.\n",
                "",
                ManifestError::EmptyFolded {
                    line: 4,
                    key: "purpose".to_owned(),
                },
            ),
        ] {
            let drifted = minimal().replace(needle, replacement);
            let error = parse_manifest(&drifted).expect_err("drift is refused");
            assert_eq!(error, wanted, "for replacement {replacement:?}");
        }
    }

    #[test]
    fn a_non_null_digest_and_a_populated_units_section_are_loud_failures() {
        let drifted = minimal().replace("  version: 1", "  digest: sha256:00\n  version: 1");
        let error = parse_manifest(&drifted).expect_err("a self-digest is refused");
        assert!(
            matches!(&error, ManifestError::DigestNotNull { .. }),
            "{error}"
        );
        let drifted = minimal().replace(
            "  settlement:",
            "  units:\n    - id: AK-001\n      claim: x\n  settlement:",
        );
        let error = parse_manifest(&drifted).expect_err("units belong to the answer key");
        assert!(
            matches!(&error, ManifestError::UnitsNotEmpty { count: 1, .. }),
            "{error}"
        );
    }

    #[test]
    fn terminal_conditions_and_reliability_prose_fail_loudly_on_drift() {
        let drifted = minimal().replace("[settled, aborted]", "[settled, misplaced]");
        let error = parse_manifest(&drifted).expect_err("an unknown terminal is refused");
        assert!(
            matches!(&error, ManifestError::UnknownTerminal { value, .. } if value == "misplaced"),
            "{error}"
        );
        let drifted = minimal().replace(">= 1 settlements before promotion.", "three clean runs.");
        let error = parse_manifest(&drifted).expect_err("prose without the pattern is refused");
        assert!(
            matches!(&error, ManifestError::ReliabilityUnparseable { .. }),
            "{error}"
        );
        // Critic-B F1: zero parses but is an illegal minimum — refused at
        // admission as a typed error, never a panic in build_definition.
        let drifted = minimal().replace(">= 1 settlements before promotion.", ">= 0 settlements.");
        let error = parse_manifest(&drifted).expect_err("a zero minimum is refused");
        assert!(
            matches!(&error, ManifestError::ReliabilityBelowOne { line: 11 }),
            "{error}"
        );
        // The pattern is positional, not a whole-text match.
        assert_eq!(
            reliability_from(">= 3 settlements under recorded variation"),
            Some(3)
        );
        assert_eq!(reliability_from("at least >= 12 settlements"), Some(12));
        assert_eq!(reliability_from("no number here"), None);
        assert_eq!(reliability_from(">= settlements"), None);
        assert_eq!(reliability_from(">= 3 passes"), None);
    }

    /// Critic-B F3: the tab-in-indentation guard must actually fire — a tab
    /// folded into the leading whitespace of any line is typed drift, not
    /// style, and must not silently pass as body text.
    #[test]
    fn a_tab_in_the_indentation_is_typed_drift_wherever_it_sits() {
        for drifted in [
            minimal().replace("  id: ref-suite-01", "\tid: ref-suite-01"),
            minimal().replace("    One paragraph.", "  \tOne paragraph."),
            minimal().replace(
                "      - the destination is reached",
                "\t  - the destination",
            ),
        ] {
            let error =
                parse_manifest(&drifted).expect_err("tab indentation is refused, not trimmed");
            assert!(
                matches!(&error, ManifestError::MalformedItem { detail, .. }
                    if detail.contains("tab in the indentation")),
                "{error}"
            );
        }
        // Exotic whitespace (NBSP) is not indentation either: it stays in
        // the body and the parse still fails loudly — the guard must not
        // widen what counts as indentation (confirmation-critic minor).
        let nbsp = minimal().replace("  id: ref-suite-01", "\u{a0}id: ref-suite-01");
        let error = parse_manifest(&nbsp).expect_err("NBSP indentation is not admitted silently");
        assert!(
            !matches!(&error, ManifestError::MalformedItem { detail, .. }
                if detail.contains("tab in the indentation")),
            "NBSP is not the tab class: {error}"
        );
    }

    /// Critic-B F6: empty flow-sequence entries are drift, not silently
    /// admitted blank values.
    #[test]
    fn empty_flow_entries_are_refused() {
        for drifted in [
            minimal().replace("[settled, aborted]", "[settled,, aborted]"),
            minimal().replace("[settled, aborted]", "[,]"),
        ] {
            let error = parse_manifest(&drifted).expect_err("an empty flow entry is refused");
            assert!(
                matches!(&error, ManifestError::MalformedItem { detail, .. }
                    if detail.contains("empty entry")),
                "{error}"
            );
        }
    }

    #[test]
    fn unknown_sub_keys_in_nested_sections_are_typed_errors() {
        let drifted = minimal().replace(
            "    minimum_reliability: >",
            "    dispute_process: none\n    minimum_reliability: >",
        );
        let error = parse_manifest(&drifted).expect_err("settlement drift is refused");
        assert!(
            matches!(&error, ManifestError::UnknownKey { key, .. } if key == "dispute_process"),
            "{error}"
        );
        let drifted = format!(
            "{}  unknowns:\n    - id: UNK-001\n      question: >\n        A question.\n      \
             mood: curious\n",
            minimal()
        );
        let error = parse_manifest(&drifted).expect_err("unknowns drift is refused");
        assert!(
            matches!(&error, ManifestError::UnknownKey { key, .. } if key == "mood"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_keys_are_refused_rather_than_last_won() {
        let drifted = minimal().replace("  version: 1", "  version: 1\n  version: 2");
        let error = parse_manifest(&drifted).expect_err("duplicates are refused");
        assert!(
            matches!(&error, ManifestError::DuplicateKey { key, .. } if key == "version"),
            "{error}"
        );
    }

    #[test]
    fn folded_scalars_fold_lines_into_single_spaced_text() {
        let text = minimal().replace(
            "    One paragraph.",
            "    First line of the purpose\n    continued on a second line\n    and a third.",
        );
        let parsed = parse_manifest(&text).expect("parses");
        assert_eq!(
            parsed.purpose,
            "First line of the purpose continued on a second line and a third."
        );
    }

    #[test]
    fn build_definition_carries_the_deferred_sections_as_empty() {
        let manifest = parse_manifest(&minimal()).expect("parses");
        let definition = build_definition(
            &manifest,
            Vec::new(),
            Digest::new("sha256", "a1b2c3d4").expect("valid"),
        );
        assert_eq!(definition.invariants, Vec::<String>::new());
        assert!(definition.quality_dimensions.is_empty());
        assert!(definition.settlement.required_evidence().is_empty());
        assert!(definition.settlement.hard_units().is_empty());
        assert_eq!(definition.settlement.minimum_reliability_passes(), 1);
        assert_eq!(definition.digest.hex(), "a1b2c3d4");
        assert_eq!(definition.settlement.success_criteria().len(), 1);
        assert_eq!(definition.settlement.terminal_conditions().len(), 2);
    }
}
