/**
 * Typed plugin composition, trust, and reversible effects (plan T15,
 * REQ-PLUGIN-001..004, REQ-SEC-004, DSN-012, DESIGN.md "Plugin slots").
 *
 * Core invariants:
 *   - SLOTS OVER STATE MUTATION (REQ-PLUGIN-001, DSN-012): plugins contribute
 *     through the typed DSN-012 slot catalog (root.nav, case.view, trace.row,
 *     inspector.section, artifact.viewer, composer, bottom.panel, status.item,
 *     overlay) plus the non-surface provider kinds (relation provider,
 *     affordance projection). A plugin never receives another plugin's state,
 *     shell state, composer state, or the operation gateway: `PluginContext`
 *     is the whole typed seam, so cross-feature state mutation has no API.
 *   - VIEW EXTENSIONS CROSS THE TYPED SHELL SEAM (DSN-015): a `case.view`
 *     contribution is applied through shell.ts `registerShellView`, so the
 *     roadmap-surface guard guards plugin views exactly like host views. This
 *     module never edits the registry cell except through that guard.
 *   - TRUST IS EXPLICIT, NEVER INFERRED (REQ-PLUGIN-002, REQ-SEC-004): a
 *     manifest that does not declare `trusted_in_process` is refused for
 *     in-process loading with a typed policy refusal. Untrusted code is routed
 *     to a narrow out-of-process capability boundary whose requests and
 *     responses are SERIALIZED STRINGS validated before admission; no live
 *     callback of an untrusted module is ever admitted into the host.
 *   - LIFETIME OWNERSHIP (REQ-PLUGIN-003): every registration, subscription,
 *     and effect binds to the mounting plugin's lifetime through a
 *     deterministic disposer. A mount that throws halfway unwinds every
 *     partial effect in reverse order — a mount is atomic: fully mounted or
 *     fully unwound, never partially registered.
 *   - REVERSIBILITY IS MEASURED (REQ-PLUGIN-004): the manager fingerprints its
 *     whole plugin-visible surface (mounted ids, contributions, subscriptions,
 *     plugin state keys) deterministically, so a harness can assert
 *     mount → exercise → unload lands on a byte-identical baseline.
 *   - NO PLUGIN OPERATION AUTHORITY (REQ-AFF-001, REQ-SEC-002 direction): an
 *     affordance projection carries a canonical operation ID from the T09
 *     registry; any other operation is refused at registration. The manager
 *     never holds a gateway and exposes no invocation path — plugins present,
 *     the host's gateway alone admits.
 */

import type {
  AffordanceDescriptor,
  AffordanceObservation,
  AffordanceSessionContext,
  PaymentEstimate,
} from "./affordance";
import { resolveAffordance } from "./affordance";
import type { FocusRef } from "./focus";
import type { InspectorSection } from "./inspector";
import type { OperationId } from "./operation-registry";
import { isOperationId } from "./operation-registry";
import type { ProjectionWorld } from "./relations";
import type { RenderedLine, RenderedRecord, ViewContent } from "./render-tree";
import type { ShellViewRegistry, ViewId } from "./shell";
import { ROADMAP_VIEW_IDS, registerShellView, VIEW_IDS } from "./shell";
import type { SemanticToken } from "./theme";
import type { ViewContentRenderer } from "./views-chat-trace";
import type { FullWorkbenchSnapshot, TraceRecord } from "./workbench-model";

// --- Trust vocabulary (REQ-PLUGIN-002, REQ-SEC-004) -----------------------------

/** Explicit trust placement of a plugin. Never inferred from behavior. */
export type PluginTrustClass = "trusted_in_process" | "untrusted_out_of_process";

export const PLUGIN_TRUST_CLASSES: readonly PluginTrustClass[] = Object.freeze([
  "trusted_in_process",
  "untrusted_out_of_process",
]);

/** A plugin manifest. Admitted only after `parsePluginManifest` validation. */
export interface PluginManifest {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  /** Explicit trust placement (REQ-PLUGIN-002). Required, never defaulted. */
  readonly trust: PluginTrustClass;
  /**
   * The narrow capability allowlist of the out-of-process boundary. REQUIRED
   * for `untrusted_out_of_process` manifests (validated); informational-only
   * for trusted in-process ones.
   */
  readonly capabilities?: readonly string[] | undefined;
}

// --- Slot catalog (DSN-012) ------------------------------------------------------

/** The DESIGN.md preferred plugin slots, frozen. */
export const PLUGIN_SLOTS = Object.freeze([
  "root.nav",
  "case.view",
  "trace.row",
  "inspector.section",
  "artifact.viewer",
  "composer",
  "bottom.panel",
  "status.item",
  "overlay",
] as const);

export type PluginSlot = (typeof PLUGIN_SLOTS)[number];

/** The DESIGN.md contribution kinds every slot/provider is typed against. */
export const PLUGIN_CONTRIBUTION_KINDS = Object.freeze([
  "view",
  "focus_renderer",
  "relation_provider",
  "cue_provider",
  "inspector_section",
  "artifact_viewer",
  "affordance_projection",
] as const);

export type PluginContributionKind = (typeof PLUGIN_CONTRIBUTION_KINDS)[number];

const PLUGIN_SLOT_SET: ReadonlySet<string> = new Set<string>(PLUGIN_SLOTS);
const VIEW_ID_SET: ReadonlySet<string> = new Set<string>(VIEW_IDS);

// --- Contribution shapes (one shape per slot; provider kinds are slot-free) ------

/** Rendered output of a plugin panel/overlay/view-extension contribution. */
export interface PluginPanelContent {
  readonly lines: readonly RenderedLine[];
  readonly records: readonly RenderedRecord[];
}

/** Slot `root.nav` — a navigator entry pointing at an existing shell View. */
export interface PluginNavEntryContribution {
  readonly kind: "view";
  readonly slot: "root.nav";
  readonly id: string;
  readonly label: string;
  /** An existing shell ViewId; roadmap ids are refused (DSN-015). */
  readonly view: ViewId;
  readonly hint?: string | undefined;
  /** Handler the host may fire when the entry is activated. */
  readonly activate?: (() => void) | undefined;
}

/**
 * Slot `case.view` — a View contribution applied THROUGH shell.ts
 * `registerShellView` as an extension appended to an existing ViewId renderer.
 */
export interface PluginViewExtensionContribution {
  readonly kind: "view";
  readonly slot: "case.view";
  readonly id: string;
  readonly view: ViewId;
  readonly render: (snapshot: FullWorkbenchSnapshot) => PluginPanelContent;
}

/** Slot `trace.row` — a Focus renderer for one durable trace record. */
export interface PluginTraceRowContribution {
  readonly kind: "focus_renderer";
  readonly slot: "trace.row";
  readonly id: string;
  readonly render: (record: TraceRecord, snapshot: FullWorkbenchSnapshot) => RenderedLine | null;
}

/** Slot `inspector.section` — an inspector section for the selected Focus. */
export interface PluginInspectorSectionContribution {
  readonly kind: "inspector_section";
  readonly slot: "inspector.section";
  readonly id: string;
  readonly render: (args: {
    snapshot: FullWorkbenchSnapshot;
    focus: FocusRef;
  }) => InspectorSection | null;
}

/** One artifact record a plugin artifact viewer renders (presentation only). */
export interface PluginArtifactRecord {
  readonly id: string;
  readonly kind: string;
  readonly summary: string;
}

/** Slot `artifact.viewer` — a viewer for one artifact kind. */
export interface PluginArtifactViewerContribution {
  readonly kind: "artifact_viewer";
  readonly slot: "artifact.viewer";
  readonly id: string;
  readonly artifact_kind: string;
  readonly view: (artifact: PluginArtifactRecord) => PluginPanelContent;
}

/** One presentation cue. Cues are text+token; they never carry behavior. */
export interface PluginCue {
  readonly text: string;
  readonly token?: SemanticToken | undefined;
}

/**
 * Slot `composer` — a PRESENTATION-ONLY cue the host may render in the
 * composer strip. This slot can never mutate composer state (mode, draft,
 * takeovers are host-owned; no API reaches them from here).
 */
export interface PluginComposerCueContribution {
  readonly kind: "cue_provider";
  readonly slot: "composer";
  readonly id: string;
  readonly cue: (snapshot: FullWorkbenchSnapshot) => PluginCue | null;
}

