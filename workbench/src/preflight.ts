/**
 * Typed startup preflight, capability validation, and degraded-mode contract.
 * Conforms to spec §20.2, REQ-CONFIG-010..012, and plan T02.
 */

import type { WorkbenchConfig } from "./config";

export type PreflightErrorCode =
  | "missing_config_error"
  | "parse_error"
  | "schema_error"
  | "unsupported_kind_error"
  | "missing_credential_error"
  | "input_validation_error"
  | "canonical_source_unavailable"
  | "prime_integration_error"
  | "plugin_trust_error";

export interface PreflightError {
  readonly code: PreflightErrorCode;
  readonly message: string;
  readonly blast_radius: string;
  readonly recoverability_hint: string;
  readonly source: "gauntlet" | "prime" | "projection" | "config" | "runtime";
}

export type ConnectionStatus = "connected" | "degraded" | "unavailable";
export type PrimeStatus = "available" | "degraded" | "unavailable";

export interface PreflightProbeResults {
  readonly gauntlet_reachable: boolean;
  readonly gauntlet_protocol_version?: string | undefined;
  readonly prime_reachable: boolean;
  readonly prime_version?: string | undefined;
  readonly projection_store_writable: boolean;
}

export interface PreflightReport {
  readonly ok: boolean;
  readonly degraded: boolean;
  readonly gauntlet_status: ConnectionStatus;
  readonly prime_status: PrimeStatus;
  readonly errors: readonly PreflightError[];
  readonly warnings: readonly string[];
  readonly allowed_capabilities: readonly string[];
  readonly blocked_capabilities: readonly string[];
}

/**
 * Evaluate preflight conditions against probe results.
 * Enforces blast-radius scoping and degraded-mode fail-closed rules (REQ-CONFIG-012).
 */
export function evaluatePreflight(
  config: WorkbenchConfig,
  probes: PreflightProbeResults,
): PreflightReport {
  const errors: PreflightError[] = [];
  const warnings: string[] = [];
  const allowed_capabilities: string[] = [];
  const blocked_capabilities: string[] = [];

  // 1. Evaluate Canonical Gauntlet Connectivity
  let gauntlet_status: ConnectionStatus = "unavailable";
  if (probes.gauntlet_reachable) {
    gauntlet_status = "connected";
    allowed_capabilities.push(
      "state_snapshot",
      "state_refresh",
      "unit_inspection",
      "escalation_inbox",
    );
  } else {
    gauntlet_status = "unavailable";
    errors.push({
      code: "canonical_source_unavailable",
      message: `Cannot reach canonical Gauntlet endpoint at ${config.gauntlet_endpoint}`,
      blast_radius:
        "All state-changing commands fail closed; projections marked stale or unavailable.",
      recoverability_hint:
        "Verify `gauntlet serve` is running and the endpoint configuration is correct.",
      source: "gauntlet",
    });
    // Attack 2 (REQ-CONFIG-012): All mutations fail closed when Gauntlet connectivity is lost
    blocked_capabilities.push(
      "state_mutation",
      "action_resolve",
      "action_retry",
      "evidence_admission",
    );
  }

  // 2. Evaluate Prime Agent Integration
  let prime_status: PrimeStatus = "unavailable";
  if (config.prime_endpoint === null && !probes.prime_reachable) {
    // Prime is optional: absence permits core read-only inspection (REQ-CONFIG-012, Attack 1)
    prime_status = "unavailable";
    warnings.push(
      "Prime integration is not configured; running in Gauntlet-only read inspection mode.",
    );
    blocked_capabilities.push("prime_chat", "prime_steering", "prime_subagent_spawn");
  } else if (probes.prime_reachable) {
    prime_status = "available";
    allowed_capabilities.push("prime_chat", "prime_steering", "prime_subagent_spawn");
  } else {
    prime_status = "unavailable";
    errors.push({
      code: "prime_integration_error",
      message: `Prime endpoint configured (${config.prime_endpoint}) but unreachable or incompatible.`,
      blast_radius:
        "Prime-dependent views and agent steering blocked; core Gauntlet inspection unaffected.",
      recoverability_hint:
        "Check Prime process or unset PRIME_ENDPOINT to run in Gauntlet-only mode.",
      source: "prime",
    });
    blocked_capabilities.push("prime_chat", "prime_steering", "prime_subagent_spawn");
  }

  // 3. Evaluate Projection Store
  if (!probes.projection_store_writable) {
    warnings.push(
      `Projection store at ${config.projection_store} is not writable; running with in-memory projections.`,
    );
  } else {
    allowed_capabilities.push("projection_caching");
  }

  // Overall status
  // If Gauntlet is reachable, workbench is operational (possibly degraded if Prime is missing)
  const ok = probes.gauntlet_reachable;
  const degraded = prime_status !== "available" || !probes.projection_store_writable;

  return {
    ok,
    degraded,
    gauntlet_status,
    prime_status,
    errors,
    warnings,
    allowed_capabilities,
    blocked_capabilities,
  };
}

/**
 * Execute runtime preflight checks against live or probed endpoints.
 */
export async function runRuntimePreflight(
  config: WorkbenchConfig,
  probeOverride?: Partial<PreflightProbeResults>,
): Promise<PreflightReport> {
  // If probe override is supplied, evaluate directly
  if (probeOverride) {
    const fullProbes: PreflightProbeResults = {
      gauntlet_reachable: probeOverride.gauntlet_reachable ?? false,
      gauntlet_protocol_version: probeOverride.gauntlet_protocol_version,
      prime_reachable: probeOverride.prime_reachable ?? false,
      prime_version: probeOverride.prime_version,
      projection_store_writable: probeOverride.projection_store_writable ?? true,
    };
    return evaluatePreflight(config, fullProbes);
  }

  // Probe live Gauntlet endpoint
  let gauntlet_reachable = false;
  let gauntlet_protocol_version: string | undefined;
  try {
    const res = await fetch(`${config.gauntlet_endpoint.replace(/\/+$/, "")}/api/v1/capabilities`, {
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (res.ok) {
      gauntlet_reachable = true;
      const data = (await res.json()) as Record<string, unknown>;
      if (typeof data["protocol_version"] === "string") {
        gauntlet_protocol_version = data["protocol_version"];
      }
    }
  } catch {
    gauntlet_reachable = false;
  }

  // Probe live Prime endpoint (if configured)
  let prime_reachable = false;
  if (config.prime_endpoint) {
    try {
      const res = await fetch(`${config.prime_endpoint.replace(/\/+$/, "")}/health`, {
        method: "GET",
      });
      prime_reachable = res.ok;
    } catch {
      prime_reachable = false;
    }
  }

  return evaluatePreflight(config, {
    gauntlet_reachable,
    gauntlet_protocol_version,
    prime_reachable,
    projection_store_writable: true,
  });
}
