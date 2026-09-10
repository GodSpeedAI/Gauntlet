/**
 * T23 cross-surface equivalence — the FROZEN representative matrix EXECUTED
 * (plan T23; REQ-AFF-001/002/003, REQ-OPS-002/004, REQ-SEC-002, REQ-UX-003,
 * REQ-VERIFY-003/004; prereg D1/D3, P1/P2, transfer tests L1/L2).
 *
 * D1 FREEZE (written before builder-evidence inspection; see the evidence
 * report's section 1): the representatives, their preconditions, and the
 * expected canonical identities/results are the VISIBLE CONSTANTS below. The
 * test executes them through EVERY route in the T23 registry
 * (testing/cross-surface-matrix.ts) and pins the routing/dependency facts that
 * make "no bypass" structural rather than observed — including a WHOLE-TREE
 * import pin on the ONE gateway module, so a new unregistered importer fails
 * this suite by construction (Correction Round 1 MAJOR-1).
 *
 * Gate: `just tui-test cross-surface-equivalence`.
 */

import { describe, expect, it } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { FocusRef } from "./focus";
import type { CanonicalOutcome, GatewayOutcome } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import {
  allowedRetryRequest,
  CrossSurfaceHarness,
  DIRECT_FAMILY_ROUTES,
  deniedResolveRequest,
  GATEWAY_FAMILY_ROUTES,
  type MatrixRouteId,
  NON_INVOCATION_SURFACES,
  normalizeRouteOutcome,
  ROUTE_REGISTRY,
  T23_ACTOR,
  T23_DENY_UNIT,
  T23_RETRY_TICKET,
  T23_RETRY_UNIT,
  T23_RUN,
} from "./testing/cross-surface-matrix";

// --- THE FROZEN REPRESENTATIVE MATRIX (prereg D1 — visible constants) -----------

/** ALLOWED: retry of the failed unit; the precondition genuinely holds at invoke time. */
const FROZEN_ALLOWED = Object.freeze({
  operation: OPERATION_IDS.UNIT_RETRY,
  target: T23_RETRY_UNIT,
  precondition: "unit status 'fail', settlement authority granted, durable run anchor",
  expected_operation: OPERATION_IDS.UNIT_RETRY,
  expected_facts: Object.freeze({
    run_id: T23_RUN,
    unit_id: T23_RETRY_UNIT,
    tickets_resolved: Object.freeze([T23_RETRY_TICKET]),
    committed_now: true,
  }),
  expected_settlements_total: 1,
});

/** DENIED: resolve of the escalation after a POST-RENDER authority revocation. */
const FROZEN_DENIED = Object.freeze({
  operation: OPERATION_IDS.UNIT_RESOLVE,
  target: T23_DENY_UNIT,
  denial_setup: "authority revoked AFTER the affordance was rendered (post-render change)",
  canonical_code: "authority_not_permitted",
  reason_codes: Object.freeze(["authority_not_permitted"]),
  expected_settlements_total: 0,
  expected_tickets_total: 0,
});

/** STALE CONTROL: the allowed affordance presented after its precondition stopped holding. */
const FROZEN_STALE = Object.freeze({
  operation: OPERATION_IDS.UNIT_RETRY,
  advance: "canonical unit status advanced 'fail' -> 'pass' after render",
  canonical_code: "readiness_not_earned",
  expected_settlements_total: 0,
  expected_tickets_total: 0,
});

/** Denial-reason variation (prereg parametric L2): the canonical code per reason class. */
const FROZEN_REASON_VARIATION: Readonly<
  Record<"governance" | "payment" | "settlement_access" | "stale_run", string>
> = Object.freeze({
  governance: "blocking_residual_open",
  payment: "budget_exhausted",
  settlement_access: "actor_authentication_required",
  stale_run: "run_not_durable",
});

const ALL_ROUTES: readonly MatrixRouteId[] = Object.freeze([
  ...ROUTE_REGISTRY.map((entry) => entry.id),
]);