/** Slot `status.item` — a status-bar cue provider. */
export interface PluginStatusCueContribution {
  readonly kind: "cue_provider";
  readonly slot: "status.item";
  readonly id: string;
  readonly cue: (snapshot: FullWorkbenchSnapshot) => PluginCue | null;
}

/** Slot `bottom.panel` — a lower-surface panel provider. */
export interface PluginPanelContribution {
  readonly kind: "view";
  readonly slot: "bottom.panel";
  readonly id: string;
  readonly render: (snapshot: FullWorkbenchSnapshot) => PluginPanelContent;
}

/** Slot `overlay` — an overlay content provider. */
export interface PluginOverlayContribution {
  readonly kind: "view";
  readonly slot: "overlay";
  readonly id: string;
  readonly render: (snapshot: FullWorkbenchSnapshot) => PluginPanelContent;
}

/**
 * Provider kind `relation_provider` (no surface slot). The derived edges are
 * presentation relations over the current projection world; `kind` MUST be
 * namespaced `plugin.<plugin-id>.<edge-kind>` so a plugin relation can never
 * impersonate a canonical source-typed relation kind.
 */
export interface PluginRelationEdgeInit {
  readonly source_ref: FocusRef;
  readonly target_ref: FocusRef;
  readonly kind: string;
}

export interface PluginRelationEdge extends PluginRelationEdgeInit {
  readonly provider: string;
}

export interface PluginRelationProviderContribution {
  readonly kind: "relation_provider";
  readonly id: string;
  readonly relations: (world: ProjectionWorld) => readonly PluginRelationEdgeInit[];
}

/**
 * Provider kind `affordance_projection`. Carries a CANONICAL operation ID from
 * the T09 registry — any other operation value is refused at registration, so
 * a plugin-defined operation cannot exist, and with it no gateway bypass. The
 * projection is presentation; invocation is always host-driven through the
 * canonical gateway.
 */
export interface PluginAffordanceProjectionContribution {
  readonly kind: "affordance_projection";
  readonly id: string;
  readonly operation: OperationId;
  readonly presentation_hint: string;
  /** Presentation-only estimate the host may feed the resolver's affordability policy. */
  readonly estimate?: PaymentEstimate | undefined;
}

/** Every contribution is one typed slot/provider shape. */
export type PluginContribution =
  | PluginNavEntryContribution
  | PluginViewExtensionContribution
  | PluginTraceRowContribution
  | PluginInspectorSectionContribution
  | PluginArtifactViewerContribution
  | PluginComposerCueContribution
  | PluginStatusCueContribution
  | PluginPanelContribution
  | PluginOverlayContribution
  | PluginRelationProviderContribution
  | PluginAffordanceProjectionContribution;

function hasSlot(
  contribution: PluginContribution,
): contribution is Extract<PluginContribution, { readonly slot: PluginSlot }> {
  return "slot" in contribution;
}

function slotOfContribution(contribution: PluginContribution): PluginSlot | "provider" {
  return hasSlot(contribution) ? contribution.slot : "provider";
}

// --- Typed errors (failure locus preserved) ---------------------------------------

/** Base class for every typed plugin failure. */
export class PluginError extends Error {
  readonly code: string;
  readonly plugin: string | null;

  constructor(code: string, message: string, plugin: string | null = null) {
    super(message);
    this.name = "PluginError";
    this.code = code;
    this.plugin = plugin;
  }
}

/** Manifest formation/trust-declaration failures (untrusted input refused). */
export class PluginManifestError extends PluginError {
  constructor(
    code:
      | "malformed_manifest"
      | "trust_not_declared"
      | "invalid_trust_class"
      | "invalid_capability_list",
    message: string,
  ) {
    super(code, message);
    this.name = "PluginManifestError";
  }
}

/** Trust-policy refusals (REQ-PLUGIN-002, REQ-SEC-004): typed, never silent. */
export class PluginTrustRefusalError extends PluginError {
  constructor(
    code: "untrusted_in_process_refused" | "trust_boundary_mismatch",
    message: string,
    plugin: string,
  ) {
    super(code, message, plugin);
    this.name = "PluginTrustRefusalError";
  }
}

/** Lifecycle violations (double mount, unknown/unmounted plugin, closed activation). */
export class PluginLifecycleError extends PluginError {
  constructor(
    code:
      | "plugin_already_mounted"
      | "plugin_not_mounted"
      | "activation_closed"
      | "unknown_contribution"
      | "not_out_of_process"
      | "snapshot_source_not_bound",
    message: string,
    plugin: string | null = null,
  ) {
    super(code, message, plugin);
    this.name = "PluginLifecycleError";
  }
}

/** Contribution formation/slot-check failures. */
export class PluginContributionError extends PluginError {
  constructor(
    code:
      | "malformed_contribution"
      | "contribution_id_collision"
      | "shell_registry_not_bound"
      | "roadmap_surface_refused"
      | "plugin_operation_not_canonical"
      | "relation_kind_not_namespaced",
    message: string,
    plugin: string,
  ) {
    super(code, message, plugin);
    this.name = "PluginContributionError";
  }
}

/** A mount that threw halfway; the partial effects were unwound before this. */
export class PluginActivationError extends PluginError {
  override readonly cause: unknown;

  constructor(plugin: string, cause: unknown) {
    super(
      "activation_failed",
      `plugin '${plugin}' failed during activation and was fully unwound: ${describeError(cause)}`,
      plugin,
    );
    this.name = "PluginActivationError";
    this.cause = cause;
  }
}

/** One unwind step that could not fully dispose its host effect. */
export interface PluginUnwindFailure {
  readonly label: string;
  readonly error: unknown;
}

/**
 * An unload whose unwind could not fully dispose every host effect (e.g. a
 * host unsubscribe that throws before removing its listener). The plugin IS
 * unmounted and the manager's ledger cleaned; this error reports, never
 * hides, the host effects that may persist (Correction Round 1 MINOR-1:
 * success-under-partial-failure is the anti-pattern).
 */
export class PluginUnwindError extends PluginError {
  readonly failures: readonly PluginUnwindFailure[];

  constructor(message: string, failures: readonly PluginUnwindFailure[]) {
    super("unwind_incomplete", message);
    this.name = "PluginUnwindError";
    this.failures = Object.freeze(failures.map((failure) => Object.freeze({ ...failure })));
  }
}

/**
 * A typed, inspectable record of a host handle the manager could not release:
 * the snapshot dispatcher whose `unsubscribe` threw. The dispatcher is
 * manager-owned and INERT once its fanout is empty (it delivers to zero
 * plugin listeners), and its stable identity makes a later re-attach safe
 * (the host Set deduplicates it). Recorded — never swallowed into an
 * untyped success (Correction Round 1 MINOR-1).
 */
export interface PluginDetachResidual {
  readonly kind: "snapshot_dispatcher_detach_failed";
  readonly error: unknown;
}

// --- Out-of-process capability boundary (REQ-SEC-004) -----------------------------

/** A serialized request crossing the out-of-process boundary. Strings only. */
export interface OutOfProcessCapabilityRequest {
  readonly plugin: string;
  readonly capability: string;
  /** JSON-serialized payload. Live objects/functions never cross. */
  readonly payload: string;
}

/** A serialized boundary response, validated before admission. */
export interface OutOfProcessCapabilityResponse {
  readonly ok: boolean;
  /** JSON-serialized response payload. */
  readonly payload: string;
}

/**
 * The transport an out-of-process adapter implements (a real child-process
 * IPC channel in production; a typed stand-in in tests). Narrow surface:
 * one request in, one serialized response out.
 */
export type OutOfProcessTransport = (
  request: OutOfProcessCapabilityRequest,
) => Promise<OutOfProcessCapabilityResponse>;

/** Boundary failures: undeclared capability, transport loss, malformed response. */
export class PluginBoundaryError extends PluginError {
  constructor(
    code:
      | "capability_not_declared"
      | "boundary_transport_failed"
      | "malformed_boundary_payload"
      | "malformed_boundary_response",
    message: string,
    plugin: string,
  ) {
    super(code, message, plugin);
    this.name = "PluginBoundaryError";
  }
}

export interface PluginCapabilityResult {
  readonly ok: boolean;
  /** The parsed (validated) response payload. */
  readonly value: unknown;
}

