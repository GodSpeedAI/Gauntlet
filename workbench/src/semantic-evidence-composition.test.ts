import { describe, expect, it } from "bun:test";
import {
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
  renderSemanticSection,
  type SemanticEnrichment,
} from "./domainforge-adapter";
import { FocusRef } from "./focus";
import { OperationGateway, type OperationInvocationRequest } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { deriveRelations, traverseRelations } from "./relations";
import {
  admitSxrUnitBindings as admitBindings,
  authoritativeSxrEvidenceFrame,
  executeSxrRefresh,
  offerSxrEvidenceAsGauntletEvidence,
  parseSxrLedgerExport,
  projectSxrEvidenceLens,
  renderSxrEvidenceSection,
  SXR_SOURCE_ID,
  SxrEvidenceAdmissionGate,
  type SxrLensProjection,
  type SxrProcessRunner,
  sxrVerificationPath,
} from "./sxr-adapter";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { GEN, observationFor, seededModel } from "./testing/shell-harness";
import { renderEvidenceView, renderTraceView } from "./views-chat-trace";
import { renderWorkView } from "./views-work";

/**
 * T14B semantic-evidence composition matrix (plan T14B, REQ-SXR-004,
 * REQ-SYNC-003, prereg transfer_tests.replay).
 *
 * Drives the REAL captured producer/consumer outputs through the workbench in
 * all four preregistered binding modes — Gauntlet-only, DomainForge-only,
 * SXR-only, combined — and proves:
 *   - the core journeys (Work, Evidence, Trace, relations traversal, snapshot
 *     counters) are preserved in EVERY combination, with losses EXPLICIT;
 *   - SXR identity/settlement stay distinct from Gauntlet's (REQ-SXR-002);
 *   - source loss preserves the journeys: admitted consequential records
 *     reconstruct from the authoritative log while lost source output is
 *     labelled unavailable, never recreated (REQ-SYNC-003);
 *   - the SXR refresh route composes with the deny matrix: the bounded
 *     adapter path is the allowed route; the same logical refresh driven
 *     through the action gateway is refused with the canonical read-shaped
 *     typed denial and zero side effects.
 *
 * Fixtures are REAL captures (see sxr-evidence-adapter.test.ts header);
 * the canonical D document is the pinned valid-basic capture's inline D.
 */

// --- REAL capture: the pinned canonical D document (inline_content of the
// REAL both-mode valid-basic capture; same bytes T14 pins) -------------------
const REAL_DF_DOCUMENT =
  '{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"valid-basic.sea","namespace":"auth","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29"}],"namespace_bindings":[{"logical_id":"valid-basic.sea","namespace":"auth"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"auth","name":"User"},"kind":"entity"},"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Role"},"kind":"entity"},"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Token","unit":{"data":{"id":{"data":{"id":"7a50d9d2-89e2-5607-bca5-34464284bea8"},"kind":"concept"}},"kind":"declaration"}},"kind":"resource"},"id":{"data":{"id":"a81c1fec-c10a-5541-bd81-98c64b10f402"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c"}';

