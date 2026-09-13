/**
 * Workbench configuration authority, precedence resolution, and secret indirection.
 * Conforms to spec §20.2, REQ-CONFIG-001..003, REQ-CONFIG-005, and plan T02.
 *
 * Strict precedence:
 *   1. Explicit CLI arguments / options
 *   2. Environment variables (GAUNTLET_WORKBENCH_*)
 *   3. Configuration file (gauntlet-workbench.yaml / .json)
 *   4. Documented defaults (for `project_root`/`gauntlet_binary`, DISCOVERY:
 *      see `discoverProjectRoot`/`discoverGauntletBinary` — discovery answers
 *      or stays null, and never guesses a wrong-but-plausible path)
 */

import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

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
  /**
   * Real project root a composer-launched Prime child works in (plan
   * workbench-agent-interaction.md P1; T13-bound: `RealPrimeHost`'s spawn
   * path policy adds exactly this root to its allowed cwd roots, never
   * bypasses the boundary). `null` (unconfigured, the default) means
   * composer-launched children still run WITH tools enabled but their `cwd`
   * falls back to the isolated session directory — guessing a repo root from
   * the workbench process's own cwd is unreliable in general deployments, so
   * this stays an explicit opt-in rather than an inferred default.
   */
  readonly project_root: string | null;
  /**
   * Path to the operator-qualified `gauntlet` CLI binary (plan
   * workbench-agent-interaction.md increment R; ICM §13.1 `run.start`).
   * `null` (unconfigured, the default) means `run.start` is
   * unavailable-with-reason — never guessed, and never inferred from
   * `prime_binary` or the workbench process's own argv0 (D6-style
   * additivity: absence never degrades any other surface).
   */
  readonly gauntlet_binary: string | null;
  readonly projection_store: string;
  readonly plugin_roots: readonly string[];
  readonly performance_budget: PerformanceBudget;
  readonly transport: TransportKind;
  readonly session_token: string | null;
  /**
   * Motion preference mirrored from `GAUNTLET_WORKBENCH_MOTION` (DSN-013, M15
   * fix: the reduced-motion setting is operator-reachable). `undefined` when
   * unset or invalid — the default lives in the theme layer's
   * `resolveMotionPreference`, not here. The union is inlined deliberately:
   * config is state-layer and must not bind presentation-module types.
   */
  readonly motion?: "full" | "reduced" | undefined;
  /**
   * Colour-capability override mirrored from `GAUNTLET_WORKBENCH_COLOR`
   * (DSN-002, V6 fix). `undefined` when unset or invalid; detection and the
   * never-assume-truecolor default live in the theme layer's
   * `detectColorCapability`.
   */
  readonly color?: "truecolor" | "color256" | "monochrome" | undefined;
  /**
   * RealityTrace (SXR) binding, resolved from `GAUNTLET_SXR_BINARY` /
   * `GAUNTLET_SXR_REPO_ID` (GUI-redesign increment 6). `undefined` when either
   * is unset or empty — `SxrLensProjection`'s status vocabulary has no
   * "unbound" case, so an unbound config keeps today's behavior unchanged.
   */
  readonly sxrBinding?: SxrBindingConfig | undefined;
}

/**
 * Optional RealityTrace (SXR) binding: the on-demand refresh path (GUI-redesign
 * increment 6) is wired only when both fields resolve to non-empty strings —
 * an unbound config is today's behavior, unchanged (D6-style additivity).
 */
export interface SxrBindingConfig {
  readonly binaryPath: string;
  readonly repoId: string;
}

