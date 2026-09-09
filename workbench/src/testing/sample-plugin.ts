/**
 * The representative T15 plugin (plan T15 step 4): a trusted in-process module
 * contributing to MULTIPLE DSN-012 semantic surfaces — root.nav, case.view,
 * trace.row, inspector.section, artifact.viewer, composer, status.item,
 * bottom.panel, overlay, plus the relation-provider and affordance-projection
 * provider kinds — and fully unloading back to baseline.
 *
 * Presentation only: every render reads the immutable snapshot / projection
 * world; nothing here owns canonical state. The affordance projections carry
 * canonical operation IDs from the T09 registry (`unit.resolve`,
 * `unit.retry`); this plugin has no invocation path — the host's gateway
 * admits, the plugin presents.
 *
 * `createSamplePlugin({ failAfterRegistrations })` deterministically injects
 * a halfway failure after exactly N successful registration steps (prereg
 * transfer test L2: failure injection at every registration step).
 */

import { FocusRef } from "../focus";
import { OPERATION_IDS } from "../operation-registry";
import type {
  PluginContribution,
  PluginManifest,
  PluginRelationEdgeInit,
  TrustedPluginModule,
} from "../plugins";

export const SAMPLE_PLUGIN_ID = "sample.provenance-lens";
const RELATION_KIND_PREFIX = `plugin.${SAMPLE_PLUGIN_ID}.`;

/** Observable activation/disposal trace for tests (deterministic instrument). */
export interface SamplePluginProbe {
  readonly pluginId: string;
  /** Labels of registration steps that completed, in activation order. */
  readonly registrationOrder: string[];
  /** Generation ids observed by the snapshot subscription, in arrival order. */
  readonly snapshotEvents: string[];
  navActivations: number;
  /** The injected failure point (after this many successful steps), or null. */
  readonly injectedFailureAfter: number | null;
}

export interface SamplePlugin {
  readonly manifest: PluginManifest;
  readonly module: TrustedPluginModule;
  readonly probe: SamplePluginProbe;
}

export function samplePluginManifest(): PluginManifest {
  return {
    id: SAMPLE_PLUGIN_ID,
    name: "Provenance Lens (representative T15 plugin)",
    version: "1.0.0",
    trust: "trusted_in_process",
  };
}

/**
 * Build the representative plugin. Registration order is fixed:
 *   1 nav entry · 2 view extension · 3 trace row · 4 inspector section
 *   5 artifact viewer · 6 composer cue · 7 status cue · 8 overlay
 *   9 bottom panel · 10 relation provider · 11 affordance projection (resolve)
 *   12 affordance projection (retry) · 13 snapshot subscription
 */
