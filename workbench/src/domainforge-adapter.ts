/**
 * DomainForge/SEA semantic enrichment adapter (plan T14, REQ-DOM-001..005).
 *
 * Parses the PINNED DomainForge canonical semantic envelope document
 * (`domainforge-semantic-envelope/v1`) and projects it into the existing
 * FocusRef/Relation/horizon seams WITHOUT creating a new identity domain or a
 * new truth store.
 *
 * Producer qualification (verified locally on the builder workstation,
 * 2026-09-09; re-qualification is T14B/T20 work — see PROTOCOL_COMPATIBILITY):
 *
 *   - Three distinct `domainforge 0.16.0` builds exist on this machine:
 *     1. `~/.cargo/bin/domainforge` (sha256 ee3ee077…): NO `envelope`
 *        subcommand at all.
 *     2. `domainforge/target/release/domainforge` (sha256 48a4ed25…):
 *        `domainforge envelope <entry.sea>` → the bare canonical semantic
 *        envelope document D on stdout, byte-identical across runs.
 *     3. `domainforge/target/debug/domainforge` (working-tree build,
 *        sha256 d5996985…): adds `--emit representation|cep|both|
 *        verification-contract`, `--pack`, `--scope`, `--registry`.
 *   - The bare-D output of (2) and of (3) `--emit representation` is
 *     byte-identical; this adapter pins that document shape.
 *   - The self_hash/semantic_closure_hash digest algorithm (sha256 over
 *     recursively key-sorted compact canonical JSON; self_hash excludes
 *     itself) was verified against a real capture before pinning.
 *   - DEC-005 holds and is now precise: the SXR Slice 0B consumer invokes
 *     `envelope --emit both <entry>` (sxr-df src/adapter.rs); the installed
 *     binary rejects the subcommand and the release build rejects the flag.
 *     Only the uncommitted working-tree build answers that invocation, so the
 *     main-CLI/SXR-0B pair stays INCOMPATIBLE until qualified (T14B).
 *
 * Core invariants:
 *   - Typed identity domains never collapse: a Bun cache hash, a file content
 *     hash, and a DomainForge semantic fingerprint are distinct branded
 *     identities; cross-domain substitution is refused at compile time AND at
 *     runtime (REQ-DOM-002, Attack 1).
 *   - Pinned schema: unknown top-level/envelope keys are schema violations;
 *     digests are verified; declared schema_version is checked BEFORE any use.
 *     Schema validity, digest integrity, and semantic diagnostics remain
 *     separate results; output carrying diagnostics is never conformant
 *     (REQ-DOM-004).
 *   - Bindings to Gauntlet Units are EXPLICIT by exact producer declaration
 *     identity. No name-based joins; missing bindings stay absent/unknown;
 *     enrichment is never fabricated (REQ-DOM-005, Attack 2).
 *   - A semantic concept keeps ONE source identity across every placement:
 *     FocusRef interning guarantees the same (source, kind, source_id) triple
 *     resolves to one identity (REQ-DOM-003).
 *   - The adapter never spawns a process: producer output arrives out of band.
 *     Invocation/qualification of live binaries is a T14B seam, deliberately
 *     out of scope here (no implicit flag fallback).
 */

import { FocusRef } from "./focus";
import type { ContextCandidate } from "./horizon";
import type {
  RelationEdge,
  RelationProvenance,
  RelationQuerySpec,
  TraversalResult,
} from "./relations";
import { traverseRelations } from "./relations";
import type { RenderedLine, RenderedRecord } from "./render-tree";

// ---------------------------------------------------------------------------
// Pinned producer contract
// ---------------------------------------------------------------------------

/** The one envelope schema this adapter admits (producer application/envelope.rs). */
export const PINNED_ENVELOPE_SCHEMA_VERSION = "domainforge-semantic-envelope/v1";

/** The only producer admitted at the document level (producer KNOWN_PRODUCERS). */
export const PINNED_PRODUCER_NAME = "domainforge-core";

/** Producer version captured from the locally qualified binaries. */
export const PINNED_PRODUCER_VERSION = "0.16.0";

/** The producer language/canonicalization pins carried by qualified captures. */
export const PINNED_LANGUAGE_SCHEMA_VERSION = "domainforge-ast/v3";
export const PINNED_CANONICALIZATION_VERSION = "domainforge-canonicalization/v1";
export const PINNED_INTERPRETATION_VERSION = "domainforge-interpretation/v1";

/**
 * Local qualification record for the three `domainforge 0.16.0` builds found on
 * the builder workstation (see module docs). Binary digests are recorded
 * evidence, not test assertions: peer/CI machines may not carry these builds.
 */
export interface ProducerQualification {
  readonly version: string;
  readonly binary: string;
  readonly binary_sha256: string;
  readonly envelope_cli: string;
  readonly emits_bare_document: boolean;
  readonly answers_sxr_slice0b_invocation: boolean;
}

export const QUALIFIED_PRODUCER_BUILDS: readonly ProducerQualification[] = Object.freeze([
  {
    version: "0.16.0",
    binary: "~/.cargo/bin/domainforge (installed)",
    binary_sha256: "ee3ee077e2dcf0a38278fa878e1fcebed1d2cc7d22b3f0031ef51a292b775ad4",
    envelope_cli: "no `envelope` subcommand",
    emits_bare_document: false,
    answers_sxr_slice0b_invocation: false,
  },
  {
    version: "0.16.0",
    binary: "domainforge/target/release/domainforge",
    binary_sha256: "48a4ed2564ebc8cea429f77e9666ada15e2065a8221599d6e379ab6f1b0ac821",
    envelope_cli: "`envelope <entry.sea>` → bare D document on stdout",
    emits_bare_document: true,
    answers_sxr_slice0b_invocation: false,
  },
  {
    version: "0.16.0",
    binary: "domainforge/target/debug/domainforge (working-tree build)",
    binary_sha256: "d599698541bff6d8e43419609106ccd2364524bd1bc0f2efa7563e6c5d4b42a5",
    envelope_cli: "`envelope --emit representation|cep|both|verification-contract <entry.sea>`",
    emits_bare_document: true,
    answers_sxr_slice0b_invocation: true,
  },
]);

/**
 * Producer/consumer protocol compatibility registry (REQ-DOM-004, DEC-005).
 * Selection is EXPLICIT: this adapter pins pair P1; the SXR Slice 0B consumer
 * pair is recorded INCOMPATIBLE against the installed/release producers and
 * UNQUALIFIED even against the working-tree build (T14B must re-qualify it;
 * DEC-005 keeps SXR enrichment optional until then). Statuses:
 *   pinned        — selected and digested by this adapter.
 *   incompatible  — the pair cannot interoperate as shipped (verified locally).
 *   unqualified   — may interoperate but is deliberately NOT selected.
 *   unknown       — an unrecognized contract; never silently adopted.
 */
export type ProtocolPairStatus = "pinned" | "incompatible" | "unqualified" | "unknown";

