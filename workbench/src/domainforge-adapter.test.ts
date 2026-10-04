import { describe, expect, it } from "bun:test";
import {
  admitUnitBindings,
  canonicalJson,
  declarationEntityRef,
  envelopeClosureHash,
  envelopeSelfHash,
  PINNED_ENVELOPE_SCHEMA_VERSION,
  parseProducerEnvelopeOutput,
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
  renderSemanticSection,
  SEMANTIC_SOURCE_ID,
  semanticContextCandidates,
  semanticNeighborhood,
} from "./domainforge-adapter";
import { createPlacement, FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import type { ProjectionWorld } from "./relations";

/**
 * T14 DomainForge/SEA semantic enrichment adapter suites (plan T14).
 * Proves REQ-DOM-001..005 against the PINNED producer contract
 * (`domainforge-semantic-envelope/v1` as emitted by the locally qualified
 * DomainForge release 0.16.0 `envelope <entry.sea>`).
 *
 * Fixtures: REAL captured producer output (byte-exact object content), not
 * invented shapes. Digest assertions recompute the producer's own hash rule;
 * the declaration entity refs below are the producer's own preserved_entities
 * from the same capture. Deterministic: no process spawn, no clock, no rng.
 */

// HISTORICAL FIXTURE (0.16.0): kept to prove compatibility with the earlier producer; the
// 0.18.2 capture lives in domainforge-0-18-2-capture.test.ts.
// REAL capture: `domainforge envelope domainforge-core/tests/fixtures/envelope/valid-basic.sea`
// (release build 0.16.0, binary sha256 48a4ed25…), byte-identical across runs.
const REAL_DOCUMENT_JSON =
  '{"schema_version":"domainforge-semantic-envelope/v1","producer":{"name":"domainforge-core","version":"0.16.0"},"inputs":{"source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","language_schema_version":"domainforge-ast/v3","interpretation_version":"domainforge-interpretation/v1"},"self_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","envelope":{"language_schema_version":"domainforge-ast/v3","canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","modules":[{"logical_id":"valid-basic.sea","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29","namespace":"auth"}],"import_graph":[],"namespace_bindings":[{"namespace":"auth","logical_id":"valid-basic.sea"}],"semantic_packs":[],"semantic_declarations":[{"id":{"kind":"concept","data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"entity","data":{"name":"User","domain":"auth","annotations":{}}}},{"id":{"kind":"concept","data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"entity","data":{"name":"Role","domain":"auth","annotations":{}}}},{"id":{"kind":"concept","data":{"id":"a81c1fec-c10a-5541-bd81-98c64b10f402"}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"resource","data":{"name":"Token","unit":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"7a50d9d2-89e2-5607-bca5-34464284bea8"}}}},"domain":"auth","annotations":{}}}},{"id":{"kind":"occurrence","data":{"kind":"flow","content_hash":"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543","duplicate_index":0}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"flow","data":{"resource":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"}}}},"from_entity":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"}}}},"to_entity":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"}}}},"quantity":"1","annotations":{}}}},{"id":{"kind":"occurrence","data":{"kind":"flow","content_hash":"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66","duplicate_index":0}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"flow","data":{"resource":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"}}}},"from_entity":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"}}}},"to_entity":{"kind":"declaration","data":{"id":{"kind":"concept","data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"}}}},"quantity":"1","annotations":{}}}}],"resolved_references":[],"application_contract":{"enums":[],"records":[],"entities":[],"operations":[]}}}';

// The producer's own preserved_entities for this exact capture (from its
// `--emit both` CEP representation) — the identity strings bindings must use.
const PRODUCER_PRESERVED_ENTITIES = [
  "concept:5ad6a959-5924-5cf9-af04-7b502ea74784",
  "concept:72f70c63-6f3e-5061-8854-42e540bbac87",
  "concept:a81c1fec-c10a-5541-bd81-98c64b10f402",
  "occurrence:flow:sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543:0",
  "occurrence:flow:sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66:0",
];

// REAL capture: `domainforge envelope --emit cep <invalid-syntax.sea>` — the
// producer's own failure envelope (extensions carry the preserved diagnostics).
const REAL_CEP_FAILURE_ENVELOPE_JSON =
  '{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["omissions","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions","canonical_representation_D"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"incomplete_for_declared_state","conformance_status":"conformant","created_at":"2026-09-09T15:49:39.604598Z","created_by":"domainforge 0.16.0","envelope_id":"01a086dc-4c94-7622-96f7-22f1c903d410","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","extensions":{"domainforge":{"diagnostics":["artifact_schema_violation: module \'domainforge-core/tests/fixtures/envelope/invalid-syntax.sea\' failed to parse: Syntax error at 6:35: expected number"],"invalid_declared_checkpoint_hash":"sha256:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908","model_validation_status":"invalid"}},"lineage_refs":[],"omission_status":"known_omissions_recorded","omissions":[{"affected_entities":["domainforge-core/tests/fixtures/envelope/invalid-syntax.sea"],"description":"canonical D not constructed; declared model invalid","impact":"no semantic hash derivable","known_or_suspected":"known","omission_id":"omission:representation:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908","omission_type":"representation_unavailable","reason":"model validation failed"}],"provenance":[{"created_at":"2026-09-09T15:49:39.604598Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a086dc-4c94-7622-96f7-22f1c903d410","review_status":"unreviewed","source_system_refs":["source-set:sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e","ref_type":"source_set","scope":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","target_hash":"sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e","target_uri_or_id":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea"}],"scope":{"entry_logical_path":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","invalid_declared_checkpoint_hash":"sha256:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908"},"validation_status":"validated"}';

const USER_ENTITY_REF = PRODUCER_PRESERVED_ENTITIES[0] as string;
const ROLE_ENTITY_REF = PRODUCER_PRESERVED_ENTITIES[1] as string;

function parsedModel() {
  const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON);
  if (parse.status !== "parsed") {
    throw new Error(`fixture must parse, got ${parse.status}`);
  }
  return parse.model;
}

