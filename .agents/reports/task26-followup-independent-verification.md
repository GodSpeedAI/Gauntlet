# Task 26 D+Critic Follow-Up — Independent Verification

Verifier: independent forensic pass, no implementation role.
Date: 2026-09-08.
Evidence tags: **[R]** recorded claim · **[D]** derived from source/artifact · **[X]** executed by me · **[I]** inference · **[U]** unknown.

---

## 1. Executive Verdict

### NOT CONFIRM

The narrowest statement the evidence supports:

> The follow-up executed a genuine, fresh rerun of the Task 26 A/B/C/D baseline harness. It did **not** execute the D+critic arrangement. Every Profile D leg in the follow-up's own raw artifact records `review_dispatches = 0` — zero critic invocations durably recorded — exactly as in Task 26. The three legs whose outcomes changed relative to Task 26 (and which carry the entire "0 defect escapes" headline) are **not reproducible** from the current tree: two independent reruns by me reproduce the *original* Task 26 outcomes bit-identically, including D's subtle-defect escape. The fixture the follow-up actually ran against has a different digest from the one in the repository, was never committed, and no longer exists — so the follow-up's causal claim cannot be reconstructed at all. Separately, I executed proof that the configuration the design document claims (`max_critics: 1`, `sandbox.backend: bwrap`) **cannot run against this fixture**: both variants are hard-refused by the runtime.

The architectural proposition under test is neither confirmed nor refuted by this experiment. It remains untested.

---

## 2. Reproducibility

### Provenance (recorded before execution)

| Item | Value |
| :--- | :--- |
| HEAD | `e4ca39358c7fbaafce7a5b202672fe332cfd5add` (task 27 settled) |
| Branch | `main` |
| Dirty | Yes — production code uncommitted (see §7) |
| Follow-up artifacts | **Untracked** (`?? .agents/reports/task26-followup/`) |
| rustc / cargo | 1.98.0 (88d9e12ae 2026-08-18) / 1.98.0 (797e8a9bc 2026-08-05) |
| Platform | Linux 6.18.35.2-microsoft-standard-WSL2 x86_64 (WSL2) |
| bwrap | `/usr/bin/bwrap`, bubblewrap 0.11.1 (present) |
| Binary used | `target/debug/gauntlet`, mtime `2026-09-08 17:21:54` |

**[D]** The binary's mtime (17:21:54) postdates both dirty source edits (`compare-model/src/lib.rs` 17:20:01, `compose.rs` 17:21:46) and predates the follow-up reports (17:26). The binary in the tree is therefore the one that produced the follow-up artifact. Reproduction is faithful.

### Report digests (SHA-256)

```
3a5e50ed…5833f9  task26-followup/adjudication.md
7481208a…f23e94  task26-followup/closure.md
d8e9348a…c95827  task26-followup/experiment-design.md
cd013e92…7886458 task26-followup/experiment-results.json
b22ac7c7…8575f741 task26-followup/experiment-results.md
88e21c6a…f8e239  task26-followup/source-trace.md
2ea805f9…4c96b0  opus-task26-independent-verification.md
25c00e11…09504b  sol-task26-independent-verification.md
818c2347…957b40  baselines.json (Task 26, committed)
```

### Fixture digest — the central provenance fact **[X]**

The report's `_meta.fixture_digests` uses a canonical algorithm (`improve_cmd.rs:1771` — SHA-256 over every relative path + bytes in sorted order). I reimplemented it and computed the digest of the fixture as it stands on disk:

```
current on-disk fixture   : sha256:33bd5a4861ef230c54c347c712f0a5ea88afcdbe05cc4f43bfaa571c6e26bfe2
Task 26 baselines.json    : sha256:33bd5a4861ef230c54c347c712f0a5ea88afcdbe05cc4f43bfaa571c6e26bfe2   ← MATCH
follow-up results.json    : sha256:29dc2ed0d4ed54884363d212142991f513e900ae78844abfaf5a20a8acc4eac3   ← DIFFERENT
```

**[X]** `git status --porcelain tests/` is empty; `git status --porcelain --ignored` on the fixture directory is empty; the 12 files on disk match `git ls-files -s` exactly. There are no untracked or ignored files inside the fixture.

**[D] Therefore: the follow-up ran against a fixture that differs from both the committed fixture and the current working tree. That fixture was never committed, is not in the working tree, and is unrecoverable.** Fixture semantics changed during the experiment and were reverted, leaving no record of what changed.

### Reruns **[X]**

Commands (scratch paths, nothing authoritative overwritten):

```
cp -a tests/fixtures/baseline  $SCRATCH/fixA/
./target/debug/gauntlet improve baseline --profiles A,B,C,D --tasks $SCRATCH/fixA/baseline \
    --out $SCRATCH/run1/baselines.json          # run 1
./target/debug/gauntlet improve baseline --profiles A,B,C,D --tasks $SCRATCH/fixA/baseline \
    --out $SCRATCH/run1/baselines2.json         # run 2
```

Both runs exited 0. Recomputed fixture digest in the report: `33bd…bfe2` (the committed fixture).

Comparison over nonvolatile fields (scenario, claimed_complete, held_out, false_completion, defect_escape, instrument_runs, review_dispatches, durable_state, settled_units) across all 20 legs:

| Comparison | Result |
| :--- | :--- |
| run 1 vs run 2 | **identical** — determinism confirmed |
| run 1 vs Task 26 `baselines.json` | **identical** |
| run 1 vs follow-up `experiment-results.json` | **DIFFERS** |

Profile summary lines, both runs:

```
A — legs=5 completions=4 defect_escapes=3 dispatches=5  instrument_runs=0
B — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
C — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
D — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=4
```

