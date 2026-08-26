# Task 13 recon — specification extraction (read-only agent, 2026-08-25)

Source: `.agents/specs/Core-spec.md` (2156 lines, v2). Quotes verbatim with line
numbers `L####`. Paraphrase marked **[Note]**.

## §16 Settlement — in full (L1016–1049)

> ### 16.1 What settlement requires
>
> A Unit settles only when all hold:
> 1. A `VerificationRecord` with `Outcome = PASS` exists for it in this run.
> 2. Its evidence ref resolves and is pinned in the evidence store.
> 3. Its `authority` permits closure by the actor closing it (`critic` → automatic; `lead_agent` → orchestrator with recorded rationale; `human` → operator action with recorded identity).
> 4. No blocking residual references it.
> 5. The reference digest is unchanged.
>
> A run settles only when every blocking Unit is settled and the reference's `settlement.minimum_reliability` condition is met.

> ### 16.2 Authority is not containment
>
> `authority` decides *whether* something may close. Containment decides *how contained the work was*. A perfectly sandboxed `cross_cutting` action that skipped human review is still a `cross_cutting` action that skipped human review — with better cleanup.

> ### 16.3 Atomic commit sequence
>
> In order, no reordering permitted:
>
> ```text
> 1. EventLog.append(SettlementEvent{unit_id, evidence_ref, verification_ref, digest, ts})  # durable, fsync
> 2. StateStore/answer-key write: status=settled, evidence=<ref>                             # file/db write
> 3. In-memory advance: release claim, update projections, notify surfaces                   # volatile
> ```
>
> Recovery: a crash between 1 and 2 is repaired by replaying the log on restart and completing the write **idempotently** (settlement events are keyed by `(run_id, unit_id, verification_ref)`; replaying twice is a no-op). A crash before 1 means the unit is retried as if verification never happened. The unit MUST NOT be released in memory before step 2 completes durably.

> ### 16.4 Single-writer discipline
>
> The runtime is the sole writer of Unit status and evidence fields. Builder and critic roles MUST NOT write the answer key even with general filesystem access; the file lives outside builder-writable workspaces or is mounted read-only. This is what makes the audit trail structural rather than aspirational — an agent that can rewrite the line that grades it has no grader.

**[Note]** §16 has exactly four subsections. Two crash windows are named in prose
(between 1–2, before 1); between 2–3 is covered structurally (step 3 `# volatile`,
MUST-NOT-release sentence, §18.3 step 3). `SettlementEvent` has no Annex B schema
(B.6 covers `settlement_record@v1` only). Replay key: `(run_id, unit_id,
verification_ref)`.

## §18 — in full (L1096–1124)

> ### 18.1 Substrate
>
> Prefer the simplest durable local substrate the environment already supports. […] Deterministic export to portable JSON/JSONL is REQUIRED regardless of backend.
>
> Minimum durable records: runs · iterations · role invocations · reference versions · observations · comparisons · verification records · residuals · payments · settlements · failures · recovery events · candidate revisions · promotion decisions · escalations.
>
> `RunId`s are collision-resistant and lexicographically sortable (ULID).

> ### 18.2 Checkpoints
>
> Every iteration boundary writes a durable checkpoint containing: run id, reference digest, iteration number, unit-state projection, budget spent, in-flight invocation ids with their result-contract paths, and the event-log offset. **Critical run state is never reconstructed from conversational memory** (I11).

> ### 18.3 Resume
>
> `gauntlet resume <run_id>`:
> 1. Replay the event log from the last checkpoint offset; complete any half-applied settlement idempotently (§16.3).
> 2. Re-freeze-verify the reference digest; mismatch → `aborted{reference_mutated}`.
> 3. Re-probe in-flight invocations: a valid result contract on disk is collected; anything else is marked `Failed{class: orphaned_by_restart}` and re-planned. No in-memory scheduler state is assumed to survive.
> 4. Sweep workspaces for settled/out-of-scope units.
> 5. Re-run preflight (§8.5) and continue.
>
> Run identity, prior evidence, and payment history survive restart intact. Re-running settled work is a defect, not a safe default.

> ### 18.4 Assume these failures
>
> Agent process dies · model times out · tool fails · MCP server disappears · network vanishes · test hangs · comparator emits malformed output · verifier disagrees with comparator · workspace becomes dirty · budget exhausted · reference invalid · environment changes mid-run. Each has a defined disposition in §11.5 and §21.

## Single-writer / answer-key / read-only — every touching section

- **§16.4** (above) — canonical rule.
- **§9.4** (L662–667): frozen copy in evidence store outside every builder-writable
  path; live answer-key file outside every workspace or mounted read-only;
  `verify_unchanged()` **at every iteration boundary and immediately before
  settlement**; mismatch → terminal `aborted{reference_mutated}`, both digests
  preserved; `CandidateRevision` MUST NOT be adopted mid-run.
- **§10.2** role table (L725–736): builder Never-column includes "answer-key
  status"; "Enforce with the environment where possible (filesystem permissions,
  read-only mounts, sandbox policy, separate credentials), not with prompt text
  alone."
