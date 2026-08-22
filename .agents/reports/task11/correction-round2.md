# Task 11 — correction round 2 report

Builder: CORRECTION-2 FINISHER. The dispatched round-2 builder died to a
usage limit MID-EDIT, leaving the tree non-compiling and no result-contract
output (nothing appended to `builder-checkpoints.md`; no round-2 report).
Mandate: `final-confirmation-round1.md` findings F1 (MAJOR), F2 (MINOR),
F3 (MINOR). Raw fail-first/recovery captures verbatim:
`correction-failfirst-round2.log`. Ruler PASS before and after. No commit made
(orchestrator commits); operator files (AGENTS.md, justfile,
`.agents/prompts/`) untouched; `.wayfinder/`, preregistrations, spec, plan,
and existing suite cases untouched.

## Recovery note — reconciling the dead round-2 builder's footprint

Reconciliation was done file-by-file against the findings BEFORE any edit
(the session-interruption lesson). Footprint verdict: the dead builder's
DESIGNS are sound and were completed, not reverted.

- **Landed by the dead builder (kept, after review, as sound):**
  - F1, authority half: the `workspace_root: PathBuf` parameter on
    `for_run` (canonicalized at the open, `WorkspaceRootRefused` admission
    error), the `workspace: PathBuf` field on `GovernedVerification`,
    the `LocatorContentMismatch` refusal variant (locator + recorded +
    examined digests) with its Display, the pre-dispatch locator-half check
    in `binding_class`, `read_locator_content` (containment + NotFound →
    `Ok(None)` + IO failure → typed `Evidence` refusal), and
    `escapes_the_workspace_root`; the `VerifyingState` fixture's real scratch
    workspace root (write-at-locator in `record`, `workspace_root()`
    accessor, Drop cleanup) and the app-layer permanent test
    `a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass`; all
    `for_run` call sites adapted (mod.rs fixture, precondition.rs,
    authority.rs tests).
  - F1, p12 half: `PASSING_REPORT`/`FAILING_REPORT` constants (byte-equal to
    the real adapter's `write_report` output), the fixture `governed`
    wiring of the workspace root, the payload-constant substitution in the
    eight existing scenarios (fixture bytes only — no assertion touched),
    and permanent proof test 12.
  - F2, complete on inspection: the `FeatureEdge` surface in
    `gauntlet-arch-tests/src/deps.rs` (`features_of` both spellings,
    collection in `declared_deps`, `member_feature_edges`,
    `root_feature_edges`, `composition_seal_violations`), the repo pin
    `the_composition_feature_seal_holds` (anti-vacuity: it asserts the
    walker SEES the one legal edge), and two unit tests.
  - F3, complete on inspection: `#[cfg(any(test, feature = "composition"))]`
    on `ModelRung::new` with the doc defense of the symmetric seal.
- **Broken and repaired by this session (compile):** exactly the 6 E0308s
  the mandate names. The dead builder had changed
  `CandidateRejection::Refused(VerificationRefusal)` to
  `Refused(Box<VerificationRefusal>)` — a deliberate clippy
  `result_large_err` fix, per its own in-code comment ("unboxed exactly
  once, at the walk's single `return Err`"), because the new
  `LocatorContentMismatch` variant grew the error past the lint threshold —
  but had updated NEITHER the five construction sites NOR the one
  consumption site. Completed per the stated design: `Box::new(..)` at the
  five constructions (authority.rs :456-consumption aside), `*refusal` at
  the single `Err(CandidateRejection::Refused(refusal)) => return Err(..)`.
  Verbatim errors in the log §0. No other footprint file needed repair:
  `gauntlet-arch-tests` and `gauntlet-cli` compiled as found.
- **Half-landed in the evidence sense and completed by this session:** the
  dead builder's in-code comments CLAIMED fail-first captures ("captured
  verbatim in correction-round2.md") that did not exist — the report file
  was never written. This session generated all of them (log §§1–3) by the
  neuter-and-capture teeth discipline, backups diff-verified byte-identical.

## F1 (MAJOR) — examined-bytes/pinned-bytes binding, locator half

**Fix.** The earned chain now binds the bytes the instrument will examine:
`VerificationAuthority::for_run(run, iteration, ladder, WORKSPACE_ROOT,
deadline)` canonicalizes the evidence workspace root at the governed open
(`std::fs::canonicalize`; a root that does not resolve is the typed
`VerificationAdmissionError::WorkspaceRootRefused` — nothing downstream may
read evidence through a root the open never pinned). At the LAST check
before a candidate is admitted to dispatch, the chain re-reads the
production's locator under that root with the adapters' own containment
discipline (`escapes_the_workspace_root`: RootDir / ParentDir / Windows
Prefix components refused — a drive-prefixed locator is refused here too)
and requires the three-way equality
`digest(locator bytes) == production.content() == digest(store bytes)`
(the store half is the pre-existing C3 check, which runs first). A mismatch
is the typed `VerificationRefusal::LocatorContentMismatch { locator,
recorded, examined }` — refused BEFORE dispatch, never an outcome, never a
class. An ABSENT artifact is not classed by the chain (`Ok(None)`): the
dispatched instrument still reports `EvidenceMissing` and the layered P12
reconciliation mints the INCONCLUSIVE record naming the rung — absent
evidence stays the instrument's finding. Any other read failure is the
typed `Evidence` refusal (the C6.1 discipline). Candidates excluded by a
class (stale, producer mismatch, off-locator) never dispatch and never
reach this read — their behavior is unchanged.

**F1 option defense (shape chosen vs the alternative).** The confirmation
critic offered two fix directions: (a) the chain re-derives the digest of
the bytes examined and compares against `production.content()`, or (b)
verification feeds instruments store-served bytes rather than a workspace
locator. Landed: (a), extended with the adapters' containment discipline
(canonicalized governed root, escape refusal) so the re-read can never be
redirected outside the workspace the instruments read. Why not (b): it
requires a `Verifier` PORT revision — every deterministic adapter's dispatch
contract is locator-based today, including the frozen Task-4 shared
`verifier_suite`; feeding store bytes couples every instrument to store
credentials (a new trust surface per adapter), and it moves the defect from
the one place §14.3 already reads (the durable record) into each adapter's
seam. Option (a) closes the recorded-content gap at the chain, where every
other binding already lives, with one typed refusal.

**Honest residual (recorded, in-code and here).** The chain's read and the
instrument's read are two reads of a mutable file microseconds apart in one
process; spec §22.2 records that the harness is not a sandbox. Closing the
race entirely is option (b) — a Verifier port revision, recorded not chosen
(this is the F1-bound residual, NOT the recorded C7/Task-12 residual: no
false durable fact is minted; every store column stays honest).

**Failing-before evidence (neuter-and-capture; the critic's executed attack
as a permanent failing run).** The locator-half block neutered in
`binding_class`; both permanent tests failed:

```text
a doctored workspace file must be a typed locator-content refusal naming the
digests BEFORE dispatch, got Ok(Pass)
```
```text
a doctored workspace file must be refused before dispatch, never mint an
outcome; got Pass (settlement readiness granted: true)
```

The second capture IS the critic's executed attack (final-confirmation-
round1.md F1) under the composed path: honest FAILING store record, doctored
PASSING workspace file → Pass + settlement readiness GRANTED, pre-fix.
Restore verified byte-identical; suites re-green.

**The tests (permanent).** App layer:
`verify::authority::tests::a_doctored_workspace_file_cannot_ride_the_locator_
to_a_pass` (asserts the refusal names the locator and BOTH digests).
Composed layer: p12 proof test 12,
`a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass` — REAL
embedded sqlite state, REAL fs evidence store, REAL test-report adapter,
the run PINS the honest evidence, the doctored write lands between
production and verification; asserts the typed refusal and (in the failure
arm, for future teeth) that settlement readiness is granted under the
neuter.

**The p12 fixture divergence (fixed, as the mandate anticipated).** The
critic noted the p12 fixture "benignly relies on the divergence (store
bytes ≠ locator file bytes)". The fixture payloads are now the byte-exact
output of the real adapter's own writer (`PASSING_REPORT`/`FAILING_REPORT`
== `TestReportVerifier::write_report`'s format, verified against
`gauntlet-adapter-verify-test-report/src/lib.rs:71-72`), so the locator
file, the stored object, and the recorded digest are ONE fact in every
honest scenario, and the F1 check has a truthful substrate to defend.
Eight existing scenarios had their payload bytes substituted (constants for
literals) — fixture bytes only, no assertion weakened; drift between the
constants and the adapter writer is itself caught by the proof (any
divergence is an F1 three-way mismatch refusal). The app-layer
`VerifyingState::record` likewise now lands the artifact at the locator
under the fixture's real scratch root.

