# Task 17 — Blind Executing Falsifier, Round 1

Status: COMPLETE — verdict NOT_CONFIRM (2 major, 3 minor findings; no blocking). This file is the result contract.

## Provenance

- HEAD verified: `1de35bb` (matches brief). Ruler at start: **PASS** (verify-ruler.sh, digests match frozen lock).
- Attacking the CURRENT WORKING TREE (carries uncommitted task17 modifications to
  `crates/gauntlet-app/src/roles/**`, `tests/roles.rs`, `compare/envelope.rs`,
  `crates/gauntlet-cli/src/main.rs`, `crates/gauntlet-arch-tests/src/single_writer.rs`, `justfile`,
  `.agents/roles/*.md`) — this is the surface under attack per the brief ("HEAD 1de35bb or later").
- Firewall honored: `.agents/reports/task17/*` (except this file, write-only), preregistrations,
  CURRENT_STATUS.yml, task15/task16 reports, `.agents/agents/**` — not opened.

## Plan (composed blind from the brief; refined after reading code)

1. Read surface: roles module (definition/category/harness/install), tests/roles.rs,
   compare/envelope.rs, cli/main.rs, arch tests, `.agents/roles/*.md`, spec anchors
   (§8.4, §10.1–10.3, §13.2, §17.5.5, Annex B, AK-063).
2. Attack families (floors from brief, expanded as code suggests):
   - A: Lock forgery (empty/recomputed/lying/traversal/dup/cross-harness fields).
   - B: Drift coverage (add/del/rename/case/type-swap at installed + source sides).
   - C: Loader fail-closed surface (keys, categories, intersection, permission values,
     $VAR classes, duplicates, no-body, FM-after-body, CRLF/BOM).
   - D: Installer containment (write_paths escapes, protected set, `..`/case/absolute,
     state-root escapes via harness id/stem, advisory fallback).
   - E: Secrets leakage of $VAR values (lock, installed bytes, errors, Debug, stderr; canary).
   - F: CLI surface (arg parsing, unknown args, exit codes, config discovery, overrides).
   - G: Envelope seam consume-side (reachability, deny-set behavior, teeth).
   - H: Role artifacts (all ten load; grammar conformance; hostile artifact).
   - T: Teeth proofs on 3–5 least-tested load-bearing guards (mutate→RED→sha256 restore→GREEN).
3. Hygiene: package-scoped tests first; live legs only against /tmp/gauntlet-falsifier-t17-* roots;
   ruler at end; scratch sweep; `.gauntlet/` cleanliness proof.

## Executed attacks

L0 baseline (executed): `just ruler` PASS (digests match frozen lock); `just check-package
gauntlet-app` clean (scratch suite compiles with NO harness repairs needed); `just test-package
gauntlet-app` 402/402 GREEN. All A–E scratch attacks below were EXECUTED in that run
(nextest, `gauntlet-app::falsifier_t17_scratch`).

