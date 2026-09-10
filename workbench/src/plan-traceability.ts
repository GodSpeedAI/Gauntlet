/**
 * Mechanical plan registry for T25 whole-system conformance (plan T25;
 * REQ-VERIFY-001, REQ-GOAL-002; prereg D1).
 *
 * This module is the SINGLE mechanical source for two T25 consumers:
 *   1. `tui-test all` (src/tui-test-all.ts) — the full normative conformance run.
 *   2. `coverage-recomputation.test.ts` — the 104-REQ / 15-DSN settled-evidence index.
 *
 * It parses the FROZEN plan file
 * (.agents/plans/gauntlet-tui-prime-workbench.plan.yaml) directly — the plan is
 * the ruler; nothing here duplicates or restates its mappings. The parser is a
 * deliberately small YAML SUBSET scanner (nested mappings, block sequences —
 * including sequences indented at their parent key — inline flow lists
 * `[a, b]`, plain/quoted scalars, `>`/`|` block scalars, plain multi-line
 * scalar continuation, full-line comments). The plan uses exactly this subset.
 * Every consumer re-asserts the plan's own declared counts
 * (traceability.spec_requirement_count / design_clause_count) after parsing, so
 * a silently rotting parser fails loudly instead of passing a fake index.
 *
 * NORMATIVE SUITE INCLUSION RULE (mechanical, per the T25 contract):
 *   A `bun test` suite file is a REGISTERED NORMATIVE SUITE iff it is the
 *   resolution of a `just tui-test <selector>` gate entry declared by a task
 *   block of the frozen plan, T00A through T24 inclusive. T25 is excluded
 *   because its gate entry IS this conformance run (self-referential).
 *   Consequences, decided by the rule rather than by taste:
 *     - `src/index.test.ts`, the `src/testing/*` helpers (including
 *       canonical-surface-stub.ts and cross-surface-matrix.ts), the gated
 *       real-PTY battery file `operator-journeys-pty.test.ts`, and the other
 *       T-task instrument suites (payment-budgets, planner-swap,
 *       release-manifest, substrate-inventory, worker-folds,
 *       inspector-lens-wiring) are NOT registry rows: no plan gate names them.
 *       They remain in the tree and run under a bare `just tui-test`.
 *     - `canonical-boundary` IS a registry row: the settled P3 T01 gate names
 *       it explicitly; excluding a plan-declared gate selector would move the
 *       ruler.
 *   Selector resolution: `src/<selector>.test.ts` must exist, or the selector
 *   must be covered by exactly one RECORDED adjudication (below). Anything
 *   else is a MISSING normative suite and fails the registry build.
 *
 * RECORDED SELECTOR ADJUDICATION (frozen plan text is NOT edited):
 *   `kernel-serialization` → `src/prime-kernel.test.ts`. The plan declares
 *   `just tui-test kernel-serialization` for T05 and T24, but no file of that
 *   name exists; the T05 suite is `prime-kernel.test.ts` (semantic content
 *   identical: serialization without interleaving + liveness != success). The
 *   orchestrator adjudicated `prime-kernel` as the canonical selector at T24
 *   settlement (current-status.yaml, 2026-09-09; T24 evidence honest-gap 1).
 *   The divergence is carried here explicitly instead of being papered over.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// --- Repository layout anchors (relative to this file: workbench/src/) --------

/** Repository root, resolved from this module's location — never from cwd. */
export const REPO_ROOT = join(import.meta.dir, "..", "..");
export const WORKBENCH_ROOT = join(import.meta.dir, "..");
export const PLAN_PATH = join(
  REPO_ROOT,
  ".agents",
  "plans",
  "gauntlet-tui-prime-workbench.plan.yaml",
);
export const EVIDENCE_ROOT = join(
  REPO_ROOT,
  ".agents",
  "reports",
  "gauntlet-tui-prime-workbench",
  "evidence",
);

/** The task whose gate declares the conformance run itself (never a row source). */
export const CONFORMANCE_TASK = "T25";

// --- Minimal YAML-subset scanner ----------------------------------------------

