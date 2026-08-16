---
type: Agent Instructions
title: Lessons Learned
description: Lessons learned during implementation work.
resource:
last_modified: 2026-08-15
---

# Purpose

This directory holds lessons captured during implementation work. A lesson is a durable, reusable unit of knowledge, not a changelog entry. It exists so a future agent, on a different task, in a different session, does not repeat a mistake or rediscover a constraint the hard way.

Every lesson file in `.agents/lessons/` is one OKF concept: YAML frontmatter plus a markdown body. This file is itself one such concept, describing how to write the others.

# What qualifies as a lesson

Use this test before writing a file: **would this still be true and useful for an agent working on unrelated code six months from now?** If the answer is no, it is not a lesson. It is task output, and it belongs in a PR description, commit message, or session log, not here.

| Write a lesson when | Do not write a lesson when |
|---|---|
| A pattern generalizes across future tasks, not just this one | It is a one-off fix specific to a single PR |
| A codebase-specific gotcha will trip up any agent touching that area | It is a bug that was fixed and has no recurrence risk |
| A constraint is needed-to-know before working in a specific module, boundary, or subsystem | It restates something already documented in an existing AGENTS.md or README |
| The harness, tooling, or agent instructions themselves were the actual root cause | The root cause was a one-time human error (bad ticket, typo, miscommunication) |
| The lesson would change what a future agent does differently, not just what it knows | It is interesting but does not change any future decision or action |

If a lesson doesn't pass the "changes future behavior" bar, don't file it. A pile of true-but-inert facts is worse than no lesson file, because agents will spend budget reading it.

# Frontmatter

Follow the Open Knowledge Format (OKF). Only `type` is required by the spec; the rest are recommended and reserved for structured querying. Do not invent new top-level keys unless a bundle-wide convention already exists for it.

| Field | Required | Value |
|---|---|---|
| `type` | yes | Always `Lesson` for files in this directory |
| `title` | yes | Short, specific, imperative or noun-phrase. Not "Bug fix," but "Retry logic must respect the circuit breaker in `payments/client.go`" |
| `description` | yes | One sentence: the lesson itself, standalone, no need to read the body to get the gist |
| `resource` | recommended | Link to the file, module, PR, or commit this lesson originates from |
| `last_modified` | yes | ISO date, updated whenever the lesson is edited, not just created |
| `tags` | recommended | Codebase area, subsystem, or failure mode, for later cross-referencing |
| `scope` | recommended | Path glob or module name this lesson applies to, e.g. `src/agents/*` or `repo-wide` |

Example:

```yaml
---
type: Lesson
title: Circuit breaker state must be checked before retry, not after
description: Retrying on a tripped circuit breaker in payments/client.go silently drops requests instead of failing loudly.
resource: src/payments/client.go
tags: [payments, retries, error-handling]
scope: src/payments/*
last_modified: 2026-08-15
---
```

# Body structure

Every lesson body uses these headings, in this order. Skip a heading only if it is genuinely not applicable, don't leave it as an empty stub.

1. **Context** — what task or investigation surfaced this. One or two sentences. Enough for a future agent to judge relevance, not a narrative.
2. **The lesson** — the generalizable rule itself, stated as an instruction an agent can act on. Write it the way you'd want it to read inside an AGENTS.md, because it may end up there verbatim.
3. **Why it matters** — the failure mode this prevents, or the cost of not knowing it. Concrete, not abstract.
4. **Scope** — where this applies and, just as important, where it does not. A lesson that overclaims its scope is a lesson that misleads the next agent.
5. **Evidence** — the specific file, line, error, or behavior that grounds this. Link with `resource` or inline markdown links per OKF cross-linking convention. No evidence, no lesson: unverified pattern-matching does not belong here.

Do not include a "How I fixed it" section describing the specific PR's diff. That's implementation detail, it belongs in the PR, not in a lesson meant to generalize.

# Promotion path

Lessons in this directory are a staging area, not a terminal destination. A lesson should be promoted when it clears a higher bar than "worth remembering":

| Signal | Action |
|---|---|
| Same lesson would apply at a codebase boundary (module entry point, API surface, service boundary) | Promote into that boundary's local AGENTS.md |
| Lesson applies repo-wide, not to one module | Promote into the root AGENTS.md |
| Lesson reveals a gap in the harness itself, not the code (missing guardrail, missing verification step, missing tool) | Flag it as a harness change, don't just document a workaround. A lesson that says "always remember to check X by hand" is a lesson that should instead become an automated check. |
| Two or more independent lessons converge on the same underlying gap | Treat convergence as a stronger promotion signal than either lesson alone. Note the pattern explicitly when filing the second one. |

When a lesson is promoted, don't delete the original file. Update it to note where it was promoted to (link via `resource` or a body note) and mark it superseded. This keeps the provenance chain intact, which matters more here than tidiness.

# Anti-patterns

- **Duplicate filing.** Before writing a new lesson, check whether an existing file already covers this scope and tag. Extend it instead of forking a near-duplicate.
- **Vague titles.** "Fix bug in auth" is not a title. Titles should be specific enough that an agent scanning a directory listing can judge relevance without opening the file.
- **Unverified claims.** Don't file a lesson based on a hunch or a single ambiguous failure. If you're not confident it will reproduce, say so explicitly in Evidence, or don't file it yet.
- **Scope creep.** A lesson learned in one module is not automatically true repo-wide. State scope narrowly; broaden it only when evidence supports it.
- **Stale lessons.** If you touch code covered by an existing lesson's `scope` and find the lesson is now wrong or outdated, update `last_modified` and correct it in place. Don't leave contradictory lessons standing.

# File naming

`kebab-case-short-slug.md`, matching the title's key phrase. One lesson per file. No numeric prefixes, no dates in the filename (that's what `last_modified` is for).