# Lesson: checkpoint-as-you-go still loses work — the record lags the tree by one item

**When:** Task 12, 2026-08-25. A correction builder was cancelled by operator
direction at 10:20. Its checkpoint file's last entry was **C3**. The handoff,
the IN-FLIGHT ledger, and `CURRENT_STATUS.yml` all therefore recorded **C5–C9
as outstanding**. They were not: the builder had worked ~40 minutes past its
last checkpoint entry and **all five sub-parts of C5 were already implemented
and asserted in the proofs** (file mtimes: `correction.rs` 09:57,
`controller.rs` 10:01, `select.rs` 10:07, `p4`/`p6` 10:11). A ninth-item
finisher dispatched against that handoff would have re-implemented C5 —
or, worse, "corrected" a landed correct implementation into a different shape
and broken the composed assertions that already depended on it.

**This is not the [session-interruption](session-interruption-recovery.md)
failure.** That one is *silence where a report should be*. This one is a
**record that is present, well-formed, disciplined, and confidently wrong** —
it understates the tree by exactly the item the agent died inside. It survives
every existing detection heuristic, because the checkpoint file looks complete
and the last entry it contains is genuinely accurate.

## Why checkpoint-as-you-go was not enough

The prior lesson's prevention rule — "checkpoint-as-you-go stays mandatory" —
was **followed**. It still failed, because of *when* the entry is written:

```
   [ start item ]---- edit ---- edit ---- test ---- fix ----[ item done ]---[ write checkpoint ]
                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                    death anywhere in here = code in tree, nothing in record
```

Write-after-completion means there is *always* an open window whose width is
the length of an item, and in that window **silence is ambiguous**: it means
either "not started" or "finished but unrecorded", and nothing on the page
distinguishes them. The reader's natural default — silence means not started —
is the unsafe one.

## Mitigation 1 (procedural): declare-first checkpointing

Make silence mean one thing. Write a marker **before** editing, and close it
after:

```markdown
## C5 — real selection — STARTED 2026-08-25 09:41
   Intent: refuse empty rejected lists; enumerate real candidates; ...

## C5 — real selection — DONE 2026-08-25 10:11
   Fail-first: ... Fix: ... Files: ... Tests: ...
```

Now the three states are distinguishable on the page:

| On the page | Means | Recovery action |
|---|---|---|
| nothing | not started | implement it |
| `STARTED`, no `DONE` | died inside the item | **reconcile against the tree first** |
| `STARTED` + `DONE` | complete | never redo |

The window does not close — nothing closes it — but it stops being *silent*.
A `STARTED` with no `DONE` is a positive signal pointing at exactly the item
to inspect, and the cost is one line written before work begins.

**This is now mandatory in every builder/finisher brief.**

## Mitigation 2 (mechanical): `scripts/reconcile-checkpoints.sh`

Procedure that depends on a dying agent remembering to do something is not a
control. The mechanical check does not depend on the agent at all:

```bash
./scripts/reconcile-checkpoints.sh 12
```

It compares the mtime of every task-surface file (from `git status`, minus
agent-tooling churn) against the mtime of the task's newest checkpoint file.
**Any file modified after the record's last write is, by construction, a
change that record cannot be describing.** Exit 1 lists them.

Validated against this incident: run at the 10:20 stop, it would have flagged
`correction.rs`, `controller.rs`, `select.rs`, `p4_corrective_loop.rs` and
`p6_budget_control.rs` — precisely the C5 surface.

It is deliberately **not** wired into `ruler` or `ci`. It makes no claim about
correctness, only about what the record can and cannot account for; its output
is a list to look at, never a verdict. Exit 0 means "not structurally stale",
**not** "the record is right".

## Mitigation 3 (epistemic): the tree outranks the record

Add to the recovery procedure, ahead of planning:

> A checkpoint file is an agent's **report**. The tree is the **contract**.
> Where they disagree, the tree wins — including when the tree says *more* was
> done than the record claims. Reports understate as readily as they overstate,
> and an understating report is the more dangerous of the two because it reads
> as conservative.

Concretely, a resuming orchestrator's order is: `just ruler` →
`reconcile-checkpoints.sh <task>` → read what it flags → *then* plan. Running
it before writing the handoff is what stops the error propagating; running it
before dispatching a finisher is what stops the finisher acting on it.

## Mitigation 4: recovered work is recorded as recovered

When reconciliation finds landed-but-unrecorded work, the entry that closes
the gap must be labelled **recovered-and-verified**, naming the evidence that
established it (mtimes, the assertions found, the tests re-run) — never
written as the recovering agent's own implementation. Two reasons: the
fail-first evidence for that item is genuinely missing and the record must not
imply otherwise, and the next reader needs to know which entries were
reconstructed after the fact.

The corresponding verification bar is lower-cost than re-implementation but is
not nothing: re-run the item's named tests, and confirm the item's assertions
are *present and discriminating* rather than merely green.

## The general shape

Every one of this project's interruption lessons is an instance of the same
rule: **state that lives only in an agent's head is lost at the moment the
agent is.** The fix is never "be more careful"; it is to move the state onto
disk *earlier* than feels necessary, and to keep a mechanical check that does
not depend on the dying process having done anything at all.

Related: [session-interruption-recovery.md](session-interruption-recovery.md)
(the orchestrator's own death; recovery classification),
[subagent-message-loss.md](subagent-message-loss.md) (the returned result
vanishing — this failure compounds with it, since a cancelled agent that also
lost its final message is recoverable *only* through its checkpoint file, and
this lesson is about that file being wrong).
