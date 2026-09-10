/**
 * Source-typed Relation edges between FocusRefs, derived from projection-store data.
 * Conforms to spec §core_domain_model vocabulary Relation, REQ-GOAL-001, and plan T08.
 *
 * Core Invariants:
 *   - A Relation is a source-typed edge (source FocusRef → target FocusRef) with a relation
 *     kind, provenance, and source version. Correlation is not causation or authority.
 *   - Edges are derived projections of authoritative store rows; they carry the row's
 *     generation/source/cursor provenance and never become independent truth.
 *   - Traversal is cycle-safe: a visited set keyed on FocusRef identity bounds every walk.
 */

import { FocusRef } from "./focus";
import type { EvidenceProjection, SessionProjection, UnitProjection } from "./projection-store";

/** The projection-store slice the relation/materialization layers read from. */
export interface ProjectionWorld {
  readonly generation: string;
  readonly units: readonly UnitProjection[];
  readonly evidence: readonly EvidenceProjection[];
  readonly sessions: readonly SessionProjection[];
  readonly source_cursors: Readonly<Record<string, string>>;
}

export interface RelationProvenance {
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
}

export interface RelationEdge {
  readonly source_ref: FocusRef;
  readonly target_ref: FocusRef;
  readonly kind: string;
  readonly provenance: RelationProvenance;
  /** Source version the edge was derived at (the owning row's cursor). */
  readonly source_version: string;
  /** Whether both endpoints currently resolve to rows in the active projection. */
  readonly valid: boolean;
}

/** One declared relation traversal eligible for projection/context construction. */
export interface RelationQuerySpec {
  readonly kind: string;
  readonly depth: number;
  readonly direction: "outbound" | "inbound" | "both";
}

/**
 * FocusRef factories for authoritative projection rows. They intentionally do NOT
 * set human_label: canonical labels come only from an owning source minting the
 * identity, while display text in projections is placement-local state (REQ-FOCUS-002).
 */
export function focusForUnit(unit: UnitProjection): FocusRef {
  return FocusRef.for(unit.source_id, "WorkItem", unit.unit_id);
}

export function focusForEvidence(evidence: EvidenceProjection): FocusRef {
  return FocusRef.for(evidence.source_id, "Evidence", evidence.evidence_id);
}

export function focusForSession(session: SessionProjection): FocusRef {
  return FocusRef.for(session.source_id, "Agent", session.session_id);
}

/**
 * Attribution rule for one endpoint of a claimed link (peer correction MAJOR-1):
 * the endpoint is attributed from the OWNING row's source when exactly one row
 * resolves the claimed id; otherwise the claiming row's own source is kept as a
 * claim and the edge is marked invalid. A claimed id that resolves in two sources
 * is ambiguous — no owner is picked, the edge stays invalid.
 */
function attributedSource(
  claimedSource: string,
  resolveOwners: () => readonly (string | undefined)[],
): { source: string; valid: boolean } {
  const owners = resolveOwners();
  if (owners.length === 1) {
    const owner = owners[0];
    if (owner !== undefined) {
      return { source: owner, valid: owner === claimedSource };
    }
  }
  return { source: claimedSource, valid: false };
}

/** Shared empty owner list for ids that resolve to no row (read-only use). */
const NO_OWNERS: readonly string[] = Object.freeze([]);

/**
 * Derive the relation graph from authoritative projection rows:
 *   - unit → evidence links (kind "has_evidence")
 *   - session parent topology (kind "child_session")
 *
 * Link endpoints are attributed from the owning row's source (see attributedSource):
 * a cross-source link yields an INVALID edge with the endpoint still attributed to
 * the true owner, never a second identity for an object that already has one.
 * Provenance always reflects the row that asserted the link.
 *
 * Ownership is resolved through a linear-time id → source-owners index built once
 * per call (identical resolution semantics to the previous per-row scans: the
 * owner array for an id is the same source_ids in the same world order, so every
 * edge is byte-identical). Total cost is O(units + evidence + sessions + edges)
 * instead of the former O(evidence·units + sessions²) scans, which dominated the
 * hoisted materialization fold at T21 target scale (measured ~1.1 s at
 * 3,000 × 25,000 rows).
 */
