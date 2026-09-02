//! `gauntlet reference …` (spec §20.1): the reference commands. Plan
//! Task 7 landed `digest` and plan Task 18 `validate`; plan Task 20
//! lands `form`, `freeze`, `show`, `diff`, and `verify` (D8: real
//! backing services, honest depth). Behavior of the pre-existing
//! subcommands is byte-identical to the pre-Task-20 surface.

use std::process::ExitCode;

use gauntlet_ports::id_generator::IdGenerator;

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("digest") => digest(&args[1..]),
        Some("validate") => validate(&args[1..]),
        Some("form") => form(&args[1..]),
        Some("freeze") => freeze(&args[1..]),
        Some("show") => show(&args[1..]),
        Some("diff") => diff(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some(other) => {
            eprintln!("gauntlet reference: unknown subcommand '{other}'");
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
    println!("usage: gauntlet reference digest --path <path>");
    println!("       gauntlet reference validate --path <path>");
    println!("       gauntlet reference form [--path <path>]");
    println!("       gauntlet reference freeze [--run <run-id>]");
    println!("       gauntlet reference show");
    println!("       gauntlet reference diff --path <path> --against <path>");
    println!("       gauntlet reference verify");
}

/// `gauntlet reference digest --path <path>` (plan Task 7): reads the file,
/// applies the application normalization (the `reference-digest.sh` rules,
/// re-implemented typed), prints exactly one line `sha256:<lowercase hex>`
/// and exits 0. A missing, unreadable, or non-UTF-8 path is a stderr
/// message and a non-zero exit — untrusted input is never coerced (the
/// script refuses identically).
fn digest(args: &[String]) -> ExitCode {
    let mut path: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet reference digest: --path requires a path");
                    return ExitCode::from(2);
                };
                if path.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet reference digest: --path given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet reference digest: unknown argument '{other}'");
                println!("usage: gauntlet reference digest --path <path>");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("gauntlet reference digest: --path is required");
        println!("usage: gauntlet reference digest --path <path>");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!(
                "gauntlet reference digest: reading {} failed: {err}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    match gauntlet_app::reference::normalize::digest_of(&bytes) {
        Ok(digest) => {
            println!("{digest}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!(
                "gauntlet reference digest: {} does not digest: {err}",
                path.display()
            );
            ExitCode::FAILURE
        }
    }
}

/// `gauntlet reference validate --path <path>` (plan Task 18 step 4): reads
/// the file and runs it through the shared answer-key validator
/// (`gauntlet_app::reference::validate::validate_answer_key`, the Task 7
/// strict parser reused, not reimplemented) — the formation skill's own
/// closing gate, so a non-conforming key is rejected before it is treated
/// as frozen. On success prints `OK: <n> units` and
/// exits 0; on failure prints the block failure's `Display` to stderr and
/// exits non-zero. Mirrors `digest`'s `--path` arg-parsing style
/// exactly.
fn validate(args: &[String]) -> ExitCode {
    let mut path: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet reference validate: --path requires a path");
                    return ExitCode::from(2);
                };
                if path.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet reference validate: --path given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet reference validate: unknown argument '{other}'");
                println!("usage: gauntlet reference validate --path <path>");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("gauntlet reference validate: --path is required");
        println!("usage: gauntlet reference validate --path <path>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "gauntlet reference validate: reading {} failed: {err}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    match gauntlet_app::reference::validate::validate_answer_key(&text) {
        Ok(units) => {
            println!("OK: {} units", units.len());
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("gauntlet reference validate: {failure}");
            ExitCode::FAILURE
        }
    }
}

