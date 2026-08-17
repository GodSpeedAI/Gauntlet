# Critic-C Result — Task 6 Confirmation Pass (post-correction verification)

## Scope declaration

Confirmation critic. Blind to builder reasoning: did NOT read builder-checkpoints.md,
correction-round1.md, IN-FLIGHT.md, or .agents/CURRENT_STATUS.md. Read only the two prior
blind-critic reports (critic-a-result.md, critic-b-result.md) as the findings checklist.

Jobs:
1. Verify each prior finding is actually closed in the corrected code: a test exists that
   would fail if the mechanism were removed, and the mechanism is what the finding asked for.
2. Regression hunt over the correction's blast radius (error paths in both adapters, export
   service, crash helper, new cli test, adapter re-exports).

Method: static reading only. No compilation, no test execution, no cargo/just. READ-ONLY
except this report. All "closed" verdicts mean "the code as read implements the mechanism
and a test exercises it", not "I observed green".

Findings appended below as each verification area completes.

## JOB 1 — Closure verification (checklist from critic-a-result.md and critic-b-result.md)

Verdict key: CLOSED = mechanism present in production code AND a test exists that fails if
the mechanism is removed. All evidence is file:line in the corrected tree, read fresh.

### MAJOR-1 / A3.2 — commit_settlement direct coverage — **CLOSED**

- In-crate direct test: `commit_settlement_is_the_spec_sequence_and_idempotent_by_key`
  (crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:1587-1671) calls the
  packaged primitive itself and pins: the settlement row names the returned offset
  (:1604-1610); the event at that offset decodes to exactly the commit's fields
  (:1614-1638); replay afterwards adds zero (:1641-1647); a second commit of the same key
  appends a second event at a strictly higher offset while the keyed row count stays one
  and replay still adds zero (:1649-1670).
- Real child-process tests: new `settle-via-commit` script
  (src/bin/crash_helper.rs:168-200) calls `commit_settlement` — one call, both durable
  steps; `the_packaged_commit_primitive_settles_completely_when_killed_after`
  (tests/crash_recovery.rs:274-300) aborts a real child after the call returns and
  asserts exactly one durable event + one row + replay adds nothing;
  `a_crash_before_the_packaged_commit_leaves_the_unit_retryable`
  (tests/crash_recovery.rs:305-331) aborts before the call and asserts nothing durable.
- Ordering note: the test proves the end state, not step order per se — but the A3.4
  fix makes the reorder structurally unconstructible: `insert_settlement` now runs
  `verify_settlement_event_at` (state_store.rs:603) before inserting, and a row-first
  implementation would have to name an offset whose event does not exist yet, which the
  verification refuses (state_store.rs:654-665). Zero-caller drift is closed on both
  fronts (direct test + child-process test).

### A4.3 — negative timestamps typed as damage at the four sites — **CLOSED**

No epoch-0 clamping remains at any of the four sites; each has a tamper-injection test:

1. event log `at`: `read_entries` errors `Damaged` naming the offset
   (src/event_log.rs:146-153); test `a_negative_event_timestamp_is_damage_naming_the_offset`
   (tests/tamper_evidence.rs:159-184) inserts `at = -5` through a raw connection.
2. `runs.created_at`: `Corrupt` naming the run (src/state_store.rs:455-462); test
   (tests/tamper_evidence.rs:188-228).
3. checkpoint-only projection `MIN(at)`: `Corrupt` (src/state_store.rs:531-539); test
   (tests/tamper_evidence.rs:232-268).
4. `records.at` on export: `Corrupt` naming the seq (src/state_store.rs:1286-1293); test
   (tests/tamper_evidence.rs:272-295).

Residual dead coercions `.max(0)` at state_store.rs:478 and :542 sit AFTER the < 0
refusals and are unreachable for negatives — cosmetic only, not clamps.

### A3.4 — settlement-row offset cross-validation before insert — **CLOSED** (one residual sliver)