export interface ProtocolPair {
  readonly pair_id: string;
  readonly producer: string;
  readonly consumer: string;
  readonly request_shape: string;
  readonly response_shape: string;
  readonly status: ProtocolPairStatus;
  /** Whether the fact was verified on this machine (vs documented only). */
  readonly locally_verified: boolean;
  readonly evidence: string;
}

export const PROTOCOL_COMPATIBILITY: readonly ProtocolPair[] = Object.freeze([
  {
    pair_id: "P1",
    producer: "domainforge release 0.16.0 `envelope <entry.sea>` (bare D)",
    consumer: "gauntlet workbench T14 domainforge-adapter",
    request_shape: "`domainforge envelope <entry.sea>` (positional entry, no flags)",
    response_shape: `bare ${PINNED_ENVELOPE_SCHEMA_VERSION} document`,
    status: "pinned",
    locally_verified: true,
    evidence:
      "captured byte-exact; self_hash/semantic_closure_hash recomputation verified against the capture",
  },
  {
    pair_id: "P2",
    producer: "domainforge installed 0.16.0 (~/.cargo/bin)",
    consumer: "sxr-df CliDomainForgeAdapter (Slice 0B)",
    request_shape: "`envelope --emit both <entry>`",
    response_shape: '{"representation": D, "cep_envelope": E}',
    status: "incompatible",
    locally_verified: true,
    evidence:
      "sxr-df src/adapter.rs argv; binary has no `envelope` subcommand at all " +
      "(exit 2: unrecognized subcommand)",
  },
  {
    pair_id: "P3",
    producer: "domainforge release 0.16.0 (target/release)",
    consumer: "sxr-df CliDomainForgeAdapter (Slice 0B)",
    request_shape: "`envelope --emit both <entry>`",
    response_shape: '{"representation": D, "cep_envelope": E}',
    status: "incompatible",
    locally_verified: true,
    evidence: "release build rejects --emit (exit 2: unexpected argument '--emit')",
  },
  {
    pair_id: "P4",
    producer: "domainforge working-tree build (target/debug)",
    consumer: "sxr-df CliDomainForgeAdapter (Slice 0B)",
    request_shape: "`envelope --emit both <entry>`",
    response_shape: '{"representation": D, "cep_envelope": E}',
    status: "unqualified",
    locally_verified: true,
    evidence:
      "invocation succeeds locally, but the pair is deliberately NOT selected; " +
      "uncommitted build + SXR ledger composition are T14B qualification scope (DEC-005)",
  },
]);

/**
 * Record the compatibility selections for a requested producer/consumer shape.
 * Multiple pairs may share a request shape (e.g. the SXR invocation across
 * producer builds); ALL recorded pairs are returned, most-severe status first.
 * Unknown shapes yield an empty list — never an implicit fallback (REQ-DOM-004).
 */
const STATUS_SEVERITY: Readonly<Record<ProtocolPairStatus, number>> = Object.freeze({
  incompatible: 0,
  unqualified: 1,
  unknown: 2,
  pinned: 3,
});

export function selectProtocolPairs(requestShape: string): readonly ProtocolPair[] {
  return PROTOCOL_COMPATIBILITY.filter((pair) => pair.request_shape === requestShape).sort(
    (a, b) => STATUS_SEVERITY[a.status] - STATUS_SEVERITY[b.status],
  );
}

// ---------------------------------------------------------------------------
// Typed identity domains (REQ-DOM-002, Attack 1)
// ---------------------------------------------------------------------------

/**
 * Three mutually non-assignable identity domains. Each is an opaque branded
 * object minted only by its own function; the brand symbols are module-private
 * (no literal can be constructed outside), and every mint registers its value
 * in a domain WeakSet so runtime consumers refuse forged or foreign objects.
 */
const BunCacheHashBrand = Symbol("domainforge-adapter.BunCacheHash");
const FileContentHashBrand = Symbol("domainforge-adapter.FileContentHash");
const SemanticFingerprintBrand = Symbol("domainforge-adapter.SemanticFingerprint");

/** Bun module/transpiler cache identity. Opaque Bun-internal token. */
export type BunCacheHash = {
  readonly [BunCacheHashBrand]: "BunCacheHash";
  readonly value: string;
};
/** Content-addressed file identity (`sha256:<hex>` of file bytes). */
export type FileContentHash = {
  readonly [FileContentHashBrand]: "FileContentHash";
  readonly value: string;
};
/**
 * DomainForge semantic model identity: the producer's semantic_closure_hash
 * (sha256 over the canonical envelope payload) — NEVER a cache or file hash.
 */
export type SemanticFingerprint = {
  readonly [SemanticFingerprintBrand]: "SemanticFingerprint";
  readonly value: string;
};

const bunCacheHashes = new WeakSet<BunCacheHash>();
const fileContentHashes = new WeakSet<FileContentHash>();
const semanticFingerprints = new WeakSet<SemanticFingerprint>();

/** Typed refusal when an identity of one domain is used where another is required. */
export class CrossDomainIdentityError extends Error {
  constructor(detail: string) {
    super(`cross-domain identity substitution refused: ${detail}`);
    this.name = "CrossDomainIdentityError";
  }
}

const SHA256_PATTERN = /^sha256:[0-9a-f]{64}$/;

/** Mint a Bun cache identity from an opaque Bun cache token. */
export function bunCacheHash(token: string): BunCacheHash {
  if (typeof token !== "string" || token.length === 0) {
    throw new CrossDomainIdentityError("Bun cache hash requires a non-empty string");
  }
  const minted: BunCacheHash = { [BunCacheHashBrand]: "BunCacheHash", value: token };
  bunCacheHashes.add(minted);
  return minted;
}

/** Mint a file content identity from a `sha256:<hex>` digest of the file bytes. */
export function fileContentHash(digest: string): FileContentHash {
  if (!SHA256_PATTERN.test(digest)) {
    throw new CrossDomainIdentityError(
      `file content hash '${digest}' is not a lowercase sha256:<hex> digest`,
    );
  }
  const minted: FileContentHash = { [FileContentHashBrand]: "FileContentHash", value: digest };
  fileContentHashes.add(minted);
  return minted;
}

/**
 * Mint a DomainForge semantic fingerprint from the producer's
 * semantic_closure_hash (`sha256:<hex>` over the canonical envelope payload).
 */
export function semanticFingerprint(closureHash: string): SemanticFingerprint {
  if (!SHA256_PATTERN.test(closureHash)) {
    throw new CrossDomainIdentityError(
      `semantic fingerprint '${closureHash}' is not a lowercase sha256:<hex> digest`,
    );
  }
  const minted: SemanticFingerprint = {
    [SemanticFingerprintBrand]: "SemanticFingerprint",
    value: closureHash,
  };
  semanticFingerprints.add(minted);
  return minted;
}

function assertDomain<T extends object>(registry: WeakSet<T>, value: T, domain: string): void {
  if (!registry.has(value)) {
    throw new CrossDomainIdentityError(
      `value was not minted as a ${domain} (forged or foreign-domain identity)`,
    );
  }
}

