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
 *
 * GUI-redesign increment 2a adds the remaining UESC §23 members (freshness,
 * danger, quarantined, authority.degraded, and the two gold proof tokens),
 * DESIGN.md's depth-level surface and text-role projections, terminal colour
 * capability detection (V6 fix: never assume truecolor), and the
 * operator-reachable reduced-motion setting (DSN-013, M15 fix).
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
  | "freshness"
  | "integrity"
  | "capability"
  | "attention"
  | "danger"
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
  | "authority.degraded"
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
  | "evidence.quarantined"
  // settlement
  | "settlement.pending"
  | "settlement.evaluating"
  | "settlement.accepted"
  | "settlement.rejected"
  | "settlement.escalated"
  | "settlement.quarantined"
  // readiness
  | "readiness.ready"
  | "readiness.degraded"
  | "readiness.blocked"
  // freshness
  | "freshness.current"
  | "freshness.stale"
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
  | "capability.quarantined"
  // attention / danger / focus / structure
  | "attention.required"
  | "danger.destructive"
  | "focus.primary"
  | "focus.ring"
  | "structure.neutral"
  // proof (gold discipline, DSN-003)
  | "proof.independent"
  | "proof.independently_confirmed"
  | "proof.sanctioned_authority";

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
  "authority.degraded": {
    family: "authority",
    color: "#C58A2A",
    glyph: "?",
    label: "DEGRADED",
    emphasis: "normal",
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
  "evidence.quarantined": {
    family: "evidence",
    color: "#C58A2A",
    glyph: "?",
    label: "EV-QUARANTINED",
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
  "settlement.quarantined": {
    family: "settlement",
    color: "#C58A2A",
    glyph: "?",
    label: "QUARANTINED",
    emphasis: "normal",
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
  "freshness.current": {
    family: "freshness",
    color: "#2F8F5B",
    glyph: "✓",
    label: "CURRENT",
    emphasis: "normal",
  },
  "freshness.stale": {
    family: "freshness",
    color: "#C58A2A",
    glyph: "◷",
    label: "STALE",
    emphasis: "normal",
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
  "capability.quarantined": {
    family: "capability",
    color: "#C58A2A",
    glyph: "?",
    label: "CAP-QUARANTINED",
    emphasis: "normal",
  },
  "attention.required": {
    family: "attention",
    color: "#C58A2A",
    glyph: "!",
    label: "ATTENTION",
    emphasis: "bold",
  },
  "danger.destructive": {
    family: "danger",
    color: "#9E2F2F",
    glyph: "×",
    label: "DESTRUCTIVE",
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
  "proof.independently_confirmed": {
    family: "evidence",
    color: GOLD,
    glyph: "◆",
    label: "CONFIRMED",
    emphasis: "bold",
  },
  "proof.sanctioned_authority": {
    family: "authority",
    color: GOLD,
    glyph: "◆",
    label: "SANCTIONED-AUTHORITY",
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
  // Surface and text-role projections (DESIGN.md dark-surface table).
  "#05070A": 232,
  "#0B1220": 233,
  "#111827": 234,
  "#0F172A": 234,
  "#08101D": 233,
  "#334155": 238,
  "#FFFFFF": 15,
  "#94A3B8": 109,
  "#64748B": 66,
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

// --- Depth-level surface tokens (DESIGN.md "Gauntlet dark-surface projection") --

/** The DESIGN.md surface depth levels: workspace field down to muted structure. */
export type SurfaceToken =
  | "surface.workspace"
  | "surface.panel"
  | "surface.elevated"
  | "surface.overlay"
  | "surface.code"
  | "surface.muted";

export interface SurfaceSpec {
  /** Background hex from DESIGN.md's dark-surface projection (DSN-001). */
  readonly background: string;
  /** Containment-border hex: slate at panel depth, lighter at overlay depth. */
  readonly border: string;
}

/**
 * The frozen surface token table. Borders sit at containment boundaries only
 * (DESIGN.md): #334155 for panel/code/workspace depth, #566978 for the
 * elevated/overlay depth.
 */
export const SURFACE_TOKENS: Readonly<Record<SurfaceToken, SurfaceSpec>> = Object.freeze({
  "surface.workspace": {
    background: "#05070A",
    border: "#334155",
  },
  "surface.panel": {
    background: "#0B1220",
    border: "#334155",
  },
  "surface.elevated": {
    background: "#111827",
    border: "#566978",
  },
  "surface.overlay": {
    background: "#0F172A",
    border: "#566978",
  },
  "surface.code": {
    background: "#08101D",
    border: "#334155",
  },
  "surface.muted": {
    background: "#334155",
    border: "#334155",
  },
});

/** A surface lowered for one terminal colour capability (DSN-002). */
export interface ResolvedSurfaceStyle {
  readonly token: SurfaceToken;
  /** Truecolor background hex; null outside truecolor capability. */
  readonly background: string | null;
  /** Truecolor border hex; null outside truecolor capability. */
  readonly border: string | null;
  /** 256-colour background index; null in monochrome. */
  readonly background256: number | null;
  /** 256-colour border index; null in monochrome. */
  readonly border256: number | null;
}

/** Lower one surface token for the terminal's colour capability (DSN-001/002). */
export function resolveSurfaceToken(
  token: SurfaceToken,
  capability: ColorCapability,
): ResolvedSurfaceStyle {
  const spec = SURFACE_TOKENS[token];
  const background256 = COLOR_256_ANCHORS[spec.background] ?? 232;
  const border256 = COLOR_256_ANCHORS[spec.border] ?? 244;
  if (capability === "monochrome") {
    return Object.freeze({
      token,
      background: null,
      border: null,
      background256: null,
      border256: null,
    });
  }
  if (capability === "color256") {
    return Object.freeze({
      token,
      background: null,
      border: null,
      background256,
      border256,
    });
  }
  return Object.freeze({
    token,
    background: spec.background,
    border: spec.border,
    background256,
    border256,
  });
}

// --- Text-role tokens (DESIGN.md fg projection) ---------------------------------

/** Foreground role tokens: content hierarchy, not state (DESIGN.md TEXT table). */
export type TextRoleToken = "fg.primary" | "fg.secondary" | "fg.tertiary";

export interface TextRoleSpec {
  /** Default truecolor hex from DESIGN.md's projection (DSN-001). */
  readonly color: string;
}

/** The frozen foreground role table: primary content, support, metadata. */
export const TEXT_ROLE_TOKENS: Readonly<Record<TextRoleToken, TextRoleSpec>> = Object.freeze({
  "fg.primary": {
    color: "#FFFFFF",
  },
  "fg.secondary": {
    color: "#94A3B8",
  },
  "fg.tertiary": {
    color: "#64748B",
  },
});

/** A text role lowered for one terminal colour capability (DSN-002). */
export interface ResolvedTextRoleStyle {
  readonly token: TextRoleToken;
  /** Truecolor hex; null in monochrome and color256. */
  readonly color: string | null;
  /** 256-colour index; null in monochrome. */
  readonly color256: number | null;
}

/** Lower one foreground role for the terminal's colour capability (DSN-001/002). */
export function resolveTextRoleToken(
  token: TextRoleToken,
  capability: ColorCapability,
): ResolvedTextRoleStyle {
  const spec = TEXT_ROLE_TOKENS[token];
  if (capability === "monochrome") {
    return Object.freeze({ token, color: null, color256: null });
  }
  if (capability === "color256") {
    return Object.freeze({ token, color: null, color256: COLOR_256_ANCHORS[spec.color] ?? 245 });
  }
  return Object.freeze({
    token,
    color: spec.color,
    color256: COLOR_256_ANCHORS[spec.color] ?? null,
  });
}

// --- Terminal capability detection (V6 fix) -------------------------------------

/**
 * Detect the terminal's colour capability (DSN-002; V6 fix — never assume
 * truecolor). Precedence, pinned by theme-capability.test.ts:
 *
 *   1. `NO_COLOR` (non-empty) forces monochrome, even when the terminal
 *      reports RGB;
 *   2. `GAUNTLET_WORKBENCH_COLOR` explicit override
 *      (`truecolor|color256|monochrome`; invalid values are ignored, never
 *      guessed);
 *   3. the renderer's terminal RGB report (`terminalRgb === true`);
 *   4. `COLORTERM` containing `truecolor` or `24bit`;
 *   5. otherwise `color256` — for both `TERM=*-256color` and unknown
 *      terminals. Assuming truecolor over SSH is the V6 defect this repairs.
 */
export function detectColorCapability(
  env: Readonly<Record<string, string | undefined>>,
  terminalRgb?: boolean,
): ColorCapability {
  const noColor = env["NO_COLOR"];
  if (noColor !== undefined && noColor !== "") return "monochrome";
  const override = env["GAUNTLET_WORKBENCH_COLOR"];
  if (override === "truecolor" || override === "color256" || override === "monochrome") {
    return override;
  }
  if (terminalRgb === true) return "truecolor";
  const colorterm = env["COLORTERM"]?.toLowerCase() ?? "";
  if (colorterm.includes("truecolor") || colorterm.includes("24bit")) return "truecolor";
  return "color256";
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

/**
 * Resolve the operator's motion preference from the environment (DSN-013; M15
 * fix — the reduced-motion setting is operator-reachable, not hardcoded).
 *
 * `GAUNTLET_WORKBENCH_MOTION` is the explicit override (`reduced|full`); invalid
 * values are ignored, never guessed. Otherwise `NO_MOTION` (non-empty) reduces.
 * Unset defaults to full: motion is optional, never required for meaning.
 */
export function resolveMotionPreference(
  env: Readonly<Record<string, string | undefined>>,
): MotionPreference {
  const explicit = env["GAUNTLET_WORKBENCH_MOTION"];
  if (explicit === "reduced" || explicit === "full") return explicit;
  const noMotion = env["NO_MOTION"];
  if (noMotion !== undefined && noMotion !== "") return "reduced";
  return "full";
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
