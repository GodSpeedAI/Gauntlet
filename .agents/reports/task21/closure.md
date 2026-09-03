# Task 21 — closure (settlement record)

**Task:** Operator surfaces (feature-gated) — plan §Task 21 (phase 5,
optional for Core). **Ruler units:** AK-061 (CLI-only default build
still completes a full loop), AK-062 (both halves: an operator action
can never settle without PASS evidence AND the positive earned path —
the success path task20 honestly left to this task). **Implementation:**
builder B1 (committed in a71eba9). **Corrections:** COR-1, COR-2
(+ addendum), COR-3 (+ r4 addendum). **Confirmation:** blind
design-as-built critic (CONFIRM) + four blind executing falsifier
rounds (NOT_CONFIRM → CONFIRM → CONFIRM → CONFIRM). **Date:** settled
2026-09-03.

## What the task delivers

- `gauntlet-adapter-surface-json`: the `OperatorSurface` port as a
  std-only HTTP/1.1 JSON surface over TcpListener (loopback-only bind,
  bounded requests, per-request deadline, §20.2 routes and error
  shapes, escalated-first section order, run-derivation disclosure);
  passes the unmodified shared contract suite.
- `gauntlet serve` (feature `surfaces` only): typed fail-closed bind
  configuration; surface failures never touch run state (§21.3,
  executed: dead store at startup fails closed; corrupt store at
  runtime = quiet reads, loud actions, surviving server).
- ONE operator-action path (`execute_operator_action`) serving both
  the CLI (`escalations resolve|retry`) and the API action endpoints;
  the durable-PASS gate runs at the composition root AND inside
  `settle_operator_action_earned` — the one method that pairs the gate
  with the ONE `settle` (§16.1 conditions re-derived inside, §16.3
  commit by the service, idempotent re-entry for replays). No second
  settlement or queue write path exists (rg-verified by the critic;
  executed zero-write audits in every falsifier round).
- The positive path is EARNED only: forged PASS records, stale
  productions, planted workspace bytes, forged store rows — every
  shortcut refused by a different defense (recency, live instrument
  judgment, digest binding, record-identity integrity). Settlement
  commits only over bytes an instrument just judged PASS at a pinned,
  digest-bound locator.
- Feature topology: `default = []`; the default build is byte-identical
  CLI-only (`serve` unknown, exit 2; `cli_help.txt` frozen snapshot
  untouched); `surfaces` is the only feature edge this task adds; mock
  E2E 12/12 in both configurations (AK-061).

## Confirmation history

1. **Blind design-as-built critic: CONFIRM** (0 blocking / 0 major /
   5 minor / 6 informational) — topology, one-path discipline, gate
   placement, read honesty, and no-second-writer all verified with
   file:line evidence (`critic-design.md`).
2. **Falsifier r1: NOT_CONFIRM** (0/0/3 minor) — re-entry message
   locus, ANSI log injection, non-durable-run unit read; forgery
   ladder refused at every executed stage (`falsifier-round1.md`).
