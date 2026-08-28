# Task 13 — FRESH BLIND CONFIRMATION, Role B (falsifier)

Date: 2026-08-27. Charter: `.agents/agents/falsifier.agent.md` (adopted verbatim).
Brief: `.agents/reports/task13/confirmation-brief.md` (Role B section).

## Blindness statement

I read nothing under `.agents/reports/task13/` except the brief, and nothing
under `taskN/**`, `CURRENT_STATUS*`, `.agents/lessons/**`,
`.agents/research/**`, `reference-candidates.md`, or `escalations.md`. I did
not read the sibling critic's report. One unavoidable disclosure: the
artifact's own source comments narrate prior correction rounds (COR-1..COR-7,
"critic round 1, F-1", etc.). Those comments are part of the authorized
artifact, but they did tell me where earlier attacks landed; my probes were
constructed independently of any prior report content.

## What I confirmed holds (executed)

- `cargo nextest run -p gauntlet-cli --test p7_recovery --test p11_critic_is_not_verifier`
  — 10/10 PASS (commit order, all three kill windows, re-resume, PASS control,
  broken/absent/INCONCLUSIVE/FAIL refusal legs).
- `cargo nextest run -p gauntlet-app settle::` — 22/22 PASS.
- `cargo nextest run -p gauntlet-cli --test g10_builder_writes_answer_key` — 2/2 PASS.
- `cargo nextest run -p gauntlet-arch-tests` — 215/215 PASS (includes
  `answer_key_single_writer`).

The composed service path is real and correct: `SettlementService::settle`
re-derives conditions 1–2 over the claim itself (no readiness parameter,
`settle/mod.rs:646-663`), grounds the run and FROZEN unit on durable state
(`:666-672`), enforces the authority table (`:674-680`), tracked+durable
blocking residuals (`:682`), a genuine provider file re-read for condition 5
(`settle_time_verify`, `:1145-1183`, backed by `FileReferenceProvider::
verify_unchanged` at `reference/provider.rs:292-326` — re-reads the artifact
and re-digests; not a tautology), then commits through
`SettlementCommitAuthority::admit` (durable PASS + evidence + verification-ref
mooring, one-settlement-per-unit) into the store's event→row sequence, with
the in-memory advance last (`:714-730`). The controller calls only this
service (`controller.rs:2515`) and advances its projection only after the
durable commit (`:2523-2529`). The attempted-list surface is closed
(`controller.rs:2356-2381`: declared non-flight units are pinned as
governance evidence and skipped). Builder-declared lists cannot reach another
unit's settlement. Raw-written PASS docs are structurally refused
(`record_verification` takes `AdmittedVerificationRecord`;
`append_record` refuses `RecordKind::VerificationRecord` for every caller,
sqlite `state_store.rs:805-822`; `apply_settlement_row` is `#[cfg(test)]`,
`:768-780`). Same-key divergent replay is loud (probe below). Timestamps are
service-clock-minted and store-cross-validated (`verify_settlement_event_at`,
sqlite `state_store.rs:1142-1281`).

---

## Findings

### F-1 · MAJOR — a durable settlement can be minted with NO PASS record, NO admission, NO authority check, and NO digest consult, through the public `StateStore::append_event` (+ §18.3 replay) and the `EventLog` port

**Claim broken:** AK-040 as written — "Settlement requires a PASS
verification record, a resolvable pinned evidence ref, satisfied authority,
no blocking residual, and an unchanged reference digest." The `settlements`
table is the canonical settlement-record surface: `SettlementCommitAuthority`
key checks, the resume projection, the export, and every settlement-counting
consumer read it. I minted a row in it with none of the five conditions
holding. (Also undermines the intent of AK-042's "no double commit" wall: a
forged row makes the honest settle path refuse the unit forever via
`UnitAlreadySettled`.)

**Loci:**

- `crates/gauntlet-ports/src/state_store.rs:59-77` — `RunEvent` has public
  fields; `RunEventKind::SettlementCommitted` is a public variant. Anyone can
  construct the settlement event.
