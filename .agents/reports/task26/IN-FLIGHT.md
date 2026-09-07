# IN-FLIGHT — task 26 (baselines A/B/C/D) — SETTLED 2026-09-07

The task is settled; this ledger is the historical record. The
cross-session anchor is `.agents/CURRENT_STATUS.yml` (task26 SETTLED,
ready set task27). The full ladder, dispositions, incidents, and
residuals: `closure.md` (authoritative) plus the round reports in this
directory.

## Final state

- B1/B2/B3: COMPLETE. COR-1..COR-5: COMPLETE. Critic r1: NOT_CONFIRM
  → resolved. Falsifier r1: NOT_CONFIRM → resolved. Falsifier r2:
  NOT_CONFIRM → boundary adjudication → COR-5 → resolved-as-recorded.
- Closure battery (orchestrator, personal): check 0; workspace test
  1625/1625; lint green (after the 3-needless-borrow surgical fix,
  3aac837f→77c32618); deny 0; boundaries 237/237; ruler PASS; the
  plan gate VERBATIM at the frozen locator (exit 0, keys exactly
  A,B,C,D,_meta); personal manifest tooth RED → byte-identical
  sha256 restore 77c32618 → GREEN.
- Settlement: one closure commit (explicit pathspecs; never
  .jolli/, CURRENT_STATUS*, prompts/), post-commit ruler re-run.

## Residuals carried OUT of task 26 (see CURRENT_STATUS unresolved_summary)

The executed none-posture judge-forgery boundary (reference-candidates
6); F6 adapter run_id leniency; F7 durable-word vocabulary note;
instrument-run denominator comparability (reference-candidates 7);
the falsifier-r2 not-attacked list; the dead-r2 incident record.

## Slices

- **B1** (fixture task set — `tests/fixtures/baseline/**` only):
  dispatched 2026-09-06. Progress: `b1-progress.md`. Completion
  contract: `b1-report.md` (observed script-level scenario matrix
  included). NO settlement authority.
- **B2** (app module — `crates/gauntlet-app/src/baseline/**` +
  `crates/gauntlet-app/tests/baseline_*.rs` only): dispatched
  2026-09-06, in parallel with B1 (file-disjoint). Progress:
  `b2-progress.md`. Completion contract: `b2-report.md`.
- **B3** (CLI wiring + gate test — `crates/gauntlet-cli/**` only):
  dispatched 2026-09-06 once B2's API landed. Charter §Slices.

## State

- B1: COMPLETE (fixture verified by orchestrator spot-check: lock
  digests reproduce via scripts/reference-digest.sh; sh -n clean;
  task.yaml matches the charter contract; observed script-level
  matrix == charter matrix).
- B2: COMPLETE (package battery 618/618, check clean; API in
  b2-report.md; deviations disclosed — reviewer wire token, critic
  body = requirement verbatim, MicroUsd cost unit, strict task-doc
  refusals).
- B3: COMPLETE (A/B/C charter rows verified EXACTLY over the real
  binary; T2 green; golden snapshot regenerated; 138/139 with the one
  red = the recorded D1 blocker; four cross-slice defects D1-D4
  reported, not patched; documented deviation: per-leg env channel
  re-architected as files + static template + EnvLayer injection +
  one safe re-exec because `unsafe_code = "forbid"` forbids set_var).
