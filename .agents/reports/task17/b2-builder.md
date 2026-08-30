# Task 17 — Builder B2 final report: adjudicated design-review corrections landed

Date 2026-08-29. HEAD at start 1de35bb. `just ruler` PASS at start and at end.
All COR-D01..D07, D09, D11..D14 items landed; R17-D08 / R17-D10 / R17-note-1 /
roles_context-vs-doctor unification NOT touched (recorded deferrals, per
adjudication). One mechanism deviation recorded below (D06(c)) and flagged for
settlement review; nothing else deviates from the build slice.

## 1. Exact touch surface

Code (all changes additive or as specified by the slice):

- `crates/gauntlet-app/src/roles/install.rs` — D01 empty-lock refusal; D03
  protected-path refusal (`protected_anchors` / `protected_path_covered`,
  `RolesError::WritePathProtected`, resolved-shape write-path validation);
  D04 set-membership drift (`DriftKind::InstalledExtra` / `SourceExtra`,
  `md_stems`); D05 staging-file fsync in `write_atomic`; module docs updated.
- `crates/gauntlet-app/src/roles/definition.rs` — D06 `FilesystemMode` /
  `NetworkMode` typed enums, `RoleError::UnknownFilesystemMode` /
  `UnknownNetworkMode` / `UnknownToolToken`, tools validated against the
  vocabulary parameter; `load` signature gained `tool_vocabulary`.
- `crates/gauntlet-app/src/roles/mod.rs` — re-export the two new enums.
- `crates/gauntlet-app/tests/roles.rs` — new teeth: D01 (1), D04 (2), D13 (3),
  D03 (4), D06 (3); `TOOL_VOCAB` fixture; all call sites threaded.
- `crates/gauntlet-app/src/compare/envelope.rs` — D07 additive seam
  `assert_within_role_deny_set` + `EnvelopeError::DeniedContextPresent` +
  teeth test. Zero call-site changes elsewhere.
- `crates/gauntlet-cli/src/main.rs` — D02 (roles_dir from the four-layer
  config, anchored), D12 (verify refuses unknown args), D06 wiring
  (`tool_vocabulary: spec_vocab::CAPABILITY_VOCABULARY`).
- `crates/gauntlet-arch-tests/src/single_writer.rs` — D11 reason text only
  (guard needles unchanged, no entries added or removed).
- `justfile` — D14 `roles-verify` recipe now invokes the CLI.
- `.agents/roles/{orchestrator,builder,verifier,optimizer,observer}.md` — D06
  value reconciliation (one token each); `.agents/roles/builder.md` + D09 §17.1
  citation; `.agents/roles/orchestrator.md` + D09 §10.7 hatch. Prose otherwise
  untouched.
- Reports: `b2-progress.md`, this file.
- `spec_vocab.rs`, denylist, literals, answer-key-surface inventory, all
  `.wayfinder/**`, `.gauntlet/**`, preregistration, CURRENT_STATUS, and the
  firewalled task17 files: NOT touched. (`cargo fmt` reformatted the three
  edited Rust files; `.agents/reports/doctor.json` is the doctor gate's own
  regenerated product from the required battery run.)

## 2. Spec quotes anchoring every vocabulary decision (D06 especially)

**§8.4** (tools — the closed 17-token list, verbatim):

> Tools bind by **semantic capability**, never by product:
>
> ```text
> code_nav · filesystem · versioning · test · build · browser · visual · web · research
> security · sandbox · memory · evidence · domain · policy · benchmark · schema
> ```

**§10.1** (front-matter grammar — the permission VALUE spellings, verbatim):

> ```yaml
> permissions:
>   filesystem: read-only
>   write_paths: ["$RESULT_CONTRACT_DIR"]
>   network: deny
>   tools: [code_nav, filesystem]
> ```

**§10.2** (role policy table — the builder row, verbatim):

> | `builder` | target implementation, its own tests, its workspace | reference (read-only), map, its unit(s), prior residuals for its unit | active reference, comparison contract, acceptance gates, evidence store, answer-key status |

**§17.3** (tier prose — the read-write mode word, verbatim):

> | `contained` | Ephemeral workspace, deny-by-default network, read-write limited to the workspace, no policy retention. |

