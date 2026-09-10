import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { planMaterialization } from "./horizon";
import {
  countT21Events,
  countT21Objects,
  createT21CaseEvents,
  T21_CASE_SEED,
  T21_FROZEN_BUDGETS,
  T21_FROZEN_HOST_FACTS,
  T21_FROZEN_SAMPLE_COUNTS,
  T21_HOT_SET,
  T21_LARGE_FIXTURE,
  T21_SCENARIOS,
  T21_SMALL_FIXTURE,
} from "./payment-budgets";
import type { ProjectionWorld } from "./relations";
import { WorkbenchModel } from "./workbench-model";

/**
 * T21 budget-table integrity, scenario-registry validity, and the
 * by-construction eager-failure assertion (plan T21, prereg P1/D1).
 *
 * These suites deliberately run at unit-test scale (the SMALL fixture only):
 * the frozen percentile budgets are MEASURED by the two T21 benches against
 * the frozen LARGE fixture; here we prove the freeze itself is sound —
 * threshold-only budgets (no measured values smuggled in), a valid scenario
 * registry, deterministic fixtures whose declared event counts match, and a
 * hot-set ceiling that a whole-world eager shape would violate.
 */

function seedSmallModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(T21_SMALL_FIXTURE.generation);
  for (const frame of createT21CaseEvents(T21_SMALL_FIXTURE)) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`fixture event rejected: ${result.reason ?? "unknown"}`);
    }
  }
  model.markReady(T21_SMALL_FIXTURE.generation);
  return model;
}

function worldOf(model: WorkbenchModel): ProjectionWorld {
  const base = model.store.getSnapshot(null);
  return {
    generation: base.generation,
    units: base.units,
    evidence: base.evidence,
    sessions: base.sessions,
    source_cursors: base.source_cursors,
  };
}

