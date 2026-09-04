# Task 23 — blind design-as-built CRITIC report

**VERDICT: NOT_CONFIRM**

The slice is genuinely well-built in most layers — the codec validator, the
contract suite with green/broken/caps legs, the sqlite adapter's
transition-derived write-once state machine, the fail-closed delta
classification, the arch scan with negative controls, and the CLI wiring are
all real and load-bearing, and every test I ran is green. But the §19.4
verdict contract — the thing this slice exists to enforce — is broken at the
margin/payment conditions, and I confirmed the breakage empirically against
the real binary over the real composed loop: **a promotion was adjudicated on
an incomplete (never-settled) control arm with `control=18446744073709551615`**,
and the shipped p9 "honest road" legs are built on exactly that incompleteness.
A second empirical probe showed the mirror defect: two equally incomplete arms
are durably REJECTED on sentinel figures where the charter demands Provisional.

---

## 1. What I executed vs read

**Executed** (all under `flock …/compile.lock`, one compiler at a time):

| Run | Result |
|---|---|
| `cargo nextest run -p gauntlet-ports -E "test(revision_ledger)"` | 3/3 PASS (green clean; broken caught on `proposals_are_idempotent_on_revision`; caps-broken caught on `proposals_read_back_exactly_as_admitted`) |
| `cargo nextest run -p gauntlet-app -E "test(promote::)"` | 30/30 PASS |
| `cargo nextest run -p gauntlet-app --test promotion candidate_only` | PASS (AK-057 behavioral leg) |
| `cargo nextest run -p gauntlet-arch-tests -E "test(optimizer)"` | 5/5 PASS (scan + 3 negative controls + empty-input refusal) |
| `cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment` | 4/4 PASS |
| Manual replication of p9 leg 2 in /tmp over the real `target/debug/gauntlet` binary, real sqlite ledger, real content-addressed evidence store, real composed mock loop | **PROMOTED, printed `control=18446744073709551615 treatment=10000 margin=0bps`** |
| Same harness, starved treatment (`budget.max_rounds=1`) | REJECTED (4 reasons incl. sentinel-driven `margin_not_met`) |
| Same harness, no-op delta (`budget.max_rounds=2` = current config), both arms equally incomplete | **REJECTED with the single reason `margin not met: control 18446744073709551615 vs treatment 18446744073709551615`** |
| `git status --porcelain` on `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`, `.wayfinder/reference.yaml` | clean — ruler untouched |

**Read in full**: charter; Core-spec §19.3, §19.4, §5.3.8, Annex B.7, §23.1
P9; AK-056/AK-057; `gauntlet-ports/src/revision_ledger.rs`;
`contract/revision_ledger.rs`; suite_runs legs; `role_result.rs` (registry +
candidate payload + its tests); sqlite `revision_ledger.rs` + migration 0010;
`promote/{mod,proposal,delta,metrics,evaluate,gate,rollback,tests}.rs`;
config margin wiring (defaults/resolve/canonical/spec_vocab);
`improve_cmd.rs`; `report_manifest.rs`; `optimizer.rs` (arch);
`tests/promotion.rs`; `tests/p9_self_improvement_containment.rs`; fixture
GAUNTLET.md + e2e-happy scenario. **Skimmed**: help snapshots, from_path.

---

## 2. BLOCKING

### B1 — The `u64::MAX` sentinel for incomplete arms breaks the §19.4 verdict contract in BOTH directions; promotion on an undecidable baseline is reachable, durable, and demonstrated by the slice's own proof fixture

