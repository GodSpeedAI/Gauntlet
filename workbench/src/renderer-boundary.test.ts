import { describe, expect, it } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * Renderer-boundary architecture tests (plan T19 step 4, REQ-GOAL-005,
 * REQ-TUI-005, REQ-SAFE-001, DSN-011): Gauntlet/Prime canonical layers cannot
 * import OpenTUI/React presentation modules.
 *
 * How the boundary is derived (no hand-maintained allowlist of state files):
 *   1. Every `src/*.ts(x)` file is enumerated from the filesystem. Tests
 *      (`*.test.ts`), benchmarks (`*.bench.ts`), and the `index.ts` barrel
 *      (the composition root, which must re-export presentation) are outside
 *      the policed universe; `src/testing/` helpers are test scaffolding.
 *   2. A file is PRESENTATION-classified when its import closure — runtime,
 *      type-only, side-effect, or dynamic — reaches a presentation seed
 *      (`theme.ts`, `geometry.ts`, `render-tree.ts`: the renderer vocabulary
 *      modules) or an OpenTUI/React package. Classification is transitive, so
 *      `shell.ts`, `views-*.ts`, `inspector.ts`, `composer.ts`, `preview.ts`,
 *      `plugins.ts`, `sxr-adapter.ts` (and every future module that leans on
 *      them, even only through their exported data shapes) classify
 *      themselves.
 *   3. Every OTHER file is STATE-LAYER and is policed: it must not import any
 *      presentation module or OpenTUI/React package in any form. A new file
 *      that does not import presentation is therefore policed automatically.
 *   4. The canonical module list below is pinned to stay state-classified. If
 *      a canonical module ever imports presentation (directly or through any
 *      transitive chain), classification would move it to the presentation
 *      side and the pin fails — naming the offending import path. That is the
 *      redesign trigger: presentation has become canonical and must be
 *      unwound (plan T19 redesign_trigger).
 *
 * The parser is exercised against planted snippets so a silently rotting
 * import parser fails this suite instead of passing a fake boundary.
 */

// --- Import parsing ----------------------------------------------------------

type ImportKind = "runtime" | "type" | "dynamic";

interface ParsedImport {
  readonly spec: string;
  readonly kind: ImportKind;
}

/** Strip block comments and full-line comments so prose cannot fake imports. */
function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .split("\n")
    .map((line) => (line.trimStart().startsWith("//") ? "" : line))
    .join("\n");
}

/**
 * Parse import specifiers with their binding kind. When the statement shape
 * is unusual the specifier still comes from a real import/export...from form;
 * the parser self-tests below pin the shapes this repository uses, so a
 * silently rotting parser fails here instead of passing a fake boundary.
 */
