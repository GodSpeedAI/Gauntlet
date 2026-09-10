import { afterAll, describe, expect, it } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  ADJUDICATED_SUITE_ALIASES,
  EVIDENCE_ROOT,
  loadPlanDocument,
  normativeSuiteRegistry,
  PLAN_PATH,
  type PlanDocument,
  REPO_ROOT,
  settledEvidenceFor,
} from "./plan-traceability";
import { assertNoRoadmapSurfaces, ROADMAP_VIEW_IDS, registerShellView } from "./shell";
import { defaultViewRegistry } from "./views-chat-trace";

/**
 * T25 mechanical coverage recomputation (plan T25 done_when: "All 104 indexed
 * normative requirements, all mandatory section-path clauses, and all fifteen
 * DSN design clauses have settled evidence"; prereg D1/P1).
 *
 * This suite recomputes requirement and design-clause coverage MECHANICALLY
 * from the frozen plan file and fails on any genuinely missing coverage:
 *   1. Every REQ in `traceability.requirement_to_tasks` and every DSN in
 *      `verification.design_clause_owners` is sourced DIRECTLY from the plan
 *      YAML (parsed by plan-traceability.ts — no duplicated mapping).
 *   2. Every owning task must carry settled evidence: an artifact under
 *      evidence/<task>/ whose content includes a `Confirmation:` line.
 *   3. DESIGN.md and the bound spec must still match the plan's pinned
 *      sha256 hashes (the same tripwire `tui-preflight` applies).
 *   4. DSN-015 (no roadmap surface) delegates to the existing guards: the
 *      renderer-boundary + opentui-shell suites must be registered normative
 *      suites, and the live shell registry scan must refuse every roadmap id.
 *
 * Negative controls pin the settled-evidence checker itself: a missing
 * directory, a Confirmation-less artifact, and a proper artifact are
 * distinguished, so the coverage check cannot pass vacuously.
 */

const plan: PlanDocument = loadPlanDocument();
const registry = normativeSuiteRegistry(plan);

// --- Negative controls: the settled-evidence checker is load-bearing ----------

describe("T25 settled-evidence checker (negative controls)", () => {
  const tmpRoots: string[] = [];

  afterAll(() => {
    for (const root of tmpRoots) rmSync(root, { recursive: true, force: true });
  });

  it("fails a task with no evidence directory", () => {
    const verdict = settledEvidenceFor("TNOSUCH", mkdtempSync(join(tmpdir(), "t25-coverage-")));
    expect(verdict.settled).toBe(false);
    expect(verdict.detail).toContain("no evidence directory");
  });

  it("fails a task whose only artifact carries no Confirmation line", () => {
    const root = mkdtempSync(join(tmpdir(), "t25-coverage-"));
    const dir = join(root, "TGHOST");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "artifact.md"), "some prose, no confirmation record\n");
    const verdict = settledEvidenceFor("TGHOST", root);
    expect(verdict.settled).toBe(false);
    expect(verdict.detail).toContain("no Confirmation: line");
  });

  it("passes a task whose artifact carries a Confirmation line", () => {
    const root = mkdtempSync(join(tmpdir(), "t25-coverage-"));
    const dir = join(root, "TREAL");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, "artifact.md"),
      "prose\nConfirmation: builder CONFIRM (all gates green; teeth load-bearing).\n",
    );
    const verdict = settledEvidenceFor("TREAL", root);
    expect(verdict.settled).toBe(true);
    expect(verdict.confirmationLine).toContain("builder CONFIRM");
  });
});

// --- Plan integrity: the parser read the real frozen mapping -------------------

