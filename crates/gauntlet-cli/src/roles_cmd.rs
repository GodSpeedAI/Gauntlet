//! `gauntlet roles …` (spec §20.1; plan Task 17, AK-063): installs the
//! repository's role artifacts into the named harness's role surface,
//! or re-reads the installed state and reports drift. Behavior is
//! byte-identical to the pre-Task-20 surface; only the module home
//! moved (the thin-library refactor), and the root-discovery plumbing
//! is the shared [crate::context] module now.

use std::process::ExitCode;

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("install") => install(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("list") => list(&args[1..]),
        Some(other) => {
            eprintln!("gauntlet roles: unknown subcommand '{other}'");
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
    println!("usage: gauntlet roles install --harness <id>");
    println!("       gauntlet roles verify");
    println!("       gauntlet roles list");
}

/// The harness registry this composition fronts (plan Task 17 D4): keyed
/// by the runner adapters' exported token constants — the only place a
/// harness id spelling belongs. The recursive harness offers a writable
/// role surface (its subagent-spec surface, Annex C.4); the generic CLI
/// runner has no writable surface, so its roles are per-invocation
/// prompt-side injections with enforcement recorded as advisory.
fn registry() -> gauntlet_app::roles::HarnessRegistry {
    use gauntlet_adapter_agent_cli as generic_cli_runner;
    use gauntlet_adapter_agent_prime as recursive_runner;
    use gauntlet_app::roles::{HarnessRegistry, RoleSurface};

    let mut registry = HarnessRegistry::new();
    registry
        .insert(recursive_runner::RUNNER_TOKEN, RoleSurface::WritableRoleDir)
        .expect("the recursive runner token is a valid registry id");
    registry
        .insert(
            generic_cli_runner::RUNNER_TOKEN,
            RoleSurface::AdvisoryPromptArtifacts,
        )
        .expect("the generic CLI runner token is a valid registry id");
    registry
}

/// `gauntlet roles install --harness <id>`: materializes the role files
/// verbatim under the state root and writes `.gauntlet/roles.lock`.
/// Re-installing an unchanged tree is a byte-identical lock; an
/// unregistered or missing harness id fails closed (no runner is
/// selected by default — that is the operator's call).
fn install(args: &[String]) -> ExitCode {
    let mut harness: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--harness" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet roles install: --harness requires an id");
                    return ExitCode::from(2);
                };
                if harness.replace(value.clone()).is_some() {
                    eprintln!("gauntlet roles install: --harness given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet roles install: unknown argument '{other}'");
                println!("usage: gauntlet roles install --harness <id>");
                return ExitCode::from(2);
            }
        }
    }
    let Some(harness) = harness else {
        eprintln!("gauntlet roles install: --harness is required");
        println!("usage: gauntlet roles install --harness <id>");
        return ExitCode::from(2);
    };
    let Ok(context) = crate::context::resolve_context(None) else {
        return report_context_failure();
    };
    let registry = registry();
    match gauntlet_app::roles::install(&gauntlet_app::roles::InstallInput {
        project_root: &context.project_root,
        state_dir: context.state_dir(),
        roles_dir: context.roles_dir(),
        harness: &harness,
        registry: &registry,
        env: &context.env,
        tool_vocabulary: crate::spec_vocab::CAPABILITY_VOCABULARY,
    }) {
        Ok(report) => {
            let enforcement = match report.enforcement {
                gauntlet_app::roles::Enforcement::Writable => "writable",
                gauntlet_app::roles::Enforcement::Advisory => "advisory",
            };
            println!(
                "roles: installed {} role(s) for harness '{}' (enforcement: {enforcement})",
                report.roles, report.harness
            );
            if report.repaired {
                println!("roles: this install repaired drifted state");
            }
            println!("roles: lock: {}", report.lock_path.display());
            if report.enforcement == gauntlet_app::roles::Enforcement::Advisory {
                println!(
                    "roles: advisory — this harness has no writable role surface; role bodies \
                     are injected per invocation and enforcement is recorded as advisory"
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet roles install: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `gauntlet roles verify`: re-reads the installed state and the
/// repository sources, comparing content digests against the lock. Any
/// drift — out-of-band edit, source edit since install, a missing or
/// extra artifact, or a hand-edited or re-forged lock — is a typed
/// failure and a non-zero exit. The command takes NO arguments (COR-D12):
/// every surface in this CLI refuses unknown arguments with exit 2, so a
/// mistyped flag (`--harness X`) is a usage error, never a silent no-op.
fn verify(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet roles verify: unknown argument '{other}'");
        println!("usage: gauntlet roles verify");
        return ExitCode::from(2);
    }
    let Ok(context) = crate::context::resolve_context(None) else {
        return report_context_failure();
    };
    let registry = registry();
    match gauntlet_app::roles::verify(&gauntlet_app::roles::VerifyInput {
        project_root: &context.project_root,
        state_dir: context.state_dir(),
        roles_dir: context.roles_dir(),
        registry: &registry,
    }) {
        Ok(report) => {
            let enforcement = match report.enforcement {
                gauntlet_app::roles::Enforcement::Writable => "writable",
                gauntlet_app::roles::Enforcement::Advisory => "advisory",
            };
            println!(
                "roles: verified {} role(s) for harness '{}' (enforcement: {enforcement}) — no \
                 drift",
                report.roles, report.harness
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet roles verify: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `gauntlet roles list` (§20.1's `roles list`; COR-D1): a read-only
/// enumeration of the INSTALLED roles from the roles lock, strict-parsed —
/// a tampered or re-forged lock is the same typed refusal `verify` raises,
/// never a forged inventory. It checks nothing against reality (that is
/// `verify`'s job) and takes NO arguments, like every read surface here.
fn list(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet roles list: unknown argument '{other}'");
        println!("usage: gauntlet roles list");
        return ExitCode::from(2);
    }
    let Ok(context) = crate::context::resolve_context(None) else {
        return report_context_failure();
    };
    match gauntlet_app::roles::list(&context.project_root) {
        Ok(report) => {
            let enforcement = match report.enforcement {
                gauntlet_app::roles::Enforcement::Writable => "writable",
                gauntlet_app::roles::Enforcement::Advisory => "advisory",
            };
            println!(
                "roles: harness '{}' (enforcement: {enforcement}) — {} installed role(s)",
                report.harness,
                report.roles.len()
            );
            for role in &report.roles {
                let dimension = role.dimension.as_deref().unwrap_or("-");
                println!(
                    "roles: installed role={} role_id={} dimension={dimension}",
                    role.role, role.role_id
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet roles list: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Reports a context-resolution failure (config unreadable or the merge
/// refused) without echoing any environment value.
fn report_context_failure() -> ExitCode {
    eprintln!("gauntlet roles: the configuration did not resolve (see gauntlet doctor)");
    ExitCode::FAILURE
}
