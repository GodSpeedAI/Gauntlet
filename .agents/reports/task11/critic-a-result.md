# Task 11 Critic Report (critic-a) — Verifier and Evidence Binding (spec §14)

Judged only against: Core-spec.md §14, §16.1, §5.2, §5.3.6, §6.2, §21, Annex B.5;
.wayfinder/answer-key.md AK-005/AK-035/AK-036; .agents/plans/gauntlet-impl.md Task 11;
crates/gauntlet-ports/src/contract/ shared suites.

Status: IN PROGRESS — findings appended below as sections are examined.

## Interim notes (after ports + app verify layer read)

Checked: crates/gauntlet-ports/src/{verification_record,verifier,state_store,role_result,evidence_store,comparator}.rs, crates/gauntlet-ports/src/contract/verifier.rs, crates/gauntlet-app/src/verify/{mod,authority,ladder,precondition}.rs, crates/gauntlet-domain/src/verification.rs; diffs vs HEAD (task 10).

- B.5 codec: all 12 annex fields present in annex order; strict unknown-key rejection at all three object levels; class null semantics enforced (Some(class) requires INCONCLUSIVE, both at construction and decode); instrument.deterministic carried; payload form drops schema token. produced_at is an epoch-seconds INTEGER, while the B.5 sample literally shows `"produced_at": ""` (string-shaped placeholder) — flag as format question (MINOR).
- Port diffs vs HEAD are additive only (verifier: +instrument() +supports(); evidence_store: +is_pinned() default-Err fail-closed; state_store: +EvidenceProduction/-Binding, +EvidenceProduced event, +3 write-once methods default-Err/None; comparator: FrozenReferenceProjection::load extracted to public load_version, old method delegates; role_result: +Verification payload variant). Contract suite: 3 cases preserved verbatim, 1 new case added (records_carry_their_instruments_identity). No weakening found in diff.
- Ladder order matches spec positions 1,2,3,4,10; deterministic-before-model structural. BUT: select_and_verify falls through to the next rung on VerifierError::InstrumentFailed — a mechanically-failed deterministic rung lets the MODEL rung decide the same claim (premature model reachability; ladder.rs:278-281 + ModelRung supports()=true). Flag for adjudication (candidate MAJOR).
- Ladder treats a dispatched rung's INCONCLUSIVE as terminal (outcomes stand) — defensible reading of 14.2/14.4, documented; noted as judgment call (MINOR).
- ModelRung hardcodes IterationNo::new(1) in RoleInvocationSpec regardless of governed iteration (ladder.rs:373) — audit-trail fidelity (MINOR).
- authority.rs binding checks: all four 14.3 conditions decided; requirement side from frozen reference (parse_answer_key per call), evidence side from durable production bindings + store; recency anchor = production.iteration_no vs governed iteration; locator check structurally tautological (lookup IS by locator) — the check at authority.rs:462 can never fire given evidence_productions_at filters by locator; dead-but-documented. Locator matching is exact-only; spec's "contract-defined for others" (non-deterministic kinds) has no contract defined anywhere (MINOR gap).
- Store read error during resolvability check is mapped to BindingClass::EvidenceUnresolvable (authority.rs:478-486) — infrastructure failure becomes a binding class rather than a typed refusal; fail-closed direction but conflates locus (MINOR).
- precondition.rs checks 16.1 conditions 1-2 exactly, re-derives evidence facts from store, fail-closed on unknown pin state, and documents 3-5 as Task-13 scope. Matches its claim.

---

# FINAL — Task 11 critic-a verdict

