# Task 17 — Builder B2 progress checkpoint (COR-D01..D14 landing)

Started 2026-08-29. HEAD 1de35bb. `just ruler` PASS at start (verbatim):
`verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
(/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)`.

Read: design-adjudication.md (contract), critic-design.md (specification),
roles/{definition,install,category,harness,mod}.rs, tests/roles.rs, cli main.rs,
compare/envelope.rs, justfile, single_writer.rs, denylist.rs + walk.rs (scope),
spec §8.4/§10.1/§10.2/§13.2/§17.1–§17.5, GAUNTLET.md, CLI spec_vocab.rs.

## Pre-work finding that shapes COR-D06(c) — recorded deviation candidate

The charter says the §8.4 17-token tool list lands "as a `pub const` in the
roles module" (gauntlet-app — an INNER crate). The frozen boundary suite makes
that impossible without gate edits (which my charter forbids):

- `denylist::Denylist::derive` derives adapter LAST segments as **whole string
  literals** denied in domain/ports/app (Role::Inner), including `test`
  (gauntlet-adapter-observe-test), `build` (gauntlet-adapter-observe-build),
  `memory` (gauntlet-adapter-meter-memory) — three of §8.4's own tokens.
- The scan covers EVERY file under the core crates, `tests/` included
  (`walk::is_core_path`), so the test file cannot carry them either.
- This exact collision is why `crates/gauntlet-cli/src/spec_vocab.rs` exists:
  "The composition root is the suite's own designated place where naming is
  legal; the application layer consumes the lists as data" — and doctor already
  consumes `spec_vocab::CAPABILITY_VOCABULARY` via `DoctorInput::new(...)`.

Plan: (a) filesystem + (b) network closed vocabularies land as typed enums in
the roles module exactly as chartered (no collisions: read-only/read-write/
deny/allow match no derived segment or vendor term); (c) tool-token VALIDATION
lands in `RoleDefinition::load` against the closed list supplied as data by the
composition root (`InstallInput::tool_vocabulary` ← spec_vocab::
CAPABILITY_VOCABULARY), mirroring the doctor precedent byte-for-byte in shape.
Deviation recorded here + in final report; flagged for orchestrator settlement
review. NOT stopping the leg: the fail-closed VALUE validation the correction
demands is fully deliverable this way with zero gate edits.

## Leg-by-leg log

- RED captured (tests/roles.rs, pre-correction shape): `forged_empty_lock_
  with_recomputed_digest_is_tamper` FAILED with `VerifyReport { harness:
  "writable-harness", enforcement: Writable, roles: 0 }` (the hole);
  `out_of_band_installed_addition_is_drift` FAILED (evil.md invisible);
  `source_addition_after_install_is_drift` FAILED (stale install
  invisible). D13 teeth passed pre-correction as expected (guards existed;
  RED owed via mutation — done, below).
- D07 RED captured: E0425 `cannot find function
  'assert_within_role_deny_set' in module 'super'` x4 + E0599
  `DeniedContextPresent` missing (compile RED).
- COR-D01 landed: `RolesLock::parse` refuses an empty-entries lock with
  typed `LockTampered` naming the construction argument (install's
  `EmptyRoleSet`). Test GREEN.