// --- Manifest validation (parse, validate, then admit) ----------------------------

const PLUGIN_ID_PATTERN = /^[a-z][a-z0-9._-]*$/;
const CAPABILITY_PATTERN = /^[a-z][a-z0-9_.-]*$/;

function manifestTrustOf(record: Record<string, unknown>): PluginTrustClass {
  // OWN-PROPERTY READ ONLY (Correction Round 1 MINOR-2): a prototype chain can
  // never speak for a manifest — inherited trust is not declared trust. The
  // trust declaration is checked FIRST: it is the security gate.
  if (!Object.hasOwn(record, "trust")) {
    throw new PluginManifestError(
      "trust_not_declared",
      "manifest declares no own 'trust' property; in-process trust is never inferred (REQ-PLUGIN-002, REQ-SEC-004)",
    );
  }
  const trust = record["trust"];
  if (typeof trust !== "string" || !(PLUGIN_TRUST_CLASSES as readonly string[]).includes(trust)) {
    throw new PluginManifestError(
      "invalid_trust_class",
      `manifest trust '${String(trust)}' is not a declared trust class (${PLUGIN_TRUST_CLASSES.join(" | ")})`,
    );
  }
  return trust as PluginTrustClass;
}

function manifestStringOf(
  record: Record<string, unknown>,
  field: string,
  pattern?: RegExp,
): string {
  if (!Object.hasOwn(record, field)) {
    throw new PluginManifestError(
      "malformed_manifest",
      `manifest declares no own '${field}' property`,
    );
  }
  const value = record[field];
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    (pattern !== undefined && !pattern.test(value))
  ) {
    throw new PluginManifestError(
      "malformed_manifest",
      `manifest field '${field}' is not a valid ${pattern === undefined ? "non-empty string" : "identifier"}`,
    );
  }
  return value;
}

function manifestCapabilitiesOf(
  record: Record<string, unknown>,
  trust: PluginTrustClass,
): readonly string[] | undefined {
  if (!Object.hasOwn(record, "capabilities")) {
    if (trust === "untrusted_out_of_process") {
      throw new PluginManifestError(
        "invalid_capability_list",
        "an untrusted_out_of_process manifest MUST declare its narrow capability allowlist",
      );
    }
    return undefined;
  }
  const rawCapabilities = record["capabilities"];
  if (!Array.isArray(rawCapabilities) || rawCapabilities.length === 0) {
    throw new PluginManifestError(
      "invalid_capability_list",
      "manifest capabilities must be a non-empty array of capability names",
    );
  }
  for (const capability of rawCapabilities) {
    if (typeof capability !== "string" || !CAPABILITY_PATTERN.test(capability)) {
      throw new PluginManifestError(
        "invalid_capability_list",
        `capability '${String(capability)}' is not a valid capability name`,
      );
    }
  }
  return Object.freeze([...rawCapabilities]);
}

/**
 * Validate an untrusted manifest document (model output is untrusted external
 * input): parse, validate against the declared shape, then admit. Every field
 * is read from OWN properties only — prototype-inherited values never speak
 * for a manifest (Correction Round 1 MINOR-2), and a non-object manifest is a
 * typed formation refusal, never a raw TypeError (Correction Round 1
 * MINOR-3). Throws the typed PluginManifestError on any formation or
 * trust-declaration defect.
 */
export function parsePluginManifest(raw: unknown): PluginManifest {
  if (typeof raw !== "object" || raw === null) {
    throw new PluginManifestError(
      "malformed_manifest",
      `plugin manifest must be an object, got ${raw === null ? "null" : typeof raw}`,
    );
  }
  const record = raw as Record<string, unknown>;
  // The trust declaration gates FIRST (it is the security decision); identity
  // and capability formation follow.
  const trust = manifestTrustOf(record);
  const id = manifestStringOf(record, "id", PLUGIN_ID_PATTERN);
  const name = manifestStringOf(record, "name");
  const version = manifestStringOf(record, "version");
  const capabilities = manifestCapabilitiesOf(record, trust);
  return Object.freeze({
    id,
    name,
    version,
    trust,
    ...(capabilities !== undefined ? { capabilities } : {}),
  });
}

/** The trust policy decision for one manifest (pure; mount consults the same policy). */
export type TrustDecision =
  | {
      readonly verdict: "load_in_process";
      readonly plugin: string;
      readonly trust: "trusted_in_process";
    }
  | {
      readonly verdict: "route_out_of_process";
      readonly plugin: string;
      readonly trust: "untrusted_out_of_process";
      readonly capabilities: readonly string[];
    };

/** Classify one validated manifest. Untrusted is ALWAYS routed out of process. */
export function decideTrustPlacement(manifest: PluginManifest): TrustDecision {
  if (manifest.trust === "trusted_in_process") {
    return Object.freeze({
      verdict: "load_in_process",
      plugin: manifest.id,
      trust: manifest.trust,
    });
  }
  return Object.freeze({
    verdict: "route_out_of_process",
    plugin: manifest.id,
    trust: manifest.trust,
    capabilities: Object.freeze([...(manifest.capabilities ?? [])]),
  });
}

// --- Plugin-facing seam (PluginContext: the whole surface a plugin can touch) -----

/** Structural slice of the T07/T08 WorkbenchModel a plugin may subscribe to. */
export interface SnapshotSource {
  subscribe(listener: (snapshot: FullWorkbenchSnapshot) => void): () => void;
}

/** Host-owned cell over the shell view registry (written only via registerShellView). */
export interface ShellViewRegistryCell {
  get(): ShellViewRegistry;
  set(registry: ShellViewRegistry): void;
}

/** A cell the host provides so plugin view extensions reach the typed shell seam. */
export function createShellViewRegistryCell(initial: ShellViewRegistry): ShellViewRegistryCell {
  let current: ShellViewRegistry = initial;
  return {
    get: () => current,
    set: (registry: ShellViewRegistry) => {
      current = registry;
    },
  };
}

/** A deterministic disposer bound to the plugin's lifetime. Dispose is idempotent. */
export interface PluginDisposable {
  readonly id: string;
  readonly dispose: () => void;
}

/** Per-plugin private read-model state. Invisible to every other plugin. */
export interface PluginStateCell {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  delete(key: string): boolean;
  keys(): readonly string[];
}

/**
 * The ONLY object a trusted in-process plugin receives. It exposes
 * registration, subscription binding, and private state — never the model,
 * the shell, the composer, the gateway, or another plugin.
 */
export interface PluginContext {
  readonly pluginId: string;
  readonly trust: "trusted_in_process";
  register(contribution: PluginContribution): PluginDisposable;
  /** Subscribe to host snapshots; the subscription binds to the plugin lifetime. */
  onSnapshot(listener: (snapshot: FullWorkbenchSnapshot) => void): PluginDisposable;
  readonly state: PluginStateCell;
}

/** A trusted in-process plugin module. Activation registers contributions. */
export interface TrustedPluginModule {
  readonly activate: (context: PluginContext) => void;
}

// --- Manager record types -----------------------------------------------------------

interface TrackedDisposer {
  readonly label: string;
  readonly dispose: () => void;
}

interface PluginRecord {
  readonly manifest: PluginManifest;
  readonly mode: "in_process" | "out_of_process";
  readonly state: Map<string, unknown>;
  readonly disposers: TrackedDisposer[];
  activating: boolean;
  readonly transport: OutOfProcessTransport | undefined;
}

interface ContributionRecord {
  readonly pluginId: string;
  readonly contribution: PluginContribution;
  readonly disposer: TrackedDisposer;
}

interface SubscriptionRecord {
  readonly id: string;
  readonly pluginId: string;
  readonly dispose: () => void;
}

/** Host-facing view of one mounted plugin (data only, frozen). */
export interface PluginRecordView {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  readonly trust: PluginTrustClass;
  readonly mode: "in_process" | "out_of_process";
  readonly capabilities: readonly string[];
  readonly contributionCount: number;
  readonly subscriptionCount: number;
}

// --- Read-model views over contributions ---------------------------------------------

export interface PluginNavEntryView {
  readonly plugin: string;
  readonly id: string;
  readonly label: string;
  readonly view: ViewId;
  readonly hint?: string | undefined;
}