- `insert_settlement` calls `verify_settlement_event_at` before the INSERT
  (state_store.rs:598-603); the verification (state_store.rs:636-742) requires the event
  at the claimed offset to exist (:654-665), decode (:678-702), be a `settlement_committed`
  (:703-720), and match the commit's run, unit, evidence, verification, and reference
  digest (:721-740).
- Teeth: `a_settlement_row_pointing_at_a_foreign_event_is_refused` and
  `..._past_the_log_is_refused` (tests/tamper_evidence.rs:304-355) prove both refusal
  modes and that nothing lands; `a_settlement_row_at_its_own_event_offset_lands`
  (:359-374) proves the legitimate path.
- Residual sliver (minor, new R1 below): `settled_at` is still caller-attested — the
  verification SELECTs only (run_id, kind, payload) (:641-650) and FEEDS `commit.at`
  into `decode_event` (:686), so the event row's own `at` column is never compared; the
  original finding named settled_at among the attested fields. All identity-bearing
  fields are validated; the timestamp alone is not.

### A3.5 — half-settled error names the durable offset and recovery path — **CLOSED**

- `commit_settlement` wraps the step-2 failure with the offset, the recovery call, and
  the re-run warning (state_store.rs:274-291).
- Test `a_half_settled_failure_names_the_durable_offset_and_the_recovery`
  (tests/tamper_evidence.rs:386-448) injects a real step-2 failure with an
  `AFTER INSERT` trigger whose body inserts into a missing table (a hard error
  `INSERT OR IGNORE` does not swallow — the test comment addresses exactly this),
  asserts all three substrings, asserts the event survived and the row did not, then
  drops the trigger and proves the error's guidance is real (replay completes, = 1).
  Not vacuous: removing the wrapper leaves the raw insert error, which lacks
  "durable at offset 1" and fails the test.

### A4.6 — EventLogError::Damaged → StateStoreError::Corrupt — **CLOSED**

- `map_log_error` (state_store.rs:47-61) maps `Damaged` to `Corrupt` with the damage
  detail preserved; everything else stays `ReadFailed`. Applied at both log-reading
  surfaces: `complete_pending_settlements` (:337) and `export_jsonl` (:1152).
- Test `log_damage_surfaces_as_corrupt_through_the_store` (tests/tamper_evidence.rs:457-494)
  asserts the `Corrupt` variant (not just the word in a string) through BOTH methods.

### A4.5 — offset-gap detection on replay — **CLOSED**

- `read_entries` tracks `next_expected` and errors `Damaged` naming the hole
  (event_log.rs:127, 135-145); contiguity is checked from the first returned row
  forward (documented :105-112), so a replay starting at or after historical damage
  stays green — a deliberate scoping choice, not an over-reach.
- Test `an_offset_gap_is_damage_naming_the_hole` (tests/tamper_evidence.rs:504-544):
  contiguous log green first, raw `DELETE` of a middle row, replay from 0 refuses with
  "expected offset 2 ... found 3", tail from 3 replays green.

### A1.10 — record_run compares the FULL identity — **CLOSED**

- Constraint-violation arm re-reads all nine identity columns and compares through
  `identity_differences` (state_store.rs:187-235, 801-935): created_at, reference_id,
  reference_version, reference_digest, intent, config_fingerprint,
  environment_fingerprint, parent_run, and nine decoded budget fields; refusal names
  every differing field with both values (:224-234).
- The discriminating tooth is `record_run_refuses_a_changed_intent_on_the_same_digest`
  (tests/tamper_evidence.rs:553-588): SAME digest, DIFFERENT intent must error — a
  digest-only check would silently no-op and fail this test. (The in-crate
  `recording_a_moved_reference_is_refused`, state_store.rs:1477-1508, changes the digest
  too, so it alone would not discriminate; the tamper test is the one that pins it.)

### A1.8 — get() re-hashes content against its address — **CLOSED**

