//! Repo-tree walking and scope decisions (plan Task 2, recritique class A).
//!
//! The governing rule of this module is **what gets scanned must equal what
//! compiles**. Every earlier hole in it was a place where the walk trusted a
//! declaration instead of the filesystem:
//!
//! - a symlink is neither `is_dir()` nor `is_file()`, so it vanished from the
//!   walk while `rustc` followed it (R3/R17/R25) — it is now a violation;
//! - a file that could not be read, was not UTF-8, or was over the size
//!   ceiling was silently skipped (R5/R26) — it is now a violation, because
//!   missing evidence is not passing evidence (I8);
//! - the scope was four hardcoded prefixes, so `#[path]`/`include!` targets
//!   escaped it (R4/R25) — see `crate::escapes`.

use std::path::{Path, PathBuf};

/// The four core crates whose sources are policed (spec §24.3): everything
/// provider-, harness-, database-, browser-, or sandbox-specific belongs
/// under `crates/adapters/**` or in configuration — never here.
pub const CORE_CRATES: &[&str] = &[
    "crates/gauntlet-domain",
    "crates/gauntlet-ports",
    "crates/gauntlet-app",
    "crates/gauntlet-cli",
];

/// The inner layers — domain, ports, application. Spec §24.3's second bullet
/// and AK-002/AK-014 scope the *adapter identifier* ban to "domain or
/// application source", so this, not [`CORE_CRATES`], is the scope of the
/// derived adapter-name denylist. See [`COMPOSITION_ROOT`].
pub const INNER_CRATES: &[&str] = &[
    "crates/gauntlet-domain",
    "crates/gauntlet-ports",
    "crates/gauntlet-app",
];

/// The composition root (spec §4.1: "cli wires everything"). It is a core
/// crate for the vendor denylist and for the hardcoded-literal scan, but it
/// is deliberately OUTSIDE the adapter-identifier scope: naming and declaring
/// adapter crates is precisely its job. Getting this wrong in either
/// direction is a real failure mode — too strict and Task 20 cannot land
/// without weakening the gate; too loose and the composition root becomes an
/// unpoliced region.
pub const COMPOSITION_ROOT: &str = "crates/gauntlet-cli";

/// The declared defaults module (plan Task 5, spec §8.6): the ONE file where
/// conservative default VALUES are permitted to be literals — and since round
/// 6 (critic-7 C1) the exemption is not file-scoped: the file
/// is inside the literal scan like every other core source, and only the
/// declarations of [`DEFAULTS_TABLE_SETTINGS`] are exempt. Since round 6.1
/// (critic-9 F2 / critic-8 E7) the exemption is DECLARATION-granular: each
/// setting's own declaration — head to its terminating `;`, computed from
/// the one lexing pass — is blanked out before matching, because a scan unit
/// can hold more than one declaration and a second one packed onto the
/// setting's line used to escape every class (see
/// `literals::mask_setting_declarations`). A whole file outside
/// the literal scan made eight of the nine §8.1 classes hardcodable by
/// anything that landed at this path; a declaration-granular exemption cannot
/// outgrow its warrant. The vendor denylist applies to the whole file either
/// way, and `crate::check_defaults_exemption` still requires the file, if it
/// exists, to DECLARE each §8.6 setting.
pub const DEFAULTS_MODULE: &str = "crates/gauntlet-app/src/config/defaults.rs";

/// Every file any repo-level scan may be told to exempt, with the reason.
///
/// The scope predicates were pinned and the walker was pinned, but the third
/// input to a scan — the exemption list its CALLER passes — was trusted. An
/// added entry looked exactly like the sanctioned pattern and silently removed
/// a file from a check. `crate::scan_core_tree` now refuses any exemption that
/// is not on this list, so widening the exemptions means editing this
/// inventory, in the open, with a reason.
///
/// EMPTY since round 6: the one entry this list carried (the §8.6 defaults
/// module) became setting-scoped inside the literal scan (critic-7 C1), so no
/// whole file is exempt from any scan any more. The inventory and the guard
/// stay: a future file-level exemption must be declared here, with a reason,
/// before any call site may pass it — and today every path is refused.
pub const DECLARED_SCAN_EXEMPTIONS: &[(&str, &str)] = &[];

