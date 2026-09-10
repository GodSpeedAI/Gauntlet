/**
 * T21 large-Case payment benchmark (plan T21, REQ-BUN-006, REQ-FOCUS-003/004,
 * REQ-GOAL-006, REQ-SYNC-002, REQ-UX-007, prereg P2/D3).
 *
 * Workload: the frozen TARGET fixture (103,610 authoritative events; 25,000
 * units, 3,000 evidence, 600 sessions in 10 parent chains 60 deep) replayed
 * through the REAL WorkbenchModel + SqliteProjectionStore, with the T20
 * production planner swap active (the model plans through the
 * MaterializationFoldCoordinator over a real Bun worker). Measured scenarios
 * (all frozen in payment-budgets.ts BEFORE any measurement):
 *
 *   HC1 cold start (fresh model + full replay + first ready snapshot)
 *   HC2 idle RSS/CPU after the baseline and bursts
 *   HC3 sustained event-replay throughput + one coalesced render per burst
 *   HC4 focus switch with the worker plan cached
 *   HC5 hoist/de-hoist through the production model path
 *   HC6 Trace scroll frames over a 200-record window of the huge history
 *   HC7 large code/diff projection (pinned SXR ledger parse + evidence lens)
 *   HC8 multiple Prime-child update bursts interleaved with embedded-terminal
 *       chunk ingest (fakes for the child processes, real session + sanitizer)
 *   HC9 high-volume event stream DURING typing/scrolling simulation (real
 *       composer state machine + renderTraceView, render subscriber attached)
 *
 * Raw samples, percentile summaries, materialization counters, and peak RSS
 * are printed. PASS/MISS against the frozen budgets decides the exit code:
 * any MISS records the miss (never a threshold change) and names the measured
 * bottleneck in the T21 evidence.
 */

import { mkdirSync } from "node:fs";
import { cpus, totalmem } from "node:os";
import { EMPTY_AFFORDANCE_OBSERVATION } from "./affordance";
import { composeKeypress, createComposerState } from "./composer";
import { FocusRef } from "./focus";
import {
  countT21Events,
  createT21CaseEvents,
  T21_FROZEN_BUDGETS,
  T21_FROZEN_HOST_FACTS,
  T21_FROZEN_SAMPLE_COUNTS,
  T21_LARGE_FIXTURE,
} from "./payment-budgets";
import type { AuthoritativeEventFrame } from "./projection-store";
import {
  parseSxrLedgerExport,
  projectSxrEvidenceLens,
  type SxrLedgerExportParse,
} from "./sxr-adapter";
import { FakeProcessProvider, TerminalSession } from "./terminal-session";
import { renderTraceView } from "./views-chat-trace";
import type { ViewRenderArgs } from "./views-work";
import { type FullWorkbenchSnapshot, WorkbenchModel } from "./workbench-model";
import { BunWorkerFoldExecutor, peakRssBytes, summarizeLatencySamples } from "./worker-folds";

// --- Bench helpers (T17 patterns; no thresholds live here) --------------------------

interface Scenario {
  readonly name: string;
  readonly samples: number[];
}

function newScenario(name: string): Scenario {
  return { name, samples: [] };
}

function report(scenario: Scenario): void {
  const summary = summarizeLatencySamples(scenario.samples);
  console.log(
    `[samples] ${scenario.name}: n=${summary.n} mean=${summary.mean_ms.toFixed(3)}ms ` +
      `p50=${summary.p50_ms.toFixed(3)}ms p95=${summary.p95_ms.toFixed(3)}ms ` +
      `p99=${summary.p99_ms.toFixed(3)}ms max=${summary.max_ms.toFixed(3)}ms`,
  );
  console.log(`[raw] ${scenario.name}: [${scenario.samples.map((s) => s.toFixed(3)).join(", ")}]`);
}

let failures = 0;
function check(ok: boolean, label: string, detail: string): void {
  console.log(`[${ok ? "PASS" : "MISS"}] ${label}: ${detail}`);
  if (!ok) {
    failures += 1;
  }
}

function lcg(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state;
  };
}

// --- Header: frozen budgets + host facts (actuals printed; drift warns, never rewrites) --

