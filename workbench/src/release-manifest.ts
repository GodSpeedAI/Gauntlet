/**
 * T20 release form (multi-target): host-derived target build + manifest +
 * launch qualification (plan T20 step 3, REQ-BUN-005, REQ-CONFIG-004; evidence
 * T20/release-qualification.md; native-distribution Slice W of
 * .agents/plans/native-distribution.md).
 *
 * `bun run src/release-manifest.ts` (wired as `just tui-build release`):
 *   1. Derives the Bun standalone-compile target from the HOST triple
 *      (process.platform/process.arch), overridable via
 *      GAUNTLET_WORKBENCH_RELEASE_TARGET for cross builds, and builds the
 *      workbench entry as a STANDALONE EXECUTABLE for that target from the
 *      pinned lockfile.
 *   2. For a host-executable target, LAUNCHES the artifact in a real PTY at
 *      120x40, asserts the rendered workbench shell, and quits it (the launch
 *      evidence, not a size check). For a cross (non-host-executable) target,
 *      the manifest records an HONEST not-qualifiable launch record — a cross
 *      artifact is never presented as launch-qualified.
 *   3. Writes `dist/release-manifest.json`: the target-qualified artifact path
 *      + sha256, bun version, entry, build target, lockfile digest, the
 *      dependency list from package.json, and the launch qualification record.
 *
 * The manifest is written by the build; `verifyReleaseManifest` re-checks a
 * manifest against the artifact on disk (digest + required fields) — a
 * manifest whose artifact moved or drifted fails. Verification accepts both
 * launch-record shapes and exposes `launch_qualified` so no consumer can
 * mistake an unqualified artifact for a qualified one.
 *
 * Target-specific limitations are recorded as configuration/support facts in
 * the manifest (`support_facts`), never as domain behavior.
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export const RELEASE_ENTRY = "src/main.ts";
export const RELEASE_OUTDIR = "dist";
export const RELEASE_MANIFEST = `${RELEASE_OUTDIR}/release-manifest.json`;

/**
 * Bun standalone-compile targets derivable from a host triple (the plan's
 * five supported platforms; see .agents/plans/native-distribution.md).
 */
export const HOST_DERIVED_TARGETS = [
  "bun-linux-x64",
  "bun-linux-arm64",
  "bun-darwin-arm64",
  "bun-darwin-x64",
  "bun-windows-x64",
] as const;

/**
 * musl Linux variants: identical derivation rules, but never auto-derived —
 * musl hosts are not reliably detectable from this builder, so a musl
 * artifact is only ever built via an explicit GAUNTLET_WORKBENCH_RELEASE_TARGET
 * override (and records its glibc/musl assumption in support_facts).
 */
export const OVERRIDE_ONLY_TARGETS = ["bun-linux-x64-musl", "bun-linux-arm64-musl"] as const;

export const KNOWN_RELEASE_TARGETS = [...HOST_DERIVED_TARGETS, ...OVERRIDE_ONLY_TARGETS] as const;

export type ReleaseTarget = (typeof KNOWN_RELEASE_TARGETS)[number];

/** The workbench package root (this module lives in src/). */
const ROOT = dirname(import.meta.dir);

// --- Target derivation ---------------------------------------------------------

/**
 * Map a host triple to its Bun standalone-compile target. Fails closed for
 * any triple this release form does not qualify — no closest-guess targets.
 */
export function deriveReleaseTarget(platform: string, arch: string): ReleaseTarget {
  switch (platform) {
    case "linux":
      if (arch === "x64") return "bun-linux-x64";
      if (arch === "arm64") return "bun-linux-arm64";
      break;
    case "darwin":
      if (arch === "arm64") return "bun-darwin-arm64";
      if (arch === "x64") return "bun-darwin-x64";
      break;
    case "win32":
      if (arch === "x64") return "bun-windows-x64";
      break;
  }
  throw new Error(
    `no qualified Bun standalone-compile target for host ${platform}/${arch} (supported host triples: ${HOST_DERIVED_TARGETS.join(", ")})`,
  );
}

export interface ResolvedReleaseTarget {
  readonly target: ReleaseTarget;
  readonly host_target: ReleaseTarget;
  /** True when the artifact's target cannot be executed on the building host. */
  readonly cross_build: boolean;
  /** Where the target came from: the host triple or the env override. */
  readonly source: "host-derived" | "env-override";
}

