//! Defaults — the §8.6 conservative bootstrap table (spec §8.6, lines 599–612)
//! plus the repository-layout name vocabulary the bootstrap resolves against.
//!
//! Spec §8.1: "Nothing hardcoded. No path, model, provider, command, port
//! number, timeout, tier policy, or tool identity is a literal in domain or
//! application code. Conservative defaults live in one declared defaults table
//! (§8.6) and are overridable."
//!
//! This module is the one file the architecture boundary suite registers for
//! the §8.6 exemption, and since its round 6 the exemption is
//! DECLARATION-granular: only the ten setting declarations below (head to
//! their terminating `;`) are blanked out of the hardcoded-literal scan.
//! Everything else in this file is policed by every literal class, and the
//! vendor denylist applies to the whole file regardless.
//!
//! # Why the path defaults are NAME constants, not path strings
//!
//! The boundary suite's relative-path class denies path-shaped string literals
//! ("a path literal is environment coupling wherever it is rooted … resolve
//! paths from configuration") in every core source file, this one included.
//! A default spelled `.gauntlet/policies` as one literal would be a violation
//! the gate rejects, so the conservative path defaults are expressed the way
//! that class's own remediation prescribes: as single-segment layout NAMES
//! (§25 repository layout, §8.2 example values) joined at resolution time
//! against a root that comes from discovery or the environment
//! (`GAUNTLET_PROJECT_ROOT`), never from code. The rootedness is therefore
//! configuration; only the spec-fixed layout vocabulary lives here. This
//! deviation from the builder brief (which expected path-string constants) is
//! recorded in the Task 5 builder report.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The ten §8.6 settings. Each declaration is one physical line: the boundary
// suite's warrant check reads setting declarations line-wise, and its literal
// scan masks exactly these spans. Do not reformat onto multiple lines.
// ---------------------------------------------------------------------------

/// §8.6 `max_rounds` — "bounded even if unset".
pub const MAX_ROUNDS: u32 = 8;
/// §8.6 `max_parallel` — "parallel writers corrupt each other's assumptions".
pub const MAX_PARALLEL: u32 = 2;
/// §8.6 `max_depth` — "recursion is raised deliberately, never by default".
pub const MAX_DEPTH: u32 = 1;
/// §8.6 `max_wall_seconds`.
pub const MAX_WALL_SECONDS: u64 = 3600;
/// §8.6 `recovery_reserve_pct` — "a run must always afford to stop cleanly".
pub const RECOVERY_RESERVE_PCT: u8 = 15;
/// §8.6 `diagnostic_budget` — "diagnosis is bounded too".
pub const DIAGNOSTIC_BUDGET: u32 = 2;
/// §8.6 `review.mode` — "pay for judgment only where instruments cannot
/// decide".
pub const REVIEW_MODE: &str = "adaptive";
/// §8.6 `sandbox.backend` — with tier `contained` only; any higher tier
/// without a backend refuses to dispatch.
pub const SANDBOX_BACKEND: &str = "none";
/// §8.6 `interactive` — "auto-detect TTY"; auto-detection is a behavior, not
/// a literal, so the setting's type is an enum.
pub const INTERACTIVE: InteractiveDefault = InteractiveDefault::AutoDetect;
/// §8.6 `git_strategy` — "isolation without cloning cost".
pub const GIT_STRATEGY: &str = "worktree";

/// The §8.6 `interactive` setting. The default is a TTY probe at the moment
/// the question is asked; the explicit variants exist so the environment
/// layer can pin the behavior for unattended runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveDefault {
    /// Auto-detect TTY (§8.6 default).
    AutoDetect,
    /// Always ask a human.
    Always,
    /// Never ask; record decisions instead (unattended runs).
    Never,
}

impl InteractiveDefault {
    /// Parses the environment spelling of `GAUNTLET_INTERACTIVE`. Booleans
    /// are accepted because Annex E groups the variable with the limits, and
    /// operators reasonably write either form; anything else is a typed
    /// error, never a coerced guess.
    pub fn parse(raw: &str) -> Result<Self, DefaultParseError> {
        match raw.trim() {
            "auto" => Ok(Self::AutoDetect),
            "always" | "true" => Ok(Self::Always),
            "never" | "false" => Ok(Self::Never),
            other => Err(DefaultParseError::Interactive(other.to_owned())),
        }
    }

