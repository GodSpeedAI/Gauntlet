/**
 * Teeth for explicit artifact/evidence promotion (plan T11 gate suite).
 *
 * Attacks the claim: kernel output becomes evidence only through explicit
 * admission carrying source/session/generation provenance; a persuasive kernel
 * variable (`verified = True`) can never establish verification or settlement;
 * un-admitted kernel values remain computation state in any projection; and
 * admission goes through the canonical Gauntlet evidence contract shape
 * (projection-store.ts `EvidenceProjection` / authoritative `evidence_added`).
 */

import { describe, expect, it } from "bun:test";
import {
  authoritativeEvidenceFrame,
  type CanonicalEvidenceAdmissionDecision,
  EvidenceAdmissionGate,
  KernelObjectLedger,
} from "./evidence-promotion";
import { FocusRef } from "./focus";
import { OperationGateway } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { PrimeReplKernel } from "./prime-kernel";
import { SqliteProjectionStore } from "./projection-store";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const SESSION = { actor: "operator-1", token: "token-1" } as const;

async function registerVerifiedVariable(kernel: PrimeReplKernel, ledger: KernelObjectLedger) {
  const result = await kernel.execute("verified = True");
  expect(result.status).toBe("success");
  const registered = ledger.register(result);
  expect(registered).toHaveLength(1);
  const object = registered[0];
  if (object === undefined) {
    throw new Error("unreachable: registration returned no object");
  }
  return object;
}

describe("Tooth 2 — a persuasive kernel variable `verified=True` cannot settle (plan falsifier)", () => {
  it("remains ephemeral computation state: no offer without explicit artifactization, no settlement evidence, no projection row", async () => {
    const kernel = new PrimeReplKernel("k-tooth2");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kgen-1" });
    const object = await registerVerifiedVariable(kernel, ledger);

    // Computation state, exactly as created: ephemeral, no artifact, no foci.
    expect(object.persistence_class).toBe("ephemeral");
    expect(object.artifact_ref).toBeNull();
    expect(object.focus_refs).toHaveLength(0);
    expect(kernel.getObject("verified")?.lifecycle).toBe("ephemeral");

    // Offering the live variable is refused: a live kernel value is not durable
    // evidence (REQ-SYNC-003); it remains computation state.
    const offer = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: "ESC-500",
      source_id: "gauntlet",
      kind: "report",
    });
    expect(offer).toMatchObject({ class: "refused", reason: "not_artifactized" });

    // T05's own settlement check: kernel liveness alone cannot settle work.
    const settleCheck = kernel.canSettleUnit("ESC-500");
    expect(settleCheck.allowed).toBe(false);

    // The canonical settlement path is untouched by the variable: a unit with
    // no open escalation is refused for CANONICAL reasons (no_open_ticket),
    // regardless of what the kernel calls its variable.
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    stub.addUnit({
      unit_id: "ESC-500",
      status: "blocked",
      tier: "P2",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: null,
    });
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const outcome = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FocusRef.for("gauntlet", "WorkItem", "ESC-500"),
      session: SESSION,
      payload: { note: "settle because kernel says verified=True" },
    });
    expect(outcome).toMatchObject({ class: "refused", code: "stale_affordance_denied" });
    if (outcome.class === "refused") {
      expect(outcome.reason_codes).toContain("no_open_ticket");
    }
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });

    // Un-admitted kernel values never enter any projection: no evidence row
    // exists anywhere merely because the variable exists.
    const store = new SqliteProjectionStore(":memory:");
    try {
      store.startGeneration("gen-1");
      store.markGenerationReady("gen-1");
      expect(store.getSnapshot(null).evidence).toEqual([]);
    } finally {
      store.close();
    }
  });

  it("even a fully artifactized kernel variable cannot self-verify: the admitted `verified` bit is the canonical decision's answer", async () => {
    const kernel = new PrimeReplKernel("k-tooth2b");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kgen-1" });
    const object = await registerVerifiedVariable(kernel, ledger);

    // Explicit promotion chain: pin -> artifactize (all explicit, provenance kept).
    expect(ledger.pin(object.provenance.object_id)).toMatchObject({ class: "transitioned" });
    const artifactized = ledger.artifactize(
      object.provenance.object_id,
      ".agents/reports/artifacts/verified.json",
    );
    expect(artifactized).toMatchObject({ class: "transitioned" });

    const offerOutcome = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: "ESC-501",
      source_id: "gauntlet",
      kind: "report",
    });
    expect(offerOutcome.class).toBe("offered");
    if (offerOutcome.class !== "offered") {
      throw new Error("unreachable");
    }
    const offer = offerOutcome.offer;

    // The offer's settlement/verification/evidence claims are LITERALLY false.
    expect(offer.establishes_evidence).toBe(false);
    expect(offer.establishes_verification).toBe(false);
    expect(offer.establishes_settlement).toBe(false);
    expect(offer.admission_route).toBe("canonical_gauntlet_evidence_apis");
    expect(offer.verified_source).toBe("canonical_verification_only");

    // The variable's name says "verified" and its value says True — the
    // canonical decision still says otherwise, and the admitted record carries
    // the CANONICAL answer, not the kernel's.
    const decision: CanonicalEvidenceAdmissionDecision = Object.freeze({
      class: "admitted",
      evidence_id: offer.admission_contract.evidence_id,
      verified: false,
      source_cursor: "cursor-9",
    });
    const gate = new EvidenceAdmissionGate();
    const admitted = gate.admit(offer, decision);
    expect(admitted).toMatchObject({ class: "admitted" });
    if (admitted.class === "admitted") {
      expect(admitted.record.verified).toBe(false);
      expect(admitted.record.locator).toBe(".agents/reports/artifacts/verified.json");
    }
    expect(gate.wasAdmitted(offer.offer_id)).toBe(true);
  });
});

