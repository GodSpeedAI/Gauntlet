# Task 11 — critic adjudication (round 1 → correction round 1)

Orchestrator dispositions on critic-a (spec) and critic-b (adversarial) findings.
Denylist-widening supplement pending; its section appended on arrival.

**Prereg R1 status: FIRED.** Critic-b MAJOR-1 + MAJOR-5 share exactly the
causal structure the prereg named ("verification validates values the caller
already holds" re-appearing at a new seam — the ladder and the precondition
store travel as per-call caller arguments). Predicted intervention class A5
(unforgeable value minted by a governed operation) applies.

## Dispositions

### FIX NOW (correction round)

| # | Finding | Disposition |
|---|---|---|
| C1 | B-M1: ladder is a per-call argument of `verify_claim`; a hand-built rung mints a genuine PASS with forged instrument identity (both instrument-side checks compare caller-supplied pairs) | A5 fix: the rung set becomes part of the earned authority — injected at construction (`for_run`), `verify_claim` loses the ladder parameter; `Ladder::new`/`LadderRung::new` sealed `pub(crate)` (AdmittedComparison precedent). Fail-first: the hand-built-rung attack must fail to compile or be refused. |
| C2 | A-M1: a supported deterministic rung that fails mechanically (`InstrumentFailed`) falls through to the model rung — §14.2 letter: model legitimate only where the property is genuinely not deterministically measurable (a property of the claim, not of the failed attempt) | InstrumentFailed at a SUPPORTED rung terminates selection with a typed refusal naming the broken rung (the correction loop's business, P12 resolution path "improve the instrument"), never model reachability. Fail-first test: supported-broken-deterministic + model rung present ⇒ no model dispatch, typed refusal. |
| C3 | B-M4: instrument verifies filesystem bytes at the locator; record/pin name the store ref; nothing binds examined bytes to pinned bytes | Invariant for the builder: the bytes the instrument examined must be provably the bytes at the bound evidence ref, or the mismatch must be detected. Preferred shape: the examined-content digest rides the Task-11-owned production/binding surface (mirror `ObservationBinding`'s content-digest discipline) and is re-derived at the chain — B.5/§5.3.6 field shapes are frozen and must NOT grow. Alternative: instruments read through the store. Fail-first: different-bytes-at-locator-vs-store attack must be refused. |
| C4 | B-M5: `settlement_precondition` takes `&dyn EvidenceStore` per call — stooge store satisfies §16.1(1–2) | Same A5 fix: the precondition derives evidence facts from the store the claim's own chain holds (carried inside `VerifiedClaim`'s context), not a fresh argument. Fail-first: stooge-store attack refused. |
| C5 | B-M2 (first half): the governed iteration is caller-chosen, validated against nothing durable | `for_run` validates the iteration against the run's durable checkpoint (or derives it); an anchor no durable state supports is a typed refusal. Second half (store refusing a production whose declared iteration exceeds the run's durable iteration) stays with the ungated-write seam → C7. |
| C6 | Minors batch 1: dead `VerificationRefusal::Evidence` (store-read errors must be a typed refusal, not `BindingClass::EvidenceUnresolvable` — §21 locus conflation, convergent A-m5/B-m4); vacuous locator check needs an app-layer tooth or an honest by-construction pin (A-m3/B-m1) AND p12 test 7's title must state what it actually exercises (no-production-at-locator); model-rung result validation omits role + iteration and hardcodes `IterationNo::new(1)` (B-m5/A-m) — validate role == Verifier, carry the governed iteration; `verifier_envelope` raw JSON interpolation → proper serialization via the ports json serializer (B-m6); `is_pinned` must refuse symlinked object paths like `get` does (B-m7); `RungKind::deterministic()` dead code — wire kind-vs-verifier consistency or remove (B-m3). | Close each with fail-first where a discriminating test is constructible. |
| C7 | B-M2 (second half) + B-M3: ungated `record_evidence_production` write; declared iteration unchecked at write | RECORDED DEFERRAL, upgrade trigger Task 12 (capability-gate the evidence-owner write + validate declared iteration there). Builder and both critics converge; ports doc already carries the residual. Ledger entry in CURRENT_STATUS. |

### ALREADY CLOSED / NOT DEFECTS (recorded)

| Finding | Disposition |
|---|---|
| A-M2: missing reference-candidates entries for AK-005/035/036 | CLOSED by orchestrator (dated entries landed 2026-08-21, before this adjudication was written). |
| A-m1: dispatched rung's INCONCLUSIVE is terminal | Accepted judgment call, documented in code — §14.4's "never re-asking dearer instruments until they say yes" reading. No change. |
| B-m2: producer check class-only | Faithful to §5.3.2's two-value producer vocabulary; documented. No change (revisit only if the spec grows producer identity). |
| A-m4: locator matching exact-only; "contract-defined for others" unimplemented | Stricter-than-spec is not a violation; recorded as a gap for the ports revision. No change now. |
| A-m6/B-m9: `produced_at` integer vs annex string placeholder | Document as a reconciled decision (annex samples use `""` placeholders for all strings); reconcile with `docs/schemas` when that lands (Task 17/27). Doc note only. |
| A-m2 (liveness): default `supports()==true` invites lazy rungs terminating selection via EvidenceMissing | Fail-closed, documented; note in the port doc. No behavior change. |
| A-unimpl-3: crash-window replay for production rows (event durable, row absent) | Ledger entry, Task-13 trigger (recovery composition), same class as the settlement commit-window discipline. |
| A-unimpl-1: rungs 5–9/11 absent | Not a plan violation (plan names four + model); recorded so "cheapest conclusive" reads against a five-rung ladder. Ports-revision / later-task surface. |

## Correction-round contract

Every FIX-NOW item lands with a fail-first test (demonstrated failing for the
named reason before the fix) in the correction round; the confirmation critic
then re-reviews blind. The hand-built-rung (C1), stooge-store (C4), and
swapped-bytes (C3) attacks become permanent proof tests.

## Denylist-widening supplement (critic C) — appended 2026-08-21

**NO CRITICAL, NO MAJOR.** The widening masks nothing (segment-exact
matching; the segments pre-exist nowhere; no spelling that denied at HEAD
passes now); the spec-vocabulary justification is genuine (15 verified
collisions, all spec vocabulary); pins have teeth in all four directions; the
Annex-B maskable intersection is unaffected (stays `["command"]`). Same
adjudicated class as Task 10's `model`.

Two MINORs → correction batch: **C8** — (a) fix the overclaiming comment at
`denylist.rs:105-107` (`digest-file`/`check-schema` are NOT the derived
spellings; `digest-file-verify`/`check-schema-verify` are); (b) the
unsupported "~75" figure in `builder-checkpoints.md` — corrected by the
orchestrator to the measured 15 (report-accuracy only). Accepted residual
recorded: bare `digest`/`check` inner-layer literals are non-distinguishing
(the `model`-class ruling, now explicitly pinned for these two words).