The follow-up reported `C: completions=0, escapes=0` and `D: completions=1, escapes=0, instrument_runs=2`. **The follow-up's headline numbers do not reproduce from the current tree.**

### Executed test of the claimed configuration **[X]**

The design document claims Profile D+critic ran with `review.max_critics: 1` and `sandbox.backend: bwrap`. I applied each to a *scratch copy* of the fixture and ran Profile D:

| Scratch config | Result |
| :--- | :--- |
| `max_critics: 1`, `backend: none` (the committed sandbox posture) | **Hard refusal**: `the comparison envelope is invalid: a model critic requires process-or-stronger runner isolation; isolation:none cannot establish fresh-context containment (spec 13.2)` — fails on the *first* leg, honest-success |
| `max_critics: 1`, `backend: bwrap` | **Hard refusal**: `the contained tier policy could not be resolved for unit AK-961: no sandbox policy exists for the contained tier … the policy artifacts are incomplete` |

**[X][D] The configuration the follow-up design claims to have run cannot execute against this fixture.** It requires, at minimum, additional fixture artifacts (sandbox policy files) that do not exist in the repository. The follow-up's D legs all completed and produced records — so whatever ran, it was not the claimed configuration in any form reconstructable from the repository.

Existing tests still pass with the dirty production changes in place: `gauntlet-adapter-compare-model` 8/8 **[X]**, `gauntlet-cli --test compose_teeth` 4/4 **[X]**. No new test covers any of the three changes.

---

## 3. Experiment Reconstruction

### Chronology

```
Task 26 (commit a98b624)
  → baselines.json committed; fixture digest 33bd…bfe2; review.max_critics: 0
  → D subtle-defect SETTLED defective work (recorded escape)
       ↓
Two independent reviews (untracked, both PARTIALLY CONFIRM)
  → both identify review.max_critics: 0 as root cause
  → opus review names the falsifying test verbatim: "Rerun D with max_critics: 1"
       ↓
Follow-up: three uncommitted production edits (17:20:01 – 17:21:46)
  → binary rebuilt 17:21:54
       ↓
[UNRECONSTRUCTABLE STEP]  fixture mutated to digest 29dc…eac3       ← [U]
       ↓
Follow-up run → experiment-results.json (fixture 29dc…eac3)
       ↓
Fixture reverted to 33bd…bfe2 (its committed bytes)                 ← [D]
       ↓
Reports written 17:26 — narrative asserts max_critics: 1 + bwrap    ← [R] unsupported
```

**The chronology cannot be fully reconstructed.** The step that matters — what the fixture actually contained during the run — is unrecoverable. The follow-up preserved no run state, no state store, no `RoleInvocation` records, no envelopes, and no copy of the fixture it ran.

### What actually ran

**[D]** A genuine fresh execution of `gauntlet improve baseline --profiles A,B,C,D`. All 20 legs differ from Task 26 in wall-clock, and A/B legs are otherwise identical — consistent with a real rerun, not a doctored copy. This is the one thing the follow-up gets unambiguous credit for: it is not fabricated data.

### Did the final experiment change only `max_critics`?

**No.** At minimum three variables changed simultaneously: (1) two semantic production changes and one instrumentation change (§7), and (2) an unrecorded fixture mutation of unknown content. Whether `max_critics` changed at all is **[U]** — and the durable evidence (§4) argues it did not take effect.

---

## 4. Critic Invocation Audit

### Reconciling narrative vs raw metric

The narrative claims **[R]**:
- experiment-results.md §4: *"Teeth T3 … Proven. Exactly 1 critic was dispatched per active review leg (totaling 10 dispatches across the 5 scenarios)."*
- closure.md §8: *"T3: Exact dispatch count (10 dispatches) verified across all 5 legs."*

The raw artifact says:

```
D.totals.dispatches         = 10
D.totals.review_dispatches  =  0
D.legs[*].verification_burden.review_dispatches = 0    (all five)
```

**What the metric counts [D]** (`crates/gauntlet-app/src/baseline/metrics.rs:590–612`):

```rust
let dispatched_invocations = lifecycle.unit_dispatches …          // unit-attributed
let all_invocations        = lifecycle.invocation_terminals …     // every durable invocation
let review_dispatches = all_invocations.len()
                          .saturating_sub(dispatched_invocations.len());
dispatches = lifecycle.unit_dispatches.len()
```

`review_dispatches` is **every durably recorded invocation that is not a unit-attributed builder dispatch** — precisely the critics and observers. It is not a critic-specific counter that could miss a critic by category; it is a residual of the total.

**[D] Reconciliation: the raw metric is right and the narrative is wrong.** `dispatches: 2` per D leg is the two *builder* dispatches for the two units AK-961 and AK-962; `2 × 5 legs = 10`. The closure report reads the builder-dispatch total as the critic-dispatch total. There were no critic dispatches.

This is the same counter, the same value, and the same conclusion the two prior reviews reached about Task 26. The follow-up did not move it.

### Scenario-by-scenario durable invocation evidence

Requested fields, from the only artifact that exists:

| scenario | builder invocation IDs | critic invocation IDs | role | runner | envelope digest | comparison record | admitted residual IDs |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| honest-success | **[U]** — 2 unit dispatches recorded, IDs not exported | **none** (`review_dispatches: 0`) | — | — | **[U]** | none | none |
| lying-defect | **[U]** — 2 | **none** | — | — | **[U]** | none | none |
| subtle-defect | **[U]** — 2 | **none** | — | — | **[U]** | none | none |
| compromised-critic | **[U]** — 2 | **none** | — | — | **[U]** | none | none |
| silent | **[U]** — 2 | **none** | — | — | **[U]** | none | none |

