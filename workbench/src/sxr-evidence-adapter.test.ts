import { afterAll, describe, expect, it } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  parseProducerEnvelopeOutput,
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
} from "./domainforge-adapter";
import { FocusRef } from "./focus";
import { SqliteProjectionStore } from "./projection-store";
import type { SxrLedgerExportParse } from "./sxr-adapter";
import {
  admitSxrUnitBindings,
  authoritativeSxrEvidenceFrame,
  bunSxrProcessRunner,
  executeSxrRefresh,
  isPendingDurability,
  offerSxrEvidenceAsGauntletEvidence,
  parseSxrCommandOutput,
  parseSxrLedgerExport,
  parseSxrSlice0bEnvelopeOutput,
  projectSxrEvidenceLens,
  QUALIFIED_SXR_BUILDS,
  renderSxrEvidenceSection,
  SXR_LIVE_QUALIFICATION,
  SxrBindingError,
  type SxrBoundRefresh,
  type SxrClock,
  SxrEvidenceAdmissionGate,
  type SxrProcessResult,
  type SxrProcessRunner,
  SxrRefreshBoundaryError,
  sxrEntityRef,
  sxrVerificationPath,
  systemSxrClock,
  validateSxrRefreshRequest,
} from "./sxr-adapter";

/**
 * T14B SXR Reality Trace evidence adapter suites (plan T14B, REQ-SXR-001..004,
 * REQ-DOM-004/005, REQ-SYNC-003).
 *
 * Fixtures are REAL captured producer/consumer outputs (byte-exact compact
 * JSON of the pinned captures under
 * .agents/reports/gauntlet-tui-prime-workbench/evidence/T14B/captures/,
 * digests pinned in CAPTURES.sha256), captured 2026-09-09 with:
 *   - sxr 0.1.0 (sha256 89c006fb…) driven through a full real lifecycle on an
 *     isolated XDG root;
 *   - the Slice 0B producer (domainforge-slice0b build, sha256 f0ca9def…)
 *     answering `envelope --emit both`.
 * Synthetic shapes (each disclosed at its use) exist only where the bounded
 * negative matrix needs a shape no real run produces; they wrap REAL
 * digest-verified document bytes. The live tests run the REAL binaries and
 * stay visible skips when the pinned binaries are absent (prereg R1).
 */

// --- REAL capture: `sxr repo enroll <git repo>` (isolated XDG root) ----------
const REAL_ENROLL_OUTPUT =
  '{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","durability":"committed","filesystem_path":"/tmp/t14b-capture/repo","identity_evidence":{"checkout_id":null,"early_tree_hashes":[],"enrollment_path":"/tmp/t14b-capture/repo","evidence_status":"sufficient","first_seen_at":"2026-09-09T17:50:29.436370Z","head_at_enrollment":"3c98f1f4048e0f8d439e6d96b6fed077d23b9553","remotes":[],"repo_id":null,"root_commits":["3c98f1f4048e0f8d439e6d96b6fed077d23b9553"],"verdict_history":[]},"ledger_position":{"append_ordinal":1,"ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"omissions":[],"repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","state":"enrolled","sxr_output_version":"1","unknowns":[]}';

// --- REAL capture: `sxr ingest --spool` — the PENDING durability shape -------
const REAL_SPOOLED_OUTPUT =
  '{"bytes_received":1,"durability":"spooled","omissions":[],"run_id":"01M23MRR5XTM78DR4KJ72QKPZ8","spool_entry":"/tmp/t14b-capture/state/sxr/spool/01M23MNV1W3ZVM8FNBJETVCNAF/0d01af666c94a634c1441cf5e53b792d63c4ea1ede1c93139c94da21f25b8f32.ndjson","sxr_output_version":"1","unknowns":[],"verifier":"cargo-test"}';

// --- REAL capture: `sxr query evidence` — not_observed shape -----------------
const REAL_QUERY_NOT_OBSERVED =
  '{"omissions":[],"query":{"decl_key":"concept:5ad6a959-5924-5cf9-af04-7b502ea74784","decl_kind":"entity","evidence":[],"name":"User","namespace":"auth","presence":"not_observed","reason":"not_collected","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","snapshot_id":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"},"sxr_output_version":"1","unknowns":[]}';

// --- REAL capture: `sxr query evidence` — observed shape (evidence present) --
const REAL_QUERY_OBSERVED =
  '{"omissions":[],"query":{"decl_key":"concept:5ad6a959-5924-5cf9-af04-7b502ea74784","evidence":[{"confidence":null,"direction":"supports","evidence_id":"01M23N051AWR4XA7Q8DGFBSXCS","evidence_type":"test_result","question_form":"is_true","question_id":"q-logistics-flows","relevance":"high","reliability":"medium","run_id":"01M23N04F3N7KBCRHT887PN8EQ","settlement_id":null,"settlement_status":null}],"presence":"observed","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","snapshot_id":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"},"sxr_output_version":"1","unknowns":[]}';

// --- REAL capture: `sxr ledger verify` ---------------------------------------
const REAL_LEDGER_VERIFY =
  '{"first_divergence":null,"limitations":["tamper-evident, not tamper-proof: anyone with write access can rewrite the whole chain (CEP-0008 \u00a755)"],"ok":true,"omissions":[],"sxr_output_version":"1","unknowns":[],"verified_count":13}';