/** Require a genuine Bun cache identity at runtime (refuses forged/foreign objects). */
export function requireBunCacheHash(value: BunCacheHash): string {
  assertDomain(bunCacheHashes, value, "Bun cache hash");
  return value.value;
}

/** Require a genuine file content identity at runtime. */
export function requireFileContentHash(value: FileContentHash): string {
  assertDomain(fileContentHashes, value, "file content hash");
  return value.value;
}

/** Require a genuine semantic fingerprint at runtime (the Attack 1 chokepoint). */
export function requireSemanticFingerprint(value: SemanticFingerprint): string {
  assertDomain(semanticFingerprints, value, "semantic fingerprint");
  return value.value;
}

/** A semantic model identity carrying all three DISTINCT identity domains. */
export interface SemanticModelIdentity {
  readonly model_id: string;
  /** DomainForge semantic fingerprint — the model's own identity domain. */
  readonly semantic_fingerprint: SemanticFingerprint;
  /** File content identity of the entry source, when the caller verified one. */
  readonly entry_file_hash: FileContentHash | undefined;
  /** Bun cache identity of a compiled artifact, when the caller verified one. */
  readonly artifact_cache_hash: BunCacheHash | undefined;
}

// ---------------------------------------------------------------------------
// Envelope document types (mirrors of the producer's serde structs, narrowed)
// ---------------------------------------------------------------------------

export interface DfProducerIdentity {
  readonly name: string;
  readonly version: string;
}

export interface DfInputs {
  readonly source_set_hash: string;
  readonly semantic_pack_set_hash: string;
  readonly language_schema_version: string;
  readonly interpretation_version: string;
}

export interface DfModuleRef {
  readonly logical_id: string;
  readonly semantic_content_hash: string;
  readonly namespace: string;
}

export interface DfImportEdge {
  readonly importer: string;
  readonly imported: string;
}

export interface DfNamespaceBinding {
  readonly namespace: string;
  readonly logical_id: string;
}

export interface DfSemanticPackRef {
  readonly pack_id: string;
  readonly content_hash: string;
}

/** Tagged declaration id: concept | application | occurrence. */
export type DfDeclarationId =
  | { readonly kind: "concept"; readonly id: string }
  | { readonly kind: "application"; readonly id: string }
  | {
      readonly kind: "occurrence";
      readonly occurrence_kind: string;
      readonly content_hash: string;
      readonly duplicate_index: number;
    };

/** Tagged reference target: declaration | namespace. */
export type DfReferenceTarget =
  | { readonly kind: "declaration"; readonly id: DfDeclarationId }
  | { readonly kind: "namespace"; readonly exact_name: string; readonly logical_module_id: string };

export const DF_DECLARATION_KINDS = [
  "dimension",
  "unit",
  "record",
  "enum",
  "operation",
  "entity",
  "resource",
  "flow",
  "pattern",
  "role",
  "relation",
  "role_binding",
  "instance",
  "policy",
  "concept_change",
  "metric",
] as const;

export type DfDeclarationKind = (typeof DF_DECLARATION_KINDS)[number];

export interface DfSemanticDeclaration {
  readonly id: DfDeclarationId;
  readonly logical_module_id: string;
  readonly kind: DfDeclarationKind;
  /** Payload data of the pinned kind; projected kinds are validated deeply. */
  readonly data: Readonly<Record<string, unknown>>;
}

export interface DfResolvedReference {
  readonly source: DfDeclarationId;
  readonly field_path: string;
  readonly target: DfReferenceTarget;
}

export interface DfEnvelope {
  readonly language_schema_version: string;
  readonly canonicalization_version: string;
  readonly compiler_interpretation_version: string;
  readonly modules: readonly DfModuleRef[];
  readonly import_graph: readonly DfImportEdge[];
  readonly namespace_bindings: readonly DfNamespaceBinding[];
  readonly semantic_packs: readonly DfSemanticPackRef[];
  readonly semantic_declarations: readonly DfSemanticDeclaration[];
  readonly resolved_references: readonly DfResolvedReference[];
  /** Opaque application contract payload (not projected by this adapter). */
  readonly application_contract: Readonly<Record<string, unknown>>;
}

export interface DfEnvelopeDocument {
  readonly schema_version: string;
  readonly producer: DfProducerIdentity;
  readonly inputs: DfInputs;
  readonly self_hash: string;
  readonly semantic_closure_hash: string;
  readonly envelope: DfEnvelope;
}

// ---------------------------------------------------------------------------
// Canonical JSON and digest verification (verified against a real capture)
// ---------------------------------------------------------------------------

/**
 * The producer's canonical JSON (semantic_pack/canonical_json.rs): recursively
 * key-sorted, compact separators. Numbers: the pinned captures carry only
 * integers; non-integer floats inside annotation payloads may serialize
 * differently between serde_json and ECMAScript and are a recorded limitation.
 */
