# Task 20 — falsifier round 4 adjudication (orchestrator)

Verdict: NOT_CONFIRM (0 blocking / 1 major / 2 minor). Over-rejection leg
clean (shipped E2E suite green as shipped; honest builders settle in both
worlds; kill-9 resumes honest). Three corrections commissioned.

- **F-R4-3 (MAJOR) → COR-R4-1.** Contract fix, mechanism-level: after a
  recovery that COMPLETES productions on an otherwise-all-settled run, the
  resume must re-assess the terminal projection AFTER recovery completes
  and close as an honest settled no-op (terminal=settled, exit 0, zero
  additional iterations) — never book a new iteration round for a run
  whose durable projection is fully settled. Reachability note carried
  honestly: the executed reproduction needs compound state damage
  (checkpoint-offset forgery) that is ALREADY the recorded
  "checkpoint() forged-row projection class (planning-layer)" deferral;
  the forged-offset class itself is NOT in scope for task20 and stays
  deferred. The test pins the distinction: recovery-completed damage on a
  settled run ⇒ settled no-op; a genuinely forged checkpoint remains the
  deferred class.
- **F-R4-1 (MINOR) → COR-R4-2.** Skills verify digests RAW source bytes
  exactly as roles verify does (no normalizer pass) — the two surfaces
  must discriminate identically; pin with the CRLF/trailing-blank reforged
  lock shape (refuses, naming both digests). Honest round trip stays
  green.
- **F-R4-2 (MINOR) → COR-R4-3.** Writer-lock Held refusal: close the
  create-vs-pid-write read window (bounded retry/reread before reporting)
  and, when the pid is blank/unreadable or a pid-reuse is plausible, the
  refusal must say "stale or unreadable writer lock (pid X) — confirm no
  writer is running and remove it" instead of asserting a live holder.
  Fail-closed both ways; the race window becomes a regression test.

Constraints unchanged: p1-p12/g6/g7/g10 + shipped E2E legs green
UNMODIFIED (mechanical adaptations listed); over-rejection leg stays
clean. Full battery at the end; `just ruler` before/after.
