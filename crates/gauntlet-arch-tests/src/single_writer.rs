//! The single-writer / no-writer rule over the answer-key surface (spec
//! §16.4; AK-043's "prevented" leg (a); plan Task 13, builder B3 Item 1).
//!
//! The construction §16.4 demands and this check pins: within the CORE
//! crates, the modules that OWN or DERIVE the answer-key path are
//! enumerated, and none of them contains a write-shaped filesystem
//! operation in production code — no writer of the ruler exists anywhere
//! in the core besides git (recon §7). The provider surface is read-only
//! by construction; this rule makes that construction mechanical, so a
//! future `std::fs::write` in `reference/provider.rs` fails the boundary
//! suite instead of waiting for a critic.
//!
//! # The three rules
//!
//! - **A — the surface inventory.** The production modules that name the
//!   answer-key surface ([`ANSWER_KEY_SURFACE_TOKENS`]: the wayfinder dir
//!   name, `AnswerKeySource`, the answer-key file name) are pinned in
//!   [`ANSWER_KEY_SURFACE_FILES`]. A new core file that names the surface
//!   fails the pin until it is inventoried deliberately; a pinned file
//!   that stops naming it fails as a stale entry.
//! - **B — no unguarded writer at the surface.** A write-API token in a
//!   surface module's production text must be pinned in
//!   [`WRITE_API_INVENTORY`] WITH a containment guard that is present in
//!   the same file's production text. A lexical rule cannot see WHICH
//!   path a write targets — the guard requirement is the surface's
//!   lexical warrant that the write never touches the ruler: the
//!   reference side (`provider`/`parse`/`manifest`/`mod`, `defaults`) is
//!   write-free outright today, and the controller — a surface HOLDER,
//!   not a path deriver — warrants its facet-locator materialization
//!   with `contained_locator_path`. There is no unguarded write at the
//!   surface, and no write at all on the reference side.
//! - **C — the write inventory.** EVERY production write-API token in
//!   EVERY core crate is pinned in [`WRITE_API_INVENTORY`] with its
//!   reason and, where the write is only legitimate behind a containment
//!   surface, a GUARD needle that must remain present in the same file's
//!   production text. A new production write anywhere in a core crate
//!   fails the pin until it is added here; a pinned write that moves or
//!   disappears fails as stale; a guarded write whose guard is deleted or
//!   renamed away fails the guard clause — the containment surface for
//!   that write cannot silently vanish.
//!
//! # What "production text" means here
//!
//! A file is production when it is `.rs` under a core crate's `src/`
//! tree and is not compiled only under test. Integration-test targets
//! (`<crate>/tests/**`) and cargo's other non-production target
//! directories live outside `src/` and never enter the scan. Inside
//! `src/`, every `#[cfg(test)] mod NAME { … }` region is blanked with
//! brace balancing over the ONE lexing pass ([`crate::text::regions`]),
//! so a quote or a `/*` inside the test module cannot shift the braces;
//! a `#[cfg(test)] mod NAME;` declaration marks `<dir>/NAME.rs` (or
//! `<dir>/NAME/mod.rs` when the flat file is absent) as test-only for
//! the cross-file exclusion — the shape `ports/src/contract` uses for
//! its suite files. Blanking PRESERVES newlines so line numbers stay
//! true. Anything the blanker cannot classify — a `#[cfg(test)]`
//! attribute on an item that is not a module, a module body whose brace
//! never closes — is a VIOLATION, never a guess (I8: the fail-closed
//! direction, because wrongly blanking production text would be an
//! evasion hole, not a false alarm).
//!
//! # THREAT MODEL — what this rule is not
//!
//! This is a LEXICAL pin of modules and write-API tokens, NOT a data-flow
//! analysis. The preregistered P5 hazard (literal overfit) was tested
//! with a bounded masked-spelling battery, and every escape is recorded
//! here rather than hidden:
//!
//! - **Env-injected path** (`env::var(…) → PathBuf → fs::write`): the
//!   path's provenance is invisible, but the write-API token still fires
//!   Rule C — the site must be inventoried. CAUGHT AT THE TOKEN.
//! - **`format!`-constructed surface mention**
//!   (`format!(".way{}inder/…", "f")`): no literal surface token, so
//!   Rule A does not see the file — though any write in it still fires
//!   Rule C. RULE A ESCAPE.
//! - **Generic helper funnel** (`fn put(p: impl AsRef<Path>)` in a
//!   non-surface core file, called with the key path from a surface
//!   file): the helper's write token fires Rule C; the surface file
//!   itself stays write-token-free, so Rule B is silent on it — the
//!   write is denied at the token, wherever it lives in a core crate.
//!   CAUGHT AT THE TOKEN.
//! - **Trait-object indirection** (a `dyn Writer` impl living in an
//!   ADAPTER, invoked from a surface file): no write-API token in any
//!   core crate at all. GENUINE ESCAPE — closing it is the job of the
//!   authority/containment layers (§8.5 check 7, §17.5) and the g10
//!   end-to-end proof (B4), not of a lexical rule. Adapters are outside
//!   [`walk::CORE_CRATES`] by the suite's declared scope.
//! - **Re-spelled std API** (`use std::fs::write as w;` then `w(…)`): an
//!   aliased import drops the `fs::write` token. ESCAPE. The
//!   non-aliased import (`use std::fs::write;`, bare `write(…)`) escapes
//!   the same way; the qualified and `fs::`-prefixed spellings — every
//!   spelling this tree actually uses — do not.
//! - **A write moved behind a macro** defined in a core crate: the
//!   macro's own body still carries the token (Rule C fires at the
//!   definition site); a write assembled entirely from macro-internal
//!   string concatenation (`concat!("fs::", "write")` in a doc or a
//!   `stringify!` shape) would not. Residual of every lexical scanner;
//!   see the crate-level "proc-macro and code generation" limit.
//!
//! The surface half is likewise lexical: it pins modules that NAME the
//! answer-key surface. `GAUNTLET_ANSWER_KEY_PATH` (the cli spec-vocab
//! env-var name, `gauntlet-cli/src/spec_vocab.rs`) redirects the answer
//! key through configuration and is deliberately NOT a surface-inventory
//! token — it names an Annex E variable, not the ruler's owning module,
//! and it carries no write API. It is recorded here so the boundary is a
//! drawn line, not an oversight.

use std::collections::BTreeSet;
use std::path::Path;

use crate::text::Region;
use crate::walk;

/// The answer-key surface tokens, lowercase, matched case-insensitively
/// over production text (code, comments, and string literals alike — a
/// module that names the ruler in a doc comment is still a module that
/// names the ruler). Growth is a deliberate edit that updates the pin in
/// [`tests`]; the tokens are the wayfinder DIRECTORY name, the provider's
/// source TYPE, and the key FILE name — the three spellings the six
/// pinned modules use.
pub const ANSWER_KEY_SURFACE_TOKENS: &[&str] = &["wayfinder", "answerkeysource", "answer_key_name"];

