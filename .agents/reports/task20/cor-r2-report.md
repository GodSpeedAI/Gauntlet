# Task 20 — COR-R2 correction report (falsifier round 2 → corrections)

Builder: COR-R2. Work order: `falsifier-adjudication-r2.md` (authoritative).
Inputs: `falsifier-round2.md` (executed attacks, exact reproductions and
loci), `charter.md` (+ operational addenda), AGENTS.md. Checkpoint trail:
`cor-r2-progress.md`.

VERDICT (builder claim, no settlement authority): all three adjudicated
corrections implemented and pinned; the falsifier's blocking reproduction —
first `gauntlet resume` of a fully settled run through the real binary —
now exits 0 `terminal=settled` with zero iterations burned and zero writes
(shipped as E2E leg 6, which FAILED with the exact `budget_exhausted`
lie before the final arm of the fix and PASSES after, in the same session —
the flip is observed, not inferred).

---

## COR-R2-1 (BLOCKING) — the settled-resume no-op now holds in BOTH produce worlds

### The unified production key surface (the definition)

`ProducePolicy::production_recorded` (controller.rs) is now THE one
definition of "this completion's production is durably recorded", consulted
by the §18.3 resume-completion orphan recovery through the run's own
produce policy. Per world:

- **`MaterializeObservation` (default)**: the production IS the facet
  document's own ref — the produce step records
  `(evidence = facet_ref, locator, producer, iteration)`, so the applied
  read is the keyed `(run, ref)` row. Kept exact-ref on purpose: it gives
  the multi-attempt / shared-locator precision the crash recovery needs
  (attempt N's facet ref is never attempt N-1's row), so world M's
  recovery semantics are byte-identical to pre-correction.
- **`BindLocatorContent` (composed)**: the production's key is the locator
  artifact's OWN content ref — a ref NO facet names — so the applied read
  is `evidence_productions_at(run, locator)` non-empty: "this unit's work
  product has a durable production". Producer-agnostic and
  iteration-insensitive BY MECHANISM, not by concession:
  - producer-agnostic because the produce step fronts the record with the
    completion's FIRST facet's instrument — a multi-facet completion (the
    composed plan fronts `filesystem`+`test`) writes ONE locator
    production, so no single facet's identity owns the row (this was the
    second discriminator, found by the new E2E leg: an instrument-matched
    read passed every in-app single-facet pin but still reproduced the
    `budget_exhausted` lie through the real binary);
  - iteration-insensitive because the produce step itself is — its
    `already_durable` idempotence compares locator + producer, never
    iteration, so a re-produced identical artifact dedupes onto the
    durable row.

There is deliberately no recovery-side guess and no predicate special-case:
the controller's no-op predicate code is UNTOUCHED (comment extended only);
the recovery consults the policy value the composition chose, and the
policy type owns what its key surface is and how produced-ness is read
from it.

### Mechanism changes

- `crates/gauntlet-app/src/control/controller.rs`
  - `impl ProducePolicy { pub(crate) fn production_recorded(...) }` — the
    unified key-surface read, with the full rationale in its doc.
  - `resume_run`: threads `self.produce` into
    `complete_pending_after_checkpoint`; the COR-F6 comment block gains
    the COR-R2-1 sentence. Predicate arms unchanged.
- `crates/gauntlet-app/src/settle/mod.rs`
  - `complete_pending_after_checkpoint(run_id, anchor, produce, deadline)`
    — the policy parameter (the ProducePolicy seam made explicit).
  - Orphan-recovery loop restructured: facet ref and instrument parsed
    once per facet (same refusal text as the old per-unit arm); per unit,
    the applied check now runs FIRST through
    `produce.production_recorded(...)`; mints run only at un-applied
    surfaces; the limitation arm fires only when the facet was neither
    recovered anywhere NOR applied anywhere (the old code recorded
    "unrecoverable" limitations for facets of perfectly healthy settled
    runs — the exact spurious arm that defeated the no-op).
  - Module doc §18.3 section documents the policy-aware completion.

### Tests (both worlds pinned; the falsifier's exact reproduction shipped)

- NEW `a_produced_world_completion_reads_produced_from_the_locator_key_surface`
  (settle tests): world-B produce row + bound bundle whose facet ref has no
  row → completion under `BindLocatorContent` yields NO productions and NO
  limitations (applied); the contrast arm runs the same durable state under
  `MaterializeObservation` and still reads by facet ref (limitation) — the
  surface decision provably lives in the policy, not the recovery.
- NEW `resuming_an_all_settled_run_is_a_settled_noop_in_the_produced_world`
  (controller tests, `compose_produced` harness): FIRST resume of the fully
  settled produced-world run is the honest no-op — terminal settled,
  iterations unchanged, zero re-dispatch, settlements/invocations/checkpoint
  untouched. The default-world pin
  `resuming_an_all_settled_run_is_an_honest_settled_no_op` stays UNMODIFIED
  and green.
