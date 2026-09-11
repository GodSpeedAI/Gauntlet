/**
 * Security, trust, secret, and privileged-input hardening instruments (plan T18,
 * REQ-SAFE-001..004, REQ-SEC-001..005, REQ-SXR-003, REQ-UX-006).
 *
 * This module is the hardening SWEEP's data + instruments layer. It owns NO
 * authority and NO admission path: every privileged boundary keeps its owning
 * module and enforcement (T09/T04/T05/T10/T11/T13/T15/T16). What lands here:
 *
 *   - DECLARED ENFORCEMENT SCOPE as typed data: trusted same-user arbitrary
 *     code is NOT sandboxed by a TUI; excluded threat classes are recorded,
 *     never implied covered (the evidence section states the same scope).
 *   - The PRIVILEGED BOUNDARY INVENTORY as data: every privileged side-effect
 *     class, its owning module, its NAMED enforcement mechanism, and its typed
 *     denial shape. `assertInventoryAttackCoverage` lets a suite enumerate the
 *     matrix so a NEW boundary cannot silently escape it: an inventory entry
 *     with no executed attack record fails the suite.
 *   - The secret-redaction scanner: pattern detection + a registered-secret
 *     redactor returning redacted views with STABLE reference markers (same
 *     secret → same marker, derived from the caller's reference or a per-
 *     redactor counter — never from the secret's content).
 *   - The fallback-visibility attestation: every provider/model/transport
 *     fallback path in config/preflight/client is either
 *     policy-permitted-and-visible or absent — no silent fallback.
 *   - Composition-level hardening wrappers for the carried T09 trigger-records
 *     that can be closed WITHOUT editing the gateway: an observe-deadline
 *     wrapper converting the unbounded head-of-line stall into the gateway's
 *     typed fail-closed refusal.
 *   - The isolation-claim audit: an instrument asserting no module surface
 *     presents kernel/worker isolation as a security sandbox and no line
 *     claims success from liveness — asserted as absent-claims over the
 *     actual sources, with the positive disclosures pinned.
 */

import type { OperatorActionRoute } from "./operation-gateway";

// --- Declared enforcement scope (stated up front, in code and in evidence) ------

/**
 * What this hardening effort does and does NOT enforce. Recorded so no test,
 * report, or module surface can imply a stronger guarantee than is enforced.
 */
export const ENFORCEMENT_SCOPE = Object.freeze({
  /** The workbench hardens authority routing and input binding for ONE user's own session. */
  threat_model: "hostile/untrusted INPUT crossing privileged boundaries of a same-user tool",
  /** REQ-SEC-001: Prime/Python/kernel/worker isolation is process containment, never a sandbox. */
  isolation_is_not_sandbox:
    "kernel/worker/plugin process containment is NOT a security sandbox — it organizes failure; hostile execution needs an external enforcement boundary appropriate to the risk",
  /** REQ-SAFE-004: liveness is provenance, never success. */
  liveness_is_not_success:
    "heartbeats, process aliveness, handle status, and mailbox volume are recorded signals; completion/settlement derives only from canonical evidence",
  /** REQ-SAFE-003: fallback is visible or absent. */
  fallback_must_be_visible:
    "provider/model/transport fallback is policy-permitted-and-visible or absent",
  /** Out of scope, owned elsewhere — never implied covered here. */
  excluded_threat_classes: Object.freeze([
    "trusted same-user arbitrary code (a TUI cannot sandbox the user's own execution context)",
    "remote network authentication and transport qualification (T20)",
    "host OS privilege escalation, kernel-level containment, or memory-safety attacks",
    "cryptographic commitment strength of the gateway's non-crypto payload digest (a binding instrument; admission re-validates regardless)",
    "secrets leaked by UPSTREAM authoritative stores into their own event payloads (the scanner detects and redacts views; upstream hygiene is the source's)",
  ]),
  /** The canonical admission path remains the ONE authority; nothing here substitutes for it. */
  authority: "the ONE canonical Gauntlet admission path decides every consequential operation",
} as const);

// --- Privileged boundary inventory (the sweep's data spine) ----------------------

/** The privileged side-effect classes the sweep covers (closed vocabulary). */
export type BoundaryArea =
  | "gauntlet_command"
  | "prime_execution"
  | "kernel_operation"
  | "terminal_process"
  | "plugin_loading"
  | "projection_store"
  | "transport_rpc"
  | "evidence_admission";

/** The NAMED enforcement mechanism each boundary relies on (closed vocabulary). */
export type EnforcementMechanism =
  | "canonical_admission"
  | "gateway_prevalidation"
  | "canonical_dispatch_before_side_effect"
  | "no_status_transition_from_communication"
  | "typed_boundary_validator"
  | "control_sequence_sanitizer"
  | "trust_policy"
  | "capability_allowlist_serialized_boundary"
  | "registry_gate"
  | "parameterized_sql"
  | "read_only_projection"
  | "canonical_decision_gate"
  | "surface_authentication";

export interface PrivilegedBoundary {
  /** Stable machine id — the attack-coverage contract key. */
  readonly id: string;
  readonly area: BoundaryArea;
  /** What privileged side effect this boundary gates. */
  readonly side_effect_class: string;
  /** The route(s) the suite drives the attack through. */
  readonly attack_route: string;
  readonly module: string;
  readonly owning_task: string;
  readonly enforcement: EnforcementMechanism;
  /** The exported symbol that embodies the enforcement (admission mirror check). */
  readonly enforcement_symbol: string;
  /** The typed denial the boundary produces (never a bare throw of string). */
  readonly denial_shape: string;
  /**
   * Honest enforcement-limit disclosure. Present ONLY on entries where a
   * related write path an adversary could name is NOT covered by the named
   * enforcement: the correction-round contract requires the true state to
   * stay visible in the inventory itself, never smoothed over. The boundary
   * suite asserts this text verbatim-shaped — removing or softening it fails.
   */
  readonly disclosed_bound?: string;
  readonly requirements: readonly string[];
}