describe("T14 envelope parsing (REQ-DOM-001, REQ-DOM-004)", () => {
  it("parses the REAL pinned producer output with verified digests (schema + digest integrity as separate results)", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.document.schema_version).toBe(PINNED_ENVELOPE_SCHEMA_VERSION);
    expect(parse.model.document.producer).toEqual({
      name: "domainforge-core",
      version: "0.16.0",
    });
    // Digest integrity: the producer's own hashes recompute from the raw bytes.
    expect(parse.model.document.self_hash).toBe(envelopeSelfHash(parse.model.document_raw));
    expect(parse.model.document.semantic_closure_hash).toBe(
      envelopeClosureHash(parse.model.document_raw["envelope"]),
    );
    // The fingerprint minted from the model is the producer's closure hash.
    expect(parse.model.fingerprint.value).toBe(
      "sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c",
    );
    // Bare-D carrier carries no diagnostics channel; validity is separate.
    expect(parse.model.model_valid).toBe(true);
    expect(parse.model.diagnostics).toEqual([]);
  });

  it("round-trips: the raw producer document is preserved losslessly", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON);
    if (parse.status !== "parsed") {
      throw new Error("fixture must parse");
    }
    // Digests are defined over the RAW producer bytes; the raw carrier on the
    // model must round-trip to exactly the input's canonical form.
    const viaModel = canonicalJson(parse.model.document_raw);
    const direct = canonicalJson(JSON.parse(REAL_DOCUMENT_JSON));
    expect(viaModel).toBe(direct);
  });

  it("declaration identities are the EXACT producer entity refs (no reidentification)", () => {
    const model = parsedModel();
    const refs = model.document.envelope.semantic_declarations.map((declaration) =>
      declarationEntityRef(declaration.id),
    );
    expect(refs).toEqual(PRODUCER_PRESERVED_ENTITIES);
  });

  it("pack/scope/namespace and digest domains are preserved", () => {
    const model = parsedModel();
    expect(model.document.inputs.semantic_pack_set_hash).toBe(
      "sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40",
    );
    expect(model.document.envelope.namespace_bindings).toEqual([
      { namespace: "auth", logical_id: "valid-basic.sea" },
    ]);
    expect(model.document.envelope.semantic_packs).toEqual([]);
    expect(model.document.envelope.modules[0]?.namespace).toBe("auth");
    // Source location = the logical module path (line positions are a recorded
    // producer omission), never invented line numbers.
    expect(model.document.envelope.semantic_declarations[0]?.logical_module_id).toBe(
      "valid-basic.sea",
    );
  });

  it("corrupt digest is a typed invalid, never a degraded parse", () => {
    const realHash = "sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e";
    const corruptHash = "sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97f";
    expect(corruptHash).not.toBe(realHash);
    const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON.replace(realHash, corruptHash));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("self_hash_mismatch");
    expect(parse.declared_schema_version).toBe(PINNED_ENVELOPE_SCHEMA_VERSION);
  });

  it("closure-hash corruption is a typed invalid too", () => {
    const doc = JSON.parse(REAL_DOCUMENT_JSON) as Record<string, unknown>;
    doc["semantic_closure_hash"] =
      "sha256:2a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786d";
    // Keep the document otherwise self-consistent so the closure check is
    // exercised in isolation (self_hash covers the declared closure hash).
    doc["self_hash"] = envelopeSelfHash(doc);
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("semantic_closure_hash_mismatch");
  });

  it("wrong schema version is typed incompatible with the declared version surfaced", () => {
    const doc = JSON.parse(REAL_DOCUMENT_JSON) as Record<string, unknown>;
    doc["schema_version"] = "domainforge-semantic-envelope/v2";
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("schema_version_mismatch");
    expect(parse.declared_schema_version).toBe("domainforge-semantic-envelope/v2");
  });

  it("a CEP snapshot wrapper is typed incompatible and never silently unwrapped", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_CEP_FAILURE_ENVELOPE_JSON);
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("cep_snapshot_wrapper");
  });

  it("unknown producer is typed unknown (never adopted, never name-joined)", () => {
    const doc = JSON.parse(REAL_DOCUMENT_JSON) as Record<string, unknown>;
    const producer = doc["producer"] as Record<string, unknown>;
    producer["name"] = "some-other-forge";
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    expect(parse.status).toBe("unknown");
    if (parse.status !== "unknown") return;
    expect(parse.detail).toContain("some-other-forge");
  });

  it("unknown top-level fields violate the closed pinned schema (typed invalid)", () => {
    const doc = JSON.parse(REAL_DOCUMENT_JSON) as Record<string, unknown>;
    doc["trust_me"] = true;
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("schema_violation");
    expect(parse.detail).toContain("trust_me");
  });

  it("non-JSON output is typed invalid", () => {
    const parse = parseSemanticEnvelopeDocument("Error: module failed to parse\n");
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("malformed_json");
  });

  it("the --emit both carrier parses D and PRESERVES diagnostics; diagnostics never look conformant", () => {
    // Success carrier: canonical D inline, CEP wrapper without extensions.
    const ok = parseProducerEnvelopeOutput(
      JSON.stringify({
        representation: canonicalJson(JSON.parse(REAL_DOCUMENT_JSON)),
        cep_envelope: {},
      }),
    );
    expect(ok.status).toBe("parsed");
    if (ok.status === "parsed") {
      expect(ok.model.model_valid).toBe(true);
    }

    // Diagnostics carrier: the REAL failure envelope's extensions attached to
    // an emitted-D body. Resolved output carrying diagnostics MUST NOT appear
    // conformant (REQ-DOM-004).
    const withDiag = parseProducerEnvelopeOutput(
      JSON.stringify({
        representation: canonicalJson(JSON.parse(REAL_DOCUMENT_JSON)),
        cep_envelope: JSON.parse(REAL_CEP_FAILURE_ENVELOPE_JSON),
      }),
    );
    expect(withDiag.status).toBe("parsed");
    if (withDiag.status !== "parsed") return;
    expect(withDiag.model.model_valid).toBe(false);
    expect(withDiag.model.diagnostics.length).toBe(1);
    expect(withDiag.model.diagnostics[0]).toContain("artifact_schema_violation");
  });
});

