import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { OperationGateway } from "./operation-gateway";
import type {
  PluginContext,
  PluginManifest,
  ShellViewRegistryCell,
  TrustedPluginModule,
} from "./plugins";
import {
  createShellViewRegistryCell,
  PluginActivationError,
  PluginCompositionManager,
  PluginContributionError,
  PluginLifecycleError,
  serializePluginSurfaceFingerprint,
} from "./plugins";
import { renderToText, type ViewContent } from "./render-tree";
import type { ViewId } from "./shell";
import { worldFromSnapshot } from "./shell";
import { CanonicalSurfaceStub, type StubUnitRecord } from "./testing/canonical-surface-stub";
import { createSamplePlugin, SAMPLE_PLUGIN_ID } from "./testing/sample-plugin";
import { coldModel, GEN, observationFor, seededModel, unitEvent } from "./testing/shell-harness";
import { defaultViewRegistry } from "./views-chat-trace";
import type { FullWorkbenchSnapshot, WorkbenchModel } from "./workbench-model";

/**
 * T15 plugin lifecycle suites (REQ-PLUGIN-001/003/004, DSN-012, plan teeth):
 * mount -> exercise -> unload returns EVERY plugin-visible surface to a
 * byte-identical baseline; a plugin that throws halfway through load has its
 * partial effects unwound (Attack 1); double mount/unload is refused or
 * idempotent; plugins cannot touch each other's state; the plugin
 * affordance-projection route re-runs the T09 deny matrix with zero side
 * effects and no gateway bypass.
 */

/** Disposer labels of the representative plugin's steps, in activation order. */
const SAMPLE_DISPOSER_LABELS = [
  "contribution:nav.provenance-lens",
  "contribution:view.work-provenance",
  "contribution:trace.provenance-rows",
  "contribution:inspector.provenance",
  "contribution:artifact.sample-log",
  "contribution:composer.provenance-cue",
  "contribution:status.provenance-cue",
  "contribution:overlay.provenance",
  "contribution:panel.provenance",
  "contribution:relations.provenance-witness",
  "contribution:affordance.resolve-provenance",
  "contribution:affordance.retry-provenance",
  "subscription:sub-1",
];

const SAMPLE_STEP_LABELS = [
  "root.nav",
  "case.view",
  "trace.row",
  "inspector.section",
  "artifact.viewer",
  "composer",
  "status.item",
  "overlay",
  "bottom.panel",
  "relation_provider",
  "affordance_projection.resolve",
  "affordance_projection.retry",
  "subscription",
];

const SAMPLE_REGISTRATION_STEPS = SAMPLE_DISPOSER_LABELS.length;

function renderWorkThroughCell(cell: ShellViewRegistryCell, model: WorkbenchModel): ViewContent {
  const renderer = cell.get()["work"];
  if (renderer === undefined) {
    throw new Error("the work renderer vanished from the shell registry cell");
  }
  return renderer({ snapshot: model.getSnapshot(), observation: observationFor(model) });
}

function expectBaselineClean(
  manager: PluginCompositionManager,
  cell: ShellViewRegistryCell,
  baseRegistry: ReturnType<typeof defaultViewRegistry>,
  model: WorkbenchModel,
  baselineFingerprint: string,
  baselineSnapshot: string,
): void {
  expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(baselineFingerprint);
  expect(JSON.stringify(model.getSnapshot())).toBe(baselineSnapshot);
  const registry = cell.get();
  expect(Object.keys(registry).sort()).toEqual(Object.keys(baseRegistry).sort());
  expect(registry["work"]).toBe(baseRegistry["work"]);
  // No plugin rendering residue in the composed Work view.
  expect(renderToText(renderWorkThroughCell(cell, model).lines)).not.toContain("PROVENANCE LENS");
  // Every DSN-012 slot reads empty after unload.
  const snapshot = model.getSnapshot();
  expect(manager.navEntries()).toEqual([]);
  expect(manager.statusCues(snapshot)).toEqual([]);
  expect(manager.composerCues(snapshot)).toEqual([]);
  expect(manager.bottomPanels(snapshot)).toEqual([]);
  expect(manager.overlayContent(snapshot)).toEqual([]);
  expect(manager.viewExtensions()).toEqual([]);
  expect(manager.affordanceProjections()).toEqual([]);
  expect(
    manager.traceRows(
      {
        trace_id: "trace-gauntlet-c-1",
        unit_id: "u-root",
        event_type: "unit_updated",
        summary: "[gauntlet] unit_updated (c-1)",
        timestamp_ms: 1000,
      },
      snapshot,
    ),
  ).toEqual([]);
  expect(
    manager.inspectorSections(snapshot, FocusRef.for("gauntlet", "WorkItem", "u-root")),
  ).toEqual([]);
  expect(
    manager.artifactViews("sample-log", { id: "a-1", kind: "sample-log", summary: "s" }),
  ).toEqual([]);
  expect(manager.pluginRelations(worldFromSnapshot(snapshot))).toEqual([]);
  expect(() => manager.activateNavEntry("nav.provenance-lens")).toThrow(PluginLifecycleError);
}

