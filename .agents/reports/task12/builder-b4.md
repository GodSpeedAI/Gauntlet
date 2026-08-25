# Task 12 — Builder slice B4 result contract (proofs p4/p6 + teeth)

**Two-session shape**: the predecessor session completed recon (three
load-bearing findings, recorded in `builder-b4-checkpoints.md`) and died
mid-Item-1 with `p4_corrective_loop.rs` orphaned; this finisher session
reconciled per `finisher-brief-b4.md`, diagnosed before repairing (the
diagnosis checkpoint entry precedes all repair work), completed Items 1–4.
All four checkpoint entries + the diagnosis entry live in
`builder-b4-checkpoints.md`; the teeth record lives in `teeth.md`.

## Item 1 — `crates/gauntlet-cli/tests/p4_corrective_loop.rs` (P4, AK-038)

**Predecessor credit**: the entire proof structure — the composition
inventory, ScriptedBuilder, FacetExitVerifier, CountingState,
NeverAdmittedCritic, Adaptive review wiring, and all three tests — was the
predecessor's; it compiled clean except one unused import.

**Finisher diagnosis (before repair)**: 2/3 tests passed; the main proof
failed on a CASE-WRONG assertion literal — the pinned §16.1 refusal reads
"the verification outcome is FAIL" (domain `Outcome` renders
SCREAMING_SNAKE) but the fixture asserted "…is Fail". The loop itself had
run correctly (every earlier assertion passed). Repairs: the literal
corrected; the unused import removed. Nothing else.

**Final shape** (3/3 PASS):
- `an_injected_defect_is_exposed_compared_and_closed_by_a_routed_correction`
  — REAL controller at the composition root: fail-then-fix through the
  proof-local scripted AgentRunner (see deviations); REAL command observer
  re-runs the fixture's declared check over the same workspace; REAL diff
  comparator under Adaptive review (zero model allowance; a never-admitted
  critic fails loudly if the lease discipline broke); governed chain
  verifies FAIL (pinned) then PASS (settles the unit); the routed
  re-dispatch carries the residual TEXT ONLY; both gated owner-writes
  counted through the authority-minted store; the settlement precondition
  is asserted honestly as Task 13's boundary (never committed).
- `a_mid_run_answer_key_edit_cancels_the_in_flight_attempt_and_aborts` —
  §11.4 step 3 composed: the per-tick strict re-read cancels the IN-FLIGHT
  attempt (durable Cancelled terminal event, exactly one dispatch), the
  boundary abort pins BOTH digests. The teeth-T3 anchor.
- `correction_selection_durably_carries_rejected_alternatives_with_reasons`
  — AK-037 at the composed level: the REAL selector over the REAL meter
  and REAL store records chosen + rejected alternatives with typed
  reasons (authority-blocked and ranked-out), read back whole through the
  same store path the loop's record travels.

**Gate-driven repairs (finisher, Item 4)**: the boundaries literals scan
flagged the predecessor's spelled check program (`"grep"` — a §8.4
COMMAND_HEADS identity) and duration-suffixed consts. The check
program/arguments/artifact path are now PARSED from the REQUIRED
lying-builder fixture (the p3 idiom — no command-head literal in source),
and the duration consts carry span vocabulary. Behavior identical (the
fixture's own check `grep PASS out/check-target.txt` is exactly what the
observer re-runs; the artifact lands at the fixture's declared changed
path).

## Item 2 — `crates/gauntlet-cli/tests/p6_budget_control.rs` (P6, AK-051/052)

New file (finisher). 2/2 PASS:
- `a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact`
  — the REAL `MockAgentRunner` replaying the REQUIRED `lying-builder`
  fixture verbatim EVERY invocation (never-fixing; the same residual
  recurs); REAL command observer re-runs the fixture's own declared check
  each iteration; tight budget rounds=6 reserve=15% (forward ceiling 5,
  computed consts, never a second literal); ATTEMPT_CEILING=7 above the
  forward ceiling so the BUDGET binds before §11.5 row 5. Asserts:
  terminal `budget_exhausted`; admitted meter reservations == durable
  invocation rows == 5 (AK-052 rows-vs-reservations, composed); reserve
  INTACT on every dimension against the final durable §18.2 checkpoint's
  `budget_spent` (≤ limit×(100−pct)/100); 6 checkpoints (5 iteration
  boundaries + the close-out, pinned exactly); 5 distinct observation
  bundles each carrying the failing re-run (the durable recurrence
  evidence — the five §16.1 FAIL refusals deduplicate to one
  content-addressed object, documented in-test); unit never settled; the
  lie stays observable on disk; final state exports.