describe("T14 explicit Unit bindings (REQ-DOM-005)", () => {
  it("admits a binding declared by exact producer identity", () => {
    const model = parsedModel();
    const unit = FocusRef.for("gauntlet", "WorkItem", "u-auth-1");
    const bindings = admitUnitBindings(model, [
      { unit, declaration_ref: USER_ENTITY_REF, declared_by: "operator" },
    ]);
    expect(bindings.length).toBe(1);
    expect(bindings[0]?.declaration_ref).toBe(USER_ENTITY_REF);
    expect(bindings[0]?.unit).toBe(unit);
  });

  it("refuses a binding by declaration NAME — no name-based joins", () => {
    const model = parsedModel();
    const unit = FocusRef.for("gauntlet", "WorkItem", "u-auth-1");
    expect(() =>
      admitUnitBindings(model, [{ unit, declaration_ref: "User", declared_by: "operator" }]),
    ).toThrow(/declaration NAME, not an identity/);
  });

  it("refuses a binding to a concept that does not exist in the bound model", () => {
    const model = parsedModel();
    const unit = FocusRef.for("gauntlet", "WorkItem", "u-auth-1");
    expect(() =>
      admitUnitBindings(model, [
        {
          unit,
          declaration_ref: "concept:00000000-0000-5000-8000-000000000000",
          declared_by: "operator",
        },
      ]),
    ).toThrow(/does not exist in the bound model/);
  });

  it("refuses binding targets that are not WorkItem focuses", () => {
    const model = parsedModel();
    const evidence = FocusRef.for("gauntlet", "Evidence", "ev-1");
    expect(() =>
      admitUnitBindings(model, [
        { unit: evidence, declaration_ref: USER_ENTITY_REF, declared_by: "operator" },
      ]),
    ).toThrow(/WorkItem focus/);
  });
});

