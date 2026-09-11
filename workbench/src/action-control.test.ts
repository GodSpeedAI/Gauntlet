import { describe, expect, it } from "bun:test";
import type { ActionControlState } from "./action-control";
import {
  actionControlFor,
  busySubmission,
  materializationOutcomeFor,
  PALETTE_ONLY_REASONS,
  reasonNextStep,
} from "./action-control";
import type { AffordanceDescriptor, AffordanceStatus } from "./affordance";
import { FocusRef } from "./focus";
import { reasonNextStep as reasonNextStepFromInspector, renderInspector } from "./inspector";
import { operationIds } from "./operation-registry";
import { OPERATOR_SESSION, observationFor, seededModel, unitEvent } from "./testing/shell-harness";
import { goldReservedTokens } from "./theme";
import type { MaterializationOutcome } from "./views-work";
import { materializeAffordances, renderWorkView } from "./views-work";

/**
 * GUI redesign increment 5: the ActionControl presentation object (ICM §30,
 * UESC §22.9). These suites pin the §30 state contract:
 *
 *   - the status × outcome → state mapping is TOTAL (every pair of the
 *     resolver's status vocabulary and the DSN-009 materialization outcome
 *     maps to exactly one state) and CORRECT (permitted statuses enable,
 *     non-permitting ones disable with reasons, palette-only hides);
 *   - BUSY_SUBMISSION is submission lifecycle, NEVER descriptor-derived: the
 *     factory cannot mint it for any input; only the explicit
 *     `busySubmission` constructor does;
 *   - the Work ACTIONS bar renders through the factory with its exact current
 *     glyph/label language, and HIDDEN rows stay absent while remaining
 *     palette-discoverable (DSN-009 teeth);
 *   - Inspector AFFORDANCES records carry the ActionControl state as a field
 *     addition with existing fields preserved;
 *   - gold/attention token discipline is unchanged (DSN-003): the bar's
 *     control-derived rows keep the readiness/attention vocabulary and never
 *     resolve gold.
 *
 * The module-contract suites were written RED-first (the module did not
 * exist); the bar/inspector suites are preservation pins proving the rewrite
 * renders byte-identical text through the new derivation path.
 */

const SYNTHETIC_FOCUS = FocusRef.for("gauntlet", "WorkItem", "u-synthetic");

/** Every member of the resolver's status vocabulary (AffordanceStatus union). */
const EVERY_STATUS: readonly AffordanceStatus[] = [
  "available",
  "requires_attention",
  "degraded",
  "unavailable",
];

/** Every member of the DSN-009 outcome vocabulary (MaterializationOutcome union). */
const EVERY_OUTCOME: readonly MaterializationOutcome[] = [
  "materialized",
  "blocked_with_reason",
  "palette_only",
];

/** A synthetic descriptor with an exact status/reasons shape (factory totality needs all of them). */
function syntheticDescriptor(
  status: AffordanceStatus,
  reasons: readonly string[] = [],
  extra: { operation?: string; required_attention_item?: string } = {},
): AffordanceDescriptor {
  const operation = extra.operation ?? "unit.resolve";
  return Object.freeze({
    id: `affordance:${operation}:${SYNTHETIC_FOCUS.id}`,
    operation,
    focus: SYNTHETIC_FOCUS,
    status,
    reason_codes: Object.freeze([...reasons]),
    ...(extra.required_attention_item !== undefined
      ? { required_attention_item: extra.required_attention_item }
      : {}),
  });
}

/** The §30 mapping the contract fixes, stated once so the grid test stays readable. */
function expectedState(
  status: AffordanceStatus,
  outcome: MaterializationOutcome,
): ActionControlState {
  if (outcome === "palette_only") return "HIDDEN";
  return status === "available" || status === "requires_attention"
    ? "VISIBLE_ENABLED"
    : "VISIBLE_DISABLED";
}

/** A seeded model plus one FAILING unit, so unit.retry resolves plainly available on it. */
function modelWithFailingUnit() {
  const model = seededModel();
  const applied = model.applyEvent(unitEvent("u-flaky", "Flaky Unit", "c-99", "fail"));
  expect(applied.applied).toBe(true);
  return model;
}

function viewText(content: ReturnType<typeof renderWorkView>): string {
  return content.lines.map((line) => line.spans.map((span) => span.text).join("")).join("\n");
}