const RETRY_FOCUS = FocusRef.for("gauntlet", "WorkItem", T23_RETRY_UNIT);
const DENY_FOCUS = FocusRef.for("gauntlet", "WorkItem", T23_DENY_UNIT);

/** The comparable denial shape (route and raw-key fields stripped). */
interface DenialShape {
  readonly kind: string;
  readonly operation: string | null;
  readonly canonical_code: string | null;
  readonly reason_codes: readonly string[];
  readonly denied_pre_admission: boolean;
  readonly accepted_facts: unknown;
}

function denialOf(result: { readonly outcome: GatewayOutcome | CanonicalOutcome }): DenialShape {
  const normalized = normalizeRouteOutcome("gateway-http", result.outcome);
  return {
    kind: normalized.kind,
    operation: normalized.operation,
    canonical_code: normalized.canonical_code,
    reason_codes: [...normalized.reason_codes],
    denied_pre_admission: normalized.denied_pre_admission,
    accepted_facts: normalized.accepted_facts,
  };
}

/** The gateway-family denial shape: the workbench refused BEFORE canonical admission. */
function expectedGatewayDenial(operation: string, canonicalCode: string): DenialShape {
  return {
    kind: "refused",
    operation,
    canonical_code: canonicalCode,
    reason_codes: [canonicalCode],
    denied_pre_admission: true,
    accepted_facts: null,
  };
}

/** The raw surface denial shape: the refusal happened AT the write boundary. */
function expectedDirectDenial(canonicalCode: string): DenialShape {
  return {
    kind: "refused",
    operation: null,
    canonical_code: canonicalCode,
    reason_codes: [canonicalCode],
    denied_pre_admission: false,
    accepted_facts: null,
  };
}

/** The denial EVERY route must present when the denial races past revalidation. */
function expectedUniformDenial(operation: string | null, canonicalCode: string): DenialShape {
  return {
    kind: "refused",
    operation,
    canonical_code: canonicalCode,
    reason_codes: [canonicalCode],
    denied_pre_admission: false,
    accepted_facts: null,
  };
}

function expectZeroConsequentialEffects(
  harness: CrossSurfaceHarness,
  admissionAttempts: number | "any",
): void {
  const effects = harness.sideEffects();
  expect(effects.settlements_committed).toBe(0);
  expect(effects.tickets_resolved).toBe(0);
  if (admissionAttempts !== "any") {
    expect(effects.admission_attempts).toBe(admissionAttempts);
  }
}

// --- The executed matrix ---------------------------------------------------------