console.log(
  `=== T21 large-case bench (frozen budgets: cold start p99<=${T21_FROZEN_BUDGETS.cold_start_p99_ms}ms, ` +
    `burst replay>=${T21_FROZEN_BUDGETS.burst_replay_min_events_per_sec} ev/s, coalesced render p99<=` +
    `${T21_FROZEN_BUDGETS.coalesced_render_p99_ms}ms, focus switch p50<=` +
    `${T21_FROZEN_BUDGETS.focus_switch_cached_p50_ms}ms/p99<=${T21_FROZEN_BUDGETS.focus_switch_cached_p99_ms}ms, ` +
    `horizon move p50<=${T21_FROZEN_BUDGETS.horizon_move_p50_ms}ms/p99<=` +
    `${T21_FROZEN_BUDGETS.horizon_move_p99_ms}ms, scroll frame p99<=` +
    `${T21_FROZEN_BUDGETS.trace_scroll_frame_p99_ms}ms, code projection p99<=` +
    `${T21_FROZEN_BUDGETS.code_projection_p99_ms}ms, prime burst>=` +
    `${T21_FROZEN_BUDGETS.prime_burst_min_events_per_sec} ev/s, terminal chunk p99<=` +
    `${T21_FROZEN_BUDGETS.terminal_chunk_ingest_p99_ms}ms, stream absorb p50<=` +
    `${T21_FROZEN_BUDGETS.stream_absorb_p50_ms}ms/p99<=${T21_FROZEN_BUDGETS.stream_absorb_p99_ms}ms, ` +
    `typing p99<=${T21_FROZEN_BUDGETS.typing_keystroke_p99_ms}ms, scroll-under-load p99<=` +
    `${T21_FROZEN_BUDGETS.scroll_under_load_p99_ms}ms, RSS<=${T21_FROZEN_BUDGETS.bench_peak_rss_bytes}B) ===`,
);

{
  const actualCores = cpus().length;
  const actualMemKb = Math.floor(totalmem() / 1024);
  if (
    actualCores !== T21_FROZEN_HOST_FACTS.cpu_cores ||
    actualMemKb !== T21_FROZEN_HOST_FACTS.memory_total_kb ||
    Bun.version !== T21_FROZEN_HOST_FACTS.runtime.replace("Bun ", "")
  ) {
    console.log(
      `[host-drift WARNING] declared ${T21_FROZEN_HOST_FACTS.cpu_cores} cores / ` +
        `${T21_FROZEN_HOST_FACTS.memory_total_kb} kB / ${T21_FROZEN_HOST_FACTS.runtime}; ` +
        `actual ${actualCores} cores / ${actualMemKb} kB / Bun ${Bun.version ?? "unknown"} ` +
        "(recorded as a comparability confound, never normalized away)",
    );
  } else {
    console.log(
      `[host] matches frozen facts: ${actualCores} cores, ${(actualMemKb / 1024 / 1024).toFixed(1)} GiB, Bun ${Bun.version}`,
    );
  }
}

// --- Fixture --------------------------------------------------------------------

const fixtureEvents = createT21CaseEvents(T21_LARGE_FIXTURE);
if (fixtureEvents.length !== countT21Events(T21_LARGE_FIXTURE)) {
  throw new Error("fixture generator disagrees with the declared event count");
}
console.log(
  `[fixture] ${T21_LARGE_FIXTURE.generation}: ${fixtureEvents.length} declared/replayed events ` +
    `(${T21_LARGE_FIXTURE.units} units, ${T21_LARGE_FIXTURE.evidence} evidence, ` +
    `${T21_LARGE_FIXTURE.sessions} sessions in ${T21_LARGE_FIXTURE.session_trees} parent chains, ` +
    `${T21_LARGE_FIXTURE.unit_update_waves} update waves = history depth)`,
);

function replayBaseline(model: WorkbenchModel): void {
  model.startGeneration(T21_LARGE_FIXTURE.generation);
  for (const frame of fixtureEvents) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`fixture event rejected: ${result.reason ?? "unknown"}`);
    }
  }
  model.markReady(T21_LARGE_FIXTURE.generation);
}

// --- HC1: cold start (fresh model + full 103,610-event replay + first snapshot) ---