**Bounded claim.** No candidate that reaches dispatch can have locator
bytes that differ from the production's recorded content while the store
and record stay honest; the workspace root the chain reads through is the
root the open pinned, canonicalized, escape-refused. NOT claimed:
race-freedom between the chain's read and the instrument's read (§22.2
residual above); store-mediated instrument reads (option (b)); the ungated
producer-side write (B-M3/C7, Task-12 ledger, unchanged); protection
against a root that is itself swapped between open and read (canonicalize
pins the resolved path, not the directory inode).

## F2 (MINOR) — the composition seal, pinned

**Fix.** The arch manifest walker no longer ignores the `features` key of
dependency specs: `FeatureEdge { section, key, dep, feature }` is collected
on every INTERNAL (workspace-member) edge — member manifests
(`member_feature_edges`, sections labeled by `dependency_sections`,
including `target.<cfg>.<section>` spellings, so a feature cannot hide
behind a target scope) and the inheritable root `[workspace.dependencies]`
table (`root_feature_edges` — inheritable by EVERY member, `gauntlet-domain`
included, so a feature enabled there can never be the dev-only edge). Both
cargo spellings surface (array and bare string). The pin,
`composition_seal_violations`, admits exactly ONE edge in the workspace:
`gauntlet-cli`'s unscoped `[dev-dependencies]` `gauntlet-app` feature
`composition` — the edge correction round 1 recorded as the audited §7.1
seam. Every other spelling is a violation: any production section, any
other member, any target-scoped or build-dependency section, any root
entry.