/**
 * Resolve the release target for this build: the host-derived target, or the
 * GAUNTLET_WORKBENCH_RELEASE_TARGET override (validated against the known
 * target list; unknown targets fail closed). A cross build never qualifies a
 * launch on the building host — the manifest records that honestly.
 */
export function resolveReleaseTarget(
  override: string | undefined,
  platform: string,
  arch: string,
): ResolvedReleaseTarget {
  const hostTarget = deriveReleaseTarget(platform, arch);
  if (override === undefined || override === "") {
    return {
      target: hostTarget,
      host_target: hostTarget,
      cross_build: false,
      source: "host-derived",
    };
  }
  if (!(KNOWN_RELEASE_TARGETS as readonly string[]).includes(override)) {
    throw new Error(
      `GAUNTLET_WORKBENCH_RELEASE_TARGET '${override}' is not a known Bun standalone-compile target (known targets: ${KNOWN_RELEASE_TARGETS.join(", ")})`,
    );
  }
  const target = override as ReleaseTarget;
  return {
    target,
    host_target: hostTarget,
    cross_build: target !== hostTarget,
    source: "env-override",
  };
}

/**
 * The target-qualified artifact path. Bun appends `.exe` to Windows
 * standalone-compile outputs even when --outfile omits it (verified against
 * the pinned Bun 1.4.0 toolchain), so the recorded path matches disk exactly.
 */
export function releaseArtifactPath(target: string): string {
  const base = `${RELEASE_OUTDIR}/gauntlet-workbench-${target}`;
  return target.startsWith("bun-windows") ? `${base}.exe` : base;
}

// --- Manifest document ---------------------------------------------------------

/** The launch record for a host-executable (qualified) artifact. */
export interface RealPtyLaunchQualification {
  readonly transport: "real-pty";
  readonly geometry: "120x40";
  readonly rendered_markers: readonly string[];
  readonly exited_cleanly: boolean;
  readonly exit_code: number | null;
}

/**
 * The honest launch record for a cross artifact: it was built and hashed, but
 * no launch qualification was (or could be) performed on the building host.
 */
export interface NotQualifiableLaunchRecord {
  readonly transport: "not-qualifiable-on-this-host";
  readonly reason: string;
}

export type LaunchQualification = RealPtyLaunchQualification | NotQualifiableLaunchRecord;

export interface ReleaseManifest {
  readonly artifact: string;
  readonly artifact_sha256: string;
  readonly artifact_bytes: number;
  readonly entry: string;
  readonly target: ReleaseTarget;
  readonly bun_version: string;
  readonly lockfile: string;
  readonly lockfile_sha256: string;
  readonly dependencies: Record<string, string>;
  readonly dev_dependencies: Record<string, string>;
  readonly build_form: "bun-compile-standalone";
  readonly launch_qualification: LaunchQualification;
  readonly support_facts: readonly string[];
  readonly built_at_utc: string;
}

/** The markers the launched artifact's first frames must contain. */
const LAUNCH_MARKERS = ["Gauntlet", "NAVIGATOR", "PRIMARY", "INSPECTOR", "COMPOSER"] as const;

function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** The parsed, typed view of the untrusted package.json document. */
interface PackageJsonDocument {
  readonly dependencies?: Record<string, string>;
  readonly devDependencies?: Record<string, string>;
}

function packageJson(): {
  dependencies: Record<string, string>;
  devDependencies: Record<string, string>;
} {
  const raw = JSON.parse(readFileSync("package.json", "utf8")) as PackageJsonDocument;
  return {
    dependencies: raw.dependencies ?? {},
    devDependencies: raw.devDependencies ?? {},
  };
}

