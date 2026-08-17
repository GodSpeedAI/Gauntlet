# Critic B — Adversarial Static Review of Task 6 Artifact

## THREAT MODEL (declared up front)

I attack the Task 6 artifact as an adversary trying to find evasion routes, silent-failure
paths, and invariant violations by STATIC READING ONLY. I have no knowledge of the builder's
reasoning and did not read it. My attacks, per axis:

1. **Immutability evasion** — any code path (public or internal, error path included) that can
   rewrite, truncate, replace, or delete an evidence object, settlement row, or event-log row.
   Includes fs-store aliasing (pre-existing dirs/symlinks at object paths), `.kind` sidecars,
   `.incoming-*` temps, pin markers (destroy/fake/isolation).
2. **Ordering/determinism** — can two exports of the same run differ? SQL ORDER BY ties, text
   collation vs numeric ordering, CanonicalKey collisions/duplicate detection in export.rs,
   number formatting, map-iteration-order dependence.
3. **Crash-window** — §16.3 durable-event-before-settlement-row; replay exactly-once keyed by
   (run_id, unit_id, verification_ref). Interleavings, transaction boundaries, error paths,
   dead-code drift among commit_settlement / apply_settlement_row / complete_pending_settlements;
   INSERT OR IGNORE semantics; cross-event resurrection; offset trustworthiness.
4. **Damage/tamper handling** — truncated rows, wrong-kind payloads, negative/absurd timestamps,
   mid-log ULID validation failure, offset gaps; silent skips vs Damaged; over-strict Damaged.
5. **Boundary/allowlist evasion** — rusqlite/sha2 allowlist entries as smuggling channels;
   pin-test inventory completeness; feature unification reaching non-adapter crates.
6. **Contract-suite gaming** — shared-suite registration vs bespoke-only tests.

## SCOPE

