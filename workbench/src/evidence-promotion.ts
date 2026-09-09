/**
 * Explicit kernel-object promotion and evidence admission (plan T11,
 * REQ-PRIME-007, REQ-SYNC-003, REQ-OUT-004; KernelObjectRef entity semantics).
 *
 * Kernel values are COMPUTATION STATE. This module implements the ONLY explicit
 * transitions a kernel object can take — pin, artifactize, attach-to-Focus,
 * offer-as-evidence — each carrying source/session/generation provenance
 * (KernelObjectRef: id, session_ref, name, kernel_generation,
 * persistence_class, optional artifact_ref/focus_refs):
 *
 *   - Kernel objects MUST NOT become Evidence merely because they exist
 *     (KernelObjectRef rule). Registration marks computation state and nothing
 *     else; no projection may read a kernel value as evidence.
 *   - Offer-as-evidence REQUIRES prior artifactization to a durable locator
 *     (REQ-SYNC-003: semantic/source bytes used for evidence MUST be pinned in
 *     the owning durable mechanism before use; a live query result alone is not
 *     durable evidence).
 *   - An `EvidenceOffer` is an offer, never evidence: `establishes_evidence`,
 *     `establishes_verification`, and `establishes_settlement` are LITERAL
 *     `false` at the type level, so no code path can construct an offer that
 *     settles or verifies (the T10 provenance-record pattern).
 *   - Admission is the only route to evidence and it is authored exclusively by
 *     the canonical Gauntlet evidence APIs: a
 *     `CanonicalEvidenceAdmissionDecision` is an INPUT to this module — no
 *     function here produces one from a kernel object or kernel value. The
 *     admitted record follows the canonical evidence projection schema
 *     (`projection-store.ts` `EvidenceProjection`), and its `verified` bit is
 *     the canonical decision's answer, never kernel state — a kernel variable
 *     named `verified` contributes nothing to verification or settlement.
 *   - The canonical→projection bridge (`authoritativeEvidenceFrame`) emits the
 *     store's authoritative `evidence_added` frame; the store's own
 *     authoritative event path remains the only writer of evidence rows.
 *   - Name reuse after a kernel restart never revives an old reference: every
 *     record carries its `kernel_generation`, and a ledger refuses objects of
 *     another generation.
 *
 * All identifiers are minted from deterministic per-process counters (no wall
 * clock in identity), so tests are deterministic in-memory.
 */

import type { FocusRef } from "./focus";
import type { PrimeReplKernel, ReplExecuteResult } from "./prime-kernel";
import type { AuthoritativeEventFrame } from "./projection-store";

// --- KernelObjectRef semantics (spec §core_domain_model) ------------------------

/** Spec persistence classes (KernelObjectRef.persistence_class). */
export type KernelPersistenceClass = "ephemeral" | "pinned" | "artifactized";

/**
 * The provenance EVERY promoted kernel object carries: the KernelObjectRef
 * required fields over T05's runtime object. `kernel_generation` is the owning
 * runtime incarnation — reuse of a name after restart MUST NOT revive the old
 * reference.
 */
export interface KernelObjectProvenance {
  /** T05 runtime object id. */
  readonly object_id: string;
  readonly name: string;
  /** Owning Prime session reference. */
  readonly session_ref: string;
  /** Owning kernel runtime incarnation. */
  readonly kernel_generation: string;
  readonly type_name: string;
  /** The serialized execution order that produced the value (provenance). */
  readonly created_at_execution_order: number;
}

/** One kernel object under explicit lifecycle management. */
export interface PromotedKernelObject {
  readonly provenance: KernelObjectProvenance;
  persistence_class: KernelPersistenceClass;
  /** The durable locator; non-null only when artifactized. */
  artifact_ref: string | null;
  /** Focus identities the object is explicitly attached to (spec optional field). */
  focus_refs: readonly FocusRef[];
}

/** Why a promotion transition was refused. */
export type PromotionRefusalReason =
  | "unknown_object"
  | "stale_kernel_generation"
  | "kernel_object_lost"
  | "already_pinned"
  | "already_artifactized"
  | "downgrade_forbidden"
  | "missing_artifact_ref"
  | "empty_focus_refs";

/** The typed result of one promotion transition. */
export type PromotionOutcome =
  | {
      readonly class: "transitioned";
      readonly from: KernelPersistenceClass;
      readonly object: PromotedKernelObject;
    }
  | {
      readonly class: "refused";
      readonly reason: PromotionRefusalReason;
      readonly message: string;
    };

// --- The ledger ------------------------------------------------------------------

/**
 * The explicit promotion ledger over one Prime kernel generation. It is
 * computation-state bookkeeping: NOTHING here is evidence until the canonical
 * admission path (below) says so.
 */