/// `gauntlet reference form [--path <path>]` (plan Task 20, D8): the
/// FORMATION entry point. Formation itself is a MODEL process (the
/// wayfinder skill grills an intent into a decision map and a typed
/// answer key) — the command never fabricates a reference. When
/// `--path` names a candidate key it is validated through the shared
/// strict validator (the formation skill's own closing gate, reused);
/// otherwise the command prints the formation route.
fn form(args: &[String]) -> ExitCode {
    let mut path: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet reference form: --path requires a path");
                    return ExitCode::from(2);
                };
                if path.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet reference form: --path given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet reference form: unknown argument '{other}'");
                println!("usage: gauntlet reference form [--path <path>]");
                return ExitCode::from(2);
            }
        }
    }
    println!("reference: formation is a MODEL process — run the wayfinder-gauntlet skill");
    println!("reference: it grills the intent into a decision map and a typed answer key;");
    println!("reference: unresolved questions stay fog, never invented answers");
    if let Some(path) = path {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!(
                    "gauntlet reference form: reading {}: {error}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        };
        return match gauntlet_app::reference::validate::validate_answer_key(&text) {
            Ok(units) => {
                println!(
                    "reference: candidate validates — {} unit(s); freeze it with `gauntlet \
                     reference freeze` when the project's lock should carry it",
                    units.len()
                );
                ExitCode::SUCCESS
            }
            Err(failure) => {
                eprintln!("gauntlet reference form: the candidate does not validate: {failure}");
                ExitCode::FAILURE
            }
        };
    }
    ExitCode::SUCCESS
}

