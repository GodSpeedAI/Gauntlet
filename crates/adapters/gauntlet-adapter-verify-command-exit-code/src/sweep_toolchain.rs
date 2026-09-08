//! The conformance sweep's process toolchain (task 27, B1; COR-1): the
//! ONE place the sweep spells tool products — the repository's test
//! runner invocation and its compile-lock discipline — per spec §8.4
//! (the runtime names capabilities; products live in adapters) and the
//! frozen AK-016 literal boundary that keeps exactly that rule
//! mechanical in every core crate.
//!
//! Pure composition, no process spawning, no dependencies beyond std:
//! the composition root (the CLI) maps the application layer's
//! STRUCTURAL sweep command onto one of these invocations and runs it.
//! The application layer never sees a tool name, and the core crates
//! never carry one.
//!
//! The compile-lock wrapper encodes the repository's documented
//! discipline: the lock acquisition is bounded (`-w`), a lock that
//! could not be acquired exits with [`LOCK_BUSY_EXIT`] and a lock
//! wrapper that could not execute its command exits with
//! [`LOCK_EXEC_FAILED_EXIT`] — both FLOCK-LEVEL outcomes the caller
//! classifies as environmental (INCONCLUSIVE), never command failures
//! — and the compiler cache daemon is stopped INSIDE the same locked
//! command with the wrapped command's exit status preserved across it
//! (the task-20 lesson: a daemon holding the lock fd; the task-23
//! lesson: never nest flocks — this wrapper is the only lock layer).
//!
//! # Reserved exit codes (COR-1 + COR-4 R2-F1/R2-F2's contract)
//!
//! flock itself, not the wrapped command, produces 86 (via `-E`) and 69
//! (it could not execute the command — the COR-1 defect's observed
//! signature). So the caller can tell a flock-level outcome from an
//! inner command's own exit, the wrapper script REMAPS the inner
//! command's own reserved codes before exiting:
//!
//! - inner 69 → [`INNER_EXIT_REMAP_69`] (169)
//! - inner 86 → [`INNER_EXIT_REMAP_86`] (186)
//! - inner 129..159 (the shell's STANDARD signal encoding: the inner
//!   command was KILLED by signal n-128 — the realistic OOM victim,
//!   COR-4 R2-F2) → [`INNER_SIGNAL_BASE`] + (n - 128) (200..231,
//!   outside every flock code); the CLI classifies that band as the
//!   typed environmental refusal naming the signal. The band is
//!   deliberately 129..159, NOT the full 128..191: the remapped 169/186
//!   live inside 128..191, so a wider band would consume a genuine
//!   inner 169/186 (executed: 169 → 241) — standard signals 1..31 are
//!   exactly 129..159, and a real-time-signal death (160..192) is a
//!   disclosed residual that stays a FAIL (the failure direction).
//!
//! The reserved set is therefore {86, 69} = lock-level; {169, 186} =
//! inner-remapped; 200..231 = inner signal death. A genuine inner exit
//! of 169/186 (or 200..231) passing through is INDISTINGUISHABLE from
//! the remapped forms — disclosed in the report's `_meta`; the failure
//! direction stays honest (169/186 still FAIL, 200..231 still claims
//! nothing as INCONCLUSIVE).
//!
//! # Argument order (COR-1's executed defect)
//!
//! The wrapper argv is `flock -w <secs> -E <code> <lockfile> -c
//! <script>` — OPTIONS BEFORE the file operand. File-first makes flock
//! treat the token after the file as the command to execute; the first
//! real sweep executed exactly that defect (45/45 commands dead in
//! ~50ms, `flock: failed to execute -w`, exit 69 —
//! `.agents/reports/conformance-evidence/20260907T193758Z/01-*.log`).

/// The exit code a lock acquisition failure is reported through (chosen
/// away from common command exits so the caller can classify it).
pub const LOCK_BUSY_EXIT: i32 = 86;

/// The exit code flock itself reports when it could not execute the
/// command (observed live in the COR-1 evidence). Flock-level, like
/// [`LOCK_BUSY_EXIT`]: environmental, never a command failure.
pub const LOCK_EXEC_FAILED_EXIT: i32 = 69;

