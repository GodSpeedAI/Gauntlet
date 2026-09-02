# COR-F progress (checkpoint protocol — declare-first)

- DONE recon 2026-08-31: all six loci read (controller ProducePolicy + resume_run +
  drive, bundle.rs FacetRecord, observe-test adapter, verify-test-report rung,
  scenario.rs load_scenario, human_escalation.rs TicketId, escalation-fs queue,
  skills_cmd.rs, escalations_cmd.rs, e2e_full_loop_mock.rs, fixture target).
- STARTED COR-F1 (controller gate) 2026-08-31.
- COR-F1 exact corroboration rule (implemented): in `ProducePolicy::BindLocatorContent`,
  a unit with `evidence_required.producer = instrument` whose governed verification
  decided PASS may settle only when THIS completion's observation bundle carries a
  facet record with kind `test`, a present `command`, and `exit_status == 0`, and the
  bundle's run_id/iteration_no equal this run and iteration (the instruments executed
  the composed test command over the evidence workspace that holds the locator
  artifact). No such record ⇒ typed pinned `producer_uncorroborated` record, unit not
  settled, run continues to a bounded non-settled terminal.

- DONE COR-F1 controller: `produced_artifact_corroborated` rule fn + settlement gate in
  process_completion (BindLocatorContent + producer=instrument + PASS only; pins
  `producer_uncorroborated`, skips settle). Default MaterializeObservation world untouched.
- DONE COR-F6 controller: all-settled resume short-circuits to terminal=settled at the
  checkpoint's iteration, zero writes, before in-flight re-probe; predicate requires
  empty in_flight + every frozen-membership unit Settled (p7/g7 crash paths unaffected).
- DONE COR-F2 scenario.rs: load_scenario stem grammar (empty/'/'/whitespace/separators/
  non-Normal components) BEFORE any join + 2 tests.
- DONE COR-F3 ports TicketId grammar (no separators, Normal components) + queue
  contained_ticket_path on read (record, poll) AND write (resolve, escalate) +
  contract-suite tests (grammar, out-of-root read refusal, forged-record write-back
  refusal).
- DONE COR-F4 queue.list() -> QueueListing {records, quarantined} (QuarantinedTicket =
  stem + QuarantineClass, no content echoed); CLI prints QUARANTINED entries, exit 0;
  contract-suite tests (quarantine beside valid, unreadable class, action fail-closed).
  Mechanical adaptation: the_list_is_total_ordered... test updated to .records (reason:
  list surface change is the correction).
- DONE COR-F5 skills_cmd install: writer emits root `skills` array (reader's schema).
- LANDED e2e: fixture GAUNTLET.md facets [filesystem, test] + docs; scenario
  e2e-forged-report.yaml; E2E leg `the_report_forging_liar_is_never_settled` (asserts
  bounded non-settled terminal, per-unit implemented-and-proven negative, settled=0,
  zero export settlement rows, forged bytes in store + producer_uncorroborated pinned);
  world.env now sets GAUNTLET_TOOL_TEST=bash <world>/scripts/check.sh.
- TODO: skills_roundtrip.rs test file (written, pending compile), controller COR-F1
  both-ways tests (written, pending compile), then battery.
- NEXT: just check results -> fix fallout -> package tests -> battery.
- PROGRESS 2026-08-31 (compile/test status):
  - just check: GREEN (all crates).
  - gauntlet-adapter-agent-mock 73/73, gauntlet-adapter-escalation-fs 13/13,
    gauntlet-ports 112/112: GREEN (incl. COR-F2/F3/F4 new tests).
  - gauntlet-app 464/464: GREEN (incl. corroboration rule pin, both-ways world tests,
    resume no-op pin; existing half-applied reconciliation + budget-watermark proofs
    stayed green UNMODIFIED).
  - e2e_full_loop_mock 5/5 + skills_roundtrip 3/3: GREEN — the forging liar leg
    (`the_report_forging_liar_is_never_settled`) bites; happy leg settles (corroborated).
  - COR-F6 design note: short-circuit predicate = completion-noop + in-flight-empty +
    event-ordered projection all-settled (NOT raw settlement rows: a checkpoint may
    legitimately project Open postdating a settlement — residual-retry class; the
    budget-watermark proof pins that and stayed green).
- RUNNING: full gauntlet-cli package suite.
- NEXT: remaining battery (test/lint/deny/boundaries/ruler/doctor + full e2e + cli_surface
  + repo-root dry-run zero-write), then cor-f-report.md.
- COMPLETE 2026-08-31. All six corrections landed + verified. Battery: check clean,
  just test 1349/1349, lint clean, deny clean, boundaries 228/228, ruler PASS,
  doctor OK, e2e_full_loop_mock+cli_surface 9/9, repo-root dry-run zero-write
  (content-hash proven). Report: cor-f-report.md. No frozen file touched; builders
  did not commit. No in-flight work remains.