// --- REAL capture: `sxr ledger export --repo <id>` — the RAW array (13 rows) -
const REAL_LEDGER_EXPORT =
  '[{"append_ordinal":1,"entry_hash":"sha256:b7d99b16b450d379d7f02de8bc96b43cbc3aeec802ea12a893960e1f721bc215","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:29.438050Z","created_by":"sxr 0.1.0","envelope_id":"01M23MNV1Y67K4G7PGF1WRST1V","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["ledger:gen:1"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","filesystem_path":"/tmp/t14b-capture/repo","git_commit":"3c98f1f4048e0f8d439e6d96b6fed077d23b9553","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"states":[{"scope":"repo:01M23MNV1W3ZVM8FNBJETVCNAF","state_id":"state:enroll","state_type":"repository"}],"transformations":[{"transformation_id":"tx:enroll","transformation_type":"enrollment"}],"validation_status":"validated"},"record_kind":"repo.enrolled"},{"append_ordinal":2,"entry_hash":"sha256:8eb8f9efdb81f3fc59b64019689d1c33196da5a51805ad4fc02559325da72cc2","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:42.714953Z","created_by":"sxr 0.1.0","envelope_id":"01M23MP80TK5ZZ1A5XP5QB2RK3","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["ledger:gen:1"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows","question_type":"declared_semantics"}],"representations":[{"inline_content":{"question_form":"is_true","question_id":"q-logistics-flows","question_type":"declared_semantics","target_entities":"concept:order_flow"},"representation_id":"rep:question:q-logistics-flows","representation_kind":"atomic","representation_type":"question_spec"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"validation_status":"validated"},"record_kind":"question.declared"},{"append_ordinal":3,"entry_hash":"sha256:013e9b2c44a8a74e0da3d94d682a98c6c7c9943aa1f493ce8d4e5f0585f47a1f","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:50:42.741809Z","created_by":"sxr 0.1.0","envelope_id":"01M23MP81NFPDACVSMFMF0EVMW","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows"],"representations":[{"inline_content":{"evaluation_profile":"v2-independent-validity","threshold_id":"th-all-supports","threshold_kind":"all_evidence_supports"},"representation_id":"rep:threshold:th-all-supports","representation_kind":"atomic","representation_type":"settlement_threshold_spec"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF"},"states":[{"scope":"question:q-logistics-flows","state_id":"th-all-supports","state_type":"settlement_threshold"}],"validation_status":"validated"},"record_kind":"threshold.declared"},{"append_ordinal":4,"entry_hash":"sha256:4f0c91c1c75cc1ac12cc1c57ef5c0419d89c43aad4a8ec7ef671ff36c7a2d53a","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["representations","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"valid-basic.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:51:21.395604Z","created_by":"domainforge 0.16.0","envelope_id":"01a0874b-b733-7d32-8ce2-13682e26cd30","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","extensions":{"domainforge":{"model_validation_status":"valid","semantics_version":"v2-independent-validity"}},"lineage_refs":[],"omission_status":"known_omissions_recorded","provenance":[{"created_at":"2026-09-09T17:51:21.395604Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a0874b-b733-7d32-8ce2-13682e26cd30","review_status":"unreviewed","source_system_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","ref_type":"source_set","scope":"valid-basic.sea","target_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","target_uri_or_id":"valid-basic.sea"}],"representations":[{"content_availability":"available","content_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","inline_content":"{\\"envelope\\":{\\"application_contract\\":{\\"entities\\":[],\\"enums\\":[],\\"operations\\":[],\\"records\\":[]},\\"canonicalization_version\\":\\"domainforge-canonicalization/v1\\",\\"compiler_interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"import_graph\\":[],\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"modules\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\",\\"semantic_content_hash\\":\\"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29\\"}],\\"namespace_bindings\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\"}],\\"resolved_references\\":[],\\"semantic_declarations\\":[{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"User\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Role\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Token\\",\\"unit\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"7a50d9d2-89e2-5607-bca5-34464284bea8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"resource\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"a81c1fec-c10a-5541-bd81-98c64b10f402\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"}],\\"semantic_packs\\":[]},\\"inputs\\":{\\"interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"semantic_pack_set_hash\\":\\"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40\\",\\"source_set_hash\\":\\"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5\\"},\\"producer\\":{\\"name\\":\\"domainforge-core\\",\\"version\\":\\"0.16.0\\"},\\"schema_version\\":\\"domainforge-semantic-envelope/v1\\",\\"self_hash\\":\\"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e\\",\\"semantic_closure_hash\\":\\"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c\\"}","media_type":"application/json","omission_record":{"known_omissions":["source_comments","source_formatting","source_line_positions"],"reason":"domainforge canonical envelope is formatting- and comment-independent by construction","recoverable_from":"the .sea sources at the recorded git blob hashes"},"preserved_distinctions":["declared_vs_observed","representation_vs_reality"],"preserved_entities":["concept:5ad6a959-5924-5cf9-af04-7b502ea74784","concept:72f70c63-6f3e-5061-8854-42e540bbac87","concept:a81c1fec-c10a-5541-bd81-98c64b10f402","occurrence:flow:sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543:0","occurrence:flow:sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66:0"],"provenance_record":{"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"resolution_inputs":{"entry_logical_path":"valid-basic.sea","registry_content_hash":null,"resolved_namespaces":{"valid-basic.sea":"auth"}}},"representation_id":"representation:sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","representation_kind":"derived","representation_type":"domainforge_semantic_envelope_document","scope":"valid-basic.sea","semantic_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","size_bytes":3195,"source":"domainforge 0.16.0","validation_status":"valid"}],"scope":{},"validation_status":"validated"},"record_kind":"semantic.snapshot"},{"append_ordinal":5,"entry_hash":"sha256:aa309b4bab6f0bf7a8e60f92107cb49f3e2a4bf7d91f510604c6425c0e2fcdda","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:52:04.799082Z","created_by":"sxr 0.1.0","envelope_id":"01M23MRR5ZH0RYT10CZ8CZ8WDW","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports","snapshot:sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"transformations":[{"transformation_id":"tx:01M23MRR5XTM78DR4KJ72QKPZ8","transformation_type":"run_start"}],"validation_status":"validated"},"record_kind":"run.started"},{"append_ordinal":6,"entry_hash":"sha256:1d9f93dfd4b2e97232d8bba2a129b578b10cbd8df4b266e97c2faf251d8ff54d","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:52:10.045981Z","created_by":"sxr 0.1.0","envelope_id":"01M23MRX9XD526664SEQG5FJGJ","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"representations":[{"inline_content":{"boundary":{"excluded":["browser","performance","security"],"limitations":["offline only","slice-1 cargo-test"]},"evaluation_profile_ref":"profile://slice2-policy@1","evidence_requirements":[{"evidence_type":"test_result","min_count":1,"min_relevance":"high","min_reliability":"medium"}],"plan_id":"01M23MRX9W1CC6SKAP20K4ZQP3","plan_version":"sxr-plan/v1","question_ref":"q-logistics-flows","semantic_verification_contract_ref":null,"snapshot_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98","steps":[{"claim_ref":null,"evidence_requirement_ref":null,"expected_answer_shape":"boolean_with_failure_list","invocation":{"argv":["cargo","test","--offline","--message-format","json"],"cwd":".","env_allowlist":["PATH","HOME","CARGO_HOME"],"timeout_s":900},"limitations":[],"obligation_ref":null,"produces_evidence_type":"test_result","relevance_to_question":"high","step_id":"unit","targets":["question:q-logistics-flows"],"verifier_id":"cargo-test"}],"threshold_ref":"th-all-supports","transformation_ref":"transformation:plan:01M23MRX9W1CC6SKAP20K4ZQP3"},"representation_id":"rep:plan:01M23MRX9W1CC6SKAP20K4ZQP3","representation_kind":"atomic","representation_type":"verification_plan"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"plan.created"},{"append_ordinal":7,"entry_hash":"sha256:db469d27d0688d8793983b01fc9662c3327d651de27aaa8cde0a82241c6e1696","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:54:41.484801Z","created_by":"sxr 0.1.0","envelope_id":"01M23MXH6CC0R4Z050GPBX94WF","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"observations":[{"observation_id":"01M23MXH6B0PGSM6FYFWJFDMWD","scope":"run:01M23MRR5XTM78DR4KJ72QKPZ8"}],"omission_status":"no_known_omissions","provenance_refs":["plan:01M23MRX9W1CC6SKAP20K4ZQP3"],"representations":[{"content_hash":"sha256:25532ad24140a963b9c27bb11d8e57c70e2235fc66b2d83766d49cfe4c604010","inline_content":{"crashed":false,"exit_code":0,"parsed":{"failed":0,"ignored":0,"passed":1,"test_count":1},"stderr_ref":"sha256:4aed9c184fe51fb727d3d26a82c1f049904773fb31cc89c6b39cc32bbe294800"},"representation_id":"rep:obs:01M23MXH6B0PGSM6FYFWJFDMWD","representation_kind":"atomic","representation_type":"verifier_stdout"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"observation.recorded"},{"append_ordinal":8,"entry_hash":"sha256:b5caea14116b16ae0e9ac2d5ff1c7268e10282d7d8267ec4ab71c74f8a03867a","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:54:53.033572Z","created_by":"sxr 0.1.0","envelope_id":"01M23MXWF951700GAFZKDQWMZC","envelope_kind":"evidence_packet","envelope_version":"1.0.0","evidence":[{"confidence_impact":"neutral","direction":"supports","evidence_id":"01M23MXWF70Z41N3H478P7851D","evidence_type":"test_result","question_ref":"q-logistics-flows","relevance":"high","reliability":"medium","representation_ref":"rep:obs:01M23MXH6B0PGSM6FYFWJFDMWD","target_entity_ref":"concept:question:q-logistics-flows"}],"lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["observation:01M23MXH6B0PGSM6FYFWJFDMWD"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"validation_status":"validated"},"record_kind":"evidence.created"},{"append_ordinal":9,"entry_hash":"sha256:96a15c0c8cd8049055f11d34cc4a800e6a5cdddf6c65776552655138b927fd58","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:55:18.332537Z","created_by":"sxr 0.1.0","envelope_id":"01M23MYN5W4B3KF330XFHPJA4T","envelope_kind":"settlement_packet","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23MRR5XTM78DR4KJ72QKPZ8"},"settlements":[{"question_ref":"q-logistics-flows","settlement_id":"01M23MYN5TFEQVW8SR6QX9MWBV","settlement_status":"settled","threshold_ref":"th-all-supports","transformation_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"}],"validation_status":"validated"},"record_kind":"settlement.registered"},{"append_ordinal":10,"entry_hash":"sha256:253e53c1de16a4b92086cd59d1e0782842928c77ced33da426f4b4f57a940797","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:06.774238Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04FPZBY1A9N9DRXWQXFZ","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports","snapshot:sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"transformations":[{"transformation_id":"tx:01M23N04F3N7KBCRHT887PN8EQ","transformation_type":"run_start"}],"validation_status":"validated"},"record_kind":"run.started"},{"append_ordinal":11,"entry_hash":"sha256:cdd99158f41c3baebdc55864c2ce29ddcece3af7bef7ad77a9071beb2f207f17","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.031652Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04QQ03ZCRSR6NZSGJSR8","envelope_kind":"work_request","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["question:q-logistics-flows","threshold:th-all-supports"],"questions":[{"question_form":"is_true","question_id":"q-logistics-flows"}],"representations":[{"inline_content":{"boundary":{"excluded":["browser","performance","security"],"limitations":["offline only","slice-1 cargo-test"]},"evaluation_profile_ref":"profile://slice2-policy@1","evidence_requirements":[{"evidence_type":"test_result","min_count":1,"min_relevance":"high","min_reliability":"medium"}],"plan_id":"01M23N04QM1PX6YF632DAAR5YQ","plan_version":"sxr-plan/v1","question_ref":"q-logistics-flows","semantic_verification_contract_ref":null,"snapshot_ref":"sha256:037f0b2471f6920af2994b05273a9f5aafe2228197936ba657d3f3b662a5cc98","steps":[{"claim_ref":null,"evidence_requirement_ref":null,"expected_answer_shape":"boolean_with_failure_list","invocation":{"argv":["cargo","test","--offline","--message-format","json"],"cwd":".","env_allowlist":["PATH","HOME","CARGO_HOME"],"timeout_s":900},"limitations":[],"obligation_ref":null,"produces_evidence_type":"test_result","relevance_to_question":"high","step_id":"unit","targets":["concept:5ad6a959-5924-5cf9-af04-7b502ea74784"],"verifier_id":"cargo-test"}],"threshold_ref":"th-all-supports","transformation_ref":"transformation:plan:01M23N04QM1PX6YF632DAAR5YQ"},"representation_id":"rep:plan:01M23N04QM1PX6YF632DAAR5YQ","representation_kind":"atomic","representation_type":"verification_plan"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"plan.created"},{"append_ordinal":12,"entry_hash":"sha256:a2dadc4ffa01ad70d1c66d6ee5535971062bd77bfbb089cd17e988f3b482288d","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.240530Z","created_by":"sxr 0.1.0","envelope_id":"01M23N04Y8GRYTXHPSC9KVVXWD","envelope_kind":"execution_trace","envelope_version":"1.0.0","lineage_refs":[],"observations":[{"observation_id":"01M23N04Y56XR0CJX7CDQ6GGHH","scope":"run:01M23N04F3N7KBCRHT887PN8EQ"}],"omission_status":"no_known_omissions","provenance_refs":["plan:01M23N04QM1PX6YF632DAAR5YQ"],"representations":[{"content_hash":"sha256:eab7270cac5625576f86d96984fee0389d4c8b51f72cedac9d8bc1a81f912f17","inline_content":{"crashed":false,"exit_code":0,"parsed":{"failed":0,"ignored":0,"passed":1,"test_count":1},"stderr_ref":"sha256:fb9cbaa66166dc172d064bd8609578763ca926dfb3d93e4544fbdd353032050b"},"representation_id":"rep:obs:01M23N04Y56XR0CJX7CDQ6GGHH","representation_kind":"atomic","representation_type":"verifier_stdout"}],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"observation.recorded"},{"append_ordinal":13,"entry_hash":"sha256:984f77f73f40d9d60f9d12a69ebb38d776c88699b030d29e63e9b3e4f92c398c","ledger_id":"01M23MNV1W3ZVM8FNBJETVCNAF","payload":{"boundary_record":{"excluded_sections":[],"included_sections":[],"known_omissions":[],"limitations":["sxr slice-1 runtime record"],"unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T17:56:07.340944Z","created_by":"sxr 0.1.0","envelope_id":"01M23N051CDS52SDCDEKX7C179","envelope_kind":"evidence_packet","envelope_version":"1.0.0","evidence":[{"confidence_impact":"neutral","direction":"supports","evidence_id":"01M23N051AWR4XA7Q8DGFBSXCS","evidence_type":"test_result","question_ref":"q-logistics-flows","relevance":"high","reliability":"medium","representation_ref":"rep:obs:01M23N04Y56XR0CJX7CDQ6GGHH","target_entity_ref":"concept:5ad6a959-5924-5cf9-af04-7b502ea74784"}],"lineage_refs":[],"omission_status":"no_known_omissions","provenance_refs":["observation:01M23N04Y56XR0CJX7CDQ6GGHH"],"scope":{"checkout_id":"01M23MNV1WK26DR10T0EGH4NJ3","repo_id":"01M23MNV1W3ZVM8FNBJETVCNAF","run_id":"01M23N04F3N7KBCRHT887PN8EQ"},"validation_status":"validated"},"record_kind":"evidence.created"}]';

// --- REAL capture: `envelope --emit both valid-basic.sea` (object carrier) ---
const REAL_BOTH_MODE =
  '{"cep_envelope":{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["representations","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"valid-basic.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T18:04:52.923907Z","created_by":"domainforge 0.16.0","envelope_id":"01a08758-193c-7ee2-b3d1-c183f5326284","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"known_omissions_recorded","provenance":[{"created_at":"2026-09-09T18:04:52.923907Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a08758-193c-7ee2-b3d1-c183f5326284","review_status":"unreviewed","source_system_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","ref_type":"source_set","scope":"valid-basic.sea","target_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5","target_uri_or_id":"valid-basic.sea"}],"representations":[{"content_availability":"available","content_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","inline_content":"{\\"envelope\\":{\\"application_contract\\":{\\"entities\\":[],\\"enums\\":[],\\"operations\\":[],\\"records\\":[]},\\"canonicalization_version\\":\\"domainforge-canonicalization/v1\\",\\"compiler_interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"import_graph\\":[],\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"modules\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\",\\"semantic_content_hash\\":\\"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29\\"}],\\"namespace_bindings\\":[{\\"logical_id\\":\\"valid-basic.sea\\",\\"namespace\\":\\"auth\\"}],\\"resolved_references\\":[],\\"semantic_declarations\\":[{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"User\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Role\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"auth\\",\\"name\\":\\"Token\\",\\"unit\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"7a50d9d2-89e2-5607-bca5-34464284bea8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"resource\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"a81c1fec-c10a-5541-bd81-98c64b10f402\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"72f70c63-6f3e-5061-8854-42e540bbac87\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a9034d4e-9276-56a8-8543-9ee845011958\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"5ad6a959-5924-5cf9-af04-7b502ea74784\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"valid-basic.sea\\"}],\\"semantic_packs\\":[]},\\"inputs\\":{\\"interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"semantic_pack_set_hash\\":\\"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40\\",\\"source_set_hash\\":\\"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5\\"},\\"producer\\":{\\"name\\":\\"domainforge-core\\",\\"version\\":\\"0.16.0\\"},\\"schema_version\\":\\"domainforge-semantic-envelope/v1\\",\\"self_hash\\":\\"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e\\",\\"semantic_closure_hash\\":\\"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c\\"}","media_type":"application/json","omission_record":{"known_omissions":["source_comments","source_formatting","source_line_positions"],"reason":"domainforge canonical envelope is formatting- and comment-independent by construction","recoverable_from":"the .sea sources at the recorded git blob hashes"},"preserved_distinctions":["declared_vs_observed","representation_vs_reality"],"preserved_entities":["concept:5ad6a959-5924-5cf9-af04-7b502ea74784","concept:72f70c63-6f3e-5061-8854-42e540bbac87","concept:a81c1fec-c10a-5541-bd81-98c64b10f402","occurrence:flow:sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543:0","occurrence:flow:sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66:0"],"provenance_record":{"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"resolution_inputs":{"entry_logical_path":"valid-basic.sea","registry_content_hash":null,"resolved_namespaces":{"valid-basic.sea":"auth"}}},"representation_id":"representation:sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","representation_kind":"derived","representation_type":"domainforge_semantic_envelope_document","scope":"valid-basic.sea","semantic_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c","size_bytes":3195,"source":"domainforge 0.16.0","validation_status":"valid"}],"scope":{"entry_logical_path":"valid-basic.sea","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c"},"validation_status":"validated"},"representation":{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"valid-basic.sea","namespace":"auth","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29"}],"namespace_bindings":[{"logical_id":"valid-basic.sea","namespace":"auth"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"auth","name":"User"},"kind":"entity"},"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Role"},"kind":"entity"},"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Token","unit":{"data":{"id":{"data":{"id":"7a50d9d2-89e2-5607-bca5-34464284bea8"},"kind":"concept"}},"kind":"declaration"}},"kind":"resource"},"id":{"data":{"id":"a81c1fec-c10a-5541-bd81-98c64b10f402"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c"}}';

