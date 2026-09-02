# Task 20 — falsifier round 3 adjudication (orchestrator)

Verdict: NOT_CONFIRM (0 blocking / 2 major / 1 minor). Over-rejection legs
clean (shipped E2E suite green as shipped; honest builders settle in both
produce worlds; kill-9 first/second resumes honest with byte-identical
zero-write re-resumes). All three findings routed to COR-R3-1..3.

- **F-R3-1 (MAJOR) → COR-R3-1.** Unify the recovery's MINT half with the
  same key-surface discipline COR-R2-1 applied to the read half: the
  orphan-recovery must mint/consult the refs the produce step actually
  records in THAT world (artifact-content refs in the produced world,
  facet refs in the default), so (a) the A5 crash window (event appended,
  row not yet inserted) self-completes in BOTH worlds, (b) damaged/missing
  rows on settled runs produce HONEST forensic records (state the world
  and what is actually missing — never a false "no event exists / bytes
  absent" claim), (c) the applied/granularity flag is per-locator, not
  facet-wide (one intact locator must not mask a sibling's missing row),
  and (d) an otherwise-settled run never reports `budget_exhausted` from
  recovery bookkeeping. The honest settled no-op (both worlds) and p7/g7
  recovery proofs stay green UNMODIFIED.
- **F-R3-2 (MAJOR) → COR-R3-2.** Skills verify content-verifies against
  the REPO SOURCES the way roles verify does: a lock whose digests are
  self-consistent but do not match the canonical `.agents/skills/**`
  bodies is DRIFT (typed, naming skill + both digests); swapped-body,
  stale-after-source-revert, and duplicate-entry shapes refuse. The
  honest round trip (install → verify, edit → drift named, restore →
  verify passes) stays green.
- **M-R3-1 (MINOR) → COR-R3-3.** State-dir single-writer guard: run/resume
  takes the writer role exclusively — an O_EXCL writer-lock file under the
  state dir recording the pid; a second run/resume refuses typed, naming
  the holder pid and the honest manual recovery for a stale lock
  (operator confirms the pid is dead, removes the file). Fail-closed, no
  liveness guessing, no automatic takeover. Concurrent double-resume
  attack becomes a regression test; p7's own single-process recovery
  proofs stay green.
- Falsifier round 1-2 standing adjudications carry (F-5 posture; queue
  quarantine; strict skills reader shape refusals — F-R3-2 adds the
  CONTENT half the shape checks cannot see).

Constraints unchanged: p1-p12/g6/g7/g10 + shipped E2E legs green
UNMODIFIED (mechanical adaptations listed with reasons); over-rejection
leg stays clean. Full battery at the end; `just ruler` before/after.
