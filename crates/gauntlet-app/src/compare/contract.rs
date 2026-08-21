//! The §9.5 comparison contract artifact (plan Task 10 D2): strict parse →
//! typed view. The artifact is `.gauntlet/comparison-contract.yaml`; it is
//! frozen with the reference and its version is recorded on every
//! `ComparisonRecord` (Annex B.4 `comparison_contract_version`).
//!
//! The dialect is a strict YAML subset (the repository's established
//! configuration style — no external YAML dependency at this layer): scalar
//! `key: value` lines and list sections opened by a bare `key:` line whose
//! items are exactly two-space-indented `- item` lines. Rubric items are
//! `<severity-token> | <definition>`. Decoding is STRICT: unknown keys,
//! duplicate or missing sections, malformed items, unknown severity tokens,
//! and unknown versions are typed errors (configuration/formation class,
//! spec §21.1), never defaults.

use core::fmt;

use gauntlet_domain::Severity;
use gauntlet_ports::comparator::{ContractView, RubricEntry};

/// The contract version this runtime understands (spec §9.5: "versioned
/// artifact ... its version is recorded on every ComparisonRecord").
pub const SUPPORTED_CONTRACT_VERSION: &str = "1";

/// The artifact's declared sections, in the artifact's documented order.
const SECTIONS: &[&str] = &[
    "version",
    "dimensions",
    "severity_rubric",
    "comparator_receives",
    "residual_definition",
];

/// Comparison-contract load/parse failures — typed (spec §21.1
/// configuration/formation class, §21.2 error discipline).
#[derive(Debug, Clone, PartialEq)]
pub enum ContractError {
    /// The artifact could not be read.
    Io { detail: String },
    /// A line is not valid in the dialect (bad indentation, no `- ` item
    /// marker inside a section, scalar continuing a list).
    Syntax { line: usize, detail: String },
    /// A key outside the declared section vocabulary.
    UnknownSection { line: usize, section: String },
    /// A section appears twice.
    DuplicateSection { section: String },
    /// A required section is absent.
    MissingSection { section: &'static str },
    /// The declared version is not one this runtime understands. Strictness
    /// is the default: a contract that cannot be validated is never
    /// partially adopted (the same rule as an unknown `payload_schema`).
    VersionUnsupported { found: String, supported: String },
    /// A rubric item is not `<severity-token> | <definition>`.
    RubricMalformed { line: usize, detail: String },
    /// A rubric item names a severity outside the §5.2 vocabulary, or the
    /// rubric does not cover all four severities.
    RubricInvalid { detail: String },
    /// A dimension entry is empty.
    DimensionInvalid { line: usize },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { detail } => write!(f, "comparison contract: {detail}"),
            Self::Syntax { line, detail } => {
                write!(f, "comparison contract: line {line} is not valid: {detail}")
            }
            Self::UnknownSection { line, section } => write!(
                f,
                "comparison contract: line {line} opens unknown section {section:?}"
            ),
            Self::DuplicateSection { section } => {
                write!(f, "comparison contract: section {section:?} appears twice")
            }
            Self::MissingSection { section } => write!(
                f,
                "comparison contract: required section {section:?} is missing"
            ),
            Self::VersionUnsupported { found, supported } => write!(
                f,
                "comparison contract: version {found:?} is not supported (this runtime \
                 understands {supported:?}); a contract that cannot be validated is never \
                 partially adopted"
            ),
            Self::RubricMalformed { line, detail } => write!(
                f,
                "comparison contract: rubric entry at line {line} is malformed: {detail}"
            ),
            Self::RubricInvalid { detail } => {
                write!(
                    f,
                    "comparison contract: the severity rubric is invalid: {detail}"
                )
            }
            Self::DimensionInvalid { line } => write!(
                f,
                "comparison contract: dimension entry at line {line} is empty"
            ),
        }
    }
}

impl std::error::Error for ContractError {}