const coldStart = newScenario("HC1 cold start (model + full baseline replay + first snapshot)");
const coldStartEventsPerSec: number[] = [];
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.cold_start_samples; i += 1) {
  const start = performance.now();
  const model = new WorkbenchModel({ enableChat: false });
  replayBaseline(model);
  const snapshot = model.getSnapshot();
  const ms = performance.now() - start;
  if (snapshot.units.length !== T21_LARGE_FIXTURE.units) {
    throw new Error("cold start snapshot missing projected units");
  }
  coldStart.samples.push(ms);
  coldStartEventsPerSec.push(fixtureEvents.length / (ms / 1000));
  model.close();
}
report(coldStart);
console.log(
  `[raw] HC1 implied baseline replay rate: [${coldStartEventsPerSec
    .map((rate) => Math.round(rate).toString())
    .join(", ")}] events/s`,
);
const coldSummary = summarizeLatencySamples(coldStart.samples);
check(
  coldSummary.p99_ms <= T21_FROZEN_BUDGETS.cold_start_p99_ms,
  "HC1 cold start p99",
  `${coldSummary.p99_ms.toFixed(0)}ms <= ${T21_FROZEN_BUDGETS.cold_start_p99_ms}ms`,
);

// --- Main production model: T20 planner swap active (real Bun worker coordinator) --

const executor = new BunWorkerFoldExecutor();
const model = new WorkbenchModel({ enableChat: false, foldExecutor: executor });
const buildStart = performance.now();
replayBaseline(model);
console.log(
  `[fixture] main model baseline replayed in ${(performance.now() - buildStart).toFixed(0)}ms ` +
    "(chat projection disabled in bench config: declared, REQ-DATA-005 non-canonical)",
);

// Declared warmup (off-clock): the once-per-generation worker world install +
// first off-path fold, so per-interaction scenarios measure steady-state paths.
{
  const warmStart = performance.now();
  model.getSnapshot(); // schedules the worker refresh; pays the inline fallback fold
  const inlineFallbackMs = performance.now() - warmStart;
  let waited = 0;
  while (model.workerPlan === null && waited < 30_000) {
    await Bun.sleep(10);
    waited += 10;
  }
  if (model.workerPlan === null) {
    throw new Error("worker plan did not converge within 30s during warmup");
  }
  console.log(
    `[warmup] worker world install + first fold: inline fallback snapshot ${inlineFallbackMs.toFixed(1)}ms, ` +
      `plan converged after ${waited}ms (once per generation; recorded, not budgeted)`,
  );
}

// --- HC3: sustained replay throughput + coalesced renders (no subscriber) ----------

function unitBurstFrames(
  cursorPrefix: string,
  count: number,
  round: number,
): AuthoritativeEventFrame[] {
  const rng = lcg(0x21b057 + round);
  const frames: AuthoritativeEventFrame[] = [];
  for (let i = 0; i < count; i += 1) {
    frames.push({
      generation_id: T21_LARGE_FIXTURE.generation,
      source_id: "gauntlet",
      cursor: `${cursorPrefix}${round}-${i}`,
      event_type: "unit_updated",
      payload: {
        unit_id: `u-${(i * 7 + round) % T21_LARGE_FIXTURE.units}`,
        title: `Synthetic unit ${(i * 7 + round) % T21_LARGE_FIXTURE.units}`,
        phase: "execution",
        status: rng() % 2 === 0 ? "active" : "settled",
        authority: "normal",
        blast_radius: "isolated",
      },
      timestamp_ms: 2000 + i,
    });
  }
  return frames;
}

const burstRate = newScenario("HC3 burst replay throughput (10,000 events, no subscriber)");
const coalescedRender = newScenario("HC3 coalesced presentation render after burst");
for (let round = 0; round < T21_FROZEN_SAMPLE_COUNTS.burst_rounds; round += 1) {
  const frames = unitBurstFrames("u-cur-b", T21_FROZEN_SAMPLE_COUNTS.burst_events, round);
  const start = performance.now();
  for (const frame of frames) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`burst event rejected: ${result.reason ?? "unknown"}`);
    }
  }
  const burstMs = performance.now() - start;
  burstRate.samples.push(frames.length / (burstMs / 1000));
  await Bun.sleep(0);
  const renderStart = performance.now();
  const snapshot = model.getSnapshot();
  coalescedRender.samples.push(performance.now() - renderStart);
  if (snapshot.trace_records.length !== fixtureEvents.length + frames.length * (round + 1)) {
    throw new Error("trace history does not match replayed volume");
  }
  await Bun.sleep(0);
}
report(burstRate);
report(coalescedRender);
check(
  Math.min(...burstRate.samples) >= T21_FROZEN_BUDGETS.burst_replay_min_events_per_sec,
  "HC3a burst replay throughput (worst round)",
  `${Math.min(...burstRate.samples).toFixed(0)}/s >= ${T21_FROZEN_BUDGETS.burst_replay_min_events_per_sec}/s`,
);
const coalescedSummary = summarizeLatencySamples(coalescedRender.samples);
check(
  coalescedSummary.p99_ms <= T21_FROZEN_BUDGETS.coalesced_render_p99_ms,
  "HC3b coalesced render p99",
  `${coalescedSummary.p99_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.coalesced_render_p99_ms}ms`,
);

