# Structural review — UNK-001 disposition and the Task-14 redesign trigger

**Reviewer:** structural-reviewer (charter: `.agents/agents/structural-reviewer.agent.md`)
**Date:** 2026-08-28
**Subject:** two structural questions raised by Task 14's closure
**Mode:** text/analysis only. No compile was run (build-lock discipline). No frozen
artifact was read-modified; nothing outside `.agents/reports/task14/` was written.
**Standing:** RECOMMEND ONLY. The orchestrator decides and writes.

---

## 1. Evidence base

Every claim below is anchored to a file:line read in this session. Nothing is inferred
from a build, a test run, or memory of prior tasks.

### Frozen artifacts (read-only)

| Fact | Locator |
|---|---|
| UNK-001 text and `status: open` | `.wayfinder/reference.yaml:150-161` |
| UNK-001's own note pre-commits the disposition to `docs/CONFORMANCE.md` | `.wayfinder/reference.yaml:157-161` |
| Settlement criterion: "`docs/CONFORMANCE.md` states no status ahead of a passing test" | `.wayfinder/reference.yaml:137` |
| Assumption tying this host's inventory to Task 14's containment decisions, "pending revision … (see UNK-001)" | `.wayfinder/reference.yaml:181-184` |
| Manifest digest locked `sha256:95331d3e34…`; answer key `sha256:ccc7c99be3…`; `frozen_at 2026-08-15T03:04:35Z`; "never edited for any reason" | `.gauntlet/reference.lock` |
| Spec: unknowns are "named, not silently resolved"; MUST NOT appear in `units` as if settled | `.agents/specs/Core-spec.md:264,271` |
| Spec: the final report distinguishes six statuses including `unknown` | `.agents/specs/Core-spec.md:871,1393` |
| Spec: implementation-defined choices MUST be documented in `docs/CONFORMANCE.md` | `.agents/specs/Core-spec.md:31,1244` |

### Landed Task-14 tree

| Fact | Locator |
|---|---|
| `cross_cutting` policy artifact pins `"backend": "bwrap"`, `deny_by_default`, denies `.git`/`.ssh`/`.aws`/`.gnupg` | `.gauntlet/policies/cross_cutting.json` |
| `contained` and `local` artifacts also pin `"backend": "bwrap"` | `.gauntlet/policies/{contained,local}.json` |
| No `irreversible.json` exists (absence by design) | `ls .gauntlet/policies/` |
| bwrap adapter probes the **real binary** before any policy judgment; honest rung is `Isolation::Process`; baseline `--ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent` | `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs:1-25,52-62,95-120` |
| Adapter depends on domain + ports only; product name confined to this crate | `crates/adapters/gauntlet-adapter-sandbox-bwrap/Cargo.toml` |
| `PolicyFs` strictly decodes the tier artifact; `irreversible` refused **structurally**, before any file read | `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs:1-21,54-70` |
| **`sandbox.backend` default is `"none"`** | `crates/gauntlet-app/src/config/defaults.rs:54-56` |
| `GovernDeps`: a non-default backend token *requires* both a Sandbox and a PolicyProvider; the conservative token *forbids* both — "an optional gate would be an unsandboxed fallback wearing a type-system costume" | `crates/gauntlet-app/src/govern/gate.rs:61-79,90-125` |
| Gate conjunct 5: above `contained`, no backend ⇒ `GateRefusal::NoBackendForTier` | `crates/gauntlet-app/src/govern/gate.rs:33-35,274-275,461` |
| Gate conjunct 6: policy validation failure ⇒ escalation, never unsandboxed fallback (AK-047) | `crates/gauntlet-app/src/govern/gate.rs:36-41` |
| bwrap adapter is a **production** `[dependencies]` entry of the composition root, used by `gauntlet doctor` check 4 to *measure* the backend | `crates/gauntlet-cli/Cargo.toml:14-20`; `crates/gauntlet-cli/src/main.rs:44-48,84-145` |
| The CLI surface today is exactly `doctor` and `reference digest`; "the full §20.1 surface lands with plan Task 20" | `crates/gauntlet-cli/src/main.rs:1-11,19-40` |
| `GovernDeps::with_backend` (the real-backend posture) is constructed **only** in `tests/p5_hard_gate.rs:778`; every other construction is `none_backend` / `none_workspace` | grep `GovernDeps` across the workspace |
| The `none` sandbox adapter is wired **nowhere** — not a dependency or dev-dependency of any crate; it appears only in workspace membership and in the arch-test denylist fixtures | grep `sandbox-none` / `sandbox_none` |
| P5 executes a real wrapped command: denied-path contents do not cross the boundary, and the wrapped command's **first word is the backend binary** (containment from process start, AK-048) | `crates/gauntlet-cli/tests/p5_hard_gate.rs:1094-1117` |
| P5 also composes the **shipped** repo artifacts through the real provider: all three dispatchable tiers resolve; `irreversible` is refused | `crates/gauntlet-cli/tests/p5_hard_gate.rs:1119-1149` |