**Mechanism.** `TargetMetric::figure`
(`crates/gauntlet-app/src/promote/metrics.rs:63-81`) returns
`Some(u64::MAX)` whenever `settled_terminal` is false — "an incomplete arm
must never read as efficient." The gate (`promote/gate.rs:304-350`) then
feeds these sentinels into conditions 7 and 8 as if they were figures.
**`gate.rs` never reads `settled_terminal` at all** (verified by rg): an
incomplete arm is never classified as the evidence gap the charter and the
module's own docs demand ("evidence MISSING or undecidable (no evaluation
record, **failed/absent runs**, metrics underivable) → Provisional — NEVER
promoted, and never rejected", charter D4; gate.rs:39-46 makes the same
promise). The harness explicitly records executed-but-unsettled runs
(`evaluate.rs:21-24`: "a run that executed but did not SETTLE is recorded as
it happened"), so the path is production-reachable, not latent.

**Two wrong directions from one root cause:**

- **Incomplete CONTROL ⇒ promotion.** Control figure = u64::MAX, so
  `margin_holds(MAX, finite, m)` is trivially true and condition 8
  (`treatment ≤ control`) is vacuous. Any treatment that merely completes
  the task set promotes — with **zero** efficiency evidence and even at
  higher real payment per settlement.
- **Incomplete TREATMENT (or both arms) ⇒ durable Rejection.** Treatment
  figure = u64::MAX ⇒ `MarginNotMet` is a FAILED condition ⇒ Rejected —
  an evidence gap upgraded to a failure, and final (write-once past
  `rejected`, no re-adjudication). The charter says failed/absent runs
  adjudicate Provisional, "never rejected (insufficient evidence is not a
  negative result)."

**Empirical confirmation** (real binary, real composed loop, real sqlite
ledger; scratch worlds under /tmp, since removed):

1. Leg-2 replication — fixture `budget.max_rounds: 2` control (settles 2 of
   3 units, terminal) vs `budget.max_rounds: 4` treatment (completes):
   ```
   improve: revision rev-dda9dae48eee60b2 PROMOTED
   improve: metric=iterations_per_settlement control=18446744073709551615 treatment=10000 margin=0bps
   ```
   The durable promotion evidence records the sentinel as the control
   figure. Condition 8 passed vacuously against the same sentinel.
2. No-op delta (treatment configured identically to control; both arms
   equally incomplete; governance/regression/protected all genuinely
   satisfied):
   ```
   improve: revision rev-6a1177ad54b1f01b REJECTED
   improve: refused — margin not met: … control 18446744073709551615 vs treatment 18446744073709551615 …
   ```
   The only refusing "condition" is the sentinel comparison; per charter D4
   and §19.4 ("Insufficient evidence ⟹ provisional ⟹ not promoted") this
   revision must rest at Provisional. Instead it is durably Rejected with
   no recovery path.

**The shipped proof is built on the defect.** p9
`World::ensure_layout` (`crates/gauntlet-cli/tests/p9_self_improvement_containment.rs:113-135`)
deliberately configures the control arm so "the current configuration
cannot finish" ("units dispatched, two settled, the run terminal … exactly
the durable incompleteness the candidate's inert ceiling raise lets the
treatment arm overcome"). Legs 2 and 3's PROMOTED verdicts are therefore
produced by the incomplete-control path, not by a genuine margin
improvement between two comparable arms; the leg-2 header's claim
"genuinely improves the target metric" is not what the fixture
demonstrates. Remove the sentinel (return `None` → evidence gap →
Provisional, the charter-faithful behavior) and legs 2/3 go red — i.e. the
proof legs depend on the contract violation they exist to exclude.

**Also latent at the same site:** when `metric_figures` returns `None`
(arm settled nothing), gate.rs pushes the `EvaluationIncomplete` gap but
then still evaluates `margin_holds` on the placeholder `(0, 0)` figures
(gate.rs:313-331), producing `MarginNotMet` as a FAILED condition — a
figure that silently becomes 0 driving a rejection. Unreachable via the
harness today (a settled_terminal run always has ≥1 settlement), but it is
the same contract violation waiting one refactor away.