export interface PluginCueView {
  readonly plugin: string;
  readonly id: string;
  readonly cue: PluginCue;
}

export interface PluginPanelView {
  readonly plugin: string;
  readonly id: string;
  readonly content: PluginPanelContent;
}

export interface PluginTraceRowView {
  readonly plugin: string;
  readonly id: string;
  readonly line: RenderedLine;
}

export interface PluginInspectorSectionView {
  readonly plugin: string;
  readonly id: string;
  readonly section: InspectorSection;
}

export interface PluginArtifactView {
  readonly plugin: string;
  readonly id: string;
  readonly content: PluginPanelContent;
}

export interface PluginViewExtensionView {
  readonly plugin: string;
  readonly id: string;
  readonly view: ViewId;
}

export interface PluginAffordanceProjectionView {
  readonly plugin: string;
  readonly id: string;
  readonly operation: OperationId;
  readonly presentation_hint: string;
  readonly estimate?: PaymentEstimate | undefined;
}

// --- Deterministic surface fingerprint (REQ-PLUGIN-004) ------------------------------

export interface PluginSurfaceFingerprint {
  readonly mounted: readonly string[];
  readonly contributions: readonly {
    readonly id: string;
    readonly plugin: string;
    readonly slot: PluginSlot | "provider";
    readonly kind: PluginContributionKind;
  }[];
  readonly subscriptions: readonly {
    readonly id: string;
    readonly plugin: string;
  }[];
  readonly stateKeys: readonly {
    readonly plugin: string;
    readonly key: string;
  }[];
}

/**
 * Byte-stable serialization of a fingerprint: records are sorted and object
 * key order is fixed by construction, so string equality is a byte comparison.
 */
export function serializePluginSurfaceFingerprint(fingerprint: PluginSurfaceFingerprint): string {
  return JSON.stringify({
    mounted: [...fingerprint.mounted].sort(),
    contributions: [...fingerprint.contributions].sort((a, b) =>
      a.plugin === b.plugin ? (a.id < b.id ? -1 : 1) : a.plugin < b.plugin ? -1 : 1,
    ),
    subscriptions: [...fingerprint.subscriptions].sort((a, b) => (a.id < b.id ? -1 : 1)),
    stateKeys: [...fingerprint.stateKeys].sort((a, b) =>
      a.plugin === b.plugin ? (a.key < b.key ? -1 : 1) : a.plugin < b.plugin ? -1 : 1,
    ),
  });
}

// --- Validation helpers ---------------------------------------------------------------

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function requireString(value: unknown, what: string, plugin: string): void {
  if (typeof value !== "string" || value.length === 0) {
    throw new PluginContributionError(
      "malformed_contribution",
      `${what} must be a non-empty string`,
      plugin,
    );
  }
}

function requireFunction(value: unknown, what: string, plugin: string): void {
  if (typeof value !== "function") {
    throw new PluginContributionError(
      "malformed_contribution",
      `${what} must be a function`,
      plugin,
    );
  }
}

/**
 * Slot-checked contribution validation (REQ-PLUGIN-001). Typed by the union;
 * re-checked at runtime because contributions can be built dynamically from
 * untrusted documents. Roadmap ids are refused at this seam too (DSN-015).
 */
function validateContribution(pluginId: string, contribution: PluginContribution): void {
  if (typeof contribution !== "object" || contribution === null) {
    throw new PluginContributionError(
      "malformed_contribution",
      "contribution must be an object",
      pluginId,
    );
  }
  requireString(contribution.id, "contribution id", pluginId);
  requireString(contribution.kind, "contribution kind", pluginId);

  if (hasSlot(contribution)) {
    if (!(PLUGIN_SLOT_SET as ReadonlySet<string>).has(contribution.slot)) {
      throw new PluginContributionError(
        "malformed_contribution",
        `unknown plugin slot '${String(contribution.slot)}'`,
        pluginId,
      );
    }
    switch (contribution.slot) {
      case "root.nav": {
        const kind: string = contribution.kind;
        if (kind !== "view") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot 'root.nav' accepts kind 'view', got '${kind}'`,
            pluginId,
          );
        }
        requireString(contribution.label, "nav entry label", pluginId);
        checkViewTarget(pluginId, contribution.view);
        if (contribution.activate !== undefined) {
          requireFunction(contribution.activate, "nav entry activate", pluginId);
        }
        return;
      }
      case "case.view": {
        const kind: string = contribution.kind;
        if (kind !== "view") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot 'case.view' accepts kind 'view', got '${kind}'`,
            pluginId,
          );
        }
        checkViewTarget(pluginId, contribution.view);
        requireFunction(contribution.render, "view extension render", pluginId);
        return;
      }
      case "bottom.panel":
      case "overlay": {
        const kind: string = contribution.kind;
        if (kind !== "view") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot '${contribution.slot}' accepts kind 'view', got '${kind}'`,
            pluginId,
          );
        }
        requireFunction(contribution.render, "panel render", pluginId);
        return;
      }
      case "trace.row": {
        const kind: string = contribution.kind;
        if (kind !== "focus_renderer") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot 'trace.row' accepts kind 'focus_renderer', got '${kind}'`,
            pluginId,
          );
        }
        requireFunction(contribution.render, "trace row render", pluginId);
        return;
      }
      case "inspector.section": {
        const kind: string = contribution.kind;
        if (kind !== "inspector_section") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot 'inspector.section' accepts kind 'inspector_section', got '${kind}'`,
            pluginId,
          );
        }
        requireFunction(contribution.render, "inspector section render", pluginId);
        return;
      }
      case "artifact.viewer": {
        const kind: string = contribution.kind;
        if (kind !== "artifact_viewer") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot 'artifact.viewer' accepts kind 'artifact_viewer', got '${kind}'`,
            pluginId,
          );
        }
        requireString(contribution.artifact_kind, "artifact kind", pluginId);
        requireFunction(contribution.view, "artifact viewer", pluginId);
        return;
      }
      case "composer":
      case "status.item": {
        const kind: string = contribution.kind;
        if (kind !== "cue_provider") {
          throw new PluginContributionError(
            "malformed_contribution",
            `slot '${contribution.slot}' accepts kind 'cue_provider', got '${kind}'`,
            pluginId,
          );
        }
        requireFunction(contribution.cue, "cue provider", pluginId);
        return;
      }
    }
  }

  const providerKind: string = contribution.kind;
  switch (contribution.kind) {
    case "relation_provider": {
      requireFunction(contribution.relations, "relation provider", pluginId);
      return;
    }
    case "affordance_projection": {
      // The gateway-bypass guard: a projection MUST reference a canonical
      // operation of the T09 registry; anything else is refused here so a
      // plugin-defined operation cannot exist at all (REQ-AFF-001).
      if (!isOperationId(contribution.operation)) {
        throw new PluginContributionError(
          "plugin_operation_not_canonical",
          `affordance projection operation '${String(contribution.operation)}' is not a canonical registry operation; plugin-defined operations do not exist`,
          pluginId,
        );
      }
      requireString(contribution.presentation_hint, "presentation hint", pluginId);
      return;
    }
    default:
      throw new PluginContributionError(
        "malformed_contribution",
        `provider contribution kind '${providerKind}' is not a known provider kind`,
        pluginId,
      );
  }
}

/** DSN-015 at the plugin seam: roadmap ids are refused before the shell sees them. */
function checkViewTarget(pluginId: string, view: ViewId): void {
  if ((ROADMAP_VIEW_IDS as readonly string[]).includes(view)) {
    throw new PluginContributionError(
      "roadmap_surface_refused",
      `plugin view target '${view}' is roadmap-only and must not be built (DSN-015)`,
      pluginId,
    );
  }
  if (!(VIEW_ID_SET as ReadonlySet<string>).has(view)) {
    throw new PluginContributionError(
      "malformed_contribution",
      `unknown view target '${String(view)}'`,
      pluginId,
    );
  }
}

function registryWithoutView(registry: ShellViewRegistry, view: ViewId): ShellViewRegistry {
  const next: Partial<Record<ViewId, ViewContentRenderer>> = { ...registry };
  delete next[view];
  return Object.freeze(next);
}