### Plan and prior adjudications

| Fact | Locator |
|---|---|
| Task-14 redesign trigger text (the instruction under review) | `.agents/plans/gauntlet-impl.md:494` |
| `docs/CONFORMANCE.md` is **written at Task 27**, step 2 | `.agents/plans/gauntlet-impl.md:804` |
| Task-27 checklist: "`docs/CONFORMANCE.md` reflects reality — no status ahead of a passing test" | `.agents/plans/gauntlet-impl.md:856` |
| Task 0 already adjudicated that the trigger does **not** fire, because a real backend exists; the Annex-F weakening is to be "stated plainly in `docs/CONFORMANCE.md` **at Task 27**, not hidden behind an 'it's sandboxed' claim" | `.agents/reports/environment.md:41-52` |
| Task-14 recon repeats the same adjudication and notes "UNK-001 in reference.yaml tracks the general question" | `.agents/reports/task14/recon.md:79-87` |
| `docs/` contains only `schemas/` — **`docs/CONFORMANCE.md` does not exist** | `ls docs/` |
| Only **one** redesign trigger in the whole plan names a frozen artifact (line 494). Lines 122, 521, 745 name only `docs/CONFORMANCE.md`, which is writable | grep `CONFORMANCE` across the repo |
| The plan is **not** under any lock and has been edited since the Task-1 freeze (commit `c4e58fa`, Task 8) | `git log -- .agents/plans/gauntlet-impl.md`; `scripts/verify-ruler.sh` covers only the answer key and manifest |

### Not established (declared, not glossed)

- No build, test, or `just` gate was executed. Every behavioral claim above is a claim
  about **what the source asserts**, not about a green run. Where I say "P5 proves X", read
  "P5 asserts X and would fail if X were false"; whether P5 currently passes is outside my
  slice.
- I did not read `.agents/CURRENT_STATUS.yml` beyond its UNK-001 block (lines 227-243).

---

## 2. Adjudication — QUESTION 1: the disposition of UNK-001

### 2.1 What Task 14 actually established

Task 14 answered a **mechanism** question that UNK-001 did not ask, and left the
**deployment** question UNK-001 did ask untouched. Precisely, four things are now facts of
the tree:

1. **The backend is a substitutable, artifact-declared choice.** `cross_cutting.json`
   names `"backend": "bwrap"` as data, not code. `PolicyFs` decodes the token strictly and
   the bwrap adapter refuses to enforce a policy naming a backend other than its own
   identity (`main.rs:104-106` documents this cross-wiring defense). Changing which backend
   `cross_cutting` runs under is therefore a **deployment-time edit to a versioned repo
   artifact plus a composition-root swap**, not a runtime discovery. The port exists so a
   stronger backend can be added later, exactly as the trigger's final clause required.

2. **On this build host, `cross_cutting` under bwrap is real and demonstrable.** The
   adapter probes the actual binary before judging any policy; P5 wraps a real command,
   asserts the denied path's contents do not cross the boundary, and asserts the first
   process *is* the sandbox. The reported rung is honestly `Isolation::Process`, not
   inflated to container or VM grade.

