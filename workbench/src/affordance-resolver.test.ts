import { describe, expect, it } from "bun:test";
import {
  type AffordanceBudgetObservation,
  type AffordanceEscalationObservation,
  type AffordanceObservation,
  type AffordanceUnitObservation,
  EMPTY_AFFORDANCE_OBSERVATION,
  type PaymentEstimate,
  resolveAffordance,
  statusPermitsInvocation,
} from "./affordance";
import { FocusRef } from "./focus";
import { lookupOperation, OPERATION_IDS, operationIds } from "./operation-registry";
import type { CapabilityObservation } from "./workbench-model";

const SESSION = { actor: "operator-alice", authenticated: true } as const;
const FOCUS_AK901 = FocusRef.for("gauntlet", "WorkItem", "AK-901");
const FOCUS_AK902 = FocusRef.for("gauntlet", "WorkItem", "AK-902");
const FOCUS_AK903 = FocusRef.for("gauntlet", "WorkItem", "AK-903");

const HEALTHY_BUDGET: AffordanceBudgetObservation = Object.freeze({
  usd_micro: 5_000_000,
  tokens: 1_000_000,
});

function baseObservation(): AffordanceObservation {
  const units: readonly AffordanceUnitObservation[] = [
    Object.freeze({ unit_id: "AK-901", status: "blocked" }),
    Object.freeze({ unit_id: "AK-902", status: "fail" }),
    Object.freeze({ unit_id: "AK-903", status: "pass" }),
  ];
  const escalations: readonly AffordanceEscalationObservation[] = [
    Object.freeze({ ticket_id: "ESC-901", unit_id: "AK-901" }),
  ];
  return Object.freeze({
    reachable: true,
    degraded: false,
    units,
    open_escalations: escalations,
    budget_remaining: HEALTHY_BUDGET,
    governance_denied_units: Object.freeze([] as string[]),
    authority_revoked_units: Object.freeze([] as string[]),
    capabilities: Object.freeze([] as readonly CapabilityObservation[]),
  });
}

function observationWith(overrides: Partial<AffordanceObservation>): AffordanceObservation {
  return Object.freeze({ ...baseObservation(), ...overrides });
}