describe("T15 plugin lifecycle — mount, exercise, unload to byte-identical baseline (REQ-PLUGIN-003/004, DSN-012)", () => {
  it("mount -> exercise -> unload: fingerprint, model snapshot, shell registry, and every slot return to baseline", () => {
    const model = seededModel();
    const baselineSnapshot = JSON.stringify(model.getSnapshot());
    const baseRegistry = defaultViewRegistry();
    const cell = createShellViewRegistryCell(baseRegistry);
    const manager = new PluginCompositionManager({ snapshots: model, shellViews: cell });
    const baselineFingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());

    const sample = createSamplePlugin();
    const view = manager.mount(sample.manifest, sample.module);
    expect(view.trust).toBe("trusted_in_process");
    expect(view.mode).toBe("in_process");
    expect(view.contributionCount).toBe(SAMPLE_REGISTRATION_STEPS - 1);
    expect(view.subscriptionCount).toBe(1);
    // The subscription was exercised by WorkbenchModel's initial emit.
    expect(sample.probe.snapshotEvents).toEqual([GEN]);

    manager.unload(SAMPLE_PLUGIN_ID);
    expect(manager.isMounted(SAMPLE_PLUGIN_ID)).toBe(false);
    expectBaselineClean(manager, cell, baseRegistry, model, baselineFingerprint, baselineSnapshot);

    // Orphan-handler probe: a live event after unload reaches no plugin listener.
    model.applyEvent(unitEvent("u-root", "Root Unit", "c-orphan-probe", "active"));
    expect(sample.probe.snapshotEvents).toEqual([GEN]);
    // Instrument sensitivity: the probe DETECTS the event in the snapshot itself.
    expect(JSON.stringify(model.getSnapshot())).not.toBe(baselineSnapshot);
    // And still no plugin residue in the changed snapshot's rendering.
    expect(renderToText(renderWorkThroughCell(cell, model).lines)).not.toContain("PROVENANCE LENS");
  });

  it("while mounted, every DSN-012 slot and provider kind serves the representative plugin", () => {
    const model = seededModel();
    const cell = createShellViewRegistryCell(defaultViewRegistry());
    const manager = new PluginCompositionManager({ snapshots: model, shellViews: cell });
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);
    const snapshot = model.getSnapshot();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");

    // root.nav
    const entries = manager.navEntries();
    expect(entries.length).toBe(1);
    expect(entries[0]?.label).toBe("Provenance lens");
    expect(entries[0]?.view).toBe("work");
    manager.activateNavEntry("nav.provenance-lens");
    expect(sample.probe.navActivations).toBe(1);
    expect(() => manager.activateNavEntry("nav.does-not-exist")).toThrow(PluginLifecycleError);

    // case.view — the extension is live in the shell registry cell composition.
    expect(manager.viewExtensions()).toEqual([
      { plugin: SAMPLE_PLUGIN_ID, id: "view.work-provenance", view: "work" },
    ]);
    const composed = renderToText(renderWorkThroughCell(cell, model).lines);
    expect(composed).toContain("PROVENANCE LENS");
    expect(composed).toContain(`gen ${GEN}`);

    // trace.row — renders unit_updated rows, declines others.
    const unitRow = manager.traceRows(
      {
        trace_id: "trace-1",
        unit_id: "u-root",
        event_type: "unit_updated",
        summary: "s",
        timestamp_ms: 1,
      },
      snapshot,
    );
    expect(unitRow.length).toBe(1);
    expect(renderToText([unitRow[0]?.line ?? { spans: [] }])).toContain("witnessed by");
    expect(
      manager.traceRows(
        {
          trace_id: "trace-2",
          unit_id: "u-root",
          event_type: "session_updated",
          summary: "s",
          timestamp_ms: 2,
        },
        snapshot,
      ).length,
    ).toBe(0);

    // inspector.section
    const sections = manager.inspectorSections(snapshot, focus);
    expect(sections.length).toBe(1);
    expect(sections[0]?.section.title).toBe("PLUGIN · PROVENANCE");

    // artifact.viewer — kind-scoped.
    expect(
      manager.artifactViews("sample-log", { id: "a-1", kind: "sample-log", summary: "log line" })
        .length,
    ).toBe(1);
    expect(
      manager.artifactViews("other-kind", { id: "a-2", kind: "other-kind", summary: "x" }).length,
    ).toBe(0);

    // composer + status.item cues (presentation only).
    const composerCues = manager.composerCues(snapshot);
    expect(composerCues.length).toBe(1);
    expect(composerCues[0]?.cue.text).toContain("provenance lens watching");
    const statusCues = manager.statusCues(snapshot);
    expect(statusCues.length).toBe(1);
    expect(statusCues[0]?.cue.text).toContain("attention item(s)");

    // bottom.panel + overlay.
    expect(manager.bottomPanels(snapshot).length).toBe(1);
    expect(manager.overlayContent(snapshot).length).toBe(1);

    // relation provider — namespaced, plugin-tagged edges over units with evidence.
    const edges = manager.pluginRelations(worldFromSnapshot(snapshot));
    expect(edges.length).toBeGreaterThanOrEqual(1);
    for (const edge of edges) {
      expect(edge.kind.startsWith(`plugin.${SAMPLE_PLUGIN_ID}.`)).toBe(true);
      expect(edge.provider).toBe(SAMPLE_PLUGIN_ID);
    }

    // affordance projections over CANONICAL operations only.
    const projections = manager.affordanceProjections();
    expect(projections.map((projection) => projection.operation).sort()).toEqual([
      "unit.resolve",
      "unit.retry",
    ]);
    expect(projections.every((projection) => projection.plugin === SAMPLE_PLUGIN_ID)).toBe(true);

    // The projection resolves through the SAME pure T09 resolver.
    const descriptor = manager.projectionDescriptor(
      "affordance.resolve-provenance",
      focus,
      observationFor(model),
      { session: { actor: "operator", authenticated: true } },
    );
    expect(descriptor.operation).toBe("unit.resolve");
    expect(descriptor.presentation_hint).toBe("provenance-lens:resolve-presentation");
    expect(() =>
      manager.projectionDescriptor("affordance.does-not-exist", focus, observationFor(model)),
    ).toThrow(PluginLifecycleError);

    // Plugin-private state is live while mounted (fingerprint carries its keys).
    const fingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());
    expect(fingerprint).toContain(`"plugin":"${SAMPLE_PLUGIN_ID}","key":"snapshots_seen"`);
    expect(fingerprint).toContain(`"plugin":"${SAMPLE_PLUGIN_ID}","key":"last_generation"`);

    manager.unload(SAMPLE_PLUGIN_ID);
  });
});

