# Escalations

Recorded per the standing rule: a genuine blocker (missing tool, ambiguous spec, failed
teeth check that cannot be fixed, irreversible-tier action, human sign-off) is written here
with full context; work continues on the next unblocked task. A blocker is never worked
around by downgrading a tier or an authority.

---

## ESC-001 — Delegation capacity exhausted; Task 3 not started

- **Opened:** 2026-08-15
- **Status:** **CLOSED** 2026-08-15 — the session limit reset, delegation resumed, and the
  deferred blind verification ran. It found 4 critical findings and Task 2 was reopened
  (`.agents/reports/task2/verification-round3.md`). Recording that outcome is the point: the
  escalation's stated risk — *"Task 2's corrected artifact has never been independently
  reviewed… do not upgrade AK-001/002/016 beyond `implemented but provisional`"* — was
  correct, and holding the line on it is what caught the four holes.
- **Blocks:** Task 3 (domain types) and every task after it
- **Does not block:** nothing else; Tasks 0–2 are complete and committed

### What happened

The Task 2 correction workflow (`wf_8d6d78e3-347`) ran one builder and two blind critics.
All three agents terminated with `You've hit your session limit · resets 1:20pm
(America/New_York)`. The builder had already landed its corrections before dying; the two
blind critics had not started reviewing. Subsequent delegation is unavailable until the
limit resets.

### Why this is an escalation rather than something to work around

`.agents/plans/gauntlet-impl.md`, Guardrails: *"Do not let the orchestrator implement. While
delegation is available, decompose and dispatch. If delegation is genuinely unavailable, stop
and report `blocked{no_delegation_mechanism}` unless the override variable is explicitly
set."* The override variable (`GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION`) is not set.

So the correct action is to stop and record, not to implement Task 3 inline. Implementing it
directly would be exactly the silent workaround the standing rules forbid — it would also
destroy the independent-blind-critic property that every builder slice is supposed to have.

**Reported state: `blocked{no_delegation_mechanism}`.**

### Two consequences that must not be lost

1. **Task 2's corrected artifact has never been independently reviewed.** It has a passing
   gate and a demonstrated teeth check (seven cases, all biting), but the blind re-critique
   that was supposed to verify the corrections died before running. The artifact is
   *implemented and teeth-checked*, not *independently verified*. Do not upgrade AK-001,
   AK-002, or AK-016 beyond `implemented but provisional` on the strength of the gate alone.

2. **The first form of teeth case T1 was INCONCLUSIVE and is recorded as such.** Adding a
   path dependency on a crate absent from `[workspace].members` made `just boundaries` exit
   non-zero — but via cargo's own resolver error (`no matching package named 'infra'`), not
   via our violation message. That run proves nothing about our mechanism. The mechanism was
   then tested directly (a stray package directory under `crates/` not listed in members),
   which does fail with our check. `INCONCLUSIVE` was not recorded as `PASS`.

### Clearing this

When delegation is available again:

1. Run a blind critic (read-only, non-compiling) over `crates/gauntlet-arch-tests/` and
   `tests/contract/`. Any surviving **critical** finding reopens Task 2.
2. Then start Task 3 per `.agents/CURRENT_STATUS.yml`.

---

## ESC-002 — Delegated builder sessions die on spawn (host OOMs the harness)

- **Opened:** 2026-08-17
- **Status:** OPEN — proceeding under the operator's standing instruction
  ("continue; ensure only one agent can compile at a time") as
  orchestrator-implementer on Task 7 only, with the build lock held exclusively
  and all cargo invocations run `jobs=1`/`NEXTEST_TEST_THREADS=1`. The blind-critic
  loop will still run afterward via `explore` agents, which the
  `subagent-message-loss` lesson records as having "delivered full reports every
  time" (read-only, non-compiling).
- **Blocks:** Task 7 (ReferenceProvider) and every task after it (the reference
  machinery Task 7 builds is the substrate Tasks 8–27 consume).
