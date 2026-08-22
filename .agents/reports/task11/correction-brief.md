# Task 11 — correction round 1 brief

You are the CORRECTION builder for Task 11 (Gauntlet, /home/sprime01/projects/gauntlet).
Round 1 landed and is green; two blind critics + a denylist supplement found
the items below. Your brief is the FIX-NOW table in
`.agents/reports/task11/correction-adjudication.md` — read it first, plus this
file. For each item: write the FAIL-FIRST test (demonstrated failing for the
named reason BEFORE the fix), then land the smallest coherent fix, then re-run
focused suites. Do not modify `.wayfinder/`, the prereg, the spec, the plan,
existing suite cases, or the frozen Task-4 suite cases. clippy -D warnings,
cargo fmt --check, one compile at a time (memory-tight host).

Items (full attack traces in `critic-a-result.md`, `critic-b-result.md`,
`critic-c-denylist.md`):

- **C1 — unforgeable ladder (B-M1; prereg R1/A5).** The rung set currently
  travels as a per-call argument of `verify_claim` (authority.rs:261-266), so
  a hand-built rung mints a genuine PASS `VerifiedClaim` with a forged
  instrument identity — the two instrument-side checks (identity at
  authority.rs:351-357, evidence echo at :359-369) compare values the same
  caller supplied. Fix: the ladder becomes part of the earned authority —
  injected at construction (`VerificationAuthority::for_run` takes it),
  `verify_claim` loses the ladder parameter; `Ladder::new`/`LadderRung::new`
  sealed `pub(crate)` (the `AdmittedComparison` precedent — no public
  constructor anywhere). The hand-built-rung attack becomes a permanent proof
  test (compile-refused or typed-refused).
- **C2 — no model reachability below a deciding-but-broken rung (A-M1).**
  `select_and_verify` falls through on `VerifierError::InstrumentFailed`
  (ladder.rs:278-281) and `ModelRung::supports()` is unconditionally true, so
  a deterministic rung that CAN decide the claim but breaks mechanically
  (e.g. malformed report JSON) hands the claim to the MODEL rung, which can
  mint a settling PASS. Spec §14.2: model legitimacy is a property of the
  CLAIM (genuinely not deterministically measurable), not of the failed
  attempt. Fix: a SUPPORTED rung's `InstrumentFailed` terminates selection
  with a typed refusal naming the broken rung (P12's "improve the instrument"
  path), never falls through. Fall-through remains legitimate ONLY for
  `RequirementUnsupported`. Fail-first: supported-broken deterministic rung +
  model rung present ⇒ no model dispatch, typed refusal carrying the broken
  rung's identity.
- **C3 — bind examined bytes to pinned bytes (B-M4).** Today the instrument
  verifies filesystem bytes at the locator while the record/pin name the
  store ref; nothing connects them (put passing bytes at the workspace
  locator, different bytes in the store — record certifies the ref, pin holds
  unexamined bytes). Fix within Task-11's own surfaces (B.5/§5.3.6 field
  shapes are frozen — do NOT grow them): mirror the
  `ObservationBinding` content-digest discipline on the production/binding
  surface — the producer-side production record carries the content digest of
  the bytes ingested at that locator, and the chain re-derives/compares
  digests so bytes that are not the recorded content are refused whoever
  serves them; or (if you can defend it more cleanly) make the instruments
  read through the store. State which shape you chose and why in the report.
  Fail-first: the swapped-bytes attack (locator bytes ≠ store bytes) must be
  refused with a typed class/refusal.
- **C4 — precondition's store must be the claim's own (B-M5).**
  `settlement_precondition` takes `&dyn EvidenceStore` per call
  (precondition.rs:129-134) — a stooge store satisfies §16.1(1–2) for any
  PASS claim. Fix: derive evidence facts from the store the claim's chain
  holds (carry it inside the `VerifiedClaim`/chain context), not a fresh
  argument. Fail-first: stooge-store attack refused.
- **C5 — validate the iteration coordinate (B-M2 first half).** `for_run`
  accepts any caller-chosen `iteration_no` and nothing cross-checks it against
  durable run state. Fix: validate against the run's durable checkpoint (or
  derive from it); an anchor no durable state supports is a typed refusal.
  The store-side write validation stays deferred (C7/Task-12 seam). Fail-first:
  anchoring at an iteration the durable checkpoint contradicts is refused.
- **C6 — minors batch** (each with a discriminating test where constructible):
  1. Store-read errors during resolvability become a TYPED REFUSAL (use the
     dead `VerificationRefusal::Evidence`), not `BindingClass::EvidenceUnresolvable`
     — §21 locus conflation (A-m5/B-m4, convergent).
  2. The vacuous locator check (authority.rs:462 can never fire; the real
     enforcement is the sqlite WHERE clause): either make it a real check with
     an app-layer tooth, or pin its by-construction enforcement AND retitle
     p12 test 7 to state what it actually exercises (no-production-at-locator)
     — your choice, documented (A-m3/B-m1, convergent).
  3. Model-rung result validation omits role + iteration, and `ModelRung`
     hardcodes `IterationNo::new(1)` (ladder.rs:373, B-m5/A): validate
     `result.role() == Verifier` and carry the governed iteration.
  4. `verifier_envelope` builds JSON by raw interpolation (ladder.rs:553-570):
     use the ports json serializer so quotes/newlines in claim/locator cannot
     corrupt the envelope (B-m6).
  5. `is_pinned` follows symlinks (evidence-fs store.rs:294-300) unlike `get`:
     refuse symlinked object paths (B-m7).
  6. `RungKind::deterministic()` is dead code (ladder.rs:100-102): wire
     kind-vs-verifier consistency (a rung whose kind says deterministic but
     whose verifier says otherwise is refused at ladder construction) or
     remove it (B-m3).
- **C8a — denylist comment fix (critic C MINOR-2).** `denylist.rs:105-107`
  claims `digest-file`/`check-schema` are derived and denied; the actual
  derived spellings are `digest-file-verify`/`check-schema-verify` (a bare
  `"digest-file"` literal passes). Fix the comment to name the correct
  spellings. (The "~75" report figure is already corrected by the orchestrator.)

NOT in your scope (recorded elsewhere): B-M3 ungated production write and
store-side iteration validation (Task-12 ledger); crash-window replay for
production rows (Task-13 ledger); rungs 5–9/11; `produced_at` annex
placeholder reconciliation (doc note only — you MAY add the doc note);
locator "contract-defined" matching (ports revision).

Deliverables: fail-first ledger + fixes for C1–C6+C8a; focused suites green
(`-p gauntlet-ports`, `-p gauntlet-adapter-state-sqlite`, `-p gauntlet-app`,
four adapters, `--test p12_missing_is_not_passing`); full gates
(`just check && just lint && just deny && just boundaries && just ruler`) +
`just test`; update `teeth.md` with any new teeth; write
`.agents/reports/task11/correction-round1.md` (per-item: failing-before
evidence, the fix, the test, bounded claims); append progress to
`builder-checkpoints.md` after each item; end with a DONE line. `just ruler`
before and after — PASS.