| Id | Attack | Outcome |
|----|--------|---------|
| A-1 | Cross-harness lying lock re-forge (lock harness label + a NEW harness dir of identical copies) | EXECUTED (test `a1_cross_harness_relabeled_lock_passes_verify`): verify returns GREEN — a fully hand-forged, canonically re-serialized lock with a lying `harness:` label and entries pointing at a different installed dir passes every verify check (canonical closure, self-digest, both content digests, both set-membership scans). See Finding FIND-1. |
| A-2 | Duplicate-entry lock (same role twice, self-consistent digests) | EXECUTED (test `a2_duplicate_entry_lock_passes_verify`): verify returns GREEN — duplicate entries pass the lock parser and both digest loops. See Finding FIND-3 (minor). |
| B-1 | Source rename with case change (builder.md → Builder.md) | EXECUTED (test `b1_source_case_rename_is_drift_both_ways`): caught — SourceMissing drift for the old name plus SourceExtra for the new name; verify RED. |
| B-2 | Installed `.md` replaced by a directory of the same name | EXECUTED (test `b2_installed_type_swap_to_directory_fails_closed`): fail-closed — `fs::read` on the directory is a typed Io failure, verify RED (not a Drift record, but never a pass). |
| C-1 | Missing-field catalog (no role / no description / no permissions / no context / no context.deny) | EXECUTED (test `c1_missing_fields_are_refused`): every missing required field is a typed refusal. |
| C-2 | `context.allow: []` (EmptyAllow) | EXECUTED (test `c2_empty_allow_is_refused`): refused. |
| C-3 | Duplicate top-level key (grammar DuplicateKey) | EXECUTED (test `c3_duplicate_top_level_key_is_refused`): refused by the shared grammar. |
| C-4 | UTF-8 BOM prefix | EXECUTED (test `c4_bom_prefix_is_refused`): refused, fail closed. |
| C-5 | CRLF line endings in a role file | EXECUTED (test `c5_crlf_role_file_fails_closed`): fails closed (typed parse failure, not silent acceptance). |
| C-6 | `$VAR` indirection classes (`$X`, `${X}`, `$$X`, `$X-suffix`, lone `$`, braced-with-garbage) | EXECUTED (test `c6_indirection_classes_are_presence_validated`): presence-validated; missing vars named by variable+locus. |
| C-9 | `deadline_seconds: 0` | EXECUTED (test `c9_zero_deadline_is_refused`): refused. |
| C-10 | Registry id grammar refusal (uppercase/traversal id at insert) | EXECUTED (test `c10_registry_id_grammar_is_closed`): refused at insertion (RegistryError). |
| D-1 | Relative `../` traversal write path | EXECUTED (test `d1_relative_traversal_write_path_is_refused`): refused as WritePathEscape. |
| D-2 | Unknown nested key under permissions / context (case-swapped `Write_Paths`) | EXECUTED (test `d2_case_swapped_nested_keys_are_refused`): refused with the field named (`permissions.Write_Paths`); non-map permissions also refused. |
| E-1 | Resolved-value echo: canary value in a mode/path field lands in typed error + stderr | EXECUTED (tests `e1a_resolved_value_echoes_into_error_display`, `e1b_resolved_path_value_echoes_into_escape_error`): CONFIRMED LEAK — a resolved `$VAR` value in an invalid `filesystem:` mode token appears in the RoleError Display, and a resolved `$VAR` write-path value appears in the WritePathEscape Display. This contradicts the module's own documented invariant (definition.rs: "the VALUE is never echoed"; install.rs RolesError doc: "no resolved indirection VALUE is ever carried"). See Finding FIND-2 (major). |
| A-3 (added during execution) | Source-redirect reforge: drift the installed artifact, re-forge the lock so the entry's `source:` points at a fabricated IN-PROJECT dir holding the drifted bytes; reviewed source untouched; digests+self-digest recomputed | EXECUTED (test `a3_source_redirect_reforge_blesses_drift_without_touching_reviewed_source`, composed during this round): verify returns GREEN. This falsifies install.rs's documented tamper story ("the only way to hide that drift is to edit the reviewed source file itself") — the source anchor moves with the forged lock. See Finding F-1 (major). |
| F-1 | CLI: unknown subcommand exit code | EXECUTED (live CLI, scratch root): `roles bogus` → stderr "unknown subcommand 'bogus'" + usage, exit 2. |
| F-2 | CLI: `roles install` missing `--harness` machinery (missing value, duplicate flag, unknown arg) | EXECUTED (live CLI): no `--harness` → "…--harness is required" exit 2; missing value → "…requires an id" exit 2; duplicate → "…given twice" exit 2; unknown `--force` → "…unknown argument" exit 2. |
| F-3 | CLI: `roles verify --harness x` (COR-D12 unknown-arg refusal) | EXECUTED (live CLI): `roles verify --harness prime-agent` → "unknown argument '--harness'" + usage, exit 2. COR-D12 holds. |
| F-4 | CLI live leg: verify-without-install exit + message | EXECUTED (live CLI, scratch root): "no lock at '…/.gauntlet/roles.lock' — nothing is installed…; missing evidence is not passing evidence", exit 1. Scratch root resolved correctly (no repo `.gauntlet/` touched). |
| F-5 | CLI live leg: install unregistered harness exit + message | EXECUTED (live CLI): `--harness no-such-harness` → typed UnknownHarness ("unknown ids fail closed"), exit 1. |
| F-6 | CLI live leg: scratch install → green verify → out-of-band edit → drift exit 1 | EXECUTED (live CLI): install 10 roles → verify green → appended a line to the installed builder.md → verify reports "[builder] installed artifact modified out-of-band (recorded sha256:db3a75dd…, actual sha256:40e9778d…)", exit 1; reinstall repairs → verify green. |
| F-7 | CLI live leg: `GAUNTLET_ROLES_DIR` env override reaches install (COR-D02) | EXECUTED (live CLI): override INSIDE the project root reaches install — lock records `source: other-roles/builder.md`, install green. (An override pointing OUTSIDE the project root is refused as SourceOutsideProject — containment also observed.) Note: first attempt's copy into the scratch dir silently failed (shell env quirk, not surface behavior); redone cleanly. |
| G-1 | Envelope seam: deny-set refusing a carried category; unrelated deny refuses nothing | EXECUTED via the surface's own seam test `compare::envelope::tests::a_role_deny_set_naming_a_carried_category_refuses_the_input_set` (GREEN in the 402-test run; assertions read and verified honest: denies carried `reference` → DeniedContextPresent; denies present `open_residuals` → refused; denies absent `applicable_constraints` → Ok; denies `builder_trajectory` (un-carriable) → Ok). Seam is `pub fn assert_within_role_deny_set`, `#[allow(dead_code)]`, DELIBERATELY UNWIRED — zero production call sites (grep-verified); consumer is plan Task 20. Declared, not a defect. |
| H-1 | All ten repository role artifacts install through the real CLI into a scratch harness | EXECUTED (live CLI, scratch project root with copies of the repo's ten `.agents/roles/*.md`): `roles install --harness prime-agent` → "installed 10 role(s) … (enforcement: writable)", lock carries 10 entries; verify green; advisory harness `cli-agent` also installs 10 with the advisory disclosure line. NOTE: the bare environment fails first with a typed error (`critic` references `$RESULT_CONTRACT_DIR`, absent from the environment) — fail-closed, honest; with the variable set (as the runtime environment supplies it) install succeeds. Secret sweep over lock + installed bytes: NO resolved value present. |
| H-2 | Hostile artifact (over-grant allow) refused by real install with typed error | EXECUTED (live CLI): builder copy with `allow: [reference, builder_trajectory]` → "role 'builder' line 12: context.allow names 'builder_trajectory', which the 13.2 envelope never carries — …refused, not silently narrowed", exit 1. |
| E-1-CLI (added during execution) | E-1 through the real CLI: does the resolved canary reach stderr? | EXECUTED (live CLI): builder copy with `filesystem: $GAUNTLET_T17_CANARY`, env value `credential-canary-9f3a1c` → stderr prints "permissions.filesystem names 'credential-canary-9f3a1c', outside the spec's own mode vocabulary…" — the resolved VALUE reaches stderr of the real process. Confirms E-1 end-to-end. |

## Findings

- **FIND-1 — MAJOR — drift invisibility / lock-forgery anchor.**
  Locus: `crates/gauntlet-app/src/roles/install.rs` — `verify`'s per-entry digest loop +
  `contained_join` (any relative in-project `source:`/`installed:` path is accepted; nothing
  ties `entry.source` to the reviewed roles directory).
  Invariant defeated: install.rs's own tamper story — "Editing installed state and re-forging
  the lock to match still fails, because the untouched repository source no longer agrees with
  the forged record — the only way to hide that drift is to edit the reviewed source file
  itself" and "the cryptographic anchor is the source tree". Executed attacks falsify both:
  (a) A-3: after drifting the installed artifact, a lock re-forged to point the entry's
  `source:` at a fabricated in-project directory (`rogue-src/builder.md`) holding the drifted
  bytes — entry digest and self-digest recomputed, canonical form preserved, reviewed source
  never touched — verifies GREEN. (b) A-1: the same forgery capability relabels the lock's
  `harness:` field and points `installed:` at a fabricated harness dir; verify GREEN.
  The source anchor therefore moves with the forged lock: whoever can write
  `.gauntlet/roles.lock` can bless arbitrary installed-state content as if it were the reviewed
  source, without the repository's review surface ever seeing an edit. (The surface's own
  forge-test suite treats lock forgery as in-scope, so the threat is not hypothetical.)
  Reproduction: scratch test `a3_source_redirect_reforge_blesses_drift_without_touching_reviewed_source`
  — verify returns `Ok(VerifyReport{roles: 2})`.