function supportFacts(resolved: ResolvedReleaseTarget): string[] {
  const facts: string[] = [];
  facts.push(
    resolved.source === "env-override"
      ? `Compile target: ${resolved.target} (GAUNTLET_WORKBENCH_RELEASE_TARGET override; host ${process.platform}/${process.arch} derives ${resolved.host_target}).`
      : `Compile target: ${resolved.target} (derived from host ${process.platform}/${process.arch}).`,
  );
  if (resolved.target.startsWith("bun-linux")) {
    facts.push(
      "Linux artifact assumes glibc linkage: the derived bun-linux targets carry no musl suffix and musl hosts are not auto-detected by this builder — a musl deployment must rebuild with the explicit bun-linux-<arch>-musl override target.",
    );
  }
  facts.push(
    "Terminal: the artifact renders the structural ShellSession tree into the host terminal; the embedded OpenTUI React viewport remains later-task wiring (see T13 evidence) and is not part of this artifact.",
  );
  if (resolved.cross_build) {
    facts.push(
      `Cross build: artifact compiled on a ${resolved.host_target} host and NOT launch-qualified here (launch_qualification.transport = not-qualifiable-on-this-host); real-PTY qualification belongs on a native ${resolved.target} runner.`,
    );
    return facts;
  }
  const { WSL_DISTRO_NAME } = process.env;
  if (WSL_DISTRO_NAME !== undefined) {
    facts.push(`WSL distribution: ${WSL_DISTRO_NAME} (same-host deployments only).`);
  }
  // Tool absence must never crash the builder (Bun.spawnSync THROWS
  // "Executable not found in $PATH" for a missing binary on e.g. macOS
  // runners): resolve with Bun.which first, then still guard the probe itself.
  const tmuxPath = Bun.which("tmux");
  if (tmuxPath === null) {
    facts.push("tmux not found in $PATH on this host (tmux transport legs not runnable here).");
  } else {
    try {
      const tmux = Bun.spawnSync(["tmux", "-V"], { stdout: "pipe", stderr: "pipe" });
      facts.push(
        tmux.exitCode === 0
          ? `tmux available: ${new TextDecoder().decode(tmux.stdout).trim()}.`
          : "tmux present but not runnable on this host.",
      );
    } catch {
      facts.push("tmux probe failed on this host (tmux transport legs recorded as unavailable).");
    }
  }
  const sshPath = Bun.which("ssh");
  if (sshPath === null) {
    facts.push(
      "ssh not found in $PATH on this host (SSH remote-mode terminal recorded as unavailable; see evidence).",
    );
  } else {
    try {
      const ssh = Bun.spawnSync(
        [
          "ssh",
          "-o",
          "BatchMode=yes",
          "-o",
          "ConnectTimeout=3",
          "-o",
          "StrictHostKeyChecking=no",
          "localhost",
          "echo ok",
        ],
        { stdout: "pipe", stderr: "pipe", timeout: 10_000 },
      );
      facts.push(
        ssh.exitCode === 0
          ? "sshd reachable on localhost (SSH remote-mode terminal usable)."
          : "sshd NOT reachable on localhost (SSH remote-mode terminal recorded as unavailable; see evidence).",
      );
    } catch {
      facts.push(
        "ssh probe failed on this host (SSH remote-mode terminal recorded as unavailable; see evidence).",
      );
    }
  }
  return facts;
}

// --- Build + launch ------------------------------------------------------------

