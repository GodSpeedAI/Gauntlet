# Task 13 / B4-completion finisher brief (post-window, 2026-08-27/28)

The B4 builder died to the usage limit mid-Item-1 (19th instance; recovery
recorded in `builder-b4-checkpoints.md`). Inherited state: `p7_recovery.rs`
EXISTS (64,698 bytes, 4 tests, mtime 09:46) but DOES NOT COMPILE (8
errors, 3 warnings — E0308-class); the STARTED entry (09:44) declares the
Item-1 design (OrderProbe delegating decorator; in-test `cargo build --bin
crash_helper`; drop+recompose restart; resume_run driving every leg);
Items 2–4 unstarted (no p11/g10 files, no teeth-b4.md).

**Phase R-B4** — complete Item 1 to green: fix the compile against the
declared design (no redesign; if the design itself is wrong, STOP and
record), make the 4 tests pass, record DONE with the RED→GREEN evidence
(the builder's own fail-first evidence is LOST — recovered-honest).

**Phase I-B4** — Items 2 (p11), 3 (g10), 4 (teeth-b4.md + gates) of
`builder-brief-b4.md` verbatim, fail-first per mechanism, teeth RED →
cmp-reverted → green. **New requirement from critic round 1:** p11 and g10
proofs must exercise the POST-COR-R2 shapes if correction round 2 has
landed before you reach Item 2 — read `correction-round2.md` if it exists
and coordinate: if COR2 has NOT landed, build against the current shapes
and note the interaction points (the completion-path guard COR2-1b and the
one-per-unit admission COR2-1a are the two likely proof-shape touchpoints).

**Interlock:** if the correction-round-2 dispatch is running concurrently,
you still hold the build lock EXCLUSIVELY — the dispatcher serializes
correction round 2 AFTER you; do not wait for it.

Gates: the plan's Task-13 gate + g10 + the standing battery + check/lint/
boundaries/deny; ruler before and after. Result contract `builder-b4.md`
(items, deviations, gate tails). All original-brief constraints unchanged.