- `crates/gauntlet-ports/src/state_store.rs:291-295` — `append_event` is a
  public trait method with no kind gate: any `Arc<dyn StateStore>` holder can
  append a `SettlementCommitted` event directly, bypassing
  `commit_settlement`'s `AdmittedSettlement` gate entirely.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/event_log.rs:78-103` and
  `:210-221` — `append_to_log` has no kind filter; the separate
  `SqliteEventLog` (the `EventLog` port) over the same database file also
  appends arbitrary `RunEvent`s.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:1949-1990`
  — `complete_pending_settlements` replays `SettlementCommitted` events into
  settlement rows with NO §16.1 re-derivation; the commit is rebuilt from the
  event itself, so `verify_settlement_event_at`'s cross-validation passes
  trivially (its own doc admits this: "the replay path passes trivially").
- Impact amplifiers: `controller.rs:893-904` — resume marks a unit `Settled`
  from the settlements table; `producer_authority.rs:809-822` — the forged
  row then refuses every honest future settle of that unit
  (`UnitAlreadySettled`).

**Failure scenario (executed):** in-process module holding a store handle (or
any writer to the SQLite file): `append_event(SettlementCommitted{unit,
evidence: <invented>, verification: <invented>, digest})` →
`complete_pending_settlements(run, offset)` (or just wait for the honest
`resume_run`, which calls it, `controller.rs:872`) → durable settlement row
exists → export/resume/promotion see the unit settled; honest settlement of
that unit is permanently refused.

**Asymmetry that makes this a defect under the artifact's own standard:** the
repository fixed this exact shape twice for the sibling surface —
`append_record` refuses `RecordKind::VerificationRecord` "for every caller"
(COR2-4, sqlite `state_store.rs:798-822`), and `record_verification`'s doc
names "any in-process store holder" forging "the exact fact the settlement
admission validates against" as the threat that gate closes.
`append_event(SettlementCommitted)` is the same attacker, the same owner
write, ungated.

**Executed evidence** (probe at `/tmp/task13-falsify`, path-deps on the
workspace crates, real `SqliteStateStore`/`SqliteEventLog`, no fixtures
modified; `cargo run` with `CARGO_TARGET_DIR` outside the repo):

```text
[A/before] settlement rows: 0 · verification records: 0 total, 0 PASS
[A] forged event appended at offset 1                     (StateStore::append_event)
[A] complete_pending_settlements applied 1 row(s)
[A/after] settlement rows: 1
[A/after]   row: unit=AK-990 verification=vrf-forged-0001 evidence=evd-forged-evidence offset=1
[A/after] verification records: 0 total, 0 PASS
[A] BYPASS CONFIRMED: durable settlement row exists with ZERO PASS records
[A/divergent] replay refused loudly: … same-key … DIVERGENT facts … (COR2-5 holds)
[B] forged event appended via EventLog port at offset 1    (SqliteEventLog over the same db file)
[B] complete_pending_settlements applied 1 row(s) → BYPASS CONFIRMED via EventLog port + replay
```

Not CRITICAL: unreachable from builder/critic processes (workspace
containment, §17.5.5) and from model output; requires in-process code or
DB-file write access. Above MINOR: it defeats the earned-admission design on
the shipped store's public surface, under the threat model the artifact
itself declares for owner-write forgery, and its consequences (poisoned
resume projection, permanent honest-settlement lockout) are durable.

### F-2 · MINOR — crash between the durable commit and the authority-decision pin loses the B.6 evidence record when recovery goes through resume replay

**Claim touched:** AK-042 "no lost evidence" (adjacent, not literal — the
record is never created, not lost), and the module's own documented B.6
mapping ("ACCOMPANYING pinned evidence document … written at commit time").

**Locus:** `crates/gauntlet-app/src/settle/mod.rs:696-713` — `commit_settlement`
(event+row) completes at `:697`; `record_authority_decision` (evidence
`put`+`pin`) runs at `:707-713`. A crash in that window recovers via
`complete_pending_after_checkpoint` (`:798-961`), which completes the row and
the evidence-production orphans but never re-pins the authority decision.
Re-entry through `settle()` WOULD re-pin it (the `AlreadyCommitted` arm
continues to `:707`), but the controller does not re-call `settle()` for a
unit whose settlement row already exists.

**Scenario:** settle commits; process dies before the evidence `put`; resume
completes the settlement exactly once — and the run's settled unit has no
`settlement_authority_decision@v1` document. The settlement's own facts
(unit, evidence, verification, digest, offset) are durable; only the
accompanying decision record goes missing. Severity MINOR: narrow window,
accompanying evidence only, settlement facts intact.

### F-3 · MINOR — the facet-materialization write boundary is lexical only; §17.5.2's "prefix check after canonicalization" is not applied on the write path

**Claim touched:** spec §17.5.2 (workspace safety invariant), defense-in-depth
for AK-043. Not builder-reachable as shipped.

**Locus:** `crates/gauntlet-app/src/control/controller.rs:3256-3275` —
`contained_locator_path` rejects `..`/root/prefix components but never
canonicalizes, so a symlink at a frozen locator inside the workspace would be
followed by the `fs::write` at `:2413`. Mitigations that hold: the locator is
ruler-frozen (not builder-chosen), the builder effect surface writes bytes
only (cannot mint symlinks), and the read/verify paths DO canonicalize
(`verify/authority.rs:280-291`; observe adapters; doctor check 7 checks the
key/workspace placement both lexically and canonically,
`doctor/checks.rs:1053-1074`). Exploitation needs an out-of-band symlink
inside the workspace plus a hostile ruler — an operator-level attack. MINOR.

### Environment finding

`/tmp` (tmpfs, 3.9 GB) filled to 100% during the first probe build
("No space left on device"). I relocated only MY probe's
`CARGO_TARGET_DIR` to `~/.cache/task13-probe-target` (outside the repo), did
not touch the workspace's `target/`, did not raise parallelism, and ran every
cargo invocation serially under the build lock. No OOM occurred.

---

## Declared search bound, threat model, budget

**Threat model:** (a) in-process Rust code holding `Arc<dyn StateStore>`,
`Arc<dyn EvidenceStore>`, the reference provider, or concrete adapter handles
— the attacker the artifact's own owner-write gates name; (b) builder/critic
agent processes confined to workspace paths (§17.5); (c) model output as
untrusted input. Out of scope: operator/root, disk-level tampering, ptrace,
and the raw SQLite file as an out-of-band writable object (noted only as
F-1's boundary; the spec accepts a local substrate and §17.5.5 keeps it
outside builder paths).

**Search bound:** static analysis of `settle/mod.rs` (service half),
`controller.rs` (composition, settle call, resume, containment,
attempted-list), `verify/{precondition,authority}.rs` (readiness, workspace
canonicalization), `producer_authority.rs` (admission), ports
`state_store.rs`/`verification_record.rs`/`event_log.rs`, the sqlite adapter's
public surface (`state_store.rs`, `event_log.rs`), `reference/provider.rs`,
arch-tests `single_writer.rs`, and the three proofs (`p7_recovery.rs`,
`p11_critic_is_not_verifier.rs`, `g10_builder_writes_answer_key.rs`);
executed: the five suites listed above plus the two-leg forged-settlement
probe. Build lock held throughout (single serial cargo; no config changes; no
repo files touched other than this report; nothing repaired).

**Not examined:** scheduler/diagnosis/disposition internals beyond their
settlement touchpoints; the operator CLI surface beyond
`settle_operator_action` (Task 21); the contract-suite stubs' interiors;
evidence-fs adapter internals (interface only); tasks outside Task 13's
claims; no mutation testing, no coverage, no fuzzing, no
concurrency/stress runs; `complete_pending_evidence_production`'s orphan
matcher was read but not adversarially probed; the `load_run`
checkpoint-projection fallback was noted as the same ungated-trait-call class
as F-1 (a forged `checkpoint()` row can project a run) but not separately
probed — it inherits F-1's class and threat model.

**Distinguishing attack results:** F-1 is a demonstrated bypass, not a
formally proven absence of defenses — the guarded paths
(`commit_settlement`, `record_verification`, `append_record`,
`apply_settlement_row`) held under every probe I ran against them. The
divergent same-key replay refusal (COR2-5) also held. "No other bypass
found" means none within the bound above.

## Verdict

**VERDICT: NOT CONFIRM**

Reasons: within the declared bound, the composed settlement path, P7, and P11
hold and their proofs are substantive (real adapter, real crash helper,
teeth by construction) — but F-1 (MAJOR) breaks AK-040 as written at the
durable-substrate layer: a durable settlement record can be minted through
the public `StateStore::append_event` / `EventLog` port plus the §18.3
replay with zero PASS records and no earned admission, poisoning the resume
projection and permanently blocking honest settlement of the unit. The
artifact's own standard (the COR2-4 gate refusing `VerificationRecord` kinds
on the generic record append, and the `AdmittedSettlement`/`AdmittedVerificationRecord`
type gates) convicts the ungated `SettlementCommitted` append; the
asymmetry is the defect. F-2 and F-3 are MINOR and do not alone defeat
confirmation.
