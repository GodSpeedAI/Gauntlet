# IN-FLIGHT — Task 16 (Generic CLI agent adapter + P8 substitution proof)

- **Started:** 2026-08-29T23:15Z (orchestrator session)
- **Phase:** builder + static design critic dispatched (concurrent; critic is
  static-only, no cargo — one compiler at a time)
- **Prereg:** frozen `f522dbd3be06695d685da426c5b1072303c6ae4dbba478cfe00acb6132bd878c`
  (`just prereg-check 16` PASS at freeze).
- **Baseline:** HEAD 054176b at dispatch; `just ruler` PASS at dispatch.
- **Slice plan:** ONE builder (B1), one phase —
  - cli adapter crate: template render (shell-word argv, no shell
    interpolation), process transport seam, runner (invoke/poll/cancel/
    collect), honest capabilities, strict contract admission;
  - ports revision: `InvocationFailure::MissingResult` + shared-suite
    shape-2 case (clean exit, no valid result ⇒ `Failed{missing_result}`,
    decided BEFORE the deadline); mock + prime factories learn the new
    script;
  - `crates/gauntlet-cli/tests/p8_adapter_substitution.rs` — full shared
    suite over mock + cli factories, zero test-body changes, adapter
    selection via test-harness flag/env;
  - doctor: measured RunnerDescriptor mirror for the cli runner (prime
    precedent); NO runners.default decision (UNK-003 stays the operator's);
  - env-gated live leg (trivial probe child through the REAL spawn path;
    no Unit, no gate).
- **Checkpoints:** builder writes `b1-progress.md` as it goes; final report
  to `builder-b1.md` (result contract: file exists + names every gate it
  ran + lists its touch surface).
- **Design review round (2026-08-29, pre-implementation):** blind static
  critic (`critic-design.md`) returned NOT_CONFIRM — 1 blocking / 5 major /
  6 minor; ALL accepted (`design-adjudication.md`); corrections RES-D1..D12
  routed to the builder mid-flight. The blocking find (C1/RES-D1): the
  shape-2 case must be a SEPARATE exported contract function, NOT a member
  of `agent_runner_suite` — the settled prime adapter's Annex C.4 one-input
  poll can never pass a universal process-exit case, and "prime stays green"
  would have been unimplementable as briefed. Other load-bearing corrections:
  envelope BODY must reach the child (sibling `<leaf>.envelope.json`, prime
  precedent); `GAUNTLET_RESULT_PATH` env channel (§10.3 rule 1);
  `dispose_failure` gains MissingResult grouped with Execution (row-2 retry);
  arch teeth re-scoped to a whole-crate scan of the cli crate only (the
  all-adapters scan was red-at-birth on settled BwrapSandbox/SqliteStateStore
  APIs); RunnerDescriptor gains a typed what-was-measured dimension so a
  config-only probe cannot emit "version probe measured clean".
- **Hard boundaries:**
  - NO real builder execution (R-1b stays open; suite over process doubles;
    live leg spawns a trivial probe child through no gate).
  - R-2 NOT discharged (fog decision D3 in the prereg: this adapter consumes
    a command template, never a wrapped SandboxedCommand; R-2 stays bound
    to the first wrap-consuming composition, task20).
  - The answer key is never opened for edit; AK-013's locator divergence
    (tests/proofs vs crates/gauntlet-cli/tests) is handled by a
    reference-candidates entry (D5).
- **Gate phase (2026-08-29 late):** builder B1 complete (report:
  `builder-b1.md`, contract satisfied; progress log `b1-progress.md`).
  All 12 RES-D corrections integrated. Builder battery: 1150/1150 workspace,
  boundaries 224/224, lint/deny/doctor/ruler PASS, p8 gate line PASS, live
  probe PASS first run (real pid, 0.257s, hygiene clean). Teeth shown RED →
  byte-identical restore (sha256-verified) → GREEN for both (a) vendor plant
  ⇒ boundaries RED and (b) TimedOut mutation ⇒ shape-2 suite RED. Recorded
  deviations D-dev-1..D-dev-6 (env-var selection; sh -c fixed-argv SIGTERM
  helper — libc/nix stays ask-first; RES-D1 sibling-suite shape; RES-D4
  row-2 disposition; scanner self-catch; literals-scanner posture).
  ORCHESTRATOR personal battery COMPLETE, all green (ruler PASS; check PASS;
  test 1150/1150; lint clean; boundaries 224/224; deny ok; doctor exit OK;
  p8 gate line PASS); additive-only claim on the shared suite personally
  verified (138 insertions, 0 deletions). Tree reconciled against the
  builder's touch surface — no unexplained files.
- **Falsifier round 1 (2026-08-29 evening):** the dispatched blind
  executing falsifier agent DIED at ~17 min (sub-agent 5-hour usage
  window exhausted; reset 2026-08-30T12:39). It had composed two
  executable attack files (template A1–A13; live B/C/D/F/H/J) — the
  live file never run. Per the standing heal+continue rule the
  ORCHESTRATOR completed the round: recovered both files (they were
  inside the untracked tests/ dir — invisible to a naive git-status
  reconciliation), repaired their harness bugs (single-poll race;
  `with_extension` vs suffix; one B4 expectation that contradicted the
  pinned shape-2 order), executed all 32 attacks + fresh floor attacks
  (p8 fail-closed, arch-scan evasion + control, doctor honesty legs).
  VERDICT: NOT_CONFIRM — F1 major (cancel escalation reported
  CancelFailed for a landed SIGKILL with pending reap; Cancelled verdict
  at risk of re-classification to missing_result), F2 major (comment-only
  template parsed clean; doctor probe reported an unusable runner
  usable), F3 minor (lib.rs doc wrong about collect), F4 major (terminal
  pin left a live child running; cancel unreachable). Blindness of the
  orchestrator-executed portion is DEGRADED and recorded; the attacks
  were composed blind by the fallen agent; retained in-tree as
  regression teeth (`tests/falsifier_round1_{template,live}_attacks.rs`).
  Round record: `falsifier-round1.md`.
- **Correction round 1 (orchestrator, tests-first):** COR-F1
  (transport reap gets a dedicated grace; Ok(None) = stopped, code
  unobserved; StopFailed reserved for real failures), COR-F2
  (zero-word template ⇒ parse refusal Empty + exported `probe_template`
  parse+render probe; doctor cli leg uses it — verified end-to-end:
  hostile templates now FAIL check 3 honestly), COR-F3 (lib.rs doc),
  COR-F4 (`retire_child` at terminal pins — no invocation outlives its
  verdict with a live child; stop outcome never a state input). RED
  evidence: 29/32 with exactly the three corrected-teeth tests failing
  pre-correction; 32/32 after. Record: `correction-round1.md`.
- **Post-correction battery (orchestrator, verbatim):** workspace
  1182/1182; lint clean (fmt applied to the retained test file);
  boundaries 224/224; deny ok; doctor exit OK; p8 gate line PASS;
  ruler PASS; prereg-check 16 PASS (frozen hash untouched). Hygiene:
  stray probe children killed, today's /tmp scratch removed, arch plants
  restored byte-identical (sha256 -c OK), `.gauntlet/` clean. 11 stale
  `/tmp/gauntlet-falsifier-*` dirs from 2026-08-28 (task15's day)
  recorded, left for a dated cleanup decision.
- **CONFIRMATION STATE: PENDING ROUND 2.** Round 1 was NOT_CONFIRM and
  the corrections are verified, but the discipline (task13/14 lesson)
  requires a FRESH BLIND round — including an executing falsifier —
  before settlement. Sub-agent dispatch is unavailable until
  2026-08-30T12:39 (usage window). NEXT ACTION on resume: dispatch the
  fresh blind falsifier (round 2) against the corrected tree with the
  same brief constraints (firewalled from builder/critic/orchestrator
  reports; may read falsifier-round1.md's NOT-ATTACKED list as its
  warm-start); on CONFIRM → closure records (closure.md, prereg
  post_run, CURRENT_STATUS) → settle. The work is committed as a
  mid-task milestone (operator-sanctioned deviation class) to protect
  it across the window.
- **Teeth owed at closure (orchestrator, personally):** vendor identifier
  planted in the cli crate's public API ⇒ `just boundaries` RED ⇒
  byte-identical restore ⇒ GREEN; the shape-2 suite case RED on a broken
  classification ⇒ restore ⇒ GREEN.
- **Recovery note:** if this session dies, reconcile `git status` + diff
  against this file and `b1-progress.md` before re-dispatching (tree
  outranks record). One compiler at a time; package-scoped checks first;
  WSL2 memory is tight (~1.6 GiB free).