- NEW E2E leg 6 `the_first_resume_of_a_fully_settled_run_is_a_settled_noop`
  (`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`): the falsifier's
  exact reproduction through the real binary — happy run settles 3 units,
  then the FIRST `gauntlet resume` must exit 0 with
  `terminal=settled iterations=3` (zero burned), the export stream
  byte-identical, and a whole-scratch content-hash diff empty (zero
  writes anywhere). Observed RED before the producer-agnostic arm
  (exit 1 `budget_exhausted iterations=4`, the falsifier's lie verbatim)
  and GREEN after.
- Mechanical adaptations: six `complete_pending_after_checkpoint` call
  sites in the settle tests gained
  `ProducePolicy::MaterializeObservation` (the fixtures exercise the
  default world); no assertion changed.

### Recorded behavior notes (bounded, not silent)

- A world-B orphan whose event names an ARTIFACT ref remains
  uncompletable through facet-ref mints (as before the correction — the
  facet walk cannot name that ref); where the locator surface already
  reads durable, the facet is applied and no limitation is pinned. The
  limitation-vs-silence difference in that unrecoverable corner is the
  accepted trade of the unified surface; the row-completing path is still
  exclusively the store's cross-validated
  `complete_pending_evidence_production`.
- World M multi-facet completions would show the same
  only-the-first-facet-gets-a-row shape, but every shipped M world is
  single-facet and its behavior is byte-identical to pre-correction
  (exact-ref arm unchanged).

---

## COR-R2-2 (MAJOR) — escalation queue: write-back bound to the REQUESTED id, lying records refused

- `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`
  - NEW typed refusal `EscalationQueueError::TicketIdentityMismatch {
    requested, claimed }` (+ Display arm).
  - `record()`: after decode, a record whose own `ticket` field ≠ the
    requested id is refused typed (cross-wiring defense; the resolve path
    reads through here, so the operator action refuses BEFORE any state
    moves).
  - `resolve()`: the write-back now targets the REQUESTED ticket id —
    never the decoded record's say-so (`write_atomically(ticket, &text)`);
    the invariant is true by construction, not by trust.
  - `poll()`: same identity binding — a forged file can never be
    reported as the polled ticket's state (the forged-`Resolved` shape
    that would let a blocked run proceed on an action aimed elsewhere)
    → typed `StateReadFailed`.
  - `list()`: a decodable record whose `ticket` field contradicts its
    own file's stem quarantines as `RecordInvalid` (coarse class, content
    never echoed) — a lying-but-decodable file can no longer enter the
    operator's queue silently under another ticket's name (the queue doc's
    promise, now actually total).
  - Module doc gains the COR-R2-2 paragraph.
- NEW regression test
  `a_lying_ticket_file_cannot_clobber_another_tickets_record`
  (`tests/contract_suite.rs`) — the falsifier's exact attack, shipped as a
  tooth: A's file rewritten out-of-band to a well-formed record naming B;
  resolve(A) refuses typed; A's file byte-identical after the refusal; B's
  record untouched and still open; poll(B) reports B's own open state;
  poll(A) of the lying file refuses; the listing shows exactly one record
  (B, itself) with the lying file quarantined `record-invalid`; and the
  REAL victim B still resolves normally through the legitimate path.
- Mechanical adaptations: none outside the new variant's Display arm (all
  existing matches on `EscalationQueueError` are variant-specific and
  unaffected); `escalations_cmd.rs` needed no change (it surfaces queue
  errors via Display with a non-zero exit).

---

## COR-R2-3 (MINOR) — skills-lock reader goes strict

- `crates/gauntlet-cli/src/skills_cmd.rs` `read_lock` — on-disk state is
  untrusted input; the reader now enforces, in order:
  1. unknown ROOT fields refused (field set closed at `schema`+`count`+
     `skills`);
  2. exact schema token (`gauntlet-skills-lock@v1`) — the version gate
     every other document enforces at decode;
  3. `skills` must be the root array; every entry must be an object with
     exactly `name`+`digest` strings (unknown entry fields refused);
  4. `count` must EQUAL the array's length;
  5. set-membership completeness: the locked name set must EQUAL
     `REPO_OWNED_SKILLS` exactly — a removed owned skill (silent coverage
     narrowing) and a foreign name are both refusals, and duplicated
     entries are refused. Missing file still reads as empty (install
     creates it), so the honest never-installed path keeps its friendly
     verify message.
- NEW round-trip test
  `the_lock_reader_refuses_the_forged_strict_shapes`
  (`crates/gauntlet-cli/tests/skills_roundtrip.rs`) with the falsifier's
  rejected shapes: wrong schema token (`@v9`), lying count (declared 99
  over 8 entries), a REMOVED entry over a consistent count (only the
  owned-set check can catch it — the digest comparison alone would
  silently pass the narrowed lock), an unknown root field, an unknown
  entry field, and the honest lock still round-tripping after the
  forgeries. All four shapes the falsifier executed as silently-accepted
  now refuse typed with non-zero exit. The three shipped round-trip tests
  stay UNMODIFIED and green (the nested legacy shape still refuses with
  `carries no skills array`; the digest forgery still names DRIFT).