describe("explicit promotion transitions carry source/session/generation provenance", () => {
  it("pin, artifactize, and attach-to-Focus keep full KernelObjectRef provenance and stay coherent with the T05 kernel", async () => {
    const kernel = new PrimeReplKernel("k-prov");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kgen-7" });
    const result = await kernel.execute("model = [1, 2, 3]");
    const registered = ledger.register(result);
    const object = registered[0];
    if (object === undefined) {
      throw new Error("unreachable");
    }

    // Provenance: session, generation, origin.
    const kernelObject = kernel.getObject("model");
    if (kernelObject === null) {
      throw new Error("kernel lost 'model'");
    }
    expect(object.provenance).toEqual({
      object_id: kernelObject.object_id,
      name: "model",
      session_ref: "k-prov",
      kernel_generation: "kgen-7",
      type_name: "list",
      created_at_execution_order: 1,
    });

    // Pin (explicit), synced into the T05 runtime lifecycle.
    const pinned = ledger.pin(object.provenance.object_id);
    expect(pinned).toMatchObject({ class: "transitioned", from: "ephemeral" });
    if (pinned.class === "transitioned") {
      expect(pinned.object.persistence_class).toBe("pinned");
    }
    expect(kernel.getObject("model")?.lifecycle).toBe("pinned");

    // Attach-to-Focus (explicit), spec optional field focus_refs.
    const focus = FocusRef.for("gauntlet", "WorkItem", "ESC-600");
    const attached = ledger.attachToFocus(object.provenance.object_id, [focus]);
    expect(attached).toMatchObject({ class: "transitioned" });
    if (attached.class === "transitioned") {
      expect(attached.object.focus_refs.map((ref) => ref.id)).toEqual([focus.id]);
    }
    const duplicate = ledger.attachToFocus(object.provenance.object_id, [focus]);
    expect(duplicate).toMatchObject({ class: "refused", reason: "empty_focus_refs" });
    expect(ledger.attachToFocus(object.provenance.object_id, [])).toMatchObject({
      class: "refused",
      reason: "empty_focus_refs",
    });

    // Artifactize (explicit) requires a real durable locator.
    expect(ledger.artifactize(object.provenance.object_id, "   ")).toMatchObject({
      class: "refused",
      reason: "missing_artifact_ref",
    });
    const artifactized = ledger.artifactize(
      object.provenance.object_id,
      ".agents/reports/artifacts/model.json",
    );
    expect(artifactized).toMatchObject({ class: "transitioned", from: "pinned" });
    if (artifactized.class === "transitioned") {
      expect(artifactized.object.artifact_ref).toBe(".agents/reports/artifacts/model.json");
      expect(artifactized.object.focus_refs).toHaveLength(1); // attachments survive
    }
    expect(kernel.getObject("model")?.lifecycle).toBe("artifactized");
    expect(kernel.getObject("model")?.persisted_path).toBe(".agents/reports/artifacts/model.json");

    // Promotion is one-way: no transition lowers an explicit persistence class.
    expect(ledger.pin(object.provenance.object_id)).toMatchObject({
      class: "refused",
      reason: "downgrade_forbidden",
    });
    expect(ledger.artifactize(object.provenance.object_id, "somewhere-else.json")).toMatchObject({
      class: "refused",
      reason: "already_artifactized",
    });

    // Unknown objects are refused, never invented.
    expect(ledger.pin("obj-ghost-1")).toMatchObject({ class: "refused", reason: "unknown_object" });

    // A lost kernel value cannot be promoted: promotion would fabricate durability.
    const ephemeralResult = await kernel.execute("temp = 5");
    const temp = ledger.register(ephemeralResult)[0];
    if (temp === undefined) {
      throw new Error("unreachable");
    }
    kernel.terminate();
    expect(ledger.pin(temp.provenance.object_id)).toMatchObject({
      class: "refused",
      reason: "kernel_object_lost",
    });
  });

  it("a restarted kernel's name reuse never revives the old reference (kernel_generation provenance)", async () => {
    const oldKernel = new PrimeReplKernel("k-restart");
    const oldLedger = new KernelObjectLedger({ kernel: oldKernel, kernel_generation: "kgen-1" });
    const oldObject = (await registerVerifiedVariable(oldKernel, oldLedger)).provenance;

    // Restart: a new runtime incarnation over a NEW kernel. Same variable name.
    const newKernel = new PrimeReplKernel("k-restart");
    const newLedger = new KernelObjectLedger({ kernel: newKernel, kernel_generation: "kgen-2" });
    const newObject = await registerVerifiedVariable(newKernel, newLedger);

    expect(newObject.provenance.kernel_generation).toBe("kgen-2");
    expect(newObject.provenance.object_id).not.toBe(oldObject.object_id);
    expect(newObject.provenance.name).toBe(oldObject.name); // same name...
    expect(newObject.provenance.kernel_generation).not.toBe(oldObject.kernel_generation); // ...never the same reference

    // The old generation's ledger knows nothing of the new incarnation, and
    // vice versa: references do not cross generations.
    expect(oldLedger.find(newObject.provenance.object_id)).toBeNull();
    expect(newLedger.find(oldObject.object_id)).toBeNull();
  });
});