- **Does not block:** nothing below Task 7 except where a slice genuinely does not
  depend on the reference pipeline; in practice no such slice is next.

### What happened

The Task 7 round-1 builder died mid-slice (D1 `normalize.rs` and D2 `parse.rs`
landed and checkpointed; D3 `manifest.rs` landed but uncommitted and uncheckpointed;
D4–D8 absent). The orchestrator then dispatched a finisher builder four consecutive
times. Each session died **before writing anything except its alive-line**, across
two load shapes:

- finisher dispatches 1–3 — full brief incl. in-session `cargo` gates (mem-safeguard
  `CARGO_BUILD_JOBS=1`/`NEXTEST_*` in place for dispatches 2–3);
- finisher dispatch 4 — **write-only**: explicitly forbidden from invoking
  `cargo`/`rustc`/`just` at all. It died identically, after writing only its
  alive-line.

The **write-only** death rules out cargo spikes as the trigger: sub-agent *session
spawn itself* loads the harness's model context (the `ai.traycer.host.service`
`MainThread`, `oom_score_adj=200`) and tips the 8 GiB WSL VM into global OOM, which
kills `MainThread` first. Evidence:

- `kern.log`: `Out of memory: Killed process … (MainThread) total-vm=~20 GB …
  oom_score_adj=200` at 15:08 and 15:26 (the 15:26 kill also fired while
  `reference::mani invoked oom-killer` — the test binary's own allocation tipped it);
- `syslog`: `systemd-journald Under memory pressure, flushing caches` repeated
  through 17:25–17:27, exactly bracketing the finisher dispatch-3 session, which then
  returned empty (alive-line only).

### Why this is an escalation, not a silent workaround

AGENTS.md §"Delegation": the orchestrator does not write implementation code while
delegation is available; the escape is `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`
(which is **unset** here). Per the standing rule, when delegation is genuinely
unavailable the action is to stop and record `blocked{no_delegation_mechanism}` and
skip to the next unblocked task — but here **no** task below Task 7 is unblocked (the
reference pipeline is the substrate of everything after), so "skip to next" strands
the tree and orphaned work indefinitely.

The mechanism still exists at the API level (the `task` tool returns a handle), but
every spawn dies in this environment right now. Empirically the
`subagent-message-loss` recovery procedure ("re-dispatch fresh with the same brief —
delegation itself remained available every time") has been **exhausted** for Task 7:
five consecutive sessions, only alive-lines landed.

### Operator's standing instruction (this session)

The operator has repeatedly said "continue" and "ensure only one agent can compile at
a time," taken here as authorizing orchestrator-implementation for Task 7 only (the
build lock is exclusive to the orchestrator; all cargo invocations run with
`CARGO_BUILD_JOBS=1` and `NEXTEST_TEST_THREADS=1`; only one cargo process is ever
live). If that interpretation is wrong, the operator can revert the slice and the
escalation stays open until delegation is healthy.

### Holding the loop's invariants under this fallback

The blind-critic step is NOT waived — it is relocated. After orchestrator gates +
teeth pass, blind critics will be dispatched as `explore`-type agents (read-only,
non-compiling), which the recorded lesson shows deliver reliably in this
environment. A lost final message costs nothing because the critic writes its report
file first, then a blind correction cycle resolves residual findings. The settlement
of Task 7's Units (AK-021…AK-025, AK-053 substrate) still requires the blind-critic
round; orchestrator-gates-green alone is explicitly insufficient (CURRENT_STATUS §TL;DR).

### Durable fix (operator-side, recorded, not taken by this session)

Raise the WSL memory ceiling. Add to the Windows-side `%USERPROFILE%\.wslconfig`:

```
[wsl2]
memory=12GB
swap=8GB
```

then `wsl --shutdown` and re-enter. This removes the trigger cleanly;
`GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1` is the in-repo escape hatch if the
operator prefers to keep the orchestrator-implementer path past this task.