/// The pinned answer-key surface inventory: every core-crate production
/// module that names the surface, each with its reason. Six of the seven
/// are the `gauntlet-app` modules recon §7 found that actually own or
/// derive the `.wayfinder` reference path; nothing in domain, ports, or
/// the composition root names the ruler (their "answer key" prose is the
/// two-word phrase, which no token here matches). The seventh
/// (`skills/mod.rs`, plan Task 19) is a lexical false positive on
/// purpose, pinned rather than avoided: `REPO_OWNED_SKILLS` must contain
/// the literal, exact directory name `wayfinder-gauntlet` for its
/// filesystem lookups to be correct, and no rephrasing removes that
/// requirement — the entry documents that the match is real but the
/// coupling it usually signals is not. Growth or shrinkage is a
/// deliberate, recorded edit — the pin test holds it exact.
pub const ANSWER_KEY_SURFACE_FILES: &[(&str, &str)] = &[
    (
        "crates/gauntlet-app/src/config/defaults.rs",
        "declares the wayfinder dir and answer-key names (§8.6) and derives the default \
         key/manifest/map paths",
    ),
    (
        "crates/gauntlet-app/src/control/controller.rs",
        "holds the AnswerKeySource and re-reads it per tick (strict_units, boundary verify)",
    ),
    (
        "crates/gauntlet-app/src/reference/mod.rs",
        "the reference module's own contract names the frozen .wayfinder surface",
    ),
    (
        "crates/gauntlet-app/src/reference/parse.rs",
        "the strict answer-key parser over .wayfinder/answer-key.md",
    ),
    (
        "crates/gauntlet-app/src/reference/manifest.rs",
        "the frozen-manifest reader for .wayfinder/reference.yaml",
    ),
    (
        "crates/gauntlet-app/src/reference/provider.rs",
        "FileReferenceProvider over .wayfinder; defines AnswerKeySource — the read-only seam",
    ),
    (
        "crates/gauntlet-cli/src/compose.rs",
        "the run composition CONSTRUCTS the provider over the configured answer-key path          (AnswerKeySource::Path from the resolved config) — it derives no ruler path itself          and its writes are the configured state/evidence dir creations, never the ruler",
    ),
    (
        "crates/gauntlet-cli/src/init_cmd.rs",
        "the project scaffold writes a COMMENTED GAUNTLET.md template whose wayfinder paths          are inert examples — it refuses any existing file and never writes a real ruler",
    ),
    (
        "crates/gauntlet-cli/src/reference_cmd.rs",
        "reference show/verify/diff READ the configured key and the frozen lock (the typed          just-ruler twin); freeze writes evidence-store objects, never the ruler files",
    ),
    (
        "crates/gauntlet-app/src/skills/mod.rs",
        "REPO_OWNED_SKILLS names the installed skill directory 'wayfinder-gauntlet' — a skill \
         identity string the lint's filesystem lookups must match exactly, not a path this \
         module owns or derives; it neither reads nor writes .wayfinder/answer-key.md",
    ),
];

/// The write-API token set. Matched over production text with identifier
/// boundaries on both sides (so `fs::symlink` does not match
/// `fs::symlink_metadata`, and `fs::create_dir` does not match
/// `fs::create_dir_all`), case-sensitively, in code, comments, and
/// strings alike — a write-API spelling anywhere in production text is
/// inventory material. `OpenOptions` is pinned as the TYPE NAME itself:
/// any construction of one in a core crate is a file-handle the
/// inventory must justify (read-only opens have `File::open`).
pub const WRITE_API_TOKENS: &[&str] = &[
    "fs::write",
    "fs::remove_file",
    "fs::remove_dir",
    "fs::remove_dir_all",
    "fs::rename",
    "fs::copy",
    "fs::create_dir",
    "fs::create_dir_all",
    "fs::hard_link",
    "fs::soft_link",
    "fs::symlink",
    "fs::link",
    "File::create",
    "OpenOptions",
];

/// One pinned production write site. `guard`, when present, is a needle
/// that must remain in the SAME file's production text for the reason to
/// hold — the containment surface (or owning function) the write is only
/// legitimate behind. Deleting or renaming the guard while the write
/// remains fails the check: the inventory must be re-litigated in the
/// open, never silently detached from its warrant.
pub struct WriteSite {
    pub file: &'static str,
    pub token: &'static str,
    pub reason: &'static str,
    pub guard: Option<&'static str>,
}

