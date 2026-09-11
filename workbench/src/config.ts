/**
 * Workbench configuration authority, precedence resolution, and secret indirection.
 * Conforms to spec §20.2, REQ-CONFIG-001..003, REQ-CONFIG-005, and plan T02.
 *
 * Strict precedence:
 *   1. Explicit CLI arguments / options
 *   2. Environment variables (GAUNTLET_WORKBENCH_*)
 *   3. Configuration file (gauntlet-workbench.yaml / .json)
 *   4. Documented defaults
 */

export type TransportKind = "local" | "remote";

export interface PerformanceBudget {
  readonly max_latency_ms?: number | undefined;
  readonly max_tokens?: number | undefined;
}

export interface WorkbenchConfig {
  readonly gauntlet_endpoint: string;
  readonly prime_endpoint: string | null;
  /**
   * Path to a real Prime binary for same-host integration (plan Phase C2;
   * REQ-CONFIG-003 names "socket" as a MUST-configurable surface, and T24
   * qualified the real substrate as a binary, not an HTTP endpoint).
   * `prime_endpoint` stays for remote/HTTP-shaped Prime; this is the
   * same-host `RealPrimeHost` path. Additive: `null` (unconfigured) behaves
   * exactly as it did before this field existed (D6).
   */
  readonly prime_binary: string | null;
  /**
   * Unix socket path for a same-host Prime daemon (REQ-BUN-005 prefers a
   * local socket for same-host integration). Declared additively; whether
   * `RealPrimeHost` connects to it is independent of this field's presence
   * — an honest declaration is not itself a claim of use (D1).
   */
  readonly prime_socket: string | null;
  readonly projection_store: string;
  readonly plugin_roots: readonly string[];
  readonly performance_budget: PerformanceBudget;
  readonly transport: TransportKind;
  readonly session_token: string | null;
}

export interface RawConfigInput {
  gauntlet_endpoint?: string | undefined;
  prime_endpoint?: string | null | undefined;
  prime_binary?: string | null | undefined;
  prime_socket?: string | null | undefined;
  projection_store?: string | undefined;
  plugin_roots?: readonly string[] | undefined;
  performance_budget?: PerformanceBudget | undefined;
  transport?: string | undefined;
  session_token?: string | null | undefined;
}

export const DEFAULT_CONFIG: WorkbenchConfig = {
  gauntlet_endpoint: "http://127.0.0.1:4000",
  prime_endpoint: null,
  prime_binary: null,
  prime_socket: null,
  projection_store: ".gauntlet/projections.db",
  plugin_roots: [],
  performance_budget: {
    max_latency_ms: 500,
    max_tokens: 100_000,
  },
  transport: "local",
  session_token: null,
};

/**
 * Determine transport kind (local loopback vs remote) per REQ-CONFIG-005.
 */
export function classifyTransport(endpoint: string): TransportKind {
  try {
    const url = new URL(endpoint.startsWith("http") ? endpoint : `http://${endpoint}`);
    const host = url.hostname;
    if (host === "127.0.0.1" || host === "localhost" || host === "::1") {
      return "local";
    }
    return "remote";
  } catch {
    return "local";
  }
}

/**
 * Deterministically resolve configuration by merging CLI, ENV, file, and defaults.
 */