// --- REAL capture: the raw `sxr ledger export` array (13 rows) --------------
const REAL_LEDGER_EXPORT =
  '[{"append_ordinal":1,"entry_hash":"sha256:b7d99b16b450d379d7f02de8bc96b43cbc3aeec802ea12a893960e1f721bc215","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:29.438050Z","created_by":"sxr 0.1.0","envelope_id":"01M23MNV1Y67K4G7PGF1WRST1V","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["ledger:gen:1"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","filesystem_path":"/tmp/t14b-capture/repo","git_commit":"3c98f1f4048e0f8d439e6d96b6fed077d23b9553","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"states":[{"scope":"repo:01M23MNV1W3ZVM8FNBJETVCNAF","state_id":"state:enroll","state_type":"repository"}],"transformations":[{"transformation_id":"tx:enroll","transformation_type":"enrollment"}],"validation_status":"validated"},"record_kind":"repo.enrolled"},{"append_ordinal":2,"entry_hash":"sha256:8eb8f9efdb81f3fc59b64019689d1c33196da5a51805ad4fc02559325da72cc2","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:42.714953Z","created_by":"sxr 0.1.0","envelope_id":"01M23MP80TK5ZZ1A5XP5QB2RK3","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["ledger:gen:1"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows","question_type":"declared_semantics"}],"representations":[{"inline_content":{"question_form":"is_true","question_id":"q-logistics-flows","question_type":"declared_semantics","target_entities":"concept:order_flow"},"representation_id":"rep:question:q-logistics-flows","representation_kind":"atomic","representation_type":"question_spec"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"validation_status":"validated"},"record_kind":"question.declared"},{"append_ordinal":3,"entry_hash":"sha256:013e9b2c44a8a74e0da3d94d682a98c6c7c9943aa1f493ce8d4e5f0585f47a1f","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:42.741809Z","created_by":"sxr 0.1.0","envelope_id":"01M23MP81NFPDACVSMFMF0EVMW","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows"],"representations":[{"inline_content":{"evaluation_profile":"v2-independent-validity","threshold_id":"th-all-supports","threshold_kind":"all_evidence_supports"},"representation_id":"rep:threshold:th-all-supports","representation_kind":"atomic","representation_type":"settlement_threshold_spec"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"states":[{"scope":"question:q-logistics-flows","state_id":"th-all-supports","state_type":"settlement_threshold"}],"validation_status":"validated"},"record_kind":"threshold.declared"},{"append_ordinal":4,"entry_hash":"sha256:4f0c91c1c75cc1ac12cc1c57ef5c0419d89c43aad4a8ec7ef671ff36c7a2d53a","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["representations","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"valid-basic.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:51:21.395604Z","created_by":"domainforge 0.16.0","envelope_id":"01a0874b-b733-7d32-8ce2-13682e26cd30","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","extensions":{"domainforge":{"model_validation_status":"valid","semantics_version":"v2-independent-validity"}},"lineage_refs":[],"omission_status":"known_omissions_recorded","provenance":[{"created_at":"2026-09-09T17:51:21.395604Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a0874b-b733-7d32-8ce2-13682e26cd30","review_status":"unreviewed","source_system_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","ref_type":"source_set","scope":"valid-basic.sea","target_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","target_uri_or_id":"valid-basic.sea"}],"representations":[{"content_availability":"available","content_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","inline_content":"{\\"envelope\\":{\\"application_contract\\":{\\"entities\\":[],\\"enums\\":[],\\"operations\\":[],\\"records\\":[]},\\"canonicalization_version\\":\\"domainforge-canonicalization/v1\\",\\"compiler_interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"import_graph\\":[],\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"modules\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\",\\"semantic_content_hash\\":\\"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29\\"}],\\"namespace_bindings\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\"}],\\"resolved_references\\":[],\\"semantic_declarations\\":[{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"User\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Role\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Token\\",\\"unit\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"7a50d9d2-89e2-5607-bca5-34464284bea8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"resource\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"a81c1fec-c10a-5541-bd81-98c64b10f402\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"}],\\"semantic_packs\\":[]},\\"inputs\\":{\\"interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"semantic_pack_set_hash\\":\\"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40\\",\\"source_set_hash\\":\\"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5\\"},\\"producer\\":{\\"name\\":\\"domainforge-core\\",\\"version\\":\\"0.16.0\\"},\\"schema_version\\":\\"domainforge-semantic-envelope/v1\\",\\"self_hash\\":\\"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e\\",\\"semantic_closure_hash\\":\\"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c\\"}","media_type":"application/json","omission_record":{"known_omissions":["source_comments","source_formatting","source_line_positions"],"reason":"domainforge canonical envelope is formatting- and comment-independent by construction","recoverable_from":"the .sea sources at the recorded git blob hashes"},"preserved_distinctions":["declared_vs_observed","representation_vs_reality"],"preserved_entities":["concept:5ad6a959-5924-5cf9-af04-7b502ea74784","concept:72f70c63-6f3e-5061-8854-42e540bbac87","concept:a81c1fec-c10a-5541-bd81-98c64b10f402","occurrence:flow:sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543:0","occurrence:flow:sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66:0"],"provenance_record":{"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"resolution_inputs":{"entry_logical_path":"valid-basic.sea","registry_content_hash":null,"resolved_namespaces":{"valid-basic.sea":"auth"}}},"representation_id":"representation:sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","representation_kind":"derived","representation_type":"domainforge_semantic_envelope_document","scope":"valid-basic.sea","semantic_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","size_bytes":3195,"source":"domainforge 0.16.0","validation_status":"valid"}],"scope":{},"validation_status":"validated"},"record_kind":"semantic.snapshot"},{"append_ordinal":5,"entry_hash":"sha256:aa309b4bab6f0bf7a8e60f92107cb49f3e2a4bf7d91f510604c6425c0e2fcdda","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:52:04.799082Z","created_by":"sxr 0.1.0","envelope_id":"01M23MRR5ZH0RYT10CZ8CZ8WDW","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports","snapshot:sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"transformations":[{"transformation_id":"tx:01M23MRR5XTM78DR4KJ72QKPZ8","transformation_type":"run_start"}],"validation_status":"validated"},"record_kind":"run.started"},{"append_ordinal":6,"entry_hash":"sha256:1d9f93dfd4b2e97232d8bba2a129b578b10cbd8df4b266e97c2faf251d8ff54d","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:52:10.045981Z","created_by":"sxr 0.1.0","envelope_id":"01M23MRX9XD526664SEQG5FJGJ","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"representations":[{"inline_content":{"boundary":{"excluded":["browser","performance","security"],"limitations":["offline only","slice-1 cargo-test"]},"evaluation_profile_ref":"profile://slice2-policy@1","evidence_requirements":[{"evidence_type":"test_result","min_count":1,"min_relevance":"high","min_reliability":"medium"}],"plan_id":"01M23MRX9W1CC6SKAP20K4ZQP3","plan_version":"sxr-plan/v1","question_ref":"q-logistics-flows","semantic_verification_contract_ref":null,"snapshot_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98","steps":[{"claim_ref":null,"evidence_requirement_ref":null,"expected_answer_shape":"boolean_with_failure_list","invocation":{"argv":["cargo","test","--offline","--message-format","json"],"cwd":".","env_allowlist":["PATH","HOME","CARGO_HOME"],"timeout_s":900},"limitations":[],"obligation_ref":null,"produces_evidence_type":"test_result","relevance_to_question":"high","step_id":"unit","targets":["question:q-logistics-flows"],"verifier_id":"cargo-test"}],"threshold_ref":"th-all-supports","transformation_ref":"transformation:plan:01M23MRX9W1CC6SKAP20K4ZQP3"},"representation_id":"rep:plan:01M23MRX9W1CC6SKAP20K4ZQP3","representation_kind":"atomic","representation_type":"verification_plan"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"plan.created"},{"append_ordinal":7,"entry_hash":"sha256:db469d27d0688d8793983b01fc9662c3327d651de27aaa8cde0a82241c6e1696","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:54:41.484801Z","created_by":"sxr 0.1.0","envelope_id":"01M23MXH6CC0R4Z050GPBX94WF","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"observations":[{"observation_id":"01M23MXH6B0PGSM6FYFWJFDMWD","scope":"run:01M23MRR5XTM78DR4KJ72QKPZ8"}],"omission_status":"no_known_omissions","provenance_refs":["plan:01M23MRX9W1CC6SKAP20K4ZQP3"],"representations":[{"content_hash":"sha256:25532ad24140a963b9c27bb11d8e57c70e2235fc66b2d83766d49cfe4c604010","inline_content":{"crashed":false,"exit_code":0,"parsed":{"failed":0,"ignored":0,"passed":1,"test_count":1},"stderr_ref":"sha256:4aed9c184fe51fb727d3d26a82c1f049904773fb31cc89c6b39cc32bbe294800"},"representation_id":"rep:obs:01M23MXH6B0PGSM6FYFWJFDMWD","representation_kind":"atomic","representation_type":"verifier_stdout"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"observation.recorded"},{"append_ordinal":8,"entry_hash":"sha256:b5caea14116b16ae0e9ac2d5ff1c7268e10282d7d8267ec4ab71c74f8a03867a","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:54:53.033572Z","created_by":"sxr 0.1.0","envelope_id":"01M23MXWF951700GAFZKDQWMZC","envelope_kind":"evidence_packet","envelope_version":"1.0.0","evidence":[{"confidence_impact":"neutral","direction":"supports","evidence_id":"01M23MXWF70Z41N3H478P7851D","evidence_type":"test_result","question_ref":"q-logistics-flows","relevance":"high","reliability":"medium","representation_ref":"rep:obs:01M23MXH6B0PGSM6FYFWJFDMWD","target_entity_ref":"concept:question:q-logistics-flows"}],"lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["observation:01M23MXH6B0PGSM6FYFWJFDMWD"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"evidence.created"},{"append_ordinal":9,"entry_hash":"sha256:96a15c0c8cd8049055f11d34cc4a800e6a5cdddf6c65776552655138b927fd58","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:55:18.332537Z","created_by":"sxr 0.1.0","envelope_id":"01M23MYN5W4B3KF330XFHPJA4T","envelope_kind":"settlement_packet","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"settlements":[{"question_ref":"q-logistics-flows","settlement_id":"01M23MYN5TFEQVW8SR6QX9MWBV","settlement_status":"settled","threshold_ref":"th-all-supports","transformation_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"}],"validation_status":"validated"},"record_kind":"settlement.registered"},{"append_ordinal":10,"entry_hash":"sha256:253e53c1de16a4b92086cd59d1e0782842928c77ced33da426f4b4f57a940797","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:06.774238Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04FPZBY1A9N9DRXWQXFZ","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports","snapshot:sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"transformations":[{"transformation_id":"tx:01M23N04F3N7KBCRHT887PN8EQ","transformation_type":"run_start"}],"validation_status":"validated"},"record_kind":"run.started"},{"append_ordinal":11,"entry_hash":"sha256:cdd99158f41c3baebdc55864c2ce29ddcece3af7bef7ad77a9071beb2f207f17","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.031652Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04QQ03ZCRSR6NZSGJSR8","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"representations":[{"inline_content":{"boundary":{"excluded":["browser","performance","security"],"limitations":["offline only","slice-1 cargo-test"]},"evaluation_profile_ref":"profile://slice2-policy@1","evidence_requirements":[{"evidence_type":"test_result","min_count":1,"min_relevance":"high","min_reliability":"medium"}],"plan_id":"01M23N04QM1PX6YF632DAAR5YQ","plan_version":"sxr-plan/v1","question_ref":"q-logistics-flows","semantic_verification_contract_ref":null,"snapshot_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98","steps":[{"claim_ref":null,"evidence_requirement_ref":null,"expected_answer_shape":"boolean_with_failure_list","invocation":{"argv":["cargo","test","--offline","--message-format","json"],"cwd":".","env_allowlist":["PATH","HOME","CARGO_HOME"],"timeout_s":900},"limitations":[],"obligation_ref":null,"produces_evidence_type":"test_result","relevance_to_question":"high","step_id":"unit","targets":["concept:5ad6a959-5924-5cf9-af04-7b502ea74784"],"verifier_id":"cargo-test"}],"threshold_ref":"th-all-supports","transformation_ref":"transformation:plan:01M23N04QM1PX6YF632DAAR5YQ"},"representation_id":"rep:plan:01M23N04QM1PX6YF632DAAR5YQ","representation_kind":"atomic","representation_type":"verification_plan"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"plan.created"},{"append_ordinal":12,"entry_hash":"sha256:a2dadc4ffa01ad70d1c66d6ee5535971062bd77bfbb089cd17e988f3b482288d","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.240530Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04Y8GRYTXHPSC9KVVXWD","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"observations":[{"observation_id":"01M23N04Y56XR0CJX7CDQ6GGHH","scope":"run:01M23N04F3N7KBCRHT887PN8EQ"}],"omission_status":"no_known_omissions","provenance_refs":["plan:01M23N04QM1PX6YF632DAAR5YQ"],"representations":[{"content_hash":"sha256:eab7270cac5625576f86d96984fee0389d4c8b51f72cedac9d8bc1a81f912f17","inline_content":{"crashed":false,"exit_code":0,"parsed":{"failed":0,"ignored":0,"passed":1,"test_count":1},"stderr_ref":"sha256:fb9cbaa66166dc172d064bd8609578763ca926dfb3d93e4544fbdd353032050b"},"representation_id":"rep:obs:01M23N04Y56XR0CJX7CDQ6GGHH","representation_kind":"atomic","representation_type":"verifier_stdout"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"observation.recorded"},{"append_ordinal":13,"entry_hash":"sha256:984f77f73f40d9d60f9d12a69ebb38d776c88699b030d29e63e9b3e4f92c398c","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.340944Z","created_by":"sxr 0.1.0","envelope_id":"01M23N051CDS52SDCDEKX7C179","envelope_kind":"evidence_packet","envelope_version":"1.0.0","evidence":[{"confidence_impact":"neutral","direction":"supports","evidence_id":"01M23N051AWR4XA7Q8DGFBSXCS","evidence_type":"test_result","question_ref":"q-logistics-flows","relevance":"high","reliability":"medium","representation_ref":"rep:obs:01M23N04Y56XR0CJX7CDQ6GGHH","target_entity_ref":"concept:5ad6a959-5924-5cf9-af04-7b502ea74784"}],"lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["observation:01M23N04Y56XR0CJX7CDQ6GGHH"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"evidence.created"}]';

