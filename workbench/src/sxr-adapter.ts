/**
 * Reality Trace (SXR) evidence lens adapter (plan T14B, REQ-DOM-004/005,
 * REQ-SXR-001..004, REQ-SYNC-003, DEC-005 requalification).
 *
 * Parses the PINNED SXR producer/consumer outputs — the versioned `--json`
 * output envelope (`sxr_output_version: "1"`, sxr-cli src/output.rs) and the
 * RAW ledger export array (sxr-cli src/commands.rs `ledger_export`) SEPARATELY
 * — and projects an inspectable declaration → linked Unit → question →
 * threshold → evidence → settlement path into the existing Inspector/Evidence/
 * Trace data contract. SXR identity and settlement stay source-typed and
 * DISTINCT from Gauntlet's; nothing here mutates a Gauntlet Settlement,
 * Residual, or EvidenceRef (REQ-SXR-002).
 *
 * Producer qualification (verified locally on the builder workstation
 * 2026-09-09; captures + digests pinned under
 * .agents/reports/gauntlet-tui-prime-workbench/evidence/T14B/captures/):
 *
 *   - `sxr 0.1.0` (target/debug build, sha256 89c006fb…) driven through a FULL
 *     real lifecycle on an isolated XDG root: repo enroll → question declare →
 *     threshold declare → semantic snapshot → run begin → plan → verify
 *     (real cargo-test observation) → evidence create → settlement register →
 *     ledger export/verify. Exit-3 F-19b pending-refusal captured too.
 *   - The snapshot path composed the REAL Slice 0B producer: sxr's default
 *     DomainForge discovery
 *     (`domainforge-slice0b/target/debug/domainforge`, sha256 f0ca9def…)
 *     answering `envelope --emit both`; the installed `~/.cargo/bin/domainforge`
 *     (ee3ee077…) still has no `envelope` subcommand (T14 pair P2 holds).
 *   - DEC-005 Correction Round 1 requalification (T14 evidence): the REAL
 *     both-mode carrier serializes top-level `representation` as a JSON OBJECT
 *     unconditionally; the canonical D string lives at
 *     `cep_envelope.representations[0].inline_content`. `canonicalJson(top-level
 *     representation object)` is byte-identical to that string on the real
 *     capture, and both digest rules recompute. `parseSxrSlice0bEnvelopeOutput`
 *     below parses that REAL shape (plus the string carrier and CAS carriage);
 *     T14's `parseProducerEnvelopeOutput` stays fail-closed on the object
 *     carrier — this adapter is the qualified SXR-side path.
 *   - CAS carriage captured for real: a model whose D exceeds the 64 KiB
 *     inline threshold emits `content_ref: cas://sha256:…` with NO
 *     `inline_content`; the producer-carried representation object verifies
 *     against the content address (self_hash excluded from its own digest).
 *   - A REAL Relation-declaration fixture capture exists (declared Roles +
 *     Relation with subject/predicate/object/via, model-valid). T14's gap 3 is
 *     closed for the relation kind; no local fixture emits a `role_binding`
 *     declaration, which stays a recorded omission.
 *
 * Core invariants:
 *   - The refresh boundary (REQ-SXR-003): refresh executes ONLY through
 *     `executeSxrRefresh` — an async, promise-returning call over a process
 *     runner port, never from render. Argv is bound as an ARRAY from a CLOSED
 *     read-only command vocabulary (`ledger export`, `ledger verify`,
 *     `query evidence`); no ledger-appending or run-launching command exists
 *     in the vocabulary, so the workbench cannot implicitly append to SXR's
 *     canonical ledger or launch its autonomous run. Every command carries a
 *     side-effect classification (projection rebuild / storage initialization)
 *     recorded BEFORE invocation. Deadline and output bounds are enforced by
 *     the runner; a hung or flooded producer is killed and its partial output
 *     is NEVER parsed.
 *   - Ledger binding (REQ-SXR-001): every lens record carries its ledger
 *     ordinal and entry hash; both are typed identities, not strings.
 *   - committed/pending is a typed distinction. The ledger export contains
 *     only committed records; a `durability` of `spooled`/`queued_for_commit`
 *     anywhere is PENDING, and pending evidence can never be offered as
 *     settled or admitted (prereg P1).
 *   - SXR settlement is source-typed (`SxrSettlementStatus`) and never
 *     substitutes for Gauntlet admission: the only route to Gauntlet evidence
 *     is `admitSxrEvidenceOffer` consuming a `CanonicalEvidenceAdmissionDecision`
 *     authored by the canonical Gauntlet evidence APIs — the same
 *     decision-as-input discipline as evidence-promotion.ts (T11).
 *   - Absence is explicit: a lost/failed/unavailable source renders a typed
 *     absent state labelled unavailable — never recreated, never faked
 *     (REQ-SYNC-003, prereg P2).
 */

import {
  canonicalJson,
  parseSemanticEnvelopeDocument,
  type QualifiedSemanticModel,
} from "./domainforge-adapter";
import type { FocusRef } from "./focus";
import type { InspectorSection } from "./inspector";
import type { RenderedLine, RenderedRecord } from "./render-tree";
import { anchorPath } from "./terminal-session";

// ---------------------------------------------------------------------------
// Pinned producer contract
// ---------------------------------------------------------------------------

/** The only SXR output envelope version this adapter admits (output.rs). */
export const PINNED_SXR_OUTPUT_VERSION = "1";

/** The SXR binary version the qualification captured (`sxr --version`). */
export const PINNED_SXR_VERSION = "0.1.0";

/**
 * Local qualification record for the binaries the real composition used
 * (see module docs). Digests and paths are recorded evidence, not test
 * assertions: peer/CI machines may not carry these builds.
 */
export interface SxrBinaryQualification {
  readonly role: "sxr consumer" | "domainforge producer" | "domainforge installed";
  readonly binary: string;
  readonly binary_sha256: string;
  readonly version: string;
  readonly answers_slice0b_invocation: boolean | null;
  readonly locally_verified: boolean;
}

export const QUALIFIED_SXR_BUILDS: readonly SxrBinaryQualification[] = Object.freeze([
  {
    role: "sxr consumer",
    binary: "sxr/target/debug/sxr",
    binary_sha256: "89c006fbea677a59e96a58cadc3ac3a2ba06ca8e06d785a6cff05102fb0d7e86",
    version: "0.1.0",
    answers_slice0b_invocation: null,
    locally_verified: true,
  },
  {
    role: "domainforge producer",
    binary: "domainforge-slice0b/target/debug/domainforge (SXR default discovery)",
    binary_sha256: "f0ca9defa0bc10444d71d2914f203c4286d89308db69167e1bfdda579d251a42",
    version: "0.16.0",
    answers_slice0b_invocation: true,
    locally_verified: true,
  },
  {
    role: "domainforge installed",
    binary: "~/.cargo/bin/domainforge (installed)",
    binary_sha256: "ee3ee077e2dcf0a38278fa878e1fcebed1d2cc7d22b3f0031ef51a292b775ad4",
    version: "0.16.0",
    answers_slice0b_invocation: false,
    locally_verified: true,
  },
]);

/**
 * The live-qualification record for this milestone (prereg A3/R1): WHAT was
 * exercised on this host, against pinned capture digests. On hosts without
 * the pinned binaries the gated live tests stay visible skips — they keep the
 * milestone open honestly instead of faking composition (representation
 * revision trigger R1 is NOT triggered on this host: qualification succeeded).
 */
export const SXR_LIVE_QUALIFICATION = Object.freeze({
  qualified_on_host: true,
  qualified_at: "2026-09-09",
  lifecycle_records_captured: 13,
  captures_manifest: "evidence/T14B/CAPTURES.sha256",
  negative_matrix_captured: Object.freeze([
    "corrupt-free negative rows verified by shape",
    "exit-3 F-19b pending refusal (17-evidence-refused-pending.err)",
    "unenrolled-repo refusal (query evidence, storage-init side effect recorded)",
  ]),
  both_mode_object_carrier_qualified: true,
  cas_content_ref_carriage_qualified: true,
  relation_declaration_fixture_real: true,
  role_binding_declaration_fixture_real: false,
});

/**
 * The pinned ledger record-kind vocabulary (sxr-core record_kinds.csv, the
 * §5.3b registry as shipped by the pinned binary). Rows carrying kinds outside
 * this vocabulary are PRESERVED and marked `known: false` — never dropped,
 * never interpreted (unknowns travel; the registry is the producer's contract).
 */
export const PINNED_SXR_RECORD_KINDS: readonly string[] = Object.freeze([
  "repo.enrolled",
  "repo.relocated",
  "repo.forgotten",
  "semantic.snapshot",
  "semantic.diff",
  "semantic.identity_drift",
  "run.started",
  "run.finished",
  "verification.contract_imported",
  "plan.created",
  "question.declared",
  "threshold.declared",
  "claim.made",
  "representation.recorded",
  "observation.recorded",
  "evidence.created",
  "settlement.registered",
  "debt.recorded",
  "residual.computed",
  "diagnostic.hypothesis",
  "diagnostic.settled",
  "proposal.raised",
  "conformance.claimed",
  "ingest.rejected",
  "ledger.generation_started",
  "ledger.destructive_recovery",
]);

// ---------------------------------------------------------------------------
// Typed SXR identity domains (REQ-SXR-001: explicit binds, never label joins)
// ---------------------------------------------------------------------------

const SxrEntityRefBrand = Symbol("sxr-adapter.SxrEntityRef");
const SxrOrdinalBrand = Symbol("sxr-adapter.SxrOrdinal");
const SxrEntryHashBrand = Symbol("sxr-adapter.SxrEntryHash");

/**
 * The exact SXR entity ref (`concept:<uuid>`, `question:<id>`, `run:<ulid>`,
 * `evidence:<ulid>`, `snapshot:sha256:…`, …) — the source identity explicit
 * bindings must use. A label or name is not an identity and never joins.
 */
export type SxrEntityRef = {
  readonly [SxrEntityRefBrand]: "SxrEntityRef";
  readonly value: string;
};

/** Ledger append ordinal (§10.6): dense, 1-based, generation-scoped. */
export type SxrOrdinal = {
  readonly [SxrOrdinalBrand]: "SxrOrdinal";
  readonly value: number;
};

/** The sha256 entry hash binding one ledger row (`sha256:<hex>`). */
export type SxrEntryHash = {
  readonly [SxrEntryHashBrand]: "SxrEntryHash";
  readonly value: string;
};

const sxrEntityRefs = new WeakSet<SxrEntityRef>();
const sxrOrdinals = new WeakSet<SxrOrdinal>();
const sxrEntryHashes = new WeakSet<SxrEntryHash>();

const SHA256_PATTERN = /^sha256:[0-9a-f]{64}$/;

export class SxrIdentityError extends Error {
  constructor(detail: string) {
    super(`sxr identity refused: ${detail}`);
    this.name = "SxrIdentityError";
  }
}

/** Mint an SXR entity ref from an exact `kind:id` ref string. */
export function sxrEntityRef(ref: string): SxrEntityRef {
  if (typeof ref !== "string" || !/^[a-z_][a-z0-9_]*:[^\s]+$/.test(ref)) {
    throw new SxrIdentityError(`'${ref.slice(0, 64)}' is not a ` + "`kind:id` entity ref");
  }
  const minted: SxrEntityRef = { [SxrEntityRefBrand]: "SxrEntityRef", value: ref };
  sxrEntityRefs.add(minted);
  return minted;
}