Scope actually examined (bounded claims): every file named in the task brief, read in full —
crates/gauntlet-ports/src/{verification_record,verifier,state_store,role_result,evidence_store,comparator}.rs,
crates/gauntlet-ports/src/contract/{verifier,stubs}.rs (diff vs HEAD),
crates/gauntlet-app/src/verify/{mod,authority,ladder,precondition}.rs + lib.rs diff,
all four crates/adapters/gauntlet-adapter-verify-*/src/lib.rs (+ their shared_contract tests),
sqlite adapter migration 0006 + full diff of migrations.rs/encode.rs/state_store.rs,
evidence-fs store.rs diff, crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs (full),
arch-tests diff (denylist/escapes/walk), root Cargo.toml + arch-tests Cargo.toml,
.agents/reports/reference-candidates.md, CURRENT_STATUS.md Task-11 section, task11/teeth.md.
Spec sections held at hand: §14, §16.1, §5.2, §5.3.2, §5.3.6, §6.2, §6.3, §21, Annex B.5.
No compilation or test runs were performed (blind critic); all "green" claims below are structural readings, and teeth.md is self-reported builder evidence I treated as unverified-but-plausible.

## Q1 — §14.1 one question per claim, three states, evidence ref, instrument identity: SATISFIED

- The chain mints exactly one `verification_record@v1` per `verify_claim` call; the document carries claim, requirement, instrument {name, version, deterministic}, evidence_ref, produced_at, outcome (crates/gauntlet-ports/src/verification_record.rs:55-68 fields; crates/gauntlet-app/src/verify/authority.rs:588-617 mint).
- All three outcomes are reachable: PASS/FAIL decided from artifact content by each adapter (e.g. test-report FAIL at failures>0, crates/adapters/gauntlet-adapter-verify-test-report/src/lib.rs:129-133); INCONCLUSIVE minted by the chain for every §14.3 failure and instrument absence.
- Instrument identity is the instrument's OWN report (verifier.rs:100-105; ladder.rs:149-153), and a returned record naming another instrument is refused (authority.rs:350-358). Domain record is the lossy §5.3.6 form (gauntlet-domain/src/verification.rs) — claim bound 1:1:1 to evidence and outcome by construction.

## Q2 — §14.2 ladder: SATISFIED WITH ONE MAJOR EXCEPTION

- Ordering: RungKind positions 1/2/3/4/10 mirror the spec ladder exactly (file-digest → schema → compiler/command → focused test → … → model; ladder.rs:88-95); dispatch is ascending position, duplicates refused (ladder.rs:211-296). Cheapest-conclusive-first holds.
- Deterministic preferred over model: structural (model at 10), tested at unit level (ladder.rs:684-720), composed level with real adapters + a zero-model-dispatches counter (p12 test 9, crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs:994-1041), with a recorded mutation tooth (teeth.md row 3).
- Model-backed flagged: `ModelRung::is_deterministic()==false` (ladder.rs:478-480); B.5 records minted over it carry `instrument.deterministic:false`, and a model record CLAIMING `deterministic:true` is refused outright (ladder.rs:527-533) — a strong reading of §14.2.
- **MAJOR-1 — premature model reachability.** `select_and_verify` falls through to the next rung on `VerifierError::InstrumentFailed` (ladder.rs:278-281), and `ModelRung::supports()` is unconditionally true (ladder.rs:489-494). Therefore a deterministic rung that CAN decide the requirement but fails mechanically (malformed report JSON → InstrumentFailed, test-report adapter lib.rs:100-117) yields dispatch to the MODEL rung, which may return PASS — the chain then mints a flagged but settling-capable PASS for a property that is deterministically measurable. §14.2: "Never ask a model to judge what a compiler, test, schema validator … establishes more reliably" and "Model-backed verification is legitimate ONLY where the property is genuinely not deterministically measurable" — that is a property of the CLAIM, not of the failed attempt. The chain's own refusal vocabulary already has the right terminal (`InstrumentsExhausted`, authority.rs:89-92); the model rung should not be reachable below a failed-but-deciding deterministic rung, or the fall-through past a supported deterministic rung should at least be recorded/adjudicated. Flagged for adjudication; mitigations present (flag carried, P12 unaffected).
- MINOR-1: a dispatched rung's own INCONCLUSIVE is terminal ("outcomes stand", ladder.rs:238-243) — a defensible §14.4 reading (no re-asking dearer instruments until they say yes), documented, but it means a cheaper rung's inability can shield a dearer conclusive instrument from ever running. Judgment call, not a letter violation.
- MINOR-2: the port's default `supports() == true` (verifier.rs:119-121) means a lazy custom rung declares every requirement; if such a rung sits at a low position and reports `EvidenceMissing`, selection TERMINATES (EvidenceAbsent, ladder.rs:263-268) without trying rungs that could decide. Fail-closed (INCONCLUSIVE), so no P12 risk — but a liveness hazard the default quietly invites.