function wrapViewExtension(
  base: ViewContentRenderer | undefined,
  extension: PluginViewExtensionContribution,
): ViewContentRenderer {
  return (args) => {
    const baseContent: ViewContent =
      base !== undefined
        ? base(args)
        : {
            view: extension.view,
            question: "plugin extension (no base renderer)",
            lines: [],
            records: [],
          };
    const addon = extension.render(args.snapshot);
    return {
      view: baseContent.view,
      question: baseContent.question,
      lines: [...baseContent.lines, ...addon.lines],
      records: [...baseContent.records, ...addon.records],
    };
  };
}

function isBoundaryResponse(value: unknown): value is OutOfProcessCapabilityResponse {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const record = value as Record<string, unknown>;
  return typeof record["ok"] === "boolean" && typeof record["payload"] === "string";
}

// --- The plugin composition manager ---------------------------------------------------

export interface PluginCompositionManagerConfig {
  /** The host snapshot source plugins may subscribe to (never the raw model otherwise). */
  readonly snapshots?: SnapshotSource | undefined;
  /** The host shell registry cell plugin view extensions are applied through. */
  readonly shellViews?: ShellViewRegistryCell | undefined;
}

export class PluginCompositionManager {
  private readonly config: PluginCompositionManagerConfig;
  private readonly plugins = new Map<string, PluginRecord>();
  private readonly contributions = new Map<string, ContributionRecord>();
  private readonly subscriptions = new Map<string, SubscriptionRecord>();
  private subscriptionCounter = 0;
  /**
   * Per-view stacking of LIVE case.view contributions (contribution ids in
   * original stacking order). The registry entry for a view is always
   * RECOMPOSED from the anchored base renderer plus this stack, so disposing
   * any single extension — in any order — rebuilds exactly the composition of
   * the remaining extensions (Correction Round 1 BLOCKING-1).
   */
  private readonly viewExtensionStacks = new Map<ViewId, string[]>();
  /**
   * The registry entry each view's extension stack is anchored onto: captured
   * from the host cell when the FIRST extension for that view registers, and
   * restored (or the key removed) when the stack empties.
   */
  private readonly viewBaseRenderers = new Map<ViewId, ViewContentRenderer | undefined>();
  /**
   * Live plugin snapshot listeners, keyed by subscription id (Correction
   * Round 1 MINOR-1). Plugin subscriptions NEVER depend on the host's
   * subscribe closure for their disposal: they are fanout entries removed by
   * a deterministic Map delete, so a plugin's unwind cannot be defeated by a
   * throwing host unsubscribe. Exactly ONE host subscription (the dispatcher)
   * exists per manager while any plugin subscription is live.
   */
  private readonly fanout = new Map<
    string,
    { readonly pluginId: string; readonly listener: (snapshot: FullWorkbenchSnapshot) => void }
  >();
  /** The stable manager-owned dispatcher (identity never changes: a host Set that retained it deduplicates on re-attach). */
  private dispatcher: ((snapshot: FullWorkbenchSnapshot) => void) | null = null;
  private dispatcherUnsubscribe: (() => void) | null = null;

  /** The reverse-order unwinding trace of the last mount/unload (test instrument). */
  unwindTrace: readonly string[] = Object.freeze([]);
  /** The unwind steps of the last mount/unload that could not fully dispose (instrument). */
  unwindFailures: readonly PluginUnwindFailure[] = Object.freeze([]);
  /**
   * Typed residuals: host unsubscribe calls that THREW while detaching the
   * snapshot dispatcher. The dispatcher is inert (its fanout was empty at the
   * detach attempt) and its stable identity keeps the next attach safe; the
   * residual records that a broken host may still hold a reference. Never
   * swallowed into an untyped success (Correction Round 1 MINOR-1).
   */
  detachResiduals: readonly PluginDetachResidual[] = Object.freeze([]);

  constructor(config: PluginCompositionManagerConfig = {}) {
    this.config = config;
  }

  // --- lifecycle -----------------------------------------------------------------

  /**
   * Mount one trusted in-process plugin. The trust policy runs FIRST: a
   * manifest that does not explicitly declare `trusted_in_process` is refused
   * with a typed policy refusal and the module is never activated. Activation
   * is atomic: a module that throws halfway has every partial registration
   * unwound in reverse order before the typed PluginActivationError escapes.
   */
  mount(manifest: PluginManifest, module: TrustedPluginModule): PluginRecordView {
    const validated = validateManifestForMount(manifest);
    if (validated.trust !== "trusted_in_process") {
      throw new PluginTrustRefusalError(
        "untrusted_in_process_refused",
        `plugin '${validated.id}' declares trust '${validated.trust}'; in-process loading requires an explicit trusted_in_process declaration (REQ-PLUGIN-002, REQ-SEC-004) — use mountOutOfProcess`,
        validated.id,
      );
    }
    if (this.plugins.has(validated.id)) {
      throw new PluginLifecycleError(
        "plugin_already_mounted",
        `plugin '${validated.id}' is already mounted`,
        validated.id,
      );
    }
    const record: PluginRecord = {
      manifest: validated,
      mode: "in_process",
      state: new Map<string, unknown>(),
      disposers: [],
      activating: true,
      transport: undefined,
    };
    this.plugins.set(validated.id, record);
    this.unwindTrace = Object.freeze([]);
    this.unwindFailures = Object.freeze([]);
    try {
      module.activate(this.createContext(validated.id));
    } catch (error) {
      this.unwind(validated.id);
      // Typed policy/formation refusals (trust, collision, roadmap,
      // non-canonical operation) preserve their own locus; unexpected plugin
      // crashes are wrapped with the full unwind guarantee.
      if (error instanceof PluginError) {
        throw error;
      }
      throw new PluginActivationError(validated.id, error);
    }
    record.activating = false;
    return this.requirePluginView(validated.id);
  }

  /**
   * Mount an UNTRUSTED plugin behind the out-of-process capability boundary.
   * No module is accepted — there is nothing to activate — so no live callback
   * of an untrusted module can ever enter the host; interactions cross the
   * transport as serialized requests only.
   */
  mountOutOfProcess(manifest: PluginManifest, transport: OutOfProcessTransport): PluginRecordView {
    const validated = validateManifestForMount(manifest);
    if (validated.trust !== "untrusted_out_of_process") {
      throw new PluginTrustRefusalError(
        "trust_boundary_mismatch",
        `plugin '${validated.id}' declares trust '${validated.trust}'; the out-of-process boundary is reserved for untrusted_out_of_process declarations (trust placement is explicit)`,
        validated.id,
      );
    }
    if (this.plugins.has(validated.id)) {
      throw new PluginLifecycleError(
        "plugin_already_mounted",
        `plugin '${validated.id}' is already mounted`,
        validated.id,
      );
    }
    const record: PluginRecord = {
      manifest: validated,
      mode: "out_of_process",
      state: new Map<string, unknown>(),
      disposers: [],
      activating: false,
      transport,
    };
    this.plugins.set(validated.id, record);
    this.unwindTrace = Object.freeze([]);
    this.unwindFailures = Object.freeze([]);
    return this.requirePluginView(validated.id);
  }

  /**
   * Unload one plugin: every disposer runs in reverse registration order.
   * Idempotent — unloading an unmounted plugin changes nothing and reports
   * `not_mounted` instead of throwing. When any disposer cannot fully dispose
   * its host effect, the plugin is STILL unmounted and the ledger cleaned, but
   * the typed `PluginUnwindError` is thrown instead of a silent success
   * (Correction Round 1 MINOR-1).
   */
  unload(pluginId: string): { readonly unloaded: boolean; readonly reason?: string } {
    if (!this.plugins.has(pluginId)) {
      return Object.freeze({ unloaded: false, reason: "not_mounted" });
    }
    const failures = this.unwind(pluginId);
    if (failures.length > 0) {
      throw new PluginUnwindError(
        `unload of '${pluginId}' could not fully dispose ${failures.length} effect(s) [${failures
          .map((failure) => failure.label)
          .join(
            ", ",
          )}]: the plugin is unmounted and the manager ledger cleaned, but the listed host effects may persist`,
        failures,
      );
    }
    return Object.freeze({ unloaded: true });
  }