// --- REAL capture: `envelope --emit both logistics-relations.sea` — a REAL
// Relation declaration (declared roles, predicate, via flow), model-valid ----
const REAL_RELATION_BOTH =
  '{"cep_envelope":{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["representations","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"logistics-relations.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T18:09:21.645503Z","created_by":"domainforge 0.16.0","envelope_id":"01a0875c-32ed-78c2-a922-b4aac9ef6086","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","lineage_refs":[],"omission_status":"known_omissions_recorded","provenance":[{"created_at":"2026-09-09T18:09:21.645503Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a0875c-32ed-78c2-a922-b4aac9ef6086","review_status":"unreviewed","source_system_refs":["source-set:sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b","ref_type":"source_set","scope":"logistics-relations.sea","target_hash":"sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b","target_uri_or_id":"logistics-relations.sea"}],"representations":[{"content_availability":"available","content_hash":"sha256:df55cf1621b97c16206ea5074e04d0dae817b93c4c7493c34b4667f497be35d6","inline_content":"{\\"envelope\\":{\\"application_contract\\":{\\"entities\\":[],\\"enums\\":[],\\"operations\\":[],\\"records\\":[]},\\"canonicalization_version\\":\\"domainforge-canonicalization/v1\\",\\"compiler_interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"import_graph\\":[],\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"modules\\":[{\\"logical_id\\":\\"logistics-relations.sea\\",\\"namespace\\":\\"logistics\\",\\"semantic_content_hash\\":\\"sha256:bfdef511086bbac590cbdfd152125e547a5392653fceb6f5da22bcc1cd58b991\\"}],\\"namespace_bindings\\":[{\\"logical_id\\":\\"logistics-relations.sea\\",\\"namespace\\":\\"logistics\\"}],\\"resolved_references\\":[],\\"semantic_declarations\\":[{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"logistics\\",\\"name\\":\\"Supplier\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"logistics\\",\\"name\\":\\"MaterialLot\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"4544479d-add8-59cd-b989-bc505dd3676b\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"logistics\\",\\"name\\":\\"Goods\\"},\\"kind\\":\\"resource\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"f5814ca5-2184-52bc-acc4-45e2bddc78d8\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"ff768ad5-8932-517f-a7ae-671cfb18d6b8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"4544479d-add8-59cd-b989-bc505dd3676b\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:5316de9690aa7135bf786302f1a4d34ea8dde532d575e3940da49913f6d5268c\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"domain\\":\\"logistics\\",\\"name\\":\\"material_lot\\"},\\"kind\\":\\"role\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"89136ce9-2418-59ff-a587-a6801680d109\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"domain\\":\\"logistics\\",\\"name\\":\\"supplier\\"},\\"kind\\":\\"role\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"a31d3c70-a715-52ca-bacf-324016a4842b\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"},{\\"declaration\\":{\\"data\\":{\\"name\\":\\"SupplierSuppliesMaterial\\",\\"object_role\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"89136ce9-2418-59ff-a587-a6801680d109\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"predicate\\":\\"supplies\\",\\"subject_role\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a31d3c70-a715-52ca-bacf-324016a4842b\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"via_flow\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"f5814ca5-2184-52bc-acc4-45e2bddc78d8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"relation\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"bfa3e79b-28b8-5161-8d23-93807f00ae44\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"logistics-relations.sea\\"}],\\"semantic_packs\\":[]},\\"inputs\\":{\\"interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"semantic_pack_set_hash\\":\\"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40\\",\\"source_set_hash\\":\\"sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b\\"},\\"producer\\":{\\"name\\":\\"domainforge-core\\",\\"version\\":\\"0.16.0\\"},\\"schema_version\\":\\"domainforge-semantic-envelope/v1\\",\\"self_hash\\":\\"sha256:df55cf1621b97c16206ea5074e04d0dae817b93c4c7493c34b4667f497be35d6\\",\\"semantic_closure_hash\\":\\"sha256:5451b991e93b081ca52b2bf01f1f143517a2985cfc309a0da0ad7a9ce2114970\\"}","media_type":"application/json","omission_record":{"known_omissions":["source_comments","source_formatting","source_line_positions"],"reason":"domainforge canonical envelope is formatting- and comment-independent by construction","recoverable_from":"the .sea sources at the recorded git blob hashes"},"preserved_distinctions":["declared_vs_observed","representation_vs_reality"],"preserved_entities":["concept:0dbb11c7-81d0-53a6-b9be-b11f3b4d6679","concept:4544479d-add8-59cd-b989-bc505dd3676b","concept:f5814ca5-2184-52bc-acc4-45e2bddc78d8","occurrence:flow:sha256:5316de9690aa7135bf786302f1a4d34ea8dde532d575e3940da49913f6d5268c:0","concept:89136ce9-2418-59ff-a587-a6801680d109","concept:a31d3c70-a715-52ca-bacf-324016a4842b","concept:bfa3e79b-28b8-5161-8d23-93807f00ae44"],"provenance_record":{"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b"},"producer":{"name":"domainforge-core","version":"0.16.0"},"resolution_inputs":{"entry_logical_path":"logistics-relations.sea","registry_content_hash":null,"resolved_namespaces":{"logistics-relations.sea":"logistics"}}},"representation_id":"representation:sha256:df55cf1621b97c16206ea5074e04d0dae817b93c4c7493c34b4667f497be35d6","representation_kind":"derived","representation_type":"domainforge_semantic_envelope_document","scope":"logistics-relations.sea","semantic_hash":"sha256:5451b991e93b081ca52b2bf01f1f143517a2985cfc309a0da0ad7a9ce2114970","size_bytes":3531,"source":"domainforge 0.16.0","validation_status":"valid"}],"scope":{"entry_logical_path":"logistics-relations.sea","semantic_closure_hash":"sha256:5451b991e93b081ca52b2bf01f1f143517a2985cfc309a0da0ad7a9ce2114970"},"validation_status":"validated"},"representation":{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"logistics-relations.sea","namespace":"logistics","semantic_content_hash":"sha256:bfdef511086bbac590cbdfd152125e547a5392653fceb6f5da22bcc1cd58b991"}],"namespace_bindings":[{"logical_id":"logistics-relations.sea","namespace":"logistics"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"logistics","name":"Supplier"},"kind":"entity"},"id":{"data":{"id":"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"annotations":{},"domain":"logistics","name":"MaterialLot"},"kind":"entity"},"id":{"data":{"id":"4544479d-add8-59cd-b989-bc505dd3676b"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"annotations":{},"domain":"logistics","name":"Goods"},"kind":"resource"},"id":{"data":{"id":"f5814ca5-2184-52bc-acc4-45e2bddc78d8"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"ff768ad5-8932-517f-a7ae-671cfb18d6b8"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"4544479d-add8-59cd-b989-bc505dd3676b"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:5316de9690aa7135bf786302f1a4d34ea8dde532d575e3940da49913f6d5268c","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"domain":"logistics","name":"material_lot"},"kind":"role"},"id":{"data":{"id":"89136ce9-2418-59ff-a587-a6801680d109"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"domain":"logistics","name":"supplier"},"kind":"role"},"id":{"data":{"id":"a31d3c70-a715-52ca-bacf-324016a4842b"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"},{"declaration":{"data":{"name":"SupplierSuppliesMaterial","object_role":{"data":{"id":{"data":{"id":"89136ce9-2418-59ff-a587-a6801680d109"},"kind":"concept"}},"kind":"declaration"},"predicate":"supplies","subject_role":{"data":{"id":{"data":{"id":"a31d3c70-a715-52ca-bacf-324016a4842b"},"kind":"concept"}},"kind":"declaration"},"via_flow":{"data":{"id":{"data":{"id":"f5814ca5-2184-52bc-acc4-45e2bddc78d8"},"kind":"concept"}},"kind":"declaration"}},"kind":"relation"},"id":{"data":{"id":"bfa3e79b-28b8-5161-8d23-93807f00ae44"},"kind":"concept"},"logical_module_id":"logistics-relations.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:b52a0614e061a0ca572c07b6d72c5da9c5c8fc1e5f8000401670177138ac578b"},"producer":{"name":"domainforge-core","version":"0.16.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:df55cf1621b97c16206ea5074e04d0dae817b93c4c7493c34b4667f497be35d6","semantic_closure_hash":"sha256:5451b991e93b081ca52b2bf01f1f143517a2985cfc309a0da0ad7a9ce2114970"}}';