// --- HC2: idle RSS/CPU over a quiet window ------------------------------------------

// Settle FIRST (run-1 lesson, recorded as a failed round in the evidence): the
// window must measure IDLE, not post-burst fold convergence or GC churn — wait
// for the worker plan to re-converge after the bursts, force one synchronous
// GC, and give the scheduler a breath before sampling.
{
  let waited = 0;
  while (model.workerPlan === null && waited < 30_000) {
    await Bun.sleep(10);
    waited += 10;
  }
  Bun.gc(true);
  await Bun.sleep(200);
}
await Bun.sleep(50);
{
  const cpuStart = process.cpuUsage();
  const wallStart = performance.now();
  await Bun.sleep(T21_FROZEN_SAMPLE_COUNTS.idle_cpu_window_ms);
  const cpuDelta = process.cpuUsage(cpuStart);
  const wallMs = performance.now() - wallStart;
  const idleCpuMs = ((cpuDelta.user + cpuDelta.system) / 1000) * (1000 / wallMs);
  const rss = peakRssBytes();
  console.log(
    `[idle] ${wallMs.toFixed(0)}ms quiet window: ${idleCpuMs.toFixed(1)}ms CPU per 1000ms wall, ` +
      `RSS ${(rss / 1024 / 1024).toFixed(0)}MB`,
  );
  check(
    idleCpuMs <= T21_FROZEN_BUDGETS.idle_cpu_max_ms_per_1000ms,
    "HC2b idle CPU",
    `${idleCpuMs.toFixed(1)}ms <= ${T21_FROZEN_BUDGETS.idle_cpu_max_ms_per_1000ms}ms per 1000ms`,
  );
  check(
    rss <= T21_FROZEN_BUDGETS.bench_peak_rss_bytes,
    "HC2a idle RSS checkpoint",
    `${(rss / 1024 / 1024).toFixed(0)}MB <= ${(T21_FROZEN_BUDGETS.bench_peak_rss_bytes / 1024 / 1024).toFixed(0)}MB`,
  );
}

// --- Render consumer (T17 pattern): interactive scenarios (HC4, HC5, HC9) run
// --- with a subscribed consumer so every model interaction synchronously pays
// --- getSnapshot — the real render path. Catch-up scenarios (HC3, HC8) run
// --- unsubscribed per their frozen registry semantics (REQ-SYNC-002 class).

let renders = 0;
const latest: { snapshot: FullWorkbenchSnapshot | null } = { snapshot: null };
const detachRef: { current: (() => void) | null } = { current: null };
const detachRenderConsumer = (): void => {
  const detach = detachRef.current;
  detachRef.current = null;
  detach?.();
};
const attachRenderConsumer = (): void => {
  detachRenderConsumer();
  detachRef.current = model.subscribe((snapshot) => {
    renders += 1;
    latest.snapshot = snapshot;
  });
};
attachRenderConsumer();

// --- HC4: focus switch with the worker plan cached ----------------------------------

function leafRef(i: number): FocusRef {
  return FocusRef.for("gauntlet", "WorkItem", `u-${i % T21_LARGE_FIXTURE.units}`);
}

const focusSwitch = newScenario("HC4 focus switch (cached worker plan, production path)");
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.focus_switch_warmups; i += 1) {
  model.selectFocus(leafRef(9_000 + i));
}
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.focus_switch_samples; i += 1) {
  const start = performance.now();
  model.selectFocus(leafRef(9_500 + i));
  focusSwitch.samples.push(performance.now() - start);
  if (i % T21_FROZEN_SAMPLE_COUNTS.yield_every === T21_FROZEN_SAMPLE_COUNTS.yield_every - 1) {
    await Bun.sleep(0);
  }
}
report(focusSwitch);
const switchSummary = summarizeLatencySamples(focusSwitch.samples);
check(
  switchSummary.p50_ms <= T21_FROZEN_BUDGETS.focus_switch_cached_p50_ms,
  "HC4 focus switch p50 (cached plan)",
  `${switchSummary.p50_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.focus_switch_cached_p50_ms}ms`,
);
check(
  switchSummary.p99_ms <= T21_FROZEN_BUDGETS.focus_switch_cached_p99_ms,
  "HC4 focus switch p99 (cached plan)",
  `${switchSummary.p99_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.focus_switch_cached_p99_ms}ms`,
);