  /**
   * Reverse-order deterministic unwinding of one plugin's whole effect set.
   * A failing disposer never blocks the unwind of the remaining effects; its
   * failure is preserved in the trace AND returned as a structured failure —
   * never swallowed into a success.
   */
  private unwind(pluginId: string): readonly PluginUnwindFailure[] {
    const record = this.plugins.get(pluginId);
    const failures: PluginUnwindFailure[] = [];
    if (record === undefined) {
      return failures;
    }
    const trace: string[] = [];
    for (let index = record.disposers.length - 1; index >= 0; index -= 1) {
      const disposer = record.disposers[index];
      if (disposer === undefined) {
        continue;
      }
      try {
        disposer.dispose();
        trace.push(`${disposer.label} disposed`);
      } catch (error) {
        failures.push(Object.freeze({ label: disposer.label, error }));
        trace.push(`${disposer.label} DISPOSER FAILED: ${describeError(error)}`);
      }
    }
    this.plugins.delete(pluginId);
    this.unwindTrace = Object.freeze(trace);
    this.unwindFailures = Object.freeze(failures);
    return failures;
  }

  isMounted(pluginId: string): boolean {
    return this.plugins.has(pluginId);
  }

  pluginIds(): readonly string[] {
    return Object.freeze([...this.plugins.keys()].sort());
  }

  plugin(pluginId: string): PluginRecordView | undefined {
    const record = this.plugins.get(pluginId);
    return record === undefined ? undefined : this.pluginView(record);
  }

  private requirePluginView(pluginId: string): PluginRecordView {
    const record = this.plugins.get(pluginId);
    if (record === undefined) {
      throw new PluginLifecycleError(
        "plugin_not_mounted",
        `plugin '${pluginId}' vanished during mount`,
        pluginId,
      );
    }
    return this.pluginView(record);
  }

  private pluginView(record: PluginRecord): PluginRecordView {
    const pluginId = record.manifest.id;
    let contributionCount = 0;
    for (const registered of this.contributions.values()) {
      if (registered.pluginId === pluginId) {
        contributionCount += 1;
      }
    }
    let subscriptionCount = 0;
    for (const subscription of this.subscriptions.values()) {
      if (subscription.pluginId === pluginId) {
        subscriptionCount += 1;
      }
    }
    return Object.freeze({
      id: record.manifest.id,
      name: record.manifest.name,
      version: record.manifest.version,
      trust: record.manifest.trust,
      mode: record.mode,
      capabilities: Object.freeze([...(record.manifest.capabilities ?? [])]),
      contributionCount,
      subscriptionCount,
    });
  }

  // --- the plugin-facing seam ------------------------------------------------------

  private createContext(pluginId: string): PluginContext {
    const state: PluginStateCell = {
      get: (key: string) => this.stateGet(pluginId, key),
      set: (key: string, value: unknown) => this.stateSet(pluginId, key, value),
      delete: (key: string) => this.stateDelete(pluginId, key),
      keys: () => this.stateKeys(pluginId),
    };
    return Object.freeze({
      pluginId,
      trust: "trusted_in_process" as const,
      register: (contribution: PluginContribution) =>
        this.registerDuringActivation(pluginId, contribution),
      onSnapshot: (listener: (snapshot: FullWorkbenchSnapshot) => void) =>
        this.subscribeDuringActivation(pluginId, listener),
      state,
    });
  }

  private requireActivatingRecord(pluginId: string, operation: string): PluginRecord {
    const record = this.plugins.get(pluginId);
    if (record === undefined) {
      throw new PluginLifecycleError(
        "plugin_not_mounted",
        `plugin '${pluginId}' cannot ${operation}: it is not mounted`,
        pluginId,
      );
    }
    if (record.mode !== "in_process") {
      throw new PluginLifecycleError(
        "not_out_of_process",
        `plugin '${pluginId}' cannot ${operation}: untrusted modules never receive a plugin context`,
        pluginId,
      );
    }
    if (!record.activating) {
      throw new PluginLifecycleError(
        "activation_closed",
        `plugin '${pluginId}' cannot ${operation}: activation is closed after mount returned`,
        pluginId,
      );
    }
    return record;
  }

  private registerDuringActivation(
    pluginId: string,
    contribution: PluginContribution,
  ): PluginDisposable {
    const record = this.requireActivatingRecord(pluginId, "register a contribution");
    validateContribution(pluginId, contribution);

    const owner = this.contributions.get(contribution.id);
    if (owner !== undefined) {
      throw new PluginContributionError(
        "contribution_id_collision",
        `contribution id '${contribution.id}' is already owned by plugin '${owner.pluginId}'; contribution ids are unique across all mounted plugins`,
        pluginId,
      );
    }

    const isViewExtension = hasSlot(contribution) && contribution.slot === "case.view";
    if (isViewExtension && this.config.shellViews === undefined) {
      throw new PluginContributionError(
        "shell_registry_not_bound",
        "no shell view registry cell is bound; the host must provide one for case.view contributions",
        pluginId,
      );
    }

    const disposer: TrackedDisposer = {
      label: `contribution:${contribution.id}`,
      dispose: () => {
        if (!this.contributions.has(contribution.id)) {
          return; // idempotent: already disposed
        }
        if (
          hasSlot(contribution) &&
          contribution.slot === "case.view" &&
          this.config.shellViews !== undefined
        ) {
          const cell = this.config.shellViews;
          // Recompose, never overwrite: drop THIS extension from the view's
          // stack and rebuild the registry entry from the remaining stack over
          // the anchored base. Restoring a private previousRenderer here would
          // silently drop (or resurrect) other plugins' stacked extensions.
          const stack = this.viewExtensionStacks.get(contribution.view) ?? [];
          const index = stack.indexOf(contribution.id);
          if (index >= 0) {
            stack.splice(index, 1);
          }
          // Compose FIRST, while the anchored base is still recorded: an empty
          // stack composes to exactly the base renderer, which is what the
          // registry is restored to. Only then is the bookkeeping dropped.
          const entry = this.composeViewEntry(contribution.view);
          if (stack.length === 0) {
            this.viewExtensionStacks.delete(contribution.view);
            this.viewBaseRenderers.delete(contribution.view);
          }
          const current = cell.get();
          const next =
            entry === undefined
              ? registryWithoutView(current, contribution.view)
              : registerShellView(current, contribution.view, entry);
          cell.set(next);
        }
        this.contributions.delete(contribution.id);
      },
    };
    // The record is admitted BEFORE the view composition runs: the composition
    // reads live contributions from the ledger, so a freshly registered
    // extension must already be visible to it (Correction Round 1 BLOCKING-1).
    this.contributions.set(contribution.id, { pluginId, contribution, disposer });
    record.disposers.push(disposer);

    if (isViewExtension) {
      const cell = this.config.shellViews;
      if (cell === undefined) {
        throw new PluginContributionError(
          "shell_registry_not_bound",
          "no shell view registry cell is bound; the host must provide one for case.view contributions",
          pluginId,
        );
      }
      // Cross the typed shell seam: the registerShellView guard (DSN-015)
      // applies to plugin view extensions exactly like host registrations.
      // The entry is a STACK, not a hand-restored previous pointer: the first
      // extension for a view anchors the cell's current entry as the base, and
      // the registry entry is recomposed from base + live extensions whenever
      // any extension registers or disposes (Correction Round 1 BLOCKING-1 —
      // correct under ANY disposal order).
      const stack = this.viewExtensionStacks.get(contribution.view) ?? [];
      if (stack.length === 0) {
        this.viewBaseRenderers.set(contribution.view, cell.get()[contribution.view]);
      }
      stack.push(contribution.id);
      this.viewExtensionStacks.set(contribution.view, stack);
      // With a non-empty stack the composed entry is always a wrapper.
      const composedNow = this.composeViewEntry(contribution.view);
      if (composedNow === undefined) {
        throw new PluginContributionError(
          "shell_registry_not_bound",
          "the recomposed registry entry vanished while the extension stack is non-empty",
          pluginId,
        );
      }
      cell.set(registerShellView(cell.get(), contribution.view, composedNow));
    }
    return Object.freeze({ id: contribution.id, dispose: disposer.dispose });
  }