// --- REAL capture: `envelope --emit both relation-invalid.sea` — exit 1, D
// constructed, diagnostics preserved, model declared invalid ------------------
const REAL_RELATION_INVALID_BOTH =
  '{"cep_envelope":{"boundary_record":{"compression_notes":[],"excluded_sections":[],"included_sections":["representations","references","provenance"],"known_omissions":["source_comments","source_formatting","source_line_positions"],"limitations":[],"out_of_scope_entities":[],"redactions":[],"scope":"relation-invalid.sea","unknowns":[]},"cep_version":"0.1.0","completeness_status":"complete_for_declared_state","created_at":"2026-09-09T18:25:31.026672Z","created_by":"domainforge 0.16.0","envelope_id":"01a0876a-fd93-7411-a222-6b626f086e78","envelope_kind":"semantic_snapshot","envelope_version":"1.0.0","extensions":{"domainforge":{"diagnostics":["artifact_schema_violation: resolved closure failed graph construction: Undefined resource: Goods at 0:0"],"model_validation_status":"invalid"}},"lineage_refs":[],"omission_status":"known_omissions_recorded","provenance":[{"created_at":"2026-09-09T18:25:31.026672Z","producer_refs":["domainforge 0.16.0"],"production_method":"domainforge envelope --emit cep","provenance_id":"provenance:01a0876a-fd93-7411-a222-6b626f086e78","review_status":"unreviewed","source_system_refs":["source-set:sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e"],"verification_status":"unverified"}],"provenance_refs":["source-set:sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e"],"references":[{"availability_status":"available","integrity_status":"verifiable","ref_id":"source-set:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e","ref_type":"source_set","scope":"relation-invalid.sea","target_hash":"sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e","target_uri_or_id":"relation-invalid.sea"}],"representations":[{"content_availability":"available","content_hash":"sha256:ba55d27694607dbc255738f1961a2dc37cba4c9db4a2db856f0a1b895f51e4e0","inline_content":"{\\"envelope\\":{\\"application_contract\\":{\\"entities\\":[],\\"enums\\":[],\\"operations\\":[],\\"records\\":[]},\\"canonicalization_version\\":\\"domainforge-canonicalization/v1\\",\\"compiler_interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"import_graph\\":[],\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"modules\\":[{\\"logical_id\\":\\"relation-invalid.sea\\",\\"namespace\\":\\"logistics\\",\\"semantic_content_hash\\":\\"sha256:fc0d4cb5a910ba29edc59e77a175abd39713b076a4960fbe9d610dc18c8d8308\\"}],\\"namespace_bindings\\":[{\\"logical_id\\":\\"relation-invalid.sea\\",\\"namespace\\":\\"logistics\\"}],\\"resolved_references\\":[],\\"semantic_declarations\\":[{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"logistics\\",\\"name\\":\\"Supplier\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"domain\\":\\"logistics\\",\\"name\\":\\"MaterialLot\\"},\\"kind\\":\\"entity\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"4544479d-add8-59cd-b989-bc505dd3676b\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"},{\\"declaration\\":{\\"data\\":{\\"annotations\\":{},\\"from_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"quantity\\":\\"1\\",\\"resource\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"f5814ca5-2184-52bc-acc4-45e2bddc78d8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"to_entity\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"4544479d-add8-59cd-b989-bc505dd3676b\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"flow\\"},\\"id\\":{\\"data\\":{\\"content_hash\\":\\"sha256:3d4a38d77f04e181f70ab0ec0d48505963df0ac208ac07e04f0130544ae871f3\\",\\"duplicate_index\\":0,\\"kind\\":\\"flow\\"},\\"kind\\":\\"occurrence\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"},{\\"declaration\\":{\\"data\\":{\\"domain\\":\\"logistics\\",\\"name\\":\\"material_lot\\"},\\"kind\\":\\"role\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"89136ce9-2418-59ff-a587-a6801680d109\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"},{\\"declaration\\":{\\"data\\":{\\"domain\\":\\"logistics\\",\\"name\\":\\"supplier\\"},\\"kind\\":\\"role\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"a31d3c70-a715-52ca-bacf-324016a4842b\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"},{\\"declaration\\":{\\"data\\":{\\"name\\":\\"SupplierSuppliesMaterial\\",\\"object_role\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"89136ce9-2418-59ff-a587-a6801680d109\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"predicate\\":\\"supplies\\",\\"subject_role\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"a31d3c70-a715-52ca-bacf-324016a4842b\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"},\\"via_flow\\":{\\"data\\":{\\"id\\":{\\"data\\":{\\"id\\":\\"f5814ca5-2184-52bc-acc4-45e2bddc78d8\\"},\\"kind\\":\\"concept\\"}},\\"kind\\":\\"declaration\\"}},\\"kind\\":\\"relation\\"},\\"id\\":{\\"data\\":{\\"id\\":\\"bfa3e79b-28b8-5161-8d23-93807f00ae44\\"},\\"kind\\":\\"concept\\"},\\"logical_module_id\\":\\"relation-invalid.sea\\"}],\\"semantic_packs\\":[]},\\"inputs\\":{\\"interpretation_version\\":\\"domainforge-interpretation/v1\\",\\"language_schema_version\\":\\"domainforge-ast/v3\\",\\"semantic_pack_set_hash\\":\\"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40\\",\\"source_set_hash\\":\\"sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e\\"},\\"producer\\":{\\"name\\":\\"domainforge-core\\",\\"version\\":\\"0.16.0\\"},\\"schema_version\\":\\"domainforge-semantic-envelope/v1\\",\\"self_hash\\":\\"sha256:ba55d27694607dbc255738f1961a2dc37cba4c9db4a2db856f0a1b895f51e4e0\\",\\"semantic_closure_hash\\":\\"sha256:84f60b80a2afe83c555505a314ff1ec6af0dfc355b1dc37066cd6c51f155ae20\\"}","media_type":"application/json","omission_record":{"known_omissions":["source_comments","source_formatting","source_line_positions"],"reason":"domainforge canonical envelope is formatting- and comment-independent by construction","recoverable_from":"the .sea sources at the recorded git blob hashes"},"preserved_distinctions":["declared_vs_observed","representation_vs_reality"],"preserved_entities":["concept:0dbb11c7-81d0-53a6-b9be-b11f3b4d6679","concept:4544479d-add8-59cd-b989-bc505dd3676b","occurrence:flow:sha256:3d4a38d77f04e181f70ab0ec0d48505963df0ac208ac07e04f0130544ae871f3:0","concept:89136ce9-2418-59ff-a587-a6801680d109","concept:a31d3c70-a715-52ca-bacf-324016a4842b","concept:bfa3e79b-28b8-5161-8d23-93807f00ae44"],"provenance_record":{"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e"},"producer":{"name":"domainforge-core","version":"0.16.0"},"resolution_inputs":{"entry_logical_path":"relation-invalid.sea","registry_content_hash":null,"resolved_namespaces":{"relation-invalid.sea":"logistics"}}},"representation_id":"representation:sha256:ba55d27694607dbc255738f1961a2dc37cba4c9db4a2db856f0a1b895f51e4e0","representation_kind":"derived","representation_type":"domainforge_semantic_envelope_document","scope":"relation-invalid.sea","semantic_hash":"sha256:84f60b80a2afe83c555505a314ff1ec6af0dfc355b1dc37066cd6c51f155ae20","size_bytes":3286,"source":"domainforge 0.16.0","validation_status":"invalid"}],"scope":{"entry_logical_path":"relation-invalid.sea","semantic_closure_hash":"sha256:84f60b80a2afe83c555505a314ff1ec6af0dfc355b1dc37066cd6c51f155ae20"},"validation_status":"validated"},"representation":{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"relation-invalid.sea","namespace":"logistics","semantic_content_hash":"sha256:fc0d4cb5a910ba29edc59e77a175abd39713b076a4960fbe9d610dc18c8d8308"}],"namespace_bindings":[{"logical_id":"relation-invalid.sea","namespace":"logistics"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"logistics","name":"Supplier"},"kind":"entity"},"id":{"data":{"id":"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679"},"kind":"concept"},"logical_module_id":"relation-invalid.sea"},{"declaration":{"data":{"annotations":{},"domain":"logistics","name":"MaterialLot"},"kind":"entity"},"id":{"data":{"id":"4544479d-add8-59cd-b989-bc505dd3676b"},"kind":"concept"},"logical_module_id":"relation-invalid.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"0dbb11c7-81d0-53a6-b9be-b11f3b4d6679"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"f5814ca5-2184-52bc-acc4-45e2bddc78d8"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"4544479d-add8-59cd-b989-bc505dd3676b"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:3d4a38d77f04e181f70ab0ec0d48505963df0ac208ac07e04f0130544ae871f3","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"relation-invalid.sea"},{"declaration":{"data":{"domain":"logistics","name":"material_lot"},"kind":"role"},"id":{"data":{"id":"89136ce9-2418-59ff-a587-a6801680d109"},"kind":"concept"},"logical_module_id":"relation-invalid.sea"},{"declaration":{"data":{"domain":"logistics","name":"supplier"},"kind":"role"},"id":{"data":{"id":"a31d3c70-a715-52ca-bacf-324016a4842b"},"kind":"concept"},"logical_module_id":"relation-invalid.sea"},{"declaration":{"data":{"name":"SupplierSuppliesMaterial","object_role":{"data":{"id":{"data":{"id":"89136ce9-2418-59ff-a587-a6801680d109"},"kind":"concept"}},"kind":"declaration"},"predicate":"supplies","subject_role":{"data":{"id":{"data":{"id":"a31d3c70-a715-52ca-bacf-324016a4842b"},"kind":"concept"}},"kind":"declaration"},"via_flow":{"data":{"id":{"data":{"id":"f5814ca5-2184-52bc-acc4-45e2bddc78d8"},"kind":"concept"}},"kind":"declaration"}},"kind":"relation"},"id":{"data":{"id":"bfa3e79b-28b8-5161-8d23-93807f00ae44"},"kind":"concept"},"logical_module_id":"relation-invalid.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:fbe8e87c41f242ad0a7fc45234dcaad016ae90adc37b492bec96776bf629ba3e"},"producer":{"name":"domainforge-core","version":"0.16.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:ba55d27694607dbc255738f1961a2dc37cba4c9db4a2db856f0a1b895f51e4e0","semantic_closure_hash":"sha256:84f60b80a2afe83c555505a314ff1ec6af0dfc355b1dc37066cd6c51f155ae20"}}';

// The REAL canonical D string inside the object carrier (representations[0]
// .inline_content of the valid-basic capture).
const REAL_BOTH_MODE_INLINE_D: string = (() => {
  const parsed = JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>;
  const envelope = parsed["cep_envelope"] as Record<string, unknown>;
  const rep0 = (envelope["representations"] as unknown[])[0] as Record<string, unknown>;
  return rep0["inline_content"] as string;
})();

// REAL identity refs from the captures (the producer's own preserved_entities).
const USER_ENTITY_REF = "concept:5ad6a959-5924-5cf9-af04-7b502ea74784";
const ROLE_ENTITY_REF = "concept:72f70c63-6f3e-5061-8854-42e540bbac87";
const RELATION_SUPPLIER_ROLE_REF = "concept:a31d3c70-a715-52ca-bacf-324016a4842b";
const RELATION_MATERIAL_ROLE_REF = "concept:89136ce9-2418-59ff-a587-a6801680d109";
const REPO_ID = "01M23MNV1W3ZVM8FNBJETVCNAF";

function parsedExport(): SxrLedgerExportParse & { status: "parsed" } {
  const parse = parseSxrLedgerExport(REAL_LEDGER_EXPORT);
  if (parse.status !== "parsed") {
    throw new Error(`fixture must parse, got ${parse.status}`);
  }
  return parse;
}

function projectedLens() {
  const lens = projectSxrEvidenceLens(parsedExport(), REPO_ID);
  if (lens.status !== "projected") {
    throw new Error(`lens must project, got ${lens.status}`);
  }
  return lens;
}

// ---------------------------------------------------------------------------
// Versioned command output parsing (plan step: "separately parse versioned
// SXR outputs and raw ledger export arrays")
// ---------------------------------------------------------------------------

describe("T14B versioned sxr output parsing (REQ-SXR-001)", () => {
  it("parses the REAL enroll output: pinned version, committed durability, typed ledger position, preserved unknowns/omissions", () => {
    const parse = parseSxrCommandOutput(REAL_ENROLL_OUTPUT);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.output_version).toBe("1");
    expect(parse.durability_present).toBe(true);
    const durability = parse.durability;
    if (durability === undefined) {
      throw new Error("enroll output carries a durability channel");
    }
    expect(durability).toBe("committed");
    expect(isPendingDurability(durability)).toBe(false);
    expect(parse.ledger_position).toBeDefined();
    expect(parse.ledger_position?.ledger_id).toBe(REPO_ID);
    expect(parse.ledger_position?.ordinal.value).toBe(1);
    expect(parse.unknowns).toEqual([]);
    expect(parse.omissions).toEqual([]);
  });

  it("round-trips the raw payload losslessly", () => {
    const parse = parseSxrCommandOutput(REAL_ENROLL_OUTPUT);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(JSON.stringify(parse.payload_raw)).toBe(JSON.stringify(JSON.parse(REAL_ENROLL_OUTPUT)));
  });

  it("parses the REAL query-evidence outputs: presence states travel (observed vs not_observed)", () => {
    for (const [capture, presence] of [
      [REAL_QUERY_NOT_OBSERVED, "not_observed"],
      [REAL_QUERY_OBSERVED, "observed"],
    ] as const) {
      const parse = parseSxrCommandOutput(capture);
      expect(parse.status).toBe("parsed");
      if (parse.status !== "parsed") continue;
      const query = parse.payload_raw["query"];
      expect(query).toBeDefined();
      if (typeof query !== "object" || query === null) continue;
      expect((query as Record<string, unknown>)["presence"]).toBe(presence);
    }
    // The observed capture carries the exact evidence record the lens later
    // offers: same id, direction, and question binding.
    const observed = JSON.parse(REAL_QUERY_OBSERVED) as {
      query: { evidence: Array<Record<string, unknown>> };
    };
    expect(observed.query.evidence[0]?.["evidence_id"]).toBe("01M23N051AWR4XA7Q8DGFBSXCS");
    expect(observed.query.evidence[0]?.["direction"]).toBe("supports");
  });

  it("types the REAL pending durability (spooled) — pending is visible and distinct", () => {
    const parse = parseSxrCommandOutput(REAL_SPOOLED_OUTPUT);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    const durability = parse.durability;
    if (durability === undefined) {
      throw new Error("spool output carries a durability channel");
    }
    expect(durability).toBe("spooled");
    expect(isPendingDurability(durability)).toBe(true);
  });

  it("never defaults a missing durability channel to committed (the REAL ledger verify output carries none)", () => {
    const parse = parseSxrCommandOutput(REAL_LEDGER_VERIFY);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.durability_present).toBe(false);
    expect(parse.durability).toBeUndefined();
  });

  it("rejects corrupt output as typed invalid", () => {
    const truncated = parseSxrCommandOutput(REAL_ENROLL_OUTPUT.slice(0, 40));
    expect(truncated.status).toBe("invalid");
    if (truncated.status !== "invalid") return;
    expect(truncated.reason).toBe("malformed_json");
    const array = parseSxrCommandOutput("[]");
    expect(array.status).toBe("invalid");
    if (array.status !== "invalid") return;
    expect(array.reason).toBe("not_an_object");
  });

  it("rejects a future output version as typed incompatible with the declared version surfaced", () => {
    const future = JSON.stringify({ ...JSON.parse(REAL_ENROLL_OUTPUT), sxr_output_version: "2" });
    const parse = parseSxrCommandOutput(future);
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("output_version_mismatch");
    expect(parse.declared_output_version).toBe("2");
  });

  it("types domainforge's CEP envelope as incompatible — a known OTHER contract, never unwrapped", () => {
    const parse = parseSxrCommandOutput(REAL_BOTH_MODE);
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("cep_envelope_not_sxr_output");
  });

  it("types a name-shaped but unpinned object as unknown — no schema guessing", () => {
    const parse = parseSxrCommandOutput(JSON.stringify({ state: "enrolled", repo_id: "x" }));
    expect(parse.status).toBe("unknown");
  });

  it("enforces the mandatory unknowns/omissions channels (§17.2)", () => {
    const stripped = JSON.parse(REAL_ENROLL_OUTPUT) as Record<string, unknown>;
    delete stripped["unknowns"];
    const parse = parseSxrCommandOutput(JSON.stringify(stripped));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("schema_violation");
    expect(parse.detail).toContain("unknowns");
  });
});