export function createSamplePlugin(
  options: { readonly failAfterRegistrations?: number | undefined } = {},
): SamplePlugin {
  const failAfter = options.failAfterRegistrations ?? null;

  const probe: SamplePluginProbe = {
    pluginId: SAMPLE_PLUGIN_ID,
    registrationOrder: [],
    snapshotEvents: [],
    navActivations: 0,
    injectedFailureAfter: failAfter,
  };

  const module: TrustedPluginModule = {
    activate: (context) => {
      let snapshotsSeen = 0;

      // Injection point 0: fail before any registration exists.
      if (failAfter === 0) {
        throw new Error("sample plugin failed halfway (injected after 0 registration step(s))");
      }

      const step = (label: string, register: () => void): void => {
        register();
        probe.registrationOrder.push(label);
        if (failAfter !== null && probe.registrationOrder.length === failAfter) {
          throw new Error(
            `sample plugin failed halfway (injected after ${failAfter} registration step(s))`,
          );
        }
      };

      // 1. root.nav — navigator entry with an activation handler.
      step("root.nav", () => {
        context.register({
          kind: "view",
          slot: "root.nav",
          id: "nav.provenance-lens",
          label: "Provenance lens",
          view: "work",
          hint: "plugin-provided Case outline entry",
          activate: () => {
            probe.navActivations += 1;
            context.state.set("nav_activations", probe.navActivations);
          },
        });
      });

      // 2. case.view — View extension appended to the Work view through the
      //    typed registerShellView seam.
      step("case.view", () => {
        context.register({
          kind: "view",
          slot: "case.view",
          id: "view.work-provenance",
          view: "work",
          render: (snapshot) => ({
            lines: [
              {
                id: "plugin.sample.provenance",
                spans: [
                  { text: "PROVENANCE LENS ", emphasis: "bold" },
                  {
                    text: `gen ${snapshot.generation} · ${snapshot.trace_records.length} trace events · ${snapshot.evidence.length} evidence records`,
                    emphasis: "dim",
                  },
                ],
              },
            ],
            records: [
              {
                id: "plugin.sample.provenance-record",
                kind: "relation",
                fields: {
                  plugin: SAMPLE_PLUGIN_ID,
                  generation: snapshot.generation,
                  trace_events: String(snapshot.trace_records.length),
                },
              },
            ],
          }),
        });
      });

      // 3. trace.row — Focus renderer annotating unit events in Trace.
      step("trace.row", () => {
        context.register({
          kind: "focus_renderer",
          slot: "trace.row",
          id: "trace.provenance-rows",
          render: (record) => {
            if (record.event_type !== "unit_updated") {
              return null;
            }
            return {
              id: `plugin.sample.trace.${record.trace_id}`,
              spans: [
                { text: `provenance · `, emphasis: "dim" },
                { text: `${record.event_type} `, emphasis: "bold" },
                { text: `unit ${record.unit_id} witnessed by ${SAMPLE_PLUGIN_ID}` },
              ],
            };
          },
        });
      });

      // 4. inspector.section — provenance section for the selected Focus.
      step("inspector.section", () => {
        context.register({
          kind: "inspector_section",
          slot: "inspector.section",
          id: "inspector.provenance",
          render: ({ snapshot, focus }) => ({
            id: "plugin.sample.section-provenance",
            title: "PLUGIN · PROVENANCE",
            lines: [
              {
                id: "plugin.sample.section.focus",
                spans: [{ text: "focus  ", emphasis: "dim" }, { text: focus.id }],
              },
              {
                id: "plugin.sample.section.generation",
                spans: [
                  { text: "seen at ", emphasis: "dim" },
                  { text: `generation ${snapshot.generation}` },
                ],
              },
            ],
            records: [],
          }),
        });
      });

      // 5. artifact.viewer — viewer for the plugin's own artifact kind.
      step("artifact.viewer", () => {
        context.register({
          kind: "artifact_viewer",
          slot: "artifact.viewer",
          id: "artifact.sample-log",
          artifact_kind: "sample-log",
          view: (artifact) => ({
            lines: [
              {
                id: `plugin.sample.artifact.${artifact.id}`,
                spans: [
                  { text: "SAMPLE LOG ", emphasis: "bold" },
                  { text: artifact.summary, emphasis: "normal" },
                ],
              },
            ],
            records: [
              {
                id: `plugin.sample.artifact-record.${artifact.id}`,
                kind: "relation",
                fields: {
                  artifact_kind: artifact.kind,
                  plugin: SAMPLE_PLUGIN_ID,
                  summary: artifact.summary,
                },
              },
            ],
          }),
        });
      });

      // 6. composer — presentation-only cue; composer STATE is never touched.
      step("composer", () => {
        context.register({
          kind: "cue_provider",
          slot: "composer",
          id: "composer.provenance-cue",
          cue: (snapshot) => ({
            text: `provenance lens watching generation ${snapshot.generation}`,
            token: "structure.neutral",
          }),
        });
      });

      // 7. status.item — status-bar cue provider.
      step("status.item", () => {
        context.register({
          kind: "cue_provider",
          slot: "status.item",
          id: "status.provenance-cue",
          cue: (snapshot) => ({
            text: `provenance: ${snapshot.attention.length} attention item(s)`,
            token: "attention.required",
          }),
        });
      });

      // 8. overlay — overlay content provider reading the plugin's own state.
      step("overlay", () => {
        context.register({
          kind: "view",
          slot: "overlay",
          id: "overlay.provenance",
          render: () => ({
            lines: [
              {
                id: "plugin.sample.overlay",
                spans: [
                  { text: "PROVENANCE LENS OVERLAY ", emphasis: "bold" },
                  { text: `snapshots observed: ${String(snapshotsSeen)}`, emphasis: "dim" },
                ],
              },
            ],
            records: [],
          }),
        });
      });

      // 9. bottom.panel — lower-surface panel provider.
      step("bottom.panel", () => {
        context.register({
          kind: "view",
          slot: "bottom.panel",
          id: "panel.provenance",
          render: (snapshot) => ({
            lines: [
              {
                id: "plugin.sample.panel",
                spans: [
                  { text: "provenance panel ", emphasis: "dim" },
                  { text: `${snapshot.units.length} units in the current projection` },
                ],
              },
            ],
            records: [],
          }),
        });
      });

      // 10. relation provider — plugin-namespaced presentation edges.
      step("relation_provider", () => {
        context.register({
          kind: "relation_provider",
          id: "relations.provenance-witness",
          relations: (world): readonly PluginRelationEdgeInit[] => {
            const edges: PluginRelationEdgeInit[] = [];
            for (const unit of world.units) {
              const witness = world.evidence.find(
                (candidate) => candidate.unit_id === unit.unit_id,
              );
              if (witness === undefined) {
                continue;
              }
              edges.push({
                source_ref: FocusRef.for(unit.source_id, "WorkItem", unit.unit_id),
                target_ref: FocusRef.for(witness.source_id, "Evidence", witness.evidence_id),
                kind: `${RELATION_KIND_PREFIX}witness`,
              });
            }
            return edges;
          },
        });
      });

      // 11. affordance projection over the CANONICAL unit.resolve operation.
      step("affordance_projection.resolve", () => {
        context.register({
          kind: "affordance_projection",
          id: "affordance.resolve-provenance",
          operation: OPERATION_IDS.UNIT_RESOLVE,
          presentation_hint: "provenance-lens:resolve-presentation",
          estimate: { tokens: 1200, cost_usd_micro: 3000 },
        });
      });

      // 12. affordance projection over the CANONICAL unit.retry operation.
      step("affordance_projection.retry", () => {
        context.register({
          kind: "affordance_projection",
          id: "affordance.retry-provenance",
          operation: OPERATION_IDS.UNIT_RETRY,
          presentation_hint: "provenance-lens:retry-presentation",
          estimate: { tokens: 800, cost_usd_micro: 2000 },
        });
      });

      // 13. snapshot subscription — bound to the plugin lifetime.
      step("subscription", () => {
        context.onSnapshot((snapshot) => {
          snapshotsSeen += 1;
          context.state.set("snapshots_seen", snapshotsSeen);
          context.state.set("last_generation", snapshot.generation);
          probe.snapshotEvents.push(snapshot.generation);
        });
      });
    },
  };

  return { manifest: samplePluginManifest(), module, probe };
}

/** Convenience for tests that only need the contribution-union type to stay honest. */
export type { PluginContribution };
