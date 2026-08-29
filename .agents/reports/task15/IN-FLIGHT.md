# IN-FLIGHT — Task 15 (Prime-Agent adapter)

- **Started:** 2026-08-29T20:50Z (orchestrator session)
- **Phase:** builder dispatched
- **Prereg:** frozen `61a0841d3e49a73915d5c7ec4477df6203c7e7ab96c5cf3673b7fa0e981a0915`
  (`just prereg-check 15` PASS at freeze); recon at `recon.md`.
- **Baseline:** HEAD 6f133a8 at dispatch; `just ruler` PASS at dispatch.
- **Slice plan:** ONE builder, two phases —
  B1 adapter crate (transport seam, kernel-program renderer, runner, probe,
  suite integration over doubles, env-gated live probe, arch await-ban test
  with negative control); B2 doctor check-3 resolution half via
  measured-descriptor wiring in the composition root.
- **Checkpoints:** builder writes `b1-progress.md` as it goes; final report to
  `builder-b1.md` (result contract: file exists + names every gate it ran).
- **Design review round (2026-08-29, pre-implementation):** blind critic
  (critic-design.md) returned NOT_CONFIRM — 1 blocking / 5 major / 6 minor;
  ALL accepted (design-adjudication.md); corrections routed to the builder
  mid-flight (RES-D1 invoke return point is the blocking one: return on
  handle-parse, never parent-run end). Recon corrected in place (RES-D1
  shape, RES-D10 typo, RES-D11 claim narrowing). Falsifier round must check
  the corrected shape, including the RES-D1 live-probe evidence leg.
- **Hard boundary:** NO real builder execution this task (R-1b stays open,
  bound to first composed builder execution). Suite runs over transport
  doubles only; the env-gated live probe spawns a trivial file-writing child
  through no gate.
- **Gate phase (2026-08-29 evening):** builder B1+B2 landed; orchestrator
  battery green (1108/1108, boundaries 220/220, lint/deny/doctor/ruler/
  prereg PASS; suite byte-identity self-verified). Live probe round 1:
  FAIL honest → COR-D1 (version on stderr) landed, 22/22. Live probe
  round 2: FAIL honest at admission → diagnosed by execution (6 parent
  probes + 1 real child) → COR-D2 routed (str coercion of handle fields;
  in-cell parent linger 30s default + detached reaper; `{"sessions":[...]}`
  wrapper parse). Live probe round 3: PASS (35.4s, real admission + D1 leg).
- **Blind falsifier round 1 (2026-08-29):** CONFIRM — 0 blocking / 2 major
  (F1 missing handle-decode teeth, F2 missing cancel-second-effect teeth) /
  1 minor (F3 late-writer shape untested in-tree). All 13 attacks repelled;
  live probe independently PASS (40.4s); hygiene proved (191-file sha256
  baseline diff empty; doctor OK; ruler PASS; zero strays). COR-D3 (tests
  only) routed: teeth for F1/F2/F3. After it lands: orchestrator personally
  re-applies mutations M2/M7 (RED) and restores (GREEN), re-runs the full
  workspace battery, then settles.
- **Recovery note:** if this session dies, reconcile `git status` + diff
  against this file and `b1-progress.md` before re-dispatching (tree
  outranks record). One compiler at a time; package-scoped checks first.
