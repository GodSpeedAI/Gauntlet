import { describe, expect, it } from "bun:test";
import {
  bunCacheHash,
  CrossDomainIdentityError,
  envelopeClosureHash,
  envelopeSelfHash,
  fileContentHash,
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
  requireBunCacheHash,
  requireFileContentHash,
  requireSemanticFingerprint,
  type SemanticModelIdentity,
  semanticFingerprint,
} from "./domainforge-adapter";

/**
 * T14 semantic identity suites (plan T14, preregistered Attack 1, prereg P1).
 * Proves REQ-DOM-002: a Bun cache hash, a file content hash, and a DomainForge
 * semantic fingerprint remain DISTINCT typed identities and cannot substitute
 * for one another — not at compile time, not at runtime, not under attacks
 * that give identical-looking labels to all three.
 */

// The preregistered attack: identical-looking labels across all three domains.
const IDENTICAL_LABEL = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

describe("T14 semantic identity typing (REQ-DOM-002, prereg Attack 1 / P1)", () => {
  it("mints three identities with IDENTICAL labels that remain three distinct domains", () => {
    const cache = bunCacheHash(IDENTICAL_LABEL);
    const file = fileContentHash(IDENTICAL_LABEL);
    const semantic = semanticFingerprint(IDENTICAL_LABEL);

    // Value equality does NOT imply domain equality: every consumer that
    // requires one domain refuses the others at runtime.
    expect(requireBunCacheHash(cache)).toBe(IDENTICAL_LABEL);
    expect(requireFileContentHash(file)).toBe(IDENTICAL_LABEL);
    expect(requireSemanticFingerprint(semantic)).toBe(IDENTICAL_LABEL);

    // Every cross-domain call below is BOTH a compile error (the directive
    // proves it) and a runtime refusal (the assertion proves it).
    expect(() =>
      // @ts-expect-error a Bun cache hash is NOT a semantic fingerprint
      requireSemanticFingerprint(cache),
    ).toThrow(CrossDomainIdentityError);
    expect(() =>
      // @ts-expect-error a file content hash is NOT a semantic fingerprint
      requireSemanticFingerprint(file),
    ).toThrow(CrossDomainIdentityError);
    expect(() =>
      // @ts-expect-error a file content hash is NOT a Bun cache hash
      requireBunCacheHash(file),
    ).toThrow(CrossDomainIdentityError);
    expect(() =>
      // @ts-expect-error a semantic fingerprint is NOT a Bun cache hash
      requireBunCacheHash(semantic),
    ).toThrow(CrossDomainIdentityError);
    expect(() =>
      // @ts-expect-error a Bun cache hash is NOT a file content hash
      requireFileContentHash(cache),
    ).toThrow(CrossDomainIdentityError);
    expect(() =>
      // @ts-expect-error a semantic fingerprint is NOT a file content hash
      requireFileContentHash(semantic),
    ).toThrow(CrossDomainIdentityError);
  });

  it("compile-time substitution is a type error in BOTH directions (verified by tsc)", () => {
    const cache = bunCacheHash(IDENTICAL_LABEL);
    const file = fileContentHash(IDENTICAL_LABEL);
    const semantic = semanticFingerprint(IDENTICAL_LABEL);

    const model: SemanticModelIdentity = {
      model_id: "m-1",
      semantic_fingerprint: semantic,
      entry_file_hash: file,
      artifact_cache_hash: cache,
    };
    // Each field refuses the other domains' values. These lines are type
    // errors: tsc --noEmit (just tui-check) fails if the domains ever unify.
    const attackFingerprint: SemanticModelIdentity = {
      ...model,
      // @ts-expect-error a Bun cache hash is NOT a semantic fingerprint
      semantic_fingerprint: cache,
    };
    const attackCache: SemanticModelIdentity = {
      ...model,
      // @ts-expect-error a file content hash is NOT a Bun cache hash
      artifact_cache_hash: file,
    };
    const attackFile: SemanticModelIdentity = {
      ...model,
      // @ts-expect-error a semantic fingerprint is NOT a file content hash
      entry_file_hash: semantic,
    };
    void attackFingerprint;
    void attackCache;
    void attackFile;
  });

  it("runtime fabrication is refused even when the object is smuggled through unknown", () => {
    const genuine = semanticFingerprint(IDENTICAL_LABEL);
    // An attacker forges the SHAPE of a semantic fingerprint from a file hash
    // instance (or any object) and smuggles it through `unknown`.
    const forged = fileContentHash(IDENTICAL_LABEL) as unknown;
    expect(() => requireSemanticFingerprint(forged as typeof genuine)).toThrow(
      CrossDomainIdentityError,
    );
    expect(() => requireSemanticFingerprint({ value: IDENTICAL_LABEL } as typeof genuine)).toThrow(
      CrossDomainIdentityError,
    );
    // The genuine one passes.
    expect(requireSemanticFingerprint(genuine)).toBe(IDENTICAL_LABEL);
  });

  it("mint validation refuses malformed digests in the digest domains", () => {
    expect(() => fileContentHash("deadbeef")).toThrow(CrossDomainIdentityError);
    expect(() => fileContentHash("sha256:DEADBEEF")).toThrow(CrossDomainIdentityError);
    expect(() => semanticFingerprint("")).toThrow(CrossDomainIdentityError);
    // Bun cache tokens are opaque and only require a non-empty string.
    const token = bunCacheHash("wyhash-token");
    expect(requireBunCacheHash(token)).toBe("wyhash-token");
  });

  it("the envelope parser mints ONLY a semantic fingerprint for a model — never the other domains", () => {
    // A minimal one-declaration model, made self-consistent through the same
    // digest rule proven against the REAL capture in domainforge-adapter.test.ts.
    const doc = JSON.parse(
      '{"schema_version":"domainforge-semantic-envelope/v1","producer":{"name":"domainforge-core","version":"0.16.0"},"inputs":{"source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","language_schema_version":"domainforge-ast/v3","interpretation_version":"domainforge-interpretation/v1"},"self_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","semantic_closure_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","envelope":{"language_schema_version":"domainforge-ast/v3","canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","modules":[{"logical_id":"valid-basic.sea","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29","namespace":"auth"}],"import_graph":[],"namespace_bindings":[{"namespace":"auth","logical_id":"valid-basic.sea"}],"semantic_packs":[],"semantic_declarations":[{"id":{"kind":"concept","data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"}},"logical_module_id":"valid-basic.sea","declaration":{"kind":"entity","data":{"name":"User","domain":"auth","annotations":{}}}}],"resolved_references":[],"application_contract":{"enums":[],"records":[],"entities":[],"operations":[]}}}',
    ) as Record<string, unknown>;
    doc["semantic_closure_hash"] = envelopeClosureHash(doc["envelope"]);
    doc["self_hash"] = envelopeSelfHash(doc);
    const parse = parseSemanticEnvelopeDocument(JSON.stringify(doc));
    if (parse.status !== "parsed") {
      throw new Error("self-consistent fixture must parse");
    }
    const model = parse.model;
    expect(model.fingerprint.value).toBe(doc["semantic_closure_hash"] as string);
    expect(requireSemanticFingerprint(model.fingerprint)).toBe(
      doc["semantic_closure_hash"] as string,
    );

    // Projected enrichment carries the model's identity, and the projection
    // never mints cache/file identities for semantic objects.
    const enrichment = projectSemanticEnrichment(parse);
    if (enrichment.status !== "bound") {
      throw new Error("enrichment must be bound");
    }
    for (const concept of enrichment.concepts) {
      expect(concept.focus.source).toBe("domainforge");
      // FocusRef id derives from the producer entity ref, not any file/cache hash.
      expect(concept.focus.id.startsWith("focus:domainforge:")).toBe(true);
    }
    for (const edge of enrichment.relations) {
      expect(edge.provenance.source_cursor).toBe(doc["semantic_closure_hash"] as string);
    }
  });

  it("a full SemanticModelIdentity keeps all three domains separate side by side", () => {
    const cache = bunCacheHash("bun-cache-entry-0123456789abcdef");
    const file = fileContentHash(
      "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    );
    const semantic = semanticFingerprint(
      "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    );
    const identity: SemanticModelIdentity = {
      model_id: "domainforge:sha256:1641b0d3",
      semantic_fingerprint: semantic,
      entry_file_hash: file,
      artifact_cache_hash: cache,
    };
    // Each domain answers only to its own requirement.
    expect(requireSemanticFingerprint(identity.semantic_fingerprint)).toBe(semantic.value);
    expect(
      requireFileContentHash(
        identity.entry_file_hash as Parameters<typeof requireFileContentHash>[0],
      ),
    ).toBe(file.value);
    expect(
      requireBunCacheHash(
        identity.artifact_cache_hash as Parameters<typeof requireBunCacheHash>[0],
      ),
    ).toBe(cache.value);
  });
});