**Failing-before evidence (live probe).** `gauntlet-cli`'s
`[dependencies] gauntlet-app` line changed to
`gauntlet-app = { workspace = true, features = ["composition"] }` — a
production edge enabling the seam. The repo pin failed:

```text
composition-seal: 1 violation(s)
  - composition-seal: crates/gauntlet-cli/Cargo.toml: gauntlet-cli enables
    feature 'composition' on 'gauntlet-app' via [dependencies] — the audited
    §7.1 seam (Task 11 correction round 1, C1) may be enabled ONLY by
    gauntlet-cli's unscoped [dev-dependencies] edge; any other edge puts the
    sealed ladder/rung constructors into a normal build of a crate that must
    not have them (Task 11 correction round 2, F2)
```

Restore verified byte-identical; the pin and both unit tests re-green.

**The tests (permanent).** `deps::repo::the_composition_feature_seal_holds`
(walks the LIVE workspace, root table included; anti-vacuity — it also
asserts the walker SEES the one legal edge, so a blind walker cannot pass
emptily) and, in the deps unit tests:
`feature_specifiers_on_internal_edges_are_visible_in_both_spellings`
(array, bare-string, target-scoped, renamed-package, external-deps-negative)
and `the_composition_seal_accepts_only_the_cli_dev_edge` (the legal edge;
production, bare-string, foreign-member, target-scoped, and root-table
spellings all violate; unrelated features stay clean).

**Boundary-suite growth (recorded act).** +3 arch tests (197 total, was
194) and one new class of manifest evidence (feature specifiers). Narrow by
design and documented in-code: this pins ONE seam; no general
feature-policy language is invented, no existing check widened.

