# Task 11 — fresh blind confirmation round 1: **NOT CONFIRM**

> Persisted by the orchestrator from the critic's final message (verbatim).
> Commit under review: a507d08. Verdict rule: any new critical/major ⇒ NOT CONFIRM.

Every closure claim C1–C6, C8a verified **true in code** — but the
new-residual hunt found one **new MAJOR (F1)**: the C3 content-digest binding
is one-directional, and the swapped-bytes attack re-mounted in the *locator*
direction (honest store record + doctored workspace file) succeeds end-to-end
through the REAL test-report adapter, minting a **PASS and
settlement-readiness over pinned bytes that say `failures:1`**.

## Claim-by-claim verification (all TRUE)

- **C1 TRUE** — rung set injected at `for_run` (authority.rs:216-222); `verify_claim` takes no ladder (authority.rs:343-347); constructors only under `#[cfg(any(test, feature = "composition"))]` (ladder.rs:145, 279, 457); only enabling edge is gauntlet-cli's `[dev-dependencies]` (Cargo.toml:41). Probed in /tmp: no transitive feature leak through gauntlet-cli (resolver "3"); without the feature all three constructors are E0599-absent; `VerifiedClaim` struct forgery is E0451. With the feature explicitly enabled externally, a forged rung mints PASS — the documented, manifest-visible §7.1 seam, not a C1 violation. **Pin status: NOT PINNED** — no test/arch rule checks feature edges; deps.rs ignores the `features` key (finding F2, MINOR).
- **C2 TRUE** — supported-rung `InstrumentFailed` → `LadderError::RungFailed { kind, instrument, detail }` immediately (ladder.rs:361-370); fall-through only `RequirementUnsupported` (:352-360); zero-dispatch test (ladder.rs:1322-1361).
- **C3 TRUE as scoped** — 0007 column; empty digest = Corrupt (sqlite state_store.rs:922-933); chain re-derives and refuses `EvidenceContentMismatch` (authority.rs:598-607, fail-first at :1403-1445). **But see F1** — covers only the store side.
- **C4 TRUE** — `settlement_precondition(claim, settling_run, deadline)` (precondition.rs:135-139) consults `claim.chain_evidence()` (:140; authority.rs:769-771); stooge test :392-411; struct forgery compile-refused.
- **C5 TRUE** — `for_run` reads `resume()` and refuses `IterationUnsupported` (authority.rs:236-250); no-checkpoint ⇒ default iteration 1 (conservative; high-iteration productions need the ungated write = recorded C7 residual, not new).
- **C6 TRUE in all six parts** — (a) typed `VerificationRefusal::Evidence` (authority.rs:573-582); (b) real locator tooth vs a contract-violating leaking store (:556-567, test :1566-1605); (c) role == Verifier + governed iteration bound (ladder.rs:568-585, 642-651, dispatch :475); (d) serialized JSON envelope (:691-725, hostile-text round-trip test); (e) `is_pinned` symlink refusal via symlink_metadata (evidence-fs store.rs:284-320, fail-first symlink test); (f) `KindVerifierMismatch` both directions (ladder.rs:284-292).
- **C8a TRUE** — comment names exactly the derived spellings the derivation produces; pin asserts them denied while bare words stay legal (denylist.rs:95-111, 1046-1124).
- **P12 core not regressed** (read-level) — absent → INCONCLUSIVE+class at every layer; three-class closed enum; class-on-PASS unrepresentable; deterministic-first with zero-model-dispatch assertions.

## New findings

### F1 (NEW, MAJOR) — examined-bytes/pinned-bytes binding is one-directional; swapped-bytes succeeds in the locator direction

The chain binds `digest(store-served bytes) == production.content()`. It never
binds the bytes the **instrument examines**: the dispatched request carries
the *locator* (authority.rs:425) and every deterministic adapter resolves it
as a mutable workspace path (test-report lib.rs:85-96). Nothing between
production and verification re-reads or re-hashes the locator bytes.

**Attack (executed, /tmp/attack-locator, REAL adapter, no forged durable
writes):** honest producer stores a FAILING report `{"tests":1,"failures":1}`
with its honest digest; run pins the ref; the workspace file at the locator
is doctored to `{"tests":7,"failures":0}` (builder-class write, spec §22.2);
locator match OK; store bytes hash to the recorded digest — content check
passes; recency/producer OK; the real adapter reads the doctored file →
**PASS**; `settlement_precondition` **GRANTS** readiness over the pinned
FAILING bytes. The record's audit claim is false both ways: pinned evidence
never examined; examined bytes never digest-bound. Falsifies the module's own
stated invariant (authority.rs:592-597) which holds only for the store side.
The p12 fixture itself benignly relies on the divergence (store bytes ≠
locator file bytes). **Not the recorded C7 residual** (ungated write +
declared iteration): this attack mints no false durable fact — every store
column is honest; the lie lives in the mutable examination surface, and
Task-12's C7 fix does not close it. Severity MAJOR not CRITICAL (requires
verifier-workspace write access between production and verification), but it
is exactly §16.4's class ("an agent that can rewrite the line that grades it
has no grader"). Fix directions: instruments return / chain re-derives the
digest of the bytes they examined and compares against `production.content()`;
or verification feeds instruments store-served bytes rather than a workspace
locator.

### F2 (MINOR) — the composition seal is not pinned

No test or arch rule asserts "no production (non-dev) edge enables
`composition`"; the arch manifest walker ignores feature specifiers
(deps.rs:287-310). True today, silent-by-regression tomorrow — contrary to
"a test enforces this, not a reviewer" (AGENTS.md Architecture).

### F3 (MINOR) — asymmetric gating of `ModelRung`

`ModelRung::new` is public in a normal build (ladder.rs:432); only `.rung()`
is gated. Harmless today (no injection path — verified by probe); gate it or
document why it is safe.

## Scope declaration (bounded claims)

Read in full: spec §5.2/§5.3.2/§5.3.6/§14/§16.1/§21/Annex B; AK-005/035/036;
plan Task 11; verify/{mod,authority,ladder,precondition}.rs; ports
verification_record/verifier/state_store (production+checkpoint+resume)/
comparator::load_version/evidence_store; sqlite 0006/0007 + state_store
(production+checkpoint) + encode grep; evidence-fs store (pin/symlink);
test-report + file-digest adapters; denylist.rs; p12 proof (full). Probed in
/tmp by compilation/execution: sealed-constructor absence, VerifiedClaim
privacy, no transitive feature leak via gauntlet-cli, the composition-seam
forged rung, and the F1 locator attack. Not done: running any repo test
suite. "Absence of further issues" claimed only within the files and probes
listed. Nothing inside the repository modified; probes at
/tmp/attack-{sealed,transitive,composed,locator} (artifacts removed).

**Bottom line:** the correction round did what it claimed — all C-claims
true, with real teeth. The round does not close because F1 shows the C3
discipline protects only the store side of the producer→instrument evidence
path; one bounded correction (bind the examined bytes, or feed instruments
store bytes) plus optionally pinning the feature seam (F2) addresses
everything found.