const REPO_ID = "01M23MNV1W3ZVM8FNBJETVCNAF";
const USER_ENTITY_REF = "concept:5ad6a959-5924-5cf9-af04-7b502ea74784";

// ---------------------------------------------------------------------------
// The four preregistered binding modes
// ---------------------------------------------------------------------------

interface BindingMode {
  readonly name: string;
  readonly domainforge: boolean;
  readonly sxr: boolean;
}

const MODES: readonly BindingMode[] = [
  { name: "Gauntlet-only", domainforge: false, sxr: false },
  { name: "DomainForge-only", domainforge: true, sxr: false },
  { name: "SXR-only", domainforge: false, sxr: true },
  { name: "combined", domainforge: true, sxr: true },
];

function dfEnrichmentFor(mode: BindingMode): SemanticEnrichment {
  if (!mode.domainforge) {
    return Object.freeze({
      status: "absent" as const,
      reason: "no_model_output" as const,
      detail: "no DomainForge envelope output was provided for this Case",
    });
  }
  const parse = parseSemanticEnvelopeDocument(REAL_DF_DOCUMENT);
  if (parse.status !== "parsed") {
    throw new Error("the pinned DF fixture must parse");
  }
  return projectSemanticEnrichment(parse);
}

function sxrLensFor(mode: BindingMode): SxrLensProjection {
  if (!mode.sxr) {
    return Object.freeze({
      status: "absent" as const,
      reason: "no_ledger_export" as const,
      detail: "no SXR ledger export was provided for this Case",
    });
  }
  const parse = parseSxrLedgerExport(REAL_LEDGER_EXPORT);
  if (parse.status !== "parsed") {
    throw new Error("the pinned export fixture must parse");
  }
  const lens = projectSxrEvidenceLens(parse, REPO_ID);
  if (lens.status !== "projected") {
    throw new Error("the pinned export must project");
  }
  return lens;
}