- `the_meter_refuses_the_reserve_consuming_round_and_books_nothing` — the
  supplement's own boundary: five admissions book, the sixth (reserve)
  round is refused with the domain's typed cause and books nothing;
  zero-round slices still hit the REAL adapter's reservation bound.

**The composition-root meter supplement (predecessor finding 3,
documented deviation)**: the REAL `MeteredBudget` for everything the port
expresses, plus a domain-`Budget` rounds shadow booking each ADMITTED
reservation's round at reserve time (pure consult first — a refused slice
takes no parallel slot); `remaining()` merges shadow rounds + real
everything-else. This is the "iteration boundaries" rounds booking the B1
meter module doc delegates to the composition.

**Terminus design note (both behaviors pinned, neither vacuously)**:
exhaustion fires COMPOSED; diagnosis CANNOT — the REAL diff comparator's
residual docs carry EMPTY unit attribution, so the recurring-residual
detector never sees an attributed recurrence through the composed path
(recorded residual below). The diagnosis terminus itself is pinned
in-crate: `control::diagnosis` tests (AK-039), run by the Task-12 gate.

## Item 3 — Teeth T1–T6 (`.agents/reports/task12/teeth.md`)

All six run personally: break → named test RED → snapshot-cmp byte-identical
revert → GREEN. Highlights:
- T1 (meter reserve refusal neutered, both arms): p6 2/2 FAIL — with the
  meter neutered the run attempted the reserve round and died LOUDLY at
  the loop budget's own domain arithmetic (`RecoveryReserveBreached`
  Rounds, ceiling 5 spent 5 requested 1): spend cannot SILENTLY pass the
  reserve. Defense in depth, not an accident.
- T2 (boundary `verify_unchanged` deleted): p4 mid-run-edit FAIL,
  Escalated ≠ Aborted.
- T3 (per-tick re-read cached once): p4 mid-run-edit FAIL, cancelled
  0 ≠ 1 — the boundary abort alone does not substitute for the IN-FLIGHT
  cancellation.
- T4 (reserve no-op pass-through): p6 2/2 FAIL — the sixth UNRESERVED
  dispatch proceeded to the loop-budget refusal.
- T5 (`record_correction_selection` deleted): p4 main FAIL, read-back
  0 ≠ 1.
- T6 (observer expired-deadline refusal removed): observe-command package
  FAIL on `an_expired_deadline_refuses_without_spawning` (sentinel proves
  the instrument ran); 9/9 green after revert.

## Item 4 — Gates (final state, all run personally after the last edit)

```
cargo nextest run --test p4_corrective_loop --test p6_budget_control   → 5/5 PASS
cargo nextest run -p gauntlet-app control::                            → 54/54 PASS
just check-package gauntlet-cli                                        → green
just test-package gauntlet-cli                                         → 41/41
just check                                                             → green
just test                                                              → 886/886
just lint                                                              → green (clippy -D warnings + fmt)
just deny                                                              → green
just boundaries                                                        → 197/197
just ruler                                                             → PASS
```

The first `just test` run was RED (10 literals-scan violations in the two
proof files — the predecessor had never run that gate on p4); the
gate-demanded repairs are recorded in the Item 4 checkpoint entry and
changed no behavioral assertion.

## Real locator spellings (for the orchestrator's reference-candidates)

CLI test targets (cargo test-target names, per the plan gate spelling):
- `gauntlet-cli::p4_corrective_loop` — tests:
  `an_injected_defect_is_exposed_compared_and_closed_by_a_routed_correction`,
  `a_mid_run_answer_key_edit_cancels_the_in_flight_attempt_and_aborts`,
  `correction_selection_durably_carries_rejected_alternatives_with_reasons`
- `gauntlet-cli::p6_budget_control` — tests:
  `a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact`,
  `the_meter_refuses_the_reserve_consuming_round_and_books_nothing`

In-crate `control::` names covering the frozen `tests/contract/`-shaped
locators:
- AK-037 `controller::records_alternatives` →
  `gauntlet-app control::select::tests::controller_records_alternatives`
  (beside the composed p4 read-back above).
- AK-039 `diagnosis::nonconvergence` →
  `gauntlet-app control::diagnosis::…::nonconvergence_triggers_on_the_nth_occurrence_not_before`
  and `…::budget_exhaustion_yields_inconclusive_quarantine_and_escalation`.
- AK-052 `budget::reserve_protected` → composed in
  `gauntlet-cli::p6_budget_control` (both tests; the rows-vs-reservations
  count and the refusal); in-crate support:
  `gauntlet-app control::controller::tests::budget_exhaustion_drives_the_terminal_state_with_the_reserve_intact`
  and the meter adapter's own suite
  (`gauntlet-adapter-meter-memory tests::the_metered_budget_passes_the_shared_contract_suite`,
  `…::reserve_agrees_with_would_breach_reserve_at_every_boundary`).

