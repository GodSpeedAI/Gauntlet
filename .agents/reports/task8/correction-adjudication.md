# Task 8 — critic adjudication (orchestrator) + correction scope

Inputs: `critic-a-result.md` (spec critic, on file) + `critic-b-result.md`
(adversarial critic, persisted verbatim from its in-message delivery after its
declared read-only deviation). Totals in: **0 critical · 3 major · 13 minor**.

## Adjudicated IN (correction round C1–C6)

| # | Source | Finding | Correction |
|---|---|---|---|
| C1 | A-MAJOR | `workspace_effects` documented+parsed as legal under `behavior: stall` but never applied by the runner's Stall arm — the dialect silently diverges from what a fixture claims | Parse-time typed refusal (a `BehaviorConflict` arm matching the existing pattern for `state`/`derailment`/`payload`/`probes` under stall) + discriminating test. Fail-closed; if a writes-files-but-no-result child is ever needed, widen deliberately then. |
| C2 | B-MAJOR-1 | Duplicate `invoke` with a known `invocation_id` silently replaces the entry — a terminal state resurrects to `Running`, violating the ports' pinned terminal stability | Refuse re-admission with `AgentError::AdmissionRefused` (existing ports variant — "admission itself failed"; no ports change). Tests: duplicate-after-terminal refused AND terminal preserved; duplicate-while-running refused. |
| C3 | B-MAJOR-2 | Scenario parser has no depth bound (staircase ⇒ stack-overflow abort, not a typed error) and Θ(n²) dash-subtree copying — the crate's own runaway-memory lesson not carried to depth | Thread a depth counter through the recursive descent (`value_of`/`mapping`/`block_sequence`/`dash_mapping_item`), typed error at a named `MAX_SCENARIO_DEPTH = 64` (matches `json.rs`'s `MAX_NESTING`), module-doc lesson claim updated to cover memory/depth. Tests: boundary depth parses, boundary+1 typed error. |
| C4 | B-MINOR-4 | `timeout_after_probes: 0` accepted although documented `<1..=64>` | Enforce ≥ 1 for the stall timeout knob (`probes_before_write: 0` stays legal — the suite's CompleteImmediately case requires it) + test. |
| C5 | B-MINOR-5 | `PROBE_CEILING` 64 exceeds every driver's probe budget (24) — parse-legal but undrivable scripts | Tighten the ceiling to 24 so legal ≡ drivable; update dialect doc + knob-boundary tests. |
| C6 | A-MINOR-1 | hooks.rs cites "§8.6 / Annex E"; §8.6 is the defaults table — the hooks live in §8.3 → Annex E | Doc-line fix. |

## Adjudicated NO-CHANGE (recorded rationale)

- **A-MINOR-2 deadlines never consulted:** port-wide recorded convention
  (Task-6 ledger: "Deadlines noted, not consulted"); the time-free design is
  documented in the runner's module docs and the suite accepts the probe
  surrogate for scripted adapters.
- **A-MINOR-3 rule-6 pair indistinguishable at integration level:** §10.3
  rule 6 collapses both mismatches to `contract_mismatch` by design;
  distinctness is carried by the crate unit tests (InvocationIdMismatch vs
  EnvelopeDigestMismatch) and the fixture identities.
- **A-MINOR-4 critic fixture wears `builder_result@v1`:** Annex B defines
  exactly one payload schema; recorded design note in-test (Task 10 owns the
  question).
- **A-MINOR-5 stand-in ruler path:** the containment proof is the real-ruler
  byte-compare before/after; a literal path would land in scratch by the
  mock's own containment. By design.
- **A-MINOR-6 crate name:** `gauntlet-adapter-agent-mock` matches the
  workspace's eleven-adapter convention; the plan's `agent-mock` is
  shorthand. AK-059's frozen locator (the fixtures dir) is spelled exactly.
- **A-MINOR-7 flow-seq comma splitting:** documented dialect shape
  ("flow sequences of scalars"); note for fixture authors, no code change.
- **B-MINOR-7 hook whitespace asymmetry:** the clock token is a closed
  vocabulary (typo detection is its purpose); the seed is opaque text — any
  non-empty string is a valid seed. Different grammars, different rules; the
  asymmetry is correct, not accidental.

## Adjudicated DEFERRED (recorded, with triggers)

- **B-MINOR-3 symlink pre-placement under the workspace root:** both effect
  guards are lexical. Outside the adapter's documented threat model
  (fixtures never name absolute paths; the root is test-provided); §17.5
  placement discipline is the primary guard. Same accepted class as Task-6's
  fanout-directory symlink residual. Trigger: mock effects ever resolving
  against a non-test-provided root.
- **B-MINOR-6 `documents` map keyed by contract path (forensic clobber):**
  classification unaffected (validated `RoleResult` lives per-entry); two
  invocations sharing one contract path violates the runtime's unique-path
  injection discipline. Trigger: a second consumer of `written_document` or
  Task-9+ forensic needs.

## Round rule applied

Per the round-5 termination rule: no criticals; all three majors have
in-threat-model triggers (fixture dialect divergence reachable by any future
fixture author; public-API misuse resurrecting pinned terminal state;
public-parse function aborting on hostile input — the repository's own
recorded historical defect class). All three reopen the round → correction
C1–C3 closes them; C4–C6 ride along as bounded minor closures.
