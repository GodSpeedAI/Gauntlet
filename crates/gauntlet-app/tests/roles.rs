//! Integration tests for the roles module (plan Task 17; AK-063).
//!
//! Every tooth is named for what it proves: over-grant refusal, unknown
//! front-matter key, deny shadowing allow, the indirection classes
//! (missing variable, secret never printed), install idempotence, drift
//! detection in all three directions, naive lock tamper, full lock
//! re-forgery, the advisory fallback, fail-closed harness ids, and the
//! verify-green-right-after-install contract. The AK-063 locator names
//! `tests/contract/roles::install_and_verify`; this file delivers that
//! test as `roles::install_and_verify` — the directory divergence is a
//! recorded locator-class matter for closure, never a reason to touch
//! the ruler.
//!
//! Fixture role documents are built with joined path names and
//! parameterized values: no host layout is a literal here (spec 8.1).

use std::fs;
use std::path::PathBuf;

use gauntlet_app::config::EnvLayer;
use gauntlet_app::roles::{
    ContextCategory, Enforcement, HarnessRegistry, InstallInput, RoleDefinition, RoleError,
    RoleSurface, RolesError, VerifyInput, install, verify,
};

/// The role files' deadline value, parameterized so no duration-shaped
/// field ever shares a line with a numeric literal.
const ROLE_TICKS: u64 = 900;

/// A canary secret value: resolves into the in-memory definition and
/// must never reach the lock, the installed bytes, an error, or a Debug
/// rendering.
const CANARY: &str = "credential-canary-9f3a1c";

/// The environment every fixture role file's indirections resolve
/// against.
fn env_pairs() -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (_role, upper) in [
        ("orchestrator", "ORCHESTRATOR"),
        ("builder", "BUILDER"),
        ("critic", "CRITIC"),
        ("verifier", "VERIFIER"),
        ("observer", "OBSERVER"),
        ("optimizer", "OPTIMIZER"),
    ] {
        pairs.push((
            format!("GAUNTLET_{upper}_RUNNER"),
            "session-token".to_owned(),
        ));
        pairs.push((format!("GAUNTLET_{upper}_MODEL"), "review-model".to_owned()));
    }
    pairs.push((
        "RESULT_CONTRACT_DIR".to_owned(),
        "invocation-results".to_owned(),
    ));
    pairs.push(("GAUNTLET_STATE_DIR".to_owned(), "state-root".to_owned()));
    pairs.push((
        "GAUNTLET_ARTIFACT_DIR".to_owned(),
        "evidence-root".to_owned(),
    ));
    pairs
}

fn env_layer() -> EnvLayer {
    EnvLayer::from_parts("", &env_pairs(), &[])
}

fn env_layer_without(name: &str) -> EnvLayer {
    let pairs: Vec<(String, String)> = env_pairs()
        .into_iter()
        .filter(|(key, _)| key != name)
        .collect();
    EnvLayer::from_parts("", &pairs, &[])
}

/// The two-capability registry the composition root would front.
fn registry() -> HarnessRegistry {
    let mut registry = HarnessRegistry::new();
    registry
        .insert("writable-harness", RoleSurface::WritableRoleDir)
        .expect("a valid registry id");
    registry
        .insert("advisory-harness", RoleSurface::AdvisoryPromptArtifacts)
        .expect("a valid registry id");
    registry
}

/// A fresh project fixture: project root, roles dir, state dir.
fn fixture_project(case: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join("gauntlet-t17-tests").join(case);
    let _ = fs::remove_dir_all(&root);
    let roles = root.join(".agents").join("roles");
    fs::create_dir_all(&roles).expect("fixture roles dir creates");
    let state = root.join("state-root");
    (root, roles, state)
}

