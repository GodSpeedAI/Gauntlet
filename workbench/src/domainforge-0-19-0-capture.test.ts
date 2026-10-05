import { describe, expect, it } from "bun:test";
import {
  envelopeClosureHash,
  envelopeSelfHash,
  PINNED_ENVELOPE_SCHEMA_VERSION,
  parseSemanticEnvelopeDocument,
} from "./domainforge-adapter";

/**
 * DomainForge 0.19.0 convergence proof. REAL capture of
 * `domainforge envelope domainforge-core/tests/fixtures/envelope/valid-basic.sea`
 * from release 0.19.0 (same source as the historical 0.16.0 and 0.18.2 captures in
 * domainforge-adapter.test.ts and domainforge-0-18-2-capture.test.ts). Semantic
 * identity must not change across upgrades: the closure hash is identical; only
 * producer.version and the document self_hash (which covers the producer block) differ.
 */
const REAL_DOCUMENT_JSON_0_19_0 =
  '{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"valid-basic.sea","namespace":"auth","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29"}],"namespace_bindings":[{"logical_id":"valid-basic.sea","namespace":"auth"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"auth","name":"User"},"kind":"entity"},"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Role"},"kind":"entity"},"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Token","unit":{"data":{"id":{"data":{"id":"7a50d9d2-89e2-5607-bca5-34464284bea8"},"kind":"concept"}},"kind":"declaration"}},"kind":"resource"},"id":{"data":{"id":"a81c1fec-c10a-5541-bd81-98c64b10f402"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.19.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:b2e8a2cd524eb8c19971c93f349ca0bea7f11007542a3363e018473d5f438e3e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c"}';

const CLOSURE_HASH_SHARED_WITH_EARLIER_RELEASES =
  "sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c";

describe("DomainForge 0.19.0 capture", () => {
  it("parses the REAL 0.19.0 producer output with verified digests", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON_0_19_0);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.document.schema_version).toBe(PINNED_ENVELOPE_SCHEMA_VERSION);
    expect(parse.model.document.producer).toEqual({ name: "domainforge-core", version: "0.19.0" });
    expect(parse.model.document.self_hash).toBe(envelopeSelfHash(parse.model.document_raw));
    expect(parse.model.document.semantic_closure_hash).toBe(
      envelopeClosureHash(parse.model.document_raw["envelope"]),
    );
    expect(parse.model.model_valid).toBe(true);
  });

  it("keeps the semantic fingerprint identical to the 0.16.0 and 0.18.2 captures of the same source", () => {
    const parse = parseSemanticEnvelopeDocument(REAL_DOCUMENT_JSON_0_19_0);
    if (parse.status !== "parsed") throw new Error("fixture must parse");
    expect(parse.model.fingerprint.value).toBe(CLOSURE_HASH_SHARED_WITH_EARLIER_RELEASES);
  });
});