/**
 * The privileged boundary inventory. DERIVED from the landed modules (read as
 * an adversary); adding a privileged path means adding an entry here AND an
 * executed attack record in the boundary suite — `assertInventoryAttackCoverage`
 * fails the suite for any entry without one.
 */
export const PRIVILEGED_BOUNDARY_INVENTORY: readonly PrivilegedBoundary[] = Object.freeze([
  Object.freeze({
    id: "gauntlet.action.gateway.http",
    area: "gauntlet_command",
    side_effect_class: "Gauntlet resolve/retry command via the workbench gateway over HTTP",
    attack_route: "OperationGateway.invoke over HttpOperatorActionRoute",
    module: "operation-gateway.ts",
    owning_task: "T09",
    enforcement: "gateway_prevalidation",
    enforcement_symbol: "OperationGateway.dispatch",
    denial_shape:
      "GatewayOutcome refused (stale_affordance_denied | revalidation_unavailable | idempotency_key_payload_mismatch)",
    requirements: ["REQ-SEC-002", "REQ-SAFE-002"],
  }),
  Object.freeze({
    id: "gauntlet.action.gateway.shim",
    area: "gauntlet_command",
    side_effect_class: "Gauntlet resolve/retry command via the in-process adapter shim route",
    attack_route: "OperationGateway.invoke over CanonicalSurfaceStub.asInProcessRoute",
    module: "operation-gateway.ts",
    owning_task: "T09",
    enforcement: "canonical_admission",
    enforcement_symbol: "OperatorActionRoute.perform",
    denial_shape: "canonical refused outcome (admission refusal code)",
    requirements: ["REQ-SEC-002", "REQ-SAFE-002"],
  }),
  Object.freeze({
    id: "gauntlet.action.direct.api",
    area: "gauntlet_command",
    side_effect_class:
      "Gauntlet action called directly at the surface's API shape (no workbench code)",
    attack_route: "CanonicalSurfaceStub.perform",
    module: "testing/canonical-surface-stub.ts",
    owning_task: "T09 (mirror)",
    enforcement: "canonical_admission",
    enforcement_symbol: "CanonicalSurfaceStub.decide",
    denial_shape: "canonical refused outcome",
    requirements: ["REQ-SEC-002"],
  }),
  Object.freeze({
    id: "gauntlet.action.direct.cli",
    area: "gauntlet_command",
    side_effect_class:
      "Gauntlet action called directly at the surface's CLI shape (no workbench code)",
    attack_route: "CanonicalSurfaceStub.performViaCli",
    module: "testing/canonical-surface-stub.ts",
    owning_task: "T09 (mirror)",
    enforcement: "canonical_admission",
    enforcement_symbol: "CanonicalSurfaceStub.decide",
    denial_shape: "canonical refused outcome",
    requirements: ["REQ-SEC-002"],
  }),
  Object.freeze({
    id: "prime.delegation.spawn",
    area: "prime_execution",
    side_effect_class: "Prime child session admission behind a canonical drive",
    attack_route: "PrimeDelegationDispatcher with a denial-class canonical route",
    module: "prime-recursion.ts",
    owning_task: "T10",
    enforcement: "canonical_dispatch_before_side_effect",
    enforcement_symbol: "PrimeDelegationRoute.perform",
    denial_shape: "canonical refusal with ZERO Prime side effect (no child spawned)",
    requirements: ["REQ-OPS-001", "REQ-SEC-002"],
  }),
  Object.freeze({
    id: "real.prime.host.spawn",
    area: "prime_execution",
    side_effect_class:
      "a REAL Bun.spawn child process launched from operator-configured prime_binary",
    attack_route:
      "RealPrimeHost construction/spawnChild with a relative/bare/metacharacter-bearing/traversal-shaped binary path, and a denial-class canonical route racing spawnChild",
    module: "real-prime-host.ts",
    owning_task: "workbench-intent-closure Phase C2",
    enforcement: "typed_boundary_validator",
    enforcement_symbol: "validateSpawnRequest",
    denial_shape:
      "TerminalBoundaryError at construction for an unbindable path; canonical refusal with ZERO real process spawned for a denial-class route (RealPrimeHost.spawnChild is never reached)",
    requirements: ["REQ-SEC-005", "REQ-OPS-001", "REQ-PRIME-003"],
  }),
  Object.freeze({
    id: "real.prime.host.stop",
    area: "prime_execution",
    side_effect_class:
      "SIGKILL delivered to a pid recovered either from a live Bun.Subprocess handle or from a qualified on-disk manifest",
    attack_route:
      "stopChild on an unknown/already-terminal session id (must refuse, zero signal sent); a forged *.qualified.json manifest naming an arbitrary pid, bounded by sessionDir's 0700 same-user directory permission",
    module: "real-prime-host.ts",
    owning_task: "workbench-intent-closure Phase D",
    enforcement: "typed_boundary_validator",
    enforcement_symbol: "RealPrimeHost.stopChild",
    denial_shape:
      "PrimeStopOutcome{stopped:false} with no process.kill call for an unknown or terminal session id; a signal is only ever sent to the exact pid this host itself recorded (spawned directly, or reattached from a manifest whose directory only this OS user can write)",
    requirements: ["REQ-SEC-005", "REQ-PRIME-009", "REQ-PRIME-010"],
  }),
  Object.freeze({
    id: "prime.mailbox.completion",
    area: "prime_execution",
    side_effect_class: "mailbox text attempting to change completion state",
    attack_route: "PrimeHostAdapter.receiveMailboxMessage with done-content",
    module: "prime-host.ts",
    owning_task: "T04",
    enforcement: "no_status_transition_from_communication",
    enforcement_symbol: "PrimeHostAdapter.receiveMailboxMessage",
    denial_shape: "handle status unchanged; completion only via schema-valid result contract",
    requirements: ["REQ-SAFE-004"],
  }),
  Object.freeze({
    id: "prime.steer.message",
    area: "prime_execution",
    side_effect_class: "operator steer intent attempting control-plane effect on admitted work",
    attack_route: "deliverOperatorMessage with steer intent",
    module: "prime-long-running.ts",
    owning_task: "T10",
    enforcement: "no_status_transition_from_communication",
    enforcement_symbol: "OperatorMessagePlan.establishes_completion",
    denial_shape: "plan.establishes_completion is literal false; child completion unchanged",
    requirements: ["REQ-SAFE-004"],
  }),
  Object.freeze({
    id: "kernel.action.callable",
    area: "kernel_operation",
    side_effect_class: "kernel-side consequential callable invocation",
    attack_route: "KernelActionCallable.invoke on a stale/invalidated set",
    module: "kernel-affordances.ts",
    owning_task: "T11",
    enforcement: "gateway_prevalidation",
    enforcement_symbol: "KernelAffordanceSet.invoke",
    denial_shape: "GatewayOutcome refused; wrapper invalidated, authority decision untouched",
    requirements: ["REQ-AFF-003", "REQ-SEC-002"],
  }),
  Object.freeze({
    id: "kernel.read.helper",
    area: "kernel_operation",
    side_effect_class: "kernel-side read routed near the action admission path",
    attack_route: "KernelReadHelper.read (no invoke exists at the type level)",
    module: "kernel-affordances.ts",
    owning_task: "T11",
    enforcement: "read_only_projection",
    enforcement_symbol: "KernelReadHelper.affects_canonical_state",
    denial_shape: "typed read_unavailable fail-closed; affects_canonical_state literal false",
    requirements: ["REQ-AFF-002"],
  }),
  Object.freeze({
    id: "kernel.input.broker",
    area: "kernel_operation",
    side_effect_class: "kernel input request attempting a direct prompt of the operator",
    attack_route: "KernelInputBroker.request with no attention sink",
    module: "kernel-affordances.ts",
    owning_task: "T11",
    enforcement: "read_only_projection",
    enforcement_symbol: "KernelInputBroker.request",
    denial_shape: "typed unroutable (no_attention_sink_registered); request stays pending",
    requirements: ["REQ-PRIME-006"],
  }),
  Object.freeze({
    id: "kernel.object.promotion",
    area: "kernel_operation",
    side_effect_class:
      "kernel object promoted into persisted/artifact state without canonical contract",
    attack_route: "KernelObjectLedger.transition to artifactized without artifact source",
    module: "evidence-promotion.ts",
    owning_task: "T11",
    enforcement: "canonical_decision_gate",
    enforcement_symbol: "KernelObjectLedger",
    denial_shape: "typed PromotionRefusalReason; nothing persisted",
    requirements: ["REQ-PRIME-006"],
  }),
  Object.freeze({
    id: "terminal.spawn.request",
    area: "terminal_process",
    side_effect_class: "process spawn from untrusted argv/path/env parameters",
    attack_route: "validateSpawnRequest with traversal/metacharacter/allowlist bypass shapes",
    module: "terminal-session.ts",
    owning_task: "T13",
    enforcement: "typed_boundary_validator",
    enforcement_symbol: "validateSpawnRequest",
    denial_shape: "TerminalBoundaryError (locus, closed code) before any provider call",
    requirements: ["REQ-SEC-005", "REQ-SXR-003"],
  }),
  Object.freeze({
    id: "terminal.automation.title",
    area: "terminal_process",
    side_effect_class: "terminal automation title carrying ANSI/OSC control sequences",
    attack_route: "bindTerminalAutomation with OSC 52/OSC 8 payloads",
    module: "terminal-session.ts",
    owning_task: "T13",
    enforcement: "control_sequence_sanitizer",
    enforcement_symbol: "sanitizeTerminalOutput",
    denial_shape: "inert sanitized text; neutralized counts recorded",
    requirements: ["REQ-SEC-005", "REQ-UX-006"],
  }),
  Object.freeze({
    id: "terminal.rpc.payload",
    area: "terminal_process",
    side_effect_class: "RPC payload destined for terminal-adjacent surfaces",
    attack_route: "validateRpcPayload with allowlist escapes and control payloads",
    module: "terminal-session.ts",
    owning_task: "T13",
    enforcement: "typed_boundary_validator",
    enforcement_symbol: "validateRpcPayload",
    denial_shape: "TerminalBoundaryError (rpc-payload locus) or inert bound strings",
    requirements: ["REQ-SEC-005"],
  }),
  Object.freeze({
    id: "plugin.load.trust",
    area: "plugin_loading",
    side_effect_class: "module loading as in-process trusted code without explicit trust",
    attack_route: "PluginCompositionManager.mount with an untrusted/undeclared manifest",
    module: "plugins.ts",
    owning_task: "T15",
    enforcement: "trust_policy",
    enforcement_symbol: "parsePluginManifest",
    denial_shape: "PluginTrustRefusalError / PluginManifestError (typed policy refusal)",
    requirements: ["REQ-SEC-004", "REQ-PLUGIN-002"],
  }),
  Object.freeze({
    id: "plugin.capability.boundary",
    area: "plugin_loading",
    side_effect_class: "untrusted plugin capability request crossing the process boundary",
    attack_route:
      "PluginCompositionManager.callCapability with undeclared capability / non-serializable payload",
    module: "plugins.ts",
    owning_task: "T15",
    enforcement: "capability_allowlist_serialized_boundary",
    enforcement_symbol: "PluginCompositionManager.callCapability",
    denial_shape: "PluginBoundaryError (typed) — nothing admitted unvalidated",
    requirements: ["REQ-SEC-004"],
  }),
  Object.freeze({
    id: "plugin.operation.registry.gate",
    area: "plugin_loading",
    side_effect_class: "plugin-defined operation attempting gateway authority",
    attack_route: "register an affordance_projection with a non-canonical operation",
    module: "plugins.ts",
    owning_task: "T15",
    enforcement: "registry_gate",
    enforcement_symbol: "validateContribution",
    denial_shape: "PluginContributionError (plugin_operation_not_canonical)",
    requirements: ["REQ-AFF-001", "REQ-SEC-002"],
  }),
  Object.freeze({
    id: "projection.store.sql",
    area: "projection_store",
    side_effect_class: "SQL/file paths receiving untrusted string content",
    attack_route: "applyEvent with SQL-metacharacter payloads into the in-memory store",
    module: "projection-store.ts",
    owning_task: "T06",
    enforcement: "parameterized_sql",
    enforcement_symbol: "SqliteProjectionStore.applyEvent",
    denial_shape: "content binds as DATA (rows round-trip verbatim); no statement executes",
    requirements: ["REQ-SEC-005"],
  }),
  Object.freeze({
    id: "transport.client.rpc",
    area: "transport_rpc",
    side_effect_class: "transport-level authentication masquerading as operation authority",
    attack_route: "GauntletReadClient.mutateUnit with missing/mismatched credentials",
    module: "client.ts",
    owning_task: "T00A",
    enforcement: "surface_authentication",
    enforcement_symbol: "CanonicalSurfaceStub.serveHttp auth gate",
    denial_shape: "401/403 surface auth refusal; transport auth never substitutes for authority",
    requirements: ["REQ-SEC-002"],
  }),
  Object.freeze({
    id: "evidence.admission.gate",
    area: "evidence_admission",
    side_effect_class: "kernel value promoted into evidence without canonical decision",
    attack_route: "EvidenceAdmissionGate.admit with forged/mismatched decision",
    module: "evidence-promotion.ts",
    owning_task: "T11",
    enforcement: "canonical_decision_gate",
    enforcement_symbol: "EvidenceAdmissionGate.admit",
    denial_shape: "refused_canonically | decision_mismatch | already_admitted",
    requirements: ["REQ-SEC-002"],
  }),
  Object.freeze({
    id: "evidence.frame.fabrication",
    area: "evidence_admission",
    side_effect_class:
      "authoritative evidence row fabricated without an admission (frame-construction helper OR the store write path)",
    attack_route:
      "authoritativeEvidenceFrame with a refused decision; direct WorkbenchModel/SqliteProjectionStore applyEvent with a forged evidence_added frame",
    module: "evidence-promotion.ts + projection-store.ts",
    owning_task: "T11 + T06",
    enforcement: "canonical_decision_gate",
    enforcement_symbol: "authoritativeEvidenceFrame",
    denial_shape:
      "frame-construction helper: typed throw — no frame exists without an admitted decision; STORE applyEvent path: UNGATED (writes whatever a trusted host hands it, including verified:true with no decision)",
    disclosed_bound:
      "the projection-store applyEvent path writing evidence rows is trusted-host-writable by design: NO gate consults a canonical decision there (a forged evidence_added frame lands a verified:true row — pinned as the true state in security-boundaries.test.ts). Mitigations, in force and tested: the projection is the noncanonical REBUILDABLE layer (a fresh generation drops the forged row); nothing consequential reads it as authority (canonical admission decides every consequential operation, demonstrated denied with the forged row present); only trusted same-user host code reaches applyEvent (declared enforcement scope).",
    requirements: ["REQ-SEC-002"],
  }),
]);