/// The pinned inventory of EVERY production write-API token in the core
/// crates. Three site families exist today, all in `gauntlet-app`:
///
/// - the controller's facet-locator materialization — the loop's only
///   builder-side byte write, gated by `contained_locator_path` (§17.5
///   containment; the refusal is pinned behaviorally by controller test
///   `an_escaping_evidence_locator_is_refused_at_the_write_boundary`);
/// - the doctor report writer (operator-facing report path from
///   configuration — never a reference path);
/// - the doctor's §8.5 check-7 writable probe (probe files inside the
///   configured state/evidence/workspace dirs; the ANSWER KEY's
///   placement is what check 7 polices, so the probe never targets it).
///
/// AK-043 prevention leg (b) rides on this inventory's guard clause: the
/// controller entries cannot lose `contained_locator_path` without this
/// check failing. Builder-effect confinement beyond the core's own write
/// boundary (Task-9 workspace containment, Task-8 mock effect guards) is
/// enforced by the shared WorkspaceProvider contract suite's escape case
/// and the mock runner's two independent guards — cited, not duplicated,
/// here.
pub const WRITE_API_INVENTORY: &[WriteSite] = &[
    // Plan Task 20: the run composition's durable-root creation — the state
    // and evidence dirs come from the RESOLVED configuration, never from a
    // literal; the guard is the composition's own path-from-config helper.
    WriteSite {
        file: "crates/gauntlet-cli/src/compose.rs",
        token: "fs::create_dir_all",
        reason: "ensuring the configured state and evidence directories exist before the                  run's stores open (plan Task 20 composition)",
        guard: Some("fn ensure_configured_dir"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/init_cmd.rs",
        token: "fs::create_dir_all",
        reason: "project scaffold creates the .gauntlet runtime dir (plan Task 20); the                  command refuses any existing file first",
        guard: Some("refusing to overwrite"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/init_cmd.rs",
        token: "fs::write",
        reason: "project scaffold writes the commented GAUNTLET.md template and the minimal                  env example — inert examples, never a real ruler (plan Task 20)",
        guard: Some("refusing to overwrite"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/skills_cmd.rs",
        token: "fs::create_dir_all",
        reason: "skills install creates the per-skill installed directory under the state                  dir's runtime tree (plan Task 20, D8)",
        guard: Some("fn installed_root"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/skills_cmd.rs",
        token: "fs::write",
        reason: "skills install writes the installed SKILL.md copies and the content-digest                  skills lock (plan Task 20, D8)",
        guard: Some("fn installed_root"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/state_cmd.rs",
        token: "fs::write",
        reason: "export --out writes the canonical export to the OPERATOR-named path only                  (plan Task 20); the default writes nothing (stdout)",
        guard: Some("--out"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/control/controller.rs",
        token: "fs::create_dir_all",
        reason: "facet-locator parent directories inside the canonical workspace root",
        guard: Some("fn contained_locator_path"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/control/controller.rs",
        token: "fs::write",
        reason: "facet-locator byte materialization inside the canonical workspace root — the \
                 loop's only builder-side byte write, behind the containment guard",
        guard: Some("fn contained_locator_path"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/doctor/mod.rs",
        token: "fs::create_dir_all",
        reason: "doctor report directory creation (operator-facing report path from \
                 configuration)",
        guard: Some("fn write_report"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/doctor/mod.rs",
        token: "fs::write",
        reason: "doctor report JSON write (operator-facing report path from configuration)",
        guard: Some("fn write_report"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/doctor/checks.rs",
        token: "fs::create_dir_all",
        reason: "§8.5 check-7 writable probe of the configured state/evidence/workspace dirs",
        guard: Some("fn probe_writable"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/doctor/checks.rs",
        token: "fs::write",
        reason: "§8.5 check-7 writable-probe scratch file inside the probed dir",
        guard: Some("fn probe_writable"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/doctor/checks.rs",
        token: "fs::remove_file",
        reason: "§8.5 check-7 writable-probe scratch removal",
        guard: Some("fn probe_writable"),
    },
    // Plan Task 17 — the roles installer (the runtime's own operator
    // action, never a graded-role capability): installs write ONLY
    // verbatim role bytes under the pre-validated state root, and the
    // lock beside the project. Warrant differs by site family (COR-D11):
    // the installed-directory create_dir_all IS behind the lexical
    // containment call (`ensure_within_root` runs on the assembled path
    // before any byte is written, pinned behaviorally by the roles tests'
    // write-path-escape refusal); the lock-path sites — the lock-dir
    // create_dir_all, the staging fs::write, and the committing rename —
    // are safe by FIXED-LEAF construction: every one of them targets
    // `project_root/.gauntlet/roles.lock` or `<validated-stem>.staging`
    // beside a target already derived from validated roots, so no
    // unvalidated component can enter the path. The guard needle below
    // stays pinned on all three entries so the containment surface cannot
    // silently disappear from this file.
    WriteSite {
        file: "crates/gauntlet-app/src/roles/install.rs",
        token: "fs::create_dir_all",
        reason: "roles install: the installed directory under the state root — this \
                 create_dir_all sits behind the ensure_within_root containment call",
        guard: Some("fn ensure_within_root"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/roles/install.rs",
        token: "fs::write",
        reason: "roles install: atomic-write staging file — fixed-leaf target beside a \
                 validated destination (the lock's `.gauntlet` leaf or an installed \
                 `<validated-stem>.md`), so no unvalidated component enters the path",
        guard: Some("fn ensure_within_root"),
    },
    WriteSite {
        file: "crates/gauntlet-app/src/roles/install.rs",
        token: "fs::rename",
        reason: "roles install: atomic-write commit over the target — fixed-leaf \
                 construction (the runtime lock leaf `project_root/.gauntlet/roles.lock` \
                 or a validated `<stem>.md` under the pre-validated installed dir)",
        guard: Some("fn ensure_within_root"),
    },
    // Plan Task 20, COR-R3-3 (falsifier round 3): the state-dir
    // single-writer guard — the lock file's path is `state_dir.join` of
    // the RESOLVED configuration dir, and the write is the single-writer
    // admission itself: `create_new(true)` is the O_EXCL exclusion whose
    // failure IS the typed second-writer refusal.
    WriteSite {
        file: "crates/gauntlet-cli/src/writer_lock.rs",
        token: "fs::create_dir_all",
        reason: "ensuring the configured state dir exists before the exclusive writer-lock \
                 file is created (the state dir comes from the resolved configuration, \
                 never a literal)",
        guard: Some("fn acquire"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/writer_lock.rs",
        token: "OpenOptions",
        reason: "the single-writer admission itself: `create_new(true)` is the O_EXCL \
                 exclusion — a second concurrent run/resume fails here and is refused \
                 typed, naming the holder pid; the guard is fail-closed with no \
                 liveness guessing and no automatic takeover",
        guard: Some("fn acquire"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/writer_lock.rs",
        token: "fs::remove_file",
        reason: "the holder's release: the lock file is removed only by the handle that \
                 created it, on every exit path of its own command (a failed removal \
                 leaves the stale-lock case the typed refusal documents for manual \
                 recovery)",
        guard: Some("impl Drop for WriterLock"),
    },
    // Task 26, COR-3: the `improve baseline` measurement harness (plan
    // §Task 26, AK-069) — one entry per production write site, in file
    // order. Every site writes ONLY the run's own scratch worlds, the
    // materialized per-leg/per-process instruments, or the operator-named
    // `--out` report — never a reference, contract, or evidence artifact.
    // The composition root resolves every path from the enumerated fixture
    // roots, the temp dir, or the operator's flag; no literal contributes
    // a path component. Guard needles are the owning functions: each write
    // is legitimate only inside the function named, and its disappearance
    // forces a re-litigation here.
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline: creating the process-unique instrument directory under the \
                 temp dir (the static judge shim + child wrapper live there for this \
                 process only)",
        guard: Some("fn establish_leg_environment"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline: materializing the static child wrapper bytes inside the \
                 process-unique instrument directory (interpreter shebang baked from \
                 the host SHELL configuration)",
        guard: Some("fn establish_leg_environment"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline: materializing the static judge shim bytes inside the \
                 process-unique instrument directory (same baked interpreter)",
        guard: Some("fn establish_leg_environment"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline: the parent's cleanup of its own process-unique instrument \
                 directory after the re-exec's status returns (every exit path short \
                 of a kill)",
        guard: Some("fn establish_leg_environment"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline: creating the run's scratch legs root (under the test-hook \
                 path or the temp dir — never the fixture tree)",
        guard: Some("fn baseline"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline: creating the operator-named report parent before the atomic \
                 write (--out; the default assembles the charter's frozen locator)",
        guard: Some("fn baseline"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline: ScratchGuard removes the run's scratch world on drop unless \
                 the test hook retained it — the guard's own path, nothing else",
        guard: Some("impl Drop for ScratchGuard"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "File::create",
        reason: "baseline: the report's STAGING file beside the operator-named --out \
                 path (the roles write_atomic pattern: create staging, write, fsync, \
                 rename)",
        guard: Some("fn write_report_atomically"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::rename",
        reason: "baseline: the atomic commit of the staged report over the \
                 operator-named --out path",
        guard: Some("fn write_report_atomically"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline: the leg's scenario file (the static child wrapper reads it \
                 from the leg directory; the workspace forbids unsafe code, so files \
                 — not the process environment — carry per-leg data)",
        guard: Some("fn write_leg_files"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline: the leg's child-path file (the leg's fixture child, inside \
                 its own scratch workspace; same per-leg channel)",
        guard: Some("fn write_leg_files"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline: a leg's fresh-world guarantee — its leg directory is wiped \
                 before materialization so nothing from a previous run leaks in",
        guard: Some("fn materialize_workspace"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline: creating the leg's fresh workspace directory tree inside \
                 its own leg directory (the scratch world the leg owns)",
        guard: Some("fn materialize_workspace"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline D leg: creating the judge-owned snapshot directory under the \
                 leg's scratch tree (outside every workspace the loop can dispatch)",
        guard: Some("fn run_full_loop"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline D leg: materializing the leg's judge wrapper (declared-check \
                 path, snapshot dir, and deliverable baked; interpreter shebang from \
                 the host SHELL) inside the leg's scratch tree",
        guard: Some("fn run_full_loop"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::write",
        reason: "baseline D leg: writing the leg-file pointer the static judge shim \
                 resolves (the wrapper's absolute path, one line, in the leg's scratch \
                 directory)",
        guard: Some("fn run_full_loop"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the snapshot judge's temp staging root removed \
                 before use (a stale layout from this process's pid must never seed a \
                 judgment)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline D judge: creating the snapshot staging layout's src directory \
                 inside the judge's own temp staging root",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the staging layout removed when a snapshot records \
                 a failed copy (the typed judge refusal path)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::copy",
        reason: "baseline D judge: staging one judged snapshot's bytes into the judge's \
                 own temp layout (snapshot reads, temp writes — the fixture instrument \
                 never runs over the original)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the staging layout removed when a snapshot cannot \
                 be staged (the typed staging-failure path)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the staging layout removed when the fixture \
                 instrument cannot spawn over it (the typed spawn-failure path)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the staging layout removed when an entry shape is \
                 unrecognized (fail-closed refuse path)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::remove_dir_all",
        reason: "baseline D judge: the staging layout removed after the last entry is \
                 judged (the judge leaves nothing behind)",
        guard: Some("fn judge_snapshots"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::create_dir_all",
        reason: "baseline: copy_tree creates each destination directory inside the leg's \
                 scratch workspace while copying the task project (the e2e World \
                 pattern; the source is the enumerated fixture root, the destination \
                 the leg's own fresh workspace)",
        guard: Some("fn copy_tree"),
    },
    WriteSite {
        file: "crates/gauntlet-cli/src/improve_cmd.rs",
        token: "fs::copy",
        reason: "baseline: copy_tree copies each fixture file into the leg's scratch \
                 workspace (verbatim materialization of the leg's fresh world; never \
                 writes back into the fixture tree)",
        guard: Some("fn copy_tree"),
    },
];

/// The check body, called by [`crate::check_answer_key_single_writer`].
pub fn check(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    // Pre-pass: the cross-file test-only set. A `#[cfg(test)] mod NAME;`
    // declaration in one file gates a DIFFERENT file, so the exclusion
    // must be computed over the whole tree before any production scan.
    let tree = walk::core_tree(root);
    let mut test_only = BTreeSet::new();
    for file in &tree.files {
        if !is_production_candidate(file) {
            continue;
        }
        let text = match walk::read_scannable(root, file) {
            Ok(text) => text,
            // The scan closure below re-reads and reports unreadable
            // files through the shared path; reporting here too would
            // duplicate every message.
            Err(_) => continue,
        };
        // Derivation failures are reported by the scan closure, which
        // re-derives the same text over the shared, guarded path.
        if let Ok((_, declared)) = derive_production_text(&text) {
            for name in declared {
                match gated_module_file(&tree.files, file, &name) {
                    Some(path) => {
                        test_only.insert(path);
                    }
                    None => violations.push(format!(
                        "single-writer: '{file}' declares #[cfg(test)] mod {name} but no \
                         source file backs it — a broken module the blanker refuses to guess \
                         about (I8)"
                    )),
                }
            }
        }
    }

    // The production scan runs through the shared path so the suite's
    // anti-vacuity guard measures it like every other repo-level check.
    let surface: Vec<(String, Vec<&'static str>)> = Vec::new();
    let writes: Vec<(String, usize, &'static str)> = Vec::new();
    let collected = std::cell::RefCell::new((surface, writes));
    let guards: Vec<(&'static str, &'static str)> = WRITE_API_INVENTORY
        .iter()
        .filter_map(|site| site.guard.map(|guard| (site.file, guard)))
        .collect();
    let mut guard_hits: Vec<(&'static str, &'static str)> = Vec::new();
    let report = crate::scan_core_tree(
        root,
        "single-writer",
        walk::in_denylist_scope,
        walk::CORE_CRATES,
        &[],
        |file, text, _| {
            let mut out = Vec::new();
            if !is_production_candidate(file) || test_only.contains(file) {
                return out;
            }
            let masked = match derive_production_text(text) {
                Ok((masked, _)) => masked,
                Err(message) => {
                    out.push(format!("single-writer: '{file}': {message}"));
                    return out;
                }
            };
            let hits = surface_tokens_of(&masked);
            let write_hits = write_tokens_of(&masked);
            let is_surface = !hits.is_empty();
            let mut store = collected.borrow_mut();
            if is_surface {
                store.0.push((file.to_string(), hits));
            }
            for &(line, token) in &write_hits {
                store.1.push((file.to_string(), line, token));
            }
            // Rule B lives where the masked text is: a write token in a
            // surface module must be an inventoried site whose containment
            // guard is PRESENT in this same production text. A lexical
            // rule cannot see WHICH path a write targets — the guard
            // requirement is the surface's lexical warrant (§16.4).
            if is_surface {
                for (line, token) in &write_hits {
                    let warranted = WRITE_API_INVENTORY.iter().any(|site| {
                        site.file == file
                            && site.token == *token
                            && site.guard.is_some_and(|guard| masked.contains(guard))
                    });
                    if !warranted {
                        out.push(format!(
                            "single-writer: '{file}:{line}' uses write-API token '{token}' in an \
                             answer-key surface module — a surface-module write must be pinned in \
                             WRITE_API_INVENTORY WITH a present containment guard (spec §16.4); \
                             there is no unguarded write at the surface"
                        ));
                    }
                }
            }
            for (guard_file, guard) in &guards {
                if *guard_file == file && !masked.contains(guard) {
                    guard_hits.push((guard_file, guard));
                }
            }
            out
        },
    );
    violations.extend(report.violations);
    let (surface, writes) = collected.into_inner();

    // Rule A — the surface inventory.
    let pinned_surface: Vec<&str> = ANSWER_KEY_SURFACE_FILES.iter().map(|(f, _)| *f).collect();
    if surface.is_empty() {
        violations.push(
            "single-writer: no production module names the answer-key surface — the surface half \
             of this rule would be vacuous"
                .to_string(),
        );
    }
    for (file, tokens) in &surface {
        if !pinned_surface.contains(&file.as_str()) {
            violations.push(format!(
                "single-writer: '{file}' names the answer-key surface in production code \
                 ({tokens:?}) but is not in the pinned inventory — the modules that own or derive \
                 the answer-key path are enumerated deliberately (spec §16.4); extend \
                 ANSWER_KEY_SURFACE_FILES with a reason or remove the mention"
            ));
        }
    }
    for pinned in &pinned_surface {
        if !surface.iter().any(|(file, _)| file == pinned) {
            violations.push(format!(
                "single-writer: pinned surface file '{pinned}' no longer names the surface — \
                 update ANSWER_KEY_SURFACE_FILES (stale inventory)"
            ));
        }
    }

    // Rule B fires inside the scan closure, where the masked text (and so
    // the guard presence test) is available; see there.

    // Rule C — the write inventory, compared as (file, token) pairs.
    if WRITE_API_INVENTORY.is_empty() {
        violations.push(
            "single-writer: WRITE_API_INVENTORY is empty — the write half of this rule would be \
             vacuous"
                .to_string(),
        );
    }
    let mut found_pairs: BTreeSet<(String, &'static str)> = BTreeSet::new();
    for (file, _, token) in &writes {
        found_pairs.insert((file.clone(), *token));
    }
    let pinned_pairs: BTreeSet<(&str, &str)> = WRITE_API_INVENTORY
        .iter()
        .map(|site| (site.file, site.token))
        .collect();
    for (file, line, token) in &writes {
        if !pinned_pairs.contains(&(file.as_str(), token)) {
            violations.push(format!(
                "single-writer: '{file}:{line}' new production write-API token '{token}' in a \
                 core crate — add the site to WRITE_API_INVENTORY with its reason and guard, or \
                 remove the write"
            ));
        }
    }
    for (file, token) in &pinned_pairs {
        if !found_pairs.contains(&((*file).to_string(), *token)) {
            violations.push(format!(
                "single-writer: pinned production write '{token}' in '{file}' is gone — remove it \
                 from WRITE_API_INVENTORY (stale inventory)"
            ));
        }
    }

    // The guard clause: an inventoried write whose containment surface
    // vanished from the file's production text.
    for (file, guard) in guard_hits {
        violations.push(format!(
            "single-writer: '{file}' inventories a guarded write but the guard '{guard}' is \
             absent from its production text — the containment surface for that write has moved \
             or been renamed; re-litigate the inventory"
        ));
    }
    violations
}

/// Whether a walked file can carry production Rust: a `.rs` file under a
/// core crate's `src/` tree. Cargo compiles `<crate>/tests/**` (and
/// `benches/`, `examples/`) only as test targets, never into the
/// production artifacts, so those directories are outside this rule's
/// definition of production — a declared scope decision, not a carve-out
/// of a walked file.
fn is_production_candidate(rel: &str) -> bool {
    rel.ends_with(".rs") && rel.contains("/src/")
}

/// Resolve the file a `#[cfg(test)] mod NAME;` declaration gates:
/// `<dir>/NAME.rs`, or `<dir>/NAME/mod.rs` when the flat file is absent
/// (rustc's own resolution order). `None` when neither exists.
fn gated_module_file(files: &[String], declaring: &str, name: &str) -> Option<String> {
    let dir = declaring.rsplit_once('/')?.0;
    let flat = format!("{dir}/{name}.rs");
    if files.contains(&flat) {
        return Some(flat);
    }
    let nested = format!("{dir}/{name}/mod.rs");
    if files.contains(&nested) {
        return Some(nested);
    }
    None
}

/// Derive a file's production text: every `#[cfg(test)]`-gated module
/// region blanked (newlines preserved, so line numbers stay true), plus
/// the names of semicolon-form gated modules for the cross-file
/// exclusion. `Err` is a violation message — the blanker refuses to
/// guess (I8), because wrongly blanking production text would be an
/// evasion hole, not a false alarm.
fn derive_production_text(src: &str) -> Result<(String, Vec<String>), String> {
    let regs = crate::text::regions(src);
    let bytes = src.as_bytes().to_vec();
    let mut masked = bytes;
    let mut declared = Vec::new();
    let mut cursor = 0usize;
    while let Some(open) = next_test_attribute(src, &regs, cursor) {
        // `bracket_end` scans to end-of-text on an unclosed bracket — the
        // whole tail then reads as one attribute body, never `cfg(test)`,
        // and the file fails to compile regardless.
        let after_attr = crate::text::bracket_end(src, open.hash + 1);
        match item_after(src, &regs, after_attr)? {
            ItemShape::BraceModule { close } => {
                blank(&mut masked, open.hash..close);
                cursor = close;
            }
            ItemShape::SemicolonModule { name, semi } => {
                blank(&mut masked, open.hash..semi + 1);
                declared.push(name);
                cursor = semi + 1;
            }
            ItemShape::Function { close } => {
                blank(&mut masked, open.hash..close);
                cursor = close;
            }
        }
    }
    Ok((
        String::from_utf8(masked)
            .map_err(|_| "masked text is not UTF-8 — blanking preserved every byte".to_string())?,
        declared,
    ))
}

/// One candidate `#[cfg(test)]` attribute found in CODE (a position not
/// inside any string, char, or comment region).
struct AttrPosition {
    hash: usize,
}

/// The next `#[cfg(test)]` at or after `from`, in code only. Attributes
/// with any other cfg predicate (`cfg(unix)`, `cfg(feature = "…")`) are
/// skipped whole — they gate platforms and features, not tests, and
/// blanking them would hide production code.
fn next_test_attribute(src: &str, regs: &[Region], from: usize) -> Option<AttrPosition> {
    let bytes = src.as_bytes();
    let mut at = from;
    while let Some(found) = find_sub(bytes, b"#[cfg(", at) {
        let hash = found;
        // The whole attribute must be code; the region check below covers
        // the `#[cfg(` head, and the cfg arguments are parsed exactly.
        if in_region(regs, hash) {
            at = hash + 1;
            continue;
        }
        let close = crate::text::bracket_end(src, hash + 1);
        let body = &src[(hash + 2).min(close)..close.saturating_sub(1)];
        if body.split_whitespace().collect::<String>() == "cfg(test)" {
            return Some(AttrPosition { hash });
        }
        at = close;
    }
    None
}

/// The item shape that follows a `#[cfg(test)]` attribute (and any
/// further attributes): a brace module, a semicolon module, a function
/// (blanked through its body), or — fail closed — anything else.
enum ItemShape {
    BraceModule { close: usize },
    SemicolonModule { name: String, semi: usize },
    Function { close: usize },
}

fn item_after(src: &str, regs: &[Region], from: usize) -> Result<ItemShape, String> {
    let bytes = src.as_bytes();
    let mut at = skip_ws_and_attributes(src, regs, from);
    // Optional visibility: `pub`, `pub(crate)`, …
    if let Some((ident, end)) = ident_at(bytes, at)
        && ident == "pub"
    {
        at = skip_ws_and_attributes(src, regs, end);
        if bytes.get(at) == Some(&b'(') {
            let close = paren_end(bytes, at).ok_or_else(|| {
                "an unclosed `pub(…)` visibility follows a #[cfg(test)] attribute".to_string()
            })?;
            at = skip_ws_and_attributes(src, regs, close);
        }
    }
    let (keyword, end) = ident_at(bytes, at).ok_or_else(|| {
        format!(
            "a #[cfg(test)] attribute at line {} is not followed by an item the blanker has \
             classified — gating any other item shape must be classified here, never guessed (I8)",
            1 + src[..at.min(src.len())].matches('\n').count()
        )
    })?;
    if keyword == "fn" {
        // A gated function: blank through its body. The body's `{` is the
        // first brace at parenthesis depth zero after the name and
        // signature (the `config/mod.rs` fixture-path shape).
        let mut depth = 0i64;
        let mut at = end;
        while at < bytes.len() {
            if region_at(regs, at).is_some() {
                at = skip_ws_and_attributes(src, regs, at);
                continue;
            }
            match bytes[at] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'{' if depth <= 0 => {
                    let close = brace_end(src, regs, at).ok_or_else(|| {
                        "a gated function never closes its body — unclassifiable (I8)".to_string()
                    })?;
                    return Ok(ItemShape::Function { close });
                }
                _ => {}
            }
            at = next_char_end(bytes, at);
        }
        return Err("a gated function has no body brace — unclassifiable (I8)".to_string());
    }
    if keyword != "mod" {
        return Err(format!(
            "a #[cfg(test)] attribute gates '{keyword}' — the blanker blanks gated modules and \
             functions; gating any other item shape must be classified here, never guessed (I8)"
        ));
    }
    let at = skip_ws_and_attributes(src, regs, end);
    let (name, after_name) =
        ident_at(bytes, at).ok_or_else(|| "a gated module without a name".to_string())?;
    let at = skip_ws_and_attributes(src, regs, after_name);
    match bytes.get(at) {
        Some(b'{') => {
            let close = brace_end(src, regs, at).ok_or_else(|| {
                format!("gated module {name} never closes its brace — unclassifiable (I8)")
            })?;
            Ok(ItemShape::BraceModule { close })
        }
        Some(b';') => Ok(ItemShape::SemicolonModule { name, semi: at }),
        other => Err(format!(
            "gated module {name} is neither braced nor semicolon-terminated (found {other:?}) — \
             unclassifiable (I8)"
        )),
    }
}

/// Skip whitespace, comments, and any further `#[…]` attributes from
/// `at`, returning the first position of something else.
fn skip_ws_and_attributes(src: &str, regs: &[Region], from: usize) -> usize {
    let bytes = src.as_bytes();
    let mut at = from;
    loop {
        if let Some(span) = region_at(regs, at) {
            at = span;
            continue;
        }
        match bytes.get(at) {
            Some(b' ' | b'\t' | b'\r' | b'\n') => at += 1,
            Some(b'#') if bytes.get(at + 1) == Some(&b'[') => {
                at = crate::text::bracket_end(src, at + 1);
            }
            _ => return at,
        }
    }
}

/// The identifier starting at `at`, if any: `(identifier, end)`.
fn ident_at(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    let is_head = |b: u8| b.is_ascii_alphabetic() || b == b'_';
    let is_tail = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    if !bytes.get(at).is_some_and(|&b| is_head(b)) {
        return None;
    }
    let mut end = at + 1;
    while bytes.get(end).is_some_and(|&b| is_tail(b)) {
        end += 1;
    }
    Some((String::from_utf8_lossy(&bytes[at..end]).into_owned(), end))
}

/// The position just past the `)` closing the `(` at `open`, over raw
/// bytes (visibility bodies carry no strings).
fn paren_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i64;
    let mut at = open;
    while at < bytes.len() {
        match bytes[at] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// The position just past the `}` closing the `{` at `open`, skipping
/// string, char, and comment regions — a quote or a `/*` inside the
/// module body cannot shift the balance. `None` when it never closes.
fn brace_end(src: &str, regs: &[Region], open: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0i64;
    let mut at = open;
    while at < bytes.len() {
        if let Some(end) = region_at(regs, at) {
            at = end;
            continue;
        }
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
        at = next_char_end(bytes, at);
    }
    None
}

/// The end of the region containing `at`, when `at` starts one.
fn region_at(regs: &[Region], at: usize) -> Option<usize> {
    regs.iter()
        .find(|region| region.span().contains(&at))
        .map(|region| region.span().end)
}

/// Whether `at` sits inside any region.
fn in_region(regs: &[Region], at: usize) -> bool {
    regs.iter().any(|region| region.span().contains(&at))
}

/// Just past the whole UTF-8 character at `at`.
fn next_char_end(bytes: &[u8], at: usize) -> usize {
    let mut end = at + 1;
    while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
        end += 1;
    }
    end
}

fn find_sub(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&at| &haystack[at..at + needle.len()] == needle)
}

/// Blank a byte span of the masked text, preserving newlines so line
/// numbers computed over the masked text match the file.
fn blank(masked: &mut [u8], span: std::ops::Range<usize>) {
    for byte in &mut masked[span] {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

/// The surface tokens present in production text (case-insensitive).
fn surface_tokens_of(masked: &str) -> Vec<&'static str> {
    let lowered = masked.to_ascii_lowercase();
    ANSWER_KEY_SURFACE_TOKENS
        .iter()
        .copied()
        .filter(|token| lowered.contains(token))
        .collect()
}

/// The write-API tokens present in production text, with 1-based lines.
/// Identifier boundaries on both sides: `fs::symlink` does not match
/// `fs::symlink_metadata`, and a token embedded in a longer identifier
/// (`xfs::write`) is not the std API.
fn write_tokens_of(masked: &str) -> Vec<(usize, &'static str)> {
    let bytes = masked.as_bytes();
    let mut out = Vec::new();
    for token in WRITE_API_TOKENS {
        let needle = token.as_bytes();
        let mut from = 0usize;
        while let Some(at) = find_sub(bytes, needle, from) {
            from = at + 1;
            let boundary = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            let before_ok = at == 0 || !boundary(bytes[at - 1]);
            let after = at + needle.len();
            let after_ok = after >= bytes.len() || !boundary(bytes[after]);
            if before_ok && after_ok {
                out.push((1 + masked[..at].matches('\n').count(), *token));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ------------------------------------------------------------------
    // The pin tests: growth of any inventory is a deliberate, recorded
    // edit that updates these assertions.
    // ------------------------------------------------------------------

    #[test]
    fn the_pinned_inventories_are_pinned() {
        assert_eq!(
            ANSWER_KEY_SURFACE_TOKENS,
            &["wayfinder", "answerkeysource", "answer_key_name"]
        );
        assert_eq!(
            ANSWER_KEY_SURFACE_FILES
                .iter()
                .map(|(file, _)| *file)
                .collect::<Vec<_>>(),
            &[
                "crates/gauntlet-app/src/config/defaults.rs",
                "crates/gauntlet-app/src/control/controller.rs",
                "crates/gauntlet-app/src/reference/mod.rs",
                "crates/gauntlet-app/src/reference/parse.rs",
                "crates/gauntlet-app/src/reference/manifest.rs",
                "crates/gauntlet-app/src/reference/provider.rs",
                "crates/gauntlet-cli/src/compose.rs",
                "crates/gauntlet-cli/src/init_cmd.rs",
                "crates/gauntlet-cli/src/reference_cmd.rs",
                "crates/gauntlet-app/src/skills/mod.rs",
            ]
        );
        let inventory: Vec<(&str, &str, Option<&str>)> = WRITE_API_INVENTORY
            .iter()
            .map(|site| (site.file, site.token, site.guard))
            .collect();
        assert_eq!(
            inventory,
            &[
                (
                    "crates/gauntlet-cli/src/compose.rs",
                    "fs::create_dir_all",
                    Some("fn ensure_configured_dir"),
                ),
                (
                    "crates/gauntlet-cli/src/init_cmd.rs",
                    "fs::create_dir_all",
                    Some("refusing to overwrite"),
                ),
                (
                    "crates/gauntlet-cli/src/init_cmd.rs",
                    "fs::write",
                    Some("refusing to overwrite"),
                ),
                (
                    "crates/gauntlet-cli/src/skills_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn installed_root"),
                ),
                (
                    "crates/gauntlet-cli/src/skills_cmd.rs",
                    "fs::write",
                    Some("fn installed_root"),
                ),
                (
                    "crates/gauntlet-cli/src/state_cmd.rs",
                    "fs::write",
                    Some("--out"),
                ),
                (
                    "crates/gauntlet-app/src/control/controller.rs",
                    "fs::create_dir_all",
                    Some("fn contained_locator_path"),
                ),
                (
                    "crates/gauntlet-app/src/control/controller.rs",
                    "fs::write",
                    Some("fn contained_locator_path"),
                ),
                (
                    "crates/gauntlet-app/src/doctor/mod.rs",
                    "fs::create_dir_all",
                    Some("fn write_report"),
                ),
                (
                    "crates/gauntlet-app/src/doctor/mod.rs",
                    "fs::write",
                    Some("fn write_report"),
                ),
                (
                    "crates/gauntlet-app/src/doctor/checks.rs",
                    "fs::create_dir_all",
                    Some("fn probe_writable"),
                ),
                (
                    "crates/gauntlet-app/src/doctor/checks.rs",
                    "fs::write",
                    Some("fn probe_writable"),
                ),
                (
                    "crates/gauntlet-app/src/doctor/checks.rs",
                    "fs::remove_file",
                    Some("fn probe_writable"),
                ),
                (
                    "crates/gauntlet-app/src/roles/install.rs",
                    "fs::create_dir_all",
                    Some("fn ensure_within_root"),
                ),
                (
                    "crates/gauntlet-app/src/roles/install.rs",
                    "fs::write",
                    Some("fn ensure_within_root"),
                ),
                (
                    "crates/gauntlet-app/src/roles/install.rs",
                    "fs::rename",
                    Some("fn ensure_within_root"),
                ),
                // COR-R3-3 (falsifier round 3): the state-dir single-writer
                // guard — the O_EXCL admission, the state-dir creation it
                // sits behind, and the holder's release-on-drop.
                (
                    "crates/gauntlet-cli/src/writer_lock.rs",
                    "fs::create_dir_all",
                    Some("fn acquire"),
                ),
                (
                    "crates/gauntlet-cli/src/writer_lock.rs",
                    "OpenOptions",
                    Some("fn acquire"),
                ),
                (
                    "crates/gauntlet-cli/src/writer_lock.rs",
                    "fs::remove_file",
                    Some("impl Drop for WriterLock"),
                ),
                // Task 26, COR-3: the `improve baseline` harness — one row
                // per production write site, same order as the inventory.
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn establish_leg_environment"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn establish_leg_environment"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn establish_leg_environment"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn establish_leg_environment"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn baseline"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn baseline"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("impl Drop for ScratchGuard"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "File::create",
                    Some("fn write_report_atomically"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::rename",
                    Some("fn write_report_atomically"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn write_leg_files"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn write_leg_files"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn materialize_workspace"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn materialize_workspace"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn run_full_loop"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn run_full_loop"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::write",
                    Some("fn run_full_loop"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::copy",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::remove_dir_all",
                    Some("fn judge_snapshots"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::create_dir_all",
                    Some("fn copy_tree"),
                ),
                (
                    "crates/gauntlet-cli/src/improve_cmd.rs",
                    "fs::copy",
                    Some("fn copy_tree"),
                ),
            ]
        );
    }

    // ------------------------------------------------------------------
    // The production-text derivation.
    // ------------------------------------------------------------------

    #[test]
    fn writes_inside_a_gated_module_are_not_production() {
        let src = "pub fn real() {}\n#[cfg(test)]\nmod tests {\n    fn fixture() {\n        \
                   std::fs::write(\"f\", b\"x\").unwrap();\n    }\n}\n";
        let (masked, declared) = derive_production_text(src).expect("derives");
        assert!(declared.is_empty());
        assert!(
            write_tokens_of(&masked).is_empty(),
            "the fixture write is test-only: {masked:?}"
        );
        assert!(masked.contains("pub fn real()"), "{masked:?}");
        // Newlines survive so line numbers stay true.
        assert_eq!(masked.matches('\n').count(), src.matches('\n').count());
    }

    #[test]
    fn production_after_a_mid_file_test_module_stays_production() {
        // The verify/mod.rs shape: a gated module in the middle, real code
        // after it. Splitting at the first #[cfg(test)] — the naive
        // production derivation — would hide the tail.
        let src = "pub fn before() {}\n#[cfg(test)]\nmod pins {\n    fn f() {\n        \
                   fs::write(1);\n    }\n}\npub fn after() {\n    std::fs::write(\"out\", \
                   b\"x\");\n}\n";
        let (masked, _) = derive_production_text(src).expect("derives");
        let hits = write_tokens_of(&masked);
        assert_eq!(hits.len(), 1, "{hits:?} in {masked:?}");
        assert_eq!(hits[0].0, 9, "the tail write reports at its true line");
    }

    #[test]
    fn a_cfg_test_inside_a_string_is_not_an_attribute() {
        // values.rs/run.rs carry this shape inside their test modules; a
        // production doc carrying it must not blank anything after it.
        let src = "pub const NOTE: &str = \"#[cfg(test)]\";\npub fn real() {\n    \
                   std::fs::write(\"out\", b\"x\");\n}\n";
        let (masked, declared) = derive_production_text(src).expect("derives");
        assert!(declared.is_empty());
        assert!(
            masked.contains("#[cfg(test)]"),
            "the string survives: {masked:?}"
        );
        assert_eq!(write_tokens_of(&masked).len(), 1);
    }

    #[test]
    fn semicolon_gated_modules_are_reported_for_cross_file_exclusion() {
        let src = "pub mod real;\n#[cfg(test)]\npub(crate) mod stubs;\n#[cfg(test)]\nmod \
                   suite_runs;\n";
        let (masked, declared) = derive_production_text(src).expect("derives");
        assert_eq!(declared, &["stubs".to_string(), "suite_runs".to_string()]);
        assert!(masked.contains("pub mod real;"), "{masked:?}");
    }

    #[test]
    fn unclassifiable_shapes_fail_closed() {
        // A gated function is classified and blanked through its body —
        // the config/mod.rs fixture-path shape.
        let src =
            "#[cfg(test)]\npub(crate) fn fixture() {\n    std::fs::write(\"f\", b\"x\");\n}\n";
        let (masked, declared) = derive_production_text(src).expect("derives");
        assert!(declared.is_empty());
        assert!(
            write_tokens_of(&masked).is_empty(),
            "the gated fn's write is test-only: {masked:?}"
        );
        // cfg(test) on anything the blanker has not classified.
        let src = "#[cfg(test)]\nuse std::fs;\n";
        assert!(derive_production_text(src).is_err());
        // A gated module whose brace never closes.
        let src = "#[cfg(test)]\nmod tests {\n    fn f() {}\n";
        assert!(derive_production_text(src).is_err());
        // A gated function whose body never closes.
        let src = "#[cfg(test)]\nfn helper() {\n    let x = 1;\n";
        assert!(derive_production_text(src).is_err());
    }

    #[test]
    fn token_boundaries_distinguish_the_read_spellings() {
        let masked = "let m = std::fs::symlink_metadata(p);\nlet w = \
                      myfs::write(a);\nstd::fs::create_dir_all(d);\nstd::fs::create_dir(d);\nlet \
                      f = std::fs::File::create(p);\n";
        let tokens: Vec<&str> = write_tokens_of(masked)
            .into_iter()
            .map(|(_, token)| token)
            .collect();
        assert_eq!(
            tokens,
            &["fs::create_dir_all", "fs::create_dir", "File::create"],
            "symlink_metadata is a read; myfs::write is not the std API; hits report in \
             line order (create_dir_all sits on the earlier line)"
        );
    }

    #[test]
    fn surface_tokens_match_case_insensitively() {
        let masked = "pub const WAYFINDER_DIR_NAME: &str = \".wayfinder\";\nuse \
                      AnswerKeySource;\nconst N: &str = ANSWER_KEY_NAME;\n";
        assert_eq!(
            surface_tokens_of(masked),
            &["wayfinder", "answerkeysource", "answer_key_name"]
        );
    }

    // ------------------------------------------------------------------
    // The masked-spelling battery (prereg P5): every case runs against
    // the real check over a synthetic workspace, and the assertions pin
    // what the rule CAUGHT and what ESCAPED — exactly what the module
    // docs record. Never claim more.
    // ------------------------------------------------------------------

    struct Repo {
        dir: PathBuf,
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    impl Repo {
        /// A minimal four-core-crate workspace reproducing the whole
        /// pinned state in miniature — the seven surface files, the seven
        /// inventoried write tokens with their guards — so the battery's
        /// assertions isolate the payload under test and the clean
        /// fixture itself proves the pins are satisfiable.
        fn clean(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "gauntlet-arch-tests-single-writer-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            let repo = Repo { dir };
            for dir_name in [
                "crates/gauntlet-domain",
                "crates/gauntlet-ports",
                "crates/gauntlet-app",
                "crates/gauntlet-cli",
            ] {
                repo.write(
                    &format!("{dir_name}/Cargo.toml"),
                    "[package]\nname = \"x\"\nedition = \"2024\"\n",
                );
                repo.write(&format!("{dir_name}/src/lib.rs"), "//! Layer docs.\n");
            }
            repo.write(
                "crates/gauntlet-app/src/config/defaults.rs",
                "pub const WAYFINDER_DIR_NAME: &str = \".wayfinder\";\n",
            );
            repo.write(
                "crates/gauntlet-app/src/control/controller.rs",
                "use crate::reference::provider::AnswerKeySource;\nfn \
                 contained_locator_path(root: &std::path::Path, rel: &str) -> \
                 std::path::PathBuf { root.join(rel) }\npub fn materialize(root: \
                 &std::path::Path) {\n    let target = contained_locator_path(root, \
                 \"out/facet.json\");\n    if let Some(parent) = target.parent() {\n        \
                 std::fs::create_dir_all(parent);\n    }\n    std::fs::write(target, \
                 b\"bytes\");\n}\n",
            );
            repo.write(
                "crates/gauntlet-app/src/doctor/mod.rs",
                "fn write_report(dir: &std::path::Path) {\n    std::fs::create_dir_all(dir);\n    \
                 std::fs::write(dir.join(\"report.json\"), b\"{}\");\n}\n",
            );
            repo.write(
                "crates/gauntlet-app/src/doctor/checks.rs",
                "fn probe_writable(dir: &std::path::Path) {\n    std::fs::create_dir_all(dir);\n    \
                 let scratch = dir.join(\"probe\");\n    std::fs::write(scratch, \
                 b\"probe\");\n    std::fs::remove_file(scratch);\n}\n",
            );
            repo.write(
                "crates/gauntlet-app/src/roles/install.rs",
                "fn ensure_within_root(child: &std::path::Path, root: &std::path::Path) -> bool \
                 {\n    child.starts_with(root)\n}\npub fn install(root: &std::path::Path) {\n    \
                 if ensure_within_root(root, root) {\n        std::fs::create_dir_all(root);\n    \
                 \n        std::fs::write(root.join(\"role.md\"), b\"bytes\");\n        \
                 std::fs::rename(root.join(\"staging\"), root.join(\"role.md\"));\n    }\n}\n",
            );
            repo.write(
                "crates/gauntlet-app/src/reference/mod.rs",
                "//! .wayfinder\n",
            );
            repo.write(
                "crates/gauntlet-app/src/reference/parse.rs",
                "//! .wayfinder\n",
            );
            repo.write(
                "crates/gauntlet-app/src/reference/manifest.rs",
                "//! .wayfinder\n",
            );
            repo.write(
                "crates/gauntlet-app/src/reference/provider.rs",
                "pub enum AnswerKeySource { Path, Shared }\n",
            );
            repo.write(
                "crates/gauntlet-app/src/skills/mod.rs",
                "pub const REPO_OWNED_SKILLS: &[&str] = &[\"wayfinder-gauntlet\"];\n",
            );
            repo.write(
                "crates/gauntlet-cli/src/compose.rs",
                "use gauntlet_app::reference::provider::AnswerKeySource;\n\
                 fn ensure_configured_dir(path: &std::path::Path) {\n    \
                 std::fs::create_dir_all(path);\n}\n",
            );
            repo.write(
                "crates/gauntlet-cli/src/init_cmd.rs",
                "fn scaffold() {\n    // the wayfinder example stays inert; refusing to \
                 overwrite\n    std::fs::create_dir_all(\"run\");\n    \
                 std::fs::write(\"GAUNTLET.md\", b\"x\");\n}\n",
            );
            repo.write(
                "crates/gauntlet-cli/src/reference_cmd.rs",
                "//! .wayfinder\n",
            );
            repo.write(
                "crates/gauntlet-cli/src/skills_cmd.rs",
                "fn installed_root() -> std::path::PathBuf { unimplemented!() }\n\
                 fn install() {\n    std::fs::create_dir_all(installed_root());\n    \
                 std::fs::write(installed_root().join(\"s\"), b\"x\");\n}\n",
            );
            repo.write(
                "crates/gauntlet-cli/src/state_cmd.rs",
                "fn export(out: Option<&std::path::Path>, flag: &str) {\n    if flag == \
                 \"--out\" {\n        if let Some(path) = out {\n            \
                 std::fs::write(path, b\"x\");\n        }\n    }\n}\n",
            );
            // COR-R3-3: the state-dir single-writer guard's inventoried
            // writes — the O_EXCL admission and its release-on-drop.
            repo.write(
                "crates/gauntlet-cli/src/writer_lock.rs",
                "pub struct WriterLock { path: std::path::PathBuf }\n\
                 impl Drop for WriterLock {\n    fn drop(&mut self) {\n        let _ = \
                 std::fs::remove_file(&self.path);\n    }\n}\n\
                 pub fn acquire(state_dir: &std::path::Path) -> std::io::Result<Self> {\n    \
                 let lock_path = state_dir.join(\"writer.lock\");\n    \
                 if let Some(parent) = lock_path.parent() {\n        \
                 std::fs::create_dir_all(parent)?;\n    }\n    \
                 let file = std::fs::OpenOptions::new().write(true).create_new(true).open(\
                 &lock_path)?;\n    Ok(Self { path: lock_path })\n}\n",
            );
            // Task 26, COR-3: the `improve baseline` harness's inventoried
            // writes — one stub call per pinned site, each inside the
            // owning function the inventory pins as its guard.
            repo.write(
                "crates/gauntlet-cli/src/improve_cmd.rs",
                "fn establish_leg_environment() {\n    std::fs::create_dir_all(\"d\");\n    \
                 std::fs::write(\"wrapper\", b\"x\");\n    std::fs::write(\"shim\", \
                 b\"x\");\n    let _ = std::fs::remove_dir_all(\"d\");\n}\n\
                 fn baseline() {\n    std::fs::create_dir_all(\"legs\");\n    \
                 std::fs::create_dir_all(\"reports\");\n}\n\
                 struct ScratchGuard { path: std::path::PathBuf }\n\
                 impl Drop for ScratchGuard {\n    fn drop(&mut self) {\n        let _ = \
                 std::fs::remove_dir_all(&self.path);\n    }\n}\n\
                 fn write_report_atomically() {\n    let staging = \
                 std::fs::File::create(\"s\");\n    std::fs::rename(\"s\", \"out\");\n    \
                 let _ = staging;\n}\n\
                 fn write_leg_files() {\n    std::fs::write(\"mode\", b\"x\");\n    \
                 std::fs::write(\"child\", b\"x\");\n}\n\
                 fn materialize_workspace() {\n    let _ = \
                 std::fs::remove_dir_all(\"leg\");\n    \
                 std::fs::create_dir_all(\"ws\");\n}\n\
                 fn run_full_loop() {\n    std::fs::create_dir_all(\"snapshots\");\n    \
                 std::fs::write(\"wrapper\", b\"x\");\n    std::fs::write(\"pointer\", \
                 b\"x\");\n}\n\
                 fn judge_snapshots() {\n    let _ = std::fs::remove_dir_all(\"t\");\n    \
                 std::fs::create_dir_all(\"t\");\n    let _ = \
                 std::fs::remove_dir_all(\"t\");\n    std::fs::copy(\"a\", \"t/b\");\n    \
                 let _ = std::fs::remove_dir_all(\"t\");\n}\n\
                 fn copy_tree() {\n    std::fs::create_dir_all(\"to\");\n    \
                 std::fs::copy(\"from\", \"to/f\");\n}\n",
            );
            repo
        }

        fn write(&self, rel: &str, contents: &str) {
            let path = self.dir.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent directory"))
                .expect("fixture directory is creatable");
            std::fs::write(&path, contents).expect("fixture file is writable");
        }

        fn root(&self) -> &std::path::Path {
            &self.dir
        }
    }

    #[test]
    fn the_clean_fixture_passes_the_check() {
        let repo = Repo::clean("baseline");
        let violations = check(repo.root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// P5 battery 1 — env-injected path: the path's provenance is
    /// invisible, but the write-API token still fires Rule C. CAUGHT AT
    /// THE TOKEN.
    #[test]
    fn an_env_injected_path_still_fires_on_the_write_token() {
        let repo = Repo::clean("envpath");
        repo.write(
            "crates/gauntlet-app/src/state/materialize.rs",
            "pub fn load() {\n    let p: std::path::PathBuf = \
             std::env::var(\"KEY_PATH\").unwrap().into();\n    std::fs::write(p, \
             b\"bytes\");\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("new production write-API token 'fs::write'")
                    && v.contains("state/materialize.rs")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
        assert!(
            !violations
                .iter()
                .any(|v| v.contains("names the answer-key surface")
                    && v.contains("state/materialize.rs")),
            "the env-injected file is not a surface module — Rule A is silent on it"
        );
    }

    /// P5 battery 2 — format!-constructed surface mention: no literal
    /// token, so Rule A does not see the file. GENUINE RULE A ESCAPE
    /// (recorded in the module docs); any write in it still fires Rule C.
    #[test]
    fn a_format_constructed_surface_mention_escapes_rule_a() {
        let repo = Repo::clean("formatsplit");
        repo.write(
            "crates/gauntlet-app/src/state/hidden.rs",
            "pub fn path() -> String {\n    format!(\".way{}inder/answer-key.md\", \"f\")\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            !violations.iter().any(|v| v.contains("state/hidden.rs")),
            "Rule A did NOT see the format!-constructed mention — the escape is pinned as \
             observed, not claimed away: {}",
            crate::summarize("single-writer", &violations)
        );
        // The same file carrying a write still fires Rule C: only the
        // surface half is blind, not the write half.
        repo.write(
            "crates/gauntlet-app/src/state/hidden.rs",
            "pub fn path() -> String {\n    format!(\".way{}inder/answer-key.md\", \"f\")\n}\n\
             pub fn save() {\n    std::fs::write(path(), b\"x\");\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("new production write-API token") && v.contains("hidden.rs")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// P5 battery 3 — generic helper funnel: the surface file stays
    /// write-token-free, and the helper's token fires Rule C wherever it
    /// lives in a core crate. CAUGHT AT THE TOKEN.
    #[test]
    fn a_generic_helper_funnel_fires_on_the_helpers_token() {
        let repo = Repo::clean("funnel");
        repo.write(
            "crates/gauntlet-app/src/state/helper.rs",
            "pub fn materialize(p: impl AsRef<std::path::Path>) {\n    \
             std::fs::write(p, b\"bytes\");\n}\n",
        );
        repo.write(
            "crates/gauntlet-app/src/reference/provider.rs",
            "pub enum AnswerKeySource { Path, Shared }\npub fn snapshot(source: &AnswerKeySource, \
             key: &std::path::Path) {\n    crate::state::helper::materialize(key);\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("new production write-API token 'fs::write'")
                    && v.contains("state/helper.rs")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
        assert!(
            !violations
                .iter()
                .any(|v| v.contains("uses write-API token") && v.contains("reference/provider.rs")),
            "the surface file itself carries no token — Rule B is silent on it; the denial is at \
             the helper's token"
        );
    }

    /// P5 battery 4 — trait-object indirection with the implementation
    /// OUTSIDE the core crates: no write-API token exists in any core
    /// crate, so nothing fires. GENUINE ESCAPE — pinned here as observed;
    /// the authority/containment layers and the g10 proof (B4) own that
    /// channel.
    #[test]
    fn trait_object_indirection_from_an_adapter_escapes_entirely() {
        let repo = Repo::clean("dynwriter");
        repo.write(
            "crates/gauntlet-app/src/reference/provider.rs",
            "pub enum AnswerKeySource { Path, Shared }\npub trait ResultWriter {\n    fn \
             put(&self, path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()>;\n}\npub fn \
             snapshot(source: &AnswerKeySource, key: &std::path::Path, writer: &dyn \
             ResultWriter) -> std::io::Result<()> {\n    writer.put(key, b\"bytes\")\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations.is_empty(),
            "no core crate carries a write-API token for the dyn call — the escape is pinned as \
             observed: {}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// Rule B's own tooth: an unwarranted write token in a PINNED
    /// surface module — no inventory entry, no guard.
    #[test]
    fn an_unwarranted_write_in_a_surface_module_has_no_escape() {
        let repo = Repo::clean("surfacewrite");
        repo.write(
            "crates/gauntlet-app/src/reference/provider.rs",
            "pub enum AnswerKeySource { Path, Shared }\npub fn save() {\n    \
             std::fs::write(\"key\", b\"\");\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("uses write-API token 'fs::write'")
                    && v.contains("reference/provider.rs")
                    && v.contains("no unguarded write at the surface")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// Rule A's own tooth: a NEW surface file must be inventoried.
    #[test]
    fn a_new_surface_file_fails_the_inventory() {
        let repo = Repo::clean("newsurface");
        repo.write(
            "crates/gauntlet-app/src/reference/extra.rs",
            "pub fn key() -> &'static str { \".wayfinder/answer-key.md\" }\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("names the answer-key surface")
                    && v.contains("reference/extra.rs")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// The guard clause's own tooth: an inventoried write whose
    /// containment surface is absent from the file fails — the AK-043
    /// prevention leg (b) arch-level pin.
    #[test]
    fn a_vanished_guard_fails_the_guard_clause() {
        // Reproduce the real inventory's file pair with the write present
        // and the guard absent, by pointing the inventory's expectations
        // at a fixture through the real check: the controller file name
        // is pinned, so build the fixture at that path with a write and
        // no contained_locator_path.
        let repo = Repo::clean("guardgone");
        repo.write(
            "crates/gauntlet-app/src/control/controller.rs",
            "use crate::reference::provider::AnswerKeySource;\npub fn save() {\n    \
             std::fs::create_dir_all(\"d\");\n    std::fs::write(\"out\", b\"x\");\n}\n",
        );
        let violations = check(repo.root());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("inventories a guarded write")
                    && v.contains("contained_locator_path")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }

    /// Rule C's stale direction: removing a pinned write fails as a
    /// stale inventory entry.
    #[test]
    fn a_removed_pinned_write_fails_as_stale() {
        let repo = Repo::clean("stale");
        // The miniature controller carried two pinned writes; replace it
        // with a write-free surface holder and both pins go stale.
        repo.write(
            "crates/gauntlet-app/src/control/controller.rs",
            "use crate::reference::provider::AnswerKeySource;\n",
        );
        let violations = check(repo.root());
        assert!(
            violations.iter().any(|v| v.contains("'fs::write'")
                && v.contains("controller.rs")
                && v.contains("stale inventory")),
            "{}",
            crate::summarize("single-writer", &violations)
        );
    }
}