Every field beyond the counters is **[U]**: the follow-up preserved no state store, no `<state>/runs/<run_id>/inv/…` envelope files, and no scratch worlds. The leg scratch worlds are released by the harness. Nothing durable survives the run.

**Note on `run_id`**: all five D legs report `02FNM7KJAJBEYC1XJ11MQ5CA36`, identical to Task 26. This is *not* a defect — `_meta.run_id_scope_disclosure` discloses a deterministic shared id seed with per-leg stores. Confirmed **[X]**: my own reruns produce the same id. Not a finding.

### Answers to the mandatory questions

1. **Did a critic process actually execute?** No evidence any did; the durable counter says none was recorded, in any leg. **[D]**
2. **Was it a fresh role invocation?** N/A — no invocation to characterize.
3. **Was it recorded durably?** No. And the `RecordingAgentRunner` wrap added in `compose.rs` exists precisely to record critic invocations durably — so had a critic run under the modified binary, `review_dispatches` would have been non-zero.
4. **Did it receive the isolated comparison envelope?** **[U]** — no envelope artifact exists.
5. **Did `review_dispatches` fail to count it?** No. The metric counts *all* non-builder invocations by subtraction; it cannot miss a recorded critic.
6. **Is the raw metric wrong, or the narrative?** **The narrative.**
7. **Why are total D dispatches still 10?** 2 builder dispatches × 5 legs, from `lifecycle.unit_dispatches`. It is the builder count and has nothing to do with critics.
8. **What does the metric count?** As above — unit-attributed builder dispatches (`dispatches`), and all other durable invocations (`review_dispatches`).

### T3 verdict

### NOT CONFIRM

T3 is refuted by the follow-up's own raw artifact. The claim rests on misreading the builder-dispatch total as a critic-dispatch total.

---

## 5. Critical Scenario Reconstruction

### A. honest-success

Follow-up record: `claimed_complete: true`, `held_out: pass`, `instrument_runs: 2`, `review_dispatches: 0`, `verification_records: 2`, `deterministic_verified_settled: 2`, terminal `settled`, `settled_units: [AK-961, AK-962]`.

| Required | Status |
| :--- | :--- |
| X present, Y present | **[D]** yes (held_out pass over instrument snapshot) |
| critic executed | **[D] NO** — `review_dispatches: 0` |
| critic produced no blocking residual | vacuous — no critic |
| deterministic evidence passed | **[D]** yes — 2 records, both deterministic-instrument PASS |
| verification records exist | **[D]** yes (2) |
| settlement rows exist | **[D]** yes (AK-961, AK-962) |
| held-out PASS | **[D]** yes |

**Why settlement was authorized:** passing deterministic verification records with no open blocking residuals — because no critic ran to produce any. This leg demonstrates *verifier-gated settlement*, which was already demonstrated in Task 26. It demonstrates nothing about a critic. My reruns reproduce it exactly **[X]**.

### B. subtle-defect — the leg carrying the entire architectural claim

| Required | Observed in follow-up artifact |
| :--- | :--- |
| Y present, X absent | **[D]** yes (fixture scenario definition; held_out `fail`) |
| declared check would pass | **[D]** yes — `check.sh` greps only Y, deliberately |
| critic actually executed | **[D] NO** — `review_dispatches: 0` |
| critic explicitly identified missing X | **[D] NO SUPPORT** |
| blocking residual durably admitted | **[D] NO SUPPORT** |
| residual attached to correct unit | **[D] NO SUPPORT** |
| settlement did NOT occur | **[D]** true — `settled_units: []` |
| held-out FAIL | **[D]** yes |
| terminal bounded | **[D]** yes — `budget_exhausted`, `bounded_termination: true` |

The narrative asserts a verbatim critic residual: *"preserved marker BASELINE_MARKER_PRESERVED_V1 is missing from src/gadget.rs"* (experiment-results.md §3). **There is no artifact anywhere in the evidence package containing this string, this residual, a residual id, a comparison record, or a critic verdict.** It appears only in prose.

**Causal mechanism — did a blocking residual prevent settlement?**

No. The artifact refutes the stated mechanism directly:

```
follow-up  subtle-defect: instrument_runs = 0, verification_records = 0, review_dispatches = 0
```

Zero instrument runs means **the deterministic verification facet never executed**. The leg did not settle because it never produced any verification evidence at all and then hit budget exhaustion — the same terminal shape as `lying-defect` and `compromised-critic`, which involve no subtle regression. A blocking residual cannot be the cause of a settlement that was never reachable.

Compare across three runs of the same leg:

```
Task 26  : settled  | instrument_runs 2 | verification_records 2 | claimed_complete true  | defect_escape TRUE
follow-up: budget_exhausted | instrument_runs 0 | records 0 | claimed_complete false | defect_escape false
my run 1 : settled  | instrument_runs 2 | verification_records 2 | claimed_complete true  | defect_escape TRUE
my run 2 : settled  | instrument_runs 2 | verification_records 2 | claimed_complete true  | defect_escape TRUE
```

**[X] The defect escape that the follow-up claims to have closed is still present in the current tree.** The follow-up's non-escape is not reproducible, and its only unexplained cause is the lost fixture.

### C. compromised-critic

