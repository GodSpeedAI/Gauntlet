# Task 20 — falsifier round 1 adjudication (orchestrator)

Verdict: NOT_CONFIRM (1 blocking / 1 major / 4 minor / 1 informational).
All findings routed; corrections COR-F1..F6 commission round 2.

- **F-1 (BLOCKING) → COR-F1.** The done-when's discrimination claim is
  falsified as executed: a report-forging liar settles everything. The
  defect is real in the COMPOSED produced-artifact world (default compose()
  world untouched, frozen proofs stay byte-identical). Correction invariant:
  in the composed produced-artifact world, a unit whose
  `evidence_required.producer` is `instrument` may settle only when the
  locator artifact is CORROBORATED by an instrument-owned observation
  record (the observation facet actually executing the instrument command)
  covering that artifact; no corroboration ⇒ the unit blocks (a bounded,
  honest terminal) — never settles, never silently downgrades. The fixture
  gains a real test facet (the instrument must actually run
  `scripts/check.sh`), and the falsifier's own forging attack becomes a
  SHIPPED scenario (`e2e-forged-report`) + E2E leg asserting non-settlement
  with the per-unit negative. This is the G3/G12 fabrication class landing
  early where it belongs: the composed path must discriminate before
  Task 24 catalogues it.
- **F-2 (MINOR) → COR-F2.** `load_scenario` contains its stem: a simple
  fixture name under the frozen-locator dir only — separators and `..`
  refused before any path join.
- **F-3 (MINOR) → COR-F3.** TicketId grammar refuses `..` (and any
  path-separator payload); the fs queue additionally validates the resolved
  ticket path stays under the queue root on BOTH read and write paths.
  Contract suites updated; no behavior weakening.
- **F-4 (MINOR) → COR-F4.** `escalations list` quarantines: valid tickets
  are listed; a corrupt file appears as a typed quarantined entry (id +
  decode-error class, no content echoed), exit 0. The operator's inbox
  must not go blind because one file is corrupt; fail-closed is preserved
  for any ACTION on a quarantined ticket.
- **F-5 (INFO) → carried.** Already adjudicated (cor-d-report.md): the
  resolve/retry success path is Task 21's surface; the typed refusal is the
  honest posture. Restated in the closure record.
- **F-6 (MAJOR) → COR-F5.** Skills lock round-trip fixed at the WRITER
  (emit the schema the reader requires: root `skills` = array); add the
  missing round-trip test: install → verify passes; an out-of-band edit →
  typed drift; a forged lock → typed refusal.
- **F-7 (MINOR) → COR-F6.** `resume` of an all-settled run short-circuits
  to an honest settled no-op (terminal=settled, exit 0, no iteration
  burned); p7/g7 recovery proofs stay green UNMODIFIED.

Constraint for the correction round: the default `compose()` world and all
p1-p12/g6/g7/g10 proofs stay byte-identical (the F-1 fix lives in the
opted-in produced-artifact world + fixture composition); every other
existing test stays green UNMODIFIED unless a correction strictly requires
a mechanical adaptation, listed with its reason. `just ruler` before/after.
Full battery at the end (check/test/lint/deny/boundaries/ruler/doctor +
e2e_full_loop_mock + cli_surface + repo-root dry-run zero-write).