export interface RawConfigInput {
  gauntlet_endpoint?: string | undefined;
  prime_endpoint?: string | null | undefined;
  prime_binary?: string | null | undefined;
  prime_socket?: string | null | undefined;
  project_root?: string | null | undefined;
  gauntlet_binary?: string | null | undefined;
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
  project_root: null,
  gauntlet_binary: null,
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
 * The Gauntlet project containing `startDir`, or null.
 *
 * A "project root" is ONE Gauntlet project — the directory holding
 * `GAUNTLET.md` (the front-matter config naming the reference manifest and
 * answer key), which is what `gauntlet run` must execute in and what a
 * work-mode Prime child is bound to as its cwd. A directory that merely
 * CONTAINS projects (`~/projects`) is deliberately never a valid answer:
 * `gauntlet run` there refuses for want of a reference, and binding a child's
 * write scope to a tree of unrelated repositories is strictly worse than
 * refusing. Walking up from the launch directory is how the operator's
 * "wherever I started the TUI" becomes the project without configuration;
 * finding nothing stays null, which every consumer renders as
 * unavailable-with-reason rather than guessing.
 */
export function discoverProjectRoot(
  startDir: string,
  exists: (path: string) => boolean = existsSync,
): string | null {
  let current = resolve(startDir);
  // Bounded by the filesystem root: `dirname("/")` is "/" itself.
  for (;;) {
    if (exists(join(current, "GAUNTLET.md"))) return current;
    const parent = dirname(current);
    if (parent === current) return null;
    current = parent;
  }
}

/**
 * The `gauntlet` binary to bind `run.start` to, or null.
 *
 * PATH first (an installed Gauntlet is the operator's real one), then this
 * checkout's own debug build so a developer running the workbench from the
 * repository gets a working `run.start` without configuration. Never
 * `prime-agent`: that is the AGENT binary (`prime_binary`), and `run.start`
 * executes `<binary> run "<intent>"` — Gauntlet's own CLI, not the agent's.
 */
export function discoverGauntletBinary(
  projectRoot: string | null,
  lookPath: (name: string) => string | null = (name) => Bun.which(name),
  exists: (path: string) => boolean = existsSync,
  /** Where the running workbench's own source lives; its checkout is the last resort. */
  moduleDir: string = import.meta.dir,
): string | null {
  const onPath = lookPath("gauntlet");
  if (onPath !== null) return onPath;
  // The target project's own build, when it happens to BE the Gauntlet
  // checkout; then the checkout this workbench is running from — a demo or
  // any other target project has no `gauntlet` build of its own, but the
  // developer running the TUI out of the repository still has one.
  for (const base of [projectRoot, ...ancestors(moduleDir)]) {
    if (base === null) continue;
    const debugBuild = join(base, "target", "debug", "gauntlet");
    if (exists(debugBuild)) return debugBuild;
  }
  return null;
}

/** `dir` and every ancestor, nearest first. */
function ancestors(dir: string): readonly string[] {
  const chain: string[] = [];
  let current = resolve(dir);
  for (;;) {
    chain.push(current);
    const parent = dirname(current);
    if (parent === current) return chain;
    current = parent;
  }
}

/**
 * Deterministically resolve configuration by merging CLI, ENV, file, and defaults.
 */
export function resolveConfig(options?: {
  cli?: RawConfigInput | undefined;
  env?: Record<string, string | undefined> | undefined;
  file?: RawConfigInput | undefined;
  /**
   * Launch directory the project-root/binary discovery defaults walk up from
   * (tests inject; production passes the real cwd).
   */
  cwd?: string | undefined;
}): WorkbenchConfig {
  const cli = options?.cli ?? {};
  const envSource = options?.env ?? process.env;
  const file = options?.file ?? {};

  // Environment variables mapping (documented fields only, REQ-CONFIG-001)
  const envGauntlet = envSource["GAUNTLET_WORKBENCH_ENDPOINT"] ?? envSource["GAUNTLET_ENDPOINT"];
  const envPrime = envSource["GAUNTLET_WORKBENCH_PRIME_ENDPOINT"] ?? envSource["PRIME_ENDPOINT"];
  const envPrimeBinary = envSource["GAUNTLET_WORKBENCH_PRIME_BINARY"];
  const envPrimeSocket = envSource["GAUNTLET_WORKBENCH_PRIME_SOCKET"];
  const envProjectRoot = envSource["GAUNTLET_WORKBENCH_PROJECT_ROOT"];
  const envGauntletBinary = envSource["GAUNTLET_WORKBENCH_GAUNTLET_BINARY"];
  const envProjection = envSource["GAUNTLET_WORKBENCH_PROJECTION_STORE"];
  const envToken = envSource["GAUNTLET_SESSION_TOKEN"] ?? envSource["GAUNTLET_API_TOKEN"];
  const envTransport = envSource["GAUNTLET_WORKBENCH_TRANSPORT"];
  const envMotion = envSource["GAUNTLET_WORKBENCH_MOTION"];
  const envColor = envSource["GAUNTLET_WORKBENCH_COLOR"];
  const envSxrBinary = envSource["GAUNTLET_SXR_BINARY"];
  const envSxrRepoId = envSource["GAUNTLET_SXR_REPO_ID"];

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

  // 2d. Project root (composer-launched work-mode cwd, plan P1)
  let project_root: string | null = null;
  if (cli.project_root !== undefined) {
    project_root = cli.project_root;
  } else if (envProjectRoot !== undefined) {
    project_root = envProjectRoot.trim() === "" ? null : envProjectRoot;
  } else if (file.project_root !== undefined) {
    project_root = file.project_root;
  } else {
    // Discovery, not a literal default: the project containing the launch
    // directory. Nothing found stays null (unavailable-with-reason).
    project_root = discoverProjectRoot(options?.cwd ?? process.cwd());
  }

  // 2e. Gauntlet CLI binary (plan increment R, ICM §13.1 run.start)
  let gauntlet_binary: string | null = null;
  if (cli.gauntlet_binary !== undefined) {
    gauntlet_binary = cli.gauntlet_binary;
  } else if (envGauntletBinary !== undefined) {
    gauntlet_binary = envGauntletBinary.trim() === "" ? null : envGauntletBinary;
  } else if (file.gauntlet_binary !== undefined) {
    gauntlet_binary = file.gauntlet_binary;
  } else {
    // Discovery: `gauntlet` on PATH, else this checkout's debug build.
    gauntlet_binary = discoverGauntletBinary(project_root);
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

  // 8. Motion preference (DSN-013) and colour capability override (DSN-002):
  //    env-mirrored only; invalid values stay undefined rather than guessed.
  const motion = envMotion === "reduced" || envMotion === "full" ? envMotion : undefined;
  const color =
    envColor === "truecolor" || envColor === "color256" || envColor === "monochrome"
      ? envColor
      : undefined;

  // 9. RealityTrace (SXR) binding: both env vars must resolve non-empty.
  const sxrBinding: SxrBindingConfig | undefined =
    envSxrBinary !== undefined &&
    envSxrBinary.trim() !== "" &&
    envSxrRepoId !== undefined &&
    envSxrRepoId.trim() !== ""
      ? { binaryPath: envSxrBinary, repoId: envSxrRepoId }
      : undefined;

  return {
    gauntlet_endpoint,
    prime_endpoint,
    prime_binary,
    prime_socket,
    project_root,
    gauntlet_binary,
    projection_store,
    plugin_roots,
    performance_budget,
    transport,
    session_token,
    motion,
    color,
    sxrBinding,
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