/// The typed view of the §9.5 artifact: the version in force, the
/// instantiated dimensions (spec §13.4), and the severity rubric — exactly
/// the members the comparison envelope's contract view carries (spec §13.2
/// allowlist member "comparison contract (dimension, rubric, severity
/// definitions)"). The `comparator_receives` and `residual_definition`
/// sections are validated-present declarations of the artifact itself;
/// they are the allowlist this module's caller enforces structurally and
/// are not re-carried per envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonContract {
    version: String,
    dimensions: Vec<String>,
    rubric: Vec<RubricEntry>,
    /// The declared §13.2 allowlist, retained rather than reduced to a
    /// count so formation/audit code can inspect the operative contract.
    comparator_receives: Vec<String>,
    /// The declared residual semantics, retained for the same reason.
    residual_definition: Vec<String>,
}

impl ComparisonContract {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn dimensions(&self) -> &[String] {
        &self.dimensions
    }

    pub fn rubric(&self) -> &[RubricEntry] {
        &self.rubric
    }

    pub fn comparator_receives(&self) -> &[String] {
        &self.comparator_receives
    }

    pub fn residual_definition(&self) -> &[String] {
        &self.residual_definition
    }

    /// The ports envelope's contract view (spec §13.2).
    pub fn to_contract_view(&self) -> ContractView {
        ContractView::new(
            self.version.clone(),
            self.dimensions.clone(),
            self.rubric.clone(),
        )
    }

    /// Strictly parses the artifact text. See the module doc for the
    /// dialect.
    pub fn parse(text: &str) -> Result<Self, ContractError> {
        let mut version: Option<String> = None;
        let mut dimensions: Option<Vec<String>> = None;
        let mut rubric: Option<Vec<RubricEntry>> = None;
        let mut receives: Option<Vec<String>> = None;
        let mut residual: Option<Vec<String>> = None;
        let mut current: Option<&'static str> = None;

        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let stripped = raw.strip_prefix('#').map(str::to_owned);
            let body = match stripped {
                // A whole-line comment: `# ...` at column zero. Indented
                // `#`-leading lines inside sections are items like any
                // other — strictness means no silent skipping.
                Some(_) => continue,
                None => raw,
            };
            if body.trim().is_empty() {
                continue;
            }
            if let Some(item) = body.strip_prefix("  - ") {
                let Some(section) = current else {
                    return Err(ContractError::Syntax {
                        line,
                        detail: "a list item appears before any section".to_owned(),
                    });
                };
                let item = item.trim();
                match section {
                    "dimensions" => {
                        if item.is_empty() {
                            return Err(ContractError::DimensionInvalid { line });
                        }
                        dimensions
                            .get_or_insert_with(Vec::new)
                            .push(item.to_owned());
                    }
                    "severity_rubric" => {
                        let Some((token, definition)) = item.split_once('|') else {
                            return Err(ContractError::RubricMalformed {
                                line,
                                detail: "expected `<severity-token> | <definition>`".to_owned(),
                            });
                        };
                        let severity = Severity::parse(token.trim()).map_err(|source| {
                            ContractError::RubricMalformed {
                                line,
                                detail: source.to_string(),
                            }
                        })?;
                        rubric
                            .get_or_insert_with(Vec::new)
                            .push(RubricEntry::new(severity, definition.trim().to_owned()));
                    }
                    "comparator_receives" => {
                        receives.get_or_insert_with(Vec::new).push(item.to_owned())
                    }
                    "residual_definition" => {
                        residual.get_or_insert_with(Vec::new).push(item.to_owned())
                    }
                    _ => {
                        return Err(ContractError::Syntax {
                            line,
                            detail: format!("section {section:?} takes no items"),
                        });
                    }
                }
                continue;
            }
            if body.starts_with(char::is_whitespace) {
                return Err(ContractError::Syntax {
                    line,
                    detail: "indented line is not a `- ` list item".to_owned(),
                });
            }
            // A top-level line: either a bare section opener or the version
            // scalar.
            let (key, value) = match body.split_once(':') {
                Some(parts) => parts,
                None => {
                    return Err(ContractError::Syntax {
                        line,
                        detail: "expected `key: value` or a `key:` section opener".to_owned(),
                    });
                }
            };
            let value = value.trim();
            match key {
                "version" => {
                    if version.is_some() {
                        return Err(ContractError::DuplicateSection {
                            section: "version".into(),
                        });
                    }
                    let token = value.trim_matches('"');
                    if token.is_empty() {
                        return Err(ContractError::Syntax {
                            line,
                            detail: "the version value is empty".to_owned(),
                        });
                    }
                    version = Some(token.to_owned());
                    current = None;
                }
                "dimensions"
                | "severity_rubric"
                | "comparator_receives"
                | "residual_definition" => {
                    if !value.is_empty() {
                        return Err(ContractError::Syntax {
                            line,
                            detail: format!(
                                "section {key:?} opens a list; its value must be empty here"
                            ),
                        });
                    }
                    let already = match key {
                        "dimensions" => dimensions.is_some(),
                        "severity_rubric" => rubric.is_some(),
                        "comparator_receives" => receives.is_some(),
                        _ => residual.is_some(),
                    };
                    if already {
                        return Err(ContractError::DuplicateSection {
                            section: key.to_owned(),
                        });
                    }
                    // Reserve the section so duplicates are caught even
                    // before any item arrives.
                    match key {
                        "dimensions" => dimensions = Some(Vec::new()),
                        "severity_rubric" => rubric = Some(Vec::new()),
                        "comparator_receives" => receives = Some(Vec::new()),
                        _ => residual = Some(Vec::new()),
                    }
                    current = SECTIONS.iter().find(|section| *section == &key).copied();
                }
                other => {
                    return Err(ContractError::UnknownSection {
                        line,
                        section: other.to_owned(),
                    });
                }
            }
        }