export function resolveConfig(options?: {
  cli?: RawConfigInput | undefined;
  env?: Record<string, string | undefined> | undefined;
  file?: RawConfigInput | undefined;
}): WorkbenchConfig {
  const cli = options?.cli ?? {};
  const envSource = options?.env ?? process.env;
  const file = options?.file ?? {};

  // Environment variables mapping (documented fields only, REQ-CONFIG-001)
  const envGauntlet = envSource["GAUNTLET_WORKBENCH_ENDPOINT"] ?? envSource["GAUNTLET_ENDPOINT"];
  const envPrime = envSource["GAUNTLET_WORKBENCH_PRIME_ENDPOINT"] ?? envSource["PRIME_ENDPOINT"];
  const envPrimeBinary = envSource["GAUNTLET_WORKBENCH_PRIME_BINARY"];
  const envPrimeSocket = envSource["GAUNTLET_WORKBENCH_PRIME_SOCKET"];
  const envProjection = envSource["GAUNTLET_WORKBENCH_PROJECTION_STORE"];
  const envToken = envSource["GAUNTLET_SESSION_TOKEN"] ?? envSource["GAUNTLET_API_TOKEN"];
  const envTransport = envSource["GAUNTLET_WORKBENCH_TRANSPORT"];

  // 1. Gauntlet endpoint
  const gauntlet_endpoint =
    cli.gauntlet_endpoint ??
    envGauntlet ??
    file.gauntlet_endpoint ??
    DEFAULT_CONFIG.gauntlet_endpoint;

  // 2. Prime endpoint
  let prime_endpoint: string | null = null;
  if (cli.prime_endpoint !== undefined) {
    prime_endpoint = cli.prime_endpoint;
  } else if (envPrime !== undefined) {
    prime_endpoint = envPrime.trim() === "" ? null : envPrime;
  } else if (file.prime_endpoint !== undefined) {
    prime_endpoint = file.prime_endpoint;
  } else {
    prime_endpoint = DEFAULT_CONFIG.prime_endpoint;
  }

  // 2b. Prime binary (same-host RealPrimeHost path, plan Phase C2)
  let prime_binary: string | null = null;
  if (cli.prime_binary !== undefined) {
    prime_binary = cli.prime_binary;
  } else if (envPrimeBinary !== undefined) {
    prime_binary = envPrimeBinary.trim() === "" ? null : envPrimeBinary;
  } else if (file.prime_binary !== undefined) {
    prime_binary = file.prime_binary;
  } else {
    prime_binary = DEFAULT_CONFIG.prime_binary;
  }

  // 2c. Prime socket (same-host daemon path, REQ-BUN-005)
  let prime_socket: string | null = null;
  if (cli.prime_socket !== undefined) {
    prime_socket = cli.prime_socket;
  } else if (envPrimeSocket !== undefined) {
    prime_socket = envPrimeSocket.trim() === "" ? null : envPrimeSocket;
  } else if (file.prime_socket !== undefined) {
    prime_socket = file.prime_socket;
  } else {
    prime_socket = DEFAULT_CONFIG.prime_socket;
  }

  // 3. Projection store
  const projection_store =
    cli.projection_store ??
    envProjection ??
    file.projection_store ??
    DEFAULT_CONFIG.projection_store;

  // 4. Plugin roots
  const plugin_roots = cli.plugin_roots ?? file.plugin_roots ?? DEFAULT_CONFIG.plugin_roots;

  // 5. Performance budget
  const performance_budget: PerformanceBudget = {
    max_latency_ms:
      cli.performance_budget?.max_latency_ms ??
      file.performance_budget?.max_latency_ms ??
      DEFAULT_CONFIG.performance_budget.max_latency_ms,
    max_tokens:
      cli.performance_budget?.max_tokens ??
      file.performance_budget?.max_tokens ??
      DEFAULT_CONFIG.performance_budget.max_tokens,
  };

  // 6. Transport: explicit override or auto-classified from endpoint
  const transportRaw = cli.transport ?? envTransport ?? file.transport;
  const transport: TransportKind =
    transportRaw === "remote" || transportRaw === "local"
      ? transportRaw
      : classifyTransport(gauntlet_endpoint);

  // 7. Session token (REQ-CONFIG-002: referenced indirectly via env/cli, never in file)
  const session_token = cli.session_token ?? envToken ?? null;

  return {
    gauntlet_endpoint,
    prime_endpoint,
    prime_binary,
    prime_socket,
    projection_store,
    plugin_roots,
    performance_budget,
    transport,
    session_token,
  };
}

/**
 * Redact secrets and tokens for safe logging and error presentation (REQ-CONFIG-002).
 */
export function redactConfig(config: WorkbenchConfig): Record<string, unknown> {
  return {
    ...config,
    session_token: config.session_token ? "[REDACTED]" : null,
  };
}