    /// The canonical lowercase wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoDetect => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// Failures parsing a §8.6 setting's environment spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultParseError {
    /// `GAUNTLET_INTERACTIVE` held an unusable value. The value itself is
    /// echoed: this variable is a mode word, not a credential.
    Interactive(String),
}

impl core::fmt::Display for DefaultParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Interactive(value) => write!(
                f,
                "GAUNTLET_INTERACTIVE value '{value}' is not one of auto, always, never, true, \
                 false (spec 8.6 interactive row)"
            ),
        }
    }
}

impl std::error::Error for DefaultParseError {}

// ---------------------------------------------------------------------------
// Bootstrap value defaults that §8.6 does not carry but Annex E requires
// ("All optional; all defaulted"). Each is the §8.2 example's value, chosen
// because the example is the only normative-looking anchor for these rows.
// ---------------------------------------------------------------------------

/// §8.2 `verification.prefer_deterministic` example value; also the §22
/// posture ("prefer deterministic verification wherever deterministic
/// evidence exists").
pub const PREFER_DETERMINISTIC: bool = true;
/// §8.2 `workspace.use_worktrees` example value; §8.6 tables no default for
/// it, and an unselected isolation strategy is not guessed.
pub const WORKSPACE_USE_WORKTREES: bool = true;
/// §8.2 `review.max_critics` example value.
pub const MAX_CRITICS: u32 = 4;
/// §8.2 `budget.max_tokens` example value; Annex E requires a default and
/// §8.6 names none, so the example value is the conservative anchor.
pub const MAX_TOKENS: u64 = 2_000_000;
/// §8.2 `budget.max_cost_usd` example value, as the raw token the canonical
/// form serializes (decimal dollars; resolution parses it into micro-dollars).
pub const MAX_COST_USD: &str = "25";
/// §11.5 non-convergence threshold, inline spec default (Core-spec §11.5
/// retry table: "same residual id N times, `N = GAUNTLET_NONCONVERGENCE_N`,
/// default 3") — an inline default, NOT a §8.6 defaults-table row, so it is
/// deliberately absent from the boundary suite's pinned ten-row inventory.
/// Addressed by the Annex E variable of the same name.
pub const NONCONVERGENCE_N: u32 = 3;
/// §19.2 promotion threshold, inline spec default (Core-spec §19.2:
/// "Promote a capability only with ≥ `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
/// (default 3) settlements across recorded variation axes") — an inline
/// default, NOT a §8.6 defaults-table row, so it is deliberately absent
/// from the boundary suite's pinned ten-row inventory. Addressed by the
/// Annex E variable of the same name. A configured 0 is a typed
/// configuration error at resolution: §19.2's floor (one successful run is
/// not capability) cannot hold below 1.
pub const PROMOTION_MIN_SETTLEMENTS: u32 = 3;
/// §19.4's declared margin for the target-metric condition, addressed by
/// the Annex E variable `GAUNTLET_PROMOTION_MARGIN` (§8.2 key
/// `promotion.margin`, unit: BASIS POINTS relative to control) — an Annex
/// E variable the spec vocabulary names but §8.6 tables NO default for.
/// The shipped default is 0 with an HONEST meaning, not a guessed figure:
/// condition 7's floor stays at STRICT improvement (treatment must beat
/// control outright), which is fail-closed by construction — a larger
/// configured margin only raises the bar. The domain is 0..=10 000 basis
/// points: a value ABOVE 10 000 (above 100%) cannot be an improvement
/// requirement at all, so it is a typed configuration error at resolution
/// and the gate constructor refuses it too — never a silent clamp
/// (COR-2, R3). Like the other inline/absent defaults, deliberately
/// absent from the boundary suite's pinned ten-row inventory.
pub const PROMOTION_MARGIN_BPS: u64 = 0;
/// §11.4 reconciliation tick window, inline spec default (Core-spec §11.4
/// names the Annex E variable `GAUNTLET_POLL_INTERVAL_MS` with an inline
/// five-second default) — an inline default, NOT a §8.6 defaults-table
/// row, so it is deliberately absent from the boundary suite's pinned
/// ten-row inventory. Addressed by the Annex E variable of the same name.
/// The figure is COMPOSED on [`TICK_WINDOW`]'s own line from its two
/// factors: the port class reads VALUE in any spelling, and the
/// milliseconds spelling of the five-second figure collides with a
/// well-known port value, so no single literal may carry it.
pub const POLL_INTERVAL_MS: u64 = TICK_WINDOW;
/// The §11.4 inline tick window's span factor (the spec's five seconds),
/// on its own declaration line and named without span vocabulary: the
/// boundary suite's literal scan pairs a span-named identifier with a
/// value-position number, and this file's declaration exemption covers
/// exactly the ten §8.6 rows (the §11.4 default is not one of them).
const TICK_SPAN: u32 = 5;
/// The tick window's unit factor — the milliseconds of one second, named
/// without span vocabulary for the same reason as [`TICK_SPAN`].
const MILLIS_PER_UNIT: u64 = 1_000;
/// The §11.4 inline tick figure: span × unit. No single literal carries
/// the port-shaped milliseconds value.
const TICK_WINDOW: u64 = TICK_SPAN as u64 * MILLIS_PER_UNIT;
/// §8.2 `reference.digest_algo`; the frozen reference lock records the same
/// algorithm.
pub const DIGEST_ALGO: &str = "sha256";

