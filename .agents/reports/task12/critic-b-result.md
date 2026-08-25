# Task 12 — Blind Adversarial Critic (Critic B)

Reviewer: blind critic; no builder reports, no attempt history, no CURRENT_STATUS read.
Method: static analysis only (no cargo runs). Effort budget declared upfront: ~35 tool calls / one pass over the control layer, domain budget/unit/correction/iteration, the five named ports, the meter-memory adapter, observe-command run_bounded, p4/p6. Every verdict below names its file:line evidence and its limits. No universal-security claims are made; several findings are partially disclosed by the builders' own doc comments, and I mark those.

Verdict counts: **9 ESCAPED, 4 BIT, 2 INCONCLUSIVE, plus minor notes (A12).**

---

## A1. Owner-write gating — **BIT** (bounded)

Attempt: find any call path to `StateStore::record_evidence_production` / `bind_observation_bundle` that bypasses the authorities; construct/clone/reuse/transmute the minted types; replay to rebind different facts.

- Workspace-wide call-site search: the ONLY non-test, non-adapter-impl callers are the authorities themselves — `crates/gauntlet-ports/src/producer_authority.rs:275` (`EvidenceProductionAuthority::record`) and `:332` (`ObservationBindingAuthority::bind`). Every other hit is a trait implementation (sqlite adapter `state_store.rs:1781,1894`; test fakes in p2/p4/p6/controller tests; contract stubs).
- Minted types `AdmittedEvidenceProduction` / `AdmittedObservationBinding` have private fields and no public constructor (`producer_authority.rs:139-166`); `Clone` exists but clones fixed content.
- Replay: the sqlite adapter is write-once per (run, iteration) for bindings — identical ref+digest returns Ok (idempotent), a different ref/digest for the same slot is REFUSED (`gauntlet-adapter-state-sqlite/src/state_store.rs:1812-1826`); evidence productions refuse differing facts for the same ref (`state_store.rs:1898-1922`). Rebinding different facts requires a fresh mint through `governed_run` (run durable; declared iteration == latest checkpoint's iteration, `producer_authority.rs:194-215`), and the digest is derived from the store's own bytes (`:263`, `:327`) — a caller-supplied digest has no parameter to travel through.

Refusing lines for an attacker: `producer_authority.rs:208-213` (iteration anchor) and `state_store.rs:1818-1826` (substitution refusal).

Limits: (a) any in-runtime code holding the authorities can mint records for arbitrary store-resolvable refs — the gate is type-level, not caller-identity-level; (b) the ports crate itself can construct the values (trusted layer); (c) I did not audit the in-memory contract stubs' write-once behavior (test-only surface). Verdict stands for the shipped sqlite composition.

## A2. Reserve bypass — **ESCAPED** (two paths; one disclosed by the builders)

Attempt 1 — rounds dimension, follow the dispatch path line by line:

- `MeteredBudget::charge` books rounds as ZERO by design ("payments carry no rounds, port-pinned"; `crates/adapters/gauntlet-adapter-meter-memory/src/lib.rs`, charge impl: `inner.budget.charge(0, …)`), and `reserve` books nothing on the domain budget.
- The controller's rounds gates read the METER: `controller.rs:929` (`state.meter.remaining().rounds == 0` → BudgetExhausted) and `controller.rs:1846`. Under the real adapter those never decrement → **the rounds gate is structurally dead**; the summed pre-dispatch consult (`controller.rs:946-956`) also consults the meter, so its rounds component never refuses either.
- The only live rounds enforcement in the controller is the SHADOW budget charge at completion (`controller.rs:1423-1431`, rounds=1 per completion). At the forward ceiling that charge fails with `RecoveryReserveBreached` → `ControlError::Budget` → **`start_run` returns Err**: no terminal state, no close-out checkpoint (the checkpoint at `:1434` is after the charge and is skipped), no report. §11.6's `budget_exhausted` terminal is unreachable via rounds under the bare adapter.
- The proof wires around it: p6's `RoundsSupplementedMeter` (`p6_budget_control.rs:474-581`) books rounds at RESERVE time on a second shadow. p4's factory — also commented "composition-root wiring" — wires the BARE adapter (`p4_corrective_loop.rs:910-912`). So the same controller has three incompatible canonical meters with three different rounds semantics: in-crate `FakeMeter` books reservation rounds at charge (`controller.rs:2854-2879`), p6's supplement books at reserve (non-refundable), the real adapter never. Under p4's wiring the p6 scenario crashes instead of terminating.
- Disclosure status: the p6 module header (lines ~63-77) and the meter adapter's module doc BOTH record this as a known residual ("no shipped composition books rounds, so the gate cannot fire"). Honest — but the gate is still dead in the only shipped adapter, and the p4 factory (bare adapter) is the wiring a future composition root would most plausibly copy.

Attempt 2 — double-book / diverge between meter and shadow:

- The loop owns TWO `Budget` objects accruing the same payments: the meter's inner budget (tokens/wall/cost via charge) and `state.budget` (same payment PLUS one round, `controller.rs:1411-1431`). If the meter books and the shadow refuses (possible on rounds exactly as above), the meter has booked spend the durable checkpoint will never carry — the books diverge and the run dies mid-completion.
- In the p6 wiring, a cancelled or transiently-failed dispatch releases its reservation via `Payment::zero()` (`controller.rs:2017-2019`), but the supplement's shadow keeps the round booked at reserve — retries burn rounds the durable `budget_spent` never shows. Divergence between the live gate and the durable record, statically demonstrated path.

Attempt 3 — forged reservation: `Reservation::new` is PUBLIC (`budget_meter.rs:82-90`). `MeteredBudget::charge` cannot detect a foreign reservation (acknowledged in its module doc) but a successful charge — including `Payment::zero()` — RELEASES A PARALLEL SLOT (`outstanding.saturating_sub(1)`). A buggy caller that mints `Reservation::new(…)` and zero-charges it frees a slot with no dispatch completing → `max_parallel` over-admission through the public API. The recovery reserve itself stays intact (the domain's cumulative charge is the floor), so this is a concurrency-accounting bypass, not a reserve bypass.

What held (BIT): per-dispatch TOCTOU for tokens/wall/cost — the summed-pending consult plus `meter.reserve`'s own `would_breach_reserve` re-check over the same domain arithmetic leave no gap I could find; the domain `charge` is the unconditional floor (`budget.rs:514-572`); `remaining()` DOES report the reserve as headroom (`budget_meter.rs:127-145` subtracts from the raw limit), so the selector's recorded `BudgetFacts` overstate affordable headroom (dishonest record, honest gate — see A7/A10 notes).

## A3. Scheduler smuggling — **split verdict**

- Irreversible Unit dispatched: **BIT**. Domain constructor refuses `irreversible ⟹ ≠ human` (`unit.rs:293-305`); scheduler eligibility refuses irreversible unconditionally (`scheduler.rs:443-447`); slice construction re-guards (`scheduler.rs:575-579`); the controller always dispatches `RoleId::Builder` only (`controller.rs:1077`). No status/depends_on/slice path around it that I could construct.
- Tier/authority LOWERED anywhere: **BIT** in the control layer — no code path lowers a tier; slice inheritance takes the MAX blast radius and MOST restrictive authority (`scheduler.rs:580-585`); the controller's tier capacity list omits `irreversible` entirely (`controller.rs:1953-1960`).
- Concurrency accounting: **ESCAPED**. The controller's dispatch loop (`controller.rs:946-956`) carries NO slot accounting; the scheduler's plan output is not capacity-consumed (its own doc says the controller's loop is the bound). When open eligible units exceed `max_parallel`, the (N+1)-th `meter.reserve` fails the scope check → `ControlError::Meter` via `?` at `controller.rs:1032` → `:954` → **the entire run returns Err** instead of dispatching up to capacity and waiting. Concrete attacker: an answer key with 3 independent contained units and `max_parallel=2`. Neither p4, p6, nor the in-crate controller tests (single-unit fixtures per the comment at `controller.rs:2169-2171`) exercise it.
- Dependency wiring: **ESCAPED**. `plan_and_dispatch` filters `PlanInput.units` to in-memory-open, non-in-flight units (`controller.rs:880-891`), but the scheduler's `validate_graph` requires EVERY `depends_on` target to be present in the input (`scheduler.rs:632-641`). The first plan AFTER a dependency settles (its in-memory status becomes `Settled` at `controller.rs:1617-1620`, excluding it) errors the whole run with `UnknownDependency` → `ControlError::Scheduler`. Multi-hop dependency graphs cannot complete; single-unit proofs never see it.
- Slice dissolution: **ESCAPED (latent)**. `dispatch()` keeps only `planned.units.first()` (`controller.rs:1024-1028`); a multi-unit cohort slice is reserved and estimated for ALL members (`scheduler.rs:593-598`) but only the first member is prompted or attempted. Latent because the controller currently always passes empty cohorts (`controller.rs:922`).

## A4. Snapshot trust — **ESCAPED** (moderate)

- In-flight staleness: **BIT** — the answer key is re-read at every plan iteration (`controller.rs:879`) and every drain tick (`:1196`); the projection diff over (tier, authority, claim, status) cancels hand-edited in-flight attempts (`:974-1009`); resume re-reads (`:751-763`).
- Non-in-flight staleness: **ESCAPED**. Eligibility is decided on `state.unit_states` (in-memory), never on the re-read file status (`controller.rs:880-891`). Since this controller never writes Unit status back to the file, the file's status field is frozen at parse-time forever, and the in-memory map is authoritative for every non-in-flight unit — the exact inversion of §11.4 step 3's "The in-memory snapshot is never authoritative over the file." An operator hand-edit marking a queued unit `settled`/`out_of_scope` is silently ignored. (Bounded: the file-status leg of the in-flight projection check is likewise near-vacuous, since the file status never changes.)
- Adjacent: an answer-key READ failure inside the loop (`plan :879`, `drain :1196` via `?`) is a hard `ControlError::Reference` — inconsistent with `boundary_verify`'s tolerant Unavailable policy (`:813-835`). A transient read hiccup kills the run with an Err rather than a recorded gap.

## A5. Non-convergence identity — **ESCAPED** (disclosed) plus key-coarseness

- Production unreachability: the real diff comparator's residual docs carry EMPTY unit attribution → `residuals_of` attributes `None` (`controller.rs:2111-2127`, `doc.unit_refs().first()`) → `detector.observe` is never called for them (`:1626-1641`) → §11.5 row 9 never routes in the composed path. p6 itself PROVES this: the same residual recurs 5 iterations with `nonconvergence_n = 3`, and the terminus is `BudgetExhausted`, not diagnosis/`Inconclusive` (`p6_budget_control.rs:988-1006`, constants at `:186-190`). The p6 header openly records this as the "diff-comparator attribution-gap residual." Meanwhile the in-crate `control::diagnosis` tests stay green because they feed attributed keys directly. Exactly the battery's "diagnosis unreachable in production while its in-crate tests stay green."
- Key coarseness: `ResidualKey = (unit, dimension, severity)` (`diagnosis.rs:146-150`) omits §13.5's residual identity `(dimension, expected, observed_locator)`. Two DIFFERENT residuals in the same dimension+severity+unit accumulate one count — non-convergence can fire falsely (documented tradeoff: "free-text expected/observed wording … drifts", but the false-positive direction has no guard); conversely `residual.dimension().unwrap_or("functional")` (`controller.rs:1632`) means dimension-string drift (or absent dimensions defaulting) splits counts and can hide a true recurrence.

## A6. Hangs / deadlines — **BIT** for adapters, **INCONCLUSIVE** (defense gap) for the controller loop

- `run_bounded` (observe-command `lib.rs:71-135`, same shape in observe-test/observe-build): **BIT** — a deadline with no time left refuses WITHOUT spawning (`:73-84`); a child still running at the kill instant is killed, reaped, and reported `Unavailable`; output is ceiling-capped per stream with drainers joined. Grandchild processes are documented as out of scope.
- Controller drain (`controller.rs:1117-1199`): **INCONCLUSIVE**. The loop has NO independent bound — no tick cap, no wall-left check (wall seconds book only on completion payments, so budget cannot stop a drain either). Liveness is wholly delegated to the runner's poll contract returning `TimedOut`; a non-compliant runner reporting `Running` with fresh `last_event_at` evades stall detection (`:1167-1179`) and spins the loop forever (virtual clock: infinite proof; real clock: unbounded real time). The shipped mock/prime runners honor deadlines per their contracts, so this is a missing defense-in-depth, not a demonstrated production hang — hence INCONCLUSIVE rather than ESCAPED.
- Already-expired deadlines: dispatch derives `wall_left` by saturating subtraction (`controller.rs:1035-1040`) → deadline = now → observers refuse per above; runners see a now-deadline. No path found that turns an expired deadline into an unbounded wait. Poll/backoff waits are virtual-time `clock.sleep_until` only (module doc's no-thread-sleep claim verified by grep).

## A7. Selection-record vacuity — **ESCAPED**

- `CorrectionSelection::new` accepts an EMPTY `rejected` vec — the only validations are trace-completeness and class/reason pairing (`correction.rs:434-464`); the domain's own test constructs `Vec::new()` (`:554`). Production `record_selection` builds exactly ONE candidate (`controller.rs:1803-1817`), so on the no-residual path every durable selection record carries ZERO rejected alternatives — AK-037's "records the selected correction together with rejected alternatives" is satisfied vacuously. Not constructible with a degenerate CHOSEN descriptor (summary/unit_refs validated, `correction.rs:158-182`), but the alternatives leg is vacuous.
- Governance facts compare a value to itself: `exercisable_authority` is set to the unit's OWN authority (`controller.rs:1798` with `required_authority` the same at `:1805`) — the selector's authority gate can never fire in production wiring (the selector's own logic at `select.rs:344-353` is sound; the wiring defangs it).
- Hardcoded tier list `[Contained, Local]` (`controller.rs:1799-1802`): a completed CROSS_CUTTING unit with no residuals makes `select` return `NoAdmissibleCandidate` → `ControlError::Selection` via `?` at `:1820` → the run ERRORS right AFTER the unit settled (`:1674`). A cross_cutting success story is unrepresentable.
- Round-trip: **BIT** — sqlite decodes selections through the domain constructors or names the row Corrupt (`gauntlet-adapter-state-sqlite/src/state_store.rs`, decode path around `:1770-1780`); lossless or refused, never degraded.
- Reasons load-bearing vs informational: the trace's ten-dimension completeness is constructor-enforced (real); but `remaining_*` facts in `BudgetFacts` come from the reserve-inclusive `remaining()` projection (A2), overstating headroom in the durable record.

## A8. Diagnosis theater — **ESCAPED**

- `enter_diagnosis` hardcodes the evidence: `implementation_shaped = true` ALWAYS (`controller.rs:1702`), with `observation_unavailable` only when the reference was unavailable (`:1699-1701`); every other locus fact has no production source. `DisconfirmingObservation { disconfirmed: true }` is hardcoded (`:1711`). The §15.2 taxonomy (sound in isolation: `diagnosis.rs:340-431`, implementation LAST) degenerates in production to ImplementationDefect-or-ObservationFailure with a predetermined "disconfirmed" observation — precisely the "loop that only ever diagnoses implementation defect" the spec forbids.
- Diagnostic spend wiring: `DiagnosticSpend` is built from the FULL allowance (`controller.rs:1691-1694`: `tokens: self.budget_shape.diagnostic.tokens()`, whole cost) and the engine charges that PER PHASE (`diagnosis.rs:614-624`). Phase 1 books the entire allowance; phase 2's charge is ALWAYS refused (`:681-688`) → every production cycle ends `Inconclusive` at phase 2; phases 3-5 and `CorrectionIndicated` are unreachable. In-crate tests use small per-phase spends, so they stay green. (Whether this counts as "budget actually charged": YES — the allowance is genuinely exhausted on the shadow budget and durably recorded via checkpoint `Spent`; the defect is that the cycle can never run past phase 2.)
- Quarantine: NOT a no-op — persisted as pinned evidence + `TerminalState::Inconclusive` + escalation ticket (`controller.rs:1728-1757`); `INCONCLUSIVE` is a recorded third state, never an error/panic. **BIT** on those two sub-items.

## A9. Checkpoint fidelity — **ESCAPED** (three defects)

1. `event_log_offset` is a constant 0 for fresh runs: `state.last_offset` starts at 0 (`controller.rs:646`), is updated only on the resume path (`:725`), the checkpoint writes the stale value (`:1881`), and the freshly appended `CheckpointWritten` event's offset is explicitly discarded (`let _ = offset;` `:1897`). §18.2's "event-log offset" is present but dishonest.
2. Wrong boundary: the checkpoint is written mid-pipeline — after charge, BEFORE observe/compare/verify/settle/dispose/select (`:1432-1434`) — contrary to §11.1's iterate order (`… dispose residuals → checkpoint`). A crash after settle (`:1607-1620`) but before the next completion's checkpoint loses the settlement; resume re-dispatches settled work (§18.3: "Re-running settled work is a defect"). The checkpoint's `unit_states` are stale by construction at every iteration boundary.
3. Fields not carried at all: attempts, timeout-retry/reminder ledgers, pending attachments (all absent from the `Checkpoint` struct, `state_store.rs:100-108`) — see A11.

The §18.2 field LIST is complete (run id, digest, iteration, unit states, budget spent, in-flight ids+paths, offset) — nothing silently defaulted at the type level; the defects are the stale offset, the mid-iteration write point, and the unchecklisted loop state.

## A10. Proof honesty — **MIXED**: real arithmetic, theatrical wiring

Real (verified): p6's ceilings are independently computed from the limits (`p6_budget_control.rs:160-166`); the reserve-intactness assertions read the DURABLE checkpoint's `budget_spent` against those ceilings (`:1067-1096`); invocation rows are counted at the store wrapper and compared against reservations counted at the meter's admission point (`:1015-1025`); the mock scenario is genuinely on the path (a REQUIRED fixture replayed by the real mock runner whose effects land in the real observers' tree, `:819-856`); p4's blindness check inspects the ACTUAL recorded second prompt for residual text and absence of critic vocabulary (`p4_corrective_loop.rs:1113-1120`).

Theater: p6's rounds-dimension reserve protection lives entirely in a fixture-local supplement, while p4's equally-"composition-root" factory wires the bare real adapter. p6 passing says nothing about p4's wiring, under which the same scenario returns Err with no terminal and no final checkpoint (A2). The two proofs canonize two different controllers. Also: the AK-052 rows-vs-reservations count reads a fixture-maintained `AtomicU32` the supplement increments — no port exposes it, so the "meter's own truth" is the fixture's truth. Can p6 pass while the reserve is consumable in production? On tokens/wall/cost: no (domain charge is unconditional — BIT). On rounds: the gate is dead and the failure is a post-hoc crash (A2) — the reserve water-mark never crosses, but the run breaks instead of terminating.

## A11. Retry-table integrity — **BIT** on leakage, **ESCAPED** on counters

- Residual text only: **BIT**. `RetryWithResidual` carries only the residual's expected/observed text (`disposition.rs:402-423`; `ResidualAttachment` has no other field, `:234-247`); the dispatch prompt appends text only and the context envelope is the allowlisted unit/claim body (`controller.rs:2038-2056`); the schema-reminder and timeout notes are controller-authored strings (`:1238-1243`, `:1318-1325`). No critic-trajectory path found. Minor fidelity gap: only the FIRST residual's text rides a re-dispatch (`:1653-1656`).
- Attempts counters reset: **ESCAPED**. `attempts`, `timeout_retries`, `reminders`, `pending_attachments` are loop-memory only and reset to empty on resume (`controller.rs:700-712`); none is checkpointed (A9.3). A crash-retry loop re-arms the retry-once, one-reminder, and max-attempts ceilings every crash, bounded only by dimensions that actually book — tokens/cost/wall, and NOT rounds under the bare adapter (compounding A2).

## A12. Anything else (bounded sweep)

- `escalate()` interpolates the summary into a JSON evidence record without escaping (`controller.rs:1353-1357`) — a summary containing `"` produces a malformed forensic record (the diagnosis path at `:1730-1745` does escape; the escalation path does not).
- A meter-charge refusal at completion silently drops the result: terminal BudgetExhausted, no settle, no residual processing, no record of the completed result's contents (`controller.rs:1411-1419`).
- The verification loop writes locator bytes with `std::fs::create_dir_all` + `std::fs::write` directly (`controller.rs:1520-1545`); runtime-side single-writer consistent, but I did not verify the `EvidenceLocator` grammar excludes absolute/`..` paths — INCONCLUSIVE, outside my remaining budget.
- Scheduler core in isolation is clean: deterministic ordering pinned by permutation tests (`scheduler.rs:1104-1166`), graph validation typed and named (`:1247-1311`), tier-unconfigured ≠ unlimited (`:1029-1038`). Every A3 escape above is in the CONTROLLER'S WIRING of the scheduler, not the scheduler.

---

## Summary table

| Area | Verdict | One-line evidence |
|---|---|---|
| A1 owner-write gating | BIT | sole path producer_authority.rs:275/:332; write-once state_store.rs:1812-1826 |
| A2 reserve bypass | **ESCAPED** (disclosed) | rounds gate dead: controller.rs:929/:1846 vs meter charge rounds=0; shadow refusal errors run at :1423-1431; p6 supplement fixture-local :474-581; forged `Reservation::new` frees slots (budget_meter.rs:82) |
| A3 scheduler smuggling | BIT (irreversible/lowering) / **ESCAPED** (wiring) | slot crash controller.rs:946-956+:1032; settled-dep drops from PlanInput → UnknownDependency (controller.rs:880-891 + scheduler.rs:632-641); slice first-member-only controller.rs:1024-1028 |
| A4 snapshot trust | **ESCAPED** (moderate) | eligibility on in-memory states controller.rs:880-891; file status never consulted/written |
| A5 non-convergence | **ESCAPED** (disclosed) | empty unit_refs → detector never fed (controller.rs:1626-1641, 2111-2127); proven by p6's own terminus; key omits §13.5 identity (diagnosis.rs:146-150) |
| A6 hangs | BIT (adapters) / INCONCLUSIVE (controller drain) | run_bounded refuses/kills (observe-command lib.rs:71-135); drain has no independent bound controller.rs:1117-1199 |
| A7 selection vacuity | **ESCAPED** | empty-rejected accepted (correction.rs:434-464); self-compared authority controller.rs:1798; cross_cutting completion errors run (:1799-1802, :1820) |
| A8 diagnosis theater | **ESCAPED** | implementation_shaped hardcoded controller.rs:1702; full allowance per phase :1691-1694 → cycle always ends phase-2 Inconclusive (diagnosis.rs:681-688); quarantine itself real |
| A9 checkpoint fidelity | **ESCAPED** | offset constant 0 (controller.rs:646/:1881/:1897); mid-pipeline write :1434 vs settle :1607; ledgers unchecklisted |
| A10 proof honesty | MIXED | arithmetic real (p6:160-166, 1067-1096); wiring theatrical (two "composition-root" factories disagree; counter is fixture-side) |
| A11 retry integrity | BIT (leakage) / **ESCAPED** (counters) | text-only BIT (disposition.rs:402-423, controller.rs:2038-2056); resume resets attempts/retries/reminders controller.rs:700-712 |
| A12 others | minor | unescaped JSON evidence controller.rs:1353-1357; silent drop on charge refusal :1411-1419 |

Escaped count by my taxonomy: A2, A3(wiring), A4, A5, A7, A8, A9, A11(counters) = 8 areas with at least one concrete path through; A10 is "mixed" rather than escaped. Two of the escapes (A2 rounds, A5 attribution) are explicitly disclosed as recorded residuals by the builders' own doc comments; the others I found no disclosure for (but as a blind critic I did not read the Task-12 report, by instruction).