/** Mint a ledger ordinal from a positive integer. */
export function sxrOrdinal(ordinal: number): SxrOrdinal {
  if (typeof ordinal !== "number" || !Number.isInteger(ordinal) || ordinal < 1) {
    throw new SxrIdentityError(`ledger ordinal ${String(ordinal)} is not a positive integer`);
  }
  const minted: SxrOrdinal = { [SxrOrdinalBrand]: "SxrOrdinal", value: ordinal };
  sxrOrdinals.add(minted);
  return minted;
}

/** Mint an entry hash from a `sha256:<hex>` digest. */
export function sxrEntryHash(hash: string): SxrEntryHash {
  if (typeof hash !== "string" || !SHA256_PATTERN.test(hash)) {
    throw new SxrIdentityError(`entry hash '${hash.slice(0, 24)}…' is not a sha256:<hex> digest`);
  }
  const minted: SxrEntryHash = { [SxrEntryHashBrand]: "SxrEntryHash", value: hash };
  sxrEntryHashes.add(minted);
  return minted;
}

function assertSxrDomain<T extends object>(registry: WeakSet<T>, value: T, domain: string): void {
  if (!registry.has(value)) {
    throw new SxrIdentityError(`value was not minted as a ${domain} (forged or foreign identity)`);
  }
}

/** Require a genuine SXR entity ref at runtime (the binding chokepoint). */
export function requireSxrEntityRef(value: SxrEntityRef): string {
  assertSxrDomain(sxrEntityRefs, value, "SXR entity ref");
  return value.value;
}

/** Require a genuine ledger ordinal at runtime. */
export function requireSxrOrdinal(value: SxrOrdinal): number {
  assertSxrDomain(sxrOrdinals, value, "SXR ledger ordinal");
  return value.value;
}

/** Require a genuine entry hash at runtime. */
export function requireSxrEntryHash(value: SxrEntryHash): string {
  assertSxrDomain(sxrEntryHashes, value, "SXR entry hash");
  return value.value;
}

/** The source id every SXR lens FocusRef carries (one adapter, one source). */
export const SXR_SOURCE_ID = "sxr";

// ---------------------------------------------------------------------------
// Typed parse results — versioned command outputs
// ---------------------------------------------------------------------------

/**
 * The §11.1a durability vocabulary, typed. `committed` records live in the
 * ledger; `spooled` and `queued_for_commit` are PENDING (transient custody)
 * and can never back settled or admissible evidence.
 */
export type SxrDurability = "committed" | "spooled" | "queued_for_commit";

export function isPendingDurability(durability: SxrDurability): boolean {
  return durability !== "committed";
}

/** Why a document was refused as a known OTHER contract. */
export type SxrIncompatibleReason =
  | "output_version_mismatch"
  | "cep_envelope_not_sxr_output"
  | "verification_contract_payload";

/** A strict schema violation of the pinned versioned-output contract. */
export class SxrOutputSchemaError extends Error {
  readonly path: string;
  constructor(path: string, detail: string) {
    super(`sxr output schema violation at ${path}: ${detail}`);
    this.name = "SxrOutputSchemaError";
    this.path = path;
  }
}

export type SxrCommandOutputParse =
  | {
      readonly status: "parsed";
      /** The whole raw payload document (lossless). */
      readonly payload_raw: Readonly<Record<string, unknown>>;
      readonly output_version: string;
      /** The §11.1a durability channel; undefined when the output carries none — never defaulted. */
      readonly durability: SxrDurability | undefined;
      /** Mandatory §17.2 channels, preserved verbatim. */
      readonly unknowns: readonly unknown[];
      readonly omissions: readonly unknown[];
      /** `ledger_position` when the output carries one (bound, typed). */
      readonly ledger_position:
        | { readonly ledger_id: string; readonly ordinal: SxrOrdinal }
        | undefined;
      /** Whether `durability`/`ledger_position` channels were present at all. */
      readonly durability_present: boolean;
    }
  | {
      readonly status: "invalid";
      readonly reason: string;
      readonly detail: string;
      readonly declared_output_version: string | undefined;
    }
  | {
      readonly status: "incompatible";
      readonly reason: SxrIncompatibleReason;
      readonly detail: string;
      readonly declared_output_version: string | undefined;
    }
  | { readonly status: "unknown"; readonly detail: string };

function asObject(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SxrOutputSchemaError(path, "expected an object");
  }
  return value as Record<string, unknown>;
}

function requireString(value: Record<string, unknown>, key: string, path: string): string {
  const field = value[key];
  if (typeof field !== "string") {
    throw new SxrOutputSchemaError(`${path}.${key}`, "expected a string");
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
    throw new SxrOutputSchemaError(`${path}.${key}`, "expected an array");
  }
  return field;
}

/**
 * Classify untrusted stdout BEFORE pinning it to the versioned-output schema:
 * a CEP-0008 envelope (domainforge's own output, including the Slice 0B
 * both-mode carrier with its top-level `cep_envelope` key) is a recognizable
 * OTHER contract — typed incompatible, never silently unwrapped.
 */
function classifySxrOuterDocument(record: Record<string, unknown>): "sxr-output" | "other" {
  if ("cep_version" in record || "envelope_kind" in record || "cep_envelope" in record) {
    return "other";
  }
  if ("verification_contract" in record) {
    return "other";
  }
  return "sxr-output";
}

/**
 * Parse one versioned SXR `--json` command output (the `sxr_output_version`
 * envelope of sxr-cli src/output.rs). Strict: mandatory `sxr_output_version`
 * (pinned "1"), `unknowns`, `omissions`; the durability channel and ledger
 * position are typed when present. Unknown producer shapes never parse.
 */
export function parseSxrCommandOutput(raw: string | Uint8Array): SxrCommandOutputParse {
  let parsed: unknown;
  try {
    parsed = JSON.parse(typeof raw === "string" ? raw : new TextDecoder().decode(raw));
  } catch (error) {
    return {
      status: "invalid",
      reason: "malformed_json",
      detail: `sxr output is not JSON: ${String(error)}`,
      declared_output_version: undefined,
    };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {
      status: "invalid",
      reason: "not_an_object",
      detail:
        "sxr command output is not a JSON object (the raw ledger export is an ARRAY; " +
        "use parseSxrLedgerExport for it)",
      declared_output_version: undefined,
    };
  }
  const record = parsed as Record<string, unknown>;
  if (classifySxrOuterDocument(record) === "other") {
    return {
      status: "incompatible",
      reason: "cep_envelope_not_sxr_output",
      detail:
        "output is a CEP-0008 envelope (or verification contract), not an " +
        "`sxr_output_version` command output",
      declared_output_version: undefined,
    };
  }
  if (!("sxr_output_version" in record)) {
    return {
      status: "unknown",
      detail:
        "output carries no `sxr_output_version`; this adapter pins the versioned " +
        "producer envelope and refuses name-shaped guesses",
    };
  }
  const declaredVersion =
    typeof record["sxr_output_version"] === "string" ? record["sxr_output_version"] : undefined;
  if (declaredVersion !== PINNED_SXR_OUTPUT_VERSION) {
    return {
      status: "incompatible",
      reason: "output_version_mismatch",
      detail:
        `declared sxr_output_version '${String(record["sxr_output_version"])}' is not the ` +
        `pinned '${PINNED_SXR_OUTPUT_VERSION}'`,
      declared_output_version: declaredVersion,
    };
  }
  try {
    const unknowns = requireArray(record, "unknowns", "$");
    const omissions = requireArray(record, "omissions", "$");
    let durability: SxrDurability | undefined;
    const durabilityPresent = "durability" in record;
    if (durabilityPresent) {
      const rawDurability = requireString(record, "durability", "$");
      if (
        rawDurability !== "committed" &&
        rawDurability !== "spooled" &&
        rawDurability !== "queued_for_commit"
      ) {
        throw new SxrOutputSchemaError("$.durability", `unknown durability '${rawDurability}'`);
      }
      durability = rawDurability;
    }
    let ledgerPosition: { readonly ledger_id: string; readonly ordinal: SxrOrdinal } | undefined;
    if ("ledger_position" in record) {
      const positionRecord = asObject(record["ledger_position"], "$.ledger_position");
      const positionKeys = Object.keys(positionRecord);
      for (const key of positionKeys) {
        if (key !== "ledger_id" && key !== "append_ordinal") {
          throw new SxrOutputSchemaError(
            "$.ledger_position",
            `unknown field '${key}' (pinned schema is closed)`,
          );
        }
      }
      ledgerPosition = {
        ledger_id: requireString(positionRecord, "ledger_id", "$.ledger_position"),
        ordinal: sxrOrdinal(positionRecord["append_ordinal"] as number),
      };
      if (
        typeof positionRecord["append_ordinal"] !== "number" ||
        !Number.isInteger(positionRecord["append_ordinal"]) ||
        (positionRecord["append_ordinal"] as number) < 1
      ) {
        throw new SxrOutputSchemaError(
          "$.ledger_position.append_ordinal",
          "expected a positive integer",
        );
      }
    }
    return {
      status: "parsed",
      payload_raw: record,
      output_version: declaredVersion,
      durability,
      unknowns,
      omissions,
      ledger_position: ledgerPosition,
      durability_present: durabilityPresent,
    };
  } catch (error) {
    if (error instanceof SxrOutputSchemaError) {
      return {
        status: "invalid",
        reason: "schema_violation",
        detail: error.message,
        declared_output_version: declaredVersion,
      };
    }
    if (error instanceof SxrIdentityError) {
      return {
        status: "invalid",
        reason: "schema_violation",
        detail: error.message,
        declared_output_version: declaredVersion,
      };
    }
    throw error;
  }
}

// ---------------------------------------------------------------------------
// Typed parse results — the RAW ledger export array (parsed SEPARATELY)
// ---------------------------------------------------------------------------

export interface SxrLedgerRow {
  readonly ledger_id: string;
  readonly ordinal: SxrOrdinal;
  readonly record_kind: string;
  /** False for kinds outside the pinned §5.3b vocabulary (preserved, uninterpreted). */
  readonly kind_known: boolean;
  readonly entry_hash: SxrEntryHash;
  /** The payload object verbatim (the export re-parses the stored payload). */
  readonly payload: Readonly<Record<string, unknown>>;
  /** Subject refs when the row carries them (may be absent from the export). */
  readonly subject_refs: readonly string[];
  /** Fields the export does NOT carry — recorded per row as typed omissions. */
  readonly export_omissions: readonly string[];
}

export type SxrLedgerExportParse =
  | {
      readonly status: "parsed";
      readonly rows: readonly SxrLedgerRow[];
      readonly ledger_ids: readonly string[];
      /**
       * Ordinal density holds (1..n contiguous, per ledger id in export order)
       * — the part of the §10.6 chain the export can check. Hash LINKAGE is
       * not checkable: the export omits previous_entry_hash/payload_hash; full
       * verification is producer-side (`sxr ledger verify`).
       */
      readonly ordinal_density_ok: boolean;
    }
  | {
      readonly status: "invalid";
      readonly reason: string;
      readonly detail: string;
    }
  | {
      readonly status: "incompatible";
      readonly reason: SxrIncompatibleReason;
      readonly detail: string;
    }
  | { readonly status: "unknown"; readonly detail: string };

/** The fields a ledger export row does not carry (recorded as typed omissions). */
const LEDGER_EXPORT_OMITTED_FIELDS: readonly string[] = Object.freeze([
  "entry_ulid",
  "record_version",
  "idempotency_key",
  "canonicalization",
  "hash_algorithm",
  "payload_hash",
  "previous_entry_hash",
  "committed_at",
  "writer_identity_ref",
]);

