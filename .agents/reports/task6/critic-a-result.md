# Critic-A Result — Task 6 (StateStore/EvidenceStore/EventLog adapters)

## Scope declaration

Blind critic review. Compared the Task 6 implementation artifact against:

- `.agents/specs/Core-spec.md` §6.2, §16.1, §16.3, §18.1–§18.4 (quoted, not paraphrased)
- `.agents/plans/gauntlet-impl.md` Task 6 section
- `.wayfinder/answer-key.md` AK-040, AK-041, AK-042, AK-053, AK-054
- Frozen port contracts in `crates/gauntlet-ports/src/`

Artifact examined (static reading only; no compilation, no test execution):

- crates/adapters/gauntlet-adapter-state-sqlite/** (src, migrations, tests, Cargo.toml, crash_helper bin)
- crates/adapters/gauntlet-adapter-evidence-fs/** (src, tests, Cargo.toml)
- crates/gauntlet-app/src/state/{mod.rs,export.rs} + lib.rs `state` declaration
- crates/gauntlet-arch-tests/src/deps.rs (allowlist entries + pin test)

Builder reasoning was NOT read. CURRENT_STATUS.md was NOT read.

Findings appended below as each review area completes.

---

## Frozen reference quotes (captured before reading the artifact)

§6.2 (ports table rows in scope):
- "`StateStore` | `append_event` · `load_run` · `checkpoint` · `resume(run_id)` · `export_jsonl` | Default adapter: embedded SQL (SQLite). Export is deterministic."
- "`EvidenceStore` | `put(bytes, kind) -> EvidenceRef` (content-addressed) · `get` · `pin(run_id, ref)` | Immutable. No overwrite. Pinned evidence survives cleanup."
- "`EventLog` | `append(event) -> Offset` · `read_from(offset)` · `last_offset()` | Append-only, fsync-on-append, the durability spine of §16.3."

§16.3: "In order, no reordering permitted: 1. EventLog.append(SettlementEvent{...}) # durable, fsync; 2. StateStore/answer-key write: status=settled, evidence=<ref>; 3. In-memory advance". "settlement events are keyed by `(run_id, unit_id, verification_ref)`; replaying twice is a no-op". "A crash before 1 means the unit is retried as if verification never happened. The unit MUST NOT be released in memory before step 2 completes durably."

§18.1: "Deterministic export to portable JSON/JSONL is REQUIRED regardless of backend." Minimum durable records: "runs · iterations · role invocations · reference versions · observations · comparisons · verification records · residuals · payments · settlements · failures · recovery events · candidate revisions · promotion decisions · escalations." RunIds ULID.

§18.2 checkpoint contents: "run id, reference digest, iteration number, unit-state projection, budget spent, in-flight invocation ids with their result-contract paths, and the event-log offset."

§18.3 resume: replay from last checkpoint offset, idempotent half-applied settlement completion, digest re-verify, in-flight re-probe, "Run identity, prior evidence, and payment history survive restart intact."

Plan Task 6 done-when: byte-identical double export (AK-054); evidence overwrite "fails to compile or returns an error"; "crash-injection leaves the log readable and truncation-free". Redesign trigger: only non-settlement events may be batched.

AK-040/041/042/053/054 captured verbatim above in answer-key; locators point at tests/contract/settlement::preconditions, tests/proofs/p7_recovery::commit_order, tests/proofs/p7_recovery, tests/contract/state::checkpoint_sufficiency, tests/contract/state::deterministic_export.

## Area 1 — EventLog adapter (src/event_log.rs, migrations/0001, open_connection)

Checked against §6.2 "Append-only, fsync-on-append, the durability spine of §16.3" and the frozen port trait `EventLog` (append/read_from/last_offset, typed `EventLogError::Damaged`).

Conforms (within examined scope):
- Append-only structurally: the only SQL this adapter issues against `event_log` is `INSERT` and `SELECT` (event_log.rs:79-88, 107-108, 143). Verified by grep: no UPDATE/DELETE/DROP anywhere in the crate (see Area 6 note for the full-crate grep).
- Monotonic offsets: `offset INTEGER PRIMARY KEY AUTOINCREMENT` (migrations/0001_event_log.sql:9) — strictly increasing, never reused; `last_insert_rowid()` returned (event_log.rs:95-96). The shared suite case `append_returns_monotonic_offsets` pins this behavior.
- fsync-on-append: connection opened with `journal_mode=DELETE` + `synchronous=FULL` (event_log.rs:60-63); each append is an autocommitted INSERT, so the commit (and its fsync) completes before `append` returns. This is the correct SQLite mechanism for the port's durability claim. Behavioral proof depends on the crash-injection tests (Area 5).
- Damaged reads are typed: undecodable payload / non-ULID run id / tag-payload mismatch → `EventLogError::Damaged` (event_log.rs:124-131, encode.rs:185-275 strict decode, tag cross-checked against the payload's own `kind` at encode.rs:194-202); engine-certified corruption (`DatabaseCorrupt`, `NotADatabase`) → `Damaged` (event_log.rs:157-174). Recovery halts rather than skipping — matches §18.3 and the port doc "never a skip".

Residuals:
- MINOR (event_log.rs:39, 47-48): `SqliteEventLog::open`/`open_in_memory` map connection-open failures to `EventLogError::AppendFailed`. Opening is not appending; the variant says "The event was NOT recorded", which is misleading for a failure that precedes any event. The frozen error enum offers no better variant; the detail string does carry "opening failed". Naming/hygiene only.
- MINOR (event_log.rs:96, 123, 133, 146): negative-rowid and negative-timestamp conversions use `unwrap_or(0)` / `.max(0) as u64`, silently coercing impossible values instead of erroring. Practically unreachable (rowids are > 0; `at` is written from `EpochSeconds`), so hygiene-level.
- MINOR (event_log.rs:180, 193, 201): the `deadline` parameters are ignored (`_deadline`). The §6 preamble requires the parameter on every blocking port; it exists but is not consulted. Note the frozen shared suite itself supplies fixed epoch deadlines in the past (contract/event_log.rs:41-43), so enforcing them against a real clock would fail the suite — this is a pre-existing port-wide convention, not a Task 6 regression, and state_store.rs:25-27 documents the choice. Recorded for completeness.
## Area 2 — StateStore adapter (src/state_store.rs, migrations/0002, encode.rs)

Checked against §6.2 StateStore row, §18.1 minimum durable records, §16.3 commit order + idempotent replay key, and the frozen `StateStore` trait.

Conforms (within examined scope):
- §18.1 minimum durable records: `runs` (identity), `settlements`, and `records` with a SQL CHECK enumerating exactly the 13 remaining spec kinds (0002_state_records.sql:55-75) — all 15 kinds from the §18.1 list are covered (13 in `records` + `runs` + `settlements`).
- §16.3 ordering in code: `commit_settlement` (state_store.rs:211-237) appends the SettlementCommitted event first (durable, synchronous=FULL commit), then writes the settlement row. The replay key is exactly the spec's: `PRIMARY KEY (run_id, unit_id, verification_ref)` + `INSERT OR IGNORE` (0002:40-49, state_store.rs:524-552) — "replaying twice is a no-op" is structural.
- Recovery path: `complete_pending_settlements` (state_store.rs:272-316) replays settlement events and applies rows idempotently; `record_run` refuses a moved frozen reference (state_store.rs:170-199, §5.3.3); `append_record` parses payloads before admission (state_store.rs:347-349, §21.2 untrusted input).
- Step 3 of §16.3 ("MUST NOT be released in memory before step 2"): the adapter holds no in-memory unit state at all — there is nothing to release early; volatile projections are future orchestration tasks.

Residuals:
- MAJOR-1 (state_store.rs:211-237): `commit_settlement` — the adapter's own §16.3 entry point whose doc claims "The §16.3 atomic commit sequence, in order, no reordering permitted" — has zero callers anywhere in the workspace (verified by grep: defined only). The crash tests exercise crash_helper's hand-written two-step (crash_helper.rs:115-142 uses `append_event` + `apply_settlement_row`), a parallel re-implementation. If the two statements inside `commit_settlement` were ever swapped, every test in the repository stays green. Offends AK-041's claim ("appends the durable event before any status write") as weakly proven at the shipped-API level; the primitive-level behavior IS proven (see Area 5). AK-041's own locator (tests/proofs/p7_recovery::commit_order) is a later task, so this is "weakly proven", not "absent".
- MAJOR-2 (state_store.rs:980-1004, `spent_or_refuse`): resume cannot reconstruct non-zero `budget_spent`. The domain `Spent` has no public value-carrying constructor (gauntlet-domain/src/budget.rs:249-284 — private `fresh()`, getters only) but `Budget::charge` exists (budget.rs:381), so the first controller that charges a budget and then writes a checkpoint produces non-zero spend — and `resume` then errors `Corrupt`. §18.2 requires the checkpoint to carry "budget spent"; §18.3 requires "budget spent"/payment history to survive restart. The adapter durably stores and exports all six counters and refuses loudly rather than zeroing (the honest interim choice, self-documented at state_store.rs:17-23), but the §18.3 resume behavior is unimplementable until the domain/port gains a reconstruction path. AK-053's "sufficient to resume" holds for identity/units/in-flight and fails for spend.
- MINOR-3 (state_store.rs:47-55, 457-513): `load_run`'s checkpoint-only projection fabricates placeholder identity (reference id "checkpoint-only", version 1, zero-hex fingerprints, minimal legal budget). Documented and self-naming (PROJECTION_INTENT), and forced by the frozen suite case `checkpointed_runs_load` (a checkpointed run must load, not None) — but callers cannot distinguish a projection from a record except by string-matching the intent. Latent hazard, not a current defect.
- MINOR-4 (state_store.rs:281): `complete_pending_settlements` replays from offset 0, never consulting the checkpoint's `event_log_offset`; §18.3 step 1 says "replay the event log from the last checkpoint offset". Idempotency makes the superset replay safe (no double rows, verified by crash test), so this is a deviation in mechanism, not in outcome.
- MINOR-5 (state_store.rs:25-27): deadlines accepted but not consulted (documented; same convention note as Area 1).

## Area 3 — Deterministic export (adapter export_jsonl + gauntlet-app state/export.rs; AK-054)

Conforms (within examined scope):
- Adapter side: every segment of `export_jsonl` has an explicit ORDER BY (events by offset, state_store.rs:821-844; checkpoints by iteration then write order, 845-906; settlements by unit then verification via `settlements_of`, 555-600; records by kind then seq, 934-971). All timestamps are stored values; none minted. The in-crate golden test pins exact bytes, including two checkpoints written OUT of iteration order that must export ordered by iteration (state_store.rs:1077-1138), and asserts two exports byte-identical — AK-054's claim directly.
- Shared-suite case `export_is_byte_identical_and_faithful` runs against the adapter through the Task 4 factory (tests/contract_suite.rs:82-93).
- App side: `export_run` re-parses the store's output strictly, enforces a canonical shape (unknown record kind / unknown field / missing field / wrong type all typed errors), refuses duplicate canonical keys ("ambiguity is not determinism"), and re-emits in a canonical total order with canonical field order — determinism as an application-layer contract independent of adapter line order. The FlipFlopStore test hands the same lines over in reversed order on the second export and still gets byte-identical canonical output (export.rs:516-530). Test module path `deterministic` matches the plan gate filter `state::export::deterministic`.

Residuals:
- MINOR-6: `export_run` is never executed against the real sqlite adapter's output anywhere — the app crate must not depend on adapters (§4.1), and no workspace-level integration test wires the pair. The canonical field lists (export.rs:156-191) and the adapter's emitted field names (state_store.rs:782-970) are two hand-maintained lists; drift would surface only as a runtime typed error, never in a test. Each half is individually well tested; the seam is not.
- MINOR-7 (evidence mapping, not code): AK-054's declared evidence locator is `tests/contract/state::deterministic_export`; no such file/test path exists — the evidence currently lives in the shared suite case, the adapter golden test, and `gauntlet-app::state::export::deterministic`. The plan's Task 6 gate names only the latter, so the plan is satisfied; whether the answer key's locator requirement is met is a verifier question for conformance time (Task 20 owns the tests/ tree).

## Area 4 — Checkpoints and resume (§18.2/§18.3; AK-053)

Conforms (within examined scope):
- Every §18.2-listed checkpoint field is durable: run id, reference digest, iteration number, unit-state projection, budget spent (all six counters, encode.rs:370-401), in-flight invocation ids WITH result-contract paths (encode.rs:324-367), event-log offset (0002:27-36; write path state_store.rs:663-694).
- `resume` returns the latest checkpoint exactly (ORDER BY iteration_no DESC, seq DESC, state_store.rs:705-762); fresh run → None; round-trip equality pinned by the shared suite case `checkpoint_and_resume_round_trip` and in-crate `resume_returns_the_latest_checkpoint_exactly` (state_store.rs:1178-1194).
- Nothing is reconstructed from conversational memory: every resumed field decodes from a durable row; undecodable rows are typed `Corrupt` errors (I11 respected).
- Crash recovery (tests/crash_recovery.rs): real child process killed via `std::process::abort()`; log reopens readable and truncation-free with exactly the appends that returned (97-127); half-settled run completes exactly once, second replay adds nothing (133-173); crash before append leaves the unit retryable with no durable trace (179-229); damaged tail is a typed `Damaged` error naming the offset (253-270). This satisfies the plan's done-when "crash-injection leaves the log readable and truncation-free" and demonstrates AK-042's "exactly once, no double commit" at the substrate level.

Residuals:
- MAJOR-2 (cross-reference, Area 2): non-zero `budget_spent` cannot resume — §18.2 content is durable but §18.3 resume refuses it. AK-053's "sufficient to resume without any conversational context" is conditionally true (zero spend only).
- OBSERVATION (scope boundary, not a Task 6 residual): the remaining §18.3 resume steps (digest re-verify, in-flight re-probe, workspace sweep, preflight) and the composition that calls `complete_pending_settlements` after `resume` belong to later tasks (13/20); nothing in the adapter ties the two calls together — a future caller could resume without replaying. The frozen port itself defines `resume` as checkpoint-return only, so the adapter matches its contract.
- OBSERVATION (fsync proof limit): process-abort tests cannot distinguish `synchronous=FULL` from `OFF` (the OS page cache survives process death), so the fsync-on-append claim rests on the configured pragmas (journal_mode=DELETE + synchronous=FULL, event_log.rs:58-68) — the correct SQLite mechanism — rather than on a falsifiable test. No CI test can do better; recorded so nobody cites the crash tests as fsync proof.
- MINOR-8 (evidence-store side of AK-042, "no lost evidence"): the crash-injection harness covers only the state-store settlement sequence; FsEvidenceStore's write-once/durability claims (temp+fsync+rename, store.rs:80-111) have no crash-injection coverage. Task 6's plan and done-when do not require it; noted because AK-042 names evidence preservation.
## Area 5 — EvidenceStore adapter (gauntlet-adapter-evidence-fs)

Checked against §6.2 "`put(bytes, kind) -> EvidenceRef` (content-addressed) · `get` · `pin(run_id, ref)` — Immutable. No overwrite. Pinned evidence survives cleanup." and plan Task 6 step 3.

Conforms (within examined scope):
- Content-addressed: the ref is `sha256:<64 hex>` of the bytes alone (store.rs:149-165); same bytes ⇒ same ref is the only reachable outcome. Different bytes ⇒ different ref (SHA-256).
- Write-once, no overwrite path in the API at all: `put` writes temp → fsync → rename (store.rs:80-111, 268-298); an existing object is verified byte-for-byte and ANY mismatch is a typed `WriteFailed`, never an overwrite (store.rs:231-247); there is no update/delete/unpin method on the store or the frozen trait. The plan's done-when "an attempt to overwrite an evidence ref fails to compile or returns an error" is satisfied both structurally (no path) and behaviorally (typed refusal, in-crate test store.rs:345-367; plus a source-scan tooth asserting no public method names a destructive operation, store.rs:373-400).
- `pin(run_id, ref)`: unknown ref (foreign grammar or missing object) → typed `NotFound`; existing ref → create-new marker, fsync file and parent directory; idempotent on re-pin (store.rs:184-226).
- "Pinned evidence survives cleanup": vacuously satisfied — no cleanup API exists on the port or the adapter.
- Shared suite registered through the Task 4 `EvidenceStoreFactory` mechanism, no bespoke assertions (tests/contract_suite.rs:43-74).

Residuals:
- MINOR-9 (store.rs:124): the kind sidecar's temp path (`<hex>.incoming-kind`) is not process/seq-unique, unlike the object temp name (`.incoming-<pid>-<seq>`, store.rs:89-93); concurrent `record_kind` calls in one process share one temp path. Compounded by MINOR-10. Kind is provenance only, unobservable through the port (no kind getter), and the runtime is single-writer (§16.4) — latent, not live.
- MINOR-10 (store.rs:95-109, 126-133): on Unix, `std::fs::rename` onto an existing target succeeds (replaces) rather than returning `AlreadyExists`, so the `AlreadyExists` race branches are effectively Windows-only. Harmless for objects (identical bytes by construction); for the kind sidecar a same-process race could replace one kind's bytes with another's. Same single-writer mitigation.

## Area 6 — Shared contract suites, registration, allowlist (deps.rs), test-vs-claim

Conforms (within examined scope):
- All three ports register through the existing Task 4 mechanism: `state_store_suite(&SqliteStateFactory)`, `event_log_suite(&SqliteEventLogFactory)`, `evidence_store_suite(&FsEvidenceFactory)` — imported from `gauntlet_ports::contract::*`, zero bespoke assertion paths (state-sqlite tests/contract_suite.rs:81-107; evidence-fs tests/contract_suite.rs:62-74). The suites themselves are untouched (frozen Task 4 artifacts; no edits found in contract/).
- Allowlist edits are minimal and pin-tested: `ADAPTER_EXTERNAL_ALLOWLIST` adds exactly `rusqlite` and `sha2`, each with a multi-sentence recorded reason naming plan Task 6 and spec §18.1/§6.2 (deps.rs:141-155); the pin test asserts the adapter list is exactly `["rusqlite", "sha2"]` and the union `["sha2", "rusqlite", "toml"]` (deps.rs:1798-1808); the anti-vacuity reason test (>= 6 words) covers the new entries (deps.rs:1820+). `rusqlite` is declared only in the adapter's own manifest with `bundled`, not in the root `[workspace.dependencies]` the denylist keeps vendor-free (adapter Cargo.toml; root Cargo.toml carries `sha2 = "0.10"` only). Both adapter crates are declared workspace members (root Cargo.toml:22-23).
- No UPDATE/DELETE/DROP/ALTER statements exist in either adapter's production sources (grep verified; the only hits are doc comments and an unrelated `impl Drop`).
- AK-040 scoping fact: the five-condition settlement conjunction (§16.1) is Task 13's SettlementService; the Task 6 substrate supports it (idempotent settlement rows + evidence pin) but no `settlement::preconditions` test exists or was claimed by Task 6.

Residuals:
- MINOR-11 (state-sqlite tests/contract_suite.rs:16-18 vs 57, 74): the factory comment says the scratch DB is "removed on drop", then `std::mem::forget(scratch)` deliberately leaks it (the Box<dyn Trait> store must outlive the guard). Every suite run leaves a `gauntlet-state-suite-*` file in /tmp forever. Test hygiene; the comment contradicts the code.
- MINOR-12 (crash_helper.rs:143-144): duplicated `println!("settlement row applied for {unit}")` line. Trivial.
- MINOR-13 (crash_recovery.rs:207-219): the "before-append" test asserts `settlements(...).is_empty()` twice with identical text between other assertions. Trivial duplication.

## What the tests prove vs. what Task 6 claims — summary judgement

Proven (within examined scope): append-only/monotonic offsets (suite + AUTOINCREMENT); typed Damaged reads (unit + real-process damage injection); byte-identical double export at BOTH layers (adapter golden, shared suite, FlipFlop reversal); §18.2 checkpoint field completeness and exact resume round-trip; idempotent keyed settlement replay after a REAL process kill (AK-042's "exactly once, no double commit" at substrate level); evidence overwrite unrepresentable + typed on collision; allowlist growth is pinned and reasoned.

Weakly proven / overstated: §16.3 ordering at the shipped `commit_settlement` API (MAJOR-1 — proven only via the helper's parallel sequence); non-zero budget-spend resume is impossible and will refuse (MAJOR-2 — loudly, with data preserved); the app-vs-adapter export seam is never tested as a pair (MINOR-6).

## NOT examined (epistemic boundary)

- No compilation, test execution, clippy, or mutation testing was performed (read-only static review by constraint). All "passes" statements above mean "the code as read implements/invokes the behavior and a test exists that would exercise it", not "I observed green".
- gauntlet-ports/src/contract/stubs.rs (2294 lines) was not read line-by-line; only its role in suite_runs.rs teeth tests was used.
- The domain crate (gauntlet-domain) beyond budget.rs `Spent`/`Budget` constructors and the types referenced by the adapters (RunId/Digest/UnitStatus/InvocationState/EvidenceKind wire shapes) was not audited; e.g. SettlementRecord's invariants were taken on faith.
- gauntlet-ports/src/json.rs was skimmed (API surface, strictness docs, writer order) but its parser was not audited byte-by-byte.
- SQLite engine internals (e.g. whether bundled 0.32's synchronous=FULL covers every edge of the DELETE-journal commit path) were not verified beyond standard documented semantics; rusqlite's transitive dependency tree (libsqlite3-sys) was not inventoried beyond the bundled-feature claim.
- Workspace lints, the other adapters (agent-mock, agent-prime, agent-cli, observe-*), gauntlet-cli, and everything under .agents/ except the spec/plan files cited (and this report) were not examined. `.agents/reports/task6/*` (other than this file) and `.agents/CURRENT_STATUS.md` were deliberately not read (blindness constraint).
- The git history of the artifact (which edits belong to Task 6 vs earlier tasks) was not examined; the diff attribution of deps.rs/answer-key-adjacent files rests on the comments and pin tests as read.

## Summary

Counts: 2 major (MAJOR-1 commit_settlement ordering untested at the API it ships on; MAJOR-2 non-zero budget_spend cannot resume per §18.2/§18.3/AK-053); 14 minor (the three unnumbered Area-1 hygiene notes — open-failure variant labeling, unwrap_or(0) coercions, unconsulted deadlines — plus MINOR-3 through MINOR-13); 3 observations (resume/replay composition boundary; fsync-proof instrument limit; AK-040 belongs to Task 13). 0 critical.

Verdict: RESIDUALS (list stands — no critical violation of §6.2/§16.3/§18.1–§18.4 or the Task 6 done-when found within the sections/files/behaviors examined; the two majors are weak-proof/reconstruction gaps, not wrong behavior).