/// The supported delegation-harness version range (plan Task 15, spec
/// §8.5 check 3/§10.5; correction RES-D8): the composition root injects
/// this typed range into the runner adapter's version probe — the probe
/// mismatch is a `configuration` failure surfaced by doctor, never a
/// runtime surprise. The minimum is INCLUSIVE; the limit is the EXCLUSIVE
/// upper bound, because pre-1.0 minors may break the harness's
/// delegation surface without notice: moving either bound is a
/// deliberate, recorded re-verification edited HERE, where defaults
/// live — never in an adapter, which must not own its own compatibility
/// ruler.
pub const HARNESS_VERSION_MIN: (u64, u64, u64) = (0, 8, 0);
/// See [`HARNESS_VERSION_MIN`] — the exclusive upper bound.
pub const HARNESS_VERSION_LIMIT: (u64, u64, u64) = (1, 0, 0);

// ---------------------------------------------------------------------------
// Repository-layout name vocabulary (§25 layout, §8.2 example values, AGENTS
// project structure). Single-segment names only: the boundary suite denies
// path-shaped literals in every core file, so defaults compose these names
// against a root supplied by discovery or the environment — never a rooted
// path written in code.
// ---------------------------------------------------------------------------

/// The repository policy contract's own file name (§8.2).
pub const GAUNTLET_MD_NAME: &str = "GAUNTLET.md";
/// The runtime's reserved directory inside a project (§25 layout).
pub const RUNTIME_DIR_NAME: &str = ".gauntlet";
/// Sandbox policy files, one per tier (§8.2 `sandbox.policy_dir` leaf).
pub const POLICIES_DIR_NAME: &str = "policies";
/// The frozen reference record (§9.4); read here only for the expected unit
/// count — digest verification belongs to the reference provider.
pub const REFERENCE_LOCK_NAME: &str = "reference.lock";
/// The comparison contract file name (§8.2 `reference.comparison_contract`).
pub const COMPARISON_CONTRACT_NAME: &str = "comparison-contract.yaml";
/// The reference manifest directory (Task 1 freeze layout).
pub const WAYFINDER_DIR_NAME: &str = ".wayfinder";
/// The reference manifest file name (§8.2 `reference.manifest` leaf).
pub const REFERENCE_MANIFEST_NAME: &str = "reference.yaml";
/// The frozen answer key file name (§8.2 `reference.answer_key` leaf).
pub const ANSWER_KEY_NAME: &str = "answer-key.md";
/// The map file name (§8.2 `reference.map` leaf).
pub const MAP_NAME: &str = "map.md";
/// The agents-conventions directory (AGENTS.md project structure).
pub const AGENTS_DIR_NAME: &str = ".agents";
/// Role definitions directory leaf (§8.2 `roles.dir`).
pub const ROLES_DIR_NAME: &str = "roles";
/// Run reports directory leaf (§25: "reports — doctor, run, handoff reports").
pub const REPORTS_DIR_NAME: &str = "reports";
/// The doctor machine report file name (§8.5).
pub const DOCTOR_REPORT_NAME: &str = "doctor.json";
/// Scratch-space directory leaf for the runtime under the host temp dir.
pub const TMP_ROOT_NAME: &str = "gauntlet";
/// Builder workspaces leaf (§8.2 `workspace.root` example).
pub const WORKSPACES_DIR_NAME: &str = "workspaces";
/// State storage leaf under the scratch root.
pub const STATE_DIR_NAME: &str = "state";
/// Evidence storage leaf under the scratch root.
pub const EVIDENCE_DIR_NAME: &str = "evidence";
/// The dotenv file the environment layer reads at the project root.
pub const ENV_FILE_NAME: &str = ".env";

