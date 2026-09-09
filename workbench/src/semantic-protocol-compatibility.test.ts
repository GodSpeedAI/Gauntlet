import { describe, expect, it } from "bun:test";
import {
  canonicalJson,
  envelopeClosureHash,
  envelopeSelfHash,
  PINNED_ENVELOPE_SCHEMA_VERSION,
  PROTOCOL_COMPATIBILITY,
  parseProducerEnvelopeOutput,
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
  QUALIFIED_PRODUCER_BUILDS,
  selectProtocolPairs,
} from "./domainforge-adapter";

/**
 * T14 producer/protocol compatibility suites (plan T14 step 2, REQ-DOM-004,
 * decisions.yaml DEC-005, prereg D3).
 *
 * Proves the compatibility boundary is EXPLICIT:
 *   - the DomainForge main CLI envelope contract and the SXR Slice 0B consumer
 *     contract are recorded INCOMPATIBLE (not interchangeable versions);
 *   - an unknown producer yields a typed unknown — never a fallback;
 *   - the adapter's compatible-pair selection is recorded with local evidence
 *     and binary digests, so T14B/T20 can re-qualify without re-deriving.
 *
 * Facts encoded here were VERIFIED on the builder workstation against the real
 * binaries (see QUALIFIED_PRODUCER_BUILDS evidence); the tests themselves stay
 * deterministic (recorded fixtures, no process spawn).
 */

/** The exact argv SXR Slice 0B's CliDomainForgeAdapter passes (sxr-df src/adapter.rs). */
const SXR_SLICE_0B_REQUEST_SHAPE = "`envelope --emit both <entry>`";

// The producer's own failure envelope (real capture, release-adjacent debug
// build): a CEP-0008 semantic snapshot wrapper, NOT the pinned bare document.
const REAL_CEP_FAILURE_ENVELOPE_JSON =
  '{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["omissions","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions","canonical_representation_D"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"incomplete_for_declared_state","conformance_status":"conformant","created_at":"2026-09-09T15:49:39.604598Z","created_by":"domainforge 0.16.0","envelope_id":"01a086dc-4c94-7622-96f7-22f1c903d410","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","extensions":{"domainforge":{"diagnostics":["artifact_schema_violation: module \'domainforge-core/tests/fixtures/envelope/invalid-syntax.sea\' failed to parse: Syntax error at 6:35: expected number"],"invalid_declared_checkpoint_hash":"sha256:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908","model_validation_status":"invalid"}},"lineage_refs":[],"omission_status":"known_omissions_recorded","omissions":[{"affected_entities":["domainforge-core/tests/fixtures/envelope/invalid-syntax.sea"],"description":"canonical D not constructed; declared model invalid","impact":"no semantic hash derivable","known_or_suspected":"known","omission_id":"omission:representation:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908","omission_type":"representation_unavailable","reason":"model validation failed"}],"provenance":[{"created_at":"2026-09-09T15:49:39.604598Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a086dc-4c94-7622-96f7-22f1c903d410","review_status":"unreviewed","source_system_refs":["source-set:sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e","ref_type":"source_set","scope":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","target_hash":"sha256:1da2f040c24d5498b63de891e834d8f882facc4676e68f7ea90c61bb46de0b9e","target_uri_or_id":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea"}],"scope":{"entry_logical_path":"domainforge-core/tests/fixtures/envelope/invalid-syntax.sea","invalid_declared_checkpoint_hash":"sha256:b7f8975f771ca54eaecce96f9f76909053e73ba23d7dc5a543675437adccf908"},"validation_status":"validated"}';

