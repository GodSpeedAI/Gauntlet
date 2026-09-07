# COR-5 report — task 26, corrections round 5 (final small corrections)

Builder: COR-5. Date: 2026-09-06/07. Inputs read (blind posture held):
`falsifier-round2.md` + `charter.md` only — no other slice reports. The
dispatch brief's adjudication is binding: the round-2 hostile-child
forgery class (FR2-1/2/3) is a **recorded boundary, not a fixable
defect**; the work items were the best-effort manifest layer (W1), the
executed boundary disclosure (W2), and this record (W3).

Completion contract: this file exists with the round's battery green. It
carries NO settlement authority.

## Disposition of the round-2 findings (per adjudication)

| Finding | Disposition |
|---|---|
| FR2-1 (wrapper swap-window with self-restore; CRITICAL) | **Recorded boundary.** Nothing in this round authenticates the bytes EXECUTED at facet time; the digest pin remains a grade-time check. Execution-time wrapper re-digestion / judge-held wrapper placement is a runtime/sandbox change (task-25 territory), recorded as the structural prerequisite in the disclosure. NOT mitigated by W1 (the forged wrapper controls what it writes; the disclosure says the manifest catches naive snapshot rewrites and nothing stronger). |
| FR2-2 (in-place snapshot content rewrite; CRITICAL) | **Recorded boundary + W1 hardening.** The wrapper now digests every entry it writes into a nonce-named manifest; grading cross-checks each snapshot's CURRENT bytes against its manifest line. The NAIVE daemon shape (rewrite content, leave judge material alone) now grades tamper-INCONCLUSIVE — proven by a named unit test and a mutation tooth. A child that rewrites the snapshot AND its manifest line together is still undetectable: the manifest is inside the child's reach; the layer raises the bar, it is a race, not an invariant. |
| FR2-3 (count-preserving `.absent`→`.rs` replacement; MAJOR) | **Recorded boundary + W1 hardening.** The replaced `.rs` has no manifest line (the marker's line dangles) → tamper-INCONCLUSIVE for the naive shape, proven by a named unit test. A manifest-aware rewrite stays a recorded boundary (same as FR2-2's strong form). |
| E3 / E1@50ms / E2 run 1 (the race "wins") | Unchanged repelled-by-design observations; W1 converts two of the three race wins (FR2-2/FR2-3 naive shapes) into detected tamper. |
| Boundary disclosure (all three) | W2: `judge_channel_boundary_disclosure` reworded to state the EXECUTED finding plainly (see below). |

The shipped matrix results stand under the charter's declared
FIXED-CHILDREN threat model; the real-binary gate run this round
reproduced COR-4's recorded table exactly (below).

## W1 — best-effort snapshot manifest (hardening, disclosed as best-effort)

Sites (EXCLUSIVE scope respected):