describe("T23 Cross-Surface Equivalence — frozen representative matrix (REQ-AFF-001..003, REQ-OPS-002/004, REQ-SEC-002, REQ-UX-003, REQ-VERIFY-003/004)", () => {
  it("the registry inventories every currently implemented invocation route", () => {
    expect(ROUTE_REGISTRY.map((entry) => entry.id)).toEqual([
      "gateway-http",
      "gateway-shim",
      "direct-api",
      "direct-cli",
      "kernel-callable",
      "plugin-projection",
      "prime-delegation",
      "shell-effect",
    ]);
    expect(ALL_ROUTES.length).toBe(GATEWAY_FAMILY_ROUTES.length + DIRECT_FAMILY_ROUTES.length);
  });

  it("(a) the ALLOWED representative converges on ONE canonical operation/result on every route", async () => {
    for (const route of ALL_ROUTES) {
      const harness = CrossSurfaceHarness.build();
      try {
        // Render first: the affordance the operator was shown.
        const rendered = harness.renderDescriptor(FROZEN_ALLOWED.operation, RETRY_FOCUS, {
          session: { actor: T23_ACTOR, authenticated: true },
        });
        expect(rendered.status).toBe("available");

        const result = await harness.drive(route, allowedRetryRequest());
        const normalized = normalizeRouteOutcome(route, result.outcome);
        expect(normalized.kind).toBe("accepted");
        // The raw surface shapes carry the operation by position (kind + unit),
        // not as a wire field — their normalized operation is honestly null.
        expect(normalized.operation).toBe(
          GATEWAY_FAMILY_ROUTES.includes(route) ? FROZEN_ALLOWED.expected_operation : null,
        );
        expect(normalized.accepted_facts).toEqual(FROZEN_ALLOWED.expected_facts);
        // One canonical admission body: the settlement happened once, total.
        const effects = harness.sideEffects();
        expect(effects.settlements_committed).toBe(FROZEN_ALLOWED.expected_settlements_total);
        expect(effects.tickets_resolved).toBe(1);
        if (route === "prime-delegation") {
          // The delegation route's own Prime consequence: exactly one child
          // behind the ONE accepted canonical drive (never a second settlement).
          expect(result.prime_child_spawned).toBe(true);
          expect(result.prime_sessions_total).toBe(1);
        }
      } finally {
        harness.close();
      }
    }
  });

  it("(b) the DENIED representative after post-render authority revocation is the SAME typed denial with zero side effects on every route", async () => {
    for (const route of ALL_ROUTES) {
      const gatewayFamily = GATEWAY_FAMILY_ROUTES.includes(route);
      // Visible revocation: the fresh revalidation sees it — workbench routes
      // deny BEFORE admission; the raw shapes deny AT the write boundary.
      const visible = CrossSurfaceHarness.build().addResolveTarget();
      try {
        const rendered = visible.renderDescriptor(FROZEN_DENIED.operation, DENY_FOCUS, {
          session: { actor: T23_ACTOR, authenticated: true },
        });
        expect(rendered.status).toBe("requires_attention");

        visible.revokeAuthority(T23_DENY_UNIT, true);
        const result = await visible.drive(route, deniedResolveRequest());
        expect(denialOf(result)).toEqual(
          gatewayFamily
            ? expectedGatewayDenial(FROZEN_DENIED.operation, FROZEN_DENIED.canonical_code)
            : expectedDirectDenial(FROZEN_DENIED.canonical_code),
        );
        expectZeroConsequentialEffects(visible, gatewayFamily ? 0 : 1);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
          expect(result.prime_sessions_total).toBe(0);
        }
        if (route === "kernel-callable") {
          // The stale denial retires the WRAPPER (projection consequence) —
          // the canonical denial itself is untouched by the projection.
          expect(result.kernel_wrapper_invalidated).toBe(true);
        }
      } finally {
        visible.close();
      }

      // The RACING denial: the revocation is invisible to reads, so it can only
      // be caught at canonical admission — and is, on EVERY route, identically.
      const racing = CrossSurfaceHarness.build().addResolveTarget();
      try {
        racing.revokeAuthority(T23_DENY_UNIT, false);
        const result = await racing.drive(route, deniedResolveRequest());
        expect(denialOf(result)).toEqual(
          expectedUniformDenial(
            gatewayFamily ? FROZEN_DENIED.operation : null,
            FROZEN_DENIED.canonical_code,
          ),
        );
        expectZeroConsequentialEffects(racing, 1);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
          expect(result.prime_sessions_total).toBe(0);
        }
      } finally {
        racing.close();
      }
    }
  });

  it("(c) the STALE control is denied before side effect on every route", async () => {
    for (const route of ALL_ROUTES) {
      const harness = CrossSurfaceHarness.build();
      try {
        const rendered = harness.renderDescriptor(FROZEN_STALE.operation, RETRY_FOCUS, {
          session: { actor: T23_ACTOR, authenticated: true },
        });
        expect(rendered.status).toBe("available");

        // Canonical state advances AFTER render; the captured affordance is stale.
        harness.setUnitStatus(T23_RETRY_UNIT, "pass");
        const result = await harness.drive(route, allowedRetryRequest());
        const denial = denialOf(result);
        expect(denial.kind).toBe("refused");
        expect(denial.canonical_code).toBe(FROZEN_STALE.canonical_code);
        expect(denial.denied_pre_admission).toBe(GATEWAY_FAMILY_ROUTES.includes(route));
        expectZeroConsequentialEffects(harness, GATEWAY_FAMILY_ROUTES.includes(route) ? 0 : 1);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
          expect(result.prime_sessions_total).toBe(0);
        }
      } finally {
        harness.close();
      }
    }
  });

  it("(P2) ONE idempotency identity across ALL projections yields ONE canonical settlement — side effects total, never per-route", async () => {
    const harness = CrossSurfaceHarness.build();
    try {
      const request = allowedRetryRequest();
      const first = normalizeRouteOutcome(
        "gateway-http",
        (await harness.drive("gateway-http", request)).outcome,
      );
      expect(first.kind).toBe("accepted");
      expect(first.accepted_facts?.committed_now).toBe(true);
      const key = first.idempotency_key ?? "";
      // The deterministic identity: operation:target:actor:payload-digest (REQ-OPS-002).
      expect(key.startsWith(`unit.retry:${T23_RETRY_UNIT}:${T23_ACTOR}:`)).toBe(true);
      const explicit = { ...request, idempotency_key: key };

      // Every remaining projection presents the SAME identity.
      const reentries = [
        ["gateway-shim", await harness.drive("gateway-shim", explicit)],
        ["kernel-callable", await harness.drive("kernel-callable", explicit)],
        ["plugin-projection", await harness.drive("plugin-projection", explicit)],
        ["shell-effect", await harness.drive("shell-effect", explicit)],
      ] as const;
      const prime = await harness.drive("prime-delegation", explicit);
      const api = await harness.drive("direct-api", explicit);
      const cli = await harness.drive("direct-cli", explicit);

      // The host-gateway family re-enters the SAME settlement (§16.3): identical
      // canonical identity; committed_now=false is the durable truth.
      for (const [route, result] of reentries) {
        const normalized = normalizeRouteOutcome(route, result.outcome);
        expect(`${route}:${normalized.kind}`).toBe(`${route}:accepted`);
        expect(normalized.idempotency_key).toBe(key);
        expect(normalized.operation).toBe(FROZEN_ALLOWED.operation);
        expect(normalized.accepted_facts?.run_id).toBe(FROZEN_ALLOWED.expected_facts.run_id);
        expect(normalized.accepted_facts?.unit_id).toBe(FROZEN_ALLOWED.expected_facts.unit_id);
        expect(normalized.accepted_facts?.committed_now).toBe(false);
      }
      // The Prime route: one child spawned behind the ONE canonical re-entry —
      // the settlement did not multiply with the projection.
      const primeNormalized = normalizeRouteOutcome("prime-delegation", prime.outcome);
      expect(primeNormalized.kind).toBe("accepted");
      expect(primeNormalized.idempotency_key).toBe(key);
      expect(primeNormalized.accepted_facts?.committed_now).toBe(false);
      expect(prime.prime_child_spawned).toBe(true);
      expect(prime.prime_sessions_total).toBe(1);
      // The raw API shape re-enters the same durable settlement too.
      const apiNormalized = normalizeRouteOutcome("direct-api", api.outcome);
      expect(apiNormalized.kind).toBe("accepted");
      expect(apiNormalized.accepted_facts?.committed_now).toBe(false);
      // The ticket-anchored CLI shape names a ticket that no longer exists:
      // a formation refusal with zero effects (disclosed, pinned truth).
      const cliNormalized = normalizeRouteOutcome("direct-cli", cli.outcome);
      expect(cliNormalized.kind).toBe("failed");
      expect(cliNormalized.canonical_code).toBe("action_failed");

      // THE no-multiplication assertion: effects are TOTAL across all routes.
      const effects = harness.sideEffects();
      expect(effects.settlements_committed).toBe(1);
      expect(effects.tickets_resolved).toBe(1);

      // Key reuse with a CHANGED payload is refused before routing on the host
      // gateway — zero additional admission attempts (REQ-OPS-002).
      const mismatch = await harness.drive("gateway-shim", {
        ...explicit,
        payload: { note: "CHANGED payload", run: T23_RUN },
      });
      expect(denialOf(mismatch)).toEqual(
        expectedGatewayDenial(FROZEN_ALLOWED.operation, "idempotency_key_payload_mismatch"),
      );
      expect(harness.sideEffects().admission_attempts).toBe(effects.admission_attempts);
    } finally {
      harness.close();
    }
  });

  it("(L2) denial-reason variation is preserved through every projection's refusal", async () => {
    // Governance: an open blocking residual, post-render, every route.
    for (const route of ALL_ROUTES) {
      const harness = CrossSurfaceHarness.build();
      try {
        harness.denyGovernance(T23_RETRY_UNIT, true);
        const result = await harness.drive(route, allowedRetryRequest());
        const denial = denialOf(result);
        expect(denial.kind).toBe("refused");
        expect(denial.canonical_code).toBe(FROZEN_REASON_VARIATION.governance);
        expect(denial.denied_pre_admission).toBe(GATEWAY_FAMILY_ROUTES.includes(route));
        expectZeroConsequentialEffects(harness, "any");
      } finally {
        harness.close();
      }
    }

    // Payment: the routes whose request shape carries estimates refuse with the
    // preserved code (the plugin route carries the projection's OWN estimate).
    const estimateCarrying: readonly MatrixRouteId[] = [
      "gateway-http",
      "gateway-shim",
      "plugin-projection",
      "prime-delegation",
      "shell-effect",
    ];
    for (const route of estimateCarrying) {
      const harness = CrossSurfaceHarness.build();
      try {
        harness.exhaustBudget(true);
        const request = allowedRetryRequest({
          resolution: {
            payment_estimates: {
              [FROZEN_ALLOWED.operation]: { tokens: 2_000_000, cost_usd_micro: 6_000_000 },
            },
          },
        });
        const result = await harness.drive(route, request);
        const denial = denialOf(result);
        expect(denial.kind).toBe("refused");
        expect(denial.canonical_code).toBe(FROZEN_REASON_VARIATION.payment);
        expect(denial.denied_pre_admission).toBe(true);
        expectZeroConsequentialEffects(harness, 0);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
        }
      } finally {
        harness.close();
      }
    }
    // Honest disclosure, pinned statically below: the kernel callable and the
    // raw surface shapes carry NO estimate channel — payment is a resolver
    // policy and the canonical admission has no affordability gate. Not a
    // rewritten refusal; an absent one.

    // Settlement access: no session presented — the resolver and the transport
    // boundaries preserve the canonical auth code; the raw CLI shape preserves
    // the surface's own operator-identity code.
    for (const route of GATEWAY_FAMILY_ROUTES) {
      const harness = CrossSurfaceHarness.build().addResolveTarget();
      try {
        const result = await harness.drive(route, deniedResolveRequest({ session: null }));
        const denial = denialOf(result);
        expect(denial.kind).toBe("refused");
        expect(denial.canonical_code).toBe(FROZEN_REASON_VARIATION.settlement_access);
        expect(denial.denied_pre_admission).toBe(true);
        expectZeroConsequentialEffects(harness, 0);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
        }
      } finally {
        harness.close();
      }
    }
    {
      // The direct API shape across its transport boundary (no credentials).
      const harness = CrossSurfaceHarness.build().addResolveTarget();
      try {
        const outcome = await harness.rawApiTransport({
          operation: OPERATION_IDS.UNIT_RESOLVE,
          action_kind: "resolve",
          unit: T23_DENY_UNIT,
          operator: T23_ACTOR,
          session_token: "",
          note: "unauthenticated direct API drive",
          run: T23_RUN,
          payload_digest: "irrelevant-at-the-wire",
          idempotency_key: "t23-settlement-access-probe",
        });
        const normalized = normalizeRouteOutcome("direct-api", outcome);
        expect(normalized.kind).toBe("refused");
        expect(normalized.canonical_code).toBe(FROZEN_REASON_VARIATION.settlement_access);
        expectZeroConsequentialEffects(harness, 0);
      } finally {
        harness.close();
      }
      // The direct CLI shape requires the operator's identity (spec 16.1).
      const cli = CrossSurfaceHarness.build();
      try {
        const result = await cli.drive("direct-cli", deniedResolveRequest({ session: null }));
        const denial = denialOf(result);
        expect(denial.kind).toBe("refused");
        expect(denial.canonical_code).toBe("operator_identity_required");
        expectZeroConsequentialEffects(cli, 1);
      } finally {
        cli.close();
      }
    }

    // Stale run: the anchor the projections held is no longer durable — the
    // canonical refusal is IDENTICAL on every route, including the flag that
    // it happened at admission (the resolver honestly has no run knowledge).
    for (const route of ALL_ROUTES) {
      const harness = CrossSurfaceHarness.build();
      try {
        harness.retireRun();
        const result = await harness.drive(route, allowedRetryRequest());
        expect(denialOf(result)).toEqual(
          expectedUniformDenial(
            GATEWAY_FAMILY_ROUTES.includes(route) ? FROZEN_ALLOWED.operation : null,
            FROZEN_REASON_VARIATION.stale_run,
          ),
        );
        expectZeroConsequentialEffects(harness, 1);
        if (route === "prime-delegation") {
          expect(result.prime_child_spawned).toBe(false);
          expect(result.prime_sessions_total).toBe(0);
        }
      } finally {
        harness.close();
      }
    }
  });

  it("no reason variation is rewritten into a generic error anywhere", () => {
    // The refusal vocabulary the routes preserve is the canonical surface's own;
    // a generic catch-all code would be a projection rewriting a denial.
    const generic = new Set(["surface_error", "error", "unknown", "internal_error"]);
    for (const code of Object.values(FROZEN_REASON_VARIATION)) {
      expect(generic.has(code)).toBe(false);
    }
  });
});