/// The settings of the spec §8.6 defaults table — all ten rows, the dotted
/// names as snake_case identifiers. Two jobs, deliberately the same list:
/// the setting-scoped literal exemption (above) skips exactly the
/// declarations of these settings, and `crate::check_defaults_exemption`
/// requires the defaults module, if it exists, to declare each of them.
/// Until round 6 only the six numeric rows were listed (critic-7 m4), which
/// mattered the moment the exemption became setting-scoped: `review_mode`'s
/// default `adaptive` is tier vocabulary and would have failed its own
/// defaults table.
pub const DEFAULTS_TABLE_SETTINGS: &[&str] = &[
    "max_rounds",
    "max_parallel",
    "max_depth",
    "max_wall_seconds",
    "recovery_reserve_pct",
    "diagnostic_budget",
    "review_mode",
    "sandbox_backend",
    "interactive",
    "git_strategy",
];

/// The ONE file where Annex-B wire vocabulary is parsed: the contract
/// envelope/payload decoder in `gauntlet-ports`. Its `from_json` paths MUST
/// match the exact JSON keys of the annex — that is schema conformance, not
/// adapter selection — so the segment-whole-literal rule (AK-014) masks the
/// declared wire keys there, in key-ACCESS position only. Everything else
/// this module contains is policed exactly as before.
pub const ANNEX_B_CONTRACT_PARSER: &str = "crates/gauntlet-ports/src/role_result.rs";

/// The declared field-key vocabulary of **Annex B.2** (`builder_result@v1`),
/// each key paired with its spec field reference:
///
/// ```json
/// { "attempted": ["AK-014"], "changed_paths": [], "commands_run": [],
///   "artifacts": [{"path": "", "evidence_ref": ""}],
///   "claims": ["free text — stored for forensics, excluded from verification"],
///   "self_checks": [{"command": "", "exit_code": 0}],
///   "blocked_on": [] }
/// ```
///
/// A string literal equal to one of these keys, inside
/// [`ANNEX_B_CONTRACT_PARSER`], at a key-access position (a `.get("k")`
/// argument or an `&[…"k"…]` allowlist array), is the parser MATCHING the
/// wire schema — never an adapter selection, and therefore not an AK-014
/// violation. The mask has three independent gates (module, declared key,
/// position), which is the shape the adversarial corner forces:
///
/// - **module**: the mask applies to this one parser module only — the same
///   literal in any other core file still reports;
/// - **declared key**: a literal equal to an adapter segment that is NOT an
///   Annex-B.2 key (`"prime"`, `"mock"`, `"git"`) reports even at a key-access
///   position in this module, so a predicate call
///   (`object.get("prime")`-shaped) cannot launder a selection literal here;
/// - **position**: the key must sit in a key-access spelling — a
///   selection-shaped `"command"` (`match role { "command" => … }`, a `let`,
///   a const) in this module reports exactly as before, so an attacker cannot
///   name an adapter segment equal to a wire key to hide selection literals
///   inside the parser.
///
/// Residual, recorded rather than hidden: the position gate is a byte-level
/// shape, not a parse tree. An array-element spelling is masked whatever the
/// array is actually for — the module's only `&["command", …]`-shaped use is
/// the unknown-field allowlist, and a second, unrelated array of the same
/// shape would be masked too; likewise a `.get` receiver that is not the JSON
/// object (`commands.get("command")`) is masked, because the shape cannot see
/// the receiver — the same receiver invisibility masks a module-path callee
/// form (`helpers::get("command")`, `json::get("command")`): the shape only
/// checks the final callee identifier ended by `(`. An escaped or padded
/// spelling that decodes to a declared key (`"com\u{6d}and"`, `"command "`,
/// `" command"`) is masked at a key-access position — the parser sees the same
/// decoded key value (segment literals are trimmed at both ends). What still
/// reports in this module:
/// `get ("command")` (a space before the parens), `budget("command")` /
/// `getter("command")` (a `get` embedded in a longer identifier), and every
/// non-key-access position — match arms, `let`, const, non-call arguments.
/// The set is pinned by test in [`scope_tests`], so growth is a deliberate
/// edit that updates the pin — exactly like [`DEFAULTS_TABLE_SETTINGS`].
pub const ANNEX_B_WIRE_KEYS: &[(&str, &str)] = &[
    (
        "attempted",
        "Annex B.2 `builder_result@v1` top-level `attempted`",
    ),
    (
        "changed_paths",
        "Annex B.2 `builder_result@v1` top-level `changed_paths`",
    ),
    (
        "commands_run",
        "Annex B.2 `builder_result@v1` top-level `commands_run`",
    ),
    (
        "artifacts",
        "Annex B.2 `builder_result@v1` top-level `artifacts`",
    ),
    ("path", "Annex B.2 `artifacts[].path`"),
    ("evidence_ref", "Annex B.2 `artifacts[].evidence_ref`"),
    ("claims", "Annex B.2 `builder_result@v1` top-level `claims`"),
    (
        "self_checks",
        "Annex B.2 `builder_result@v1` top-level `self_checks`",
    ),
    ("command", "Annex B.2 `self_checks[].command`"),
    ("exit_code", "Annex B.2 `self_checks[].exit_code`"),
    (
        "blocked_on",
        "Annex B.2 `builder_result@v1` top-level `blocked_on`",
    ),
];