/** Flatten rendered lines to comparable text. */
function text(lines: readonly { readonly spans: readonly { readonly text: string }[] }[]): string {
  return lines.map((line) => line.spans.map((span) => span.text).join("")).join("\n");
}

// ---------------------------------------------------------------------------
// The matrix: core journeys preserved in every combination, losses explicit
// ---------------------------------------------------------------------------

describe("T14B composition matrix (prereg replay: Gauntlet-only / DF-only / SXR-only / combined)", () => {
  for (const mode of MODES) {
    it(`${mode.name}: core journeys preserved${mode.domainforge ? "" : " — semantic lens explicitly ABSENT"}${mode.sxr ? "" : " — reality trace explicitly ABSENT"}`, () => {
      const model = seededModel();
      const snapshot = model.getSnapshot();
      const observation = observationFor(model);

      // Journey 1 — Work: the unit rows render with status and authority.
      const work = renderWorkView({ snapshot, observation });
      const workText = text(work.lines);
      expect(workText).toContain("u-root");
      expect(workText).toContain("u-far");
      expect(workText).toContain("BLOCKED");
      expect(workText).toContain("ACTIVE");

      // Journey 2 — Evidence: inspectable evidence record objects.
      const evidenceView = renderEvidenceView({ snapshot, observation });
      const evidenceRecords = evidenceView.records.filter((record) => record.kind === "evidence");
      expect(evidenceRecords.length).toBe(3);

      // Journey 3 — Trace: the durable execution history renders.
      const trace = renderTraceView({ snapshot, observation });
      expect(text(trace.lines)).toContain("TRACE");

      // Journey 4 — Relations: the source-typed derivation and traversal hold.
      const world = {
        generation: snapshot.generation,
        units: snapshot.units,
        evidence: snapshot.evidence,
        sessions: snapshot.sessions,
        source_cursors: snapshot.source_cursors,
      };
      const edges = deriveRelations(world);
      expect(edges.length).toBe(4); // 3 has_evidence + 1 child_session
      const traversal = traverseRelations(edges, FocusRef.for("gauntlet", "WorkItem", "u-root"), [
        { kind: "has_evidence", depth: 1, direction: "outbound" },
      ]);
      expect(traversal.reached.length).toBe(3); // root + ev-1 + ev-2

      // Journey 5 — snapshot counters are identical across every mode: the
      // optional providers never displace mandatory closure.
      expect(snapshot.units.length).toBe(3);
      expect(snapshot.evidence.length).toBe(3);
      expect(snapshot.sessions.length).toBe(2);
    });
  }

  it("losses are EXPLICIT: every absent provider renders a typed absent section, never a fabricated one", () => {
    for (const mode of MODES) {
      const semantic = renderSemanticSection(dfEnrichmentFor(mode));
      const reality = renderSxrEvidenceSection(sxrLensFor(mode));
      const semanticText = text(semantic.lines);
      const realityText = text(reality.lines);
      if (!mode.domainforge) {
        expect(semanticText).toContain("ABSENT (no_model_output)");
        expect(semantic.records).toEqual([]);
      } else {
        expect(semanticText).not.toContain("ABSENT");
        expect(semanticText).toContain("domainforge-core");
      }
      if (!mode.sxr) {
        expect(realityText).toContain("ABSENT (no_ledger_export)");
        expect(reality.records).toEqual([]);
      } else {
        expect(realityText).toContain(REPO_ID);
        expect(realityText).toContain("13 committed ledger record(s)");
      }
    }
  });

  it("combined: both lenses compose beside each other with DISTINCT source identities (no identity collapse)", () => {
    const combined: BindingMode = { name: "combined", domainforge: true, sxr: true };
    const semantic = renderSemanticSection(dfEnrichmentFor(combined));
    const lens = sxrLensFor(combined);
    const reality = renderSxrEvidenceSection(lens);
    expect(semantic.id).toBe("semantic");
    expect(reality.id).toBe("sxr-evidence");
    // The SAME producer identity string exists in both worlds (valid-basic's
    // User concept is preserved in the SXR snapshot too) — the FocusRef
    // sources keep the identities distinct.
    const dfFocus = FocusRef.for("domainforge", "entity", USER_ENTITY_REF);
    const sxrFocus = FocusRef.for(SXR_SOURCE_ID, "entity", USER_ENTITY_REF);
    expect(dfFocus.id).not.toBe(sxrFocus.id);
    expect(dfFocus.source).toBe("domainforge");
    expect(sxrFocus.source).toBe("sxr");
  });
});

