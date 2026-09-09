/**
 * Terminal-path benchmark (plan T13 gate: `just tui-bench terminal-path`).
 *
 * Measures the embedded-terminal output path — real buffers through the
 * DSN-014 sanitizer and the session projection — with FROZEN fixture sizes
 * and deterministic content (seeded LCG, no Math.random):
 *
 *   1. sanitizer one-shot throughput (strip and caret), 8 MiB hostile mix;
 *   2. chunked session ingest → sanitize → scrollback projection throughput
 *      (2000 × 4 KiB PTY-shaped chunks, sequences split across boundaries);
 *   3. projection rebuild latency over a full 1000-line scrollback;
 *   4. resize-churn latency (100 resize → project cycles);
 *   5. real child-process round trip (spawn → output ingest → exit) as an
 *      end-to-end liveness floor.
 *
 * Raw samples are printed before the summary; the suite exits 0.
 */

import { mkdirSync } from "node:fs";
import { FakeProcessProvider, sanitizeTerminalOutput, TerminalSession } from "./terminal-session";

const POLICY = {
  allowedProgramRoots: ["/bin", "/usr/bin"],
  allowedPathRoots: ["/tmp/gauntlet-t13"],
} as const;

/** Deterministic PRNG so every run measures the identical byte stream. */
function lcg(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

/** A PTY-shaped hostile chunk: plain text laced with CSI, OSC 8, OSC 52, DCS. */
function hostileChunk(rng: () => number, size: number): string {
  const words = "build ok warning failed retry sync 42 done path /tmp/gauntlet-t13".split(" ");
  let out = "";
  while (out.length < size) {
    switch (Math.floor(rng() * 6)) {
      case 0:
        out += `\x1b[${Math.floor(rng() * 8) + 30}m`;
        break;
      case 1:
        out += "\x1b]8;;http://example.invalid\x07link\x1b]8;;\x07";
        break;
      case 2:
        out += `\x1b]52;c;${Math.floor(rng() * 1e9).toString(36)}\x07`;
        break;
      case 3:
        out += "\x1bP+q544e\x1b\\";
        break;
      case 4:
        out += "\r\n";
        break;
      default:
        out += `${words[Math.floor(rng() * words.length)]} `;
    }
  }
  return out.slice(0, size);
}

function percentile(sorted: number[], p: number): number {
  const index = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return sorted[index] ?? 0;
}

function timeSamples(runs: number, body: () => void): number[] {
  const samples: number[] = [];
  body(); // warm-up (JIT + caches), not recorded
  for (let i = 0; i < runs; i++) {
    const t0 = performance.now();
    body();
    samples.push(performance.now() - t0);
  }
  return samples;
}

console.log("=== T13 Terminal-Path Benchmark (frozen fixtures, deterministic buffers) ===");

// --- 1. sanitizer one-shot throughput (strip + caret) --------------------------
const SANITIZE_BYTES = 8 * 1024 * 1024; // 8 MiB frozen fixture
const sanitizeRng = lcg(0x7131);
const hostileText = hostileChunk(sanitizeRng, SANITIZE_BYTES);

for (const mode of ["strip", "caret"] as const) {
  const samples = timeSamples(10, () => {
    sanitizeTerminalOutput(hostileText, { mode });
  });
  const sorted = [...samples].sort((a, b) => a - b);
  const med = sorted[Math.floor(sorted.length / 2)] ?? 0;
  console.log(
    `[sanitize:${mode}] 8.00MiB hostile text: median ${med.toFixed(2)}ms ` +
      `${(SANITIZE_BYTES / 1048576 / (med / 1000) || 0).toFixed(0)} MiB/s ` +
      `(min ${Math.min(...samples).toFixed(2)}ms, samples ${samples.map((s) => s.toFixed(1)).join(",")})`,
  );
}

// --- 2. session ingest → projection throughput (chunked PTY stream) -------------
const CHUNKS = 2000; // frozen
const CHUNK_SIZE = 4096; // frozen → 8 MiB stream
const chunkRng = lcg(0x7132);
const chunks = Array.from({ length: CHUNKS }, () =>
  new TextEncoder().encode(hostileChunk(chunkRng, CHUNK_SIZE)),
);

// The ingest path is exercised through the provider seam the way the runtime
// does: runScript delivers the encoded chunks through the session's spawn-time
// data handler (sanitize → scrollback → projection), so every measured byte
// crosses the real DSN-014 boundary.

function runIngestStream(): number {
  const provider = new FakeProcessProvider();
  const session = new TerminalSession({ program: "/bin/sh", cwd: "/tmp/gauntlet-t13" }, provider, {
    pathPolicy: POLICY,
    maxScrollback: 1000,
  });
  provider.runScript(chunks.map((chunk) => ({ emit: new TextDecoder().decode(chunk) })));
  const projected = session.project();
  if (projected.scrollback.length === 0) {
    throw new Error("ingest bench produced no scrollback");
  }
  return projected.generation;
}

const ingestSamples = timeSamples(8, () => {
  runIngestStream();
});
{
  const sorted = [...ingestSamples].sort((a, b) => a - b);
  const med = sorted[Math.floor(sorted.length / 2)] ?? 0;
  const mib = (CHUNKS * CHUNK_SIZE) / 1048576;
  console.log(
    `[session-ingest] ${CHUNKS}×${CHUNK_SIZE}B PTY stream (${mib.toFixed(1)}MiB): median ${med.toFixed(2)}ms ` +
      `${(mib / (med / 1000) || 0).toFixed(0)} MiB/s ` +
      `(min ${Math.min(...ingestSamples).toFixed(2)}ms, samples ${ingestSamples.map((s) => s.toFixed(1)).join(",")})`,
  );
}

// --- 3. projection rebuild latency over a full scrollback -----------------------
{
  const provider = new FakeProcessProvider();
  const session = new TerminalSession({ program: "/bin/sh", cwd: "/tmp/gauntlet-t13" }, provider, {
    pathPolicy: POLICY,
    maxScrollback: 1000,
  });
  const fillRng = lcg(0x7133);
  provider.runScript([{ emit: hostileChunk(fillRng, 256 * 1024) }]); // fills 1000-line cap
  const samples = timeSamples(50, () => {
    const projected = session.project();
    if (projected.scrollback.length !== 1000) {
      throw new Error("scrollback cap violated during projection bench");
    }
  });
  const sorted = [...samples].sort((a, b) => a - b);
  console.log(
    `[projection] full 1000-line project(): p50 ${percentile(sorted, 50).toFixed(3)}ms ` +
      `p95 ${percentile(sorted, 95).toFixed(3)}ms (samples ${samples.map((s) => s.toFixed(3)).join(",")})`,
  );
}

// --- 4. resize churn latency ------------------------------------------------------
{
  const provider = new FakeProcessProvider();
  const session = new TerminalSession({ program: "/bin/sh", cwd: "/tmp/gauntlet-t13" }, provider, {
    pathPolicy: POLICY,
  });
  const samples = timeSamples(20, () => {
    for (let i = 0; i < 100; i++) {
      session.resize(80 + (i % 60), 24 + (i % 20));
    }
  });
  const sorted = [...samples].sort((a, b) => a - b);
  console.log(
    `[resize-churn] 100 resize→project cycles: p50 ${percentile(sorted, 50).toFixed(3)}ms ` +
      `p95 ${percentile(sorted, 95).toFixed(3)}ms (samples ${samples.map((s) => s.toFixed(3)).join(",")})`,
  );
}

// --- 5. real child round trip (liveness floor, T03-qualified Bun.spawn) -----------
{
  mkdirSync("/tmp/gauntlet-t13", { recursive: true });
  const samples: number[] = [];
  for (let i = 0; i < 5; i++) {
    const t0 = performance.now();
    const proc = Bun.spawn(["/bin/echo", "terminal-path-liveness"], {
      cwd: "/tmp/gauntlet-t13",
      stdout: "pipe",
      stderr: "pipe",
    });
    const text = await new Response(proc.stdout).text();
    const code = await proc.exited;
    samples.push(performance.now() - t0);
    if (code !== 0 || !text.includes("terminal-path-liveness")) {
      throw new Error("real child round trip failed");
    }
  }
  const sorted = [...samples].sort((a, b) => a - b);
  console.log(
    `[real-child] Bun.spawn echo round trip: p50 ${percentile(sorted, 50).toFixed(2)}ms ` +
      `(samples ${samples.map((s) => s.toFixed(2)).join(",")})`,
  );
}

console.log("=== Terminal-Path Benchmark Complete: sanitizer, projection, and churn measured ===");