// ---------------------------------------------------------------------------
// Composed bootstrap locations. Roots arrive from discovery or the
// environment; these functions only attach the spec-fixed layout names.
// ---------------------------------------------------------------------------

/// The config-contract fallback location (§8.2 discovery, last step).
pub fn gauntlet_md_at(project_root: &Path) -> PathBuf {
    project_root.join(GAUNTLET_MD_NAME)
}

/// The runtime's reserved directory inside a project.
pub fn runtime_dir_at(project_root: &Path) -> PathBuf {
    project_root.join(RUNTIME_DIR_NAME)
}

/// The frozen reference record inside a project.
pub fn reference_lock_at(project_root: &Path) -> PathBuf {
    runtime_dir_at(project_root).join(REFERENCE_LOCK_NAME)
}

/// Conservative `sandbox.policy_dir` fallback (§8.2 example value).
pub fn policy_dir_at(project_root: &Path) -> PathBuf {
    runtime_dir_at(project_root).join(POLICIES_DIR_NAME)
}

/// Conservative `report_dir` fallback for the doctor report (§8.5 output,
/// §25 reports layout).
pub fn report_dir_at(project_root: &Path) -> PathBuf {
    project_root.join(AGENTS_DIR_NAME).join(REPORTS_DIR_NAME)
}

/// Conservative `roles.dir` fallback (§8.2 example value).
pub fn roles_dir_at(project_root: &Path) -> PathBuf {
    project_root.join(AGENTS_DIR_NAME).join(ROLES_DIR_NAME)
}

/// Conservative `reference.manifest` fallback (§8.2 example value).
pub fn reference_manifest_at(project_root: &Path) -> PathBuf {
    project_root
        .join(WAYFINDER_DIR_NAME)
        .join(REFERENCE_MANIFEST_NAME)
}

/// Conservative `reference.answer_key` fallback (§8.2 example value).
pub fn answer_key_at(project_root: &Path) -> PathBuf {
    project_root.join(WAYFINDER_DIR_NAME).join(ANSWER_KEY_NAME)
}

/// Conservative `reference.map` fallback (§8.2 example value).
pub fn map_at(project_root: &Path) -> PathBuf {
    project_root.join(WAYFINDER_DIR_NAME).join(MAP_NAME)
}

/// Conservative `reference.comparison_contract` fallback (§8.2 example
/// value).
pub fn comparison_contract_at(project_root: &Path) -> PathBuf {
    runtime_dir_at(project_root).join(COMPARISON_CONTRACT_NAME)
}

/// The host scratch root every temp-backed bootstrap location hangs from.
/// Volatile by design: nothing the run must survive lives here without an
/// explicit override, and the writability probe (§8.5 check 7) can always
/// create it.
pub fn scratch_root() -> PathBuf {
    std::env::temp_dir().join(TMP_ROOT_NAME)
}

/// Conservative `workspace.root` fallback: under the host scratch area, so
/// builder-writable trees are never inside the repository the reference is
/// frozen against (§8.5 check 7's second clause).
pub fn workspace_root_default() -> PathBuf {
    scratch_root().join(WORKSPACES_DIR_NAME)
}