## Deviations (documented, with reasons)

1. **Proof-local facet_evidence@v1 rung (both proofs; predecessor finding
   1)**: the B3 controller writes the FIRST observation facet's
   `facet_evidence@v1` bytes at the attempted unit's evidence locator, and
   no Task-11 verifier rung's declared artifact contract is
   `facet_evidence@v1` (test-report = tests/failures JSON; command-exit =
   two-line transcript; file-digest = sha manifest; schema-check =
   required-keys JSON). Each proof carries ONE deterministic rung at the
   §14.2 focused-test position that decodes the facet document and decides
   by its RECORDED EXIT STATUS (zero ⇒ PASS, nonzero ⇒ FAIL; absent bytes
   are the typed missing-evidence error; undecodable bytes the typed
   instrument failure).
2. **Proof-local scripted AgentRunner (p4 only; predecessor finding 2)**:
   one real `MockAgentRunner` instance replays ONE scenario for EVERY
   admitted invocation — the fail-then-fix shape (defective artifact on
   invocation 1, corrected on invocation 2) is unrepresentable with the
   real mock. The scripted runner follows the mock's own discipline
   (strict `read_result_contract` identity check; effects under the
   dispatch's workspace ref against the one canonical root). p6's
   never-fixing task uses the REAL mock + the REQUIRED lying-builder
   fixture.
3. **Composition-root meter rounds supplement (p6; predecessor finding
   3)**: see Item 2. The REAL adapter books zero rounds per charge
   (port-pinned by the shared suite), so without the supplement the
   controller's rounds gate is structurally dead under the real adapter.

## Residuals / integration findings for the orchestrator and critics

1. **Controller locator-write vs rung artifact contracts (integration
   finding, predecessor finding 1)**: the controller's evidence owner
   writes `facet_evidence@v1` bytes at the required locator, but no
   shipped verifier rung decides that contract — every composed proof
   must carry a proof-local rung. A rung over `facet_evidence@v1` (or a
   controller write shaped to an existing rung's contract) belongs in a
   candidate revision.
2. **The rounds dimension cannot move through the shipped meter
   composition (predecessor finding 3)**: no shipped composition books
   iteration-boundary rounds, so the controller's `remaining().rounds`
   gate is dead under the bare REAL adapter; p6 wires the documented
   supplement. The B3 in-crate exhaustion test passed only because its
   FakeMeter booked rounds on charge — semantics the shared suite forbids.
   The supplement (or a port revision) should be promoted.
3. **Diff-comparator attribution gap**: the REAL diff comparator's
   residual docs carry EMPTY `unit_refs`, so composed non-convergence
   detection (§11.5 row 9 → diagnosis, AK-039) cannot fire through it —
   p6 documents exhaustion-fires/diagnosis-pinned-in-crate. Attribution
   plumbing from residuals to units is a real gap in the composed path.
4. **The drive loop has no exit for "nothing open, nothing in flight, no
   terminal"** (found during teeth T3; a degenerate reread break hangs
   forever). Latent robustness defect: a future reread/store failure mode
   that yields zero units turns into a silent spin instead of a typed
   error.
5. **`MockAgentRunner` is per-run, not per-invocation** (predecessor
   finding 2, structural): scripted fail-then-fix scenarios are
   unrepresentable; noted for any future mock revision.
6. **Pinned-record dedup**: byte-identical §16.1 refusal records collapse
   to one content-addressed object (p6 documents in-test); per-iteration
   recurrence evidence lives in the distinct observation bundles. Not a
   defect — a consequence worth knowing when counting pinned records.
7. **The ms→seconds threshold rounding in the controller's stall window**
   (`stall_timeout_ms().div_ceil(1000)`, second-granularity clock): a
   sub-second quiet window becomes a 1-second threshold that the first
   1-second tick meets immediately (found while re-deriving fixture
   figures). Worth a named bound or a documented minimum.

## Boundaries respected

No commits. No frozen/operator files touched (`.wayfinder/**`,
`.agents/specs/**`, `.agents/plans/**`, `AGENTS.md`, `justfile`,
`deny.toml`, `.agents/CURRENT_STATUS.md`, prereg files, existing task12
recon/adjudication/brief/report files, `.agents/prompts/**`, `.tmp/**`,
`.jolli/**`). All temporary instrumentation (two controller eprintlns
across the session) reverted byte-identical (untracked files — manual
inverse, cmp-verified against pre-break snapshots). No settlement commit
wired (Task 13); no weakened assertions — the one literal repaired in p4
was a fixture spelling bug (SCREAMING_SNAKE), fixed to the correct token.