/** Typed coverage failure listing every inventory boundary without an attack record. */
export class InventoryCoverageError extends Error {
  readonly code = "inventory_attack_coverage_missing";
  readonly missing: readonly string[];

  constructor(missing: readonly string[]) {
    super(
      `the boundary suite has no executed attack record for ${missing.length} inventoried ` +
        `boundar${missing.length === 1 ? "y" : "ies"}: ${missing.join(", ")} — a new privileged ` +
        "boundary must be attacked, not silently admitted to the inventory",
    );
    this.name = "InventoryCoverageError";
    this.missing = Object.freeze([...missing]);
  }
}

/**
 * The attack-coverage contract: every inventory boundary id must appear in the
 * suite's executed-record set. Missing ids throw the typed
 * `InventoryCoverageError` (the suite calls this after executing its matrix).
 */
export function assertInventoryAttackCoverage(executedAttackIds: readonly string[]): void {
  const executed = new Set(executedAttackIds);
  const missing = PRIVILEGED_BOUNDARY_INVENTORY.filter(
    (boundary) => !executed.has(boundary.id),
  ).map((boundary) => boundary.id);
  if (missing.length > 0) {
    throw new InventoryCoverageError(missing);
  }
}

/** Inventory entries grouped by area (the evidence report's matrix view). */
export function inventoryByArea(): ReadonlyMap<BoundaryArea, readonly PrivilegedBoundary[]> {
  const grouped = new Map<BoundaryArea, PrivilegedBoundary[]>();
  for (const boundary of PRIVILEGED_BOUNDARY_INVENTORY) {
    const bucket = grouped.get(boundary.area);
    if (bucket === undefined) {
      grouped.set(boundary.area, [boundary]);
    } else {
      bucket.push(boundary);
    }
  }
  return grouped;
}