type YamlValue = string | YamlMapping | YamlSequence;
interface YamlMapping {
  readonly kind: "map";
  readonly entries: Map<string, YamlValue | null>;
}
interface YamlSequence {
  readonly kind: "seq";
  readonly items: YamlValue[];
}

const BLOCK_SCALAR_MARKERS = new Set([">", ">-", ">+", "|", "|-", "|+"]);

interface Line {
  readonly indent: number;
  readonly text: string; // trimmed content
}

function scanLines(source: string): Line[] {
  const lines: Line[] = [];
  for (const raw of source.split("\n")) {
    if (raw.trim().length === 0) continue;
    const trimmed = raw.trimStart();
    if (trimmed.startsWith("#")) continue;
    const indent = raw.length - trimmed.length;
    if (raw.includes("\t")) {
      throw new Error(`plan parser: tab indentation is not valid YAML: ${JSON.stringify(raw)}`);
    }
    lines.push({ indent, text: trimmed });
  }
  return lines;
}

/** Split a mapping line into key and value remainder; null when not a mapping line. */
function splitMapEntry(text: string): { key: string; value: string } | null {
  // The key is everything lazy-up to the first colon; this plan's keys are
  // simple identifiers, but some embed brackets, angle brackets, slashes,
  // semicolons, or spaces (`tui-test [selector]:`, `tui-preflight bootstrap:`,
  // `sources/core_authority; substrate_baseline; claim_discipline:`). Prose
  // never reaches here: block-scalar bodies and plain-scalar continuations are
  // consumed before line classification.
  const match = /^("[^"]*"|'[^']*'|[^:]+?):(?:[ \t]+(.*))?$/.exec(text);
  if (match === null) return null;
  const rawKey = (match[1] ?? "").trim();
  if (rawKey.length === 0) return null;
  const key = rawKey.startsWith('"') || rawKey.startsWith("'") ? rawKey.slice(1, -1) : rawKey;
  return { key, value: (match[2] ?? "").trim() };
}

function splitSeqItem(text: string): string | null {
  const match = /^-(?:[ \t]+(.*))?$/.exec(text);
  if (match === null) return null;
  return (match[1] ?? "").trim();
}

/** Parse an inline flow list `[a, b]`; null when the text is not a flow list. */
function parseFlowList(text: string): string[] | null {
  if (!(text.startsWith("[") && text.endsWith("]"))) return null;
  const inner = text.slice(1, -1).trim();
  if (inner.length === 0) return [];
  return inner.split(",").map((part) => stripQuotes(part.trim()));
}

function stripQuotes(text: string): string {
  if (
    text.length >= 2 &&
    ((text.startsWith('"') && text.endsWith('"')) || (text.startsWith("'") && text.endsWith("'")))
  ) {
    return text.slice(1, -1);
  }
  return text;
}

interface MapFrame {
  readonly kind: "map";
  readonly indent: number;
  readonly value: YamlMapping;
  /** Pending `key:` with an empty value waiting to see what indents under it. */
  pendingKey: { key: string; indent: number } | null;
}

interface SeqFrame {
  readonly kind: "seq";
  readonly indent: number;
  readonly value: YamlSequence;
}

type Frame = MapFrame | SeqFrame;

/**
 * Parse the YAML subset into a tree. Scalars stay strings; the typed accessors
 * below convert what they need and throw on shape surprises.
 */
