# Product

## Register

product

## Users

Solo operator supervising long-horizon agent work from a terminal. The operator runs a bounded work contract, leaves it executing, returns to inspect what actually happened, and settles whether the target stayed fixed while the work changed underneath it.

Context: Bun + OpenTUI terminal workbench, typically 120x40 down to 80x24 minimum geometry. Keyboard-first with additive GUI-grade mouse interaction: the terminal is the renderer, not the interaction metaphor, and mouse gestures must converge on the same semantic commands as keys. No browser DOM conventions. The operator's scarcest resource is attention across Foci (Case, WorkItem, Run, Agent, Evidence, File, AttentionItem), not pixels.

Job to be done: know where I am, what is happening, what requires attention, what evidence exists, and what can legitimately be done next. Then spend the next affordable, governable, recoverable move through the canonical gateway.

## Product Purpose

Gauntlet is a constrained execution harness for long-horizon agentic work. It turns a bounded work contract into execute, observe, compare, verify, correct, settle, and durable evidence, while keeping the governing objective and acceptance criteria outside the executor.

The TUI workbench is the terminal-native operator surface for that loop. It makes the current Case world computationally and perceptually coincident around the operator's Focus, progressively materializing only the representations, capabilities, and affordances justified by the current horizon.

Success for this layout pass: repair composition so the released entry is a complete projection of the implemented modules. Shell-calculated geometry is honored, headings align with body columns, palette plus help plus attention plus composer lines are all visible, semantic styling survives to cells, and interaction callbacks reach the model and gateway. No layout polish can substitute for that wiring.

## Brand Personality

Precise, quiet, evidence-bearing.

Voice: institutional authority without spectacle. States what is known, what is missing, and what is inconclusive, in plain operational language. Never performs confidence it cannot prove.

Emotional goals: trust through restraint, calm under long runs, clarity at consequential boundaries. Serious infrastructure under active use, dense when density is useful, quiet when judgment is required.

## Anti-references

What this should explicitly NOT look like:

* AI spectacle or chat-first assistant framing
* Busy dashboard with competing widgets
* Command-center cosplay with decorative maps and blinkenlights
* Neon hacker aesthetic on black
* Status theater that reports green without evidence
* SaaS hero-metric template (big number, small label, gradient accent)
* Identical icon plus heading plus text card grids repeated endlessly
* Glassmorphism, gradient text, side-stripe accent borders, modal as first thought

## Design Principles

1. Ruler stays outside the executor. Layout never lets presentation redefine what counts as success. Comparison and settlement read from canonical evidence, not from what the screen happens to show.
2. Evidence over persuasion. Missing evidence is shown as missing, not as passing. Inconclusive stays visible until proven. Every consequential boundary carries its artifact.
3. Focus coincidence. When the operator focuses a WorkItem, Run, Agent, Evidence record, or File, its semantic relations, execution relations, agents, files, evidence, history, attention conditions, and current affordances become locally adjacent. The operator never manually reconstructs joins from separate apps.
4. Router, not authority. The workbench routes legitimate moves through the canonical gateway with fresh pre-admission checks. Views are read-only projections, never optimistic state. Capability is inventory until resolved against Focus and Horizon with reason codes.
5. Quiet until judgment is required. Density serves the task, motion conveys state change only, and attention surfaces interrupt only for blocked, stale, refused, or settlement-pending conditions.

## Accessibility & Inclusion

Terminal baseline. Keyboard-only operation with stable focus identity; every keyboard path remains fully usable when mouse is unavailable. Minimum 80x24 geometry supported, 120x40 as the primary composition target. Consequential state remains intelligible in monochrome through glyph plus label plus emphasis (DSN-002 discipline). No color-only signaling. Text fitting uses terminal cell width, not UTF-16 length. Standalone Escape, Shift-Tab, and resize paths must resolve without indefinite waits.
