# Task 14 — IN-FLIGHT (orchestrator dispatch ledger)

Task 14 — Authority gate, sandbox port, tier policies (proves P5).
Targets: AK-044, AK-045, AK-046, AK-047, AK-048, AK-050 (AK-049 already
satisfied since Task 5 — doctor check 5; verification re-run in the final
battery). Prereg frozen (sha256 0ba5ce79…) before implementation began;
`just prereg-check 14` PASS. Recon: `recon.md`.

Loop plan (one builder at a time — single-compiler/build-lock discipline):

| Slice | Result contract | State |
|---|---|---|
| B1 sandbox adapters (none + bwrap) + policy-fs adapter + in-repo tier policies + 3 new sandbox contract cases w/ green+broken stub teeth | `builder-b1.md` | Builder died to the 5h sub-agent usage window (21st instance; resets 2026-08-29 01:44) — but NOT before landing work: tree shows Item 4 complete (ports suite +3 cases, +2 broken stubs, +2 teeth; ports 107/107 per b1-progress.md), all three adapter crates + Cargo.toml registration + `.gauntlet/policies/*.json` landed, progress file current through "Item 1 code written". Recovered per standing heal+continue: orchestrator verifying/completing B1 personally from the tree (tree outranks record) |
| B2 app `govern::` (tier posture, AuthorityGate, capability-vs-tier, downgrade rejection) + controller dispatch wiring + doctor check 4 + p4/p6/p12 fixture posture moves | `builder-b2.md` | pending |
| B3 proofs p5_hard_gate + g7_tier_downgrade (composition root wires REAL bwrap + policy-fs) + teeth | `builder-b3.md` | pending |
| Critics (spec + executing falsifier), blind, charters verbatim | `final-confirmation-*.md` | pending |
| Correction round(s) as required | `correction-roundN.md` | pending |
| Gates + teeth re-verified by orchestrator; one-task-one-commit | — | pending |

After any interruption: `just ruler` first; `scripts/reconcile-checkpoints.sh 14`;
reconcile `git status` vs CURRENT_STATUS.yml vs this file before treating any
slice as unstarted; resume at the missing step per
`.agents/lessons/session-interruption-recovery.md` (the tree outranks the
record). Never redo recorded work.

## Update 2026-08-28 ~15:40 UTC (orchestrator)