- **FIND-2 — MAJOR — resolved `$VAR` value leaks into typed error text and process stderr.**
  Locus: `crates/gauntlet-app/src/roles/definition.rs` (mode/network/tool token errors carry the
  already-resolved token); `crates/gauntlet-app/src/roles/install.rs`
  (`WritePathEscape`/`WritePathProtected` Display carries the resolved path);
  `crates/gauntlet-cli/src/main.rs` prints `{error}` to stderr.
  Invariant defeated: definition.rs — "the typed error names the variable and the locus, and
  the VALUE is never echoed (spec §8.1)"; "never appear in the lock, in logs, or in error
  text"; install.rs — "no resolved indirection VALUE is ever carried".
  Executed: e1a (canary env value referenced by an invalid `filesystem:` token appears in the
  RoleError Display), e1b (canary resolved into a write path appears in the escape error
  Display), and E-1-CLI (the real `gauntlet roles install` printed `credential-canary-9f3a1c`
  to stderr). Any env value a role file can reference — exactly the credentials the
  indirection scheme protects — is exfiltratable by referencing it in a field that then fails
  validation; the typed error becomes the leak channel.

- **FIND-3 — MINOR — lock parser accepts duplicate entries.**
  Locus: `RolesLock::parse`'s entry loop; nothing rejects a repeated role stem. Executed (A-2):
  a hand-forged lock with the builder entry twice (self-consistent digests) verifies GREEN and
  inflates `report.roles` to 3. No content is weakened (digests still anchor bytes), but a
  canonical, self-digest-consistent lock with a shape install would never write is accepted
  where the empty-entries shape is refused (COR-D01's logic stops one entry short).

- **FIND-4 — MINOR — the repository's own artifacts do not install in a bare environment.**
  Locus: `.agents/roles/critic.md` (`write_paths: [$RESULT_CONTRACT_DIR]`) vs the CLI
  composition: `RESULT_CONTRACT_DIR` is not advertised by the CLI's own Annex E vocabulary and
  not set in a plain shell, so `gauntlet roles install` against a fresh checkout fails typed
  ("references indirection 'RESULT_CONTRACT_DIR' that the environment does not carry").
  Fail-closed and honest; the runtime environment presumably supplies it — but the operator-
  facing CLI cannot install the repository's roles without an env var it never names.
  Ergonomics/documentation gap, not a security defect.

- **FIND-5 — MINOR — the repair flag is lock-relative, not drift-relative.**
  Locus: `install.rs` (`repaired` = previous lock bytes differ) + CLI "this install repaired
  drifted state". Executed (F-6): after out-of-band installed drift, a reinstall that repairs
  the installed bytes prints NO repair notice, because the lock bytes are unchanged. The
  message under-reports (never over-reports); honesty/ergonomics.

No blocking defect found (nothing corrupts the ruler, the frozen reference, the frozen
preregistration, or the repository outside the runtime's own state; every fail-closed shape
attacked held). Two major findings stand unrepaired → verdict NOT_CONFIRM.

## Teeth proofs

Method: pre-digest (pristine `sha256` of `crates/gauntlet-app/src/roles/install.rs` =
`a9f4c4b268c8dfa0b8b2a0279cb5693d7d33ce76290345eedc8e54cb53fdee04`), single-mechanism mutation,
targeted nextest run (expect RED), restore from byte copy, digest match, targeted run again
(expect GREEN). Serial cargo throughout. Full `cargo-mutants` NOT run (host cost — requires
asking first, per brief).

| Tooth | Guard mutated | Targeted test | RED? | Restore digest | GREEN? |
|-------|---------------|---------------|------|----------------|--------|
| T1 (claimed: forged empty lock) | COR-D01 empty-entries refusal → tautology-false | `forged_empty_lock_with_recomputed_digest_is_tamper` | FAIL 1/1 (forged empty lock verifies green without the guard) | MATCH | PASS |
| T2 (claimed: canonical-closure trap) | closure check `serialize_body() != join_lines(body)` → `&& false` | `noncanonical_lock_…` + `hand_edited_lock_…` | FAIL 1/2 (noncanonical no longer refused) | MATCH | PASS 2/2 |
| T3 (claimed: out-of-band addition) | InstalledExtra membership scan → `false && …` | `out_of_band_installed_addition_is_drift` | FAIL 1/1 | MATCH | PASS |
| T4 (claimed: source addition) | SourceExtra membership scan → `false && …` | `source_addition_after_install_is_drift` | FAIL 1/1 | MATCH | PASS |
| T5 (extra least-tested guard) | installed-side digest compare → `false && …` | `out_of_band_installed_edit_is_drift` | FAIL 1/1 | MATCH | PASS |

All four claimed milestone teeth (forged empty lock, out-of-band addition, source addition,
canonical-closure trap) re-proven as actually tripping: disabling each guard turns the
corresponding committed test RED, and byte-identical restore (digest-verified) returns it GREEN.

## Not attacked

- Full `cargo-mutants` workspace/package sweep — expensive on this host; requires asking first
  (brief). Only the five targeted single-mechanism mutations above were run.
- Symlink-based escape from the installed tree or roles dir — the installer's containment is
  explicitly lexical (documented: "a resolved symlink check is the sandbox's job"); not probed.
- TOCTOU between containment checks and atomic writes; concurrent install/verify races.
- Case-insensitive filesystem semantics for B-1 (this host is case-sensitive WSL2 ext4).
- Unicode/homoglyph role stems against `is_valid_stem` (registry-id grammar attacked at C-10;
  the stem grammar only reached via the B-1 case-rename path).
- dotenv/`GAUNTLET.md` injection variants — the four-layer config merge was exercised only via
  `GAUNTLET_PROJECT_ROOT`/`GAUNTLET_STATE_DIR`/`GAUNTLET_ROLES_DIR` (F-4..F-7 legs).
- The composed-dispatch consumer of `assert_within_role_deny_set` (plan Task 20) — does not
  exist yet; the seam is deliberately unwired (grep-verified: zero production call sites).
- Doctor's contract checks over `roles.lock`, scheduler/authority gating, and everything
  outside the declared Task 17 surface.

## Hygiene

- Ruler start: PASS (see provenance). Ruler end: **PASS** (re-run after scratch removal).
- Repository untouched by attacks: all live CLI legs ran under
  `GAUNTLET_PROJECT_ROOT=/tmp/gauntlet-falsifier-t17-cli/*`; every roles.lock created lived in
  scratch. End state of `.gauntlet/`: exactly `comparison-contract.yaml`, `policies`,
  `reference.lock` (verified after the final run). No repo `.agents/roles/` file was written.
- Mutations: five targeted mutations on `crates/gauntlet-app/src/roles/install.rs`, each
  restored byte-identically (sha256 match against pristine
  `a9f4c4b268c8dfa0b8b2a0279cb5693d7d33ce76290345eedc8e54cb53fdee04`); `git status` shows only
  the pre-existing Task 17 working-tree modifications. No mutation left planted.
  `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`, frozen preregistrations: never
  modified, never written.
- Scratch sweep: removed this instance's artifacts — `/tmp/gauntlet-falsifier-t17-cli`
  (live-leg projects), `/tmp/gauntlet-t17-falsifier` (scratch-test fixtures),
  `/tmp/gauntlet-falsifier-t17-{teeth,tooth*.sh,teeth-chain.sh,*.log,install.rs.bak,install.sha256}`,
  `/tmp/gauntlet-t17-final-verify.{sh,log}`, the empty predecessor dir
  `/tmp/gauntlet-falsifier-t17-scratch`, `/tmp/gauntlet-t17-falsifier-legs`, and
  `/tmp/t17_scratch_run{,2..7}.log` (falsifier scratch-run logs). Also present in /tmp and
  deliberately LEFT: `/tmp/gauntlet-t17-tests/` (fixture base of the committed
  `tests/roles.rs` — regenerates on every test run, builder-side infra) and
  `/tmp/t17-predispatch-battery.log`, `/tmp/t17_a2.log`, `/tmp/t17_cli_build.log`
  (ambiguous ownership — plausibly builder/orchestrator-side; not this instance's scratch, so
  not removed without adjudication). No lingering cargo/nextest processes (ps-verified).
- Scratch suite disposition: `crates/gauntlet-app/tests/falsifier_t17_scratch.rs` REMOVED at
  round close per its header. Decision: no attack retained as a permanent tooth. Rationale:
  the guards that hold are already the committed `tests/roles.rs` tests (teeth-proven T1–T5);
  the attacks that exposed defects (A-1/A-3, E-1a/E-1b) currently PASS as demonstrations of
  defective behavior, so retaining them verbatim would codify the defect, and asserting their
  refusal would be RED until a correction lands — a falsifier must not plant failing tests.
  The correction task should re-introduce them as refusal tests, e.g.
  `full_lock_source_redirect_reforge_cannot_bless_drift` beside the existing
  `full_lock_reforge_cannot_bless_installed_drift`, plus a
  `resolved_indirection_value_never_echoes_into_error_text` pair.
- End verification (after scratch removal): `just check-package gauntlet-app` clean;
  `just test-package gauntlet-app` 386/386 GREEN; `just ruler` PASS;
  `ls -la .gauntlet/` clean (above). Blindness: no firewall-listed artifact was opened; no
  violation to record.

# Verdict

**NOT_CONFIRM.**

The surface's fail-closed discipline is real and heavily toothed. All twelve loader shapes
(C-family), both installer-containment shapes (D-family), both drift-rename shapes (B-family),
the CLI usage machinery (F-1..F-3), verify-without-install (F-4), unregistered-harness (F-5),
the hostile-artifact over-grant refusal (H-2), and the envelope deny-set seam (G-1) refused
exactly as claimed; all-ten-artifact install verifies green with no secret reaching the lock or
installed bytes (H-1); and five targeted mutation proofs show the load-bearing guards
(empty-lock refusal, canonical closure, both set-membership scans, the installed-side digest
compare) have live teeth — each goes RED when disabled and GREEN after a digest-verified
byte-identical restore. The four teeth the milestone claimed all trip.

But two MAJOR findings stand after EXECUTED attacks, each defeating a stated invariant of the
surface:

1. **FIND-1 (drift invisibility / lock-forgery anchor).** Verify's tamper story anchors drift
   detection to "the source tree", but the anchor is actually "whatever in-project path the
   (forgeable) lock names". A re-forged lock pointing `source:` at a fabricated in-project copy
   blesses out-of-band installed drift GREEN without ever touching the reviewed source (A-3),
   and the lock's `harness:` label is equally forgeable (A-1). The documented claim "the only
   way to hide that drift is to edit the reviewed source file itself" is false.
2. **FIND-2 (secret leakage).** Resolved `$VAR` values — the values §8.1's
   secrets-by-indirection scheme exists to protect — are echoed into typed error Display and,
   through the real CLI, onto stderr (e1a, e1b, E-1-CLI), contradicting the module's own
   documented "the VALUE is never echoed" / "no resolved indirection VALUE is ever carried"
   invariant.

Three minor findings (duplicate-entry lock acceptance, unadvertised `RESULT_CONTRACT_DIR`
dependency of the repository's own artifacts, under-reporting repair flag) are recorded and do
not by themselves defeat the verdict.

Verdict rule: CONFIRM only if no blocking and no major defect stands after executed attacks.
Two majors stand → **NOT_CONFIRM**.