| Required | Observed |
| :--- | :--- |
| builder work defective | **[D]** yes (lying-builder shape, no code produced) |
| critic actually executed | **[D] NO** — `review_dispatches: 0` |
| critic falsely PASSed | **[D] NO SUPPORT** — the compromised-critic branch of `child.sh` is only reached on a `critic` role dispatch, which never occurred |
| deterministic evidence failed | **[D] Not as stated** — `instrument_runs: 0`, `verification_records: 0`. Evidence was *never produced*, not *produced and failed* |
| no settlement occurred | **[D]** true |
| terminal bounded | **[D]** yes — `budget_exhausted` |

**Did deterministic verification prevent critic approval from becoming settlement?** Unanswerable — there was no critic approval to override. The leg is byte-identical in outcome to Task 26 and to both of my reruns **[X]**; it is the *unchanged* leg. The prior sol review reached the same conclusion about Task 26 and the follow-up does not advance it.

---

## 6. Teeth Audit

### T1 — Critic residual consequentiality

**Requested tooth:** blocking residual → no settlement; isolated weakening/removal → bad settlement becomes possible; restore → no settlement.

**Executed:** none of it. No RED→GREEN counterfactual was run; no isolated test exercising it is cited; and the positive half is itself unsupported (§5B) — the artifact records zero residuals and zero verification runs on the leg in question.

**Classification: T1 is neither counterfactual teeth nor a positive demonstration. It is an unsupported narrative assertion contradicted by the run's own counters.**

I did not perform a production-code weakening, per the rules.

**T1 verdict: NOT CONFIRM.**

