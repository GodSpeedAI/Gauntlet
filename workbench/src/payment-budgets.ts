/**
 * T21 frozen payment budgets, host facts, fixture ladder, and scenario registry.
 * Conforms to plan T21, REQ-BUN-006, REQ-FOCUS-003/004, REQ-GOAL-006,
 * REQ-SYNC-002, REQ-UX-007, and the frozen preregistration
 * `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T21.prereg.yaml`
 * (sha256 b4b32e5b7934320c999621de19d15164b44bb6a1432c481a24f55306ad96dc84, never modified).
 *
 * Core Invariants:
 *   - THE TASK IS THE FREEZE DISCIPLINE: every fixture size, event rate,
 *     resource ceiling, percentile threshold, and sample count below was
 *     declared BEFORE any T21 measurement ran, from static reasoning about the
 *     production code paths only. No number here was tuned after seeing a
 *     measurement, and a budget miss is recorded as a miss — never absorbed by
 *     editing this table (prereg D1). The mirror of this table is frozen FIRST
 *     in `.agents/reports/gauntlet-tui-prime-workbench/evidence/T21/payment-benchmark.md`.
 *   - Fixtures are deterministic: seeded LCG (T17's generator constants), fixed
 *     emission order, stable id scheme, round-robin evidence attachment. The
 *     same spec always produces byte-identical events; there is no Math.random.
 *   - The budgets are TARGET-scale and declared independently of T17's
 *     representative-scale FROZEN_BUDGETS (worker-folds.ts): T17 measured the
 *     placement decision; T21 measures whether the production path (T20
 *     planner swap: the model plans through the coordinator) stays affordable
 *     at 100k+-event scale, and whether the hot set stays bounded by declared
 *     relations instead of total history (prereg P1/P2).
 *   - Budget classes are declared, not accidental: "interactive" budgets bound
 *     surfaces that must stay perceptually immediate (typing, scroll frames);
 *     "coalesced" budgets bound per-event absorb/render costs that REQ-SYNC-002
 *     explicitly allows to coalesce with visible lag under overload. A budget
 *     miss names the measured bottleneck; optimization touches only that
 *     bottleneck and never canonical boundaries.
 */

import type { AuthoritativeEventFrame } from "./projection-store";

// --- Frozen host facts (declared before measurement; bench processes verify, never mutate) ---

/**
 * The host every T21 number is recorded against. The task statement and the
 * T17/T13 evidence fix this environment: a memory-constrained WSL2 host.
 * `bun --version` = 1.4.0, `nproc` = 6, MemTotal = 8,132,720 kB at freeze time.
 * Bench processes PRINT their actuals and warn on drift; they never rewrite
 * these facts (a drifted host invalidates comparability — it is recorded, not
 * normalized away).
 */
export const T21_FROZEN_HOST_FACTS = Object.freeze({
  os: "linux 6.18.35.2-microsoft-standard-WSL2",
  cpu_cores: 6,
  memory_total_kb: 8_132_720,
  runtime: "Bun 1.4.0",
  /** Benches are headless processes: no OpenTUI renderer or TTY is attached. */
  terminal: "headless bench process (no renderer attached)",
  parallelism: "one bench process at a time",
  prereg_sha256: "b4b32e5b7934320c999621de19d15164b44bb6a1432c481a24f55306ad96dc84",
});

// --- Frozen budgets (PASS = within budget; a miss is recorded as a miss) ----------------------

/**
 * Mirror of the frozen budget table in
 * `.agents/reports/gauntlet-tui-prime-workbench/evidence/T21/payment-benchmark.md`
 * (frozen FIRST, before any measurement). Every bench threshold lives here —
 * bench code contains no thresholds of its own.
 */