Files in scope (read): the sqlite state adapter crate, the fs evidence adapter crate,
crates/gauntlet-app/src/state/{mod.rs,export.rs}, crates/gauntlet-arch-tests/src/deps.rs,
the frozen port/contract context, the Core-spec sections named, and the Task 6 plan section.
NOT read: .agents/reports/task6/* (except this file), .agents/CURRENT_STATUS.md.
NOT done: compiling, running tests, dynamic execution of any kind.

Method note: findings below are appended attack-by-attack as instructed.

---

## AXIS 1 — Immutability evasion

### A1.1 SQL: any UPDATE/DELETE/DROP path in the sqlite crate?
Attack: grep the entire crate (src, tests, migrations, bin) for rewrite verbs.
Path: crates/adapters/gauntlet-adapter-state-sqlite/** (all SQL issuers: src/event_log.rs, src/state_store.rs, src/migrations.rs, src/bin/crash_helper.rs, migrations/*.sql).
Outcome: **BITE** — the only statements issued are INSERT (event_log.rs:80, state_store.rs:152, :351, :532, :672, migrations.rs:96), INSERT OR IGNORE (settlements, state_store.rs:532), SELECT, CREATE TABLE (migrations.rs:65, and CREATE TABLE IF NOT EXISTS in migration files). No UPDATE, no DELETE, no DROP anywhere in the crate, migrations included. (Verified by reading every SQL site, not grep alone.)

### A1.2 fs: overwrite of an existing evidence object
Attack: put the same digest with different bytes (collision / aliasing).
Path: store.rs:80-111 `write_bytes_at_digest` — exists-check at :82 routes to `verify_untouched` (:231-247), which errors on ANY byte difference. Test tooth at tests in store.rs (`a_colliding_digest_with_different_bytes_is_a_typed_refusal`).
Outcome: **BITE** — typed `WriteFailed` naming the digest; the stored object is untouched.

### A1.3 fs: pre-existing DIRECTORY at an object path
Attack: create `objects/ab/cd/<64hex>` as a directory, then put/get that address.
Path: store.rs:82 `target.exists()` is true for a directory → `verify_untouched` (:232) `fs::read(target)` fails (EISDIR) → typed WriteFailed. `get` (:175) fails EISDIR → `ReadFailed` (not NotFound, not silent).
Outcome: **BITE** — loud on both paths; no truncation, no replacement of the directory with a file.

### A1.4 fs: SYMLINK at/inside the object fanout (aliasing escape)
Attack 1 — symlink AT the object path pointing elsewhere: `target.exists()` follows symlinks; `verify_untouched` reads THROUGH it. If the aliased bytes match, put accepts and returns Ok — the "object" is a symlink living outside the store. No write occurs through the link (exists short-circuit; rename is never reached), so no overwrite — but the store silently admits an object whose bytes it does not own.
Attack 2 — symlinked fanout DIRECTORY (`objects/ab` → outside root): `create_dir_all(parent)` (:88) resolves through the link, the `.incoming-*` temp is written OUTSIDE the store root, and `rename` publishes the object outside the root. There is NO canonicalization/path-containment check anywhere in this adapter (contrast spec §17.5, which mandates canonicalized prefix checks for workspaces).
Path: store.rs:61-66 `object_path`, :82, :88-95.
Outcome: **EVADE — minor**. Requires prior write access to the store root, which §17.5 says builders never have ("the evidence store ... outside every builder-writable path"), so this is defense-in-depth, not a reachable agent route. Still: the adapter trusts its root tree absolutely; a compromised helper or operator mistake (a symlinked backup dir) redirects writes invisibly, and `get` will happily serve through the link forever.

### A1.5 fs: `.incoming-*` temp files
Attack: crash between `write_durable` and `rename`; pid/seq reuse colliding with a stale temp.
Path: store.rs:89-94 — temp = `.incoming-<pid>-<seq>`; `write_durable` (:270-288) opens with create+truncate, so a same-named stale temp is truncated, never mistaken for an object; temps are never addressed by `get` (hex_of requires `sha256:`+64 hex, :138-145); `record_kind`'s marker is `<hex>.incoming-kind` (:124), also never addressable.
Outcome: **BITE** — temps can only litter, never publish. (Litter on the write_durable-failure path is never swept — cosmetic only.)

### A1.6 fs: the `.kind` sidecar
Attack: put same bytes twice with DIFFERENT kinds; or fake a sidecar.
Path: store.rs:116-134 `record_kind` — existing sidecar → unconditional `Ok(())` at :118-119 WITHOUT comparing content. First writer's kind stands silently; the second caller's kind assertion is discarded with no error and no signal. Faking a sidecar requires direct fs write (out of band). Note the port has no kind-read method, so the divergence is unobservable through the port at all.
Outcome: **EVADE — minor**. Documented ("the first writer's kind stands", store.rs:114-115) and kind is provenance-not-identity, but it is a silent acceptance of a contradicting assertion — a provenance-confusion mask, not a data-corruption route. Concurrency variant: two racing `record_kind` writers use the SAME marker name (`<hex>.incoming-kind`) so last-rename wins — the "first writer stands" claim is not actually enforced under concurrency (harmless today: sidecar content unreachable via the port).

### A1.7 fs: pin markers — destroy, fake, cross-run aliasing
Attack: pin for run A destroyed by run B's activity; pin faked; ref/run_id used as path-traversal vectors.
Path: store.rs:69-73 `pin_path` = `pins/<ULID run-id>/<ref>`; `hex_of` (:138-145) admits only `sha256:`+64 lowercase hex (no `/`, no `..`); `RunId` is ULID-validated by the domain. `pin` (:184-226) uses `create_new(true)` (:205-208) — AlreadyExists → Ok (idempotent, :220); object-existence check first (:195-198) → pinning a never-put ref is NotFound. No code path in the crate removes any pin. Per-run pin directories are disjoint.
Outcome: **BITE** — pins are additive-only, per-run isolated, and path-safe given the ref grammar; destruction/faking requires out-of-band fs access.

### A1.8 fs: tampered object served on get
Attack: bytes at an object path swapped out-of-band; does get detect it?
Path: store.rs:167-182 `get` — plain `fs::read`, NO digest re-verification of the content against the address.
Outcome: **EVADE — minor**. A content-addressed store whose read path never re-hashes: `get(sha256:X)` silently returns bytes that are not X's preimage. Nothing in the port text (evidence_store.rs:60-67) REQUIRES read-time verification, so this is not a violation of a written invariant — but it means the "content-addressed" property is enforced only at write time, and tamper detection (bit rot or swap) does not exist in the adapter. Detection responsibility is unassigned.

### A1.9 sqlite: settlement rows / event rows rewritten through a public path
Attack: search all public methods for a second-write/row-replace route.
Path: state_store.rs — every settlement write goes through `insert_settlement` (:526-552, INSERT OR IGNORE keyed by PK (run_id, unit_id, verification_ref), migration 0002). Event writes go through `append_to_log` (event_log.rs:72-97, plain INSERT, AUTOINCREMENT). Offsets never reused; no code path rewrites payload/at/kind.
Outcome: **BITE**.

### A1.10 sqlite: run identity rewrite
Attack: record_run with a disagreeing identity.
Path: state_store.rs:145-200 — PK on runs.run_id; on constraint violation only the SAME reference digest is a no-op (:184-185); a different digest is a typed refusal (:186-194).
Sub-finding: the doc claim (:142-144) "a recording that DISAGREES with the stored identity is a typed error" is enforced for `reference_digest` ONLY. A re-record with the same digest but different intent / budget / config_fingerprint / created_at / parent is silently accepted as a no-op; `load_run` keeps returning the first identity. The disagreement is never surfaced.
Outcome: **EVADE — minor** for the identity-check gap itself (no history is rewritten; the stored row always wins), but the module doc overclaims what is checked. The digest IS the invariant §5.3.3 names, so the enforced check is the load-bearing one.


## AXIS 2 — Ordering / determinism

### A2.1 SQLite ORDER BY ties
Attack: find a segment query whose sort key can tie.
- events: `ORDER BY offset ASC` (event_log.rs:107-108); `offset` is INTEGER PRIMARY KEY — unique, never ties.
- checkpoints: `ORDER BY iteration_no ASC, seq ASC` (state_store.rs:848-851); `seq` is AUTOINCREMENT PK — ties on iteration broken uniquely.
- settlements: `ORDER BY unit_id ASC, verification_ref ASC` (state_store.rs:561-562) — exactly the PK columns (migration 0002) within one run_id — unique.
- records: `ORDER BY record_type ASC, seq ASC` (state_store.rs:936-938) — unique by seq.
- latest-checkpoint picks: `ORDER BY iteration_no DESC, seq DESC LIMIT 1` (state_store.rs:634-635, 707-708) — same tie-break.
Outcome: **BITE** — no tie is expressible; SQLite BINARY collation (byte memcmp) and the ids' ASCII-only domain grammars agree with Rust `str` ordering (values.rs:466-488 UnitId is `[A-Z0-9-]`; ULIDs are Crockford ASCII).

### A2.2 CanonicalKey collision / duplicate-detection bypass (gauntlet-app/src/state/export.rs)
Attack 1 — two distinct records collide on the FULL key `(rank, kind, text_key, number_key, sequence_key)` and are silently reordered. Collision requires full equality, and full equality is detected first: export.rs:296-306 scans all pairs and returns `DuplicateKey` before the sort at :307. So collision ⇒ loud refusal, never silent reorder.
Attack 2 — composite-key injection into the settlement `text_key` `"{unit_id}\u{1f}{verification}"` (export.rs:258-266). A collision needs `\u{1f}` inside a unit_id; `UnitId::new` (values.rs:469-488) admits only ASCII uppercase/dash/digits — injection impossible. `VerificationRef` MAY contain `\u{1f}` (`validate_plain_ref`, values.rs:709-722, forbids only ASCII whitespace/leading slash), but the suffix side cannot break injectivity when the prefix side is separator-free.
Attack 3 — same (unit, verification) with different evidence: same canonical key ⇒ `DuplicateKey` (and in the sqlite adapter, impossible anyway — PK).
Attack 4 — map-iteration-order dependence: none. `JsonValue::Object` is a `Vec<(String, JsonValue)>`; the ports parser preserves written order and REJECTS duplicate object keys (json.rs:68, :125, test :566); the rebuilt line is assembled from the canonical field list (export.rs:269-281), never from map iteration.
Attack 5 — number formatting: no floats are produced by any encoding path — only `JsonNumber::Uint` (encode.rs:66-68, state_store.rs:602-604); decode paths accept `Uint` only (encode.rs:81-90). The sqlite golden test (state_store.rs:1127-1137) pins the exact bytes.
Outcome: **BITE** on all five.
Residual notes (not evasions): (a) a generic record's `at` and `payload`/`body` fields are presence-checked but not TYPE-checked in export.rs (only `offset`/`iteration_no`/`seq` go through `uint_field`) — a mis-shaped `at` from a bespoke adapter passes through; deterministic passthrough, so not a determinism break, but validation is asymmetric. (b) `JsonNumber::Float` passthrough (json.rs write arm) formats via `f64::to_string` — byte-stable within one toolchain, not pinned cross-version; reachable only through a hand-written record payload, unreachable from this adapter's encoders.

### A2.3 Two exports of the same run differing
Attack: replay every decision in `export_jsonl` (state_store.rs:768-973) and `export_run` (export.rs:197-313) for hidden time or ordering inputs.
- No timestamp is generated anywhere (`at` fields are stored values; the adapter re-encodes events through the same deterministic encoder, export.rs doc "no timestamp is generated here").
- The app layer re-sorts EVERYTHING by the canonical key regardless of adapter order, and the adapter's own order already matches it; the stub test proves order-independence explicitly (FlipFlopStore, export.rs:363-426, 516-530).
Outcome: **BITE** — within this threat model (same process, same durable state), no divergence route found.

## AXIS 3 — Crash-window / §16.3 ordering

### A3.1 Ordering of the two durable steps
Path: `commit_settlement` (state_store.rs:211-237) — `append_to_log` (own implicit transaction; `synchronous=FULL` + rollback journal makes the commit fsync before returning, event_log.rs:58-68) THEN `insert_settlement` (:526-552). Both under one held `Mutex<Connection>` lock, so no same-store thread can interleave between the steps of one commit. `apply_settlement_row` used alone (crash_helper.rs:116-142) preserves the same order: append first, row second, with the REAL offset from the append.
Outcome: **BITE** — event-before-row holds on both code paths.

### A3.2 Dead-code drift: `commit_settlement` has ZERO callers
Attack: enumerate every caller of `commit_settlement`, `apply_settlement_row`, `complete_pending_settlements` repo-wide.
- `commit_settlement` (state_store.rs:211): NO callers anywhere — not in the crate's unit tests, not in tests/crash_recovery.rs, not in tests/contract_suite.rs (it is adapter-specific, not on the `StateStore` trait), not in gauntlet-app, not in gauntlet-cli. The §16.3 sequence it embodies is tested ONLY through a REIMPLEMENTATION in the crash instrument: crash_helper.rs:116-142 does `append_event(...)` + `apply_settlement_row(commit, offset, ...)` as two separate calls.
- `apply_settlement_row` (:244): one caller — crash_helper.rs:141.
- `complete_pending_settlements` (:272): crash_recovery.rs ×3, in-crate golden test ×1. No production caller (expected: app wiring is a later task).
Consequence: someone can reorder `commit_settlement`'s two steps, drop the row insert, or mis-map its errors and EVERY test in the repository stays green. The tested path and the packaged §16.3 primitive are different code.
Outcome: **EVADE — major** (untested §16.3 primitive presented as the atomic commit sequence; the plan's own gate exercises only the instrument's reimplementation).

### A3.3 `INSERT OR IGNORE` semantics
Attack: can OR IGNORE swallow a NON-idempotency constraint violation and lose a settlement silently?
Path: insert_settlement (state_store.rs:531-545) against the settlements schema (migration 0002): constraints are the PK `(run_id, unit_id, verification_ref)` and NOT NULLs. Every column is supplied from validated domain types (NOT NULL unreachable); `u64`→SQLite conversions that cannot fit i64 fail as `ToSqlConversionFailure` (an Err, not an IGNORE). So the only IGNORE trigger is the intended PK.
Cross-event resurrection: replay of a DIFFERENT event at the same key (e.g. a retried settlement appended at a later offset with different evidence) inserts nothing — first-write-wins, the existing row is never mutated (INSERT, no UPDATE anywhere).
Outcome: **BITE**, with a scope note: this holds for THIS schema; adding any future CHECK/FK to `settlements` would silently widen what OR IGNORE swallows — the hazard is structural and unpinned by a test.

### A3.4 Is the row's stored offset trustworthy?
Path: `apply_settlement_row` (state_store.rs:244-255) takes `offset` and `commit` ENTIRELY from the caller and performs NO check that an event with that offset exists, or that its fields match the commit. The row's evidence/reference_digest/settled_at are likewise caller-attested, not derived from the replayed event. Replay (`complete_pending_settlements` :287-314) uses the log's own `entry.offset` — trustworthy — but a direct `apply_settlement_row` call can pin a row to an offset holding a different event, or none.
Outcome: **EVADE — minor** — no current caller lies (crash_helper passes the append's real offset), but the public API's rows are caller-attested, and nothing cross-validates row-against-log at read or export time.

### A3.5 Error path after a durable step
Path: commit_settlement returns `Err` when step 2 fails — but step 1 is already durable. The error is a plain `WriteFailed` whose text does NOT say "the settlement event IS durable at offset N; replay will complete it". A caller treating Err as "nothing happened" and re-running the unit appends a duplicate settlement event (new offset; row unchanged; export then shows two settlement events for one key). Not a correctness break (log is append-only, row idempotent), but the error contract invites the wrong conclusion — the append-side error in event_log.rs:89-94 is carefully worded, the apply-side is not.
Outcome: **EVADE — minor** (misleading failure locus on the half-settled path; recovery still repairs it).

## AXIS 4 — Damage / tamper handling

### A4.1 Truncated / non-decoding payload
Path: read_entries → decode_event (event_log.rs:119-131; encode.rs:185-275). Bad JSON, missing fields, wrong types, values domain constructors reject, unknown tag, tag/payload disagreement — all typed `DecodeError` → surfaced as `EventLogError::Damaged`; replay halts. Proven by a real process writing a truncated payload (crash_helper.rs:174-189) and the Damaged assertion (crash_recovery.rs:253-270).
Outcome: **BITE**.

### A4.2 Payload that decodes to the WRONG event kind
Path: encode.rs:194-202 — payload's own `kind` must equal the row's kind column, else `ValueInvalid` → Damaged; unknown column tags refused (:267-272); negative/fractional numbers in numeric fields refused by `get_u64` (Uint-only, :81-90); iteration 0 refused by `IterationNo::new` (values.rs:440-446) → Damaged.
Outcome: **BITE**.

### A4.3 Negative timestamp — silently CLAMPED, not Damaged
Attack: tamper `event_log.at = -5` (out-of-band sqlite edit); read it back.
Path: event_log.rs:120-123 `row.get::<_, i64>(4).max(0) as u64` → the event replays with `at = 0` — NO Damaged, no error. Same silent clamp at state_store.rs:418 (`created_at`), :473 (projection `MIN(at)`), :966 (records export `at.max(0)`). `EpochSeconds::new` (values.rs:862-865) accepts any u64, so nothing downstream flags it either. The module doc claims "damaged rows are a typed Damaged error, never a skip" — a negative timestamp is damage that is neither skipped NOR flagged; it is silently rewritten to epoch 0.
Outcome: **EVADE — minor** (requires out-of-band tamper; but it is a read-side normalization of impossible-by-construction data — the adapter itself can never write a negative — so ANY negative read is tamper evidence that is being erased).

### A4.4 run_id failing ULID validation mid-log
Path: event_log.rs:124-126 — `RunId::new` failure → `Damaged` naming the offset; recovery halts. Rows before it were already collected but the whole read errors — nothing silently skipped.
Outcome: **BITE** — with a coupling note: `complete_pending_settlements` (:281-285) and `export_jsonl` (:821-825) read the ENTIRE log (all runs) and filter in Rust, so a damaged row belonging to a DIFFERENT run halts this run's recovery/export too. Loud, arguably over-strict; never silent.

### A4.5 Offset gap
Attack: delete a middle row out-of-band; observe replay.
Path: read_entries (event_log.rs:107-112) — `WHERE offset >= ?1 ORDER BY offset ASC`; a hole replays as consecutive entries with no gap detection anywhere (`last_offset` is MAX, not a count/contiguity check). AUTOINCREMENT never reuses ids, so a gap is impossible through the adapter and possible only by tamper — but tamper leaves no trace: the gap is silently tolerated.
Outcome: **EVADE — minor** (out-of-band only; no adapter path can create it; still, tamper evidence a durability spine could cheaply notice goes unnoticed).

### A4.6 Type degradation of `Damaged` through the store surface
Path: state_store.rs maps `EventLogError::Damaged` → `StateStoreError::ReadFailed` (:281-285, :821-825) while decode failures elsewhere map to `Corrupt`. The word survives only in the detail string, not the type. Loud either way, but the failure class differs by entry door.
Related hygiene: `SqliteEventLog::open` maps OPEN/migration failures to `EventLogError::AppendFailed` (event_log.rs:38-39) — an error whose contract says "A durable append failed. The event was NOT recorded" (event_log.rs:21-23) — nothing was being appended.
Outcome: **EVADE — minor** (error-locus drift; no silent success).

### A4.7 Over-strict Damaged (readable data refused)?
- Settlement rows with any unparseable column → `Corrupt` (state_store.rs:585-595) — correct.
- Non-zero `budget_spent` on resume → refused `Corrupt` (spent_or_refuse, :980-1004) — a documented port gap, refused loudly, never zeroed silently. Correct per its own doc.
- Checkpoint iteration 0 → Corrupt. Correct.
Outcome: **BITE** — nothing readable is refused that I can construct statically.

## AXIS 5 — Boundary / allowlist evasion

### A5.1 Do the new entries let another crate smuggle a dependency?
- rusqlite is declared ONLY in the sqlite adapter's manifest (Cargo.toml:29, `{ version = "0.32", features = ["bundled"] }`), deliberately NOT in the root `[workspace.dependencies]` (verified: root table carries only members + toml + sha2). A non-adapter crate adding rusqlite (directly or by inheritance) fails its layer allowlist: domain/ports/cli are empty, application is sha2-only (deps.rs:116-135). Feature unification cannot compile rusqlite into app/domain/ports either — app's internal deps are domain+ports only (deps.rs:233), enforced with dev/build deps counted.
- sha2 sits on the ROOT table; root entries must be on the union allowlist (deps.rs:404-465) — sha2 is; but inheritance still only passes the MEMBER's layer check, so only app and adapters (which list sha2) may pull it. domain/ports/cli inheriting sha2 fail.
Outcome: **BITE** — no route from the added entries into a non-adapter crate through the workspace table or unification.

### A5.2 Granularity of the adapter entry (the widened channel)
The adapter allowlist (deps.rs:141-155) admits a (name, default-registry) PAIR for EVERY crate under crates/adapters/**. Any adapter — present or future — may now declare `rusqlite` with ANY version and ANY feature set (e.g. `features = ["load_extension"]`, SQLite extension loading) and pass the gate; same for sha2. The gate does not pin version, features, or which adapter may use which entry. This is the allowlist design's inherent granularity, not a Task 6 bug, but Task 6 is what widened it from "empty" to "every adapter may embed a SQL engine".
Outcome: **EVADE — minor** (deliberate, justified, and pinned; but the pin polices names, not the power granted).

### A5.3 Pin-test inventory completeness
`the_external_allowlists_are_pinned` (deps.rs:1776-1813) asserts the exact contents of all six lists, the union (`["sha2","rusqlite","toml"]`), and the layer count 6; `allowlist_entries_state_a_reason` (:1821-1834) forces a ≥6-word justification per entry. Adding/growing any list without touching the pin fails. A new layer classification requires editing `classify` AND the inventory count.
Outcome: **BITE** — the inventory is complete for the current classification; growth cannot be silent.

## AXIS 6 — Contract-suite gaming

### A6.1 Are the adapters registered for the shared suites?
- sqlite: tests/contract_suite.rs:81-107 registers BOTH shared suites (`state_store_suite`, `event_log_suite`) through the Task 4 factories, findings asserted empty.
- evidence-fs: tests/contract_suite.rs:62-74 registers `evidence_store_suite` the same way.
- The suites themselves have teeth: green/broken stub pairs exist for event_log and evidence_store (suite_runs.rs:135-161) and state_store (:285-294) — the shared suites cannot silently degrade into always-green.
Outcome: **BITE** — no bespoke-only substitution for the port surface.

### A6.2 What the shared suites do NOT cover (and what covers it instead)
- Event-log damage handling, crash windows, and §16.3 replay: not in the shared suite (event_log.rs cases: monotonic offsets, exact replay); covered only by the adapter's bespoke crash_recovery.rs — legitimate as adapter physics, but a second StateStore adapter would ship with zero damage/crash coverage from the shared suite.
- Evidence overwrite-refusal: the shared suite has put/get/pin cases only; the collision refusal is only the in-crate test (store.rs:346-367) and a source-scan tooth (:374-400).
- The app-layer canonical export (export_run) is tested ONLY against the FlipFlopStore STUB; it has never been composed with the real sqlite adapter's output in any test. Each half is pinned separately; the seam between them (adapter line shape vs canonical_fields) would fail loudly at runtime (UnknownField/MalformedJson — typed, never silent), but the plan's acceptance "exporting the same run twice is byte-identical" is proven for the halves, not the stack.
Outcome: **BITE** on the gaming question proper; **minor gap** on composition (adapter export → canonical re-export untested as a stack).

### A6.3 Misc
crash_helper.rs:143-144 prints "settlement row applied for {unit}" twice (duplicate line) — test-instrument noise only.


## EPISTEMIC DISCIPLINE — what I did NOT attempt

"No evasion found" above means: none found within the declared threat model, within the
listed files, by static reading only. Specifically NOT attempted:

- Any dynamic verification: no compile, no test run, no process spawn (hard constraint).
  Every claim about test behavior is inferred from reading the test source, not from a
  green run.
- Concurrency attacks: real two-process contention on one DB file (busy_timeout/mutex
  behavior under load), interleaved `commit_settlement` calls from two connections,
  fs-store racing writers on a real kernel. The Linux `rename(2)`-replaces-target
  analysis (making the `AlreadyExists` branch at store.rs:97 dead on Linux) is POSIX
  reasoning, not an experiment.
- SQLite engine-version-specific behaviors of the bundled build (rusqlite 0.32 bundled);
  `pragma_update`/`execute_batch` semantics were settled by source reasoning about the
  rusqlite API, not by running it.
- Cryptographic attacks: SHA-256 collision construction against content addressing.
- Anything in gauntlet-domain / gauntlet-ports beyond the files the adapter paths forced
  me to read (values.rs id grammars, json.rs, the contract modules); other adapters
  (agent-mock, observe-*, compare-diff) are out of scope.
- Whether the gates can be skipped at the CI/justfile layer (not in the declared surface).
- Whether the tests actually pass as claimed; filesystem/env side channels outside the
  repo; fuzzing the JSON parser beyond the cases the code and tests exhibit.

## SUMMARY

Counts by outcome (attacks, not sub-notes):

| Axis | BITE | EVADE major | EVADE minor | UNCLEAR |
|---|---|---|---|---|
| 1 Immutability | 6 | 0 | 4 (A1.4 symlinked fanout; A1.6 kind-sidecar silence + race; A1.8 no read-time verification; A1.10 identity check covers digest only) | 0 |
| 2 Ordering/determinism | 3 | 0 | 0 | 0 |
| 3 Crash-window | 2 | 1 (A3.2 `commit_settlement` zero callers/zero coverage) | 2 (A3.4 caller-attested offsets; A3.5 misleading half-settled error) | 0 |
| 4 Damage/tamper | 4 | 0 | 3 (A4.3 negative-ts clamp; A4.5 offset-gap silence; A4.6 error-type drift) | 0 |
| 5 Allowlist | 2 | 0 | 1 (A5.2 name-level granularity widened to every adapter) | 0 |
| 6 Contract suites | 1 | 0 | 1 (A6.2 adapter export never composed with canonical export) | 0 |
| **Total** | **18** | **1** | **11** | **0** |

The one finding I would block settlement on: **A3.2** — the packaged §16.3 primitive
(`commit_settlement`, state_store.rs:211) is exercised by no test anywhere; the
crash-injection suite proves the sequence only as re-implemented inside the test
instrument. Reordering or breaking `commit_settlement` keeps every gate green. The
cheapest repair is a test that calls it.

