# Workbench inspection — 2026-09-10

The released entry is an incomplete composition of the implemented workbench modules. Rendering polish alone cannot make it functional. No implementation changes were made during this inspection.

## Confirmed findings

1. **The launcher never connects to Gauntlet.** `src/main.ts` resolves configuration only to print the endpoint. It creates an empty `WorkbenchModel`, marks a local generation ready, hardcodes `gauntlet/unbound`, and supplies an unreachable `coldObservation()` on every render. Starting the backend cannot populate this entry. Client, stream, and gateway modules exist separately.
2. **OpenTUI is not mounted.** The release entry writes ANSI clear-screen sequences and concatenated strings. It discards semantic styling by flattening spans. Searches found no production `createCliRenderer` or `createRoot` mounting path in `src/`.
3. **Layout allocation is incorrect.** At 120×40, `main.ts` allocates 30/30/60 columns rather than consuming the shell's calculated 28/56/36 allocation. It then adds two separators beyond the terminal width and truncates the result. Even empty body rows acquire an ellipsis. Heading padding is removed with `trimEnd()`, so headings do not align with body columns.
4. **Interactive content is dropped.** `shell.ts` produces palette, help, attention, and composer content; `main.ts` omits the first three and substitutes a fixed composer label for the actual composer lines. Operators cannot see the palette or their draft.
5. **Host callbacks are absent.** The launcher supplies only `quit` to `ShellSession`. Focus selection, hoisting, affordance invocation, and composer submission have no host callbacks connected to the model or canonical gateway.
6. **Escape waits indefinitely.** The raw decoder buffers a trailing Escape without a timeout. A standalone Escape produces no redraw until another byte arrives. Shift-Tab and other longer terminal sequences are also absent from its key map. Text fitting uses UTF-16 length rather than terminal cell width.
7. **Verification misses the composition gap.** `opentui-shell.test.ts` tests the renderer-neutral shell structure. The gated PTY suite launches the real entry but checks strings in ANSI-stripped output from the empty cold start, rather than screen cell geometry or a populated backend journey. It does not prove OpenTUI mounting, usable palette/composer rendering, or consequential operation wiring.

## Evidence gathered

- Launched `bun src/main.ts`: confirmed the permanently empty, unbound cold frame.
- Ran the real entry through Bun PTYs at 120×40 and 80×24: reproduced compressed columns, misaligned headings, trailing ellipses, invisible palette, and standalone Escape producing no redraw.
- `XDG_RUNTIME_DIR=/tmp just tui-test opentui-shell`: **12 passed, 0 failed**, despite the reproduced launcher defects. The environment override was needed because the default just temporary directory is read-only.
- Read the spec, plan, current implementation status, release entry, geometry, shell rendering and input paths, and relevant test harnesses. The recorded whole-plan settlement is not sufficient evidence of a working production TUI given these direct contradictions.

## Repair order

First define and connect the production host composition using the existing canonical client, projection model, stream handling, and gateway. Then mount the intended OpenTUI renderer, preserve the complete shell output and semantic styles, and connect interaction callbacks. Verify real terminal cells, input, resizing, and populated backend journeys through the actual release entry before broader visual polish and conformance claims.

This was an initial inspection, not a full end-to-end verification or an exhaustive module audit. No existing settlement records or evidence files were changed.