## Q3 — §14.3 binding: SATISFIED; data provenance is the strongest part

- All four conditions are decided, and from durable state, not caller values:
  - requirement side (kind/locator/producer) ← the frozen `ReferenceVersion` of the durable run, digest-verified on open (`FrozenReferenceProjection::load_version`, authority.rs:162-189) and re-parsed from the answer key per claim (authority.rs:222-244);
  - evidence side ← write-once `EvidenceProductionBinding` rows (sqlite migration 0006, PK (run_id, evidence_ref), idempotent re-record / conflicting-refused tested in adapter state_store.rs) plus `EvidenceStore::get` for resolvability;
  - recency ← production's durable iteration vs the governed iteration; the ordering anchor is the store-minted event-log offset, never an argument (state_store.rs:325-352; sqlite record_evidence_production mints it from its own append).
  - The `VerificationRequest`'s requirement/evidence/claim/id are provably never read for binding decisions — demonstrated by the lying-request tests (authority.rs:864-900; p12 tests 2-5).
- The governed ITERATION is a caller-chosen coordinate of `for_run` (authority.rs:162-167). This mirrors the Task-10 comparator-authority pattern and is the coordinate of the question, not evidence data; a caller anchoring at an old iteration makes OLD evidence fresh only relative to that old anchor. Accepted pattern; noted, not charged.
- Four-conditions-to-three-classes mapping: the locator condition maps to `evidence_unresolvable` and this is documented in three places (verification_record.rs:29-39, authority.rs:459-473, p12 test 7) — not silently collapsed, and defensible ("evidence at another locator is not the required evidence"). Caveats:
  - MINOR-3: the locator equality check (authority.rs:462) is structurally dead — candidates are FETCHED by locator (`evidence_productions_at`), so the branch can never fire; the real decision lives in the sqlite WHERE clause. Correct but indirect; a regression in the adapter's query would defeat the condition with no failing check at the app layer (no tooth exists for it — teeth.md has none).
  - MINOR-4: locator matching is exact-only; §14.3's "contract-defined for [non-deterministic] kinds" has no defined contract anywhere. Exactness is stricter, not looser, so no violation — the looser regime is simply unimplemented.
- MINOR-5: an evidence-store READ ERROR during the resolvability check is mapped into `BindingClass::EvidenceUnresolvable` (authority.rs:478-486) — an infrastructure failure becomes a binding class on the record rather than a typed refusal. Fail-closed direction (never PASS), but it conflates §21's `persistence` locus with §14.3's `verification/inconclusive_evidence` classes.

## Q4 — §14.4 / P12: SATISFIED (no PASS from missing/stale/wrong; no INCONCLUSIVE→PASS)

Bounded search: full read of the verify chain + all four adapters; grep across crates/gauntlet-app/src/verify and adapters for `Outcome::Pass` constructions and Inconclusive conversions (all hits are fixtures/tests or artifact-content decisions); `VerificationRecordDoc::new` makes class-on-PASS/FAIL UNREPRESENTABLE (verification_record.rs:379-389); the settlement precondition refuses any non-PASS with the outcome carried distinctly (precondition.rs:136-146). Missing evidence: no production → INCONCLUSIVE/unresolvable; production without bytes → unresolvable; artifact absent at a resolving production → Err(EvidenceMissing) becomes INCONCLUSIVE/unresolvable with the reporting instrument NAMED (authority.rs:400-439, p12 tests 2-3). Stale → INCONCLUSIVE/evidence_stale, freshest-candidate-wins tie-break tested (authority.rs:1028-1104). Wrong producer/locator → INCONCLUSIVE with the respective class. Present-and-wrong CONTENT is FAIL (adapters), never PASS. The only PASS sources are instrument decisions over present, bound, fresh evidence.