describe("T15 Attack 1 — a plugin that throws halfway through load is fully unwound", () => {
  it("every injection point 0..N unwinds exactly its partial effects and lands on a clean baseline", () => {
    for (let failAfter = 0; failAfter <= SAMPLE_REGISTRATION_STEPS; failAfter += 1) {
      const model = seededModel();
      const baselineSnapshot = JSON.stringify(model.getSnapshot());
      const baseRegistry = defaultViewRegistry();
      const cell = createShellViewRegistryCell(baseRegistry);
      const manager = new PluginCompositionManager({ snapshots: model, shellViews: cell });
      const baselineFingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());

      const sample = createSamplePlugin({ failAfterRegistrations: failAfter });
      expect(() => manager.mount(sample.manifest, sample.module)).toThrow(PluginActivationError);

      // Exactly failAfter steps completed; the mount is atomic: nothing remains.
      expect(sample.probe.registrationOrder.length).toBe(failAfter);
      expect(manager.isMounted(SAMPLE_PLUGIN_ID)).toBe(false);
      expect(manager.pluginIds()).toEqual([]);
      expectBaselineClean(
        manager,
        cell,
        baseRegistry,
        model,
        baselineFingerprint,
        baselineSnapshot,
      );

      // The unwind ran in reverse registration order with deterministic disposers.
      const expectedTrace = sample.probe.registrationOrder
        .map((stepLabel) => {
          const index = SAMPLE_STEP_LABELS.indexOf(stepLabel);
          const label = SAMPLE_DISPOSER_LABELS[index];
          if (label === undefined) {
            throw new Error(`unknown registration step '${stepLabel}'`);
          }
          return label;
        })
        .reverse()
        .map((label) => `${label} disposed`);
      expect(manager.unwindTrace).toEqual(expectedTrace);

      // No orphan subscription: an event after the failed mount reaches nobody.
      const eventsBefore = sample.probe.snapshotEvents.length;
      model.applyEvent(unitEvent("u-root", "Root Unit", `c-attack1-${failAfter}`, "active"));
      expect(sample.probe.snapshotEvents.length).toBe(eventsBefore);

      // Unloading the never-mounted plugin stays idempotent.
      expect(manager.unload(SAMPLE_PLUGIN_ID)).toEqual({ unloaded: false, reason: "not_mounted" });
    }
  });

  it("the typed activation error preserves the failure locus (the injected cause)", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const sample = createSamplePlugin({ failAfterRegistrations: 3 });
    try {
      manager.mount(sample.manifest, sample.module);
      throw new Error("unreachable: mount must throw");
    } catch (error) {
      expect(error instanceof PluginActivationError).toBe(true);
      const activationError = error as PluginActivationError;
      expect(activationError.code).toBe("activation_failed");
      expect(activationError.plugin).toBe(SAMPLE_PLUGIN_ID);
      expect((activationError.cause as Error).message).toContain("failed halfway");
      expect(manager.unwindTrace).toEqual([
        "contribution:trace.provenance-rows disposed",
        "contribution:view.work-provenance disposed",
        "contribution:nav.provenance-lens disposed",
      ]);
    }
  });
});

