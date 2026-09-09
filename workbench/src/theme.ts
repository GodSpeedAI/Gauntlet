/**
 * Semantic theme tokens and glyph formatters.
 * Conforms to DSN-001, DSN-002, DSN-003, and DSN-004.
 *
 * T12 extension (plan T12, DSN-001..004): this file is the ONE place raw colour
 * values may live (DSN-001). Presentation components resolve semantic tokens and
 * lower them through `resolveToken` to truecolor, 256-colour, or monochrome
 * presentation (DSN-002); token families stay distinct even where default
 * colours coincide (DSN-004); gold is reserved for proof and sanctioned
 * authority (DSN-003).
 */

export interface StatusDescriptor {
  readonly glyph: string;
  readonly label: string;
  readonly category: "settlement" | "evidence" | "execution" | "readiness";
  readonly emphasis: "normal" | "bold" | "dim";
}

// --- Semantic token table (DESIGN.md "Utility states" + gold discipline) -----

/** The semantic token families; distinct families stay distinct labels (DSN-004). */
export type TokenFamily =
  | "authority"
  | "execution"
  | "evidence"
  | "settlement"
  | "readiness"
  | "integrity"
  | "capability"
  | "attention"
  | "focus"
  | "structure";

/** Emphasis is part of the colour-degraded distinguishability triple (DSN-002). */
export type Emphasis = "normal" | "bold" | "dim";

/**
 * Terminal colour capability. Consequential state must remain intelligible
 * through glyph + label + emphasis even in "monochrome" (DSN-002).
 */
export type ColorCapability = "truecolor" | "color256" | "monochrome";

/** Every semantic token name the presentation layer may resolve. */
export type SemanticToken =
  // authority
  | "authority.allowed"
  | "authority.denied"
  | "authority.escalated"
  | "authority.pending"
  | "authority.sanctioned"
  // execution
  | "execution.running"
  | "execution.waiting"
  | "execution.succeeded"
  | "execution.failed"
  | "execution.cancelled"
  // evidence
  | "evidence.complete"
  | "evidence.missing"
  | "evidence.failed"
  | "evidence.stale"
  // settlement
  | "settlement.pending"
  | "settlement.evaluating"
  | "settlement.accepted"
  | "settlement.rejected"
  | "settlement.escalated"
  // readiness
  | "readiness.ready"
  | "readiness.degraded"
  | "readiness.blocked"
  // integrity
  | "integrity.verified"
  | "integrity.partial"
  | "integrity.pending"
  | "integrity.failed"
  // capability
  | "capability.attempted"
  | "capability.demonstrated"
  | "capability.proven"
  | "capability.degraded"
  // attention / focus / structure
  | "attention.required"
  | "focus.primary"
  | "focus.ring"
  | "structure.neutral"
  // proof (gold discipline, DSN-003)
  | "proof.independent";

export interface TokenSpec {
  readonly family: TokenFamily;
  /** Default truecolor value from DESIGN.md's palette. Raw literals live ONLY here (DSN-001). */
  readonly color: string;
  /** Semantic glyph from DESIGN.md's recommended family (renderer vocabulary, not state). */
  readonly glyph: string;
  /** Uppercase text label; carries the state when colour degrades (DSN-002). */
  readonly label: string;
  readonly emphasis: Emphasis;
}

const GOLD = "#D7A84A"; // brand.gold — proof and sanctioned authority ONLY (DSN-003).

/**
 * The frozen semantic token table. Colours are the DESIGN.md dark-surface
 * projection defaults; 256-colour values below are the maintained nearest
 * anchors for capability lowering.
 */
