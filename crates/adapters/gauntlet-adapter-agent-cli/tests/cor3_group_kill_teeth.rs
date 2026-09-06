//! COR-3 (task 25 corrections round 3) transport-level teeth for the
//! stop's ESCALATION leg, from falsifier round 2 finding F2-4 (MAJOR).
//!
//! The falsifier's A3-M3 mutant — the stop's SIGKILL leg re-pointed from
//! the process GROUP (`-<pid>`) to the DIRECT child pid — left the ENTIRE
//! delivered battery green: the delivered TERM-ignoring-child teeth use a
//! direct `sleep` payload the `inner.kill()` backstop retires anyway, so
//! no delivered test drives the KILL leg into a tree TERM could not
//! retire. This file binds the correction's "BOTH stop legs address the
//! WHOLE GROUP" claim at the half the delivered battery left unpinned:
//!
//! a TERM-ignoring driver WITH a TERM-ignoring grandchild (the module
//! docs' documented bounded claim — a grandchild that escapes the group
//! via `setsid` stays beyond it) survives the graceful leg untouched, so
//! the group SIGKILL is the ONLY retire path; survivors are asserted by
//! bounded `/proc/<pid>/stat` liveness.
//!
//! Tiny bounded `sh` probe children only (the round-1 live-attack
//! posture: no agent binary, no model, no Unit dispatch). Hygiene: the
//! whole group is killed — and observed dead — BEFORE the failure
//! assertion fires, so even a RED run leaks no child (falsifier F2-6: a
//! leaked spawned child inherits and HOLDS the repository compile lock
//! fd across the flock→nextest→test→transport chain).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use gauntlet_adapter_agent_cli::transport::{ProcessChildTransport, ProcessSpec, ProcessTransport};
use gauntlet_domain::EpochSeconds;
use gauntlet_ports::deadline::Deadline;

/// The probe driver: TERM-immune itself (`trap '' TERM`), forks a
/// TERM-immune grandchild (ignores TERM in its own right; the sleep
/// children it loops are belt-and-braces — whether or not the shell's
/// SIG_IGN survives into them, the DRIVER and the GRANDCHILD SHELL
/// survive a group TERM, which is the shape whose only retire path is
/// the escalation leg's group SIGKILL), then waits.
fn term_immune_driver_script() -> String {
    r#"trap '' TERM
sh -c 'trap "" TERM; while :; do sleep 1; done' &
wait
"#
    .to_owned()
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-cor3-killteeth-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch creatable");
    dir
}

fn bound_from_now(seconds: u64) -> Deadline {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now + seconds))
}

/// F2-5 (falsifier round 2, observation): group-liveness idioms built on
/// `kill -0 -- -PGID` are reaper-latency-sensitive — an orphaned,
/// unreaped CORPSE (zombie) is still a group member for kill(2) until the
/// reaper collects it (observed multi-second latency on this host), so a
/// `kill -0`-based check false-alarms on dead trees. The authority here
/// is the `/proc/<pid>/stat` read: a member is LIVE only while its stat
/// entry exists, its state is not `Z`, and its pgrp is the child's group.
fn live_group_members(pgid: u64) -> Vec<u64> {
    let mut members = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return members;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(pid) = name.parse::<u64>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // `comm` may contain spaces and parens: parse AFTER the last ')'.
        // Fields then run state(1) ppid(2) pgrp(3) session(4) ...
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        let Ok(member_pgid) = fields[2].parse::<u64>() else {
            continue;
        };
        if member_pgid == pgid && fields[0] != "Z" {
            members.push(pid);
        }
    }
    members
}

/// The hygiene reaper: group-addressed SIGKILL through a fixed-argv
/// helper (the pid is an argv ARGUMENT; dash's kill builtin rejects a
/// `--` separator, so the negated form IS the portable group-pid
/// spelling — the transport's own convention).
fn group_kill(pgid: u64) {
    let _ = std::process::Command::new("sh")
        .args([
            "-c",
            "kill -KILL \"$1\" 2>/dev/null || true",
            "sh",
            &format!("-{pgid}"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// F2-4 (falsifier round 2, MAJOR — COR-3): a TERM-ignoring driver WITH a
/// TERM-ignoring grandchild is retired ONLY by the escalation leg's
/// group-addressed SIGKILL — the graceful leg retires nothing (both
/// generations trap TERM away), and the direct-child backstop cannot
/// reach the grandchild. RED under the falsifier's exact A3-M3 mutant
/// (the KILL leg's argv target re-pointed from `-<pid>` to the direct
/// pid): the driver dies, the grandchild survives LIVE, and the captured
/// survivor pids are named in the failure. GREEN on the real tree.
#[test]
fn the_stop_kill_leg_retires_the_whole_group_a_term_immune_tree_survives() {
    let scratch = scratch("kill-leg");
    let driver_path = scratch.join("term-immune-driver.sh");
    std::fs::write(&driver_path, term_immune_driver_script())
        .expect("the probe driver script is writable");
    let transport = ProcessChildTransport::new();
    let spec = ProcessSpec {
        argv: vec!["sh".to_owned(), driver_path.to_string_lossy().into_owned()],
        cwd: scratch.clone(),
        result_path: "/unused/result.json".to_owned(),
    };
    let child = transport
        .spawn(&spec)
        .expect("the TERM-immune driver spawns");
    // F-R3 mechanics (verified live by the falsifier's A3-L1): the spawned
    // child IS its own process-group leader, so the child's pid IS the pgid.
    let pgid = child.raw();

    // The grandchild must EXIST before the stop runs — otherwise even the
    // mutant could pass by killing the driver before it forked the payload.
    // Bounded wait for a live non-driver member of the group.
    let grandchild_deadline = Instant::now() + Duration::from_secs(5);
    while live_group_members(pgid).len() <= 1 {
        assert!(
            Instant::now() < grandchild_deadline,
            "the TERM-immune grandchild never appeared in the child's group"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // THE STOP: the graceful leg cannot retire anything (both generations
    // ignore TERM), so the short bound forces the escalation leg — the
    // group SIGKILL that is this tree's only retire path.
    let stopped = transport.stop(child, bound_from_now(2));
    assert!(stopped.is_ok(), "the stop succeeds: {stopped:?}");

    // THE RETIREMENT OBLIGATION: bounded poll (F2-5 — past reaper latency;
    // corpses are excluded by the state read, so the wait bounds only the
    // dying, never the reaping).
    let poll_deadline = Instant::now() + Duration::from_secs(15);
    let mut survivors = live_group_members(pgid);
    while !survivors.is_empty() && Instant::now() < poll_deadline {
        std::thread::sleep(Duration::from_millis(50));
        survivors = live_group_members(pgid);
    }

    // HYGIENE BEFORE THE ASSERTION (F2-6): kill the whole group and observe
    // it empty FIRST — a RED run must name its survivors, not leak them
    // (a leaked child holds the compile-lock fd and can wedge the
    // repository's whole compile loop).
    group_kill(pgid);
    let cleanup_deadline = Instant::now() + Duration::from_secs(10);
    while !live_group_members(pgid).is_empty() && Instant::now() < cleanup_deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let still_live = live_group_members(pgid);

    assert!(
        survivors.is_empty(),
        "the escalation leg's group SIGKILL must retire the WHOLE tree — live members \
         survived: {survivors:?} (after the hygiene group-kill: {still_live:?}) — the KILL \
         leg addressed something smaller than the group"
    );
    assert!(
        still_live.is_empty(),
        "the hygiene kill failed to empty the group: {still_live:?}"
    );
}
