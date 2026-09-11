import {
  BoxRenderable,
  type CliRenderer,
  type KeyEvent,
  RGBA,
  StyledText,
  TextAttributes,
  TextRenderable,
} from "@opentui/core";
import type { RenderedLine, RenderedSpan } from "./render-tree";
import type { RenderedShell } from "./shell";
import { cellWidth, fitCellText, sanitizeCellText } from "./geometry";
import { resolveToken } from "./theme";

/** Native parsing owns escape timeouts, modifiers and Unicode decoding. */
export function terminalKey(key: KeyEvent): string | null {
  if (key.eventType === "release") return null;
  if (key.ctrl) return `ctrl+${key.name}`;
  if (key.meta || key.option) return null;
  if (key.name === "tab") return key.shift ? "shift+tab" : "tab";
  if (key.name === "return" || key.name === "linefeed") return "enter";
  if (
    ["escape", "backspace", "delete", "up", "down", "left", "right", "home", "end"].includes(
      key.name,
    )
  )
    return key.name;
  return key.sequence || key.name;
}

/** Fit one line's spans to whole terminal cells: sanitize control input,
 * then clip span by span so leading spans (focus markers) keep priority and
 * the tail yields. The row can never exceed maxCells cells. */
function fitSpans(spans: readonly RenderedSpan[], maxCells: number): RenderedSpan[] {
  const out: RenderedSpan[] = [];
  let used = 0;
  for (const span of spans) {
    if (used >= maxCells) break;
    const piece = fitCellText(sanitizeCellText(span.text), maxCells - used);
    used += cellWidth(piece);
    out.push({ ...span, text: piece });
  }
  return out;
}

/** Lowers immutable shell output to terminal cells; owns no application state. */
export class TerminalShellRenderer {
  private frame: BoxRenderable | null = null;

  constructor(private readonly renderer: CliRenderer) {}

  destroy(): void {
    this.frame?.destroyRecursively();
    this.frame = null;
  }

  render(shell: RenderedShell): void {
    this.destroy();
    const { width, height } = this.renderer;
    const frame = new BoxRenderable(this.renderer, {
      id: "workbench",
      width,
      height,
      overflow: "hidden",
    });
    this.frame = frame;
    this.renderer.root.add(frame);
    const line = (content: RenderedLine, x: number, y: number, cols: number) => {
      if (y < 0 || y >= height || cols <= 0) return;
      frame.add(
        new TextRenderable(this.renderer, {
          id: `cell-${x}-${y}`,
          position: "absolute",
          left: x,
          top: y,
          width: cols,
          height: 1,
          wrapMode: "none",
          truncate: true,
          content: new StyledText(
            fitSpans(content.spans, cols).map((span) => {
              const style = span.token
                ? resolveToken(
                    span.token,
                    process.env["NO_COLOR"] === undefined ? "truecolor" : "monochrome",
                  )
                : null;
              const emphasis = span.emphasis ?? style?.emphasis;
              return {
                __isChunk: true as const,
                text: span.text,
                ...(style?.color ? { fg: RGBA.fromHex(style.color) } : {}),
                attributes:
                  emphasis === "bold"
                    ? TextAttributes.BOLD
                    : emphasis === "dim"
                      ? TextAttributes.DIM
                      : TextAttributes.NONE,
              };
            }),
          ),
        }),
      );
    };
    line(shell.header, 0, 0, width);
    if (shell.prompt) {
      shell.prompt.lines.forEach((row, index) => {
        line(row, 0, index + 2, width);
      });
      return;
    }
    const composer = shell.surfaces.find((surface) => surface.id === "Composer");
    const helpRows = Math.min(shell.help.length, 2);
    const composerRows = Math.min(composer?.lines.length ?? 0, Math.max(3, height - 8));
    const attentionRows = Math.min(shell.attention.length, 3);
    const footerTop = height - helpRows - composerRows - attentionRows;
    let x = 0;
    const columns = shell.surfaces.filter((surface) => surface.id !== "Composer");
    columns.forEach((surface, columnIndex) => {
      // One-cell negative-space gutter between adjacent columns, spent from
      // inside the existing allocation: the x-advance is unchanged, so the
      // geometry sums still hold exactly and no separator is drawn.
      const gutter = columnIndex < columns.length - 1 ? 1 : 0;
      const cols = Math.max(1, surface.width - gutter);
      surface.lines.slice(0, Math.max(0, footerTop - 2)).forEach((row, index) => {
        const displayed =
          index === 0 && surface.focused
            ? { ...row, spans: [{ text: "▸ ", token: "focus.ring" as const }, ...row.spans] }
            : row;
        line(displayed, x, index + 2, cols);
      });
      x += surface.width;
    });
    shell.attention.slice(0, attentionRows).forEach((row, index) => {
      line(row, 0, footerTop + index, width);
    });
    composer?.lines.slice(0, composerRows).forEach((row, index) => {
      line(row, 0, footerTop + attentionRows + index, width);
    });
    shell.help.slice(0, helpRows).forEach((row, index) => {
      line(row, 0, height - helpRows + index, width);
    });
    if (shell.palette) {
      // The overlay gets a bounded scrolling viewport, including the selected row.
      const available = Math.max(1, footerTop - 4);
      const selected = Math.min(shell.palette.index, shell.palette.lines.length - 1);
      const start = Math.max(0, selected - available + 1);
      const overlay = new BoxRenderable(this.renderer, {
        id: "palette-background",
        position: "absolute",
        left: 1,
        top: 1,
        width: Math.max(1, width - 2),
        height: Math.max(1, footerTop - 1),
        backgroundColor: RGBA.defaultBackground(),
        border: true,
        title: " Palette · ↑/↓ select · Enter open · Esc dismiss ",
      });
      frame.add(overlay);
      shell.palette.lines.slice(start, start + available).forEach((row, index) => {
        line(row, 2, index + 2, width - 4);
      });
    }
  }
}
