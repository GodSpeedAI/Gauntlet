# Task 23 — executing falsifier, round 1 (post-COR-1)

VERDICT: **NOT_CONFIRM** — two reproducible §19.4 enforcement holes were
executed to green adjudication paths (one through the REAL binary, one at the
real gate/service boundary over the real sqlite adapters), plus three
secondary arithmetic/substrate/scan findings. All declared proof teeth BITE
(each neutralization went RED; every file restored byte-identically,
sha256-proven). Post-restore regression sweep: green.

Attack surface: blind over builder/critic/correction reports; knowledge =
falsifier brief, charter, spec §19.3/§19.4/§5.3.8/B.7/I7/P9, AK-056/AK-057,
the tree as it stands. All attacks executed against the REAL adapters
(embedded-sqlite revision/capability/state stores), the REAL §19.4 gate, and
the REAL `gauntlet` binary in /tmp scratch worlds.

---

## Findings

### F-1 MAJOR (executed through the real binary) — §19.4 condition 2 covers only the held-out arms; the regression arms' reference digests are never recorded nor checked (a moved regression ruler evaluates cleanly)

Mechanism: `EvaluationRecord`
(`/home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/revision_ledger.rs`,
fields ~line 511-512) carries ONLY `control_reference_digest` /
`treatment_reference_digest`, populated by the harness
(`crates/gauntlet-app/src/promote/evaluate.rs`, record constructor ~line 369)
from the HELD-OUT outcomes only. The record has no regression-arm digest
fields, so the gate (`crates/gauntlet-app/src/promote/gate.rs`, condition 2,
~line 238) structurally cannot check 2 of the 4 evaluation arms. The
regression set is an operator-named project root with its OWN `.wayfinder`;
if its lock is absent it freezes its own (possibly moved) ruler, the arms
settle under it, the evaluation records fine, and condition 2 passes.

Reproduction (real binary, scratch world /tmp/gauntlet-falsifier/binworld):

```
# held = pristine fixture copy; reg = fixture copy with:
#   rm reg/.gauntlet/reference.lock
#   sed -i 's/GADGET_MARKER_V1\./GADGET_MARKER_V1 (amended for the falsifier probe)./' reg/.wayfinder/answer-key.md
sha256sum held/.wayfinder/answer-key.md reg/.wayfinder/answer-key.md
# ebba17b091b2c66b161a0196d3fd3497759a27cc747c5b7550f0430f48a44ce5  held/...
# 3f6179169ba2bd92d9cf20b810b89fc86b2bb51361c35c82783eba227e8404d9  reg/...
gauntlet improve propose --from-result candidate.json
# improve: revision=rev-95d2e59be8a15509 admitted (idempotent=false)
gauntlet improve evaluate rev-95d2e59be8a15509 --held-out $W/held \
    --regression $W/reg --protected cap-eval --metric iterations_per_settlement
# improve: evaluation of rev-95d2e59be8a15509 recorded (idempotent=false)
# improve: held-out control=DNAJTJAQY481P5YDG4V1NVYVE2 treatment=4WQ2WBBH3ZZS1X0DEJA1VW7Q72 (parent_run set)
# improve: regression control=9TG5CJZ0EKGWWA3MV7BY03FFK3 treatment=VBBEAXGVMW568J1Y9KF0893386 (parent_run set)
# (durable run records, read back through the state store:)
# run=DNAJTJAQY481P5YDG4V1NVYVE2 digest=sha256:ebba17b0... parent=none
# run=4WQ2WBBH3ZZS1X0DEJA1VW7Q72 digest=sha256:ebba17b0... parent=DNAJTJAQ...
# run=9TG5CJZ0EKGWWA3MV7BY03FFK3 digest=sha256:3f617916... parent=none          <- MOVED RULER
# run=VBBEAXGVMW568J1Y9KF0893386 digest=sha256:3f617916... parent=9TG5CJZ...   <- MOVED RULER
# (durable evaluation record:) control_reference_digest = sha256:ebba17...
#                              treatment_reference_digest = sha256:ebba17...
#                              (no regression-arm digest fields exist at all)
gauntlet improve promote rev-95d2e59be8a15509
# improve: revision rev-95d2e59be8a15509 REJECTED
# improve: refused — margin not met: iterations_per_settlement control 10000 vs
#          treatment 10000 ... (spec 19.4, condition 7)
```