describe("T15 — deny matrix re-run over the plugin affordance-projection route (T09 trigger-records)", () => {
  const OPERATOR = { actor: "operator", token: "tok-t15" };

  function allowedUnit(): StubUnitRecord {
    return {
      unit_id: "ABC-123",
      status: "active",
      tier: "t1",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: "TKT-001",
    };
  }

  function authorityDeniedUnit(): StubUnitRecord {
    return {
      unit_id: "DEF-456",
      status: "active",
      tier: "t1",
      settled: false,
      authority: "revoked",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: "TKT-002",
    };
  }

  function stubFor(record: StubUnitRecord): CanonicalSurfaceStub {
    return new CanonicalSurfaceStub({
      run_id: "R-T15",
      bound_actor: "operator",
      session_token: "tok-t15",
    }).addUnit(record);
  }

  function managerWithPlugin(): PluginCompositionManager {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);
    return manager;
  }

  function invocation(
    operation: string,
    focus: FocusRef,
    estimate: { tokens: number; cost_usd_micro: number },
  ) {
    return {
      operation,
      focus,
      session: OPERATOR,
      payload: { note: "plugin affordance-projection route", run: "R-T15" },
      resolution: { payment_estimates: { [operation]: estimate } },
    };
  }

  it("one allowed operation through the plugin projection route is admitted once and re-enters idempotently", async () => {
    const stub = stubFor(allowedUnit());
    const manager = managerWithPlugin();
    const gateway = new OperationGateway(stub.asInProcessRoute());

    const projection = manager
      .affordanceProjections()
      .find((candidate) => candidate.operation === "unit.resolve");
    if (projection === undefined) {
      throw new Error("the representative plugin must project unit.resolve");
    }
    const focus = FocusRef.for("gauntlet", "WorkItem", "ABC-123");

    // Presentation resolution through the SAME pure T09 resolver.
    const descriptor = manager.projectionDescriptor(
      projection.id,
      focus,
      stub.observe({ unit: "ABC-123" }),
      { session: { actor: "operator", authenticated: true } },
    );
    expect(descriptor.status).toBe("requires_attention");
    expect(descriptor.required_attention_item).toBe("TKT-001");

    const request = invocation(projection.operation, focus, { tokens: 1200, cost_usd_micro: 3000 });
    const first = await gateway.invoke(request);
    if (first.class !== "accepted") {
      throw new Error(`expected acceptance through the plugin route, got ${first.class}`);
    }
    expect(first.committed_now).toBe(true);
    expect(first.tickets_resolved).toEqual(["TKT-001"]);
    expect(stub.sideEffects).toEqual({
      settlements_committed: 1,
      tickets_resolved: 1,
      admission_attempts: 1,
    });

    // T09 trigger-record 1 re-check (anchored shape): the SAME deterministic
    // key re-enters the durable settlement with committed_now=false — exactly
    // one commit, never two.
    const reDrive = await gateway.invoke(request);
    if (reDrive.class !== "accepted") {
      throw new Error(`expected idempotent re-entry, got ${reDrive.class}`);
    }
    expect(reDrive.committed_now).toBe(false);
    expect(reDrive.idempotency_key).toBe(first.idempotency_key);
    expect(stub.sideEffects.settlements_committed).toBe(1);

    manager.unload(SAMPLE_PLUGIN_ID);
  });

  it("denied operations through the plugin route earn the same typed canonical denial as the direct gateway route, with zero side effects", async () => {
    // Denied drive 1: authority-revoked resolve. Denied drive 2: readiness-not-earned retry.
    const deniedStub = stubFor(authorityDeniedUnit());
    const retryStub = stubFor(allowedUnit());
    const pluginDenied = managerWithPlugin();
    const pluginRetry = managerWithPlugin();
    const deniedGateway = new OperationGateway(deniedStub.asInProcessRoute());
    const retryGateway = new OperationGateway(retryStub.asInProcessRoute());
    // Reference routes: identical scenarios with NO plugin anywhere.
    const referenceDeniedGateway = new OperationGateway(
      stubFor(authorityDeniedUnit()).asInProcessRoute(),
    );
    const referenceRetryGateway = new OperationGateway(stubFor(allowedUnit()).asInProcessRoute());

    const resolveProjection = pluginDenied
      .affordanceProjections()
      .find((candidate) => candidate.operation === "unit.resolve");
    const retryProjection = pluginRetry
      .affordanceProjections()
      .find((candidate) => candidate.operation === "unit.retry");
    if (resolveProjection === undefined || retryProjection === undefined) {
      throw new Error("the representative plugin must project both canonical operations");
    }

    const authorityFocus = FocusRef.for("gauntlet", "WorkItem", "DEF-456");
    const retryFocus = FocusRef.for("gauntlet", "WorkItem", "ABC-123");

    // The plugin projection itself resolves the denial honestly first.
    const descriptor = pluginDenied.projectionDescriptor(
      resolveProjection.id,
      authorityFocus,
      deniedStub.observe({ unit: "DEF-456" }),
      { session: { actor: "operator", authenticated: true } },
    );
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toEqual(["authority_not_permitted"]);

    const pluginAuthorityOutcome = await deniedGateway.invoke(
      invocation(resolveProjection.operation, authorityFocus, {
        tokens: 1200,
        cost_usd_micro: 3000,
      }),
    );
    const pluginRetryOutcome = await retryGateway.invoke(
      invocation(retryProjection.operation, retryFocus, { tokens: 800, cost_usd_micro: 2000 }),
    );
    const referenceAuthorityOutcome = await referenceDeniedGateway.invoke(
      invocation(resolveProjection.operation, authorityFocus, {
        tokens: 1200,
        cost_usd_micro: 3000,
      }),
    );
    const referenceRetryOutcome = await referenceRetryGateway.invoke(
      invocation(retryProjection.operation, retryFocus, { tokens: 800, cost_usd_micro: 2000 }),
    );

    for (const outcome of [pluginAuthorityOutcome, pluginRetryOutcome]) {
      if (outcome.class !== "refused") {
        throw new Error(`expected a typed refusal through the plugin route, got ${outcome.class}`);
      }
      expect(outcome.stale).toBe(true);
      expect(outcome.code).toBe("stale_affordance_denied");
    }
    if (pluginAuthorityOutcome.class === "refused") {
      expect(pluginAuthorityOutcome.reason_codes).toContain("authority_not_permitted");
    }
    if (pluginRetryOutcome.class === "refused") {
      expect(pluginRetryOutcome.reason_codes).toContain("readiness_not_earned");
    }

    // The SAME typed canonical denial as the identical non-plugin request.
    expect(pluginAuthorityOutcome).toEqual(referenceAuthorityOutcome);
    expect(pluginRetryOutcome).toEqual(referenceRetryOutcome);

    // Zero side effects: no denied drive reached admission on any surface.
    expect(deniedStub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
    expect(retryStub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });

    // The manager exposes no invocation path at all: plugins cannot bypass the
    // gateway because they never hold one.
    const managerSurface = pluginDenied as unknown as Record<string, unknown>;
    expect(managerSurface["invoke"]).toBeUndefined();
    expect(managerSurface["perform"]).toBeUndefined();

    pluginDenied.unload(SAMPLE_PLUGIN_ID);
    pluginRetry.unload(SAMPLE_PLUGIN_ID);
    expect(pluginDenied.affordanceProjections()).toEqual([]);
    expect(() =>
      pluginDenied.projectionDescriptor(
        resolveProjection.id,
        authorityFocus,
        deniedStub.observe({ unit: "DEF-456" }),
      ),
    ).toThrow(PluginLifecycleError);
  });
});