/**
 * Parse the RAW `sxr ledger export --repo <id>` stdout — a BARE JSON ARRAY of
 * ledger rows, deliberately NOT the versioned output envelope (the plan pins
 * the two parses separately). Row shapes are strict; kinds outside the pinned
 * vocabulary are preserved with `kind_known: false`; ordinals are typed and
 * their density checked; entry hashes are format-verified. Chain linkage is a
 * recorded omission — `sxr ledger verify` owns it.
 */
export function parseSxrLedgerExport(raw: string | Uint8Array): SxrLedgerExportParse {
  let parsed: unknown;
  try {
    parsed = JSON.parse(typeof raw === "string" ? raw : new TextDecoder().decode(raw));
  } catch (error) {
    return {
      status: "invalid",
      reason: "malformed_json",
      detail: `ledger export is not JSON: ${String(error)}`,
    };
  }
  if (!Array.isArray(parsed)) {
    if (typeof parsed === "object" && parsed !== null && "sxr_output_version" in parsed) {
      return {
        status: "incompatible",
        reason: "cep_envelope_not_sxr_output",
        detail:
          "got a versioned command output where the raw ledger export array was expected; " +
          "the two parses are separate contracts",
      };
    }
    return {
      status: "unknown",
      detail: "ledger export is not a JSON array; this adapter pins the raw export shape",
    };
  }
  const rows: SxrLedgerRow[] = [];
  try {
    for (const [index, item] of parsed.entries()) {
      const record = asObject(item, `$[${index}]`);
      const allowed = ["append_ordinal", "record_kind", "ledger_id", "entry_hash", "payload"];
      for (const key of Object.keys(record)) {
        if (!allowed.includes(key)) {
          throw new SxrOutputSchemaError(`$[${index}]`, `unknown field '${key}'`);
        }
      }
      const ordinalValue = record["append_ordinal"];
      if (typeof ordinalValue !== "number" || !Number.isInteger(ordinalValue) || ordinalValue < 1) {
        throw new SxrOutputSchemaError(`$[${index}].append_ordinal`, "expected a positive integer");
      }
      const entryHashValue = requireString(record, "entry_hash", `$[${index}]`);
      if (!SHA256_PATTERN.test(entryHashValue)) {
        throw new SxrOutputSchemaError(`$[${index}].entry_hash`, "expected a sha256:<hex> digest");
      }
      const payload = asObject(record["payload"], `$[${index}].payload`);
      const kind = requireString(record, "record_kind", `$[${index}]`);
      const subjectRefs = payload["subject_refs"];
      rows.push({
        ledger_id: requireString(record, "ledger_id", `$[${index}]`),
        ordinal: sxrOrdinal(ordinalValue),
        record_kind: kind,
        kind_known: (PINNED_SXR_RECORD_KINDS as readonly string[]).includes(kind),
        entry_hash: sxrEntryHash(entryHashValue),
        payload,
        subject_refs:
          Array.isArray(subjectRefs) && subjectRefs.every((s) => typeof s === "string")
            ? (subjectRefs as readonly string[])
            : [],
        export_omissions: LEDGER_EXPORT_OMITTED_FIELDS,
      });
    }
  } catch (error) {
    if (error instanceof SxrOutputSchemaError) {
      return { status: "invalid", reason: "schema_violation", detail: error.message };
    }
    if (error instanceof SxrIdentityError) {
      return { status: "invalid", reason: "schema_violation", detail: error.message };
    }
    throw error;
  }
  const ledgerIds = [...new Set(rows.map((row) => row.ledger_id))];
  const ordinalDensityOk = ledgerIds.every((id) => {
    const ordinals = rows
      .filter((row) => row.ledger_id === id)
      .map((row) => row.ordinal.value)
      .sort((a, b) => a - b);
    return ordinals.every((ordinal, position) => ordinal === position + 1);
  });
  return {
    status: "parsed",
    rows,
    ledger_ids: ledgerIds,
    ordinal_density_ok: ordinalDensityOk,
  };
}

// ---------------------------------------------------------------------------
// DEC-005 requalification — the SXR Slice 0B both-mode carrier (REAL shapes)
// ---------------------------------------------------------------------------

/**
 * A content-addressed CAS store view (§10.7a): objects keyed by the bare hex
 * digest. `read` returns the stored bytes or undefined — missing objects stay
 * typed missing, never silently substituted.
 */
export interface SxrCasStore {
  readonly read: (hexDigest: string) => Uint8Array | undefined;
}

/** Why the producer envelope output was refused (SXR-side parsing). */
export type SxrProducerIncompatibleReason =
  | "schema_version_mismatch"
  | "cep_envelope_not_sxr_output"
  | "verification_contract_payload";

/** Why CAS-carried representation materialization failed (fail-closed). */
export type SxrCasMaterializeFailure =
  | "content_ref_malformed"
  | "content_ref_scheme_unsupported"
  | "content_ref_hash_mismatch"
  | "content_expired"
  | "no_carried_representation_and_no_cas_store"
  | "cas_object_missing"
  | "cas_object_corrupt"
  | "cas_object_hash_mismatch";

export type SxrProducerEnvelopeParse =
  | {
      readonly status: "parsed";
      /** T14's strict qualified model over the extracted canonical D. */
      readonly model: QualifiedSemanticModel;
      /** How D was carried in this output. */
      readonly carriage:
        | { readonly mode: "inline_content_string" }
        | { readonly mode: "top_level_object"; readonly canonical_match: boolean }
        | { readonly mode: "cas_content_ref"; readonly source: "producer" | "cas_store" };
      /** Preserved producer diagnostics — non-empty forces model_valid false. */
      readonly diagnostics: readonly string[];
    }
  | {
      readonly status: "invalid";
      readonly reason:
        | "malformed_json"
        | "not_an_object"
        | "representation_missing"
        | "representation_neither_string_nor_object"
        | "cas_materialization_failed"
        | "self_hash_mismatch"
        | "semantic_closure_hash_mismatch"
        | "schema_violation";
      readonly detail: string;
      readonly cas_failure: SxrCasMaterializeFailure | undefined;
      readonly declared_schema_version: string | undefined;
    }
  | {
      readonly status: "incompatible";
      readonly reason: SxrProducerIncompatibleReason;
      readonly detail: string;
      readonly declared_schema_version: string | undefined;
    }
  | { readonly status: "unknown"; readonly detail: string };

interface Rep0View {
  readonly content_hash: string | undefined;
  readonly semantic_hash: string | undefined;
  readonly inline_content: unknown;
  readonly content_ref: unknown;
  readonly content_availability: unknown;
}

function rep0View(envelope: Record<string, unknown>): Rep0View | undefined {
  const representations = envelope["representations"];
  if (!Array.isArray(representations) || representations.length === 0) {
    return undefined;
  }
  const rep0 = representations[0];
  if (typeof rep0 !== "object" || rep0 === null) {
    return undefined;
  }
  const record = rep0 as Record<string, unknown>;
  const str = (key: string): string | undefined =>
    typeof record[key] === "string" ? (record[key] as string) : undefined;
  return {
    content_hash: str("content_hash"),
    semantic_hash: str("semantic_hash"),
    inline_content: record["inline_content"],
    content_ref: record["content_ref"],
    content_availability: record["content_availability"],
  };
}

function sha256Hex(text: string): string {
  return `sha256:${new Bun.CryptoHasher("sha256").update(text).digest("hex")}`;
}

/**
 * Verify the producer-carried representation (top-level object form) against
 * the representation's declared content hash: sha256 over canonical JSON with
 * self_hash excluded from its own digest (the producer's
 * envelope_document_self_hash rule — verified against the real capture).
 */
function carriedRepresentationHash(repObject: Record<string, unknown>): string {
  const selfless: Record<string, unknown> = {};
  for (const key of Object.keys(repObject)) {
    if (key !== "self_hash") {
      selfless[key] = repObject[key];
    }
  }
  return sha256Hex(canonicalJson(selfless));
}

/**
 * Materialize CAS-carried D (§10.7a / sxr-df adapter.rs discipline, SXR-side):
 * the producer's own top-level representation first (verified against the
 * content address), the CAS store second (verified again after read). Missing,
 * expired, or hash-mismatched content is a TYPED failure — never a fallback to
 * different bytes.
 */
function materializeCasCarried(
  rep: Rep0View,
  representation: Record<string, unknown> | undefined,
  cas: SxrCasStore | undefined,
): { readonly d: string; readonly source: "producer" | "cas_store" } | SxrCasMaterializeFailure {
  const contentRef = rep.content_ref;
  if (typeof contentRef !== "string" || !contentRef.includes("://")) {
    return "content_ref_malformed";
  }
  const [scheme, refHash] = contentRef.split("://", 2) as [string, string];
  if (scheme !== "cas") {
    return "content_ref_scheme_unsupported";
  }
  if (rep.content_hash === undefined || refHash !== rep.content_hash) {
    return "content_ref_hash_mismatch";
  }
  if (rep.content_availability === "expired") {
    return "content_expired";
  }
  const bareHex = rep.content_hash.startsWith("sha256:")
    ? rep.content_hash.slice("sha256:".length)
    : rep.content_hash;
  if (representation !== undefined) {
    const computed = carriedRepresentationHash(representation);
    if (computed !== rep.content_hash) {
      return "content_ref_hash_mismatch";
    }
    return { d: canonicalJson(representation), source: "producer" };
  }
  if (cas === undefined) {
    return "no_carried_representation_and_no_cas_store";
  }
  const bytes = cas.read(bareHex);
  if (bytes === undefined) {
    return "cas_object_missing";
  }
  let document: unknown;
  try {
    document = JSON.parse(new TextDecoder().decode(bytes));
  } catch {
    return "cas_object_corrupt";
  }
  if (typeof document !== "object" || document === null || Array.isArray(document)) {
    return "cas_object_corrupt";
  }
  const computed = carriedRepresentationHash(document as Record<string, unknown>);
  if (computed !== rep.content_hash) {
    return "cas_object_hash_mismatch";
  }
  return { d: canonicalJson(document), source: "cas_store" };
}

/**
 * Parse the SXR Slice 0B producer's `envelope --emit both <entry>` stdout over
 * the REAL capture shapes (DEC-005 Correction Round 1 requalification):
 *
 *   - REAL working carrier: `{"representation": <D as OBJECT>,
 *     "cep_envelope": <E>}` — the object carrier T14's parser types invalid.
 *     D is taken from `cep_envelope.representations[0].inline_content` (the
 *     canonical string) and cross-checked against the top-level object via
 *     canonical JSON; a mismatch is typed invalid.
 *   - CAS carriage: when representations[0] carries `content_ref` and no
 *     `inline_content`, D is materialized per `materializeCasCarried`.
 *   - String carrier (`"representation": "<canonical D>"`): the release-build/
 *     synthesized shape — accepted, disclosed as NOT what the current working
 *     build emits unconditionally.
 *   - Failure envelope (bare CEP wrapper, no D): typed — the diagnostics are
 *     the failure boundary's payload, and nothing conformant is fabricated.
 *
 * The extracted canonical D is parsed by T14's strict
 * `parseSemanticEnvelopeDocument` (reuse, not duplication); diagnostics force
 * `model_valid: false` exactly as REQ-DOM-004 requires.
 */
