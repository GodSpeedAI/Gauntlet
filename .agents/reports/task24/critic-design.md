# Task 24 — blind design-as-built critic report

Critic: BLIND design-as-built critic, fresh context, read-only. Inputs used:
`.agents/reports/task24/charter.md`, this brief, spec §13.5/§16/§19.4/§23.1/§23.2
(`.agents/specs/Core-spec.md`), ruler units AK-058/AK-070 (`.wayfinder/answer-key.md`,
read-only), the delivered tree (`crates/gauntlet-cli/tests/antigaming.rs`,
`.agents/reports/defended-mechanisms.txt`, `scripts/run-mutation.sh`,
`.agents/reports/mutation.json`, the gate.rs / optimizer.rs / .gitignore diffs, and
every manifest `source_location`). Blindness rule observed: `b1-report.md`,
`b1-progress.md`, `mutation-scratch/`, `mutation-run.log` were never opened.

## Instruments actually run (compile discipline observed)

Every cargo/just invocation was `flock .agents/reports/task24/compile.lock -c '...'`
(never nested; `sccache --stop-server` inside the same flock'd command):

- `just ruler` — **PASS** ("answer key and manifest digests match the frozen lock").
- `cargo nextest run --test-threads 4 antigaming::` (plan-gate shape 1 verbatim) —
  **0 tests run, exit 4** (see F-2).
- `cargo nextest run --test-threads 4 --test antigaming` — **18/18 PASS** (0.2 s).
- `cargo nextest run -p gauntlet-arch-tests` — **237/237 PASS** (E-1 negative
  controls green).
- `cargo mutants --list` probes (read-only) over `posture.rs`, `report/mod.rs`,
  `aggregate.rs`, `gate.rs`, `contract.rs`, `developmental.rs`, `promotion.rs` —
  used to reconcile every mutation.json shard count and every recorded exclusion
  against the real mutant set. All recorded counts reconcile exactly
  (61 total = 54 caught + 7 unviable; per-shard examined counts match the
  independent lists).

---

## Findings

### F-1 — MAJOR — The G7 exclusion justification is false, and the `-E` regex excludes catchable mutants the justification never names

Evidence:
- `.agents/reports/defended-mechanisms.txt:31-35` — "G7, `posture.rs:136:42` and
  `:139:50` (`<` -> `<=`/`==` in the two rank-decrease arms): EQUIVALENT mutants —
  each arm evaluates only when its own axis MOVED (`!=`), so `to <= from` and
  `to == from` coincide with `to < from` whenever the arm is evaluated at all".
- `scripts/run-mutation.sh:99` — shard excludes with one positional regex
  `posture.rs:136:42|posture.rs:139:50`, which matches EVERY mutant at those
  positions.
- `cargo mutants --list` (real set): each position carries THREE replacements —
  `<`→`==`, `<`→`>`, `<`→`<=` (six excluded mutants total, not the two the
  justification names).
- `crates/gauntlet-domain/src/unit.rs:258-276` — `blast_rank` {0,1,2,3} and
  `authority_rank` {0,1,2} are injective.

Analysis against the shipped g7 test (`crates/gauntlet-cli/tests/antigaming.rs:1400-1476`):
- `<`→`<=` (both arms): genuinely equivalent — when the arm evaluates the axis
  moved, so ranks differ and `<=` ≡ `<`. TRUE.
- `<`→`==` at 136:42 (blast arm): **not equivalent and provably caught by the
  shipped test**. Leg AK-9505 (`antigaming.rs:1466-1475`): operator,
  Local→Contained is a rank decrease (0 < 1); under `==` `decreased` becomes
  false, the operator arm returns Allowed, and the test's `!is_allowed()`
  assertion goes RED.
- `<`→`>` at 136:42 AND at 139:50: **not equivalent and provably caught by the
  shipped test**. Leg AK-9504 (`antigaming.rs:1456-1465`): operator raise on both
  axes; under `>` the decrease flag becomes true on the raise, the operator arm
  refuses, and `is_allowed()` goes RED.
- `<`→`==` at 139:50 (authority arm): not equivalent (an operator authority
  decrease would be Allowed under the mutant), though no shipped g7 leg exercises
  an operator authority-decrease, so it would be a survivor.

Consequence: 3 of the 6 excluded mutants are catchable by the very test that
guards G7; 5 of 6 are non-equivalent; only the two `<=` mutants satisfy the
recorded justification. The shipped `missed == 0` evidence for the G7 shard is
therefore narrower than the manifest's own recorded justification describes, and
the justification's central claim ("`to == from` coincides with `to < from`
whenever the arm is evaluated") is false — it holds only on increases, not
decreases. The g7 TEST itself has real teeth (demonstrated above); the defect is
in the teeth MANIFEST and the mutation evidence built from it — the exact
artifacts AK-058/AK-070 cite. Repair is cheap: shrink the `-E` regex to the two
genuinely equivalent `<=` mutants (or drop the G7 exclusion and let the run
catch the other mutants), and correct the manifest comment.

### F-2 — MINOR — The plan gate's shape-1 spelling selects ZERO tests (documented divergence, fails closed, but no delivered working recipe for the suite gate)

- Verified: `cargo nextest run --test-threads 4 antigaming::` → "Starting 0 tests
  across 94 binaries … error: no tests to run", exit 4. The plan gate's literal
  first command (charter lines 43, 265) is vacuous on this nextest; test ids
  display as `gauntlet-cli::antigaming <fn>` (confirmed in the run output), so the
  `antigaming::` positional parses as the filter DSL's package selector and
  matches nothing — exactly the mechanism `scripts/run-mutation.sh:29-40`
  (divergence note 3) records for the per-mutant invocation.
- Mitigations observed: the broken spelling FAILS CLOSED (exit 4 under the plan
  gate's `&&` chain halts the gate rather than passing vacuously), and the
  divergence is documented in the delivered script with the working spelling
  (`--test antigaming`), which I verified runs the full 18-test suite.
- Residual: no delivered artifact states the corrected shape-1 command for the
  orchestrator's closure battery; anyone running the charter's battery verbatim
  hits a hard failure, and the honest-battery spelling lives only inside a script
  header comment written for a different purpose. Not a silent weakening — hence
  MINOR — but the closure battery should run
  `cargo nextest run --test-threads 4 --test antigaming`.

### F-3 — MINOR — mutation.json predates the delivered suite and optimizer files (stale-by-24-minutes evidence)

- `.agents/reports/mutation.json` generated_at 2026-09-05T03:11:29Z
  (mtime 23:11:29 local); `crates/gauntlet-cli/tests/antigaming.rs` and
  `crates/gauntlet-arch-tests/src/optimizer.rs` were both written at 23:35:41 —
  AFTER the mutation run. The recorded caught/missed counts therefore describe a
  prior revision of the suite, not certified-ly the delivered one.
- Mitigations: the designed process re-runs the mutation gate on the final tree at
  orchestrator closure (CURRENT_STATUS.yml next_action records exactly this), and
  I independently confirmed the delivered suite is green (18/18). A post-run suite
  edit could only invalidate the evidence in the direction of missed mutants, so
  this must not be skipped: **closure MUST re-run `./scripts/run-mutation.sh` on
  the final tree.** Also note `--baseline skip` (script line 168) is legitimate
  only inside the contractually ordered pipeline (suite green before the script);
  the script itself never verifies baseline-green — the closure re-run must keep
  the suite-green-first ordering.

### F-4 — MINOR — Catchable line-level mutants inside the G16 and G1 defense fns are unexamined (evidence margin; claim-level teeth are proven)

- G16: `cargo mutants --list` shows six mutants inside
  `earlier_record_failures`' logic arms — `gate.rs:888:7` (`&&`→`||`),
  `gate.rs:888:10` (`delete !`), `gate.rs:900:7` (`&&`→`||`),
  `gate.rs:900:20` (`>`→`==`/`<`/`>=`) — all outside the two G16 shards
  (280:292, 842:846). At least three are provably catchable by the shipped g16
  tests: `888:10` (delete `!` → the earlier Flat evaluation records no margin
  failure → laundering → `g16_a_failing_evaluation_cannot_be_laundered_by_a_rerun`
  RED) and `900:20` `==`/`>=` (Flat payment treated as increased → the same
  test's `reasons.len() == 1` assertion RED).
- G1: `contract.rs:307:20` (`!=`→`==` in the version check) is outside the
  347:360 span but is catchable by the shipped g1 test's first assertion (the
  VALID contract would be refused).
- Context that keeps this MINOR: the G16 claim's teeth ARE demonstrated — the
  examined `gate.rs:842:5` mutant is the FULL body-kill
  (`replace earlier_record_failures -> Vec<GateRefusal> with vec![]`, caught),
  and both sweep-bound mutants (286:66 `-`→`+`, `-`→`/`) are caught; likewise the
  G1 rubric-loop mutants (348:16, 348:58) are caught. The unexamined set is
  partial-neutralization margin, not a toothless claim. Widening the two spans to
  the full fns would close it.

### I-1 — INFORMATIONAL — The G9 advisory disposition is honest (the honest-boundary rule is satisfied)

- Verified by search: `FilesystemMode` (`crates/gauntlet-app/src/roles/definition.rs:53`)
  has NO consumer outside the roles module — no dispatch, sandbox, or containment
  path enforces role filesystem permissions. The runtime genuinely has no
  mechanical write-scope defense for a critic-context process, so advisory is the
  correct classification, not an underclaim.
- The test (`antigaming.rs:2487-2556`) asserts exactly the surface that exists —
  the recorded typed posture (ReadOnly/Deny) and the closed vocabulary's refusal
  to widen (`UnknownFilesystemMode`) — and its header says "ADVISORY TEST — no
  mutation scope … not a mechanical containment proof".
- The manifest entry (line 58) carries `advisory:<reason>` with the reason spelled
  out and no mutation scope; `install.rs:113`'s `Enforcement::Advisory`
  recorded-weakness class exists as described (`install.rs:104-116`); the
  candidate-revision note (runtime write-scope enforcement as the missing
  mechanical defense) is present in the test header. This is a recorded weakness,
  not a disguised fake.
- Cosmetic nuance: the g9 test binds to `roles/definition.rs` parsing, while the
  entry's documentary location is `install.rs:113` (the lock's Advisory class,
  which the test does not exercise). Harmless for an advisory entry.

### I-2 — INFORMATIONAL — The gate.rs re-adjudication change is verified against the §19.4 contract

- RED-first plausible: pre-change, `adjudicate` read `evaluations.last()` alone
  (the zero-evidence shape still at `gate.rs:263`); with a failing-then-passing
  pair the re-rolled record was adjudicated — the shipped laundering test
  (`antigaming.rs:793-846`) would have been RED pre-change and is GREEN post-change.
- Semantics preserved: failed ⟹ Rejected (written), gaps ⟹ Provisional (written,
  never promoted) (`gate.rs:472-494`); the zero-evidence provisional stays
  unwritten (`gate.rs:262-271`); AlreadyFinal write-once past Promoted/Rejected
  (`gate.rs:248-254`), pinned by the g16 honest-road test's third adjudication
  (`antigaming.rs:919-930`).
- The honest road stays open: the sweep deliberately collects only record-derived
  FAILURES (governance, instrument series, regression, margin, payment) and never
  gaps (`gate.rs:821-836` doc; `g16_the_honest_road_…` GREEN — a protected-set
  gap superseded by a complete re-evaluation promotes with derived durable
  figures).
- Reasons ordering: per-record condition order is preserved (each failure carries
  its condition's own position); the sweep prepends earlier-record failures ahead
  of the fresh record's, so the global vector interleaves records rather than
  staying strictly condition-ordered across records. Acceptable — the code
  comment's "the conditions' own order" claim holds within each record.
- Smallest typed change: one private fn + one sweep loop + doc comments. Two
  sub-notes: (a) `earlier_record_failures`' early `return failed` on an
  unparseable earlier-record metric (`gate.rs:887-889`) also skips that record's
  payment re-derivation, which does not depend on the declared metric —
  unreachable via the harness (both records carry the same metric token) but
  over-broad; (b) condition 2/4/6 are honestly excluded from the sweep with the
  correct reason (they compare against current input/ledger state, derivable only
  over the freshest record).

### I-3 — INFORMATIONAL — Manifest validity and pipeline honesty verified end-to-end

- All 16 G-IDs present, pipe-format valid; all 16 `source_location`s exist and
  contain the described mechanism at/near the named line (verified line-by-line:
  contract.rs:347 rubric loop; provider.rs:318 verify_unchanged compare,
  provider.rs:267 freeze tripwire; producer_authority.rs:701 no-records arm,
  :715 no-PASS arm; promotion.rs:486 variation fn; gate.rs:780 regression_verdict;
  observation_digest.rs:29 normalized_bytes; posture.rs:124 lattice review;
  aggregate.rs:383 verdict rule; install.rs:113 Enforcement; comparator.rs:1486
  assert_blind; delta.rs:159 settlement_access_verdict; developmental.rs:304
  completed floor; report/mod.rs:485 classify_unit; gate.rs:263 the re-ask
  adjudication).
- The in-suite validator (`antigaming.rs:2565-2656`) mechanically enforces format,
  sixteen-G coverage, location existence, and non-advisory `antigaming::`
  target binding — the teeth map cannot silently rot.
- mutation.json reconciles exactly with independent `--list` recon on the current
  tree (per-shard examined counts: contract 2, provider 3, producer_authority 13,
  promotion 2, gate 781:800 4, observation_digest 10, posture 7 = 13−6 excluded,
  aggregate 1 = 5−4 excluded, comparator 7, delta 2, developmental 2, report 4 =
  5−1 excluded, gate 280:292 2, gate 842:846 2). `manifest_sha256` matches the
  delivered manifest. The merge fails non-zero on missed ≠ 0 (script line 266) and
  the plan's python assert backs it.
- Exclusion justifications checked against source: G4 TRUE (body→`false` only
  over-refuses — fail-safe; the unsafe `with true` mutant IS examined and caught),
  G8 TRUE (the 383:30 blocking rule is examined and caught; the excluded
  385-routing is §13.5 completeness covered by the compare module's in-crate
  tests, which exist), G15 TRUE (`&&`→`||` is indistinguishable through any
  honest fixture because `AdmittedSettlement` is unconstructable outside the
  authority — private field, no public constructor, `producer_authority.rs:623-633`
  — and admission moors every committed row's verification ref to a durable PASS
  record), G14 complete (the run-durability leg contains ZERO mutable sites per
  `--list` — nothing is hidden; both live mutants at 304:9/304:22 are examined
  and caught). G7 is the false one (F-1).

### I-4 — INFORMATIONAL — Per-test teeth (AK-058) adjudication: every one of the sixteen bites

Each suite test binds to its manifest mechanism such that a plausible mutation of
that mechanism turns THAT test RED (verified analytically against the shipped
assertions, and via the mutation evidence where examined): G1 (refusals vanish →
`expect_err` RED), G2 (digest compare → moved ruler unnoticed), G3 (no-records arm
→ typed-error AND exact-detail assertions RED), G4 (`!=`→`==` or body→`true` →
identical axes admitted, refusal assertion RED), G5 (regression condition → skip
invisible → Promoted → panic RED), G6 (gap arrays exempted → digests coincide →
`assert_ne` RED), G7 (lattice review permissive → agent downgrade Allowed → RED;
also operator-decrease and operator-raise legs independently catch the F-1
mutants), G8 (`blocking` dropped → blocked-under-pass-intent aggregates Pass →
RED; clean-pass positive control keeps the refusal attributable), G10 (freeze
tripwire → tampered key freezes), G11 (deny list neutralized → nested-array
canary assertion RED — non-vacuous by construction), G12 (PASS filter → FAIL
admits; plus foreign-evidence, invented-ref, idempotent-replay, and
exactly-once-with-distinct-refusal legs), G13 (settlement-access classification →
weakening classifies inert → Promoted despite margin → RED), G14 (floor removed →
unearned credit accepted → typed-error assertions RED), G15 (proven pair broken →
either the positive control loses proven or the FAIL unit reads proven → RED),
G16 (fn-kill or sweep-bound mutant → laundering → panic RED; honest-road test
pins the gap-supersession and AlreadyFinal floor). Suite-level thinness where
deep proofs exist (G6/G7/G10, and the cited p-proofs) is disclosed in headers and
the manifest's citation block, per the charter.

### I-5 — INFORMATIONAL — Scope discipline clean

- Tracked diffs are exactly: `gate.rs` (the sanctioned RED-first i1 correction),
  `optimizer.rs` (E-1), `.gitignore` (+`mutants.out/`), and
  `.agents/CURRENT_STATUS.yml` (the orchestrator's own dispatch-time
  IN-FLIGHT update per the AGENTS.md phase-transition rule — orchestrator-voiced
  content, not a builder surface violation). No settled proof file
  (g6/g7/g10/p*.rs), no ruler file, no justfile recipe touched; no new
  dependencies (no Cargo.toml/lock changes).
- E-1 is consistent with the existing negative-control style: five new
  file-mutation token classes each carry a positive fixture
  (`every_task24_file_mutation_token_is_reported_individually`), and the `write!`
  rule is the TARGET with same-line and wrapped negative controls for the
  formatter shape — 237/237 arch tests green, and the promote module's real
  `write!(f, …)` shapes are all formatter-form.
- The charter's probe-hygiene warning was checked implicitly: no stray files under
  `.gauntlet/policies/` were created by this critic, and `just ruler` passes.

---

## Verdict

One MAJOR finding (F-1: the G7 exclusion's equivalence justification is false and
its `-E` regex excludes three mutants the shipped g7 test provably catches —
an honesty defect in the teeth evidence itself, cheap to repair), three MINOR
findings (F-2 plan-gate shape-1 spelling vacuous-but-loud, F-3 mutation.json
staleness vs the delivered suite, F-4 unexamined catchable line-level mutants in
the G16/G1 fns), and five INFORMATIONAL entries. The suite itself (18/18 green),
the gate.rs §19.4 correction, the honest G9 advisory disposition, the manifest
structure, and the pipeline mechanics are sound.

**VERDICT: NOT_CONFIRM**

Finding counts: 0 BLOCKING · 1 MAJOR · 3 MINOR · 5 INFORMATIONAL.