        let Some(version) = version else {
            return Err(ContractError::MissingSection { section: "version" });
        };
        if version != SUPPORTED_CONTRACT_VERSION {
            return Err(ContractError::VersionUnsupported {
                found: version,
                supported: SUPPORTED_CONTRACT_VERSION.to_owned(),
            });
        }
        let dimensions = dimensions.ok_or(ContractError::MissingSection {
            section: "dimensions",
        })?;
        if dimensions.is_empty() {
            return Err(ContractError::RubricInvalid {
                detail: "no dimensions are instantiated; a comparison contract that compares \
                         nothing is invalid (spec 13.4)"
                    .to_owned(),
            });
        }
        let rubric = rubric.ok_or(ContractError::MissingSection {
            section: "severity_rubric",
        })?;
        let receives = receives.ok_or(ContractError::MissingSection {
            section: "comparator_receives",
        })?;
        let residual = residual.ok_or(ContractError::MissingSection {
            section: "residual_definition",
        })?;
        // The two declaration sections exist to be declarations: an empty
        // one is a missing declaration, not a vacuous pass (I8).
        if receives.is_empty() {
            return Err(ContractError::MissingSection {
                section: "comparator_receives",
            });
        }
        if residual.is_empty() {
            return Err(ContractError::MissingSection {
                section: "residual_definition",
            });
        }
        // The rubric covers the whole §5.2 severity vocabulary: a residual's
        // severity is assigned per these definitions, so a missing severity
        // would be unassignable.
        for severity in Severity::ALL {
            if !rubric.iter().any(|entry| entry.severity == *severity) {
                return Err(ContractError::RubricInvalid {
                    detail: format!("the rubric does not define severity {}", severity),
                });
            }
        }
        Ok(Self {
            version,
            dimensions,
            rubric,
            comparator_receives: receives,
            residual_definition: residual,
        })
    }

    /// Loads and strictly parses the artifact at `path`.
    pub fn load(path: &std::path::Path) -> Result<Self, ContractError> {
        let text = std::fs::read_to_string(path).map_err(|source| ContractError::Io {
            detail: format!("reading {}: {source}", path.display()),
        })?;
        Self::parse(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::{ComparisonContract, ContractError, SUPPORTED_CONTRACT_VERSION};
    use crate::config::defaults::{COMPARISON_CONTRACT_NAME, RUNTIME_DIR_NAME};
    use gauntlet_domain::Severity;

    const MINIMAL: &str = "\
version: 1
dimensions:
  - functional behavior
severity_rubric:
  - blocking | invalidates the iteration
  - major | real, not invalidating
  - minor | small
  - informational | recorded only
comparator_receives:
  - the allowlist members
residual_definition:
  - a difference
";

    #[test]
    fn the_repository_artifact_parses_into_the_typed_view() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("the app crate sits two levels under the repository root");
        let contract = ComparisonContract::load(
            &repo_root
                .join(RUNTIME_DIR_NAME)
                .join(COMPARISON_CONTRACT_NAME),
        )
        .expect("the repository's frozen artifact parses");
        assert_eq!(contract.version(), SUPPORTED_CONTRACT_VERSION);
        assert_eq!(contract.dimensions().len(), 5);
        assert!(
            contract
                .dimensions()
                .contains(&"functional behavior".to_owned())
        );
        assert_eq!(contract.rubric().len(), 4);
        assert_eq!(contract.comparator_receives().len(), 5);
        assert_eq!(contract.residual_definition().len(), 4);
        // The typed view is exactly what the envelope carries (spec 13.2).
        let view = contract.to_contract_view();
        assert_eq!(view.version(), SUPPORTED_CONTRACT_VERSION);
        assert_eq!(view.dimensions(), contract.dimensions());
    }

    #[test]
    fn a_minimal_contract_parses_and_the_view_round_trips() {
        let contract = ComparisonContract::parse(MINIMAL).expect("minimal artifact parses");
        assert_eq!(contract.version(), "1");
        assert_eq!(contract.dimensions(), &["functional behavior".to_owned()]);
        assert_eq!(contract.rubric().len(), 4);
        assert_eq!(
            contract.comparator_receives(),
            &["the allowlist members".to_owned()]
        );
        assert_eq!(contract.residual_definition(), &["a difference".to_owned()]);
        assert_eq!(contract.rubric()[0].severity, Severity::Blocking);
    }

    #[test]
    fn unknown_versions_are_typed_errors_never_partially_adopted() {
        let future = MINIMAL.replace("version: 1", "version: 2");
        assert!(matches!(
            ComparisonContract::parse(&future),
            Err(ContractError::VersionUnsupported { found, .. }) if found == "2"
        ));
        let quoted = MINIMAL.replace("version: 1", "version: \"1\"");
        assert!(
            ComparisonContract::parse(&quoted).is_ok(),
            "quoted scalar ok"
        );
    }

    #[test]
    fn structural_defects_are_typed_errors_with_loci() {
        // Unknown section.
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace(
                "comparator_receives:",
                "bonus_section:"
            )),
            Err(ContractError::UnknownSection { section, .. }) if section == "bonus_section"
        ));
        // Duplicate section.
        assert!(matches!(
            ComparisonContract::parse(&format!("{MINIMAL}severity_rubric:\n  - minor | again\n")),
            Err(ContractError::DuplicateSection { section, .. }) if section == "severity_rubric"
        ));
        // Missing section.
        let truncated = MINIMAL.replace("residual_definition:\n  - a difference\n", "");
        assert!(matches!(
            ComparisonContract::parse(&truncated),
            Err(ContractError::MissingSection {
                section: "residual_definition"
            })
        ));
        // Item outside a section.
        assert!(matches!(
            ComparisonContract::parse(&format!("  - stray\n{MINIMAL}")),
            Err(ContractError::Syntax { line: 1, .. })
        ));
        // Indented non-item line (the residual-definition item sits at
        // line 12 of MINIMAL).
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace("  - a difference", "    nested: 1")),
            Err(ContractError::Syntax { line: 12, .. })
        ));
    }

    #[test]
    fn rubric_and_dimension_vocabulary_is_enforced() {
        // Bad rubric item shape.
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace("  - minor | small", "  - minor")),
            Err(ContractError::RubricMalformed { line: 7, .. })
        ));
        // Unknown severity token.
        assert!(matches!(
            ComparisonContract::parse(
                &MINIMAL.replace("  - minor | small", "  - fatal | not a severity")
            ),
            Err(ContractError::RubricMalformed { line: 7, .. })
        ));
        // Rubric missing a severity.
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace("  - minor | small\n", "")),
            Err(ContractError::RubricInvalid { .. })
        ));
        // Empty dimension.
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace("  - functional behavior", "  - ")),
            Err(ContractError::DimensionInvalid { line: 3 })
        ));
        // Zero dimensions.
        assert!(matches!(
            ComparisonContract::parse(&MINIMAL.replace("  - functional behavior\n", "")),
            Err(ContractError::RubricInvalid { .. })
        ));
    }
}