export function parseSxrSlice0bEnvelopeOutput(
  raw: string | Uint8Array,
  cas?: SxrCasStore | undefined,
): SxrProducerEnvelopeParse {
  let parsed: unknown;
  try {
    parsed = JSON.parse(typeof raw === "string" ? raw : new TextDecoder().decode(raw));
  } catch (error) {
    return {
      status: "invalid",
      reason: "malformed_json",
      detail: `producer output is not JSON: ${String(error)}`,
      cas_failure: undefined,
      declared_schema_version: undefined,
    };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {
      status: "invalid",
      reason: "not_an_object",
      detail: "producer output is not a JSON object",
      cas_failure: undefined,
      declared_schema_version: undefined,
    };
  }
  const record = parsed as Record<string, unknown>;
  const envelopeValue = record["cep_envelope"];
  if (envelopeValue === undefined) {
    if ("verification_contract" in record) {
      return {
        status: "incompatible",
        reason: "verification_contract_payload",
        detail: "output is a verification contract, not an envelope carrier",
        declared_schema_version: undefined,
      };
    }
    // Bare D or bare CEP wrapper: delegate to the strict pinned parser, which
    // types the CEP wrapper `incompatible` and the bare document per its own
    // contract.
    const inner = parseSemanticEnvelopeDocument(raw);
    if (inner.status === "parsed") {
      return {
        status: "parsed",
        model: inner.model,
        carriage: { mode: "inline_content_string" },
        diagnostics: [],
      };
    }
    if (inner.status === "invalid") {
      return {
        status: "invalid",
        reason:
          inner.reason === "self_hash_mismatch" || inner.reason === "semantic_closure_hash_mismatch"
            ? inner.reason
            : "schema_violation",
        detail: inner.detail,
        cas_failure: undefined,
        declared_schema_version: inner.declared_schema_version,
      };
    }
    if (inner.status === "incompatible") {
      return {
        status: "incompatible",
        reason:
          inner.reason === "schema_version_mismatch"
            ? "schema_version_mismatch"
            : "cep_envelope_not_sxr_output",
        detail: inner.detail,
        declared_schema_version: inner.declared_schema_version,
      };
    }
    return { status: "unknown", detail: inner.detail };
  }
  if (typeof envelopeValue !== "object" || envelopeValue === null || Array.isArray(envelopeValue)) {
    return {
      status: "incompatible",
      reason: "cep_envelope_not_sxr_output",
      detail: "cep_envelope is not an object",
      declared_schema_version: undefined,
    };
  }
  const envelope = envelopeValue as Record<string, unknown>;
  const rep = rep0View(envelope);
  if (rep === undefined) {
    // Failure envelope: D was never constructed; the diagnostics ARE the
    // result. Nothing conformant is fabricated from a failure boundary.
    const diagnostics = extractProducerDiagnostics(envelope);
    return {
      status: "invalid",
      reason: "representation_missing",
      detail:
        diagnostics.length > 0
          ? `producer failure envelope carries ${diagnostics.length} diagnostic(s) and no D`
          : "producer envelope carries no representations; no D was constructed",
      cas_failure: undefined,
      declared_schema_version: undefined,
    };
  }
  const representationValue = record["representation"];
  const representationObject =
    typeof representationValue === "object" &&
    representationValue !== null &&
    !Array.isArray(representationValue)
      ? (representationValue as Record<string, unknown>)
      : undefined;
  let canonicalD: string;
  let carriage:
    | { readonly mode: "inline_content_string" }
    | {
        readonly mode: "top_level_object";
        readonly canonical_match: boolean;
      }
    | { readonly mode: "cas_content_ref"; readonly source: "producer" | "cas_store" };
  if (typeof rep.inline_content === "string") {
    canonicalD = rep.inline_content;
    carriage = { mode: "inline_content_string" };
  } else if (rep.content_ref !== undefined) {
    const materialized = materializeCasCarried(rep, representationObject, cas);
    if (typeof materialized === "string") {
      return {
        status: "invalid",
        reason: "cas_materialization_failed",
        detail: `CAS-carried representation could not be materialized: ${materialized}`,
        cas_failure: materialized,
        declared_schema_version: undefined,
      };
    }
    canonicalD = materialized.d;
    carriage = { mode: "cas_content_ref", source: materialized.source };
  } else if (representationObject !== undefined) {
    // The REAL working carrier: D rides as the top-level object. The canonical
    // string is absent, so the object's canonical JSON IS the D bytes — verify
    // its binding before use.
    const computed = carriedRepresentationHash(representationObject);
    if (rep.content_hash !== undefined && computed !== rep.content_hash) {
      return {
        status: "invalid",
        reason: "self_hash_mismatch",
        detail: `top-level representation object hashes ${computed} != declared content_hash ${rep.content_hash}`,
        cas_failure: undefined,
        declared_schema_version: undefined,
      };
    }
    canonicalD = canonicalJson(representationObject);
    carriage = { mode: "top_level_object", canonical_match: true };
  } else {
    return {
      status: "invalid",
      reason: "representation_missing",
      detail:
        "envelope carries no inline_content, no content_ref, and no producer representation object",
      cas_failure: undefined,
      declared_schema_version: undefined,
    };
  }
  // Cross-check the object carrier against the canonical string: on the real
  // capture they are byte-identical; a producer that lets them diverge is a
  // contract drift this adapter must not paper over.
  if (
    carriage.mode === "inline_content_string" &&
    representationObject !== undefined &&
    carriedRepresentationHash(representationObject) !== rep.content_hash
  ) {
    return {
      status: "invalid",
      reason: "self_hash_mismatch",
      detail: "top-level representation object does not match the declared content_hash",
      cas_failure: undefined,
      declared_schema_version: undefined,
    };
  }
  const inner = parseSemanticEnvelopeDocument(canonicalD);
  if (inner.status === "parsed") {
    const diagnostics = extractProducerDiagnostics(envelope);
    const declaredStatus = producerDeclaredModelStatus(envelope);
    const modelInvalid = diagnostics.length > 0 || declaredStatus === "invalid";
    const model = modelInvalid
      ? {
          document: inner.model.document,
          document_raw: inner.model.document_raw,
          model_id: inner.model.model_id,
          fingerprint: inner.model.fingerprint,
          model_valid: false,
          diagnostics,
        }
      : inner.model;
    return { status: "parsed", model, carriage, diagnostics };
  }
  if (inner.status === "invalid") {
    return {
      status: "invalid",
      reason:
        inner.reason === "self_hash_mismatch" || inner.reason === "semantic_closure_hash_mismatch"
          ? inner.reason
          : "schema_violation",
      detail: inner.detail,
      cas_failure: undefined,
      declared_schema_version: inner.declared_schema_version,
    };
  }
  if (inner.status === "incompatible") {
    return {
      status: "incompatible",
      reason:
        inner.reason === "schema_version_mismatch"
          ? "schema_version_mismatch"
          : "cep_envelope_not_sxr_output",
      detail: inner.detail,
      declared_schema_version: inner.declared_schema_version,
    };
  }
  return { status: "unknown", detail: inner.detail };
}

function extractProducerDiagnostics(envelope: Record<string, unknown>): string[] {
  const extensions = envelope["extensions"];
  if (typeof extensions !== "object" || extensions === null) {
    return [];
  }
  const domainforge = (extensions as Record<string, unknown>)["domainforge"];
  if (typeof domainforge !== "object" || domainforge === null) {
    return [];
  }
  const raw = (domainforge as Record<string, unknown>)["diagnostics"];
  return Array.isArray(raw) ? raw.filter((item): item is string => typeof item === "string") : [];
}

function producerDeclaredModelStatus(envelope: Record<string, unknown>): string | undefined {
  const extensions = envelope["extensions"];
  if (typeof extensions !== "object" || extensions === null) {
    return undefined;
  }
  const domainforge = (extensions as Record<string, unknown>)["domainforge"];
  if (typeof domainforge !== "object" || domainforge === null) {
    return undefined;
  }
  const status = (domainforge as Record<string, unknown>)["model_validation_status"];
  return typeof status === "string" ? status : undefined;
}

// ---------------------------------------------------------------------------
// Bounded refresh contract (REQ-SXR-003)
// ---------------------------------------------------------------------------

/**
 * The CLOSED read-only refresh vocabulary. Nothing that appends to SXR's
 * canonical ledger, launches its autonomous run, or mutates enrolled state
 * exists here — writes are not refused-at-runtime, they are ABSENT from the
 * vocabulary (REQ-SXR-003: the workbench must neither append implicitly nor
 * launch autonomously).
 */
export type SxrRefreshCommand =
  | { readonly kind: "ledger_export" }
  | { readonly kind: "ledger_verify" }
  | { readonly kind: "query_evidence"; readonly entity: SxrEntityRef };

/** Side-effect classification, recorded BEFORE invocation (REQ-SXR-003). */
export interface SxrSideEffectClass {
  /** SXR `query`/`export` rebuild projections under the writer lock. */
  readonly rebuilds_projections: boolean;
  /** Opening a repo's ledger CREATES `$XDG_DATA_HOME/sxr/repos/<id>/ledger.db` when missing. */
  readonly initializes_local_storage: boolean;
  /** Appends to SXR's canonical ledger — impossible for this vocabulary. */
  readonly appends_canonical_ledger: false;
  /** Launches SXR's autonomous run — impossible for this vocabulary. */
  readonly launches_autonomous_run: false;
}

export const SXR_SIDE_EFFECT_CLASSES: Readonly<
  Record<SxrRefreshCommand["kind"], SxrSideEffectClass>
> = Object.freeze({
  ledger_export: Object.freeze({
    rebuilds_projections: false,
    initializes_local_storage: true,
    appends_canonical_ledger: false as const,
    launches_autonomous_run: false as const,
  }),
  ledger_verify: Object.freeze({
    rebuilds_projections: false,
    initializes_local_storage: true,
    appends_canonical_ledger: false as const,
    launches_autonomous_run: false as const,
  }),
  query_evidence: Object.freeze({
    rebuilds_projections: true,
    initializes_local_storage: true,
    appends_canonical_ledger: false as const,
    launches_autonomous_run: false as const,
  }),
});

/** Why a refresh request was refused at the boundary (closed vocabulary). */
export type SxrRefreshRefusalCode =
  | "program_not_absolute"
  | "program_metacharacter"
  | "repo_id_invalid"
  | "entity_ref_invalid"
  | "xdg_root_not_absolute"
  | "deadline_not_positive"
  | "output_cap_not_positive"
  | "command_not_in_contract";

export class SxrRefreshBoundaryError extends Error {
  readonly code: SxrRefreshRefusalCode;
  constructor(code: SxrRefreshRefusalCode, detail: string) {
    super(`sxr refresh boundary refusal (${code}): ${detail}`);
    this.name = "SxrRefreshBoundaryError";
    this.code = code;
  }
}

const REPO_ID_PATTERN = /^[0-9A-Za-z_-]{1,64}$/;

/** Defaults: 10 s deadline, 32 MiB output cap (the biggest pinned export is ~1 MB). */
export const SXR_REFRESH_DEFAULT_DEADLINE_MS = 10_000;
export const SXR_REFRESH_DEFAULT_MAX_OUTPUT_BYTES = 32 * 1024 * 1024;

export interface SxrRefreshRequestInput {
  /** Absolute path of the sxr binary (the pinned qualification record's role). */
  readonly binary_path: string;
  /** The selected SXR repository id (explicit — never discovered by label). */
  readonly repo_id: string;
  readonly command: SxrRefreshCommand;
  /** Selected workspace roots for the child's XDG state/data (explicit isolation). */
  readonly xdg_data_home?: string | undefined;
  readonly xdg_state_home?: string | undefined;
  /** Deadline in milliseconds (default 10_000). */
  readonly deadline_ms?: number | undefined;
  /** Output cap in bytes (default 32 MiB). */
  readonly max_output_bytes?: number | undefined;
}