// --- Static routing inspection (the structural half of the no-bypass claim) ------

const PRESENTATION_LEAVES = new Set(["theme.ts", "geometry.ts", "render-tree.ts"]);
const PRESENTATION_PACKAGES = ["@opentui/", "react"];
/**
 * The admitted importer set of the ONE gateway module, keyed by path relative
 * to src/. Enforced by a WHOLE-TREE scan (every .ts/.tsx under src/ is
 * enumerated and classified — Correction Round 1 MAJOR-1 replaced a fixed
 * candidate list that was blind to new files), so a NEW unregistered importer
 * fails this suite by construction, and an allowlisted file that vanished
 * fails too. Test/bench files are the exempt driver class: they legitimately
 * drive the gateway without being seams.
 */
const GATEWAY_IMPORT_ALLOWLIST: readonly string[] = Object.freeze([
  "index.ts",
  "kernel-affordances.ts",
  "prime-recursion.ts",
  "security-hardening.ts",
  "testing/canonical-surface-stub.ts",
  "testing/cross-surface-matrix.ts",
]);

/** Test and benchmark files legitimately drive the gateway; they are not seams. */
const DRIVER_FILE = /\.(test|bench)\.tsx?$/;
const GATEWAY_MODULE_LEAF = "operation-gateway.ts";

