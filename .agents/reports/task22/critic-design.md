# Task 22 blind design-as-built critique — developmental memory and the capability ledger

Critic round, 2026-09-03. Static reading only (rg + Read + reasoning); no builds, no
mutations, no ruler contact, no builder history. Judged against: charter D1–D11
(`.agents/reports/task22/charter.md`), spec §19.1–19.2 + I6 (`.agents/specs/Core-spec.md`
:1129–1144, :128), §5.3.8 (:381–400), AK-055 (`.wayfinder/answer-key.md` :383–388).

---

## Verified claims (verified-absence is a result)

- **No set-status escape hatch exists.** `rg "set_status|force_status|elevate"` over
  `crates/` hits doc comments only. The register's only status writer is
  `CapabilityLedger::apply_transition`
  (`crates/gauntlet-ports/src/capability_ledger.rs:1639`); the SQLite register has no
  status column at all — status is derived from the latest transition payload
  (`crates/adapters/gauntlet-adapter-state-sqlite/src/capability_ledger.rs:85-112`,
  migration 0009 has no status column). `StatusTransition::new`
  (capability_ledger.rs:1349-1376) enforces reason↔target (`ReasonMismatch`) and state
  machine legality (`IllegalMove`); `apply_transition` additionally refuses an
  unregistered capability and a stale `from` claim (adapter :284-295, inside
  `BEGIN IMMEDIATE`). The re-decode of a stored transition re-validates its own shape
  (capability_ledger.rs:1477-1488), so a corrupted stored record cannot smuggle a move.
- **The settlement path has zero diff.** `git diff HEAD --stat` over
  `crates/gauntlet-app/src/settle/`, `producer_authority.rs`, and the state-sqlite
  settlement/event-log/store code is empty. Modified production files are exactly the
  commissioned surfaces (config trio, ports lib/contract, adapter lib/migrations/
  contract-suite, one docs table line). `crates/gauntlet-app/src/memory/` only reads the
  StateStore (`load_run`, `resume`, `verification_records`, `correction_selections`,
  `settlements` — developmental.rs:122-213); the only write-shaped names in memory/ are
  the test double's always-error stubs under `#[cfg(test)]`. `record_recovery` appends to
  the ledger only (:260-286); p10 drives it strictly AFTER
  `complete_pending_settlements` returns 1 (p10_no_false_learning.rs:486-505).
- **The contract suite can fail, per defect, on distinct cases.** Broken stub
  (duplicate reads + false `idempotent` receipts, stubs.rs:3786-3853) is caught by
  `observation_appends_are_idempotent_on_identity`; caps stub (instrument/detail flipped
  on read, stubs.rs:3860-3915) is caught by `observations_read_back_exactly_as_appended`.
  Green passes all seven cases; the third factory leg runs the same unmodified suite over
  the real adapter (`crates/adapters/gauntlet-adapter-state-sqlite/tests/contract_suite.rs:135`).
- **Idempotency identity excludes the recording instant; no double-count path.**
  Identity = (capability, class, run, fact); `facts_agree` compares all facts but not
  `at` (capability_ledger.rs:480-492). A row is one INSERT under
  `UNIQUE(capability, class, run_id, fact)` (0009:33) — a partial import leaves durable
  rows that a retry re-appends as idempotent no-ops; a concurrent duplicate insert hits
  the UNIQUE key and is a typed `AppendFailed`, never a second row. The §18.3
  re-completed settlement re-imports under the same identity; p10 leg 3 asserts
  `settlements_observed == 3` after the replay (not 4).
- **Configured 0 refuses at every layer.** Resolution: `ValueOutOfRange` at
  `config/resolve.rs:1013-1021` (env/doc/file layers tested :1988-2020). Service:
  `InvalidMinimum` at `promotion.rs:171-173` (test :1083). Default 3 with the verbatim
  spec quote (`config/defaults.rs:149-157`); canonical fingerprint + decode wired
  (`config/canonical.rs:270-275, 598-600`); docs line added;
  `GAUNTLET_PROMOTION_MARGIN` remains vocabulary-only (no consumption anywhere in
  `crates/`).
- **Charter mechanics D5/D8/D9/D10/D11 hold.** Four-state domain enum; D5 status
  semantics as documented; p10's four legs assert the single-success refusal, the
  identical-axes refusal, the full-conjunction promotion with a durable decision that
  survives reopening, and sticky revoke — all against the REAL sqlite ledger + REAL
  state store written only through the earned authorities; per-leg teeth declarations are
  in-module. AK-055's locator divergence is recorded in
  `.agents/reports/reference-candidates.md` (2026-09-03 entry); no ruler file is in the
  diff. `improve_cmd.rs` untouched, still naming task23. No new CLI/doctor/report surface.