export const SEMANTIC_TOKENS: Readonly<Record<SemanticToken, TokenSpec>> = Object.freeze({
  "authority.allowed": {
    family: "authority",
    color: "#2F8F5B",
    glyph: "✓",
    label: "ALLOWED",
    emphasis: "normal",
  },
  "authority.denied": {
    family: "authority",
    color: "#9E2F2F",
    glyph: "×",
    label: "DENIED",
    emphasis: "bold",
  },
  "authority.escalated": {
    family: "authority",
    color: "#C58A2A",
    glyph: "!",
    label: "ESCALATED",
    emphasis: "bold",
  },
  "authority.pending": {
    family: "authority",
    color: "#7C8790",
    glyph: "○",
    label: "PENDING",
    emphasis: "dim",
  },
  "authority.sanctioned": {
    family: "authority",
    color: GOLD,
    glyph: "◆",
    label: "SANCTIONED",
    emphasis: "bold",
  },
  "execution.running": {
    family: "execution",
    color: "#2563EB",
    glyph: "●",
    label: "RUNNING",
    emphasis: "bold",
  },
  "execution.waiting": {
    family: "execution",
    color: "#7C8790",
    glyph: "○",
    label: "WAITING",
    emphasis: "dim",
  },
  "execution.succeeded": {
    family: "execution",
    color: "#2F8F5B",
    glyph: "✓",
    label: "SUCCEEDED",
    emphasis: "normal",
  },
  "execution.failed": {
    family: "execution",
    color: "#9E2F2F",
    glyph: "×",
    label: "FAILED",
    emphasis: "bold",
  },
  "execution.cancelled": {
    family: "execution",
    color: "#7C8790",
    glyph: "○",
    label: "CANCELLED",
    emphasis: "dim",
  },
  "evidence.complete": {
    family: "evidence",
    color: "#2F8F5B",
    glyph: "✓",
    label: "COMPLETE",
    emphasis: "normal",
  },
  "evidence.missing": {
    family: "evidence",
    color: "#C58A2A",
    glyph: "!",
    label: "MISSING",
    emphasis: "normal",
  },
  "evidence.failed": {
    family: "evidence",
    color: "#9E2F2F",
    glyph: "×",
    label: "FAILED",
    emphasis: "bold",
  },
  "evidence.stale": {
    family: "evidence",
    color: "#C58A2A",
    glyph: "◷",
    label: "STALE",
    emphasis: "normal",
  },
  "settlement.pending": {
    family: "settlement",
    color: "#7C8790",
    glyph: "○",
    label: "PENDING",
    emphasis: "dim",
  },
  "settlement.evaluating": {
    family: "settlement",
    color: "#2563EB",
    glyph: "●",
    label: "EVALUATING",
    emphasis: "normal",
  },
  "settlement.accepted": {
    family: "settlement",
    color: "#2F8F5B",
    glyph: "✓",
    label: "ACCEPTED",
    emphasis: "bold",
  },
  "settlement.rejected": {
    family: "settlement",
    color: "#9E2F2F",
    glyph: "×",
    label: "REJECTED",
    emphasis: "bold",
  },
  "settlement.escalated": {
    family: "settlement",
    color: "#C58A2A",
    glyph: "!",
    label: "ESCALATED",
    emphasis: "bold",
  },
  "readiness.ready": {
    family: "readiness",
    color: "#2F8F5B",
    glyph: "✓",
    label: "READY",
    emphasis: "normal",
  },
  "readiness.degraded": {
    family: "readiness",
    color: "#C58A2A",
    glyph: "?",
    label: "DEGRADED",
    emphasis: "normal",
  },
  "readiness.blocked": {
    family: "readiness",
    color: "#9E2F2F",
    glyph: "×",
    label: "BLOCKED",
    emphasis: "bold",
  },
  "integrity.verified": {
    family: "integrity",
    color: "#2F8F5B",
    glyph: "✓",
    label: "VERIFIED",
    emphasis: "normal",
  },
  "integrity.partial": {
    family: "integrity",
    color: "#7C8790",
    glyph: "○",
    label: "PARTIAL",
    emphasis: "dim",
  },
  "integrity.pending": {
    family: "integrity",
    color: "#C58A2A",
    glyph: "○",
    label: "PENDING",
    emphasis: "normal",
  },
  "integrity.failed": {
    family: "integrity",
    color: "#9E2F2F",
    glyph: "×",
    label: "FAILED",
    emphasis: "bold",
  },
  "capability.attempted": {
    family: "capability",
    color: "#7C8790",
    glyph: "○",
    label: "ATTEMPTED",
    emphasis: "dim",
  },
  "capability.demonstrated": {
    family: "capability",
    color: "#2563EB",
    glyph: "●",
    label: "DEMONSTRATED",
    emphasis: "normal",
  },
  "capability.proven": {
    family: "capability",
    color: "#2F8F5B",
    glyph: "✓",
    label: "PROVEN",
    emphasis: "bold",
  },
  "capability.degraded": {
    family: "capability",
    color: "#C58A2A",
    glyph: "?",
    label: "DEGRADED",
    emphasis: "normal",
  },
  "attention.required": {
    family: "attention",
    color: "#C58A2A",
    glyph: "!",
    label: "ATTENTION",
    emphasis: "bold",
  },
  "focus.primary": {
    family: "focus",
    color: "#2563EB",
    glyph: "●",
    label: "FOCUSED",
    emphasis: "bold",
  },
  "focus.ring": {
    family: "focus",
    color: "#60A5FA",
    glyph: "◉",
    label: "RING",
    emphasis: "normal",
  },
  "structure.neutral": {
    family: "structure",
    color: "#566978",
    glyph: "·",
    label: "NEUTRAL",
    emphasis: "dim",
  },
  "proof.independent": {
    family: "evidence",
    color: GOLD,
    glyph: "◆",
    label: "INDEPENDENT",
    emphasis: "bold",
  },
});