/// One standard §10.1 role document with the given context lists.
fn role_document(role: &str, upper: &str, allow: &str, deny: &str) -> String {
    format!(
        "---\nrole: {role}\ndescription: Test contract for the {role} role\n\
         runner: $GAUNTLET_{upper}_RUNNER\nmodel: $GAUNTLET_{upper}_MODEL\n\
         permissions:\n  filesystem: read-only\n  write_paths: [$RESULT_CONTRACT_DIR]\n  \
         network: deny\n  tools: [code_nav]\ncontext:\n  allow: [{allow}]\n  deny: [{deny}]\n\
         result_contract: comparison_record@v1\ndeadline_seconds: {ticks}\n---\n\
         # {role} body\n\nProse cites spec 10.1 and never carries control logic.\n",
        role = role,
        upper = upper,
        allow = allow,
        deny = deny,
        ticks = ROLE_TICKS,
    )
}

/// Writes the standard two-role fixture tree.
fn write_fixture_tree(roles: &std::path::Path) {
    fs::write(
        roles.join("builder.md"),
        role_document(
            "builder",
            "BUILDER",
            "reference, open_residuals",
            "comparison_contract, builder_trajectory",
        ),
    )
    .expect("fixture role file writes");
    fs::write(
        roles.join("critic.md"),
        role_document(
            "critic",
            "CRITIC",
            "reference, observation_bundle, comparison_contract, applicable_constraints, \
             open_residuals",
            "builder_trajectory, builder_reasoning, attempt_count, builder_identity, \
             prior_verdicts, operator_chat",
        ),
    )
    .expect("fixture role file writes");
}

fn lock_bytes(root: &std::path::Path) -> Vec<u8> {
    fs::read(root.join(".gauntlet").join("roles.lock")).expect("the lock exists")
}

fn installed_path(state: &std::path::Path, harness: &str, stem: &str) -> PathBuf {
    state
        .join("roles")
        .join("installed")
        .join(harness)
        .join(format!("{stem}.md"))
}

// ---------------------------------------------------------------------------
// AK-063: install, verify green right after, content is verbatim
// ---------------------------------------------------------------------------

#[test]
fn install_and_verify() {
    let (root, roles, state) = fixture_project("install-and-verify");
    write_fixture_tree(&roles);
    let env = env_layer();
    let registry = registry();

    let report = install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env,
    })
    .expect("install succeeds");
    assert_eq!(report.enforcement, Enforcement::Writable);
    assert_eq!(report.roles, 2);
    assert!(!report.repaired, "a first install is not a repair");
    assert!(
        report.lock_path.exists(),
        "the lock lands where install wrote it"
    );

    // Verify is GREEN immediately after install (AK-063's done-when).
    let seen = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect("verify is green right after install");
    assert_eq!(seen.harness, "writable-harness");
    assert_eq!(seen.enforcement, Enforcement::Writable);
    assert_eq!(seen.roles, 2);

    // The installed artifact is the source file's bytes, verbatim.
    let source = fs::read(roles.join("builder.md")).expect("source reads");
    let installed = fs::read(installed_path(&state, "writable-harness", "builder"))
        .expect("installed artifact reads");
    assert_eq!(source, installed, "install materializes verbatim bytes");
}

// ---------------------------------------------------------------------------
// Idempotence
// ---------------------------------------------------------------------------

#[test]
fn reinstall_of_unchanged_tree_is_byte_identical_lock() {
    let (root, roles, state) = fixture_project("idempotent-install");
    write_fixture_tree(&roles);
    let env = env_layer();
    let registry = registry();
    let input = InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env,
    };
    let first = install(&input).expect("first install");
    let first_bytes = lock_bytes(&root);
    let second = install(&input).expect("second install");
    let second_bytes = lock_bytes(&root);
    assert_eq!(
        first_bytes, second_bytes,
        "an unchanged tree reinstalls to a byte-identical lock"
    );
    assert!(
        !second.repaired,
        "re-installing an unchanged tree repairs nothing"
    );
    let _ = first;
}

// ---------------------------------------------------------------------------
// Drift: installed side, source side, missing files
// ---------------------------------------------------------------------------