**Why blocking:** §19.4's floor ("insufficient evidence ⟹ provisional ⟹
not promoted") and the charter's verdict contract are the slice's
acceptance core; AK-056/AK-057's authority rests on the gate's honesty.
The defect also silently defeats condition 8 (payment) whenever control is
incomplete, and the proof that claims to prove the ruler is demonstrat­ing
the wrong thing.

**Fix direction (for the orchestrator):** treat `settled_terminal == false`
as `EvaluationIncomplete` (gap ⇒ Provisional) at the gate for **both**
arms — the figure function should return `None`/an error for incomplete
arms, not a sentinel — and rebuild the p9 leg-2/3 fixture so both arms
reach settled terminal with genuinely different per-settlement figures.

---

## 3. MAJOR

### M1 — Proposal-admission idempotency is broken on its production path by the moving clock
`RevisionProposal::facts_agree` compares `admitted_at`
(`gauntlet-ports/src/revision_ledger.rs:180`), and the admission service
stamps `admitted_at: self.clock.now()` (`promote/proposal.rs:316`). The
content-derived revision id hashes only the nine payload fields
(`proposal.rs:336-363`), so replaying the **same** document a second later
mints the same id with a different instant ⇒ `ProposalConflict` — not the
documented "idempotent no-op" (`proposal.rs:16-19`, port docs
revision_ledger.rs:13-16, charter D2). The failure message is false
("already admitted with **different declared facts**" — the facts are
identical; only the admission instant differs). The unit test
`admission_is_idempotent_on_the_content_derived_id` passes only because it
runs under a `FixedClock`. Fail-closed (no inflation), but the chartered
idempotency does not hold for `improve propose --from-result` retries or a
re-dispatched optimizer that re-emits the same proposal; `admitted_at` is
admission provenance and must not participate in replay identity.

### M2 — `parent_run` (the isolated-evaluation marker) is set by convention and never verified as evidence
The charter's mission says a revision cannot promote without isolated
evaluation "**with `parent_run` set**". As built: the harness constructs
treatment requests with `parent_run` (`evaluate.rs:338-362`) and trusts the
executor; `ExecutionOutcome` carries no parent fact;
`derive_run_metrics` never reads `RunDefinition.parent_run`; the durable
`EvaluationRecord` does not record it; the gate holds no `StateStore` and
cannot check it. A non-conforming (or substituted) executor that ignores
`parent_run` produces evaluations the gate cannot distinguish from isolated
ones. The p9 leg's assertion that stdout contains "(parent_run set)"
verifies the CLI **echoing its own intent**, not a durable fact. The
harness has `outcome.state` in hand and could verify
`load_run(treatment).parent_run == control_run_id` before appending; it
does not.

### M3 — Condition 4 fails open on an evidence-store read error
`improve promote` resolves the rollback ref and converts a store read
**error** into `None` via `.map_err(…).ok()`
(`crates/gauntlet-cli/src/improve_cmd.rs:747-754`); the gate then decides
condition 4 "on presence alone" (`gate.rs:261-268`) — i.e. **passes** it.
The evidence store opened successfully three lines earlier; only the `get`
failed. This converts uncertainty into a pass-deciding input, against
AGENTS.md's error-handling law and the fail-closed posture everywhere else
in the slice. (The `_ => None` arm on the proposal read is harmless — the
gate re-reads and would error — but the `.ok()` on the evidence error is a
real default-allow.)

### M4 — A capability-ledger infrastructure failure becomes a final Rejection
`PromotionGate::protected_verdict` maps a `CapabilityLedger` read failure
to `GateRefusal::ProtectedCapabilityInferior`
(`promote/gate.rs:447-453`) — a FAILED condition ⇒ **Rejected**, and
rejection is write-once (`apply_decision` past `rejected` is refused; the
gate refuses `AlreadyFinal`). A transient persistence error therefore
permanently rejects a revision with no typed retry path. An infrastructure
failure is not evidence of inferiority; it should surface as a typed
`GateError` (abort, retryable), not as a negative verdict.