- `crates/gauntlet-cli/src/improve_cmd.rs`:
  - `BASELINE_JUDGE_WRAPPER_BODY`: after each entry write the wrapper
    appends one `<name> <sha256 hex>` line into `<snapdir>/<nonce>.manifest`
    (`sha256sum | cut -d ' ' -f 1`, PATH-resolved host tools like the
    wrapper's `cp`/`basename`/`date`); the copy-failed branch creates the
    empty manifest (`: >> "$manifest"`), keeping the invariant
    **manifest exists IFF ≥1 facet execution happened**. The check's
    observable contract is untouched (stdout/stderr/exit verbatim; the
    manifest write is `2>&-`-discarded noise, fail-degraded only on the
    judge surface).
  - `manifest_name(nonce)` — the one naming rule.
  - `snapshot_manifest_fact(channel, entries) -> Option<String>` — the
    cross-check: manifest against zero facet executions = tamper
    (planted judge material); missing manifest with executions = tamper;
    malformed/duplicate/dangling lines = tamper; every `.rs`/`.absent`
    entry must carry exactly one line whose digest equals the file's
    current sha256 (`content_digest(...).hex()`), else tamper naming the
    fact. `.copy-failed` entries carry no line by design (still the
    typed judgment error).
  - `grade_judge_channel` order is now: wrapper pin → nonce set (the
    manifest passes by name) → manifest split + cross-check → zero/zero
    leg-copy fallback → count cross-check (manifest excluded) →
    judgment. The manifest check runs BEFORE the fallback so a planted
    empty manifest on a no-facet leg fails closed instead of falling
    back.
- `crates/gauntlet-app/src/baseline/report.rs`: only the disclosure const
  (W2). No schema/keys changed; `_meta` still has exactly the same ten
  disclosures, one with new wording.
- `crates/gauntlet-cli/tests/improve_baseline.rs`: the disclosure
  assertions (W2) + the designed T5 update — the judge dirs now carry
  the wrapper's manifest, so the content loops skip `.manifest` names and
  the honest leg's manifest must cover each snapshot's name+digest
  (asserted through the real binary).
- `crates/gauntlet-app/tests/baseline_harness.rs`: the boundary-disclosure
  assertion (W2).
- Arch inventory (`single_writer.rs`): **no entry owed** — the manifest is
  written by the shell wrapper (the existing "materialize the leg's judge
  wrapper" write site); the grading path only READS. No new
  `WRITE_API_TOKENS` occurrence exists in production text (verified by
  the arch battery below).

RED-first records (both named tests written and proven RED before the
mechanism existed):

- `a_snapshot_rewritten_under_a_stale_manifest_is_tamper` — RED because
  the old code's tamper fact named the COUNT, not a manifest cross-check
  ("the nonce-named snapshot count (2) is inconsistent…").
- `an_honest_manifest_grades_normally` — RED because the count
  cross-check counted the manifest as an entry (2 ≠ 1 ⇒ tamper instead
  of Pass).

## W2 — the executed boundary disclosure

`JUDGE_CHANNEL_BOUNDARY_DISCLOSURE` (`crates/gauntlet-app/src/baseline/
report.rs`) now states, in the recorded evidence document: under the
none posture the dispatched child holds full same-user OS permissions;
every piece of judge material inside its reach — wrapper, snapshots,
snapshot manifest, state and evidence stores — is forgeable by it and
**WAS forged successfully by executed hostile-child attacks** (wrapper
swap-window with self-restore, in-place snapshot content rewrites,
count-preserving `.absent`→`.rs` replacement — task-26 falsifier rounds
1-2); the defenses (wrapper digest pin, nonce+count cross-check,
snapshot manifest) raise the bar and fail grading closed over
`judge-channel-tampered` on detected tamper, but detection at grading
time is **a race, not an invariant**; the recorded D held-out figures
stand under the declared **FIXED-CHILDREN threat model** (fixture
children are repo-owned, git-pinned data); a hostile-child threat model
requires a **privileged judge channel** (uid separation or sandboxed
judge) — a recorded structural prerequisite, out of this task's fixed
posture. Every other disclosure is byte-identical to COR-4's.

## Teeth (this round's rows)

File: `crates/gauntlet-cli/src/improve_cmd.rs`. The mutation was applied
ALONE, proven RED, and restored byte-identically before anything else
(never batched). Evidence logs: `/tmp/cor5/tooth-manifest-red.log`,
`tooth-manifest-green.log` (volatile /tmp — contents quoted here).

| Tooth | Locus mutated | Mutation (exact) | Named test that went RED | Restore | Green re-proof |
|---|---|---|---|---|---|
| M1 (manifest cross-check) | `improve_cmd.rs` `grade_judge_channel`, the single call site | `if let Some(fact) = snapshot_manifest_fact(channel, &snapshot_entries) {` → `if false && let Some(fact) = snapshot_manifest_fact(channel, &snapshot_entries) {` | `improve_cmd::baseline_executor_tests::a_snapshot_rewritten_under_a_stale_manifest_is_tamper` FAIL — the in-place forged rewrite graded **Pass** (FR2-2's exact damage resurfaces); `an_honest_manifest_grades_normally` stayed PASS as required | sha256 before `8f51b9d33472dc0b7e04e1fe7f884c8535830c0b99d183c57f35a447348dff1d` = after ✓ | executor module 9/9 PASS |

The neutralization silenced ALL manifest clauses at once (existence,
coverage, digest match, dangling lines) — one tooth, one line.

## RED/GREEN ledger (W2)

- Assertions first: app `the_report_has_exactly_the_charter_keys_and_
  disclosures` RED; cli `a_planted_snapshot_on_a_no_facet_leg_is_tamper_
  inconclusive` RED (line 1434) and `the_plan_gate_shape_writes_the_
  charter_report_and_the_matrix_holds` RED (line 495) — each at its
  disclosure assertion, old wording lacking the new tokens
  (`/tmp/cor5/w2-red-*.log`).
- Const reworded → app GREEN; plant test GREEN; gate-shape test exposed
  the DESIGNED T5 surface change (the wrapper's manifest in the judge
  dirs) → T5 updated (skip `.manifest` names; assert honest-leg manifest
  coverage) → both GREEN (`/tmp/cor5/w2-green-cli2.log`).
- A `cargo fmt` reflow of the new code landed after the first battery
  (whitespace only; the teeth locus line verified unchanged); the full
  gauntlet-cli package battery was re-run green afterwards.

## Battery (all green; flock top-level, sccache stop inside the same command, package-scoped)

| Gate | Result |
|---|---|
| `cargo nextest run -p gauntlet-arch-tests` | 237/237 PASS |
| `just check-package gauntlet-app` / `gauntlet-cli` | exit 0 / exit 0 |
| `just test-package gauntlet-app` | 627/627 PASS |
| `just test-package gauntlet-cli` | 151/151 PASS (first run and post-fmt re-run) |
| `./scripts/verify-ruler.sh` | PASS — digests match the frozen lock |
| Real gate shape into scratch | exit 0 (below) |
| Kill-window flake discipline | no flake fired; `b1_contract_wins_over_exit7` never tripped; nothing re-run in isolation |

Real-binary gate shape (`GAUNTLET_BASELINE_SCRATCH_ROOT` +
explicit `--out` into the scratch world; the repo-root locator never
written): **matrix rows IDENTICAL to COR-4's recorded table** —
profile totals A 5 legs/4 completions/3 escapes/5 dispatches/0
instrument_runs, B 5/2/1/10/5, C 5/2/1/10/5, D 5/2/1/10/4; A/B/C
honest-success pass on `leg-copy`; D honest-success settles AK-961+AK-962
(rel 10000) on `instrument-snapshot`; A escapes lying-defect and
compromised-critic; B refuses lying/compromised, escapes subtle; C
catches subtle, escapes compromised; D refuses lying and compromised
(zero settlements), escapes subtle (`false_completion=true,
defect_escape=true` on the TRUE basis), silent `escalated` with
`reserve_intact=true` and `leg-copy-fallback`; A/B/C silent legs carry
`null` reliability; every D leg carries a non-empty `held_out_basis`;
the reworded `judge_channel_boundary_disclosure` is present in `_meta`
(all pinned tokens verified). Retained-world inspection: the honest
leg's manifest carries one `<name> <sha256>` line per snapshot (the two
settling units' snapshots, identical digests — the same honest work
product); the lying leg's manifest covers its `.absent` markers. Scratch
world removed after inspection; evidence logs remain in `/tmp/cor5/`.

## sha256 ledger

| File | Round start | Final |
|---|---|---|
| `crates/gauntlet-cli/src/improve_cmd.rs` | `2b019bb2937fd9bf91725b44d85d657b31179e17097fc0d89646c18ba5ca5af7` (matches the falsifier's ledger) | `3aac837f7fe2d4f6b55ed203faa3dd30dc8eb0a3cfa59c5505b431c982b4e771` |
| `crates/gauntlet-cli/tests/improve_baseline.rs` | `1a386ad8b1f3bd23d8f82b16ed059db3190192da320cb87b9a09334ffd2cc488` | `ff0106c8a255cac7ea1170f51ef7814e4a2378e8e785c222613f6cbd6ed25a1b` |
| `crates/gauntlet-app/src/baseline/report.rs` | `c5568a6220da6f47a20fa3ad2e040a4e8ae512f3995a3a2e0f2ad9decc67dbea` | `8d24a29e78f49c2b3e782f4bd29a2e6757f7c79d3d42193a29e6214f2113d5f0` |
| `crates/gauntlet-app/tests/baseline_harness.rs` | `555848ecb74b1d130ad7ac14236c7f0b7d986112e737a60402cec6ad4457b705` | `35b0b374a5f1ad1b03f99a9af358094b2deb8a3a0d261404ea2bd952a98abd5c` |

Teeth-time values: pre-mutation = post-restore
`8f51b9d33472dc0b7e04e1fe7f884c8535830c0b99d183c57f35a447348dff1d`
(an intermediate W1-complete state; the final file differs only by the
fmt reflow, full battery re-run green). `crates/gauntlet-arch-tests/src/
single_writer.rs` untouched this round (`94365718ae5fcc94d4ce6da6b952565
aa3b715417dfc5d6707b1b3d724838880` before and after). No `git add`/
`commit`; `tests/fixtures/**`, `.wayfinder/`, `.gauntlet/`, the justfile,
and the charter untouched; mutations restored one at a time.

## Deviations (plain)

1. The T5 gate-test update was not fully anticipated in the first W1
   edit: the first post-reword gate run failed at the T5 content loop
   (the manifest IS wrapper output and now appears in the judge dirs).
   This is the designed surface change, not a matrix movement — the
   failure came after every matrix assertion had passed. The loops skip
   `.manifest` names and now also assert honest-leg manifest coverage
   through the real binary (a strengthening, recorded in the file's T5
   doc).
2. A `cargo fmt` reflow of my new code landed after the first full
   battery (whitespace only). The teeth locus line was verified
   unchanged and the full gauntlet-cli battery re-run green.
3. `just lint` (clippy) was not part of this round's dispatched battery
   list and was not run; the closure gates remain the orchestrator's.

## For the orchestrator's records (suggestions only; you own both)

- **Charter append (COR-5 adjudication record):** the FR2-1/2/3
  disposition line above; the manifest layer's exact strength ("catches
  naive in-place rewrites and nothing stronger; race, not invariant");
  the FIXED-CHILDREN scope of every shipped D figure.
- **Reference-candidates entry:** the hostile-child judge-channel
  forgery class with its structural prerequisite — a privileged judge
  channel (uid separation / sandboxed judge), i.e. a runtime/sandbox
  capability this task's fixed `none` posture cannot provide; candidate
  threat-model extension with the three executed vectors cited
  (falsifier rounds 1-2).
- **Narrative wording (closure):** the judge-channel paragraph should
  carry the disclosure's substance (executed forgeries; defenses raise
  the bar; race not invariant; FIXED-CHILDREN scope; privileged-channel
  prerequisite) — not just the old "substitution boundary" sentence.
  F7's vocabulary note (bounded-not-converged vs "refuses") also remains
  open for the narrative.
- `.agents/CURRENT_STATUS.yml` needs the orchestrator's transition update
  (outside this builder's file scope).