export class KernelObjectLedger {
  private readonly kernel: PrimeReplKernel;
  readonly kernel_generation: string;
  private readonly objects = new Map<string, PromotedKernelObject>();
  private offerCounter = 0;

  constructor(deps: { kernel: PrimeReplKernel; kernel_generation: string }) {
    this.kernel = deps.kernel;
    this.kernel_generation = deps.kernel_generation;
  }

  /**
   * Register the objects one execution created, as EPHEMERAL computation state.
   * Registration is not promotion: it confers nothing.
   */
  register(result: ReplExecuteResult): readonly PromotedKernelObject[] {
    const registered: PromotedKernelObject[] = [];
    for (const created of result.created_objects) {
      const object: PromotedKernelObject = {
        provenance: Object.freeze({
          object_id: created.object_id,
          name: created.name,
          session_ref: this.kernel.sessionId,
          kernel_generation: this.kernel_generation,
          type_name: created.type_name,
          created_at_execution_order: result.execution_order,
        }),
        persistence_class: "ephemeral",
        artifact_ref: null,
        focus_refs: Object.freeze([]),
      };
      this.objects.set(created.object_id, object);
      registered.push(object);
    }
    return Object.freeze(registered);
  }

  /** All registered computation state (evidence is never listed here). */
  list(): readonly PromotedKernelObject[] {
    return Object.freeze([...this.objects.values()]);
  }

  find(objectId: string): PromotedKernelObject | null {
    return this.objects.get(objectId) ?? null;
  }

  /** Explicit pin: the operator (or an admitted strategy) keeps the value alive. */
  pin(objectId: string): PromotionOutcome {
    return this.transitionTo(objectId, "pinned", null);
  }

  /**
   * Explicit artifactization to a durable locator (REQ-SYNC-003: the bytes are
   * pinned in the owning durable mechanism). The locator is REQUIRED — there is
   * no default artifact path, because a made-up path would fabricate
   * durability.
   */
  artifactize(objectId: string, locator: string): PromotionOutcome {
    if (locator.trim().length === 0) {
      return refused(
        "missing_artifact_ref",
        "artifactization requires a non-empty durable locator",
      );
    }
    return this.transitionTo(objectId, "artifactized", locator);
  }

  /** Explicit attach-to-Focus (KernelObjectRef.focus_refs). */
  attachToFocus(objectId: string, focusRefs: readonly FocusRef[]): PromotionOutcome {
    const object = this.requireOwnGeneration(objectId);
    if (object === null) {
      return this.unknownOrStale(objectId);
    }
    if (focusRefs.length === 0) {
      return refused("empty_focus_refs", "attach-to-Focus requires at least one FocusRef");
    }
    const fresh = focusRefs.filter(
      (focus) => !object.focus_refs.some((existing) => existing.id === focus.id),
    );
    if (fresh.length === 0) {
      return refused("empty_focus_refs", "every given FocusRef is already attached to this object");
    }
    object.focus_refs = Object.freeze([...object.focus_refs, ...fresh]);
    return Object.freeze({
      class: "transitioned" as const,
      from: object.persistence_class,
      object,
    });
  }

  /**
   * Offer the object's artifactized value as candidate evidence. The offer is
   * NOT evidence — see `EvidenceOffer`. Refused for anything not artifactized:
   * a live kernel value is not durable evidence (REQ-SYNC-003).
   */
  offerAsEvidence(offer: {
    objectId: string;
    /** Target unit the evidence would support. */
    unit_id: string;
    /** Owning-source namespace of the target unit. */
    source_id: string;
    /** Evidence kind vocabulary of the canonical projection (e.g. "report"). */
    kind: string;
  }): EvidenceOfferOutcome {
    const object = this.requireOwnGeneration(offer.objectId);
    if (object === null) {
      return this.unknownOrStaleOffer(offer.objectId);
    }
    if (object.persistence_class !== "artifactized" || object.artifact_ref === null) {
      return Object.freeze({
        class: "refused" as const,
        reason: "not_artifactized" as const,
        message: `kernel object '${object.provenance.name}' is ${object.persistence_class}; only an explicitly artifactized value with a durable locator can be offered as candidate evidence (REQ-SYNC-003)`,
      });
    }
    this.offerCounter += 1;
    const evidenceId = `kev:${this.kernel.sessionId}:${this.kernel_generation}:${object.provenance.name}:${this.offerCounter}`;
    return Object.freeze({
      class: "offered" as const,
      offer: Object.freeze({
        offer_id: `offer:${evidenceId}`,
        provenance: object.provenance,
        target: Object.freeze({
          unit_id: offer.unit_id,
          source_id: offer.source_id,
        }),
        /** The contract shape the canonical evidence API receives (EvidenceProjection fields). */
        admission_contract: Object.freeze({
          evidence_id: evidenceId,
          unit_id: offer.unit_id,
          kind: offer.kind,
          locator: object.artifact_ref,
          source_id: offer.source_id,
        }),
        establishes_evidence: false as const,
        establishes_verification: false as const,
        establishes_settlement: false as const,
        admission_route: "canonical_gauntlet_evidence_apis" as const,
        verified_source: "canonical_verification_only" as const,
      }),
    });
  }

