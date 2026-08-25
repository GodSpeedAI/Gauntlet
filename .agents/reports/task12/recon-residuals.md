# Task-12 recon: every deferral-ledger residual triggered by the Task-12 controller

Date: 2026-08-23. Read-only recon. Sources: `.agents/reports/task{6,7,8,9,10,11}/*`,
`.agents/CURRENT_STATUS.md` (deferral ledger, lines 513-727), `.agents/lessons/*`.
Upgrade rule (CURRENT_STATUS.md l.726-727, l.515-516): "a deferred finding **upgrades to
blocking** the moment a task walks into its trigger." Task 12 = "Controller: selection,
budget, diagnosis (proves P4, P6)" (CURRENT_STATUS.md l.1064). Everything below names
Task 12 (or a Task-12-owned surface) as its trigger and is therefore blocking for the
Task-12 builder brief.

---

## 1. Ungated evidence-production write + declared-iteration validation at write time

**Original finding** — `task11/critic-b-result.md` l.27-31, "MAJOR-3 — `record_evidence_production` is an ungated public write: forged provenance for any store-resolvable bytes (self-declared residual, but it is the trust root of every §14.3 check)". Key sentences (verbatim):

> "ports `state_store.rs:337-341` records the residual honestly ("composition-root-wired with no role check in the store")"
> "Attack trace: any code holding the `Arc<dyn StateStore>` writes `EvidenceProduction::new(ref_of_any_existing_bytes, Identity("gauntlet-verify-test-report", ..), required_locator, current_iteration, now)`. Every §14.3 binding check then passes by construction ... a *fresh ref for the same bytes* is trivially recordable (content-addressing forbids nothing), and the same ref under a different run is a new key."
> "Grep confirms no production caller of `record_evidence_production` exists yet (only adapter self-tests and the p12 fixture) — so this is a seam Task 12 must close, exactly as the builder recorded."
> "Fixed would be proven by: capability-gating the write to the evidence-owner role at the store (as `bind_observation_bundle`'s doc already aspires to), or minting production facts from the observation pipeline rather than accepting them as arguments."

The declared-iteration half is MAJOR-2's second half (`critic-b-result.md` l.24-25): "a writer can record iteration N for iteration-1 bytes ... within-iteration ordering ... is unimplementable today"; "Fixed would be proven by: ... the store (or Task-12 wiring) refusing a production whose declared iteration exceeds the run's durable current iteration."

**Adjudication** — `task11/correction-adjudication.md` l.24, disposition **C7** (verbatim):

> "C7 | B-M2 (second half) + B-M3: ungated `record_evidence_production` write; declared iteration unchecked at write | RECORDED DEFERRAL, upgrade trigger Task 12 (capability-gate the evidence-owner write + validate declared iteration there). Builder and both critics converge; ports doc already carries the residual. Ledger entry in CURRENT_STATUS."

**Task-10 identical residual** — `task10/final-confirmation-round7.md` l.29-37, MINOR finding 2 "The observation-owner write is ungated." (verbatim key text):

> "`bind_observation_bundle` is a plain public port write and `ObservationBinding::new` is public, so any in-process holder of the real state store can DEFINE the observation for a not-yet-bound iteration (write-once still prevents substitution). This is asymmetric with `accept_comparison`, which is capability-gated. The critic names it the largest residual risk and suggests a capability token on the observation-owner write. Recorded as the successor to the "no production writer" residual: when the Task-12 controller wires the real writer, that write should be gated, not merely called."

Predecessor wording — `task10/correction-round10.md` l.107-112, residual 1: "`bind_observation_bundle` has no production writer. `grep` finds it only in tests, the sqlite impl, and the port default ... Wiring the observation owner belongs to the Task-12 controller; recorded here rather than papered over."

**Ledger** — CURRENT_STATUS.md l.518-526: "Trigger: **Task 12** — capability-gate BOTH writes when the controller wires their production writers (evidence-owner and observation-owner roles)." Also l.89-94 (Task-10 recorded MINOR residuals): "the observation-owner write (`bind_observation_bundle`) is ungated and has no production writer yet — when the Task-12 controller wires it, that write should be capability-gated."

---

## 2. Workspace-root pairing is convention-bound at the library seam

**Original** — `task11/final-confirmation-round2.md` l.91-93, non-blocking NOTE-2 (verbatim):