describe("T09 Affordance Resolver (REQ-AFF-001..005, REQ-OPS-003, REQ-CONFIG-011, DSN-005/009)", () => {
  it("resolves resolve into requires_attention with the escalation as the required attention item", () => {
    const descriptor = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS_AK901,
      baseObservation(),
      {
        session: SESSION,
      },
    );
    expect(descriptor.operation).toBe("unit.resolve");
    expect(descriptor.focus).toBe(FOCUS_AK901);
    expect(descriptor.status).toBe("requires_attention");
    expect(descriptor.required_attention_item).toBe("ESC-901");
    expect(descriptor.reason_codes).toEqual([]);
    expect(descriptor.id).toBe("affordance:unit.resolve:focus:gauntlet:WorkItem:AK-901");
    expect(statusPermitsInvocation(descriptor.status)).toBe(true);
  });

  it("resolves retry into available for a failed unit; the descriptor is frozen and immutable", () => {
    const descriptor = resolveAffordance(OPERATION_IDS.UNIT_RETRY, FOCUS_AK902, baseObservation(), {
      session: SESSION,
    });
    expect(descriptor.status).toBe("available");
    expect(descriptor.reason_codes).toEqual([]);
    expect(Object.isFrozen(descriptor)).toBe(true);
    expect(Object.isFrozen(descriptor.reason_codes)).toBe(true);
  });

  it("keeps unavailable affordances inspectable with machine-readable reasons (REQ-AFF-005)", () => {
    const denied = observationWith({
      reachable: false,
      governance_denied_units: ["AK-901"],
      authority_revoked_units: ["AK-901"],
    });
    const descriptor = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS_AK901, denied, {
      session: undefined,
      payment_estimates: { "unit.resolve": { tokens: 2_000_000, cost_usd_micro: 9_000_000 } },
    });
    // The resolver RETURNS the descriptor — it never discards an unavailable affordance.
    expect(descriptor.status).toBe("unavailable");
    for (const reason of [
      "surface_unreachable" as const,
      "actor_authentication_required" as const,
      "authority_not_permitted" as const,
      "blocking_residual_open" as const,
      "budget_exhausted" as const,
    ]) {
      expect(descriptor.reason_codes).toContain(reason);
    }
    expect(statusPermitsInvocation(descriptor.status)).toBe(false);
  });

  it("treats installed capability as inventory only — never implies available (REQ-AFF-004)", () => {
    const fullyInstalled: readonly CapabilityObservation[] = [
      Object.freeze({
        capability: "terminal",
        installed: true,
        gates: Object.freeze({
          visibility: true,
          reachability: true,
          payment: true,
          governance: true,
          settlement_access: true,
          recovery: true,
        }),
      }),
    ];
    // AK-903 has no open escalation, so resolve is canonically unavailable
    // even with a fully-installed, fully-passing capability in the inventory.
    const withCapability = observationWith({ capabilities: fullyInstalled });
    const descriptor = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS_AK903, withCapability, {
      session: SESSION,
    });
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toContain("no_open_ticket");
    // Even the empty observation with installed capabilities stays unavailable.
    const offline = Object.freeze({
      ...EMPTY_AFFORDANCE_OBSERVATION,
      capabilities: fullyInstalled,
    });
    const offlineDescriptor = resolveAffordance(OPERATION_IDS.UNIT_RETRY, FOCUS_AK902, offline, {
      session: SESSION,
    });
    expect(offlineDescriptor.status).toBe("unavailable");
    expect(offlineDescriptor.reason_codes).toContain("surface_unreachable");
  });

  it("resolves degraded reads to the degraded status and fails closed for invocation", () => {
    const degraded = observationWith({ degraded: true });
    const descriptor = resolveAffordance(OPERATION_IDS.UNIT_RETRY, FOCUS_AK902, degraded, {
      session: SESSION,
    });
    expect(descriptor.status).toBe("degraded");
    expect(descriptor.reason_codes).toEqual(["observation_degraded"]);
    expect(statusPermitsInvocation(descriptor.status)).toBe(false);
  });

  it("renders operations outside the registry unsupported — never a fallback (REQ-OPS-003)", () => {
    const descriptor = resolveAffordance("launch", FOCUS_AK901, baseObservation(), {
      session: SESSION,
    });
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toEqual(["unsupported_operation"]);
    expect(descriptor.operation).toBe("launch");
    expect(lookupOperation("launch")).toBeNull();
    expect(lookupOperation("session.create")).toBeNull();
    // The catalog carries exactly the canonical surface's own operations.
    expect(operationIds()).toEqual(["unit.resolve", "unit.retry", "state.refresh"]);
  });

  it("gates resolve and retry on the canonical preconditions observable from reads", () => {
    // resolve without an open escalation on the target: canonical no_open_ticket.
    const resolveWithoutTicket = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS_AK902,
      baseObservation(),
      { session: SESSION },
    );
    expect(resolveWithoutTicket.status).toBe("unavailable");
    expect(resolveWithoutTicket.reason_codes).toEqual(["no_open_ticket"]);
    // retry without a failed verification: canonical readiness_not_earned.
    for (const focus of [FOCUS_AK901, FOCUS_AK903]) {
      const retryWithoutReadiness = resolveAffordance(
        OPERATION_IDS.UNIT_RETRY,
        focus,
        baseObservation(),
        { session: SESSION },
      );
      expect(retryWithoutReadiness.status).toBe("unavailable");
      expect(retryWithoutReadiness.reason_codes).toEqual(["readiness_not_earned"]);
    }
    // An unauthenticated session is a settlement-access failure.
    const unauthenticated = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS_AK901,
      baseObservation(),
      { session: { actor: "operator-alice", authenticated: false } },
    );
    expect(unauthenticated.reason_codes).toContain("actor_authentication_required");
    expect(unauthenticated.status).toBe("unavailable");
  });

  it("evaluates payment honestly: exceeding refuses, within passes, absent constraint is absence", () => {
    const estimate: PaymentEstimate = { tokens: 2_000_000, cost_usd_micro: 6_000_000 };
    const unaffordable = resolveAffordance(
      OPERATION_IDS.UNIT_RETRY,
      FOCUS_AK902,
      baseObservation(),
      {
        session: SESSION,
        payment_estimates: { "unit.retry": estimate },
      },
    );
    expect(unaffordable.status).toBe("unavailable");
    expect(unaffordable.reason_codes).toContain("budget_exhausted");

    const affordableEstimate: PaymentEstimate = { tokens: 10, cost_usd_micro: 100 };
    const affordable = resolveAffordance(OPERATION_IDS.UNIT_RETRY, FOCUS_AK902, baseObservation(), {
      session: SESSION,
      payment_estimates: { "unit.retry": affordableEstimate },
    });
    expect(affordable.status).toBe("available");
    expect(affordable.estimated_payment).toEqual(affordableEstimate);

    // No observable budget constraint is honest absence — never invented.
    const noBudget = observationWith({ budget_remaining: null });
    const unconstrained = resolveAffordance(OPERATION_IDS.UNIT_RETRY, FOCUS_AK902, noBudget, {
      session: SESSION,
      payment_estimates: { "unit.retry": estimate },
    });
    expect(unconstrained.status).toBe("available");
  });

  it("derives deterministic, context-stable descriptor identities", () => {
    const first = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS_AK901, baseObservation(), {
      session: SESSION,
    });
    const second = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS_AK901, baseObservation(), {
      session: SESSION,
    });
    expect(first).toEqual(second);
    expect(first.id).toBe(second.id);
    const otherFocus = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS_AK902,
      baseObservation(),
      {
        session: SESSION,
      },
    );
    expect(otherFocus.id).not.toBe(first.id);
    const otherOperation = resolveAffordance(
      OPERATION_IDS.UNIT_RETRY,
      FOCUS_AK901,
      baseObservation(),
      {
        session: SESSION,
      },
    );
    expect(otherOperation.id).not.toBe(first.id);
  });
});