- `get` re-hashes the bytes and refuses a mismatch as a typed `ReadFailed` naming both
  addresses (evidence-fs src/store.rs:227-236).
- Test `a_tampered_object_is_a_typed_integrity_error_on_get` (store.rs:544-569) swaps
  bytes out-of-band and expects the error; without the re-hash the get returns Some and
  the test fails. The old collision test was honestly updated to use a real address
  (store.rs:467-496 comment explains why a hand-chosen address can no longer round-trip).

### A1.4 — object paths must be regular files — **CLOSED** (checklist scope; one half of the original finding remains, see R2)

- `get` uses `symlink_metadata` and refuses any non-regular file with the kind named
  (store.rs:195-212); the put-side `verify_untouched` does the same (store.rs:289-307);
  `describe_path_kind` names "a symbolic link" / "a directory" (store.rs:352-361).
- Teeth: `a_symlink_at_an_object_path_is_refused_by_put_and_get` (unix; store.rs:576-611)
  points the symlink at a file holding the SAME bytes — exactly the case where following
  the link would succeed silently — and both doors refuse; directory case at :615-632.
- NOT closed (and not on this checklist): a symlinked FANOUT directory (objects/ab or
  objects/ab/cd → outside root) still traverses; no canonicalization/prefix containment
  exists (store.rs:61-66, 88, 213). Unchanged from critic-B A1.4 attack 2; see R2.

### A1.6 — kind sidecar unique temp + loud contradiction — **CLOSED**

- Marker is now pid/seq-unique `.incoming-kind-<pid>-<seq>` (store.rs:126-132).
- An existing sidecar is compared: same kind is the no-op, a DIFFERENT kind is a typed
  refusal naming both (store.rs:120-121, 329-349).
- Test `a_contradicting_kind_sidecar_is_a_typed_refusal` (store.rs:637-662): same bytes
  put as Log then Diff must error naming "log"/"diff"; same-kind re-put stays a no-op.
  This also closes critic-A MINOR-9 and the sidecar race half of MINOR-10.

### A2.2-note — export_run type-checks `at` fields — **CLOSED**

- Event `at` and generic-record `at` go through `uint_field`
  (gauntlet-app src/state/export.rs:256, 274).
- Test `mis_shaped_at_fields_are_typed_errors` (export.rs:593-615) covers both with
  `"at":"soon"` → `FieldType`.

### MINOR-6 / A6.2 — composition test wiring real adapter export through export_run — **CLOSED**

- crates/gauntlet-cli/tests/export_stack.rs:151-259: seeds a real file-backed store
  (recorded run, two events, settlement via the packaged `commit_settlement`, two
  checkpoints written OUT of iteration order, one generic record), runs
  `gauntlet_app::state::export_run` over the real adapter twice (byte-identical),
  asserts the canonical total order (checkpoints by iteration despite write order), and
  — the seam pin (:250-258) — asserts the canonical output equals the adapter's own
  `export_jsonl` byte for byte, pinning the two hand-maintained shape lists together.
  I re-checked the field lists and orders on both sides (adapter state_store.rs:1113-1305
  vs export.rs:156-191): they agree, so the byte-equality assertion is satisfiable.

### MINOR-11 / 12 / 13 — comment truth, duplicate println, duplicate assertion — **CLOSED**

- MINOR-11: the factory comment now says the guard is deliberately LEAKED and the file
  is "not removed on drop, by design" (state-sqlite tests/contract_suite.rs:16-22) —
  the comment matches the `mem::forget` behavior.
- MINOR-12: the duplicate println is gone; one line carrying the applied count
  (src/bin/crash_helper.rs:153).
- MINOR-13: the before-append crash test asserts `settlements(...).is_empty()` once
  (tests/crash_recovery.rs:207-210); the duplicated identical assertion is gone.

Closure tally: 13/13 checklist items CLOSED as phrased. Two residuals ride along
(A3.4's settled_at sliver; A1.4's fanout-parent half, unchanged from the original
review) — recorded below rather than silently narrowed.