> "Workspace-root pairing with the adapters' root is convention-bound at the library seam (same §7.1 composition-root trust position as the ladder); Tasks 12/20 production wiring must keep them one fact."

**Ledger** — CURRENT_STATUS.md l.532-537: "`for_run` accepts any root; nothing in the library forces it to equal the root the adapters were constructed with. Same §7.1 composition-root trust position as the ladder. Trigger: **Tasks 12/20** production wiring must keep them one fact (the composed fixture already does)."

**Task-10 equivalent class**: no task10 note names workspace-root pairing specifically; the shared ancestor is the recorded §7.1 composition-root trust residual — `task10/final-confirmation-round7.md` l.17-19: "Attacker-supplied `StateStore`/`EvidenceStore` still yields a context — the acknowledged §7.1 residual, documented at the port"; cf. `task10/correction-round8.md` l.152 and `task11/correction-round1.md` l.111-115 ("the composition root itself is trusted (that is §7.1's position, unchanged)").

---

## 3. Observer process timeouts + subprocess output-size caps

**Originals**:
- `task9/critic-a-result.md` l.22-23, **M2**: "The four subprocess adapters block unboundedly in time and capture unboundedly in memory; the `Deadline` parameter is not enforced on the child." — build/test/command/git observers "call `std::process::Command::output()` with no timeout and no output-size cap; the `deadline` parameter is only forwarded to the evidence-store write"; "Spec §18.4 explicitly assumes 'test hangs' as a failure the system must dispose of; a hung build/test command instead hangs `observe()` forever"; "this is the strongest AGENTS.md bounded-autonomy violation in the artifact."
- `task9/critic-b-result.md` l.91, **BIT-7a**: "no timeout: a builder-authored command can hang the whole observation (MAJOR; documented deferral)" — "a lying builder declares `sleep infinity` and no bundle, no gap record, no divergence ever exists — suppression by denial of service rather than by dropping an entry. Discriminating test: a self-check of `sleep` past the deadline; today the assembler blocks forever rather than recording a gap." (Also table row l.124 and the fix-first list.)

**Adjudication** — `task9/correction-adjudication.md` l.68-72, ledger deferral **D1** (verbatim):

> "**D1 ← B-7a + A-M2 (MAJOR):** observer process timeouts and subprocess output-size caps. Extends the standing Task-8 "deadlines noted, not consulted" deferral to `observe()` explicitly. Trigger: **Task 12** (controller deadline composition — §18.4 hang disposal needs the controller anyway)."

**Ledger** — CURRENT_STATUS.md l.562-567 (same wording, trigger Task 12).

---

## 4. Runner deadlines modeled, not enforced

**Original** — `task8/critic-a-result.md` l.51, MINOR finding 2 (verbatim core):

> "**The runner never consults any `Deadline`.** `RoleInvocationSpec.deadline` is stored but never read; `collect`'s deadline parameter is ignored (`runner.rs:350-354`); §10.3 rule 5's "no result by deadline" is modeled as `timeout_after_probes` poll counts ... the deadline fields are dead weight in this adapter and rule 5 is *modeled*, not *enforced*."

**Adjudication** — `task8/correction-adjudication.md` l.20-23, NO-CHANGE (verbatim):

> "**A-MINOR-2 deadlines never consulted:** port-wide recorded convention (Task-6 ledger: "Deadlines noted, not consulted"); the time-free design is documented in the runner's module docs and the suite accepts the probe surrogate for scripted adapters."

**Ledger** — CURRENT_STATUS.md l.617-620: "Runner deadlines modeled, not enforced (A-m2): probe counts stand in for §10.3 rule 5 deadlines ... Trigger: **any task composing real deadline behavior (Task 12 controller)**."

---

## 5. Iteration-boundary / settlement teeth are harness-only

**Original** — `task7/critic-a-result.md` l.98-114, **MAJOR-4** "The done-when 'teeth' holds only inside the test's own harness; both-digests-as-evidence not yet persisted" (verbatim core):

