# Task 11 — builder round 1 report

Round 1 complete: D1–D6 landed, all gates green, teeth bite. Two builder
sessions: D1–D4 landed 2026-08-21 (see `builder-checkpoints.md`); the
FINISHER session (this one) landed the toolchain-drift lint closure, the
D5 proof, the teeth, and the D6 gates + this report. Rust toolchain moved
1.92.0 → 1.98.0 mid-task; every drift fix is listed in §7 and is
compensation, not logic change.

Ruler: PASS before work began and PASS after (§6 verbatim tails).

## 1. Artifact summary

| Deliverable | Artifact | Tests |
|---|---|---|
| D1 | `crates/gauntlet-ports/src/verification_record.rs` (B.5 strict codec, `BindingClass`); `verifier.rs` port revision (defaulted `supports`, layered doc); `role_result.rs` `RolePayload::Verification`; shared-suite EXTENSION `records_carry_their_instruments_identity` | ports 80/80 |
| D2 | `EvidenceProduction`/`EvidenceProductionBinding` + default-refusing trait methods in `ports/state_store.rs`; sqlite migration `0006_evidence_productions.sql`; server-minted anchor in the adapter | state-sqlite 36/36 |
| D3 | `crates/gauntlet-app/src/verify/{mod,authority,ladder,precondition}.rs` — earned chain, §14.2 ladder with `ModelRung`, §16.1 conditions 1–2 precondition | gauntlet-app 207/207 (31 under `verify::`) |
| D4 | `crates/adapters/gauntlet-adapter-verify-{command-exit-code,test-report,file-digest,schema-check}` — each passes the UNMODIFIED Task-4 shared `verifier_suite` | 15 adapter tests |
| D5 | `crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs` (11 composed proof tests; cargo target name exactly `p12_missing_is_not_passing`) + the four verify adapters as CLI dev-dependencies; `task11/teeth.md` | proof 11/11; 4 teeth bite |
| D6 | Gates in §6; this report; checkpoints appended with DONE | workspace 764/764 |

## 2. What the D5 proof proves (bounded)

The proof exercises the COMPOSED path at the composition root (the CLI
crate, spec §4.1): the real embedded state store on a temp-dir database
file, the real content-addressed fs evidence store, a frozen fixture
answer key stored and digest-pinned through that store, the four REAL
verifier adapters, the app-layer earned `verify::` chain, and the §16.1
settlement precondition. The model rung runs over a scripted in-test
`AgentRunner` (the repo's ScriptedCritic pattern; no network, no live
runner — its strict `role_result@v1`/`verification_record@v1` parsing and
identity cross-checks are on the composed path).

Eleven tests cover, as separate `#[test]` fns:

1. **Instrument layer:** each of the four real adapters PASSes a present
   artifact and reports the typed `Err(EvidenceMissing)` at an absent
   locator — never silence.
2. **Binding layer (absent store evidence):** a fresh production record
   whose ref never landed in the store, with the artifact file PRESENT and
   the request lying about everything — `Outcome::Inconclusive` with class
   `evidence_unresolvable` ("does not resolve" in the detail), and the
   settlement precondition refuses (`NotPass { Inconclusive }`).
3. **Layered P12:** binding checks pass, the real adapter finds no file at
   the locator — `Err(EvidenceMissing)` becomes INCONCLUSIVE/
   `evidence_unresolvable`, the absent rung named on the record, the detail
   cites P12.
4. **Stale:** prior-iteration production → `evidence_stale`; settlement
   refuses.
5. **Lying request:** the request asserts freshness/human producer/another
   locator — the record still says `evidence_stale`, and its requirement
   fields are the FROZEN ones, not the caller's.
6. **Producer mismatch:** instrument production against a frozen
   human-producer requirement → `evidence_producer_mismatch`.
7. **Locator mismatch:** fresh, resolving, passing evidence produced at a
   different locator → `evidence_unresolvable` naming the required locator
   (the recorded mapping).
8. **Happy anchor:** fresh + resolving + pinned → PASS, class `None`, the
   deterministic instrument's identity, settlement readiness granted.
9. **Deterministic-first (AK-035):** a both-rung ladder (model rung
   constructed FIRST) — the deterministic rung decides, its identity is
   recorded, and the model runner records ZERO dispatches.
10. **Model flag:** over the full five-rung ladder, a screenshot claim only
    the model rung can decide — the minted B.5 record carries
    `instrument.deterministic=false` and the model rung's identity.