// ---------------------------------------------------------------------------
// Raw ledger export parsing (the SEPARATE array contract)
// ---------------------------------------------------------------------------

describe("T14B raw ledger export parsing (REQ-SXR-001: ordinal/hash binding)", () => {
  it("parses the REAL 13-row export with dense ordinals and typed entry hashes", () => {
    const parse = parsedExport();
    expect(parse.rows.length).toBe(13);
    expect(parse.ordinal_density_ok).toBe(true);
    expect(parse.ledger_ids).toEqual([REPO_ID]);
    expect(parse.rows[0]?.ordinal.value).toBe(1);
    expect(parse.rows[12]?.ordinal.value).toBe(13);
    for (const row of parse.rows) {
      expect(row.entry_hash.value).toMatch(/^sha256:[0-9a-f]{64}$/);
      expect(row.kind_known).toBe(true);
    }
    expect(parse.rows.map((row) => row.record_kind)).toEqual([
      "repo.enrolled",
      "question.declared",
      "threshold.declared",
      "semantic.snapshot",
      "run.started",
      "plan.created",
      "observation.recorded",
      "evidence.created",
      "settlement.registered",
      "run.started",
      "plan.created",
      "observation.recorded",
      "evidence.created",
    ]);
  });

  it("records the fields the export omits — chain linkage is producer-side, never pretended", () => {
    const parse = parsedExport();
    const row = parse.rows[7];
    expect(row).toBeDefined();
    if (row === undefined) return;
    expect(row.export_omissions).toContain("payload_hash");
    expect(row.export_omissions).toContain("previous_entry_hash");
  });

  it("preserves unknown record kinds marked kind_known=false — never dropped, never interpreted", () => {
    const rows = JSON.parse(REAL_LEDGER_EXPORT) as unknown[];
    const futureKindRow = {
      ...(rows[0] as Record<string, unknown>),
      append_ordinal: 14,
      record_kind: "settlement.forwarded",
    };
    const parse = parseSxrLedgerExport(JSON.stringify([...rows, futureKindRow]));
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    const extra = parse.rows[13];
    expect(extra).toBeDefined();
    if (extra === undefined) return;
    expect(extra.record_kind).toBe("settlement.forwarded");
    expect(extra.kind_known).toBe(false);
  });

  it("rejects a corrupt row (bad hash format / unknown field) as typed invalid", () => {
    const rows = JSON.parse(REAL_LEDGER_EXPORT) as unknown[];
    const badHash = JSON.parse(JSON.stringify(rows)) as Record<string, unknown>[];
    (badHash[3] as Record<string, unknown>)["entry_hash"] = "md5:deadbeef";
    const parse = parseSxrLedgerExport(JSON.stringify(badHash));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("schema_violation");
    expect(parse.detail).toContain("sha256");

    const extraField = JSON.parse(JSON.stringify(rows)) as Record<string, unknown>[];
    (extraField[0] as Record<string, unknown>)["surprise"] = true;
    const parse2 = parseSxrLedgerExport(JSON.stringify(extraField));
    expect(parse2.status).toBe("invalid");
  });

  it("flags non-dense ordinals (the density part of the chain the export CAN check)", () => {
    const rows = JSON.parse(REAL_LEDGER_EXPORT) as Record<string, unknown>[];
    rows[5] = { ...rows[5], append_ordinal: rows[4]?.["append_ordinal"] };
    const parse = parseSxrLedgerExport(JSON.stringify(rows));
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.ordinal_density_ok).toBe(false);
  });

  it("refuses the versioned envelope in the array parser and vice versa — the parses are separate contracts", () => {
    const asLedger = parseSxrLedgerExport(REAL_ENROLL_OUTPUT);
    expect(asLedger.status).toBe("incompatible");
    if (asLedger.status !== "incompatible") return;
    expect(asLedger.reason).toBe("cep_envelope_not_sxr_output");

    const asCommand = parseSxrCommandOutput(REAL_LEDGER_EXPORT);
    expect(asCommand.status).toBe("invalid");
    if (asCommand.status !== "invalid") return;
    expect(asCommand.reason).toBe("not_an_object");
  });
});

// ---------------------------------------------------------------------------
// DEC-005 requalification: the SXR Slice 0B both-mode carrier (REAL shapes)
// ---------------------------------------------------------------------------

describe("T14B Slice 0B both-mode carrier requalification (REQ-DOM-004, DEC-005)", () => {
  it("parses the REAL object carrier: top-level representation is an OBJECT, D rides at representations[0].inline_content", () => {
    const parsed = JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>;
    expect(typeof parsed["representation"]).toBe("object");
    const parse = parseSxrSlice0bEnvelopeOutput(REAL_BOTH_MODE);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.carriage.mode).toBe("inline_content_string");
    expect(parse.model.model_valid).toBe(true);
    expect(parse.diagnostics).toEqual([]);
    // The parsed D is exactly the real inline string's document.
    const direct = parseSemanticEnvelopeDocument(REAL_BOTH_MODE_INLINE_D);
    expect(direct.status).toBe("parsed");
    if (direct.status !== "parsed") return;
    expect(parse.model.document.self_hash).toBe(direct.model.document.self_hash);
    expect(parse.model.fingerprint.value).toBe(direct.model.fingerprint.value);
  });

  it("T14's pinned parser stays FAIL-CLOSED on the real object carrier — the drift is real and this adapter is the qualified SXR-side path", () => {
    const t14 = parseProducerEnvelopeOutput(REAL_BOTH_MODE);
    expect(t14.status).toBe("invalid");
    if (t14.status !== "invalid") return;
    expect(t14.reason).toBe("representation_not_inline_string");
  });

  it("catches a drifted envelope-side content_hash even when D's internal digests verify", () => {
    const parsed = JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>;
    const envelope = parsed["cep_envelope"] as Record<string, unknown>;
    const rep0 = (envelope["representations"] as Record<string, unknown>[])[0];
    if (rep0 === undefined) {
      throw new Error("fixture must carry representations[0]");
    }
    // Swap ONLY the envelope's declared content_hash: the canonical D is
    // untouched, so D's internal self_hash still verifies — the drift is
    // catchable ONLY by the carrier binding cross-check.
    rep0["content_hash"] = "sha256:" + "a".repeat(64);
    const parse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(parsed));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("self_hash_mismatch");
  });

  it("accepts a synthesized string carrier (release-build shape, disclosed) over the same REAL D", () => {
    const inner = JSON.parse(REAL_BOTH_MODE_INLINE_D) as Record<string, unknown>;
    const synthesized = JSON.stringify({
      representation: REAL_BOTH_MODE_INLINE_D,
      cep_envelope: (JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>)["cep_envelope"],
    });
    expect(typeof synthesized).toBe("string");
    expect(Object.keys(inner).length).toBeGreaterThan(0);
    const parse = parseSxrSlice0bEnvelopeOutput(synthesized);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.model_valid).toBe(true);
  });

  it("object-carrier bytes are cross-checked: a tampered top-level object is typed invalid, never re-serialized into conformity", () => {
    const parsed = JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>;
    const rep = parsed["representation"] as Record<string, unknown>;
    rep["envelope"] = {
      ...(rep["envelope"] as Record<string, unknown>),
      canonicalization_version: "domainforge-canonicalization/v2",
    };
    const parse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(parsed));
    expect(parse.status).toBe("invalid");
    if (parse.status !== "invalid") return;
    expect(parse.reason).toBe("self_hash_mismatch");
  });

  it("preserves diagnostics and refuses conformance for the REAL invalid-with-D capture (exit-1 envelope)", () => {
    const parse = parseSxrSlice0bEnvelopeOutput(REAL_RELATION_INVALID_BOTH);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.model_valid).toBe(false);
    expect(parse.diagnostics.length).toBe(1);
    expect(parse.diagnostics[0]).toContain("Undefined resource: Goods");
    // The parsed model EXISTS but must never project as conformant.
    const enrichment = projectSemanticEnrichment(parse);
    expect(enrichment.status).toBe("absent");
    if (enrichment.status === "absent") {
      expect(enrichment.reason).toBe("model_invalid");
    }
  });

  it("projects the REAL Relation declaration: semantic_relation:supplies edge between the declared roles, endpoints in-model", () => {
    const parse = parseSxrSlice0bEnvelopeOutput(REAL_RELATION_BOTH);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.model.model_valid).toBe(true);
    const relationDecls = parse.model.document.envelope.semantic_declarations.filter(
      (declaration) => declaration.kind === "relation",
    );
    expect(relationDecls.length).toBe(1);
    const relation = relationDecls[0];
    expect(relation).toBeDefined();
    if (relation === undefined) return;
    expect(relation.data["predicate"]).toBe("supplies");
    const enrichment = projectSemanticEnrichment(parse);
    expect(enrichment.status).toBe("bound");
    if (enrichment.status !== "bound") return;
    const relationEdges = enrichment.relations.filter(
      (edge) => edge.kind === "semantic_relation:supplies",
    );
    expect(relationEdges.length).toBe(1);
    const edge = relationEdges[0];
    expect(edge).toBeDefined();
    if (edge === undefined) return;
    expect(edge.source_ref.source_id).toBe(RELATION_SUPPLIER_ROLE_REF);
    expect(edge.target_ref.source_id).toBe(RELATION_MATERIAL_ROLE_REF);
    expect(edge.valid).toBe(true);
    // The real flow declaration also projects.
    expect(enrichment.relations.some((candidate) => candidate.kind === "semantic_flow")).toBe(true);
  });

  it("types the bare CEP failure envelope incompatible — no D, nothing conformant fabricated", () => {
    const failure = JSON.stringify({
      cep_version: "0.1.0",
      envelope_kind: "semantic_snapshot",
      conformance_status: "non_conformant",
      omissions: [],
    });
    const parse = parseSxrSlice0bEnvelopeOutput(failure);
    expect(parse.status).toBe("incompatible");
    if (parse.status !== "incompatible") return;
    expect(parse.reason).toBe("cep_envelope_not_sxr_output");
  });
});

// ---------------------------------------------------------------------------
// CAS content_ref carriage (deterministic negatives over REAL digest-verified
// bytes; the >64 KiB REAL capture is exercised by the live test below)
// ---------------------------------------------------------------------------

