# Critic C — blind confirmation of the Task-5 correction round (K1–K10)

Verified each of critic-A's residuals (A-R1..R3) and critic-B's residuals (B-R1..R8)
against the CURRENT tree, by reproduction with the built `gauntlet-cli` on throwaway
trees under `/tmp/t5probe-c` (report dir and project root redirected; the repository
was not mutated) and by direct source/gate inspection. Builder history was not read.
Surface check: `git status` shows no changes under `crates/gauntlet-ports/` (ports
untouched, as required).

## Verdict — residuals-remain

Ten of eleven residuals are CLOSED with reproductions that no longer reproduce. One
residual is PARTIALLY closed: **B-R2** — the io-error-kind split is real (non-UTF-8
and permission-denied now fail check 1 with the right class), but the residual's
case 1 (an explicitly requested `--config` path that does not exist) still silently
conflates with "no contract": check 1 passes with "no GAUNTLET.md found", defaults
win, and a healthy project exits 0. No regressions were found in the areas the
corrections touched.

## Per-residual

| id | verdict | evidence |
|---|---|---|
| A-R1 | CLOSED | `checks.rs:621-641` `forbidden_entries_in`: stem `eq_ignore_ascii_case("irreversible")`, any extension; `symlink_metadata` so broken symlinks count; `checks.rs:524-555` scans effective AND default dir, unconditionally of backend. Live: `irreversible.yaml`, `IRREVERSIBLE.JSON`, `Irreversible.JSON`, extensionless `irreversible`, `irreversible.toml`, and a broken symlink `irreversible.json` each fail check 5 citing AK-049, exit 1. False positives clean: `irreversible-notes.json`, `irreversibles.json`, `irreversibl.json` pass. Note: the required-tier clause (`checks.rs:577`) is still exact `<tier>.json` — critic A itself called that direction fail-safe, and the adjudicated K4 covered only the forbidden direction. |
| A-R2 | CLOSED | `defaults.rs:134` `WORKSPACE_USE_WORKTREES` used at `resolve.rs:414`; `defaults.rs:56` `SANDBOX_BACKEND` used at `checks.rs:557`. `rg '"none"'` over `gauntlet-app/src` hits only defaults.rs (declaration + its own pin test). `just boundaries` green (190/190) — defaults module remains the only literals home. |
| A-R3 | CLOSED (documented path) | `report.rs:72-83` codifies the reading rule: a check-level pass relies only on clauses that pass; a skipped clause under a pass carries its reason and is never relied upon. Observed live in `just doctor`: check 5 `pass` with "tier policies exist" `skipped` + recorded reason, serialized in doctor.json. |
| B-R1 | CLOSED | `gauntlet_md.rs:311-315` rejects a leading U+FEFF with typed `ByteOrderMark { line: 1, col: 1 }` (`:206-209`, Display `:296-301` with remediation; tests `:961-975`, `doctor/mod.rs:434-448`). Live: BOM-prefixed GAUNTLET.md → check 1 fail "gauntlet md 1:1: the document begins with a byte-order mark (U+FEFF)…", exit 1; byte-identical control without the BOM parses (rounds 5, exit 0). |
| B-R2 | **NOT-CLOSED (partial: 2 of 3 cases closed)** | Closed: `doctor/mod.rs:114-127` splits the read — `Err(NotFound)` stays the honest absence; any other kind becomes typed `ConfigError::ContractUnreadable` (`error.rs:45-48`, class word `:98-104`). Live: non-UTF-8 GAUNTLET.md → check 1 fail "found but unreadable: invalid data (not valid UTF-8)", exit 1; mode-000 → "permission denied", exit 1; missing file → check 1 pass "no GAUNTLET.md found" (honest, preserved). **Still reproducing — case 1:** `gauntlet doctor --config /tmp/t5probe-c/proj/GAUNTLET.mdd` (typo; file absent) on an otherwise healthy scratch → check 1 **pass**, evidence "no GAUNTLET.md found; the environment fully specifies the run (spec 8.2)", budget falls back to defaults (`rounds 8`), **exit 0**. `discover_config_path` (`resolve.rs:1423-1438`) returns the explicit path verbatim and its own doc says "a missing file is not fatal"; `main.rs` does not validate the override. The operator-named file's absence is still flattened into "no contract" — the exact sub-case critic B called the worst. (The adjudicated K2 text scoped the correction to the kind split; the remainder is recorded here, severity major, for re-adjudication.) |
| B-R3 | CLOSED | `gauntlet_md.rs:681-688`: `read_flow_map` rejects duplicates with `DuplicateKey` carrying line:col (tests `:978-999`, `doctor/mod.rs:517-526`). Live: `builder: { runner: aaa, runner: bbb }` → check 1 fail "gauntlet md 9:28: duplicate key 'runner'", exit 1; the document no longer parses, so the fingerprint collapse is structurally gone; `{runner: aaa}` vs `{runner: bbb}` digest differently (sha256:cd3ed4a2… vs sha256:cc8f97f0…). |
| B-R4 | CLOSED | Both escapes from the residual are shut. Override-away: forbidden `irreversible.json` in default `.gauntlet/policies` while `sandbox.policy_dir: policies2` → check 5 fail, exit 1, both dirs evidenced ("policies2" clean line + ".gauntlet/policies" offending line); forbidden in the overridden dir also caught. Broken symlink `irreversible.json` → fail, exit 1 (`symlink_metadata`, links not followed). Indirected `policy_dir: ${PROBE_PDIR}/pdir-hidden` still caught, label withholds the resolved value (shows the raw token). |
| B-R5 | CLOSED | `canonical.rs:678-708`: the tautological loop is gone; the canonical-text assertions each discriminate independently (`contains("$TASK5_CANARY")` fails if the writer stops emitting tokens; `!contains("canary-value-xyz")` fails on a leak). The two digest-side assertions remain unfailable-by-construction (fixed-shape hex) but are documented as the opacity/shape guarantee and backed by the length pin — the misleading vacuity the critic flagged is removed. |
| B-R6 | CLOSED | `env_layer.rs:167` strips the `export ` prefix; `:199-213` strips a whitespace-preceded `#` from unquoted values only; `:146-151` matches the `GAUNTLET_` prefix case-insensitively so lowercase names are collected as unknown. Live: `export GAUNTLET_MAX_ROUNDS=5` in `.env` beats the file's 9 (rounds 5); `gauntlet_max_rounds=7` resolves nothing and is reported exactly once ("unknown variable 'gauntlet_max_rounds' from the dotenv file"); `GAUNTLET_STATE_DIR=…/state # inline note` probes the clean path and no `#`-named directory is created. Unit tests `env_layer.rs:290-336`. |
| B-R7 | CLOSED | `checks.rs:121-153,176-193`: value-conversion failures are their own "config values" clause. Live: `max_cost_usd: 1e3` / `-1` / `25.1.2` → "config values" **fail** naming `budget.max_cost_usd`; "indirections" is `skipped` with pointer evidence "indirection accounting stopped at a config value failure — see 'config values'"; no "indirection failed" text anywhere; exit 1. Reverse direction intact: a genuine `$NO_SUCH_VAR` still fails under "indirections" ("indirection failed: … presence is validated, the value is never printed"). Test `doctor/mod.rs:597-625`. |
| B-R8 | CLOSED | `.agents/reports/reference-candidates.md` exists, dated 2026-08-17, titled to the K8/critic-B-R8 correction. Its citations are real: the three named tests exist (`doctor/mod.rs:409`, `:529`, `:558`); `scan_cargo_target_paths` and the exemption list exactly `["crates/gauntlet-arch-tests"]` exist at `gauntlet-arch-tests/src/escapes.rs:456` and `:91`. The ruler was NOT moved: `git diff HEAD -- .wayfinder/ .gauntlet/` is empty and `just ruler` PASSes (digests match the frozen lock); AK-049 still declares its original locator in the answer key, with the dangling locator now a recorded candidate per the lock's own discipline. |