export const T21_FROZEN_BUDGETS = Object.freeze({
  /** HC1 cold start: fresh model + full 103,610-event baseline replay + first ready snapshot. */
  cold_start_p99_ms: 60_000,
  /** HC2a peak RSS of each bench process (1.5 GiB — the T17 B4 ceiling, same constrained host). */
  bench_peak_rss_bytes: 1_610_612_736,
  /** HC2b idle CPU: <= 10% of one core over a 1,000 ms quiet window (GC jitter allowed). */
  idle_cpu_max_ms_per_1000ms: 100,
  /** HC3a burst replay throughput (catch-up class, no render subscriber): sustained events/s. */
  burst_replay_min_events_per_sec: 2_000,
  /** HC3b one coalesced presentation render after a 10,000-event burst (REQ-SYNC-002 class). */
  coalesced_render_p99_ms: 500,
  /** HC4 focus switch with the worker plan cached (production T20 path, plan not invalidated). */
  focus_switch_cached_p50_ms: 100,
  focus_switch_cached_p99_ms: 300,
  /** HC5 hoist/de-hoist through the production model path (inline fallback fold included). */
  horizon_move_p50_ms: 1_000,
  horizon_move_p99_ms: 3_000,
  /** HC6 Trace scroll frame: one windowed Trace render over a 200-record window. */
  trace_scroll_frame_p99_ms: 50,
  /** HC7 large code/diff projection: pinned SXR ledger parse + evidence-lens projection. */
  code_projection_p99_ms: 500,
  /** HC8a Prime-child burst throughput (catch-up class, interleaved with terminal ingest). */
  prime_burst_min_events_per_sec: 2_000,
  /** HC8b embedded-terminal chunk ingest latency DURING the event burst (T13 isolated ~0.06 ms). */
  terminal_chunk_ingest_p99_ms: 5,
  /** HC9a high-volume stream absorb per event, render subscriber attached, typing interleaved. */
  stream_absorb_p50_ms: 250,
  stream_absorb_p99_ms: 500,
  /** HC9b typing keystroke through the real composer state machine DURING the stream (frame class). */
  typing_keystroke_p99_ms: 16.6,
  /** HC9c Trace scroll frame DURING the stream. */
  scroll_under_load_p99_ms: 100,
  /** HS hot-set ceilings for a one-Focus Horizon: bounded by declared relations, not history. */
  hot_set_subscription_ceiling: 128,
  hot_set_context_ceiling: 128,
  hot_set_query_ceiling: 8,
});

/**
 * Frozen sample counts, warmups, and workload sizes (declared before
 * measurement; never tuned after seeing a number).
 */
export const T21_FROZEN_SAMPLE_COUNTS = Object.freeze({
  cold_start_samples: 5,
  burst_events: 10_000,
  burst_rounds: 3,
  focus_switch_samples: 60,
  focus_switch_warmups: 3,
  hoist_samples: 30,
  dehoist_samples: 30,
  horizon_move_warmups: 2,
  trace_scroll_samples: 60,
  trace_scroll_window: 200,
  code_projection_rows: 5_000,
  code_projection_samples: 10,
  prime_burst_children: 20,
  prime_burst_events_per_child: 50,
  prime_burst_rounds: 3,
  terminal_chunks_per_burst: 200,
  terminal_chunk_bytes: 4_096,
  stream_events: 200,
  stream_scroll_every: 10,
  idle_cpu_window_ms: 1_000,
  /** Realistic event arrival: yield to the event loop every N interactions. */
  yield_every: 5,
});

// --- Frozen fixture ladder (deterministic synthetic Cases; prereg D1/L2) -----------------------

export const T21_CASE_SEED = 0x21ca5e5;

/**
 * A frozen synthetic Case spec. Distinct-object counts and history depth are
 * independent: `unit_update_waves` replays additional full update passes over
 * every unit, so total EVENT volume (the history the horizon must not load)
 * scales without changing the declared relation shape of any single Focus.
 */
export interface T21CaseSpec {
  readonly generation: string;
  readonly units: number;
  /** Evidence records, attached round-robin (unit k % units) — bounded per-unit closure. */
  readonly evidence: number;
  readonly sessions: number;
  /** Session parent chains: ceil(sessions / trees) deep each (deep relation graphs). */
  readonly session_trees: number;
  /** Extra full update passes over all units after creation (history depth). */
  readonly unit_update_waves: number;
  /** Units held permanently blocked (Case-wide attention band at the head of the id space). */
  readonly blocked_units: number;
  readonly focus_events: number;
  readonly seed?: number | undefined;
}

function freezeSpec(spec: T21CaseSpec): T21CaseSpec {
  return Object.freeze({ ...spec });
}

/** Parity/control point (also the deterministic-test fixture). */
export const T21_SMALL_FIXTURE: T21CaseSpec = freezeSpec({
  generation: "gen-t21-small",
  units: 120,
  evidence: 40,
  sessions: 24,
  session_trees: 4,
  unit_update_waves: 0,
  blocked_units: 6,
  focus_events: 0,
  seed: T21_CASE_SEED,
});

/** Mid ladder point: 41,805 events, 10,000 units, same 10x60 session topology as large. */
export const T21_MEDIUM_FIXTURE: T21CaseSpec = freezeSpec({
  generation: "gen-t21-medium",
  units: 10_000,
  evidence: 1_200,
  sessions: 600,
  session_trees: 10,
  unit_update_waves: 3,
  blocked_units: 30,
  focus_events: 5,
  seed: T21_CASE_SEED,
});