describe("T14B CAS content_ref carriage (REQ-SYNC-003, sxr-df §10.7a discipline)", () => {
  interface CasWrapper {
    readonly raw: string;
    readonly content_hash: string;
    readonly d_bytes: Uint8Array;
  }

  /** Disclosed synthetic CAS wrapper around the REAL valid-basic D bytes. */
  function casWrapper(): CasWrapper {
    const parsed = JSON.parse(REAL_BOTH_MODE) as Record<string, unknown>;
    const envelope = parsed["cep_envelope"] as Record<string, unknown>;
    const rep0 = (envelope["representations"] as Record<string, unknown>[])[0];
    if (rep0 === undefined) {
      throw new Error("fixture must carry representations[0]");
    }
    const contentHash = rep0["content_hash"] as string;
    const representations = (envelope["representations"] as Record<string, unknown>[]).map(
      (rep) => {
        const clone = { ...rep };
        delete clone["inline_content"];
        clone["content_ref"] = `cas://${contentHash}`;
        return clone;
      },
    );
    const wrapper = {
      representation: parsed["representation"],
      cep_envelope: { ...envelope, representations },
    };
    return {
      raw: JSON.stringify(wrapper),
      content_hash: contentHash,
      d_bytes: new TextEncoder().encode(JSON.stringify(parsed["representation"])),
    };
  }

  it("materializes from the producer-carried representation, digest-verified", () => {
    const wrapper = casWrapper();
    const parse = parseSxrSlice0bEnvelopeOutput(wrapper.raw);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    expect(parse.carriage.mode).toBe("cas_content_ref");
    if (parse.carriage.mode !== "cas_content_ref") return;
    expect(parse.carriage.source).toBe("producer");
    expect(parse.model.model_valid).toBe(true);
  });

  it("materializes from a CAS store when no producer D rides along, re-verifying after read", () => {
    const wrapper = casWrapper();
    const parsed = JSON.parse(wrapper.raw) as Record<string, unknown>;
    delete parsed["representation"];
    const hex = wrapper.content_hash.slice("sha256:".length);
    const cas = { read: (digest: string) => (digest === hex ? wrapper.d_bytes : undefined) };
    const parse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(parsed), cas);
    expect(parse.status).toBe("parsed");
    if (parse.status !== "parsed") return;
    if (parse.carriage.mode !== "cas_content_ref") {
      throw new Error(`expected cas carriage, got ${parse.carriage.mode}`);
    }
    expect(parse.carriage.source).toBe("cas_store");
  });

  it("refuses expired content and missing CAS objects as typed failures — never a silent substitute", () => {
    const wrapper = casWrapper();
    const expired = JSON.parse(wrapper.raw) as Record<string, unknown>;
    const expiredEnvelope = expired["cep_envelope"] as Record<string, unknown>;
    const expiredReps = expiredEnvelope["representations"] as Record<string, unknown>[];
    const expiredRep0 = expiredReps[0];
    if (expiredRep0 === undefined) {
      throw new Error("fixture must carry representations[0]");
    }
    expiredRep0["content_availability"] = "expired";
    const expiredParse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(expired));
    expect(expiredParse.status).toBe("invalid");
    if (expiredParse.status !== "invalid") return;
    expect(expiredParse.reason).toBe("cas_materialization_failed");
    expect(expiredParse.cas_failure).toBe("content_expired");

    const noD = JSON.parse(wrapper.raw) as Record<string, unknown>;
    delete noD["representation"];
    const missingParse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(noD), {
      read: () => undefined,
    });
    expect(missingParse.status).toBe("invalid");
    if (missingParse.status !== "invalid") return;
    expect(missingParse.cas_failure).toBe("cas_object_missing");

    const noStoreParse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(noD));
    expect(noStoreParse.status).toBe("invalid");
    if (noStoreParse.status !== "invalid") return;
    expect(noStoreParse.cas_failure).toBe("no_carried_representation_and_no_cas_store");
  });

  it("refuses hash-mismatched CAS bytes and mismatched content_ref addresses", () => {
    const wrapper = casWrapper();
    const noD = JSON.parse(wrapper.raw) as Record<string, unknown>;
    delete noD["representation"];
    const hex = wrapper.content_hash.slice("sha256:".length);
    const corrupt = new TextEncoder().encode('{"self_hash":"sha256:' + "0".repeat(64) + '"}');
    const corruptParse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(noD), {
      read: (digest) => (digest === hex ? corrupt : undefined),
    });
    expect(corruptParse.status).toBe("invalid");
    if (corruptParse.status !== "invalid") return;
    expect(corruptParse.cas_failure).toBe("cas_object_hash_mismatch");

    const mismatched = JSON.parse(wrapper.raw) as Record<string, unknown>;
    const mismatchEnvelope = mismatched["cep_envelope"] as Record<string, unknown>;
    const mismatchReps = mismatchEnvelope["representations"] as Record<string, unknown>[];
    const mismatchRep0 = mismatchReps[0];
    if (mismatchRep0 === undefined) {
      throw new Error("fixture must carry representations[0]");
    }
    mismatchRep0["content_ref"] = "cas://sha256:" + "f".repeat(64);
    const mismatchParse = parseSxrSlice0bEnvelopeOutput(JSON.stringify(mismatched));
    expect(mismatchParse.status).toBe("invalid");
    if (mismatchParse.status !== "invalid") return;
    expect(mismatchParse.cas_failure).toBe("content_ref_hash_mismatch");
  });
});

// ---------------------------------------------------------------------------
// Bounded refresh contract (REQ-SXR-003)
// ---------------------------------------------------------------------------

function mutableClock(start = 1_000_000): SxrClock & { advance: (ms: number) => void } {
  let current = start;
  return {
    now: () => current,
    advance: (ms: number) => {
      current += ms;
    },
  };
}

interface FakeRunnerOptions {
  readonly behavior: "hang" | "flood" | "exit";
  readonly exit_code?: number;
  readonly stdout?: string;
  readonly stderr?: string;
}

type MutableClock = ReturnType<typeof mutableClock>;

function fakeRunner(
  options: FakeRunnerOptions,
  kills: string[],
  mutable?: MutableClock,
): SxrProcessRunner {
  return {
    kind: `fake-${options.behavior}`,
    async run(bound: SxrBoundRefresh, clock: SxrClock): Promise<SxrProcessResult> {
      const encoder = new TextEncoder();
      if (options.behavior === "hang") {
        // A hung producer: partial output arrives, the process never exits.
        // The fake consumes the deadline on the injected mutable clock — the
        // same contract bunSxrProcessRunner enforces against the real clock.
        const advance = mutable?.advance;
        const deadline_at = clock.now() + bound.deadline_ms;
        while (clock.now() < deadline_at) {
          if (advance === undefined) {
            break;
          }
          advance(Math.ceil(bound.deadline_ms / 4));
        }
        kills.push("SIGTERM");
        return {
          class: "deadline_exceeded",
          stdout_bytes: encoder.encode('{"partial_tr'),
          stderr_text: "",
        };
      }
      if (options.behavior === "flood") {
        kills.push("SIGKILL");
        return {
          class: "output_limit_exceeded",
          stdout_bytes: new Uint8Array(bound.max_output_bytes + 1),
          stderr_text: "",
        };
      }
      kills.push("reaped");
      return {
        class: "exited",
        exit_code: options.exit_code ?? 0,
        signal: null,
        stdout_bytes: encoder.encode(options.stdout ?? ""),
        stderr_text: options.stderr ?? "",
      };
    },
  };
}

describe("T14B bounded refresh boundary (REQ-SXR-003)", () => {
  it("binds the closed argv vocabulary as an argv ARRAY with anchored env and typed side-effect classification", () => {
    const bound = validateSxrRefreshRequest({
      binary_path: "/opt/sxr/bin/sxr",
      repo_id: REPO_ID,
      command: { kind: "ledger_export" },
    });
    expect(bound.program).toBe("/opt/sxr/bin/sxr");
    expect(bound.args).toEqual(["ledger", "export", "--repo", REPO_ID]);
    expect(bound.side_effects.appends_canonical_ledger).toBe(false);
    expect(bound.side_effects.launches_autonomous_run).toBe(false);
    expect(bound.side_effects.initializes_local_storage).toBe(true);

    const query = validateSxrRefreshRequest({
      binary_path: "/opt/sxr/bin/sxr",
      repo_id: REPO_ID,
      command: { kind: "query_evidence", entity: sxrEntityRef(USER_ENTITY_REF) },
    });
    expect(query.args).toEqual([
      "query",
      "evidence",
      "--repo",
      REPO_ID,
      "--entity",
      USER_ENTITY_REF,
    ]);
    expect(query.side_effects.rebuilds_projections).toBe(true);
    // No write command exists in the vocabulary: no binding can ever emit
    // ledger-appending or run-launching argv.
    for (const command of [
      { kind: "ledger_export" as const },
      { kind: "ledger_verify" as const },
    ]) {
      const anyBound = validateSxrRefreshRequest({
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command,
      });
      const joined = anyBound.args.join(" ");
      expect(joined).not.toContain("settlement");
      expect(joined).not.toContain("run ");
      expect(joined).not.toContain("ingest");
      expect(joined).not.toContain("claim");
    }
  });

  it("MINOR-2 regression: absent XDG variables are omitted entirely — an empty XDG_DATA_HOME means unset per spec", () => {
    const stateOnly = validateSxrRefreshRequest({
      binary_path: "/opt/sxr/bin/sxr",
      repo_id: REPO_ID,
      command: { kind: "ledger_export" },
      xdg_state_home: "/tmp/state-only",
    });
    expect(stateOnly.env["XDG_STATE_HOME"]).toBe("/tmp/state-only");
    expect(stateOnly.env["XDG_DATA_HOME"]).toBeUndefined();
    const dataOnly = validateSxrRefreshRequest({
      binary_path: "/opt/sxr/bin/sxr",
      repo_id: REPO_ID,
      command: { kind: "ledger_export" },
      xdg_data_home: "/tmp/data-only",
    });
    expect(dataOnly.env["XDG_DATA_HOME"]).toBe("/tmp/data-only");
    expect(dataOnly.env["XDG_STATE_HOME"]).toBeUndefined();
    const neither = validateSxrRefreshRequest({
      binary_path: "/opt/sxr/bin/sxr",
      repo_id: REPO_ID,
      command: { kind: "ledger_export" },
    });
    expect(neither.env["XDG_DATA_HOME"]).toBeUndefined();
    expect(neither.env["XDG_STATE_HOME"]).toBeUndefined();
  });

  it("MINOR-3 regression: a runtime-forged command kind is a typed refusal, never a silent empty-argv spawn", () => {
    const forged = { kind: "settlement_register" } as unknown as Parameters<
      typeof validateSxrRefreshRequest
    >[0]["command"];
    expect(() =>
      validateSxrRefreshRequest({
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: forged,
      }),
    ).toThrow(SxrRefreshBoundaryError);
    try {
      validateSxrRefreshRequest({
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: forged,
      });
    } catch (error) {
      expect((error as SxrRefreshBoundaryError).code).toBe("command_not_in_contract");
    }
  });

  it("refuses malformed boundary inputs with typed codes", () => {
    const refusals: Array<() => unknown> = [
      () =>
        validateSxrRefreshRequest({
          binary_path: "sxr",
          repo_id: REPO_ID,
          command: { kind: "ledger_export" },
        }),
      () =>
        validateSxrRefreshRequest({
          binary_path: "/opt/sx r;rm/bin/sxr",
          repo_id: REPO_ID,
          command: { kind: "ledger_export" },
        }),
      () =>
        validateSxrRefreshRequest({
          binary_path: "/opt/sxr/bin/sxr",
          repo_id: "not a repo id",
          command: { kind: "ledger_export" },
        }),
      () =>
        validateSxrRefreshRequest({
          binary_path: "/opt/sxr/bin/sxr",
          repo_id: REPO_ID,
          command: { kind: "ledger_export" },
          deadline_ms: 0,
        }),
      () =>
        validateSxrRefreshRequest({
          binary_path: "/opt/sxr/bin/sxr",
          repo_id: REPO_ID,
          command: { kind: "ledger_export" },
          max_output_bytes: -1,
        }),
    ];
    for (const refusal of refusals) {
      expect(() => refusal()).toThrow(SxrRefreshBoundaryError);
    }
  });

  it("kills a hung producer at the deadline; partial output is DISCARDED, never parsed", async () => {
    const kills: string[] = [];
    const clock = mutableClock();
    const outcome = await executeSxrRefresh(
      {
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: { kind: "ledger_export" },
        deadline_ms: 1000,
      },
      fakeRunner({ behavior: "hang" }, kills, clock),
      clock,
    );
    expect(outcome.class).toBe("deadline_exceeded");
    if (outcome.class !== "deadline_exceeded") return;
    expect(outcome.partial_output_bytes).toBe('{"partial_tr'.length);
    expect(kills).toEqual(["SIGTERM"]);
    expect(clock.now()).toBeGreaterThanOrEqual(1_000_000 + 1000);
    // The keyboard/render side is untouched by construction: refresh is a
    // promise the render loop never awaits.
  });

  it("kills a flooded producer at the output bound; partial output is DISCARDED, never parsed", async () => {
    const kills: string[] = [];
    const outcome = await executeSxrRefresh(
      {
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: { kind: "ledger_export" },
        max_output_bytes: 1024,
      },
      fakeRunner({ behavior: "flood" }, kills),
    );
    expect(outcome.class).toBe("output_limit_exceeded");
    if (outcome.class !== "output_limit_exceeded") return;
    expect(outcome.partial_output_bytes).toBe(1025);
    expect(kills).toEqual(["SIGKILL"]);
  });

  it("types a non-zero producer exit as producer_refused with the REAL F-19b pending-refusal stderr preserved", async () => {
    const kills: string[] = [];
    const outcome = await executeSxrRefresh(
      {
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: { kind: "query_evidence", entity: sxrEntityRef(USER_ENTITY_REF) },
      },
      fakeRunner(
        {
          behavior: "exit",
          exit_code: 3,
          stderr:
            "sxr: record 01M23MRR5XTM78DR4KJ72QKPZ8 is spooled (spooled (not yet validated)); not committed — run `sxr await` or `sxr spool drain`/`sxr queue recover` first (AC-DAEMON-003/F-19b)",
        },
        kills,
      ),
    );
    expect(outcome.class).toBe("producer_refused");
    if (outcome.class !== "producer_refused") return;
    expect(outcome.exit_code).toBe(3);
    expect(outcome.stderr_text).toContain("not committed");
    expect(outcome.stderr_text).toContain("AC-DAEMON-003/F-19b");
  });

  it("refreshed outcomes parse under their command's contract", async () => {
    const kills: string[] = [];
    const outcome = await executeSxrRefresh(
      {
        binary_path: "/opt/sxr/bin/sxr",
        repo_id: REPO_ID,
        command: { kind: "ledger_export" },
      },
      fakeRunner({ behavior: "exit", stdout: REAL_LEDGER_EXPORT }, kills),
    );
    expect(outcome.class).toBe("refreshed");
    if (outcome.class !== "refreshed") return;
    expect(outcome.command).toBe("ledger_export");
    if (outcome.parse.status !== "parsed") {
      throw new Error(`refreshed export did not parse: ${outcome.parse.status}`);
    }
    if (!("rows" in outcome.parse)) {
      throw new Error("ledger_export refresh must parse the export array");
    }
    expect(outcome.parse.rows.length).toBe(13);
  });
});