// --- Secret redaction scanner -----------------------------------------------------

/** The secret classes the scanner recognizes (closed vocabulary). */
export type SecretKind =
  | "bearer_token"
  | "credential_url"
  | "key_value_secret"
  | "private_key_block"
  | "known_token_shape"
  | "jwt"
  | "registered_secret";

export interface SecretPatternMatch {
  readonly kind: SecretKind;
  readonly start: number;
  readonly end: number;
  /** The matched span (needed by the detector's callers to build redacted views). */
  readonly matched: string;
}

interface PatternRule {
  readonly kind: SecretKind;
  readonly pattern: RegExp;
}

/**
 * Detection rules for secret material that was never registered. Each rule is
 * calibrated against the negative-control corpus in secret-leakage.test.ts:
 * non-secret look-alikes (policy/rate/schedule/ttl/hash-qualified key names,
 * placeholder bearers, plain URLs, run ids, elided PAT docs, bare JWT-header
 * prose) must NOT match. Correction round 1 added the verifier's demonstrated
 * candidates: UPPERCASE env-style keys (covered case-insensitively, with
 * secret_access_key named explicitly), ghp_ PATs, JWTs without key hints, and
 * single-quoted values — each with a positive AND a corpus negative.
 */
const DETECTION_RULES: readonly PatternRule[] = Object.freeze([
  {
    kind: "bearer_token",
    // "Bearer <credentials>" — credential alphabet, 8+ chars. "<token>" (with
    // angle brackets) is intentionally NOT in the alphabet: placeholders pass.
    pattern: /\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/g,
  },
  {
    kind: "credential_url",
    // scheme://user:password@host — a password-bearing URL. Plain host:port
    // URLs never match (the "@" is mandatory before the host).
    pattern: /[a-zA-Z][a-zA-Z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]+@[^\s"'<>]+/g,
  },
  {
    kind: "key_value_secret",
    // password/token/key-family assignments — case-insensitive, so UPPERCASE
    // env-style keys (PASSWORD=) match. Compound cloud-key names must be
    // named IN FULL (aws_secret_access_key): the leading \b refuses any key
    // word CONTAINED mid-word ("SECRET" inside AWS_SECRET_ACCESS_KEY has "_"
    // before it), which is exactly what keeps qualified look-alikes
    // (PASSWORD_HASH_ALGORITHM, AWS_SECRET_ACCESS_KEY_POLICY) out. Single-
    // quoted values are the second alternative.
    pattern:
      /\b(?:password|passwd|secret|aws_secret_access_key|secret_access_key|token|api_key|api-key|access_token|access-key|access_key|secret_key|secret-key|private_key|private-key|session_token|session-token|auth_token|auth-token|client_secret|client-secret|credential|credentials)\b\s*[:=]\s*(?:"([^"\s]{8,})"|'([^'\s]{8,})'|([A-Za-z0-9._~+/=-]{8,}))/gi,
  },
  {
    kind: "private_key_block",
    pattern: /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g,
  },
  {
    kind: "known_token_shape",
    // Well-known provider token shapes (sk-…, ghp_…, github_pat_…, AWS
    // AKIA…, Slack xox…). Elided prose ("tokens look like ghp_ …") never
    // reaches the minimum lengths.
    pattern:
      /\b(?:sk-[A-Za-z0-9_-]{16,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{12,20}|xox[baprs]-[A-Za-z0-9-]{10,})\b/g,
  },
  {
    kind: "jwt",
    // A three-segment base64url JWT (header.payload.signature) — detectable
    // WITHOUT any key hint because an encoded JWT header always starts with
    // "eyJ" (base64 of '{"'). Two-segment or dotless base64 prose never
    // matches (the corpus pins that).
    pattern: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g,
  },
]);

/** Detect (never modify) probable secret material in one string. */
export function detectSecretPatterns(text: string): readonly SecretPatternMatch[] {
  const matches: SecretPatternMatch[] = [];
  for (const rule of DETECTION_RULES) {
    rule.pattern.lastIndex = 0;
    let hit = rule.pattern.exec(text);
    while (hit !== null) {
      if (hit[0].length > 0) {
        matches.push({
          kind: rule.kind,
          start: hit.index,
          end: hit.index + hit[0].length,
          matched: hit[0],
        });
      }
      if (hit.index === rule.pattern.lastIndex) {
        rule.pattern.lastIndex += 1;
      }
      hit = rule.pattern.exec(text);
    }
  }
  matches.sort((a, b) => (a.start === b.start ? a.end - b.end : a.start - b.start));
  // Non-overlapping: on ties/overlap keep the earlier, longer match.
  const kept: SecretPatternMatch[] = [];
  let cursor = -1;
  for (const match of matches) {
    if (match.start >= cursor) {
      kept.push(match);
      cursor = match.end;
    }
  }
  return Object.freeze(kept);
}

/** A stable redaction marker: `[REDACTED:<kind>:<ref>]` — carries no secret content. */
export function redactionMarker(kind: SecretKind, ref: string): string {
  return `[REDACTED:${kind}:${ref}]`;
}

/**
 * The registered-secret redactor. Secrets are REGISTERED (from approved
 * sources: env, cli, config indirection) and every redacted view replaces each
 * occurrence with the SAME stable marker — correlations survive, content does
 * not. Marker refs come from the caller or a per-redactor counter, never from
 * the secret's content (no fingerprint leak).
 */
export class SecretRedactor {
  private readonly byValue = new Map<
    string,
    { readonly kind: SecretKind; readonly marker: string }
  >();
  private counter = 0;

  /**
   * Register one secret under a kind. Returns its stable marker. Re-registering
   * the same value returns the SAME marker (idempotent registration).
   */
  register(kind: SecretKind, value: string, ref?: string): string {
    if (value.length === 0) {
      throw new Error("the secret redactor refuses to register an empty secret");
    }
    const known = this.byValue.get(value);
    if (known !== undefined) {
      return known.marker;
    }
    this.counter += 1;
    const marker = redactionMarker(kind, ref ?? String(this.counter));
    this.byValue.set(value, { kind, marker });
    return marker;
  }

  /** How many distinct secrets are registered. */
  get size(): number {
    return this.byValue.size;
  }

  /**
   * Redact ONE string: registered secrets by exact occurrence (any case count),
   * then unregistered pattern matches. Returns the redacted view plus what was
   * replaced (spans refer to the INPUT string).
   */
  redactText(text: string): {
    readonly text: string;
    readonly redactions: readonly SecretPatternMatch[];
  } {
    // Collect exact-occurrence spans for registered secrets first.
    const spans: SecretPatternMatch[] = [];
    for (const [value, entry] of this.byValue) {
      let index = text.indexOf(value);
      while (index !== -1) {
        spans.push({
          kind: entry.kind,
          start: index,
          end: index + value.length,
          matched: value,
        });
        index = text.indexOf(value, index + value.length);
      }
    }
    // Then unregistered pattern matches that don't overlap a registered span.
    for (const match of detectSecretPatterns(text)) {
      const overlapped = spans.some((span) => match.start < span.end && span.start < match.end);
      if (!overlapped) {
        spans.push(match);
      }
    }
    spans.sort((a, b) => a.start - b.start);
    let out = "";
    let cursor = 0;
    const redactions: SecretPatternMatch[] = [];
    for (const span of spans) {
      if (span.start < cursor) {
        continue;
      }
      out += text.slice(cursor, span.start);
      const marker = this.markerFor(span);
      out += marker;
      redactions.push(span);
      cursor = span.end;
    }
    out += text.slice(cursor);
    return { text: out, redactions: Object.freeze(redactions) };
  }

  private markerFor(span: SecretPatternMatch): string {
    const known = this.byValue.get(span.matched);
    if (known !== undefined) {
      return known.marker;
    }
    // Unregistered pattern match: kind-scoped generic marker with a per-kind
    // reference, stable for the redactor's lifetime but value-independent.
    return redactionMarker(span.kind, "pattern");
  }

  /**
   * Deep-redact a JSON-like value (snapshots, evidence frames, manifests,
   * outcome objects). Returns a NEW structure; the input is never mutated.
   * Unknown node types pass through untouched.
   *
   * T18 correction-round trigger-record (recorded, NOT fixed here): descent
   * covers strings, arrays, and plain objects — Map and Set instances are
   * passed through UNREDACTED (treated as opaque leaves). Callers holding
   * Map/Set-shaped payloads must convert them to plain structures first; the
   * forbidden channels this redactor guards (snapshots, frames, manifests,
   * outcome objects) are plain JSON-shaped by construction.
   */
  redactDeep<T>(value: T): T {
    if (typeof value === "string") {
      return this.redactText(value).text as unknown as T;
    }
    if (Array.isArray(value)) {
      const out: unknown[] = [];
      for (const entry of value) {
        out.push(this.redactDeep(entry));
      }
      return out as unknown as T;
    }
    if (typeof value === "object" && value !== null) {
      const out: Record<string, unknown> = {};
      for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
        out[key] = this.redactDeep(entry);
      }
      return out as unknown as T;
    }
    return value;
  }

  /** True when the text still contains any registered secret (the leakage probe). */
  leaksAny(text: string): boolean {
    for (const value of this.byValue.keys()) {
      if (text.includes(value)) {
        return true;
      }
    }
    return false;
  }

  /** Every registered secret's stable marker (the reference map for evidence). */
  markers(): readonly string[] {
    return Object.freeze([...this.byValue.values()].map((entry) => entry.marker));
  }
}