export function canonicalJson(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJson(item)).join(",")}]`;
  }
  if (typeof value === "object") {
    const record = value as Record<string, unknown>;
    const keys = Object.keys(record).sort();
    return `{${keys
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
      .join(",")}}`;
  }
  if (typeof value === "undefined") {
    throw new Error("canonical JSON cannot represent undefined");
  }
  const serialized = JSON.stringify(value);
  if (serialized === undefined) {
    throw new Error(`canonical JSON cannot represent value of type ${typeof value}`);
  }
  return serialized;
}

function sha256Hex(text: string): string {
  return `sha256:${new Bun.CryptoHasher("sha256").update(text).digest("hex")}`;
}

/**
 * Recompute the producer's self-hash rule (application/envelope.rs
 * `envelope_document_self_hash`): sha256 of canonical JSON of the RAW producer
 * document with self_hash excluded from its own digest. The digest is defined
 * over the producer's exact document bytes — the adapter's typed projection
 * view is NOT the hashed form.
 */
export function envelopeSelfHash(rawDocument: Record<string, unknown>): string {
  const selfless: Record<string, unknown> = {};
  for (const key of Object.keys(rawDocument)) {
    if (key !== "self_hash") {
      selfless[key] = rawDocument[key];
    }
  }
  return sha256Hex(canonicalJson(selfless));
}

/**
 * sha256 of canonical JSON of the raw envelope payload (the semantic
 * fingerprint the producer declares).
 */
export function envelopeClosureHash(rawEnvelope: unknown): string {
  return sha256Hex(canonicalJson(rawEnvelope));
}

// ---------------------------------------------------------------------------
// Typed parse results
// ---------------------------------------------------------------------------

/** Why a document was refused as incompatible (a KNOWN other contract). */
export type IncompatibleReason =
  | "schema_version_mismatch"
  | "cep_snapshot_wrapper"
  | "verification_contract_payload";

/** A strict schema violation (the pinned contract itself was violated). */
export class EnvelopeSchemaError extends Error {
  readonly path: string;
  constructor(path: string, detail: string) {
    super(`envelope schema violation at ${path}: ${detail}`);
    this.name = "EnvelopeSchemaError";
    this.path = path;
  }
}

/**
 * Discriminated parse result. `parsed` means schema-valid AND digest-verified;
 * model validity and diagnostics travel separately so resolved output carrying
 * diagnostics never appears conformant (REQ-DOM-004).
 */
export type SemanticEnvelopeParse =
  | { readonly status: "parsed"; readonly model: QualifiedSemanticModel }
  | {
      readonly status: "invalid";
      readonly reason: string;
      readonly detail: string;
      readonly declared_schema_version: string | undefined;
    }
  | {
      readonly status: "incompatible";
      readonly reason: IncompatibleReason;
      readonly detail: string;
      readonly declared_schema_version: string | undefined;
    }
  | { readonly status: "unknown"; readonly detail: string };

/**
 * A qualified semantic model: the parsed document plus its separate validity
 * and diagnostics channel, and the typed identity domains it earned.
 */
export interface QualifiedSemanticModel {
  /** Typed projection view (renames producer payload tags; NOT the hashed form). */
  readonly document: DfEnvelopeDocument;
  /**
   * The RAW producer document, lossless. Digest verification is defined over
   * these bytes; the typed view above is a reinterpretation for projection.
   */
  readonly document_raw: Readonly<Record<string, unknown>>;
  readonly model_id: string;
  /** DomainForge semantic fingerprint = producer semantic_closure_hash. */
  readonly fingerprint: SemanticFingerprint;
  /**
   * Model validation status, separate from parse status. A model with
   * diagnostics is `false` and its output NEVER passes as conformant.
   */
  readonly model_valid: boolean;
  /** Preserved producer diagnostics (`"<rule>: <message>"` strings), if any. */
  readonly diagnostics: readonly string[];
}

// --- strict schema helpers ---------------------------------------------------

function asObject(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new EnvelopeSchemaError(path, "expected an object");
  }
  return value as Record<string, unknown>;
}

function requireExactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
): void {
  for (const key of Object.keys(value)) {
    if (!allowed.includes(key)) {
      throw new EnvelopeSchemaError(path, `unknown field '${key}' (pinned schema is closed)`);
    }
  }
}

function requireString(value: Record<string, unknown>, key: string, path: string): string {
  const field = value[key];
  if (typeof field !== "string") {
    throw new EnvelopeSchemaError(`${path}.${key}`, "expected a string");
  }
  return field;
}

function requireArray(
  value: Record<string, unknown>,
  key: string,
  path: string,
): readonly unknown[] {
  const field = value[key];
  if (!Array.isArray(field)) {
    throw new EnvelopeSchemaError(`${path}.${key}`, "expected an array");
  }
  return field;
}

function parseDeclarationId(value: unknown, path: string): DfDeclarationId {
  const record = asObject(value, path);
  const kind = requireString(record, "kind", path);
  const data = asObject(record["data"], `${path}.data`);
  if (kind === "concept") {
    requireExactKeys(data, ["id"], `${path}.data`);
    return { kind: "concept", id: requireString(data, "id", `${path}.data`) };
  }
  if (kind === "application") {
    requireExactKeys(data, ["id"], `${path}.data`);
    return { kind: "application", id: requireString(data, "id", `${path}.data`) };
  }
  if (kind === "occurrence") {
    requireExactKeys(data, ["kind", "content_hash", "duplicate_index"], `${path}.data`);
    const occurrenceKind = requireString(data, "kind", `${path}.data`);
    const contentHash = requireString(data, "content_hash", `${path}.data`);
    const duplicateIndex = data["duplicate_index"];
    if (typeof duplicateIndex !== "number" || !Number.isInteger(duplicateIndex)) {
      throw new EnvelopeSchemaError(`${path}.data["duplicate_index"]`, "expected an integer");
    }
    return {
      kind: "occurrence",
      occurrence_kind: occurrenceKind,
      content_hash: contentHash,
      duplicate_index: duplicateIndex,
    };
  }
  throw new EnvelopeSchemaError(`${path}.kind`, `unknown declaration id kind '${kind}'`);
}

function parseReferenceTarget(value: unknown, path: string): DfReferenceTarget {
  const record = asObject(value, path);
  const kind = requireString(record, "kind", path);
  const data = asObject(record["data"], `${path}.data`);
  if (kind === "declaration") {
    requireExactKeys(data, ["id"], `${path}.data`);
    return { kind: "declaration", id: parseDeclarationId(data["id"], `${path}.data.id`) };
  }
  if (kind === "namespace") {
    requireExactKeys(data, ["exact_name", "logical_module_id"], `${path}.data`);
    return {
      kind: "namespace",
      exact_name: requireString(data, "exact_name", `${path}.data`),
      logical_module_id: requireString(data, "logical_module_id", `${path}.data`),
    };
  }
  throw new EnvelopeSchemaError(`${path}.kind`, `unknown reference target kind '${kind}'`);
}

function parseDeclarationEntry(value: unknown, path: string): DfSemanticDeclaration {
  const record = asObject(value, path);
  requireExactKeys(record, ["id", "logical_module_id", "declaration"], path);
  const id = parseDeclarationId(record["id"], `${path}.id`);
  const logicalModuleId = requireString(record, "logical_module_id", path);
  const payload = asObject(record["declaration"], `${path}.declaration`);
  requireExactKeys(payload, ["kind", "data"], `${path}.declaration`);
  const kind = requireString(payload, "kind", `${path}.declaration`);
  if (!(DF_DECLARATION_KINDS as readonly string[]).includes(kind)) {
    throw new EnvelopeSchemaError(
      `${path}.declaration.kind`,
      `unknown declaration kind '${kind}' (pinned producer kinds only)`,
    );
  }
  const data = asObject(payload["data"], `${path}.declaration.data`);
  return {
    id,
    logical_module_id: logicalModuleId,
    kind: kind as DfDeclarationKind,
    data,
  };
}

function parseEnvelope(value: unknown, path: string): DfEnvelope {
  const record = asObject(value, path);
  requireExactKeys(
    record,
    [
      "language_schema_version",
      "canonicalization_version",
      "compiler_interpretation_version",
      "modules",
      "import_graph",
      "namespace_bindings",
      "semantic_packs",
      "semantic_declarations",
      "resolved_references",
      "application_contract",
    ],
    path,
  );
  const modules = requireArray(record, "modules", path).map((item, index) => {
    const moduleRecord = asObject(item, `${path}.modules.${index}`);
    requireExactKeys(
      moduleRecord,
      ["logical_id", "semantic_content_hash", "namespace"],
      `${path}.modules.${index}`,
    );
    return {
      logical_id: requireString(moduleRecord, "logical_id", `${path}.modules.${index}`),
      semantic_content_hash: requireString(
        moduleRecord,
        "semantic_content_hash",
        `${path}.modules.${index}`,
      ),
      namespace: requireString(moduleRecord, "namespace", `${path}.modules.${index}`),
    };
  });
  const importGraph = requireArray(record, "import_graph", path).map((item, index) => {
    const edgeRecord = asObject(item, `${path}.import_graph.${index}`);
    requireExactKeys(edgeRecord, ["importer", "imported"], `${path}.import_graph.${index}`);
    return {
      importer: requireString(edgeRecord, "importer", `${path}.import_graph.${index}`),
      imported: requireString(edgeRecord, "imported", `${path}.import_graph.${index}`),
    };
  });
  const namespaceBindings = requireArray(record, "namespace_bindings", path).map((item, index) => {
    const bindingRecord = asObject(item, `${path}.namespace_bindings.${index}`);
    requireExactKeys(
      bindingRecord,
      ["namespace", "logical_id"],
      `${path}.namespace_bindings.${index}`,
    );
    return {
      namespace: requireString(bindingRecord, "namespace", `${path}.namespace_bindings.${index}`),
      logical_id: requireString(bindingRecord, "logical_id", `${path}.namespace_bindings.${index}`),
    };
  });
  const semanticPacks = requireArray(record, "semantic_packs", path).map((item, index) => {
    const packRecord = asObject(item, `${path}.semantic_packs.${index}`);
    requireExactKeys(packRecord, ["pack_id", "content_hash"], `${path}.semantic_packs.${index}`);
    return {
      pack_id: requireString(packRecord, "pack_id", `${path}.semantic_packs.${index}`),
      content_hash: requireString(packRecord, "content_hash", `${path}.semantic_packs.${index}`),
    };
  });
  const semanticDeclarations = requireArray(record, "semantic_declarations", path).map(
    (item, index) => parseDeclarationEntry(item, `${path}.semantic_declarations.${index}`),
  );
  const resolvedReferences = requireArray(record, "resolved_references", path).map(
    (item, index) => {
      const refRecord = asObject(item, `${path}.resolved_references.${index}`);
      requireExactKeys(
        refRecord,
        ["source", "field_path", "target"],
        `${path}.resolved_references.${index}`,
      );
      return {
        source: parseDeclarationId(
          refRecord["source"],
          `${path}.resolved_references.${index}.source`,
        ),
        field_path: requireString(refRecord, "field_path", `${path}.resolved_references.${index}`),
        target: parseReferenceTarget(
          refRecord["target"],
          `${path}.resolved_references.${index}.target`,
        ),
      };
    },
  );
  return {
    language_schema_version: requireString(record, "language_schema_version", path),
    canonicalization_version: requireString(record, "canonicalization_version", path),
    compiler_interpretation_version: requireString(record, "compiler_interpretation_version", path),
    modules,
    import_graph: importGraph,
    namespace_bindings: namespaceBindings,
    semantic_packs: semanticPacks,
    semantic_declarations: semanticDeclarations,
    resolved_references: resolvedReferences,
    application_contract: asObject(record["application_contract"], `${path}.application_contract`),
  };
}

/**
 * Classify untrusted producer output BEFORE pinning it to a schema: a
 * CEP-0008 snapshot wrapper and a verification contract are recognizable
 * OTHER contracts, reported as typed `incompatible`, never silently unwrapped.
 */
function classifyOuterDocument(record: Record<string, unknown>): "document" | "incompatible" {
  if ("cep_version" in record || "envelope_kind" in record) {
    return "incompatible";
  }
  if ("verification_contract" in record) {
    return "incompatible";
  }
  return "document";
}

/**
 * Parse the PINNED bare canonical semantic envelope document (producer stdout
 * of `domainforge envelope <entry.sea>`, release build 0.16.0). Strict: closed
 * key sets, digest verification, typed incompatible/invalid/unknown results.
 */
export function parseSemanticEnvelopeDocument(raw: string | Uint8Array): SemanticEnvelopeParse {
  let parsed: unknown;
  try {
    parsed = JSON.parse(typeof raw === "string" ? raw : new TextDecoder().decode(raw));
  } catch (error) {
    return {
      status: "invalid",
      reason: "malformed_json",
      detail: `producer output is not JSON: ${String(error)}`,
      declared_schema_version: undefined,
    };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {
      status: "invalid",
      reason: "not_an_object",
      detail: "producer output is not a JSON object",
      declared_schema_version: undefined,
    };
  }
  const record = parsed as Record<string, unknown>;
  if (classifyOuterDocument(record) === "incompatible") {
    return {
      status: "incompatible",
      reason: "cep_snapshot_wrapper",
      detail:
        "output is a CEP-0008 semantic snapshot wrapper (or verification contract); " +
        `this adapter pins the bare ${PINNED_ENVELOPE_SCHEMA_VERSION} document. ` +
        "Use parseProducerEnvelopeOutput for the --emit both carrier.",
      declared_schema_version: undefined,
    };
  }
  const declaredSchemaVersion =
    typeof record["schema_version"] === "string" ? record["schema_version"] : undefined;
  if (declaredSchemaVersion !== PINNED_ENVELOPE_SCHEMA_VERSION) {
    return {
      status: "incompatible",
      reason: "schema_version_mismatch",
      detail: `declared schema_version '${String(
        record["schema_version"],
      )}' is not the pinned '${PINNED_ENVELOPE_SCHEMA_VERSION}'`,
      declared_schema_version: declaredSchemaVersion,
    };
  }
  let document: DfEnvelopeDocument;
  try {
    requireExactKeys(
      record,
      ["schema_version", "producer", "inputs", "self_hash", "semantic_closure_hash", "envelope"],
      "$",
    );
    const producerRecord = asObject(record["producer"], "$.producer");
    requireExactKeys(producerRecord, ["name", "version"], "$.producer");
    const producerName = requireString(producerRecord, "name", "$.producer");
    const producerVersion = requireString(producerRecord, "version", "$.producer");
    if (producerName !== PINNED_PRODUCER_NAME) {
      return {
        status: "unknown",
        detail: `unknown producer '${producerName}'; this adapter pins '${PINNED_PRODUCER_NAME}'`,
      };
    }
    const inputsRecord = asObject(record["inputs"], "$.inputs");
    requireExactKeys(
      inputsRecord,
      [
        "source_set_hash",
        "semantic_pack_set_hash",
        "language_schema_version",
        "interpretation_version",
      ],
      "$.inputs",
    );
    document = {
      schema_version: declaredSchemaVersion,
      producer: { name: producerName, version: producerVersion },
      inputs: {
        source_set_hash: requireString(inputsRecord, "source_set_hash", "$.inputs"),
        semantic_pack_set_hash: requireString(inputsRecord, "semantic_pack_set_hash", "$.inputs"),
        language_schema_version: requireString(inputsRecord, "language_schema_version", "$.inputs"),
        interpretation_version: requireString(inputsRecord, "interpretation_version", "$.inputs"),
      },
      self_hash: requireString(record, "self_hash", "$"),
      semantic_closure_hash: requireString(record, "semantic_closure_hash", "$"),
      envelope: parseEnvelope(record["envelope"], "$.envelope"),
    };
    // Digest integrity: separate from schema validity, verified before use —
    // over the RAW producer document, not the typed projection view.
    const recomputedSelf = envelopeSelfHash(record);
    if (recomputedSelf !== document.self_hash) {
      return {
        status: "invalid",
        reason: "self_hash_mismatch",
        detail: `declared self_hash ${document.self_hash} != recomputed ${recomputedSelf}`,
        declared_schema_version: declaredSchemaVersion,
      };
    }
    const recomputedClosure = envelopeClosureHash(record["envelope"]);
    if (recomputedClosure !== document.semantic_closure_hash) {
      return {
        status: "invalid",
        reason: "semantic_closure_hash_mismatch",
        detail: `declared semantic_closure_hash ${document.semantic_closure_hash} != recomputed ${recomputedClosure}`,
        declared_schema_version: declaredSchemaVersion,
      };
    }
  } catch (error) {
    if (error instanceof EnvelopeSchemaError) {
      return {
        status: "invalid",
        reason: "schema_violation",
        detail: error.message,
        declared_schema_version: declaredSchemaVersion,
      };
    }
    throw error;
  }
  // The bare-D contract carries no diagnostics channel: absence here is real
  // absence, and model validity for this carrier is `true` by construction
  // (the producer only emits D when the model resolved).
  return {
    status: "parsed",
    model: {
      document,
      document_raw: record,
      model_id: `domainforge:${document.self_hash}`,
      fingerprint: semanticFingerprint(document.semantic_closure_hash),
      model_valid: true,
      diagnostics: [],
    },
  };
}