---

## 4. MINOR

- **m1 — Dead branch in the decision state machine.**
  `RevisionDecisionRecord::new`'s `carried` computation is `true` for every
  match arm, so `DecisionRecordError::ReasonMismatch`
  (`gauntlet-ports/src/revision_ledger.rs:1305-1314`) is unreachable and
  its doc ("the verdict does not carry the claimed prior state") describes
  a check that does not exist. The real stale-claim enforcement lives in
  the ledger (`StaleDecision`), which is fine — but the dead variant
  misdocuments the design.
- **m2 — Condition 1 docs claim more than the code checks.** gate.rs:4-8
  and mod.rs say the treatment mix "equals control's"; the gate comment
  (`gate.rs:520-528`) says a treatment that "dispatched one control never
  saw" is altered — but the code checks only `control ⊆ treatment`
  (superset passes). The implemented direction is the conservative one
  (cherry-picking caught; extra dispatch is not an improvement vector), but
  the documentation overstates the mechanism.
- **m3 — The one legal verification-bearing delta cannot be executed.**
  `verification.prefer_deterministic` has no Annex E variable
  (`config/resolve.rs:206`, `env() ⇒ None`), so the CLI executor refuses it
  ("delta key … names no Annex E override variable",
  improve_cmd.rs:560-564). Fail-closed, but the single value
  `delta.rs:250-252` declares preserving cannot be evaluated end-to-end.
- **m4 — `dispatched_units` is derived from verification records, not
  dispatch records** (`promote/metrics.rs:200-207`): a unit attempted but
  never verified never appears, so the "every unit the run dispatched"
  doc (metrics.rs:201-202) overstates the fact. Both arms share the
  projection, so the governance comparison stays symmetric; the figure is
  narrower than named.
