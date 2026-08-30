# Task 17 — orchestrator-completion corrections (heal+continue, window closed)

The builder died on the usage window before running its own tests. The
orchestrator completed the slice personally per the operator's standing
heal+continue instruction (window reset 2026-08-30 14:06:59; degraded
independence recorded in IN-FLIGHT). Corrections found and landed during
completion, in discovery order:

## Latent builder defects (found by RUNNING the never-run tests)

| Id | Defect | Fix |
|---|---|---|
| COR-O-fix-1 | The lock SERIALIZER wrote bare hex (`digest: <hex>`) while the strict PARSER requires the prefixed token (`digest: sha256:<hex>`) — every verify after install failed `LockSchema` at the entry digest line. Serialize/parse asymmetry; the builder never ran its tests. | `install()` records `sha256:<hex>` digests. |
| COR-O-fix-2 | `verify()` compared BARE `sha256_hex(bytes)` against the lock's prefixed digest — every check reported phantom drift (expected `sha256:x`, actual `x`). | Both compare sites (source-side, installed-side) compute the prefixed token. |
| COR-O-fix-3 | The reforge TEST wrote `lock_digest: <hex>` (unprefixed) — its own forged lock was schema-invalid before reaching the interesting check. | Test writes `lock_digest: sha256:<hex>` (the lock's documented form). |
| COR-O-fix-4 | The deny-set determinism test asserted ALPHABETICAL first element; the derived `Ord` sorts by variant order (deterministic, different collation). The test asserted a collation, not the property. | Asserts sortedness itself (`windows(2).all(<)`), which is the actual guarantee the module documents. |
| COR-O-fix-5 | `DenyShadowsAllow` carried `line: 0` ("line 0" in the message) — the deny list's line was dropped in the walk. | Deny entries carry their list line; the refusal names the true locus. (Minor, found in review.) |
| COR-O-fix-6 | A debug `eprintln!("ADVISORY LOCK DUMP")` was left in the advisory test. | Removed. |

## Teeth finding (found by the owed RED discipline — the important one)

**COR-O1 — the lock canonical-closure check had ZERO discriminating
tests.** Gutting the canonical re-serialization equality
(`serialize_body() != join_lines(body)` → `false && …`) left ALL 21
tests GREEN. The hand-edit test passes via the SELF-DIGEST check; the
reforge test passes it by construction (its forged body IS canonical)
and is caught by DRIFT. The discriminating shape — a parse-transparent
but non-canonical edit (trailing whitespace on a body line, invisible
to the strict prefix parse) WITH a recomputed self-digest (so the
digest check passes) and unchanged content digests (so drift is silent)
— was covered by no test: without the closure, that forgery verifies
GREEN.

**Correction (tests-only):** `noncanonical_lock_with_recomputed_digest_
is_tamper` added. RED proven: closure gutted → exactly that test RED →
pristine restore → 22/22 GREEN. Same class as task15 U2 / task16 F1+F4
(missing teeth for a correct mechanism), found this time by the
orchestrator's own RED evidence duty, pre-falsifier.

## Arch-surface integrations (the recovered code tripping settled gates)

- **Denylist:** `walk_permissions`'s §10.1 key literal `'filesystem'`
  tripped `adapter_selection_is_config_driven` (the word is the last
  segment of `observe-filesystem`). Resolved by the established
  spec-vocabulary mechanism (Task 10/11/14 precedent):
  `SPEC_VOCABULARY_SEGMENTS` + `filesystem`, with the documented
  justification (§10.1's own mandatory `permissions.filesystem` field
  key; the distinguishing spellings `observe-filesystem`,
  `filesystem-observe`, and the full crate name remain denied) and the
  pin test grown (now asserts the five segments and the
  filesystem-derived spellings still deny).
- **Single-writer:** the installer's four write sites
  (`create_dir_all` ×2, `fs::write` staging, `fs::rename` commit) are
  now pinned in `WRITE_API_INVENTORY` with the `ensure_within_root`
  guard; both pin tests grown (inventory pin + clean fixture carries
  the roles file). Installer writes are operator-action state-root
  writes, never graded-role capabilities.

## Deviations recorded during completion

- **Untracked-file restore:** the first tooth's restore used
  `git checkout --` on an UNTRACKED file (the whole roles/ dir is new)
  — a no-op; the mutation survived and was re-repaired by re-inserting
  the block. Same class as the task16 falsifier's recorded deviation;
  pristine-COPY restores only from then on (teeth 2, 3, and COR-O1 all
  used `cp` + sha256 verification).
- **CLI wiring done by the orchestrator** (roles install/verify
  subcommands, registry keyed by the adapters' exported RUNNER_TOKEN
  constants, four-layer config resolution mirroring doctor's
  discovery): the builder died before this step. Composition facts
  live in the composition root (§4.1) — the registry maps
  `prime-agent` → WritableRoleDir, `cli-agent` → AdvisoryPrompt
  Artifacts.
- **`.gitignore` gains `.gauntlet/roles.lock`** (runtime-owned host
  state written by operator installs — never a committed artifact;
  same hazard class as the stray-probe incident).

## Live verification through the real binary (2026-08-30 ~08:2x EDT)

`gauntlet roles install --harness prime-agent` → 10 roles, writable ·
`verify` GREEN · second install → BYTE-IDENTICAL lock (sha256) ·
out-of-band edit to an installed artifact → verify RED with typed
per-role digests · reinstall repairs, verify GREEN · `--harness
cli-agent` → advisory path with the honest weakness line · unregistered
and empty harness ids fail closed with typed errors · indirections
absent from the environment fail closed naming the variable, never the
value · `just doctor` exit OK with a lock present (D2 live).

## Battery (orchestrator, personally, on the completed tree)

check PASS · **test 1211/1211** (1189 + 22 roles) · lint clean (fmt
applied to the new code) · **boundaries 228/228** · deny ok · doctor
exit OK · ruler PASS · prereg-check 17 PASS (frozen b3616216
untouched).