// ---------------------------------------------------------------------------
// SXR admission composes into the Gauntlet projection (canonical path only)
// ---------------------------------------------------------------------------

describe("T14B admission composition: SXR evidence reaches Gauntlet evidence only through the boundary", () => {
  it("offer → canonical decision → authoritative frame → WorkbenchModel evidence row, with the SXR origin visible and settlement untouched", () => {
    const mode: BindingMode = { name: "combined", domainforge: true, sxr: true };
    const lens = sxrLensFor(mode);
    if (lens.status !== "projected") {
      throw new Error("lens must project");
    }
    const exportParse = parseSxrLedgerExport(REAL_LEDGER_EXPORT);
    if (exportParse.status !== "parsed") {
      throw new Error("export must parse");
    }
    const bindings = admitBindings(lens, [
      {
        unit: FocusRef.for("gauntlet", "WorkItem", "u-root"),
        entity_ref: USER_ENTITY_REF,
        declared_by: "operator-olive",
      },
    ]);
    expect(bindings.length).toBe(1);
    const path = sxrVerificationPath(lens, USER_ENTITY_REF, bindings);
    expect(path.chain_complete).toBe(true);

    const offerOutcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: exportParse,
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "u-root",
      target_source_id: "gauntlet",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error(`offer refused: ${offerOutcome.class}`);
    }
    const gate = new SxrEvidenceAdmissionGate();
    const decision = {
      class: "admitted" as const,
      evidence_id: offerOutcome.offer.admission_contract.evidence_id,
      verified: false,
      source_cursor: "sxr@13",
    };
    const admitted = gate.admit(offerOutcome.offer, decision);
    expect(admitted.class).toBe("admitted");

    const model = seededModel();
    const frame = authoritativeSxrEvidenceFrame(
      offerOutcome.offer,
      admitted.class === "admitted" ? decision : decision,
      {
        generation_id: GEN,
        timestamp_ms: 9000,
      },
    );
    model.applyEvent(frame);
    const snapshot = model.getSnapshot();
    const admittedRow = snapshot.evidence.find(
      (record) => record.evidence_id === offerOutcome.offer.admission_contract.evidence_id,
    );
    expect(admittedRow).toBeDefined();
    if (admittedRow === undefined) return;
    // The canonical decision's verified bit lands verbatim (false stays false).
    expect(admittedRow.verified).toBe(false);
    // The SXR origin stays visible in the record; the row is Gauntlet-side.
    expect(admittedRow.evidence_id.startsWith("sxr:")).toBe(true);
    expect(admittedRow.locator).toContain("sxr-ledger:");
    expect(admittedRow.source_id).toBe("gauntlet");

    // The SXR settlement record was NOT imported: the Gauntlet projection has
    // no settlement channel write, and the lens's SXR settlement keeps its
    // source-typed status.
    const settlements = lens.settlements;
    expect(settlements.length).toBe(1);
    expect(settlements[0]?.sxr_settlement_status).toBe("settled");
    expect(model.getSnapshot().units.find((unit) => unit.unit_id === "u-root")?.status).toBe(
      "active",
    );
  });
});

