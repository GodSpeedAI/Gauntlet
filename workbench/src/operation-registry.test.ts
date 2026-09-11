import { describe, expect, it } from "bun:test";
import { EMPTY_AFFORDANCE_OBSERVATION, resolveAffordance } from "./affordance";
import { FocusRef } from "./focus";
import { isOperationId, OPERATION_IDS, operationIds } from "./operation-registry";
import {
  CAPABILITY_HANDSHAKE_VERSION,
  capabilityHandshake,
  contractIsComplete,
  isPlannedOperationId,
  lookupPlannedOperation,
  PLANNED_CONTRACT_FIELDS,
  PLANNED_OPERATION_IDS,
  PLANNED_OPERATION_REGISTRY,
  type PlannedOperationContract,
  plannedOperationForComposerMode,
  plannedOperationIds,
} from "./planned-operations";

/**
 * Tests for the C1 planned-operation declaration surface (plan Phase C1,
 * workbench-intent-closure.md §4; REQ-OPS-003, D7).
 */

describe("planned-operations: C1 — the nine-field contract", () => {
  it("every planned operation carries all nine required REQ-OPS-003 fields, non-empty", () => {
    expect(PLANNED_CONTRACT_FIELDS.length).toBe(9);
    for (const id of plannedOperationIds()) {
      const contract = PLANNED_OPERATION_REGISTRY[id];
      expect(contractIsComplete(contract)).toBe(true);
      for (const field of PLANNED_CONTRACT_FIELDS) {
        expect(contract[field].trim().length).toBeGreaterThan(0);
      }
    }
  });

  it("a planned operation with a hollow field fails the completeness check (negative control)", () => {
    const hollow: PlannedOperationContract = {
      ...PLANNED_OPERATION_REGISTRY[PLANNED_OPERATION_IDS.PRIME_LAUNCH],
      budget: "   ",
    };
    expect(contractIsComplete(hollow)).toBe(false);
  });

  it("the five REQ-OPS-003 categories (launch, steering, cancellation, evidence admission, session operations) are all declared, plus message delivery", () => {
    const ids = plannedOperationIds();
    expect(ids).toContain(PLANNED_OPERATION_IDS.PRIME_LAUNCH);
    expect(ids).toContain(PLANNED_OPERATION_IDS.PRIME_STEER);
    expect(ids).toContain(PLANNED_OPERATION_IDS.PRIME_CANCEL);
    expect(ids).toContain(PLANNED_OPERATION_IDS.EVIDENCE_ADMIT);
    expect(ids).toContain(PLANNED_OPERATION_IDS.PRIME_SESSION);
    expect(ids).toContain(PLANNED_OPERATION_IDS.PRIME_MESSAGE);
    expect(ids.length).toBe(6);
  });
});

describe("planned-operations: C1 — planned never means invocable (D7)", () => {
  it("no planned operation id is a registered EXISTING operation id", () => {
    for (const plannedId of plannedOperationIds()) {
      expect(isOperationId(plannedId)).toBe(false);
    }
  });

  it("OPERATION_REGISTRY's exact three-entry shape is untouched by this file existing", () => {
    // Pinned identically in operation-isomorphism.test.ts; re-asserted here so
    // a future edit to this file trips locally, not only in a sibling suite.
    expect(operationIds()).toEqual([
      OPERATION_IDS.UNIT_RESOLVE,
      OPERATION_IDS.UNIT_RETRY,
      OPERATION_IDS.STATE_REFRESH,
    ]);
  });

  it("resolveAffordance treats every planned operation as unsupported_operation, never available (the tooth: exposing a planned op as invocable fails the suite)", () => {
    const focus = FocusRef.for("gauntlet", "unit", "AK-planned-1");
    for (const plannedId of plannedOperationIds()) {
      const descriptor = resolveAffordance(plannedId, focus, EMPTY_AFFORDANCE_OBSERVATION);
      expect(descriptor.status).toBe("unavailable");
      expect(descriptor.reason_codes).toContain("unsupported_operation");
    }
  });

  it("isPlannedOperationId/lookupPlannedOperation agree and reject unknown strings", () => {
    for (const plannedId of plannedOperationIds()) {
      expect(isPlannedOperationId(plannedId)).toBe(true);
      expect(lookupPlannedOperation(plannedId)?.id).toBe(plannedId);
    }
    expect(isPlannedOperationId("prime.teleport")).toBe(false);
    expect(lookupPlannedOperation("prime.teleport")).toBeNull();
  });
});

describe("planned-operations: C1 — versioned capability handshake distinguishes existing from planned", () => {
  it("the handshake version has incremented past the implicit existing-only predecessor", () => {
    expect(CAPABILITY_HANDSHAKE_VERSION).toBeGreaterThanOrEqual(2);
  });

  it("existing_operations is exactly today's OPERATION_REGISTRY — an old client reading only this field sees no change (additive versioning)", () => {
    const handshake = capabilityHandshake();
    expect(handshake.version).toBe(CAPABILITY_HANDSHAKE_VERSION);
    expect(handshake.existing_operations).toEqual(operationIds());
  });

  it("planned_operations carries the full declared catalog, disjoint from existing_operations", () => {
    const handshake = capabilityHandshake();
    expect(handshake.planned_operations.map((c) => c.id).sort()).toEqual(
      [...plannedOperationIds()].sort(),
    );
    for (const contract of handshake.planned_operations) {
      expect(handshake.existing_operations).not.toContain(contract.id as never);
    }
  });
});

describe("planned-operations: C1 — composer modes report why, not a bare 'no adapter connected'", () => {
  it("prime/message/kernel modes each resolve to their declared planned operation", () => {
    expect(plannedOperationForComposerMode("prime")?.id).toBe(PLANNED_OPERATION_IDS.PRIME_LAUNCH);
    expect(plannedOperationForComposerMode("message")?.id).toBe(
      PLANNED_OPERATION_IDS.PRIME_MESSAGE,
    );
    expect(plannedOperationForComposerMode("kernel")?.id).toBe(PLANNED_OPERATION_IDS.PRIME_STEER);
  });

  it("operation mode is EXISTING, not planned — always null here", () => {
    expect(plannedOperationForComposerMode("operation")).toBeNull();
  });
});
