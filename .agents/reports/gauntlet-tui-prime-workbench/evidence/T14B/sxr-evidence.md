# T14B Evidence — Reality Trace evidence lens and qualified semantic composition

Date: 2026-09-09
Task: T14B
Proof Level: P3 (confirmation: independent)
Scope: Typed SXR output parser (versioned command envelope + raw ledger export array, parsed separately), the DEC-005 Correction Round 1 requalification (both-mode object carrier, CAS `content_ref` carriage, real Relation-declaration capture), the bounded refresh contract (closed read-only argv vocabulary, deadline/output bounds, process isolation from render), the declaration → linked Unit → question → threshold → evidence → SXR-settlement lens over the existing Inspector/Evidence/Trace data contract, and the SXR→Gauntlet evidence admission boundary that refuses pending and imports no settlement authority.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T14B.prereg.yaml` — verified this session via `just prereg-check gauntlet-tui-prime-workbench-T14B` (sha256 `60943b46a799b141de1123cafc2ad60e342f5461e3f2ce9ca79b029802127692`, matches the frozen `.sha256`), never modified.

## Claim (preregistered)

Real qualified producer/consumer and UI compose while SXR identity/settlement remain distinct and optional, and source loss preserves core journeys. NOT proven (per plan): SXR evidence automatically becoming Gauntlet evidence or settlement; SXR explain/autonomous runs; a graph engine.

## Producer qualification — what was REALLY exercised on this host (2026-09-09)

The hot_context sibling repo exists (`../sxr`) with a real debug build, and the full SXR lifecycle was driven on an isolated XDG root (`XDG_DATA_HOME`/`XDG_STATE_HOME` under a temp dir; the operator's real SXR state was never touched). Every capture is pinned byte-exact under `captures/` with digests in `CAPTURES.sha256`:

- **Binaries (sha256, recorded in `QUALIFIED_SXR_BUILDS`)**:
  - `sxr 0.1.0` (`sxr/target/debug/sxr`): `89c006fbea677a59e96a58cadc3ac3a2ba06ca8e06d785a6cff05102fb0d7e86`.
  - Slice 0B producer `domainforge-slice0b/target/debug/domainforge` 0.16.0 (`f0ca9def…`) — this is the binary SXR discovers by DEFAULT (`sxr-cli/src/commands.rs` `domainforge_bin()`), and it ANSWERS `envelope --emit both` (T14 pair P4's build; a fourth local build beyond T14's three).
  - Installed `~/.cargo/bin/domainforge` (`ee3ee077…`): still has NO `envelope` subcommand (re-verified directly; T14 pair P2 holds).
- **Full real lifecycle, 13 committed ledger records**: repo enroll → question declare → threshold declare → `semantic snapshot create` (composed the REAL slice0b producer through sxr-df's adapter: D constructed, digests verified) → run begin → plan create → `verify run` (a REAL `cargo test` observation with CAS `stdout_ref`/`stderr_ref`) → evidence create (twice; second targets a declared concept) → settlement register → ledger export/verify. Plus the negative captures: `ingest --spool` (pending durability, exit 75), the REAL F-19b refusal (`evidence create` exit 3 with the "spooled … not committed (AC-DAEMON-003/F-19b)" stderr), and an unenrolled-repo query refusal.
- **Side effects verified by observation, not just source reading**: `ledger export --repo <unknown>` exits 0 and CREATES `$XDG_DATA_HOME/sxr/repos/<id>/ledger.db` (storage initialization); `query evidence` calls `projections::rebuild` under the writer lock. Both are classified in `SXR_SIDE_EFFECT_CLASSES`.
- **DEC-005 Correction Round 1 requalification (all four T14-triggered items)**:
  1. **Both-mode object carrier QUALIFIED in this adapter's own parser**: on the real capture, top-level `representation` is a JSON OBJECT; `cep_envelope.representations[0].inline_content` is the 3195-char canonical D string; `canonicalJson(top-level representation object)` is byte-identical to that string; both digest rules recompute. `parseSxrSlice0bEnvelopeOutput` parses this real shape; the test ALSO proves T14's `parseProducerEnvelopeOutput` still types it `invalid: representation_not_inline_string` — the SXR-side parser in `sxr-adapter.ts` is the qualified path; `domainforge-adapter.ts` was not edited.
  2. **CAS `content_ref` carriage QUALIFIED**: a real model whose D exceeds the 64 KiB inline threshold (pinned `big-model.sea`, 1.2 MB both-mode capture, `content_ref: cas://sha256:4894a86b…`, model-valid) was captured and is re-emitted live by the gated real-binary test; deterministic tests cover producer-carried materialization, CAS-store materialization (re-verified after read), expired, missing object, corrupt object, and address/hash mismatch — all typed, never substituted.
  3. **Real Relation-declaration fixture CAPTURED**: `logistics-relations.sea` (declared Roles + `Relation` with subject/predicate/object/via) → real producer capture, model-valid; the envelope carries a real `relation` declaration with exact role entity refs and `predicate: "supplies"`; projection derives the `semantic_relation:supplies` edge with both endpoints in-model. T14's gap 3 is closed for `relation`.
  4. **SXR live qualification DONE**: the gated live tests run the REAL pinned sxr binary through `executeSxrRefresh` with the real `bunSxrProcessRunner` (ledger verify round-trip; empty fresh-repo export; unenrolled-repo typed refusal) and the REAL producer binary for the large CAS carriage. Digest checks gate them: a changed binary flips them to visible skips (the requalification signal), per prereg R1 — R1 is NOT triggered on this host because qualification succeeded.

