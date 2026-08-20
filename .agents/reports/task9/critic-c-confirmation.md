# Task 9 — confirmation critic C, 2026-08-20

Dispatched blind to builder claims (adjudication + tree only; no builder
checkpoints/brief/reports/teeth). Static reading only. Report persisted
verbatim from the critic's final message by the orchestrator.

---

# Task 9 correction round — confirmation critique (2026-08-19)

**Scope of review.** I read: `.agents/reports/task9/correction-adjudication.md`; all five `crates/adapters/gauntlet-adapter-observe-*/src/lib.rs` in full; `gauntlet-adapter-observe-build/tests/shared_contract.rs` in full, the git contract's failure section, and grepped the set; `crates/gauntlet-ports/src/observer_evidence.rs`; `crates/gauntlet-app/src/observation/{mod,assembly,bundle,digest,input}.rs`; `crates/gauntlet-arch-tests/src/{denylist,walk}.rs`; `deps.rs::read_members`; `crates/gauntlet-cli/tests/{p3_real_observation,g6_hide_failed_observations}.rs`; `gauntlet-domain/src/values.rs` (WorkspaceRef); root `Cargo.toml`; the ports observer contract's error handling; `git status` (the whole Task-9 surface is uncommitted working tree, HEAD = task 8). I did **not** read the other three shared contracts in full, `text.rs`/`literals.rs`/`escapes.rs`, the agent-mock clock change, or any builder report (excluded by instruction). Nothing was compiled or run.

## Per-item verdicts

**C1 (workspace containment, all five adapters) — CLOSED.** Identical mechanism in filesystem (lib.rs:215–266), git (:200–251), build (:209–260), test (:210–261), command (:220–271): `canonical_root` at construction (:215 etc.), `resolve_workspace` refusing any `.`/`..` component outright (:232–243), canonicalizing the join, and an `starts_with` prefix check whose refusal detail names "containment" and "symlink". Each adapter has `workspace_resolution_is_contained_inside_the_observer_root` refusing `../sibling`, `./ws`, and a symlink-to-outside, with two positive controls (plain ref; symlinked ROOT canonicalized through). WorkspaceRef is an opaque string newtype (values.rs:848–853) — the fixtures are honest; the adapter check is the real gate. Regression probe (root deleted/recreated): the stored root is the canonical absolute path; recreation as a symlink to outside resolves outside → prefix check refuses (fail-closed); a root that never existed is stored unresolved, and a relative stored root can never prefix an absolute canonicalization → refuses (fail-closed, documented at e.g. filesystem:212–214). Remaining canonicalize→`current_dir` TOCTOU is the adjudication's explicitly documented no-action nit, not a silent half-fix.

**C2 (mismatch populates both arrays) — CLOSED.** `cross_check_facet`'s single `mismatch` closure pushes both a `provenance_mismatch` failure and an `unavailable_evidence` record (assembly.rs:211–221). Test `garbage_evidence_populates_both_gap_arrays` (:787–820) asserts both arrays name the kind; pre-fix, `unavailable_evidence` would be empty — discriminating.

**C3 (cross-check agreement + None-bypass) — CLOSED.** Name, version, exit, command, started/ended compared (assembly.rs:254–312); `None` facet exit vs evidence's always-present exit is a mismatch (:279–286) — the bypass is closed. `cross_wired_evidence_with_equal_exits_is_a_provenance_mismatch` (:827–868) uses equal exits with a foreign instrument/command/window — pre-fix it would pass silently; discriminating. `an_exitless_facet_against_exited_evidence…` (:873–900) pins the None closure. False-mismatch probe: every adapter builds the evidence doc and the facet from the *same captured locals* (`started`, `ended`, `exit`, `query`) — `clock.now()` is read exactly twice per observe and never re-read between evidence write and facet construction — so clock advance cannot split the pair. Window/command comparisons are Some-only, matching the adjudication. (Nano: the `ended_at` branch omits its siblings' `return;` — it is the final statement, behaviorally identical.)

**C4 (three typed plan refusals) — CLOSED.** `assemble` refuses empty observers, non-`sha256:`+64-lowercase-hex fingerprint, empty revision (assembly.rs:129–139, :319–327). Three tests, including nine bad fingerprint shapes (:965–994). The p3/g6 proofs' fingerprints come from `env_fingerprint`, which produces exactly that shape — they pass. Bounded residual (consistent with the ordered scope): the shape gate lives only at the assemble path; `ObservationBundleDoc::new`/`decode` still accept any non-empty fingerprint (bundle.rs:762) — the assembly doc comment states this construction-path scoping honestly.

**C5 (manifest error lines under the cap) — CLOSED.** Cap checks precede all three push sites: read_dir error (filesystem:315–324), iteration error (:333–342), entry lines (:348–351); each loop iteration pushes at most one line, so total ≤ cap + truncation line. The test (:752–806) fills the root to exactly the cap with two unreadable subdirs queued, asserts ≤ cap+1, `truncated`, and the truncation marker last — pre-fix it would be cap+2 lines with no truncation flag; discriminating. Host-vacuity precondition asserted (:775–781).

**C6 (non-UTF-8 escape determinism) — CLOSED.** `component_spelling` (filesystem:463–480): pure-ASCII names render historically (backslash→slash); any non-ASCII byte switches to `\xNN` lowercase escapes with backslash doubling; per-directory sort is by this spelling (:345). Test (:812–854) uses genuinely lossy-colliding `a\xffb`/`a\xfdb` (distinctness and collision-loss both asserted), checks two distinct lines with visible escapes, and byte-identical manifests across creation orders. Discriminating. Regression probe below (MINOR-1).