- COR-D02 landed: `roles_context()` returns `resolved.effective.roles_dir`
  (mirroring the `state_dir.into()` one line above — the charter's
  resolve.rs:1166 `PathBuf` field is on `ResolvedConfig`, while `resolve()`
  hands back `effective: EffectiveConfig` whose `roles_dir` is the merged
  `String`; the in-function precedent is state_dir's `.into()` and I
  matched it rather than refactor roles_context). `roles_install` now
  passes the configured dir; `roles_verify` destructures `_roles_dir`
  (verify anchors on the lock's own recorded paths). Live smoke owed.
- COR-D03 landed: `PROTECTED_PATHS` (`.wayfinder`,
  `.gauntlet/reference.lock`, `.gauntlet/comparison-contract.yaml`,
  `.gauntlet/policies`, `.gauntlet/roles.lock`) + `protected_path_covered`
  (inside OR covers, lexical) + `RolesError::WritePathProtected`; relative
  entries anchor at the project root (also closes the relative `../`
  traversal hole into the existing escape refusal — recorded as a
  conservative generalization inside D03's mandate). Tests: answer-key
  refused (names `.wayfinder`), runtime-dir refused (names a covered
  `.gauntlet/*` anchor — first in list order is `.gauntlet/reference.lock`),
  project root refused, `workspaces` passes.
- COR-D04 landed: verify lists both directories (anchors derived from the
  lock's own recorded paths via `contained_join`), reports `.md` files the
  lock doesn't record as `DriftKind::InstalledExtra` (Annex C.4 harness
  mutation) / `SourceExtra` (stale install); extra files reported, never
  loaded; unreadable dir → the per-entry loop already surfaces it. Tests
  GREEN (were the RED holes above).
- COR-D05 landed: `write_atomic` fsyncs the staged file through a fresh
  `File::open` + `sync_all` before the rename (read-only handle suffices
  for fsync; avoids introducing a `File::create`/`OpenOptions` write-API
  token the single-writer inventory would have to re-litigate). Doc comment
  states torn-read prevention + file durability, no parent-dir fsync
  (R17-note-1).
- COR-D06 landed: `FilesystemMode` (read-only/read-write — §10.1 grammar,
  §10.2 table, §17.3 tier prose) and `NetworkMode` (deny/allow — §10.1)
  typed enums in the roles module; `RoleError::UnknownFilesystemMode` /
  `UnknownNetworkMode` / `UnknownToolToken` name token + line. Tools
  validated against the §8.4 list SUPPLIED BY THE COMPOSITION ROOT
  (`InstallInput::tool_vocabulary` ← `spec_vocab::CAPABILITY_VOCABULARY`,
  the doctor precedent) — see the pre-work finding above for why the const
  cannot live in an inner crate. Artifacts reconciled: orchestrator
  `run-state`→`read-write`, builder `workspace`→`read-write`, verifier
  `own-evidence`→`read-write`, optimizer `proposals`→`read-write`,
  observer `own-facets`→`read-write` (comments/prose untouched).
  `repository_role_artifacts_load` stays green.
- COR-D07 landed: `compare::envelope.rs` gains pub
  `assert_within_role_deny_set(&EnvelopeInputs, &ContextDenySet)` +
  `EnvelopeError::DeniedContextPresent`; five permitted categories map to
  the five members; ZERO call-site changes (documented in the doc
  comment). Test GREEN.
- COR-D09 landed: builder.md §12.2→§17.1; orchestrator.md carries §10.7's
  hatch quoting the spec's own wording.
- COR-D11 landed: single_writer roles entries reworded — installed-dir
  create_dir_all IS behind the containment CALL; lock-path sites
  (lock-dir create_dir_all, staging write, rename) safe by FIXED-LEAF
  construction; guard needles unchanged.
- COR-D12 landed: `roles_verify` refuses any argument with usage + exit 2.
- COR-D13 landed: forged traversal (`../escape.md`) and absolute
  (`/etc/hosts`) sources with recomputed self-digests → `LockTampered`;
  deleted source → `SourceMissing`. Mutation RED: `contained_join` mutated
  to join unconditionally → both forged-path tests FAIL (2 failed, got
  `Drift`/`SourceModified` instead of `LockTampered`); restored → GREEN.
- COR-D14 landed: justfile `roles-verify` now runs
  `cargo run -q -p gauntlet-cli -- roles verify`; comment updated. No
  `roles-install` placeholder exists anywhere in the justfile (checked;
  `schemas` remains a later-task placeholder, not Task 17's).
- `just check-package gauntlet-app` / `gauntlet-cli` / `gauntlet-arch-tests`
  PASS; `just test-package gauntlet-app` = 386/386 PASS.

## Post-implementation gate interactions (all resolved without gate edits)

- literals gate: comma-introduced spec-section numerals in new Display
  strings joined deadline-hinted text (reworded); multi-segment protected
  path literals (anchors now DERIVED from defaults single-segment consts +
  `answer_key_at(...).parent()`); `/etc/hosts` test literal (temp-dir
  built).
- answer-key-surface pin: install.rs must not spell the ruler dir —
  derivation removed the token entirely.
- D02 live smoke initially failed: `resolve()`'s effective dir strings are
  RAW (relative) and install's own containment requires absolute roots →
  `roles_context` now uses `effective.resolve_against(...)` for BOTH dirs
  (recorded as a deviation note in the final report).
- Live smokes LEG 1–6 recorded verbatim in b2-builder.md §5; scratch and
  `.gauntlet/roles.lock` removed.

## FINAL STATE

All corrections landed. Battery (final): test-package gauntlet-app 386/386;
check PASS; test 1225/1225; lint PASS; boundaries 228/228; deny ok; doctor
exit OK; ruler PASS. Full record: `b2-builder.md`.