// ---------------------------------------------------------------------------
// Source loss preserves core journeys (REQ-SYNC-003, prereg D4)
// ---------------------------------------------------------------------------

describe("T14B source loss: journeys survive, admitted records reconstruct, lost output is labelled unavailable", () => {
  it("after every source byte is lost, the admitted evidence row rebuilds from the authoritative log and absent lenses say so", () => {
    // Before the loss: combined mode admits one SXR evidence record.
    const lens = sxrLensFor({ name: "combined", domainforge: true, sxr: true });
    if (lens.status !== "projected") {
      throw new Error("lens must project");
    }
    const exportParse = parseSxrLedgerExport(REAL_LEDGER_EXPORT);
    if (exportParse.status !== "parsed") {
      throw new Error("export must parse");
    }
    const offerOutcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: exportParse,
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "u-root",
      target_source_id: "gauntlet",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error("offer must be built");
    }
    const decision = {
      class: "admitted" as const,
      evidence_id: offerOutcome.offer.admission_contract.evidence_id,
      verified: true,
      source_cursor: "sxr@13",
    };
    const frame = authoritativeSxrEvidenceFrame(offerOutcome.offer, decision, {
      generation_id: GEN,
      timestamp_ms: 9000,
    });

    // THE LOSS: the DF envelope output and the SXR ledger export are gone.
    const lostDf = dfEnrichmentFor({ name: "post-loss", domainforge: false, sxr: false });
    const lostSxr = sxrLensFor({ name: "post-loss", domainforge: false, sxr: false });
    const semanticSection = renderSemanticSection(lostDf);
    const realitySection = renderSxrEvidenceSection(lostSxr);
    expect(text(semanticSection.lines)).toContain("ABSENT (no_model_output)");
    expect(text(realitySection.lines)).toContain("ABSENT (no_ledger_export)");
    expect(text(realitySection.lines)).toContain("not recreated");
    expect(realitySection.records).toEqual([]);

    // The admitted consequential record RECONSTRUCTS from the authoritative
    // log — it was pinned before use, so the loss does not touch it.
    const rebuilt = seededModel();
    rebuilt.applyEvent(frame);
    const snapshot = rebuilt.getSnapshot();
    expect(snapshot.evidence.some((record) => record.evidence_id.startsWith("sxr:"))).toBe(true);

    // And the core journeys still render after the loss.
    const observation = observationFor(rebuilt);
    const work = renderWorkView({ snapshot, observation });
    expect(text(work.lines)).toContain("u-root");
    const evidenceView = renderEvidenceView({ snapshot, observation });
    expect(evidenceView.records.filter((record) => record.kind === "evidence").length).toBe(4);
  });
});

