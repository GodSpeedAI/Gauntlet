---
name: grilling
description: Grill the user relentlessly about a plan, decision, or idea. Use when the user wants to stress-test their thinking, or uses any 'grill' trigger phrases.
---

This is a direct port of the upstream `grilling` skill
(`skills/productivity/grilling` in `mattpocock/skills`). In this repo it is
the companion `wayfinder-gauntlet` calls on during formation to extract
sharp questions from a loose intent (Core-spec Annex D.4).

Interview the requester relentlessly until reaching a shared understanding.
Map this as a **design tree**: every decision branches into the decisions
that hang off it.

Work the tree in **rounds**. The **frontier** is every decision whose
prerequisites are already settled — the questions that can be asked _now_
without guessing at answers not yet heard. Ask the whole frontier in one
round: number each question and give a recommended answer. Then wait for
the answers before the next round.

Format a round like so:

```
❓ **Q1** - **<question title>**: <question body, might be multiple paragraphs, including multiple choices>

➡️ <recommended answer>

---

❓ **Q2** - **<question title>**: <question body, might be multiple paragraphs, including multiple choices>

➡️ <recommended answer>
```

Each round's answers reshape the tree: settled decisions push the frontier
outward and unblock questions that depended on them. Recompute the frontier
and ask the next round. A question whose answer depends on another question
still open in this round belongs to a _later_ round, not this one.

Finding _facts_ is the grilling agent's job, never the requester's. When a
frontier question needs a fact from the environment (filesystem, tools,
etc.), dispatch a sub-agent to find it rather than asking the requester for
anything that could be looked up directly. Don't block on it: a running
exploration is an unsettled prerequisite, so only the questions downstream
of it wait for the sub-agent to report — ask the rest of the frontier now.
The _decisions_ belong to the requester: put each to them and wait.

The session is done when the frontier is empty: every branch of the design
tree visited, nothing left silently assumed. Do not act on the result until
the requester confirms a shared understanding has been reached.