#[test]
fn out_of_band_installed_edit_is_drift() {
    let (root, roles, state) = fixture_project("installed-drift");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");
    let target = installed_path(&state, "writable-harness", "builder");
    let mut tampered = fs::read(&target).expect("installed artifact reads");
    tampered.push(b'#');
    fs::write(&target, &tampered).expect("out-of-band edit applies");
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("an out-of-band installed edit is drift");
    let RolesError::Drift { entries } = error else {
        panic!("drift is the failure class, got {error:?}");
    };
    assert!(
        entries.iter().any(|record| record.role == "builder"
            && record.kind == gauntlet_app::roles::DriftKind::InstalledModified),
        "the installed side is what drifted: {entries:?}"
    );
}

#[test]
fn source_edit_after_install_is_drift() {
    let (root, roles, state) = fixture_project("source-drift");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");
    let target = roles.join("critic.md");
    let mut edited = fs::read_to_string(&target).expect("source reads");
    edited.push_str("\nAn unreviewed afterthought.\n");
    fs::write(&target, edited).expect("source edit applies");
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("a source edit since install is drift");
    let RolesError::Drift { entries } = error else {
        panic!("drift is the failure class, got {error:?}");
    };
    assert!(
        entries.iter().any(|record| record.role == "critic"
            && record.kind == gauntlet_app::roles::DriftKind::SourceModified),
        "the source side is what drifted: {entries:?}"
    );
}

#[test]
fn missing_installed_file_is_drift() {
    let (root, roles, state) = fixture_project("missing-installed");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");
    fs::remove_file(installed_path(&state, "writable-harness", "critic"))
        .expect("the harness loses a file");
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("a missing installed artifact is drift");
    let RolesError::Drift { entries } = error else {
        panic!("drift is the failure class, got {error:?}");
    };
    assert!(
        entries.iter().any(|record| record.role == "critic"
            && record.kind == gauntlet_app::roles::DriftKind::InstalledMissing),
        "the missing artifact is named: {entries:?}"
    );
}

// ---------------------------------------------------------------------------
// Lock tamper: naive edit, and full re-forgery
// ---------------------------------------------------------------------------

#[test]
fn hand_edited_lock_is_a_typed_tamper_failure() {
    let (root, roles, state) = fixture_project("naive-lock-tamper");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");
    let lock_path = root.join(".gauntlet").join("roles.lock");
    let edited = String::from_utf8(lock_bytes(&root))
        .expect("the lock is text")
        .replacen(
            "harness: writable-harness",
            "harness: writable-harness-two",
            1,
        );
    fs::write(&lock_path, edited).expect("the hand edit applies");
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("a hand-edited lock is tamper");
    assert!(
        matches!(error, RolesError::LockTampered { .. }),
        "tamper is the failure class, got {error:?}"
    );
}