## Declared deviations — adjudication (b1-report §Deviations)

1. **Variation axes omit blast_radius/tier** — ACCEPTABLE-AND-RECORDED. Independently
   confirmed: no `StateStore` read the import projects carries a unit's tier or blast
   radius (settlement rows, verification records, checkpoints, run — none do; there is no
   unit-enumeration read). D4's axis list and D3's no-invention rule conflict here;
   resolving toward D3 is correct and fail-closed (fewer axes ⟹ variation harder).
2. **Revoked is terminal** — ACCEPTABLE-AND-RECORDED. The domain `Capability::restatus`
   table (capability.rs:297-302, unchanged by this slice) permits no exit from Revoked;
   spec §5.3.8 defines statuses but no transition table. The stronger sticky reading is
   fail-closed and suite-pinned. Record-only consequence: operator re-promotion needs a
   new capability id, or a future CandidateRevision opening a governed re-evaluation move.
3. **Provisional→Provisional re-record** — ACCEPTABLE-AND-RECORDED. Required by D4's
   refusal semantics for a second evaluation; a `Refused` reason forces `to ==
   Provisional`, so the extension cannot elevate; the suite pins that demotions are
   unrepresentable through it.
4. **Production call-site wiring deferred** — ACCEPTABLE-AND-RECORDED, with the sharpest
   caveat in this report (see RES-08): D3's two production call sites do not exist; the
   charter's own out-of-scope list (no report/doctor changes, no improve wiring, do not
   build task23) leaves them nowhere to land. Services + composed proof shipped instead.
5. **`at` is the recording instant** — ACCEPTABLE-AND-RECORDED. Not even a deviation from
   D1–D11's letter: it removes timestamps from every decision input and makes ordering
   deterministically append order.

## Findings

### RES-01 — MINOR — variation is decided on axis-VECTOR inequality, conflating absent facts with differing facts

`settled_with_real_variation`, `crates/gauntlet-app/src/memory/promotion.rs:350-362`,
accepts the window when ANY row's `axes()` vector differs from the FIRST row's. A row
with a shorter vector (missing `evidence_kind`/`evidence_locator` facts) "differs" from a
full-facts row even where all populated content is identical — variation is credited by
absence, violating the module's own fail-closed rule ("a window that cannot decide a
condition refuses, it never passes", promotion.rs:711). Scenario: window = [full row,
full row, sparse row carrying only unit+reference axes] → `InsufficientVariation` is not
recorded; erosion passes because the sparse row carries `instrument_deterministic`, burden
passes because it carries `orchestration_cost` → promotion with no real content variation.
Not reachable through import+service alone (the import populates all four axes together
from the matched verification record, and an unmatched settlement independently refuses
via `InsufficientInstrumentEvidence`), but `append_observation` is ungated by design, so a
direct writer can seed exactly this row. Correction direction: decide variation over the
axes populated across the whole window, or refuse windows whose rows disagree in
fact-population. Related nit: `promotion_evidence`'s `filter_map` witnesses
(promotion.rs:484-494) would silently drop such rows from the durable decision while
`settlements_observed` still counts them.

### RES-02 — MINOR — window proxies assume import order ≡ occurrence order; nothing enforces it

Failure-rate halves (promotion.rs:367-393), the erosion scan (:421-436), and the burden
scan (:440-459) all read the ledger's append order — declared, deterministic, and immune
to timestamp games, but append order is fixed at IMPORT time. If runs are imported out of
occurrence order (a stale run re-imported between evaluations), the halves silently
invert and a genuinely rising failure rate can PASS: importing newest-first places the
worse, newer outcomes in the `earlier` half, so `later > earlier` never fires. Today
unreachable (only the p10/module tests drive imports, in order); when the production call
sites land (deviation 4) this needs an occurrence-order projection or an import-order
guard. Fail-closed for burden/erosion is not guaranteed here: the inversion flips
verdicts in both directions.

### RES-03 — MINOR — re-import of an EVOLVING run wedges on `ObservationConflict` and permanently loses that run's later memory