  // -- internals ------------------------------------------------------------------

  private transitionTo(
    objectId: string,
    target: "pinned" | "artifactized",
    locator: string | null,
  ): PromotionOutcome {
    const object = this.requireOwnGeneration(objectId);
    if (object === null) {
      return this.unknownOrStale(objectId);
    }
    if (target === "pinned" && object.persistence_class === "pinned") {
      return refused(
        "already_pinned",
        `kernel object '${object.provenance.name}' is already pinned`,
      );
    }
    if (target === "pinned" && object.persistence_class === "artifactized") {
      // Promotion is one-way: a ledger transition never lowers a persistence
      // class (the lifecycle authority for reversal is the owning kernel's).
      return refused(
        "downgrade_forbidden",
        `kernel object '${object.provenance.name}' is artifactized; pinning cannot lower an explicit persistence class`,
      );
    }
    if (target === "artifactized" && object.persistence_class === "artifactized") {
      return refused(
        "already_artifactized",
        `kernel object '${object.provenance.name}' is already artifactized at '${object.artifact_ref}'`,
      );
    }
    // Fail closed if the owning kernel no longer holds the value: promotion
    // would claim durability the owning runtime does not have.
    if (this.kernel.getObject(object.provenance.name) === null) {
      return refused(
        "kernel_object_lost",
        `the owning kernel no longer holds '${object.provenance.name}'; the value cannot be promoted`,
      );
    }
    const from = object.persistence_class;
    object.persistence_class = target;
    if (target === "artifactized") {
      const ref = locator;
      if (ref === null) {
        return refused(
          "missing_artifact_ref",
          "artifactization requires a non-empty durable locator",
        );
      }
      object.artifact_ref = ref;
      // Keep the T05 runtime model coherent (its settlement check reads this).
      this.kernel.transitionLifecycle(object.provenance.name, "artifactized", ref);
    } else {
      this.kernel.transitionLifecycle(object.provenance.name, "pinned");
    }
    return Object.freeze({ class: "transitioned" as const, from, object });
  }

  private requireOwnGeneration(objectId: string): PromotedKernelObject | null {
    const object = this.objects.get(objectId);
    if (object === undefined) {
      return null;
    }
    if (object.provenance.kernel_generation !== this.kernel_generation) {
      return null;
    }
    return object;
  }

  private unknownOrStale(objectId: string): PromotionOutcome {
    return refused(
      "unknown_object",
      `no kernel object '${objectId}' is registered in generation '${this.kernel_generation}'; a restarted kernel's names never revive old references`,
    );
  }

  private unknownOrStaleOffer(objectId: string): EvidenceOfferOutcome {
    return Object.freeze({
      class: "refused" as const,
      reason: "not_artifactized" as const,
      message: `no artifactized kernel object '${objectId}' is registered in generation '${this.kernel_generation}'`,
    });
  }
}

function refused(reason: PromotionRefusalReason, message: string): PromotionOutcome {
  return Object.freeze({ class: "refused" as const, reason, message });
}

// --- Offers and canonical admission ---------------------------------------------

export interface EvidenceOffer {
  readonly offer_id: string;
  /** Source/session/generation provenance of the offered value. */
  readonly provenance: KernelObjectProvenance;
  readonly target: { readonly unit_id: string; readonly source_id: string };
  /**
   * The contract shape the canonical Gauntlet evidence API receives — the field
   * vocabulary of the canonical evidence projection (projection-store.ts
   * `EvidenceProjection`).
   */
  readonly admission_contract: {
    readonly evidence_id: string;
    readonly unit_id: string;
    readonly kind: string;
    readonly locator: string;
    readonly source_id: string;
  };
  /** LITERAL false at the type level: an offer is not evidence. */
  readonly establishes_evidence: false;
  /** LITERAL false at the type level: an offer can never establish verification. */
  readonly establishes_verification: false;
  /** LITERAL false at the type level: an offer can never settle a unit. */
  readonly establishes_settlement: false;
  /** The ONLY admittor. */
  readonly admission_route: "canonical_gauntlet_evidence_apis";
  /** `verified` in any admitted record comes from canonical verification, never kernel state. */
  readonly verified_source: "canonical_verification_only";
}

