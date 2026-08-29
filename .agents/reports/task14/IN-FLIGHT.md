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

## Update 2026-08-28 — correction rounds 1 and 2 landed; round 3 dispatched

**Correction round 1 (COR-1..COR-3), `correction-round1.md` — all FIXED.**
Verified battery: workspace 1065/1065, ports sandbox 6/6, three adapters
12+13+16, app govern::/doctor:: 44/44, p5+g7 7/7, lint 0, fmt clean,
boundaries 215/215, ruler PASS.
- COR-1 quotes every rendered word at the END of assembly (a future `push`
  cannot forget the quoting because no push does the quoting); conservative
  allowlist; `policy_findings()` deliberately unchanged so spaced paths stay
  legal. Regression in the SHARED port contract suite + broken stub.
- COR-2 implements §17.5.2 in order: lexical normalization -> component-wise
  containment (`/ws-evil` is not inside `/ws`) -> canonicalized re-check for
  symlink escape, with `existing_canonical()` canonicalizing the deepest
  EXISTING ancestor so a not-yet-existing path neither falsely refuses nor
  bypasses the symlink check.
- COR-3 made the malformed shape UNREPRESENTABLE (private `Containment` enum)
  rather than merely rejected, and changed the return type to
  `Result<BuilderCommand, WrapRefusal>` so `if let Some(c) = wrap(..) { run(c) }
  else { run(raw) }` no longer type-checks. Follows the Task-13
  `AdmittedSettlement` earned-capability precedent.

**Blind round 2 (`final-confirmation-round2-falsifier.md`): NOT CONFIRM.**
It independently re-attacked the corrections and BROKE NEITHER — 10 quoting
payloads and 8 anchor-escape shapes all refused. But it found a MAJOR round 1
had missed:
- round2-F1 MAJOR: a `denied_paths` entry NESTED inside a `writable_roots`
  entry was silently re-exposed read AND write; executed end-to-end through
  real PolicyFs + gate + bubblewrap with the host file overwritten ("PWNED").
  `render` emitted masks BEFORE the writable binds and bubblewrap applies
  mounts in argument order. The adapter's own doc comment stated the correct
  rule and the code implemented its inverse. Missed by every prior test
  because every denied path in the suite is a SIBLING of the writable root —
  nested is the only case where "denied even if otherwise writable" has content.
- round2-F2 MINOR-but-real: an UNANCHORED absolute path was carried verbatim
  and bounded by nothing; `writable_roots: ["/"]` validated CLEAN and rendered
  `--bind / /`.
- round2-F3 MINOR: AK-044's frozen locator had teeth only on the SCHEDULER
  conjunct, not the gate's own conjunct 1 (round 1 found this independently
  as its F7 — two blind rounds converging).

**Correction round 2 (COR-4..COR-6), `correction-round2.md` — all FIXED.**
Battery: ports 110/110, three adapters 45/45 (both live bwrap confinement legs),
govern::/doctor:: 44/44, p5+g7 7/7, boundaries 215/215, lint PASS (recorded
honestly: the FIRST lint invocation failed on FORMATTING ONLY, zero clippy
findings; `cargo fmt --all` then PASS), ruler PASS.
- COR-4 emits writable binds BEFORE masks so masks outrank the binds enclosing
  them; both inverted doc comments corrected; live bwrap legs prove the nested
  denial is neither readable nor writable and the host original survives.
- COR-5 removed the unanchored-absolute passthrough entirely (every entry must
  be anchored) AND added a second line of defense in bwrap `policy_findings`
  (`names_filesystem_root`, deliberately lexical so a nonexistent path cannot
  dodge the bound). Shipped artifacts VERIFIED to still resolve, not assumed.
- COR-6 added coverage only, keeping the scheduler leg byte for byte, and
  asserts the refusal VARIANT is `GateRefusal::Irreversible`. Acceptance
  demonstration: disabling the GATE conjunct alone now FAILS the frozen locator;
  CONTROL: disabling the SCHEDULER conjunct alone also fails it, on the
  pre-existing assertion — proving the added coverage did not displace the old.

**Orchestrator dispositions (not delegated):**
- round2-F4 (earned-type discipline stops at the PORT boundary:
  `SandboxedCommand`/`PolicyValidation` public fields, `BuilderCommand::Unwrapped`
  public variant) DEFERRED to the ports revision, trigger Tasks 15/16. Rationale:
  a port-contract change rippling through every sandbox adapter and the shared
  suite, with no composed consumer today (round2-F6). Recorded, not silently dropped.
- round1-F5 (the `contained` tier binds host root read-only with `denied_paths: []`,
  so a contained builder can READ `~/.ssh/id_ed25519` and `.wayfinder/answer-key.md`)
  is literally spec-conformant (§17.3 puts credential denial at `local` and above;
  §17.5.5 constrains only WRITABLE paths) — so NOT a Task-14 defect, and NOT patched
  mid-correction. Recorded as a residual plus a CandidateRevision candidate against
  the spec, because a builder able to read the frozen ruler can target its assertions
  without ever modifying it. Escalated to the operator.
- F11/denylist `none` addition: reviewed personally. Justified, precedented
  (Task 11 did the same for `digest`/`check`), pinned by `assert_eq!`, and the
  distinguishing spellings stay denied. NOT a gate weakened to land the task.

**Interruption instances 24 and 25:** the first COR-1..3 dispatch timed out after
38 tool uses (tree verified byte-identical, nothing lost); the first round-3
falsifier died to the usage window after mapping the surface only (stub had no
findings). Both reconciled against an orchestrator-held sha256 baseline before
re-dispatch. Baseline re-cut after each correction round.

NEXT: blind round 3 in flight. On CONFIRM -> orchestrator personally re-verifies
teeth + full battery, then outcome.yaml, closure record, CURRENT_STATUS, ONE commit.