/**
 * The scanner's own negative control: the corpus of non-secret look-alikes the
 * detector must NOT mangle. Exported so the test suite and the detector stay
 * calibrated together — changing a detection rule that breaks one of these
 * fails the suite.
 */
export const SECRET_LOOKALIKE_NEGATIVE_CONTROLS: readonly string[] = Object.freeze([
  "password_policy=strict",
  "password_policy: strict",
  "token_bucket_fill_rate=1000/s",
  "token_bucket: documented",
  "session_token_ttl=3600",
  "credentials_report=annual",
  "key_rotation_schedule=cron(0 0 * * *)",
  "Authorization: Bearer <token>",
  "Bearer <your-token-here>",
  "endpoint http://127.0.0.1:4000/api/v1/state",
  "endpoint https://gauntlet.example/served",
  "run 01J9T09DENYBYPASSRUN settled",
  "digest 4bd5c124a3fef1b113375eb79da32b9257cbd2394ea45643d6e4116ccd0583d3",
  "SSH key fingerprint SHA256:0000000000000000000000000000000000000000000",
  "password hashing uses argon2id",
  // Correction round 1 — look-alikes for the verifier's demonstrated
  // candidates (each new pattern calibrates with a positive AND a negative):
  "PASSWORD_HASH_ALGORITHM=argon2id",
  "PASSWORDS_FILE=/etc/checked",
  "AWS_SECRET_ACCESS_KEY_POLICY=managed",
  "GitHub PATs look like ghp_ (elided in docs)",
  "the JWT header segment always starts eyJ (plain base64 of a brace)",
  "token: 'short'",
]);