/** Maintained nearest-anchor lowering from default truecolour to 256-colour indexes. */
const COLOR_256_ANCHORS: Readonly<Record<string, number>> = Object.freeze({
  "#2F8F5B": 36,
  "#9E2F2F": 124,
  "#C58A2A": 137,
  "#2563EB": 33,
  "#7C8790": 245,
  "#60A5FA": 75,
  "#566978": 244,
  [GOLD]: 178,
});

/**
 * A token lowered for one terminal colour capability. In monochrome `color`
 * is null: the glyph + label + emphasis triple carries the state (DSN-002).
 */
export interface ResolvedTokenStyle {
  readonly token: SemanticToken;
  readonly family: TokenFamily;
  readonly glyph: string;
  readonly label: string;
  readonly emphasis: Emphasis;
  /** Truecolor hex for truecolor terminals; null in monochrome. */
  readonly color: string | null;
  /** 256-colour index for reduced terminals; null in monochrome. */
  readonly color256: number | null;
}

/** Lower one semantic token for the terminal's colour capability (DSN-001/002). */
export function resolveToken(
  token: SemanticToken,
  capability: ColorCapability,
): ResolvedTokenStyle {
  const spec = SEMANTIC_TOKENS[token];
  if (capability === "monochrome") {
    return Object.freeze({
      token,
      family: spec.family,
      glyph: spec.glyph,
      label: spec.label,
      emphasis: spec.emphasis,
      color: null,
      color256: null,
    });
  }
  if (capability === "color256") {
    return Object.freeze({
      token,
      family: spec.family,
      glyph: spec.glyph,
      label: spec.label,
      emphasis: spec.emphasis,
      color: null,
      color256: COLOR_256_ANCHORS[spec.color] ?? 245,
    });
  }
  return Object.freeze({
    token,
    family: spec.family,
    glyph: spec.glyph,
    label: spec.label,
    emphasis: spec.emphasis,
    color: spec.color,
    color256: COLOR_256_ANCHORS[spec.color] ?? null,
  });
}

/** The semantic tokens whose default colour is brand gold (DSN-003 discipline set). */
export function goldReservedTokens(): readonly SemanticToken[] {
  return (Object.keys(SEMANTIC_TOKENS) as SemanticToken[]).filter(
    (token) => SEMANTIC_TOKENS[token].color === GOLD,
  );
}

/**
 * Motion is optional (DSN-013). Full motion may use restrained frame sets;
 * reduced motion resolves to a static marker so meaning never depends on
 * animation.
 */
export type MotionPreference = "full" | "reduced";

/** Restrained running-indicator frames; empty in reduced motion (DSN-013). */
export function runningIndicatorFrames(motion: MotionPreference): readonly string[] {
  return motion === "full" ? Object.freeze(["◐", "◓", "◑", "◒"]) : Object.freeze([]);
}

/** The static running marker used when motion is reduced (or as the base glyph). */
export const STATIC_RUNNING_MARKER = "●";

/**
 * Maps unit/system states to distinct glyph + label + emphasis tuples (DSN-002).
 * Color degrades gracefully to plain text without loss of distinction.
 */
export function formatStatus(status: string): StatusDescriptor {
  switch (status.toLowerCase()) {
    case "pass":
    case "settled":
      return {
        glyph: "✓",
        label: "PASS",
        category: "settlement",
        emphasis: "bold",
      };
    case "fail":
      return {
        glyph: "✗",
        label: "FAIL",
        category: "evidence",
        emphasis: "bold",
      };
    case "inconclusive":
      return {
        glyph: "?",
        label: "INCONCLUSIVE",
        category: "evidence",
        emphasis: "normal",
      };
    case "running":
      return {
        glyph: "▶",
        label: "RUNNING",
        category: "execution",
        emphasis: "bold",
      };
    case "blocked":
      return {
        glyph: "■",
        label: "BLOCKED",
        category: "readiness",
        emphasis: "normal",
      };
    default:
      return {
        glyph: "·",
        label: status.toUpperCase(),
        category: "readiness",
        emphasis: "dim",
      };
  }
}

/**
 * Format an evidence reference with source identity and outcome (DSN-008).
 */
export function formatEvidenceItem(
  unitId: string,
  outcome: string,
  evidenceRef: string,
  verificationRef: string,
): string {
  const st = formatStatus(outcome);
  const ev = evidenceRef || "unavailable";
  const ver = verificationRef || "unavailable";
  return `[${st.glyph} ${st.label}] Unit: ${unitId} | Ev: ${ev} | Ver: ${ver}`;
}
