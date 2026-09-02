//! `gauntlet skills install | verify | list` (spec §20.1; plan Task 20,
//! charter D8): the skills surface MIRRORS the roles surface — the
//! repository's owned skills (`gauntlet_app::skills::REPO_OWNED_SKILLS`,
//! never a directory glob) are installed into the state dir's skill
//! tree with a content-digest lock (`.gauntlet/skills.lock`), `verify`
//! re-reads + re-digests + re-lints (the task19 lint, REUSED — the
//! REPO_OWNED_SKILLS semantics are not grown here), and `list`
//! enumerates the owned skills with provenance presence.
//!
//! COR-R3-2 (falsifier round 3, F-R3-2): verify's anchor is the
//! CANONICAL SOURCE tree (`.agents/skills/**`, the repo-owned reviewed
//! copies) exactly the way roles verify anchors its lock to the roles
//! tree. A shape-valid lock whose digests are self-consistent but bind
//! TAMPERED installed bodies is DRIFT — typed, naming the skill and both
//! digests — never a clean verify; a source edited after install is
//! drift the same way ("source file modified since install" is the
//! roles surface's refusal class). The strict-shape half alone (the
//! reader) cannot see either: both forgeries are internally consistent.
//!
//! COR-R4-2 (falsifier round 4, F-R4-1): every digest on this surface is
//! RAW BYTES — `gauntlet_app::skills::digest_raw`, the roles lock's own
//! instrument — never the reference normalizer. The normalizer folds
//! line endings and trailing blank lines, so a reforged lock over a
//! normalization-equivalent installed body (CRLF, extra trailing blanks)
//! verified "no drift" where roles refuses the identical shape, and a
//! clean verify claimed "no drift" over bytes that were not the
//! canonical bytes. Byte-exact digesting makes the two surfaces
//! discriminate identically and makes the happy-path claim honest: the
//! installed bytes ARE the source bytes. (Install writes the source
//! bytes verbatim, so the honest round trip is unchanged.)

use std::process::ExitCode;

use gauntlet_app::skills::{REPO_OWNED_SKILLS, lint_skill_text};
use gauntlet_ports::json::{JsonNumber, JsonValue};

use crate::context::resolve_context;

/// The skill source directory under the project's agent tree (the
/// roles-dir convention's sibling).
const SKILLS_DIR_NAME: &str = "skills";

/// The lock's file name under the state dir.
const SKILLS_LOCK_NAME: &str = "skills.lock";

/// The lock schema token.
const SKILLS_LOCK_SCHEMA: &str = "gauntlet-skills-lock@v1";

fn source_root(project_root: &std::path::Path) -> std::path::PathBuf {
    project_root
        .join(gauntlet_app::config::defaults::AGENTS_DIR_NAME)
        .join(SKILLS_DIR_NAME)
}

fn installed_root(project_root: &std::path::Path) -> std::path::PathBuf {
    // The installed tree lives under the runtime dir, beside the locks —
    // the same anchoring the roles tree uses.
    project_root
        .join(gauntlet_app::config::defaults::RUNTIME_DIR_NAME)
        .join(SKILLS_DIR_NAME)
}

fn lock_path(project_root: &std::path::Path) -> std::path::PathBuf {
    project_root
        .join(gauntlet_app::config::defaults::RUNTIME_DIR_NAME)
        .join(SKILLS_LOCK_NAME)
}

/// `gauntlet skills <subcommand>`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("install") => install(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("list") => list(&args[1..]),
        Some(other) => {
            eprintln!("gauntlet skills: unknown subcommand '{other}'");
            usage();
            ExitCode::from(2)
        }
        None => {
            usage();
            ExitCode::from(2)
        }
    }
}

fn usage() {
    println!("usage: gauntlet skills install");
    println!("       gauntlet skills verify");
    println!("       gauntlet skills list");
}

/// The digest over one SKILL.md's RAW bytes — the roles lock's own
/// instrument ([`gauntlet_app::skills::digest_raw`]), byte-exact.
/// COR-R4-2: NEVER the reference normalizer — a normalization-equivalent
/// body must be drift, exactly as roles calls it.
fn digest_of_file(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(gauntlet_app::skills::digest_raw(&bytes))
}

