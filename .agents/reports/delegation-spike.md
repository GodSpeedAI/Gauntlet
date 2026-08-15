# Delegation spike — admission vs. completion (Task 0, step 2)

**Status:** empirically verified, this session, 2026-08-14. Throwaway spike; nothing under test was committed.

## Question

Core-spec.md §10.5/Annex C designs `AgentRunner` as admission-based specifically because
Prime-Agent's `rlm(...)` "returns immediately after task admission … it never waits for or
returns the child's answer" (spec D1). Before building Task 4's `AgentRunner` port and
Task 15/16's adapters on that assumption, Task 0 requires confirming empirically whether
*this* environment's harness (Claude Code, reached here via the `Agent` tool and the
`Workflow` tool) behaves the same way, or whether it is actually a blocking call-and-return
interface that would make the admission-based design unnecessary machinery.

## Method

Invoked the `Agent` tool directly with a trivial, non-fork, `general-purpose`-class task
(model: haiku) whose entire instruction was to echo a fixed sentinel string
(`DELEGATION_SPIKE_OK_12345`) and do nothing else. Inspected exactly what came back in the
tool's own return value versus what arrived later, and how.

## Observation 1 — the tool call's own return value

The `Agent` tool call's result, returned synchronously in the same turn, was **not** the
child's answer. It was:

```text
Async agent launched successfully.
agentId: a3222a3928c0f8f78
The agent is working in the background. You will be notified automatically when it
completes. You know nothing about its results until that notification arrives — do not
report, assume, or predict them.
output_file: /tmp/.../tasks/a3222a3928c0f8f78.output   (explicitly: do not read/tail this)
```

This is an **admission handle**: an opaque child identifier, a session/output-file
reference for forensics, and an explicit instruction not to treat anything in this
response as the result. Structurally identical in kind to Prime-Agent's
`{rlm_child_id, name, session_dir, model}` handle described in spec Annex C.1.

## Observation 2 — where the actual result showed up

The child's answer arrived several tool calls later, unprompted, as a separate
`<task-notification>` event pushed into the conversation out-of-band:

```xml
<task-notification>
  <task-id>a3222a3928c0f8f78</task-id>
  <status>completed</status>
  <result>DELEGATION_SPIKE_OK_12345</result>
  <usage>...</usage>
</task-notification>
```

This is delivered as a *signal* attached to the surrounding conversation flow, not as a
file the child wrote to a path this session declared in advance. That is one concrete
difference from Prime-Agent's file-based result contract (§10.3): Prime-Agent children
reply via `agent_message.send(...)` or by writing files, and the runtime is told to trust
neither in isolation — "the mailbox is a signal; the file is the contract." This harness's
`<task-notification>` plays the mailbox's role (a completion *signal*, and in this case it
happens to carry the free-form text), not the role of a schema-valid result document at a
declared path.

## Finding

**The admission-handle assumption in Core-spec.md §10.5/Annex C holds in this environment
too — confirmed, not merely assumed.** Both delegation surfaces available in this session
(`Agent`, including plain non-fork invocations, and `Workflow`, whose own tool
description independently states "this tool returns immediately with a task ID, and a
`<task-notification>` arrives when the workflow completes") are fire-and-forget by
contract, not blocking call-and-return. `Workflow`'s async contract was not independently
re-spiked (to avoid burning a second live agent run for a fact its own tool description
already states unambiguously and which is now corroborated by the `Agent` spike's
identical notification mechanism) — if this ever needs re-confirming empirically, run the
same probe again with a trivial `Workflow` script.

**Consequence for the port design (Task 4 and Task 15/16): no adaptation needed.**
`AgentRunner::invoke -> InvocationHandle` with `poll`/`collect` against a declared result
contract (spec §6.1, §10.3) is the correct shape here, exactly as specified. The one
adapter-level nuance worth carrying into Task 15/16: if a future adapter wraps this
harness's own `Agent`/`Workflow` tools as an `AgentRunner`, its `poll()` must still gate
completion on a schema-valid `result.json` at the declared `GAUNTLET_RESULT_PATH` — never
on the contents of a `<task-notification>` — for the same reason Prime-Agent's adapter
must not trust the mailbox alone (§10.3 rule 2–3, Annex C.2): free-form completion text is
untrusted external input until parsed and validated. That adapter is not in scope for
Tasks 15–16 (which target `prime-agent` and a generic `cli-agent`), so this is recorded
here for future reference rather than acted on now.

No blocker. Task 4 proceeds on the admission-based `AgentRunner` design as specified.