describe("T14 protocol compatibility (REQ-DOM-004, DEC-005)", () => {
  it("records the main-CLI / SXR Slice 0B pair as INCOMPATIBLE with locally verified evidence", () => {
    const slice0bPairs = PROTOCOL_COMPATIBILITY.filter((pair) =>
      pair.consumer.includes("Slice 0B"),
    );
    expect(slice0bPairs.length).toBe(3);
    // The installed binary and the release build both REJECT the Slice 0B
    // invocation; recorded as verified local facts, not assumptions.
    const incompatible = slice0bPairs.filter((pair) => pair.status === "incompatible");
    expect(incompatible.length).toBe(2);
    for (const pair of incompatible) {
      expect(pair.locally_verified).toBe(true);
      expect(pair.evidence).toMatch(/exit 2/);
    }
  });

  it("records the compatible-pair selection: bare-D pinned, SXR unqualified until T14B", () => {
    const pinned = PROTOCOL_COMPATIBILITY.filter((pair) => pair.status === "pinned");
    expect(pinned.length).toBe(1);
    expect(pinned[0]?.producer).toContain("bare D");
    expect(pinned[0]?.response_shape).toBe(`bare ${PINNED_ENVELOPE_SCHEMA_VERSION} document`);
    const unqualified = PROTOCOL_COMPATIBILITY.find((pair) => pair.status === "unqualified");
    expect(unqualified?.consumer).toContain("Slice 0B");
    expect(unqualified?.evidence).toContain("T14B");
    // DEC-005 holds: no SXR pair is selected by this adapter.
    expect(
      PROTOCOL_COMPATIBILITY.some(
        (pair) => pair.consumer.includes("Slice 0B") && pair.status === "pinned",
      ),
    ).toBe(false);
  });

  it("selectProtocolPairs answers typed results — unknown shapes never fall back", () => {
    const main = selectProtocolPairs(
      "`domainforge envelope <entry.sea>` (positional entry, no flags)",
    );
    expect(main.length).toBe(1);
    expect(main[0]?.status).toBe("pinned");

    // The SXR request shape is recorded against three producer builds: the
    // most-severe verdict (incompatible) leads; NOTHING is implicitly adopted.
    const slice0b = selectProtocolPairs(SXR_SLICE_0B_REQUEST_SHAPE);
    expect(slice0b.length).toBe(3);
    expect(slice0b[0]?.status).toBe("incompatible");
    expect(slice0b.some((pair) => pair.status === "pinned")).toBe(false);

    expect(selectProtocolPairs("vendor-x widget --dump")).toEqual([]);
  });

  it("a payload from the OTHER contract (CEP snapshot wrapper) is typed incompatible, never adapted", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_CEP_FAILURE_ENVELOPE_JSON);
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("cep_snapshot_wrapper");
    // And projection of an incompatible contract is explicit ABSENCE.
    const enrichment = projectSemanticEnrichment(parse);
    expect(enrichment).toMatchObject({ status: "absent", reason: "producer_incompatible" });
  });

  it("an unknown producer document is typed unknown — identity never adopted", () => {
    // A structurally plausible document from a producer this adapter does not
    // pin: refused as unknown regardless of how well-formed it looks.
    const doc: Record<string, unknown> = JSON.parse(
      '{"schema_version":"domainforge-semantic-envelope/v1","producer":{"name":"semantic-wizard","version":"9.9.9"},"inputs":{"source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","language_schema_version":"domainforge-ast/v3","interpretation_version":"domainforge-interpretation/v1"},"self_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","semantic_closure_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","envelope":{}}',
    );
    // NOTE: the unknown-producer verdict fires before digest verification, so
    // the fake digests above never become evidence of anything.
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    expect(parse.status).toBe("unknown");
    if (parse.status !== "unknown") return;
    expect(parse.detail).toContain("semantic-wizard");
  });

  it("the --emit both carrier from a Slice-0B-shaped producer parses via the dedicated path with diagnostics preserved", () => {
    // A producer that DOES answer the Slice 0B invocation carries D inline in
    // its both-mode output; the adapter's dedicated path re-verified D strictly.
    const validDoc = JSON.parse(
      '{"schema_version":"domainforge-semantic-envelope/v1","producer":{"name":"domainforge-core","version":"0.16.0"},"inputs":{"source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","language_schema_version":"domainforge-ast/v3","interpretation_version":"domainforge-interpretation/v1"},"self_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","envelope":{"language_schema_version":"domainforge-ast/v3","canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","modules":[{"logical_id":"valid-basic.sea","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29","namespace":"auth"}],"import_graph":[],"namespace_bindings":[{"namespace":"auth","logical_id":"valid-basic.sea"}],"semantic_packs":[],"semantic_declarations":[],"resolved_references":[],"application_contract":{"enums":[],"records":[],"entities":[],"operations":[]}}}',
    ) as Record<string, unknown>;
    // Make the trimmed document self-consistent through the pinned digest rule
    // (the trim changes the canonical bytes; digests must cover the real ones).
    validDoc["semantic_closure_hash"] = envelopeClosureHash(validDoc["envelope"]);
    validDoc["self_hash"] = envelopeSelfHash(validDoc);
    const both = {
      representation: canonicalJson(validDoc),
      cep_envelope: JSON.parse(REAL_CEP_FAILURE_ENVELOPE_JSON),
    };
    const parse = parseProducerEnvelopeOutput(JSON.stringify(both));
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    // Resolved output carrying diagnostics MUST NOT appear conformant.
    expect(parse.model.model_valid).toBe(false);
    expect(parse.model.diagnostics.length).toBeGreaterThan(0);
    // Digests over the raw document still verify the pinned contract.
    expect(parse.model.document.self_hash).toBe(envelopeSelfHash(parse.model.document_raw));
    expect(parse.model.document.semantic_closure_hash).toBe(
      envelopeClosureHash(parse.model.document_raw["envelope"]),
    );
  });

  it("binary digests for all locally qualified builds are recorded for requalification", () => {
    expect(QUALIFIED_PRODUCER_BUILDS.length).toBe(3);
    const digests = new Set(QUALIFIED_PRODUCER_BUILDS.map((build) => build.binary_sha256));
    expect(digests.size).toBe(3);
    for (const build of QUALIFIED_PRODUCER_BUILDS) {
      expect(build.binary_sha256).toMatch(/^[0-9a-f]{64}$/);
      expect(build.version).toBe("0.16.0");
    }
    // Exactly one build answers the SXR Slice 0B invocation, and the adapter
    // still does not select it (uncommitted working-tree state).
    expect(
      QUALIFIED_PRODUCER_BUILDS.filter((build) => build.answers_sxr_slice0b_invocation).length,
    ).toBe(1);
    expect(QUALIFIED_PRODUCER_BUILDS.filter((build) => build.emits_bare_document).length).toBe(2);
  });
});