describe("T21 frozen budget table integrity (thresholds only; prereg D1)", () => {
  it("contains only finite positive numeric thresholds", () => {
    for (const [key, value] of Object.entries(T21_FROZEN_BUDGETS)) {
      expect(typeof value).toBe("number");
      expect(Number.isFinite(value)).toBe(true);
      expect((value as number) > 0).toBe(true);
      expect(key.length).toBeGreaterThan(0);
    }
  });

  it("keeps paired percentile budgets ordered (p50 <= p99 classes)", () => {
    expect(T21_FROZEN_BUDGETS.focus_switch_cached_p50_ms).toBeLessThanOrEqual(
      T21_FROZEN_BUDGETS.focus_switch_cached_p99_ms,
    );
    expect(T21_FROZEN_BUDGETS.horizon_move_p50_ms).toBeLessThanOrEqual(
      T21_FROZEN_BUDGETS.horizon_move_p99_ms,
    );
    expect(T21_FROZEN_BUDGETS.stream_absorb_p50_ms).toBeLessThanOrEqual(
      T21_FROZEN_BUDGETS.stream_absorb_p99_ms,
    );
  });

  it("keeps hot-set ceilings below the smallest ladder fixture's object count (discriminating)", () => {
    expect(T21_FROZEN_BUDGETS.hot_set_subscription_ceiling).toBeLessThan(
      countT21Objects(T21_SMALL_FIXTURE),
    );
    expect(T21_FROZEN_BUDGETS.hot_set_context_ceiling).toBeLessThan(
      countT21Objects(T21_SMALL_FIXTURE),
    );
  });

  it("is frozen against mutation (a tampered threshold throws)", () => {
    expect(() => {
      (T21_FROZEN_BUDGETS as Record<string, unknown>)["cold_start_p99_ms"] = 1;
    }).toThrow();
    expect(() => {
      (T21_HOT_SET as Record<string, unknown>)["subscription_ceiling"] = 1_000_000;
    }).toThrow();
  });

  it("records the frozen host facts with the frozen prereg hash", () => {
    expect(T21_FROZEN_HOST_FACTS.runtime).toBe("Bun 1.4.0");
    expect(T21_FROZEN_HOST_FACTS.cpu_cores).toBeGreaterThan(0);
    expect(T21_FROZEN_HOST_FACTS.memory_total_kb).toBeGreaterThan(0);
    expect(T21_FROZEN_HOST_FACTS.prereg_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(T21_FROZEN_HOST_FACTS.prereg_sha256).toBe(
      "b4b32e5b7934320c999621de19d15164b44bb6a1432c481a24f55306ad96dc84",
    );
  });

  it("declares sample counts and workload sizes as frozen positive integers", () => {
    for (const [key, value] of Object.entries(T21_FROZEN_SAMPLE_COUNTS)) {
      expect(Number.isInteger(value)).toBe(true);
      expect((value as number) > 0).toBe(true);
      expect(key.length).toBeGreaterThan(0);
    }
  });
});

describe("T21 scenario registry validity", () => {
  it("covers every frozen fixture ladder point with a valid spec", () => {
    expect(T21_SMALL_FIXTURE.units).toBeGreaterThan(0);
    expect(T21_LARGE_FIXTURE.units).toBeGreaterThan(T21_SMALL_FIXTURE.units);
    expect(T21_LARGE_FIXTURE.unit_update_waves).toBeGreaterThanOrEqual(1);
    expect(T21_LARGE_FIXTURE.session_trees).toBeGreaterThanOrEqual(1);
    expect(T21_LARGE_FIXTURE.evidence).toBeLessThan(T21_LARGE_FIXTURE.units);
  });

  it("declares the TARGET fixture at the frozen 100k+ event scale", () => {
    expect(countT21Events(T21_LARGE_FIXTURE)).toBeGreaterThanOrEqual(100_000);
    expect(countT21Events(T21_LARGE_FIXTURE)).toBe(103_610);
    expect(T21_LARGE_FIXTURE.units).toBeGreaterThanOrEqual(1_000);
  });

  it("registry entries reference a real bench and a declared budget class", () => {
    expect(T21_SCENARIOS.length).toBeGreaterThanOrEqual(10);
    const ids = new Set<string>();
    for (const scenario of T21_SCENARIOS) {
      expect(scenario.bench === "large-case" || scenario.bench === "long-horizon").toBe(true);
      expect(["cold", "interactive", "coalesced", "off-path", "shape"]).toContain(
        scenario.bench === "long-horizon" ? "shape" : scenario.budget_class,
      );
      expect(scenario.surface.length).toBeGreaterThan(0);
      expect(scenario.description.length).toBeGreaterThan(0);
      ids.add(scenario.id);
    }
    expect(ids.has("horizon-shape")).toBe(true);
    expect(ids.has("stream-typing-scroll")).toBe(true);
    expect(ids.size).toBe(T21_SCENARIOS.length);
  });

  it("generates byte-identical events for identical specs (seeded LCG, no Math.random)", () => {
    const a = createT21CaseEvents(T21_SMALL_FIXTURE);
    const b = createT21CaseEvents(T21_SMALL_FIXTURE);
    expect(a.length).toBe(b.length);
    expect(JSON.stringify(a)).toBe(JSON.stringify(b));
    expect(a.length).toBe(countT21Events(T21_SMALL_FIXTURE));
  });

  it("emits the declared event mix with unique cursors and stable provenance", () => {
    const events = createT21CaseEvents(T21_SMALL_FIXTURE);
    const kinds = new Map<string, number>();
    const cursors = new Set<string>();
    for (const event of events) {
      kinds.set(event.event_type, (kinds.get(event.event_type) ?? 0) + 1);
      expect(cursors.has(event.cursor)).toBe(false);
      cursors.add(event.cursor);
      expect(event.generation_id).toBe(T21_SMALL_FIXTURE.generation);
    }
    expect(kinds.get("unit_updated")).toBe(T21_SMALL_FIXTURE.units);
    expect(kinds.get("evidence_added")).toBe(T21_SMALL_FIXTURE.evidence);
    expect(kinds.get("session_updated")).toBe(T21_SMALL_FIXTURE.sessions);
    // wave cursor uniqueness: a wave over the same units advances cursors, not ids
    const waved = createT21CaseEvents({
      ...T21_SMALL_FIXTURE,
      generation: "gen-t21-wave",
      unit_update_waves: 2,
    });
    expect(waved.length).toBe(
      T21_SMALL_FIXTURE.units * 3 + T21_SMALL_FIXTURE.evidence + T21_SMALL_FIXTURE.sessions,
    );
    const waveCursors = new Set(waved.map((event) => event.cursor));
    expect(waveCursors.size).toBe(waved.length);
  });
});

describe("T21 hot-set shape: eager whole-world fails by construction (prereg P1)", () => {
  it("keeps a one-Focus horizon inside the declared ceilings while the eager plan violates them", () => {
    const model = seedSmallModel();
    try {
      const world = worldOf(model);

      const root = FocusRef.for("gauntlet", "WorkItem", "u-0");
      const horizon = model.hoistToFocus(root, {
        relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
      });
      const hoisted = planMaterialization(world, horizon);
      expect(hoisted.counters.subscriptions).toBeLessThanOrEqual(T21_HOT_SET.subscription_ceiling);
      expect(hoisted.counters.context_candidates).toBeLessThanOrEqual(T21_HOT_SET.context_ceiling);
      expect(hoisted.counters.queries).toBeLessThanOrEqual(T21_HOT_SET.query_ceiling);

      // The eager whole-world shape: subscriptions scale with EVERY object in the
      // Case, so the SAME frozen ceiling rejects it — the bound discriminates.
      const eager = planMaterialization(world, null);
      expect(eager.counters.subscriptions).toBe(countT21Objects(T21_SMALL_FIXTURE));
      expect(eager.counters.subscriptions).toBeGreaterThan(T21_HOT_SET.subscription_ceiling);
      expect(eager.counters.context_candidates).toBeGreaterThan(T21_HOT_SET.context_ceiling);
    } finally {
      model.close();
    }
  });

  it("bounds the hoisted working set by the declared relation closure, not the Case", () => {
    const model = seedSmallModel();
    try {
      const world = worldOf(model);
      const horizon = model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-0"), {});
      const hoisted = planMaterialization(world, horizon);
      // Declared closure: root + round-robin evidence closure (<= ceil(evidence/units))
      // + mandatory evidence + reference (per source) + governance (per authority).
      const maxEvidencePerUnit = Math.ceil(T21_SMALL_FIXTURE.evidence / T21_SMALL_FIXTURE.units);
      const declaredClosureBound = 1 + maxEvidencePerUnit + maxEvidencePerUnit + 2 + 3;
      expect(hoisted.counters.subscriptions).toBeLessThanOrEqual(declaredClosureBound);
      expect(hoisted.working_set.length).toBeLessThanOrEqual(declaredClosureBound);
      // and the whole-Case world stays untouched by the horizon (narrowing, not deletion)
      expect(world.units.length).toBe(T21_SMALL_FIXTURE.units);
    } finally {
      model.close();
    }
  });

  it("freezes the seed the ladder determinism relies on", () => {
    expect(T21_CASE_SEED).toBe(0x21ca5e5);
  });
});