- B2 CLOSED (heal+continue; `builder-b2.md`): govern module (posture+gate,
  11 tests), controller dispatch wiring (gate before reservation/spec/
  invoke; refusals = pinned evidence + first-refusal ticket + Blocked
  terminal; sandbox_policy ref on the spec; exercisable authority one
  source), doctor check 4 real (composition-root MEASURED descriptor; the
  CLI's FIRST production adapter wiring measures the real backend live),
  p4/p6 fixture posture moves with recorded reasons. Gates: 1049/1049.
- B3 CLOSED (heal+continue; `builder-b3.md`): p5_hard_gate 4/4 (incl.
  EXECUTED bwrap confinement + shipped-policy compose) and g7 3/3;
  teeth re-verified on the FINAL tree (capability conjunct RED→restore→
  GREEN; irreversible double-line RED→restore→GREEN). Final battery:
  workspace 1056/1056, lint/fmt/boundaries 215/215/doctor/deny/ruler/
  prereg ALL GREEN.
- NEXT: reference-candidates entries (locator class), UNK-001 update,
  outcome.yaml draft, then the BLIND CONFIRMATION ROUND (critic +
  EXECUTING falsifier charters) — blocked on the sub-agent usage window
  (resets 2026-08-29 01:44 UTC). Probe + schedule if still exhausted.

## Update 2026-08-28 (orchestrator — resumed session, team dispatch)

Reconciled tree vs record before treating anything as unstarted (tree outranks
record). Established state:

- Prereg FROZEN (`task-14.prereg.yaml` + `.sha256`, 2026-08-28 09:54) before
  implementation. DONE.
- B1/B2/B3 all CLOSED and landed (three adapter crates, `.gauntlet/policies/*.json`,
  `govern::`, controller dispatch wiring, doctor check 4, p5_hard_gate + g7 proofs,
  ports sandbox contract suite +3 cases +2 broken stubs). DONE.
- Confirmation round A (spec critic, `final-confirmation-a-spec.md`): **CONFIRM**,
  10 gates re-run independently + 4 teeth legs mutation-verified, 3 informational
  observations (OBS-1/2/3), none blocking or major. DONE.
- Confirmation round B (EXECUTING falsifier): **INCOMPLETE** — the report is a
  stub, all 8 attacks `(pending)`, agent died to the sub-agent usage window
  (22nd interruption instance). This is the one open confirmation loop.
- Closure artifacts NOT written: reference-candidates entries (AK-044..AK-050
  locator class), UNK-001 disposition, `task-14.outcome.yaml`, closure record,
  CURRENT_STATUS update, the single commit.

Orchestrator captured an independent sha256 baseline of all 181 `crates/**/*.rs`
files + the three policy artifacts + root `Cargo.toml` before dispatch, so
falsifier teeth-mutation restoration is verifiable by the orchestrator rather than
on the falsifier's own say-so.

Team dispatched (charters adopted verbatim on general-purpose transport; the
`.agents/agents/` types are still not registered as invocable subagent types):

| Agent | Charter | Slice | Compiles? |
|---|---|---|---|
| A1 | falsifier | Blind EXECUTING confirmation round B — restart from scratch | YES (sole compiler) |
| A2 | semantic-verifier | reference-candidates entries, AK-044..AK-050 locator class | no |
| A3 | structural-reviewer | UNK-001 disposition + frozen-manifest instruction tension | no |
| A4 | finisher | `task-14.outcome.yaml` + closure record (gated on A1 verdict) | no |

Build-lock preserved: only A1 compiles; A2/A3/A4 are text-only. Orchestrator
retains settlement authority, personally re-verifies teeth + the final battery,
and owns the CURRENT_STATUS update and the single commit.

## Update 2026-08-28 — blind confirmation round 1 result: NOT CONFIRM

Round-1 team closed. Results:

| Agent | Outcome |
|---|---|
| A1 falsifier (executing, blind) | **NOT CONFIRM** — 11 attacks executed, 13 attack sections, 2 executed MAJORs. Report: `final-confirmation-b-falsifier.md` |
| A2 semantic-verifier | DONE — AK-044..AK-050 locator-class entries appended to `reference-candidates.md` (+25KB) |
| A3 structural-reviewer | DONE — `structural-review-unk001.md`; UNK-001 = PARTIALLY RESOLVED (not closed) |
| A4 finisher | CORRECTLY NOT RUN — the workflow's own guard halted closure on the NOT CONFIRM |

**The round-1 lesson from Task 13 reproduced exactly.** Critic A (spec track) independently
re-ran ten gates and mutation-verified four teeth legs and returned CONFIRM. The EXECUTING
falsifier then broke the same surface in two places. A static/gate-only CONFIRM is again
demonstrated to be weaker evidence than an executing falsifier. Keep the executing role in
every future confirmation round; this is now the second consecutive task where it was
decisive.

Falsifier MAJORs (both executed, both in landed Task-14 code, neither adversary-requiring,
neither caught by any test in the repository):

- **F1** — `BwrapSandbox::render` does `words.join(" ")` with policy `writable_roots` /
  `denied_paths` pushed RAW; only the command payload is quoted. A policy string
  `/real/path --share-net` becomes a real bwrap flag after `--unshare-net`, and
  last-flag-wins silently defeats the declared `deny_by_default` posture while
  `validate()` reports CLEAN. Proven end-to-end through the real `PolicyFs` reading a real
  policy file: clean policy -> sandbox sees only `lo`; hostile policy -> sandbox sees the
  HOST namespace. Non-adversarial trigger: any operator whose `$HOME` or workspace root
  contains a space mis-splits the SHIPPED `local.json` / `cross_cutting.json` masks.
- **F2** — `resolve_anchored_path` does token substitution + `is_absolute()` and nothing
  else. `$WORKSPACE_ROOT/../outside` escapes the anchor. Falsifies the module's OWN
  documented invariant ("an artifact cannot widen its own anchor") and does not apply spec
  §17.5.2 (prefix check AFTER canonicalization; symlink escape checked, not assumed absent).

Orchestrator verified tree restoration INDEPENDENTLY (not on the falsifier's say-so):
all 185 baselined files byte-identical, scratch attack target deleted, `.gauntlet/policies/`
clean of the planted `irreversible*` probes, answer-key and manifest digests unchanged.

**Interruption instance 23:** the first correction dispatch died on the session usage limit
with 0 tool uses / 1.7s. Tree re-verified byte-identical afterwards — nothing to reconcile,
no work lost. Re-dispatched after the window reset.

Correction contract dispatched (builder charter, `correction-round1.md`):
COR-1 = F1 (quote every rendered word; explicitly NOT by rejecting spaces in paths),
COR-2 = F2 (canonicalize + prefix re-check per §17.5.2, typed Invalid finding on escape),
COR-3 = F4 (`GateAdmission` unforgeable + malformed admission is a typed error, never
`Ok(None)`; follow Task 13's COR3 earned-kind precedent).
F3/F5/F6/F7-F11 explicitly OUT of correction scope — orchestrator dispositions them as
recorded residuals with triggers. Round 2 is a FRESH BLIND executing falsifier that may
not read round 1's report or the builder's correction account.