/** A bound refresh request — the ONLY shape a runner may be handed. */
export interface SxrBoundRefresh {
  readonly program: string;
  readonly args: readonly string[];
  readonly env: Readonly<Record<string, string>>;
  readonly repo_id: string;
  readonly command: SxrRefreshCommand;
  readonly side_effects: SxrSideEffectClass;
  readonly deadline_ms: number;
  readonly max_output_bytes: number;
}

const PROGRAM_METACHARACTERS = new Set([
  ";",
  "|",
  "&",
  "$",
  "(",
  ")",
  "<",
  ">",
  "`",
  '"',
  "'",
  "*",
  "?",
  "~",
  "!",
  "{",
  "}",
  "[",
  "]",
  "\n",
  "\r",
  "\0",
  " ",
]);

function anchorOptionalPath(value: string | undefined): string | undefined {
  if (value === undefined) {
    return undefined;
  }
  return anchorPath(value, ["/"]);
}

/**
 * Validate + bind a refresh request at the privileged boundary: the program is
 * an absolute, metacharacter-free path; the repo id is format-checked; entity
 * refs are minted identities; XDG roots are anchored absolute paths; argv is
 * built ONLY from the closed vocabulary. Either a bound request or a typed
 * refusal — never a partially-bound spawn.
 */
export function validateSxrRefreshRequest(input: SxrRefreshRequestInput): SxrBoundRefresh {
  const program = input.binary_path;
  if (program.length === 0) {
    throw new SxrRefreshBoundaryError("program_not_absolute", "binary_path is required");
  }
  if (!program.startsWith("/")) {
    throw new SxrRefreshBoundaryError(
      "program_not_absolute",
      `binary_path must be absolute, got '${program.slice(0, 64)}'`,
    );
  }
  for (const ch of program) {
    if (PROGRAM_METACHARACTERS.has(ch)) {
      throw new SxrRefreshBoundaryError(
        "program_metacharacter",
        `binary_path contains metacharacter ${JSON.stringify(ch)} — spawn binds argv, never a shell string`,
      );
    }
  }
  const anchoredProgram = anchorPath(program, ["/"]);
  if (!REPO_ID_PATTERN.test(input.repo_id)) {
    throw new SxrRefreshBoundaryError(
      "repo_id_invalid",
      `repo id '${input.repo_id.slice(0, 64)}' is not a ULID-shaped repository identity`,
    );
  }
  if (input.command.kind === "query_evidence") {
    requireSxrEntityRef(input.command.entity);
  }
  const dataHome = anchorOptionalPath(input.xdg_data_home);
  const stateHome = anchorOptionalPath(input.xdg_state_home);
  const env: Record<string, string> = {
    PATH: "/usr/bin:/bin",
  };
  // Absent XDG variables are OMITTED entirely: an empty XDG_DATA_HOME means
  // unset per the XDG spec, which would silently redirect the child to
  // $HOME/.local/share (MINOR-2, Correction Round 1).
  if (dataHome !== undefined) {
    env["XDG_DATA_HOME"] = dataHome;
  }
  if (stateHome !== undefined) {
    env["XDG_STATE_HOME"] = stateHome;
  }
  const deadline_ms = input.deadline_ms ?? SXR_REFRESH_DEFAULT_DEADLINE_MS;
  if (!(Number.isInteger(deadline_ms) && deadline_ms > 0)) {
    throw new SxrRefreshBoundaryError(
      "deadline_not_positive",
      "deadline_ms must be a positive integer",
    );
  }
  const max_output_bytes = input.max_output_bytes ?? SXR_REFRESH_DEFAULT_MAX_OUTPUT_BYTES;
  if (!(Number.isInteger(max_output_bytes) && max_output_bytes > 0)) {
    throw new SxrRefreshBoundaryError(
      "output_cap_not_positive",
      "max_output_bytes must be a positive integer",
    );
  }
  // Argv from the CLOSED vocabulary only — data joins as argv ELEMENTS.
  const args: string[] = [];
  switch (input.command.kind) {
    case "ledger_export":
      args.push("ledger", "export", "--repo", input.repo_id);
      break;
    case "ledger_verify":
      args.push("ledger", "verify", "--repo", input.repo_id);
      break;
    case "query_evidence":
      args.push(
        "query",
        "evidence",
        "--repo",
        input.repo_id,
        "--entity",
        requireSxrEntityRef(input.command.entity),
      );
      break;
    default: {
      // Runtime-forged command shapes land here (the type system cannot be
      // trusted at this boundary): a typed refusal BEFORE runner invocation —
      // never a silent empty-argv spawn (MINOR-3, Correction Round 1).
      const kind = (input.command as { readonly kind?: unknown }).kind;
      throw new SxrRefreshBoundaryError(
        "command_not_in_contract",
        `command kind ${JSON.stringify(kind ?? null)} is not in the closed read-only refresh vocabulary`,
      );
    }
  }
  return Object.freeze({
    program: anchoredProgram,
    args: Object.freeze(args),
    env: Object.freeze(env),
    repo_id: input.repo_id,
    command: input.command,
    side_effects: SXR_SIDE_EFFECT_CLASSES[input.command.kind],
    deadline_ms,
    max_output_bytes,
  });
}

// --- Runner port -------------------------------------------------------------

/** Monotonic-enough wall clock port (injectable for tests). */
export interface SxrClock {
  readonly now: () => number;
}

export const systemSxrClock: SxrClock = Object.freeze({ now: () => Date.now() });

export type SxrProcessResult =
  | {
      readonly class: "exited";
      readonly exit_code: number | null;
      readonly signal: string | null;
      readonly stdout_bytes: Uint8Array;
      readonly stderr_text: string;
    }
  | {
      readonly class: "deadline_exceeded";
      readonly stdout_bytes: Uint8Array;
      readonly stderr_text: string;
    }
  | {
      readonly class: "output_limit_exceeded";
      readonly stdout_bytes: Uint8Array;
      readonly stderr_text: string;
    }
  | { readonly class: "spawn_failed"; readonly detail: string };

/**
 * The process-isolation seam: the ONLY component that may spawn for a refresh.
 * Implementations MUST enforce the deadline and output cap themselves (they
 * own the child) and kill the child when a bound trips.
 */
export interface SxrProcessRunner {
  readonly kind: string;
  run(bound: SxrBoundRefresh, clock: SxrClock): Promise<SxrProcessResult>;
}

/**
 * Real Bun pipe runner (Bun.spawn, the T03-qualified spawn shape). Drains
 * stderr under its own cap, enforces the deadline by polling `proc.exited`
 * against the injected clock, and kills (SIGTERM, then SIGKILL) on trip.
 */
export function bunSxrProcessRunner(poll_interval_ms = 25): SxrProcessRunner {
  return {
    kind: "bun-pipe",
    async run(bound: SxrBoundRefresh, clock: SxrClock): Promise<SxrProcessResult> {
      let proc: Bun.Subprocess<"ignore", "pipe", "pipe">;
      try {
        proc = Bun.spawn([bound.program, ...bound.args], {
          cwd: "/",
          env: { ...bound.env },
          stdin: "ignore",
          stdout: "pipe",
          stderr: "pipe",
        });
      } catch (error) {
        return { class: "spawn_failed", detail: String(error) };
      }
      const stdout: Uint8Array[] = [];
      let stdout_bytes = 0;
      let overflow = false;
      const pump = (async (): Promise<void> => {
        const reader = proc.stdout.getReader();
        for (;;) {
          const { done, value } = await reader.read();
          if (done) {
            break;
          }
          if (value) {
            stdout_bytes += value.byteLength;
            if (stdout_bytes > bound.max_output_bytes) {
              overflow = true;
              return;
            }
            stdout.push(value);
          }
        }
      })();
      const stderrDrain = (async (): Promise<string> => {
        try {
          const text = await new Response(proc.stderr).text();
          return text.length > 64 * 1024 ? text.slice(0, 64 * 1024) : text;
        } catch {
          return "";
        }
      })();
      const deadline_at = clock.now() + bound.deadline_ms;
      let timedOut = false;
      for (;;) {
        const raced = await Promise.race([
          proc.exited.then((code): number | null => code ?? -1),
          new Promise<"poll">((resolve) => setTimeout(resolve, poll_interval_ms, "poll")),
        ]);
        if (raced !== "poll") {
          break;
        }
        if (overflow) {
          void proc.kill("SIGKILL");
          await proc.exited.catch(() => undefined);
          await pump.catch(() => undefined);
          return {
            class: "output_limit_exceeded",
            stdout_bytes: concatChunks(stdout),
            stderr_text: await stderrDrain,
          };
        }
        if (clock.now() >= deadline_at) {
          timedOut = true;
          proc.kill();
          const killer = setTimeout(() => {
            try {
              proc.kill("SIGKILL");
            } catch {
              // already gone
            }
          }, 250);
          void killer.unref?.();
          await proc.exited.catch(() => undefined);
          await pump.catch(() => undefined);
          return {
            class: "deadline_exceeded",
            stdout_bytes: concatChunks(stdout),
            stderr_text: await stderrDrain,
          };
        }
      }
      if (overflow) {
        return {
          class: "output_limit_exceeded",
          stdout_bytes: concatChunks(stdout),
          stderr_text: await stderrDrain,
        };
      }
      void timedOut;
      await pump.catch(() => undefined);
      const exit_code = await proc.exited;
      return {
        class: "exited",
        exit_code,
        signal: proc.signalCode,
        stdout_bytes: concatChunks(stdout),
        stderr_text: await stderrDrain,
      };
    },
  };
}

function concatChunks(chunks: readonly Uint8Array[]): Uint8Array {
  const total = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out;
}

// --- The refresh entrypoint -----------------------------------------------------

export type SxrRefreshOutcome =
  | {
      /** The bounded refresh completed and the output PARSED under its contract. */
      readonly class: "refreshed";
      readonly command: SxrRefreshCommand["kind"];
      readonly side_effects: SxrSideEffectClass;
      readonly exit_code: number | null;
      readonly parse: SxrLedgerExportParse | SxrCommandOutputParse;
      readonly stderr_text: string;
    }
  | {
      /** Non-zero exit with parseable or empty output — recorded, never swallowed. */
      readonly class: "producer_refused";
      readonly exit_code: number | null;
      readonly stderr_text: string;
      readonly detail: string;
    }
  | {
      readonly class: "deadline_exceeded";
      readonly detail: string;
      readonly partial_output_bytes: number;
    }
  | {
      readonly class: "output_limit_exceeded";
      readonly detail: string;
      readonly partial_output_bytes: number;
    }
  | { readonly class: "spawn_failed"; readonly detail: string };

/**
 * Execute one bounded SXR refresh. This is the ONLY refresh path: it is async
 * (a promise — the render loop never calls it; render reads already-refreshed
 * state), spawns through the runner port under deadline/output bounds, and
 * kills a hung or flooded producer with partial output DISCARDED (never
 * parsed — fail-closed against truncated JSON and partial leads).
 */