// --- Fallback visibility attestation (REQ-SAFE-003) -------------------------------

export type FallbackStatus = "policy_permitted_visible" | "absent";

export interface FallbackPathAttestation {
  readonly id: string;
  readonly module: string;
  /** The fallback transition described (from → to). */
  readonly transition: string;
  readonly status: FallbackStatus;
  /** WHERE the operator sees the fallback. Required unless status is "absent". */
  readonly visibility_signal: string | null;
  readonly policy_ref: string;
}

/**
 * Every provider/model/transport fallback path in the landed config/preflight/
 * client surface, attested. The rule (REQ-SAFE-003): each path is either
 * policy-permitted AND visible, or absent. There is no third attestation.
 */
export const FALLBACK_VISIBILITY_LEDGER: readonly FallbackPathAttestation[] = Object.freeze([
  Object.freeze({
    id: "config.default-endpoint",
    module: "config.ts",
    transition: "unset gauntlet endpoint → documented default http://127.0.0.1:4000",
    status: "policy_permitted_visible",
    visibility_signal:
      "DEFAULT_CONFIG export; the resolved config returns the effective endpoint verbatim",
    policy_ref: "REQ-CONFIG-001/003 (documented defaults)",
  }),
  Object.freeze({
    id: "config.transport-classification",
    module: "config.ts",
    transition: "unset transport → auto-classified from the endpoint host",
    status: "policy_permitted_visible",
    visibility_signal: "resolved config .transport field; classifyTransport is a pure function",
    policy_ref: "REQ-CONFIG-005",
  }),
  Object.freeze({
    id: "prime.absent.gauntlet-only",
    module: "preflight.ts",
    transition: "Prime unconfigured/unreachable → Gauntlet-only read-inspection mode",
    status: "policy_permitted_visible",
    visibility_signal:
      "preflight warning text + prime_status 'unavailable' + prime capabilities listed blocked",
    policy_ref: "REQ-CONFIG-012, REQ-SAFE-003",
  }),
  Object.freeze({
    id: "gauntlet.unreachable.disconnected",
    module: "client.ts + preflight.ts",
    transition: "canonical surface unreachable → disconnected snapshot / fail-closed capabilities",
    status: "policy_permitted_visible",
    visibility_signal:
      "RuntimeSnapshot.status 'disconnected' + preflight canonical_source_unavailable error with blocked mutation capabilities",
    policy_ref: "REQ-SAFE-002, REQ-CONFIG-012",
  }),
  Object.freeze({
    id: "provider.model.fallback",
    module: "config.ts",
    transition: "primary model/provider → alternate model/provider",
    status: "absent",
    visibility_signal: null,
    policy_ref:
      "REQ-SAFE-003 — no model/provider field exists in WorkbenchConfig; nothing to fall back",
  }),
  Object.freeze({
    id: "transport.alternate-endpoint-retry",
    module: "client.ts",
    transition: "primary endpoint failure → automatic retry against an alternate endpoint",
    status: "absent",
    visibility_signal: null,
    policy_ref:
      "REQ-SAFE-003 — the read client holds ONE baseUrl; failures surface as status, never a second endpoint",
  }),
  Object.freeze({
    id: "transport.automatic-route-switch",
    module: "operation-gateway.ts",
    transition: "route failure → automatic switch to another admission route",
    status: "absent",
    visibility_signal: null,
    policy_ref:
      "REQ-SAFE-003 — invokeVia is an explicit caller action on one gateway; no automatic switching exists",
  }),
]);