3. **The shipped default posture is not bwrap — it is refusal.** This is the sharpest and
   most easily misread fact in the tree. `defaults::SANDBOX_BACKEND == "none"`
   (`defaults.rs:56`). Under that token `GovernDeps` *forbids* carrying a Sandbox or
   PolicyProvider at all (`gate.rs:90-125`), and gate conjunct 5 refuses every tier above
   `contained` with `NoBackendForTier` (`gate.rs:461`). So a default-configured runtime does
   not run `cross_cutting` under bwrap; it does not run `cross_cutting` **at all**. That is
   fail-closed and correct, but it means "cross_cutting runs under bwrap" is false of the
   shipped defaults.

4. **No production dispatch path composes the real backend yet.** `GovernDeps::with_backend`
   is constructed in exactly one place in the workspace: `tests/p5_hard_gate.rs:778`. The
   composition root's only production use of the bwrap adapter is the `doctor` check-4
   *measurement* (`main.rs:92-145`) — it validates a throwaway `doctor-probe` policy at the
   `contained` tier and reports the measured rung. That is a real production code path
   exercising a real binary, but it is a **measurement**, not a dispatch wrap. There is no
   dispatch command to wire: `main.rs:1-11` states the §20.1 surface lands with Task 20.

### 2.2 Verdict: **PARTIALLY RESOLVED**

I apply the strictness the question demands and refuse to launder host capability into a
production answer.

**Resolved half (durably, with evidence):** the *mechanism* by which `cross_cutting`'s
backend is chosen, declared, validated, and enforced — and the proof that the strongest
backend available on this host genuinely contains. Also resolved: the fail-closed answer for
the unconfigured case (refusal, not degradation).

**Open half (and it is the half UNK-001 asked about):** which backend `cross_cutting` will
run under *in production use of this runtime*. Three distinct residues keep it open:

- **R1 — no production target is named.** No deployment environment other than this WSL2
  build host is described anywhere in the tree. A question about production cannot be closed
  by evidence drawn entirely from the build host; that is precisely the inference the
  manifest's own `assumptions` block flags as revisable (`reference.yaml:181-184`).
- **R2 — the Annex-F weakening is unstated.** Spec Annex F's typical `cross_cutting`
  backend is microVM or container grade; this composition delivers `Isolation::Process`.
  Task 0 committed to stating that plainly in `docs/CONFORMANCE.md`
  (`environment.md:47-49`). That file does not exist. Until it does, the weakening is
  recorded only in working reports, and a reader of the shipped artifacts sees
  `"backend": "bwrap"` with no note that it is a *substitution* for the typical backend.
- **R3 — no composed dispatch path.** Until Task 20 wires a run command, no shipped code
  path dispatches a `cross_cutting` unit under any backend. The question "what will it run
  under" has no production referent yet.

**Explicitly rejected upgrade to RESOLVED.** The tempting argument — "bwrap is landed,
validated, and pinned in `cross_cutting.json`, therefore the backend is known" — conflates
three different propositions:

| Proposition | Status |
|---|---|
| This build host *can* run `cross_cutting` under bwrap | **Established** (P5) |
| The shipped artifact *names* bwrap for `cross_cutting` | **Established** (`cross_cutting.json`) |
| A default-configured runtime *runs* `cross_cutting` under bwrap | **False** — it refuses (`defaults.rs:56` + `gate.rs:461`) |
| `cross_cutting` runs under bwrap **in production** | **Unknown** — no production target exists to observe |

Recording UNK-001 as RESOLVED would put a status ahead of its evidence, which is the exact
failure mode `reference.yaml:137` and `.agents/specs/Core-spec.md:871` exist to prevent.

### 2.3 Where the disposition may legitimately be recorded

`.wayfinder/reference.yaml` is frozen and digest-locked; it is not a candidate and I do not
propose touching it. Four non-frozen destinations were considered:

| Destination | Verdict |
|---|---|
| `docs/CONFORMANCE.md` (at Task 27) | **PRIMARY.** The manifest's own UNK-001 note names this file as the disposition's home (`reference.yaml:159-161`); the plan assigns its authorship to Task 27 step 2 (line 804); Task 0 pre-committed the weakening statement to it (`environment.md:47`); and the spec requires implementation-defined choices to be documented there (`Core-spec.md:31,1244`). The frozen ruler already routed this disposition here. |
| `.agents/CURRENT_STATUS.yml` (lines 229-234) | **INTERIM, orchestrator-owned.** The working status line should narrow from bare UNKNOWN to UNKNOWN-with-established-bounds so the residue is not re-litigated at Tasks 15-27. I do not write it; proposed text below. |
| `.agents/reports/task14/structural-review-unk001.md` (this file) | **EVIDENCE ANCHOR.** Already written; cite it, do not duplicate it. |
| `.agents/reports/reference-candidates.md` | **REJECTED.** Its contract is explicit and narrow: "A Unit that seems wrong becomes a dated entry … with evidence" (`.gauntlet/reference.lock` note; file header). UNK-001 is not a Unit, and nothing about it is wrong — the manifest recorded it correctly and its note anticipated exactly what Task 14 did. Filing a correct unknown's disposition in a proposed-corrections log would corrupt that log's meaning and invite a future reader to think the ruler needs amending. It does not. |

---

### RECOMMENDATION 1

**R1a — Disposition: PARTIALLY RESOLVED. Do not close UNK-001.** The mechanism half is
settled and provable; the production half is unresolvable from this tree and stays open
until a deployment target is named or Task 27 states the posture.

**R1b — Interim record.** Orchestrator to replace `.agents/CURRENT_STATUS.yml` lines
229-234 with the following. Exact proposed text:

```yaml
  - status: UNKNOWN
    statement: >-
      UNK-001: which containment backend cross_cutting runs under in real
      use, beyond this host's bwrap-only environment. NARROWED by task 14,
      not closed. Established: the backend is an artifact-declared,
      substitutable choice (.gauntlet/policies/cross_cutting.json names
      "bwrap"); on this host bwrap genuinely contains, at an honest
      Isolation::Process rung (p5_hard_gate wraps a real command, denied
      paths do not leak, the first process IS the sandbox); and the SHIPPED
      DEFAULT posture is sandbox.backend="none", under which cross_cutting
      refuses dispatch (NoBackendForTier) rather than degrading. Still open:
      no production deployment target is named anywhere in the tree; the
      Annex-F weakening (typical cross_cutting backend is microVM/container
      grade, this composition delivers process grade) is not yet stated in
      docs/CONFORMANCE.md; and no composed dispatch path wires the real
      backend outside the p5 proof (GovernDeps::with_backend appears only in
      tests/p5_hard_gate.rs — production dispatch lands with Task 20).
      Terminal disposition belongs to docs/CONFORMANCE.md at Task 27.
    evidence: >-
      .wayfinder/reference.yaml:150-161;
      .agents/reports/task14/structural-review-unk001.md
```

**R1c — Terminal record.** At Task 27, `docs/CONFORMANCE.md` must state, at minimum: the
tier→backend mapping as shipped; that `cross_cutting` is served by a process-isolation
backend rather than Annex F's typical microVM/container grade, named as a substitution; that
the default posture refuses `cross_cutting` outright; and that UNK-001's production half was
never observed. Suggested wording to carry forward:

> **`cross_cutting` containment — a declared substitution.** Spec Annex F's typical
> `cross_cutting` backend is a microVM or the strongest available container tier. This
> implementation ships neither. The strongest real backend adjudicated on the build host
> (`.agents/reports/environment.md`) is bubblewrap, a user-namespace mount sandbox whose
> honest rung is `Isolation::Process`. `.gauntlet/policies/cross_cutting.json` therefore
> names `bwrap`, and the achieved isolation for `cross_cutting` is **process grade, not
> kernel or VM grade**. This is a weakening relative to Annex F and is stated here rather
> than concealed behind an unqualified "it is sandboxed" claim. The shipped default
> (`sandbox.backend = "none"`) does not run `cross_cutting` at all: the authority gate
> refuses every tier above `contained` when no backend is composed, and a sandbox
> validation failure escalates and never falls back to unsandboxed execution. Which backend
> `cross_cutting` runs under in any deployment other than the build host was recorded as
> UNK-001 at freeze time and **remains unobserved**; it is a deployment-configuration fact,
> resolved by editing the versioned tier artifact and composing the corresponding adapter.

