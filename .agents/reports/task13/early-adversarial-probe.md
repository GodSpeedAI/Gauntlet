# Task 13 — early adversarial probe of the settlement seam (read-only agent, 2026-08-26, pre-B4)

Static, bounded battery (no compile, no runtime). Scope: ports
`producer_authority.rs` + `state_store.rs` settlement surface; app
`settle/mod.rs`; controller PASS-leg/resume/disposition wiring; the sqlite
adapter's settlement implementations. Full method and search battery as the
agent reported; findings verbatim below. This ran BEFORE the formal blind
critic round (which still covers the whole task surface in fresh context);
it exists to shorten the correction loop before B4 builds proofs atop the
seam. Prereg P2/P4 FIRED (as predicted: R1-class seams at the settlement
boundary caught by critique, not by the green in-crate suite).

## CRITICAL

**C1 — `record_verification` is an un-minted owner write.**
`state_store.rs:526-534` accepts a raw `VerificationRecordDoc`
(`VerificationRecordDoc::new` is public). Unlike commit_settlement /
record_evidence_production / bind_observation_bundle there is no
`Admitted*` token. Attack: any `Arc<dyn StateStore>` holder builds a PASS
doc (any unit, any evidence ref), `record_verification`, then
`SettlementCommitAuthority::admit` (reads exactly those records) then
`commit_settlement` — all of §16.1 bypassed. Shipped executable sequence:
`crash_helper.rs:195-219`. Aggravator: inherent pub
`apply_settlement_row` (adapter `state_store.rs:753-771`) writes a
settlement row from a caller-built commit with no admission at all;
`append_event` + constructible `RunEventKind::SettlementCommitted` complete
a gate-free fabrication path.

## MAJOR

**M1 — the §16.3 key's `verification_ref` is caller-invented, tied to no
durable fact.** `admit` filters records by unit+evidence only
(`producer_authority.rs:526-572`); `VerificationRecordDoc` has NO
verification-ref field, so the durable PASS cannot name the verification
the commit claims. Refs flow from caller-built
`format!("{}-{}", run_id, invocation_id)` (controller.rs:2414-2419). With
one honest PASS, N distinct refs commit N distinct-key settlements (each
appending a durable event; `insert_settlement` is INSERT OR IGNORE and
`commit_settlement` appends unconditionally — duplicate-key commit returns
Ok with a silently-deduped row). Also wins any race between admit's read
and the commit (no atomicity).

**M2 — `AlreadyCommitted`-as-success can bless divergent evidence.**
`settle/mod.rs:644-652` discards the stored row's fields; the key match is
(unit, verification) only. A later readiness with the same verification
ref but evidence E2 (durable PASS for E2 written first by
`ensure_verification_recorded`) hits AlreadyCommitted → success with the
old offset while the in-memory advance names E2 vs the row's E1. The
divergent shape is untested (fixture comment admits the reuse).

**M3 — claim↔readiness pairing unchecked; the claim's own PASS is enforced
only AFTER the durable commit.** `SettleRequest` takes them as independent
fields; nothing re-derives the pairing (readiness is portable between
claims). `settle()` never checks `claim.record().outcome()`; the only PASS
enforcement over the claim is the domain transition, which runs after
event+row+pin. Attack: FAIL-claim + stale PASS-readiness for the same unit
→ settlement commits durably, domain refuses, controller leaves the unit
open, resume reconciliation marks it Settled — durable-settled vs
in-memory-open divergence manufactured through the service's own ordering.

**M4 — admission is iteration/recency-blind; a replayed PASS satisfies
condition 1.** Neither `admit` nor the service reads
`record.iteration_no()`/`produced_at`. A unit re-opened at iteration k
(verification FAILED) can settle over its iteration-j PASS (j<k) with the
old evidence. §16.1's "exists… in this run" arguably permits; the §14.3
recency discipline lives in the verification chain and the settle path
never re-consults it.

**M5 — `admit` trusts the store's run-scoping; no defense-in-depth on
`record.run_id()`.** Filters only `unit_id` over `verification_records(
run_id)`; never compares `record.run_id()` to `commit.run_id`. The ports'
own test fake ignores run_id entirely — a conforming-but-sloppy store can
feed foreign-run PASS records to admission.

## MINOR

- **m1** — non-conforming-store read defaults silently under-inform:
  `settlements()`/`verification_records()` default `Ok(empty)` while write
  defaults fail closed → re-entry appends duplicate events; the
  disagreement check never fires.
- **m2** — operator identity interpolated raw into the pinned
  authority-decision JSON (`settle/mod.rs:1181-1191`); crafted identity
  injects members. (`pin_limitation` escapes; this site doesn't.)
- **m3** — orphan-production recovery re-derives the content digest from
  bytes at recovery time (sound on the content-addressed adapter; the port
  does not mandate content addressing; ports fakes are name-addressed).
- **m4** — transient poll-error disposition never escalates on its own;
  bounded only by rounds exhaustion (per-unit ceiling absent, unlike the
  residual arm).
- **m5** — verification-stage claim/requirement built from the RE-READ
  answer key (controller.rs:2335, 2426-2429), service checks only unit
  existence in the frozen projection; edit+revert window between boundary
  verifies.

## Seams that HELD (verified — the critic round need not re-litigate)

Condition-5 non-tautology (file-derived verify_unchanged vs frozen
digest); in-memory advance strictly after the durable commit (the only
production settle site + resume reconciliation, grep-cited); no production
caller reaches the settlement write outside the service; frozen-unit
grounding through the digest-verifying projection; degraded iterations
cannot float settlements (I8 holds); `settle_operator_action` has no
bypass (zero non-test callers, always refuses); replay cannot fabricate
rows (field-for-field rebuild from durable events, contiguity enforced,
event-derived timestamps).

Not statically testable: compilation, runtime/DB behavior, fsync
atomicity, concurrency interleavings, real CLI composition (the CLI main
wires only doctor; controller composition is test-side — consistent with
prior tasks' proof-driven composition).