- **§17.5** invariants (L1086–1092): "5. The evidence store, state store, frozen
  reference, and answer key are outside every builder-writable path."
- **§22.5** item 5 (L1248): "The grader is not writable by the graded. Answer key,
  frozen reference, evidence store, and event log are outside every
  builder-writable path."
- **§8.5** check 7 (L594): "State dir, evidence dir, workspace root writable;
  answer key path **not** inside any builder-writable workspace."
- **§11.4** step 3 (L842): operator hand-edit of an in-flight unit cancels the
  attempt; in-memory snapshot never authoritative over the file.
- **§5.3.2** (L293–298): `status = settled` requires non-null evidence ref AND a
  PASS `VerificationRecord` for this unit id in this run; tier MUST NOT be
  lowered by an agent.
- **§5.3.3** (L312): `units_state:` is "projection, rebuilt from events".
- **§9.2** (L637): writer-authority model "needs a small structured file with a
  single writer".
- **§23.2** G10 (L1293): "Let the builder write its own answer key".

## P7 / P11 and the proof table (§23.1, L1259–1276)

> | **P7** | Recovery | Kill the process after a committed iteration and mid-settlement. Resume completes the settlement exactly once and loses no run identity or evidence. |
> | **P11** | Critic/verifier separation | A comparator returning `verdict: pass` with no verification record does not settle a unit. |

Invariant mapping (§3): I1→P1 (digest re-verified before settlement); I3→P11
("Settlement reads verification records; comparator verdicts alone cannot close a
Unit."); I11→P7. I5: "Settlement, authority, budget, security, and irreversibility
gates read typed fields, never model prose." I8: "INCONCLUSIVE never satisfies a
settlement requirement."

> Each proof MUST have a **teeth check**: disabling the defense makes the proof fail. (L1276)

## §21 settlement/recovery-relevant rows (L1219–1238)

Failure loci include `settlement`, `persistence`, `recovery` (§21.1). §21.3:
"Agent session failure | Retry or escalate per §11.5; **never mark the unit
settled**"; "Integrity violation | Abort the run; preserve everything". §11.5
rows: reference digest mismatch → abort, no retry; authority ≠ critic for
required closure → escalate, never loop past it. §11.6 terminal `settled` row:
"Every blocking Unit `settled` with `PASS` verification and pinned evidence;
reference digest unchanged; settlement record durable".

## §15 handoff; §17/§22 placement

§15 selects corrections (inputs include verification results; per-candidate
estimates include "settlement accessibility"); selection + rejected alternatives
recorded per iteration. §17.1: authority escalation before any transform;
§17.2: strongest-available enforcement, prompt-only ⇒ `enforcement: advisory`.
§22.5 extends outside-writable to the event log (§16.3 step 1's target).

## SettlementService / verification input / critic-≠-verifier

- `SettlementService` appears exactly once: §4.1 application layer (L157–159).
  No dedicated section; §16 is its contract.
- §14.3 evidence binding validity (resolves; producer matches; produced in this
  run at/after the transform; locator matches) → else `INCONCLUSIVE` with
  `evidence_unresolvable` / `evidence_stale` / `evidence_producer_mismatch`.
- §13.1: comparator "never closes a Unit, and never issues a settlement".
- §14.4 / D3 / AK-031: comparator verdict alone never settles.
- §5.3.6 (L361–362): "`SettlementRecord` is append-only and references the
  event-log offset that made it durable."
- Annex B.6 `settlement_record@v1` (L1976–1983): `{schema, run_id, unit_id,
  verification_ref, evidence_ref, reference_digest, authority_decision:
  {required, exercised_by, operator_identity}, event_offset, settled_at}`.
- §20.2 (L1204): action endpoints perform the same §16.3 sequence, never bypass
  it; operator identity logged (AK-062).
- §5.3.1 reference manifest `settlement:` block: `success_criteria`,
  `required_evidence`, `minimum_reliability`, `terminal_conditions` (the §16.1
  run-level sentence's condition).

## Explicitly absent from the spec

"Two-phase commit"/"transaction"/"WAL" terms; a named 2↔3 crash window;
`SettlementEvent` schema; sections beyond §16.4; hyphenated "exactly-once"
(only "exactly once" in P7/AK-042); any lock protocol beyond §16.4/§17.5/§22.5
placement + AK-043 "prevented or detected".

## Summary

Five §16.1 preconditions: (1) PASS VerificationRecord in this run; (2) resolvable
pinned evidence ref; (3) authority permits closure by the closing actor (critic
auto / lead_agent orchestrator+rationale / human operator+identity); (4) no
blocking residual; (5) reference digest unchanged — run settles when every
blocking Unit settled AND `settlement.minimum_reliability` met. Commit order
fixed, no reordering: durable fsync'd event append → status/evidence write →
volatile in-memory advance; "MUST NOT be released in memory before step 2
completes durably"; crash repair by idempotent replay keyed `(run_id, unit_id,
verification_ref)`; crash before step 1 ⇒ retry as if verification never
happened.