#[test]
fn full_lock_reforge_cannot_bless_installed_drift() {
    let (root, roles, state) = fixture_project("lock-reforge");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");

    // The committed attack: drift the installed artifact, then re-forge
    // the lock with the drifted digest AND a recomputed self-digest, so
    // every self-consistency check the lock can make passes.
    let target = installed_path(&state, "writable-harness", "builder");
    let mut drifted = fs::read(&target).expect("installed artifact reads");
    drifted.push(b'#');
    fs::write(&target, &drifted).expect("out-of-band edit applies");

    use sha2::{Digest as _, Sha256};
    let hex_of = |bytes: &[u8]| {
        let mut hex = String::new();
        for byte in Sha256::digest(bytes).iter() {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    };
    let drifted_digest = format!("sha256:{}", hex_of(&drifted));
    let original = String::from_utf8(lock_bytes(&root)).expect("the lock is text");
    let body = original
        .lines()
        .filter(|line| !line.starts_with("lock_digest:"))
        .collect::<Vec<&str>>()
        .join("\n");
    let recorded = format!(
        "    digest: sha256:{}",
        original_digest_for(&original, "builder")
    );
    let replacement = format!("    digest: {drifted_digest}");
    let forged_body = body.replacen(&recorded, &replacement, 1);
    // The self-digest covers the canonical body INCLUDING its final
    // newline — the re-forge is done by the lock's own documented rule.
    let self_digest = hex_of(format!("{forged_body}\n").as_bytes());
    let forged = format!("{forged_body}\nlock_digest: sha256:{self_digest}\n");
    fs::write(root.join(".gauntlet").join("roles.lock"), forged).expect("the re-forge applies");

    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("even a fully re-forged lock cannot bless drift");
    let RolesError::Drift { entries } = error else {
        panic!("drift is the failure class, got {error:?}");
    };
    assert!(
        entries.iter().any(|record| record.role == "builder"
            && record.kind == gauntlet_app::roles::DriftKind::SourceModified),
        "the untouched repository source exposes the forgery: {entries:?}"
    );
}

/// Reads one entry's recorded digest back out of a lock's text (test
/// helper for the re-forge).
fn original_digest_for(lock_text: &str, role: &str) -> String {
    let marker = format!("  - role: {role}\n");
    let start = lock_text
        .find(&marker)
        .unwrap_or_else(|| panic!("lock records {role}"));
    let window = &lock_text[start..start + marker.len() + 400];
    let line = window
        .lines()
        .find(|line| line.starts_with("    digest: "))
        .unwrap_or_else(|| panic!("lock records a digest for {role}"));
    line.trim_start_matches("    digest: sha256:").to_owned()
}

/// SHA-256 as lowercase hex (test helper shared by the forgery legs).
fn sha256_hex_of(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hex = String::new();
    for byte in Sha256::digest(bytes).iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// COR-O1: a NON-canonical lock edit carrying a RECOMPUTED self-digest
/// is caught ONLY by the canonical closure. The digest check passes
/// (the attacker recomputed it), the strict parse passes (the deviation
/// is parse-transparent — trailing whitespace is trimmed by the reader),
/// and the digests all still agree (only the harness line changed) — so
/// without the canonical re-serialization equality this forgery shape
/// verifies green. Proven RED when the closure is gutted.
#[test]
fn noncanonical_lock_with_recomputed_digest_is_tamper() {
    let (root, roles, state) = fixture_project("noncanonical-reforge");
    write_fixture_tree(&roles);
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("install");
    let original = String::from_utf8(lock_bytes(&root)).expect("the lock is text");
    // The deviation: a trailing space on the harness line — invisible to
    // the strict prefix parse (the reader trims), visible to the
    // canonical re-serialization.
    let edited_body = original
        .lines()
        .filter(|line| !line.starts_with("lock_digest:"))
        .map(|line| {
            if line.starts_with("harness: ") {
                format!("{line} ")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    let self_digest = sha256_hex_of(format!("{edited_body}\n").as_bytes());
    let forged = format!("{edited_body}\nlock_digest: sha256:{self_digest}\n");
    fs::write(root.join(".gauntlet").join("roles.lock"), forged).expect("the re-forge applies");
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("a parse-transparent non-canonical lock is tamper");
    assert!(
        matches!(error, RolesError::LockTampered { .. }),
        "the canonical closure is what catches this shape, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Loader teeth: over-grant, unknown key, shadowed allow
// ---------------------------------------------------------------------------

#[test]
fn allow_naming_an_envelope_denied_category_is_refused() {
    let text = role_document("builder", "BUILDER", "reference, builder_trajectory", "");
    let error = RoleDefinition::load("builder", &text, &env_layer()).expect_err("over-grant");
    assert!(
        matches!(error, RoleError::OverGrant { .. }),
        "over-grant is the failure class, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("never grant beyond"),
        "the refusal names the invariant: {message}"
    );
}

#[test]
fn unknown_front_matter_key_is_refused() {
    let mut text = role_document("builder", "BUILDER", "reference", "builder_trajectory");
    text = text.replace(
        "result_contract:",
        "governance: self-service\nresult_contract:",
    );
    let error = RoleDefinition::load("builder", &text, &env_layer()).expect_err("unknown key");
    assert!(
        matches!(&error, RoleError::UnknownField { field, .. } if field == "governance"),
        "the unknown key is named, got {error:?}"
    );
}

#[test]
fn deny_shadowing_allow_is_refused() {
    let text = role_document(
        "builder",
        "BUILDER",
        "reference, open_residuals",
        "reference",
    );
    let error = RoleDefinition::load("builder", &text, &env_layer())
        .expect_err("a deny that shadows its own allow");
    assert!(
        matches!(error, RoleError::DenyShadowsAllow { category, .. } if category == ContextCategory::Reference),
        "the shadowed category is named, got {error:?}"
    );
}

#[test]
fn unknown_context_category_is_refused() {
    let text = role_document(
        "builder",
        "BUILDER",
        "reference, buildr_intent",
        "builder_trajectory",
    );
    let error = RoleDefinition::load("builder", &text, &env_layer()).expect_err("typo");
    assert!(
        matches!(&error, RoleError::UnknownContextCategory { token, .. } if token == "buildr_intent"),
        "the typo is named, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Indirection classes
// ---------------------------------------------------------------------------

#[test]
fn missing_indirection_is_a_typed_error_naming_the_variable() {
    let text = role_document("critic", "CRITIC", "reference", "builder_trajectory");
    let env = env_layer_without("GAUNTLET_CRITIC_MODEL");
    let error = RoleDefinition::load("critic", &text, &env).expect_err("missing variable");
    let RoleError::Indirection { source, .. } = &error else {
        panic!("indirection is the failure class, got {error:?}");
    };
    let message = source.to_string();
    assert!(
        message.contains("GAUNTLET_CRITIC_MODEL"),
        "the variable is named: {message}"
    );
    assert!(
        message.contains("roles.critic.model"),
        "the locus is named: {message}"
    );
}

#[test]
fn resolved_secrets_never_reach_the_lock_or_bytes_or_debug() {
    let (root, roles, state) = fixture_project("secret-never-prints");
    fs::write(
        roles.join("builder.md"),
        role_document(
            "builder",
            "BUILDER",
            "reference, open_residuals",
            "comparison_contract, builder_trajectory",
        ),
    )
    .expect("fixture role file writes");
    let mut pairs = env_pairs();
    for pair in pairs.iter_mut() {
        if pair.0 == "GAUNTLET_BUILDER_RUNNER" {
            pair.1 = CANARY.to_owned();
        }
    }
    let env = EnvLayer::from_parts("", &pairs, &[]);
    let definition = RoleDefinition::load(
        "builder",
        &fs::read_to_string(roles.join("builder.md")).expect("fixture reads"),
        &env,
    )
    .expect("the definition loads with the secret resolved");
    assert!(
        !format!("{definition:?}").contains(CANARY),
        "the Debug rendering redacts resolved values"
    );
    let registry = registry();
    install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env,
    })
    .expect("install succeeds with the secret present");
    let lock = String::from_utf8(lock_bytes(&root)).expect("the lock is text");
    assert!(
        !lock.contains(CANARY),
        "the lock never carries a resolved value"
    );
    let installed = fs::read(installed_path(&state, "writable-harness", "builder"))
        .expect("installed artifact reads");
    assert!(
        !String::from_utf8(installed)
            .expect("installed artifact is text")
            .contains(CANARY),
        "install writes verbatim source bytes, never resolved values"
    );
}

// ---------------------------------------------------------------------------
// Containment: a write path escaping the roots is refused
// ---------------------------------------------------------------------------

#[test]
fn write_path_resolving_outside_the_roots_is_refused() {
    let (root, roles, _state) = fixture_project("write-path-escape");
    let escape_var = "GAUNTLET_T17_ESCAPE_TARGET";
    let escape_target = std::env::temp_dir()
        .join("gauntlet-t17-escape-target")
        .to_string_lossy()
        .into_owned();
    let mut pairs = env_pairs();
    pairs.push((escape_var.to_owned(), escape_target));
    let env = EnvLayer::from_parts("", &pairs, &[]);
    let text = role_document("builder", "BUILDER", "reference", "builder_trajectory").replace(
        "write_paths: [$RESULT_CONTRACT_DIR]",
        &format!("write_paths: [${escape_var}]"),
    );
    fs::write(roles.join("builder.md"), text).expect("fixture role file writes");
    let registry = registry();
    let error = install(&InstallInput {
        project_root: &root,
        state_dir: &root.join("state-root"),
        roles_dir: &roles,
        harness: "writable-harness",
        registry: &registry,
        env: &env,
    })
    .expect_err("an escaping write path is refused");
    assert!(
        matches!(error, RolesError::WritePathEscape { .. }),
        "the escape is a typed refusal, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Advisory fallback + the whole-registry sweep
// ---------------------------------------------------------------------------

#[test]
fn advisory_harness_records_advisory_and_materializes_injectable_artifacts() {
    let (root, roles, state) = fixture_project("advisory-fallback");
    write_fixture_tree(&roles);
    let registry = registry();
    let report = install(&InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "advisory-harness",
        registry: &registry,
        env: &env_layer(),
    })
    .expect("the fallback install succeeds");
    assert_eq!(report.enforcement, Enforcement::Advisory);
    let injected = state
        .join("roles")
        .join("injectable")
        .join("advisory-harness")
        .join("critic.md");
    assert!(injected.exists(), "the injectable bodies are materialized");
    let lock = String::from_utf8(lock_bytes(&root)).expect("the lock is text");
    assert!(
        lock.contains("enforcement: advisory"),
        "the lock records the visible weakness: {lock}"
    );
    let seen = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect("verify is green right after the advisory install");
    assert_eq!(seen.enforcement, Enforcement::Advisory);
}

#[test]
fn every_registered_harness_installs_and_verifies() {
    let (root, roles, state) = fixture_project("registry-sweep");
    write_fixture_tree(&roles);
    let registry = registry();
    let ids: Vec<&str> = registry.ids().collect();
    assert_eq!(ids.len(), 2, "both registered capabilities are swept");
    for id in ids {
        install(&InstallInput {
            project_root: &root,
            state_dir: &state,
            roles_dir: &roles,
            harness: id,
            registry: &registry,
            env: &env_layer(),
        })
        .unwrap_or_else(|error| panic!("install over {id} succeeds: {error:?}"));
        verify(&VerifyInput {
            project_root: &root,
            state_dir: &state,
        })
        .unwrap_or_else(|error| panic!("verify over {id} is green: {error:?}"));
    }
}

// ---------------------------------------------------------------------------
// Fail-closed harness ids (D3)
// ---------------------------------------------------------------------------

#[test]
fn unknown_or_missing_harness_fails_closed() {
    let (root, roles, state) = fixture_project("fail-closed-harness");
    write_fixture_tree(&roles);
    let registry = registry();
    let input = InstallInput {
        project_root: &root,
        state_dir: &state,
        roles_dir: &roles,
        harness: "",
        registry: &registry,
        env: &env_layer(),
    };
    let error = install(&input).expect_err("an empty harness id fails closed");
    assert!(
        matches!(error, RolesError::HarnessUnspecified),
        "unspecified is its own class, got {error:?}"
    );
    let input = InstallInput {
        harness: "never-heard-of",
        ..input
    };
    let error = install(&input).expect_err("an unregistered harness id fails closed");
    let RolesError::UnknownHarness { id } = error else {
        panic!("unknown is its own class, got {error:?}");
    };
    assert_eq!(id, "never-heard-of");
}

#[test]
fn verify_without_install_is_a_configuration_failure() {
    let (root, roles, state) = fixture_project("verify-without-install");
    write_fixture_tree(&roles);
    let error = verify(&VerifyInput {
        project_root: &root,
        state_dir: &state,
    })
    .expect_err("verify without install has nothing to check");
    assert!(
        matches!(error, RolesError::LockMissing { .. }),
        "missing evidence is not passing evidence, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// The envelope consume-side hook, as data
// ---------------------------------------------------------------------------

#[test]
fn deny_set_and_envelope_grant_are_consumable_data() {
    let text = role_document(
        "builder",
        "BUILDER",
        "reference, open_residuals",
        "comparison_contract, builder_trajectory",
    );
    let definition = RoleDefinition::load("builder", &text, &env_layer()).expect("loads");
    assert_eq!(
        definition.envelope_grant(),
        vec![ContextCategory::Reference, ContextCategory::OpenResiduals],
        "the effective envelope grant is allow, already proven inside the permitted set"
    );
    let deny = definition.context_deny_set();
    assert!(
        deny.denies(ContextCategory::ComparisonContract),
        "the deny-set carries the narrowing the envelope side must consume"
    );
    assert!(
        deny.denies(ContextCategory::BuilderTrajectory),
        "the deny-set carries the blindness categories too"
    );
    assert!(
        deny.categories().windows(2).all(|pair| pair[0] < pair[1]),
        "the deny-set is deterministic (sorted, de-duplicated): {:?}",
        deny.categories()
    );
}

// ---------------------------------------------------------------------------
// Dimension stems load through the same constructor
// ---------------------------------------------------------------------------

#[test]
fn critic_dimension_files_load_uniformly_and_mismatches_are_refused() {
    let text = role_document(
        "critic",
        "CRITIC",
        "reference, observation_bundle",
        "builder_trajectory",
    );
    let definition =
        RoleDefinition::load("critic-security", &text, &env_layer()).expect("dimension loads");
    assert_eq!(definition.dimension(), Some("security"));
    assert_eq!(definition.role(), &gauntlet_domain::RoleId::Critic);
    let error = RoleDefinition::load(
        "critic-security",
        &text.replace("role: critic", "role: builder"),
        &env_layer(),
    )
    .expect_err("a dimension stem must declare the critic role");
    assert!(matches!(error, RoleError::RoleDimensionMismatch { .. }));
    let plain = role_document("builder", "BUILDER", "reference", "builder_trajectory");
    let error = RoleDefinition::load("worker", &plain, &env_layer())
        .expect_err("a plain stem must name its role");
    assert!(matches!(error, RoleError::RoleDimensionMismatch { .. }));
}

// ---------------------------------------------------------------------------
// The repository's own role artifacts load (they are install inputs)
// ---------------------------------------------------------------------------

#[test]
fn repository_role_artifacts_load() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .ancestors()
        .nth(2)
        .expect("the manifest dir is two levels below the repository root");
    let roles = repo_root.join(".agents").join("roles");
    let env = env_layer();
    let mut stems = Vec::new();
    let mut role_names = Vec::new();
    for entry in fs::read_dir(&roles).expect("the repository roles dir reads") {
        let path = entry.expect("entry reads").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("a stem")
            .to_owned();
        let text = fs::read_to_string(&path).expect("role file reads");
        let definition = RoleDefinition::load(&stem, &text, &env)
            .unwrap_or_else(|error| panic!("repository role {stem} loads: {error:?}"));
        role_names.push(definition.role().name().to_owned());
        stems.push(stem);
    }
    for named in [
        "orchestrator",
        "builder",
        "critic",
        "verifier",
        "observer",
        "optimizer",
    ] {
        assert!(
            role_names.iter().any(|role| role == named),
            "the named role {named} is an artifact"
        );
    }
    assert!(
        stems.iter().any(|stem| stem == "critic-functional"),
        "the critic fan-out dimensions are artifacts"
    );
    assert!(
        stems
            .iter()
            .any(|stem| stem == "critic-evidence-completeness"),
        "the evidence-completeness dimension is an artifact"
    );
}
