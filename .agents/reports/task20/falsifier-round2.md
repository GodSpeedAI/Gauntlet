# Task 20 — falsifier round 2 (blind executing falsifier)

VERDICT: NOT_CONFIRM (1 blocking / 1 major / 1 minor).

Round 2 attacks the CORRECTIONS (COR-F1..F6), their boundaries, and the
over-rejection leg (task19 lesson: an honest builder must still settle).
Blindness held: no `*report*.md` (except `falsifier-adjudication-r1.md`), no
`*progress*.md`, no `critic-design.md` / `design-adjudication.md` /
`falsifier-round1.md`, no `*.log` under `.agents/reports/task20/` was read.
Code, tests, fixtures, charter, adjudication, spec-facing docs only.

All attack artifacts live in `/tmp/fals2*`. The only repo write is this
file. Every cargo/just call ran under `flock …/compile.lock` with
`sccache --stop-server` inside the same command.

---

## F-R2-3 (BLOCKING) — COR-F6's settled-resume no-op is defeated at the
## composed surface: the FIRST `gauntlet resume` of a settled run exits 1
## claiming `budget_exhausted` and burns an iteration

Commission line (adjudication COR-F6): "resume of an all-settled run
short-circuits to an honest settled no-op (terminal=settled, exit 0, no
iteration burned)". Executed against the real binary, the FIRST resume of a
fully settled run does the exact opposite — the same misrepresentation the
correction text names ("reporting a `budget_exhausted` terminal that
misrepresents a fully settled run").

- **Executed (twice, deterministic).** Fixture-copy world, real
  `gauntlet` binary, `e2e-happy` scenario, real `scripts/check.sh`
  instrument, produced-artifact composition:
  - run → `terminal=settled iterations=3`, exit 0, settled=3; final
    durable checkpoint seq 4 (iteration 3, offset 12, all three units
    `settled`, in_flight `[]`);
  - `gauntlet resume <id>` → **exit 1, `terminal=budget_exhausted
    iterations=4`**; the burn advanced the durable checkpoint to
    iteration 4 (seq 5/6) — an iteration WAS burned;
  - `gauntlet resume <id>` AGAIN → exit 0, `terminal=settled
    iterations=4` — the no-op fires only from the SECOND resume on;
  - reproduced in a fresh world (runs
    `1QRJRXG5TN398YNWQW8BSVSMD5`, `3D0Y4CYX8QKP4EHP6M04TPDJK4`) and
    under DOUBLE concurrent resume (both concurrent resumes lied
    identically; settlements stayed exactly 3 — no duplicate-settlement
    corruption).
- **Mechanism (code + durable-record evidence).** The controller's no-op
  predicate (controller.rs ~1118–1134) requires, among others,
  `completion.productions_completed.is_empty()` from
  `SettlementService::complete_pending_after_checkpoint`. In the
  PRODUCED-ARTIFACT composition (`ProducePolicy::BindLocatorContent`,
  compose.rs:915), `produce_unit_evidence` records each production over
  the INGESTED ARTIFACT's content ref (state DB: production rows keyed
  `98cb…/873c…/2614…` — the artifact digests, named with the first
  facet's instrument identity), while the resume-completion's orphan
  recovery (settle/mod.rs ~853–870) walks the bound bundle's FACET
  records and checks `evidence_production(run, facet.evidence_ref)` —
  the observers' facet_evidence refs, which have NO production rows in
  this world. Nothing is ever "already applied", so the recovery re-mints
  and `productions_completed` is non-empty on every resume whose anchor
  iteration still has an observation binding. On the first resume the
  anchor is the last DISPATCH iteration (3, binding exists) → predicate
  fails → full path → all-settled planning burns the remaining round →
  `budget_exhausted`. After the burn the anchor iteration (4) has no
  binding → completion empty → no-op. The R2-fail → R3-pass flip with
  unit_states, budget_spent, settlements, and bindings all identical
  pins the discriminator to the completion arm.
- **Why the green tests do not contradict this.** The pinned in-app test
  `resuming_an_all_settled_run_is_an_honest_settled_no_op` passes because
  its world rides the DEFAULT `MaterializeObservation` policy, where the
  production row's ref IS the facet's ref and recovery finds everything
  applied. p7/g7 resume proofs owe OPEN units. No shipped leg resumes a
  SETTLED run through the binary (`e2e_full_loop_mock` has no resume
  leg). The defect lives exactly where the corrections were told to live
  — the composed produced-artifact world — and it is the interaction of
  COR-F6's predicate with the COR-D3/COR-B3a produce-ref binding that
  was never composed into any test.
- **Damage.** Dishonest operator terminal (settled run reports
  `budget_exhausted`, exit 1), a burned iteration, and a durable
  checkpoint advance. No settlement/evidence corruption (settled rows and
  the report's `implemented-and-proven 3` survive intact).
- **Shape of a fix (proposal, not applied).** Make the orphan recovery's
  "already applied" check admission-world aware (a facet whose ref has no
  production row but whose producing completion's locator production IS
  durable is applied), or key the produced-artifact production record to
  be discoverable from the bundle facets, or restrict the facet-recovery
  to iterations with no durable locator production.

---

## F-R2-1 (MAJOR) — escalation queue: an operator action on a lying
## ticket file resolves and CLOBBERS a DIFFERENT ticket's record
## (cross-ticket write; ticket-identity unbound from the file)

- **Executed (LANDED).** Harness `/tmp/fals2/escattack` (path-deps on the
  adapter; log `/tmp/fals2/escattack-run.log`). Two honest tickets
  A=`esc-6a96fb74-0`, B=`esc-6a96fb74-1`. A's FILE was rewritten
  out-of-band to a well-formed `gauntlet-escalation-ticket@v1` record
  whose `"ticket"` field names B:
  - the file is NOT quarantined (it decodes clean) and `list()` shows it
    as a valid record — printed under B's claimed id;
  - `queue.resolve(A, operator, note, Resolve)` decoded A's file,
    found carried ticket B, and wrote the resolved record to
    `esc-…-1.json` — the REAL ticket B's record was overwritten (B now
    `resolved`, its summary/context/run_id replaced by the forged
    file's content);
  - A's own file still says `open` — infinitely re-actionable,
    re-clobbering B each time;
  - `list()` afterwards shows TWO entries named `esc-…-1`: one open (the
    lying file), one resolved (the clobbered victim).
- **Code locus.** `crates/adapters/gauntlet-adapter-escalation-fs/src/
  queue.rs`, `resolve()` (~268–292): the write-back is
  `write_atomically(&record.ticket, …)` where `record` is the DECODED
  record from the file at the caller's path — no check
  `record.ticket == *ticket` anywhere (read path `record()`, `poll()`,
  or `resolve()`).
- **Why it matters.** COR-F3's letter (path containment on read AND
  write) HOLDS — the write stayed inside the root. But the queue doc
  promises "a hand-forged or foreign document cannot enter the operator's
  queue silently", and COR-F4 keeps fail-closed only for files that do
  not DECODE. A lying-but-decodable file silently enters the list and
  steers an operator action onto another ticket: a run blocked on B
  would `poll()` B as `Resolved` and proceed as if the operator had
  acted, while the operator's action was aimed at A — operator intent
  and durable queue state diverge, with the operator's identity recorded
  on the wrong resolution.
- **Shape of a fix (proposal, not applied).** Typed refusal when the
  decoded record's ticket field disagrees with the requested id, in
  `record()` (covers `resolve` and `poll`) — which also fixes `list()`'s
  display id.

---

## F-R2-2 (MINOR) — the skills-lock reader validates almost nothing
## beyond `skills` being an array of {name,digest} strings

Writer (COR-F5) emits the reader's schema and the shipped round-trip /
drift / nested-refusal tests pass. But the reader
(`crates/gauntlet-cli/src/skills_cmd.rs::read_lock`, ~82–105) never looks
at anything else; executed against the real binary
(`/tmp/fals2/b3-run.log`), `verify` exits 0 (silently accepts) for:

- a wrong schema token (`gauntlet-skills-lock@v9`) — the reader never
  reads `schema`; the version gate every other document enforces at
  decode is absent;
- a lying `count` (`"count":99` over 8 entries) — declared, never
  cross-checked;
- an entry REMOVED (7 of 8) — a forged lock silently narrows
  verification coverage, although the owned set is known to the app;
- unknown fields at the root and inside entries — the strict
  "drift is not data" posture of every other reader does not extend
  here.

Still correctly refused (executed): nested legacy shape and missing
`skills` array (typed refusal), non-string digest, missing lock; and the
digest anchor holds — out-of-band edit → typed drift, swapped digests →
typed drift for both, re-install repairs. COR-F5's letter is met for the
forged shapes its shipped test probes; the boundary around them is open.
Severity MINOR: the digest comparison is intact and the lock is
operator-owned state, but the accepted forgeries include a version-gate
bypass and a silent coverage narrowing.

---

## Informational probes (executed; adjudication optional)

- **Settlement-row deletion is masked by the no-op arm.** With every
  `settlements` row deleted out-of-band, `resume` returns
  `terminal=settled exit 0` (the no-op arm: nothing to complete at the
  anchor; binding beyond the last bound iteration) and `status` still
  prints `settled=3` (the checkpoint projection). The views stay
  self-consistent because they share the projection; the deletion is
  visible ONLY in the export stream (0 `"record":"settlement"` rows vs
  3 `verification_record`s). Recorded because the brief asked; severity
  low — it requires out-of-band DB surgery and the operator surface does
  not diverge.
- **Fix-then-resume dispatch is unreachable through the CLI budget
  posture.** Every honest stop of a failing run leaves the entire
  forward budget spent (attempts=5 → rounds exhausted; attempts=2 with
  `GAUNTLET_MAX_ROUNDS=12` still burned 10 of 12 rounds on residual
  retries without an escalated stop), and the restored reserve is not
  dispatchable — so a corrective resume with a now-passing instrument
  can never re-dispatch (executed: settled stays 0, another iteration
  burns). This is §18.2 restore/reserve semantics, not a COR correction
  defect; recorded because the corroboration gate's corrective-resume
  settle is therefore only proven structurally (per-iteration bundle)
  and by the parallel-carry leg (honest settles at iterations 2 and 3).
- **Double resume concurrency.** Two concurrent resumes of a settled
  run: both returned the F-R2-3 `budget_exhausted` lie; settlement rows
  stayed exactly 3 (SQLite single-writer + settle idempotency held).
  No separate defect.

---

## Attacks that FAILED to land (corrections holding)

- **COR-F1 gate (letter and shipped spirit).** The forged-report liar
  shape — PASS verification over forged report bytes while the real
  instrument fails — is shipped as `e2e-forged-report` and stays shut:
  bounded non-settled terminal, `settled=0`, zero settlement records in
  export, forged bytes ingested AND `producer_uncorroborated` pinned
  (test green in this round's battery). The gate's inputs are
  structurally liar-proof at the assembly layer: facets come only from
  `Observer` port calls; the builder-authored claims facet is excluded
  from `doc.facets()` BY CONSTRUCTION and no conversion exists; the
  instrument's exit status is the observer's own spawn result (a signal
  death records NO exit; spawn failure is `Unavailable`, never a facet).
  The run/iteration binding check cannot be fed a foreign doc: the
  assembled doc is constructed fresh from the loop's own facts each
  completion. Digest-collision and reordered-JSON shapes have no lever —
  the gate consumes the assembled doc's facet records, not digests.
  Observation-records-that-name-the-unit-but-not-the-artifact: the rule
  doc deliberately scopes coverage to the completion's per-unit evidence
  workspace (COR-D3's one-surface anchoring), which the produce step
  binds the artifact into; a cross-unit coverage hole would need shared
  per-unit workspaces, which the composition does not create. The
  in-crate pin `the_corroboration_rule_demands_this_runs_executed_
  passing_test_facet` (wrong-kind, missing command, missing/nonzero
  exit, foreign run, foreign iteration, empty bundle → all refuse) and
  the settle/no-settle world pins: 7/7 green this round.
- **Over-rejection leg (required CONFIRM evidence — honest builders still
  settle).** Happy leg settles all three units; `parallel_completions_
  carry_and_never_collide` settles carried completions at iterations 2
  and 3 (later-iteration corroboration); the corroborated-produced-
  world pin settles; dry-run zero-write holds; `a_corroborated_produced_
  artifact_settles_in_the_produced_world` green. No honest path newly
  blocked by the gate was found. (The corrected-resume settle could not
  be exercised through the CLI for budget-posture reasons — see the
  informational block; judged out of the gate's scope.)
- **COR-F2 `load_scenario` stem containment.** 30-probe fuzz
  (`/tmp/fals2/scenattack`): every traversal shape (`..`, `../happy`,
  `a/b`, `sub/happy`, absolute, dot components, whitespace spellings)
  refused at the stem layer BEFORE any join; percent-encoded, unicode
  solidi (U+2215/U+FF0F), zero-width space, NEL, U+2028, NUL, trailing
  dot spellings pass the grammar by design and land at a typed
  `FixtureUnreadable` INSIDE the frozen fixture dir — reach exactly
  `tests/fixtures/mock_runner_scenarios/`. Control stems load. No
  residual.
- **COR-F3 TicketId grammar + queue containment.** Port-level fuzz:
  `..`, `../escape`, `a/b`, `.`, empty, `/abs`, whitespace — refused;
  length 127/128 accepted, 129+ refused; percent/backslash/trailing-dot
  accepted as plain components and the queue's `contained_ticket_path`
  double-guard (no separators in the file name, parent==root,
  Normal-components check) kept every join inside the root on read AND
  write. The cross-ticket defect above is an identity-binding gap, not a
  containment escape. Contract suite 13/13 green.
- **COR-F4 quarantine.** Empty queue → clean; 1-valid + 7-corrupt mix →
  five decode-failure shapes quarantined `record-invalid`, a directory
  named `esc-dirdir.json` and a mode-000 file quarantined `unreadable`,
  dotfile/non-json correctly skipped; no content echoed; records
  oldest-first, quarantined id-sorted; actions on quarantined, unreadable,
  and ghost tickets all fail closed; double-resolve refused
  (`AlreadyResolved`). CLI escalations tests 3/3 green.
- **COR-F5 shipped round trip.** install → verify passes; out-of-band
  edit → typed drift; re-install repairs; forged nested shape → typed
  refusal (all executed via the real binary and the shipped tests).

## Shipped-teeth battery (this round, all green)

- `gauntlet-adapter-escalation-fs` 13/13; `gauntlet-adapter-agent-mock`
  73/73; `gauntlet-cli` targeted 11/11 (five e2e legs, three skills
  roundtrip, three escalations CLI); `gauntlet-app` targeted 7/7
  (corroboration world pins x3, settled no-op, half-applied completion,
  attempts restoration, budget restoration).

## Provenance

- Worlds/harnesses/logs: `/tmp/fals2/` (escattack, scenattack,
  b3-run.log, b4b-run.log, r*/p* outputs, state DBs). Report file:
  `.agents/reports/task20/falsifier-round2.md` (this file; the only repo
  write). Repo untouched otherwise (one early leg misfired at the repo
  root and was refused at runner selection — UNK-003 — before any store
  opened; verified no state db appeared).

VERDICT: NOT_CONFIRM (1 blocking / 1 major / 1 minor).