/// Reads the lock's (name → digest) pairs; a missing lock is an empty
/// map (install creates it), a malformed lock is a typed refusal.
///
/// COR-R2-3 (falsifier round 2, F-R2-2): the reader is STRICT — on-disk
/// state is untrusted input, the repo's standard strict-codec discipline
/// ("drift is not data"). Enforced here, in order:
///
/// 1. unknown ROOT fields are refused (the field set is closed at
///    `schema` + `count` + `skills`);
/// 2. the `schema` token must be EXACTLY this lock's — the version gate
///    every other document enforces at decode;
/// 3. `skills` must be the root array and every entry must carry
///    exactly `name`+`digest` strings (unknown entry fields refused);
/// 4. `count` must EQUAL the array's length — a declared count is a
///    commitment, never decoration;
/// 5. the locked name set must be EXACTLY the application's owned set
///    (`REPO_OWNED_SKILLS`): a lock that omits an owned skill silently
///    narrows verification coverage and a lock that names a foreign
///    skill verifies something this tree never owned — both refusals,
///    never a narrowed verify; duplicated entries are refused too.
fn read_lock(path: &std::path::Path) -> Result<Vec<(String, String)>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("reading {}: {error}", path.display())),
    };
    let parsed = gauntlet_ports::json::parse(&text).map_err(|error| error.to_string())?;
    let JsonValue::Object(fields) = &parsed else {
        return Err(format!("{} is not a JSON object", path.display()));
    };
    for (key, _) in fields {
        if key != "schema" && key != "count" && key != "skills" {
            return Err(format!(
                "{} carries unknown root field {key:?}",
                path.display()
            ));
        }
    }
    let schema = match parsed.get("schema") {
        Some(JsonValue::Str(text)) => text.clone(),
        other => {
            return Err(format!(
                "{} carries no schema token: {other:?}",
                path.display()
            ));
        }
    };
    if schema != SKILLS_LOCK_SCHEMA {
        return Err(format!(
            "{} carries schema {schema:?}, not {SKILLS_LOCK_SCHEMA:?}",
            path.display()
        ));
    }
    let Some(JsonValue::Array(entries)) = parsed.get("skills") else {
        return Err(format!("{} carries no skills array", path.display()));
    };
    let count = match parsed.get("count") {
        Some(JsonValue::Number(number)) => number
            .as_u64()
            .ok_or_else(|| format!("{} carries a non-integer count", path.display()))?,
        other => return Err(format!("{} carries no count: {other:?}", path.display())),
    };
    if count != entries.len() as u64 {
        return Err(format!(
            "{} declares {count} skill(s) but carries {}",
            path.display(),
            entries.len()
        ));
    }
    let mut pairs = Vec::new();
    for entry in entries {
        let JsonValue::Object(entry_fields) = entry else {
            return Err(format!(
                "{} carries a non-object skill entry",
                path.display()
            ));
        };
        for (key, _) in entry_fields {
            if key != "name" && key != "digest" {
                return Err(format!(
                    "{} carries a skill entry with unknown field {key:?}",
                    path.display()
                ));
            }
        }
        let (Some(JsonValue::Str(name)), Some(JsonValue::Str(digest))) =
            (entry.get("name"), entry.get("digest"))
        else {
            return Err(format!(
                "{} carries a skill entry without name/digest strings",
                path.display()
            ));
        };
        pairs.push((name.clone(), digest.clone()));
    }
    // The owned-set commitment: the lock records EXACTLY the closed
    // owned list — no fewer entries, no foreign ones, no duplicates.
    let locked: std::collections::BTreeSet<&str> =
        pairs.iter().map(|(name, _)| name.as_str()).collect();
    if locked.len() != pairs.len() {
        return Err(format!(
            "{} records a duplicated skill entry",
            path.display()
        ));
    }
    let owned: std::collections::BTreeSet<&str> = REPO_OWNED_SKILLS.iter().copied().collect();
    if locked != owned {
        let missing: Vec<&str> = owned.difference(&locked).copied().collect();
        let foreign: Vec<&str> = locked.difference(&owned).copied().collect();
        return Err(format!(
            "{} does not record the owned skill set exactly (missing {missing:?}, foreign \
             {foreign:?}) — re-install to repair",
            path.display()
        ));
    }
    Ok(pairs)
}

