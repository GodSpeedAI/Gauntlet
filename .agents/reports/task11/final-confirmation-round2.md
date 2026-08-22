# Task 11 — fresh blind confirmation round 2: **CONFIRM**

> Persisted by the orchestrator from the critic's final message (verbatim).
> Working tree under review: a507d08 + correction-round-2 edits. Four
> non-blocking NOTEs; no new CRITICAL/MAJOR.

Every closure claim verified TRUE in code with independently executed
evidence; no new CRITICAL or MAJOR found.

## R2-F1 — locator-half examined-bytes binding: TRUE (code + executed)

- `for_run` takes `workspace_root`, canonicalizes at the governed open, typed
  `WorkspaceRootRefused` (authority.rs:265-291). Last check before dispatch:
  re-reads the locator under that root, requires
  `digest(locator bytes) == production.content()` (authority.rs:717-746);
  with the C3 store-half check (`digest(store bytes) == production.content()`,
  authority.rs:662-677) the three-way equality holds. Mismatch = typed
  `VerificationRefusal::LocatorContentMismatch` (authority.rs:149-153) — a
  refusal, never an outcome; BindingClass stays the closed three-value enum.
  Absent artifact stays `Ok(None)` → the instrument's EvidenceMissing →
  layered INCONCLUSIVE/P12. Containment (`escapes_the_workspace_root`,
  authority.rs:764-794) refuses RootDir/ParentDir/Prefix before any read, at
  least as strong as the adapters' `contained_path`.
- Permanent tests at both layers: app
  `a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass`
  (authority.rs:1654-1707); p12 test 12 (real sqlite/fs/adapter, run-pinned
  honest evidence, doctored write, typed refusal asserted;
  p12...rs:1162-1206). Fail-first captures verbatim in
  correction-failfirst-round2.log §1 (`got Pass (settlement readiness
  granted: true)` pre-fix).
- **Executed by the critic** (external probe, real adapters, 15/15 scenarios):
  S0 honest control passes; **S1 round-1 attack re-executed →
  `Err(LocatorContentMismatch)`, settlement unreachable — genuinely closed on
  the composed path**; S2 symlink→doctored bytes refused; S3 symlink→honest
  bytes = honest Fail over bound bytes (NOTE-1); S4 `sub/../../outside`
  escape locator → typed Evidence refusal before any read; S5 absent →
  INCONCLUSIVE/unresolvable; S6 stale+doctored → EvidenceStale
  (class-excluded candidates never reach the locator read).
- Race window honestly documented in-code (authority.rs:144-148, 730-734);
  race-freedom explicitly NOT claimed; §22.2 cited; option-(b) port revision
  recorded as the race-free closure.

## R2-F2 — composition seal pinned: TRUE (code + executed)

`FeatureEdge`/`features_of` on every internal edge (deps.rs:566-582,
668-682); `root_feature_edges` incl. `package=` renames (deps.rs:848-889);
`composition_seal_violations` admits exactly owner=gauntlet-cli + unscoped
dev-dependencies (deps.rs:891-918). Pin
`deps::repo::the_composition_feature_seal_holds` (deps.rs:1596-1649) with
anti-vacuity, runs under `just boundaries`. **Attacked with crafted
manifests**: target-scoped production edge → violation; build-dependencies →
violation; renamed package → violation; path-only edge → violation; optional
edge → violation; root-table rename → violation; legal cli dev edge → 0;
unrelated feature → 0. **Live walk: exactly one composition edge in the whole
tree (the legal cli dev edge), 0 violations.** No new masking: internal/
external/unresolved classifications unchanged; only non-additive edit is
`dep` → `dep.clone()`.

## R2-F3 — symmetric ModelRung gating: TRUE (code + executed)

`#[cfg(any(test, feature = "composition"))]` on `ModelRung::new`
(ladder.rs:441-442). **Seal probe executed**: exactly four E0599s in a normal
build (`Ladder::new`, `LadderRung::new`, `ModelRung::new`, `ModelRung::rung`).

## Round-1 closure set — no regression

C1 verify_claim takes no ladder (authority.rs:413-417); C2 RungFailed
terminal (ladder.rs:361-370); C3 EvidenceContentMismatch intact and firing
FIRST (store-half precedes locator read — swapped-store tooth preserved);
C4 store-less precondition; C5 checkpoint-validated anchor (authority.rs:
298-312); C6(c/e) role/iteration/envelope + symlink refusal; C8a untouched.

## Suite growth and fixture change — adversarial review

- Boundaries 194→197: +3 tests (pin + 2 unit), 0 removed/modified;
  arithmetic exact; purely additive; documented recorded act.
- p12 11→12: constants now byte-equal to the adapter writer format; eight
  payload-literal substitutions; `workspace.clone()` argument; new test 12.
  **No assertion line touched.** Fixture strengthened: locator file, store
  bytes, and recorded digest are one fact in every honest scenario; drift
  between constants and the adapter writer is itself an F1 refusal.
- App diff hygiene: removals purely mechanical (Box refactor for clippy
  `result_large_err`; call-site `state.clone()`).

## Notes (non-blocking)

1. Containment is path-string level, not inode level (S3): both readers
   follow a locator symlink identically; no false outcome mintable; requires
   workspace write access (§22.2). One sentence in the bounded claim keeps
   "escape-refused" honest.
2. Workspace-root pairing with the adapters' root is convention-bound at the
   library seam (same §7.1 composition-root trust position as the ladder);
   Tasks 12/20 production wiring must keep them one fact.
3. The absent-at-chain-read→written-before-instrument-read race variant is
   within the documented residual; stating it explicitly makes the residual
   airtight.
4. Working tree contains operator-file edits (AGENTS.md restructure, justfile
   additive recipes, `.agents/prompts/`) — process/tooling changes; no gate
   weakened. Trivial typo in AGENTS.md's Rust-conventions line (mangled
   backticks). Disclosed: the critic's /tmp cleanup glob deleted the
   builder's backup `/tmp/r2-teeth-f1-authority.bak` (byte-identical per
   cmp-verified restore; other two backups remain).

## Scope declaration (bounded claims)

Read in full: correction-round2.md, correction-failfirst-round2.log,
final-confirmation-round1.md; verify/{mod,authority,ladder(1-300,408-630),
precondition}.rs; p12 proof (full + full diff vs a507d08); deps.rs (diff in
full + seal sections in full); test-report adapter; evidence-fs symlink
sections; BindingClass; EvidenceLocator validation; root/cli/app manifests;
justfile/AGENTS.md diffs; spec §14.3/§16.4/§22.2; AK-005/035/036. Executed in
/tmp (artifacts removed): 15-scenario F1/F2 probe; F3 seal compile probe.
Not done: running repo suites (orchestrator's gates); the CLI/production
composition wiring (does not exist yet — Tasks 12/20); resolver-level
external feature unification (unchanged from round 1, out of scope).

**Verdict: CONFIRM** — all three round-2 closure claims true with executed
evidence; round-1 closures intact; suite growth legitimate; the executed
locator-direction attack is now a permanent two-layer refusal proof and did
not reproduce under re-execution.