/// `gauntlet reference freeze [--run <run-id>]` (plan Task 20, D8): the
/// standalone freeze — resolve → normalize → digest → freeze through the
/// production `FileReferenceProvider` into the evidence store, printing
/// the frozen digest. Without `--run` a fresh run id is minted (a freeze
/// is bound to a run identity; the runtime's id adapter mints one).
fn freeze(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => match args.get(index + 1) {
                Some(value) => {
                    run_id = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet reference freeze: --run requires a run id");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet reference freeze: unknown argument '{other}'");
                println!("usage: gauntlet reference freeze [--run <run-id>]");
                return ExitCode::from(2);
            }
        }
    }
    let context = match crate::context::resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet reference freeze: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let run_id = match run_id {
        Some(id) => match gauntlet_domain::RunId::new(&id) {
            Ok(run_id) => run_id,
            Err(error) => {
                eprintln!("gauntlet reference freeze: {id} is not a valid run id: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => match gauntlet_adapter_id_entropy::UlidIdGenerator::new() {
            Ok(ids) => ids.run_id(),
            Err(error) => {
                eprintln!("gauntlet reference freeze: could not mint a run id: {error}");
                return ExitCode::FAILURE;
            }
        },
    };
    let evidence =
        match gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir) {
            Ok(store) => store,
            Err(error) => {
                eprintln!("gauntlet reference freeze: {error}");
                return ExitCode::FAILURE;
            }
        };
    let provider = gauntlet_app::reference::provider::FileReferenceProvider::from_paths(
        context.resolved.reference.answer_key.clone(),
        context.resolved.reference.manifest.clone(),
        std::rc::Rc::new(evidence),
    );
    let deadline = crate::context::doctor_bound();
    let outcome = (|| -> Result<String, String> {
        use gauntlet_ports::reference_provider::ReferenceProvider;
        let draft = provider
            .resolve("gauntlet reference freeze", deadline)
            .map_err(|error| error.to_string())?;
        let reference = provider
            .normalize(draft, deadline)
            .map_err(|error| error.to_string())?;
        let version = provider
            .freeze(&run_id, &reference, deadline)
            .map_err(|error| error.to_string())?;
        provider
            .verify_unchanged(&version, deadline)
            .map_err(|error| error.to_string())?;
        Ok(version.digest().to_string())
    })();
    match outcome {
        Ok(digest) => {
            println!("reference: frozen run_id={run_id} digest={digest}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet reference freeze: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `gauntlet reference show` (plan Task 20, D8): parse the project's
/// configured answer key and print the reference summary — the digest,
/// the unit count, and the units by tier and authority (frozen ruler
/// facts, read never invented).
fn show(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet reference show: unknown argument '{other}'");
        println!("usage: gauntlet reference show");
        return ExitCode::from(2);
    }
    let context = match crate::context::resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet reference show: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let key_path = &context.resolved.reference.answer_key;
    let bytes = match std::fs::read(key_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "gauntlet reference show: reading {}: {error}",
                key_path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let digest = match gauntlet_app::reference::normalize::digest_of(&bytes) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!(
                "gauntlet reference show: {} does not digest: {error}",
                key_path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "gauntlet reference show: {} is not UTF-8: {error}",
                key_path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let parsed = gauntlet_app::reference::parse::parse_answer_key(&text);
    if !parsed.failures.is_empty() {
        eprintln!(
            "gauntlet reference show: the answer key carries {} strict-parse failure(s) — \
             `gauntlet reference validate` names them",
            parsed.failures.len()
        );
        return ExitCode::FAILURE;
    }
    let mut by_tier: Vec<(gauntlet_domain::BlastRadius, usize)> = Vec::new();
    let mut by_authority: Vec<(gauntlet_domain::Authority, usize)> = Vec::new();
    for unit in &parsed.units {
        let tier = unit.blast_radius();
        match by_tier.iter_mut().find(|(existing, _)| *existing == tier) {
            Some(entry) => entry.1 += 1,
            None => by_tier.push((tier, 1)),
        }
        let authority = unit.authority();
        match by_authority
            .iter_mut()
            .find(|(existing, _)| *existing == authority)
        {
            Some(entry) => entry.1 += 1,
            None => by_authority.push((authority, 1)),
        }
    }
    println!("reference: path={}", key_path.display());
    println!("reference: digest={digest}");
    println!("reference: units={}", parsed.units.len());
    for (tier, count) in &by_tier {
        println!("reference: tier={tier} units={count}");
    }
    for (authority, count) in &by_authority {
        println!("reference: authority={authority} units={count}");
    }
    ExitCode::SUCCESS
}

/// `gauntlet reference diff --path <path> --against <path>` (plan Task
/// 20, D8): the application digest over BOTH files, both digests
/// printed, and one honest equal/differ line.
fn diff(args: &[String]) -> ExitCode {
    let mut path: Option<std::path::PathBuf> = None;
    let mut against: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => match args.get(index + 1) {
                Some(value) => {
                    if path.replace(std::path::PathBuf::from(value)).is_some() {
                        eprintln!("gauntlet reference diff: --path given twice");
                        return ExitCode::from(2);
                    }
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet reference diff: --path requires a path");
                    return ExitCode::from(2);
                }
            },
            "--against" => match args.get(index + 1) {
                Some(value) => {
                    if against.replace(std::path::PathBuf::from(value)).is_some() {
                        eprintln!("gauntlet reference diff: --against given twice");
                        return ExitCode::from(2);
                    }
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet reference diff: --against requires a path");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet reference diff: unknown argument '{other}'");
                println!("usage: gauntlet reference diff --path <path> --against <path>");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(path), Some(against)) = (path, against) else {
        eprintln!("gauntlet reference diff: --path and --against are both required");
        println!("usage: gauntlet reference diff --path <path> --against <path>");
        return ExitCode::from(2);
    };
    let digest_of = |file: &std::path::Path| -> Result<String, String> {
        let bytes = std::fs::read(file).map_err(|error| error.to_string())?;
        gauntlet_app::reference::normalize::digest_of(&bytes)
            .map(|digest| digest.to_string())
            .map_err(|error| error.to_string())
    };
    let left = match digest_of(&path) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!("gauntlet reference diff: {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let right = match digest_of(&against) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!("gauntlet reference diff: {}: {error}", against.display());
            return ExitCode::FAILURE;
        }
    };
    println!("reference: diff path={} digest={left}", path.display());
    println!(
        "reference: diff against={} digest={right}",
        against.display()
    );
    if left == right {
        println!("reference: diff verdict=equal");
        ExitCode::SUCCESS
    } else {
        println!("reference: diff verdict=differ");
        ExitCode::SUCCESS
    }
}

/// `gauntlet reference verify` (plan Task 20, D8): the typed twin of
/// `just ruler` — the project lock's digests recomputed against the
/// artifacts on disk (`crate::reference_lock`).
fn verify(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet reference verify: unknown argument '{other}'");
        println!("usage: gauntlet reference verify");
        return ExitCode::from(2);
    }
    let context = match crate::context::resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet reference verify: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    match crate::reference_lock::verify_lock(&context.project_root) {
        Ok(report) => {
            println!("reference: verify verdict=unchanged");
            println!(
                "reference: verify answer_key={} digest={}",
                report.answer_key_path, report.answer_key_digest
            );
            println!(
                "reference: verify manifest={} digest={}",
                report.manifest_path, report.manifest_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet reference verify: {error}");
            ExitCode::FAILURE
        }
    }
}