export function parseYamlSubset(source: string): YamlMapping {
  const lines = scanLines(source);
  const root: YamlMapping = { kind: "map", entries: new Map() };
  const stack: Frame[] = [{ kind: "map", indent: -1, value: root, pendingKey: null }];

  // Continuation state for a plain multi-line scalar (or a block scalar body):
  // lines deeper than `indent` belong to that scalar until dedent.
  let continuationIndent = -1;

  const top = (): Frame => {
    const frame = stack[stack.length - 1];
    if (frame === undefined) throw new Error("plan parser: frame stack underflow");
    return frame;
  };

  const closePending = (): void => {
    const frame = top();
    if (frame.kind === "map" && frame.pendingKey !== null) {
      frame.value.entries.set(frame.pendingKey.key, null);
      frame.pendingKey = null;
    }
  };

  const attachValue = (key: string, value: YamlValue | null): void => {
    const frame = top();
    if (frame.kind !== "map") {
      throw new Error(`plan parser: scalar/flow value outside a mapping: ${key}`);
    }
    frame.value.entries.set(key, value);
  };

  const openPendingAs = (kind: "map" | "seq", indent: number): void => {
    const frame = top();
    if (frame.kind !== "map" || frame.pendingKey === null) {
      throw new Error("plan parser: cannot open container without a pending key");
    }
    if (kind === "map") {
      const container: YamlMapping = { kind: "map", entries: new Map() };
      frame.value.entries.set(frame.pendingKey.key, container);
      frame.pendingKey = null;
      stack.push({ kind: "map", indent, value: container, pendingKey: null });
    } else {
      const container: YamlSequence = { kind: "seq", items: [] };
      frame.value.entries.set(frame.pendingKey.key, container);
      frame.pendingKey = null;
      stack.push({ kind: "seq", indent, value: container });
    }
  };

  for (const line of lines) {
    const { indent, text } = line;

    // Block-scalar / plain-scalar continuation: deeper (or blank-adjacent)
    // lines belong to the scalar; this scanner discards the prose body.
    if (continuationIndent > -1 && indent > continuationIndent) continue;
    continuationIndent = -1;

    // Close frames that are strictly deeper than this line.
    while (stack.length > 1 && top().indent > indent) {
      closePending();
      stack.pop();
    }

    const seqContent = splitSeqItem(text);
    const mapEntry = seqContent === null ? splitMapEntry(text) : null;

    if (mapEntry !== null) {
      // A mapping entry at the same indent closes an open sequence frame there.
      while (stack.length > 1 && top().indent === indent && top().kind === "seq") {
        stack.pop();
      }
      const frame = top();
      if (frame.kind === "map" && frame.pendingKey !== null) {
        if (indent > frame.pendingKey.indent) {
          // `key:` followed by a deeper `subkey:` — the pending key opens a
          // nested mapping that this entry belongs to.
          openPendingAs("map", indent);
        } else {
          // `key:` followed by an entry at its own indent — it had no value.
          closePending();
        }
      }
      if (mapEntry.value.length === 0) {
        // `key:` — value arrives below (nested map, sequence at same indent,
        // or nothing at all).
        const parent = top();
        if (parent.kind !== "map") {
          throw new Error(`plan parser: mapping key inside a sequence: ${mapEntry.key}`);
        }
        parent.pendingKey = { key: mapEntry.key, indent };
        continue;
      }
      if (BLOCK_SCALAR_MARKERS.has(mapEntry.value)) {
        attachValue(mapEntry.key, ""); // body is prose; recorded as empty scalar
        continuationIndent = indent;
        continue;
      }
      const flow = parseFlowList(mapEntry.value);
      if (flow !== null) {
        attachValue(
          mapEntry.key,
          flow.length === 0 ? { kind: "seq", items: [] } : { kind: "seq", items: [...flow] },
        );
      } else {
        attachValue(mapEntry.key, stripQuotes(mapEntry.value));
        continuationIndent = indent; // plain scalar; deeper lines continue it
      }
      continue;
    }

    if (seqContent !== null) {
      const frame = top();
      if (frame.kind === "map" && frame.pendingKey !== null && indent >= frame.pendingKey.indent) {
        openPendingAs("seq", indent);
      } else if (frame.kind !== "seq" || frame.indent !== indent) {
        throw new Error(`plan parser: orphan sequence item at indent ${indent}: ${text}`);
      }
      const seqFrame = top();
      if (seqFrame.kind !== "seq") throw new Error("plan parser: internal seq frame corruption");
      const flow = parseFlowList(seqContent);
      if (flow !== null) {
        seqFrame.value.items.push({ kind: "seq", items: [...flow] });
      } else if (seqContent.length > 0) {
        seqFrame.value.items.push(stripQuotes(seqContent));
        continuationIndent = indent;
      } else {
        seqFrame.value.items.push(""); // bare "-"
      }
      continue;
    }

    throw new Error(`plan parser: unrecognized line at indent ${indent}: ${text}`);
  }
  while (stack.length > 1) {
    closePending();
    stack.pop();
  }
  closePending();
  return root;
}