// --- HC5: hoist/de-hoist through the production model path ---------------------------

const hoistInteraction = newScenario("HC5 hoist interaction (model call incl. render)");
const dehoistInteraction = newScenario("HC5 de-hoist interaction (model call incl. render)");
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.horizon_move_warmups; i += 1) {
  model.hoistToFocus(leafRef(100 + i), {
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  model.dehoist();
}
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.hoist_samples; i += 1) {
  const start = performance.now();
  model.hoistToFocus(leafRef(200 + i), {
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  hoistInteraction.samples.push(performance.now() - start);
  if (i % T21_FROZEN_SAMPLE_COUNTS.yield_every === T21_FROZEN_SAMPLE_COUNTS.yield_every - 1) {
    await Bun.sleep(0);
  }
}
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.dehoist_samples; i += 1) {
  const start = performance.now();
  model.dehoist();
  dehoistInteraction.samples.push(performance.now() - start);
  if (i % T21_FROZEN_SAMPLE_COUNTS.yield_every === T21_FROZEN_SAMPLE_COUNTS.yield_every - 1) {
    await Bun.sleep(0);
  }
}
report(hoistInteraction);
report(dehoistInteraction);
const horizonPooled = [...hoistInteraction.samples, ...dehoistInteraction.samples];
const horizonSummary = summarizeLatencySamples(horizonPooled);
check(
  horizonSummary.p50_ms <= T21_FROZEN_BUDGETS.horizon_move_p50_ms,
  "HC5 horizon movement p50 (pooled hoist+de-hoist)",
  `${horizonSummary.p50_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.horizon_move_p50_ms}ms`,
);
check(
  horizonSummary.p99_ms <= T21_FROZEN_BUDGETS.horizon_move_p99_ms,
  "HC5 horizon movement p99 (pooled hoist+de-hoist)",
  `${horizonSummary.p99_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.horizon_move_p99_ms}ms`,
);

// Off-clock (plan step 4): record loaded/subscribed/indexed counters for the
// horizon vs the whole-Case world — data logistics, not only visuals.
{
  model.hoistToFocus(leafRef(7), {
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  const hoistedSummary = model.getSnapshot().materialization;
  model.dehoist();
  const wholeCaseSummary = model.getSnapshot().materialization;
  console.log(
    `[logistics] hoisted one-Focus horizon: ${JSON.stringify(hoistedSummary)}` +
      "\n" +
      `[logistics] whole-Case (unhoisted):       ${JSON.stringify(wholeCaseSummary)}`,
  );
}

// --- HC6: Trace scroll frames over a 200-record window -------------------------------

const scrollSnapshot = model.getSnapshot();
const totalTrace = scrollSnapshot.trace_records.length;
const traceScroll = newScenario("HC6 Trace scroll frame (200-record window render)");
{
  const observation = EMPTY_AFFORDANCE_OBSERVATION;
  for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.trace_scroll_samples; i += 1) {
    const position = Math.floor(
      (i / T21_FROZEN_SAMPLE_COUNTS.trace_scroll_samples) *
        Math.max(1, totalTrace - T21_FROZEN_SAMPLE_COUNTS.trace_scroll_window),
    );
    const start = performance.now();
    const windowed: ViewRenderArgs = {
      snapshot: {
        ...scrollSnapshot,
        trace_records: scrollSnapshot.trace_records.slice(
          position,
          position + T21_FROZEN_SAMPLE_COUNTS.trace_scroll_window,
        ),
      },
      observation,
    };
    const content = renderTraceView(windowed);
    traceScroll.samples.push(performance.now() - start);
    if (content.lines.length !== T21_FROZEN_SAMPLE_COUNTS.trace_scroll_window + 1) {
      throw new Error("trace scroll frame rendered an unexpected window");
    }
  }
}
report(traceScroll);
const scrollSummary = summarizeLatencySamples(traceScroll.samples);
check(
  scrollSummary.p99_ms <= T21_FROZEN_BUDGETS.trace_scroll_frame_p99_ms,
  "HC6 Trace scroll frame p99",
  `${scrollSummary.p99_ms.toFixed(3)}ms <= ${T21_FROZEN_BUDGETS.trace_scroll_frame_p99_ms}ms ` +
    `(history=${totalTrace} records; an O(history) frame would miss)`,
);

// --- HC7: large code/diff projection through the real SXR adapter --------------------

function buildLedgerDocument(rows: number): string {
  const kinds = [
    "run.started",
    "plan.created",
    "semantic.diff",
    "observation.recorded",
    "evidence.created",
    "run.finished",
  ] as const;
  const entries: Record<string, unknown>[] = [];
  for (let i = 1; i <= rows; i += 1) {
    const kind = kinds[(i - 1) % kinds.length] ?? "run.started";
    const payload =
      kind === "semantic.diff"
        ? {
            path: `src/module_${i % 50}.ts`,
            additions: i % 97,
            deletions: i % 31,
            subject_refs: ["repo:t21-code"],
          }
        : { ref: `${kind.split(".")[0]}:${i}`, subject_refs: ["repo:t21-code"] };
    entries.push({
      append_ordinal: i,
      record_kind: kind,
      ledger_id: "ledger-t21-code",
      entry_hash: `sha256:${new Bun.CryptoHasher("sha256").update(`${i}:${kind}`).digest("hex")}`,
      payload,
    });
  }
  return JSON.stringify(entries);
}

const ledgerJson = buildLedgerDocument(T21_FROZEN_SAMPLE_COUNTS.code_projection_rows);
const codeProjection = newScenario("HC7 large code/diff projection (SXR parse + evidence lens)");
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.code_projection_samples; i += 1) {
  const start = performance.now();
  const parse: SxrLedgerExportParse = parseSxrLedgerExport(ledgerJson);
  if (parse.status !== "parsed") {
    throw new Error(`SXR ledger parse refused: ${JSON.stringify(parse)}`);
  }
  if (
    !parse.ordinal_density_ok ||
    parse.rows.length !== T21_FROZEN_SAMPLE_COUNTS.code_projection_rows
  ) {
    throw new Error("SXR ledger parse lost rows or ordinal density");
  }
  const lens = projectSxrEvidenceLens(parse, "t21-code");
  if (lens.status !== "projected") {
    throw new Error(`SXR lens projection refused: ${JSON.stringify(lens)}`);
  }
  codeProjection.samples.push(performance.now() - start);
}
report(codeProjection);
console.log(
  `[fixture] HC7 ledger: ${T21_FROZEN_SAMPLE_COUNTS.code_projection_rows} rows, ` +
    `${(ledgerJson.length / 1024 / 1024).toFixed(2)} MiB pinned document`,
);
const codeSummary = summarizeLatencySamples(codeProjection.samples);
check(
  codeSummary.p99_ms <= T21_FROZEN_BUDGETS.code_projection_p99_ms,
  "HC7 code projection p99",
  `${codeSummary.p99_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.code_projection_p99_ms}ms`,
);

// --- HC8: multiple Prime-child bursts + embedded-terminal ingest under load ----------

// Catch-up class (frozen registry semantics): burst absorption WITHOUT the
// per-event snapshot materialization — detach the render consumer for this
// scenario (run-2 lesson: a subscribed consumer here turned every absorbed
// event into a full inline-fold snapshot and wedged the bench; recorded as a
// failed round in the T21 evidence).
detachRenderConsumer();

const TERMINAL_CWD = "/tmp/gauntlet-t21";
mkdirSync(TERMINAL_CWD, { recursive: true });
const TERMINAL_POLICY = {
  allowedProgramRoots: ["/bin", "/usr/bin"],
  allowedPathRoots: [TERMINAL_CWD],
} as const;

function hostileChunk(rng: () => number, size: number): string {
  const words = "build ok warning failed retry sync 42 done path /tmp/gauntlet-t21".split(" ");
  let out = "";
  while (out.length < size) {
    switch (Math.floor((rng() / 0x100000000) * 6)) {
      case 0:
        out += `\x1b[${Math.floor((rng() / 0x100000000) * 8) + 30}m`;
        break;
      case 1:
        out += "\x1b]8;;http://example.invalid\x07link\x1b]8;;\x07";
        break;
      case 2:
        out += `\x1b]52;c;${Math.floor((rng() / 0x100000000) * 1e9).toString(36)}\x07`;
        break;
      case 3:
        out += "\x1bP+q544e\x1b\\";
        break;
      case 4:
        out += "\r\n";
        break;
      default:
        out += `${words[Math.floor((rng() / 0x100000000) * words.length) % words.length] ?? "ok"} `;
    }
  }
  return out;
}

function childBurstFrames(round: number): AuthoritativeEventFrame[] {
  const frames: AuthoritativeEventFrame[] = [];
  const children = T21_FROZEN_SAMPLE_COUNTS.prime_burst_children;
  const perChild = T21_FROZEN_SAMPLE_COUNTS.prime_burst_events_per_child;
  for (let step = 0; step < perChild; step += 1) {
    for (let child = 0; child < children; child += 1) {
      frames.push({
        generation_id: T21_LARGE_FIXTURE.generation,
        source_id: "prime",
        cursor: `c-cur-${round}-${child}-${step}`,
        event_type: "session_updated",
        payload: {
          session_id: `s-${child % 10}-${Math.floor(child / 10)}`,
          parent_id: null,
          model: child % 2 === 0 ? "model-a" : "model-b",
          status: step % 2 === 0 ? "running" : "waiting",
        },
        timestamp_ms: 3000 + step,
      });
    }
  }
  return frames;
}

const primeBurstRate = newScenario("HC8 Prime-child burst throughput (20 children interleaved)");
const terminalChunk = newScenario("HC8 terminal chunk ingest during burst");
for (let round = 0; round < T21_FROZEN_SAMPLE_COUNTS.prime_burst_rounds; round += 1) {
  const provider = new FakeProcessProvider();
  const session = new TerminalSession({ program: "/bin/sh", cwd: TERMINAL_CWD }, provider, {
    pathPolicy: TERMINAL_POLICY,
    maxScrollback: 1000,
  });
  const chunkRng = lcg(0x217e24 + round);
  const chunks = Array.from({ length: T21_FROZEN_SAMPLE_COUNTS.terminal_chunks_per_burst }, () =>
    hostileChunk(chunkRng, T21_FROZEN_SAMPLE_COUNTS.terminal_chunk_bytes),
  );
  const frames = childBurstFrames(round);
  let chunkIndex = 0;
  const start = performance.now();
  for (const [index, frame] of frames.entries()) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`child burst event rejected: ${result.reason ?? "unknown"}`);
    }
    if (index % 5 === 4 && chunkIndex < chunks.length) {
      const chunkStart = performance.now();
      provider.runScript([{ emit: chunks[chunkIndex] ?? "" }]);
      terminalChunk.samples.push(performance.now() - chunkStart);
      chunkIndex += 1;
    }
  }
  const burstMs = performance.now() - start;
  primeBurstRate.samples.push(frames.length / (burstMs / 1000));
  const projected = session.project();
  if (projected.scrollback.length === 0) {
    throw new Error("terminal ingest produced no scrollback during the burst");
  }
  await Bun.sleep(0);
}
report(primeBurstRate);
report(terminalChunk);
check(
  Math.min(...primeBurstRate.samples) >= T21_FROZEN_BUDGETS.prime_burst_min_events_per_sec,
  "HC8a Prime-child burst throughput (worst round)",
  `${Math.min(...primeBurstRate.samples).toFixed(0)}/s >= ${T21_FROZEN_BUDGETS.prime_burst_min_events_per_sec}/s`,
);
const chunkSummary = summarizeLatencySamples(terminalChunk.samples);
check(
  chunkSummary.p99_ms <= T21_FROZEN_BUDGETS.terminal_chunk_ingest_p99_ms,
  "HC8b terminal chunk ingest p99 under burst load",
  `${chunkSummary.p99_ms.toFixed(3)}ms <= ${T21_FROZEN_BUDGETS.terminal_chunk_ingest_p99_ms}ms ` +
    `(4 KiB hostile chunks, real sanitizer + scrollback)`,
);

