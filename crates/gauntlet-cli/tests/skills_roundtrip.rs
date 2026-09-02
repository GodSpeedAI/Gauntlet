//! The skills-lock ROUND TRIP proof (COR-F5; falsifier round 1, F-6):
//! `gauntlet skills install` → `gauntlet skills verify` PASSES — the
//! writer emits the schema the reader requires (root `skills` = array) —
//! and the failure halves are typed: an out-of-band edit to an installed
//! skill is named DRIFT; a hand-forged lock (including the old writer's
//! doubly-nested shape the reader never accepted) is a typed refusal.
//! Before COR-F5 the documented install → verify pair failed typed on
//! EVERY tree and no shipped test noticed; this file pins the round trip
//! shut.
//!
//! The world: a scratch project root with the eight REPO_OWNED_SKILLS'
//! sources copied from the repository's own `.agents/skills/` tree (the
//! closed list is the application's; the sources are the repo's). The
//! real binary runs with the scratch as its working directory, so
//! project-root discovery anchors there — the repo tree is never
//! touched.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The workspace root: this package's manifest sits two levels below it
/// (`crates/gauntlet-cli`) — derived, never a path literal (AK-016).
fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// The skills subcommand and its world.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-skills-roundtrip-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        for name in gauntlet_app::skills::REPO_OWNED_SKILLS {
            let source = workspace_root()
                .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
                .join("skills")
                .join(name)
                .join("SKILL.md");
            let text = std::fs::read_to_string(&source)
                .unwrap_or_else(|error| panic!("the repo's own {name} skill reads: {error}"));
            let destination = root
                .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
                .join("skills")
                .join(name);
            std::fs::create_dir_all(&destination).expect("the skill source dir is creatable");
            std::fs::write(destination.join("SKILL.md"), text).expect("the skill source copies");
        }
        Self { root }
    }

    fn skills(&self, args: &[&str]) -> Command {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(&self.root);
        command.args(["skills"].iter().chain(args.iter()));
        command
    }

    fn output(&self, args: &[&str]) -> (String, String, bool) {
        let output = self
            .skills(args)
            .output()
            .expect("the gauntlet binary runs");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.success(),
        )
    }

    /// The lock file's path under the world's runtime dir.
    fn lock(&self) -> PathBuf {
        self.root
            .join(gauntlet_app::config::defaults::RUNTIME_DIR_NAME)
            .join("skills.lock")
    }

    /// One installed skill's file.
    fn installed(&self, name: &str) -> PathBuf {
        self.root
            .join(gauntlet_app::config::defaults::RUNTIME_DIR_NAME)
            .join("skills")
            .join(name)
            .join("SKILL.md")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The lock's root `skills` value, parsed from the written bytes.
fn root_skills_value(lock: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(lock).map_err(|error| error.to_string())?;
    let parsed =
        gauntlet_ports::json::parse(&text).map_err(|error| format!("not JSON: {error}"))?;
    match parsed.get("skills") {
        Some(gauntlet_ports::json::JsonValue::Array(entries)) => Ok(entries.len()),
        other => Err(format!("the root skills value is not an array: {other:?}")),
    }
}

#[test]
fn install_then_verify_round_trips_and_the_lock_is_the_readers_schema() {
    let world = World::new("roundtrip");
    // INSTALL: lint-gated, lock written, exit 0.
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds: stdout={stdout} stderr={stderr}");
    assert!(
        stdout.contains("installed") && stdout.contains("lint: pass"),
        "install reports its honest posture: {stdout}"
    );
    // THE SCHEMA PIN: the WRITER emits the READER's schema — the root
    // `skills` field IS the array (the pre-COR-F5 writer nested it a
    // second time, `{"skills":{"skills":[…]}}`, and verify could never
    // pass). The count matches the closed owned-skills list.
    let entries =
        root_skills_value(&world.lock()).expect("the written lock is the reader's schema");
    assert_eq!(
        entries,
        gauntlet_app::skills::REPO_OWNED_SKILLS.len(),
        "every owned skill is locked"
    );
    // VERIFY: the documented pair passes.
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        ok,
        "verify passes right after install: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("no drift, lint clean"),
        "verify reports a clean round trip: {stdout}"
    );
}