export function deriveRelations(world: ProjectionWorld): readonly RelationEdge[] {
  const edges: RelationEdge[] = [];

  const unitOwners = new Map<string, string[]>();
  for (const unit of world.units) {
    const owners = unitOwners.get(unit.unit_id);
    if (owners === undefined) {
      unitOwners.set(unit.unit_id, [unit.source_id]);
    } else {
      owners.push(unit.source_id);
    }
  }

  const sessionOwners = new Map<string, string[]>();
  for (const session of world.sessions) {
    const owners = sessionOwners.get(session.session_id);
    if (owners === undefined) {
      sessionOwners.set(session.session_id, [session.source_id]);
    } else {
      owners.push(session.source_id);
    }
  }

  for (const evidence of world.evidence) {
    const unitEndpoint = attributedSource(
      evidence.source_id,
      () => unitOwners.get(evidence.unit_id) ?? NO_OWNERS,
    );
    edges.push(
      Object.freeze({
        source_ref: FocusRef.for(unitEndpoint.source, "WorkItem", evidence.unit_id),
        target_ref: focusForEvidence(evidence),
        kind: "has_evidence",
        provenance: Object.freeze({
          generation_id: evidence.generation_id,
          source_id: evidence.source_id,
          source_cursor: evidence.source_cursor,
        }),
        source_version: evidence.source_cursor,
        valid: unitEndpoint.valid,
      }),
    );
  }

  for (const session of world.sessions) {
    if (session.parent_id === null) {
      continue;
    }
    const parentId: string = session.parent_id;
    const parentEndpoint = attributedSource(
      session.source_id,
      () => sessionOwners.get(parentId) ?? NO_OWNERS,
    );
    edges.push(
      Object.freeze({
        source_ref: FocusRef.for(parentEndpoint.source, "Agent", session.parent_id),
        target_ref: focusForSession(session),
        kind: "child_session",
        provenance: Object.freeze({
          generation_id: session.generation_id,
          source_id: session.source_id,
          source_cursor: session.source_cursor,
        }),
        source_version: session.source_cursor,
        valid: parentEndpoint.valid,
      }),
    );
  }

  return Object.freeze(edges);
}

export interface TraversalResult {
  readonly reached: readonly FocusRef[];
  readonly edges: readonly RelationEdge[];
}

/**
 * Cycle-safe traversal from a root FocusRef along declared relation queries.
 * Each query walks up to its own depth in its own direction; results are unioned
 * and deduplicated by FocusRef identity. A visited set bounds every walk, so
 * graph-shaped (cyclic) relations terminate.
 */
export function traverseRelations(
  relations: readonly RelationEdge[],
  root: FocusRef,
  queries: readonly RelationQuerySpec[],
): TraversalResult {
  const reached = new Map<string, FocusRef>([[root.id, root]]);
  const edgeIndex = new Map<string, RelationEdge>();

  for (const spec of queries) {
    if (spec.depth < 1) {
      continue;
    }
    const seen = new Set<string>([root.id]);
    let frontier: FocusRef[] = [root];
    for (let hop = 0; hop < spec.depth; hop += 1) {
      const next: FocusRef[] = [];
      for (const node of frontier) {
        for (const edge of relations) {
          if (edge.kind !== spec.kind) {
            continue;
          }
          let neighbor: FocusRef | null = null;
          if (
            (spec.direction === "outbound" || spec.direction === "both") &&
            edge.source_ref.id === node.id
          ) {
            neighbor = edge.target_ref;
          }
          if (
            neighbor === null &&
            (spec.direction === "inbound" || spec.direction === "both") &&
            edge.target_ref.id === node.id
          ) {
            neighbor = edge.source_ref;
          }
          if (neighbor === null || seen.has(neighbor.id)) {
            continue;
          }
          seen.add(neighbor.id);
          reached.set(neighbor.id, neighbor);
          edgeIndex.set(`${edge.kind}|${edge.source_ref.id}|${edge.target_ref.id}`, edge);
          next.push(neighbor);
        }
      }
      frontier = next;
      if (frontier.length === 0) {
        break;
      }
    }
  }

  return Object.freeze({
    reached: Object.freeze([...reached.values()]),
    edges: Object.freeze([...edgeIndex.values()]),
  });
}