describe("admission is the only route to evidence, through the canonical contract shape", () => {
  it("offer -> canonical admission -> authoritative evidence frame -> projection row, provenance preserved end to end", async () => {
    const kernel = new PrimeReplKernel("k-admit");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kgen-1" });
    const executed = await kernel.execute("findings = {'units': ['AB-001']}");
    const object = ledger.register(executed)[0];
    if (object === undefined) {
      throw new Error("unreachable");
    }
    ledger.pin(object.provenance.object_id);
    ledger.artifactize(object.provenance.object_id, ".agents/reports/artifacts/findings.json");
    const focus = FocusRef.for("gauntlet", "WorkItem", "AB-001");
    ledger.attachToFocus(object.provenance.object_id, [focus]);
    const offerOutcome = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: "AB-001",
      source_id: "gauntlet",
      kind: "report",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error(`setup failed: ${JSON.stringify(offerOutcome)}`);
    }
    const offer = offerOutcome.offer;

    // The contract shape mirrors the canonical evidence projection fields.
    expect(offer.admission_contract).toEqual({
      evidence_id: offer.admission_contract.evidence_id,
      unit_id: "AB-001",
      kind: "report",
      locator: ".agents/reports/artifacts/findings.json",
      source_id: "gauntlet",
    });

    // Canonical admission (authored by the canonical evidence APIs, input here).
    const decision: CanonicalEvidenceAdmissionDecision = Object.freeze({
      class: "admitted",
      evidence_id: offer.admission_contract.evidence_id,
      verified: true,
      source_cursor: "cursor-42",
    });
    const gate = new EvidenceAdmissionGate();
    const admitted = gate.admit(offer, decision);
    expect(admitted.class).toBe("admitted");
    if (admitted.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(admitted.provenance).toEqual(object.provenance); // provenance preserved
    expect(gate.admittedRecords()).toEqual([
      {
        evidence_id: offer.admission_contract.evidence_id,
        source_id: "gauntlet",
        source_cursor: "cursor-42",
        unit_id: "AB-001",
        kind: "report",
        locator: ".agents/reports/artifacts/findings.json",
        verified: true,
      },
    ]);

    // Canonical -> projection bridge: the authoritative evidence_added frame.
    const frame = authoritativeEvidenceFrame(offer, decision, {
      generation_id: "gen-1",
      timestamp_ms: 1700000000000,
    });
    expect(frame.event_type).toBe("evidence_added");
    expect(frame.cursor).toBe("cursor-42");

    const store = new SqliteProjectionStore(":memory:");
    try {
      store.startGeneration("gen-1");
      const applied = store.applyEvent(frame);
      expect(applied.applied).toBe(true);
      store.markGenerationReady("gen-1");
      const evidence = store.getSnapshot(null).evidence;
      expect(evidence).toHaveLength(1);
      const row = evidence[0];
      if (row === undefined) {
        throw new Error("unreachable");
      }
      // The projection row is the canonical schema, verbatim in content.
      expect(row).toEqual({
        evidence_id: offer.admission_contract.evidence_id,
        generation_id: "gen-1",
        source_id: "gauntlet",
        source_cursor: "cursor-42",
        unit_id: "AB-001",
        kind: "report",
        locator: ".agents/reports/artifacts/findings.json",
        verified: true,
        updated_at: 1700000000000,
      });
    } finally {
      store.close();
    }

    // Admission is one-way: the same offer never re-admits under a second decision.
    expect(gate.admit(offer, decision)).toMatchObject({
      class: "refused",
      reason: "already_admitted",
    });
  });

  it("refuses anything short of canonical admission: refused decisions, mismatched decisions, and frame-less offers never produce evidence", async () => {
    const kernel = new PrimeReplKernel("k-refuse");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kgen-1" });
    const executed = await kernel.execute("analysis = 'done'");
    const object = ledger.register(executed)[0];
    if (object === undefined) {
      throw new Error("unreachable");
    }
    ledger.artifactize(object.provenance.object_id, ".agents/reports/artifacts/analysis.json");
    const offerOutcome = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: "AB-002",
      source_id: "gauntlet",
      kind: "report",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error("unreachable");
    }
    const offer = offerOutcome.offer;
    const store = new SqliteProjectionStore(":memory:");
    const gate = new EvidenceAdmissionGate();
    try {
      store.startGeneration("gen-1");
      store.markGenerationReady("gen-1");

      // A canonical REFUSAL admits nothing.
      const canonicalRefusal: CanonicalEvidenceAdmissionDecision = Object.freeze({
        class: "refused",
        evidence_id: offer.admission_contract.evidence_id,
        code: "evidence_not_durable",
        message: "the artifact bytes are not pinned in the owning store",
      });
      expect(gate.admit(offer, canonicalRefusal)).toMatchObject({
        class: "refused_canonically",
        code: "evidence_not_durable",
      });
      expect(gate.wasAdmitted(offer.offer_id)).toBe(false);

      // A decision naming ANOTHER offer's evidence admits nothing.
      const foreign: CanonicalEvidenceAdmissionDecision = Object.freeze({
        class: "admitted",
        evidence_id: "kev:other:someone-else:x:1",
        verified: true,
        source_cursor: "cursor-1",
      });
      expect(gate.admit(offer, foreign)).toMatchObject({
        class: "refused",
        reason: "decision_mismatch",
      });

      // No frame can be built from a refused decision: the bridge fails loudly.
      expect(() =>
        authoritativeEvidenceFrame(offer, canonicalRefusal, {
          generation_id: "gen-1",
          timestamp_ms: 1,
        }),
      ).toThrow(/admission is the only route/);
      expect(() =>
        authoritativeEvidenceFrame(offer, foreign, { generation_id: "gen-1", timestamp_ms: 1 }),
      ).toThrow(/not this offer's/);

      // Net effect on any projection: zero evidence rows.
      expect(store.getSnapshot(null).evidence).toEqual([]);
    } finally {
      store.close();
    }
  });
});