/// What the wrapper script remaps the inner command's OWN 69 to, so the
/// pair {86, 69} stays unambiguously flock-level.
pub const INNER_EXIT_REMAP_69: i32 = 169;

/// What the wrapper script remaps the inner command's OWN 86 to (COR-4
/// R2-F1: the remap was half-built — only 69 was remapped, so a genuine
/// inner 86 was misattributed as lock-busy INCONCLUSIVE).
pub const INNER_EXIT_REMAP_86: i32 = 186;

/// The base of the inner-signal band (COR-4 R2-F2): the shell encodes a
/// signal-killed child as 128+n; the wrapper remaps that band to
/// 200+signal so the CLI can classify it as the typed environmental
/// refusal naming the signal. 200..231 stays clear of every flock code.
pub const INNER_SIGNAL_BASE: i32 = 200;

/// How long the wrapper waits for the repository compile lock before
/// reporting the environmental refusal, in seconds.
pub const LOCK_WAIT_SECONDS: &str = "3600";

/// The repository-wide compile lock this toolchain serializes against.
pub const COMPILE_LOCK: &str = "/tmp/gauntlet-build.lock";

/// How long the wrapper's compiler-cache daemon stop may take before it
/// is killed (COR-4 R2-F3: with NO daemon live the bare stop stalls
/// ~2m14s — lock-held — which the committed sweep's own evidence shows
/// as a ~134s floor per command; bounded, and its exit is swallowed).
pub const DAEMON_STOP_TIMEOUT: &str = "10";

/// One runnable process invocation: program, arguments, and the
/// canonical display the sweep records for it.
pub struct ToolInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
}

/// How a wrapped command's exit code classifies (COR-4: the mapping the
/// CLI executor applies to a LOCKED arm's exit — a pure function so the
/// contract is testable without running cargo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitClassification {
    /// The lock stayed busy past the bounded wait — environmental.
    LockBusy,
    /// The lock wrapper could not execute the command — environmental.
    LockExecFailed,
    /// The inner command FAILED on its own merits: the code the CLI
    /// observed and the inner code it stands for (identical for a plain
    /// code; 169 stands for inner 69 and 186 for inner 86 — a direct
    /// inner 169/186 is indistinguishable, disclosed).
    InnerFailure { observed: i32, inner: i32 },
    /// The inner command was KILLED by a signal (the 200..231 band) —
    /// environmental, naming the signal.
    InnerSignal { signal: i32 },
}

/// Classifies a locked arm's observed exit code (see the type's docs).
pub fn classify_exit(observed: i32) -> ExitClassification {
    match observed {
        LOCK_BUSY_EXIT => ExitClassification::LockBusy,
        LOCK_EXEC_FAILED_EXIT => ExitClassification::LockExecFailed,
        INNER_EXIT_REMAP_69 => ExitClassification::InnerFailure {
            observed,
            inner: LOCK_EXEC_FAILED_EXIT,
        },
        INNER_EXIT_REMAP_86 => ExitClassification::InnerFailure {
            observed,
            inner: LOCK_BUSY_EXIT,
        },
        code if (INNER_SIGNAL_BASE..INNER_SIGNAL_BASE + 32).contains(&code) => {
            ExitClassification::InnerSignal {
                signal: code - INNER_SIGNAL_BASE,
            }
        }
        code => ExitClassification::InnerFailure {
            observed: code,
            inner: code,
        },
    }
}