## Regression hunt — findings

**No regressions introduced by the corrections.** Areas probed:

- BOM mid-file: a BOM in the documentation body (after the closing fence) parses
  normally (config layer intact); a BOM starting a front-matter key line is a typed
  `InvalidKeyCharacter` error with position (`gauntlet md 3:3`), not a crash or a
  misclassification. Only a LEADING BOM triggers `ByteOrderMark`.
- Missing config: no GAUNTLET.md at all still yields the honest check-1 pass with
  "no GAUNTLET.md found" (`doctor/mod.rs:498-514` test + live probe).
- Flow mappings: distinct keys (`{ runner: seeded, model: m-1 }`) still parse;
  single-key documents still parse; rejection carries `line:col` (9:28 observed).
- Policy probe false positives: `irreversible-notes.json` (stem
  `irreversible-notes`), `irreversibles.json`, `irreversibl.json` all pass; the
  probe does not over-match prefixes. `Irreversible.JSON` flags.
- export-prefix: `export=plain` parses as a variable literally named `export`
  (unit-tested, `env_layer.rs:309-310`); `exported=1` cannot match the 7-char
  `"export "` prefix (code-verified: position 6 is `e`, not a space); neither
  perturbs the run (exit 0, no spurious unknowns).
- Comment stripping: glued `#` survives (`/tmp/…/frag#f` kept verbatim as the state
  dir — URL-style fragments intact); quoted `#` survives (`"/tmp/…/q#u"`); quoted
  value followed by a trailing comment is clean. Only whitespace-preceded `#` in
  unquoted values is a comment.