export async function executeSxrRefresh(
  input: SxrRefreshRequestInput,
  runner: SxrProcessRunner,
  clock: SxrClock = systemSxrClock,
): Promise<SxrRefreshOutcome> {
  const bound = validateSxrRefreshRequest(input);
  const result = await runner.run(bound, clock);
  switch (result.class) {
    case "spawn_failed":
      return { class: "spawn_failed", detail: result.detail };
    case "deadline_exceeded":
      return {
        class: "deadline_exceeded",
        detail: `sxr producer exceeded the ${bound.deadline_ms}ms deadline and was killed`,
        partial_output_bytes: result.stdout_bytes.byteLength,
      };
    case "output_limit_exceeded":
      return {
        class: "output_limit_exceeded",
        detail: `sxr producer exceeded the ${bound.max_output_bytes}-byte output bound and was killed`,
        partial_output_bytes: result.stdout_bytes.byteLength,
      };
    case "exited": {
      const decoder = new TextDecoder();
      const stdout_text = decoder.decode(result.stdout_bytes);
      if (result.exit_code !== 0) {
        return {
          class: "producer_refused",
          exit_code: result.exit_code,
          stderr_text: result.stderr_text,
          detail:
            result.stderr_text.trim().length > 0
              ? result.stderr_text.trim().slice(0, 512)
              : `sxr exited ${String(result.exit_code)} with no stderr`,
        };
      }
      const parse =
        bound.command.kind === "ledger_export"
          ? parseSxrLedgerExport(stdout_text)
          : parseSxrCommandOutput(stdout_text);
      return {
        class: "refreshed",
        command: bound.command.kind,
        side_effects: bound.side_effects,
        exit_code: result.exit_code,
        parse,
        stderr_text: result.stderr_text,
      };
    }
  }
}

// ---------------------------------------------------------------------------
// Evidence lens (REQ-SXR-001/002): records, identity, and the canonical path
// ---------------------------------------------------------------------------

/** A parsed lens view of one ledger row's payload fields (typed-optional). */
export interface SxrQuestionRecord {
  readonly row: SxrLedgerRow;
  readonly question_id: string;
  readonly question_form: string | undefined;
  readonly question_type: string | undefined;
  /** The declared spec's target entity ref when the producer recorded one. */
  readonly target_entity_ref: string | undefined;
}

export interface SxrThresholdRecord {
  readonly row: SxrLedgerRow;
  readonly threshold_id: string;
  readonly question_ref: string | undefined;
  readonly threshold_kind: string | undefined;
  readonly evaluation_profile: string | undefined;
}

export interface SxrSnapshotRecord {
  readonly row: SxrLedgerRow;
  readonly semantic_closure_hash: string | undefined;
  readonly content_hash: string | undefined;
  /** The exact declared entity refs the snapshot preserved (declaration identities). */
  readonly preserved_entities: readonly string[];
  /**
   * Declaration NAMES carried by the snapshot's inline D — used ONLY to make
   * binding refusals precise (a label is refused with the no-name-join
   * message); a name is never an identity and never binds.
   */
  readonly declaration_names: readonly string[];
}

export interface SxrRunRecord {
  readonly row: SxrLedgerRow;
  readonly run_id: string | undefined;
  readonly repo_id: string | undefined;
  readonly question_ref: string | undefined;
  readonly threshold_ref: string | undefined;
  readonly snapshot_ref: string | undefined;
}

export interface SxrEvidenceRecord {
  readonly row: SxrLedgerRow;
  readonly evidence_id: string;
  readonly question_ref: string | undefined;
  readonly target_entity_ref: string | undefined;
  readonly direction: string | undefined;
  readonly evidence_type: string | undefined;
  readonly relevance: string | undefined;
  readonly reliability: string | undefined;
}

/**
 * SXR settlement status — a SOURCE-Typed view. This is SXR's settlement state,
 * never Gauntlet's: it must not be interpreted as, or converted into, a
 * Gauntlet Settlement/verification verdict (REQ-SXR-002).
 */
export type SxrSettlementStatus = "settled" | "settled_with_debt" | string;

export interface SxrSettlementRecord {
  readonly row: SxrLedgerRow;
  readonly settlement_id: string;
  readonly question_ref: string | undefined;
  readonly threshold_ref: string | undefined;
  readonly sxr_settlement_status: SxrSettlementStatus;
}

function payloadArrayOfRecords(
  payload: Record<string, unknown>,
  key: string,
): readonly Record<string, unknown>[] {
  const value = payload[key];
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter(
    (item): item is Record<string, unknown> =>
      typeof item === "object" && item !== null && !Array.isArray(item),
  );
}

function firstString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

/**
 * Extract declaration NAMES from a snapshot representation's inline D (best
 * effort over untrusted bytes): `envelope.semantic_declarations[].declaration
 * .data.name` strings. Names feed ONLY binding-refusal messages — they are
 * never identities and never bind (no name-based joins).
 */
function extractDeclarationNames(inlineContent: unknown): readonly string[] {
  if (typeof inlineContent !== "string") {
    return [];
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(inlineContent);
  } catch {
    return [];
  }
  if (typeof parsed !== "object" || parsed === null) {
    return [];
  }
  const envelope = (parsed as Record<string, unknown>)["envelope"];
  if (typeof envelope !== "object" || envelope === null) {
    return [];
  }
  const declarations = (envelope as Record<string, unknown>)["semantic_declarations"];
  if (!Array.isArray(declarations)) {
    return [];
  }
  const names: string[] = [];
  for (const declaration of declarations) {
    if (typeof declaration !== "object" || declaration === null) {
      continue;
    }
    const payload = (declaration as Record<string, unknown>)["declaration"];
    if (typeof payload !== "object" || payload === null) {
      continue;
    }
    const data = (payload as Record<string, unknown>)["data"];
    if (typeof data !== "object" || data === null) {
      continue;
    }
    const name = (data as Record<string, unknown>)["name"];
    if (typeof name === "string") {
      names.push(name);
    }
  }
  return names;
}

function inlineSpec(payload: Record<string, unknown>): Record<string, unknown> | undefined {
  for (const rep of payloadArrayOfRecords(payload, "representations")) {
    const inline = rep["inline_content"];
    if (typeof inline === "object" && inline !== null && !Array.isArray(inline)) {
      return inline as Record<string, unknown>;
    }
  }
  return undefined;
}

/**
 * Project a parsed ledger export into the typed evidence-lens records. Rows
 * with kinds outside the lens's interpretation set are counted and preserved
 * (never dropped, never interpreted); SXR identity/settlement stay
 * source-typed; every record keeps its ledger ordinal + entry hash.
 */
export interface SxrEvidenceLens {
  readonly status: "projected";
  readonly repo_id: string;
  readonly questions: readonly SxrQuestionRecord[];
  readonly thresholds: readonly SxrThresholdRecord[];
  readonly snapshots: readonly SxrSnapshotRecord[];
  readonly runs: readonly SxrRunRecord[];
  readonly evidence: readonly SxrEvidenceRecord[];
  readonly settlements: readonly SxrSettlementRecord[];
  /** Rows the lens does not interpret (count + kinds only — preserved upstream). */
  readonly uninterpreted: readonly { readonly record_kind: string; readonly count: number }[];
  /** Every row, in export order — the lossless base of the lens. */
  readonly rows: readonly SxrLedgerRow[];
}

export type SxrLensProjection =
  | {
      readonly status: "absent";
      readonly reason: "no_ledger_export" | "export_invalid";
      readonly detail: string;
    }
  | SxrEvidenceLens;

export function projectSxrEvidenceLens(
  parse: SxrLedgerExportParse | undefined,
  repoId: string,
): SxrLensProjection {
  if (parse === undefined) {
    return {
      status: "absent",
      reason: "no_ledger_export",
      detail: "no SXR ledger export was provided for this Case",
    };
  }
  if (parse.status !== "parsed") {
    return {
      status: "absent",
      reason: "export_invalid",
      detail:
        parse.status === "invalid"
          ? `${parse.reason}: ${parse.detail}`
          : `export refused (${parse.status})`,
    };
  }
  const questions: SxrQuestionRecord[] = [];
  const thresholds: SxrThresholdRecord[] = [];
  const snapshots: SxrSnapshotRecord[] = [];
  const runs: SxrRunRecord[] = [];
  const evidence: SxrEvidenceRecord[] = [];
  const settlements: SxrSettlementRecord[] = [];
  const uninterpreted = new Map<string, number>();
  for (const row of parse.rows) {
    const payload = row.payload as Record<string, unknown>;
    switch (row.record_kind) {
      case "question.declared": {
        const q = payloadArrayOfRecords(payload, "questions")[0];
        const spec = inlineSpec(payload);
        if (q === undefined) {
          uninterpreted.set(row.record_kind, (uninterpreted.get(row.record_kind) ?? 0) + 1);
          break;
        }
        const questionId = firstString(q["question_id"]);
        if (questionId === undefined) {
          uninterpreted.set(row.record_kind, (uninterpreted.get(row.record_kind) ?? 0) + 1);
          break;
        }
        questions.push({
          row,
          question_id: questionId,
          question_form: firstString(q["question_form"]),
          question_type: firstString(q["question_type"]),
          target_entity_ref: spec === undefined ? undefined : firstString(spec["target_entities"]),
        });
        break;
      }
      case "threshold.declared": {
        const spec = inlineSpec(payload);
        const state = payloadArrayOfRecords(payload, "states")[0];
        const thresholdId =
          spec === undefined
            ? undefined
            : (firstString(spec["threshold_id"]) ?? firstString(state?.["state_id"]));
        if (thresholdId === undefined) {
          uninterpreted.set(row.record_kind, (uninterpreted.get(row.record_kind) ?? 0) + 1);
          break;
        }
        thresholds.push({
          row,
          threshold_id: thresholdId,
          question_ref:
            row.subject_refs.find((ref) => ref.startsWith("question:")) ??
            firstString(state?.["scope"]),
          threshold_kind: spec === undefined ? undefined : firstString(spec["threshold_kind"]),
          evaluation_profile:
            spec === undefined ? undefined : firstString(spec["evaluation_profile"]),
        });
        break;
      }
      case "semantic.snapshot": {
        const representations = payloadArrayOfRecords(payload, "representations");
        const rep0 = representations[0];
        const preserved = rep0?.["preserved_entities"];
        snapshots.push({
          row,
          semantic_closure_hash:
            rep0 === undefined ? undefined : firstString(rep0["semantic_hash"]),
          content_hash: rep0 === undefined ? undefined : firstString(rep0["content_hash"]),
          preserved_entities: Array.isArray(preserved)
            ? preserved.filter((item): item is string => typeof item === "string")
            : [],
          declaration_names: extractDeclarationNames(rep0?.["inline_content"]),
        });
        break;
      }
      case "run.started": {
        const scopeRecord = payload["scope"];
        const scope =
          typeof scopeRecord === "object" && scopeRecord !== null
            ? (scopeRecord as Record<string, unknown>)
            : undefined;
        runs.push({
          row,
          run_id: scope === undefined ? undefined : firstString(scope["run_id"]),
          repo_id: scope === undefined ? undefined : firstString(scope["repo_id"]),
          question_ref: row.subject_refs.find((ref) => ref.startsWith("question:")),
          threshold_ref: row.subject_refs.find((ref) => ref.startsWith("threshold:")),
          snapshot_ref: row.subject_refs.find((ref) => ref.startsWith("snapshot:")),
        });
        break;
      }
      case "evidence.created": {
        const entries = payloadArrayOfRecords(payload, "evidence");
        for (const entry of entries) {
          const evidenceId = firstString(entry["evidence_id"]);
          if (evidenceId === undefined) {
            continue;
          }
          evidence.push({
            row,
            evidence_id: evidenceId,
            question_ref: firstString(entry["question_ref"]),
            target_entity_ref: firstString(entry["target_entity_ref"]),
            direction: firstString(entry["direction"]),
            evidence_type: firstString(entry["evidence_type"]),
            relevance: firstString(entry["relevance"]),
            reliability: firstString(entry["reliability"]),
          });
        }
        break;
      }
      case "settlement.registered": {
        const entries = payloadArrayOfRecords(payload, "settlements");
        for (const entry of entries) {
          const settlementId = firstString(entry["settlement_id"]);
          if (settlementId === undefined) {
            continue;
          }
          settlements.push({
            row,
            settlement_id: settlementId,
            question_ref: firstString(entry["question_ref"]),
            threshold_ref: firstString(entry["threshold_ref"]),
            sxr_settlement_status: firstString(entry["settlement_status"]) ?? "unknown",
          });
        }
        break;
      }
      default:
        uninterpreted.set(row.record_kind, (uninterpreted.get(row.record_kind) ?? 0) + 1);
    }
  }
  return {
    status: "projected",
    repo_id: repoId,
    questions,
    thresholds,
    snapshots,
    runs,
    evidence,
    settlements,
    uninterpreted: [...uninterpreted.entries()].map(([record_kind, count]) => ({
      record_kind,
      count,
    })),
    rows: parse.rows,
  };
}