## Q5 — Annex B.5 codec: SATISFIED (one format question)

- All 12 document fields, `requirement{kind,locator,producer}`, `instrument{name,version,deterministic}` in annex order; strict decode with unknown-key rejection at all three object levels and typed errors (verification_record.rs:79-103, 634-641); payload form drops exactly the schema token; wrong schema token, bad enum tokens, missing members all refused whole (tests at 714-855).
- `class: null` semantics: absent key → error; `null` decodes to None; a class token requires INCONCLUSIVE (ClassOutcomeMismatch both on construction and decode); INCONCLUSIVE-with-null-class (instrument uncertainty) is representable and documented as distinct from binding failure. Exactly the annex shape.
- `instrument.deterministic` round-trips as a boolean and gates model records (Q2).
- MINOR-6: `produced_at` is encoded/decoded as a non-negative INTEGER (epoch seconds; verification_record.rs:527-540) while the B.5 sample literally shows `"produced_at": ""` (string-shaped placeholder, like `observed_at` in B.3). The annex samples use "" placeholders for all strings, so this is most plausibly a placeholder artifact — but a strict schema-file consumer expecting a string would reject these bytes. Should be reconciled with docs/schemas when that lands.

## Q6 — §16.1 precondition slice: SATISFIED AND HONEST

- Checks exactly conditions 1 (PASS in THIS run; ForeignRun variant for the second half) and 2 (resolves AND pinned), re-deriving both evidence facts from the store rather than the record's say-so; pin-membership unknown FAILS CLOSED (precondition.rs:129-176). §16.1's letter "resolves and is pinned" is fully covered, including the is_pinned port addition whose default is an ERROR (fail-closed), closing Task 10's recorded residual.
- The module doc names conditions 3-5 as deliberately NOT checked and Task 13's scope (precondition.rs:18-24). Honest declaration; nothing in scope pretends to settle.
- `SettlementReadiness` has no public constructor — earned only through the chain-minted `VerifiedClaim`. Good typestate discipline.

## Q7 — Port-revision discipline: NO WEAKENING FOUND

Working tree diffed against HEAD (task-10 commit 1dab05c):
- contract/verifier.rs: the three Task-4 cases are byte-identical; ONE new case added (`records_carry_their_instruments_identity`) and registered in the suite runner. Recorded in-file as the extension act.
- verifier port: two additions (`instrument()`, `supports()` with default true). `instrument()` is a new REQUIRED method (breaking for implementors — only stubs and new adapters implement Verifier; all updated). No existing method's semantics touched; the doc records the layered EvidenceMissing-vs-INCONCLUSIVE reconciliation explicitly.
- state_store: additive (new event kind, new types, three new methods with fail-closed default bodies); evidence_store: `is_pinned` default errors; comparator: public `load_version` extraction with the old method delegating — behavior-preserving; role_result: new payload variant, existing variants untouched.
- contract/stubs.rs: only adds `instrument()` to the three verifier stubs, consistent with their minted records.
- arch-tests: denylist grows SPEC_VOCABULARY_SEGMENTS by "digest"/"check" (crate last-segments of two new adapters) with the pin test updated and the DISTINGUISHING spellings still asserted denied — a recorded, mechanical extension, not a loosening; escapes.rs/walk.rs diffs are behavior-preserving clippy refactors (guard-equivalent arms). I compared removed and added arms directly.

## Q8 — AK-005 / AK-035 / AK-036