/// Upper bound for a scannable file. Beyond this the file is not skipped —
/// it is a violation (I8): a scanner that reports "clean" for bytes it never
/// read is reporting nothing.
pub const MAX_SCAN_BYTES: u64 = 1024 * 1024;

/// Resolve the repository root from this crate's manifest directory:
/// `<repo>/crates/gauntlet-arch-tests` two levels up.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// The result of walking the core crates: the regular files found, and the
/// violations the walk itself discovered (symlinks, unreadable directories).
#[derive(Default)]
pub struct CoreTree {
    /// Repo-relative POSIX paths of regular files, sorted.
    pub files: Vec<String>,
    /// Scope-integrity violations found while walking.
    pub violations: Vec<String>,
}

/// Walk the four core crates. Symlinks are NOT followed and NOT ignored:
/// each one is reported, because `rustc` follows them and the scanners
/// cannot (following them would also let a link escape the repo entirely).
pub fn core_tree(root: &Path) -> CoreTree {
    let mut tree = CoreTree::default();
    for dir in CORE_CRATES {
        collect_files(root, &root.join(dir), &mut tree);
    }
    tree.files.sort();
    tree.violations.sort();
    tree
}

fn collect_files(root: &Path, dir: &Path, tree: &mut CoreTree) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            if dir.exists() {
                tree.violations.push(format!(
                    "scope-integrity: {}: directory is not readable ({err}) — the scanners cannot \
                     see what the compiler compiles",
                    rel_of(root, dir)
                ));
            }
            return;
        }
    };
    let mut sorted: Vec<_> = entries.flatten().collect();
    sorted.sort_by_key(|entry| entry.file_name());
    for entry in sorted {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            tree.violations.push(format!(
                "scope-integrity: {}: file type is not readable — a file the scanners cannot \
                 classify is missing evidence, not a pass",
                rel_of(root, &path)
            ));
            continue;
        };
        if file_type.is_symlink() {
            let target = std::fs::read_link(&path)
                .map(|t| t.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| "<unreadable>".to_string());
            tree.violations.push(format!(
                "scope-integrity: {}: symlink to '{target}' inside a core crate — rustc follows \
                 symlinks and the boundary scanners do not, so the target would compile into the \
                 crate unscanned (spec §24.3)",
                rel_of(root, &path)
            ));
            continue;
        }
        if file_type.is_dir() {
            collect_files(root, &path, tree);
        } else if file_type.is_file() {
            tree.files.push(rel_of(root, &path));
        } else {
            tree.violations.push(format!(
                "scope-integrity: {}: not a regular file, a directory, or a symlink — the \
                 scanners cannot read it",
                rel_of(root, &path)
            ));
        }
    }
}