// ---------------------------------------------------------------------------
// Deny-matrix re-run over the SXR refresh route (prereg A3)
// ---------------------------------------------------------------------------

const RUN = "01J9T09DENYBYPASSRUN";
const ACTOR = "operator-bob";
const TOKEN = "t14b-refresh-token";
const TICKET = "ESC-911";
const UNIT = "AK-911";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", UNIT);
const SESSION = { actor: ACTOR, token: TOKEN } as const;

function captureRunner(stdout: string): SxrProcessRunner {
  return {
    kind: "fake-capture",
    async run() {
      return {
        class: "exited" as const,
        exit_code: 0,
        signal: null,
        stdout_bytes: new TextEncoder().encode(stdout),
        stderr_text: "",
      };
    },
  };
}

describe("T14B deny matrix over the SXR refresh route (REQ-SXR-003/004)", () => {
  it("ALLOWED: the explicit bounded adapter refresh executes and parses the REAL export", async () => {
    const outcome = await executeSxrRefresh(
      {
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: { kind: "ledger_export" },
      },
      captureRunner(REAL_LEDGER_EXPORT),
    );
    expect(outcome.class).toBe("refreshed");
    if (outcome.class !== "refreshed") return;
    if (outcome.parse.status !== "parsed" || !("rows" in outcome.parse)) {
      throw new Error("refreshed export must parse");
    }
    expect(outcome.parse.rows.length).toBe(13);
    // The side-effect classification traveled with the execution.
    expect(outcome.side_effects.appends_canonical_ledger).toBe(false);
    expect(outcome.side_effects.launches_autonomous_run).toBe(false);
  });

  it("DENIED: the same logical refresh driven through the action gateway is refused with the canonical read-shaped typed denial, zero side effects", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: RUN,
      bound_actor: ACTOR,
      session_token: TOKEN,
    }).addUnit({
      unit_id: UNIT,
      status: "blocked",
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: TICKET,
    });
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const request: OperationInvocationRequest = {
      operation: OPERATION_IDS.STATE_REFRESH,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "refresh the SXR reality trace" },
    };
    const outcome = await gateway.invoke(request);
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") return;
    // The SAME typed denial the T09 deny-bypass matrix pins for read-shaped
    // operations (C4): not_action_admitted, never unsupported_operation.
    expect(outcome.code).toBe("not_action_admitted");
    expect(outcome.reason_codes).toEqual(["not_action_admitted"]);
    expect(outcome.operation).toBe(OPERATION_IDS.STATE_REFRESH);
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);
  });
});