/**
 * Parse the producer's `--emit both` stdout carrier:
 * `{"representation": D, "cep_envelope": E}` — or the bare D / bare CEP
 * wrapper shapes. Diagnostics from the CEP extensions are PRESERVED and force
 * `model_valid: false`; the inline D is re-verified with the strict pinned
 * parser and cross-checked against the representation's declared
 * content_hash/semantic_hash (carriage must not change semantics).
 */
export function parseProducerEnvelopeOutput(raw: string | Uint8Array): SemanticEnvelopeParse {
  let parsed: unknown;
  try {
    parsed = JSON.parse(typeof raw === "string" ? raw : new TextDecoder().decode(raw));
  } catch (error) {
    return {
      status: "invalid",
      reason: "malformed_json",
      detail: `producer output is not JSON: ${String(error)}`,
      declared_schema_version: undefined,
    };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {
      status: "invalid",
      reason: "not_an_object",
      detail: "producer output is not a JSON object",
      declared_schema_version: undefined,
    };
  }
  const record = parsed as Record<string, unknown>;
  if (!("cep_envelope" in record)) {
    // Bare D, or a bare CEP wrapper (typed incompatible via the strict parser).
    return parseSemanticEnvelopeDocument(raw);
  }
  const representation = record["representation"];
  if (typeof representation !== "string") {
    return {
      status: "invalid",
      reason: "representation_not_inline_string",
      detail:
        "both-mode carrier requires an inline canonical D string in 'representation' " +
        "(CAS-carried content_ref requalification is T14B scope)",
      declared_schema_version: undefined,
    };
  }
  const inner = parseSemanticEnvelopeDocument(representation);
  if (inner.status !== "parsed") {
    return inner;
  }
  // Preserve the CEP extension diagnostics: resolved output carrying
  // diagnostics MUST NOT appear conformant (REQ-DOM-004).
  const envelopeValue = asObject(record["cep_envelope"], "$.cep_envelope");
  const extensions = asObject(envelopeValue["extensions"] ?? {}, "$.cep_envelope.extensions");
  const domainforge = asObject(
    extensions["domainforge"] ?? {},
    "$.cep_envelope.extensions.domainforge",
  );
  const rawDiagnostics = domainforge["diagnostics"];
  const diagnostics: string[] = Array.isArray(rawDiagnostics)
    ? rawDiagnostics.filter((item): item is string => typeof item === "string")
    : [];
  const declaredModelStatus = domainforge["model_validation_status"];
  const modelInvalid = diagnostics.length > 0 || declaredModelStatus === "invalid";
  if (modelInvalid) {
    return {
      status: "parsed",
      model: {
        document: inner.model.document,
        document_raw: inner.model.document_raw,
        model_id: inner.model.model_id,
        fingerprint: inner.model.fingerprint,
        model_valid: false,
        diagnostics,
      },
    };
  }
  return inner;
}