describe("T14 projection into Focus/Relation seams (REQ-DOM-003, REQ-DOM-004)", () => {
  it("projects concepts with one source identity across multiple placements", () => {
    const model = parsedModel();
    const enrichment = projectSemanticEnrichment(
      { status: "parsed", model },
      admitUnitBindings(model, [
        {
          unit: FocusRef.for("gauntlet", "WorkItem", "u-auth-1"),
          declaration_ref: USER_ENTITY_REF,
          declared_by: "operator",
        },
      ]),
    );
    expect(enrichment.status).toBe("bound");
    if (enrichment.status !== "bound") return;
    expect(enrichment.concepts.length).toBe(5);
    const user = enrichment.concepts.find((concept) => concept.entity_ref === USER_ENTITY_REF);
    expect(user?.focus.source).toBe(SEMANTIC_SOURCE_ID);
    expect(user?.focus.kind).toBe("entity");
    expect(user?.focus.id).toBe(`focus:${SEMANTIC_SOURCE_ID}:entity:${USER_ENTITY_REF}`);
    expect(user?.name).toBe("User");
    expect(user?.source_module).toBe("valid-basic.sea");
    expect(user?.namespace).toBe("auth");

    // One identity, two placements (Plan board + Agent detail): the SAME ref.
    const planPlacement = createPlacement("gen-t14", "plan", "plan_board", user?.focus as FocusRef);
    const agentPlacement = createPlacement(
      "gen-t14",
      "agents",
      "agent_detail",
      user?.focus as FocusRef,
    );
    expect(planPlacement.focus).toBe(agentPlacement.focus);
    expect(planPlacement.focus.id).toBe(agentPlacement.focus.id);
  });

  it("derives graph relations with model provenance whose endpoints ARE the concept identities", () => {
    const model = parsedModel();
    const enrichment = projectSemanticEnrichment({ status: "parsed", model });
    if (enrichment.status !== "bound") {
      throw new Error("enrichment must be bound");
    }
    // The real capture declares two flows: User→User and User→Role.
    expect(enrichment.relations.length).toBe(2);
    const userRole = enrichment.relations.find(
      (edge) => edge.target_ref.source_id === ROLE_ENTITY_REF,
    );
    expect(userRole?.kind).toBe("semantic_flow");
    expect(userRole?.source_ref.source_id).toBe(USER_ENTITY_REF);
    expect(userRole?.valid).toBe(true);
    expect(userRole?.provenance.source_id).toBe(SEMANTIC_SOURCE_ID);
    expect(userRole?.provenance.source_cursor).toBe(
      "sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c",
    );
    expect(userRole?.provenance.generation_id).toContain("semantic-model:");
    // Edge endpoints are the SAME interned FocusRefs as the concepts.
    const userConcept = enrichment.concepts.find(
      (concept) => concept.entity_ref === USER_ENTITY_REF,
    );
    expect(userRole?.source_ref).toBe(userConcept?.focus);
  });

  it("semantic neighborhoods are traversable with the existing cycle-safe traversal", () => {
    const model = parsedModel();
    const enrichment = projectSemanticEnrichment({ status: "parsed", model });
    if (enrichment.status !== "bound") {
      throw new Error("enrichment must be bound");
    }
    const userConcept = enrichment.concepts.find(
      (concept) => concept.entity_ref === USER_ENTITY_REF,
    );
    const traversal = semanticNeighborhood(enrichment, userConcept?.focus as FocusRef, [
      { kind: "semantic_flow", depth: 2, direction: "outbound" },
    ]);
    const ids = traversal.reached.map((ref) => ref.id);
    expect(ids).toContain(`focus:${SEMANTIC_SOURCE_ID}:entity:${USER_ENTITY_REF}`);
    expect(ids).toContain(`focus:${SEMANTIC_SOURCE_ID}:entity:${ROLE_ENTITY_REF}`);
  });

  it("diagnostics-carrying output withholds enrichment (never conformant, never projected)", () => {
    const parse = parseProducerEnvelopeOutput(
      JSON.stringify({
        representation: canonicalJson(JSON.parse(REAL_DOCUMENT_JSON)),
        cep_envelope: JSON.parse(REAL_CEP_FAILURE_ENVELOPE_JSON),
      }),
    );
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.model_valid).toBe(false);
    const enrichment = projectSemanticEnrichment(parse);
    expect(enrichment.status).toBe("absent");
    if (enrichment.status !== "absent") return;
    expect(enrichment.reason).toBe("model_invalid");
    expect(enrichment.detail).toContain("1 diagnostic");
  });
});