The settlement row's `orchestration_cost` is the run's WHOLE correction-selection count at
import time (developmental.rs:240), and the payment row's `spend` is the latest checkpoint
at import time (:148-155). If an import lands mid-run and the run then grows, the
re-import re-appends the same identity with different facts → `ObservationConflict` → the
whole `import_run` errors, and every later import of that run fails at the same immutable
row (no repair path; the stored row stands by design). Direction is fail-closed — memory
goes missing, never false (an under-count can only cause refusals) — but it contradicts
developmental.rs:24-25's "re-importing a run … appends nothing and changes nothing" for
living runs and can permanently block a legitimate promotion. Post-terminal imports (the
commissioned call site) are stable because the terminal substrate is frozen, so this is
latent until mid-run evaluation is wired. Correction: make per-settlement cost the count
at settlement time, or exclude evolving proxies from `facts_agree`.

### RES-04 — MINOR — "governed promotion only" is convention-governed and auditable, not authority-gated

`StatusTransition::new` + `TransitionReason::Promoted(PromotionEvidence)` +
`apply_transition` are public and unsealed: a composition of public API calls —
`StatusTransition::new(cap, Provisional, Promoted, TransitionReason::Promoted(fabricated
evidence), …)` then `ledger.apply_transition(..)` — elevates status without the service
and without §19.2. This is the shape D2 itself commissioned ("earn it through a typed
transition record the service composes"), and a unforgeable seal is structurally
impossible across the layering (ports cannot know the §19.2 conditions; app cannot inject
constructors into ports) — so this is not a contract breach. But the charter's
discovered-facts house pattern ("earned writes are minted through authority types with no
public constructors … wherever a write must be earned") was applied to settlements
(`AdmittedSettlement`) and not to elevation. Mitigations present: no `set_status`, no
status column, state-machine + staleness checks, and the fabricated evidence travels
INSIDE the durable transition, so a register-vs-log audit detects fabrication post hoc.
Correction direction (future): a promotion-audit check that re-derives a `Promoted`
transition's evidence from the observation log, or an adapter-level derivation gate.

### RES-05 — INFORMATIONAL — nine-dimension inventory: what the decisions actually track

Tracked in conditions + durable decision record: **variation** (axes, condition 2),
**recovery** (count, condition 3), **failure rate** (condition 4), **gate erosion**
(instrument series, condition 5a), **orchestration burden** (cost series, condition 5b).
Recorded but never decided on: **performance** (spend counters land in payment
observations; no decision field uses them), **settlement reliability** (witnesses +
outcomes recorded; no trend condition beyond failure rate). Silently absent everywhere:
**transfer** (no fact, no condition) and **stability** (no fact, no condition; the
failure-rate window is the nearest proxy). Not a charter breach — D4 commissioned the five
conditions — but the port doc's nine-dimension paragraph
(capability_ledger.rs:45-52) reads as broader coverage than exists; the vocabulary list
and the coverage claim are worth separating when task23 touches this.

### RES-06 — INFORMATIONAL — the recovery condition decides on a caller-supplied claim

`DevelopmentalImport::record_recovery` (developmental.rs:260-286) binds to nothing: the
">0 completed" ordering lives purely at the call site (charter D4's own seam, since the
substrate writes no durable recovery marker). Any caller can mint recovery credit with an
empty substrate; every other condition decides on durable substrate facts. p10 drives the
honest seam (real damage → real `complete_pending_settlements` → append after). When
production wiring lands, the completion call site must be the only production caller —
a dedicated wrapper over the completion's returned count would close it.

### RES-07 — INFORMATIONAL — variation-rule strength notes (both within the commissioned rule)

(a) Any-axis-differs means a window of N−1 identical rows + 1 differing row passes; I6's
"N distinct settlements" could be read stricter than the charter's mechanical rule
("identical axes across all N ⟹ refuse"), which is what was built and commissioned.
(b) The locator axis is the exact locator string — the domain `EvidenceLocator` is an
opaque newtype with no class partition (values.rs:781-785) — but under one frozen
reference the locator co-varies with the unit, so its practical reach beyond the reference
axis is nil. Both are recorded interpretations, not defects.

### RES-08 — INFORMATIONAL — deviation 4 leaves the capability composition-unreachable in production (for now)

No production code imports developmental memory or evaluates promotion; first callers are
tests and p10. The slice's claim (mechanism + AK-055 proof) is honestly scoped and the
deferral is declared, but the AGENTS.md "existence is not composition" rule means task23
(or the report-path consumer) must wire D3's two call sites before any production claim
about memory accumulation is true. Flagged so the gap is not forgotten at settlement.

---

## Verdict

The proof's claims are true as stated, the invariants hold on every reachable path, and
the contract is satisfied to its letter with five declared deviations, each adjudicated
acceptable-and-recorded. The four minors are mechanism-quality residuals for a correction
round or the consuming task; none makes a false promotion reachable through the composed
production path.

VERDICT: CONFIRM
0 blocking / 0 major / 4 minor / 4 informational