// --- Typed accessors over the parsed plan --------------------------------------

export interface PlanTask {
  readonly id: string;
  readonly title: string;
  readonly gate: readonly string[];
}

export interface PlanDocument {
  readonly tasks: ReadonlyMap<string, PlanTask>;
  readonly requirementToTasks: ReadonlyMap<string, readonly string[]>;
  readonly designClauseOwners: ReadonlyMap<string, readonly string[]>;
  readonly specRequirementCount: number;
  readonly mappedRequirementCount: number;
  readonly designClauseCount: number;
  readonly mappedDesignClauseCount: number;
  readonly unmappedRequirements: readonly string[];
  readonly sourceSpecSha256: string;
  readonly sourceDesignSha256: string;
}

function requireMap(value: YamlValue | null | undefined, path: string): YamlMapping {
  if (typeof value !== "object" || value === null || value.kind !== "map") {
    throw new Error(`plan parser: ${path} is not a mapping`);
  }
  return value;
}

function requireSeq(value: YamlValue | null | undefined, path: string): readonly string[] {
  if (typeof value !== "object" || value === null || value.kind !== "seq") {
    throw new Error(`plan parser: ${path} is not a sequence`);
  }
  return value.items.map((item) => {
    if (typeof item !== "string") throw new Error(`plan parser: ${path} has a non-scalar item`);
    return item;
  });
}

function requireString(value: YamlValue | null | undefined, path: string): string {
  if (typeof value !== "string") throw new Error(`plan parser: ${path} is not a scalar`);
  return value;
}

function requireInt(value: YamlValue | null | undefined, path: string): number {
  const text = requireString(value, path);
  const parsed = Number.parseInt(text, 10);
  if (!Number.isFinite(parsed)) throw new Error(`plan parser: ${path} is not an integer: ${text}`);
  return parsed;
}

export function parsePlanDocument(source: string): PlanDocument {
  const root = parseYamlSubset(source);

  const tasksRoot = requireMap(root.entries.get("tasks"), "tasks");
  const tasks = new Map<string, PlanTask>();
  for (const [id, node] of tasksRoot.entries) {
    const body = requireMap(node, `tasks.${id}`);
    tasks.set(id, {
      id,
      title: requireString(body.entries.get("title"), `tasks.${id}.title`),
      gate:
        body.entries.get("gate") === null
          ? []
          : requireSeq(body.entries.get("gate"), `tasks.${id}.gate`),
    });
  }

  const traceability = requireMap(root.entries.get("traceability"), "traceability");
  const requirementToTasks = new Map<string, readonly string[]>();
  const reqRoot = requireMap(
    traceability.entries.get("requirement_to_tasks"),
    "requirement_to_tasks",
  );
  for (const [req, node] of reqRoot.entries) {
    requirementToTasks.set(
      req,
      node === null ? [] : requireSeq(node, `requirement_to_tasks.${req}`),
    );
  }

  // DSN clause ownership lives under `verification:` in the plan.
  const verification = requireMap(root.entries.get("verification"), "verification");
  const ownersRoot = requireMap(
    verification.entries.get("design_clause_owners"),
    "verification.design_clause_owners",
  );
  const designClauseOwners = new Map<string, readonly string[]>();
  for (const [clause, node] of ownersRoot.entries) {
    if (clause === "rule") continue; // prose key, not a clause row
    designClauseOwners.set(
      clause,
      node === null ? [] : requireSeq(node, `design_clause_owners.${clause}`),
    );
  }

  return {
    tasks,
    requirementToTasks,
    designClauseOwners,
    specRequirementCount: requireInt(
      traceability.entries.get("spec_requirement_count"),
      "spec_requirement_count",
    ),
    mappedRequirementCount: requireInt(
      traceability.entries.get("mapped_requirement_count"),
      "mapped_requirement_count",
    ),
    designClauseCount: requireInt(
      traceability.entries.get("design_clause_count"),
      "design_clause_count",
    ),
    mappedDesignClauseCount: requireInt(
      traceability.entries.get("mapped_design_clause_count"),
      "mapped_design_clause_count",
    ),
    unmappedRequirements: requireSeq(
      traceability.entries.get("unmapped_requirements"),
      "unmapped_requirements",
    ),
    sourceSpecSha256: requireString(
      requireMap(
        requireMap(root.entries.get("source"), "source").entries.get("spec"),
        "source.spec",
      ).entries.get("sha256"),
      "source.spec.sha256",
    ),
    sourceDesignSha256: requireString(
      requireMap(
        requireMap(root.entries.get("source"), "source").entries.get("design"),
        "source.design",
      ).entries.get("sha256"),
      "source.design.sha256",
    ),
  };
}