I have **not** drafted `docs/CONFORMANCE.draft.md` — see Recommendation 2a for why.

---

## 3. Adjudication — QUESTION 2

### 3.1 (a) Did the trigger fire? **NO.** `docs/CONFORMANCE.md` is not owed by Task 14.

The trigger is a conditional: *"If no real sandbox backend exists on this host, do not fake
containment…"* (`plan:494`). Its antecedent is a claim about **backend existence on this
host**, and that claim is false on every available reading:

- **Host inventory:** bubblewrap 0.11.1 is present and was smoke-tested at Task 0
  (`environment.md:35-40`).
- **Adapter reality:** the bwrap adapter refuses to opine on any policy until it has
  spawned the actual binary under the baseline containment shape; an unusable backend is a
  typed `BackendUnavailable`, never a policy opinion
  (`gauntlet-adapter-sandbox-bwrap/src/lib.rs:7-9,95-120`).
- **Demonstrated containment:** P5 executes a wrapped command against a real denied path
  and asserts the boundary holds and the sandbox binary is the first process
  (`p5_hard_gate.rs:1094-1117`).

A real backend exists, is fronted by a real adapter, and demonstrably contains. The
antecedent is false, so the consequent — refuse `cross_cutting` dispatch, write
`docs/CONFORMANCE.md`, record in the manifest's `unknowns` — is not owed by Task 14. This is
not a new adjudication; Task 0 reached it first, in writing, and routed the residual
weakening statement to Task 27 (`environment.md:47-52`). Task 14's recon carried it forward
unchanged (`recon.md:79-87`).

**On "wired through the production composition path or only present as a crate":** neither
extreme is accurate, and the precise answer matters for the disposition. The bwrap adapter
is:

- **more than a crate** — it is a production `[dependencies]` entry of the composition root
  (`gauntlet-cli/Cargo.toml:14-20`) and is executed by a shipped command: `gauntlet doctor`
  check 4 constructs the real backend, validates a policy through it, and reports a
  **measured** rung, refusing to declare one (`main.rs:84-145`). The `none` sandbox adapter,
  by contrast, *is* only a crate — it is wired nowhere at all.
- **less than a dispatch path** — `GovernDeps::with_backend` exists only in the P5 proof,
  and there is no production dispatch command to wire it into, because the §20.1 surface is
  Task 20's scope (`main.rs:1-11`).

That gap is **scheduled, not defective**. Faulting Task 14 for the absence of a dispatch
composition would be faulting it for not doing Task 20's work.

**One inversion worth recording explicitly**, because a hasty reader will mistake it for
the trigger having fired: Task 14 *does* make `cross_cutting` refuse dispatch under the
shipped defaults (`defaults.rs:56` + `gate.rs:461`). But it does so for a **stronger and
different reason** than the trigger contemplated — a fail-closed default posture that
demands the operator compose a backend deliberately — not because containment would
otherwise be faked. Same observable, different cause. The trigger still did not fire.

**Why I am not drafting `docs/CONFORMANCE.md` now.** Beyond it not being owed, drafting it
at Task 14 would actively violate the ruler. `reference.yaml:137` makes it a settlement
criterion that "`docs/CONFORMANCE.md` states no status ahead of a passing test", and
`plan:856` repeats it. A Task-14-era conformance document would have to characterize the
trust posture, profile conformance (Core/Extended/Self-improving), and advisory-only
enforcements for capabilities that do not exist yet (Tasks 15-26, including the entire
G1-G16 anti-gaming suite at Task 24). Every such line would be a status ahead of its test.
The correct action is to leave the file uncreated and carry the required content forward as
Recommendation 1c.

### 3.2 (b) Is the "manifest's `unknowns`" instruction a structural defect?