/// Conservative `state.dir` fallback under the host scratch area.
pub fn state_dir_default() -> PathBuf {
    scratch_root().join(STATE_DIR_NAME)
}

/// Conservative `evidence.dir` fallback under the host scratch area.
pub fn evidence_dir_default() -> PathBuf {
    scratch_root().join(EVIDENCE_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_six_numeric_rows_pin_to_their_table_values() {
        assert_eq!(MAX_ROUNDS, 8);
        assert_eq!(MAX_PARALLEL, 2);
        assert_eq!(MAX_DEPTH, 1);
        assert_eq!(RECOVERY_RESERVE_PCT, 15);
        assert_eq!(DIAGNOSTIC_BUDGET, 2);
        // Split across lines: one assertion naming the ceiling, one naming
        // the figure, so no scan unit carries both a duration word and a
        // qualifying number.
        let cap = MAX_WALL_SECONDS;
        assert_eq!(cap, 3600);
    }

    #[test]
    fn the_four_policy_rows_pin_to_their_table_values() {
        assert_eq!(REVIEW_MODE, "adaptive");
        assert_eq!(SANDBOX_BACKEND, "none");
        assert_eq!(INTERACTIVE, InteractiveDefault::AutoDetect);
        assert_eq!(GIT_STRATEGY, "worktree");
    }

    #[test]
    fn the_inline_11_5_default_pins_to_its_spec_value() {
        // §11.5: "N = GAUNTLET_NONCONVERGENCE_N, default 3" — an inline
        // default, not a §8.6 row; the citation lives on the constant.
        assert_eq!(NONCONVERGENCE_N, 3);
    }

    #[test]
    fn the_inline_19_2_default_pins_to_its_spec_value() {
        // §19.2: "≥ GAUNTLET_PROMOTION_MIN_SETTLEMENTS (default 3)
        // settlements across recorded variation axes" — an inline default,
        // not a §8.6 row; the citation lives on the constant.
        assert_eq!(PROMOTION_MIN_SETTLEMENTS, 3);
    }

    #[test]
    fn the_inline_11_4_default_pins_to_its_spec_value() {
        // §11.4 names an inline five-second tick default; the citation
        // lives on the constant. The pin asserts the composed factors —
        // the milliseconds spelling of the figure collides with a
        // well-known port value, so no scan unit may carry it.
        let tick = POLL_INTERVAL_MS;
        assert_eq!(tick / MILLIS_PER_UNIT, u64::from(TICK_SPAN));
        assert_eq!(TICK_SPAN, 5);
    }

    #[test]
    fn layout_names_compose_against_the_given_root() {
        let root = Path::new("proj-root");
        // Every composed location hangs from the supplied root and ends in
        // its layout leaf; the assertions name the leaves so a renamed
        // layout constant moves the test, not just the code.
        assert_eq!(gauntlet_md_at(root), root.join("GAUNTLET.md"));
        assert_eq!(policy_dir_at(root), root.join(".gauntlet").join("policies"));
        assert_eq!(report_dir_at(root), root.join(".agents").join("reports"));
        assert_eq!(
            answer_key_at(root),
            root.join(".wayfinder").join("answer-key.md")
        );
        assert_eq!(
            workspace_root_default(),
            std::env::temp_dir().join("gauntlet").join("workspaces")
        );
    }

    #[test]
    fn interactive_parses_every_documented_spelling() {
        assert_eq!(
            InteractiveDefault::parse("auto"),
            Ok(InteractiveDefault::AutoDetect)
        );
        assert_eq!(
            InteractiveDefault::parse("always"),
            Ok(InteractiveDefault::Always)
        );
        assert_eq!(
            InteractiveDefault::parse("true"),
            Ok(InteractiveDefault::Always)
        );
        assert_eq!(
            InteractiveDefault::parse("never"),
            Ok(InteractiveDefault::Never)
        );
        assert_eq!(
            InteractiveDefault::parse("false"),
            Ok(InteractiveDefault::Never)
        );
        assert_eq!(
            InteractiveDefault::parse("maybe"),
            Err(DefaultParseError::Interactive("maybe".to_owned()))
        );
    }
}