// ---------------------------------------------------------------------------
// Declaration identity (exact producer entity refs — the preserved_entities form)
// ---------------------------------------------------------------------------

/**
 * The producer's exact entity ref for a declaration id (application/envelope.rs
 * `declaration_entity_ref`): `concept:<id>`, `application:<id>`,
 * `occurrence:<kind>:<content_hash>:<duplicate_index>`. This string is THE
 * source identity of a semantic concept; bindings and FocusRefs use it and
 * nothing else (no name-based joins).
 */
export function declarationEntityRef(id: DfDeclarationId): string {
  switch (id.kind) {
    case "concept":
      return `concept:${id.id}`;
    case "application":
      return `application:${id.id}`;
    case "occurrence":
      return `occurrence:${id.occurrence_kind}:${id.content_hash}:${id.duplicate_index}`;
  }
}

function declarationName(declaration: DfSemanticDeclaration): string | undefined {
  const name = declaration.data["name"];
  return typeof name === "string" ? name : undefined;
}

function refTargetId(target: unknown): string | undefined {
  if (typeof target !== "object" || target === null) {
    return undefined;
  }
  const record = target as Record<string, unknown>;
  if (record["kind"] !== "declaration") {
    return undefined;
  }
  const data = record["data"];
  if (typeof data !== "object" || data === null) {
    return undefined;
  }
  const id = (data as Record<string, unknown>)["id"];
  if (typeof id !== "object" || id === null) {
    return undefined;
  }
  const idRecord = id as Record<string, unknown>;
  if (typeof idRecord["kind"] !== "string" || typeof idRecord["data"] !== "object") {
    return undefined;
  }
  try {
    return declarationEntityRef(parseDeclarationId(id, "$target"));
  } catch {
    return undefined;
  }
}

// ---------------------------------------------------------------------------
// Explicit Unit bindings (REQ-DOM-005) — declared links, never name joins
// ---------------------------------------------------------------------------

/** Typed refusal of a fabricated or name-based semantic binding. */
export class SemanticBindingError extends Error {
  constructor(detail: string) {
    super(`semantic unit binding refused: ${detail}`);
    this.name = "SemanticBindingError";
  }
}

export interface SemanticUnitBindingInput {
  /** The Gauntlet Unit FocusRef this binding attaches to (kind "WorkItem"). */
  readonly unit: FocusRef;
  /**
   * The EXACT producer declaration entity ref (`concept:<uuid>`,
   * `application:<id>`, `occurrence:...`). A declaration NAME is not an
   * identity and is refused.
   */
  readonly declaration_ref: string;
  /** Who declared this binding (never inferred by the adapter). */
  readonly declared_by: string;
}

export interface SemanticUnitBinding {
  readonly unit: FocusRef;
  readonly declaration_ref: string;
  readonly declared_by: string;
}

