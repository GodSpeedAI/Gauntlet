# production-readiness — IN-FLIGHT ledger

Plan: `.agents/plans/production-readiness.md` (W1–W9). Branch:
`task26-dcritic-apparatus-repair`.
Status: **CLOSED 2026-09-15** — all nine workstreams settled, then re-proved.
The cross-session anchor is `.agents/CURRENT_STATUS.yml`
(`production_readiness`, status complete); the proof record is
`w9-final-proof.md` in this directory.

## Closure

- W1 (30e11c7): prime-agent rpc parent — loop-free cell, handle watch with
  early refusal detection, telemetry-driven liveness.
- W2–W4 (9f4f2e6, c3f1a2a, 524360b, be0174c): bounded admission retry with
  backoff, honest wall time plus in-flight budget guard, real timestamps,
  ceilings sized from a measured build.
- W5 (0443ead): seeded workspaces over a run integration tree,
  promote-on-settlement, `gauntlet deliver` plus `--apply` behind a drift gate.
- W6 (711cf70): detective containment guard — dispatch-time manifest,
  completion diff, `containment_breach` plus BLOCKING residual.
- W7 (251abcb): small correctness items (schema reminder, AK-9011 wording plus
  re-digested demo lock, state `run` from the latest run, doctor workspace
  root, held-out race).
- W8 (77bca63): docs and status.
- W9 (final commit + re-proof): final clean-seed live proof — terminal settled,
  3/3 units implemented-and-proven, delivery byte-identical to the promoted
  digest, `check.sh` exit 0 in the target; then re-proved 10:44 under the
  sharpened builder role, which exposed the remaining model failure mode as a
  unit-scope misreading (recorded in the status residuals).

## Interruption recovery (2026-09-15)

The previous session died on a usage limit one minute after launching the final
proof run. Reconciled before resuming: the orphaned run `HC64R8M6…` archived as
`targets/.runs/demo-calculator.interrupted-2313`; no stray processes, locks, or
uncommitted source work; the frozen ruler and the demo's own reference lock
both still matched; the launch/verify harness rebuilt under gitignored
`targets/` because `/tmp` had been cleaned out. Detail:
`w9-final-proof.md` §Reconciliation.

## Open (none blocking)

- **Demo-role sharpening.** The builder role text should say what to do when
  `scripts/check.sh` exits non-zero (fix and re-run; otherwise say so in
  `blocked_on` rather than claiming completion in `claims`). Deliberately NOT
  applied after the proof: the template's role text is part of the proven
  configuration, so changing it requires re-proving the demo loop.
- **Informational.** Two builders invoked `check.sh` by an absolute path
  outside their workspace (the `examples/` and `targets/` copies) — a live
  instance of the recorded detective/advisory containment boundary for the
  no-backend prime-agent admission.
- **Carried.** agent-prime adapter flake unreproduced in isolation;
  `conformance.json` and `handoff.md` stay frozen as task-27 settlement
  evidence.

## Next move

Nothing outstanding on this plan. Either the demo-role sharpening above as the
next demo-authoring touch (then re-run the proof, since the role text is part of
the proven configuration), or resume the `tui_gui_redesign` stream — increment 2,
GUI shell primitives per `.agents/reports/tui-gui-redesign/03-hallmark-redesign.md`
§8 step 2.