#[test]
fn an_out_of_band_edit_is_named_drift_and_reinstall_repairs() {
    let world = World::new("drift");
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds first: {stdout} {stderr}");
    // Out-of-band edit: the installed tree drifts from the lock.
    let victim = gauntlet_app::skills::REPO_OWNED_SKILLS[0];
    let path = world.installed(victim);
    let mut text = std::fs::read_to_string(&path).expect("the installed skill reads");
    text.push_str("\n<!-- out-of-band edit -->\n");
    std::fs::write(&path, text).expect("the installed skill is edited");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "verify fails typed after an out-of-band edit: stdout={stdout}"
    );
    assert!(
        stderr.contains("DRIFTED") && stderr.contains(victim),
        "the drift is named (skill + both digests): {stderr}"
    );
    // Re-install repairs (the roles lock's repair semantics).
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "re-install repairs: {stdout} {stderr}");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(ok, "verify is clean after the repair: {stdout} {stderr}");
}

#[test]
fn a_forged_lock_is_a_typed_refusal_and_the_old_nested_shape_never_reads() {
    let world = World::new("forged");
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds first: {stdout} {stderr}");
    let lock = world.lock();
    // Forge ONE digest inside an otherwise well-formed lock: the reader
    // refuses the lock's shape, then verify names the drift it produced.
    let original = std::fs::read_to_string(&lock).expect("the lock reads");
    let forged = original.replace("sha256:", "sha257:");
    std::fs::write(&lock, forged).expect("the forged lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(!ok, "a forged lock is refused: stdout={stdout}");
    assert!(!stderr.is_empty(), "the refusal is typed: {stderr}");
    // The OLD writer's doubly-nested shape — the exact bytes the pre-COR-F5
    // writer emitted — is a typed refusal (`carries no skills array`), the
    // regression that made verify unreachable after a real install.
    let nested = "{\"schema\":\"gauntlet-skills-lock@v1\",\"count\":1,\"skills\":{\"skills\":\
                  [{\"name\":\"grilling\",\"digest\":\"sha256:00000000000000000000000000000000\
                   000000000000000000000000000000000\"}]}}\n";
    std::fs::write(&lock, nested).expect("the nested-shape lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "the doubly-nested shape never verifies: stdout={stdout}"
    );
    assert!(
        stderr.contains("carries no skills array"),
        "the refusal names the schema disagreement: {stderr}"
    );
}

/// COR-R2-3 (falsifier round 2, F-R2-2): the reader is STRICT — every
/// forged shape the falsifier executed against the lenient reader now
/// refuses typed: a wrong schema token (the version-gate bypass), a lying
/// `count`, a REMOVED entry (silent coverage narrowing over a consistent
/// count, so only the owned-set completeness check can catch it), and
/// unknown fields at the root and inside entries. The honest lock and the
/// documented round trip stay byte for byte.
#[test]
fn the_lock_reader_refuses_the_forged_strict_shapes() {
    let world = World::new("strict");
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds first: {stdout} {stderr}");
    let lock = world.lock();
    let original = std::fs::read_to_string(&lock).expect("the lock reads");
    let owned = gauntlet_app::skills::REPO_OWNED_SKILLS.len();
    assert_eq!(
        root_skills_value(&lock).expect("the lock reads"),
        owned,
        "the installed lock locks every owned skill"
    );

    // 1. A wrong schema token — the reader enforces the version gate.
    std::fs::write(
        &lock,
        original.replace("gauntlet-skills-lock@v1", "gauntlet-skills-lock@v9"),
    )
    .expect("the wrong-token lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(!ok, "a wrong schema token is refused: stdout={stdout}");
    assert!(
        stderr.contains("schema"),
        "the refusal names the schema: {stderr}"
    );

    // 2. A lying count — declared, then cross-checked against the array.
    std::fs::write(
        &lock,
        original.replace(
            &format!("\"count\":{owned}"),
            &format!("\"count\":{}", owned + 91),
        ),
    )
    .expect("the lying-count lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(!ok, "a lying count is refused: stdout={stdout}");
    assert!(
        stderr.contains("declares"),
        "the refusal names the count disagreement: {stderr}"
    );

    // 3. A REMOVED entry over a CONSISTENT count (owned-1 entries, count
    //    owned-1): the digest comparison alone would silently pass a
    //    narrowed lock — the owned-set completeness check is the tooth.
    let entry_start = original
        .find("{\"name\":\"")
        .expect("the first entry starts");
    let entry_end = original[entry_start..].find('}').expect("the entry ends");
    let mut narrowed = String::with_capacity(original.len());
    narrowed.push_str(&original[..entry_start]);
    let after_entry = &original[entry_start + entry_end + 1..];
    let after_entry = after_entry.strip_prefix(',').unwrap_or(after_entry);
    narrowed.push_str(after_entry);
    narrowed = narrowed.replace(
        &format!("\"count\":{owned}"),
        &format!("\"count\":{}", owned - 1),
    );
    std::fs::write(&lock, &narrowed).expect("the narrowed lock is written");
    assert_eq!(
        root_skills_value(&lock).expect("the narrowed lock parses"),
        owned - 1,
        "the forge removed exactly one entry"
    );
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "a coverage-narrowing lock is refused despite honest digests: stdout={stdout}"
    );
    assert!(
        stderr.contains("owned skill set"),
        "the refusal names the set disagreement: {stderr}"
    );

    // 4. An unknown ROOT field — drift is not data.
    let mut forged = original.clone();
    forged.replace_range(1..1, "\"extra\":1,");
    std::fs::write(&lock, &forged).expect("the unknown-root-field lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(!ok, "an unknown root field is refused: stdout={stdout}");
    assert!(
        stderr.contains("unknown root field"),
        "the refusal names the field: {stderr}"
    );

    // 5. An unknown field INSIDE an entry.
    let poisoned = original.replace("{\"name\":\"", "{\"note\":\"x\",\"name\":\"");
    assert_ne!(poisoned, original, "the entry forge landed");
    std::fs::write(&lock, &poisoned).expect("the unknown-entry-field lock is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(!ok, "an unknown entry field is refused: stdout={stdout}");
    assert!(
        stderr.contains("unknown field"),
        "the refusal names the entry field: {stderr}"
    );

    // The honest lock still verifies — the strictness refuses forgeries,
    // never the documented round trip.
    std::fs::write(&lock, &original).expect("the honest lock is restored");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        ok,
        "the honest round trip survives the strict reader: {stdout} {stderr}"
    );
}

/// COR-R3-2 (falsifier round 3, F-R3-2): the CONTENT half the shape checks
/// cannot see — verify anchors every lock entry to the CANONICAL SOURCE
/// body (`.agents/skills/**`, the repo's reviewed copy), exactly the way
/// roles verify anchors its lock to the roles tree. The falsifier's exact
/// attack shapes refuse typed:
///
/// - a shape-valid lock whose digests are internally consistent over a
///   SWAPPED installed body (the lock re-forged to match the tampered
///   body, lint clean and all) — the source anchor is the ONLY check that
///   can see it, and it names the skill and both digests;
/// - a SOURCE edited after install (installed body + lock untouched, the
///   stale-install shape roles verify names "source file modified since
///   install").
///
/// The honest round trip (install → verify; body restored → verify;
/// source restored → verify) stays green.
///
/// COR-R4-2 mechanical adaptation (falsifier round 4): the forgeries'
/// digests are computed with the surface's NEW instrument —
/// [`gauntlet_app::skills::digest_raw`], raw bytes, the roles lock's own
/// hashing rule — because the surface now digests raw bytes exactly as
/// roles does (the correction under test). The forgery stays internally
/// consistent the way the falsifier's attack was; every assertion's
/// strength is unchanged.
#[test]
fn a_self_consistent_lock_over_tampered_bodies_is_drift_against_the_canonical_source() {
    let world = World::new("content-anchor");
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds first: {stdout} {stderr}");
    let victim = gauntlet_app::skills::REPO_OWNED_SKILLS[0];
    let lock = world.lock();

    // SHAPE 1 — the swapped body under a self-consistent, re-forged lock.
    // The replacement prose is lint-clean ON PURPOSE: the anchor check
    // must be the check that refuses, not the lint.
    let installed = world.installed(victim);
    let tampered = {
        let mut text = std::fs::read_to_string(&installed).expect("the installed skill reads");
        text.push_str(
            "\nThis closing paragraph documents the installed skill for readers \
                       of the tree and carries no operational content.\n",
        );
        text
    };
    std::fs::write(&installed, &tampered).expect("the swapped body is written");
    let tampered_digest = gauntlet_app::skills::digest_raw(tampered.as_bytes());
    let original_lock = std::fs::read_to_string(&lock).expect("the lock reads");
    // Re-forge the lock honestly over the tampered body: the victim
    // entry's digest becomes the tampered body's digest.
    let victim_entry = original_lock
        .split('{')
        .find(|entry| entry.contains(victim))
        .expect("the victim's entry exists")
        .split('}')
        .next()
        .expect("the entry ends");
    let old_digest = victim_entry
        .split("\"digest\":\"")
        .nth(1)
        .expect("the entry carries a digest")
        .split('"')
        .next()
        .expect("the digest value ends")
        .to_owned();
    let forged_lock = original_lock.replace(&old_digest, &tampered_digest);
    assert_ne!(forged_lock, original_lock, "the lock forge landed");
    std::fs::write(&lock, forged_lock).expect("the re-forged lock is written");

    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "a shape-valid lock over tampered bodies refuses: stdout={stdout}"
    );
    assert!(
        stderr.contains("SOURCE DRIFTED") && stderr.contains(victim),
        "the drift is typed, naming the skill: {stderr}"
    );
    assert!(
        stderr.contains(&tampered_digest),
        "the refusal names the lock's (forged) digest: {stderr}"
    );
    let source_digest = {
        let source = world
            .root
            .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
            .join("skills")
            .join(victim)
            .join("SKILL.md");
        gauntlet_app::skills::digest_raw(
            &std::fs::read(&source).expect("the canonical source reads"),
        )
    };
    assert!(
        stderr.contains(&source_digest),
        "the refusal names the canonical source digest: {stderr}"
    );

    // Re-install repairs (the roles lock's repair semantics): the lock is
    // rewritten from the CANONICAL SOURCE, so the tampered installed body
    // is overwritten and verify cleans only after the real repair.
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(
        ok,
        "re-install repairs from the canonical source: {stdout} {stderr}"
    );
    let repaired =
        std::fs::read_to_string(world.installed(victim)).expect("the repaired body reads");
    assert!(
        !repaired.contains("no operational content"),
        "the repair overwrote the tampered body from the canonical source"
    );
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(ok, "verify is clean after the repair: {stdout} {stderr}");

    // SHAPE 2 — the stale install: the SOURCE is edited after install,
    // installed body + lock untouched. Roles names this class typed
    // ("source file modified since install"); skills verify must too.
    let source = world
        .root
        .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
        .join("skills")
        .join(victim)
        .join("SKILL.md");
    let honest_source = std::fs::read_to_string(&source).expect("the canonical source reads");
    let mut edited = honest_source.clone();
    edited.push_str("\nA later repository-side revision of this skill's prose.\n");
    std::fs::write(&source, &edited).expect("the source edit is written");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "a source edit since install is never a clean verify: stdout={stdout}"
    );
    assert!(
        stderr.contains("SOURCE DRIFTED") && stderr.contains(victim),
        "the stale-install drift is typed, naming the skill: {stderr}"
    );
    // Restoring the source restores the clean verify (content equality is
    // the anchor's discipline — the digests agree again).
    std::fs::write(&source, &honest_source).expect("the canonical source is restored");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        ok,
        "verify is clean after the source is restored: {stdout} {stderr}"
    );
}