describe("T15 — double mount, unload idempotence, and remount cycles", () => {
  it("double mount is refused with the second module never activated; unload is idempotent", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const first = createSamplePlugin();
    manager.mount(first.manifest, first.module);
    const fingerprintWhileMounted = serializePluginSurfaceFingerprint(manager.fingerprint());

    const second = createSamplePlugin();
    expect(() => manager.mount(second.manifest, second.module)).toThrow(PluginLifecycleError);
    expect(second.probe.registrationOrder.length).toBe(0);
    expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(fingerprintWhileMounted);

    expect(manager.unload(SAMPLE_PLUGIN_ID)).toEqual({ unloaded: true });
    expect(manager.unload(SAMPLE_PLUGIN_ID)).toEqual({ unloaded: false, reason: "not_mounted" });
  });

  it("remount cycles each return to baseline with no accumulation (prereg transfer test L1)", () => {
    const model = seededModel();
    const baselineSnapshot = JSON.stringify(model.getSnapshot());
    const baseRegistry = defaultViewRegistry();
    const cell = createShellViewRegistryCell(baseRegistry);
    const manager = new PluginCompositionManager({ snapshots: model, shellViews: cell });
    const baselineFingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());

    for (let cycle = 1; cycle <= 2; cycle += 1) {
      const sample = createSamplePlugin();
      manager.mount(sample.manifest, sample.module);
      expect(sample.probe.snapshotEvents.length).toBeGreaterThanOrEqual(1);
      manager.unload(SAMPLE_PLUGIN_ID);
      expectBaselineClean(
        manager,
        cell,
        baseRegistry,
        model,
        baselineFingerprint,
        baselineSnapshot,
      );
      // The fresh module's own listener saw exactly its cycle's initial emit:
      // no cross-cycle listener residue.
      expect(sample.probe.snapshotEvents.length).toBe(1);
      expect(manager.unwindTrace.every((entry) => entry.endsWith(" disposed"))).toBe(true);
    }
    expect(manager.isMounted(SAMPLE_PLUGIN_ID)).toBe(false);
  });
});