**Bounded claim.** Every feature specifier visible in workspace manifest
TEXT on an internal edge or the root table is policed against the one
legal composition edge. NOT claimed: cargo resolver-level feature
unification through EXTERNAL crates (the critic's /tmp transitive-leak
probe stays outside manifest text; resolver "3" was probed at round 1), and
features on external dependencies (out of the seal's scope).

## F3 (MINOR) — symmetric gating of `ModelRung::new`

**Fix.** `ModelRung::new` is `#[cfg(any(test, feature = "composition"))]`,
identically to `Ladder::new`/`LadderRung::new`/`ModelRung::rung` (ladder.rs).
The in-code doc records the defense: harmless in isolation (a
constructed-but-unwrappable rung decides nothing — the critic verified no
injection path), but the seal's claim is that NO rung-shaped value can be
built outside the audited §7.1 seam, and one constructible half is a seam a
future refactor could grow on. Gate chosen over a why-public-is-safe note:
the note defends a fact about today's callers, the gate makes the fact
structural.

**Failing-before / seal evidence.** Pre-fix (the fact the critic cited,
verbatim from `git show a507d08:crates/gauntlet-app/src/verify/ladder.rs`):
`impl ModelRung { pub fn new(` — no cfg gate; public in a normal build.
Post-fix seal probe (a temporary non-test lib reference,
`just check-package gauntlet-app`, feature off):

```text
error[E0599]: no associated function or constant named `new` found for
struct `ModelRung` in the current scope
   --> crates/gauntlet-app/src/verify/mod.rs:585:32
```

Probe removed; restore verified byte-identical; `verify::ladder` 17/17
(the cfg(test) half of the gate) and p12 12/12 (the composition half — the
dev-dep edge is the one legal F2 edge) both green.

**Bounded claim.** In a normal build of any crate in this workspace, no
rung-shaped value is constructible outside the audited seam — all four
constructor surfaces are gated identically. NOT claimed: the composition
root's own trust (§7.1's position, unchanged).

## Recorded acts (deliverable deltas)

1. `CandidateRejection::Refused` boxes its refusal (the dead builder's
   clippy `result_large_err` fix, completed): `Box<VerificationRefusal`,
   unboxed exactly once at the walk's single `return Err`.
2. `for_run` gained the `workspace_root` parameter (all call sites adapted:
   the app fixture, precondition tests, authority tests, p12).
3. New typed variants: `VerificationAdmissionError::WorkspaceRootRefused`,
   `VerificationRefusal::LocatorContentMismatch` (refusal, never a class).
4. `VerifyingState` (app test fixture): real scratch workspace root,
   `record` lands the artifact at the locator, `workspace_root()`
   accessor, Drop cleanup.
5. p12: `PASSING_REPORT`/`FAILING_REPORT` constants byte-equal to the real
   adapter writer's output; eight scenarios' payload literals substituted
   (fixture bytes only — no assertion weakened); permanent test 12.
6. deps.rs: the `FeatureEdge` surface + the composition seal pin (+3 arch
   tests) — boundary-suite growth, documented in-code.
7. New app test (1) and the Box refactor in authority.rs.
   No existing suite case was modified in its assertions; no frozen
   artifact touched.

## Gates

- Focused: `-p gauntlet-app` 220/220 (44 `verify::`, was 43);
  `-p gauntlet-arch-tests` 197/197 (was 194);
  `--test p12_missing_is_not_passing` 12/12 (was 11).
- Full: `cargo fmt --all --check` clean; `just check` green; `just lint`
  green (clippy -D warnings, all targets); `just deny` ok (advisories/
  bans/licenses/sources; pre-existing allowlist notes); `just boundaries`
  197/197; `just test` **783/783** (778 round-1 baseline + 1 app F1 + 1
  p12 F1 + 3 arch F2).
- `just ruler` PASS before and after.

## Bounded overall claim

Every finding of `final-confirmation-round1.md` is addressed with captured
failing-before evidence and a permanent discriminating test (F1 twice:
app layer and composed layer through the REAL adapters; F2 as a live
manifest probe; F3 as a seal probe against the a507d08 fact). The executed
locator-direction attack is now a permanent refusal proof at both layers.
NOT claimed: the chain-read→instrument-read race (§22.2; the race-free
closure is the option-(b) Verifier port revision, recorded), the ungated
producer-side write and store-side iteration validation (B-M3/C7,
Task-12), crash-window replay (Task-13), rungs 5–9/11, resolver-level
transitive feature unification through external crates.

DONE — Task 11 correction round 2: F1 (locator-half examined-bytes binding,
typed refusal, permanent two-layer proof of the executed attack), F2
(feature-edge visibility + the composition seal pin, live fail-first), F3
(symmetric ModelRung::new gating, seal probe) — all landed, fail-first
evidence captured verbatim (correction-failfirst-round2.log), neuter
reverts byte-identical, all gates green (fmt/check/lint/deny/boundaries
197/ruler PASS before and after), `just test` 783/783. Awaiting the
confirmation critic.
