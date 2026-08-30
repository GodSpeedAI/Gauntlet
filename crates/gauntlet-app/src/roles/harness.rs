//! The semantic harness registry — a data map from a harness id to the
//! role-surface capability that harness offers (plan Task 17; the frozen
//! preregistration's D4).
//!
//! It is a CAPABILITY map, not a vendor list: the application layer
//! never spells a product identifier (the architecture denylist applies
//! to this crate), and the composition root keys its entries with the
//! runner adapters' exported token constants — the only place a harness
//! id spelling belongs. Adding a harness is two data insertions in the
//! composition root, not a match arm spreading role logic (the frozen
//! preregistration's parametric-variation expectation).
//!
//! Two capabilities exist today, per the plan's prescribed fallback:
//!
//! - [`RoleSurface::WritableRoleDir`] — the harness offers a writable
//!   role surface; the installer materializes the role files under the
//!   configured state root and the installed state is the enforced
//!   contract.
//! - [`RoleSurface::AdvisoryPromptArtifacts`] — the harness has NO
//!   writable role surface; injection is per-invocation, prompt-side,
//!   and the installer records `enforcement: advisory` on the invocation
//!   contract while materializing the injectable role bodies as
//!   artifacts under the state root. VISIBLE WEAKNESS BEATS INVISIBLE
//!   WEAKNESS: the fallback records its own limits instead of faking
//!   enforcement.
//!
//! Registry ids are validated at insertion (lowercase letters, digits,
//! hyphens; never a path) so an id can never carry a separator or
//! traversal into an installed path. An unknown or unset id at lookup
//! fails closed with a typed error (the frozen preregistration's D3:
//! no runner is selected by default — that is the operator's call).

/// The role-surface capability a harness offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleSurface {
    /// The harness accepts installed role files as enforced state.
    WritableRoleDir,
    /// No writable role surface: role bodies are injected per
    /// invocation, prompt-side; enforcement is advisory and recorded as
    /// such.
    AdvisoryPromptArtifacts,
}

impl RoleSurface {
    /// The state-root directory leaf this surface materializes into.
    /// Two distinct leaves keep each surface's verified subject
    /// well-defined.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::WritableRoleDir => "installed",
            Self::AdvisoryPromptArtifacts => "injectable",
        }
    }
}

/// Registry authoring failures — a registry key violating the id
/// grammar. Refused at insertion, so no installed path can ever be
/// steered by a malformed id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The id is empty.
    EmptyId,
    /// The id carries characters outside the grammar (lowercase
    /// letters, digits, hyphens, starting alphanumeric).
    InvalidId { id: String },
}

impl core::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyId => write!(f, "harness registry: a harness id must not be empty"),
            Self::InvalidId { id } => write!(
                f,
                "harness registry: '{id}' violates the id grammar — lowercase letters, \
                 digits, and hyphens, never a path"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

/// The harness registry: a deterministic, semantic data map.
#[derive(Debug, Clone, Default)]
pub struct HarnessRegistry {
    entries: std::collections::BTreeMap<String, RoleSurface>,
}

impl HarnessRegistry {
    /// An empty registry. The composition root inserts the harnesses it
    /// fronts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts one harness with its role-surface capability. The id is
    /// validated at the authoring boundary (fail closed).
    pub fn insert(&mut self, id: &str, surface: RoleSurface) -> Result<(), RegistryError> {
        if id.is_empty() {
            return Err(RegistryError::EmptyId);
        }
        let bytes = id.as_bytes();
        let first_ok = bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric());
        let grammar = first_ok
            && bytes.len() <= 64
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-');
        if !grammar {
            return Err(RegistryError::InvalidId { id: id.to_owned() });
        }
        self.entries.insert(id.to_owned(), surface);
        Ok(())
    }

    /// The capability a registered harness offers; `None` for an
    /// unregistered id — the caller fails closed (no guessed default).
    pub fn surface(&self, id: &str) -> Option<RoleSurface> {
        self.entries.get(id).copied()
    }

    /// Every registered id, sorted — install and verify can run over
    /// the WHOLE registry with one code path.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Whether the registry carries no harness at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