function srcDir(): string {
  return import.meta.dir;
}

function readSrc(relative: string): string {
  return readFileSync(join(srcDir(), relative), "utf8");
}

/** Every .ts/.tsx file under src/, recursively, as paths relative to src/. */
function enumerateSrcFiles(): readonly string[] {
  const files: string[] = [];
  const walk = (relative: string): void => {
    for (const entry of readdirSync(join(srcDir(), relative), { withFileTypes: true })) {
      const path = relative === "" ? entry.name : `${relative}/${entry.name}`;
      if (entry.isDirectory()) {
        walk(path);
      } else if (/\.(ts|tsx)$/.test(entry.name)) {
        files.push(path);
      }
    }
  };
  walk("");
  return files.sort();
}

/**
 * Whether a file's import text reaches the ONE gateway module: by relative
 * import in ANY form (static, dynamic, side-effect, type-only, star
 * re-export), or by binding the OperationGateway symbol from ANY module
 * (closing the barrel re-export bypass).
 */
function importsGatewayModule(text: string): boolean {
  const byModule = relativeSpecifiers(text).some(
    (spec) => spec.split("/").pop() === GATEWAY_MODULE_LEAF,
  );
  const bySymbol = /(?:import|export)\s+(?:type\s+)?\{[^}]*\bOperationGateway\b[^}]*\}\s*from/.test(
    text,
  );
  return byModule || bySymbol;
}