/// `gauntlet skills install`: lints each OWNED skill's source, copies it
/// verbatim into the installed tree, and writes the content-digest lock.
/// Idempotent and drift-repairing: re-installing rewrites drifted
/// installed files from the source (the roles lock's repair semantics),
/// so verify-after-install is clean by construction.
fn install(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet skills install: unknown argument '{other}'");
        println!("usage: gauntlet skills install");
        return ExitCode::from(2);
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet skills: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let source = source_root(&context.project_root);
    let installed = installed_root(&context.project_root);
    let mut entries: Vec<(String, String)> = Vec::new();
    for name in REPO_OWNED_SKILLS {
        let source_skill = source.join(name).join("SKILL.md");
        let text = match std::fs::read_to_string(&source_skill) {
            Ok(text) => text,
            Err(error) => {
                eprintln!(
                    "gauntlet skills install: provenance for '{name}' is missing ({}) — the \
                     skill cannot be installed honestly: {error}",
                    source_skill.display()
                );
                return ExitCode::FAILURE;
            }
        };
        // The task19 lint gates the install (a skill that does not lint
        // is not installed).
        if let Err(error) = lint_skill_text(name, &text) {
            eprintln!("gauntlet skills install: '{name}' failed the skill lint: {error}");
            return ExitCode::FAILURE;
        }
        let installed_dir = installed.join(name);
        if let Err(error) = std::fs::create_dir_all(&installed_dir) {
            eprintln!(
                "gauntlet skills install: creating {}: {error}",
                installed_dir.display()
            );
            return ExitCode::FAILURE;
        }
        let installed_skill = installed_dir.join("SKILL.md");
        if let Err(error) = std::fs::write(&installed_skill, &text) {
            eprintln!(
                "gauntlet skills install: writing {}: {error}",
                installed_skill.display()
            );
            return ExitCode::FAILURE;
        }
        let digest = match digest_of_file(&installed_skill) {
            Ok(digest) => digest,
            Err(error) => {
                eprintln!("gauntlet skills install: {error}");
                return ExitCode::FAILURE;
            }
        };
        entries.push(((*name).to_owned(), digest));
    }
    // The lock: canonical JSON, sorted names, one array AT THE ROOT
    // `skills` key — the exact schema `read_lock` (verify/list's reader)
    // requires. COR-F5 (falsifier round 1, F-6): the writer used to bind
    // the array inside a second nested object
    // (`{"skills":{"skills":[…]}}`), so the documented install → verify
    // round trip ALWAYS failed typed; the WRITER is fixed to emit the
    // reader's schema, never the reader widened to accept both.
    entries.sort();
    let skills = JsonValue::Array(
        entries
            .iter()
            .map(|(name, digest)| {
                JsonValue::Object(vec![
                    ("name".to_owned(), JsonValue::Str(name.clone())),
                    ("digest".to_owned(), JsonValue::Str(digest.clone())),
                ])
            })
            .collect(),
    );
    let lock = lock_path(&context.project_root);
    let body = format!(
        "{}\n",
        JsonValue::Object(vec![
            (
                "schema".to_owned(),
                JsonValue::Str(SKILLS_LOCK_SCHEMA.to_owned())
            ),
            (
                "count".to_owned(),
                JsonValue::Number(JsonNumber::Uint(entries.len() as u64))
            ),
            ("skills".to_owned(), skills),
        ])
        .to_json_text()
    );
    if let Err(error) = std::fs::write(&lock, body) {
        eprintln!(
            "gauntlet skills install: writing {}: {error}",
            lock.display()
        );
        return ExitCode::FAILURE;
    }
    println!(
        "skills: installed {} skill(s) for the local harness (lint: pass)",
        entries.len()
    );
    println!("skills: lock: {}", lock.display());
    ExitCode::SUCCESS
}

