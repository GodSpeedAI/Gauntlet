# Task 12 — activated-residual adjudication (orchestrator, pre-implementation)

Upgrade rule (CURRENT_STATUS): a deferred finding blocks the moment a task
walks into its trigger. Each Task-12-triggered ledger item is dispositioned
here: COMPOSED (in scope of a named slice), LATER-SCOPE (with the
authoritative reason), or BLOCKER (none). Source wording in
`recon-residuals.md`.

## COMPOSED into Task 12

1. **Ungated `record_evidence_production` write + declared-iteration
   validation** (Task-11 critic-b MAJOR-3 / adjudication C7; twin on Task-10's
   `bind_observation_bundle`, final-confirmation-round7 MINOR-2).
   → **B1**: ports-level minting authorities (AdmittedComparison precedent);
   digests derived from stored bytes, never caller-supplied; declared
   iteration validated against the SAME durable anchor
   `VerificationAuthority::for_run` uses (latest checkpoint iteration, else 1).
2. **Workspace-root pairing convention-bound** (task11 confirmation-round2
   NOTE-2). → **B3**: the controller owns ONE canonical root; every derived
   consumer (verify `for_run`, observer requests, workspace refs) flows from
   that single field.
3. **Observer process timeouts + subprocess output-size caps** (Task-9
   D1/B-7a). → **B3**: deadline consulted in the observe adapters' command
   path (kill-on-deadline → typed Unavailable/gap) + bounded capture.
4. **Runner deadlines modeled, not enforced** (Task-8 A-m2). → **B3**: the
   controller composes real deadline behaviour over the port (bounded poll
   loops, §10.3 rule 5 TimedOut disposition, §11.4 stall detection via the
   injected Clock). Real-process enforcement remains with the real runner
   adapters (Task 15); the mock runner is time-free by design.
5. **Iteration-boundary `verify_unchanged` has no production caller**
   (Task-7 M4; AK-023/AK-024 substrate). → **B3**: the controller calls the
   reference provider's verify path at EVERY iteration boundary; mismatch →
   terminal `aborted{reference_mutated}` with both digests persisted as
   evidence. Settlement pre-check half stays Task 13.
6. **Unavailable vs ReferenceMutated controller policy undefined**
   (Task-7 m3). → **B3**: typed disposition policy grounded in §21.3/§22.2
   (mutated → abort, preserve, no retry; unavailable → non-fatal gap →
   INCONCLUSIVE for dependent claims) with tests.
7. **depends_on existence/acyclicity unchecked** (Task-7 F7). → **B2**: the
   scheduler/plan step validates the unit graph (unknown target → typed
   error; cycle → typed error) before scheduling.
8. **Non-zero `budget_spent` cannot resume** (Task-6 critic-A MAJOR-2).
   → **B1**: domain slice — `Budget::restore`/`Spent` reconstruction with
   invariant validation + tests (constructor, not an adapter patch).
9. **Empty-ruler legality if the controller trusts `verify_unchanged` solo**
   (Task-7 F18). → **B3**: the controller refuses a zero-Unit parsed
   reference at composition (digest agreement alone is never the sole
   authority).

## LATER-SCOPE (reason on record; not silently carried)

- **Command-adapter kind↔command binding** (Task-9 m10/D9, trigger
  "production wiring Task 12/15"): the implemented architecture injects
  already-constructed `Observer` values into the controller; the misbinding
  surface (CommandObserver::from_parts kind choice) is exercised at the
  composition root, which per the dependency graph and plan is Task 20's
  wiring (Task 15 covers real-tool binding). Task 12 neither constructs
  command observers from configuration nor widens the port. Trigger moves to
  the composition-root task; recorded here, not silently.
- **Doctor check 2 manifest-clause deepening** (Task-7 M2, trigger "next
  doctor touch or Task 12 (config/reference surface work)"): Task 12
  CONSUMES drift-capable checks at iteration boundaries (§8.5) but modifies
  no doctor/config/reference-parsing surface. Not activated by consumption.
- **Chain-read → instrument-read race** (task11 NOTE-3; trigger
  race-freedom/ports revision): Task 12 claims no race-freedom.
- All ledger items with Task 13/14/15/16/20/25 triggers (settlement commit
  crash windows, capability-vs-tier honesty pin, env fingerprint, replay
  offset, evidence-fs crash injection, etc.): untouched surfaces.

## BLOCKERS

None.
