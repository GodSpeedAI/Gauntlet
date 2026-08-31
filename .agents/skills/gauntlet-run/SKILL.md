---
name: gauntlet-run
description: Operator procedure for launching and supervising a Gauntlet run — preflight, reference, dispatch, escalations, evidence. Use when the user wants to run, monitor, or inspect a Gauntlet loop against a frozen reference.
---

Repo-owned, no upstream derivation.

This skill is the operator-facing procedure for running and supervising a
Gauntlet loop (Core-spec Annex D.4), companion to `wayfinder-gauntlet`
(which forms the reference this skill then runs against) and to the
`gauntlet-role-<role>` skills (per-role operating procedure, where one
exists). It describes *when and how to invoke* the operator surface — it
never itself decides settlement, authority, or budget. Every such decision
belongs to the runtime, which is typed and tested (Core-spec Annex D.4);
this skill only ever describes what the runtime does, never instructs an
agent to make that call itself.

## Procedure: doctor → reference → run → supervise escalations → inspect evidence

### 1. Doctor — preflight before anything else

Run `gauntlet doctor` before starting or resuming a run. It validates
configuration, the reference and answer key's schema, runner availability
per configured role, sandbox-tier/runner-capability matching, required
semantic tool capabilities, writable state/evidence/workspace directories,
and non-zero budget dimensions (Core-spec §8.5). It writes a machine-
readable report and exits non-zero on any required failure — treat a
non-zero exit as blocking, not as something to route around. Re-run it
after any configuration change and whenever a run is resumed after an
interruption.

### 2. Reference — form or freeze before dispatch

A run needs a sufficient, frozen reference before any builder is
dispatched. If no sufficient reference exists yet, formation runs first —
invoke `wayfinder-gauntlet` (via the Skill tool) rather than starting this
procedure over a loose intent. Once a reference exists:

```bash
gauntlet reference validate --path <path to the answer key>
```

validates it against the answer-key schema (Core-spec §5.3.2) before it is
ever frozen. The runtime itself performs the freeze — resolve, normalize,
digest, and lock the reference outside every builder-writable path — as
part of run start (Core-spec §9.4); this skill's job is only to make sure
that step is invoked at the right time, never to perform normalization or
digesting itself.

### 3. Run — dispatch the loop

```bash
gauntlet run "<desired transformation>"
```

is the intended command from Core-spec §20.1: the runtime discovers
configuration, determines whether a sufficient reference exists, freezes
it, executes the observe → compare → verify → select → correct loop, stops
at a terminal state, and prints an evidence-backed report. **This command
may not be wired yet** — it lands with plan Task 20. Until Task 20 settles,
do not claim `gauntlet run` works; check the plan and this repo's task
status first, and if it is not yet implemented, say so plainly rather than
attempting to narrate a successful run that did not happen.

`gauntlet status` shows in-flight runs, units by tier, in-flight
invocations, and budget burn while a run is active; `gauntlet resume
<run_id>` continues an interrupted one from its durable checkpoint rather
than restarting.

### 4. Supervise escalations

```bash
gauntlet escalations   # list | resolve | retry
```

lists tickets the runtime raised because it reached a condition only an
operator may resolve — an authority a run cannot exercise, a non-converging
correction, exhausted retries, a sandbox validation failure, or a reference-
integrity violation (Core-spec §11.5–§11.6). Read each ticket's recorded
cause before acting: the runtime records why it stopped rather than
guessing at a fix, and the same discipline applies to resolving it — decide
from the recorded evidence, not from a hunch about what probably happened.
`resolve` and `retry` are operator actions; this skill never resolves a
ticket on its own authority, since escalation exists precisely because the
runtime determined the decision needs a human.

### 5. Inspect evidence

```bash
gauntlet inspect   # run/iteration/invocation/unit/evidence detail
```

walks a run's durable record. Evidence and reports live under two
locations in this repository:

- `.agents/reports/` — per-task and per-run reports, checkpoints, and
  progress notes written by roles as they work.
- `.gauntlet/` — runtime-owned policy, lock, and reference artifacts (for
  example the frozen reference lock and comparison contract).

Prefer `gauntlet inspect` and `gauntlet export` (deterministic JSONL of a
run) over reading raw state-store files directly — the CLI surface applies
the same read path the runtime uses internally, so it will not show a
partially-written or stale record as if it were durable. When evidence for
a claim is marked unavailable or a verification outcome is `INCONCLUSIVE`,
treat it exactly as such: the runtime never treats missing evidence as a
pass (Core-spec I8), and neither should a reading of its output.