## JOB 2 — Regression hunt over the correction's blast radius

### RH-1 — Do the new damage/corrupt mappings over-reach? **No over-reach found (examined scope).**

- `map_log_error` reclassifies ONLY `EventLogError::Damaged` → `Corrupt`; every other
  log error stays `ReadFailed` (state_store.rs:52-61). No legible data is refused.
- Gap detection fires only WITHIN the returned range (event_log.rs:127, 135-145): a
  replay starting at or after a historical hole sees a contiguous tail and stays green
  — proven by the gap test's tail assertion (tests/tamper_evidence.rs:539-543).
- Negative-timestamp refusals fire only on values this adapter cannot write
  (u64 `EpochSeconds` into an INTEGER column): every legitimate path stays green — the
  golden export (state_store.rs:1412-1473), both shared suites, and all crash tests
  exercise normal timestamps.
- Kind-contradiction refusal (store.rs:120-121): the frozen port text
  (gauntlet-ports/src/evidence_store.rs:51-60) promises "putting the same bytes again
  returns the same ref" but says nothing about a SECOND KIND; the frozen shared suite
  only ever re-puts with the same kind (contract/evidence_store.rs:61-91), so the
  strictness cannot false-fail the suite. Same-bytes-same-kind remains the idempotent
  no-op (store.rs:658-661).

### RH-2 — Does offset cross-validation break legitimate replay paths? **No (examined scope).**

- `complete_pending_settlements` hands the log's own `entry.offset` and a commit
  reconstructed FROM the decoded event to `insert_settlement`
  (state_store.rs:337-365); the verification re-reads the same row and matches itself —
  passes trivially, by construction. The golden export test drives exactly this path
  (state_store.rs:1436-1439) with unchanged pinned bytes.
- Crash recovery: `settle` appends then applies with the same commit and the append's
  real offset (crash_helper.rs:126-152) — consistent; the half-settled and
  uninterrupted crash tests (tests/crash_recovery.rs:132-244) exercise it.
- The frozen shared suites never reach `insert_settlement` (it is not on the
  `StateStore` trait), so the suite surface is unaffected.
- Cost note (observation, not a defect): each replayed settlement event now costs one
  extra indexed SELECT + decode before its insert. Recovery is not a hot path.

### RH-3 — Does get()-re-hashing break legitimate reads? **No (within this store's writes).**

Every object reachable through a `sha256:` ref was written by
`write_bytes_at_digest` at its own hash (store.rs:80-111, 170-172); such objects always
re-hash to their address. No alternate writer, older layout, or foreign-algorithm path
exists in this crate (`hex_of` admits only `sha256:` + 64 lowercase hex, store.rs:153-160;
foreign shapes return None, not an error). Cost: one SHA-256 pass per read — documented
at store.rs:224-226.

### RH-4 — Do the capability re-exports leak or mask what the denylist should catch? **No (examined scope).**

- `EmbeddedStateStore`/`EmbeddedEventLog` (state-sqlite src/lib.rs:36-39) contain no
  vendor substring.
- The denylist masks ONLY the full adapter crate names (both spellings), greedily
  longest-first, and only for the CompositionRoot role
  (gauntlet-arch-tests/src/denylist.rs:248-270, 305-315). A vendor-flavored type name
  in cli (`SqliteStateStore`) survives masking with "sqlite" intact and is reported —
  `sqlite`/`rusqlite` are DENIED_TERMS for every role (denylist.rs:92-99). Masking is a
  replace-with-space, so it cannot splice a term back together (denylist.rs:260-261).
- Inner crates (domain/ports/app) additionally get adapter identifiers as denied terms
  (denylist.rs:236-246) and cannot depend on the adapter crate at all
  (deps.rs:229-246), so the re-export is unreachable there. gauntlet-app's manifest
  carries domain+ports+sha2 only; its lib re-exports only `export_run`/`ExportError`.