> "the only iteration-boundary callsites are inside the P1 test itself ... and the Task-4 contract-suite stubs; grep shows no production caller of `verify_unchanged`. Deleting the helper call from the test's loop does fail the test ..., but that is a harness-internal, self-referential tooth ... Likewise both digests live in the typed error ... but are not persisted as evidence anywhere (no event, no evidence object ...) ... the plan itself sequences the controller at Task 12 and settlement at Task 13 — so this is a documented, owner-assigned deferral, but the done-when clause is not dischargeable as written at Task 7."

Obligation table l.39: "Step 3: verify_unchanged at every boundary + before settlement ... **Primitive met; boundary wiring not-yet (Tasks 12–13)** ... 'Sets terminal state to aborted' and 'preserves both digests as evidence' ... are controller responsibilities — see MAJOR-4."

**Confirmation disposition** — `task7/critic-c-confirmation.md` l.38: "Harness-only boundary teeth -> Tasks 12/13 (A-M4) | **Yes** | ... Correctly deferred — the plan itself sequences controller/settlement at Tasks 12/13." (Task 7 has no correction-round disposition ledger; critic-c l.40 records that gap.)

**Ledger** — CURRENT_STATUS.md l.632-636: "no production `verify_unchanged` caller exists; both digests live in the typed error but are not persisted as evidence. Trigger: **Tasks 12–13** (existing substrate-level ledger entries for AK-023/AK-024 carry this)."

---

## 6. Unavailable vs ReferenceMutated boundary behavior for broken artifacts undefined at the controller

**Original** — `task7/critic-a-result.md` l.132-138, **MINOR-3** "Deleted / non-UTF-8 answer key classified `Unavailable`, not `ReferenceMutated`" (verbatim core):

> "`provider.rs:279-298` deliberately reports a broken artifact as `Unavailable` ("a broken artifact is not proof of mutation"), pinned at `provider.rs:531-568`. ... a builder who cannot edit the ruler undetected could delete or corrupt it and receive a different error class whose boundary behavior (abort? retry?) is undefined until Task 12. Worth an explicit controller rule then."

**Ledger** — CURRENT_STATUS.md l.640-643: "the classification is deliberate and pinned; what the controller DOES with `Unavailable` is not. Trigger: **Task 12** controller error policy."

---

## 7. depends_on existence/acyclicity unchecked (B-F7)

**Original** — `task7/critic-b-result.md` l.117-124, **F7 — MINOR — dangling and cyclic `depends_on` accepted; comment overclaims** (verbatim core):

> "parse.rs:298-302's doc ("a `depends_on` target could never resolve to a block outside it") is about the AK prefix, but nothing checks EXISTENCE: `depends_on: [AK-999]` with no AK-999 block parses clean (close_block parse.rs:496-584 checks duplicates and required fields only), and cycles (A→B→A) are unchecked anywhere in the chain. Bounded claim: no existence or acyclicity check exists in parse.rs/provider.rs/manifest.rs."

**Adjudication** — no task-7 correction ledger exists (`task7/critic-c-confirmation.md` l.40: "No correction-round disposition ledger exists ... None of these minors should have blocked: all are leniency/diagnostics/doc nits ... but a correction-round ledger naming dispositions + upgrade triggers should be appended"). Disposition lives in the CURRENT_STATUS ledger.