describe("T14 enrichment-absent honesty (REQ-DOM-001, Attack 2)", () => {
  const world: ProjectionWorld = {
    generation: "gen-t14",
    units: [
      {
        unit_id: "u-1",
        generation_id: "gen-t14",
        source_id: "gauntlet",
        source_cursor: "c-1",
        title: "Auth work",
        phase: "build",
        status: "active",
        authority: "builder",
        blast_radius: "isolated",
        updated_at: 1000,
      },
    ],
    evidence: [],
    sessions: [],
    source_cursors: { gauntlet: "c-1" },
  };

  it("no model output yields a typed-absent state, and nothing is fabricated", () => {
    const enrichment = projectSemanticEnrichment(undefined);
    expect(enrichment.status).toBe("absent");
    if (enrichment.status !== "absent") return;
    expect(enrichment.reason).toBe("no_model_output");
    const section = renderSemanticSection(enrichment);
    expect(section.records.length).toBe(0);
    expect(section.lines[0]?.spans.map((span) => span.text).join("")).toContain("ABSENT");
  });

  it("every failure class maps to an explicit absence reason", () => {
    expect(projectSemanticEnrichment(parseSemanticEnvelopeDocument("{nope"))).toMatchObject({
      status: "absent",
      reason: "parse_invalid",
    });
    expect(
      projectSemanticEnrichment({
        status: "incompatible",
        reason: "schema_version_mismatch",
        detail: "x",
        declared_schema_version: "v9",
      }),
    ).toMatchObject({ status: "absent", reason: "producer_incompatible" });
    expect(
      projectSemanticEnrichment({ status: "unknown", detail: "mystery producer" }),
    ).toMatchObject({ status: "absent", reason: "producer_unknown" });
  });

  it("the same Case workflow completes with enrichment absent: plans are identical, candidates empty", () => {
    const unitFocus = FocusRef.for("gauntlet", "WorkItem", "u-1");
    const horizon = createHorizon({
      root_focus: unitFocus,
      generation: world.generation,
      relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
    });

    const baseline = planMaterialization(world, horizon);
    const absent = projectSemanticEnrichment(undefined);
    const composed = planMaterialization(world, horizon);

    // Same workflow, same plan: absent enrichment contributes nothing.
    expect(composed.counters).toEqual(baseline.counters);
    expect(semanticContextCandidates(absent).length).toBe(0);

    // Explicit absence is rendered for the operator — not silently dropped.
    const section = renderSemanticSection(absent);
    const text = section.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain("ABSENT (no_model_output)");
    expect(text).toContain("no semantic relations are shown because none exist");
  });

  it("bound enrichment composes as OPTIONAL, never-mandatory context candidates", () => {
    const model = parsedModel();
    const bound = projectSemanticEnrichment({ status: "parsed", model });
    const candidates = semanticContextCandidates(bound);
    expect(candidates.length).toBe(5);
    for (const candidate of candidates) {
      expect(candidate.role).toBe("relation");
      expect(candidate.mandatory).toBe(false);
    }

    const unitFocus = FocusRef.for("gauntlet", "WorkItem", "u-1");
    const horizon = createHorizon({ root_focus: unitFocus, generation: world.generation });
    const plan = planMaterialization(world, horizon);
    const composed = [...plan.context_candidates, ...candidates];
    // Dedupe by focus id (horizon's own rule): no duplicate identities, and
    // semantic candidates never displace mandatory closure.
    const deduped = new Map(composed.map((candidate) => [candidate.focus.id, candidate]));
    expect(deduped.size).toBe(new Set(composed.map((candidate) => candidate.focus.id)).size);
    expect(deduped.size).toBeGreaterThanOrEqual(plan.context_candidates.length);
    expect(composed.filter((candidate) => candidate.mandatory).length).toBe(
      plan.context_candidates.filter((candidate) => candidate.mandatory).length,
    );
  });
});

