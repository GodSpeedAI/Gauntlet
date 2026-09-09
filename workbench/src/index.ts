/**
 * Gauntlet Prime-Native Terminal Workbench entry point.
 *
 * Implements the early operator preview journey (T00A) and exports the
 * read-only client, geometry classification, semantic theme, and preview engine.
 *
 * See .agents/plans/gauntlet-tui-prime-workbench.plan.yaml and DESIGN.md.
 */

export * from "./affordance";
export * from "./client";
export * from "./composer";
export * from "./config";
export * from "./focus";
export * from "./geometry";
export * from "./horizon";
export * from "./inspector";
export * from "./operation-gateway";
export * from "./operation-registry";
export * from "./preflight";
export * from "./preview";
export * from "./prime-context-envelope";
export * from "./prime-host";
export * from "./prime-kernel";
export * from "./prime-long-running";
export * from "./prime-recursion";
export * from "./prime-topology";
export * from "./projection-store";
export * from "./relations";
export * from "./render-tree";
export * from "./shell";
export * from "./theme";
export * from "./views-chat-trace";
export * from "./views-work";
export * from "./workbench-model";