describe("T25 plan coverage recomputation (104 REQs, 15 DSNs)", () => {
  it("parsed the plan's declared counts exactly", () => {
    expect(plan.requirementToTasks.size).toBe(plan.specRequirementCount);
    expect(plan.requirementToTasks.size).toBe(plan.mappedRequirementCount);
    expect(plan.designClauseOwners.size).toBe(plan.designClauseCount);
    expect(plan.designClauseOwners.size).toBe(plan.mappedDesignClauseCount);
    expect(plan.unmappedRequirements).toEqual([]);
  });

  it("maps every REQ to known plan tasks with settled evidence", () => {
    const problems: string[] = [];
    for (const [req, tasks] of plan.requirementToTasks) {
      if (!/^REQ-[A-Z0-9]+-\d{3}$/.test(req)) problems.push(`${req}: malformed requirement id`);
      if (tasks.length === 0) {
        problems.push(`${req}: mapped to no task`);
        continue;
      }
      for (const task of tasks) {
        if (!plan.tasks.has(task)) {
          problems.push(`${req}: owner ${task} is not a plan task`);
          continue;
        }
        const evidence = settledEvidenceFor(task);
        if (!evidence.settled)
          problems.push(`${req}: owner ${task} lacks settled evidence (${evidence.detail})`);
      }
    }
    expect(problems).toEqual([]);
  });

  it("maps every DSN clause to known plan tasks with settled evidence", () => {
    const problems: string[] = [];
    for (let index = 1; index <= plan.designClauseCount; index++) {
      const clause = `DSN-${String(index).padStart(3, "0")}`;
      const owners = plan.designClauseOwners.get(clause);
      if (owners === undefined) {
        problems.push(`${clause}: missing from design_clause_owners`);
        continue;
      }
      if (owners.length === 0) {
        problems.push(`${clause}: mapped to no task`);
        continue;
      }
      for (const task of owners) {
        if (!plan.tasks.has(task)) {
          problems.push(`${clause}: owner ${task} is not a plan task`);
          continue;
        }
        const evidence = settledEvidenceFor(task);
        if (!evidence.settled)
          problems.push(`${clause}: owner ${task} lacks settled evidence (${evidence.detail})`);
      }
    }
    expect(problems).toEqual([]);
  });
});

// --- Source authority: bound hashes still match (preflight hash rule) ----------

describe("T25 source-authority hash re-verification", () => {
  function sha256File(path: string): string {
    return createHash("sha256").update(readFileSync(path)).digest("hex");
  }

  it("DESIGN.md still matches the plan's pinned source.design.sha256", () => {
    expect(sha256File(join(REPO_ROOT, "DESIGN.md"))).toBe(plan.sourceDesignSha256);
  });

  it("the bound spec still matches the plan's pinned source.spec.sha256", () => {
    const specPath = join(REPO_ROOT, ".agents", "specs", "gauntlet-tui-prime-workbench.spec.yaml");
    expect(existsSync(specPath)).toBe(true);
    expect(sha256File(specPath)).toBe(plan.sourceSpecSha256);
  });

  it("the scanned plan file and evidence root exist", () => {
    expect(existsSync(PLAN_PATH)).toBe(true);
    expect(existsSync(EVIDENCE_ROOT)).toBe(true);
  });
});

// --- DSN-015: no roadmap surface (delegated guards + live registry scan) -------

describe("T25 DSN-015 roadmap-surface recomputation", () => {
  it("keeps the delegated guards (renderer-boundary, opentui-shell) in the normative registry", () => {
    const files = new Set(registry.rows.map((row) => row.file));
    expect(files.has("src/renderer-boundary.test.ts")).toBe(true);
    expect(files.has("src/opentui-shell.test.ts")).toBe(true);
  });

  it("carries the recorded selector adjudications explicitly", () => {
    for (const alias of ADJUDICATED_SUITE_ALIASES.values()) {
      expect(alias.file.startsWith("src/")).toBe(true);
      expect(alias.record.length).toBeGreaterThan(40);
    }
  });

  it("scans the live shell registry: no roadmap surface is materializable", () => {
    const views = defaultViewRegistry();
    for (const roadmapId of ROADMAP_VIEW_IDS) {
      expect(Object.keys(views)).not.toContain(roadmapId);
      expect(() => registerShellView(views, roadmapId as never, views.work)).toThrow(/roadmap/);
    }
    expect(() => assertNoRoadmapSurfaces(views)).not.toThrow();
  });
});