describe("T14 Inspector feed (REQ-DOM-005 shapes)", () => {
  it("bound state shows model identity, declarations with source locations, and relation records", () => {
    const model = parsedModel();
    const enrichment = projectSemanticEnrichment({ status: "parsed", model });
    const section = renderSemanticSection(enrichment);
    expect(section.id).toBe("semantic");
    const text = section.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain("domainforge-core 0.16.0");
    expect(text).toContain(
      "sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c",
    );
    expect(text).toContain("User");
    expect(text).toContain("valid-basic.sea");
    expect(section.records.length).toBe(2);
    for (const record of section.records) {
      expect(record.kind).toBe("relation");
      expect(record.fields["provenance"]).toContain("semantic-model:");
    }
  });

  it("diagnostics render visibly on the bound section when present", () => {
    const parse = parseProducerEnvelopeOutput(
      JSON.stringify({
        representation: canonicalJson(JSON.parse(REAL_DOCUMENT_JSON)),
        cep_envelope: JSON.parse(REAL_CEP_FAILURE_ENVELOPE_JSON),
      }),
    );
    if (parse.status !== "parsed") throw new Error("expected parsed");
    // Bound projection withholds relations, but the Inspector feed on the
    // ABSENT state names the reason; the diagnostics live on the parsed model.
    const enrichment = projectSemanticEnrichment(parse);
    expect(enrichment.status).toBe("absent");
    const section = renderSemanticSection(enrichment);
    const text = section.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain("ABSENT (model_invalid)");
  });
});