// --- Explicit Unit bindings (no name joins) --------------------------------------

export class SxrBindingError extends Error {
  constructor(detail: string) {
    super(`sxr unit binding refused: ${detail}`);
    this.name = "SxrBindingError";
  }
}

export interface SxrUnitBindingInput {
  /** The Gauntlet Unit FocusRef this binding attaches to (kind "WorkItem"). */
  readonly unit: FocusRef;
  /** The EXACT SXR entity ref (declaration ref, question id ref, …). */
  readonly entity_ref: string;
  /** Who declared this binding (never inferred by the adapter). */
  readonly declared_by: string;
}

export interface SxrUnitBinding {
  readonly unit: FocusRef;
  readonly entity_ref: SxrEntityRef;
  readonly declared_by: string;
}

/**
 * Admit explicit SXR ↔ Gauntlet Unit bindings (REQ-SXR-001: bind explicitly,
 * never join by label). The entity ref must be a well-formed `kind:id` ref
 * present in the bound lens's declaration/question/evidence identities; a
 * human label is refused with the no-name-join message.
 */
export function admitSxrUnitBindings(
  lens: SxrEvidenceLens,
  inputs: readonly SxrUnitBindingInput[],
): readonly SxrUnitBinding[] {
  const knownRefs = new Set<string>();
  const knownNames = new Set<string>();
  for (const snapshot of lens.snapshots) {
    for (const ref of snapshot.preserved_entities) {
      knownRefs.add(ref);
    }
    for (const name of snapshot.declaration_names) {
      knownNames.add(name);
    }
  }
  for (const question of lens.questions) {
    knownRefs.add(`question:${question.question_id}`);
    if (question.target_entity_ref !== undefined) {
      knownRefs.add(question.target_entity_ref);
    }
  }
  for (const evidence of lens.evidence) {
    knownRefs.add(`evidence:${evidence.evidence_id}`);
    if (evidence.target_entity_ref !== undefined) {
      knownRefs.add(evidence.target_entity_ref);
    }
  }
  return inputs.map((input) => {
    if (input.unit.kind !== "WorkItem") {
      throw new SxrBindingError(
        `binding target must be a WorkItem focus, got kind '${input.unit.kind}'`,
      );
    }
    if (!knownRefs.has(input.entity_ref)) {
      throw new SxrBindingError(
        knownNames.has(input.entity_ref)
          ? `'${input.entity_ref}' is a declaration NAME, not an identity; bind the exact \`kind:id\` entity ref (no name-based joins)`
          : `entity ref '${input.entity_ref}' does not exist in the bound SXR ledger`,
      );
    }
    if (input.declared_by.trim().length === 0) {
      throw new SxrBindingError("declared_by must name the binding authority");
    }
    return Object.freeze({
      unit: input.unit,
      entity_ref: sxrEntityRef(input.entity_ref),
      declared_by: input.declared_by,
    });
  });
}

// --- The canonical verification path (declaration → Unit → question → …) ---------

/** One inspectable step of the canonical verification path (DSN-008 record). */
export interface SxrPathStep {
  readonly stage:
    | "declaration"
    | "unit_binding"
    | "question"
    | "threshold"
    | "evidence"
    | "sxr_settlement";
  readonly identity: string;
  /** Ledger binding: ordinal + entry hash of the record that asserts this step. */
  readonly ledger: { readonly ordinal: number; readonly entry_hash: string } | undefined;
  readonly detail: string;
  /** SXR's own settlement word — never a Gauntlet verification verdict. */
  readonly sxr_settlement_status: SxrSettlementStatus | undefined;
}

export interface SxrVerificationPath {
  readonly declaration_ref: string;
  readonly unit_bindings: readonly SxrUnitBinding[];
  readonly steps: readonly SxrPathStep[];
  /**
   * Whether the recorded question/threshold/evidence chain is COMPLETE for the
   * declaration. Completeness is a lens observation about SXR records — never
   * a Gauntlet verification result.
   */
  readonly chain_complete: boolean;
  readonly missing: readonly string[];
}

/**
 * Build the canonical verification path for one declaration entity ref:
 * declaration → linked Units (explicit bindings) → recorded question →
 * threshold → evidence → SXR settlement — as inspectable records carrying
 * their ledger bindings. Unknowns (a missing threshold, no settlement yet)
 * are explicit `missing` entries, never invented links.
 */
export function sxrVerificationPath(
  lens: SxrEvidenceLens,
  declarationRef: string,
  bindings: readonly SxrUnitBinding[] = [],
): SxrVerificationPath {
  const steps: SxrPathStep[] = [];
  const missing: string[] = [];
  const relevantQuestions = lens.questions.filter(
    (question) =>
      question.target_entity_ref === declarationRef ||
      lens.evidence.some(
        (evidence) =>
          evidence.target_entity_ref === declarationRef &&
          evidence.question_ref === question.question_id,
      ),
  );
  const relevantEvidence = lens.evidence.filter(
    (evidence) => evidence.target_entity_ref === declarationRef,
  );
  for (const question of relevantQuestions) {
    steps.push({
      stage: "question",
      identity: `question:${question.question_id}`,
      ledger: { ordinal: question.row.ordinal.value, entry_hash: question.row.entry_hash.value },
      detail: `question_form=${question.question_form ?? "unknown"} question_type=${question.question_type ?? "unknown"}`,
      sxr_settlement_status: undefined,
    });
    const threshold = lens.thresholds.find(
      (candidate) => candidate.question_ref === `question:${question.question_id}`,
    );
    if (threshold === undefined) {
      missing.push(`threshold for question:${question.question_id}`);
      continue;
    }
    steps.push({
      stage: "threshold",
      identity: `threshold:${threshold.threshold_id}`,
      ledger: { ordinal: threshold.row.ordinal.value, entry_hash: threshold.row.entry_hash.value },
      detail: `kind=${threshold.threshold_kind ?? "unknown"} profile=${threshold.evaluation_profile ?? "unknown"}`,
      sxr_settlement_status: undefined,
    });
    const questionEvidence = relevantEvidence.filter(
      (evidence) => evidence.question_ref === question.question_id,
    );
    for (const evidence of questionEvidence) {
      steps.push({
        stage: "evidence",
        identity: `evidence:${evidence.evidence_id}`,
        ledger: { ordinal: evidence.row.ordinal.value, entry_hash: evidence.row.entry_hash.value },
        detail: `${evidence.evidence_type ?? "unknown"} ${evidence.direction ?? "unknown"} (relevance=${evidence.relevance ?? "unknown"}, reliability=${evidence.reliability ?? "unknown"})`,
        sxr_settlement_status: undefined,
      });
    }
    if (questionEvidence.length === 0) {
      missing.push(`evidence for question:${question.question_id} on ${declarationRef}`);
    }
    const settlement = lens.settlements.find(
      (candidate) => candidate.question_ref === question.question_id,
    );
    if (settlement === undefined) {
      missing.push(`settlement for question:${question.question_id}`);
    } else {
      steps.push({
        stage: "sxr_settlement",
        identity: `settlement:${settlement.settlement_id}`,
        ledger: {
          ordinal: settlement.row.ordinal.value,
          entry_hash: settlement.row.entry_hash.value,
        },
        detail: `SXR-sourced settlement state (threshold:${settlement.threshold_ref ?? "unknown"}) — distinct from any Gauntlet verdict`,
        sxr_settlement_status: settlement.sxr_settlement_status,
      });
    }
  }
  if (relevantQuestions.length === 0) {
    missing.push(`question targeting ${declarationRef}`);
  }
  const unitBindings = bindings.filter((binding) => binding.entity_ref.value === declarationRef);
  for (const binding of unitBindings) {
    steps.push({
      stage: "unit_binding",
      identity: binding.unit.id,
      ledger: undefined,
      detail: `declared binding by ${binding.declared_by}`,
      sxr_settlement_status: undefined,
    });
  }
  const declarationStep: SxrPathStep = {
    stage: "declaration",
    identity: declarationRef,
    ledger: undefined,
    detail: lens.snapshots.some((snapshot) => snapshot.preserved_entities.includes(declarationRef))
      ? "declared entity preserved by a recorded snapshot"
      : "declaration identity not found in recorded snapshots",
    sxr_settlement_status: undefined,
  };
  const snapshot = lens.snapshots.find((candidate) =>
    candidate.preserved_entities.includes(declarationRef),
  );
  const boundDeclarationStep: SxrPathStep =
    snapshot === undefined
      ? declarationStep
      : {
          ...declarationStep,
          ledger: {
            ordinal: snapshot.row.ordinal.value,
            entry_hash: snapshot.row.entry_hash.value,
          },
        };
  return {
    declaration_ref: declarationRef,
    unit_bindings: unitBindings,
    steps: [boundDeclarationStep, ...steps],
    chain_complete: missing.length === 0 && steps.length > 1,
    missing,
  };
}

// ---------------------------------------------------------------------------
// Admission boundary (REQ-SXR-002): pending can NEVER pass; the canonical
// decision is an INPUT authored by Gauntlet's evidence APIs — no factory here.
// ---------------------------------------------------------------------------

export type SxrEvidenceOfferRefusal =
  | "pending_evidence"
  | "evidence_not_in_qualified_ledger"
  | "repository_mismatch"
  | "settlement_is_sxr_only";

export type SxrEvidenceOfferOutcome =
  | { readonly class: "offered"; readonly offer: SxrEvidenceOffer }
  | {
      readonly class: "refused";
      readonly reason: SxrEvidenceOfferRefusal;
      readonly message: string;
    };

/**
 * The SXR→Gauntlet evidence offer. Like T11's kernel-object offer, the
 * establishes_* flags are LITERAL false at the type level: an offer is not
 * evidence and can never settle or verify anything.
 */
export interface SxrEvidenceOffer {
  readonly offer_id: string;
  readonly source: { readonly source_id: string; readonly repo_id: string };
  /** The lens record the offer is built from (committed + chain-shape-checked). */
  readonly evidence: SxrEvidenceRecord;
  readonly target: { readonly unit_id: string; readonly source_id: string };
  readonly admission_contract: {
    readonly evidence_id: string;
    readonly unit_id: string;
    readonly kind: string;
    /** Durable locator: the SXR ledger position of the evidence record. */
    readonly locator: string;
    readonly source_id: string;
  };
  readonly establishes_evidence: false;
  readonly establishes_verification: false;
  readonly establishes_settlement: false;
  readonly admission_route: "canonical_gauntlet_evidence_apis";
  readonly verified_source: "canonical_verification_only";
}

let offerCounter = 0;

/**
 * Offer one SXR evidence record as CANDIDATE Gauntlet evidence. Refused unless
 * the record is (a) from the qualified export's committed ledger, (b) not
 * pending (ledger records are committed by construction; any pending
 * durability observed anywhere refuses the offer), and (c) offered for the
 * same repo it was exported from. SXR settlements are never importable as
 * Gauntlet settlements — the settlement channel refuses by construction.
 */
