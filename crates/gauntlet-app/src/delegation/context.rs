//! Fresh context for a delegated invocation (E4a).
//!
//! The kind vocabulary is CLOSED and deliberately has no variant for a parent
//! transcript, parent reasoning, a builder trajectory, an attempt count, or an
//! identity. Fresh context is structural: a caller cannot express those kinds,
//! so they cannot reach a child's envelope.
//!
//! The canonical body is deterministic and length-prefixed. Each item is a
//! header line `<kind> <label_len> <content_len>` followed by exactly
//! `label_len` bytes of label, a newline, `content_len` bytes of content, and a
//! newline. Parsing is length-directed, so content cannot forge a second item.

use core::fmt;
use std::collections::BTreeSet;

use gauntlet_ports::ContextEnvelope;

/// Named ceiling on the canonical envelope body, in bytes.
pub const MAX_ENVELOPE_BYTES: usize = 65_536;

/// The format tag on the first line of every canonical body.
pub(crate) const BODY_HEADER: &str = "gauntlet-delegation-context v1\n";

/// The closed vocabulary of context kinds a delegation may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContextItemKind {
    TaskStatement,
    ReferenceExcerpt,
    FileExcerpt,
    EvidenceReference,
    ConstraintStatement,
    ResultSchema,
}

impl ContextItemKind {
    /// The wire token written into the canonical body.
    pub fn token(self) -> &'static str {
        match self {
            Self::TaskStatement => "task-statement",
            Self::ReferenceExcerpt => "reference-excerpt",
            Self::FileExcerpt => "file-excerpt",
            Self::EvidenceReference => "evidence-reference",
            Self::ConstraintStatement => "constraint-statement",
            Self::ResultSchema => "result-schema",
        }
    }
}

impl fmt::Display for ContextItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One piece of context the caller chooses to hand to the child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    pub kind: ContextItemKind,
    pub label: String,
    pub content: String,
}

/// Which kinds a delegation may carry. `deny` wins over `allow`; a kind absent
/// from `allow` is refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextPolicy {
    pub allow: BTreeSet<ContextItemKind>,
    pub deny: BTreeSet<ContextItemKind>,
}

impl ContextPolicy {
    /// Whether `kind` may appear in an envelope under this policy.
    pub fn permits(&self, kind: ContextItemKind) -> bool {
        self.allow.contains(&kind) && !self.deny.contains(&kind)
    }
}

/// Why an envelope could not be built. A refusal builds nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextRefusal {
    /// The kind is denied by the policy or absent from its allow set.
    KindNotAllowed { kind: ContextItemKind },
    /// Two items share a (kind, label) pair.
    DuplicateItem {
        kind: ContextItemKind,
        label: String,
    },
    /// No non-blank task statement is present.
    EmptyTaskStatement,
    /// The canonical body exceeds [`MAX_ENVELOPE_BYTES`].
    BodyTooLarge { limit: usize, size: usize },
}

impl fmt::Display for ContextRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KindNotAllowed { kind } => {
                write!(f, "context kind {kind} is not allowed by the policy")
            }
            Self::DuplicateItem { kind, label } => {
                write!(f, "context item {kind} '{label}' appears more than once")
            }
            Self::EmptyTaskStatement => f.write_str("the task statement is missing or blank"),
            Self::BodyTooLarge { limit, size } => {
                write!(
                    f,
                    "context body is {size} bytes, above the {limit} byte limit"
                )
            }
        }
    }
}

impl std::error::Error for ContextRefusal {}

/// Builds the fresh-context envelope for a delegation: checks the policy,
/// sorts items by (kind, label), writes the length-prefixed canonical body,
/// and digests exactly those bytes.
pub fn build_envelope(
    policy: &ContextPolicy,
    items: &[ContextItem],
) -> Result<ContextEnvelope, ContextRefusal> {
    for item in items {
        if !policy.permits(item.kind) {
            return Err(ContextRefusal::KindNotAllowed { kind: item.kind });
        }
    }
    let mut sorted: Vec<&ContextItem> = items.iter().collect();
    sorted.sort_by(|a, b| (a.kind, &a.label).cmp(&(b.kind, &b.label)));
    for pair in sorted.windows(2) {
        if pair[0].kind == pair[1].kind && pair[0].label == pair[1].label {
            return Err(ContextRefusal::DuplicateItem {
                kind: pair[0].kind,
                label: pair[0].label.clone(),
            });
        }
    }
    let has_task = sorted
        .iter()
        .any(|item| item.kind == ContextItemKind::TaskStatement && !item.content.trim().is_empty());
    if !has_task {
        return Err(ContextRefusal::EmptyTaskStatement);
    }
    let mut body = String::from(BODY_HEADER);
    for item in sorted {
        body.push_str(item.kind.token());
        body.push(' ');
        body.push_str(&item.label.len().to_string());
        body.push(' ');
        body.push_str(&item.content.len().to_string());
        body.push('\n');
        body.push_str(&item.label);
        body.push('\n');
        body.push_str(&item.content);
        body.push('\n');
    }
    if body.len() > MAX_ENVELOPE_BYTES {
        return Err(ContextRefusal::BodyTooLarge {
            limit: MAX_ENVELOPE_BYTES,
            size: body.len(),
        });
    }
    let digest = crate::verify::content_digest(body.as_bytes());
    Ok(ContextEnvelope::new(body, digest))
}