  /**
   * Rebuild one view's registry entry from the anchored base renderer plus the
   * LIVE same-view extension stack, in original stacking order. Called on
   * every register/dispose of a case.view contribution, so the entry is always
   * exactly the composition of what is still registered — nothing dropped,
   * nothing resurrected (Correction Round 1 BLOCKING-1).
   */
  private composeViewEntry(view: ViewId): ViewContentRenderer | undefined {
    let entry = this.viewBaseRenderers.get(view);
    for (const id of this.viewExtensionStacks.get(view) ?? []) {
      const registered = this.contributions.get(id);
      if (
        registered === undefined ||
        !hasSlot(registered.contribution) ||
        registered.contribution.slot !== "case.view"
      ) {
        continue;
      }
      entry = wrapViewExtension(entry, registered.contribution);
    }
    return entry;
  }

  private subscribeDuringActivation(
    pluginId: string,
    listener: (snapshot: FullWorkbenchSnapshot) => void,
  ): PluginDisposable {
    const record = this.requireActivatingRecord(pluginId, "subscribe to snapshots");
    const source = this.config.snapshots;
    if (source === undefined) {
      throw new PluginLifecycleError(
        "snapshot_source_not_bound",
        "no snapshot source is bound; the host must provide one for plugin subscriptions",
        pluginId,
      );
    }
    this.subscriptionCounter += 1;
    const subscriptionId = `sub-${String(this.subscriptionCounter)}`;
    // FANOUT SUBSCRIPTION (Correction Round 1 MINOR-1): the plugin's listener
    // becomes a fanout entry; the manager owns the single host subscription.
    // Disposal of a plugin subscription is a deterministic Map delete — a
    // throwing host unsubscribe can never defeat a plugin unwind, keep a
    // plugin listener alive, or orphan a ledger record.
    this.fanout.set(subscriptionId, { pluginId, listener });
    const disposer: TrackedDisposer = {
      label: `subscription:${subscriptionId}`,
      dispose: () => {
        if (!this.fanout.has(subscriptionId)) {
          return; // idempotent: already disposed
        }
        // The plugin-facing effect is fully disposed HERE, infallibly. The
        // ledger delete happens unconditionally alongside it.
        this.fanout.delete(subscriptionId);
        this.subscriptions.delete(subscriptionId);
        this.detachDispatcherIfIdle();
      },
    };
    this.subscriptions.set(subscriptionId, {
      id: subscriptionId,
      pluginId,
      dispose: disposer.dispose,
    });
    record.disposers.push(disposer);
    this.attachDispatcherIfIdle();
    return Object.freeze({ id: subscriptionId, dispose: disposer.dispose });
  }

  /**
   * Attach the shared snapshot dispatcher while at least one plugin
   * subscription is live. The dispatcher identity is STABLE for the manager's
   * lifetime: if a broken host retained it after a failed detach, the
   * re-subscribe deduplicates in the host's Set (or re-adds it if the host did
   * release it) — either way exactly one effective copy delivers.
   */
  private attachDispatcherIfIdle(): void {
    const source = this.config.snapshots;
    if (source === undefined || this.dispatcherUnsubscribe !== null) {
      return;
    }
    if (this.dispatcher === null) {
      this.dispatcher = (snapshot: FullWorkbenchSnapshot) => {
        for (const entry of [...this.fanout.values()]) {
          entry.listener(snapshot);
        }
      };
    }
    this.dispatcherUnsubscribe = source.subscribe(this.dispatcher);
  }

  /**
   * Detach the shared dispatcher when no plugin subscription remains. A host
   * unsubscribe that throws is recorded as a typed residual: the dispatcher is
   * INERT (its fanout is empty — it delivers to zero plugin listeners) and the
   * handle is released for a clean re-attach later; the residual records that
   * a broken host may still hold the reference. Never swallowed into an
   * untyped success (Correction Round 1 MINOR-1).
   */
  private detachDispatcherIfIdle(): void {
    if (this.fanout.size > 0 || this.dispatcherUnsubscribe === null) {
      return;
    }
    const unsubscribe = this.dispatcherUnsubscribe;
    this.dispatcherUnsubscribe = null;
    try {
      unsubscribe();
    } catch (error) {
      this.detachResiduals = Object.freeze([
        ...this.detachResiduals,
        Object.freeze({ kind: "snapshot_dispatcher_detach_failed" as const, error }),
      ]);
    }
  }

  // --- per-plugin private state ------------------------------------------------------

  private requireStateRecord(pluginId: string): PluginRecord {
    const record = this.plugins.get(pluginId);
    if (record === undefined) {
      throw new PluginLifecycleError(
        "plugin_not_mounted",
        `plugin '${pluginId}' state is unreachable: the plugin is not mounted`,
        pluginId,
      );
    }
    return record;
  }

  private stateGet(pluginId: string, key: string): unknown {
    return this.requireStateRecord(pluginId).state.get(key);
  }

  private stateSet(pluginId: string, key: string, value: unknown): void {
    this.requireStateRecord(pluginId).state.set(key, value);
  }

  private stateDelete(pluginId: string, key: string): boolean {
    return this.requireStateRecord(pluginId).state.delete(key);
  }

  private stateKeys(pluginId: string): readonly string[] {
    return Object.freeze([...this.requireStateRecord(pluginId).state.keys()]);
  }

  // --- host-facing slot reads (all read-only, deterministic) ---------------------------