- Lowercase unknown reporting: `gauntlet_max_rounds` and a genuinely-unknown
  `GAUNTLET_TYPO_ROUNDZ` each appear exactly once in the unknown-environment
  clause — no double-report; proper-case recognized names apply and are never
  reported.
- Moved constants: `"none"` and the worktrees `true` exist only in defaults.rs;
  boundaries green (190/190).
- Usage surface (adjacent, `main.rs` in the diff): no-args / unknown-command /
  dangling `--config` all still exit 2.

Findings (observations, none introduced by this correction round):

1. **major — residual remainder of B-R2, not a new defect:** an explicitly
   requested `--config <missing path>` passes preflight as "no GAUNTLET.md found"
   with defaults and exit 0 on a healthy project (exact reproduction above). The
   adjudicated K2 text deliberately split only the error kinds; whether the
   explicit-override NotFound deserves its own treatment is a re-adjudication
   question, not a builder evasion.
2. **minor — pre-existing, unchanged by K4:** a mode-000 (unreadable) policy
   directory hides a present forbidden file: `read_dir` failure is treated as
   "clean" (`checks.rs:622-624`) and check 5's forbidden clause passes with exit 0
   (reproduced). Round 1's single-path `exists()` probe had the same hole
   (stat inside a non-searchable dir fails → pass), so this is a boundary
   condition carried forward, not a regression. Exploiting it requires the ability
   to make the policy dir non-readable to the operator running doctor.
3. **minor — corner of the adjudicated stem contract:** multi-suffix names
   (`irreversible.json.bak` — Rust file_stem is `irreversible.json`) and dotfiles
   (`.irreversible` — stem `.irreversible`) are not flagged. This matches the
   adjudicated contract exactly ("any stem spelling case-insensitively, any
   extension") and neither spelling is loadable as a policy file under any
   convention in this repo; recorded for completeness.

## Gates — what I ran

- `just doctor` → **exit 0**. All eight checks reported; check 1 pass (contract
  found and parsed at the repo GAUNTLET.md, no unknowns), checks 2/3/4 honestly
  unavailable with real task references, checks 5–8 pass; report written to
  `.agents/reports/doctor.json`.
- `cargo nextest run -p gauntlet-app` → **60/60 pass, 0 failed, 0 skipped**.
- `just boundaries` → **190/190 pass** (includes `no_hardcoded_literals` and the
  defaults-exemption warrant checks — relevant to A-R2/K9).
- `just ruler` → **PASS** — "answer key and manifest digests match the frozen
  lock"; additionally `git diff HEAD -- .wayfinder/ .gauntlet/` is empty (no ruler
  edit anywhere in the correction round).
- Surface: `git status --porcelain -- crates/gauntlet-ports/` → empty (ports
  untouched).

All live probes ran against `target/debug/gauntlet-cli` (built from the current
tree) with `env -i` and redirected `GAUNTLET_PROJECT_ROOT`/`GAUNTLET_REPORT_DIR`
under `/tmp/t5probe-c` (removed after the run).