export function loadPlanDocument(): PlanDocument {
  return parsePlanDocument(readFileSync(PLAN_PATH, "utf8"));
}

// --- Normative suite registry ---------------------------------------------------

export interface SuiteClaim {
  readonly task: string;
  readonly selector: string;
}

export interface NormativeSuiteRow {
  /** Resolved suite file, relative to the workbench package root (POSIX). */
  readonly file: string;
  readonly claims: readonly SuiteClaim[];
  /** True when resolution went through a recorded adjudication, not a direct file. */
  readonly adjudicated: boolean;
}

/** Recorded selector divergences (see module header). Plan text is NOT edited. */
export const ADJUDICATED_SUITE_ALIASES: ReadonlyMap<
  string,
  { readonly file: string; readonly record: string }
> = new Map([
  [
    "kernel-serialization",
    {
      file: "src/prime-kernel.test.ts",
      record:
        "Orchestrator adjudication at T24 settlement (2026-09-09, current-status.yaml + T24 evidence " +
        "honest-gap 1): plan gate selector `just tui-test kernel-serialization` matches no test file; " +
        "the T05 suite `prime-kernel.test.ts` (semantic content identical: serialization without " +
        "interleaving + liveness != success) is the canonical selector; the frozen plan text is NOT edited.",
    },
  ],
  [
    "kernel-object-lifecycle",
    {
      file: "src/prime-kernel.test.ts",
      record:
        "Same recorded divergence class, carried to T05's second plan selector (T25 registry build, " +
        "2026-09-09): plan gate selector `just tui-test kernel-object-lifecycle` matches no test file; " +
        "T05's own settled evidence (evidence/T05/prime-kernel.md gate table) executed `just tui-test " +
        "prime-kernel` (5/5 PASS) as its gate, and prime-kernel.test.ts contains the object-lifecycle " +
        'content ("transitions object lifecycle: ephemeral -> pinned -> artifactized"). The frozen ' +
        "plan text is NOT edited; the divergence is recorded here and in the T25 evidence.",
    },
  ],
]);

const TUI_TEST_GATE = /^just tui-test (.+)$/;

/** All task ids whose gate entries may declare normative suites (T00A..T24). */
export function registrySourceTasks(plan: PlanDocument): readonly string[] {
  return [...plan.tasks.keys()].filter((id) => id !== CONFORMANCE_TASK);
}

/**
 * Build the normative suite registry from the frozen plan. Throws (listing the
 * unresolved selectors) when any declared selector has no suite — a MISSING
 * normative suite is a conformance failure, never silently dropped.
 */