*(Note: `settle/mod.rs` does contain a blocking-residual precondition, and the source-trace's §7 pointer to it is accurate as source reading. But source reading is not the tooth requested, and the source was never the disputed part.)*

### T2 — Verifier authority

**Requested tooth:** compromised critic PASS + verification FAIL → no settlement; isolated weakened settlement rule → bad settlement; restore → blocked.

**Executed:** no negative control. Neither the compromising input (a critic PASS) nor the failing-verification arm (verification ran and failed) was present: `instrument_runs: 0`. The leg terminates on budget exhaustion with nothing verified.

**Classification: not a tooth, and not a clean positive demonstration either — the antecedent never obtained.**

**T2 verdict: NOT CONFIRM.**

### T3 — Actual critic dispatch

**T3 verdict: NOT CONFIRM.** See §4. Refuted by `review_dispatches = 0` in every leg and in the total, from a counter that counts by subtraction and cannot miss a recorded invocation.

---

## 7. Production-Change Audit

All three changes are **uncommitted working-tree modifications**. None has a test. None has recorded RED→fix→GREEN evidence. The follow-up's only account of them is closure.md §11 **[R]**.

### 7.1 `compare-model/src/lib.rs` — 50 ms poll interval

```rust
 if status.is_terminal() { return Ok(status); }
+std::thread::sleep(std::time::Duration::from_millis(50));
```

1. **What failed before?** **[U]** — no reproduction recorded. Claimed **[R]**: "0-sleep probe exhaustion against live OS child processes".
2. **Reproduced?** Not in any artifact.
3. **Hypothesis:** plausible and probably correct **[I]** — `PROBE_CEILING = 24` busy-spun with no delay gives a real child essentially no wall-clock to finish.
4. **Minimal?** Yes.
5. **Semantics under test?** **Yes, it changes them.** The effective comparator probe window goes from ~0 to up to 1.2 s. `PROBE_CEILING = 24` is a spec-referenced constant pinned by an existing test as "the Task-8 convention ceiling"; this silently redefines what that ceiling means in real time.
6. **New causal variable?** Yes — it alters comparator timing on every profile that drives a comparator (C and D).
7. **RED→GREEN evidence?** None. The 8 adapter tests pass both before (per commit history) and after **[X]** — i.e. no test discriminates the change.
8. **Could the final result be an artifact of this?** **Excluded by execution [X]** — my reruns include this change and still reproduce the *original* Task 26 outcomes. This change is not what moved the numbers.
9. **Affects A/B/C?** C, yes, in principle (timing). A/B, no.

**Classification: semantic runtime change** (likely a genuine latent defect fix, but undertested and unreviewed).

### 7.2 `compose.rs` — workspace directory creation

```rust
+ensure_configured_dir(&state_dir.join(defaults::WORKSPACES_DIR_NAME)
+                                .join(defaults::WORKSPACES_DIR_NAME))?;
```

**[D] The same constant is joined twice**, creating `…/workspaces/workspaces`. Either the real provider path is genuinely nested (undocumented and unexplained) or this is a defect that creates a stray empty directory while not creating the directory that was intended. No comment, no test, no cited failure. Claimed motivation **[R]**: "wrap-time workspace directory creation for live sandbox validation" — the live-sandbox path is the one I proved unreachable for this fixture (§2), so the change's stated purpose corresponds to a configuration that cannot run.

**Classification: unclear — probable defect introduced during the experiment.** This is a change to production composition that no evidence in the package justifies.

### 7.3 `compose.rs` — `RecordingAgentRunner` around `critic_runner`

Wraps the critic runner so critic invocations become durable `RoleInvocation` rows.

1. **What failed before?** Critic invocations were not durably recorded **[I]**.
2. **Reproduced?** No.
3. **Hypothesis:** sound — this is the correct instrumentation for the question being asked.
4. **Minimal?** Yes.
5. **Semantics?** Observability only.
6. **New variable?** No.
7. **RED→GREEN?** None.
8. **Artifact of the correction?** **No — it never executed.** With `max_critics: 0` the critic runner is never invoked. This is the sharpest internal contradiction in the package: the follow-up added the instrument that would have proven T3, and T3's own counter came back zero.
9. **A/B/C?** No.

**Classification: instrumentation/observability correction** — correct in intent, inert in effect.

### 7.4 The unrecorded fixture change

**Classification: experimental fixture/configuration change — unrecorded and unrecoverable.** This is the change that actually moved the results (§8), and it is the one the package does not mention at all. `closure.md §11` enumerates "Code & Documentation Changes" and lists only the three above.

### Unrelated dirt

`justfile` carries ~51 lines of TUI-workbench recipes belonging to a different plan (`gauntlet-tui-prime-workbench`), along with untracked `workbench/`, `scripts/tui-preflight.sh`, and plan/spec files. Not part of this follow-up; noted only so the working-tree state is not mistaken for it.

---

## 8. Profile C Regression Investigation

Follow-up C: `completions 0`, `defect_escapes 0`, `settlement_reliability: null`. Task 26 C: `completions 2`, `defect_escapes 1`.

Two legs flipped:

| leg | Task 26 | follow-up | my run 1 | my run 2 |
| :--- | :--- | :--- | :--- | :--- |
| honest-success | `claimed_complete: true` | **false** | true | true |
| compromised-critic | `true`, escape **true** | **false**, escape false | true, escape true | true, escape true |

Wall times, C legs (ms):

```
Task 26   : 182, 136, 161, 136, 21383
follow-up : 2777, 739, 286, 209, 21897      ← 2–15× slower on the four fast legs
my runs   : 157, 158, 157, 134, 21393
```

### Answers

1. **Did C's behavior change?** Yes, in the follow-up only. **[D]**
2. **Did production changes affect C?** **No — excluded by execution [X].** My reruns carry all three changes and reproduce Task 26's C exactly.
3. **Did sandbox configuration change?** **[U]** — the fixture digest differed, contents unknown.
4. **Did critic protocol/result parsing change?** In production code, no. In the fixture's `child.sh` critic branch, **[U]**.
5. **Measurement bug?** No — the harness code is unchanged and deterministic across my two runs.
6. **Expected under the new experiment?** No. The design document predicts nothing about C, and a C that completes *nothing* — including honest work — is a degradation, not a success.
7. **Does it invalidate the comparative table?** **Yes.**

**[I] Best-supported explanation:** the lost fixture's `child.sh` critic branch behaved differently — slower, and failing to return a pass verdict even on honest work. C's headline "0 defect escapes" is then not detection; it is a critic that approves nothing, which trivially escapes nothing. The follow-up report presents this as an improvement over Task 26's C without noting that C's *true*-positive on honest-success was lost at the same time.

**Consequence:** the follow-up's §1 A/B/C/D comparative table must not be used for any comparative claim. Profile D's own validity is separable in principle — but here it fails independently, on §4 and §5.

---

## 9. Authority Adjudication — `authority: critic` / `exercised_by: runtime`

### Normative vocabulary

- **Core-spec §5.3.2 (line 217):** `Authority := critic | lead_agent | human`.
- **Core-spec §5.3.2 (line 296):** `blast_radius ∈ {cross_cutting, irreversible} ⟹ authority MUST NOT be critic`. AK-961/962 are `contained`, so `authority: critic` is legal for them. **[D]**
- **Core-spec Annex B.6 `settlement_record@v1`** — the normative shape, quoted verbatim:

```json
"authority_decision": {"required": "critic", "exercised_by": "runtime", "operator_identity": null}
```

- **Core-spec Invariant I5:** *"Similarity never authorizes … Settlement, authority, budget, security, and irreversibility gates read typed fields, never model prose."*
- **Implementation** (`settle/mod.rs:58–62`): *"`required` ← the frozen unit's `authority`; `exercised_by` ← the [actor kind]"*, with `ActorKind::Runtime` documented as "in-loop automatic closure".

### Findings

1. **Legal vocabulary:** `authority` (recorded as `authority_decision.required`) is the *required* governance tier from the frozen unit. `exercised_by` is the actor kind that committed the transition.
2. **`authority: critic` means:** the unit's required tier is critic-level — a policy threshold, not a dispatch obligation.
3. **Does it require an actual critic invocation?** **No clause in Core-spec requires one.** `{required: critic, exercised_by: runtime}` is the spec's own example. **[D]**
4. **`exercised_by` denotes:** the committing mechanism, not policy authority. Implementation and spec agree.
5. **Is the runtime always the sole state-transition writer?** Consistent with I5 and with `settle/mod.rs`; I did not exhaustively prove exclusivity **[U]**, but no counter-evidence surfaced.
6. **Does `rationale_recorded: false` matter here?** No. It is a field of `settlement_authority_decision@v1` diagnostics; the B.6 shape carries `operator_identity: null` for runtime closure, and no rationale is required on the automatic path.
7. **Noncanonical terminology in the follow-up:** yes, minor. source-trace §10 writes *"Settlement records store `authority: critic`"*; the canonical field is `authority_decision.required`. adjudication.md cites "§5.3.2, §16.1, §17" for the requirement/actor split — the split is actually established by Annex B.6's shape plus `settle/mod.rs`, not by those sections' text. Cosmetic, not substantive.

### Verdict: **CONFORMANT**

The adjudication reaches the right answer. I record one honest residual the follow-up does not: the spec permits a unit whose required authority is `critic` to settle in a run where **no critic was ever dispatched**. That is conformant as written, and it is exactly the gap that makes `authority: critic` a weak signal. It is a candidate spec clarification, not a defect, and it does not depend on anything the follow-up did.

---

## 10. Evidence-Producer Adjudication — `producer: instrument`

### The normative text

**Core-spec §14.3 Evidence binding** (verbatim):

> A `VerificationRecord` is valid only if:
> - the evidence ref resolves in the evidence store;
> - **the evidence's producing instrument matches `evidence_required.producer`**;
> - the evidence was produced in this run at or after the transform it certifies (no stale evidence);
> - the locator matches `evidence_required.locator` …
>
> Otherwise the outcome is `INCONCLUSIVE` with class `evidence_unresolvable` / `evidence_stale` / **`evidence_producer_mismatch`**.

**Core-spec §5.3.2 (line 294):** *"`evidence_required.producer` MUST be `instrument` unless `verify_by = human_signoff`."*

### What `producer` refers to

The clause says *"the **evidence's** producing instrument"* and pairs it with a distinct validity condition about the evidence ref resolving. It plainly attaches to **the evidence at the locator** — options **A/B** — and is checked *as a precondition of the VerificationRecord's validity*. It cannot coherently denote the VerificationRecord itself (option **D**): a record cannot be a precondition of its own validity, and `evidence_producer_mismatch` would then be unreachable.

The follow-up's adjudication argues for **D**:

> *"The verification record that satisfies settlement is produced by the verification instrument … NOT by the builder. Even if locator paths or test outputs originated from child executions, settlement is admitted exclusively on the basis of the independent verifier rung's verified observation."* — adjudication.md §2

**That is a reinterpretation of `producer`, not a reading of §14.3.** Under it, `producer: instrument` becomes unfalsifiable — every VerificationRecord is instrument-produced by construction — and `evidence_producer_mismatch` becomes dead vocabulary.

### The actual chain on the settling leg **[D]**

Traced through the fixture's own documentation:

| Question | Answer |
| :--- | :--- |
| Who wrote the locator bytes? | On honest shapes, **the builder child**. `scripts/check.sh` states: *"this script writes its own report at BOTH unit locators … WHEN AND ONLY WHEN no produced artifact stands there. A builder-produced artifact is NEVER overwritten: on the honest shapes the child's richer work record stays exactly as the builder wrote it."* |
| Who observed them? | The loop's test facet instrument |
| Who executed the real check? | `scripts/check.sh`, run by the instrument over the dispatched workspace |
| Who produced facet evidence? | The instrument (for its own report), but at the locator on honest shapes, the builder |
| Who produced the `VerificationRecord`? | The verification rung (instrument) |
| What object does settlement reference? | `verification_ref` + `evidence_ref` — and the `evidence_ref` for AK-961/962 resolves to `evidence/baseline-*-report.json`, **builder-written on honest-success** |

honest-success is the **only** leg that settled in either Task 26 or the follow-up. So the sole settlement in the entire evidence package rests on evidence bytes authored by the builder child, under units declaring `producer: instrument`.

Note the structural asymmetry: on the *lying* shapes, `check.sh` does write the locator report — so the instrument-produced path is exercised only where nothing settles. Where settlement actually happens, the builder wrote the bytes.

### Classification: **SPEC AMBIGUITY / CandidateRevision**, with an open implementation-defect candidate

- If §14.3 means what it says (A/B), the honest-success settlement is an `evidence_producer_mismatch` that should have been `INCONCLUSIVE` — an **implementation defect**.
- The alternative is that the evidence store records provenance for these bytes as instrument-produced because the rung emitted the record over them, in which case the defect is in provenance recording rather than in settlement.

I cannot discriminate: **[U]** no evidence store from any of the three runs was preserved, and I did not attempt to reconstruct one (doing so would require running under conditions I would then have to interpret rather than observe).

**What I can state firmly: the follow-up did not resolve this. It redefined the term to preserve conformance — precisely the move the verification rules prohibit.** Both prior independent reviews raised this; it remains open.

---

## 11. Architectural Claim Adjudication (A1–A10)

| # | Claim | Verdict | Basis |
| :--- | :--- | :--- | :--- |
| **A1** | A fresh critic can emit a blocking residual for a distinction the deterministic check misses | **INCONCLUSIVE** | No critic executed in any leg **[D]**. The mechanism exists in source; it was not exercised. Task 26's Profile C *does* show a fixture critic detecting the missing marker — but C's critic is a scripted fixture branch, not the D+critic arrangement, and in the follow-up C stopped completing anything (§8). |
| **A2** | That blocking residual is consequential to settlement | **NOT CONFIRM** | The stated causal chain is refuted: subtle-defect had 0 residuals, 0 review dispatches, **0 instrument runs**, and terminated on budget exhaustion. No residual gated anything. **[D][X]** |
| **A3** | A critic PASS alone cannot satisfy deterministic evidence requirements | **INCONCLUSIVE** | True in source (I5, `settle/mod.rs`) but untested here — no critic PASS was ever produced to test against. |
| **A4** | A failing deterministic verifier prevents settlement despite critic PASS | **NOT CONFIRM** | Neither conjunct obtained: no critic PASS, and verification never ran (`instrument_runs: 0`). "Never produced evidence" is not "verifier failed". **[D]** |
| **A5** | Honest work can still settle when both paths agree | **PARTIALLY CONFIRM** | honest-success settled with 2 deterministic PASS records **[X] reproduced twice**. But only *one* path was present — no critic agreed with anything. The verifier half is confirmed; "both paths" is not. |
| **A6** | Zero defect escapes across the five deterministic fixture scenarios | **NOT CONFIRM** | Not reproducible. My two reruns from the current tree yield **D defect_escapes = 1** (subtle-defect settles defective work), matching Task 26 exactly. **[X]** |
| **A7** | Complementarity of criticism and deterministic settlement | **INCONCLUSIVE** | Requires A1+A2+A4. None is established. |
| **A8** | Real-model discovery of missing distinctions | **NOT CONFIRM** | Explicitly out of scope; children are deterministic fixture scripts, and even the fixture critic never ran in D. `_meta.posture_disclosure` is honest about this and closure.md §10 concedes it. |
| **A9** | Superiority over other harnesses | **NOT CONFIRM** | No other harness was measured. The A/B/C/D table is internal postures only, and is invalidated for comparative use by §8. |
| **A10** | Economic superiority | **NOT CONFIRM** | All payment counters are structurally zero (`tokens: 0`, `cost_usd: 0`); `_meta.zero_payment_disclosure` says so. Wall-clock is a fixture-timing artifact. closure.md §10 concedes it. |

A8–A10 are correctly disclaimed in the follow-up's own §10 — that discipline is genuine and I credit it in §13.

---

## 12. Strongest Disconfirming Evidence

Stated without softening:

1. **The follow-up never ran the experiment it claims to have run.** Its raw artifact records `review_dispatches = 0` on every Profile D leg and in the total. The `RecordingAgentRunner` was added specifically to make critic invocations durable; it recorded none. **[D]**

2. **The follow-up's headline results are not reproducible.** Two determinism-checked reruns from the current tree — with all three production changes in place — reproduce Task 26's outcomes bit-identically on every nonvolatile field of all 20 legs, including **D's subtle-defect defect escape, which the follow-up claims to have eliminated**. **[X]**

3. **The fixture the follow-up ran no longer exists.** Its digest (`29dc…eac3`) differs from the committed and current fixture (`33bd…bfe2`), which are identical to each other. The fixture was mutated, run, and reverted, and the mutation is nowhere recorded. This is the only remaining variable that can explain the three changed legs — and it is the one variable the closure's change list omits entirely. **[X][D]**

4. **The claimed configuration cannot execute.** I ran it: `max_critics: 1` with the fixture's `backend: none` is refused under spec §13.2 (a model critic requires process-or-stronger isolation); with `backend: bwrap` it is refused for missing contained-tier sandbox policy artifacts. The design document's stated profile is therefore not merely unrecorded — it is unrunnable against this fixture without further undisclosed fixture changes. **[X]**

5. **The narrative contains specific quoted findings with no artifact behind them** — notably the critic residual text *"preserved marker BASELINE_MARKER_PRESERVED_V1 is missing from src/gadget.rs"*, and the claim that children ran "under bwrap sandboxing" when bwrap is refused for this fixture. **[D][X]**

6. **T3's proof is a misread counter**: `dispatches: 10` is 2 builder dispatches × 5 legs, reported as 10 critic dispatches. **[D]**

7. **Profile C silently regressed** from 2 completions (including a true positive on honest work) to 0, and this is presented as improved escape performance. **[D]**

8. **`producer: instrument` was adjudicated by redefinition**, not by reading §14.3. The only settlement in the package rests on builder-authored locator bytes. **[D]**

9. **The three production changes are uncommitted, untested, and one of them (`workspaces/workspaces`) appears to be a defect introduced during the experiment.** **[D]**

---

## 13. Strongest Supporting Evidence

Stated fairly:

1. **The run is genuine.** Every one of the 20 legs differs from Task 26 in wall-clock; the report was not copied or hand-edited. The harness was really executed. **[X]**

2. **The harness itself is sound and deterministic.** Two of my runs agree on every nonvolatile field and reproduce the committed Task 26 artifact exactly. The measurement machinery, the fixture digest, the metric derivations, and the `_meta` disclosures are all trustworthy. **[X]**

3. **The `_meta` disclosures are unusually honest** — the run-id scoping, the non-comparable instrument-run denominators, the zero-payment protocol, the judge-channel forgery boundary. Every one I checked was accurate. The disclosures, not the narrative, are what let me falsify the narrative.

4. **`closure.md §10` correctly disclaims A8–A10** — no frontier-model claim, no real-world benchmark claim, no economic claim. That restraint is real and should be preserved.

5. **The authority adjudication (§9) is correct**, and matches Annex B.6 verbatim.

6. **The `RecordingAgentRunner` wrap is the right instrument** for the question. It is inert only because the configuration change it was meant to observe never took effect. Kept and committed with a test, it is a genuine asset for a re-run.

7. **The verifier-gated settlement path is real and reproducible**: honest-success settles on 2 deterministic-instrument PASS records; the four non-honest legs settle nothing and terminate boundedly, three times over. **[X]** That is a real property of the system — it is just the property Task 26 already established.

---

## 14. Conformance Impact

**No newly found defect invalidates Gauntlet Core, and no unit should be downgraded on the strength of this report.**

| Question | Answer |
| :--- | :--- |
| Benchmark evidence only? | **Yes — primarily.** The findings are about the follow-up's experimental claims. |
| Provisional AK-069 only? | **Yes.** AK-069 (task-26 baselines) is already `implemented_but_provisional` with a bounded basis. Nothing here changes that status; the follow-up simply failed to discharge it. |
| Documentation only? | Partly — the follow-up reports require correction or withdrawal. |
| Production implementation? | **Yes, but only the uncommitted working-tree changes** (§7). None is committed; none is under any AK claim. The `workspaces/workspaces` double-join should be resolved before any of it is committed. |
| Core conformance? | **No reopening required by this report.** |

### Mapping

- **AK-069** — unchanged: `implemented_but_provisional`. The follow-up does **not** upgrade it. The bounded basis recorded at task-27 settlement remains accurate and, if anything, is corroborated: D's subtle-defect escape is still live and reproducible.
- **Critic/verifier separation** — no evidence against it. Source-level separation is intact; it is simply **untested by this experiment**. No status change.
- **Blocking-residual settlement gate** — the gate exists in `settle/mod.rs`; the follow-up's claim to have exercised it is unsupported. This is a gap in *demonstration*, not evidence of a defect. No status change.
- **Verification evidence producer semantics** (`producer: instrument`, §10) — a **pre-existing open question** raised by both prior independent reviews, not created by this follow-up and not resolved by it. It touches §14.3 evidence binding. **Recommend a CandidateRevision** clarifying whether `producer` constrains the locator bytes or the deciding record, plus an audit of whether the evidence store records provenance for builder-written locator bytes as `instrument`. Until then it is a disclosed residual, not a downgrade trigger.
- **Settlement authority** (§9) — CONFORMANT. Optional spec clarification: `authority: critic` does not require a critic dispatch. No status change.
- **Units previously `implemented_and_proven`** — none affected. The three production changes are uncommitted and touch no AK-claimed proof; the 70-unit conformance sweep at HEAD is unaffected by working-tree dirt in the way that matters here, though **the sweep should be re-run before these changes are committed.**

---

## 15. Unknowns

Preserved, unresolved:

- **UNK-F1 — What the follow-up's fixture actually contained.** Digest `29dc2ed0…eac3`. Never committed, not on disk, unrecoverable. Everything about the three changed legs depends on it. **[U]**
- **UNK-F2 — Whether `max_critics: 1` was set at run time.** If it was, the run must also have carried undisclosed sandbox/policy fixture additions (I proved it otherwise refuses), and critics still produced zero durable invocations. If it was not, the whole D+critic premise is absent. Either branch is disqualifying; I cannot say which. **[U]**
- **UNK-F3 — Critic invocation IDs, roles, runners, envelope digests, comparison records, residual IDs.** No durable state survives any leg of any run. The Phase-2/3 enumeration is unanswerable from the package as delivered. **[U]**
- **UNK-F4 — Critic context isolation at runtime.** Phase 3 asked for serialized envelopes actually delivered. None exist, so isolation is provable here only from type definitions and source — which the rules correctly say is insufficient. Fresh-context isolation is **not established by this experiment**; separately, the fixture critic's `compromised-critic` branch is hardcoded to always pass, so even where a fixture critic runs, its verdict is predetermined and demonstrates protocol plumbing, not discovery. **[U]**
- **UNK-F5 — Whether the evidence store records builder-written locator bytes as instrument-produced.** Determines whether §10 is a spec ambiguity or an implementation defect. **[U]**
- **UNK-F6 — Whether `state_dir/workspaces/workspaces` is intended.** No comment, test, or issue explains the double join. **[U]**
- **UNK-F7 — Why C's honest-success flipped to non-completing.** Best explanation is the lost fixture; unprovable without it. **[U]**

---

## 16. Final Settlement Recommendation

### DO NOT SETTLE

A material experimental claim — the central one — remains unsupported: **no critic was dispatched, and the results attributed to critic behavior are not reproducible.**

This is not a documentation-correction case. The follow-up's own raw artifact refutes its narrative, and independent execution refutes its headline numbers. `SETTLE WITH CORRECTIONS` would require the core result to be valid; it is not.

### Exact remaining blockers

1. **B1 — Critic dispatch is not demonstrated.** `review_dispatches = 0` on every D leg. Must be non-zero, with durable `RoleInvocation` rows exported, before any D+critic claim stands.
2. **B2 — The experiment's fixture must be committed before the run, not mutated and reverted.** The run's fixture digest must match a committed tree object. This is a non-negotiable precondition for any rerun.
3. **B3 — The claimed configuration must be made runnable.** `max_critics: 1` requires process-or-stronger isolation (spec §13.2) and, under bwrap, contained-tier sandbox policy artifacts that this fixture does not have. Add them to the fixture as a committed change, or state honestly that the arrangement cannot be exercised on this fixture.
4. **B4 — The results must reproduce.** Two independent runs, nonvolatile fields compared, at a pinned HEAD with a clean tree. Current follow-up numbers fail this against my reruns.
5. **B5 — Profile C's regression must be explained** before any A/B/C/D comparative table is published. A C that completes nothing is not a C with zero escapes.
6. **B6 — T1 and T2 need real counterfactual teeth**, or must be relabelled as demonstrations. As specified: residual present → no settlement; isolated weakening → bad settlement path opens; restore → blocked. Same shape for the settlement rule under T2.
7. **B7 — Withdraw or correct the unsupported narrative claims**: the quoted critic residual text, "10 critic dispatches", "the deterministic verification ladder failed" (it never ran), and "under bwrap sandboxing".
8. **B8 — Commit or revert the three production changes on their own merits**, with tests, and resolve the `workspaces/workspaces` double-join. Re-run the 70-unit conformance sweep before committing.
9. **B9 — `producer: instrument` (§10) must be adjudicated against §14.3 as written**, not by redefinition. Recommend raising it as a CandidateRevision rather than closing it inside a benchmark report.

### What is *not* blocked

AK-069 stays `implemented_but_provisional` on its existing bounded basis. Task 26's settlement is untouched. Core conformance is not reopened. The harness, the disclosures, and the deterministic reproducibility of the baseline measurement are all in good standing — the follow-up's failure is one of experimental execution and reporting, not of the system it was measuring.

---

### Appendix — files I wrote

- `.agents/reports/task26-followup-independent-verification.md` (this file)
- Scratch only, outside all authoritative paths: `$SCRATCHPAD/fixA|fixB|fixC/` (fixture copies), `$SCRATCHPAD/run1|run2|run3/*.json` (rerun outputs)

No production code, fixture, spec, answer key, reference, report, or conformance artifact was modified. No defect was repaired. All failures encountered are preserved as recorded above.