describe("Increment 5 §30: status × outcome mapping is total, correct, and never BUSY", () => {
  it("maps every (status, outcome) pair to exactly the contract state — no pair is unmapped or BUSY", () => {
    expect(EVERY_STATUS.length).toBe(4);
    expect(EVERY_OUTCOME.length).toBe(3);
    for (const status of EVERY_STATUS) {
      for (const outcome of EVERY_OUTCOME) {
        const control = actionControlFor(syntheticDescriptor(status, ["example_reason"]), outcome);
        expect(control.state).toBe(expectedState(status, outcome));
        expect(control.state).not.toBe("BUSY_SUBMISSION");
        expect(control.operation).toBe("unit.resolve");
      }
    }
  });

  it("palette_only hides regardless of status (HIDDEN precedence) and carries the reasons", () => {
    for (const status of EVERY_STATUS) {
      const control = actionControlFor(
        syntheticDescriptor(status, ["not_action_admitted"]),
        "palette_only",
      );
      expect(control.state).toBe("HIDDEN");
      expect([...control.reasons]).toEqual(["not_action_admitted"]);
    }
  });

  it("returns frozen controls with reasons copied, and absent facts are absent keys", () => {
    const blocked = actionControlFor(
      syntheticDescriptor("unavailable", ["no_open_ticket"]),
      "blocked_with_reason",
    );
    expect(Object.isFrozen(blocked)).toBe(true);
    expect(Object.isFrozen(blocked.reasons)).toBe(true);
    expect("attentionItem" in blocked).toBe(false);

    const enabled = actionControlFor(syntheticDescriptor("available"), "materialized");
    expect("nextStep" in enabled).toBe(false);
    expect("attentionItem" in enabled).toBe(false);
  });
});

describe("Increment 5 §30: real-resolver derivations (DSN-005 vocabulary, no second predicate)", () => {
  it("requires_attention + materialized → VISIBLE_ENABLED carrying the attention item", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const observation = observationFor(model, {
      open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-root" }],
    });
    const resolve = materializeAffordances(focus, observation, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "unit.resolve",
    );
    expect(resolve?.descriptor.status).toBe("requires_attention");
    const control = actionControlFor(
      resolve?.descriptor as AffordanceDescriptor,
      resolve?.outcome as MaterializationOutcome,
    );
    expect(control.state).toBe("VISIBLE_ENABLED");
    expect("attentionItem" in control).toBe(true);
    expect(control.attentionItem).toBe("ESC-9");
    // Enabled controls carry no next step: nothing is being refused.
    expect("nextStep" in control).toBe(false);
  });

  it("blocked_with_reason → VISIBLE_DISABLED with the machine reasons and the grammar's next step", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    const observation = observationFor(model);
    const resolve = materializeAffordances(focus, observation, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "unit.resolve",
    );
    expect(resolve?.outcome).toBe("blocked_with_reason");
    const control = actionControlFor(
      resolve?.descriptor as AffordanceDescriptor,
      resolve?.outcome as MaterializationOutcome,
    );
    expect(control.state).toBe("VISIBLE_DISABLED");
    expect([...control.reasons]).toEqual(["no_open_ticket"]);
    expect(control.nextStep).toBe("requires an open escalation ticket for this unit");
  });

  it("degraded (materialized, non-permitting) → VISIBLE_DISABLED with observation_degraded", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    // An open ticket passes resolve's gate; the degraded read is then the only
    // defect, so the resolver yields status degraded (never permitted).
    const observation = observationFor(model, {
      degraded: true,
      open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-root" }],
    });
    const resolve = materializeAffordances(focus, observation, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "unit.resolve",
    );
    expect(resolve?.descriptor.status).toBe("degraded");
    expect(resolve?.outcome).toBe("materialized");
    const control = actionControlFor(
      resolve?.descriptor as AffordanceDescriptor,
      resolve?.outcome as MaterializationOutcome,
    );
    expect(control.state).toBe("VISIBLE_DISABLED");
    expect([...control.reasons]).toEqual(["observation_degraded"]);
    expect(control.nextStep).toBe("refresh reads before acting on degraded data");
  });

  it("palette_only → HIDDEN from the bar but still addressable with its reason (DSN-009)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    const observation = observationFor(model);
    const refresh = materializeAffordances(focus, observation, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "state.refresh",
    );
    expect(refresh?.outcome).toBe("palette_only");
    const control = actionControlFor(
      refresh?.descriptor as AffordanceDescriptor,
      refresh?.outcome as MaterializationOutcome,
    );
    expect(control.state).toBe("HIDDEN");
    expect([...control.reasons]).toEqual(["not_action_admitted"]);
    expect(control.nextStep).toBe("read-shaped; served by the canonical read path");
  });

  it("every materializeAffordances outcome equals materializationOutcomeFor on the same descriptor (one WHERE policy)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    const items = materializeAffordances(focus, observationFor(model), {
      session: OPERATOR_SESSION,
    });
    expect(items.length).toBe(operationIds().length);
    for (const item of items) {
      expect(item.outcome).toBe(materializationOutcomeFor(item.descriptor));
    }
  });
});