- COR-1: COMPLETE (all four defects resolved fixture-side; D-leg table
  over the real binary: settles honest+subtle, refuses lying+
  compromised with zero settlements, silent escalated-bounded reserve
  intact; script matrix matches; batteries green except b3's own
  superseded deviation pins; D4 root causes: AK-962 tier vs none
  posture, 0644 script modes killing the test facet, absent-artifact
  materialization mechanism [b3's stdout story corrected];
  permission-denied corroboration loop WAS the "no residual records"
  world — the records were always in the store, b3's export-only read
  under-reported).
- Orchestrator adjudications recorded (charter §Adjudications):
  flip the b3 pins; D held-out via instrument-snapshot wrapper
  (release_workspace has no retention knob — verified); payment tie
  recorded honestly as the AK-069 D-does-not-win case; `escalated`
  accepted as the silent leg's bounded terminal.
- COR-2: IN FLIGHT (work item 1: instrument-snapshot held-out surface
  for D at the composition root; work item 2: gate-test pin flip +
  honest payment assertions).
- COR-2: COMPLETE (instrument-snapshot held-out surface delivered +
  wrapper contract proven byte-identical; gate-test pins flipped to
  the adjudicated rows; batteries: improve_baseline 5/5, cli 139/139,
  app 618/618; disclosed deviation: the GAUNTLET_TOOL_TEST binding is
  a static judge shim resolving the per-leg wrapper via the leg-files
  channel — process-static env, set_var forbidden).
- Critic r1: COMPLETE — blind design-as-built NOT_CONFIRM (3 BLOCKING
  arch-gate violations in the new code [adapter token in app-layer
  strings; 290 hardcoded-literal sites; 25 un-inventoried write-API
  sites] / 1 MAJOR = the settlement act itself [frozen locator +
  narrative, orchestrator-owned] / 4 MINOR [Some(0) reliability on
  empty legs; fallback basis not in the recorded report; shared run_id
  undisclosed; sub-second wall = 0] / 7 informational). Positively
  verified by execution: the matrix, the honest-measurement audit (no
  invented figures, no circular judge, no INCONCLUSIVE-as-PASS), the
  snapshot wrapper's contract preservation. Report: critic-design.md.
- COR-3: COMPLETE (all critic residuals landed: B1 token-free app
  disclosures; B2 literals 290→0 with no gate changes, verified
  against the real scanner with a throwaway probe; B3 26 inventory
  entries reason+guard, pin test in lockstep; m1 null reliability
  RED-first; m2 HeldOutBasis through leg JSON + _meta disclosure;
  m3 run-id scope disclosure; m4 Instant-pair wall RED-first.
  Batteries: arch 237/237, app 620/620, cli 139/139, real gate shape
  exit 0 with all-PASS python verification, ruler PASS. Disclosed
  deviations: child wrapper shebang from $SHELL fail-closed; harness
  Clock dropped for Instant; one self-caught nested-flock, killed and
  re-run clean, recorded).
- Falsifier r1: COMPLETE — blind executing NOT_CONFIRM (1 CRITICAL:
  F1 the D judge channel forgeable — child overwrites the wrapper via
  the dirname chain it was taught, plants snapshots; executed repro
  recorded held_out=pass/reliability=10000 on defective work / 2
  MAJOR: F2 planted snapshot on a no-facet leg (false basis), F3
  task.yaml id traversal out of the scratch root / 2 MINOR: F4
  instrument-run denominator not disclosed in _meta, F5 held-out
  crash conflated with substantive fail / 2 informational (F6
  non-echoing run_id admitted by the shipped adapter — task-16
  territory; F7 durable word budget_exhausted vs charter "refuses").
  T1-T5 ALL bit, sha256-restored byte-identically. 8+9 codec shapes,
  8 malformed-child shapes, both judge daemons, unwritable --out all
  repelled honestly. AK-069 probe: D-does-not-win visible in the JSON
  alone. Report: falsifier-round1.md.)
- COR-4: IN FLIGHT (F1 judge provenance fail-closed: wrapper sha256
  pin + nonce-named snapshots + durable facet-count cross-check +
  disclosed substitution boundary; F2 closed by the cross-check +
  regression; F3 codec id validation + executor containment, RED-first;
  F4 denominator disclosure; F5 exit-contract 0/1/other→inconclusive
  RED-first; F6/F7 recorded for closure.)
- COR-4: COMPLETE (F1 judge provenance fail-closed: wrapper sha256
  pin + ULID-nonce snapshots + durable facet-count cross-check named
  over observation bindings → bundle docs → test FacetRecords +
  _meta substitution-boundary disclosure; F2 closed + real-binary
  regression; F3 codec Component guard RED-first + executor
  containment; F4 denominator disclosure; F5 0/1/other exit contract,
  false_completion now Option; F6/F7 restated for closure. Teeth: 5
  new mutations each turned a named test RED, sha256-restored; both
  falsifier repros retained in-tree over the real binary. Batteries:
  arch 237/237, app 627/627, cli 149/149, fmt+clippy clean, gate
  shape exit 0 with 10 disclosures, ruler PASS.)
- Falsifier r2 attempt 1: DIED on a usage-limit window mid-round
  (the task14/15/16 class). Reconciliation per the documented
  procedure: lock free, no live compiles; the dead agent's ONE
  un-restored mutation was CAUGHT by the personal reconciliation
  battery (the F3a codec guard in task.rs carried a `true ||`
  short-circuit — its own teeth tests went RED) and restored by the
  orchestrator with sha256 records (task.rs f403b1b6… → 3710b6d7…,
  /tmp/task26-restore-before.txt); scratch residue under /tmp
  cleaned. Post-restore reconciliation: arch 237/237, app 627/627,
  cli 149/149 — the tree is the COR-4 final state. LESSON CARRIED:
  r2 agents must mutate-restore ONE AT A TIME and write their report
  progressively.
- Falsifier r2 attempt 2: COMPLETE — NOT_CONFIRM. Round-1 fixes held
  (F1/F2/F3 vectors REPelled; both new teeth bite, sha256-restored;
  regression sweep all green). But two NEW CRITICALs of the same
  forgery class landed through the judge channel: FR2-1 wrapper
  swap-window with self-restore (digest verified only at grade time),
  FR2-2 in-place snapshot content rewrites (5ms daemon beat the
  judge; "a race, not an invariant"), FR2-3 count-preserving
  .absent→.rs replacement (caught once, landed once). Structural
  fact, three-times demonstrated: same-user child ⇒ no judge material
  in its reach is unforgeable.
- ORCHESTRATOR ADJUDICATION (FR2): recorded BOUNDARY, not a fixable
  defect — a privileged judge channel does not exist in the fixed
  none posture (would be runtime/sandbox change, task-25 territory);
  the baseline's declared threat model is FIXED scripted children
  (repo-owned, git-pinned) and the measured rows stand under it; the
  hostile-child forgery class becomes an EXECUTED boundary disclosure
  + a reference-candidates entry (settlement) + one best-effort
  hardening layer. No third falsifier round for the same class: the
  residual is the disclosed boundary itself; the r2 not-attacked list
  carries forward as triggers.
- COR-5: IN FLIGHT (W1 nonce-manifest snapshot hardening, best-effort,
  RED-first teeth; W2 executed-boundary disclosure reword; W3
  cor5-report.md).
- Closure (orchestrator): pending — personal battery, teeth
  re-verification, frozen-locator gate run, narrative (F7 wording +
  the payment ratio + the boundary), reference-candidates entry,
  settlement commit, CURRENT_STATUS.
- Closure (orchestrator): pending — personal battery, teeth
  re-verification, frozen-locator gate run, narrative, settlement.

## Residuals carried in from task 25 (watch list)

- b1_contract_wins_over_exit7 load-flaky under full-workspace load —
  green-in-isolation ⇒ record, move on (charter §Battery).
- Ticket↔unit binding on escalation records: ports revision, open.
- The five ruler-owner reference-candidates items (2026-09-06): carry;
  nothing here touches the ruler.
