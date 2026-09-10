/**
 * `just tui-test all` — the FULL NORMATIVE CONFORMANCE RUN (plan T25 gate;
 * proposed_recipe_contracts: "all selects full conformance and requires every
 * normative suite"; prereg intervention class A2/D1).
 *
 * What this mode does — for real, replacing the long-standing NOT_IMPLEMENTED
 * stub:
 *   1. Build the normative suite registry MECHANICALLY from the frozen plan
 *      (workbench/src/plan-traceability.ts): every `just tui-test <selector>`
 *      gate entry declared by task blocks T00A..T24, each resolved to its suite
 *      file (or to a recorded adjudication — `kernel-serialization` →
 *      `prime-kernel`, per the T24 settlement adjudication). Any selector that
 *      resolves to nothing aborts the run: a missing normative suite is a
 *      conformance failure, never a silent drop (gate_binding_rule: "missing
 *      artifacts ... fail").
 *   2. Run every registered suite as its own `bun test` invocation and show the
 *      verbatim output.
 *   3. Print a per-suite table (pass/fail/skip per suite, owning plan gates)
 *      and FAIL (exit 1) if any normative suite fails or selects zero tests
 *      (gate_binding_rule: "zero selected tests ... fail").
 *
 * What this mode deliberately does NOT do: it does not adjudicate skips. Gated
 * capabilities (opt-in env batteries) and recorded-unqualified limitations stay
 * visible here as per-suite skip counts; their named, evidence-backed
 * adjudication — the rule that an unbacked REQUIRED skip fails — is
 * `just tui-preflight conformance` (REQ-CONFIG-011/012).
 *
 * Exit codes: 0 = every normative suite ran green; 1 = a suite failed or
 * selected zero tests; 2 = the registry itself could not be built.
 */

import {
  loadPlanDocument,
  type NormativeSuiteRow,
  normativeSuiteRegistry,
  PLAN_PATH,
  type PlanDocument,
  WORKBENCH_ROOT,
} from "./plan-traceability";

interface SuiteResult {
  readonly file: string;
  readonly selectors: string;
  readonly tasks: string;
  readonly pass: number;
  readonly fail: number;
  readonly skip: number;
  readonly exitCode: number;
  readonly adjudicated: boolean;
}

function parseSummary(output: string): { pass: number; fail: number; skip: number } {
  let pass = 0;
  let fail = 0;
  let skip = 0;
  for (const match of output.matchAll(/^\s*(\d+) (pass|fail|skip)\s*$/gm)) {
    const count = Number.parseInt(match[1] ?? "0", 10);
    const kind = match[2];
    if (kind === "pass") pass = count;
    else if (kind === "fail") fail = count;
    else if (kind === "skip") skip = count;
  }
  return { pass, fail, skip };
}

/** Load the frozen plan or abort with exit 2 (a plan that cannot be parsed is not conformance). */
function loadPlanOrExit(): PlanDocument {
  try {
    return loadPlanDocument();
  } catch (error) {
    console.error(`tui-test all: FAIL — the frozen plan could not be parsed: ${String(error)}`);
    return process.exit(2);
  }
}

/** Build the registry or abort with exit 2 (a missing normative suite is a conformance failure). */
function buildRegistryOrExit(plan: PlanDocument): readonly NormativeSuiteRow[] {
  try {
    return normativeSuiteRegistry(plan).rows;
  } catch (error) {
    console.error(`tui-test all: FAIL — ${String(error)}`);
    return process.exit(2);
  }
}

// --- 1. Registry -----------------------------------------------------------------

const plan = loadPlanOrExit();
const rows = buildRegistryOrExit(plan);
console.log("tui-test all: full normative conformance run (plan T25 gate).");
console.log(`  plan: ${PLAN_PATH}`);
console.log("  inclusion rule: a suite is normative iff it resolves a `just tui-test <selector>`");
console.log("  gate entry of a plan task block T00A..T24 (T25's gate is this run itself).");
console.log(`  registry: ${rows.length} normative suites.`);

// --- 2. Run every registered suite ------------------------------------------------

const results: SuiteResult[] = [];
for (const row of rows) {
  const selectors = [...new Set(row.claims.map((claim) => claim.selector))].sort().join(", ");
  const tasks = [...new Set(row.claims.map((claim) => claim.task))].sort().join("/");
  console.log("");
  console.log(
    `=== tui-test all: running ${row.file} (selector: ${selectors}; plan gates: ${tasks}) ===`,
  );
  const proc = Bun.spawnSync(["bun", "test", row.file], {
    cwd: WORKBENCH_ROOT,
    stdout: "pipe",
    stderr: "pipe",
    env: process.env,
  });
  if (proc.stdout !== null && proc.stdout.byteLength > 0) {
    process.stdout.write(proc.stdout);
  }
  if (proc.stderr !== null && proc.stderr.byteLength > 0) {
    process.stderr.write(proc.stderr);
  }
  const decoder = new TextDecoder();
  const output =
    decoder.decode(proc.stdout ?? new Uint8Array()) +
    decoder.decode(proc.stderr ?? new Uint8Array());
  const summary = parseSummary(output);
  results.push({
    file: row.file,
    selectors,
    tasks,
    pass: summary.pass,
    fail: summary.fail,
    skip: summary.skip,
    exitCode: proc.exitCode ?? -1,
    adjudicated: row.adjudicated,
  });
}

// --- 3. Per-suite table + verdict ---------------------------------------------------

console.log("");
console.log("=== tui-test all: per-suite conformance table ===");
console.log(
  "RESULT".padEnd(9),
  "PASS".padStart(6),
  "FAIL".padStart(6),
  "SKIP".padStart(6),
  "EXIT".padStart(5),
  "SUITE",
);
let totalPass = 0;
let totalFail = 0;
let totalSkip = 0;
const failures: string[] = [];
for (const result of results) {
  const zeroTests = result.pass + result.fail + result.skip === 0;
  const outcome =
    result.exitCode !== 0 || result.fail > 0
      ? "FAIL"
      : zeroTests
        ? "NO-TESTS"
        : result.adjudicated
          ? "PASS*"
          : "PASS";
  console.log(
    outcome.padEnd(9),
    String(result.pass).padStart(6),
    String(result.fail).padStart(6),
    String(result.skip).padStart(6),
    String(result.exitCode).padStart(5),
    `${result.file}  [${result.tasks}]`,
  );
  totalPass += result.pass;
  totalFail += result.fail;
  totalSkip += result.skip;
  if (outcome !== "PASS" && outcome !== "PASS*") {
    failures.push(
      `${result.file}: ${outcome} (exit ${result.exitCode}, ${result.fail} failed, selectors ${result.selectors})`,
    );
  }
}
console.log("");
console.log(
  `suites: ${results.length}, tests: ${totalPass} pass / ${totalFail} fail / ${totalSkip} skip. ` +
    "`PASS*` = suite reached through a recorded plan-selector adjudication (kernel-serialization → prime-kernel).",
);
console.log(
  "Skips are gated/recorded capabilities and stay visible by design; their named, evidence-backed " +
    "adjudication is `just tui-preflight conformance`.",
);

if (failures.length > 0) {
  console.error("tui-test all: FAIL — failing or empty normative suites:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exitCode = 1;
} else {
  console.log("tui-test all: PASS — every registered normative suite ran green.");
}
