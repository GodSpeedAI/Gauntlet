/**
 * Gauntlet Prime-Native Terminal Workbench entry point.
 *
 * Implements the early operator preview journey (T00A) and exports the
 * read-only client, geometry classification, semantic theme, and preview engine.
 *
 * See .agents/plans/gauntlet-tui-prime-workbench.plan.yaml and DESIGN.md.
 */

export * from "./client";
export * from "./config";
export * from "./geometry";
export * from "./preflight";
export * from "./preview";
export * from "./prime-host";
export * from "./prime-kernel";
export * from "./prime-topology";
export * from "./projection-store";
export * from "./theme";
export * from "./workbench-model";