**§13.2** (allowlist — the five envelope-permitted categories, verbatim):

> ```text
> reference (frozen, or the relevant projection of it)
> observation bundle (instrument facets only)
> comparison contract (dimension, rubric, severity definitions)
> applicable constraints
> unresolved residuals that remain relevant
> ```

**§17.5.5** (protected paths, verbatim):

> The evidence store, state store, frozen reference, and answer key are outside every builder-writable path.

**§10.7** (operator hatch, quoted into orchestrator.md, verbatim):

> If the environment genuinely provides none, direct implementation is permitted only when `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`; otherwise the run stops with `blocked{no_delegation_mechanism}`.

**§17.1** (never-lower rule — builder.md's corrected citation, verbatim):

> A tier may be raised by an operator; it is never lowered by an agent.

Decisions derived from these:

- `FilesystemMode` accepts exactly `read-only` (§10.1's own grammar spelling)
  and `read-write` (§10.2 table wording "read-only"/role modify columns and
  §17.3's "read-write limited to the workspace"). Every other token is
  `RoleError::UnknownFilesystemMode { token, line }`.
- `NetworkMode` accepts exactly `deny` (§10.1's grammar spelling) and `allow`
  (the deny/allow word pair §10.1's context grammar establishes). Consistency
  note owed by the slice: the ports sandbox names its runtime posture
  `NetworkPosture::{DenyByDefault, Allowlist { hosts }}` — a FINER runtime
  concept. The role-file `allow` is the coarser grant; at task-20 composition
  it must lower into an allowlist requiring hosts, never a blanket-open
  posture. Both vocabularies treat deny as the closed conservative baseline;
  naming is consistent, granularity is deliberately different.
- `tools` entries validate against §8.4's 17 tokens; refusal is
  `RoleError::UnknownToolToken { token, line }`.

## 3. DEVIATION (one): D06(c) mechanism — recorded, flagged for settlement

The slice says the 17-token list lands "as a `pub const` in the roles module"
(gauntlet-app — an INNER crate). The frozen boundary suite makes that literal
reading impossible without gate edits my charter forbids:

- `denylist::Denylist::derive` derives adapter LAST segments as whole string
  literals denied in domain/ports/application. Three of §8.4's own tokens are
  such segments: `test` (gauntlet-adapter-observe-test), `build`
  (gauntlet-adapter-observe-build), `memory` (gauntlet-adapter-meter-memory).
  The scan covers every file under the core crates, `tests/` included
  (`walk::is_core_path`), so neither production nor test code in gauntlet-app
  can spell them.
- This exact collision is the documented reason
  `crates/gauntlet-cli/src/spec_vocab.rs` exists: "The composition root is
  the suite's own designated place where naming is legal; the application
  layer consumes the lists as data" — and doctor already consumes
  `spec_vocab::CAPABILITY_VOCABULARY` through `DoctorInput::new`.

Landed mechanism (the doctor precedent, byte-for-byte in shape): the
VALIDATION lives in the roles module (`RoleDefinition::load` /
`walk_permissions` refuse unknown tokens with typed errors naming token +
line); the closed LIST is injected by the composition root via
`InstallInput::tool_vocabulary ← spec_vocab::CAPABILITY_VOCABULARY`. The
in-crate test fixture carries the fourteen collision-free §8.4 tokens (the
repository's artifacts use only `code_nav` and `filesystem`, both present).
Live evidence that the REAL list governs the REAL artifacts: smoke LEG 1/2
installed all ten repository artifacts through the composition root — any
non-§8.4 tool token would have failed install. Alternative rejected: growing
`SPEC_VOCABULARY_SEGMENTS` (outside touch surface, weakens a frozen gate);
evasion spellings (contradict the scanner's documented posture). If the
orchestrator prefers the const in the roles module, that requires a boundary
decision (segment exemption or an app-layer const fed from the CLI at test
time) — not something this slice could do honestly.

Two smaller recorded deviations (conservative generalizations inside the
slice's mandate, both noted in code):

- D03: anchoring RELATIVE write_paths at the project root also routes a
  relative `../`-traversing entry into the existing `WritePathEscape`
  refusal (previously relative entries skipped the escape check entirely —
  the critic's "undefined anchoring semantics" note).
- D02: `resolve()` returns the RAW merged layer (`effective`), whose dir
  strings can be relative; install's own containment requires absolute
  roots, so `roles_context` now calls
  `resolved.effective.resolve_against(&env, &project_root)` (the anchored
  `PathBuf` products the slice's resolve.rs:1166 reference points at) and
  passes BOTH anchored dirs. This replaced the previous
  `effective.state_dir.into()` — without it the configured roles_dir fails
  install's own §10.1 root containment (demonstrated live: the first smoke
  leg failed exactly so before the anchoring fix).

## 4. RED → GREEN evidence per owed tooth

**COR-D01** (the defect is the failing-missing test — the hole demonstrated
first): `forged_empty_lock_with_recomputed_digest_is_tamper` forged a
canonical, self-digest-consistent empty-entries lock; pre-correction verify
returned GREEN — verbatim panic:
`an empty-entries lock is hand-forged, never runtime-written: VerifyReport { harness: "writable-harness", enforcement: Writable, roles: 0 }`.
Fix: `RolesLock::parse` refuses zero entries with `LockTampered` whose detail
states the construction argument (install's `EmptyRoleSet`). GREEN after.

**COR-D04** (same protocol): pre-correction, planting `evil.md` in the
installed dir left verify GREEN —
`an installed file the lock does not record is drift: VerifyReport { … roles: 2 }`;
likewise a new source file post-install. Fix: verify lists both directories
(anchors derived from the lock's own recorded paths, `contained_join`ed) and
reports unrecorded `.md` files as `InstalledExtra` / `SourceExtra`; extra
files are REPORTED, never loaded. Both tests GREEN after.

**COR-D07** (compile RED): the teeth test failed to compile against the
pre-correction shape — verbatim: `error[E0425]: cannot find function
'assert_within_role_deny_set' in module 'super'` (×4) and `error[E0599]: no
variant named 'DeniedContextPresent' found for enum 'EnvelopeError'`.
Implementation landed; test GREEN.

**COR-D13** (mutation RED): `contained_join` was mutated to join
unconditionally (`if false && (…)`); both forged-path teeth FAILED —
`forged_traversal_source_in_lock_is_tamper` and
`forged_absolute_source_in_lock_is_tamper` each got `Drift { … kind:
SourceModified … }` instead of `LockTampered` (summary: 3 tests run: 1
passed, 2 failed). Mutant restored; 3/3 GREEN. The `SourceMissing` tooth
(`deleted_source_file_is_source_missing_drift`) covers the fourth drift kind.

Also-captured gate interactions while landing (all resolved without touching
any gate): the literals scanner flagged comma-introduced spec-section
numerals inside my new Display strings joining deadline-hinted text
(reworded: "the spec 10.2 role table" etc.), the multi-segment protected-path
literals (anchors now DERIVED from `defaults` single-segment constants and
`answer_key_at(...).parent()`), the `/etc/hosts` test literal (temp-dir
built), and the answer-key-surface pin (install.rs no longer spells the
ruler dir at all — it derives it). `File::open + sync_all` was chosen for
D05's fsync specifically to avoid introducing a `File::create`/`OpenOptions`
write-API token the single-writer inventory would have to re-litigate.

## 5. Live CLI smoke (D02 / D12 / D14) — verbatim

Setup: `GAUNTLET_STATE_DIR=/tmp/gauntlet-t17-b2-state`,
`GAUNTLET_ARTIFACT_DIR=…/artifacts`, `RESULT_CONTRACT_DIR=…/results`, the
twelve `GAUNTLET_{ROLE}_{RUNNER,MODEL}` vars set to placeholders. Binary:
`cargo build -q -p gauntlet-cli`.

- LEG 1 (baseline, default roles dir): `roles: installed 10 role(s) for
  harness 'prime-agent' (enforcement: writable)`, `roles: lock:
  /home/sprime01/projects/gauntlet/.gauntlet/roles.lock`, exit 0. Lock's
  builder digest: `sha256:db3a75dd…`.
- LEG 2 (D02): `GAUNTLET_ROLES_DIR=<repo>/.agents/roles-t17-b2-smoke` (a
  copy with ONE file changed — a marker line appended to builder.md):
  install exit 0; the lock's builder digest became
  `sha256:b9db57f788df964b4a2da3194092fc864fbfc9c3531f3c8a01924e3e32ff2ad9` —
  exactly `sha256sum` of the modified override file, differing from LEG 1's.
  The configured roles.dir demonstrably reaches the installer. (An
  out-of-root `/tmp` override was first attempted and correctly REFUSED by
  the pre-existing §10.1 `SourceOutsideProject` guard — evidence the override
  reached install and was validated, and that the guard is orthogonal to
  D02.)
- LEG 3 (D02, empty override): `GAUNTLET_ROLES_DIR=<repo>/.agents/
  roles-t17-b2-empty` → `roles: the roles directory '…' carries no role
  files — an install that recorded nothing would be dishonest success (spec
  10.1)`, exit 1 (`EmptyRoleSet`).
- LEG 4 (default reinstall + verify): `roles: verified 10 role(s) for
  harness 'prime-agent' (enforcement: writable) — no drift`, exit 0.
- LEG 5 (D12): `gauntlet roles verify --harness prime-agent` → `gauntlet
  roles verify: unknown argument '--harness'` + usage, exit 2.
- LEG 6 (D14): `just roles-verify` → `cargo run -q -p gauntlet-cli --
  roles verify` / `roles: verified 10 role(s) … — no drift`, exit 0. No
  `roles-install` placeholder exists anywhere in the justfile (checked;
  `schemas` remains a later-task placeholder, untouched).

Scratch `/tmp/gauntlet-t17-b2-*` and the in-repo smoke dirs removed;
`.gauntlet/roles.lock` removed; `.gauntlet/` back to its pre-smoke contents.

## 6. Battery (final, verbatim summaries)

- `just ruler` (start): `verify-ruler.sh: PASS — answer key and manifest
  digests match the frozen lock`.
- `just test-package gauntlet-app`: `Summary — 386 tests run: 386 passed,
  0 skipped`.
- `just check`: `Finished 'dev' profile … in 5.01s` (workspace, all targets).
- `just test`: `Summary — 1225 tests run: 1225 passed, 0 skipped`.
- `just lint`: clippy `-D warnings` Finished + `cargo fmt --check`, exit 0.
- `just boundaries`: `Summary — 228 tests run: 228 passed, 0 skipped`.
- `just deny`: `advisories ok, bans ok, licenses ok, sources ok`.
- `just doctor`: `exit: OK` (report at `.agents/reports/doctor.json`).
- `just ruler` (end): `verify-ruler.sh: PASS — answer key and manifest
  digests match the frozen lock`.

## 7. Per-correction index

| Id | What landed | Evidence |
|---|---|---|
| D01 | `RolesLock::parse` refuses empty entries (`LockTampered`) | RED hole → GREEN |
| D02 | CLI passes the configured, anchored roles_dir to install | smoke LEG 1–4 |
| D03 | `WritePathProtected` + resolved-shape validation + derived anchors | 4 tests |
| D04 | `InstalledExtra` / `SourceExtra` membership drift | RED holes → GREEN |
| D05 | staging-file fsync before rename; doc states scope (R17-note-1 out) | inspection + suite |
| D06 | `FilesystemMode` / `NetworkMode` enums + tool-token refusal + 5 artifacts reconciled | 3 tests + `repository_role_artifacts_load` + smoke |
| D07 | `assert_within_role_deny_set` seam, zero call sites | compile RED → GREEN |
| D09 | builder.md §17.1; orchestrator.md §10.7 hatch (spec-quoted) | diff |
| D11 | single_writer roles reasons reworded (guard needles unchanged) | `just boundaries` 228/228 |
| D12 | `roles verify` refuses any argument (exit 2) | smoke LEG 5 |
| D13 | traversal/absolute forged sources + `SourceMissing` teeth | mutation RED → GREEN |
| D14 | `just roles-verify` runs the real CLI | smoke LEG 6 |

Builder's claim of completeness carries no settlement authority; the battery
and this report are the evidence for independent verification.