**Ledger** — CURRENT_STATUS.md l.644-649: "Manifest dialect leniencies (B-F4/F5/F7/F8/F9/F19): ... `depends_on` existence/acyclicity unchecked (**F7 belongs to the Task 12 scheduler**); ... Trigger: **next manifest/parse touch** — batch-close." (The batch's generic trigger is "next manifest/parse touch", but F7 is explicitly assigned to the Task-12 scheduler.)

---

## 8. Provider-level empty-ruler legality (B-F18)

**Original** — `task7/critic-b-result.md` l.237-246, **F18 — MINOR — an all-empty answer key is a legal ruler at provider level** (verbatim core):

> "`parse_answer_key("")` → 0 units, 0 failures; `Reference::new` requires no units; freeze pins sha256(empty) happily. Compensating control exists and is real (doctor check 2's count + zero-blocks schema clause, checks.rs:261-279, 308-316), and the port-level leniency is the recorded contract-suite decision (provider.rs:23-48) — recorded, therefore not a silent weakening; listed for completeness with the note that `verify_unchanged` alone cannot distinguish "emptied ruler" from "legitimately blockless artifact" if a controller ever trusted it solo."

**Ledger** — CURRENT_STATUS.md l.654-657: "Trigger: **Task 12** if the controller ever trusts `verify_unchanged` solo."

---

## 9. Non-zero budget_spent cannot resume

**Original** — `task6/critic-a-result.md` l.70, **MAJOR-2** (verbatim):

> "resume cannot reconstruct non-zero `budget_spent`. The domain `Spent` has no public value-carrying constructor (gauntlet-domain/src/budget.rs:249-284 — private `fresh()`, getters only) but `Budget::charge` exists (budget.rs:381), so the first controller that charges a budget and then writes a checkpoint produces non-zero spend — and `resume` then errors `Corrupt`. §18.2 requires the checkpoint to carry "budget spent"; §18.3 requires "budget spent"/payment history to survive restart. The adapter durably stores and exports all six counters and refuses loudly rather than zeroing (the honest interim choice, self-documented at state_store.rs:17-23), but the §18.3 resume behavior is unimplementable until the domain/port gains a reconstruction path. AK-053's "sufficient to resume" holds for identity/units/in-flight and fails for spend."

(Cross-refs in the same file: l.95, l.131, l.145.)

**Adjudication** — `task6/correction-round1.md` l.270-273 (verbatim): "Non-zero budget_spend resume (critic A MAJOR-2). Not in the adjudicated corrections; untouched. It needs a domain/port change (a `Spent` reconstruction path), recorded as a port gap in the module docs." Also `task6/critic-c-result.md` l.320-323: "Carried forward, NOT on this checklist and unchanged: critic-A MAJOR-2 (non-zero budget_spend cannot resume; still refuses loudly per state_store.rs:1315-1339)". Summary l.365: "notably critic-A MAJOR-2, the domain-side Spent reconstruction gap."

**Ledger** — CURRENT_STATUS.md l.669-674: "Trigger: **Task 12** (first production `charge` caller) — needs a domain reconstruction slice (constructor + tests), not an adapter patch."

---

## 10. Port-shape gaps A-m10: command adapter kind is caller-bound

**Original** — `task9/critic-a-result.md` l.61-62, **m10** (verbatim):

> "The command adapter's facet kind is caller-bound with no type-level restraint. `CommandObserver` accepts any `FacetKind` (`gauntlet-adapter-observe-command/src/lib.rs:10-15, 84-131`), so a composition root could bind a build command to `FacetKind::Filesystem`. `facet_kind()` remains self-consistent, so the shared suite passes regardless; the misbinding risk sits entirely on the (future) production caller."

**Adjudication** — `task9/correction-adjudication.md` l.95-96, ledger deferral **D9** (verbatim):

> "**D9 ← A-m10:** command adapter's kind is caller-bound. Trigger: **production wiring (Task 12/15)** must bind kind↔command correctly."

**Ledger** — CURRENT_STATUS.md l.586-591 ("Port-shape gaps (A-m7/m9/m10)": "command adapter's kind is caller-bound with no type restraint. Trigger: **ports revision** (facet Optionality) and **production wiring** (kind↔command binding, Task 12/15).").

---

## 11. Additional Task-12 triggers found (beyond the ten above)

- **Doctor check 2's manifest clause stays shape-only** (task-7 deferral, CURRENT_STATUS.md l.625-631; original `task7/critic-a-result.md` l.66-78 MAJOR-2: doctor check 2 checks "only readability and `top_level_mapping_keys`" while "`parse_manifest`, in the same crate since this task, is never consulted by check 2"; "the fixtures only pass because the strict parser is not wired in"). Ledger trigger: "**next doctor touch or Task 12** (config/reference surface work) — deepen the clause and promote the fixture to a full manifest."
- **task11/final-confirmation-round2.md four NOTEs** (l.85-102): only NOTE-2 (workspace-root pairing, item 2 above) carries a Task-12 trigger. NOTE-1 (containment path-string not inode, ledger CURRENT_STATUS.md l.553-557, no trigger), NOTE-3 (absent-at-chain-read→written-before-instrument-read race, ledger l.527-531, trigger = race-freedom requirement / option-(b) ports revision), NOTE-4 (operator-file edits disclosure, process note, no trigger) are NOT Task-12 blockers.
- **Task-10 recorded MINOR residuals** (CURRENT_STATUS.md l.89-94): of the four (comparator_suite fixture capability; observation-owner write ungated; blindness field-name-only; assert_blind unfalsifiable), only the ungated observation-owner write names Task 12 — covered by item 1 above.
- CURRENT_STATUS.md l.1032-1033 reiterates: the observation-owner write gate is "one that hands work to the Task-12 controller when that task opens."

### Consolidated Task-12 blocking list (for the builder brief)

| # | Residual | Source finding | Adjudication/ledger | Task-12 obligation |
|---|---|---|---|---|
| 1 | Ungated `record_evidence_production` + unchecked declared iteration; ungated `bind_observation_bundle` | task11 critic-b MAJOR-3 (+MAJOR-2 2nd half); task10 round7 MINOR-2 / round10 residual-1 | C7 (task11 adjudication); ledger l.518-526 | Capability-gate BOTH writes (evidence-owner + observation-owner roles); validate declared iteration vs durable run iteration |
| 2 | Workspace-root pairing convention-bound | task11 round2 NOTE-2 | ledger l.532-537 | Keep chain root == adapters' root one fact in production wiring |
| 3 | Observer timeouts + output caps | task9 A-M2 + B-7a (MAJOR) | D1; ledger l.562-567 | Compose controller deadlines; §18.4 hang disposal; cap subprocess output |
| 4 | Runner deadlines modeled not enforced | task8 A-m2 | task8 NO-CHANGE; ledger l.617-620 | Enforce real §10.3 rule-5 deadline behavior when composing |
| 5 | verify_unchanged teeth harness-only | task7 A-M4 (MAJOR) | critic-c l.38; ledger l.632-636 | Production iteration-boundary + settlement callsites; terminal aborted{reference_mutated}; persist both digests as evidence |
| 6 | Unavailable vs ReferenceMutated controller behavior | task7 A-m3 | ledger l.640-643 | Explicit controller error policy for `Unavailable` (abort? retry?) |
| 7 | depends_on existence/acyclicity | task7 B-F7 | ledger l.644-649 | Scheduler checks existence + acyclicity |
| 8 | Empty-ruler legality | task7 B-F18 | ledger l.654-657 | Never trust `verify_unchanged` solo (pair with doctor count/zero-blocks clauses) |
| 9 | Non-zero budget_spent resume | task6 critic-A MAJOR-2 | task6 correction-round1 l.270-273; ledger l.669-674 | Domain `Spent` reconstruction slice (constructor + tests) before/with first production `charge` caller |
| 10 | Command adapter kind caller-bound | task9 A-m10 | D9; ledger l.586-591 | Bind kind↔command correctly in production wiring |
| 11 | Doctor check 2 manifest clause shallow | task7 A-M2 / B-F11 | ledger l.625-631 | Deepen clause + full-manifest scratch fixture if Task 12 touches config/reference surface |

---

## 12. Dispatch-time practices (from `.agents/lessons/session-interruption-recovery.md` and `.agents/lessons/subagent-message-loss.md`)

1. Before dispatching any slice, the orchestrator writes an IN-FLIGHT line into the task's report directory naming the slice, its result-contract path, and the loop steps still ahead — written before the agent runs.
2. Checkpoint-as-you-go is mandatory for every delegated slice: append to a durable progress file after each numbered item, before the next gate run.
3. The durable artifact is the report; the agent's final message is a convenience — critics write their report file first, then summarize.
4. After any interruption: run `just ruler` first; reconcile `git status` against CURRENT_STATUS.md before treating any task as unstarted (unexplained task-scoped files are the signature).
5. Reconstruct state from `git log`, `git status`, and report files — never from conversational memory or "what should have happened next".
6. Run the task's gates personally before dispatching anything new; never trust "N gates green" from a truncated message alone.
7. Classify the orphan and resume at the missing step: code+proofs landed/reports missing → add report slice, resume at critics; code landed/proofs missing → finisher brief that proves only, changes no behavior; code partial/uncompiling → read-only recon gap reports, then a finisher brief against the gap.
8. Never redo work the files already record; the tree is the contract, a checkpoint file is the agent's report.
9. A cancelled agent that also lost its final message is recoverable only through its checkpoint file; when a slice dies with nothing written, re-dispatch fresh with the same brief.
10. For read-only/recon/adversarial work prefer `explore`-type agents; `general` agents lost tail messages repeatedly.