describe("Increment 5 §30: materialization outcome policy (DSN-009 single source)", () => {
  it("classifies exactly the three palette-only reasons; any-of semantics; everything else blocks", () => {
    expect([...PALETTE_ONLY_REASONS]).toEqual([
      "target_not_visible",
      "unsupported_operation",
      "not_action_admitted",
    ]);
    expect(materializationOutcomeFor(syntheticDescriptor("available"))).toBe("materialized");
    expect(materializationOutcomeFor(syntheticDescriptor("requires_attention"))).toBe(
      "materialized",
    );
    expect(
      materializationOutcomeFor(syntheticDescriptor("degraded", ["observation_degraded"])),
    ).toBe("materialized");
    for (const reason of ["target_not_visible", "unsupported_operation", "not_action_admitted"]) {
      expect(materializationOutcomeFor(syntheticDescriptor("unavailable", [reason]))).toBe(
        "palette_only",
      );
    }
    // ANY palette-only reason wins over an informative one (the .some rule the
    // bar has always used: an illegitimate target never occupies a bar row).
    expect(
      materializationOutcomeFor(
        syntheticDescriptor("unavailable", ["target_not_visible", "actor_authentication_required"]),
      ),
    ).toBe("palette_only");
    expect(
      materializationOutcomeFor(syntheticDescriptor("unavailable", ["authority_not_permitted"])),
    ).toBe("blocked_with_reason");
  });

  it("the reason grammar keeps its canonical vocabulary (moved from inspector.ts, re-exported for API stability)", () => {
    expect(reasonNextStep("no_open_ticket")).toBe(
      "requires an open escalation ticket for this unit",
    );
    expect(reasonNextStep("surface_unreachable")).toBe("restore the canonical surface connection");
    expect(reasonNextStep("unknown_future_gate")).toBe(
      "inspect the blocking gate in the canonical record",
    );
    // The inspector module still exports the grammar it always exported.
    expect(reasonNextStepFromInspector).toBe(reasonNextStep);
  });
});

describe("Increment 5 §30: BUSY_SUBMISSION exists only through the explicit constructor", () => {
  it("no descriptor input can mint BUSY_SUBMISSION (it is submission lifecycle, not affordance status)", () => {
    for (const status of EVERY_STATUS) {
      for (const outcome of EVERY_OUTCOME) {
        for (const reasons of [[], ["no_open_ticket"], ["observation_degraded"]]) {
          const control = actionControlFor(syntheticDescriptor(status, reasons), outcome);
          expect(control.state).not.toBe("BUSY_SUBMISSION");
        }
      }
    }
  });

  it("busySubmission(operation) mints the one BUSY shape: no reasons, no next step, no domain claim", () => {
    const busy = busySubmission("unit.resolve");
    expect(busy.operation).toBe("unit.resolve");
    expect(busy.state).toBe("BUSY_SUBMISSION");
    expect(busy.reasons).toEqual([]);
    expect("nextStep" in busy).toBe(false);
    expect("attentionItem" in busy).toBe(false);
    expect(Object.isFrozen(busy)).toBe(true);
  });
});

