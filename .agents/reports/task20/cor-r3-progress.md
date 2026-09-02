# COR-R3 progress (declare-first checkpoint trail)

STARTED cor-r3 session 2026-09-01 — read charter, falsifier-round3, adjudication-r3, cor-r2-report; work space verified quiet (no compile.lock holders, no cargo procs); `just ruler` (before) commissioned next.

STARTED cor-r3-1 (ports + sqlite + settle restructure + controller predicate + tests) — code read complete; design: produced-world recovery walks units/locators and mints the durable orphan event's own ref via a new fail-closed StateStore read `pending_evidence_production_events`; default-world facet walk kept byte-identical except honest limitation text; settled no-op predicate drops the limitations clause (forensic pins are not state movement).
DONE reading phase.
DONE cor-r3-1 core edits: ports (OrphanEvidenceProduction + pending_evidence_production_events fail-closed default), sqlite impl, settle per-world arms (default world text-honest; produced world locator walk, settled-unit gating, per-locator forensics), controller predicate drops limitations clause, FakeState/SettleState fixture impls + helpers, 4 new settle tests + 1 new controller test. Package check running.
DONE cor-r3-1 first verification: 10/10 green (5 new tests + shipped pins unmodified) via nextest filter on gauntlet-app.
DONE cor-r3-2 code edits (skills_cmd source anchor + roundtrip test); check clean, tests pending.
DONE cor-r3-3 code edits (writer_lock module + run/resume wiring + lib export).
STARTED e2e legs (rusqlite dev-dep for damage injection; writer-lock legs; damaged-rows leg).
DONE package tests: sqlite 50/50, app 471/471, cli 90/90, ports 112/112. E2E mock suite 9/9 (3 new legs green, 6 shipped legs unmodified). STARTED full battery.
DONE cor-r3 session — all three corrections landed and pinned; battery green:
just test 1365/1365, boundaries PASS, lint PASS, deny PASS, doctor PASS,
ruler PASS before+after, e2e_full_loop_mock+cli_surface 13/13, dry-run
zero-write (git diff 0 bytes), packages: ports 112, sqlite 50, app 471,
cli 90. Report: cor-r3-report.md. Notes: denylist/deps gates correctly
refused a rusqlite dev-dep -> damage injection moved to the adapter's
doc-hidden test_support (crash_helper precedent); scope-escape scanner
false positive on `let path = ...` resolved by renaming locals; b5 flaky
test failed once under load, green in isolation and both later full runs
(known class).
DONE cor-r3-1.
DONE cor-r3-2.
DONE cor-r3-3.
