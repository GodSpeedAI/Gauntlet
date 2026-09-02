//! `gauntlet init` (spec §20.1; plan Task 20, charter D8): scaffolds a
//! project so the other commands have something to resolve. HONEST and
//! BOUNDED — it writes a minimal GAUNTLET.md template whose values are
//! comments (never fabricated policy), a minimal `.env.example` pointing
//! at the runtime checkout's Annex E mirror, and the `.gauntlet`
//! directory skeleton. It REFUSES to overwrite any existing file: an
//! existing project is never silently rewritten.

use std::process::ExitCode;

/// The scaffold's env-example file name (single-segment by intent; the
/// defaults module pins `.env` itself, not the example mirror).
const ENV_EXAMPLE_NAME: &str = ".env.example";

/// `gauntlet init`.
pub fn run(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet init: unknown argument '{other}'");
        println!("usage: gauntlet init");
        return ExitCode::from(2);
    }
    let project_root = match crate::context::resolve_context(None) {
        Ok(context) => context.project_root,
        Err(error) => {
            // Init runs BEFORE a project has a resolvable config; only a
            // genuinely broken environment (unreadable dotenv) surfaces.
            eprintln!("gauntlet init: the project root could not be resolved: {error}");
            return ExitCode::FAILURE;
        }
    };
    let gauntlet_md = project_root.join(gauntlet_app::config::defaults::GAUNTLET_MD_NAME);
    let env_example = project_root.join(ENV_EXAMPLE_NAME);
    let runtime_dir = project_root.join(gauntlet_app::config::defaults::RUNTIME_DIR_NAME);
    for existing in [&gauntlet_md, &env_example] {
        if existing.exists() {
            eprintln!(
                "gauntlet init: {} already exists — refusing to overwrite an existing project \
                 (remove it first if you mean to re-scaffold)",
                existing.display()
            );
            return ExitCode::FAILURE;
        }
    }
    if let Err(error) = std::fs::create_dir_all(&runtime_dir) {
        eprintln!("gauntlet init: creating {}: {error}", runtime_dir.display());
        return ExitCode::FAILURE;
    }
    if let Err(error) = std::fs::write(&gauntlet_md, GAUNTLET_MD_TEMPLATE) {
        eprintln!("gauntlet init: writing {}: {error}", gauntlet_md.display());
        return ExitCode::FAILURE;
    }
    if let Err(error) = std::fs::write(&env_example, ENV_EXAMPLE_TEMPLATE) {
        eprintln!("gauntlet init: writing {}: {error}", env_example.display());
        return ExitCode::FAILURE;
    }
    println!(
        "init: scaffolded {} ({} + {} + {})",
        project_root.display(),
        gauntlet_app::config::defaults::GAUNTLET_MD_NAME,
        ENV_EXAMPLE_NAME,
        gauntlet_app::config::defaults::RUNTIME_DIR_NAME
    );
    println!("init: the template fabricates no policy — fill runners.default before a real run");
    println!(
        "init: the full environment-variable mirror lives in the runtime checkout's \
         .env.example (spec Annex E)"
    );
    ExitCode::SUCCESS
}

/// The minimal GAUNTLET.md template: comments only — the operator's
/// decisions (runners.default among them, UNK-003) stay theirs.
const GAUNTLET_MD_TEMPLATE: &str = "\
# GAUNTLET.md — the four-layer configuration's project layer (spec §8.2).
# Precedence: defaults < this file < environment < flags. Every key below
# is OPTIONAL and commented out; a value here overrides the §8.6 default.
#
# The default runner (REQUIRED for a real run; unset refuses dispatch):
#   runners.default = mock-agent | prime-agent | cli-agent
# Reference paths (defaults resolve to the wayfinder layout):
#   [reference]
#   answer_key = .wayfinder/answer-key.md
#   manifest = .wayfinder/reference.yaml
# Observation facets (test | build | filesystem | versioning):
#   [observation]
#   facets = [test]
# Budget ceilings (§8.6 defaults apply unless overridden):
#   [budget]
#   max_rounds = 8
#   max_cost_usd = 25
";

/// The minimal `.env.example`: a pointer, not a duplicate of Annex E —
/// the runtime checkout's `.env.example` is the pinned full mirror.
const ENV_EXAMPLE_TEMPLATE: &str = "\
# GAUNTLET_* environment overrides (spec Annex E). Copy to .env for local
# values; never commit real values — secrets are referenced by
# indirection, never stored inline (spec §8.1). The full variable mirror
# lives in the runtime checkout's .env.example.
GAUNTLET_AGENT_RUNNER=
GAUNTLET_STATE_DIR=
";