Condition 2 PASSED over an evaluation whose regression arms ran under a
different frozen reference; the rejection came only from the margin (the
fixture loop's recorded scale-invariance keeps the figures equal — the same
reason the p9 header records for the legs-2/3 gate-level deviation). The
moment any inert candidate produces a genuine figure improvement (the
recorded binding re-key would create exactly that), this hole reaches a
full Promoted with a regression suite adjudicated under a moved ruler.
Condition 5's control-vs-treatment regression comparison is likewise
unit-set equality without any ruler binding.

Fix locus: `EvaluationRecord` gains `regression_control_reference_digest` /
`regression_treatment_reference_digest` (port + adapter payload + harness
derivation), and gate condition 2 checks all four. The harness additionally
ought to refuse an arm whose project resolves a digest different from the
promoting project's verified lock at evaluate time.

### F-2 MAJOR (gate/service boundary, real sqlite adapters) — condition 3's instrument-series comparison pairs settlements POSITIONALLY over store order; an extra treatment settlement earlier in the series shields a genuinely weakened unit

Mechanism: `instrument_series_verdict`
(`/home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/gate.rs`
~line 639) compares `control.settlements[position]` against
`treatment.settlements[position]` and only while `position <
control.len()`. Treatment settling MORE units is accepted (extra dispatch
"passes"), so a treatment ordering `[extra_unit(det), u1(NONDET)]` pairs its
extra deterministic settlement against control's only settlement and never
compares the weakened u1. Condition 5 matches BY UNIT but runs over the
regression metrics only — the weakened held-out unit is invisible. The gate
then PROMOTES with `instrument_series_non_weaker = true` while verification
was actually weaker on a real unit — precisely AK-056's hidden-weakening
shape (e.g. a run that gamed instrument selection without a delta).

Reproduction (falsifier probe A over `EmbeddedRevisionLedger` +
`EmbeddedCapabilityLedger` + the real `PromotionGate`; probe passed):

```
control held-out   : settlements=[(u1, deterministic)]                 dispatched=[u1]  iters=3 pay=3000
treatment held-out : settlements=[(u0, deterministic), (u1, MODEL RUNG)] dispatched=[u0,u1] iters=3 pay=2000
delta=[] (inert), all digests = current, protected cap with 1/1 settlement memories
gate.adjudicate(...) => GateVerdict::Promoted(evidence { instrument_series_non_weaker: true, ... })
```