/// The repository test runner's invocation for one allowlisted package
/// and target selection, under the repository's configured parallelism
/// (never raised — the per-target filters have no justfile equivalent,
/// the raw tool is the honest spelling; recorded in the report's
/// `_meta`).
pub fn nextest_invocation(
    package: &str,
    target: Option<&str>,
    lib: bool,
    filter: Option<&str>,
    features: &[&str],
) -> ToolInvocation {
    let mut args = vec![
        "nextest".to_owned(),
        "run".to_owned(),
        "-p".to_owned(),
        package.to_owned(),
    ];
    // COR-2: the feature edges are part of the command — a target
    // compiled only under a feature config runs ZERO tests under the
    // default config (nextest exit 4). `--features` rides with the
    // cargo options, directly after the package (verified empirically:
    // cargo parses and resolves the flag in this position; a bogus edge
    // fails at metadata stage naming the feature).
    if !features.is_empty() {
        args.push("--features".to_owned());
        args.push(features.join(","));
    }
    if lib {
        args.push("--lib".to_owned());
    }
    if let Some(target) = target {
        args.push("--test".to_owned());
        args.push(target.to_owned());
    }
    if let Some(filter) = filter {
        args.push(filter.to_owned());
    }
    // The display is program + args (args already carry the subcommand
    // and verb), so the recorded spelling is exactly what runs.
    let mut display = String::from("cargo");
    for arg in &args {
        display.push(' ');
        display.push_str(arg);
    }
    ToolInvocation {
        program: "cargo".to_owned(),
        args,
        display,
    }
}

/// Wraps an invocation in the repository's compile-lock discipline over
/// the shared compile lock with the production bounded wait.
pub fn lock_wrapped(inner: &ToolInvocation) -> ToolInvocation {
    lock_wrapped_over(COMPILE_LOCK, LOCK_WAIT_SECONDS, inner)
}

