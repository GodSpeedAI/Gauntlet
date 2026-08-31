---
name: domain-modeling
description: Build and sharpen a project's domain model. Use when discussing codebase terminology, writing or editing a CONTEXT.md, or recording or editing an ADR.
---

This is a direct port of the upstream `domain-modeling` skill
(`skills/engineering/domain-modeling` in `mattpocock/skills`); its front
matter already conformed to this repo's skill schema, so no field changes
were needed. See `PROVENANCE.md` in this directory for upstream commit,
licence, and delta. In this repo it is the companion `wayfinder-gauntlet`
calls on during formation to produce the vocabulary a reference is written
in (Core-spec Annex D.4).

Actively build and sharpen the project's domain model while designing. This
is the *active* discipline: challenging terms, inventing edge-case
scenarios, and writing the glossary and decisions down the moment they
crystallise. Merely reading `CONTEXT.md` for vocabulary is not this skill —
that's a one-line habit any skill can do. This skill is for changing the
model, not just consuming it.

## File structure

Most repos have a single context: a `CONTEXT.md` at the repo root, plus
`docs/adr/` holding numbered decision records such as
`docs/adr/0001-event-sourced-orders.md`.

If a `CONTEXT-MAP.md` exists at the repo root, the repo has multiple
contexts instead. The map lists each context and points to where its own
`CONTEXT.md` and `docs/adr/` live (for example under a per-context source
directory), plus a Relationships section describing how contexts refer to
each other.

Create files lazily: only when there is something to write. If no
`CONTEXT.md` exists, create one when the first term is resolved. If no
`docs/adr/` exists, create it when the first ADR is needed.

## During the session

### Challenge against the glossary

When the requester uses a term that conflicts with the existing language in
`CONTEXT.md`, call it out immediately: "Your glossary defines term X as A,
but you seem to mean B — which is it?"

### Sharpen fuzzy language

When the requester uses vague or overloaded terms, propose a precise
canonical term: "You're saying 'account' — do you mean the Customer or the
User? Those are different things."

### Discuss concrete scenarios

When domain relationships are being discussed, stress-test them with
specific scenarios. Invent scenarios that probe edge cases and force
precision about the boundaries between concepts.

### Cross-reference with code

When the requester states how something works, check whether the code
agrees. If a contradiction turns up, surface it: "Your code cancels entire
Orders, but you just said partial cancellation is possible — which is
right?"

### Update CONTEXT.md inline

When a term resolves, update `CONTEXT.md` right there — don't batch these
up, capture them as they happen. `CONTEXT.md` is a glossary and nothing
else: keep each definition to one or two sentences describing what a term
IS (not what it does), note synonyms to avoid under each term, and include
only terms specific to this project's context — general programming
concepts don't belong even if the project uses them extensively. Group
related terms under subheadings once natural clusters emerge; a flat list
is fine until then.

### Offer ADRs sparingly

Only offer to create an ADR when all three are true:

1. **Hard to reverse** — the cost of changing course later is meaningful.
2. **Surprising without context** — a future reader will wonder why it was
   done this way.
3. **The result of a real trade-off** — there were genuine alternatives and
   one was picked for specific reasons.

If any of the three is missing, skip the ADR. An ADR can be a single
paragraph: what the decision was and why — the value is in recording *that*
a decision was made, not in filling out sections. Number sequentially
(`0001-slug.md`, `0002-slug.md`, ...) inside `docs/adr/`, scanning for the
highest existing number and incrementing by one.