3. **COR-1** (13 items across both rounds' findings) → **r2: CONFIRM**
   (0/0/3 minor) — all ten corrections held; new minors: ticket-id log
   channel, METHOD log channel, accept-loop hot-spin
   (`falsifier-round2.md`).
4. **COR-2 + addendum** → **r3: CONFIRM** (0/0/3 minor) — all fixes
   held; backoff proven (19.9 lines/s / 0.2% CPU vs ~1M lines/s / 100%);
   regression leg held end-to-end; three same-class minors at sibling
   loci — class shown non-convergent (`falsifier-round3.md`).
5. **COR-3 class-closing sweep** (one `log_quote` discipline +
   enumeration-free invariant teeth, RED 11 violations → GREEN) →
   **r4: CONFIRM** (0/0/1 minor) — all three attack families held; the
   invariant teeth survived attack; sole minor = the invariant test
   accommodated the recorded human-line boundary by avoidance
   (`falsifier-round4.md`).
6. **r4 addendum** (tests-only): the three hostile-id read-failure
   shapes now driven; raw bytes pinned BY CONSTRUCTION to the single
   sanctioned human failure line; carve-out declared as the tooth that
   makes future widening fail. RED-first proven (4 violations), GREEN.

## Orchestrator personal verification (never settled on say-so)

- Gate batteries personally run at every phase transition (pre-COR,
  post-COR-1, post-COR-2, post-COR-3, closure): plan gate
  (`operator_actions_no_bypass` under `surfaces`), no-default build,
  mock E2E both configs, cli_surface+escalations, adapter package,
  app settle tests — all green each time (raw logs `orch-*.log`).
- **D11 teeth verified personally** (RED → byte-identical sha256-
  pinned restore → GREEN): service-side gate deletion → in-crate
  sensor RED with a real settlement committing past it; composition
  bypassing both gates → E2E leg 1 RED. Recorded inaccuracy: the
  builder's teeth-note predicted the wrong layer catches mutation 1 —
  defense in depth means each layer's bypass is caught by its own
  sensor (`orchestrator-teeth.md`).
- Final closure battery on the settled tree: see below.

## Closure battery (final tree, personally run)

Raw log: `orch-closure-battery.log`. All green:

| Gate | Result |
|---|---|
| `just check` | clean |
| `just test` | **1392/1392 PASS** |
| `just lint` (clippy -D warnings + fmt --check) | clean |
| `just deny` | advisories/bans/licenses/sources ok |
| `just boundaries` | 228/228 PASS |
| `just ruler` | PASS (frozen digests match) |
| PLAN GATE `operator_actions_no_bypass` (surfaces) | 13/13 PASS |
| `cargo build -p gauntlet-cli --no-default-features` | exit 0, 0 errors |
| mock E2E `e2e_full_loop_mock` default / surfaces | 12/12 + 12/12 |
| `cli_surface` + `escalations_cli` default / surfaces | 7/7 + 7/7 |

## Residuals and deferrals (all trigger-recorded)

- **R4/I other-subcommand usage errors** still interpolate raw
  arguments (CLI-wide style question, outside the family scope;
  trigger: next CLI-surface touch).
- **Ticket↔unit binding** on escalation records: principled fix is a
  ports revision (`EscalationRecord` unit field); acceptance output
  discloses both ids as separate facts (trigger: ports revision or
  next escalation-schema touch).
- **`open_residuals` structurally empty** on the wire (no typed
  durable residual read exists; trigger: the recorded ports-revision
  deferral lands).
- **Matcher drift-pin shape**: gate-side test enumerates variants by
  hand; a compiler-enforced summary→token association is the recorded
  future shape (trigger: next govern touch).
- **AK-062 frozen locator** (`tests/contract/operator_actions::
  no_bypass`) matches no materializable target — as-built is
  `crates/gauntlet-cli/tests/operator_actions_no_bypass.rs`;
  recorded in reference-candidates 2026-08-28, restated here. Never
  the ruler.
- **D10 leg-3 substitution**: an authority level an identified operator
  cannot close does not exist in the service's vocabulary
  (`check_authority` permits identified operators at every level); the
  leg exercises the closest service-refused family member
  (frozen-ruler membership). If AK-062's intent requires otherwise,
  that is a CandidateRevision against the spec — operator's call.
- **UNK-003 / D12 dogfood fog**: `runners.default` remains unset by
  design; dogfooding through `gauntlet run` stays blocked on the
  operator's selection. Recorded every round, never silently dropped.

## Deviations and records

- **a71eba9 bundling**: the task21 implementation + reports were
  committed by the operator bundled with their own documentation tree
  and .jolli churn under a tutorials-titled message — operator-
  confirmed intentional ("i committed some documentation fyi"); the
  bundling deviates from one-task-one-commit and stands as history.
- **Staging discrepancy**: COR-1's changes were found staged mid-r2
  despite the builder's "nothing staged" claim; the staging set was
  exactly correct (10 files); recorded in `falsifier-adjudication-r2.md`.
- **Serial HTTP serving**: port traits carry no Send/Sync; the server
  serves connections serially with bounded per-request deadlines —
  recorded mechanism choice, not a relaxation.

## Ruler outcome

**48 units provisional through Task 21** (47 through Task 20 + AK-062:
both halves now delivered — refusal parity executed byte-identical
across CLI and API, positive earned path settled exactly-once through
both surfaces with pinned instrument evidence). 39 proposed, 0 blocked.
`just ruler` PASS at every phase; the ruler never moved.