function relativeSpecifiers(text: string): readonly string[] {
  const specifiers = new Set<string>();
  for (const match of text.matchAll(/(?:import|export)[^"';]*?from\s*["']([^"']+)["']/g)) {
    specifiers.add(match[1] ?? "");
  }
  for (const match of text.matchAll(/import\s*\(\s*["']([^"']+)["']/g)) {
    specifiers.add(match[1] ?? "");
  }
  for (const match of text.matchAll(/import\s*["']([^"']+)["']/g)) {
    specifiers.add(match[1] ?? "");
  }
  return [...specifiers].filter((spec) => spec.startsWith("."));
}

function countOccurrences(haystack: string, needle: string): number {
  let count = 0;
  let index = haystack.indexOf(needle);
  while (index !== -1) {
    count += 1;
    index = haystack.indexOf(needle, index + needle.length);
  }
  return count;
}

describe("T23 Static routing inspection — one canonical admission body, no projection authority, renderer ban intact", () => {
  it("WHOLE-TREE: only consciously registered, still-present files import the ONE gateway", () => {
    const universe = enumerateSrcFiles();
    // An allowlisted file that vanished fails the pin too: future refactors
    // cannot silently strand the allowlist.
    for (const allowed of GATEWAY_IMPORT_ALLOWLIST) {
      expect(existsSync(join(srcDir(), allowed))).toBe(true);
    }
    const importers: string[] = [];
    const offenders: string[] = [];
    for (const relative of universe) {
      if (DRIVER_FILE.test(relative)) {
        continue;
      }
      if (importsGatewayModule(readSrc(relative))) {
        importers.push(relative);
        if (!GATEWAY_IMPORT_ALLOWLIST.includes(relative)) {
          offenders.push(relative);
        }
      }
    }
    // ANY unregistered importer anywhere in the tree fails, by construction.
    expect(offenders).toEqual([]);
    // The landed seams are still bound, and the presentation projections hold
    // NO gateway: plugins present, the host admits; the shell emits effects,
    // the host admits.
    expect(importers).toContain("kernel-affordances.ts");
    expect(importers).toContain("prime-recursion.ts");
    expect(importers).not.toContain("plugins.ts");
    expect(importers).not.toContain("shell.ts");
  });

  it("only the two landed transports implement OperatorActionRoute (no fourth admission)", () => {
    // Textual class-implementor pin, with Correction Round 1's disclosure:
    // route VALUES can also arise without `implements` — the stub's
    // object-literal shim (asInProcessRoute) and security-hardening's
    // observe-deadline route decorator both yield OperatorActionRoute values.
    // Neither is a separate authority (both dispatch into the ONE canonical
    // route); the structural pins below (single gateway call sites, single
    // decide body) cover them.
    const implementors = new Set<string>();
    for (const name of [
      "operation-gateway.ts",
      "prime-recursion.ts",
      "kernel-affordances.ts",
      "security-hardening.ts",
      "testing/canonical-surface-stub.ts",
    ]) {
      if (readSrc(name).includes("implements OperatorActionRoute")) {
        implementors.add(name.split("/").pop() ?? name);
      }
    }
    expect([...implementors].sort()).toEqual(["operation-gateway.ts", "prime-recursion.ts"]);
  });

  it("the kernel callable and the Prime dispatcher route through the gateway EXACTLY once — no second admission, no hidden channel", () => {
    const kernel = readSrc("kernel-affordances.ts");
    expect(countOccurrences(kernel, "this.gateway.invoke(")).toBe(1);
    expect(kernel.includes(".perform(")).toBe(false);
    // The kernel request shape structurally drops payment estimates: the
    // payment denial is honestly ABSENT there, never rewritten.
    expect(kernel.includes("resolution")).toBe(false);

    const prime = readSrc("prime-recursion.ts");
    expect(countOccurrences(prime, "this.gateway.invokeVia(")).toBe(1);
    // The delegation route dispatches into the INNER canonical route exactly
    // once, before any Prime side effect.
    expect(countOccurrences(prime, "this.inner.perform(command")).toBe(1);

    // Plugin projections resolve (predict) but have NO invocation counterpart.
    const plugins = readSrc("plugins.ts");
    expect(plugins.includes(".perform(")).toBe(false);
    expect(plugins.includes("gateway.invoke")).toBe(false);

    // The shell's only outbound edge is the interaction callback.
    const shell = readSrc("shell.ts");
    expect(shell.includes(".perform(")).toBe(false);
    expect(shell.includes("gateway.invoke")).toBe(false);
  });

  it("every direct surface shape terminates in the ONE decision body with the only settlement increment", () => {
    const stub = readSrc("testing/canonical-surface-stub.ts");
    expect(countOccurrences(stub, "return this.decide(")).toBe(2);
    expect(countOccurrences(stub, "settlements_committed += 1")).toBe(1);
    expect(countOccurrences(stub, "admission_attempts += 1")).toBe(2);
    const gateway = readSrc("operation-gateway.ts");
    expect(countOccurrences(gateway, "/api/v1/units/")).toBe(1);
  });

  it("the T19 renderer-boundary import ban still holds for every admission-side route module", () => {
    const queue = [
      "operation-gateway.ts",
      "kernel-affordances.ts",
      "prime-recursion.ts",
      "testing/canonical-surface-stub.ts",
    ];
    const visited = new Set<string>();
    while (queue.length > 0) {
      const name = queue.pop();
      if (name === undefined || visited.has(name)) {
        continue;
      }
      visited.add(name);
      for (const spec of relativeSpecifiers(readSrc(name))) {
        const leaf = spec.split("/").pop() ?? "";
        expect(PRESENTATION_LEAVES.has(leaf)).toBe(false);
        for (const banned of PRESENTATION_PACKAGES) {
          expect(spec.startsWith(banned)).toBe(false);
        }
        if (leaf.endsWith(".ts")) {
          queue.push(spec.startsWith("..") ? `testing/${leaf}` : leaf);
        }
      }
    }
    expect(visited.has("operation-gateway.ts")).toBe(true);
    expect(visited.has("testing/canonical-surface-stub.ts")).toBe(true);
  });

  it("the honest non-routes are recorded, not forced", () => {
    expect(NON_INVOCATION_SURFACES.map((surface) => surface.id)).toEqual([
      "release-host-loop",
      "read-client",
    ]);
    for (const surface of NON_INVOCATION_SURFACES) {
      expect(surface.not_an_invocation_route_because.length).toBeGreaterThan(40);
    }
    // The release form builds, launches, and qualifies — it never invokes.
    const release = readSrc("release-manifest.ts");
    expect(release.includes("OperatorActionRoute")).toBe(false);
    expect(release.includes(".perform(")).toBe(false);
    // The client is a read surface whose ONLY POST is the T18 auth probe,
    // aimed at the SAME canonical action endpoint the gateway route drives.
    const client = readSrc("client.ts");
    expect(client.includes("Emits zero mutations")).toBe(true);
    expect(countOccurrences(client, 'method: "POST"')).toBe(1);
    expect(client.includes("mutateUnit")).toBe(true);
  });
});