/**
 * Admit explicit Unit bindings against a parsed model. Every binding must use
 * the exact declaration entity ref present in the envelope; anything else (a
 * concept name, an unknown ref, a non-WorkItem focus) is a typed refusal.
 * Missing bindings stay missing — the adapter never invents links.
 */
export function admitUnitBindings(
  model: QualifiedSemanticModel,
  inputs: readonly SemanticUnitBindingInput[],
): readonly SemanticUnitBinding[] {
  const knownRefs = new Set(
    model.document.envelope.semantic_declarations.map((declaration) =>
      declarationEntityRef(declaration.id),
    ),
  );
  return inputs.map((input) => {
    if (input.unit.kind !== "WorkItem") {
      throw new SemanticBindingError(
        `binding target must be a WorkItem focus, got kind '${input.unit.kind}'`,
      );
    }
    if (!knownRefs.has(input.declaration_ref)) {
      const byName = model.document.envelope.semantic_declarations.some(
        (declaration) => declarationName(declaration) === input.declaration_ref,
      );
      throw new SemanticBindingError(
        byName
          ? `'${input.declaration_ref}' is a declaration NAME, not an identity; bind the exact entity ref (no name-based joins)`
          : `declaration ref '${input.declaration_ref}' does not exist in the bound model`,
      );
    }
    if (input.declared_by.trim().length === 0) {
      throw new SemanticBindingError("declared_by must name the binding authority");
    }
    return Object.freeze({
      unit: input.unit,
      declaration_ref: input.declaration_ref,
      declared_by: input.declared_by,
    });
  });
}

// ---------------------------------------------------------------------------
// Projection into FocusRef/Relation seams (REQ-DOM-003/004, Attack 2)
// ---------------------------------------------------------------------------

/** The source id every semantic FocusRef carries (one adapter, one source). */
export const SEMANTIC_SOURCE_ID = "domainforge";

/** Why enrichment is absent. Absence is explicit and typed — never fabricated. */
export type SemanticEnrichmentAbsentReason =
  | "no_model_output"
  | "parse_invalid"
  | "producer_incompatible"
  | "producer_unknown"
  | "model_invalid";

export type SemanticEnrichment =
  | {
      readonly status: "absent";
      readonly reason: SemanticEnrichmentAbsentReason;
      readonly detail: string;
    }
  | {
      readonly status: "bound";
      readonly model: QualifiedSemanticModel;
      /** One FocusRef per declaration — one source identity across placements. */
      readonly concepts: readonly SemanticConcept[];
      /** Graph-derived edges with model provenance (valid only in-model). */
      readonly relations: readonly RelationEdge[];
      /** Explicit, admitted Unit bindings. */
      readonly unit_bindings: readonly SemanticUnitBinding[];
    };

export interface SemanticConcept {
  /** Exact producer identity (`concept:<uuid>` …) — stable across placements. */
  readonly entity_ref: string;
  readonly focus: FocusRef;
  readonly kind: DfDeclarationKind;
  readonly name: string | undefined;
  /** Source location: the logical module path from the envelope (never a line). */
  readonly source_module: string;
  readonly namespace: string | undefined;
}

const SEMANTIC_MODEL_GENERATION_PREFIX = "semantic-model";

function semanticProvenance(model: QualifiedSemanticModel): RelationProvenance {
  return Object.freeze({
    generation_id: `${SEMANTIC_MODEL_GENERATION_PREFIX}:${model.document.self_hash}`,
    source_id: SEMANTIC_SOURCE_ID,
    source_cursor: model.document.semantic_closure_hash,
  });
}

function conceptFocus(declaration: DfSemanticDeclaration, name: string | undefined): FocusRef {
  // FocusRef interning guarantees one identity per entity_ref across placements.
  return FocusRef.for(
    SEMANTIC_SOURCE_ID,
    declaration.kind,
    declarationEntityRef(declaration.id),
    name,
  );
}

/**
 * Project a parsed model (or an explicit absence) into enrichment. With no
 * model output the result is typed-absent: callers get no relations, no
 * concepts, and nothing synthesized from names or hope (Attack 2).
 */
export function projectSemanticEnrichment(
  parse: SemanticEnvelopeParse | undefined,
  unitBindings: readonly SemanticUnitBinding[] = [],
): SemanticEnrichment {
  if (parse === undefined) {
    return Object.freeze({
      status: "absent",
      reason: "no_model_output",
      detail: "no DomainForge envelope output was provided for this Case",
    });
  }
  if (parse.status === "invalid") {
    return Object.freeze({
      status: "absent",
      reason: "parse_invalid",
      detail: `${parse.reason}: ${parse.detail}`,
    });
  }
  if (parse.status === "incompatible") {
    return Object.freeze({
      status: "absent",
      reason: "producer_incompatible",
      detail: `${parse.reason}: ${parse.detail}`,
    });
  }
  if (parse.status === "unknown") {
    return Object.freeze({
      status: "absent",
      reason: "producer_unknown",
      detail: parse.detail,
    });
  }
  const model = parse.model;
  if (!model.model_valid) {
    return Object.freeze({
      status: "absent",
      reason: "model_invalid",
      detail: `producer reported ${model.diagnostics.length} diagnostic(s); enrichment withheld`,
    });
  }
  const moduleNamespace = new Map<string, string>();
  for (const binding of model.document.envelope.namespace_bindings) {
    moduleNamespace.set(binding.logical_id, binding.namespace);
  }
  const concepts: SemanticConcept[] = model.document.envelope.semantic_declarations.map(
    (declaration) => {
      const name = declarationName(declaration);
      return Object.freeze({
        entity_ref: declarationEntityRef(declaration.id),
        focus: conceptFocus(declaration, name),
        kind: declaration.kind,
        name,
        source_module: declaration.logical_module_id,
        namespace: moduleNamespace.get(declaration.logical_module_id),
      });
    },
  );
  const knownRefs = new Set(concepts.map((concept) => concept.entity_ref));
  const conceptFocusByIdentity = new Map<string, FocusRef>(
    model.document.envelope.semantic_declarations.map((declaration) => [
      declarationEntityRef(declaration.id),
      conceptFocus(declaration, declarationName(declaration)),
    ]),
  );
  const provenance = semanticProvenance(model);
  const edges: RelationEdge[] = [];

  const pushEdge = (
    sourceRef: string,
    targetRef: string,
    kind: string,
    sourceVersion: string,
  ): void => {
    // Endpoints resolve to the SAME interned FocusRefs the concepts carry, so
    // one source identity holds across relations and placements alike.
    const sourceFocus =
      conceptFocusByIdentity.get(sourceRef) ??
      FocusRef.for(SEMANTIC_SOURCE_ID, "semantic", sourceRef);
    const targetFocus =
      conceptFocusByIdentity.get(targetRef) ??
      FocusRef.for(SEMANTIC_SOURCE_ID, "semantic", targetRef);
    edges.push(
      Object.freeze({
        source_ref: sourceFocus,
        target_ref: targetFocus,
        kind,
        provenance,
        source_version: sourceVersion,
        valid: knownRefs.has(sourceRef) && knownRefs.has(targetRef),
      }),
    );
  };

  for (const declaration of model.document.envelope.semantic_declarations) {
    const _sourceRef = declarationEntityRef(declaration.id);
    const closure = model.document.semantic_closure_hash;
    if (declaration.kind === "flow") {
      const fromRef = refTargetId(declaration.data["from_entity"]);
      const toRef = refTargetId(declaration.data["to_entity"]);
      if (fromRef !== undefined && toRef !== undefined) {
        pushEdge(fromRef, toRef, "semantic_flow", closure);
      }
    }
    if (declaration.kind === "relation") {
      const subjectRef = refTargetId(declaration.data["subject_role"]);
      const objectRef = refTargetId(declaration.data["object_role"]);
      const predicate = declaration.data["predicate"];
      if (subjectRef !== undefined && objectRef !== undefined && typeof predicate === "string") {
        pushEdge(subjectRef, objectRef, `semantic_relation:${predicate}`, closure);
      }
    }
    if (declaration.kind === "role_binding") {
      const roleRef = refTargetId(declaration.data["role"]);
      const entityRef = refTargetId(declaration.data["entity"]);
      if (roleRef !== undefined && entityRef !== undefined) {
        pushEdge(roleRef, entityRef, "semantic_role_binding", closure);
      }
    }
  }

  for (const reference of model.document.envelope.resolved_references) {
    const sourceRef = declarationEntityRef(reference.source);
    if (reference.target.kind === "declaration") {
      pushEdge(
        sourceRef,
        declarationEntityRef(reference.target.id),
        `semantic_reference:${reference.field_path}`,
        model.document.semantic_closure_hash,
      );
    }
  }

  return Object.freeze({
    status: "bound",
    model,
    concepts: Object.freeze(concepts),
    relations: Object.freeze(edges),
    unit_bindings: Object.freeze([...unitBindings]),
  });
}