**Finding of fact.** The instruction is genuinely unexecutable as written. The manifest was
frozen at `2026-08-15T03:04:35Z` and digest-locked at `sha256:95331d3e34…`; the lock's own
note says it is "never edited for any reason, including a gate that will not otherwise
pass". Any agent that obeyed `plan:494` literally would break `just ruler` — the I1 tripwire
— and invalidate work done since the freeze. So the plan does contain a directive that the
project's freeze discipline forbids executing. That much is not in dispute.

**Three facts make it latent rather than active, and they are decisive:**

1. **The branch is unreachable.** The instruction lives inside a conditional whose
   antecedent is false and was adjudicated false at Task 0, before any code was written. It
   has never been reached and, for this host, never can be. An unexecutable instruction in a
   dead branch is a latent defect, not an operative one.

2. **The manifest already contains what the instruction would have written.** This is the
   strongest evidence for authoring order over structural defect. `reference.yaml:157-161`
   reads: *"Task 14 wraps cross_cutting with bwrap as the strongest available real backend
   and records the limitation in docs/CONFORMANCE.md rather than assuming a stronger backend
   exists elsewhere."* The manifest's author, at freeze time, pre-recorded the exact content
   the trigger would demand — including the routing to `docs/CONFORMANCE.md`. The freeze did
   not orphan the instruction; it **absorbed** it. The plan and the manifest were authored in
   the same window, and the plan's sentence simply outlived the moment when the manifest was
   still writable.

3. **It is a single site, and the neighbours are clean.** Of the four redesign triggers
   naming a documentation destination, only `plan:494` names a frozen artifact. Lines 122,
   521, and 745 name only `docs/CONFORMANCE.md`, which is writable and correctly routed.
   This is not a systemic pattern of the plan reaching into frozen artifacts; it is one
   stale clause.

**The eight-condition gate.** A `CandidateRevision` is permitted only if all eight are
proven with durable evidence. Applied to a proposed structural revision against the plan or
spec on account of `plan:494`:

| # | Condition | Verdict |
|---|---|---|
| 1 | Reliable divergence — defect reproduces consistently across multiple runs | **NOT PROVEN.** Zero runs diverged. The branch never executed. There is no observed defect, only a readable one. |
| 2 | Attributable to structure | **PARTIAL.** The wording is in a specification-class artifact, so *if* a defect were active it would be structural. Moot while (1) fails. |
| 3 | Local repair exhausted | **NOT PROVEN.** Local repair is a one-line plan-text edit and has not been attempted once. The plan is not frozen: it carries no lock entry, `verify-ruler.sh` covers only the answer key and manifest, and it was edited after the freeze (commit `c4e58fa`, Task 8). |
| 4 | Environment addressed | **PROVEN.** Host inventory adjudicated at Task 0 and unchanged. |
| 5 | Concrete alternative exists | **PROVEN.** Supplied verbatim below. |
| 6 | Affordable | **PROVEN.** One sentence; no code, no migration. |
| 7 | Governable | **PROVEN**, and it cuts *against* revision: the plan is a mutable working artifact, so correcting it is ordinary local repair and never reaches the CandidateRevision machinery at all. |
| 8 | Rollback & regression ready | **PROVEN.** Trivially git-reversible; no test depends on the sentence. |

**Decision: `Block Revision`.** Two conditions fail, and the failing one is condition 1 —
the load-bearing one, the whole point of which is to stop specification churn driven by
reading rather than by observed failure. Nothing about the ruler is wrong: the manifest
recorded UNK-001 correctly, noted the right disposition, and routed it to the right file.
No `CandidateRevision` against `.wayfinder/answer-key.md`, `.wayfinder/reference.yaml`, or
`.agents/specs/Core-spec.md` is warranted, and I do not propose an entry in
`.agents/reports/reference-candidates.md`.

**Characterization:** a harmless artifact of authoring order, with a real but latent
wording defect at exactly one site, correctable by ordinary local repair whenever the plan
is next touched.

---

### RECOMMENDATION 2

**R2a — The trigger did not fire; `docs/CONFORMANCE.md` is NOT owed by Task 14.** Do not
create it now. It is owed at Task 27 (`plan:804`), and creating it early would state
statuses ahead of passing tests, violating `reference.yaml:137`. No
`CONFORMANCE.draft.md` has been written. Carry Recommendation 1c forward as its seed.

