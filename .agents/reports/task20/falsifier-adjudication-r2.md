# Task 20 — falsifier round 2 adjudication (orchestrator)

Verdict: NOT_CONFIRM (1 blocking / 1 major / 1 minor). Over-rejection leg
clean (honest builders settle — happy, parallel carry, corroborated-world
pin; all shipped teeth green). All three findings routed to corrections
COR-R2-1..3.

- **F-R2-3 (BLOCKING) → COR-R2-1.** The settled-resume no-op must hold in
  BOTH produce worlds. Mechanism: the produced-artifact produce step keys
  production rows by ARTIFACT refs while the resume-completion orphan
  recovery checks FACET refs, so `productions_completed` is never empty on
  the first resume of a composed run. Fix at the mechanism: the
  resume-completion predicate must consult the SAME production key surface
  the produce step writes — policy-aware, one definition of "produced", not
  a second world-specific one (this is the ProducePolicy seam leaking into
  a second consumer; unify the key surface, do not special-case the
  predicate). The in-app test world must be extended to pin the
  produced-artifact world too (both worlds: first resume of a fully settled
  run is a settled no-op, exit 0, zero iterations, zero writes).
- **F-R2-1 (MAJOR) → COR-R2-2.** Escalation queue `resolve` binds its
  write-back to the DECODED record's own `ticket` field. Fix: the write
  path targets the REQUESTED ticket id, and a decoded record whose ticket
  field differs from the requested id is refused (cross-wiring defense,
  same discipline as the sandbox backend-identity check); add the
  clobbering attack as a regression test (lying ticket must not overwrite
  a victim ticket's record; poll of the victim still shows its own state).
- **F-R2-2 (MINOR) → COR-R2-3.** Skills-lock reader goes strict: exact
  schema token, count equality, set-membership completeness against the
  installed/repo side, unknown fields rejected — the repo's standard
  strict-codec discipline (model output and on-disk state are untrusted
  input). The round-trip test gains the rejected shapes.

Constraint unchanged: p1-p12/g6/g7/g10 and the shipped E2E legs stay green
UNMODIFIED (mechanical adaptations listed with reasons); the over-rejection
leg must stay clean (honest builders settle in both worlds). Full battery
at the end; `just ruler` before/after.
