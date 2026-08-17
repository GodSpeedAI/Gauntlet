# Task 6 builder brief — round 1 (2026-08-17)

Plan Task 6: "State store, evidence store, event log". Goal: durable,
append-only run state with content-addressed immutable evidence and
deterministic export.

Ruling documents (read, quote, do not paraphrase from memory):
`.agents/specs/Core-spec.md` §16.3 (commit order), §18.1-18.4 (substrate,
checkpoints, resume, failure assumptions), §6.2 (port contracts).
Frozen interface: `crates/gauntlet-ports/src/{state_store,evidence_store,event_log}.rs`
and the SHARED CONTRACT SUITES at
`crates/gauntlet-ports/src/contract/{state_store,evidence_store,event_log}.rs`.
Adding an adapter means passing the existing suites (AGENTS.md), NOT writing
bespoke ones. Do not modify the ports crate in this task.

## Deliverables (three items, one crate each where named)

1. **EventLog + StateStore adapter** — `crates/adapters/gauntlet-adapter-state-sqlite`
   (crate exists as a stub).
   - `rusqlite` with `features = ["bundled"]` (reproducible, no system-lib
     coupling; the host has a C toolchain). SQLite is an adapter choice per
     §18.1, never visible beyond this crate.
   - `event_log.rs` module: one table, append-only (no UPDATE/DELETE path in
     any statement the crate issues), monotonic offsets, `synchronous=FULL`
     durability per append (fsync-on-append, §6.2). Damaged/short reads are
     `EventLogError::Damaged`, never silent skips.
   - StateStore schema covers the §18.1 minimum durable records: runs,
     iterations, role invocations, reference versions, observations,
     comparisons, verification records, residuals, payments, settlements,
     failures, recovery events, candidate revisions, promotion decisions,
     escalations. Migrations live in-repo as embedded SQL applied in order
     from a migrations table; migration set is append-only.
   - `export_jsonl` (§18.1, port signature returns `String`): stable
     ordering over the run's records; timestamps may be INCLUDED as record
     fields but the ordering and record selection must be a pure function of
     durable state, never of query timing.
   - Checkpoint/resume per port contract (§18.2 fields; §18.3 replay).

2. **EvidenceStore adapter** — `crates/adapters/gauntlet-adapter-evidence-fs`
   (stub exists). Content-addressed filesystem store: `sha256:<hex>`-keyed
   files under a two-level fanout dir, write-once (an existing digest path is
   NEVER rewritten — content-addressing makes rewrite a no-op check, and any
   byte mismatch on a colliding path is a typed error, not an overwrite),
   `pin(run_id, ref)` as a durable pin table/file, `get` returns stored
   bytes. No overwrite/delete method may appear anywhere in the crate's
   public API (plan: "no overwrite path in the API at all").

3. **Application export service** — `crates/gauntlet-app/src/state/`
   (`mod.rs`, `export.rs`): thin orchestration that pulls a run's records
   through the StateStore port and produces the deterministic JSONL with a
   stable canonical shape. Tests live at
   `gauntlet-app ... state::export::deterministic` (the plan's gate filter
   depends on that exact path existing).

## Recorded-act edits these require (follow the pinned procedure in
`crates/gauntlet-arch-tests/src/deps.rs`; each edit updates the pin test in
the same change)

- `ADAPTER_EXTERNAL_ALLOWLIST`: `rusqlite` (state-sqlite) and `sha2`
  (evidence-fs content addressing; std has no SHA-256). Reasons recorded in
  the entry strings.
- Root `Cargo.toml` `[workspace.dependencies]`: `rusqlite = "0.32"` (or
  latest 0.3x; bundled feature) with justification comment; sha2 already
  present.
- `deny.toml` licences allow-list: add exactly what rusqlite's tree needs
  (expect MIT/ISC for the -sys crates; if the bundled SQLite's public-domain
  stance surfaces as an unrecognized license id, add the minimal entry with
  a comment — do not drop the check).
- Justfile: no new recipes (existing gates cover this task).

## Crash-injection harness (plan step 5 — required, real, not simulated)

A helper binary inside the state-sqlite crate (e.g.
`src/bin/crash_helper.rs`) that performs a scripted sequence
(`append-events`, `half-settle`, `corrupt-tail`...) and can
`std::process::abort()` at a named step. Integration tests spawn it as a
real child process, then reopen the database and assert: the log is
readable and truncation-free; replay after `half-settle` completes the
settlement exactly once (idempotent by `(run_id, unit_id, verification_ref)`
key, §16.3); a crash before the event append leaves the unit retryable.
Killing a real process, not dropping a connection.

## Gates (all must be green)

```
cargo nextest run -p gauntlet-adapter-state-sqlite && \
cargo nextest run -p gauntlet-adapter-evidence-fs && \
cargo nextest run -p gauntlet-app state::export::deterministic && \
cargo nextest run -p gauntlet-ports contract::state_store contract::evidence_store contract::event_log -- --include-ignored \
  (however the suites select their adapter-under-test — read contract/mod.rs and suite_runs.rs FIRST and follow the existing mechanism) \
&& just check && just lint && just deny && just boundaries && just ruler
```

If the contract suites are wired to run adapter implementations via a
factory registry, register both adapters there in the way Task 4 designed;
do not invent a parallel mechanism.

## Teeth (demonstrate once, record output, revert)

a) Determinism: remove the ORDER BY from the export query (or shuffle it)
   → `state::export::deterministic` fails; restore → green.
b) Overwrite refusal: call the evidence store's internal write path with a
   colliding digest but different bytes via a test → typed error (if you can
   make it compile at all); and confirm no public API exposes overwrite.
c) Durability ordering: flip `synchronous` to OFF and/or reorder §16.3
   steps 1/2 in a scratch branch of the crate → the crash-injection
   exactly-once test fails; restore → green.
d) `just boundaries` fails if rusqlite is added to any non-adapter crate
   (spot-check once in gauntlet-app, revert).

## Constraints

- Memory-tight host: build serially; rusqlite bundled compile is the big
  one — do not fan out.
- No `unsafe` (workspace forbids). No new external deps beyond rusqlite and
  the already-allowed sha2/toml. No timestamps generated at export time.
- Secrets/values discipline: evidence content is addressed by digest; never
  log record payloads in error strings beyond digests/ids.
- Do not touch: `.wayfinder/**`, `.gauntlet/reference.lock`, `scripts/**`,
  `AGENTS.md`, `.agents/specs/**`, `.agents/plans/**`, the ports crate, or
  any Task-5 artifact under `crates/gauntlet-app/src/{config,doctor}`.
- Write checkpoints to `.agents/reports/task6/builder-checkpoints.md` as you
  go (one line per work item). Final report: `.agents/reports/task6/builder-round1.md`
  with sections Artifacts / Decisions / Gate (verbatim tail) / Teeth /
  Deviations.