/// Every non-directory entry under `dirs`, listed by a recursion that shares
/// NO code with [`core_tree`] (critic-4 vacuity M1): the anti-vacuity guard
/// used to measure the walker with the walker's own output, so a carve-out
/// added to `collect_files` shrank both sides in lockstep and nothing failed.
/// This listing has no exclusion logic to shrink. Symlinks (to files or
/// directories) are recorded as entries and never followed — the walker
/// separately reports each one as a violation, so the guard can excuse
/// exactly those and still catch a silent carve-out.
///
/// A directory that does not exist yields no entries (the guard's per-crate
/// floors are what report a missing crate); a directory that exists but
/// cannot be read PANICS — the guard cannot verify coverage of a tree it
/// cannot enumerate, and failing hard is the fail-closed direction.
pub fn unfiltered_listing(root: &Path, dirs: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for dir in dirs {
        list_unfiltered(root, &root.join(dir), &mut out);
    }
    out.sort();
    out
}

fn list_unfiltered(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if !dir.exists() => return,
        Err(err) => panic!("{} is readable: {err}", rel_of(root, dir)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = match entry.file_type() {
            Ok(file_type) => file_type.is_dir(),
            Err(err) => panic!("{} has a readable type: {err}", rel_of(root, &path)),
        };
        if is_dir {
            list_unfiltered(root, &path, out);
        } else {
            out.push(rel_of(root, &path));
        }
    }
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Read one in-scope file as scannable text. `Err` is a VIOLATION message,
/// never a skip: unreadable, non-UTF-8, and oversized files all mean the
/// scanners did not see bytes the compiler will (recritique R5/R26, I8).
pub fn read_scannable(root: &Path, rel: &str) -> Result<String, String> {
    let path = root.join(rel);
    match std::fs::metadata(&path) {
        Ok(meta) if meta.len() > MAX_SCAN_BYTES => {
            return Err(format!(
                "scope-integrity: {rel}: {} bytes exceeds the {MAX_SCAN_BYTES}-byte scan ceiling \
                 — split it or move it out of a core crate; an unscanned file is missing \
                 evidence, not a pass",
                meta.len()
            ));
        }
        Ok(_) => {}
        Err(err) => {
            return Err(format!(
                "scope-integrity: {rel}: metadata is not readable ({err}) — missing evidence is \
                 not passing evidence"
            ));
        }
    }
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("scope-integrity: {rel}: not readable ({err})"))?;
    String::from_utf8(bytes).map_err(|_| {
        format!(
            "scope-integrity: {rel}: not valid UTF-8 — a binary blob inside a core crate cannot \
             be scanned; move it out of the core tree"
        )
    })
}

/// Denylist scope (mechanism b): every regular file under the core crates —
/// `.rs`, `Cargo.toml`, and any other text file — excluding nothing. The
/// defaults module is not exempt from the denylist, only from the literal
/// scan.
///
/// This predicate must stay exactly [`is_core_path`]. `crate::check_denylist`
/// asserts that equality over the whole walked tree, so an added exclusion
/// (`&& !rel.contains("/generated/")`) fails the gate instead of silently
/// shrinking it (recritique R30).
pub fn in_denylist_scope(rel: &str) -> bool {
    is_core_path(rel)
}

/// Hardcoded-literal scope (mechanism c): identical to the denylist scope
/// since round 6. The §8.6 defaults exemption used to remove the whole
/// declared defaults module from this scan; it is SETTING-scoped now —
/// applied inside `literals::scan_literals`, to the declared default-setting
/// declarations only (critic-7 C1) — so no file is outside the literal scan
/// and the two scopes must agree on every path. `crate::check_denylist`
/// asserts that equality over the walked tree. `crates/gauntlet-arch-tests`
/// is not a core crate, so this crate's own pattern strings are outside the
/// scope by construction, not by special case.
pub fn in_literal_scope(rel: &str) -> bool {
    in_denylist_scope(rel)
}