describe("Increment 5: the Work ACTIONS bar renders through actionControlFor", () => {
  it("VISIBLE_DISABLED rows keep the exact BLOCKED + next language; HIDDEN rows stay absent but palette-discoverable", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(focus);
    const observation = observationFor(model);
    const content = renderWorkView({
      snapshot: model.getSnapshot(),
      observation,
      session: OPERATOR_SESSION,
    });
    const text = viewText(content);
    expect(text).toContain(
      "× UNIT.RESOLVE BLOCKED: no_open_ticket · next: requires an open escalation ticket for this unit",
    );
    expect(text).toContain(
      "× UNIT.RETRY BLOCKED: readiness_not_earned · next: retry requires a failed verification on this unit",
    );
    const lineIds = content.lines.map((line) => line.id ?? "");
    expect(lineIds.some((id) => id === "work.affordance.state.refresh")).toBe(false);
    expect(text).toContain("more in command palette");
    expect(text).toContain("state.refresh: not_action_admitted");
  });

  it("VISIBLE_ENABLED rows keep the current glyph/label language: ✓ ready and ! responds-to-attention", () => {
    const model = modelWithFailingUnit();
    const flaky = FocusRef.for("gauntlet", "WorkItem", "u-flaky");
    model.selectFocus(flaky);
    // No ticket: resolve is blocked, retry on the failed unit is plainly available.
    const plain = viewText(
      renderWorkView({
        snapshot: model.getSnapshot(),
        observation: observationFor(model),
        session: OPERATOR_SESSION,
      }),
    );
    expect(plain).toContain("✓ UNIT.RETRY ready");

    // With an open ticket the same operation upgrades to the attention row.
    const withTicket = viewText(
      renderWorkView({
        snapshot: model.getSnapshot(),
        observation: observationFor(model, {
          open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-flaky" }],
        }),
        session: OPERATOR_SESSION,
      }),
    );
    expect(withTicket).toContain("! UNIT.RESOLVE responds to ESC-9");
  });

  it("bar records carry control_state equal to a fresh factory derivation over a fresh resolution (never HIDDEN in the bar)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(focus);
    const observation = observationFor(model);
    const content = renderWorkView({
      snapshot: model.getSnapshot(),
      observation,
      session: OPERATOR_SESSION,
    });
    const items = materializeAffordances(focus, observation, { session: OPERATOR_SESSION });
    const affordanceRecords = content.records.filter((record) => record.kind === "affordance");
    expect(affordanceRecords.length).toBe(2);
    for (const record of affordanceRecords) {
      const item = items.find(
        (candidate) => candidate.descriptor.operation === record.fields["operation"],
      );
      expect(item).toBeDefined();
      const control = actionControlFor(
        item?.descriptor as AffordanceDescriptor,
        item?.outcome as MaterializationOutcome,
      );
      expect(record.fields["control_state"]).toBe(control.state);
      expect(control.state).not.toBe("HIDDEN");
      // Resolver-owned fields ride along unchanged (DSN-005 teeth).
      expect(record.fields["status"]).toBe(item?.descriptor.status);
      expect(record.fields["reasons"]).toBe(item?.descriptor.reason_codes.join(",") || "none");
    }
  });

  it("the control-derived bar adds no token: affordance rows keep readiness/attention vocabulary and never resolve gold (DSN-003)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.selectFocus(focus);
    const content = renderWorkView({
      snapshot: model.getSnapshot(),
      observation: observationFor(model, {
        open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-root" }],
      }),
      session: OPERATOR_SESSION,
    });
    const barLines = content.lines.filter((line) => (line.id ?? "").startsWith("work.affordance."));
    expect(barLines.length).toBeGreaterThan(0);
    const tokens = new Set(
      barLines.flatMap((line) =>
        line.spans.filter((span) => span.token !== undefined).map((span) => span.token as string),
      ),
    );
    expect(tokens.has("attention.required")).toBe(true);
    for (const token of tokens) {
      expect(["readiness.ready", "attention.required", "readiness.blocked"]).toContain(token);
      expect(goldReservedTokens().includes(token as never)).toBe(false);
    }
  });
});

describe("Increment 5: Inspector AFFORDANCES records carry the ActionControl state", () => {
  it("every operation stays inspectable with its state; palette-only ones record HIDDEN with reasons (DSN-009)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    const observation = observationFor(model);
    const { sections } = renderInspector({
      snapshot: model.getSnapshot(),
      focus,
      observation,
      session: OPERATOR_SESSION,
    });
    const records = sections.find((section) => section.id === "affordances")?.records ?? [];
    expect(records.length).toBe(operationIds().length);
    const byOperation = new Map(records.map((record) => [record.fields["operation"], record]));
    expect(byOperation.get("unit.resolve")?.fields["control_state"]).toBe("VISIBLE_DISABLED");
    expect(byOperation.get("unit.retry")?.fields["control_state"]).toBe("VISIBLE_DISABLED");
    expect(byOperation.get("state.refresh")?.fields["control_state"]).toBe("HIDDEN");
    // Existing fields preserved (field ADDITIONS only).
    expect(byOperation.get("unit.resolve")?.fields["status"]).toBe("unavailable");
    expect(byOperation.get("unit.resolve")?.fields["reasons"]).toBe("no_open_ticket");
    expect(byOperation.get("unit.resolve")?.fields["next_step"]).toBe(
      "requires an open escalation ticket for this unit",
    );
    expect(byOperation.get("state.refresh")?.fields["reasons"]).toBe("not_action_admitted");
  });

  it("the Work bar and the Inspector derive the SAME control state per operation (REQ-AFF-002 convergence)", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(focus);
    const observation = observationFor(model);
    const work = renderWorkView({
      snapshot: model.getSnapshot(),
      observation,
      session: OPERATOR_SESSION,
    });
    const inspector = renderInspector({
      snapshot: model.getSnapshot(),
      focus,
      observation,
      session: OPERATOR_SESSION,
    });
    const inspectorStates = new Map(
      (inspector.sections.find((section) => section.id === "affordances")?.records ?? []).map(
        (record) => [record.fields["operation"], record.fields["control_state"]],
      ),
    );
    const barRecords = work.records.filter((record) => record.kind === "affordance");
    expect(barRecords.length).toBeGreaterThan(0);
    for (const record of barRecords) {
      expect(record.fields["control_state"]).toBe(
        inspectorStates.get(record.fields["operation"] ?? ""),
      );
    }
  });
});