/// COR-R4-2 (falsifier round 4, F-R4-1): the content anchor digests RAW
/// bytes exactly as roles does — the normalizer pass is gone. The
/// falsifier's exact attack: the INSTALLED body is rewritten to be
/// BYTE-DIFFERENT but NORMALIZATION-EQUIVALENT (every `\n` → `\r\n`,
/// plus extra trailing blank lines), and the lock's entry is reforged to
/// the NORMALIZED digest of the swapped bytes (count, schema, and the
/// name set all left consistent). Through the normalizer the shape was
/// inside the fixed-point class and verified "no drift" — a false claim
/// over bytes that are not the canonical bytes, where roles refuses the
/// identical forgery. Under raw digesting the two shapes discriminate
/// exactly as roles does, and the honest round trip stays green:
///
/// - the normalized-digest reforge (the falsifier's exact shape) cannot
///   even land — it is a FIXPOINT of the honest lock, because the
///   canonical source is normalization-canonical — and the drift still
///   refuses, at the INSTALLED half (raw installed bytes ≠ the lock),
///   never a clean verify;
/// - the raw-digest reforge (lock internally consistent over the swapped
///   bytes) refuses through the SOURCE anchor, naming both digests;
/// - re-install repairs from the canonical source, byte-exactly.
#[test]
fn a_normalization_equivalent_body_under_a_reforged_lock_refuses_byte_drift() {
    let world = World::new("raw-anchor");
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "install succeeds first: {stdout} {stderr}");
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        ok && stdout.contains("no drift, lint clean"),
        "the honest baseline round trip is clean: {stdout} {stderr}"
    );
    let victim = gauntlet_app::skills::REPO_OWNED_SKILLS[0];
    let lock = world.lock();
    let installed = world.installed(victim);
    let source = world
        .root
        .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
        .join("skills")
        .join(victim)
        .join("SKILL.md");
    let source_bytes = std::fs::read(&source).expect("the canonical source reads");

    // THE SWAP: byte-different, normalization-equivalent.
    let swapped = {
        let mut text = String::from_utf8(source_bytes.clone()).expect("the source is UTF-8");
        text = text.replace('\n', "\r\n");
        text.push_str("\r\n\r\n");
        text.into_bytes()
    };
    assert_ne!(
        swapped, source_bytes,
        "the precondition holds: the installed bytes are NOT the source bytes"
    );
    assert_eq!(
        gauntlet_app::reference::normalize::digest_of(&swapped).expect("the swapped bytes digest"),
        gauntlet_app::reference::normalize::digest_of(&source_bytes)
            .expect("the source bytes digest"),
        "the precondition holds: the swap is INSIDE the normalizer's fixed-point class — \
         only raw digesting can see it"
    );
    std::fs::write(&installed, &swapped).expect("the swapped body is written");

    // The lock-reforge splice shared by the raw-reforged shape.
    let reforge_lock = |new_digest: &str| {
        let original_lock = std::fs::read_to_string(&lock).expect("the lock reads");
        let victim_entry = original_lock
            .split('{')
            .find(|entry| entry.contains(victim))
            .expect("the victim's entry exists")
            .split('}')
            .next()
            .expect("the entry ends");
        let old_digest = victim_entry
            .split("\"digest\":\"")
            .nth(1)
            .expect("the entry carries a digest")
            .split('"')
            .next()
            .expect("the digest value ends")
            .to_owned();
        let forged = original_lock.replace(&old_digest, new_digest);
        assert_ne!(forged, original_lock, "the lock forge landed");
        std::fs::write(&lock, forged).expect("the re-forged lock is written");
    };

    // SHAPE 1 — the falsifier's exact reforge: the lock binds the
    // NORMALIZED digest of the swapped bytes. Under raw digesting this
    // reforge is a FIXPOINT of the honest lock — the canonical source is
    // already normalization-canonical (raw digest == normalized digest)
    // and the normalizer maps the swapped body into that same class — so
    // the "reforged" lock IS the honest lock and the forgery cannot even
    // land. The pin: the drift is still refused, at the INSTALLED half
    // (raw installed bytes ≠ the lock) — exactly where roles refuses the
    // same shape — and the clean-verify claim is never printed over
    // non-canonical bytes.
    let normalized_digest = gauntlet_app::reference::normalize::digest_of(&swapped)
        .expect("the swapped bytes digest")
        .to_string();
    let original_lock = std::fs::read_to_string(&lock).expect("the lock reads");
    let victim_entry = original_lock
        .split('{')
        .find(|entry| entry.contains(victim))
        .expect("the victim's entry exists")
        .split('}')
        .next()
        .expect("the entry ends");
    let honest_digest = victim_entry
        .split("\"digest\":\"")
        .nth(1)
        .expect("the entry carries a digest")
        .split('"')
        .next()
        .expect("the digest value ends")
        .to_owned();
    assert_eq!(
        normalized_digest, honest_digest,
        "the normalized reforge is a fixpoint of the honest lock under raw digesting — the \
         canonical source is normalization-canonical, so the falsifier's reforged lock \
         cannot even land"
    );
    let raw_swapped_digest = gauntlet_app::skills::digest_raw(&swapped);
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "a normalization-equivalent body is drift even under the honest lock: \
         stdout={stdout}"
    );
    assert!(
        !stdout.contains("no drift, lint clean"),
        "the clean-verify claim is never printed over non-canonical bytes: {stdout}"
    );
    assert!(
        stderr.contains("DRIFTED") && stderr.contains(victim),
        "the drift is typed, naming the skill: {stderr}"
    );
    assert!(
        stderr.contains(&normalized_digest),
        "the refusal names the lock's digest (the canonical source's raw digest): {stderr}"
    );
    assert!(
        stderr.contains(&raw_swapped_digest),
        "the refusal names the installed body's RAW digest: {stderr}"
    );
    assert!(
        !stderr.contains("SOURCE DRIFTED"),
        "the source anchor passes here — the lock names the canonical source's exact \
         digest: {stderr}"
    );

    // SHAPE 2 — the raw reforge: the lock is re-forged over the swapped
    // bytes' RAW digest (internally consistent over the tampered body,
    // count/schema/name set untouched), so the installed half passes and
    // the SOURCE anchor must refuse — naming the skill and both digests.
    reforge_lock(&raw_swapped_digest);
    let raw_source_digest = gauntlet_app::skills::digest_raw(&source_bytes);
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(
        !ok,
        "the raw-reforged lock over the swapped body is drift too: stdout={stdout}"
    );
    assert!(
        stderr.contains("SOURCE DRIFTED") && stderr.contains(victim),
        "the source anchor refuses, naming the skill: {stderr}"
    );
    assert!(
        stderr.contains(&raw_swapped_digest),
        "the refusal names the lock's (forged) digest: {stderr}"
    );
    assert!(
        stderr.contains(&raw_source_digest),
        "the refusal names the source's RAW digest: {stderr}"
    );

    // Re-install repairs from the canonical source, byte-exactly.
    let (stdout, stderr, ok) = world.output(&["install"]);
    assert!(ok, "re-install repairs: {stdout} {stderr}");
    assert_eq!(
        std::fs::read(&installed).expect("the repaired body reads"),
        source_bytes,
        "the repair restored the CANONICAL bytes"
    );
    let (stdout, stderr, ok) = world.output(&["verify"]);
    assert!(ok, "verify is clean after the repair: {stdout} {stderr}");
}