- AK-005 ("three-state, no boolean conversion; INCONCLUSIVE never satisfies settlement"): the settlement half is carried STRONGLY by the composed p12 proof (tests 2,3,4,7 assert NotPass{Inconclusive} through the real chain) and precondition unit tests; the no-boolean-conversion half is carried by the pre-existing domain test (gauntlet-domain/src/values.rs:1196-1229, source-scanning for the From impl), which the p12 locator does not itself cover — the claim's evidence therefore spans the named proof plus a domain test outside it. Acceptable, but the split should be stated wherever the unit is declared.
- AK-035 (cheapest conclusive, deterministic over model): carried by ladder unit tests, p12 test 9 (real adapters, model-dispatch counter = 0), and tooth 3. Can carry the claim.
- AK-036 (INCONCLUSIVE on unresolvable/stale/wrong-producer): carried by authority unit tests + p12 tests 2/4/6/7/11 + teeth 1/2/4; the three classes asserted pairwise distinct. Can carry the claim.
- Locator materialization: all three declared locators (`tests/proofs/p12_missing_is_not_passing`, `tests/contract/verifier::deterministic_first`, `tests/contract/verifier::evidence_binding`) name repo-root targets that CANNOT exist under the boundary suite's cargo-target-path rule (TARGET_PATH_EXEMPT_CRATES is exactly gauntlet-arch-tests, which depends on no internal crate and so cannot host verifier-behavior tests). The p12 header documents this and materializes under the plan-gate target NAME at crates/gauntlet-cli/tests/ — the p1/p3 precedent.
- **MAJOR-2 — the reference-candidates ledger was NOT updated.** Tasks 7, 9, and 10 each recorded a dated locator-materialization entry in `.agents/reports/reference-candidates.md` proposing the re-declaration for every unmaterializable locator they touched (entries at 2026-08-17 AK-049, 2026-08-20 AK-026/027, AK-032/033/034, plus AK-021/022/023/024, AK-028/029/030). Task 11 added NONE for AK-005/AK-035/AK-036 (grep of the file: zero hits for those ids; git status: file unmodified). CURRENT_STATUS.md:965 mentions the p12 locator but the ledger — the sanctioned re-declaration channel for the frozen answer key — is silent. A verifier resolving any of the three locators today finds an empty directory (tests/proofs/ holds only .gitkeep). Same defect class prior critics required be recorded; process conformance gap, not a behavioral one.

## Unimplemented within Task-11 scope (missing, not different)

1. Ladder rungs 5-9 and 11 (broad test, static analysis, benchmark, runtime probe, visual diff, human) — absent. Plan Task-11 step 1 names only the four adapters (+ model rung), and §6.3 marks the rest RECOMMENDED, so this is NOT a plan violation; recorded so the "cheapest conclusive" promise is read against a five-rung ladder. Consequence worth noting: visual-diff-shaped evidence (p12's AK-9104 screenshot) currently falls to MODEL judgment because no position-9 rung exists.
2. No locator-condition tooth (see MINOR-3): the fourth §14.3 condition's actual enforcement lives in the sqlite query predicate and has no discriminating test at the app layer.
3. Crash-window replay for `record_evidence_production` (event durable, row absent) is documented in the error string ("replay recovery should complete the row") but no replay code exists; recovery is deferred without a ledger entry I could find.
4. No reference-candidates entries (MAJOR-2 above).

## Verdict

Behaviorally, this is a strong §14 artifact: the earned-chain binding design (frozen requirement side, write-once durable evidence side, store-minted ordering anchor, lying-request proofs) exceeds the letter of §14.3, the B.5 codec is strict and exact, the precondition slice is honest, and P12 holds at every layer I could trace. Two MAJOR findings for adjudication: the model rung is reachable when a decidable deterministic rung fails mechanically (§14.2 letter), and the locator-materialization ledger entries for AK-005/035/036 that every prior task recorded are missing. Everything else is MINOR and documented above. Teeth are recorded in task11/teeth.md with plausible verbatim failures, but I could not run them (blind).