11. **Three distinct classes:** three independent scenarios mint
    `evidence_unresolvable`, `evidence_stale`, `evidence_producer_mismatch`
    — pairwise distinct, no collapsed catch-all (and PASS records carry
    class `None`, test 8).

**Bounded-claims language:** these properties were proved for the composed
path described above, within the scenario space the eleven tests and the
four teeth enumerate, under rustc 1.98.0 on this host. No claim is made
about live model runners, network dispatch, concurrent writers, or
scenarios not constructed here. "No issue found" in this report means none
was found within that declared scope and budget.

## 3. Design decisions for critic adjudication

Decisions 1–4 were taken at D1–D4 (restated from the checkpoints; full
rationale there):

1. **Layered `EvidenceMissing` reconciliation** (spec §14.3, §14.4, P12 vs
   the frozen Task-4 suite case
   `missing_evidence_is_an_error_not_silence`). The INSTRUMENT reports the
   typed error; the application chain converts it — and every other §14.3
   binding failure — into an `Outcome::Inconclusive` record with the typed
   `BindingClass`. No existing suite case modified; the suite was EXTENDED
   with `records_carry_their_instruments_identity` (recorded act).
2. **Locator mismatch → `evidence_unresolvable`** (§14.3 names three
   classes for four conditions): evidence produced at a different locator
   is not the required evidence, i.e. the requirement's evidence does not
   resolve AT ITS LOCATOR. Documented in the ports module doc, the chain,
   and proved by proof test 7.