export function parseImports(source: string): ParsedImport[] {
  const text = stripComments(source);
  const found: ParsedImport[] = [];
  const push = (spec: string | undefined, kind: ImportKind) => {
    if (typeof spec === "string") found.push({ spec, kind });
  };

  // Side-effect imports: import "./x";
  for (const match of text.matchAll(/import\s*["']([^"']+)["']/g)) {
    push(match[1], "runtime");
  }
  // Dynamic imports: import("./x")
  for (const match of text.matchAll(/import\s*\(\s*["']([^"']+)["']/g)) {
    push(match[1], "dynamic");
  }
  // Type-only re/export bindings: import type ... from "./x" / export type ... from "./x"
  for (const match of text.matchAll(/(?:import|export)\s+type[\s\S]*?from\s*["']([^"']+)["']/g)) {
    push(match[1], "type");
  }
  // Runtime import/export bindings (never matching "import type ...").
  for (const match of text.matchAll(
    /(?:import|export)(?!\s+type\b)[\s\S]*?from\s*["']([^"']+)["']/g,
  )) {
    push(match[1], "runtime");
  }
  return found;
}

// --- Boundary policy ---------------------------------------------------------

/** The renderer vocabulary seeds: a module reaching these is presentation. */
const PRESENTATION_SEEDS: ReadonlySet<string> = new Set([
  "theme.ts",
  "geometry.ts",
  "render-tree.ts",
]);

/** OpenTUI/React packages are banned in the state layer in every form. */
export function isBannedPackage(spec: string): boolean {
  return (
    spec === "react" ||
    spec === "react-dom" ||
    spec === "react-reconciler" ||
    spec.startsWith("react/") ||
    spec.startsWith("react-dom/") ||
    spec.startsWith("@opentui/")
  );
}

const UNIVERSE_EXCLUSIONS = /\.(test|bench)\.tsx?$/;
const TESTING_DIR = "testing";

function srcDir(): string {
  return import.meta.dir;
}

/** All policed-universe candidate files (relative names, filesystem-derived). */
export function enumerateCandidateFiles(): string[] {
  return readdirSync(srcDir())
    .filter((name) => /\.(ts|tsx)$/.test(name))
    .filter((name) => !UNIVERSE_EXCLUSIONS.test(name))
    .filter((name) => name !== "index.ts") // composition-root barrel re-exports everything
    .sort();
}

function readFile(relative: string): string {
  return readFileSync(join(srcDir(), relative), "utf8");
}

/** Resolve an import specifier to the leaf file name it names in src/. */
function resolveLeaf(spec: string): string | null {
  if (!spec.startsWith(".")) return null;
  const leaf = spec.split("/").pop();
  if (leaf === undefined || leaf.length === 0) return null;
  return leaf.includes(".") ? leaf : `${leaf}.ts`;
}

/**
 * Classify every candidate file. Returns the presentation set with, per file,
 * the witnessed reason (for failure messages), plus the policed state set.
 */
export function classifyPresentation(): {
  presentation: Map<string, string>;
  state: string[];
} {
  const candidates = enumerateCandidateFiles();
  const imports = new Map<string, ParsedImport[]>();
  for (const file of candidates) {
    imports.set(file, parseImports(readFile(file)));
  }

  const presentation = new Map<string, string>();
  const explain = (file: string, reason: string) => {
    if (!presentation.has(file)) {
      presentation.set(file, reason);
      return true;
    }
    return false;
  };

  // Pass 0: the vocabulary seeds ARE presentation by definition.
  for (const seed of PRESENTATION_SEEDS) {
    if (candidates.includes(seed)) {
      explain(seed, "is a presentation vocabulary seed");
    }
  }

  // Pass 1: direct evidence (banned packages, presentation vocabulary seeds).
  for (const file of candidates) {
    for (const imported of imports.get(file) ?? []) {
      if (isBannedPackage(imported.spec)) {
        if (explain(file, `imports OpenTUI/React package '${imported.spec}' (${imported.kind})`)) {
          break;
        }
      }
      const target = resolveLeaf(imported.spec);
      if (target !== null && PRESENTATION_SEEDS.has(target)) {
        if (
          explain(file, `imports presentation vocabulary '${imported.spec}' (${imported.kind})`)
        ) {
          break;
        }
      }
    }
  }

  // Pass 2: transitive closure — anything importing a presentation-classified
  // file (in any binding form) is presentation-coupled itself.
  const queue = [...presentation.keys()];
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined) continue;
    const currentReason = presentation.get(current) ?? "presentation";
    for (const file of candidates) {
      if (presentation.has(file)) continue;
      for (const imported of imports.get(file) ?? []) {
        if (resolveLeaf(imported.spec) === current) {
          if (explain(file, `-> ${current} (${currentReason})`)) {
            queue.push(file);
          }
          break;
        }
      }
    }
  }

  const state = candidates.filter((file) => !presentation.has(file));
  return { presentation, state };
}

// --- The canonical state-layer file list (plan T19's policed modules) --------

const CANONICAL_STATE_MODULES: readonly string[] = Object.freeze([
  "affordance.ts",
  "canonical-surface-stream-port.ts",
  "client.ts",
  "config.ts",
  "focus.ts",
  "horizon.ts",
  "live-streams.ts",
  "operation-gateway.ts",
  "operation-registry.ts",
  "planned-operations.ts",
  "preflight.ts",
  "prime-context-envelope.ts",
  "prime-host.ts",
  "prime-kernel.ts",
  "prime-long-running.ts",
  "prime-recursion.ts",
  "prime-topology.ts",
  "projection-store.ts",
  "real-prime-host.ts",
  "relations.ts",
  "workbench-model.ts",
  "worker-folds.ts",
]);

/** Further known state-layer modules (T05/T11/T13/T18/T19). */
const FURTHER_STATE_CANARIES: readonly string[] = Object.freeze([
  "evidence-promotion.ts",
  "headless-replay.ts",
  "kernel-affordances.ts",
  "security-hardening.ts",
  "terminal-session.ts",
]);

const PRESENTATION_CANARIES: readonly string[] = Object.freeze([
  "composer.ts",
  "domainforge-adapter.ts", // type-binds render-tree data shapes (renderer-coupled)
  "geometry.ts",
  "inspector.ts",
  "plugins.ts",
  "preview.ts",
  "render-tree.ts",
  "shell.ts",
  "sxr-adapter.ts",
  "theme.ts",
  "views-chat-trace.ts",
  "views-work.ts",
]);

// --- Tests -------------------------------------------------------------------