/** Typed failure when the ledger itself would attest a silent fallback. */
export class SilentFallbackError extends Error {
  readonly code = "silent_fallback_forbidden";
  readonly offending: readonly string[];

  constructor(offending: readonly string[]) {
    super(
      `REQ-SAFE-003 violation in the fallback ledger: ${offending.join(", ")} — a fallback must be ` +
        "policy-permitted-and-visible or absent; a permitted path without a visibility signal is silent fallback",
    );
    this.name = "SilentFallbackError";
    this.offending = Object.freeze([...offending]);
  }
}

/**
 * Assert the ledger contains only the two attested statuses, and every
 * policy-permitted path names WHERE the operator sees it.
 */
export function assertNoSilentFallback(
  ledger: readonly FallbackPathAttestation[] = FALLBACK_VISIBILITY_LEDGER,
): void {
  const offending: string[] = [];
  for (const entry of ledger) {
    if (entry.status === "policy_permitted_visible") {
      if (entry.visibility_signal === null || entry.visibility_signal.trim().length === 0) {
        offending.push(entry.id);
      }
    }
  }
  if (offending.length > 0) {
    throw new SilentFallbackError(offending);
  }
}

// --- Composition-level hardening: the observe head-of-line deadline (T09 N1b) ----

/** The typed deadline refusal (the gateway maps ANY observe throw to `revalidation_unavailable`). */
export class ObserveDeadlineExceededError extends Error {
  readonly code = "observe_deadline_exceeded";

  constructor(deadlineMs: number) {
    super(
      `the canonical surface observation did not settle within ${deadlineMs}ms; ` +
        "the drive fails closed instead of stalling the host decision queue",
    );
    this.name = "ObserveDeadlineExceededError";
  }
}

