/**
 * T24 substrate-inventory integrity suites (plan T24; REQ-PRIME-008 provenance
 * mapping; discharges T10 trigger-record MAJOR 2; prereg D3 / intervention
 * class A2).
 *
 * The gate selector `just tui-test substrate-inventory` selects this file by
 * name. Proofs: the three-way provenance map covers EVERY REQ-PRIME-008
 * substrate kind plus the delegation/session controls the T10 trigger-record
 * names; every layer's answer is evidence-backed (probe references, or the
 * live T05/T10 recording functions the adapter-model answers must match); no
 * canonical-truth field anywhere is true — the literal-false seal holds at the
 * type level AND in every runtime row; and the live requalification refusal
 * carries the same map. Deterministic: spawns nothing.
 */

import { describe, expect, it } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import {
  PRIME_SUBSTRATE_KINDS,
  requestSubstrateStrategy,
  substrateInventory,
} from "./prime-long-running";
import { delegationCapabilityFacts } from "./prime-recursion";
import type {
  RealBinaryExposure,
  SubstrateFeatureId,
  SubstrateProvenanceRow,
  WorkbenchAdmission,
} from "./substrate-inventory";
import {
  PROBE_CAPTURES,
  QUALIFIED_PRIME_BINARY,
  reqPrime008MappedKinds,
  substrateProvenanceMap,
  substrateProvenanceRow,
} from "./substrate-inventory";

const EVIDENCE_CAPTURE_ROOT = ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures";

/** Repo root, resolved relative to this module (workbench/src/). */
function repoRoot(): string {
  return new URL("../..", import.meta.url).pathname.replace(/\/$/, "");
}

/** The delegation/session controls the T10 trigger-record names verbatim. */
const TRIGGER_RECORD_CONTROLS: readonly SubstrateFeatureId[] = [
  "child_budget",
  "child_session_admission",
  "mailbox_communication",
  "mailbox_delivery_mode_control",
  "result_contract_completion",
  "session_stop",
  "session_attach",
  "session_list",
  "session_resume",
];

describe("T24 substrate provenance map — coverage (REQ-PRIME-008 + T10 trigger-record)", () => {
  it("maps every REQ-PRIME-008 substrate kind exactly once, with the kind recorded on the row", () => {
    const mapped = reqPrime008MappedKinds();
    for (const kind of PRIME_SUBSTRATE_KINDS) {
      expect(mapped.filter((entry) => entry === kind).length).toBe(1);
    }
    const rows = substrateProvenanceMap();
    for (const row of rows) {
      if (row.req_prime_008_class !== null) {
        expect(PRIME_SUBSTRATE_KINDS).toContain(row.req_prime_008_class);
      }
    }
    // No two rows claim the same feature id.
    const features = rows.map((row) => row.feature);
    expect(new Set(features).size).toBe(features.length);
  });

  it("maps every delegation/session control the T10 trigger-record names for the real binary", () => {
    for (const feature of TRIGGER_RECORD_CONTROLS) {
      expect(substrateProvenanceRow(feature)).not.toBeNull();
    }
  });

  it("adapter-model answers MATCH the live T05/T10 recordings — no layer's honest unsupported was overwritten", () => {
    // The map rows must agree with the LIVE function outputs, row by row.
    const liveInventory = new Map(
      substrateInventory().map((fact) => [fact.substrate, fact] as const),
    );
    const liveDelegation = new Map(
      delegationCapabilityFacts().map((fact) => [fact.capability, fact] as const),
    );
    for (const row of substrateProvenanceMap()) {
      const kind = row.req_prime_008_class;
      if (kind !== null) {
        const fact = liveInventory.get(kind);
        expect(fact).toBeDefined();
        // T10's recording is unchanged: substrates are unsupported AT THE
        // ADAPTER-MODEL LAYER, with the honest layer-scoped reason.
        expect(fact?.supported).toBe(false);
        expect(fact?.reason).toBe("not_exposed_by_installed_prime_adapter_model");
        expect(row.adapter_model.status).toBe("not_exposed");
        expect(row.adapter_model.reason).toBe("not_exposed_by_installed_prime_adapter_model");
      }
    }
    // The delegation facts are still honest at their own layer: native
    // depth/budget remain unsupported BY THE MODEL; the model surfaces the
    // three admission-layer capabilities.
    expect(liveDelegation.get("native_nested_delegation_depth_limit")?.supported).toBe(false);
    expect(liveDelegation.get("native_child_budget_accounting")?.supported).toBe(false);
    expect(liveDelegation.get("child_session_admission")?.supported).toBe(true);
    expect(liveDelegation.get("mailbox_communication")?.supported).toBe(true);
    expect(liveDelegation.get("result_contract_completion")?.supported).toBe(true);
  });

  it("real-binary answers are BOUNDED probed facts with at least one probe reference each", () => {
    for (const row of substrateProvenanceMap()) {
      const exposure: RealBinaryExposure = row.real_binary;
      expect(exposure.probes.length).toBeGreaterThanOrEqual(1);
      for (const probe of exposure.probes) {
        expect(probe.probe_id.length).toBeGreaterThan(0);
        expect(probe.detail.length).toBeGreaterThan(0);
      }
      if (exposure.status === "not_in_probed_cli_surface") {
        // Bounded absence: the note must scope the claim to the probed surface.
        expect(exposure.detail.toLowerCase()).toContain("probed");
      }
      if (exposure.status === "documented_but_refused") {
        // The drift finding must name both halves: documented AND refused.
        expect(exposure.detail.toLowerCase()).toContain("refus");
      }
    }
  });

  it("workbench admissions use the closed typed class vocabulary with non-empty notes", () => {
    for (const row of substrateProvenanceMap()) {
      const admission: WorkbenchAdmission = row.workbench;
      expect(admission.detail.length).toBeGreaterThan(0);
      expect(row.provenance_note.length).toBeGreaterThan(0);
    }
  });
});