describe("T15 — cross-plugin isolation and slot-check refusals (typed seam only)", () => {
  it("contribution ids are unique across plugins; a collision refuses the latecomer and leaves the owner intact", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const first = createSamplePlugin();
    manager.mount(first.manifest, first.module);

    const collidingModule = {
      activate: (context: PluginContext) => {
        context.register({
          kind: "view",
          slot: "root.nav",
          id: "nav.provenance-lens",
          label: "Collision attempt",
          view: "work",
        });
      },
    };
    expect(() =>
      manager.mount(
        { id: "sample.collider", name: "Collider", version: "1.0.0", trust: "trusted_in_process" },
        collidingModule,
      ),
    ).toThrow(PluginContributionError);
    expect(manager.isMounted("sample.collider")).toBe(false);
    expect(manager.navEntries().length).toBe(1);
    expect(manager.navEntries()[0]?.plugin).toBe(SAMPLE_PLUGIN_ID);
    manager.unload(SAMPLE_PLUGIN_ID);
  });

  it("unloading one plugin leaves another plugin's contributions and state intact", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const otherModule = {
      activate: (context: PluginContext) => {
        context.register({
          kind: "cue_provider",
          slot: "status.item",
          id: "status.other-cue",
          cue: () => ({ text: "other plugin cue", token: "structure.neutral" }),
        });
      },
    };
    manager.mount(
      {
        id: "sample.other-lens",
        name: "Other lens",
        version: "1.0.0",
        trust: "trusted_in_process",
      },
      otherModule,
    );
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);
    const snapshot = coldModel().getSnapshot();
    expect(manager.statusCues(snapshot).length).toBe(2);

    manager.unload(SAMPLE_PLUGIN_ID);
    expect(manager.isMounted("sample.other-lens")).toBe(true);
    const remaining = manager.statusCues(snapshot);
    expect(remaining.length).toBe(1);
    expect(remaining[0]?.plugin).toBe("sample.other-lens");
    expect(manager.unload("sample.other-lens")).toEqual({ unloaded: true });
  });

  it("a plugin cannot register after its activation closed (no stash-and-register-later)", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    let stashed: PluginContext | null = null;
    const module = {
      activate: (context: PluginContext) => {
        stashed = context;
        context.register({
          kind: "view",
          slot: "bottom.panel",
          id: "panel.stash-probe",
          render: () => ({ lines: [], records: [] }),
        });
      },
    };
    manager.mount(
      { id: "sample.stasher", name: "Stasher", version: "1.0.0", trust: "trusted_in_process" },
      module,
    );
    if (stashed === null) {
      throw new Error("the module must have captured its context");
    }
    const context = stashed as PluginContext | null;
    if (context === null) {
      throw new Error("unreachable: context narrowed away");
    }
    expect(() =>
      context.register({
        kind: "view",
        slot: "bottom.panel",
        id: "panel.post-activation",
        render: () => ({ lines: [], records: [] }),
      }),
    ).toThrow(PluginLifecycleError);
    expect(() => context.onSnapshot(() => undefined)).toThrow(PluginLifecycleError);
    manager.unload("sample.stasher");
  });

  it("plugin relation kinds must stay inside the plugin's own namespace", () => {
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const module = {
      activate: (context: PluginContext) => {
        context.register({
          kind: "relation_provider",
          id: "relations.impersonator",
          relations: () => [
            {
              // Impersonates a canonical source-typed relation kind.
              source_ref: FocusRef.for("gauntlet", "WorkItem", "ABC-123"),
              target_ref: FocusRef.for("gauntlet", "Evidence", "ev-1"),
              kind: "has_evidence",
            },
          ],
        });
      },
    };
    manager.mount(
      {
        id: "sample.impersonator",
        name: "Impersonator",
        version: "1.0.0",
        trust: "trusted_in_process",
      },
      module,
    );
    expect(() => manager.pluginRelations(worldFromSnapshot(coldModel().getSnapshot()))).toThrow(
      PluginContributionError,
    );
    manager.unload("sample.impersonator");
  });

  it("roadmap view targets are refused at the plugin seam and unwound (DSN-015 defense in depth)", () => {
    const baseRegistry = defaultViewRegistry();
    const cell = createShellViewRegistryCell(baseRegistry);
    const manager = new PluginCompositionManager({ snapshots: coldModel(), shellViews: cell });
    const module = {
      activate: (context: PluginContext) => {
        // A nav entry first (succeeds), then the roadmap attempt (refused):
        // the failed mount must unwind BOTH.
        context.register({
          kind: "view",
          slot: "bottom.panel",
          id: "panel.roadmap-probe",
          render: () => ({ lines: [], records: [] }),
        });
        context.register({
          kind: "view",
          slot: "root.nav",
          id: "nav.harness-attempt",
          label: "Harness",
          view: "harness" as ViewId,
        });
      },
    };
    expect(() =>
      manager.mount(
        { id: "sample.roadmap", name: "Roadmap", version: "1.0.0", trust: "trusted_in_process" },
        module,
      ),
    ).toThrow(PluginContributionError);
    expect(manager.isMounted("sample.roadmap")).toBe(false);
    // The failed mount unwound: the shell registry cell never kept a roadmap id.
    expect(Object.keys(cell.get()).sort()).toEqual(Object.keys(baseRegistry).sort());
    expect(cell.get()["work"]).toBe(baseRegistry["work"]);
    expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(
      serializePluginSurfaceFingerprint(
        new PluginCompositionManager({ snapshots: coldModel(), shellViews: cell }).fingerprint(),
      ),
    );
  });
});