**R2b — `Block Revision` on Question 2(b).** No CandidateRevision against the plan, the
spec, the manifest, or the answer key. No entry in `.agents/reports/reference-candidates.md`.

**R2c — Bounded local repair, deferrable.** When the plan is next edited for any reason,
replace `plan:494` with text that routes the record to writable destinations. Exact
proposed replacement:

> **Redesign trigger:** If no real sandbox backend exists on this host, do not fake
> containment. Configure `cross_cutting` to refuse dispatch, record the limitation in
> `docs/CONFORMANCE.md` (written at Task 27) and as a dated entry in
> `.agents/reports/reference-candidates.md` naming UNK-001, and keep the port so a backend
> can be added later. The manifest's `unknowns` are frozen and are never edited: UNK-001
> already carries this question, and its disposition is recorded outside the frozen
> artifacts.

**R2d — Deferral triggers.** If R2c is deferred, revisit when **any** of these occurs:

- **T1 — the defect goes active.** Any redesign trigger fires whose consequent requires
  writing to a frozen artifact and whose content is *not* already pre-recorded there.
  `plan:494` is the only such site today; audit any newly authored trigger against this test.
- **T2 — zero marginal cost.** The plan is edited in the Task-14 or Task-27 region for any
  other reason. Fold R2c in then.
- **T3 — the unknown moves.** UNK-001 (or any manifest unknown) needs a status change before
  Task 27 — e.g. a production deployment target is named, or a stronger backend is added —
  which would make "record in the manifest's `unknowns`" an instruction someone is actually
  tempted to follow.
- **T4 — Task 27 opens.** Writing `docs/CONFORMANCE.md` is the natural moment to reconcile
  every trigger that routes content to it.

---

## 4. Bounded claims

What this review does **not** establish, stated so no downstream reader over-reads it:

1. **No compile, no test run.** Build-lock discipline was observed: no `cargo`, `just`,
   or `nextest` was invoked. Every behavioral statement is a claim about what the source
   *asserts*. I have **not** verified that `p5_hard_gate`, `just doctor`, `just boundaries`,
   or `just ruler` currently pass. If P5 is red, the "bwrap genuinely contains on this host"
   leg of the PARTIALLY-RESOLVED verdict weakens from demonstrated to merely asserted — and
   UNK-001 would then be **STILL OPEN**, not partially resolved. **This is the single
   compile-dependent question I could not settle and am recording as unresolved, per
   instruction.**

2. **Digest not recomputed.** I read `.gauntlet/reference.lock`'s recorded digests. I did
   not run `scripts/reference-digest.sh` to confirm the frozen artifacts still match. I
   assert only that the lock *records* `sha256:95331d3e34…`, not that the manifest still
   hashes to it.

3. **Production is a negative claim from absence.** "No production deployment target is
   named" is grounded in not finding one across the plan, spec, manifest, environment
   report, and `CURRENT_STATUS.yml`. An operator may hold that knowledge outside the
   repository. If so, UNK-001's disposition changes and R1b should be revised — the absence
   of evidence here is genuinely the evidence I am reasoning from.

4. **Task-20 scope is inferred from a source comment.** "Production dispatch lands with
   Task 20" rests on `main.rs:1-11` and `plan:43`. I did not read Task 20's plan section in
   full; if it does not in fact compose `GovernDeps::with_backend`, the R3 residue persists
   past Task 20 and someone should say so at Task 27.

5. **Adapter internals read partially.** I read the bwrap adapter's first ~120 lines and
   header, and the policy-fs adapter's first ~160. I did not audit `wrap_builder_command`'s
   full argument construction, so I make no claim about whether the rendered bwrap invocation
   is *correct* in every policy shape — only that P5 asserts the boundary holds for the case
   it exercises and that the first process is the sandbox binary.

6. **Charter scope.** I recommend; I did not write to `.wayfinder/`, `.gauntlet/`,
   `.agents/CURRENT_STATUS.yml`, `.agents/plans/`, `.agents/specs/`, or
   `.agents/reports/reference-candidates.md`, and I mutated no git state. The only file this
   review created is this one.