// ---------------------------------------------------------------------------
// Evidence lens: identity bindings, path, admission (REQ-SXR-001/002)
// ---------------------------------------------------------------------------

describe("T14B evidence lens projection (REQ-SXR-001)", () => {
  it("projects the REAL export into typed records carrying ledger ordinal + entry hash", () => {
    const lens = projectedLens();
    expect(lens.questions.length).toBe(1);
    expect(lens.questions[0]?.question_id).toBe("q-logistics-flows");
    expect(lens.questions[0]?.target_entity_ref).toBe("concept:order_flow");
    expect(lens.thresholds.length).toBe(1);
    expect(lens.thresholds[0]?.threshold_id).toBe("th-all-supports");
    expect(lens.thresholds[0]?.question_ref).toBe("question:q-logistics-flows");
    expect(lens.snapshots.length).toBe(1);
    expect(lens.snapshots[0]?.preserved_entities).toContain(USER_ENTITY_REF);
    expect(lens.runs.length).toBe(2);
    expect(lens.evidence.length).toBe(2);
    expect(lens.settlements.length).toBe(1);
    expect(lens.settlements[0]?.sxr_settlement_status).toBe("settled");
    const evidence = lens.evidence.find((record) => record.target_entity_ref === USER_ENTITY_REF);
    expect(evidence).toBeDefined();
    if (evidence === undefined) return;
    expect(evidence.row.ordinal.value).toBe(13);
    expect(evidence.row.entry_hash.value).toMatch(/^sha256:[0-9a-f]{64}$/);
    // Rows the lens does not interpret are counted, not dropped.
    const uninterpretedKinds = lens.uninterpreted.map((entry) => entry.record_kind);
    expect(uninterpretedKinds).toContain("repo.enrolled");
    expect(uninterpretedKinds).toContain("plan.created");
  });

  it("types absent lenses: no export and invalid export are explicit, never fabricated", () => {
    const noExport = projectSxrEvidenceLens(undefined, REPO_ID);
    expect(noExport.status).toBe("absent");
    if (noExport.status === "absent") {
      expect(noExport.reason).toBe("no_ledger_export");
    }
    const badExport = projectSxrEvidenceLens(
      { status: "invalid", reason: "schema_violation", detail: "boom" },
      REPO_ID,
    );
    expect(badExport.status).toBe("absent");
    if (badExport.status === "absent") {
      expect(badExport.reason).toBe("export_invalid");
    }
  });

  it("admits EXACT entity-ref bindings and refuses label joins, unknown refs, and non-WorkItem targets", () => {
    const lens = projectedLens();
    const unit = FocusRef.for("gauntlet", "WorkItem", "AK-1414");
    const admitted = admitSxrUnitBindings(lens, [
      { unit, entity_ref: USER_ENTITY_REF, declared_by: "operator-olive" },
    ]);
    expect(admitted.length).toBe(1);
    expect(admitted[0]?.entity_ref.value).toBe(USER_ENTITY_REF);

    expect(() =>
      admitSxrUnitBindings(lens, [{ unit, entity_ref: "User", declared_by: "operator-olive" }]),
    ).toThrow(SxrBindingError);
    try {
      admitSxrUnitBindings(lens, [{ unit, entity_ref: "User", declared_by: "operator-olive" }]);
    } catch (error) {
      expect((error as Error).message).toContain("no name-based joins");
    }
    expect(() =>
      admitSxrUnitBindings(lens, [
        { unit, entity_ref: "concept:not-in-the-ledger", declared_by: "operator-olive" },
      ]),
    ).toThrow(SxrBindingError);
    expect(() =>
      admitSxrUnitBindings(lens, [
        {
          unit: FocusRef.for("gauntlet", "Evidence", "some-evidence"),
          entity_ref: USER_ENTITY_REF,
          declared_by: "operator-olive",
        },
      ]),
    ).toThrow(SxrBindingError);
  });
});

describe("T14B canonical verification path (REQ-SXR-001: recorded question/threshold/evidence)", () => {
  it("builds the full declaration → unit → question → threshold → evidence → SXR settlement path from REAL records", () => {
    const lens = projectedLens();
    const bindings = admitSxrUnitBindings(lens, [
      {
        unit: FocusRef.for("gauntlet", "WorkItem", "AK-1414"),
        entity_ref: USER_ENTITY_REF,
        declared_by: "operator-olive",
      },
    ]);
    const path = sxrVerificationPath(lens, USER_ENTITY_REF, bindings);
    expect(path.chain_complete).toBe(true);
    expect(path.missing).toEqual([]);
    const stages = path.steps.map((step) => step.stage);
    expect(stages[0]).toBe("declaration");
    expect(stages).toContain("unit_binding");
    expect(stages).toContain("question");
    expect(stages).toContain("threshold");
    expect(stages).toContain("evidence");
    expect(stages).toContain("sxr_settlement");
    // Ledger bindings travel on every recorded step.
    const questionStep = path.steps.find((step) => step.stage === "question");
    expect(questionStep?.ledger?.ordinal).toBe(2);
    const thresholdStep = path.steps.find((step) => step.stage === "threshold");
    expect(thresholdStep?.ledger?.ordinal).toBe(3);
    const settlementStep = path.steps.find((step) => step.stage === "sxr_settlement");
    expect(settlementStep?.ledger?.ordinal).toBe(9);
    expect(settlementStep?.sxr_settlement_status).toBe("settled");
    // The declaration step binds to the snapshot record.
    const declarationStep = path.steps[0];
    expect(declarationStep?.ledger?.ordinal).toBe(4);
  });

  it("records explicit missing links instead of inventing them (a declaration with no question)", () => {
    const lens = projectedLens();
    const path = sxrVerificationPath(lens, ROLE_ENTITY_REF);
    expect(path.chain_complete).toBe(false);
    expect(path.missing.some((entry) => entry.includes("question"))).toBe(true);
    expect(path.steps.length).toBe(1);
  });
});

describe("T14B admission boundary (REQ-SXR-002: pending can never pass; SXR is never Gauntlet settlement)", () => {
  it("offers committed evidence with literal-false establishes_* and the canonical admission route", () => {
    const lens = projectedLens();
    const exportParse = parsedExport();
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: exportParse,
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(outcome.class).toBe("offered");
    if (outcome.class !== "offered") return;
    expect(outcome.offer.establishes_evidence).toBe(false);
    expect(outcome.offer.establishes_verification).toBe(false);
    expect(outcome.offer.establishes_settlement).toBe(false);
    expect(outcome.offer.admission_route).toBe("canonical_gauntlet_evidence_apis");
    expect(outcome.offer.admission_contract.locator).toContain("sxr-ledger:");
    expect(outcome.offer.admission_contract.locator).toContain("13");
  });

  it("refuses pending evidence — the F-19b durability state can never be offered as settled", () => {
    const lens = projectedLens();
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: parsedExport(),
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
      pending_durability_observed: true,
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") return;
    expect(outcome.reason).toBe("pending_evidence");
    expect(outcome.message).toContain("pending");
  });

  it("refuses evidence that is not a committed record of the qualified export", () => {
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens: projectedLens(),
      export_parse: parsedExport(),
      evidence_id: "01FAKEEVIDENCEIDNOTREAL000000",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") return;
    expect(outcome.reason).toBe("evidence_not_in_qualified_ledger");
  });

  it("refuses offers across repositories", () => {
    const lens = projectSxrEvidenceLens(parsedExport(), "OTHER-REPO");
    if (lens.status !== "projected") {
      throw new Error("lens must project");
    }
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: parsedExport(),
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") return;
    expect(outcome.reason).toBe("repository_mismatch");
  });

  it("MAJOR-1 regression: refuses an offer whose lens repo disagrees with the export ledger even when the export carries NO run rows", () => {
    // The confirmation's probe: the old runs-based guard disappeared when the
    // export had no run.started rows, and the ledger-vs-lens guard was dead
    // code for non-empty exports — a fabricated lens repo_id rode into the
    // offer's source. The offer must be REFUSED with the swapped-repository
    // typed reason.
    const rowsWithoutRuns = (
      JSON.parse(REAL_LEDGER_EXPORT) as Array<{ record_kind: string }>
    ).filter((row) => row.record_kind !== "run.started");
    const exportParse = parseSxrLedgerExport(JSON.stringify(rowsWithoutRuns));
    if (exportParse.status !== "parsed") {
      throw new Error("the run-less export must parse");
    }
    const lens = projectSxrEvidenceLens(exportParse, "FABRICATED-REPO");
    if (lens.status !== "projected") {
      throw new Error("lens must project");
    }
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: exportParse,
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") return;
    expect(outcome.reason).toBe("repository_mismatch");
  });

  it("MAJOR-1 regression: the legitimate same-repo offer still builds without run rows, carrying the true ledger provenance", () => {
    const rowsWithoutRuns = (
      JSON.parse(REAL_LEDGER_EXPORT) as Array<{ record_kind: string }>
    ).filter((row) => row.record_kind !== "run.started");
    const exportParse = parseSxrLedgerExport(JSON.stringify(rowsWithoutRuns));
    if (exportParse.status !== "parsed") {
      throw new Error("the run-less export must parse");
    }
    const lens = projectSxrEvidenceLens(exportParse, REPO_ID);
    if (lens.status !== "projected") {
      throw new Error("lens must project");
    }
    const outcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: exportParse,
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(outcome.class).toBe("offered");
    if (outcome.class !== "offered") return;
    expect(outcome.offer.source.repo_id).toBe(REPO_ID);
    // The locator still carries the true ledger id and ordinal of the record.
    expect(outcome.offer.admission_contract.locator).toContain(REPO_ID);
    expect(outcome.offer.admission_contract.locator).toContain("13");
  });

  it("admits only under a canonical decision authored elsewhere; SXR settlement state never substitutes", () => {
    const lens = projectedLens();
    const offerOutcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: parsedExport(),
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error("offer must be built");
    }
    const offer = offerOutcome.offer;
    const gate = new SxrEvidenceAdmissionGate();

    // A canonical refusal is a typed canonical refusal.
    const refused = gate.admit(offer, {
      class: "refused",
      evidence_id: offer.admission_contract.evidence_id,
      code: "verification_failed",
      message: "canonical verification did not admit this evidence",
    });
    expect(refused.class).toBe("refused_canonically");
    if (refused.class === "refused_canonically") {
      expect(refused.code).toBe("verification_failed");
    }
    expect(gate.wasAdmitted(offer.offer_id)).toBe(false);

    // A decision naming different evidence is a mismatch — never admitted.
    const mismatch = gate.admit(offer, {
      class: "admitted",
      evidence_id: "sxr:some-other-evidence",
      verified: true,
      source_cursor: "cursor-1",
    });
    expect(mismatch.class).toBe("refused");
    if (mismatch.class === "refused") {
      expect(mismatch.reason).toBe("decision_mismatch");
    }

    // The canonical decision's own verified bit lands verbatim (false stays false).
    const admitted = gate.admit(offer, {
      class: "admitted",
      evidence_id: offer.admission_contract.evidence_id,
      verified: false,
      source_cursor: "cursor-2",
    });
    expect(admitted.class).toBe("admitted");
    if (admitted.class === "admitted") {
      expect(admitted.record.verified).toBe(false);
      expect(admitted.record.source_id).toBe("gauntlet");
    }

    // Admission is one-way.
    const again = gate.admit(offer, {
      class: "admitted",
      evidence_id: offer.admission_contract.evidence_id,
      verified: true,
      source_cursor: "cursor-3",
    });
    expect(again.class).toBe("refused");
    if (again.class === "refused") {
      expect(again.reason).toBe("already_admitted");
    }

    // An SXR settlement record is NOT offerable evidence: the settlement
    // channel is distinct and refuses by construction.
    const settlementOffer = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: parsedExport(),
      evidence_id: "01M23MYN5TFEQVW8SR6QX9MWBV",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    expect(settlementOffer.class).toBe("refused");
    if (settlementOffer.class === "refused") {
      expect(settlementOffer.reason).toBe("evidence_not_in_qualified_ledger");
    }
  });

  it("carries a canonical admission through the authoritative evidence_added frame into the projection store", () => {
    const lens = projectedLens();
    const offerOutcome = offerSxrEvidenceAsGauntletEvidence({
      lens,
      export_parse: parsedExport(),
      evidence_id: "01M23N051AWR4XA7Q8DGFBSXCS",
      unit_id: "AK-1414",
      target_source_id: "gauntlet",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error("offer must be built");
    }
    const decision = {
      class: "admitted" as const,
      evidence_id: offerOutcome.offer.admission_contract.evidence_id,
      verified: true,
      source_cursor: "sxr@9",
    };
    const frame = authoritativeSxrEvidenceFrame(offerOutcome.offer, decision, {
      generation_id: "gen-t14b",
      timestamp_ms: 1_000,
    });
    expect(frame.event_type).toBe("evidence_added");
    expect(frame.source_id).toBe("gauntlet");

    expect(() =>
      authoritativeSxrEvidenceFrame(
        offerOutcome.offer,
        {
          class: "refused",
          evidence_id: offerOutcome.offer.admission_contract.evidence_id,
          code: "x",
          message: "y",
        },
        { generation_id: "gen-t14b", timestamp_ms: 1_000 },
      ),
    ).toThrow(/admission is the only route/);

    // The store's authoritative path is the only writer of evidence rows.
    const store = SqliteProjectionStore.rebuildFromAuthoritativeLog(":memory:", "gen-t14b", [
      frame,
    ]);
    const snapshot = store.getSnapshot();
    expect(snapshot.evidence.length).toBe(1);
    const row = snapshot.evidence[0];
    expect(row?.evidence_id).toBe(offerOutcome.offer.admission_contract.evidence_id);
    expect(row?.verified).toBe(true);
    expect(row?.source_cursor).toBe("sxr@9");
    store.close();
  });
});

