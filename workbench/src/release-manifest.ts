/**
 * T20 release form: pinned-target build + manifest + launch qualification
 * (plan T20 step 3, REQ-BUN-005, REQ-CONFIG-004; evidence
 * T20/release-qualification.md).
 *
 * `bun run src/release-manifest.ts` (wired as `just tui-build release`):
 *   1. Builds the workbench entry as a STANDALONE EXECUTABLE for the pinned
 *      target (bun-linux-x64) from the pinned lockfile — the form actually
 *      qualified on this host (Bun 1.4.0 supports `--compile`).
 *   2. LAUNCHES the artifact in a real PTY at 120x40, asserts the rendered
 *      workbench shell, and quits it (the launch evidence, not a size check).
 *   3. Writes `dist/release-manifest.json`: artifact path + sha256, bun
 *      version, entry, build target, lockfile digest, the dependency list
 *      from package.json, and the launch qualification record.
 *
 * The manifest is written by the build; `verifyReleaseManifest` re-checks a
 * manifest against the artifact on disk (digest + required fields) — a
 * manifest whose artifact moved or drifted fails.
 *
 * Target-specific limitations are recorded as configuration/support facts in
 * the manifest (`support_facts`), never as domain behavior.
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export const RELEASE_TARGET = "bun-linux-x64";
export const RELEASE_ENTRY = "src/main.ts";
export const RELEASE_OUTDIR = "dist";
export const RELEASE_ARTIFACT = `${RELEASE_OUTDIR}/gauntlet-workbench`;
export const RELEASE_MANIFEST = `${RELEASE_OUTDIR}/release-manifest.json`;

/** The workbench package root (this module lives in src/). */
const ROOT = dirname(import.meta.dir);

// --- Manifest document ---------------------------------------------------------

export interface ReleaseManifest {
  readonly artifact: string;
  readonly artifact_sha256: string;
  readonly artifact_bytes: number;
  readonly entry: string;
  readonly target: string;
  readonly bun_version: string;
  readonly lockfile: string;
  readonly lockfile_sha256: string;
  readonly dependencies: Record<string, string>;
  readonly dev_dependencies: Record<string, string>;
  readonly build_form: "bun-compile-standalone";
  readonly launch_qualification: {
    readonly transport: "real-pty";
    readonly geometry: "120x40";
    readonly rendered_markers: readonly string[];
    readonly exited_cleanly: boolean;
    readonly exit_code: number | null;
  };
  readonly support_facts: readonly string[];
  readonly built_at_utc: string;
}

/** The markers the launched artifact's first frames must contain. */
const LAUNCH_MARKERS = ["Gauntlet", "NAVIGATOR", "PRIMARY", "INSPECTOR", "COMPOSER"] as const;

function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function packageJson(): {
  dependencies: Record<string, string>;
  devDependencies: Record<string, string>;
} {
  const raw = JSON.parse(readFileSync("package.json", "utf8")) as Record<string, unknown>;
  return {
    dependencies: (raw["dependencies"] ?? {}) as Record<string, string>,
    devDependencies: (raw["devDependencies"] ?? {}) as Record<string, string>,
  };
}

function supportFacts(): string[] {
  const facts: string[] = [];
  facts.push(
    "Target: Linux WSL2 (x86_64); artifact built with Bun's standalone-compile form (--target=bun-linux-x64), the closest qualified form on this host.",
  );
  facts.push(
    "Terminal: the artifact renders the structural ShellSession tree into the host terminal; the embedded OpenTUI React viewport remains later-task wiring (see T13 evidence) and is not part of this artifact.",
  );
  if (process.env["WSL_DISTRO_NAME"] !== undefined) {
    facts.push(`WSL distribution: ${process.env["WSL_DISTRO_NAME"]} (same-host deployments only).`);
  }
  const tmux = Bun.spawnSync(["tmux", "-V"], { stdout: "pipe", stderr: "pipe" });
  if (tmux.exitCode === 0) {
    facts.push(`tmux available: ${new TextDecoder().decode(tmux.stdout).trim()}.`);
  } else {
    facts.push("tmux not runnable on this host.");
  }
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
  return facts;
}

// --- Build + launch ------------------------------------------------------------