export function normativeSuiteRegistry(
  plan: PlanDocument,
  workbenchRoot: string = WORKBENCH_ROOT,
): {
  readonly rows: readonly NormativeSuiteRow[];
  readonly adjudicationsUsed: readonly { readonly selector: string; readonly record: string }[];
} {
  const byFile = new Map<string, { claims: SuiteClaim[]; adjudicated: boolean }>();
  const adjudicationsUsed: { selector: string; record: string }[] = [];

  for (const taskId of registrySourceTasks(plan)) {
    const task = plan.tasks.get(taskId);
    if (task === undefined) throw new Error(`registry: plan task ${taskId} vanished`);
    for (const gate of task.gate) {
      const match = TUI_TEST_GATE.exec(gate);
      if (match === null) continue; // not a tui-test suite declaration
      const selector = (match[1] ?? "").trim();
      if (selector.length === 0) {
        // Bare `just tui-test` selects "all currently implemented suites"; it
        // declares no specific suite, so it contributes no registry row.
        continue;
      }
      const direct = `src/${selector}.test.ts`;
      let file: string;
      let adjudicated = false;
      if (existsSync(join(workbenchRoot, direct))) {
        file = direct;
      } else {
        const alias = ADJUDICATED_SUITE_ALIASES.get(selector);
        if (alias === undefined || !existsSync(join(workbenchRoot, alias.file))) {
          throw new Error(
            `registry: plan ${taskId} gate 'just tui-test ${selector}' resolves to no normative suite ` +
              `(no ${direct}, no recorded adjudication) — MISSING normative suite`,
          );
        }
        file = alias.file;
        adjudicated = true;
        if (!adjudicationsUsed.some((entry) => entry.selector === selector)) {
          adjudicationsUsed.push({ selector, record: alias.record });
        }
      }
      const row = byFile.get(file);
      if (row === undefined) {
        byFile.set(file, { claims: [{ task: taskId, selector }], adjudicated });
      } else {
        row.claims.push({ task: taskId, selector });
        row.adjudicated = row.adjudicated || adjudicated;
      }
    }
  }

  const rows = [...byFile.entries()]
    .map(([file, row]) => ({
      file,
      claims: row.claims,
      adjudicated: row.adjudicated,
    }))
    .sort((a, b) => (a.file < b.file ? -1 : a.file > b.file ? 1 : 0));
  return { rows, adjudicationsUsed };
}

// --- Settled evidence lookup -----------------------------------------------------

export interface SettledEvidence {
  readonly settled: boolean;
  readonly artifact: string | null;
  readonly confirmationLine: string | null;
  readonly detail: string;
}

/**
 * A task's evidence is SETTLED-for-index-purposes iff its evidence directory
 * exists and carries at least one markdown artifact whose content includes a
 * `Confirmation:` line (the builder's evidence confirmation record; the
 * independent verdicts themselves are anchored in the settlement records).
 * The negative-control tests in coverage-recomputation.test.ts pin this
 * checker's failure behavior so it cannot pass vacuously.
 */
export function settledEvidenceFor(
  taskId: string,
  evidenceRoot: string = EVIDENCE_ROOT,
): SettledEvidence {
  const dir = join(evidenceRoot, taskId);
  if (!existsSync(dir)) {
    return {
      settled: false,
      artifact: null,
      confirmationLine: null,
      detail: `no evidence directory ${dir}`,
    };
  }
  const artifacts = readdirSync(dir)
    .filter((name) => name.endsWith(".md"))
    .sort();
  if (artifacts.length === 0) {
    return {
      settled: false,
      artifact: null,
      confirmationLine: null,
      detail: `no .md artifact in ${dir}`,
    };
  }
  let confirmed: { artifact: string; line: string } | null = null;
  for (const name of artifacts) {
    const text = readFileSync(join(dir, name), "utf8");
    const match = /^Confirmation:.*$/m.exec(text);
    if (match !== null) {
      confirmed = { artifact: `${taskId}/${name}`, line: match[0].trim() };
      break;
    }
  }
  if (confirmed === null) {
    return {
      settled: false,
      artifact: artifacts[0] ?? null,
      confirmationLine: null,
      detail: `no Confirmation: line in any artifact under ${dir}`,
    };
  }
  return {
    settled: true,
    artifact: confirmed.artifact,
    confirmationLine: confirmed.line,
    detail: `${confirmed.artifact}: ${confirmed.line}`,
  };
}