**C7 (maskable-intersection pin from the real member list) — CLOSED.** `the_maskable_wire_key_intersection_is_pinned` (denylist.rs:1434–1463) reads the real root manifest, `read_members` (deps.rs:764–792, confirmed to enumerate the explicit members list, which includes all five observe adapters), derives, and asserts the segment∩wire-key intersection is exactly `["command"]` — plus a widening control (hypothetical `gauntlet-adapter-observe-path` must grow it to contain `"path"`). Discriminating in both directions.

**C8 (spawn failure → Unavailable; g6 pin updated) — CLOSED.** build (:138–144), test (:138–144), command (:149–155) map spawn failure to `Unavailable` with C8 comments; build and test carry in-crate `a_missing_program_classifies_as_unavailable` tests that panic on any other class. The g6 pin now asserts `error_class == "unavailable"` with the drift-history comment (g6:269–278). Git's spawn path was already `Unavailable` (git:119–122); its contract's `InstrumentFailed` pin (git shared_contract:235–254) is the *documented nonzero-exit exception*, not a spawn case — no contradiction. Ports contract accepts either variant (contract/observer.rs:121), so "no suite edit" holds as predicted. Nano: the command adapter lacks the in-crate spawn test its build/test siblings have; the property is still discriminated cross-crate by g6 face 1 (same adapter, same path).

**C9 (split-argument residual pinned as masking) — CLOSED.** `annex_b_mask_corners_are_pinned_to_the_documented_residual` (denylist.rs:1328–1419) adds the critic's exact `registry.select("builder",\n "command",\n)` fixture, asserts it **masks** in all four `ANNEX_B_CONTRACT_PARSERS` modules naming D4, and — the anti-vacuity control — asserts the same fixture **reports** outside the codec modules (:1385–1394). No shape hack: `annex_b_wire_key_access` (:381–423) retains its three gates; the residual stays recorded in code comments and the walk.rs wire-key doc. This is pinned visibility, exactly as ordered.

**C10 (input.rs doc note) — CLOSED.** input.rs:12–20: command is the AK-028-required invocation record, gap details are diagnostic context, the exit status is the verdict carrier, and builder-penned text in those strings cannot flip an outcome.

## No-weakening check (previously verified clean)

- **Claims exclusion**: unchanged — private `InstrumentFacets`, `of_bundle`-only construction, canary tests intact (input.rs:269–321), p3 re-asserts no claims text reaches inputs (p3:305–315).
- **Gap totality**: single population site (`gap_entries_for` + the C2 closure), typed `Vec`s, decode requires both arrays present (bundle.rs:802–835).
- **Digest normalization**: timestamp stripping + digest-field pinning intact (digest.rs:26–66); the assembly placeholder still cannot leak (normalized form pins the field to `""`; assembly.rs:166–189).
- **Strict decode**: unknown-field rejection at every level in both codecs, with tests, intact (observer_evidence.rs:287–291; bundle.rs:246, :700, `reject_unknown`).

## Deferred items — no silent half-implementation

D1 (deadlines noted-not-enforced, assembly header :7–13), D2 (`new()` still declares via `std::env::vars()`), D3 (no recompute at consumption; `VerificationInputs` carries the recorded digest), D4 (pinned as masking, not fixed — correct), D5 (`as_domain_view` still drops facets/identity/revision), D6–D8 (port shapes unchanged), D9 (command kind still caller-bound; factory and p3 both bind `Test`, no hard binding), D10 (no action, none taken). All match the ledger.

## New findings

None CRITICAL or MAJOR.

- **MINOR-1 — conservative but inaccurate residual note.** `crates/adapters/gauntlet-adapter-observe-filesystem/src/lib.rs:460-462` records that an escaped spelling "can still collide with a PURE-ASCII name that literally contains the escape text." That collision is unreachable: the pure-ASCII branch replaces `\` with `/`, so an ASCII rendering can never contain a backslash, while every escaped rendering (≥1 non-ASCII byte ⇒ ≥1 `\xNN`) always does. The note over-states exposure in the safe direction; a future reader might "fix" a non-problem or mistrust the spelling's injectivity. Doc accuracy only.
- **MINOR-2 (informational) — C4 gate scope.** The fingerprint-shape refusal exists only at `assemble`; direct `ObservationBundleDoc` construction/decode still accepts any non-empty fingerprint (bundle.rs:762). This matches the adjudicated locus and the honest doc comment (assembly.rs:41–44), recorded here so it is not rediscovered as a gap.

## Overall verdict: **CONFIRM**

All ten fix items are present at the stated loci with tests that assert the strong property (each would fail against the pre-fix code by inspection), no previously-verified-clean property was weakened, no deferred item was silently implemented or half-implemented, and the regression probes I was asked to run (C1 root lifecycle, C6 cross-class collision, C3 honest-adapter false mismatch, C4-vs-p3 fingerprint) all come back clean, with the two minor notes above. Bounded claim: this is a static read of the files listed; I did not execute the suite.

---

**Orchestrator post-script:** MINOR-1 closed by the orchestrator as an
N-class doc-accuracy micro-fix (the confirmation critic itself proved the
collision unreachable; comment-only change, zero behavioral surface;
filesystem crate 7/7 + `cargo fmt --check` green after). MINOR-2 requires
no action — it IS the recorded scope note.
