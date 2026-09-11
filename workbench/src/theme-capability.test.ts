import { describe, expect, it } from "bun:test";
import {
  type ColorCapability,
  detectColorCapability,
  type MotionPreference,
  resolveMotionPreference,
  resolveSurfaceToken,
  resolveTextRoleToken,
  resolveToken,
  SEMANTIC_TOKENS,
  SURFACE_TOKENS,
  TEXT_ROLE_TOKENS,
} from "./theme";

/**
 * GUI-redesign increment 2a: DESIGN.md depth-level surface tokens + fg roles,
 * the UESC §23 member set, terminal colour-capability detection (DSN-002, V6
 * fix), and the operator-reachable reduced-motion setting (DSN-013, M15 fix).
 */

describe("UESC §23 member completeness", () => {
  it("every §23 family member is addressable as a semantic token", () => {
    const required = [
      "authority.allowed",
      "authority.denied",
      "authority.escalated",
      "authority.degraded",
      "authority.pending",
      "execution.running",
      "execution.waiting",
      "execution.succeeded",
      "execution.failed",
      "execution.cancelled",
      "settlement.pending",
      "settlement.evaluating",
      "settlement.accepted",
      "settlement.rejected",
      "settlement.escalated",
      "settlement.quarantined",
      "evidence.complete",
      "evidence.missing",
      "evidence.failed",
      "evidence.stale",
      "evidence.quarantined",
      "readiness.ready",
      "readiness.degraded",
      "readiness.blocked",
      "freshness.current",
      "freshness.stale",
      "integrity.verified",
      "integrity.partial",
      "integrity.pending",
      "integrity.failed",
      "capability.attempted",
      "capability.demonstrated",
      "capability.proven",
      "capability.degraded",
      "capability.quarantined",
      "focus.primary",
      "focus.ring",
      "attention.required",
      "danger.destructive",
      "proof.independently_confirmed",
      "proof.sanctioned_authority",
    ] as const;
    for (const token of required) {
      expect(SEMANTIC_TOKENS[token], `missing token ${token}`).toBeDefined();
    }
  });

  it("quarantined members stay distinct tokens even where colours coincide (DSN-004)", () => {
    const tokens = [
      "settlement.quarantined",
      "evidence.quarantined",
      "capability.quarantined",
    ] as const;
    const labels = new Set(tokens.map((t) => SEMANTIC_TOKENS[t].label));
    expect(labels.size).toBe(3);
  });

  it("gold stays reserved: only proof/sanctioned tokens carry brand gold (DSN-003)", () => {
    const gold = "#D7A84A";
    const goldTokens = Object.entries(SEMANTIC_TOKENS)
      .filter(([, spec]) => spec.color === gold)
      .map(([name]) => name);
    expect(goldTokens.sort()).toEqual(
      [
        "authority.sanctioned",
        "proof.independent",
        "proof.independently_confirmed",
        "proof.sanctioned_authority",
      ].sort(),
    );
  });

  it("new members degrade to glyph+label+emphasis in monochrome (DSN-002)", () => {
    for (const token of [
      "freshness.stale",
      "danger.destructive",
      "settlement.quarantined",
    ] as const) {
      const mono = resolveToken(token, "monochrome");
      expect(mono.color).toBeNull();
      expect(mono.glyph.length).toBeGreaterThan(0);
      expect(mono.label.length).toBeGreaterThan(0);
    }
  });
});

describe("DESIGN depth-level surface tokens", () => {
  it("every DESIGN surface token carries bg + border and lowers per capability", () => {
    for (const name of [
      "surface.workspace",
      "surface.panel",
      "surface.elevated",
      "surface.overlay",
      "surface.code",
      "surface.muted",
    ] as const) {
      const spec = SURFACE_TOKENS[name];
      expect(spec, `${name} missing`).toBeDefined();
      expect(spec.background).toMatch(/^#[0-9A-F]{6}$/);
      expect(spec.border).toMatch(/^#[0-9A-F]{6}$/);
      const mono = resolveSurfaceToken(name, "monochrome");
      expect(mono.background).toBeNull();
      expect(mono.border).toBeNull();
      const c256 = resolveSurfaceToken(name, "color256");
      expect(c256.background256).not.toBeNull();
      expect(c256.border256).not.toBeNull();
      const tc = resolveSurfaceToken(name, "truecolor");
      expect(tc.background).toBe(spec.background);
    }
  });

  it("fg roles exist and lower (DESIGN TEXT projection)", () => {
    for (const name of ["fg.primary", "fg.secondary", "fg.tertiary"] as const) {
      expect(TEXT_ROLE_TOKENS[name], `${name} missing`).toBeDefined();
      expect(resolveTextRoleToken(name, "monochrome").color).toBeNull();
      expect(resolveTextRoleToken(name, "truecolor").color).toBe(TEXT_ROLE_TOKENS[name].color);
    }
  });
});

describe("colour capability detection (V6 fix: never assume truecolor)", () => {
  const cases: readonly [
    string,
    Record<string, string | undefined>,
    boolean | undefined,
    ColorCapability,
  ][] = [
    ["NO_COLOR forces monochrome", { NO_COLOR: "1" }, true, "monochrome"],
    [
      "terminal rgb report wins when NO_COLOR absent",
      { COLORTERM: "truecolor" },
      true,
      "truecolor",
    ],
    ["no rgb, COLORTERM truecolor", { COLORTERM: "truecolor" }, false, "truecolor"],
    ["no rgb, 256color TERM", { TERM: "xterm-256color" }, false, "color256"],
    [
      "no rgb, plain TERM defaults to 256 (never assumed truecolor)",
      { TERM: "xterm" },
      false,
      "color256",
    ],
    ["empty env defaults to color256", {}, false, "color256"],
    [
      "explicit capability override",
      { GAUNTLET_WORKBENCH_COLOR: "monochrome" },
      true,
      "monochrome",
    ],
  ];

  for (const [name, env, terminalRgb, expected] of cases) {
    it(name, () => {
      expect(detectColorCapability(env, terminalRgb)).toBe(expected);
    });
  }
});

describe("reduced-motion setting (DSN-013, M15 fix)", () => {
  const cases: readonly [string, Record<string, string | undefined>, MotionPreference][] = [
    ["explicit reduced", { GAUNTLET_WORKBENCH_MOTION: "reduced" }, "reduced"],
    ["explicit full", { GAUNTLET_WORKBENCH_MOTION: "full" }, "full"],
    ["unset defaults to full", {}, "full"],
    ["invalid value is ignored, not guessed", { GAUNTLET_WORKBENCH_MOTION: "spinning" }, "full"],
    ["NO_MOTION=1 also reduces", { NO_MOTION: "1" }, "reduced"],
  ];

  for (const [name, env, expected] of cases) {
    it(name, () => {
      expect(resolveMotionPreference(env)).toBe(expected);
    });
  }
});