// --- Correction Round 1 (adversarial verification findings) -------------------------

type Round1Listener = (snapshot: FullWorkbenchSnapshot) => void;

/**
 * MINOR-1 instrument: a host source whose unsubscribe throws either AFTER a
 * successful removal ("recoverable": a second call fails with a DIFFERENT,
 * already-removed error) or BEFORE any removal ("stuck": every call fails
 * identically, the listener is genuinely undetachable through the handle).
 */
function round1Source(mode: "recoverable" | "stuck"): {
  subscribe(listener: Round1Listener): () => void;
  emit(snapshot: FullWorkbenchSnapshot): void;
  count(): number;
} {
  const listeners = new Set<Round1Listener>();
  return {
    subscribe(listener: Round1Listener): () => void {
      listeners.add(listener);
      return () => {
        if (mode === "stuck") {
          throw new Error("host unsubscribe failed before removal");
        }
        const removed = listeners.delete(listener);
        if (!removed) {
          throw new Error("unsubscribe called for a listener that is already removed");
        }
        throw new Error("host unsubscribe failed after removal");
      };
    },
    emit(snapshot: FullWorkbenchSnapshot): void {
      for (const listener of [...listeners]) {
        listener(snapshot);
      }
    },
    count: () => listeners.size,
  };
}

describe("T15 Correction Round 1 BLOCKING-1 — same-view extension disposal recomposes the registry", () => {
  it("cross-plugin non-LIFO unload: the earlier plugin's unload keeps the later plugin's extension live and its own closure dead; final unload restores the exact baseline renderer", () => {
    const baseRegistry = defaultViewRegistry();
    const cell = createShellViewRegistryCell(baseRegistry);
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: cell,
    });

    const runs = { a: 0, b: 0 };
    const extensionModule = (
      id: "a" | "b",
      key: keyof typeof runs,
    ): {
      manifest: PluginManifest;
      module: TrustedPluginModule;
    } => ({
      manifest: {
        id: `sample.ext-${id}`,
        name: `Ext ${id}`,
        version: "1.0.0",
        trust: "trusted_in_process",
      },
      module: {
        activate: (context: PluginContext) => {
          context.register({
            kind: "view",
            slot: "case.view",
            id: `view.ext-${id}`,
            view: "work",
            render: () => {
              runs[key] += 1;
              return { lines: [{ id: `l.${id}`, spans: [{ text: `EXT:${id}` }] }], records: [] };
            },
          });
        },
      },
    });
    const a = extensionModule("a", "a");
    const b = extensionModule("b", "b");
    manager.mount(a.manifest, a.module);
    manager.mount(b.manifest, b.module);

    // Both extensions serve while mounted.
    let composed = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    expect(composed).toContain("EXT:a");
    expect(composed).toContain("EXT:b");
    expect(runs.a).toBe(1);
    expect(runs.b).toBe(1);

    // Non-LIFO: unload the EARLIER plugin while the later stays mounted.
    manager.unload("sample.ext-a");
    composed = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    // B's extension still serves, A's closure NEVER runs again.
    expect(composed).toContain("EXT:b");
    expect(composed).not.toContain("EXT:a");
    expect(runs.a).toBe(1);
    expect(runs.b).toBe(2);
    expect(manager.viewExtensions().map((extension) => extension.plugin)).toEqual(["sample.ext-b"]);

    // Final unload restores the EXACT baseline renderer identity.
    manager.unload("sample.ext-b");
    expect(cell.get()["work"]).toBe(baseRegistry["work"]);
    composed = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    expect(composed).not.toContain("EXT:a");
    expect(composed).not.toContain("EXT:b");
    expect(runs.a).toBe(1);
    expect(runs.b).toBe(2);
  });

  it("one plugin self-disposes its FIRST same-view extension out of order: the second keeps serving and unload restores the baseline", () => {
    const baseRegistry = defaultViewRegistry();
    const cell = createShellViewRegistryCell(baseRegistry);
    const manager = new PluginCompositionManager({
      snapshots: coldModel(),
      shellViews: cell,
    });

    const runs = { first: 0, second: 0 };
    let firstDispose: (() => void) | null = null;
    const module = {
      activate: (context: PluginContext) => {
        const first = context.register({
          kind: "view",
          slot: "case.view",
          id: "view.self-first",
          view: "work",
          render: () => {
            runs.first += 1;
            return { lines: [{ id: "l.f", spans: [{ text: "SELF:FIRST" }] }], records: [] };
          },
        });
        context.register({
          kind: "view",
          slot: "case.view",
          id: "view.self-second",
          view: "work",
          render: () => {
            runs.second += 1;
            return { lines: [{ id: "l.s", spans: [{ text: "SELF:SECOND" }] }], records: [] };
          },
        });
        firstDispose = first.dispose;
      },
    };
    manager.mount(
      { id: "sample.self-disposer", name: "Self", version: "1.0.0", trust: "trusted_in_process" },
      module,
    );

    // Both extensions serve while mounted.
    let composed = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    expect(composed).toContain("SELF:FIRST");
    expect(composed).toContain("SELF:SECOND");
    expect(runs.first).toBe(1);
    expect(runs.second).toBe(1);

    // Out-of-order self-disposal of the FIRST extension while both are
    // mounted. (The handle is re-read through a widened alias: the assignment
    // happened inside the activation closure, outside TypeScript's flow
    // analysis.)
    const disposeFirst = firstDispose as (() => void) | null;
    disposeFirst?.();
    composed = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    expect(composed).not.toContain("SELF:FIRST");
    expect(composed).toContain("SELF:SECOND");
    expect(runs.first).toBe(1);
    expect(runs.second).toBe(2);
    expect(manager.viewExtensions().map((extension) => extension.id)).toEqual(["view.self-second"]);

    manager.unload("sample.self-disposer");
    expect(cell.get()["work"]).toBe(baseRegistry["work"]);
    const final = renderToText(renderWorkThroughCell(cell, coldModel()).lines);
    expect(final).not.toContain("SELF:FIRST");
    expect(final).not.toContain("SELF:SECOND");
    expect(runs.first).toBe(1);
    expect(runs.second).toBe(2);
  });
});