- **m5 — Unrecorded charter deviation: the domain `CandidateRevision`
  record is unused.** Charter discovered-facts say "reuse it, do not
  duplicate it" and D4 says "write-once decision via the domain record";
  as built, the floors are re-derived in the port/gate (transition-derived
  state, `GateDecision::rejected` requiring the rollback ref, the
  gate's written:false zero-evidence shape). The re-derivation is
  behaviorally pinned by tests and D2's append-only design genuinely
  conflicts with the domain record's mutable `decision` field — but per
  the charter's own rule ("deviations get recorded, not smuggled") this
  belongs in the builder report, which does not mention it.
- **m6 — `delta.rs:216-226` justifies the `Inert` class of
  `max_attempts_per_unit` with an argument that only holds on the
  regression set** ("produces FEWER settlements, which the margin …
  refuse[s]"). On the held-out set, dropping hard units would *reward*
  per-settlement metrics — the reason this does not break the gate is
  solely `settled_terminal` ⇒ u64::MAX (see B1: the wrong mechanism
  rescuing a wrong argument). Once B1 is fixed to Provisional, re-derive
  this classification honestly.

---

## 5. INFORMATIONAL

- **i1 — Re-evaluation cherry-picking:** the gate adjudicates
  `evaluations.last()` (gate.rs:196); an operator may re-run
  `improve evaluate` until a favorable record lands, then promote. The
  charter does not forbid re-evaluation; note for the anti-gaming task 24.
- **i2 — `target_is_governance_bearing` is a prefix heuristic over an open
  vocabulary** (gate.rs:547-558); adversarial target naming could evade it.
  Real containment lives in the delta classification and the ledger; the
  target check is advisory.
- **i3 — The optimizer dispatch's `budget_slice` is the whole configured
  run budget** including the recovery-reserve percentage
  (improve_cmd.rs:288-297), not a bounded slice; bounded in practice by
  the invocation deadline and probe cap.
- **i4 — `MemoryRevisions` in `tests/promotion.rs` returns empty histories
  from `evaluations()`/`decisions()`** (promotion.rs:180-186, 215-221) and
  the AK-057 behavioral leg runs over the in-memory double, not the sqlite
  adapter. Acceptable (the app crate cannot depend on an adapter; the real
  adapter is pinned by the suite legs and p9), but the locator evidence
  for AK-057's "requiring isolated evaluation" clause leans on the p9
  proof for realism.
- **i5 — Leg-1 header vs mechanism:** "parallel dispatch settles the
  fixture in fewer iterations" (p9 file header) does not match the
  fixture (max_parallel: 3 is in BOTH arms' baseline; the actual delta is
  the round ceiling completing an incomplete control — see B1).
- **i6 — `rollback_resolvable`'s `_ => None` arm** also swallows
  `Ok(None)`; harmless today (the gate re-reads the proposal and errors
  `UnknownRevision` first), noted with M3.

---

## 6. Contract decisions D1–D11 — as-built status

| Decision | Status |
|---|---|
| D1 payload validator | **MET.** Nine fields exact, non-empty; unknown fields (incl. `decision`, `evaluation_run_ids`, results) mechanically refused; registry learns exactly one token; codec tests cover every refusal shape. |
| D2 port + suite | **MET structurally; idempotency broken in production** (M1). Green/broken/caps legs present and green; no status column; typed conflicts. |
| D3 sqlite adapter | **MET.** INSERT-only, UNIQUE-key idempotency, transactional claim validation, `Corrupt` on undecodable payload, migration 0010 registered. |
| D4 app module | **PARTIAL.** All eight conditions implemented and typed; the verdict contract is violated on incomplete arms (B1); condition-4 fail-open at the CLI (M3); condition-6 infra-error-as-rejection (M4); parent_run unverified (M2). |
| D5 margin config | **MET.** Full wiring (key arm, env parse, GAUNTLET.md walk, effective+resolved fields, canonical serialize/parse, overrides struct, fingerprint test), honest 0-default doc. |
| D6 CLI wiring | **MET** (subject to M3). Manifest/usage/snapshots/surface pins updated deliberately; fail-closed exit codes; no `baseline`. |
| D7 p9 proof | **4 legs green over real adapters — but legs 2/3 rest on the B1 defect**; leg-1's teeth declaration is real (delta classification). |
| D8 AK-057 | **MET** (behavioral leg + arch scan with negative controls; locator divergence recorded in reference-candidates.md:1492). |
| D9 module tests | **MET** (30 tests, each condition refusing in isolation; idempotency test only valid under a frozen clock — see M1). |
| D10 boundaries/scans | **MET.** Layering correct; no new crate; sha2 already on the app allowlist; settlement-path untouched (no promote-module writer); scan bites (negative controls) and refuses an empty input. |
| D11 typed delta surface | **MET.** Parse-time schema validation + fail-closed classification; content-addressed diff pinning makes evaluate/promote see identical bytes (re-pinning attacks fail because a different digest is a different ref). |

## 7. What I could NOT verify

- Global gates (`just check && just test && just lint && just deny && just
  boundaries && just ruler`) — out of my time/memory budget; I ran the
  slice-scoped suites (all green) and confirmed the ruler files unmodified
  in git, but the full workspace battery is the orchestrator's to run.
- The interior of `compose_run`/`start_run_with_parent` — I took the
  executor seam's composition and `parent_run` plumbing at the CLI on
  trust for anything beyond what the p9 legs exercise; per M2, nothing
  mechanically proves the treatment runs' `parent_run` fact.
- Whether the developmental import's 28-row/16-row/10-row appends in my
  replications were exactly the §19.1-expected sets (I confirmed they ran
  post-terminal and non-inferred, not their row-level correctness — that
  is task 22's settled surface).
- Long-horizon concurrency of the ledger (two concurrent `improve promote`
  processes): the adapter serializes via `BEGIN IMMEDIATE` + Mutex, but I
  did not exercise cross-process races.