### RH-5 — Does the cli dev-dependency respect layering? **Yes (composition-root freedom, not a production channel).**

- The CLI layer's allowed internal set already includes every adapter member
  (deps.rs:237-244) — composition-root wiring is the designed freedom; the
  dev-dependency is strictly narrower than what the layer permits.
- Dev-dependencies are counted by the checks (deps.rs:280-301); the adapter is a
  workspace member, so no external allowlist is touched — the pin test is unchanged and
  still pins adapter externals to ["rusqlite","sha2"] and the union to
  ["sha2","rusqlite","toml"] (deps.rs:1796-1808).
- Root [workspace.dependencies] entry is a member path — permitted
  (check_workspace_dependency_table, deps.rs:404-465); `rusqlite` remains absent from
  the root table.
- No production wiring: crates/gauntlet-cli/src/{main.rs,spec_vocab.rs} reference no
  adapter; only tests/export_stack.rs does.

### RH-6 — Does tamper_evidence.rs stay confined to tests? **Yes.**

The raw DELETE/CREATE TRIGGER/DROP TRIGGER SQL exists only in
tests/tamper_evidence.rs (:395-399, :438, :525); production sources remain INSERT/SELECT
only (grep over src/ finds no UPDATE/DELETE/DROP statements; `pragma_update(...,"DELETE")`
is a journal-mode value, not a statement). The raw connection rides a dev-dependency on
the same already-allowlisted rusqlite (adapter Cargo.toml dev-dependencies). No
source-scan tooth in this crate scans SQL verbs, so the instrument cannot trip a gate.

### RH-7 — Tests asserting their own implementation? **None found (examined scope).**

All new tests assert observable outcomes: typed error variants with locus-bearing
detail substrings, row counts, offsets, byte-identical exports. The in-crate
commit_settlement test reads the log through the crate's own `read_entries`
(state_store.rs:1614-1638) — mild self-reference — but its load-bearing assertions
(row/offset agreement, replay counts, second-commit growth) are externally observable,
and the child-process test independently confirms durability through a fresh process
and connection.

### RH-8 — Fail-first claims circular or vacuous? **None found (examined scope).**

The riskiest one — the trigger-injected half-settled test — was checked specifically:
the trigger fires only on INSERT to `settlements`, so it cannot preempt the offset
verification (a SELECT); the asserted substrings ("durable at offset 1",
"complete_pending_settlements", "duplicate") exist only in the wrapper text the finding
demanded, and the test finishes by proving the guidance (replay completes). The gap,
negative-ts, and foreign-offset tests each inject the condition out-of-band and assert
the typed refusal — removing the mechanism flips them to concrete failures.

### New residuals found in the regression hunt

- **R1 (minor)** — `settled_at` remains caller-attested in settlement rows:
  `verify_settlement_event_at` SELECTs only (run_id, kind, payload) and passes
  `commit.at` INTO `decode_event` (state_store.rs:641-650, 686), so the durable event's
  own `at` is never compared; a direct `apply_settlement_row` with an `at` differing
  from the event's passes validation and its claim becomes the row's `settled_at`. The
  replay and commit paths are consistent by construction; only a lying direct caller
  (or tamper, which bypasses validation anyway) can diverge. Downgraded remainder of
  the original A3.4, which listed settled_at among the attested fields.
- **R2 (minor, unchanged from the original review, not on the closure list)** —
  symlinked fanout PARENT directories (objects/ab, objects/ab/cd) still traverse; no
  canonicalization or root-containment check exists (store.rs:61-66, 88, 213). Requires
  prior write access to the store root (§17.5 says builders never have it), so
  defense-in-depth, not a reachable agent route.