/// Adapter-identifier scope (AK-002, AK-014, spec §24.3 second bullet):
/// domain, ports, and application only. See [`COMPOSITION_ROOT`] for why the
/// cli is excluded — and note it stays in [`in_denylist_scope`] and
/// [`in_literal_scope`], so it is not an unpoliced region.
pub fn in_adapter_identifier_scope(rel: &str) -> bool {
    INNER_CRATES
        .iter()
        .any(|c| rel == *c || rel.starts_with(&format!("{c}/")))
}

pub fn is_core_path(rel: &str) -> bool {
    CORE_CRATES
        .iter()
        .any(|c| rel == *c || rel.starts_with(&format!("{c}/")))
}

/// The core crate directory a repo-relative path belongs to, if any.
pub fn core_crate_dir_of(rel: &str) -> Option<&'static str> {
    CORE_CRATES
        .iter()
        .find(|c| rel.starts_with(&format!("{c}/")))
        .copied()
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn scope_decisions_match_the_declared_exclusions() {
        // Core sources are policed.
        assert!(in_denylist_scope("crates/gauntlet-domain/src/lib.rs"));
        assert!(in_denylist_scope("crates/gauntlet-domain/src/deep/leaf.rs"));
        assert!(in_denylist_scope("crates/gauntlet-cli/Cargo.toml"));

        // Adapters and this test crate are never in scope: identifiers and
        // literals are legal there and in configuration.
        assert!(!in_denylist_scope(
            "crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs"
        ));
        assert!(!in_denylist_scope(
            "crates/adapters/gauntlet-adapter-compare-diff/Cargo.toml"
        ));
        assert!(!in_denylist_scope(
            "crates/gauntlet-arch-tests/src/denylist.rs"
        ));

        // Every regular file in a core crate is in both scopes, whatever its
        // extension — a vendor name or environment literal does not stop
        // being a leak by moving into a `.txt` or a `.json`.
        assert!(in_denylist_scope("crates/gauntlet-app/README.md"));
        assert!(in_denylist_scope("crates/gauntlet-app/fixtures/notes.txt"));
        assert!(in_literal_scope("crates/gauntlet-app/fixtures/notes.txt"));
        assert!(in_literal_scope("crates/gauntlet-cli/data/wiring.json"));

        // Recritique R30: no subtree may be carved out of the scan. A
        // `/generated/` (or any other) exclusion has to fail here.
        assert!(in_denylist_scope(
            "crates/gauntlet-app/src/generated/wiring.rs"
        ));
        assert!(in_literal_scope(
            "crates/gauntlet-app/src/generated/wiring.rs"
        ));
        assert!(in_denylist_scope("crates/gauntlet-app/target/build.rs"));
        assert!(in_denylist_scope("crates/gauntlet-app/vendor/store.rs"));

        // The defaults module is INSIDE the literal scan since round 6: the
        // §8.6 exemption is setting-scoped (applied inside the scanner, to
        // the declared default-setting declarations only), not file-scoped —
        // a file-level exemption made eight of the nine §8.1 literal classes
        // hardcodable by anything that landed at that path (critic-7 C1).
        assert!(in_denylist_scope(
            "crates/gauntlet-app/src/config/defaults.rs"
        ));
        assert!(in_literal_scope(
            "crates/gauntlet-app/src/config/defaults.rs"
        ));
        assert!(in_denylist_scope(
            "crates/gauntlet-app/src/config/parser.rs"
        ));
        assert!(in_literal_scope("crates/gauntlet-app/src/config/parser.rs"));
        assert!(in_literal_scope("crates/gauntlet-app/src/anything_else.rs"));
        assert!(!in_literal_scope(
            "crates/gauntlet-arch-tests/src/literals.rs"
        ));

        // The file-level exemption inventory is EMPTY (critic-7 m3 pinned the
        // count): the defaults entry became setting-scoped, and any future
        // file-level exemption is a deliberate edit here, in the open.
        assert!(
            DECLARED_SCAN_EXEMPTIONS.is_empty(),
            "no whole file may be exempt from a repo-level scan; the §8.6 exemption is \
             setting-scoped (critic-7 C1). Growth is a recorded decision: {:?}",
            DECLARED_SCAN_EXEMPTIONS
        );
        // And the §8.6 warrant vocabulary covers all ten rows of spec §8.6
        // (critic-7 m4): the four policy-carrying rows are settings too.
        assert_eq!(
            DEFAULTS_TABLE_SETTINGS,
            &[
                "max_rounds",
                "max_parallel",
                "max_depth",
                "max_wall_seconds",
                "recovery_reserve_pct",
                "diagnostic_budget",
                "review_mode",
                "sandbox_backend",
                "interactive",
                "git_strategy",
            ]
        );
    }

    #[test]
    fn annex_b_wire_keys_are_pinned_to_their_spec_references() {
        // The mask consumes exactly the declared Annex-B.2 vocabulary (see
        // the const's doc): widening, deleting, or reordering a key is a
        // deliberate edit that updates this pin — exactly like
        // DEFAULTS_TABLE_SETTINGS above.
        assert_eq!(
            ANNEX_B_WIRE_KEYS,
            &[
                (
                    "attempted",
                    "Annex B.2 `builder_result@v1` top-level `attempted`"
                ),
                (
                    "changed_paths",
                    "Annex B.2 `builder_result@v1` top-level `changed_paths`"
                ),
                (
                    "commands_run",
                    "Annex B.2 `builder_result@v1` top-level `commands_run`"
                ),
                (
                    "artifacts",
                    "Annex B.2 `builder_result@v1` top-level `artifacts`"
                ),
                ("path", "Annex B.2 `artifacts[].path`"),
                ("evidence_ref", "Annex B.2 `artifacts[].evidence_ref`"),
                ("claims", "Annex B.2 `builder_result@v1` top-level `claims`"),
                (
                    "self_checks",
                    "Annex B.2 `builder_result@v1` top-level `self_checks`"
                ),
                ("command", "Annex B.2 `self_checks[].command`"),
                ("exit_code", "Annex B.2 `self_checks[].exit_code`"),
                (
                    "blocked_on",
                    "Annex B.2 `builder_result@v1` top-level `blocked_on`"
                ),
            ]
        );
    }

    #[test]
    fn adapter_identifier_scope_is_the_inner_layers_only() {
        // AK-002/§24.3: domain and application source may not name adapters.
        for inner in [
            "crates/gauntlet-domain/src/lib.rs",
            "crates/gauntlet-ports/src/lib.rs",
            "crates/gauntlet-app/src/lib.rs",
            "crates/gauntlet-app/Cargo.toml",
        ] {
            assert!(in_adapter_identifier_scope(inner), "{inner}");
        }
        // The composition root may name and declare adapters (spec §4.1) —
        // but it stays inside both other scopes.
        for root_file in [
            "crates/gauntlet-cli/Cargo.toml",
            "crates/gauntlet-cli/src/main.rs",
        ] {
            assert!(!in_adapter_identifier_scope(root_file), "{root_file}");
            assert!(in_denylist_scope(root_file), "{root_file}");
            assert!(in_literal_scope(root_file), "{root_file}");
        }
    }

    #[test]
    fn repo_root_reaches_the_workspace_manifest() {
        let root = repo_root();
        assert!(
            root.join("Cargo.toml").is_file(),
            "repo root {} does not contain the workspace manifest",
            root.display()
        );
        assert!(
            root.join(".wayfinder/answer-key.md").is_file(),
            "repo root {} does not contain the frozen ruler",
            root.display()
        );
    }

    #[test]
    fn the_independent_listing_agrees_with_the_walker_on_the_real_repo() {
        // Critic-4 vacuity M1: the guard's universe must be able to disagree
        // with the walker, or guarding the walker is impossible. On a clean
        // repo the two agree on every file.
        let root = repo_root();
        let listed = unfiltered_listing(&root, CORE_CRATES);
        let tree = core_tree(&root);
        assert!(
            !listed.is_empty(),
            "the listing found nothing under the core crates"
        );
        for file in &listed {
            assert!(
                tree.files.contains(file)
                    || tree.violations.iter().any(|v| v.contains(file.as_str())),
                "'{file}' is on disk under a core crate but the walker neither listed nor \
                 reported it — a walker-level carve-out"
            );
        }
        // A carved subtree is exactly what the independent listing still sees.
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-listing-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let crate_dir = dir.join("crates/gauntlet-app/src/generated");
        std::fs::create_dir_all(&crate_dir).expect("temp tree is creatable");
        std::fs::write(crate_dir.join("wiring.rs"), b"pub const X: u8 = 1;\n")
            .expect("fixture is written");
        let listed = unfiltered_listing(&dir, CORE_CRATES);
        assert!(
            listed.contains(&"crates/gauntlet-app/src/generated/wiring.rs".to_string()),
            "the independent listing must not grow an exclusion: {listed:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unscannable_files_are_violations_not_skips() {
        // Recritique R5/R26: the 1.1 MiB module and the binary blob used to
        // vanish silently. Both are now reported.
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-scan-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir is creatable");
        std::fs::write(dir.join("text.txt"), b"plain text").expect("text fixture is written");
        std::fs::write(dir.join("blob.bin"), [0xff_u8, 0xfe, 0x00, 0x80])
            .expect("binary fixture is written");
        std::fs::write(dir.join("big.rs"), vec![b'.'; MAX_SCAN_BYTES as usize + 1])
            .expect("oversize fixture is written");

        assert_eq!(
            read_scannable(&dir, "text.txt").as_deref(),
            Ok("plain text")
        );

        let blob = read_scannable(&dir, "blob.bin").expect_err("non-UTF8 is a violation");
        assert!(blob.starts_with("scope-integrity: blob.bin:"), "{blob}");
        assert!(blob.contains("UTF-8"), "{blob}");

        let big = read_scannable(&dir, "big.rs").expect_err("oversize is a violation");
        assert!(big.starts_with("scope-integrity: big.rs:"), "{big}");
        assert!(big.contains("scan ceiling"), "{big}");

        let missing = read_scannable(&dir, "absent.rs").expect_err("missing is a violation");
        assert!(missing.contains("not readable"), "{missing}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn symlinks_inside_a_core_crate_are_violations() {
        // Recritique R3/R17/R25: `ln -s ../../../vendor/store.rs
        // crates/gauntlet-app/src/store.rs` used to remove the file from the
        // walk entirely.
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-arch-tests-link-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let crate_dir = dir.join("crates/gauntlet-app/src");
        std::fs::create_dir_all(&crate_dir).expect("temp tree is creatable");
        std::fs::write(crate_dir.join("lib.rs"), b"// clean\n").expect("lib fixture is written");
        std::fs::write(dir.join("outside.rs"), b"const DB: &str = \"sqlite\";\n")
            .expect("outside fixture is written");

        #[cfg(unix)]
        let linked =
            std::os::unix::fs::symlink("../../../outside.rs", crate_dir.join("store.rs")).is_ok();
        #[cfg(not(unix))]
        let linked = false;

        if linked {
            let tree = core_tree(&dir);
            assert!(
                tree.violations.iter().any(
                    |v| v.contains("crates/gauntlet-app/src/store.rs") && v.contains("symlink")
                ),
                "symlink was not reported: {:?}",
                tree.violations
            );
            assert!(
                !tree
                    .files
                    .contains(&"crates/gauntlet-app/src/store.rs".to_string()),
                "a symlink must not be scanned as a regular file"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