// ---------------------------------------------------------------------------
// Lens rendering (existing Inspector contract; pending never reads as settled)
// ---------------------------------------------------------------------------

describe("T14B lens rendering (REQ-SXR-001, REQ-SYNC-003)", () => {
  it("renders the ABSENT state explicitly — source loss is labelled unavailable, never recreated", () => {
    const section = renderSxrEvidenceSection({
      status: "absent",
      reason: "no_ledger_export",
      detail: "no SXR ledger export was provided for this Case",
    });
    const text = section.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain("ABSENT (no_ledger_export)");
    expect(text).toContain("not recreated");
    expect(section.records).toEqual([]);
  });

  it("renders the projected path as inspectable records with ledger bindings; SXR settlement stays source-typed", () => {
    const lens = projectedLens();
    const bindings = admitSxrUnitBindings(lens, [
      {
        unit: FocusRef.for("gauntlet", "WorkItem", "AK-1414"),
        entity_ref: USER_ENTITY_REF,
        declared_by: "operator-olive",
      },
    ]);
    const path = sxrVerificationPath(lens, USER_ENTITY_REF, bindings);
    const section = renderSxrEvidenceSection(projectedLens(), [path]);
    expect(section.title).toBe("REALITY TRACE");
    const settlementRecords = section.records.filter(
      (record) => record.fields["stage"] === "sxr_settlement",
    );
    expect(settlementRecords.length).toBe(1);
    expect(settlementRecords[0]?.fields["sxr_settlement"]).toBe("settled");
    expect(settlementRecords[0]?.fields["detail"]).toContain("distinct from any Gauntlet verdict");
    const evidenceRecords = section.records.filter(
      (record) => record.fields["stage"] === "evidence",
    );
    expect(evidenceRecords.length).toBe(1);
    expect(evidenceRecords[0]?.fields["ledger"]).toContain("ordinal 13");
  });
});

// ---------------------------------------------------------------------------
// Live qualification (prereg A3; R1 honesty): REAL binaries through the REAL
// bounded runner. Visible skips when the pinned binaries are absent — skips
// keep this milestone open, they never fake composition.
// ---------------------------------------------------------------------------

const SXR_BINARY = "/home/sprime01/projects/sxr/target/debug/sxr";
const DF_PRODUCER_BINARY = "/home/sprime01/projects/domainforge-slice0b/target/debug/domainforge";
const CAPTURES_DIR = "../.agents/reports/gauntlet-tui-prime-workbench/evidence/T14B/captures";
const BIG_MODEL_SEA = join(CAPTURES_DIR, "big-model.sea");

async function sha256File(path: string): Promise<string> {
  const bytes = await Bun.file(path).arrayBuffer();
  return new Bun.CryptoHasher("sha256").update(new Uint8Array(bytes)).digest("hex");
}

const sxrBinaryMatches =
  existsSync(SXR_BINARY) &&
  (await sha256File(SXR_BINARY)) === QUALIFIED_SXR_BUILDS[0]?.binary_sha256;
const producerBinaryMatches =
  existsSync(DF_PRODUCER_BINARY) &&
  (await sha256File(DF_PRODUCER_BINARY)) === QUALIFIED_SXR_BUILDS[1]?.binary_sha256;

describe.skipIf(!sxrBinaryMatches)("T14B live sxr binary through the bounded runner", () => {
  const tempRoots: string[] = [];
  afterAll(() => {
    for (const root of tempRoots) {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("runs the REAL pinned binary through executeSxrRefresh: ledger verify round-trips its output contract", async () => {
    const data = mkdtempSync(join(tmpdir(), "t14b-live-data-"));
    const state = mkdtempSync(join(tmpdir(), "t14b-live-state-"));
    tempRoots.push(data, state);
    const outcome = await executeSxrRefresh(
      {
        binary_path: SXR_BINARY,
        repo_id: "01T14BLIVEROUNDRIPREPOTEST00",
        command: { kind: "ledger_verify" },
        xdg_data_home: data,
        xdg_state_home: state,
      },
      bunSxrProcessRunner(),
      systemSxrClock,
    );
    expect(outcome.class).toBe("refreshed");
    if (outcome.class !== "refreshed") return;
    expect(outcome.parse.status).toBe("parsed");
    if (outcome.parse.status !== "parsed" || !("output_version" in outcome.parse)) return;
    expect(outcome.parse.output_version).toBe("1");
    expect(outcome.parse.unknowns).toEqual([]);
  });

  it("runs the REAL raw ledger export on a fresh repo: an empty committed chain, storage initialized", async () => {
    const data = mkdtempSync(join(tmpdir(), "t14b-live-data-"));
    const state = mkdtempSync(join(tmpdir(), "t14b-live-state-"));
    tempRoots.push(data, state);
    const outcome = await executeSxrRefresh(
      {
        binary_path: SXR_BINARY,
        repo_id: "01T14BLIVEEMPTYEXPORTREPOT00",
        command: { kind: "ledger_export" },
        xdg_data_home: data,
        xdg_state_home: state,
      },
      bunSxrProcessRunner(),
      systemSxrClock,
    );
    expect(outcome.class).toBe("refreshed");
    if (outcome.class !== "refreshed") return;
    if (outcome.parse.status !== "parsed" || !("rows" in outcome.parse)) return;
    expect(outcome.parse.rows).toEqual([]);
    expect(outcome.parse.ordinal_density_ok).toBe(true);
  });

  it("surfaces an unenrolled-repo query as a typed producer_refused (the side effects stay classified)", async () => {
    const data = mkdtempSync(join(tmpdir(), "t14b-live-data-"));
    const state = mkdtempSync(join(tmpdir(), "t14b-live-state-"));
    tempRoots.push(data, state);
    const outcome = await executeSxrRefresh(
      {
        binary_path: SXR_BINARY,
        repo_id: "01T14BLIVEUNENROLLEDREPOTEST",
        command: { kind: "query_evidence", entity: sxrEntityRef(USER_ENTITY_REF) },
        xdg_data_home: data,
        xdg_state_home: state,
      },
      bunSxrProcessRunner(),
      systemSxrClock,
    );
    expect(outcome.class).toBe("producer_refused");
    if (outcome.class !== "producer_refused") return;
    expect(outcome.stderr_text).toContain("not enrolled");
  });
});

describe.skipIf(!producerBinaryMatches || !existsSync(BIG_MODEL_SEA))(
  "T14B live producer: REAL >64KiB CAS carriage through the real binary",
  () => {
    it("parses the REAL large-model both-mode output with content_ref carriage and producer-verified D", async () => {
      const sea = await Bun.file(BIG_MODEL_SEA).text();
      const workdir = mkdtempSync(join(tmpdir(), "t14b-cas-"));
      try {
        const entry = join(workdir, "big-model.sea");
        await Bun.write(entry, sea);
        const proc = Bun.spawn(
          [DF_PRODUCER_BINARY, "envelope", "--emit", "both", "big-model.sea"],
          {
            cwd: workdir,
            stdout: "pipe",
            stderr: "pipe",
          },
        );
        const stdout = await new Response(proc.stdout).text();
        const exit = await proc.exited;
        expect(exit).toBe(0);
        expect(stdout.length).toBeGreaterThan(1_000_000);
        const parse = parseSxrSlice0bEnvelopeOutput(stdout);
        expect(parse.status).toBe("parsed");
        if (parse.status !== "parsed") return;
        expect(parse.carriage.mode).toBe("cas_content_ref");
        if (parse.carriage.mode !== "cas_content_ref") return;
        expect(parse.carriage.source).toBe("producer");
        expect(parse.model.model_valid).toBe(true);
        expect(parse.model.document_raw["self_hash"]).toMatch(/^sha256:[0-9a-f]{64}$/);
      } finally {
        rmSync(workdir, { recursive: true, force: true });
      }
    });
  },
);

// ---------------------------------------------------------------------------
// Qualification record honesty (prereg A1/A3)
// ---------------------------------------------------------------------------

describe("T14B qualification record honesty", () => {
  it("records exactly what was live-qualified on this host — and what was not", () => {
    expect(SXR_LIVE_QUALIFICATION.qualified_on_host).toBe(true);
    expect(SXR_LIVE_QUALIFICATION.both_mode_object_carrier_qualified).toBe(true);
    expect(SXR_LIVE_QUALIFICATION.cas_content_ref_carriage_qualified).toBe(true);
    expect(SXR_LIVE_QUALIFICATION.relation_declaration_fixture_real).toBe(true);
    expect(SXR_LIVE_QUALIFICATION.role_binding_declaration_fixture_real).toBe(false);
    expect(SXR_LIVE_QUALIFICATION.lifecycle_records_captured).toBe(13);
  });

  it("pins the qualified binary digests, including the installed binary's Slice 0B incompatibility", () => {
    const sxr = QUALIFIED_SXR_BUILDS.find((build) => build.role === "sxr consumer");
    const producer = QUALIFIED_SXR_BUILDS.find((build) => build.role === "domainforge producer");
    const installed = QUALIFIED_SXR_BUILDS.find((build) => build.role === "domainforge installed");
    expect(sxr?.binary_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(producer?.answers_slice0b_invocation).toBe(true);
    expect(installed?.answers_slice0b_invocation).toBe(false);
    for (const build of QUALIFIED_SXR_BUILDS) {
      expect(build.locally_verified).toBe(true);
    }
  });
});
