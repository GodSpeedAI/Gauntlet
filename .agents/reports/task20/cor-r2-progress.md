# COR-R2 progress (checkpoint protocol — declare-first)

Builder: COR-R2 (falsifier-round-2 corrections). Work order:
`falsifier-adjudication-r2.md` (COR-R2-1 blocking, COR-R2-2 major,
COR-R2-3 minor). Result contract: `.agents/reports/task20/cor-r2-report.md`.

- DONE recon 2026-08-31: loci read — controller.rs (ProducePolicy ~761,
  resume_run no-op predicate ~1118, produce_unit_evidence ~3322,
  compose_produced harness ~8705), settle/mod.rs (complete_pending_after_
  checkpoint ~798 + orphan recovery ~853, module doc §18.3, settle tests
  2945–3137), escalation-fs queue.rs (record/resolve/poll/list/write_
  atomically/decode), skills_cmd.rs (read_lock ~82), skills_roundtrip.rs,
  escalations contract_suite.rs, e2e_full_loop_mock.rs harness, run_cmd
  resume, state_cmd export. git status reconciled against IN-FLIGHT.md:
  task20 tree is the uncommitted task20 work (orchestrator owns commits) —
  no orphaned recovery needed. No live compile.lock holders.
- STARTED COR-R2-1 (unified production key surface) 2026-08-31.
- DONE COR-R2-1 code 2026-08-31: `ProducePolicy::production_recorded` (controller.rs)
  is THE production key-surface read — M world: keyed (run, facet ref); B world:
  (locator, producer) surface via `evidence_productions_at`, iteration-insensitive
  because the produce step's own `already_durable` dedupe is. `complete_pending_
  after_checkpoint` gained the policy parameter (controller threads `self.produce`);
  recovery loop restructured (applied check per facet/unit through the policy;
  limitation only when nothing applied and nothing recovered); predicate code
  UNTOUCHED. `just ruler` PASS before edits; `just check-package gauntlet-app`
  green (all-targets). Settle tests adapted mechanically (policy arg).
- DONE settle-level pins 2026-08-31: new
  `a_produced_world_completion_reads_produced_from_the_locator_key_surface`
  (applied surface → no mint/no limitation; contrast arm: default policy still
  reads by facet ref) + new controller-level
  `resuming_an_all_settled_run_is_a_settled_noop_in_the_produced_world`
  (FIRST resume = settled no-op in compose_produced world; default-world pin
  unmodified).
- STARTED COR-R2-2 (escalation queue identity binding) 2026-08-31.
- DONE COR-R2-2 code 2026-08-31: new `TicketIdentityMismatch` typed refusal;
  record()/poll() refuse a decoded record whose ticket field ≠ requested id;
  resolve() write-back targets the REQUESTED id; list() quarantines a record
  whose ticket field contradicts its file's stem; regression test
  `a_lying_ticket_file_cannot_clobber_another_tickets_record` (the falsifier's
  exact attack: refuse, victim untouched, victim poll-visible, listing honest,
  real victim still resolvable).
- STARTED COR-R2-3 (skills-lock strict reader) 2026-08-31.
- DONE COR-R2-3 reader 2026-08-31: read_lock strict — unknown root/entry fields
  refused, exact schema token, count equality, name-set equality with
  REPO_OWNED_SKILLS (+ duplicate refusal).
- DONE tests 2026-08-31: skills_roundtrip gains
  `the_lock_reader_refuses_the_forged_strict_shapes` (wrong token, lying count,
  consistent-count removed entry, unknown root field, unknown entry field,
  honest lock still round-trips); e2e_full_loop_mock gains leg 6
  `the_first_resume_of_a_fully_settled_run_is_a_settled_noop` (the falsifier's
  exact reproduction: exit 0, terminal=settled, zero iterations, export stream
  byte-identical, whole-scratch content-hash diff empty).
- COR-R2-1 SECOND DISCRIMINATOR (found by the new E2E leg, 2026-08-31): the
  in-app pins passed but the REAL-BINARY leg still reproduced budget_exhausted.
  Root cause: the composed plan fronts TWO facets (filesystem, test) but the
  produce step records ONE locator production per completion fronted by the
  FIRST facet's instrument — so per-facet INSTRUMENT matching could never cover
  the second facet (limitation on every anchor iteration). Fix: the B arm of
  `production_recorded` reads the locator's durable-production surface
  producer-agnostically (`evidence_productions_at(locator)` non-empty) — "this
  unit's work product has a durable production" is the world's own applied
  predicate; the record's instrument is the produce step's fronting choice.
  Module doc + settle pin comments aligned. Re-running app tests + E2E.
- gauntlet-app package tests: 466/466 green (first round, instrument-matched
  arm); escalation adapter 14/14 (incl. the clobber regression); cli targeted
  (skills_roundtrip 4 + escalations_cli 3) 7/7.
- DONE E2E red→green 2026-09-01: first run of the new leg FAILED with the
  falsifier's exact lie (exit 1, budget_exhausted iterations=4) while in-app
  pins passed → second discriminator found (multi-facet completion, record
  fronted by FIRST facet's instrument) → B arm made producer-agnostic → E2E
  10/10, app 466/466.
- DONE COR-R2 (all three corrections) 2026-09-01. Full battery:
  check 0 / boundaries 0 / lint 0 (after cargo fmt on new code; clippy clean
  throughout) / deny 0 / just test 1354/1354 / ruler PASS before+after /
  doctor 0 / e2e_full_loop_mock+cli_surface 10/10 / repo-root dry-run exit 0
  "zero writes ahead" with EMPTY before/after git-status diff.
- Report: .agents/reports/task20/cor-r2-report.md. Builder does not commit.