async function buildArtifact(): Promise<void> {
  mkdirSync(RELEASE_OUTDIR, { recursive: true });
  const proc = Bun.spawn(
    [
      "bun",
      "build",
      "--compile",
      `--target=${RELEASE_TARGET}`,
      RELEASE_ENTRY,
      "--outfile",
      RELEASE_ARTIFACT,
    ],
    { stdout: "pipe", stderr: "pipe", cwd: ROOT },
  );
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  if (code !== 0 || !existsSync(join(ROOT, RELEASE_ARTIFACT))) {
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
}

/** Validate a manifest document's shape and its artifact digests. */
export function verifyReleaseManifest(manifest: unknown): ManifestCheckResult {
  const failures: string[] = [];
  if (typeof manifest !== "object" || manifest === null) {
    return { valid: false, failures: ["manifest is not an object"] };
  }
  const doc = manifest as Record<string, unknown>;
  for (const field of REQUIRED_MANIFEST_FIELDS) {
    if (doc[field] === undefined) {
      failures.push(`missing field: ${field}`);
    }
  }
  if (
    typeof doc["artifact_sha256"] !== "string" ||
    !/^[0-9a-f]{64}$/.test(doc["artifact_sha256"])
  ) {
    failures.push("artifact_sha256 is not a hex sha256 digest");
  }
  if (doc["build_form"] !== "bun-compile-standalone") {
    failures.push("build_form must name the qualified form");
  }
  const launch = doc["launch_qualification"] as Record<string, unknown> | undefined;
  if (launch === undefined || typeof launch !== "object") {
    failures.push("launch_qualification missing");
  } else {
    if (launch["transport"] !== "real-pty") {
      failures.push("launch_qualification.transport must be a real PTY launch");
    }
    if (launch["exited_cleanly"] !== true) {
      failures.push("launch_qualification must record a clean exit");
    }
    const markers = launch["rendered_markers"];
    if (!Array.isArray(markers) || markers.length === 0) {
      failures.push("launch_qualification.rendered_markers must be recorded");
    }
  }
  const facts = doc["support_facts"];
  if (!Array.isArray(facts) || facts.length === 0) {
    failures.push("support_facts must record target limitations as configuration facts");
  }
  if (failures.length === 0 && typeof doc["artifact"] === "string") {
    const artifactPath = join(ROOT, doc["artifact"] as string);
    if (!existsSync(artifactPath)) {
      failures.push(`artifact missing on disk: ${doc["artifact"]}`);
    } else {
      const actual = sha256File(artifactPath);
      if (actual !== doc["artifact_sha256"]) {
        failures.push(
          `artifact sha256 drift: manifest ${String(doc["artifact_sha256"])} vs disk ${actual}`,
        );
      }
    }
  }
  return { valid: failures.length === 0, failures };
}

// --- The build entry -----------------------------------------------------------

async function main(): Promise<number> {
  const bunVersion = Bun.version ?? "unknown";
  process.stderr.write(`tui-build release: bun ${bunVersion}, target ${RELEASE_TARGET}\n`);

  const lockfileSha = existsSync("bun.lock")
    ? sha256File("bun.lock")
    : "unavailable (no lockfile on disk)";
  const { dependencies, devDependencies } = packageJson();

  await buildArtifact();
  process.stderr.write(`tui-build release: built ${RELEASE_ARTIFACT}\n`);

  const artifactPath = join(ROOT, RELEASE_ARTIFACT);
  const launch = await launchAndQualify(artifactPath);
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

  const manifest: ReleaseManifest = {
    artifact: RELEASE_ARTIFACT,
    artifact_sha256: sha256File(artifactPath),
    artifact_bytes: readFileSync(artifactPath).byteLength,
    entry: RELEASE_ENTRY,
    target: RELEASE_TARGET,
    bun_version: bunVersion,
    lockfile: "bun.lock",
    lockfile_sha256: lockfileSha,
    dependencies,
    dev_dependencies: devDependencies,
    build_form: "bun-compile-standalone",
    launch_qualification: {
      transport: "real-pty",
      geometry: "120x40",
      rendered_markers: [...LAUNCH_MARKERS],
      exited_cleanly: launch.exitCode === 0,
      exit_code: launch.exitCode,
    },
    support_facts: supportFacts(),
    built_at_utc: new Date().toISOString(),
  };
  writeFileSync(join(ROOT, RELEASE_MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`);
  const check = verifyReleaseManifest(manifest);
  if (!check.valid) {
    process.stderr.write(`tui-build release: manifest invalid: ${check.failures.join("; ")}\n`);
    return 1;
  }
  process.stderr.write(
    `tui-build release: OK — ${RELEASE_ARTIFACT} sha256=${manifest.artifact_sha256.slice(0, 16)}… launch 120x40 exit 0, manifest ${RELEASE_MANIFEST}\n`,
  );
  return 0;
}

/** Re-verify a written manifest from disk (used by checks and the unit test). */
export function verifyManifestOnDisk(): ManifestCheckResult {
  const path = join(ROOT, RELEASE_MANIFEST);
  if (!existsSync(path)) {
    return { valid: false, failures: [`no manifest at ${RELEASE_MANIFEST}`] };
  }
  return verifyReleaseManifest(JSON.parse(readFileSync(path, "utf8")));
}

/** Remove build outputs (used by tests to avoid ordering dependencies). */
export function cleanReleaseOutputs(): void {
  rmSync(join(ROOT, RELEASE_ARTIFACT), { force: true });
  rmSync(join(ROOT, RELEASE_MANIFEST), { force: true });
}

if (import.meta.main) {
  process.exit(await main());
}
