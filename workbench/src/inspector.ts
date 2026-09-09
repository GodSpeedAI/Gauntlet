/**
 * Inspector surface: "What is relevant about the current Focus?"
 * (plan T12, REQ-UX-002/003, REQ-GOAL-001, DSN-008, DSN-009, DESIGN.md
 * Inspector Surface).
 *
 * Core invariants:
 *   - Sections derive ONLY from the immutable Workbench snapshot (plus the
 *     canonical affordance resolver); nothing here owns or mutates state.
 *   - Evidence and authority render as inspectable RECORD OBJECTS carrying
 *     claim, producer, result, identity and provenance — never badges
 *     (DSN-008). Fields the projection does not expose say `unavailable`.
 *   - Relations come from the source-typed relation derivation over the
 *     snapshot's own projection rows, with provenance.
 *   - ALL affordances for the focus stay inspectable here — including
 *     palette-only ones — each with its machine reason and a next step
 *     (DSN-009).
 */

import type {
  AffordanceDescriptor,
  AffordanceObservation,
  AffordanceSessionContext,
  PaymentEstimate,
} from "./affordance";
import { resolveAffordance } from "./affordance";
import type { SemanticEnrichment } from "./domainforge-adapter";
import { renderSemanticSection } from "./domainforge-adapter";
import type { FocusRef } from "./focus";
import { operationIds } from "./operation-registry";
import type { EvidenceProjection, SessionProjection, UnitProjection } from "./projection-store";
import type { ProjectionWorld, RelationEdge } from "./relations";
import { deriveRelations } from "./relations";
import type { RenderedLine, RenderedRecord } from "./render-tree";
import type { SxrLensProjection, SxrVerificationPath } from "./sxr-adapter";
import { renderSxrEvidenceSection } from "./sxr-adapter";

export interface InspectorSection {
  readonly id: string;
  readonly title: string;
  readonly lines: readonly RenderedLine[];
  readonly records: readonly RenderedRecord[];
}

export interface InspectorArgs {
  readonly snapshot: {
    readonly generation: string;
    readonly units: readonly UnitProjection[];
    readonly evidence: readonly EvidenceProjection[];
    readonly sessions: readonly SessionProjection[];
    readonly source_cursors: Record<string, string>;
    /**
     * T20 lens wiring (T14/T14B obligation): optional PUBLISHED lens data an
     * operator host may carry on the snapshot (see `publishLensData`). The
     * semantic and reality-trace sections appear ONLY when the snapshot
     * carries the projected data — absence publishes nothing and no section
     * is fabricated. Never canonical state; projections, not truth stores.
     */
    readonly semantic?: SemanticEnrichment | undefined;
    readonly sxr_lens?: SxrLensProjection | undefined;
    readonly sxr_paths?: readonly SxrVerificationPath[] | undefined;
  };
  readonly focus: FocusRef;
  readonly observation: AffordanceObservation;
  readonly session?: AffordanceSessionContext | undefined;
  readonly paymentEstimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
}

/**
 * Map one canonical refusal/reason code to a valid operator next step
 * (DSN-009). The mapping restates the canonical gate vocabulary of T09's
 * resolver in operator language; it is presentation text, never authority.
 */
export function reasonNextStep(reason: string): string {
  switch (reason) {
    case "target_not_visible":
      return "select a unit the current reads name";
    case "surface_unreachable":
      return "restore the canonical surface connection";
    case "actor_authentication_required":
      return "authenticate an operator session";
    case "authority_not_permitted":
      return "obtain authority for this target";
    case "blocking_residual_open":
      return "resolve the open residual denial first";
    case "budget_exhausted":
      return "increase remaining budget or choose a cheaper action";
    case "no_open_ticket":
      return "requires an open escalation ticket for this unit";
    case "readiness_not_earned":
      return "retry requires a failed verification on this unit";
    case "observation_degraded":
      return "refresh reads before acting on degraded data";
    case "unsupported_operation":
      return "served by no canonical operation";
    case "not_action_admitted":
      return "read-shaped; served by the canonical read path";
    default:
      return "inspect the blocking gate in the canonical record";
  }
}

function fieldLine(key: string, value: string): RenderedLine {
  return {
    spans: [{ text: `${key.padEnd(12)} `, emphasis: "dim" }, { text: value }],
  };
}

/**
 * One inspectable EVIDENCE record object (DSN-008): identity, claim,
 * producer, result, reference and provenance from the projection row.
 */
export function renderEvidenceRecord(evidence: EvidenceProjection): RenderedRecord {
  return {
    id: `evidence:${evidence.evidence_id}`,
    kind: "evidence",
    token: evidence.verified ? "integrity.verified" : "integrity.partial",
    fields: {
      identity: evidence.evidence_id,
      claim: `${evidence.kind} evidence at ${evidence.locator || "unavailable"}`,
      producer: evidence.source_id,
      result: evidence.verified ? "VERIFIED" : "UNVERIFIED",
      reference: evidence.locator || "unavailable",
      provenance: `generation ${evidence.generation_id} · source ${evidence.source_id} @ cursor ${evidence.source_cursor}`,
    },
  };
}