3. **Recency anchor** (prereg P1's disconfirming observation): the
   production record carries `(iteration_no, event_log_offset)`; the offset
   is MINTED SERVER-SIDE by the state store when it appends the durable
   `EvidenceProduced` event — no caller of the record API supplies any
   ordering fact, and the verify caller supplies neither side of any
   binding comparison. Wall-clock `produced_at` is provenance only, never a
   recency input.
4. **Model rung in the app layer** (`verify/ladder.rs::ModelRung`) over the
   `AgentRunner` port, `is_deterministic() == false`, §14.2 position 10;
   every record it mints is flagged; a model record claiming
   `instrument.deterministic=true` is refused.

Decisions 5–6 are the finisher session's:

5. **AK-005's frozen locator cannot materialize at the repo root** — see
   §4. The proof lands as an in-crate composition-root integration test
   whose cargo target name is exactly the plan's gate spelling (`cargo
   nextest run --test p12_missing_is_not_passing` runs verbatim). This
   follows the recorded p1/p3 precedent, and a draft reference-candidates
   entry for the locator is included (§8).
6. **Toolchain drift compensation only** (§7): every lint-era edit is a
   minimal warning fix or formatting normalization; none changes behavior,
   and the full suite + boundaries + ruler bracket them.

## 4. The AK-005 locator conflict (evidence, not preference)

The brief's D5 spelling — declare
`tests/proofs/p12_missing_is_not_passing.rs` (repo root) as a `[[test]]`
target of `gauntlet-cli` — was attempted first and is FORBIDDEN by the
frozen boundary suite, empirically (2026-08-21, rustc 1.98.0):

- `scope_integrity` FAILED:
  `scope-escape: crates/gauntlet-cli/Cargo.toml: [test] path =
  '../../tests/proofs/p12_missing_is_not_passing.rs' (resolved:
  'tests/proofs/p12_missing_is_not_passing.rs') points a compilation
  target's source root outside crates/gauntlet-cli — the walk scans a file
  that is no longer compiled and the compiler compiles a file nothing
  scans (spec §24.3, AK-001)`
- `core_crates_contain_no_hardcoded_literals` FAILED:
  `hardcoded-relative-path: crates/gauntlet-cli/Cargo.toml:65:
  '../../tests/proofs/p12_missing_is_not_passing.rs' — a path literal is
  environment coupling wherever it is rooted (spec §8.1, AK-016)`

The exemption list is pinned by an existing suite case
(`escapes::tests::the_target_path_exemption_is_exactly_the_boundary_suite`)
and `gauntlet-arch-tests` cannot host the target either (it depends on no
internal crate — its Cargo.toml constraint). Extending the exemption to a
core crate would weaken an acceptance gate — not a builder act. The probe
was reverted; resolution per the p1/p3 precedent: the proof lives at
`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`, autodiscovered
with the exact target name, its header names the frozen locator and records
the materialization failure, and the four verify adapters landed as CLI
dev-dependencies (the real wiring change). No `[[test]]` path entry exists.

## 5. Teeth (summary — full table and reproduction in `teeth.md`)

| Neutered check | Failing proof test | Failure |
|---|---|---|
| §14.3 recency (condition 3) | `stale_evidence_from_a_prior_iteration_is_inconclusive_stale` (+ lying twin) | `left: Pass / right: Inconclusive` |
| §14.3 producer (condition 2) | `instrument_produced_evidence_never_satisfies_a_human_producer` | `left: Pass / right: Inconclusive` |
| §14.2 ascending ladder sort | `the_deterministic_rung_decides_when_the_model_rung_could_too` | `left: ModelJudgment / right: TestReport` |
| §14.3 resolvability (condition 1) | `absent_store_evidence_is_inconclusive_at_every_layer_never_pass` | `left: Pass / right: Inconclusive` |

Each neutering was reverted by manual inverse restore from a byte-identical
backup (`diff` clean; the files are untracked), and the full proof suite
re-ran green after the last revert.

## 6. Gate outputs (verbatim tails, 2026-08-21, rustc 1.98.0)

Focused (the plan's gate, verbatim commands):

```text
$ cargo nextest run --test p12_missing_is_not_passing && cargo nextest run -p gauntlet-app verify::
     Summary [  0.074s] 11 tests run: 11 passed, 0 skipped
     Summary [  0.097s] 31 tests run: 31 passed, 176 skipped
```

Full gates (`just check && just lint && just deny && just boundaries && just ruler`):

```text
    Finished `dev` profile [unoptimized] target(s) in 0.35s        # cargo check --workspace --all-targets
    Finished `dev` profile [unoptimized] target(s) in 0.24s        # cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check                                               # clean
advisories ok, bans ok, licenses ok, sources ok                      # cargo deny check (warnings are pre-existing allowlist notes, exit 0)
     Summary [  15.067s] 194 tests run: 194 passed, 0 skipped       # just boundaries
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

Full workspace suite:

```text
$ just test
     Summary [  15.610s] 764 tests run: 764 passed, 0 skipped
```

Baseline context: 697 at round-1 dispatch + D1 (7) + D2 (3) + D3 (31) +
D4 (15) + D5 (11) = 764. Boundaries 194/194 with the two D4-fallout pins
(checkpoints D4 note) and no new exemptions this session.

## 7. Toolchain-drift lint closure (rustc/clippy/rustfmt 1.92.0 → 1.98.0)

All are compensation for the newer toolchain; none is a logic change; each
is bracketed by the green gates above:

1. `crates/gauntlet-arch-tests/src/walk.rs` (`list_unfiltered`): the guard
   arm `Err(err) if !dir.exists() => return` binds `err` unused — 1.98
   lints it. Fixed to `Err(_)` in that arm only; the reporting arm still
   uses `err`. (The brief's named pre-existing line.)
2. `crates/gauntlet-adapter-state-sqlite/src/state_store.rs`
   (`production_from_parts`): clippy 1.98 counts 8 args over the 7
   threshold. Added `clippy::too_many_arguments` beside the existing
   `type_complexity` allow (the repo's recorded `mint`-style allow).
3. `crates/gauntlet-arch-tests/src/escapes.rs` (`scan_unterminated`):
   clippy 1.98 `collapsible_match` — the `b']'` arm's body was only the
   `if brackets.pop().is_none()`. Collapsed into the match guard;
   behavior-identical (the guard pops exactly once per `]`; a false guard
   falls to `_ => {}` exactly as the skipped body did).
4. rustfmt 1.98 normalization over five untracked round-1 files
   (`verify/authority.rs`, the four verify adapters): `cargo fmt --all`
   applied; `--check` clean.

Non-drift lint events this session (recorded for completeness): two
`clippy::arc_with_non_send_sync` allows in the NEW proof file at the
seeded-id-generator and scripted-runner `Arc::new` sites — the same
recorded allow `verify/ladder.rs` itself carries for the mock-backed
rungs.

## 8. DRAFT reference-candidates entries (for the orchestrator to land)

### AK-005 evidence locator (Task 11, D5)

- **Unit:** AK-005 (`.wayfinder/answer-key.md` — three-state Outcome;
  INCONCLUSIVE never satisfies a settlement requirement).
- **Claim under review (unchanged, satisfied at the instrument level):**
  the composed P12 property is proved (this report §2).
- **Defect (evidence declaration only):** the locator
  `tests/proofs/p12_missing_is_not_passing` cannot materialize as a cargo
  target of a core crate — the boundary suite's cargo-target-path rule and
  literals scan both fire (verbatim evidence in §4). Same
  locator-materialization class first recorded 2026-08-17 (AK-049); the p3
  header records the same for P3.
- **Proposed correction:** re-declare the locator to
  `gauntlet-cli tests/p12_missing_is_not_passing` — the composition-root
  integration test whose cargo target name is exactly the plan's focused
  gate spelling. Carrying tests: all eleven (§2 list); the settlement
  refusal halves pair with
  `gauntlet-app verify::precondition::tests::an_inconclusive_record_is_refused_and_stays_distinct_from_fail`.
- **Not changed by this entry:** the answer key, the spec, the frozen lock.

### AK-035, AK-036 evidence locators (Task 11, D5)

- **Units:** AK-035, AK-036 (`.wayfinder/answer-key.md`).
- **Claims under review (unchanged, satisfied at the instrument level):**
  "Verification selects the cheapest conclusive instrument and prefers
  deterministic verifiers over model judgment" (AK-035); "A verification
  record is rejected as INCONCLUSIVE when its evidence is unresolvable,
  stale relative to the transform, or produced by a non-matching producer"
  (AK-036).
- **Defect (evidence declaration only):** the locators
  `tests/contract/verifier::deterministic_first` and
  `tests/contract/verifier::evidence_binding` spell repo-root test targets
  with `::` in the path, which the same cargo-target-path rule forbids for
  core crates. Additional instances of the AK-049
  locator-materialization class (same as AK-026/027/028–030/032–034,
  recorded 2026-08-17 through 2026-08-20) — not a new defect class.
- **Proposed correction — AK-035:**
  `gauntlet-cli tests::p12_missing_is_not_passing
  the_deterministic_rung_decides_when_the_model_rung_could_too` (composed:
  both rungs declare support, the deterministic instrument's identity is
  recorded, the scripted model runner records ZERO dispatches), corroborated
  by `gauntlet-app verify::ladder::tests::when_a_deterministic_rung_and_the_model_rung_could_both_decide_the_deterministic_one_does`
  and `...::the_model_rung_decides_only_where_no_deterministic_rung_can`,
  plus the composed flag test
  `every_model_rung_record_is_flagged_non_deterministic`.
- **Proposed correction — AK-036:**
  `gauntlet-cli tests::p12_missing_is_not_passing
  the_three_binding_classes_are_reachable_and_distinct` with its sibling
  scenario tests
  (`absent_store_evidence_is_inconclusive_at_every_layer_never_pass`,
  `stale_evidence_from_a_prior_iteration_is_inconclusive_stale`,
  `instrument_produced_evidence_never_satisfies_a_human_producer`,
  `evidence_at_another_locator_is_unresolvable_not_the_required_evidence`,
  `an_absent_artifact_at_a_resolving_production_is_inconclusive_p12`),
  corroborated by `gauntlet-app verify::authority::tests::*`
  (`an_unresolvable_evidence_ref_is_inconclusive_unresolvable`,
  `evidence_from_a_prior_iteration_is_inconclusive_stale`,
  `instrument_produced_evidence_never_satisfies_a_human_requirement`,
  `a_lying_request_cannot_make_stale_evidence_fresh`) and
  `gauntlet-adapter-state-sqlite
  state_store::tests::evidence_production_records_are_write_once_and_idempotent`.
- **Not changed by this entry:** the answer key, the spec, the frozen lock
  (`just ruler` PASS brackets this round; the orchestrator's post-land
  re-run is the confirming evidence). Settlement of these candidates
  belongs to the ruler's owner, not the builder.

## 9. Deferrals with triggers

- **Settlement conditions 3–5** (authority chain, blocking residuals,
  reference digest at settlement) — Task 13; recorded in
  `verify/precondition.rs`'s module doc. Trigger: plan Task 13 dispatch.
- **Capability-gating the evidence-production write** to the evidence-owner
  role — residual recorded in `ports/state_store.rs` (mirrors Task 10's
  observation-write note). Trigger: Task 12 wiring the production writer.
- **`cargo-mutants` at the settlement boundary** (AGENTS testing loop): the
  four named teeth discriminate this round's checks; the broader mutation
  run belongs to the settlement boundary, not mid-round. Trigger: Task 11
  settlement / round-1 critic pass.
- **Locator re-declarations** for AK-005/AK-035/AK-036 (§8 drafts).
  Trigger: orchestrator lands them in
  `.agents/reports/reference-candidates.md` and the ruler's owner settles.

## 10. Suitability statement

Within the declared scope (§2 bounded claims), the round-1 state is green
on every gate the plan names, the frozen ruler is untouched and PASSes,
the preregistration is untouched, and no existing suite case was modified.
Every consequential decision is listed for adjudication in §3.