async function buildArtifact(target: ReleaseTarget, artifactRel: string): Promise<void> {
  mkdirSync(RELEASE_OUTDIR, { recursive: true });
  // Bun appends .exe to windows compile outputs itself (see releaseArtifactPath);
  // pass the outfile without the suffix so the on-disk name is deterministic.
  const outfile = artifactRel.endsWith(".exe") ? artifactRel.slice(0, -4) : artifactRel;
  // Tool absence must never crash the builder (the F2 class): "bun" is a
  // PATH-resolved external binary like any other probe — resolve it first, then
  // still guard the spawn itself, failing through the typed error channel.
  if (Bun.which("bun") === null) {
    throw new Error(
      "release build failed: the 'bun' executable is not in $PATH on this host (standalone compilation needs the Bun toolchain this builder runs under).",
    );
  }
  let stderr = "";
  let code: number | null = null;
  try {
    const proc = Bun.spawn(
      ["bun", "build", "--compile", `--target=${target}`, RELEASE_ENTRY, "--outfile", outfile],
      { stdout: "pipe", stderr: "pipe", cwd: ROOT },
    );
    stderr = await new Response(proc.stderr).text();
    code = await proc.exited;
  } catch (error) {
    throw new Error(
      `release build failed to start: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (code !== 0 || !existsSync(join(ROOT, artifactRel))) {
    throw new Error(`release build failed (exit ${String(code)}): ${stderr.trim().slice(0, 2000)}`);
  }
}

interface LaunchResult {
  output: string;
  exitCode: number | null;
}

/** Launch the artifact in a REAL PTY at 120x40 and quit it. */
async function launchAndQualify(artifactPath: string): Promise<LaunchResult> {
  let output = "";
  const proc = Bun.spawn([artifactPath], {
    cwd: ROOT,
    env: { ...process.env, TERM: "xterm-256color" },
    terminal: {
      cols: 120,
      rows: 40,
      name: "xterm-256color",
      data: (_terminal, data) => {
        output += new TextDecoder().decode(data);
      },
    },
  });
  const terminal = proc.terminal;
  if (terminal === undefined) {
    proc.kill();
    throw new Error("artifact launch: Bun.spawn returned no terminal");
  }
  // Wait for the shell to render, then quit through the keyboard (the real
  // operator exit path — the artifact is never killed while qualifying).
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline && !output.includes("COMPOSER")) {
    await Bun.sleep(100);
  }
  terminal.write("q");
  const exitCode = await Promise.race([
    proc.exited,
    Bun.sleep(10_000).then(() => {
      proc.kill();
      return null;
    }),
  ]);
  return { output, exitCode: typeof exitCode === "number" ? exitCode : null };
}

/** Strip ANSI escape sequences (CSI) without a control-character regex. */
export function stripAnsiSequences(text: string): string {
  let result = "";
  for (let index = 0; index < text.length; ) {
    const char = text[index];
    if (char === "\x1b" && text[index + 1] === "[") {
      index += 2;
      while (index < text.length && !/[a-zA-Z]/.test(text[index] ?? "")) {
        index += 1;
      }
      index += 1; // consume the final letter
    } else {
      result += char;
      index += 1;
    }
  }
  return result;
}

function lastFrameOf(output: string): string {
  const lastClear = output.lastIndexOf("\x1b[2J");
  const frame = lastClear >= 0 ? output.slice(lastClear) : output;
  return stripAnsiSequences(frame);
}

// --- Manifest verification (ts-level check) ------------------------------------

const REQUIRED_MANIFEST_FIELDS = [
  "artifact",
  "artifact_sha256",
  "artifact_bytes",
  "entry",
  "target",
  "bun_version",
  "lockfile",
  "lockfile_sha256",
  "dependencies",
  "dev_dependencies",
  "build_form",
  "launch_qualification",
  "support_facts",
  "built_at_utc",
] as const;

export interface ManifestCheckResult {
  readonly valid: boolean;
  readonly failures: readonly string[];
  /**
   * True only when the manifest records a REAL 120x40 PTY launch with a clean
   * exit and rendered markers. An honest not-qualifiable record verifies as a
   * manifest but is never launch-qualified — consumers must not present a
   * merely-valid manifest as a qualified artifact.
   */
  readonly launch_qualified: boolean;
}

/**
 * Loosely-typed view of an UNTRUSTED manifest document (model/tool output is
 * parsed, schema-validated, then admitted): every contract field is declared,
 * but typed `unknown` until the runtime checks below admit its value.
 */
type ManifestRecord = { readonly [K in keyof ReleaseManifest]?: unknown };

/** Loosely-typed view of an untrusted launch_qualification record. */
type LaunchRecord = {
  readonly [K in keyof RealPtyLaunchQualification | keyof NotQualifiableLaunchRecord]?: unknown;
};

/** Validate a manifest document's shape and its artifact digests. */
export function verifyReleaseManifest(manifest: unknown): ManifestCheckResult {
  const failures: string[] = [];
  let launchQualified = false;
  if (typeof manifest !== "object" || manifest === null) {
    return { valid: false, failures: ["manifest is not an object"], launch_qualified: false };
  }
  const doc = manifest as ManifestRecord;
  for (const field of REQUIRED_MANIFEST_FIELDS) {
    if (doc[field] === undefined) {
      failures.push(`missing field: ${field}`);
    }
  }
  if (typeof doc.artifact_sha256 !== "string" || !/^[0-9a-f]{64}$/.test(doc.artifact_sha256)) {
    failures.push("artifact_sha256 is not a hex sha256 digest");
  }
  if (doc.build_form !== "bun-compile-standalone") {
    failures.push("build_form must name the qualified form");
  }
  const target = doc.target;
  if (
    typeof target !== "string" ||
    !(KNOWN_RELEASE_TARGETS as readonly string[]).includes(target)
  ) {
    failures.push("target is not a known Bun standalone-compile target");
  }
  if (
    typeof target === "string" &&
    (KNOWN_RELEASE_TARGETS as readonly string[]).includes(target) &&
    typeof doc.artifact === "string" &&
    doc.artifact !== releaseArtifactPath(target)
  ) {
    failures.push(
      `artifact path does not match the recorded target (expected ${releaseArtifactPath(target)})`,
    );
  }
  const launch = doc.launch_qualification as LaunchRecord | undefined;
  if (launch === undefined || launch === null || typeof launch !== "object") {
    failures.push("launch_qualification missing");
  } else if (launch.transport === "not-qualifiable-on-this-host") {
    // Honest cross-build record: shape-checked, never marked qualified.
    if (typeof launch.reason !== "string" || launch.reason.length === 0) {
      failures.push("launch_qualification.reason must state why the artifact is not qualified");
    }
  } else if (launch.transport === "real-pty") {
    if (launch.geometry !== "120x40") {
      failures.push("launch_qualification.geometry must record the qualified 120x40 PTY geometry");
    }
    if (launch.exited_cleanly !== true) {
      failures.push("launch_qualification must record a clean exit");
    }
    const markers = launch.rendered_markers;
    if (!Array.isArray(markers) || markers.length === 0) {
      failures.push("launch_qualification.rendered_markers must be recorded");
    }
    // Qualification requires the ACTUAL rendered launch markers (all five), not
    // merely any non-empty marker list — a forged marker list never qualifies.
    const markersComplete =
      Array.isArray(markers) && LAUNCH_MARKERS.every((marker) => markers.includes(marker));
    if (Array.isArray(markers) && markers.length > 0 && !markersComplete) {
      failures.push(
        `launch_qualification.rendered_markers must include the qualified launch markers (${LAUNCH_MARKERS.join(", ")})`,
      );
    }
    launchQualified =
      launch.geometry === "120x40" && launch.exited_cleanly === true && markersComplete;
  } else {
    failures.push(
      "launch_qualification.transport must be a real-pty launch or an honest not-qualifiable record",
    );
  }
  const facts = doc.support_facts;
  if (!Array.isArray(facts) || facts.length === 0) {
    failures.push("support_facts must record target limitations as configuration facts");
  }
  if (failures.length === 0 && typeof doc.artifact === "string") {
    const artifactPath = join(ROOT, doc.artifact as string);
    if (!existsSync(artifactPath)) {
      failures.push(`artifact missing on disk: ${doc.artifact}`);
    } else {
      const actual = sha256File(artifactPath);
      if (actual !== doc.artifact_sha256) {
        failures.push(
          `artifact sha256 drift: manifest ${String(doc.artifact_sha256)} vs disk ${actual}`,
        );
      }
    }
  }
  return { valid: failures.length === 0, failures, launch_qualified: launchQualified };
}

// --- The build entry -----------------------------------------------------------

async function main(): Promise<number> {
  const bunVersion = Bun.version ?? "unknown";
  const { GAUNTLET_WORKBENCH_RELEASE_TARGET: releaseTargetOverride } = process.env;
  let resolved: ResolvedReleaseTarget;
  try {
    resolved = resolveReleaseTarget(releaseTargetOverride, process.platform, process.arch);
  } catch (error) {
    process.stderr.write(
      `tui-build release: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return 1;
  }
  const artifactRel = releaseArtifactPath(resolved.target);
  process.stderr.write(
    `tui-build release: bun ${bunVersion}, target ${resolved.target} (${resolved.source}${resolved.cross_build ? ", cross build — no PTY qualification on this host" : ""})\n`,
  );

  const lockfileSha = existsSync("bun.lock")
    ? sha256File("bun.lock")
    : "unavailable (no lockfile on disk)";
  const { dependencies, devDependencies } = packageJson();

  // Build/launch failures (including spawn-time throws for an absent or
  // unspawnable binary) fail through the same typed channel as target
  // resolution — the builder never dies on an unhandled throw.
  try {
    await buildArtifact(resolved.target, artifactRel);
  } catch (error) {
    process.stderr.write(
      `tui-build release: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return 1;
  }
  process.stderr.write(`tui-build release: built ${artifactRel}\n`);
  // The pre-multi-target artifact name is superseded; drop the stale file so it
  // can never be mistaken for the qualified artifact (dist/ is gitignored).
  const legacyArtifact = join(ROOT, `${RELEASE_OUTDIR}/gauntlet-workbench`);
  if (existsSync(legacyArtifact)) {
    rmSync(legacyArtifact, { force: true });
  }

  const artifactPath = join(ROOT, artifactRel);

  let launchQualification: LaunchQualification;
  if (resolved.cross_build) {
    launchQualification = {
      transport: "not-qualifiable-on-this-host",
      reason: `artifact compiled for ${resolved.target} on a ${resolved.host_target} host (${resolved.source}); a cross artifact cannot be executed here, so no PTY launch was attempted — qualify on a native ${resolved.target} runner before recording a real-pty launch`,
    };
  } else {
    let launch: LaunchResult;
    try {
      launch = await launchAndQualify(artifactPath);
    } catch (error) {
      process.stderr.write(
        `tui-build release: artifact launch failed: ${error instanceof Error ? error.message : String(error)}\n`,
      );
      return 1;
    }
    const frame = lastFrameOf(launch.output);
    const missingMarkers = LAUNCH_MARKERS.filter((marker) => !frame.includes(marker));
    if (missingMarkers.length > 0) {
      process.stderr.write(
        `tui-build release: LAUNCH RENDER INCOMPLETE — missing ${missingMarkers.join(", ")}\n${frame.slice(0, 1500)}\n`,
      );
      return 1;
    }
    if (launch.exitCode !== 0) {
      process.stderr.write(
        `tui-build release: artifact did not exit cleanly (${String(launch.exitCode)})\n`,
      );
      return 1;
    }
    // Persist the launch frame as evidence (read by release-qualification.md).
    const framePath = join(
      ROOT,
      "..",
      ".agents",
      "reports",
      "gauntlet-tui-prime-workbench",
      "evidence",
      "T20",
      "frames",
      "release-artifact-120x40.txt",
    );
    mkdirSync(dirname(framePath), { recursive: true });
    writeFileSync(framePath, frame);
    launchQualification = {
      transport: "real-pty",
      geometry: "120x40",
      rendered_markers: [...LAUNCH_MARKERS],
      exited_cleanly: launch.exitCode === 0,
      exit_code: launch.exitCode,
    };
  }

  const manifest: ReleaseManifest = {
    artifact: artifactRel,
    artifact_sha256: sha256File(artifactPath),
    artifact_bytes: readFileSync(artifactPath).byteLength,
    entry: RELEASE_ENTRY,
    target: resolved.target,
    bun_version: bunVersion,
    lockfile: "bun.lock",
    lockfile_sha256: lockfileSha,
    dependencies,
    dev_dependencies: devDependencies,
    build_form: "bun-compile-standalone",
    launch_qualification: launchQualification,
    support_facts: supportFacts(resolved),
    built_at_utc: new Date().toISOString(),
  };
  writeFileSync(join(ROOT, RELEASE_MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`);
  const check = verifyReleaseManifest(manifest);
  if (!check.valid) {
    process.stderr.write(`tui-build release: manifest invalid: ${check.failures.join("; ")}\n`);
    return 1;
  }
  if (resolved.cross_build) {
    process.stderr.write(
      `tui-build release: OK — ${artifactRel} sha256=${manifest.artifact_sha256.slice(0, 16)}… CROSS BUILD, NOT launch-qualified (launch_qualification.transport = ${launchQualification.transport}), manifest ${RELEASE_MANIFEST}\n`,
    );
  } else {
    if (!check.launch_qualified) {
      process.stderr.write(
        `tui-build release: manifest did not record a qualified launch: ${check.failures.join("; ")}\n`,
      );
      return 1;
    }
    process.stderr.write(
      `tui-build release: OK — ${artifactRel} sha256=${manifest.artifact_sha256.slice(0, 16)}… launch 120x40 exit 0, manifest ${RELEASE_MANIFEST}\n`,
    );
  }
  return 0;
}

/** Re-verify a written manifest from disk (used by checks and the unit test). */
export function verifyManifestOnDisk(): ManifestCheckResult {
  const path = join(ROOT, RELEASE_MANIFEST);
  if (!existsSync(path)) {
    return {
      valid: false,
      failures: [`no manifest at ${RELEASE_MANIFEST}`],
      launch_qualified: false,
    };
  }
  return verifyReleaseManifest(JSON.parse(readFileSync(path, "utf8")));
}

/** Remove build outputs (used by tests to avoid ordering dependencies). */
export function cleanReleaseOutputs(): void {
  rmSync(join(ROOT, RELEASE_MANIFEST), { force: true });
  for (const target of KNOWN_RELEASE_TARGETS) {
    rmSync(join(ROOT, releaseArtifactPath(target)), { force: true });
  }
  rmSync(join(ROOT, `${RELEASE_OUTDIR}/gauntlet-workbench`), { force: true });
}

if (import.meta.main) {
  process.exit(await main());
}