/**
 * One inspectable AUTHORITY record object (DSN-008) from the fields the unit
 * projection actually exposes; fields it does not expose say `unavailable`
 * (REQ-UX-007). Allow/deny/escalation decisions are NOT inferred from a
 * status string.
 */
export function renderAuthorityRecord(unit: UnitProjection): RenderedRecord {
  return {
    id: `authority:${unit.unit_id}`,
    kind: "authority",
    token: "authority.pending",
    fields: {
      actor: unit.authority || "unavailable",
      operation: "unavailable",
      resource: unit.unit_id,
      decision: "unavailable",
      blast_radius: unit.blast_radius || "unavailable",
      basis: `generation ${unit.generation_id} · source ${unit.source_id} @ cursor ${unit.source_cursor}`,
    },
  };
}

function relationLines(
  focus: FocusRef,
  edges: readonly RelationEdge[],
): {
  lines: RenderedLine[];
  records: RenderedRecord[];
} {
  const touching = edges.filter(
    (edge) => edge.source_ref.id === focus.id || edge.target_ref.id === focus.id,
  );
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];
  if (touching.length === 0) {
    lines.push({ spans: [{ text: "no relations in the current projection", emphasis: "dim" }] });
    return { lines, records };
  }
  for (const edge of touching) {
    const outbound = edge.source_ref.id === focus.id;
    const other = outbound ? edge.target_ref : edge.source_ref;
    lines.push({
      spans: [
        { text: `${outbound ? "→" : "←"} `, token: "structure.neutral" },
        { text: `${edge.kind} `, emphasis: "normal" },
        { text: `${other.kind}:${other.source_id}`, emphasis: "bold" },
        {
          text: `  (${edge.valid ? "valid" : "invalid attribution"} · ${edge.provenance.source_id}@${edge.provenance.source_cursor})`,
          emphasis: "dim",
        },
      ],
    });
    records.push({
      id: `relation:${edge.kind}:${edge.source_ref.id}:${edge.target_ref.id}`,
      kind: "relation",
      fields: {
        kind: edge.kind,
        source: edge.source_ref.id,
        target: edge.target_ref.id,
        valid: String(edge.valid),
        provenance: `${edge.provenance.generation_id} · ${edge.provenance.source_id}@${edge.provenance.source_cursor}`,
      },
    });
  }
  return { lines, records };
}

function affordanceSection(
  focus: FocusRef,
  observation: AffordanceObservation,
  session: AffordanceSessionContext | undefined,
  paymentEstimates: Readonly<Record<string, PaymentEstimate>> | undefined,
): InspectorSection {
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];
  for (const operation of operationIds()) {
    const descriptor: AffordanceDescriptor = resolveAffordance(operation, focus, observation, {
      session,
      payment_estimates: paymentEstimates,
    });
    const gate =
      descriptor.status === "available"
        ? "available"
        : descriptor.status === "requires_attention"
          ? "requires attention"
          : descriptor.status;
    lines.push({
      spans: [
        {
          text: `${operation} `,
          emphasis: "bold",
        },
        {
          text: `${descriptor.status === "available" ? "✓" : "×"} ${gate}`,
          token: descriptor.status === "available" ? "readiness.ready" : "readiness.blocked",
        },
        ...(descriptor.reason_codes.length > 0
          ? [
              {
                text: ` — ${descriptor.reason_codes.join(", ")}`,
                emphasis: "dim" as const,
              },
            ]
          : []),
      ],
    });
    if (descriptor.reason_codes.length > 0) {
      lines.push({
        spans: [
          { text: "next step   ".padEnd(13), emphasis: "dim" },
          { text: reasonNextStep(descriptor.reason_codes[0] ?? "") },
        ],
      });
    }
    if (descriptor.required_attention_item !== undefined) {
      // The attention item this affordance responds to (REQ-UX-003).
      lines.push({
        spans: [
          { text: "responds to ".padEnd(13), emphasis: "dim" },
          { text: descriptor.required_attention_item, token: "attention.required" },
        ],
      });
    }
    records.push({
      id: `affordance:${operation}:${focus.id}`,
      kind: "affordance",
      token: descriptor.status === "available" ? "readiness.ready" : "readiness.blocked",
      fields: {
        operation,
        status: descriptor.status,
        reasons: descriptor.reason_codes.join(",") || "none",
        next_step:
          descriptor.reason_codes.length > 0
            ? reasonNextStep(descriptor.reason_codes[0] ?? "")
            : "none",
        ...(descriptor.required_attention_item !== undefined
          ? { required_attention_item: descriptor.required_attention_item }
          : {}),
        target: focus.id,
      },
    });
  }
  return { id: "affordances", title: "AFFORDANCES", lines, records };
}