/**
 * Wrap a route so its observation must settle within a deadline (T09 trigger-
 * record 3 closure at the composition level, without editing the gateway):
 * a route whose observe never settles otherwise stalls every later drive on
 * the gateway instance (a bounded DoS). With the wrapper, the stalled drive
 * becomes the gateway's typed fail-closed `revalidation_unavailable` refusal
 * and later drives proceed. Hosts construct the gateway over the wrapped route.
 *
 * T18 correction-round trigger-record (recorded, NOT fixed here): this wrapper
 * covers the OBSERVE leg only. A route whose PERFORM never settles still
 * stalls its gateway instance (the serialized decision queue holds the drive
 * open) — the same instance-wide DoS shape, one leg over. The fix shape is a
 * composition-level perform-deadline (and/or a host-level drive budget);
 * it is T09-owned because the queue is the gateway's.
 */
export function withObserveDeadline(
  route: OperatorActionRoute,
  deadlineMs: number,
): OperatorActionRoute {
  if (!Number.isFinite(deadlineMs) || deadlineMs <= 0) {
    throw new Error("withObserveDeadline requires a positive finite deadline in milliseconds");
  }
  return {
    observe: (target: {
      unit: string | null;
    }): Promise<Awaited<ReturnType<OperatorActionRoute["observe"]>>> =>
      new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          reject(new ObserveDeadlineExceededError(deadlineMs));
        }, deadlineMs);
        route
          .observe(target)
          .then((observation) => {
            clearTimeout(timer);
            resolve(observation);
          })
          .catch((error: unknown) => {
            clearTimeout(timer);
            reject(error);
          });
      }),
    perform: (command) => route.perform(command),
  };
}

// --- Isolation / liveness claim audit (REQ-SEC-001, REQ-SAFE-004) -----------------

export interface ClaimAuditFile {
  /** Module-relative file name (for the report). */
  readonly name: string;
  /** Full source text. */
  readonly text: string;
}

export interface ClaimAuditFinding {
  readonly file: string;
  readonly line: number;
  readonly text: string;
}

export interface IsolationClaimAudit {
  readonly files_scanned: readonly string[];
  /** sandbox mentions whose line does NOT carry a negation/disclosure context. */
  readonly unhedged_sandbox_claims: readonly ClaimAuditFinding[];
  /** Required disclosure patterns that are MISSING from their module. */
  readonly missing_disclosures: readonly { readonly file: string; readonly pattern: string }[];
  readonly ok: boolean;
}

/**
 * The disclosure each landed module MUST carry, as source patterns. These pin
 * the honest-language requirements (REQ-SEC-001/REQ-SAFE-004) so a refactor
 * that silently drops the disclosure fails the suite.
 */
export const REQUIRED_ISOLATION_DISCLOSURES: readonly {
  readonly file: string;
  readonly pattern: RegExp;
}[] = Object.freeze([
  Object.freeze({
    file: "prime-kernel.ts",
    pattern: /liveness alone cannot settle work|liveness != work success/i,
  }),
  Object.freeze({
    file: "prime-host.ts",
    pattern: /NOT a contract|mailbox reply is a signal/i,
  }),
  Object.freeze({
    file: "prime-long-running.ts",
    pattern: /Liveness never implies success|never a basis/i,
  }),
  Object.freeze({
    file: "prime-recursion.ts",
    pattern: /not Prime capabilities|never presented as a Prime capability/i,
  }),
  Object.freeze({
    file: "plugins.ts",
    pattern: /NEVER INFERRED|never inferred/i,
  }),
  Object.freeze({
    file: "terminal-session.ts",
    pattern: /values are never recorded|secret discipline/i,
  }),
]);

/** Lines that merely mention "sandbox" must carry a negation on the SAME line. */
const NEGATION_CONTEXT = /\b(not|never|no\b|isn't|cannot|can't|excluded|out of scope|instead)\b/i;

/**
 * Audit module sources for misrepresenting isolation/liveness as stronger
 * guarantees: every "sandbox" mention must be in a negation/disclosure context,
 * and every required disclosure pattern must be present. Returns the findings;
 * `ok` is true only when both lists are empty.
 *
 * `sandboxScanExempt` names files whose "sandbox" mentions are the
 * META-discussion of the rule itself (the audit instrument module and the
 * pinned ENFORCEMENT_SCOPE statement live there) — the suite must pin those
 * files' disclosures with dedicated assertions instead, and any OTHER file
 * naming "sandbox" without a negation still fails.
 */
export function auditIsolationClaims(
  files: readonly ClaimAuditFile[],
  options: { readonly sandboxScanExempt?: readonly string[] } = {},
): IsolationClaimAudit {
  const exempt = new Set(options.sandboxScanExempt ?? []);
  const filesByName = new Map(files.map((file) => [file.name, file]));
  const unhedged: ClaimAuditFinding[] = [];
  for (const file of files) {
    if (exempt.has(file.name)) {
      continue;
    }
    const lines = file.text.split("\n");
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index] ?? "";
      if (/sandbox/i.test(line) && !NEGATION_CONTEXT.test(line)) {
        unhedged.push({ file: file.name, line: index + 1, text: line.trim().slice(0, 160) });
      }
    }
  }
  const missing: { file: string; pattern: string }[] = [];
  for (const disclosure of REQUIRED_ISOLATION_DISCLOSURES) {
    const file = filesByName.get(disclosure.file);
    if (file === undefined || !disclosure.pattern.test(file.text)) {
      missing.push({ file: disclosure.file, pattern: String(disclosure.pattern) });
    }
  }
  return {
    files_scanned: Object.freeze(files.map((file) => file.name)),
    unhedged_sandbox_claims: Object.freeze(unhedged),
    missing_disclosures: Object.freeze(missing),
    ok: unhedged.length === 0 && missing.length === 0,
  };
}