/// Wraps an invocation in the compile-lock discipline over an explicit
/// lock path and bounded wait — the seam the BEHAVIORAL tooth uses (a
/// REAL flock over a TEMP lock with a short bound; the production
/// spelling delegates above). Options come BEFORE the file operand
/// (COR-1). The script, in order: the inner command runs; its status is
/// captured; the compiler-cache daemon stop is BOUNDED and swallowed
/// (COR-4 R2-F3: an unbounded stop stalls ~2m14s lock-held when no
/// daemon is live); the inner signal band remaps FIRST (a remapped
/// 169/186 would otherwise fall inside it), then the inner own-69/86
/// remap so the lock-level pair {86, 69} stays unambiguous.
pub fn lock_wrapped_over(lock: &str, wait: &str, inner: &ToolInvocation) -> ToolInvocation {
    let mut command = String::new();
    // Shell-quote nothing: the inner invocation's pieces are structural
    // values (allowlisted package, plain tokens) — there is nothing to
    // quote, and the display is derived the same way.
    command.push_str(&inner.program);
    for arg in &inner.args {
        command.push(' ');
        command.push_str(arg);
    }
    let script = format!(
        "{command}; __sweep_status=$?; \
         timeout {DAEMON_STOP_TIMEOUT} sccache --stop-server >/dev/null 2>&1 || true; \
         if [ \"$__sweep_status\" -ge 129 ] && [ \"$__sweep_status\" -le 159 ]; then \
         __sweep_status=$((__sweep_status - 128 + {INNER_SIGNAL_BASE})); fi; \
         if [ \"$__sweep_status\" -eq 69 ]; then __sweep_status={INNER_EXIT_REMAP_69}; fi; \
         if [ \"$__sweep_status\" -eq 86 ]; then __sweep_status={INNER_EXIT_REMAP_86}; fi; \
         exit \"$__sweep_status\""
    );
    ToolInvocation {
        program: "flock".to_owned(),
        args: vec![
            "-w".to_owned(),
            wait.to_owned(),
            "-E".to_owned(),
            LOCK_BUSY_EXIT.to_string(),
            lock.to_owned(),
            "-c".to_owned(),
            script,
        ],
        display: inner.display.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The composition's argument ORDER, pinned (COR-1: the file-first
    /// order executed as a defect — flock treated `-w` as the command).
    #[test]
    fn the_lock_wrapper_puts_options_before_the_file_operand() {
        let inner = nextest_invocation("gauntlet-cli", Some("p5_hard_gate"), false, None, &[]);
        let wrapped = lock_wrapped(&inner);
        assert_eq!(wrapped.program, "flock");
        // Options first: -w <wait>, -E <code>, THEN the lock file, -c,
        // and the script.
        assert_eq!(wrapped.args[0], "-w");
        assert_eq!(wrapped.args[1], LOCK_WAIT_SECONDS);
        assert_eq!(wrapped.args[2], "-E");
        assert_eq!(wrapped.args[3], "86");
        assert_eq!(wrapped.args[4], COMPILE_LOCK);
        assert_eq!(wrapped.args[5], "-c");
        let script = &wrapped.args[6];
        // The exit status is preserved across the (bounded, swallowed)
        // daemon stop; the inner signal band remaps FIRST, then the
        // inner own-69/86 remaps keep {86, 69} flock-reserved (COR-4).
        assert!(script.contains("__sweep_status=$?"), "{script}");
        assert!(script.contains("exit \"$__sweep_status\""), "{script}");
        assert!(
            script.contains("timeout 10 sccache --stop-server"),
            "the daemon stop is bounded: {script}"
        );
        assert!(script.contains("|| true"), "{script}");
        assert!(script.contains("-eq 69"), "{script}");
        assert!(script.contains("-eq 86"), "{script}");
        assert!(script.contains("__sweep_status=169"), "{script}");
        assert!(script.contains("__sweep_status=186"), "{script}");
        assert!(script.contains("-ge 129"), "{script}");
        assert!(script.contains("-le 159"), "{script}");
        // The band remap comes BEFORE the reserved-code remaps (a fresh
        // 169 would otherwise fall inside a wider band).
        let band = script.find("-ge 129").expect("band check present");
        let reserved = script.find("-eq 69").expect("reserved remap present");
        assert!(band < reserved, "band remap precedes the 69/86 remaps");
        // The recorded display stays the REAL command, not the wrapper.
        assert_eq!(wrapped.display, inner.display);
    }

    /// COR-4: the classification table the CLI executor applies to a
    /// LOCKED arm's observed exit (pure, so it is testable without
    /// cargo).
    #[test]
    fn the_exit_classification_table_holds() {
        assert_eq!(classify_exit(86), ExitClassification::LockBusy);
        assert_eq!(classify_exit(69), ExitClassification::LockExecFailed);
        assert_eq!(
            classify_exit(169),
            ExitClassification::InnerFailure {
                observed: 169,
                inner: 69
            }
        );
        assert_eq!(
            classify_exit(186),
            ExitClassification::InnerFailure {
                observed: 186,
                inner: 86
            }
        );
        for signal in [1, 9, 15, 31] {
            assert_eq!(
                classify_exit(200 + signal),
                ExitClassification::InnerSignal { signal },
                "signal {signal}"
            );
        }
        assert_eq!(
            classify_exit(1),
            ExitClassification::InnerFailure {
                observed: 1,
                inner: 1
            }
        );
        assert_eq!(
            classify_exit(4),
            ExitClassification::InnerFailure {
                observed: 4,
                inner: 4
            }
        );
        // The band endpoints: 200..231 classify as signals; 232 and 199
        // are plain failures.
        assert_eq!(
            classify_exit(231),
            ExitClassification::InnerSignal { signal: 31 }
        );
        assert_eq!(
            classify_exit(232),
            ExitClassification::InnerFailure {
                observed: 232,
                inner: 232
            }
        );
        assert_eq!(
            classify_exit(199),
            ExitClassification::InnerFailure {
                observed: 199,
                inner: 199
            }
        );
    }

    #[test]
    fn a_targeted_invocation_carries_the_repository_spelling() {
        let invocation = nextest_invocation("gauntlet-cli", Some("p5_hard_gate"), false, None, &[]);
        assert_eq!(invocation.program, "cargo");
        assert_eq!(
            invocation.args,
            vec![
                "nextest",
                "run",
                "-p",
                "gauntlet-cli",
                "--test",
                "p5_hard_gate"
            ]
        );
        assert_eq!(
            invocation.display,
            "cargo nextest run -p gauntlet-cli --test p5_hard_gate"
        );
    }

    #[test]
    fn a_lib_invocation_with_a_filter_spells_both() {
        let invocation =
            nextest_invocation("gauntlet-app", None, true, Some("reference::parse"), &[]);
        assert_eq!(
            invocation.args,
            vec![
                "nextest",
                "run",
                "-p",
                "gauntlet-app",
                "--lib",
                "reference::parse"
            ]
        );
    }

    /// COR-2: the feature edges ride with the cargo options, directly
    /// after the package and BEFORE the target selection — the placement
    /// verified empirically against the real tool.
    #[test]
    fn feature_edges_ride_with_the_cargo_options_before_the_target() {
        let invocation = nextest_invocation(
            "gauntlet-cli",
            Some("operator_actions_no_bypass"),
            false,
            None,
            &["surfaces"],
        );
        assert_eq!(
            invocation.args,
            vec![
                "nextest",
                "run",
                "-p",
                "gauntlet-cli",
                "--features",
                "surfaces",
                "--test",
                "operator_actions_no_bypass"
            ]
        );
        assert_eq!(
            invocation.display,
            "cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass"
        );
        // Multiple edges join into one comma-separated flag value.
        let joined = nextest_invocation(
            "gauntlet-cli",
            Some("operator_actions_no_bypass"),
            false,
            None,
            &["surfaces", "other"],
        );
        assert_eq!(joined.args[4], "--features");
        assert_eq!(joined.args[5], "surfaces,other");
    }

    // ------------------------------------------------------------------
    // The behavioral tooth (COR-1 defect 3): EXECUTE the real flock over
    // the real wrapper composition and observe the exit-code contract.
    // The shape test above once enshrined the file-first defect
    // verbatim; these legs prove the composition RUNS.
    // ------------------------------------------------------------------

    fn run(invocation: &ToolInvocation) -> std::process::Output {
        std::process::Command::new(&invocation.program)
            .args(&invocation.args)
            .output()
            .expect("flock executes the wrapped invocation")
    }

    fn inner(program: &str) -> ToolInvocation {
        ToolInvocation {
            program: program.to_owned(),
            args: Vec::new(),
            display: program.to_owned(),
        }
    }

    /// A tiny executable that exits with a chosen code — the stand-in
    /// for a sweep command that fails on its own merits.
    fn inner_exiting(code: i32) -> ToolInvocation {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-sweep-tooth-exit-{code}-{}",
            std::process::id()
        ));
        std::fs::write(&path, format!("#!/bin/sh\nexit {code}\n")).expect("stub write");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("stub is made executable");
        ToolInvocation {
            program: path.to_string_lossy().to_string(),
            args: Vec::new(),
            display: path.to_string_lossy().to_string(),
        }
    }

    /// A tiny executable that kills ITSELF with a signal — the OOM
    /// victim's shape (the inner command dies, the wrapper lives).
    fn inner_killing(signal: i32) -> ToolInvocation {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-sweep-tooth-kill-{signal}-{}",
            std::process::id()
        ));
        std::fs::write(&path, format!("#!/bin/sh\nkill -{signal} $$\n")).expect("stub write");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("stub is made executable");
        ToolInvocation {
            program: path.to_string_lossy().to_string(),
            args: Vec::new(),
            display: path.to_string_lossy().to_string(),
        }
    }

    fn temp_lock(tag: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "gauntlet-sweep-tooth-{tag}-{}.lock",
                std::process::id()
            ))
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn the_wrapper_executes_the_inner_command_under_the_real_flock() {
        let lock = temp_lock("exec");
        let _ = std::fs::remove_file(&lock);
        // A passing inner command exits 0 THROUGH the wrapper.
        let ok = lock_wrapped_over(&lock, "5", &inner("/bin/true"));
        let output = run(&ok);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stdout {:?} stderr {:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // A failing inner command's exit is preserved across the daemon
        // stop (never averaged, never masked).
        let bad = lock_wrapped_over(&lock, "5", &inner("/bin/false"));
        assert_eq!(run(&bad).status.code(), Some(1));
        let _ = std::fs::remove_file(&lock);
    }

    #[test]
    fn the_inner_own_exit_69_is_remapped_so_the_reserved_pair_stays_flock_level() {
        let lock = temp_lock("remap");
        let _ = std::fs::remove_file(&lock);
        let inner_69 = inner_exiting(69);
        let wrapped = lock_wrapped_over(&lock, "5", &inner_69);
        assert_eq!(
            run(&wrapped).status.code(),
            Some(INNER_EXIT_REMAP_69),
            "the inner command's own 69 surfaces as the remap code"
        );
        // And an unrelated inner code passes through untouched.
        let inner_7 = inner_exiting(7);
        let wrapped = lock_wrapped_over(&lock, "5", &inner_7);
        assert_eq!(run(&wrapped).status.code(), Some(7));
        let _ = std::fs::remove_file(&lock);
        let _ = std::fs::remove_file(inner_69.display.as_str());
        let _ = std::fs::remove_file(inner_7.display.as_str());
    }

    /// COR-4 R2-F1: the remap was half-built — a genuine inner exit 86
    /// surfaced as lock-busy INCONCLUSIVE (a fabricated environmental
    /// excuse the plan gate exits 0 on). The wrapper must remap it to
    /// 186 so the CLI can FAIL it on its own merits.
    #[test]
    fn the_inner_own_exit_86_is_remapped_not_misattributed_to_the_lock() {
        let lock = temp_lock("remap86");
        let _ = std::fs::remove_file(&lock);
        let inner_86 = inner_exiting(86);
        let wrapped = lock_wrapped_over(&lock, "5", &inner_86);
        assert_eq!(
            run(&wrapped).status.code(),
            Some(INNER_EXIT_REMAP_86),
            "the inner command's own 86 surfaces as its remap, never as lock-busy"
        );
        let _ = std::fs::remove_file(&lock);
        let _ = std::fs::remove_file(inner_86.display.as_str());
    }

    /// COR-4 R2-F2: an inner command KILLED by a signal (the realistic
    /// OOM victim) must surface in the 200+signal band — never as the
    /// shell's fabricated 128+n the CLI would read as an honest FAIL.
    /// Both legs through the REAL wrapper.
    #[test]
    fn an_inner_signal_death_surfaces_in_the_signal_band() {
        let lock = temp_lock("signal");
        let _ = std::fs::remove_file(&lock);
        for (signal, expected) in [(15, 200 + 15), (9, 200 + 9)] {
            let killer = inner_killing(signal);
            let wrapped = lock_wrapped_over(&lock, "5", &killer);
            assert_eq!(
                run(&wrapped).status.code(),
                Some(expected),
                "kill -{signal} surfaces as {expected}, never as the shell's \
                 128+{signal} encoding"
            );
            let _ = std::fs::remove_file(killer.display.as_str());
        }
        let _ = std::fs::remove_file(&lock);
    }

    /// COR-4 R2-F2's band ambiguity, pinned: a genuine inner exit INSIDE
    /// the remap outputs (169/186) or the signal band (200..231) passes
    /// through unchanged and is indistinguishable from the remapped
    /// forms — disclosed in the report's _meta; the failure direction
    /// stays honest.
    #[test]
    fn a_genuine_inner_band_exit_passes_through_disclosed() {
        let lock = temp_lock("band");
        let _ = std::fs::remove_file(&lock);
        for code in [169, 186, 200, 231] {
            let stub = inner_exiting(code);
            let wrapped = lock_wrapped_over(&lock, "5", &stub);
            assert_eq!(run(&wrapped).status.code(), Some(code), "{code}");
            let _ = std::fs::remove_file(stub.display.as_str());
        }
        let _ = std::fs::remove_file(&lock);
    }

    #[test]
    fn a_held_lock_reports_the_busy_code_within_the_bounded_wait() {
        let lock = temp_lock("busy");
        let _ = std::fs::remove_file(&lock);
        // A real holder process acquires the lock for a few seconds
        // (options before the file operand — the same proven order).
        let mut holder = std::process::Command::new("flock")
            .arg("-w")
            .arg("5")
            .arg(&lock)
            .arg("-c")
            .arg("sleep 4")
            .spawn()
            .expect("the holder acquires the lock");
        std::thread::sleep(std::time::Duration::from_millis(400));
        let busy = lock_wrapped_over(&lock, "1", &inner("/bin/true"));
        let output = run(&busy);
        assert_eq!(
            output.status.code(),
            Some(LOCK_BUSY_EXIT),
            "a held lock is the environmental busy code, not a command failure: stderr {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = holder.wait();
        let _ = std::fs::remove_file(&lock);
    }
}