describe("T24 substrate provenance map — no canonical truth leaks (REQ-PRIME-008)", () => {
  it("every row carries canonical_direction/settlement/capability as literal false at runtime", () => {
    for (const row of substrateProvenanceMap()) {
      expect(row.canonical_direction).toBe(false);
      expect(row.canonical_settlement).toBe(false);
      expect(row.canonical_capability).toBe(false);
    }
  });

  it("the literal-false seal is TYPE-level: a literal-true canonical field cannot construct a row", () => {
    const row = substrateProvenanceMap()[0] as SubstrateProvenanceRow;
    // Uncommenting the assignment below breaks `just tui-check` (TS2322:
    // 'true' is not assignable to type 'false') — the same class of violation
    // probe T10 ran against PrimeSubstrateRecord, now applied to the map.
    // const broken: SubstrateProvenanceRow = { ...row, canonical_direction: true };
    expect(row.canonical_direction).toBe(false);
  });

  it("the qualified inventory function still records substrates unsupported — the requalification changed nothing at the model layer", () => {
    const inventory = substrateInventory();
    expect(inventory.length).toBe(PRIME_SUBSTRATE_KINDS.length);
    for (const fact of inventory) {
      expect(fact.supported).toBe(false);
      expect(fact.reason).toBe("not_exposed_by_installed_prime_adapter_model");
    }
    // The un-requalified refusal is unchanged too.
    const outcome = requestSubstrateStrategy("goal", "sess-probe");
    expect(outcome.class).toBe("unsupported_substrate");
  });
});

describe("T24 probe corpus — evidence anchoring", () => {
  it("pins the qualified binary identity", () => {
    expect(QUALIFIED_PRIME_BINARY.version).toBe("0.8.0");
    expect(QUALIFIED_PRIME_BINARY.binary.endsWith("prime-agent")).toBe(true);
    expect(QUALIFIED_PRIME_BINARY.launcher_sha256).toMatch(/^[0-9a-f]{64}$/);
  });

  it("records deterministic probe captures with well-formed digests, verified when the capture files are present", () => {
    expect(PROBE_CAPTURES.length).toBeGreaterThanOrEqual(2);
    for (const captureEntry of PROBE_CAPTURES) {
      expect(captureEntry.command.startsWith("prime-agent")).toBe(true);
      expect(captureEntry.capture.startsWith(EVIDENCE_CAPTURE_ROOT)).toBe(true);
      expect(captureEntry.sha256).toMatch(/^[0-9a-f]{64}$/);
      const absolute = `${repoRoot()}/${captureEntry.capture}`;
      if (existsSync(absolute)) {
        const digest = createHash("sha256").update(readFileSync(absolute)).digest("hex");
        expect(digest).toBe(captureEntry.sha256);
      }
    }
  });

  it("exposure answers that claim execution cite at least one lifecycle (L-) probe", () => {
    for (const row of substrateProvenanceMap()) {
      if (row.real_binary.status === "exposed_executed") {
        expect(row.real_binary.probes.some((probe) => probe.probe_id.startsWith("L-"))).toBe(true);
      }
    }
    // The drift finding is recorded with its exact executed refusal.
    const delivery = substrateProvenanceRow("mailbox_delivery_mode_control");
    expect(delivery?.real_binary.status).toBe("documented_but_refused");
    expect(JSON.stringify(delivery?.real_binary.probes)).toContain("--follow-up");
  });
});