/**
 * TARGET fixture: 103,610 events (>= 100k declared scale), 25,000 units,
 * 3,000 evidence, 600 sessions in 10 parent chains 60 deep. Declared before
 * measurement from the static cost shape of the production paths (the O(E*U)
 * relation derivation recorded as T17 adjacent scope bounds E; event volume is
 * bought with update waves, which is the honest long-history shape).
 */
export const T21_LARGE_FIXTURE: T21CaseSpec = freezeSpec({
  generation: "gen-t21-large",
  units: 25_000,
  evidence: 3_000,
  sessions: 600,
  session_trees: 10,
  unit_update_waves: 3,
  blocked_units: 30,
  focus_events: 10,
  seed: T21_CASE_SEED,
});

/** Total authoritative events a spec replays: units*(1+waves) + evidence + sessions + focus. */
export function countT21Events(spec: T21CaseSpec): number {
  return (
    spec.units * (1 + spec.unit_update_waves) + spec.evidence + spec.sessions + spec.focus_events
  );
}

/** Distinct canonical objects a spec projects (the eager whole-world hot-set size). */
export function countT21Objects(spec: T21CaseSpec): number {
  return spec.units + spec.evidence + spec.sessions;
}

/**
 * Deterministic synthetic Case events (T17's LCG generator patterns: seeded
 * state, fixed emission order, stable ids, no Math.random). Emission order:
 * units (blocked band at the head), evidence round-robin, session parent
 * chains, unit update waves (history), focus events last. Wave cursors are
 * unique (`u-cur-w<w>-<i>`), so every wave event is real store work with its
 * own provenance.
 */
export function createT21CaseEvents(spec: T21CaseSpec): AuthoritativeEventFrame[] {
  let state = (spec.seed ?? T21_CASE_SEED) >>> 0;
  const next = (): number => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state;
  };
  const authorities = ["normal", "builder", "orchestrator"] as const;
  const blastRadii = ["isolated", "task", "workspace"] as const;
  const events: AuthoritativeEventFrame[] = [];
  const statusFor = (unit: number, wave: number): string => {
    if (unit < spec.blocked_units) {
      return "blocked"; // the attention band never un-blocks (stable Case-wide attention)
    }
    return (wave + next()) % 2 === 0 ? "active" : "settled";
  };
  const unitEvent = (unit: number, wave: number, cursor: string): AuthoritativeEventFrame => ({
    generation_id: spec.generation,
    source_id: "gauntlet",
    cursor,
    event_type: "unit_updated",
    payload: {
      unit_id: `u-${unit}`,
      title: `Synthetic unit ${unit}`,
      phase: "execution",
      status: statusFor(unit, wave),
      authority: authorities[next() % authorities.length],
      blast_radius: blastRadii[next() % blastRadii.length],
    },
    timestamp_ms: 1000 + events.length,
  });
  for (let i = 0; i < spec.units; i += 1) {
    events.push(unitEvent(i, 0, `u-cur-${i}`));
  }
  for (let k = 0; k < spec.evidence; k += 1) {
    events.push({
      generation_id: spec.generation,
      source_id: "gauntlet",
      cursor: `e-cur-${k}`,
      event_type: "evidence_added",
      payload: {
        evidence_id: `ev-${k}`,
        unit_id: `u-${k % spec.units}`, // round-robin attachment: per-unit closure stays bounded
        kind: "execution_evidence",
        locator: `.agents/reports/synthetic/ev-${k}.json`,
        verified: next() % 2 === 0,
      },
      timestamp_ms: 1000 + events.length,
    });
  }
  const perTree = Math.ceil(spec.sessions / Math.max(1, spec.session_trees));
  let emitted = 0;
  for (let tree = 0; tree < spec.session_trees && emitted < spec.sessions; tree += 1) {
    for (let depth = 0; depth < perTree && emitted < spec.sessions; depth += 1) {
      emitted += 1;
      events.push({
        generation_id: spec.generation,
        source_id: "prime",
        cursor: `s-cur-${tree}-${depth}`,
        event_type: "session_updated",
        payload: {
          session_id: `s-${tree}-${depth}`,
          parent_id: depth === 0 ? null : `s-${tree}-${depth - 1}`,
          model: depth % 2 === 0 ? "model-a" : "model-b",
          status: "running",
        },
        timestamp_ms: 1000 + events.length,
      });
    }
  }
  for (let wave = 1; wave <= spec.unit_update_waves; wave += 1) {
    for (let i = 0; i < spec.units; i += 1) {
      events.push(unitEvent(i, wave, `u-cur-w${wave}-${i}`));
    }
  }
  for (let f = 0; f < spec.focus_events; f += 1) {
    events.push({
      generation_id: spec.generation,
      source_id: "gauntlet",
      cursor: `f-cur-${f}`,
      event_type: "focus_changed",
      payload: { kind: "unit", id: `u-${f % spec.units}` },
      timestamp_ms: 1000 + events.length,
    });
  }
  return events;
}

