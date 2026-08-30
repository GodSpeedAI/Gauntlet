# Task 17 — B3 progress checkpoint

## Firewall declaration
Read so far: correction-round1.md (contract), `just ruler` output. No firewalled
reports touched. Will read only: touch-surface sources/tests, Core-spec §8.1,
§10.1–10.3, §13.2, answer-key AK-063.

## Start gate
`just ruler` (verbatim, 2026-08-30):
```
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

## Plan
1. Read touch surface: crates/gauntlet-app/src/roles/{definition,install}.rs,
   crates/gauntlet-app/tests/roles.rs; spec §8.1, §10.1–10.3, §13.2; AK-063.
2. RED-first per tooth (mutation/restore with sha256 proof, or demonstrate the
   defective behavior).
3. Land COR-R1-1..4; typed errors; no bare strings; no unwrap/expect in prod paths.
4. Battery serial: check-package → test-package (386/386 + new) → check → lint →
   boundaries → deny → ruler.
5. Finish b3-builder.md.

## Log
- [t0] Ruler PASS recorded above. Reading touch surface.
- [t1] Touch surface read in full: definition.rs (952), install.rs (1056),
  tests/roles.rs (1359), harness.rs, main.rs roles section, Core-spec §8.1/
  §10.1–10.3/§13.2, AK-063. Firewall clean — no firewalled file opened.

## Design (per contract)
- COR-R1-1: `VerifyInput` gains `roles_dir` + `registry` (plumbing only first).
  verify(), after parse and BEFORE digest comparisons: (a) lock harness must be
  registered (LockTampered naming the label); (b) lock enforcement must equal
  Enforcement::from_surface(registered surface); (c) per entry, `source` must
  equal rel_to(project_root, roles_dir/<stem>.md) and `installed` must equal
  rel_to(state_root, roles/<surface.dir_name()>/<harness>/<stem>.md) — mismatch
  = LockTampered naming entry + divergence. Bounded-scope note for the report:
  a FULLY consistent relabel+relocation (label + enforcement + relocated paths
  + files actually copied) is byte-indistinguishable from a legitimate
  reinstall under the other harness — nothing in the record can distinguish it;
  the checks refuse every partially-forged shape.
- COR-R1-2: new `DeclaredPath { raw, resolved }` (manual Debug showing raw
  only) for `RolePermissions.write_paths`; walk_permissions parses/validates
  the RESOLVED value but error token fields carry the RAW (unresolved) token;
  installer path refusals echo `entry.raw()`, never the resolved path. Mode
  refusals: raw echoed, resolved parsed. Display messages name the SHAPE.
- COR-R1-3: in RolesLock::parse (before Ok): repeated stem → LockTampered
  naming the stem.
- COR-R1-4: install captures per-role installed bytes before write; repaired =
  a previous lock existed AND any role's installed bytes changed (before≠after,
  including missing→present). Never over-reports (first install false;
  unchanged reinstall false).
- Teeth (6 new tests): source-redirect reforge refusal; cross-harness
  relabel+relocate tamper (incl. unregistered label); duplicate-entry tamper;
  reinstall-over-drift reports repair; canary-no-echo mode refusal;
  canary-no-echo write-path escape.

## RED discipline
Stage A = plumbing only → check-package green. Stage B = add teeth → run
package tests → expect exactly the 6 new RED. Stage C = implement → GREEN.

## Deviations
- D1: main.rs touched beyond "message shape" — verify needs the configured
  roles dir + registry to derive expected paths; input plumbing only, stderr
  shape unchanged, no new CLI surface.
- D2: mod.rs NOT touched (not in touch surface): DeclaredPath is reachable as
  `gauntlet_app::roles::definition::DeclaredPath` (definition is a pub mod).

## Log (cont.)
- [t3] Stage A done: VerifyInput gained roles_dir+registry (install.rs), main.rs
  plumbing updated, 15 test verify call sites + verify_without_install registry
  binding updated. check-package GREEN. (Sed initially duplicated fields inside
  InstallInput blocks — caught by E0062, reverted with a targeted perl pass.)
- [t4] Stage B RED: 6/6 new teeth FAILED on the pre-correction behavior, each
  for the expected defective reason:
  * full_lock_source_redirect_reforge…: got `Drift` (shape-invalid lock treated
    as drift; with the entry digest recomputed the pre-correction verify would
    have been fully green — test strengthened to the full reforge before Stage C)
  * cross_harness_relabeled_relocated…: unregistered label → `Ok`-shaped digest
    pass / relocated variant → `Drift`, never tamper
  * duplicate_entry_lock_is_tamper: canonical duplicate-entry lock accepted → Ok
  * reinstall_over…reports_the_repair: repaired=false (lock bytes identical)
  * both COR-R1-2 canary teeth: resolved CANARY echoed verbatim in the refusal
- [t5] Stage C landed:
  * definition.rs: `DeclaredPath { raw, resolved }` (manual Debug = raw only);
    `expect_resolved_pair` / `expect_resolved_string_list`; walk_permissions
    validates RESOLVED, echoes RAW in UnknownFilesystemMode/NetworkMode/
    ToolToken; write_paths: Vec<DeclaredPath>.
  * install.rs: write-path walk validates entry.resolved(), errors carry
    entry.raw(); WritePathEscape/WritePathProtected Display names the SHAPE +
    raw token; enum doc updated; RolesLock::parse refuses duplicate stems
    (LockTampered naming the stem); verify: registered-harness check,
    enforcement-vs-surface check, per-entry derived source/installed path
    checks BEFORE digest comparisons; install: repaired = installed-bytes
    before/after delta per role; InstallReport doc; module tamper story +
    membership-scan comments updated.
- [t6] GREEN: check-package clean; 6/6 new teeth PASS (nextest, narrow filter).


