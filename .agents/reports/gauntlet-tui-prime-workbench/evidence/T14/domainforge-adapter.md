# T14 Evidence — DomainForge/SEA Semantic Enrichment Adapter

Date: 2026-09-09
Task: T14
Proof Level: P2 (confirmation: peer)
Scope: Typed parser for the pinned DomainForge canonical semantic envelope (`domainforge-semantic-envelope/v1`), three distinct typed identity domains (Bun cache hash / file hash / semantic fingerprint), projection of semantic concepts and graph-derived relations into the existing FocusRef/Relation/horizon seams with one source identity across placements, explicit Unit bindings without name joins, explicit typed enrichment-absent state, and an explicit producer/protocol compatibility record (DEC-005).
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T14.prereg.yaml` — verified this session via `just prereg-check gauntlet-tui-prime-workbench-T14` (sha256 `c421a2c87f91d2d7a9e8950eaa9c8a1c458b5f9bbab625d1bcc50dde5d3b51f4`), never modified.

## Claim (preregistered)

Cases with semantic models gain graph-derived relations and reusable semantic placements without reidentifying concepts; Cases without DomainForge continue through the same workbench with enrichment absent rather than faked.

## Producer qualification — locally verified vs documented

The hot_context sibling repos exist on this machine and were inspected; the producer is NOT binary-digest-absent. Verified facts (2026-09-09, builder workstation):

- **Three distinct `domainforge 0.16.0` builds**, each exercised directly:
  1. `~/.cargo/bin/domainforge` (sha256 `ee3ee077…`): has NO `envelope` subcommand (`envelope --help` → exit 2, "unrecognized subcommand").
  2. `domainforge/target/release/domainforge` (sha256 `48a4ed25…`): `domainforge envelope <entry.sea>` → the bare canonical semantic envelope document D on stdout, **byte-identical across two runs** (verified with `cmp`); parse failure → empty stdout, stderr message, exit 1.
  3. `domainforge/target/debug/domainforge` (working-tree build, sha256 `d5996985…`): the new CLI (`--emit representation|cep|both|verification-contract`, `--pack`, `--scope`); its `--emit representation` output is **byte-identical** to build 2's default output (sha256 `a4cba842…` for the `valid-basic.sea` fixture).
- **Digest algorithm verified end-to-end**: `self_hash = sha256(canonical_json(document minus self_hash))` and `semantic_closure_hash = sha256(canonical_json(envelope))`, where canonical JSON is recursively key-sorted compact serialization. Recomputation in both Python and TypeScript matched the producer's declared hashes on the real capture before anything was pinned.
- **DEC-005 confirmed and made precise**: the SXR Slice 0B consumer invokes `envelope --emit both <entry>` (`sxr/sxr-df/src/adapter.rs` argv construction). The installed binary rejects the subcommand and the release build rejects `--emit` (both exit 2, observed). Only the uncommitted working-tree build answers it. The pair is therefore recorded INCOMPATIBLE (installed, release) and UNQUALIFIED (working-tree build) — SXR requalification is T14B/T20 work, as DEC-005 requires.
- **Fixtures are real captures, not inventions**: the test fixture is the byte-exact `valid-basic.sea` envelope; the declaration entity refs asserted in tests are the producer's own `preserved_entities` from its `--emit both` CEP representation; the failure fixture is the real `--emit cep` output for `invalid-syntax.sea`.

Documented-but-not-exercised (honest limitations): CEP `content_ref` CAS carriage (>64 KiB models) is not materialized by this adapter (T14B scope — the both-mode parser requires inline D); a relation-declaration fixture from a real capture was unavailable (no local `.sea` fixture declares `Relation`), so relation/role-binding projection is tested against documents matching the producer's serde shapes (`deny_unknown_fields`, stable field names) rather than a captured example.

## Settled Requirements

- **REQ-DOM-001** (enrichment used when bound): `projectSemanticEnrichment` derives relations (flows, resolved references; relation/role-binding shims included) and concept placements from the bound model; `semanticContextCandidates`/`semanticNeighborhood` feed horizon-local context construction as OPTIONAL inputs (composition with `planMaterialization` proven in tests; mandatory closure never displaced).
- **REQ-DOM-002** (identity domains distinct): `BunCacheHash` / `FileContentHash` / `SemanticFingerprint` are branded opaque types with module-private brand symbols plus per-domain WeakSet registries; each `require*` chokepoint refuses foreign-domain values and forged objects at runtime, and `@ts-expect-error`-marked substitutions fail compilation (Attack 1, prereg P1 — identical-looking labels stay three identities).
- **REQ-DOM-003** (one source identity across placements): each declaration gets exactly one interned `FocusRef(source="domainforge", kind=<declaration kind>, source_id=<exact producer entity ref>)`; semantic edges resolve endpoints through the same intern map, so an edge endpoint IS the concept's FocusRef (`toBe` in tests); two placements of one concept are referentially identical.
- **REQ-DOM-004** (explicit qualified pair, separate results): the adapter pins pair P1 (release 0.16.0 bare-D document; digests and binary sha256s recorded in `QUALIFIED_PRODUCER_BUILDS`, `PROTOCOL_COMPATIBILITY` P1–P4 with `locally_verified` flags and observed exit-2 evidence); schema validity (`schema_violation`, closed key sets), digest integrity (`self_hash_mismatch` / `semantic_closure_hash_mismatch`), and semantic diagnostics travel as separate results — output carrying diagnostics is `model_valid: false` and is never projected as conformant; unknown producer → typed `unknown`; wrong schema version / CEP wrapper → typed `incompatible` with declared version surfaced; no implicit flag fallback (`selectProtocolPairs` returns recorded pairs only).
- **REQ-DOM-005** (bound semantic Focus content): `renderSemanticSection` produces Inspector-contract shapes (`RenderedLine`/`RenderedRecord` from `render-tree.ts`) showing producer/model identity, semantic fingerprint, pack and source-set digests, declarations with source module (logical path — line positions are a recorded producer omission, never invented), preserved diagnostics, and explicit Unit bindings (`declared_by` required). Bindings are admitted only by exact producer entity ref; a declaration NAME is refused with a no-name-join message; non-WorkItem targets and unknown refs are refused (`SemanticBindingError`). Semantic authority information never substitutes for Gauntlet admission — the adapter computes no authority and owns no truth.

## Attack 2 (prereg P2) — enrichment absent is honest

`projectSemanticEnrichment(undefined | invalid | incompatible | unknown | model_invalid)` returns a typed `absent` state (`no_model_output`, `parse_invalid`, `producer_incompatible`, `producer_unknown`, `model_invalid`); `semanticContextCandidates(absent)` is empty; the same Case's `planMaterialization` counters are identical with enrichment absent; `renderSemanticSection(absent)` renders "ABSENT (<reason>)" plus "no semantic relations are shown because none exist" and emits zero relation records. Nothing is fabricated.

## Verbatim gates (all exit 0, final state after restoration)

```
just tui-test domainforge-adapter            → 26 pass, 0 fail, 107 expect() calls   [exit 0]
just tui-test semantic-identity              → 6 pass, 0 fail, 23 expect() calls     [exit 0]
just tui-test semantic-protocol-compatibility→ 7 pass, 0 fail, 38 expect() calls     [exit 0]
just tui-check                               → tsc --noEmit, no errors               [exit 0]
just tui-lint                                → Checked 80 files, no errors           [exit 0]
just tui-preflight bootstrap                 → PASS (deps pinned, lockfile in sync, audit clean, spec/DESIGN digests match) [exit 0]
cd workbench && bun test                     → 323 tests, 38 files, 0 fail           [exit 0]
```

(323 includes this task's 39 tests plus concurrent builders' suites; the "187 existing" baseline grew during the parallel wave.)

## Teeth (mutation checks — each defense was broken and the suite bit)

1. **Digest verification disabled** (`if (false && recomputedSelf !== …)`) → `corrupt digest is a typed invalid` FAILS (25 pass / 1 fail). Restored.
2. **Identity registry bypass** (`assertDomain` check short-circuited) → both runtime substitution tests FAIL (identical-labels attack; smuggled-forgery attack). Restored.
3. **Name-based join allowed** (binding falls back to declaration-name match) → `refuses a binding by declaration NAME — no name-based joins` FAILS. Restored.

After restoration `sha256sum -c` confirmed the adapter file byte-exact; the full gate battery was re-run green post-restoration.

## Implementation files (owned)

- `workbench/src/domainforge-adapter.ts` — pinned constants and producer qualification; typed identity domains; canonical JSON + digest rule; strict parser (`parseSemanticEnvelopeDocument`, `parseProducerEnvelopeOutput`); `admitUnitBindings`; `projectSemanticEnrichment`; `semanticContextCandidates` / `semanticNeighborhood`; `renderSemanticSection`; `PROTOCOL_COMPATIBILITY` / `selectProtocolPairs`.
- `workbench/src/domainforge-adapter.test.ts` (26 tests) — real-capture parse/digest/round-trip, declaration identities vs producer `preserved_entities`, corrupt digest, wrong version, CEP wrapper, unknown producer, closed-schema violation, both-mode diagnostics preservation, bindings (admit + three refusals), one-identity-two-placements, graph relations with provenance, neighborhood traversal, absent-state honesty, optional candidate composition.
- `workbench/src/semantic-identity.test.ts` (6 tests) — Attack 1: identical labels across three domains; six cross-domain refusals (compile + runtime); smuggled forgery refused; digest-format validation; parser mints only a semantic fingerprint.
- `workbench/src/semantic-protocol-compatibility.test.ts` (7 tests) — DEC-005 as recorded facts: Slice 0B pairs incompatible (verified evidence, exit 2), no SXR pair pinned, working-tree pair unqualified with T14B pointer, CEP payload typed incompatible, unknown producer typed unknown, digests recorded for requalification.

No new dependencies; TypeScript strict (exactOptionalPropertyTypes, noUncheckedIndexedAccess, noPropertyAccessFromIndexSignature honored); tests deterministic (no process spawn, no clock, no rng). No commits made; no files outside the owned set edited (status anchors, prereg, other tasks' files untouched).

## Needed index exports (not applied — `workbench/src/index.ts` is outside T14's owned set)

```ts
export * from "./domainforge-adapter";
```

## Honest gaps / limitations for the peer and T14B/T20

1. **CAS carriage**: `parseProducerEnvelopeOutput` requires inline D (`representation` string); `content_ref` (>64 KiB models) materialization is not implemented — recorded as invalid with T14B scope rather than silently skipped.
2. **Canonical-JSON floats**: the pinned captures contain only integers/strings; producer annotation payloads with non-integer floats could serialize differently between serde_json and ECMAScript and break digest recomputation. Recorded in code docs; not exercised.
3. **Relation-declaration fixtures**: no local `.sea` fixture declares `Relation`; relation/role-binding projection follows the producer's serde struct shapes but lacks a captured example.
4. **Enrichment→Inspector wiring**: the section renderer produces Inspector-contract data, but no T12 `renderInspector` section calls it yet (inspector.ts is read-only for this task) — composition is proven at the data-shape level, not by editing T12's surface.
5. **SXR pairs P2–P4 are records of observed facts, not integrations**: this adapter spawns no processes by design; live-binary qualification is T14B.
6. **`resolved_references`-derived edges** (`semantic_reference:<field_path>`) follow the schema; the local fixtures' `resolved_references` arrays were empty, so those edges are exercised only structurally via projection of the pinned shapes.

## What the peer should attack

- Feed a hand-crafted document whose digests verify but whose declaration payload shapes are wrong for their kind (deep-structure checks are intentionally validated deeply only for projected kinds — flow/relation/role_binding field extraction is defensive at projection time; argue whether malformed payload data should be a parse-invalid instead of an edge skip).
- Try to construct a binding that joins by name through any public API (the refusal must hold), or to get enrichment from a `model_valid: false` parse.
- Attempt cross-domain identity substitution through `unknown`/casts (WeakSet must refuse) and check whether removing any `@ts-expect-error` in `semantic-identity.test.ts` turns a compile refusal into a compile PASS (it must fail `tui-check` if the domains ever unify).
- Verify the fixture digests against a freshly emitted envelope from a re-built DomainForge binary (the pinned shape should hold; a divergence is exactly the requalification signal T14B exists for).

Confirmation: BUILDER evidence pending peer confirmation.

## Correction Round 1 (peer findings)

Peer verdict on the round above: CONFIRM with MAJOR-1 (evidence mis-scoping). This round is evidence-honesty only — no parser or test changes were made.

1. **MAJOR-1 correction — both-mode carrier framing.** Gap 1 above framed the carrier limitation as "CAS carriage (>64 KiB models)", implying small models carry inline D in the both-mode wrapper. That framing is factually wrong for the current producer and is corrected: the REAL working-tree build (the only build answering the SXR Slice 0B invocation) serializes the both-mode wrapper's top-level `representation` as a JSON OBJECT unconditionally — `domainforge-core/src/cli/envelope.rs:288-294` parses the canonical D string into a `Value` and re-serializes it inside `{"representation": …, "cep_envelope": …}`; the canonical string actually lives at `cep_envelope.representations[0].inline_content`. Re-verified this session against the real capture (`--emit both` on `valid-basic.sea`): top-level `representation` is an object; `representations[0].inline_content` is the 3195-char canonical D string. Consequence, stated plainly: `parseProducerEnvelopeOutput` types the real working-tree build's both-mode carrier `invalid: representation_not_inline_string` today — fail-closed, never fake-conformant — and small models do NOT carry inline D on this build. The settled claim rests on the pinned P1 pair (installed + release builds' default-mode bare-D stdout), whose parser path is digest-verified against real captures; it does not rest on the both-mode carrier. The synthesized both-mode fixtures in `domainforge-adapter.test.ts` (`{"representation": <string>, "cep_envelope": …}` carriers) are shapes no real build produces — already disclosed as synthetic in the test comments; they remain diagnostic fixtures for the diagnostics-preservation channel, not evidence about a real carrier.

2. **Trigger-record for T14B (recorded verbatim-fair):** "T14B requalification must cover the both-mode object-carrier contract drift, the CAS content_ref carriage, a real Relation-declaration fixture capture, Inspector section wiring, and SXR live qualification; the synthesized both-mode test fixtures are synthetic shapes, disclosed as such."

3. **Peer MINORs, recorded for T14B/integration (no code change this round):**
   - MINOR-2: `semanticNeighborhood` does not filter `valid: false` edges — out-of-model endpoints can enter a semantic neighborhood and could kind-fork if a later model declares them; fix during T14B/integration by filtering on `valid` in the semantic traversal.
   - MINOR-3: parallel-edge relation record ids can collide (`semantic-relation:<kind>:<source>:<target>` carries no disambiguator) — cosmetic, record-only.
   - MINOR-4: fixture phrasing — the embedded document fixture is byte-exact compact canonical object content of the captured document, not byte-exact stdout (the producer pretty-prints stdout; canonical JSON is the identity-preserving form the digests are defined over).

Re-verification after this evidence-only round (verbatim):

```
just tui-test domainforge-adapter             → 26 pass, 0 fail, 107 expect() calls   [exit 0]
just tui-test semantic-identity               → 6 pass, 0 fail, 23 expect() calls     [exit 0]
just tui-test semantic-protocol-compatibility → 7 pass, 0 fail, 38 expect() calls     [exit 0]
just tui-check                                → tsc --noEmit, no errors               [exit 0]
cd workbench && bun test                      → 322 pass, 1 gated skip, 0 fail        [exit 0]
```

Confirmation: BUILDER evidence pending peer confirmation.