---

## Files touched

- `crates/gauntlet-app/src/control/controller.rs` — ProducePolicy::
  production_recorded (new), resume_run policy threading + comment,
  NEW produced-world no-op pin test.
- `crates/gauntlet-app/src/settle/mod.rs` — policy-aware completion
  (signature + recovery loop + module doc), six mechanical test-call-site
  adaptations, NEW produced-world key-surface pin test.
- `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs` —
  TicketIdentityMismatch variant + Display, record/poll identity
  binding, resolve write-back to the requested id, list() stem
  consistency, module doc.
- `crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs`
  — NEW clobbering-attack regression test.
- `crates/gauntlet-cli/src/skills_cmd.rs` — strict read_lock.
- `crates/gauntlet-cli/tests/skills_roundtrip.rs` — NEW strict-shapes
  round-trip test.
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` — NEW E2E leg 6
  (settled-resume no-op through the binary).
- No assertion was weakened anywhere; no shipped proof or E2E leg was
  modified (the six settle call sites are mechanical signature
  adaptations, listed above with reasons). Frozen surfaces
  (`.wayfinder/**`, `.agents/specs/**`, locks) untouched.

## Battery (evidence = what the runs printed, logs in .agents/reports/task20/)

| Gate | Result | Log |
|---|---|---|
| `just ruler` (before edits) | PASS — "answer key and manifest digests match the frozen lock" | session log |
| `just check` | PASS (exit 0) | corr2-bat-check.log |
| `just boundaries` | PASS (exit 0) | corr2-bat-boundaries.log |
| `just lint` | PASS (exit 0) after rustfmt on new code (clippy `-D warnings` was clean throughout; the one failure was fmt-check whitespace on the new lines) | corr2-bat-lint2.log |
| `just deny` | PASS (exit 0) | corr2-bat-deny.log |
| `just test` | PASS — **1354 tests run: 1354 passed, 0 skipped** (workspace, cargo nextest) | corr2-bat-test.log |
| `just ruler` (after) | PASS (exit 0) | corr2-bat-ruler.log |
| `just doctor` | PASS (exit 0; spec 8.5 preflight) | corr2-bat-doctor.log |
| gauntlet-app package tests | 466/466 (run twice: instrument-matched arm, then final producer-agnostic arm) | corr2-app-test1/2.log |
| escalation-fs package tests | 14/14 (incl. the new clobber regression) | corr2-esc-test2.log |
| cli targeted (skills_roundtrip + escalations_cli) | 7/7 | corr2-cli-test1.log |
| `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test cli_surface` | 10/10 — incl. the new settled-resume leg | corr2-e2e2.log |
| repo-root dry-run | exit 0 — "dry-run — preflight clean, zero writes ahead", "lock verified (70 unit(s) frozen)"; before/after `git status --porcelain` diff EMPTY (0 bytes) | corr2-dryrun.log, corr2-git-diff.txt |

Over-rejection leg (required clean): the shipped E2E legs all stay green
UNMODIFIED — happy (settles 3/3), lying builder (bounded non-settlement),
forged-report liar (`producer_uncorroborated`, zero settlements),
dry-run zero-write, parallel carry — plus the in-app corroboration world
pins, the default-world settled no-op, the half-applied recovery, the
attempts/budget restoration pins (all inside the 1354). Honest builders
settle in both worlds.

## Deviations

- None from the work order. Two implementation notes for the record:
  1. The first cut of the B-world arm matched the row's producer against
     the facet's instrument; the new E2E leg proved that reading wrong for
     multi-facet completions (the composed `filesystem`+`test` plan) and
     it was corrected to the producer-agnostic locator surface described
     above — the E2E leg's red→green flip in the same session is the
     evidence.
  2. `just lint` failed once on `cargo fmt --check` formatting of the new
     code only (clippy clean throughout); fixed with `cargo fmt`, no
     semantic change.

(Report finalized after the battery completed — table filled from the logs.)

---

## Result summary

- **COR-R2-1 (blocking)**: unified production key surface at
  `ProducePolicy::production_recorded`; recovery is policy-aware; the
  settled-resume no-op is pinned in BOTH worlds — in-app (default +
  `compose_produced`) and through the real binary (new E2E leg, red→green
  observed in-session). Predicate code untouched.
- **COR-R2-2 (major)**: queue write-back targets the REQUESTED id; lying
  records refused typed on record/poll, quarantined on list; the
  clobbering attack shipped as a regression tooth.
- **COR-R2-3 (minor)**: skills-lock reader strict (exact token, count
  equality, owned-set equality, unknown fields refused); the falsifier's
  silently-accepted forgeries all refuse typed now; the honest round trip
  is unchanged.
- Full battery green; `just ruler` PASS before and after; no assertion
  weakened; no shipped proof or E2E leg modified.
