/**
 * Renderer-neutral render-tree description types shared by every T12 shell
 * module (plan T12, REQ-TUI-001, REQ-GOAL-005, DSN-011 direction).
 *
 * Render functions are stateless: they consume immutable Workbench snapshots
 * plus shell presentation state and PRODUCE this structural description. No
 * function here owns business state, talks to adapters, or writes a terminal;
 * lowering this tree to OpenTUI/React elements is a later adapter concern.
 *
 * Spans carry a semantic token NAME — never a resolved colour (DSN-001);
 * resolution/lowering happens only in theme.ts.
 */

import type { Emphasis, SemanticToken } from "./theme";

/** One styled run of text. `token` is a semantic token name, not a colour. */
export interface RenderedSpan {
  readonly text: string;
  readonly token?: SemanticToken | undefined;
  readonly emphasis?: Emphasis | undefined;
}

/** One rendered row. `id` is a stable structural anchor for tests/navigation. */
export interface RenderedLine {
  readonly id?: string | undefined;
  readonly spans: readonly RenderedSpan[];
}

/**
 * An inspectable record object (DSN-008): evidence and authority render as
 * records carrying their fields (claim, producer, result, identity,
 * provenance), never as bare badges.
 */
export interface RenderedRecord {
  readonly id: string;
  readonly kind: "evidence" | "authority" | "affordance" | "relation" | "attention";
  readonly fields: Readonly<Record<string, string>>;
  readonly token?: SemanticToken;
}

/** The content one View produces for one snapshot. */
export interface ViewContent {
  readonly view: string;
  /** The operator question this View answers (DESIGN.md view grammar). */
  readonly question: string;
  readonly lines: readonly RenderedLine[];
  readonly records: readonly RenderedRecord[];
}

/** Concatenated plain text of one line (glyphs and labels included). */
export function lineText(line: RenderedLine): string {
  return line.spans.map((span) => span.text).join("");
}

/** Concatenated plain text of a line list, one row per line. */
export function renderToText(lines: readonly RenderedLine[]): string {
  return lines.map(lineText).join("\n");
}

/** Whether any span in the line list carries the given semantic token. */
export function linesUseToken(lines: readonly RenderedLine[], token: SemanticToken): boolean {
  return lines.some((line) => line.spans.some((span) => span.token === token));
}
