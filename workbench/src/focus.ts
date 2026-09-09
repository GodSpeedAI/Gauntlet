/**
 * Stable FocusRef identity and FocusPlacement records.
 * Conforms to spec §core_domain_model entities FocusRef/FocusPlacement, REQ-FOCUS-001,
 * REQ-FOCUS-002, DSN-006, and plan T08.
 *
 * Core Invariants:
 *   - Focus identity is source/kind-based and deterministic. It is NEVER derived from
 *     visual position, React keys, or mutable display text (REQ-FOCUS-001).
 *   - FocusRef.for(source, kind, sourceId) returns the SAME immutable identity for the
 *     same (source, kind, sourceId) triple for the lifetime of the process; every
 *     placement of one object resolves to that one identity (DSN-006).
 *   - A FocusPlacement is one rendered occurrence of a FocusRef. It carries NO canonical
 *     object state; placement-local labels never touch the FocusRef (REQ-FOCUS-002).
 *   - Placement identity is never used as, or substituted for, Focus identity.
 */

/** Deterministic workbench reference id derived from owning-system identity and object kind. */
export function focusRefId(source: string, kind: string, sourceId: string): string {
  return `focus:${source}:${kind}:${sourceId}`;
}

/** Typed error for malformed external FocusRef documents (model output is untrusted input). */
export class MalformedFocusRefError extends Error {
  constructor(detail: string) {
    super(`malformed focus ref: ${detail}`);
    this.name = "MalformedFocusRefError";
  }
}

/**
 * Stable semantic reference to the smallest complete object currently selected,
 * inspected, related, or rendered across multiple placements (spec §FocusRef).
 * Instances are immutable and interned: one triple, one object, forever.
 */
export class FocusRef {
  readonly id: string;
  readonly kind: string;
  readonly source: string;
  readonly source_id: string;
  readonly human_label: string | undefined;

  private static readonly registry = new Map<string, FocusRef>();

  private constructor(source: string, kind: string, sourceId: string, humanLabel?: string) {
    this.id = focusRefId(source, kind, sourceId);
    this.kind = kind;
    this.source = source;
    this.source_id = sourceId;
    this.human_label = humanLabel;
    Object.freeze(this);
  }

  /**
   * Identity factory: always returns the SAME FocusRef for the same
   * (source, kind, sourceId) triple. `humanLabel` is canonical, source-provided
   * display text and only applies when this call mints a fresh identity; it can
   * never rewrite an existing FocusRef.
   */
  static for(source: string, kind: string, sourceId: string, humanLabel?: string): FocusRef {
    const id = focusRefId(source, kind, sourceId);
    const existing = FocusRef.registry.get(id);
    if (existing !== undefined) {
      return existing;
    }
    const minted = new FocusRef(source, kind, sourceId, humanLabel);
    FocusRef.registry.set(id, minted);
    return minted;
  }

  /**
   * Re-admit an untrusted/serialized FocusRef document: validate its shape and
   * internal id consistency, then resolve it through the identity registry so a
   * restored ref is the SAME identity, never a fork.
   */
  static fromJSON(value: unknown): FocusRef {
    if (typeof value !== "object" || value === null) {
      throw new MalformedFocusRefError("expected an object");
    }
    const record = value as Record<string, unknown>;
    const id = record["id"];
    const kind = record["kind"];
    const source = record["source"];
    const sourceId = record["source_id"];
    const humanLabel = record["human_label"];
    if (
      typeof id !== "string" ||
      typeof kind !== "string" ||
      typeof source !== "string" ||
      typeof sourceId !== "string"
    ) {
      throw new MalformedFocusRefError("requires string id, kind, source, and source_id");
    }
    if (humanLabel !== undefined && typeof humanLabel !== "string") {
      throw new MalformedFocusRefError("human_label must be a string when present");
    }
    const resolved = FocusRef.for(source, kind, sourceId, humanLabel);
    if (resolved.id !== id) {
      throw new MalformedFocusRefError(
        `declared id '${id}' does not match derived identity '${resolved.id}'`,
      );
    }
    return resolved;
  }

  /** Plain serializable form (JSON.stringify picks this up automatically). */
  toJSON(): { id: string; kind: string; source: string; source_id: string; human_label?: string } {
    if (this.human_label === undefined) {
      return { id: this.id, kind: this.kind, source: this.source, source_id: this.source_id };
    }
    return {
      id: this.id,
      kind: this.kind,
      source: this.source,
      source_id: this.source_id,
      human_label: this.human_label,
    };
  }
}

export interface FocusPlacementInit {
  readonly relation_role?: string | undefined;
  readonly presentation_hint?: string | undefined;
  /** Placement-local display label. Overrides nothing canonical; never touches the FocusRef. */
  readonly label_override?: string | undefined;
}

/**
 * One rendered occurrence of a FocusRef in a Surface/View (spec §FocusPlacement).
 * Carries NO canonical object state; multiple placements of one FocusRef coexist
 * without copying or forking the underlying object state.
 */
export interface FocusPlacement {
  /** Occurrence identity, valid only within the publishing snapshot generation. */
  readonly id: string;
  /** The single underlying object this occurrence renders. */
  readonly focus: FocusRef;
  readonly surface: string;
  readonly view: string;
  readonly generation: string;
  readonly relation_role: string | undefined;
  readonly presentation_hint: string | undefined;
  readonly label_override: string | undefined;
}

/** Occurrence identity: generation-scoped, surface/view-scoped — never a Focus identity. */
export function makePlacementId(
  generation: string,
  surface: string,
  view: string,
  focus: FocusRef,
): string {
  return `placement:${generation}:${surface}:${view}:${focus.id}`;
}

export function createPlacement(
  generation: string,
  surface: string,
  view: string,
  focus: FocusRef,
  init: FocusPlacementInit = {},
): FocusPlacement {
  return Object.freeze({
    id: makePlacementId(generation, surface, view, focus),
    focus,
    surface,
    view,
    generation,
    relation_role: init.relation_role,
    presentation_hint: init.presentation_hint,
    label_override: init.label_override,
  });
}