// --- HC9: high-volume stream DURING typing/scrolling (render consumer attached) -------

attachRenderConsumer();
const streamAbsorb = newScenario("HC9 stream event absorb (apply + render, subscriber attached)");
const typingLatency = newScenario("HC9 composer keystroke during stream");
const scrollUnderLoad = newScenario("HC9 Trace scroll frame during stream");
const composer = createComposerState();
const keys = "abcdefghij";
for (let i = 0; i < T21_FROZEN_SAMPLE_COUNTS.stream_events; i += 1) {
  const frame = unitBurstFrames("u-cur-s", 1, 10_000 + i)[0];
  if (frame === undefined) {
    throw new Error("stream frame missing");
  }
  const start = performance.now();
  const result = model.applyEvent(frame);
  streamAbsorb.samples.push(performance.now() - start);
  if (!result.applied) {
    throw new Error(`stream event rejected: ${result.reason ?? "unknown"}`);
  }
  const keyStart = performance.now();
  const keypress = composeKeypress(composer, keys.charAt(i % keys.length) ?? "x");
  typingLatency.samples.push(performance.now() - keyStart);
  if (keypress.submitted !== null) {
    throw new Error("typing simulation submitted unexpectedly");
  }
  if (
    i % T21_FROZEN_SAMPLE_COUNTS.stream_scroll_every ===
    T21_FROZEN_SAMPLE_COUNTS.stream_scroll_every - 1
  ) {
    const current: FullWorkbenchSnapshot | null = latest.snapshot;
    if (current !== null) {
      const position = Math.max(
        0,
        current.trace_records.length - T21_FROZEN_SAMPLE_COUNTS.trace_scroll_window,
      );
      const scrollStart = performance.now();
      const content = renderTraceView({
        snapshot: {
          ...current,
          trace_records: current.trace_records.slice(
            position,
            position + T21_FROZEN_SAMPLE_COUNTS.trace_scroll_window,
          ),
        },
        observation: EMPTY_AFFORDANCE_OBSERVATION,
      });
      scrollUnderLoad.samples.push(performance.now() - scrollStart);
      if (content.lines.length === 0) {
        throw new Error("scroll-under-load rendered nothing");
      }
    }
  }
  if (i % T21_FROZEN_SAMPLE_COUNTS.yield_every === T21_FROZEN_SAMPLE_COUNTS.yield_every - 1) {
    await Bun.sleep(0);
  }
}
detachRenderConsumer();
report(streamAbsorb);
report(typingLatency);
report(scrollUnderLoad);
const absorbSummary = summarizeLatencySamples(streamAbsorb.samples);
check(
  absorbSummary.p50_ms <= T21_FROZEN_BUDGETS.stream_absorb_p50_ms,
  "HC9a stream absorb p50 (render subscriber attached)",
  `${absorbSummary.p50_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.stream_absorb_p50_ms}ms`,
);
check(
  absorbSummary.p99_ms <= T21_FROZEN_BUDGETS.stream_absorb_p99_ms,
  "HC9a stream absorb p99 (render subscriber attached)",
  `${absorbSummary.p99_ms.toFixed(2)}ms <= ${T21_FROZEN_BUDGETS.stream_absorb_p99_ms}ms`,
);
const typingSummary = summarizeLatencySamples(typingLatency.samples);
check(
  typingSummary.p99_ms <= T21_FROZEN_BUDGETS.typing_keystroke_p99_ms,
  "HC9b typing keystroke p99 during stream",
  `${typingSummary.p99_ms.toFixed(3)}ms <= ${T21_FROZEN_BUDGETS.typing_keystroke_p99_ms}ms`,
);
const loadedScrollSummary = summarizeLatencySamples(scrollUnderLoad.samples);
check(
  loadedScrollSummary.p99_ms <= T21_FROZEN_BUDGETS.scroll_under_load_p99_ms,
  "HC9c scroll frame p99 during stream",
  `${loadedScrollSummary.p99_ms.toFixed(3)}ms <= ${T21_FROZEN_BUDGETS.scroll_under_load_p99_ms}ms`,
);

// --- Final resource accounting --------------------------------------------------------

{
  const rss = peakRssBytes();
  check(
    rss <= T21_FROZEN_BUDGETS.bench_peak_rss_bytes,
    "HC2a peak RSS (final)",
    `${(rss / 1024 / 1024).toFixed(0)}MB <= ${(T21_FROZEN_BUDGETS.bench_peak_rss_bytes / 1024 / 1024).toFixed(0)}MB`,
  );
  console.log(
    `[history] total authoritative events absorbed by the main model: ` +
      `${model.getTrace().length} (fixture ${fixtureEvents.length} + bursts + stream)`,
  );
  console.log(`[renders] render consumer observed ${renders} snapshot publications`);
  console.log(`[coordinator] stats: ${JSON.stringify(model.foldStats)}`);
}

executor.dispose();
model.close();

console.log(
  `\n=== T21 large-case bench complete: ${failures === 0 ? "ALL BUDGETS PASS" : `${failures} BUDGET MISS(ES)`} ===`,
);
if (failures > 0) {
  process.exitCode = 1;
}