describe("T19 renderer boundary: import parser is load-bearing", () => {
  it("detects runtime, type-only, side-effect, dynamic, and package imports", () => {
    expect(parseImports(`import { SEMANTIC_TOKENS } from "./theme";`)).toContainEqual({
      spec: "./theme",
      kind: "runtime",
    });
    expect(
      parseImports(`import type { RenderedLine, RenderedRecord } from "./render-tree";`),
    ).toContainEqual({ spec: "./render-tree", kind: "type" });
    expect(parseImports(`import "./theme";`)).toContainEqual({
      spec: "./theme",
      kind: "runtime",
    });
    expect(parseImports(`const m = await import("./geometry");`)).toContainEqual({
      spec: "./geometry",
      kind: "dynamic",
    });
    expect(parseImports(`import React from "react";`)).toContainEqual({
      spec: "react",
      kind: "runtime",
    });
    expect(parseImports(`import {\n  WorkbenchModel,\n} from "./workbench-model";`)).toContainEqual(
      {
        spec: "./workbench-model",
        kind: "runtime",
      },
    );

    // A type-only binding is never misclassified as a runtime import.
    const parsed = parseImports(`import type { A } from "./somewhere";`);
    expect(parsed.some((entry) => entry.spec === "./somewhere" && entry.kind === "runtime")).toBe(
      false,
    );
  });

  it("ignores import-shaped prose in comments", () => {
    expect(parseImports(`// import { x } from "./theme";\nconst a = 1;`)).toEqual([]);
    expect(parseImports(`/* from "./theme" */\nconst a = 1;`)).toEqual([]);
  });
});

describe("T19 renderer boundary: presentation/state classification (DSN-011, REQ-TUI-005)", () => {
  const { presentation, state } = classifyPresentation();

  it("classifies the known presentation surfaces as presentation", () => {
    const missing = PRESENTATION_CANARIES.filter((file) => !presentation.has(file));
    if (missing.length > 0) {
      throw new Error(
        `expected presentation surfaces lost their classification (boundary moved): ${missing.join(", ")}`,
      );
    }
    expect(missing).toEqual([]);
  });

  it("polices every canonical Gauntlet/Prime state-layer module as presentation-free", () => {
    const moved = [...CANONICAL_STATE_MODULES, ...FURTHER_STATE_CANARIES]
      .filter((file) => presentation.has(file))
      .map((file) => `${file}: ${presentation.get(file) ?? "presentation"}`);
    if (moved.length > 0) {
      throw new Error(
        `canonical state-layer module(s) now import presentation — presentation has become ` +
          `canonical and must be unwound (plan T19 redesign_trigger): ${moved.join("; ")}`,
      );
    }
    expect(moved).toEqual([]);
  });

  it("every state-layer file imports no presentation module and no OpenTUI/React package", () => {
    const violations: string[] = [];
    for (const file of state) {
      for (const imported of parseImports(readFile(file))) {
        if (isBannedPackage(imported.spec)) {
          violations.push(`${file}: banned package '${imported.spec}' (${imported.kind})`);
        }
        const target = resolveLeaf(imported.spec);
        if (target !== null && presentation.has(target)) {
          violations.push(`${file}: presentation module '${imported.spec}' (${imported.kind})`);
        }
      }
    }
    if (violations.length > 0) {
      throw new Error(`state-layer files import presentation: ${violations.join("; ")}`);
    }
    expect(violations).toEqual([]);
  });

  it("the policed state set is filesystem-derived, so new clean files are policed", () => {
    const candidates = enumerateCandidateFiles();
    // Nothing escapes policing by omission: the state set is exactly the
    // non-presentation candidates, and every candidate is one or the other.
    expect([...state].sort()).toEqual(candidates.filter((file) => !presentation.has(file)).sort());
    expect(state.length + presentation.size).toBe(candidates.length);
  });
});

describe("T19 renderer boundary: the T19 harness and consumer are presentation-free", () => {
  function bannedSpecifiersIn(relativePath: string): string[] {
    const source = readFile(relativePath);
    return parseImports(source)
      .map((imported) => imported.spec)
      .filter((spec) => {
        if (isBannedPackage(spec)) return true;
        const leaf = resolveLeaf(spec);
        return (
          leaf !== null &&
          (PRESENTATION_SEEDS.has(leaf) ||
            /theme|geometry|render-tree|shell|views-|inspector|composer|preview|plugins|sxr-adapter|domainforge-adapter/.test(
              leaf,
            ))
        );
      });
  }

  it("headless-replay.ts imports no presentation module and no OpenTUI/React package", () => {
    expect(bannedSpecifiersIn("headless-replay.ts")).toEqual([]);
  });

  it("testing/minimal-projection.ts imports no presentation module and no OpenTUI/React package", () => {
    expect(bannedSpecifiersIn(join(TESTING_DIR, "minimal-projection.ts"))).toEqual([]);
  });

  it("the minimal consumer imports only state-layer modules", () => {
    const { state } = classifyPresentation();
    const source = readFile(join(TESTING_DIR, "minimal-projection.ts"));
    const targets = parseImports(source)
      .map((imported) => imported.spec)
      .filter((spec) => spec.startsWith("."))
      .map((spec) => resolveLeaf(spec));
    expect(targets.length).toBeGreaterThan(0);
    for (const target of targets) {
      if (target === null) continue;
      expect(state.includes(target)).toBe(true);
    }
  });
});