export type EvidenceOfferOutcome =
  | { readonly class: "offered"; readonly offer: EvidenceOffer }
  | { readonly class: "refused"; readonly reason: "not_artifactized"; readonly message: string };

/**
 * The canonical side's OWN answer. Authored exclusively by the canonical
 * Gauntlet evidence APIs (StateStore evidence production/verification, §14.3);
 * this module only CONSUMES it — there is no factory here, and no function in
 * this module can build one from kernel state.
 */
export type CanonicalEvidenceAdmissionDecision =
  | {
      readonly class: "admitted";
      /** Must equal the offered contract's evidence_id. */
      readonly evidence_id: string;
      /** Canonical verification's own answer — never derived from kernel values. */
      readonly verified: boolean;
      /** The canonical source cursor the admission landed at. */
      readonly source_cursor: string;
    }
  | {
      readonly class: "refused";
      readonly evidence_id: string;
      readonly code: string;
      readonly message: string;
    };

/**
 * The admitted evidence record in the canonical projection schema
 * (`EvidenceProjection` shape; `generation_id`/`updated_at` land with the
 * authoritative frame).
 */
export interface AdmittedEvidenceRecord {
  readonly evidence_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly unit_id: string;
  readonly kind: string;
  readonly locator: string;
  readonly verified: boolean;
}

export type EvidenceAdmissionOutcome =
  | {
      readonly class: "admitted";
      readonly record: AdmittedEvidenceRecord;
      readonly provenance: KernelObjectProvenance;
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
 * The admission gate: the workbench-side checkpoint every offer passes through
 * on its way to evidence. It holds NO authority — it validates that a
 * canonical decision exists for the offer and records the admitted truth
 * verbatim (including a canonical `verified: false`).
 */
export class EvidenceAdmissionGate {
  private readonly admitted = new Map<
    string,
    { record: AdmittedEvidenceRecord; provenance: KernelObjectProvenance }
  >();

  /**
   * Admit (or refuse) one offer under a canonical decision. The decision is an
   * input: only the canonical evidence APIs author it.
   */
  admit(
    offer: EvidenceOffer,
    decision: CanonicalEvidenceAdmissionDecision,
  ): EvidenceAdmissionOutcome {
    if (decision.class === "refused") {
      return Object.freeze({
        class: "refused_canonically" as const,
        evidence_id: decision.evidence_id,
        code: decision.code,
        message: decision.message,
      });
    }
    if (decision.evidence_id !== offer.admission_contract.evidence_id) {
      return Object.freeze({
        class: "refused" as const,
        reason: "decision_mismatch" as const,
        message: `canonical decision names evidence '${decision.evidence_id}', not this offer's '${offer.admission_contract.evidence_id}'`,
      });
    }
    if (this.admitted.has(offer.offer_id)) {
      return Object.freeze({
        class: "refused" as const,
        reason: "already_admitted" as const,
        message: `offer '${offer.offer_id}' was already admitted; admission is one-way and never re-derived`,
      });
    }
    const record: AdmittedEvidenceRecord = Object.freeze({
      evidence_id: offer.admission_contract.evidence_id,
      source_id: offer.admission_contract.source_id,
      source_cursor: decision.source_cursor,
      unit_id: offer.admission_contract.unit_id,
      kind: offer.admission_contract.kind,
      locator: offer.admission_contract.locator,
      verified: decision.verified,
    });
    this.admitted.set(offer.offer_id, { record, provenance: offer.provenance });
    return Object.freeze({
      class: "admitted" as const,
      record,
      provenance: offer.provenance,
    });
  }

  /** Whether an offer was admitted (never a claim about un-admitted kernel values). */
  wasAdmitted(offerId: string): boolean {
    return this.admitted.has(offerId);
  }

  /** Every admitted record, in admission order (the workbench's promotion ledger). */
  admittedRecords(): readonly AdmittedEvidenceRecord[] {
    return Object.freeze([...this.admitted.values()].map((entry) => entry.record));
  }
}

/**
 * Build the authoritative `evidence_added` frame that carries a canonical
 * admission into the projection store — the ONLY writer path for evidence
 * rows. Requires the canonical decision and refuses to fabricate one: a bare
 * kernel object or an un-admitted offer cannot produce a frame, because there
 * is no decision to build it from.
 */
export function authoritativeEvidenceFrame(
  offer: EvidenceOffer,
  decision: CanonicalEvidenceAdmissionDecision,
  projection: { generation_id: string; timestamp_ms: number },
): AuthoritativeEventFrame {
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
      // The canonical decision's answer — kernel values never reach this field.
      verified: decision.verified,
    },
    timestamp_ms: projection.timestamp_ms,
  });
}