  navEntries(): readonly PluginNavEntryView[] {
    const views: PluginNavEntryView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === "root.nav") {
        views.push(
          Object.freeze({
            plugin: registered.pluginId,
            id: contribution.id,
            label: contribution.label,
            view: contribution.view,
            ...(contribution.hint !== undefined ? { hint: contribution.hint } : {}),
          }),
        );
      }
    }
    return Object.freeze(views);
  }

  /** Fire one nav entry's activation handler. Unknown ids are typed refusals. */
  activateNavEntry(contributionId: string): void {
    const registered = this.contributions.get(contributionId);
    const contribution = registered?.contribution;
    if (
      registered === undefined ||
      contribution === undefined ||
      !hasSlot(contribution) ||
      contribution.slot !== "root.nav"
    ) {
      throw new PluginLifecycleError(
        "unknown_contribution",
        `no root.nav contribution '${contributionId}' is registered`,
        registered?.pluginId ?? null,
      );
    }
    if (contribution.activate !== undefined) {
      contribution.activate();
    }
  }

  statusCues(snapshot: FullWorkbenchSnapshot): readonly PluginCueView[] {
    return this.collectCues("status.item", snapshot);
  }

  composerCues(snapshot: FullWorkbenchSnapshot): readonly PluginCueView[] {
    return this.collectCues("composer", snapshot);
  }

  private collectCues(
    slot: "status.item" | "composer",
    snapshot: FullWorkbenchSnapshot,
  ): readonly PluginCueView[] {
    const views: PluginCueView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === slot) {
        const cue = contribution.cue(snapshot);
        if (cue !== null) {
          views.push(
            Object.freeze({
              plugin: registered.pluginId,
              id: contribution.id,
              cue: Object.freeze(cue),
            }),
          );
        }
      }
    }
    return Object.freeze(views);
  }

  bottomPanels(snapshot: FullWorkbenchSnapshot): readonly PluginPanelView[] {
    return this.collectPanels("bottom.panel", snapshot);
  }

  overlayContent(snapshot: FullWorkbenchSnapshot): readonly PluginPanelView[] {
    return this.collectPanels("overlay", snapshot);
  }

  private collectPanels(
    slot: "bottom.panel" | "overlay",
    snapshot: FullWorkbenchSnapshot,
  ): readonly PluginPanelView[] {
    const views: PluginPanelView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === slot) {
        views.push(
          Object.freeze({
            plugin: registered.pluginId,
            id: contribution.id,
            content: freezeContent(contribution.render(snapshot)),
          }),
        );
      }
    }
    return Object.freeze(views);
  }

  traceRows(record: TraceRecord, snapshot: FullWorkbenchSnapshot): readonly PluginTraceRowView[] {
    const views: PluginTraceRowView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === "trace.row") {
        const line = contribution.render(record, snapshot);
        if (line !== null) {
          views.push(Object.freeze({ plugin: registered.pluginId, id: contribution.id, line }));
        }
      }
    }
    return Object.freeze(views);
  }

  inspectorSections(
    snapshot: FullWorkbenchSnapshot,
    focus: FocusRef,
  ): readonly PluginInspectorSectionView[] {
    const views: PluginInspectorSectionView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === "inspector.section") {
        const section = contribution.render({ snapshot, focus });
        if (section !== null) {
          views.push(Object.freeze({ plugin: registered.pluginId, id: contribution.id, section }));
        }
      }
    }
    return Object.freeze(views);
  }

  artifactViews(kind: string, artifact: PluginArtifactRecord): readonly PluginArtifactView[] {
    const views: PluginArtifactView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (
        hasSlot(contribution) &&
        contribution.slot === "artifact.viewer" &&
        contribution.artifact_kind === kind
      ) {
        views.push(
          Object.freeze({
            plugin: registered.pluginId,
            id: contribution.id,
            content: freezeContent(contribution.view(artifact)),
          }),
        );
      }
    }
    return Object.freeze(views);
  }

  viewExtensions(): readonly PluginViewExtensionView[] {
    const views: PluginViewExtensionView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (hasSlot(contribution) && contribution.slot === "case.view") {
        views.push(
          Object.freeze({
            plugin: registered.pluginId,
            id: contribution.id,
            view: contribution.view,
          }),
        );
      }
    }
    return Object.freeze(views);
  }

  /**
   * Plugin-derived presentation relations. Every edge kind is re-validated
   * against the provider's own namespace at read time, so a plugin can never
   * emit an edge impersonating a canonical source-typed relation kind.
   */
  pluginRelations(world: ProjectionWorld): readonly PluginRelationEdge[] {
    const edges: PluginRelationEdge[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (contribution.kind === "relation_provider") {
        for (const edge of contribution.relations(world)) {
          if (!edge.kind.startsWith(`plugin.${registered.pluginId}.`)) {
            throw new PluginContributionError(
              "relation_kind_not_namespaced",
              `relation kind '${edge.kind}' is not namespaced under 'plugin.${registered.pluginId}.*'`,
              registered.pluginId,
            );
          }
          edges.push(Object.freeze({ ...edge, provider: registered.pluginId }));
        }
      }
    }
    return Object.freeze(edges);
  }

  affordanceProjections(): readonly PluginAffordanceProjectionView[] {
    const views: PluginAffordanceProjectionView[] = [];
    for (const registered of this.contributions.values()) {
      const contribution = registered.contribution;
      if (contribution.kind === "affordance_projection") {
        views.push(
          Object.freeze({
            plugin: registered.pluginId,
            id: contribution.id,
            operation: contribution.operation,
            presentation_hint: contribution.presentation_hint,
            ...(contribution.estimate !== undefined ? { estimate: contribution.estimate } : {}),
          }),
        );
      }
    }
    return Object.freeze(views);
  }

  /**
   * Resolve one affordance projection against a fresh observation — the same
   * pure T09 resolver every host projection uses. This PREDICTS; it never
   * admits. There is deliberately no invocation counterpart: the host drives
   * its own gateway with `projection.operation`.
   */
  projectionDescriptor(
    contributionId: string,
    focus: FocusRef,
    observation: AffordanceObservation,
    options: { readonly session?: AffordanceSessionContext | undefined } = {},
  ): AffordanceDescriptor {
    const registered = this.contributions.get(contributionId);
    if (registered === undefined || registered.contribution.kind !== "affordance_projection") {
      throw new PluginLifecycleError(
        "unknown_contribution",
        `no affordance projection '${contributionId}' is registered`,
        registered?.pluginId ?? null,
      );
    }
    const projection = registered.contribution;
    return resolveAffordance(projection.operation, focus, observation, {
      ...(options.session !== undefined ? { session: options.session } : {}),
      presentation_hint: projection.presentation_hint,
      ...(projection.estimate !== undefined
        ? { payment_estimates: { [projection.operation]: projection.estimate } }
        : {}),
    });
  }

  // --- the out-of-process capability boundary -----------------------------------------

  /**
   * Cross the out-of-process boundary for one untrusted plugin: the payload is
   * serialized BEFORE the crossing, the capability must be in the manifest's
   * declared allowlist, and the response is validated then parsed before any
   * value is admitted into host state (untrusted external input).
   */
  async callCapability(
    pluginId: string,
    capability: string,
    payload: unknown,
  ): Promise<PluginCapabilityResult> {
    const record = this.plugins.get(pluginId);
    if (record === undefined) {
      throw new PluginLifecycleError(
        "plugin_not_mounted",
        `plugin '${pluginId}' is not mounted`,
        pluginId,
      );
    }
    if (record.mode !== "out_of_process" || record.transport === undefined) {
      throw new PluginLifecycleError(
        "not_out_of_process",
        `plugin '${pluginId}' is not an out-of-process plugin; the capability boundary applies only to untrusted plugins`,
        pluginId,
      );
    }
    if (!(record.manifest.capabilities ?? []).includes(capability)) {
      throw new PluginBoundaryError(
        "capability_not_declared",
        `capability '${capability}' is not in the declared allowlist of plugin '${pluginId}'`,
        pluginId,
      );
    }
    let serialized: string;
    try {
      serialized = JSON.stringify(payload) ?? "null";
    } catch (error) {
      throw new PluginBoundaryError(
        "malformed_boundary_payload",
        `the capability payload is not serializable: ${describeError(error)}`,
        pluginId,
      );
    }
    const request: OutOfProcessCapabilityRequest = Object.freeze({
      plugin: pluginId,
      capability,
      payload: serialized,
    });
    let response: OutOfProcessCapabilityResponse;
    try {
      const raw = await record.transport(request);
      if (!isBoundaryResponse(raw)) {
        throw new PluginBoundaryError(
          "malformed_boundary_response",
          "the boundary response is not a validated {ok, payload} document",
          pluginId,
        );
      }
      response = raw;
    } catch (error) {
      if (error instanceof PluginBoundaryError) {
        throw error;
      }
      throw new PluginBoundaryError(
        "boundary_transport_failed",
        `the out-of-process transport failed: ${describeError(error)}`,
        pluginId,
      );
    }
    let value: unknown;
    try {
      value = JSON.parse(response.payload) as unknown;
    } catch {
      throw new PluginBoundaryError(
        "malformed_boundary_response",
        "the boundary response payload is not valid JSON; nothing was admitted",
        pluginId,
      );
    }
    return Object.freeze({ ok: response.ok, value });
  }

  // --- fingerprint ----------------------------------------------------------------------

  /** The whole plugin-visible surface, deterministically ordered. */
  fingerprint(): PluginSurfaceFingerprint {
    const contributions = [...this.contributions.values()].map((registered) => ({
      id: registered.contribution.id,
      plugin: registered.pluginId,
      slot: slotOfContribution(registered.contribution),
      kind: registered.contribution.kind,
    }));
    const subscriptions = [...this.subscriptions.values()].map((subscription) => ({
      id: subscription.id,
      plugin: subscription.pluginId,
    }));
    const stateKeys: { plugin: string; key: string }[] = [];
    for (const pluginId of [...this.plugins.keys()].sort()) {
      const record = this.plugins.get(pluginId);
      if (record === undefined) {
        continue;
      }
      for (const key of [...record.state.keys()].sort()) {
        stateKeys.push({ plugin: pluginId, key });
      }
    }
    return Object.freeze({
      mounted: Object.freeze([...this.plugins.keys()].sort()),
      contributions: Object.freeze(contributions.map((entry) => Object.freeze(entry))),
      subscriptions: Object.freeze(
        subscriptions.map((entry) => Object.freeze({ id: entry.id, plugin: entry.plugin })),
      ),
      stateKeys: Object.freeze(stateKeys.map((entry) => Object.freeze(entry))),
    });
  }
}

// ---------------------------------------------------------------------------------------------

function freezeContent(content: PluginPanelContent): PluginPanelContent {
  return Object.freeze({
    lines: Object.freeze([...content.lines]),
    records: Object.freeze([...content.records]),
  });
}

function validateManifestForMount(manifest: PluginManifest): PluginManifest {
  // Re-validate through the strict parser even for caller-built objects:
  // manifests can arrive as untrusted documents, and mount never infers trust.
  // The RAW object goes to the parser — never a reconstruction via property
  // reads, which would resurrect prototype-inherited fields as own properties
  // and bypass the own-property gate (Correction Round 1 MINOR-2) — and a
  // non-object manifest meets the typed formation refusal there, never a raw
  // TypeError (Correction Round 1 MINOR-3).
  return parsePluginManifest(manifest);
}