- **R3 (hygiene)** — the `unwrap_or(0)` negative-coercion pattern the correction
  eliminated for timestamps survives for non-timestamp integers: a tampered negative
  `settlements.event_log_offset` reads back as offset 0, not `Corrupt`
  (state_store.rs:786); same for checkpoint `event_log_offset` on resume (:1091),
  `append_record`'s rowid (:409), and log offsets (:event_log.rs:134, :180). Tamper-only
  reachable; timestamps were the finding's scope. `settlements_of` (:786) is the closest
  in spirit to A3.4 (offsets are load-bearing) and would be the one worth fixing.
- **R4 (hygiene, wording)** — migration 0002's header still claims "no UPDATE, no
  DELETE anywhere in this crate" (migrations/0002_state_records.sql:5-7); with
  tests/tamper_evidence.rs now DELETE-ing a row, "crate" must be read as the production
  sources. The tamper file's own header self-declares the exception; a one-word scoping
  ("in this crate's production sources") would remove the tension.
- Carried forward, NOT on this checklist and unchanged: critic-A MAJOR-2 (non-zero
  budget_spend cannot resume; still refuses loudly per state_store.rs:1315-1339),
  MINOR-4 (replay from offset 0 rather than checkpoint offset), and the unconsulted
  deadline convention.

## NOT examined (epistemic boundary)

- No compilation, test execution, clippy, or mutation run (hard constraint). Every
  "closed"/"green" statement means the code as read implements the mechanism and a test
  exercises it — not that I observed a pass.
- SQLite/rusqlite runtime semantics were settled by source reasoning: trigger-body hard
  errors aborting an `INSERT OR IGNORE` statement; `AUTOINCREMENT` contiguity under
  single-writer use; `symlink_metadata` vs `rename` behavior on links. None executed.
- The spec, plan, and answer-key were used through the verbatim quotes captured in the
  two prior critic reports; I did not independently re-read Core-spec §6.2/§16.3/§18 or
  AK-040/041/042/053/054 in the source documents, and I did not read
  builder-checkpoints.md, correction-round1.md, IN-FLIGHT.md, or CURRENT_STATUS.md.
- git history / diff attribution was not examined (would expose builder reasoning);
  "correction round" scoping is inferred from the correction-naming comments in the
  code and the checklist itself.
- Frozen contract suite internals beyond the cited points (evidence put usage,
  event_log read_from usage, stubs teeth) were not read line-by-line; gauntlet-ports
  json.rs, the domain crate beyond the types the adapters use, other adapters
  (agent-*, observe-*, compare-diff), and gauntlet-cli runtime code beyond the two
  files named were not audited.
- Concurrency was not executed: two writers on one store file, rename races on the
  evidence fs, or cross-process interleaving of commit_settlement. POSIX `rename(2)`
  reasoning only. Cryptographic attacks on SHA-256 content addressing: out of scope.
- The denylist/boundary gates were read as code; `just boundaries` / the denylist scan
  was not run, so "the scan would pass" is a static judgment from denylist.rs logic.
- Windows behavior of the unix-gated symlink test; CI wiring of the new tests; the
  fsync claims' physics (process-abort tests cannot distinguish synchronous=FULL from
  OFF — the prior critics' observation stands).

## Summary

JOB 1: 13/13 checklist closures verified CLOSED as phrased, each with a mechanism plus
at least one test that fails if the mechanism is removed. Two original-finding halves
ride along as residuals and are recorded, not narrowed away: A3.4's settled_at sliver
(R1) and A1.4's fanout-parent symlink half (R2, unchanged from the original review).

JOB 2: no new critical or major defects found within the examined scope. New residuals:
2 minor (R1 settled_at not cross-validated; R3 surviving negative-coercion sites for
non-timestamp integers — R2 is carried-forward, not new), 2 hygiene (R4 migration
comment wording; replay cost observation), plus carried-forward items outside the
correction's scope (notably critic-A MAJOR-2, the domain-side Spent reconstruction gap).

FINAL: CONFIRM — all verified closures stand; no new critical/major within the examined
scope (0 critical, 0 major, 2 new minor, 2 hygiene; 2 carried-forward minor/open items
noted above).