/// `gauntlet skills verify`: re-reads the installed tree, re-digests
/// against the lock, re-runs the task19 lint on every installed skill,
/// and — COR-R3-2 — re-digests every skill's CANONICAL SOURCE against the
/// same lock. Any drift is a typed failure and a non-zero exit.
fn verify(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet skills verify: unknown argument '{other}'");
        println!("usage: gauntlet skills verify");
        return ExitCode::from(2);
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet skills: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let lock = lock_path(&context.project_root);
    let recorded = match read_lock(&lock) {
        Ok(pairs) => pairs,
        Err(error) => {
            eprintln!("gauntlet skills verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    if recorded.is_empty() {
        eprintln!(
            "gauntlet skills verify: no installed skills recorded — run `gauntlet skills \
             install` first"
        );
        return ExitCode::FAILURE;
    }
    let installed = installed_root(&context.project_root);
    let source = source_root(&context.project_root);
    let mut drift = 0usize;
    for (name, digest) in &recorded {
        // COR-R3-2: the CONTENT anchor. The lock is runtime-written state;
        // the canonical `.agents/skills` body is the reviewed source it
        // must agree with (the roles lock's source-tree anchor, mirrored).
        // A lock whose digests are internally consistent over a tampered
        // installed body refuses HERE: the lock's digest no longer matches
        // the canonical source, and the refusal names the skill and both
        // digests.
        let source_skill = source.join(name).join("SKILL.md");
        let source_bytes = match std::fs::read(&source_skill) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                eprintln!("gauntlet skills verify: '{name}' source is unreadable: {error}");
                drift += 1;
                continue;
            }
        };
        let Some(source_bytes) = source_bytes else {
            eprintln!(
                "gauntlet skills verify: '{name}' SOURCE is MISSING from the provenance tree \
                 ({}) — the canonical source is the anchor and nothing verifies without it",
                source_skill.display()
            );
            drift += 1;
            continue;
        };
        // COR-R4-2: the source digest is RAW bytes — the roles lock's
        // instrument — never the reference normalizer, so the two
        // surfaces discriminate identically under a reforged lock.
        let source_digest = gauntlet_app::skills::digest_raw(&source_bytes);
        if source_digest != *digest {
            eprintln!(
                "gauntlet skills verify: '{name}' SOURCE DRIFTED from the lock — lock {digest}, \
                 source {source_digest} — the canonical .agents/skills body is the anchor; \
                 re-install to repair"
            );
            drift += 1;
            continue;
        }
        let installed_skill = installed.join(name).join("SKILL.md");
        let text = match std::fs::read_to_string(&installed_skill) {
            Ok(text) => text,
            Err(error) => {
                eprintln!(
                    "gauntlet skills verify: '{name}' is MISSING from the installed tree: \
                     {error}"
                );
                drift += 1;
                continue;
            }
        };
        if let Err(error) = lint_skill_text(name, &text) {
            eprintln!("gauntlet skills verify: '{name}' failed the skill lint: {error}");
            drift += 1;
            continue;
        }
        match digest_of_file(&installed_skill) {
            Ok(current) if current == *digest => {}
            Ok(current) => {
                eprintln!(
                    "gauntlet skills verify: '{name}' DRIFTED — lock {digest}, installed \
                     {current}"
                );
                drift += 1;
            }
            Err(error) => {
                eprintln!("gauntlet skills verify: '{name}' is unreadable: {error}");
                drift += 1;
            }
        }
    }
    if drift > 0 {
        eprintln!(
            "gauntlet skills verify: {drift} of {} skill(s) drifted — re-install to repair",
            recorded.len()
        );
        ExitCode::FAILURE
    } else {
        println!(
            "skills: verified {} skill(s) — no drift, lint clean",
            recorded.len()
        );
        ExitCode::SUCCESS
    }
}

/// `gauntlet skills list`: the owned skills with provenance presence and
/// install state.
fn list(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet skills list: unknown argument '{other}'");
        println!("usage: gauntlet skills list");
        return ExitCode::from(2);
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet skills: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let source = source_root(&context.project_root);
    let installed = installed_root(&context.project_root);
    for name in REPO_OWNED_SKILLS {
        let provenance = source.join(name).join("SKILL.md").is_file();
        let is_installed = installed.join(name).join("SKILL.md").is_file();
        println!(
            "skills: name={name} provenance={} installed={}",
            if provenance { "present" } else { "absent" },
            if is_installed { "yes" } else { "no" }
        );
    }
    ExitCode::SUCCESS
}