Control probe A2 — the SAME weakened u1 with no shielding extra settlement
aligns positionally and is REJECTED with `settlement_access_weakened`; so
the check bites exactly and only while the pairing aligns. Reachability
today is bounded: through the current CLI composition both arms derive
instruments from the same configuration and all instrument-affecting delta
keys are refused/unappliable — but the gate is the durable second-line
defense against exactly the run-level gaming §19.4 lists ("adversarial/
gaming cases"), and it is dodgeable by settlement ORDER, which neither the
harness nor the gate controls.

Fix locus: pair settlements BY UNIT (as `regression_verdict` already does),
refuse/undecide on unit mismatch, and treat a treatment unit settled under a
weaker instrument than control's SAME unit as `SettlementAccessWeakened`
regardless of list shape.

### F-3 MINOR/MAJOR (figure arithmetic, service level) — margins above 10 000 bps silently clamp to 10 000; a declared >100% margin is enforced as exactly 100%

Mechanism: `margin_holds` (`gate.rs` ~line 727):
`10_000u64.saturating_sub(margin_bps.min(10_000))`. A configured
`GAUNTLET_PROMOTION_MARGIN` of 15000 (a 150% required improvement —
mathematically unsatisfiable for positive figures) is enforced as "treatment
figure must be 0", i.e. a 100% improvement PASSES a declared 150% margin.
Undocumented saturation; the doc comment claims only the margin-0 floor.

Reproduction (falsifier probe B; passed): figures control=100 / treatment=0
with `PromotionGate::new(..., 15_000)` → `Promoted`. Fix locus: refuse
`margin_bps > 10_000` at config resolution (typed error), or document the
saturation; do not silently reinterpret the operator's declared bar.

### F-4 MINOR/MAJOR (substrate idempotency, real adapter) — the evaluation record's declared `metric` and `protected` set are OUTSIDE the idempotency agreement; a re-append with different declarations is silently swallowed

Mechanism: `EvaluationRecord::facts_agree`
(`/home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/revision_ledger.rs`
~line 532) omits `metric`, `protected` (and `revision` is part of the UNIQUE
key). `append_evaluation` (sqlite adapter ~line 335) treats a same-run-set
record with a DIFFERENT declared metric / EMPTIER protected set as the
idempotent no-op and keeps the ORIGINAL declaration, never the typed
`EvaluationConflict` the module docs promise ("the same four runs
re-appended agree" is not what is compared).

Reproduction (falsifier probe C; passed): append record
(metric=iterations_per_settlement, protected=[cap-probe]); re-append same
four run ids with metric=payment_per_settlement, protected=[] →
`Ok(EvaluationAppend { idempotent: true })`; stored record keeps the
original declaration. Not reachable through the current CLI (every evaluate
mints fresh run ids) — a port-consumer hazard only. Fix locus: include
`metric` and `protected` in `facts_agree` (one line each) and add a suite
case.

### F-5 MINOR (governance classification, service level) — `target_is_governance_bearing` is CASE-SENSITIVE prefix matching; capitalized governance-bearing targets classify as free

Mechanism: `gate.rs` ~line 615: `GOVERNANCE_PREFIXES` matched with
`starts_with`, no normalization. Reproduction (falsifier probe E; passed):
`target_is_governance_bearing("Reference-pipeline") == false`,
`("Settlement-machinery") == false`. The target spelling only gates
condition 1's refusal sub-check (the substantive reference/delta conditions
are independent), so this weakens the recorded REASON fidelity, not the
ruler. Fix locus: case-normalize (or exact-token match) the classification;
the payload validator could normalize the target at admission.

### F-6 MINOR (AK-057 containment scan coverage) — the optimizer source scan's token list misses the raw write classes (`std::fs::write`, `File::create`/`OpenOptions`, `std::env::set_var`, `Command::new`)

Mechanism: `crates/gauntlet-arch-tests/src/optimizer.rs` bans only
`set_whole(|set_toggle(|set_list(|set_text(|FlagOverrides|apply_env_layer|
apply_env_value` (rule A) and `InstallInput|InstallReport|RolesLock|
::install|install::` (rule B). A promote module that rewrites GAUNTLET.md or
the roles dir through raw fs/env/process would pass rule A and B while
violating "the optimizer may not modify the active loop". Reproduction
(falsifier probe H over the real `scan_optimizer_containment` function;
passed): a fixture file containing `std::fs::write(project.join("GAUNTLET.md"),
...)`, `std::fs::write(.../critic.md ...)`, `std::env::set_var("GAUNTLET_REVIEW_MODE",
...)`, and `Command::new("cp")` reports ZERO violations (positive sanity
control with `FlagOverrides` reports). Mitigations that keep this MINOR: the
behavioral half of AK-057 (`promotion::candidate_only`) proves the REAL
services change nothing, and the service structs hold only ledger+clock.
Fix locus: add the raw-write tokens to rule A (and a `set_var` token), or
replace the token list with an import-level allowlist for the module.

### INFORMATIONAL — verified-holding surfaces (negative controls)

- Decision state machine over the REAL adapter: stale `claimed` refused
  (`StaleDecision`), `pending→provisional→promoted` legal, write-once past
  final states (falsifier probe D).
- Reopen durability: promoted+restored state survives a fresh ledger open;
  second restore refuses (`StaleRestore`); re-adjudication refuses
  (`AlreadyFinal`) (probe F; also p9 leg 3).
- CLI refusal shapes, real binary: decision-carrying payload refused
  (`candidate_revision@v1 rejects unknown field "decision"`); unknown
  payload schema refused (`candidate_revision@v2 has no validator`); unknown
  metric token refused (typed vocabulary); rollback of a pending revision
  (`NothingToRestore`); promote of an unknown revision (typed
  `UnknownRevision`); malformed answer-key amendments refused by the
  reference FORMAT parser.
- Delta application is real: delta overrides are appended after the host env
  in `EnvLayer::from_parts` insertion order, so the declared candidate value
  provably reaches the treatment arm's resolved config; `validate_delta`
  refuses entries with no Annex E variable BEFORE any arm runs.
- `runners.by_role.<role>.model` is classified INERT in `delta.rs` (a latent
  policy question: a critic/verifier model downgrade is a review-strength
  reduction), but it is UNREACHABLE through the binary — by_role keys have no
  Annex E env mapping, so `validate_delta` refuses any evaluation carrying
  one. Fail-closed today; flag for the ruler owner before any composition
  learns to apply by_role keys.
- Rejection honesty: `GateDecision::rejected` refuses empty reasons; an
  admission can never carry an empty rollback ref (EvidenceRef grammar), so
  the "rejection without rollback" shape is unrepresentable.
- Margin floor semantics at margin 0: strict improvement enforced
  (equal figures refused — exercised by the binary run in F-1).

---

## Teeth — every declared tooth BITES (neutralize → RED → restore byte-identically)

| Tooth (as declared by the leg) | Neutralization | Observed |
|---|---|---|
| Leg 1 + isolation `a_weakening_candidate_improves_the_metric_yet_is_rejected_with_settlement_access_weakened`: the settlement-access classification | `preserves_verification` → `true` (delta.rs) | leg 1 RED (`AK-056's refusal is recorded` assertion failed; rejection degraded to margin_not_met); isolation test RED and shows the weakened candidate **PROMOTED** (metric 30000→20000) — the promotion-by-weakening shape is real one gate-mechanism away |
| Leg 2's condition-7 floor (pinned by the margin unit tests over the same gate) | `margin_holds` → `true` (gate.rs) | `margin_zero_demands_strict_improvement`, `a_configured_margin_the_improvement_does_not_meet_refuses`, `an_equal_metric_refuses_at_margin_zero` all RED (3/3) |
| Leg 3: transition-derived restore history | ignore `entry.rolled_back` in `RollbackService::restore` (rollback.rs) | leg 3 RED (second-restore refusal no longer `NothingToRestore`) |
| Leg 4: `MissingEvaluation` ⇒ provisional floor | zero-evaluation early return → `Promoted` (gate.rs) | leg 4 RED; `missing_evaluation_adjudicates_provisional_and_stays_pending` RED |
| AK-057 structural scan: negative controls | in-suite fixtures | PASS (planted rule-A/rule-B shapes reported; empty source set reported) — see F-6 for the coverage gap |

Each mutation was reverted in the same session; all three touched files
verified byte-identical by sha256 against pre-mutation digests (see ledger).

## Not attacked (next round's benefit)

- The promote-module's 40 in-module tests were used but not individually
  audited for assertion strength; only the teeth-relevant ones were exercised
  under mutation.
- Concurrent double-adjudication (two processes racing `apply_decision`
  across two `SqliteRevisionLedger` handles on one file) — the BEGIN
  IMMEDIATE transaction makes this safe in-process; cross-process
  interleaving was not exercised.
- `just ruler`, `just boundaries` full runs, and the full 1498-test battery
  were not re-run post-restore (slice suites + arch optimizer suite were;
  all green).
- Real-runner optimizer dispatch (`propose` without `--from-result`) —
  mock/scripted paths only; task 25 territory.
- Payment figure derivation (`cost_usd` accumulation across the composed
  mock loop) was accepted as p10-proven, not re-derived.
- Fuzzing the hand-rolled JSON codecs (structurally strict by reading;
  not fuzzed).

## Restore ledger

| File | sha256 before | sha256 after | verdict |
|---|---|---|---|
| `crates/gauntlet-app/src/promote/delta.rs` | 4276075ececd7e80b08b0995a8162786ca6c1e264301cceb7995a5d37b30b9bc | 4276075ececd7e80b08b0995a8162786ca6c1e264301cceb7995a5d37b30b9bc | byte-identical |
| `crates/gauntlet-app/src/promote/gate.rs` | 38594f47883a3dd5a814c7640e853a3218e1a8daaa8a77638945942cf114b8ce | 38594f47883a3dd5a814c7640e853a3218e1a8daaa8a77638945942cf114b8ce | byte-identical |
| `crates/gauntlet-app/src/promote/rollback.rs` | 037c8b2d84ad913857ea814ad70d46f8ec72bc30077b9e69339cac88201c51fc | 037c8b2d84ad913857ea814ad70d46f8ec72bc30077b9e69339cac88201c51fc | byte-identical |

Repository files CREATED by the falsifier: this report only. Scratch worlds
exclusively under /tmp/gauntlet-falsifier (probe crate + binary worlds),
removed after the sweep. Nothing planted under `.gauntlet/`; the frozen
ruler (`.wayfinder/answer-key.md`), `.gauntlet/reference.lock`, and
`.wayfinder/reference.yaml` were never opened for write; `git status`
delta vs. dispatch is the builder's uncommitted task work, untouched.

Progress log:
- Initialized. Brief, charter, spec §19.3/§19.4/§5.3.8/B.7/I7/P9, AK-056/AK-057 read.
- Attack plan: (1) AK-056 gate+teeth, (2) AK-057 containment scan+codec+CLI,
  (3) §19.4 verdict boundary arithmetic, (4) revision ledger substrate,
  (5) evaluation harness symmetry/isolation.
- Baseline confirmed (my own run): promotion::candidate_only 1/1, promote:: 40/40,
  p9 4/4 — all PASS. (Build harness note: `sccache --stop-server` exits 2 with
  "couldn't connect" when the server is idle; the suites themselves were green.)
- Scratch probe crate at /tmp/gauntlet-falsifier (probes over REAL sqlite adapters
  + REAL gate; shares the workspace target dir under the compile lock).
- Probe set: A series-pairing shield; A2 aligned-pairing control; B margin
  saturation; C evaluation redeclaration idempotency; D decision state machine
  (negative control); E governance-target case sensitivity; F reopen durability
  (negative control).
- PROBE RESULTS (round 1, real sqlite adapters + real gate):
  A  series-pairing shield ............ CONFIRMED (Promoted with weakened u1;
                                       instrument_series_non_weaker=true)
  A2 aligned-pairing control .......... caught (Rejected) — the check works
                                       only while the pairing aligns
  B  margin saturation (15000bps) ..... CONFIRMED (100% improvement passed a
                                       declared 150% margin)
  C  evaluation redeclaration ......... CONFIRMED (different metric/protected
                                       on same run-set = idempotent no-op)
  D  decision state machine ........... HOLDS (stale claims refuse; write-once)
  E  governance-target case spelling .. CONFIRMED ("Reference-pipeline" not
                                       classified governance-bearing)
  F  reopen durability ................ HOLDS
- BINARY WORLD (real gauntlet binary, /tmp scratch fixture world):
  * Moved-regression-ruler attack CONFIRMED: `improve evaluate` succeeded with
    held-out arms freezing digest sha256:ebba17... and regression arms freezing
    sha256:3f6179... (amended answer key, fresh freeze). The durable evaluation
    record carries NO regression-arm digest fields. `improve promote` adjudicated
    past condition 2 and rejected ONLY on condition 7 (equal figures). The §19.4
    reference-integrity condition structurally sees 2 of 4 arms.
  * CLI refusal shapes all HELD: decision-carrying payload refused (unknown
    field); candidate_revision@v2 refused (no validator); unknown metric token
    refused (typed vocabulary); rollback of pending = NothingToRestore; promote
    of unknown revision = typed UnknownRevision; malformed answer-key edit
    refused by the reference FORMAT parser (a different tripwire than digest).
- Leads resolved during the round:
  L3 (by_role model Inert): unreachable through the binary — no Annex E env
  mapping; validate_delta refuses. Latent policy note only.
  L4 (per-settlement dilution): unreachable through the binary — frozen plan
  bounds the unit set; governance superset rule documented as intentional.
  L7 (caller-supplied delta): CLOSED — content-addressed re-read at both ends.
- Teeth T1 (delta classification) RED x2, restored. T2 (margin floor) RED x3,
  restored. T4 (provisional floor) RED x2, restored. T3 (rollback derived
  state) RED x1, restored. All sha256-verified byte-identical.
- F-6 arch-scan coverage gap CONFIRMED at function level (probe H).
- POST-RESTORE REGRESSION SWEEP (all green): promotion::candidate_only 1/1;
  promote:: 40/40; p9_self_improvement_containment 4/4; gauntlet-arch-tests
  optimizer:: 5/5.
- Scratch world /tmp/gauntlet-falsifier removed after the sweep; repo
  residue re-checked with git status (untracked = the builder's task work +
  this report only).