export function offerSxrEvidenceAsGauntletEvidence(input: {
  readonly lens: SxrEvidenceLens;
  readonly export_parse: SxrLedgerExportParse & { status: "parsed" };
  readonly evidence_id: string;
  readonly unit_id: string;
  readonly target_source_id: string;
  readonly pending_durability_observed?: boolean | undefined;
}): SxrEvidenceOfferOutcome {
  if (input.pending_durability_observed === true) {
    return {
      class: "refused",
      reason: "pending_evidence",
      message:
        "an SXR record for this Case is pending (spooled or queued_for_commit); pending evidence can never be offered as settled or admitted (F-19b/AC-DAEMON-003)",
    };
  }
  if (input.lens.status !== "projected") {
    return {
      class: "refused",
      reason: "evidence_not_in_qualified_ledger",
      message: "the lens is not projected; no offer can be built",
    };
  }
  // MAJOR-1 (independent confirmation, Correction Round 1): the lens repo
  // must agree with the export's LEDGER provenance, independent of run rows.
  // A non-empty export whose ledger ids never name the lens repo is a
  // swapped/fabricated repository, and the offered record's own row must be
  // a ledger record of that repo — refused either way, never offered (the
  // locator carries the true ledger id, so the disagreement would be
  // visible; it must never get that far).
  const exportLedgerIds = new Set(input.export_parse.rows.map((row) => row.ledger_id));
  if (input.export_parse.rows.length > 0 && !exportLedgerIds.has(input.lens.repo_id)) {
    return {
      class: "refused",
      reason: "repository_mismatch",
      message:
        `the export's ledger ids (${[...exportLedgerIds].join(", ")}) never name the ` +
        `lens repository '${input.lens.repo_id}'; offering across repositories is refused`,
    };
  }
  const record = input.lens.evidence.find(
    (candidate) => candidate.evidence_id === input.evidence_id,
  );
  if (record === undefined) {
    return {
      class: "refused",
      reason: "evidence_not_in_qualified_ledger",
      message: `evidence '${input.evidence_id}' is not a committed record of the qualified ledger export`,
    };
  }
  if (record.row.ledger_id !== input.lens.repo_id) {
    return {
      class: "refused",
      reason: "repository_mismatch",
      message:
        `evidence '${input.evidence_id}' is a record of ledger '${record.row.ledger_id}', ` +
        `not of the lens repository '${input.lens.repo_id}'`,
    };
  }
  const runsMatchingRepo = input.lens.runs.filter((run) => run.repo_id === input.lens.repo_id);
  if (input.lens.runs.length > 0 && runsMatchingRepo.length === 0) {
    return {
      class: "refused",
      reason: "repository_mismatch",
      message: `the lens records runs for another repository; offering across repositories is refused`,
    };
  }
  offerCounter += 1;
  const offer_id = `sxr-evidence-offer:${input.lens.repo_id}:${record.evidence_id}:${offerCounter}`;
  return {
    class: "offered",
    offer: Object.freeze({
      offer_id,
      source: { source_id: SXR_SOURCE_ID, repo_id: input.lens.repo_id },
      evidence: record,
      target: { unit_id: input.unit_id, source_id: input.target_source_id },
      admission_contract: Object.freeze({
        evidence_id: `sxr:${input.lens.repo_id}:${record.evidence_id}`,
        unit_id: input.unit_id,
        kind: `sxr_${record.evidence_type ?? "evidence"}`,
        locator: `sxr-ledger:${record.row.ledger_id}#${record.row.ordinal.value}:${record.row.entry_hash.value}`,
        source_id: input.target_source_id,
      }),
      establishes_evidence: false as const,
      establishes_verification: false as const,
      establishes_settlement: false as const,
      admission_route: "canonical_gauntlet_evidence_apis" as const,
      verified_source: "canonical_verification_only" as const,
    }),
  };
}

/** Refusal codes the canonical side may return (open vocabulary — its contract). */
export type CanonicalSxrEvidenceDecision =
  | {
      readonly class: "admitted";
      readonly evidence_id: string;
      /** Canonical verification's own answer — never derived from SXR state. */
      readonly verified: boolean;
      readonly source_cursor: string;
    }
  | {
      readonly class: "refused";
      readonly evidence_id: string;
      readonly code: string;
      readonly message: string;
    };

export type SxrEvidenceAdmissionOutcome =
  | {
      readonly class: "admitted";
      readonly record: {
        readonly evidence_id: string;
        readonly source_id: string;
        readonly source_cursor: string;
        readonly unit_id: string;
        readonly kind: string;
        readonly locator: string;
        readonly verified: boolean;
      };
    }
  | {
      readonly class: "refused_canonically";
      readonly evidence_id: string;
      readonly code: string;
      readonly message: string;
    }
  | {
      readonly class: "refused";
      readonly reason: "decision_mismatch" | "already_admitted";
      readonly message: string;
    };

/**
 * The SXR evidence admission gate: validates that a canonical decision exists
 * for this exact offer and records the admitted truth verbatim (including a
 * canonical `verified: false`). The decision is an INPUT — no function here
 * can author one from SXR state, so SXR settlement authority cannot leak into
 * Gauntlet evidence (REQ-SXR-002).
 */
export class SxrEvidenceAdmissionGate {
  private readonly admitted = new Map<string, CanonicalSxrEvidenceDecision>();

  admit(
    offer: SxrEvidenceOffer,
    decision: CanonicalSxrEvidenceDecision,
  ): SxrEvidenceAdmissionOutcome {
    if (decision.class === "refused") {
      return {
        class: "refused_canonically",
        evidence_id: decision.evidence_id,
        code: decision.code,
        message: decision.message,
      };
    }
    if (decision.evidence_id !== offer.admission_contract.evidence_id) {
      return {
        class: "refused",
        reason: "decision_mismatch",
        message: `canonical decision names evidence '${decision.evidence_id}', not this offer's '${offer.admission_contract.evidence_id}'`,
      };
    }
    if (this.admitted.has(offer.offer_id)) {
      return {
        class: "refused",
        reason: "already_admitted",
        message: `offer '${offer.offer_id}' was already admitted; admission is one-way and never re-derived`,
      };
    }
    this.admitted.set(offer.offer_id, decision);
    return {
      class: "admitted",
      record: {
        evidence_id: offer.admission_contract.evidence_id,
        source_id: offer.admission_contract.source_id,
        source_cursor: decision.source_cursor,
        unit_id: offer.admission_contract.unit_id,
        kind: offer.admission_contract.kind,
        locator: offer.admission_contract.locator,
        verified: decision.verified,
      },
    };
  }

  wasAdmitted(offerId: string): boolean {
    return this.admitted.has(offerId);
  }
}

/**
 * Build the authoritative `evidence_added` frame carrying a canonical SXR
 * admission into the projection store — the ONLY writer path for evidence
 * rows. Refuses to fabricate a frame from a non-admitted decision.
 */
export function authoritativeSxrEvidenceFrame(
  offer: SxrEvidenceOffer,
  decision: CanonicalSxrEvidenceDecision,
  projection: { generation_id: string; timestamp_ms: number },
): {
  readonly generation_id: string;
  readonly source_id: string;
  readonly cursor: string;
  readonly event_type: "evidence_added";
  readonly payload: Record<string, unknown>;
  readonly timestamp_ms: number;
} {
  if (decision.class !== "admitted") {
    throw new Error(
      `cannot build an authoritative evidence frame from a ${decision.class} decision; admission is the only route to an evidence row`,
    );
  }
  if (decision.evidence_id !== offer.admission_contract.evidence_id) {
    throw new Error(
      `canonical decision names evidence '${decision.evidence_id}', not this offer's '${offer.admission_contract.evidence_id}'`,
    );
  }
  return Object.freeze({
    generation_id: projection.generation_id,
    source_id: offer.admission_contract.source_id,
    cursor: decision.source_cursor,
    event_type: "evidence_added" as const,
    payload: {
      evidence_id: offer.admission_contract.evidence_id,
      unit_id: offer.admission_contract.unit_id,
      kind: offer.admission_contract.kind,
      locator: offer.admission_contract.locator,
      verified: decision.verified,
    },
    timestamp_ms: projection.timestamp_ms,
  });
}

// ---------------------------------------------------------------------------
// Inspector lens rendering (existing RenderedLine/RenderedRecord contract)
// ---------------------------------------------------------------------------

/**
 * Render the SXR evidence lens state as an Inspector section. Absent state
 * renders an explicit ABSENT line (labelled unavailable, never recreated —
 * REQ-SYNC-003); projected state renders the per-declaration canonical
 * verification path as inspectable records carrying ledger bindings, with the
 * SXR settlement channel visibly source-typed.
 */
export function renderSxrEvidenceSection(
  projection: SxrLensProjection,
  paths: readonly SxrVerificationPath[] = [],
): InspectorSection {
  if (projection.status === "absent") {
    return {
      id: "sxr-evidence",
      title: "REALITY TRACE",
      lines: [
        {
          spans: [
            { text: "reality trace ", emphasis: "dim" },
            { text: `ABSENT (${projection.reason})`, token: "structure.neutral" },
            { text: ` — ${projection.detail}`, emphasis: "dim" },
          ],
        },
        {
          spans: [
            {
              text: "source output is unavailable; recorded state is not recreated",
              emphasis: "dim",
            },
          ],
        },
      ],
      records: [],
    };
  }
  const lines: RenderedLine[] = [
    {
      spans: [
        { text: "repo        ".padEnd(13), emphasis: "dim" },
        { text: projection.repo_id, emphasis: "bold" },
        { text: ` · ${projection.rows.length} committed ledger record(s)`, emphasis: "dim" },
      ],
    },
    {
      spans: [
        { text: "records     ".padEnd(13), emphasis: "dim" },
        {
          text: `${projection.questions.length} question(s) · ${projection.thresholds.length} threshold(s) · ${projection.evidence.length} evidence · ${projection.settlements.length} SXR settlement(s)`,
        },
      ],
    },
  ];
  for (const entry of projection.uninterpreted) {
    lines.push({
      spans: [
        { text: "unparsed    ".padEnd(13), emphasis: "dim" },
        {
          text: `${entry.count} × ${entry.record_kind} (preserved, not interpreted)`,
          token: "integrity.partial",
        },
      ],
    });
  }
  const records: RenderedRecord[] = [];
  for (const path of paths) {
    for (const step of path.steps) {
      const base: {
        readonly id: string;
        readonly kind: RenderedRecord["kind"];
        readonly fields: Readonly<Record<string, string>>;
      } = {
        id: `sxr-path:${path.declaration_ref}:${step.stage}:${step.identity}`,
        kind:
          step.stage === "evidence"
            ? "evidence"
            : step.stage === "sxr_settlement"
              ? "authority"
              : "relation",
        fields: {
          stage: step.stage,
          identity: step.identity,
          detail: step.detail,
          ledger:
            step.ledger === undefined
              ? "unavailable (declared binding, not a ledger record)"
              : `ordinal ${String(step.ledger.ordinal)} · ${step.ledger.entry_hash}`,
          sxr_settlement: step.sxr_settlement_status ?? "unavailable",
          chain: path.chain_complete
            ? "complete"
            : `missing: ${path.missing.join("; ") || "unknown"}`,
        },
      };
      records.push(
        step.sxr_settlement_status !== undefined
          ? { ...base, token: "integrity.partial" as const }
          : base,
      );
    }
  }
  return { id: "sxr-evidence", title: "REALITY TRACE", lines, records };
}