/** Build the Inspector sections for one Focus from one snapshot. */
export function renderInspector(args: InspectorArgs): {
  focus: FocusRef;
  sections: readonly InspectorSection[];
} {
  const { snapshot, focus, observation } = args;
  const sections: InspectorSection[] = [];

  // Identity — the one FocusRef identity, distinct from any placement.
  sections.push({
    id: "identity",
    title: "IDENTITY",
    lines: [
      fieldLine("ref", focus.id),
      fieldLine("kind", focus.kind),
      fieldLine("source", focus.source),
      fieldLine("object", focus.source_id),
      ...(focus.human_label !== undefined ? [fieldLine("label", focus.human_label)] : []),
    ],
    records: [],
  });

  // State — only for kinds the projection actually carries (REQ-UX-007:
  // fields not exposed say unavailable; nothing is inferred).
  const unit = snapshot.units.find((candidate) => candidate.unit_id === focus.source_id);
  if (focus.kind === "WorkItem" && unit !== undefined) {
    sections.push({
      id: "state",
      title: "STATE",
      lines: [
        fieldLine("title", unit.title || "unavailable"),
        fieldLine("phase", unit.phase || "unavailable"),
        fieldLine("status", unit.status || "unavailable"),
        fieldLine("authority", unit.authority || "unavailable"),
        fieldLine("blast radius", unit.blast_radius || "unavailable"),
        fieldLine("updated", String(unit.updated_at)),
      ],
      records: [],
    });
  } else {
    sections.push({
      id: "state",
      title: "STATE",
      lines: [
        {
          spans: [{ text: "no projected state for this focus kind", emphasis: "dim" }],
        },
      ],
      records: [],
    });
  }

  // Relations — source-typed edges touching this focus.
  const world: ProjectionWorld = {
    generation: snapshot.generation,
    units: snapshot.units,
    evidence: snapshot.evidence,
    sessions: snapshot.sessions,
    source_cursors: snapshot.source_cursors,
  };
  const relation = relationLines(focus, deriveRelations(world));
  sections.push({
    id: "relations",
    title: "RELATIONS",
    lines: relation.lines,
    records: relation.records,
  });

  // Authority — an inspectable record, never a badge (DSN-008).
  if (unit !== undefined) {
    const authorityRecord = renderAuthorityRecord(unit);
    sections.push({
      id: "authority",
      title: "AUTHORITY",
      lines: Object.entries(authorityRecord.fields).map(([key, value]) => fieldLine(key, value)),
      records: [authorityRecord],
    });
  }

  // Evidence — inspectable record objects (DSN-008).
  const focusEvidence = snapshot.evidence.filter(
    (evidence) => focus.kind === "WorkItem" && evidence.unit_id === focus.source_id,
  );
  sections.push({
    id: "evidence",
    title: "EVIDENCE",
    lines:
      focusEvidence.length === 0
        ? [
            {
              spans: [
                {
                  text: "! none in the current projection",
                  token: "evidence.missing",
                  emphasis: "dim",
                },
              ],
            },
          ]
        : focusEvidence.map((evidence): RenderedLine => {
            const record = renderEvidenceRecord(evidence);
            return {
              spans: [
                { text: `${record.fields["result"]} `, token: record.token },
                { text: `${record.fields["identity"]} `, emphasis: "bold" },
                { text: record.fields["claim"] ?? "" },
                { text: `  (${record.fields["provenance"] ?? ""})`, emphasis: "dim" },
              ],
            };
          }),
    records: focusEvidence.map(renderEvidenceRecord),
  });

  // Provenance — the owning source and cursor behind this projection.
  const cursor = snapshot.source_cursors[focus.source] ?? "unavailable";
  sections.push({
    id: "provenance",
    title: "PROVENANCE",
    lines: [
      fieldLine("generation", snapshot.generation),
      fieldLine("source", focus.source),
      fieldLine("cursor", cursor),
    ],
    records: [],
  });

  // T20 LENS WIRING (T14/T14B obligation): the semantic-enrichment and
  // reality-trace sections join the live section list, gated on the snapshot
  // actually CARRYING the projected data. Nothing here invents or infers
  // provider output; when the host published no lens, no section appears.
  if (snapshot.semantic !== undefined) {
    sections.push(renderSemanticSection(snapshot.semantic));
  }
  if (snapshot.sxr_lens !== undefined) {
    sections.push(renderSxrEvidenceSection(snapshot.sxr_lens, snapshot.sxr_paths ?? []));
  }

  // Affordances — every operation with reasons and next steps (DSN-009).
  sections.push(affordanceSection(focus, observation, args.session, args.paymentEstimates));

  return { focus, sections };
}

// --- T20 lens publication seam (read-only, mirrors publishTerminalProvider) -------

/** The noncanonical lens data a host may publish onto a snapshot. */
export interface PublishedLensData {
  readonly semantic?: SemanticEnrichment | undefined;
  readonly sxr_lens?: SxrLensProjection | undefined;
  readonly sxr_paths?: readonly SxrVerificationPath[] | undefined;
}

/**
 * Publish projected lens data as OWN properties on a snapshot so
 * `renderInspector` includes the semantic and reality-trace sections through
 * the live shell path. Canonical fields are untouched; publishing nothing
 * (or omitting a lens) keeps that section absent — the gate is the data's
 * presence on the snapshot, never a default.
 */
export function publishLensData<S extends object>(
  snapshot: S,
  lenses: PublishedLensData,
): S & PublishedLensData {
  return Object.freeze({ ...snapshot, ...lenses });
}