describe("T09 Correction Round 1 (adversarial verification findings)", () => {
  it("C4 — a registered read-shaped operation resolves unavailable with not_action_admitted, never toward available", () => {
    const descriptor = resolveAffordance(
      OPERATION_IDS.STATE_REFRESH,
      FOCUS_AK901,
      baseObservation(),
      {
        session: SESSION,
      },
    );
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toEqual(["not_action_admitted"]);
    expect(descriptor.operation).toBe(OPERATION_IDS.STATE_REFRESH);
    expect(statusPermitsInvocation(descriptor.status)).toBe(false);
  });

  it("C7 — disclosed policy: the resolver's no_open_ticket prediction is STRICTER than the anchored admission shape (operator_action.rs:246-249)", () => {
    // Admission semantics (crates/gauntlet-cli/src/operator_action.rs:246-249):
    // an EXPLICITLY-ANCHORED action proceeds even with zero open tickets —
    // "the settle is the action's effect; the (empty, disclosed) ticket list
    // records that there was nothing to mark"; no_open_ticket (:232) is the
    // UNANCHORED shape's refusal. The workbench projection's resolve control
    // is the UNANCHORED shape, so the resolver predicts strictly: zero open
    // escalations on the target resolve to unavailable/no_open_ticket. This
    // is a disclosed fail-closed resolver policy (an operator who genuinely
    // wants the anchored set-with-nothing-to-mark uses the CLI/API shape),
    // pinned here so the strictness cannot silently drift.
    const descriptor = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS_AK903,
      baseObservation(),
      {
        session: SESSION,
      },
    );
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toEqual(["no_open_ticket"]);
  });
});
