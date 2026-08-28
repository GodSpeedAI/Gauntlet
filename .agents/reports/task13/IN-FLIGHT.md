# Task 13 — IN-FLIGHT (orchestrator dispatch ledger)

Task 13 — Settlement, atomic commit, single-writer discipline (proves P7, P11).
Targets: AK-040, AK-041, AK-042, AK-043, AK-062 (service-level half); AK-031
rides with P11; AK-024 settle-time half upgrades; AK-053 claim assessed at
closure. Prereg frozen (sha256 d949e1e0…) before implementation began.
Recon: `recon-{spec,answerkey-residuals,code}.md`; inherited-trigger
adjudication: `residual-adjudication.md` (A1–A7 compose, N1–N4 re-grounded).

Loop plan (one builder at a time — single-compiler/build-lock discipline):

| Slice | Result contract | State |
|---|---|---|
| B1 ports settlement surface + earned admission + store disciplines (A1/A2/A3/A5 store legs) | `builder-b1.md` | RETURNED 2026-08-26 00:40; orchestrator re-verified: ruler PASS, reconcile clean, ports 94/94, sqlite 46/46, lint 0 |
| B2 app `settle::` service + controller wiring + resume completion (A1/A2/A4/A6/A7) | `builder-b2.md` | CLOSED 2026-08-26 20:3x: builder (16th instance) + finisher (17th, Items 1–5 DONE fail-first) both died to limits; operator committed the mid-edit state `62d1a5f`; operator then instructed heal+continue — orchestrator completed Phase R (boxing refactor, 3 sites + clippy×4 + fmt) and Phase G (ruler PASS; app **323/323**; proofs **31/31**; lint 0; boundaries **197/197**; zero warnings) and wrote Phase C `builder-b2.md`. All evidence in checkpoints |
| B3 single-writer arch rule + re-litigation record (N4 + standing rule) | `builder-b3.md` | RETURNED 2026-08-26 ~21:0x (parallel dispatch batch: B3 builder + R1 re-litigation + R2 early probe). Orchestrator verified: ruler PASS, boundaries **215/215** (pinned growth), reconcile clean. arch-relitigation.md landed from R1 (all four HOLD, one re-record). R2 early probe: **1 CRITICAL + 5 MAJOR + 5 MINOR** at the settlement seam (prereg P2/P4 FIRED) → `early-adversarial-probe.md` + `early-probe-adjudication.md`: COR-1..7 fix-now, M4/m3/m4/m5 later-scope w/ triggers |
| Correction round 1 (COR-1..COR-7) | `correction-round1.md` | CLOSED 2026-08-27: builder died mid-COR-1 (18th instance; recovered+completed by orchestrator per standing heal instruction, suite fixture made coherent) → finisher landed COR-2..7 fail-first each + re-derived COR-1's lost evidence (scratch-worktree vs HEAD). Orchestrator verified: ruler PASS, reconcile clean, ports 103/103, app 329/329, proofs 31/31, lint 0, boundaries 215/215 |
| B4 proofs p7/p11/g10 + teeth | `builder-b4.md` | DIED mid-Item-1 to the limit (19th instance; window resets 2026-08-28 01:25:31): p7_recovery.rs landed NON-COMPILING, Items 2–4 unstarted; recovered+recorded; finisher brief ready (`finisher-brief-b4.md`). Reference-candidates research RETURNED (drafts held for closure). BLIND CRITIC round 1 (landed surfaces) RETURNED + adjudicated: COR2-1..6 fix-now (`correction-brief-r2.md`), M-1 minimum_reliability → reference candidate + deferral, rest trigger-recorded. One-shot automation armed for post-window: B4 finisher → COR-R2 → confirmation |
| Correction round 2 (COR2-1..6) | `correction-round2.md` | CLOSED 2026-08-27: three dispatch deaths left a partial unrecorded window (1a guard, 2, 3 — recovered); orchestrator completed 1a-test, 1b guard+test, 4 kind-gate, 5 replay-divergence (+suite case + green stub), 6 doc per the standing instruction. FINAL BATTERY: ports 105/105, app 330/330, sqlite 48/48, proofs 12/12, standing 33/33, lint 0, boundaries 215/215, deny ok, ruler PASS |
| Fresh blind confirmation round 1 (spec + falsifier, COMPLETE surface incl. B4 + COR-1 + COR-2) | `final-confirmation-{a-spec,b-falsifier}.md` | RETURNED 2026-08-27: Role A spec critic **CONFIRM** (5 minors RES-A1..A5); Role B falsifier **NOT CONFIRM** — F-1 MAJOR (executed: ungated `append_event`/`EventLog::append` of `SettlementCommitted` + replay mints a durable settlement with zero conditions; poisons projection, locks out honest settle). Both ran the operator's role charters verbatim (`.agents/agents/{critic,falsifier}.agent.md`; general-purpose transport — typed role agents still not invocable, re-probed 2x this session). Adjudication: NOT CONFIRM → COR3; F-2/F-3/RES-A1..A5 trigger-recorded → `final-confirmation-round1-adjudication.md` |
| Correction round 3 (COR3: gate `SettlementCommitted` on both public append surfaces) | `correction-round3.md` | CLOSED 2026-08-28: builder landed gates+typed errors+earned half-step then died to the limit (20th instance; unrecorded — recovered+verified by orchestrator); orchestrator completed contract suites+stubs, fixture conversions, caller audit, gates (recovered-honest RED 0/2 → GREEN 2/2; ports 105, app 330, sqlite 50, proofs 12, standing 33, lint 0, boundaries 215, deny, ruler, prereg) per standing heal+continue |
| Fresh blind confirmation round 2 | `final-confirmation-round2-{a,b}.md` | RETURNED 2026-08-28: Role A **CONFIRM** (7 minors FC2-A1..A7), Role B **CONFIRM** (1 minor F-1 instrument-route seam + 3 observations, executed). Combined **CONFIRM** — adjudication + settlement basis in `final-confirmation-round2.md` |
| Closure: gates+teeth re-verify, reference-candidates paste (9 entries, verified), task-13.outcome.yaml (P1-P4 supported, P5 falsified-honestly), ONE commit w/ explicit pathspecs | `final-confirmation-round2.md`, `.agents/reports/reference-candidates.md`, `.agents/research/preregistrations/task-13.outcome.yaml` | **DONE 2026-08-28 — TASK 13 SETTLED.** Teeth: COR3 gate tooth + B4 Tooth 1 re-run personally (RED→byte-identical→GREEN); gates: ports 105, app 330, sqlite 50, proofs 12, standing 33, lint 0, fmt, boundaries 215, deny, ruler, prereg-check 13 |
| B4 proofs p7/p11/g10 + teeth | `builder-b4.md` | pending |
| Critics A (spec) + B (adversarial), blind via explore agents | `critic-{a,b}-result.md` | pending |
| Correction round(s) as required | `correction-roundN.md` | pending |
| Fresh blind confirmation | `final-confirmation-roundN.md` | pending |
| Gates + teeth re-verified by orchestrator; one-task-one-commit | — | pending |

After any interruption: `just ruler` first; `scripts/reconcile-checkpoints.sh 13`;
reconcile `git status` vs CURRENT_STATUS.yml vs this file before treating any
slice as unstarted; resume at the missing step per
`.agents/lessons/session-interruption-recovery.md` and
`.agents/lessons/unrecorded-landed-work.md` (the tree outranks the record,
including when the tree says MORE was done). Never redo recorded work.