/**
 * OPTIONAL semantic context candidates for horizon-local context construction.
 * Shaped exactly like horizon.ts ContextCandidate (role "relation", never
 * mandatory): callers compose them into a materialization plan when — and only
 * when — enrichment is bound.
 */
export function semanticContextCandidates(
  enrichment: SemanticEnrichment,
): readonly ContextCandidate[] {
  if (enrichment.status !== "bound") {
    return Object.freeze([]);
  }
  return enrichment.concepts.map((concept) =>
    Object.freeze({
      focus: concept.focus,
      role: "relation" as const,
      detail: `semantic ${concept.kind} from model ${enrichment.model.document.self_hash}`,
      mandatory: false,
    }),
  );
}

/**
 * Semantic neighborhood traversal: the semantic edges are ordinary RelationEdges,
 * so horizon-local context construction reuses the cycle-safe traverseRelations.
 */
export function semanticNeighborhood(
  enrichment: SemanticEnrichment,
  root: FocusRef,
  queries: readonly RelationQuerySpec[],
): TraversalResult {
  if (enrichment.status !== "bound") {
    return Object.freeze({ reached: Object.freeze([root]), edges: Object.freeze([]) });
  }
  return traverseRelations(enrichment.relations, root, queries);
}

// ---------------------------------------------------------------------------
// Inspector feed (existing render-tree shapes; T12 sections render this data)
// ---------------------------------------------------------------------------

/**
 * Render the semantic enrichment state as Inspector-section content using the
 * existing RenderedLine/RenderedRecord contract. Absent state renders an
 * explicit "absent" line and emits NO relation records — absence is shown,
 * never faked. Bound state shows model identity, declarations with source
 * locations, preserved diagnostics, and explicit Unit bindings.
 */
export function renderSemanticSection(enrichment: SemanticEnrichment): {
  readonly id: string;
  readonly title: string;
  readonly lines: readonly RenderedLine[];
  readonly records: readonly RenderedRecord[];
} {
  if (enrichment.status === "absent") {
    return {
      id: "semantic",
      title: "SEMANTIC",
      lines: [
        {
          spans: [
            { text: "semantic enrichment ", emphasis: "dim" },
            { text: `ABSENT (${enrichment.reason})`, token: "structure.neutral" },
            { text: ` — ${enrichment.detail}`, emphasis: "dim" },
          ],
        },
        {
          spans: [{ text: "no semantic relations are shown because none exist", emphasis: "dim" }],
        },
      ],
      records: [],
    };
  }
  const doc = enrichment.model.document;
  const lines: RenderedLine[] = [
    {
      spans: [
        { text: "model       ".padEnd(13), emphasis: "dim" },
        { text: doc.producer.name, emphasis: "bold" },
        { text: ` ${doc.producer.version} · ${doc.schema_version}` },
        {
          text: enrichment.model.model_valid ? "" : " · MODEL INVALID",
          token: "integrity.partial",
        },
      ],
    },
    {
      spans: [
        { text: "fingerprint ".padEnd(13), emphasis: "dim" },
        { text: doc.semantic_closure_hash },
      ],
    },
    {
      spans: [
        { text: "source      ".padEnd(13), emphasis: "dim" },
        { text: `${doc.inputs.source_set_hash} · packs ${doc.inputs.semantic_pack_set_hash}` },
      ],
    },
  ];
  for (const diagnostic of enrichment.model.diagnostics) {
    lines.push({
      spans: [
        { text: "diagnostic  ".padEnd(13), emphasis: "dim" },
        { text: diagnostic, token: "integrity.partial" },
      ],
    });
  }
  const records: RenderedRecord[] = [];
  for (const concept of enrichment.concepts) {
    lines.push({
      spans: [
        { text: `${concept.kind} `.padEnd(13), emphasis: "dim" },
        { text: `${concept.name ?? concept.entity_ref}`, emphasis: "bold" },
        {
          text: `  (${concept.source_module}${concept.namespace ? ` · ns ${concept.namespace}` : ""} · ${concept.entity_ref})`,
          emphasis: "dim",
        },
      ],
    });
  }
  for (const edge of enrichment.relations) {
    records.push({
      id: `semantic-relation:${edge.kind}:${edge.source_ref.source_id}:${edge.target_ref.source_id}`,
      kind: "relation",
      fields: {
        kind: edge.kind,
        source: edge.source_ref.source_id,
        target: edge.target_ref.source_id,
        valid: String(edge.valid),
        provenance: `${edge.provenance.generation_id} · ${edge.provenance.source_id}@${edge.provenance.source_cursor}`,
      },
    });
  }
  for (const binding of enrichment.unit_bindings) {
    lines.push({
      spans: [
        { text: "binding     ".padEnd(13), emphasis: "dim" },
        { text: `${binding.unit.id} ⇄ ${binding.declaration_ref}` },
        { text: `  (declared by ${binding.declared_by})`, emphasis: "dim" },
      ],
    });
  }
  return { id: "semantic", title: "SEMANTIC", lines, records };
}
