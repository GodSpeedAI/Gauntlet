# TUI runtime repair

Status: settled, 2026-09-10. All seven TUI-INSPECTION.md findings fixed and verified end to end.

Dispatches: renderer builder owns OpenTUI rendering/input adapter and tests; host builder owns canonical host composition and tests. Orchestrator owns integration contracts and verification. Existing working changes and historical settlement evidence are preserved.

Result contract: each builder writes `repair/<role>-result.json`, an object with `status` (complete or blocked), `files` (string array), `tests` (string array of observed commands/results), and `remaining` (string array). Completion signals alone are not accepted.

Acceptance: actual release entry connects configured canonical reads, routes supported actions through gateway, renders all shell content with bounded terminal geometry/styles, handles keyboard/Unicode/Escape, restores terminal, and has production composition regression evidence. No new dependency or authority semantics. Required gates: tui-check, tui-test, tui-lint, tui-preflight bootstrap; graft rebuild after code changes.

Next: none — settled. Repository handoff targets written (.agents/CURRENT_STATUS.yml tui_runtime_repair=settled, .agents/plans/tui-runtime-repair-followup.md). Release evidence rebuilt (dist/gauntlet-workbench sha256 1cb5a1b3, T20 frame written); complete suites green (tui-test 611/0/12, tui-test all 49 suites 530/0/9); graft build green; no Rust changes so just verify not required. Bounded transport residuals recorded in CURRENT_STATUS.yml and the followup plan.
