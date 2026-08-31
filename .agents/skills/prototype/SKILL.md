---
name: prototype
description: Build a throwaway prototype to answer a design question. Use when the user wants to sanity-check whether a state model or logic feels right, or explore what a UI should look like.
---

This is a direct port of the upstream `prototype` skill
(`skills/engineering/prototype` in `mattpocock/skills`); its front matter
already conformed to this repo's skill schema, so no field changes were
needed. See `PROVENANCE.md` in this directory for upstream commit, licence,
and delta. In this repo it is the companion `wayfinder-gauntlet` calls on
during formation for a cheap concrete artifact that raises discussion
fidelity on a HITL decision (Core-spec Annex D.4).

A prototype is **throwaway code that answers a question**. The question
decides the shape.

## Pick a branch

Identify which question is being answered, using the requester's prompt,
the surrounding code, or by asking if the requester is around:

- **"Does this logic / state model feel right?"** Build a single
  shareable HTML file (free-play buttons plus tabbed guided walkthroughs)
  that pushes the state machine through cases that are hard to reason about
  on paper, and that a non-developer can drive.
- **"What should this look like?"** Generate several radically different
  UI variations on a single route, switchable via a URL search param and a
  floating bottom bar.

Getting the branch wrong wastes the whole prototype: the two produce very
different artifacts. If the question is genuinely ambiguous and the
requester isn't reachable, default to whichever branch better matches the
surrounding code (a backend module leans logic; a page or component leans
UI) and state the assumption at the top of the prototype.

## Logic branch

Isolate the actual logic (the bit answering the question) in a small, pure
module that could be lifted out and dropped into the real codebase later —
a reducer, an explicit state machine, a small set of pure functions, or a
class with a clear method surface, whichever shape best fits the question.
Keep it free of DOM access or button handlers reaching inside it; the page
calls into it, nothing flows the other way.

Build one self-contained HTML file, plain HTML/CSS/JS, everything inline so
it opens by double-click. Write it in domain language, not code. Lay it out
top to bottom: a one-line statement of the question being explored, the
current relevant state rendered as a readable panel (re-rendered after
every action), free-play buttons — one per action, always available — and a
set of guided-walkthrough tabs, each a short scenario description plus the
ordered buttons to press, resetting to a known initial state when started.
Choose scenarios that demonstrate the awkward cases: the happy path, a
tricky edge case, an attempt at something that should be illegal.

## UI branch

Prefer adjusting an existing page over creating a new route: variants
rendered on the same route, gated by a `variant` URL search param, with
existing data fetching, params, and auth left alone — only the rendering
swaps. Reach for a brand-new throwaway route only when the thing being
prototyped genuinely has no existing page to live inside; when it does,
follow whatever routing convention the project already uses and name it so
it's obviously a prototype.

Default to three variants (cap at five). Variants must be structurally
different — different layout, different information hierarchy, different
primary affordance — not just different colours; two drafts that come out
too similar means redoing one with explicit guidance to diverge. Wire them
behind a small switcher component: a floating, fixed-position bar with a
previous arrow, a variant label, and a next arrow, updating the URL search
param on click (and on the left/right arrow keys, except while a text input
is focused) so the current variant is shareable and reload-stable. Gate the
switcher out of production builds.

## Rules that apply to both branches

1. **Throwaway from day one, and clearly marked as such.** Locate the
   prototype close to where it will actually be used so context is
   obvious, but name it so a casual reader can see it's a prototype, not
   production.
2. **Trivial to run.** A UI prototype starts from one command in the
   project's task runner; a logic demo is a single HTML file the requester
   double-clicks. Either way, no thinking required to start it.
3. **No persistence by default.** State lives in memory. Persistence is the
   thing the prototype is *checking*, not something it should depend on,
   unless the question explicitly involves a database — in that case hit a
   scratch database or a local file with a clear "prototype, wipe me" name.
4. **Skip the polish.** No tests, no error handling beyond what makes the
   prototype runnable, no abstractions. The point is to learn something
   fast.
5. **Surface the state.** After every action (logic) or on every variant
   switch (UI), render the full relevant state so the requester can see
   what changed.
6. **Capture it when done.** Fold any validated decision into the real
   code, then capture the prototype itself as a primary source: commit it
   to a throwaway branch, out of the main line, and leave a pointer to that
   branch from wherever the decision is recorded, along with the verdict
   and the question it settled. The main line keeps only the validated
   decision, never the losing variants or the switcher scaffolding.

## Anti-patterns

- Variants (or reducer states) that differ only cosmetically — that's a
  tweak, not a prototype.
- Wiring a prototype to a real mutation path or a real database when the
  question doesn't require it.
- Promoting prototype code directly into production without rewriting it
  under normal quality constraints.
- Adding tests to a prototype, or generalising it beyond the one question
  it exists to answer.