// --- Scenario registry (what each measured surface is; frozen before measurement) -------------

export type T21BudgetClass = "cold" | "interactive" | "coalesced" | "off-path" | "shape";

export interface T21Scenario {
  readonly id: string;
  /** The bench that measures it: `large-case` or `long-horizon`. */
  readonly bench: "large-case" | "long-horizon";
  /** The production surface exercised (no synthetic stands-in except where declared). */
  readonly surface: string;
  readonly budget_class: T21BudgetClass;
  readonly description: string;
}

export const T21_SCENARIOS: readonly T21Scenario[] = Object.freeze([
  Object.freeze({
    id: "cold-start",
    bench: "large-case",
    surface: "WorkbenchModel + SqliteProjectionStore baseline replay to first ready snapshot",
    budget_class: "cold",
    description: "Fresh model, full 103,610-event replay, markReady, first getSnapshot.",
  }),
  Object.freeze({
    id: "idle-resources",
    bench: "large-case",
    surface: "process RSS + cpuUsage over a quiet window after the baseline",
    budget_class: "coalesced",
    description: "Idle RSS ceiling and idle CPU (<=10% of one core over 1s).",
  }),
  Object.freeze({
    id: "replay-throughput",
    bench: "large-case",
    surface: "WorkbenchModel.applyEvent burst, no render subscriber (catch-up class)",
    budget_class: "coalesced",
    description: "10,000-event sustained bursts + one coalesced render per burst.",
  }),
  Object.freeze({
    id: "focus-switch",
    bench: "large-case",
    surface: "WorkbenchModel.selectFocus with the coordinator plan cached (T20 production path)",
    budget_class: "interactive",
    description: "Cached-plan focus switches; the fold does not re-run.",
  }),
  Object.freeze({
    id: "hoist-dehoist",
    bench: "large-case",
    surface: "WorkbenchModel.hoistToFocus/dehoist (inline fallback fold + off-path worker refresh)",
    budget_class: "interactive",
    description: "Horizon movement through the production model path at target scale.",
  }),
  Object.freeze({
    id: "trace-scroll",
    bench: "large-case",
    surface: "renderTraceView over a 200-record window of the immutable snapshot",
    budget_class: "interactive",
    description: "Scroll frames whose cost must not depend on total history size.",
  }),
  Object.freeze({
    id: "code-projection",
    bench: "large-case",
    surface: "parseSxrLedgerExport + projectSxrEvidenceLens over a 5,000-row pinned ledger",
    budget_class: "coalesced",
    description: "Large code/diff artifact projection through the real SXR adapter.",
  }),
  Object.freeze({
    id: "prime-bursts",
    bench: "large-case",
    surface:
      "session_updated bursts from 20 Prime children interleaved with TerminalSession ingest",
    budget_class: "coalesced",
    description: "Multi-child burst absorption (fakes) + embedded-terminal ingest under load.",
  }),
  Object.freeze({
    id: "stream-typing-scroll",
    bench: "large-case",
    surface:
      "subscribed render consumer + composer keypresses + Trace scroll frames during the stream",
    budget_class: "interactive",
    description: "High-volume unit stream DURING scrolling/typing simulation (prereg P2).",
  }),
  Object.freeze({
    id: "horizon-shape",
    bench: "long-horizon",
    surface: "one-Focus Horizon plans over the frozen size ladder (small/medium/large)",
    budget_class: "shape",
    description:
      "Hot set (subscriptions/queries/context candidates) bounded by declared relations at every size; the eager whole-world shape violates the ceiling by construction (prereg P1).",
  }),
]);

/**
 * The declared relation bound a one-Focus Horizon is held to. The ceiling is a
 * constant INDEPENDENT of Case size: a unit-root horizon closes over its
 * evidence (round-robin attachment bounds it at ceil(evidence/units) <= 1 plus
 * mandatory closure), and a session-root horizon closes over its declared
 * traversal depth. The eager whole-Case plan's subscriptions equal the total
 * distinct object count, which exceeds the ceiling at the SMALLEST ladder size
 * (small: 184 objects > 128) — so an eager whole-world implementation fails the
 * bound by construction (prereg P1; asserted in payment-budgets.test.ts and
 * re-asserted at every ladder size by long-horizon.bench).
 */
export const T21_HOT_SET = Object.freeze({
  subscription_ceiling: T21_FROZEN_BUDGETS.hot_set_subscription_ceiling,
  context_ceiling: T21_FROZEN_BUDGETS.hot_set_context_ceiling,
  query_ceiling: T21_FROZEN_BUDGETS.hot_set_query_ceiling,
});
