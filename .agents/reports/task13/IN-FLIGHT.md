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
| B2 app `settle::` service + controller wiring + resume completion (A1/A2/A4/A6/A7) | `builder-b2.md` | DIED to usage limit mid-Item-1 (16th instance; reset 16:23:55 EDT 08-26). Recovered per unrecorded-landed-work: settle/mod.rs LANDED-PARTIAL (tests 4/16, all fixture-level digest-mismatch failures — never green), Items 2–5 unstarted. FINISHER brief ready (`finisher-brief-b2.md`); dispatch when the window opens |
| B3 single-writer arch rule + re-litigation record (N4 + standing rule) | `builder-b3.md` | pending |
| B4 proofs p7/p11/g10 + teeth | `builder-b4.md` | pending |
| Critics A (spec) + B (adversarial), blind via explore agents | `critic-{a,b}-result.md` | pending |
| Correction round(s) as required | `correction-roundN.md` | pending |
| Fresh blind confirmation | `final-confirmation-roundN.md` | pending |
| Gates + teeth re-verified by orchestrator; one-task-one-commit | — | pending |

After any interruption: `just ruler` first; `scripts/reconcile-checkpoints.sh 13`;
reconcile `git status` vs CURRENT_STATUS.md vs this file before treating any
slice as unstarted; resume at the missing step per
`.agents/lessons/session-interruption-recovery.md` and
`.agents/lessons/unrecorded-landed-work.md` (the tree outranks the record,
including when the tree says MORE was done). Never redo recorded work.
