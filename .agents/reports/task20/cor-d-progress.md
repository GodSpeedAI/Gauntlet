# COR-D progress (correction builder COR-D)

Work order: design-adjudication.md (COR-D1..D10 + doc nits + 2 RECORD entries
in reference-candidates.md). Order: D3, D2, D1, D4-D6, D7+nits, D8, D9, D10,
records. Battery after each MAJOR, full battery at end.

## Pre-flight

STARTED 2026-08-31: pre-flight — no live compile.lock holder, no running
cargo/rustc/sccache processes; tree carries task20 uncommitted work on HEAD
(main @ d447a22). Reading key files.

## COR-D3 (R-1b behavioral half)

STARTED 2026-08-31: COR-D3. Design settled: (1) mock adapter resolves effects
under constructed_root/<spec.workspace> with key-class validation; (2) CLI
adapter resolves execution root likewise for cwd + {workspace} + prompt;
(3) controller process_completion anchors produce-locator + verification
for_run at the flight's per-unit workspace when a provider is wired
(workspaces: None proofs keep byte-identical behavior); (4) E2E artifacts
assertions adapt (per-unit workspaces are RELEASED after each completion, so
post-run byte-identity moves to the durable store: ingested artifact bytes +
observation manifests + absence of the shared-root leak); (5) teeth: adapter
red-when-bypassed tests. Note: release-after-completion means operator verify
re-reads anchors at the canonical root and will report INCONCLUSIVE where the
locator resolved only inside a released per-unit workspace (feeds the RES-11
locator-semantics record).

COR-D3 progress: adapters + controller + E2E + proof-construction adaptations
LANDED (compile-clean). Adapter tests 71/71 mock, 58/58 cli PASS. Pending:
p2/p3/p5/p6/g7 test run (in flight), E2E run, then battery. Assertion changes
so far (all mechanical, listed for the report): mock playback paths now read
under root/<WS_REF>; p5/p6/g7/p3/p2 mock constructions use the canonical root
so effect paths are unchanged on disk; p5 unused import removed; E2E
artifact/planted-reality assertions moved from released per-unit workspaces to
the durable store (exact-object + manifest scans) + shared-root-leak absence;
verify-leg comment updated (PASS -> honest INCONCLUSIVE, no assertion change).

COR-D3: p2/p3/p5/p6/g7 adapted binaries 24/24 PASS (cord3-test3.log; exit-2
was a sccache stop-server timeout after tests passed). Running e2e_full_loop_mock.

## COR-D3 completion + COR-D2 controller work

DONE (checkpoint 2026-08-31): COR-D3 fully landed incl. the ladder-factory
correction: the §14.2 rung set is now a ControllerDeps `ladder_for` factory
(LadderFactory = Arc<dyn Fn(&Path) -> Result<Ladder, String>>, no Send+Sync —
the loop is single-threaded); the controller memoizes one ladder instance per
anchored workspace (stateful rung instruments keep their cross-iteration
state; provider-less proofs keep byte-identical behavior); compose.rs hands
the factory; ControlError::Ladder added. Root cause of the first E2E failure:
rung adapters were anchored at the canonical root while produce/14.3 read the
per-unit workspace — instruments saw no artifact.

COR-D2 landed in controller.rs: LoopState.carried + iteration_binding_spent;
drain carries surplus completions (pinned `completion_carried` evidence;
reservation+workspace held); drive absorbs ONE carried completion per
iteration right after boundary_verify (at that iteration's own anchor);
checkpoint in_flight includes carried (§18.3 re-probe surface); stranded
carried released at terminal exit; plan_and_dispatch accounts carried in
owes_work/claimed/global_in_flight/pending-slice/rounds-gate. E2E leg 4
(`parallel_completions_carry_and_never_collide`, GAUNTLET_MAX_PARALLEL=2)
added. Test run in flight (cord2-test1.log).

DONE 2026-08-31: COR-D3 + COR-D2 batteries GREEN: gauntlet-app 460/460,
gauntlet-cli 71/71, e2e_full_loop_mock 4/4 legs (incl. new
parallel_completions_carry_and_never_collide). RECORD entries appended to
reference-candidates.md. Starting COR-D1.

## COR-D1, D4, D5, D6, D7, D8, D9, D10

STARTED+LANDED 2026-08-31 (pending compile/test): D1 roles list (app-layer
`roles::list` read-only over the strict-parsed lock + CLI arm + usage line);
D7 usage lines fixed (status --run required; inspect full flags; roles list
added) + cli_help.txt golden regenerated DELIBERATELY + cli_surface.rs golden-
path doc nit + improve_cmd now CONSULTS report_manifest::is_wired (RES-15,
agreement by construction with a loud drift refusal); D8 doctor check-3 reason
reworded token-neutral; D9 diagnostic allowance mapping moved to
ResolvedBudget::diagnostic_allowance in gauntlet_app::config::resolve (CLI
passes scalar through; compose/run_cmd updated); D6 dry-run zero-write tooth
now hashes file CONTENTS (DefaultHasher per file, before/after sets);
D10 happy-leg report assertions now PARSE the report JSON for unit->category
pairs (no exact prose); lying-leg vacuous contains() removed; D4 teeth battery
new test file compose_teeth.rs (unset token names key+env+no store; unknown
token names fronted set; mock-without-scenario names the hook; live-backend
wrap refusal names unit+Task 25, honest skip when bwrap missing);
D5 new test file escalations_cli.rs — NOTE: adjudication wording vs shipped
gate: settle_operator_action structurally NEVER returns Ok (AK-062 pins
no-bypass; full operator surface is Task 21), so the legs pin (1) resolve
without records -> PASS-gate refusal, ticket open; (2) retry with non-PASS ->
same gate; (3) earned durable PASS -> STILL refused (no chain-earned
readiness), zero settlement rows (D9 single writer held); the
queue-write-failed CLI branch is unreachable end-to-end until Task 21 —
recorded in the test doc + the report (fog for the orchestrator).
Compile in flight.