describe("T15 Correction Round 1 MINOR-1 — throwing host unsubscribes are contained honestly", () => {
  it("recoverable unsubscribe (removes, then throws): the plugin listener is fully disposed, ledger cleaned, unload reports success, residual recorded", () => {
    const source = round1Source("recoverable");
    const manager = new PluginCompositionManager({
      snapshots: source,
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);

    const result = manager.unload(SAMPLE_PLUGIN_ID);
    expect(result).toEqual({ unloaded: true });
    // The host Set actually shrank: the dispatcher was released despite the throw.
    expect(source.count()).toBe(0);
    expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(
      serializePluginSurfaceFingerprint(
        new PluginCompositionManager({
          snapshots: source,
          shellViews: createShellViewRegistryCell(defaultViewRegistry()),
        }).fingerprint(),
      ),
    );
    // The throwing host call is still recorded as a typed residual — never
    // laundered into an untyped success.
    expect(manager.detachResiduals.length).toBe(1);
    expect(manager.detachResiduals[0]?.kind).toBe("snapshot_dispatcher_detach_failed");
  });

  it("stuck unsubscribe (throws before removal every time): the plugin listener is still fully disposed, the manager's inert dispatcher residual is typed and inspectable — never a silent orphan", () => {
    const source = round1Source("stuck");
    const manager = new PluginCompositionManager({
      snapshots: source,
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);
    // Delivery while mounted (the fanout dispatcher relays host emits; this
    // bare source does not replay on subscribe, so exactly one event lands).
    source.emit(coldModel().getSnapshot());
    expect(sample.probe.snapshotEvents.length).toBe(1);

    // The PLUGIN's unwind cannot be defeated by the broken host handle: the
    // fanout delete is deterministic, so the unload succeeds cleanly.
    const result = manager.unload(SAMPLE_PLUGIN_ID);
    expect(result).toEqual({ unloaded: true });
    expect(manager.isMounted(SAMPLE_PLUGIN_ID)).toBe(false);
    expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(
      serializePluginSurfaceFingerprint(
        new PluginCompositionManager({
          snapshots: source,
          shellViews: createShellViewRegistryCell(defaultViewRegistry()),
        }).fingerprint(),
      ),
    );

    // The residual — the host retaining the manager's dispatcher — is a TYPED,
    // inspectable record, never a swallowed failure.
    expect(manager.detachResiduals.length).toBe(1);
    expect(manager.detachResiduals[0]?.kind).toBe("snapshot_dispatcher_detach_failed");

    // CONTAINMENT: the retained dispatcher is inert — a host emit after unload
    // reaches zero plugin listeners (the plugin closure never runs again).
    const eventsBefore = sample.probe.snapshotEvents.length;
    source.emit(coldModel().getSnapshot());
    expect(sample.probe.snapshotEvents.length).toBe(eventsBefore);
  });
});
