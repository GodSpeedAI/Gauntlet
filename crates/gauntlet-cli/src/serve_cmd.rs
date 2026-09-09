//! `gauntlet serve` (spec §20.2; plan Task 21, charter D9): the §20.2
//! JSON operator surface, compiled ONLY under the `surfaces` feature.
//! The bind comes from `GAUNTLET_API_BIND` (Annex E): unset, empty, or
//! unparsable is a typed fail-closed configuration error; a NON-loopback
//! bind is refused (the surface serves the local operator only). The
//! surface is additional observability, never load-bearing (§21.3): it
//! writes nothing of the run state itself — actions walk the ONE
//! operator-action path through the settlement service — and a surface
//! failure never crashes a run.
//!
//! In the DEFAULT build this command does not exist: `gauntlet serve` is
//! an unknown command there (exit 2), and the frozen `cli_help.txt`
//! snapshot stays byte-identical (charter D1/D9).

use std::process::ExitCode;

use gauntlet_adapter_surface_json::SurfaceHttp;

use crate::context::resolve_context;
use crate::durable_surface::DurableSurface;

/// The Annex E configuration variable the bind is read from.
const BIND_ENV: &str = "GAUNTLET_API_BIND";

/// `gauntlet serve`.
pub fn serve(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet serve: unknown argument '{other}'");
        println!("usage: gauntlet serve");
        return ExitCode::from(2);
    }
    // Fail-closed configuration: the bind is REQUIRED, typed when absent
    // or unusable — the surface never guesses an address.
    let bind = match std::env::var(BIND_ENV) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => {
            eprintln!(
                "gauntlet serve: {BIND_ENV} is set but empty — the surface refuses to guess an \
                 address; set it to a loopback host:port"
            );
            return ExitCode::FAILURE;
        }
        Err(std::env::VarError::NotPresent) => {
            eprintln!(
                "gauntlet serve: {BIND_ENV} is not set — the surface refuses to guess an \
                 address; set it to a loopback host:port"
            );
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("gauntlet serve: {BIND_ENV} is not readable: {error}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match SurfaceHttp::bind(&bind) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("gauntlet serve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet serve: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match DurableSurface::open(context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet serve: {error}");
            return ExitCode::FAILURE;
        }
    };
    // The ONE durable surface behind all four seams: reads and the
    // disclosure share its composition; the action handler IS the shared
    // operator-action path (charter D3).
    let mut server = SurfaceHttp::new(
        Box::new(surface.clone()),
        Box::new(surface.clone()),
        Box::new(surface.clone()),
        Box::new(surface),
    );
    if let Ok(token) =
        std::env::var("GAUNTLET_API_TOKEN").or_else(|_| std::env::var("GAUNTLET_SESSION_TOKEN"))
    {
        let trimmed_token = token.trim();
        if !trimmed_token.is_empty() {
            let actor =
                std::env::var("GAUNTLET_SESSION_ACTOR").unwrap_or_else(|_| "operator".to_owned());
            server = server.with_authenticator(Box::new(
                gauntlet_adapter_surface_json::action::TokenSessionAuthenticator::new(
                    trimmed_token,
                    actor.trim(),
                ),
            ));
        }
    }
    let addr = match SurfaceHttp::local_addr(&listener) {
        Ok(addr) => addr,
        Err(error) => {
            eprintln!("gauntlet serve: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("serve: listening on {addr} — the §20.2 operator surface (loopback only)");
    println!("serve: serving until interrupted; a surface failure never touches the run state");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    server.serve(listener);
    ExitCode::SUCCESS
}