## Settled requirements

- **REQ-SXR-001** (committed evidence + ledger lens, explicit binds): the lens projects the REAL export into typed records, each carrying its ledger ordinal + entry hash as branded identities; unknowns/omissions preserved; entity binds are EXACT `kind:id` refs (`admitSxrUnitBindings`); a declaration NAME is refused with the no-name-join message (names from the snapshot's inline D feed only the refusal text, never a bind); `sxrVerificationPath` renders declaration → Unit → question → threshold → evidence → SXR settlement with per-step ledger bindings, and records missing links instead of inventing them.
- **REQ-SXR-002** (source-typed, no imported authority): `SxrSettlementStatus` is SXR's own typed channel — a settlement record is NOT offerable evidence (refused by construction); the only route to Gauntlet evidence is `offerSxrEvidenceAsGauntletEvidence` → `SxrEvidenceAdmissionGate.admit` consuming a `CanonicalEvidenceAdmissionDecision` authored by the canonical APIs (no factory here; decision-mismatch, canonical refusal, and double-admission all typed); `authoritativeSxrEvidenceFrame` is the only writer path and refuses non-admitted decisions.
- **REQ-SXR-003** (bounded refresh, side effects classified): refresh executes ONLY via `executeSxrRefresh` (async — the render loop never calls it) over the `SxrProcessRunner` port; argv bound as an ARRAY from a CLOSED read-only vocabulary (`ledger export | ledger verify | query evidence`) — no ledger-appending or run-launching command exists in the vocabulary at all; program path anchored/absolute/metacharacter-free; XDG workspace selection validated; deadline (default 10 s) and output cap (default 32 MiB) enforced in `bunSxrProcessRunner` with SIGTERM→SIGKILL; hung/flooded producers are killed and their partial output is DISCARDED, never parsed.
- **REQ-SXR-004** (real composition + negative matrix): the real producer+consumer+workbench path ran for real (above); the four binding modes (Gauntlet-only / DF-only / SXR-only / combined) run through the workbench with core journeys preserved and losses explicit; the negative matrix (corrupt digest, missing/expired CAS, swapped repository, pending evidence, unenrolled repo, hung/flooded producer) yields typed incompatible/invalid/unknown/refused results with zero side effects; the deny-matrix re-run over the refresh route shows the bounded path as the ALLOWED route and the gateway action path refusing the same logical refresh with the canonical `not_action_admitted` typed denial (zero admission attempts, matching T09 C4).
- **REQ-DOM-004/005 (T14 requalification obligations)**: schema validity, digest integrity, and diagnostics remain separate results — the REAL invalid-with-D capture (exit 1, D constructed, diagnostics present) parses with `model_valid: false` and projects as typed-absent `model_invalid`; diagnostics never appear conformant. The declaration→verification path renders as inspectable DSN-008 records; nothing here computes Gauntlet authority.
- **REQ-SYNC-003** (source loss): with every source byte lost, the lenses render `ABSENT (no_model_output)` / `ABSENT (no_ledger_export)` with "recorded state is not recreated"; the admitted consequential evidence record RECONSTRUCTS from the authoritative log (`rebuildFromAuthoritativeLog`); live query output is labelled unavailable, never recreated.

## Composition matrix (prereg replay) — what each combination proves

All four modes run the seeded workbench (real WorkbenchModel + views): Work rows, Evidence record objects, Trace history, relation derivation (4 edges) + cycle-safe traversal, and identical snapshot counters (3 units / 3 evidence / 2 sessions) — the optional providers never displace mandatory closure. Absent providers render typed absent sections (never fabricated); combined mode renders both lenses beside each other with distinct FocusRef sources (`domainforge` vs `sxr` — the same identity string in both worlds stays two identities).

## Attack teeth (prereg falsifiers — each defense was broken and the suite bit)

1. **Carrier binding cross-check disabled** (`carriage.mode === "inline_content_string" && … !== rep.content_hash` → `false && …`) → BOTH `catches a drifted envelope-side content_hash even when D's internal digests verify` and `object-carrier bytes are cross-checked…` FAIL. (A first attempt at disabling the object-carrier branch was caught by defense-in-depth — the delegated strict parser's internal digest check — so the sharper envelope-side-drift test was added; the tooth then bit twice.) Restored.
2. **Pending evidence accepted** (`pending_durability_observed === true` → `false && …`) → `refuses pending evidence — the F-19b durability state can never be offered as settled` FAILS (prereg P1). Restored.
3. **Deadline outcome falsified** (deadline_exceeded remapped to `refreshed` with a salvaged empty parse) → `kills a hung producer at the deadline; partial output is DISCARDED, never parsed` FAILS (prereg P2). (An intermediate "salvage-then-fall-through" mutation changed nothing observable — the invalid-JSON fallback held; the falsified-outcome form proves the assertion bites.) Restored.
4. **Cross-repository offers allowed** (repo-mismatch guard → `false && …`) → `refuses offers across repositories` FAILS. Restored.
5. **Name-based join admitted** (`!knownRefs.has(...) && !knownNames.has(...)` binds by name) → `admits EXACT entity-ref bindings and refuses label joins…` FAILS. Restored.

After restoration `sha256sum` confirmed `sxr-adapter.ts` byte-exact (`5f7c650558d314caf6ea9d7618ede026d89049497f377b9bf9bcf7cc11c27674`), and the full gate battery was re-run green.

## Verbatim gates (all exit 0, final state after restoration)

```
just tui-test sxr-evidence-adapter             → 53 pass, 0 fail, 290 expect() calls    [exit 0]
just tui-test semantic-evidence-composition    → 10 pass, 0 fail, 94 expect() calls     [exit 0]
just tui-test deny-bypass                      → 16 pass, 0 fail, 293 expect() calls    [exit 0]
just tui-check                                 → tsc --noEmit, no errors                [exit 0]
just tui-lint                                  → Checked 90 files, no errors            [exit 0]
just tui-preflight bootstrap                   → PASS (deps pinned, lockfile in sync, audit clean, spec/DESIGN digests match) [exit 0]
cd workbench && bun test                       → 450 pass, 1 skip, 0 fail, 5389 expect() calls, 43 files [exit 0]
```

The 1 skip is T13's env-gated real-PTY test (`GAUNTLET_T13_REAL_PTY=1`) — the documented baseline, not this task's. Both T14B live-binary blocks RAN on this host (real sxr round-trip; real 1.2 MB CAS capture). The 450 includes the parallel wave's suites (T17/T18 files landed during this task).

## Implementation files (owned)

- `workbench/src/sxr-adapter.ts` — pinned contract (`PINNED_SXR_OUTPUT_VERSION "1"`, `QUALIFIED_SXR_BUILDS`, `SXR_LIVE_QUALIFICATION`, pinned §5.3b record-kind vocabulary); branded SXR identity domains (`SxrEntityRef`/`SxrOrdinal`/`SxrEntryHash` with runtime chokepoints); strict typed parsers (`parseSxrCommandOutput`, `parseSxrLedgerExport` — the separate raw-array contract with ordinal-density checking and per-row export omissions, `parseSxrSlice0bEnvelopeOutput` — the real object carrier + CAS carriage, delegating D parsing to T14's strict parser); bounded refresh (`SXR_SIDE_EFFECT_CLASSES`, `validateSxrRefreshRequest`, `SxrProcessRunner` port, `bunSxrProcessRunner`, `executeSxrRefresh`); evidence lens (`projectSxrEvidenceLens`, `admitSxrUnitBindings`, `sxrVerificationPath`); admission boundary (`offerSxrEvidenceAsGauntletEvidence`, `SxrEvidenceAdmissionGate`, `authoritativeSxrEvidenceFrame`); Inspector renderer (`renderSxrEvidenceSection`).
- `workbench/src/sxr-evidence-adapter.test.ts` (53 tests) — REAL-capture parsing/round-trip, pending-vs-committed typing, separate-contract refusals, corrupt digest / bad hash / unknown kind / non-dense ordinals → typed invalid, both-mode object carrier + T14 fail-closed proof, envelope-side digest drift, diagnostics preservation (real exit-1 capture), CAS matrix, hung/flooded producer (fake clock/runner) with discarded partial output, REAL F-19b stderr as typed producer_refused, lens projection + no-name-join + explicit missing links, admission refusals (pending/unknown/cross-repo/settlement-not-evidence), canonical decision discipline + authoritative frame into `SqliteProjectionStore`, rendering, gated live-binary blocks, qualification-record honesty.
- `workbench/src/semantic-evidence-composition.test.ts` (10 tests) — the four-mode composition matrix, explicit-loss rendering, distinct source identities, admission composition into `WorkbenchModel`, source-loss preservation with authoritative-log reconstruction, deny-matrix re-run over the refresh route (one allowed + one denied with the same typed canonical denial).
- Evidence: this directory — `sxr-evidence.md`, `CAPTURES.sha256`, `captures/` (22 real capture files + the three pinned `.sea` fixtures).

No new dependencies; TypeScript strict (`exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`, `noPropertyAccessFromIndexSignature` honored); deterministic suites spawn no processes (live blocks are digest-gated and visibly skip elsewhere); no commits made; no files outside the owned set modified (status anchors, prereg, frozen documents, T14/T17/T18 files untouched).

## Needed index exports (not applied — `workbench/src/index.ts` is outside T14B's owned set)

```ts
export * from "./sxr-adapter";
```

## Inspector/Evidence/Trace wiring note (for the orchestrator)

The lens is data-shaped per the plan: `renderSxrEvidenceSection(projection, paths)` produces an `InspectorSection` (`id: "sxr-evidence"`, title `REALITY TRACE`) whose records are DSN-008 record objects, composable into T12's `renderInspector` section list the same way T14's `renderSemanticSection` is. Wiring it into the live Inspector view requires a one-line section append in `inspector.ts` (read-only for this task) once a snapshot carries the projected lens; no second approval button exists on the path — the lens renders, the admission boundary alone writes.

## Honest gaps / limitations

1. **Canonical-JSON floats**: inherited from the pinned digest rule (T14 limitation) — pinned captures carry only integers/strings; non-integer floats in producer payloads could serialize differently between serde_json and ECMAScript and break digest recomputation. Not exercised.
2. **Ledger chain linkage is not verifiable from the export**: `sxr ledger export` rows omit `payload_hash`/`previous_entry_hash`/`entry_ulid`/`committed_at` etc. The parser checks ordinal density (the part the export CAN check) and records the omitted fields per row as typed omissions; full chain verification remains producer-side (`sxr ledger verify` — included in the refresh vocabulary and exercised live).
3. **`role_binding` declaration fixtures**: no local `.sea` fixture emits a `role_binding` declaration on the slice0b grammar; the relation kind (which T14 projects via `subject_role`/`object_role`) is now covered by a REAL capture, but role_binding projection remains exercised only structurally. `SXR_LIVE_QUALIFICATION.role_binding_declaration_fixture_real` records this as `false`.
4. **Live hang through the REAL binary is not producible**: the closed read-only argv vocabulary cannot command the pinned binary to hang, so the hang/deadline test is fake-runner + fake-clock; the deadline MECHANISM in `bunSxrProcessRunner` is exercised live only for the happy path. The plan's "hung producer" tooth is therefore proven at the outcome-contract level (tooth 3 bites on the real assertion).
5. **Lens→view wiring is data-shaped, not surfaced** (see wiring note) — composition is proven at the Inspector-contract data level; `inspector.ts` was read-only for this task.
6. **Query-evidence presence states** (`not_in_declared_graph` / `not_observed` / `observed`) are parsed and pinned from real captures, but the lens currently builds paths from ledger records; wiring live query results into the path view is integration work for the orchestrator's index/wiring pass.

## Trigger-records for the orchestrator (not fixed — outside T14B's owned set)

1. **`relations.ts` `traverseRelations` ignores `edge.valid`** (carried from T14 Correction Round 1 MINOR-2, re-confirmed against current source): a `valid: false` edge (e.g. a cross-source or out-of-model endpoint) is traversed like any other, so `semanticNeighborhood`/horizon walks can reach out-of-model endpoints. Fix belongs in `relations.ts` (filter on `edge.valid` in the semantic traversal, or document why traversal includes invalid edges).
2. **`relations.ts` parallel-edge record ids can collide**: T14's `renderSemanticSection` builds relation record ids as `semantic-relation:<kind>:<source>:<target>` with no disambiguator (T14 MINOR-3, cosmetic, record-only) — surfaced again while composing sections in the combined mode.
3. **Inspector wiring**: `workbench/src/index.ts` needs `export * from "./sxr-adapter";` and the Inspector needs the one-line section append described above (both orchestrator-owned).

## What the independent confirmer should attack

- Rebuild either pinned binary and re-run: the digest-gated live blocks must flip to visible skips (and the digest assertions to failures) — that IS the requalification signal, not a defect.
- Construct any path by which pending (`spooled`/`queued_for_commit`) SXR state reaches an offer, an admitted record, or a rendered "settled" field; the refusal must hold from every entry point.
- Craft a both-mode capture whose top-level object and inline D agree with each other but NOT with `representations[0].content_hash` (the drift test's shape) — the carrier cross-check must type it invalid.
- Drive `executeSxrRefresh` with an argv string (not an array), a relative binary path, a shell-metacharacter path, or a `settlement register`-shaped command — the boundary must refuse or be unable to express it.
- Check whether `parseSxrLedgerExport`'s unknown-kind preservation (`kind_known: false`) can smuggle an interpreted lens record out of a future-kinds ledger.

Confirmation: BUILDER evidence pending independent confirmation.

## Correction Round 1 (independent confirmation findings)

Independent verdict on the round above: CONFIRM_COMPOSITION with MAJOR-1 (dead swapped-repository guard). This round closes MAJOR-1 and two minors RED-first in `sxr-adapter.ts`; the remaining findings are recorded as trigger-records only.

1. **MAJOR-1 CLOSED — swapped-repository guard was dead code for non-empty exports.** The confirmed probe reproduced exactly: a lens projected with a fabricated `repo_id` over the real export rows minus every `run.started` row BUILT an offer carrying the fabricated `source.repo_id` (bounded impact, as the confirmation noted — an offer establishes nothing by type and still requires a canonical decision — but the guard was dead and the bypass real). RED-first regression added (`MAJOR-1 regression: refuses an offer whose lens repo disagrees with the export ledger even when the export carries NO run rows`) and shown failing against the old code before the fix. Fix: the dead `rows.length === 0` condition is DELETED and replaced by two live ledger-provenance guards — (a) a non-empty export whose ledger ids never name the lens repo is refused `repository_mismatch` (names the offending ids in the message); (b) the offered record's OWN row must be a ledger record of the lens repo (`record.row.ledger_id !== input.lens.repo_id` → refused), so no path can build an offer whose lens repo disagrees with the export's ledger provenance. The runs-payload guard is retained as belt-and-braces. The companion test pins the legitimate same-repo offer still building without run rows, carrying the true ledger id and ordinal 13 in the locator.

2. **MINOR-2 CLOSED — absent XDG variables are now omitted entirely.** The old code could set `XDG_DATA_HOME: ""` when only `xdg_state_home` was provided; per the XDG spec an empty value means unset, silently redirecting the child to `$HOME/.local/share`. RED-first regression (`MINOR-2 regression: absent XDG variables are omitted entirely…`) shown failing, then fixed: each variable is set only when its validated input is present.

3. **MINOR-3 CLOSED — runtime-forged command kinds are a typed refusal.** A forged `command` shape (e.g. `{kind: "settlement_register"}` cast through the boundary) previously fell through the argv switch to a silent empty-argv spawn. RED-first regression shown failing, then fixed: a `default` arm throws `SxrRefreshBoundaryError` with the new closed-vocabulary code `command_not_in_contract` BEFORE any runner invocation can occur.

Trigger-records (recorded, NOT fixed — orchestrator/integration scope):

- **MINOR-1 (deadline kill vs pipe holders)**: the deadline kill resolves only when every pipe holder exits — a grandchild that inherits the child's stdout can delay `bunSxrProcessRunner`'s outcome resolution past the deadline. The typed outcome / no-partial-parse contract HELD in every probe; the residual risk is outcome latency, not misclassification. A process-group kill (setsid + `kill(-pgid)`) is the noted direction if a producer is ever added that spawns children; not implementable through the closed read-only vocabulary today.
- **INFORMATIONAL (a)**: `SxrEvidenceAdmissionGate` keys by `offer_id`, so one evidence record CAN be admitted to two units via two distinct offers — cross-unit dedupe belongs to the canonical Gauntlet evidence APIs, which author the decisions this gate consumes; the workbench-side gate deliberately holds no authority to refuse what the canonical side admitted.
- **INFORMATIONAL (b)**: `offerCounter` is module-global, so `offer_id`s are unique per process but not deterministic across processes. Offer ids are workbench-local bookkeeping, never evidence identity (the admission contract's `evidence_id` carries the repo + ledger record identity); noted in case a future consumer expects replay-stable offer ids.

Post-fix verification (RED → GREEN order preserved): the three new regressions failed against the pre-fix code (MAJOR-1 probe built the offer; MINOR-2/3 tests failed; `tsc` additionally failed on the not-yet-existing refusal code), and all pass after the fix. Verbatim gate battery after the fixes (captured during a verified-clean window of the parallel wave):

```
just tui-test sxr-evidence-adapter             → 57 pass, 0 fail, 304 expect() calls    [exit 0]
just tui-test semantic-evidence-composition    → 10 pass, 0 fail, 94 expect() calls     [exit 0]
just tui-test deny-bypass                      → 16 pass, 0 fail, 293 expect() calls    [exit 0]
just tui-check                                 → tsc --noEmit, no errors                [exit 0]
just tui-lint                                  → Checked 90 files, no errors            [exit 0]
just tui-preflight bootstrap                   → PASS                                   [exit 0]
cd workbench && bun test                       → 468 pass, 1 skip, 0 fail, 5517 expect() calls, 43 files [exit 0]
```

The 1 skip remains T13's env-gated real-PTY test (`GAUNTLET_T13_REAL_PTY=1`). The full-workbench count grew from 450 to 468 because T18's suites landed in the tree during this round (468 = this task's 67 + the parallel wave's suites); the required 451+/1 gated skip/0 fail holds. During one window `tui-check`/`tui-lint` transiently failed ONLY in T18's in-flight `security-boundaries.test.ts`/`unsafe-input-boundaries.test.ts` (attributed via per-file error output; this task's files were clean throughout) — after the documented retry the wave settled and both gates passed with zero errors. Post-fix digests: `sxr-adapter.ts` sha256 `51829e27365bb5385ee826e98ec7c0d20727d608962905109cf9489f9786a7c3`; `sxr-evidence-adapter.test.ts` sha256 `9695466e6362084486dabe13e24f9d268b8ef50083bcc9a191c77c18ea83eb32`. No commits made, no files outside the owned set touched.

Confirmation: BUILDER evidence pending independent confirmation.
