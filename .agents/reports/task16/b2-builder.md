# Task 16 — Builder B2 final report: COR-R2-1 + COR-R2-2 landed (tests-only)

Executed: 2026-08-29, HEAD 8550aaa. Brief: `.agents/` correction-round2
adjudication (COR-R2-1, COR-R2-2) + falsifier-round2 evidence (F1, F4,
mutation M1, leg L10b). Progress log: `b2-progress.md` (same directory).

**Status: both corrections landed, both teeth proven RED→GREEN with sha256
restoration proofs, full battery green, `just ruler` PASS at start and end.**

## Touch surface (exact)

Permanent writes:

1. `crates/adapters/gauntlet-adapter-agent-cli/tests/probe_template_honesty.rs`
   — NEW file (COR-R2-1): three tests pinning `probe_template`
   (`gauntlet_adapter_agent_cli::probe_template`, the lib.rs root re-export):
   - `parse_ok_render_impossible_template_is_refused` — the DISCRIMINATING
     leg: `'' {prompt_file}` (parses; argv[0] is the empty word) must be
     REFUSED with the typed render error `TemplateError::EmptyProgram`;
   - `valid_template_with_all_placeholders_probes_ok` — all four
     placeholders bound must probe `Ok` (no over-fail);
   - `unknown_placeholder_is_refused` — `agent {fabricated}` must refuse
     with the typed `UnknownPlaceholder { name: "fabricated" }`.
2. `crates/gauntlet-arch-tests/src/delegation.rs` — MODIFIED only (COR-R2-2):
   the prime scan and its controls untouched (no refactor); appended:
   - module-doc section "The cli sibling (COR-R2-2)";
   - `pub const CLI_ADAPTER_SRC` (`crates/adapters/gauntlet-adapter-agent-cli/src`);
   - `const CLI_DECODING_FNS: &[&str] = &["poll", "collect", "classify_contract"]`;
   - `pub fn scan_cli_delegation_ban(&BTreeMap<String, String>) -> Vec<String>`
     — same mechanics and failure style as the prime scan: rule A (decode
     calls `read_result_contract(` / `RoleResult::decode(` only in
     `runner.rs`), rule B (within `runner.rs`, only inside the poll path's
     fn bodies, fn-header tracked), missing-runner "must not pass silently"
     violation, sorted+deduped;
   - repo-scan test `the_generic_cli_adapter_derives_no_result_outside_the_poll_path`;
   - negative controls: `the_control_proves_the_cli_rule_a_has_teeth`
     (decode planted in a fixture template module → REPORTED),
     `the_control_proves_the_cli_rule_b_has_teeth` (decode in fixture
     `fn invoke` → REPORTED; fixture `poll`/`collect`/`classify_contract`
     → exemption holds), `the_control_proves_a_missing_cli_crate_cannot_pass_silently`.

Transient RED-evidence mutations (both restored byte-identical, sha256
verified; pristine copies staged under `/tmp/gauntlet-b2-scratch/`, restored
with `cp`, scratch removed at close):

- `crates/adapters/gauntlet-adapter-agent-cli/src/template.rs` — mutated
  twice (once per tooth), see below. NOT in the final touch surface:
  `git diff --name-only -- crates/` lists only delegation.rs; template.rs
  digest equals HEAD's.

Nothing else under `crates/`, no `Cargo.toml`, no docs, no other tests, no
`.gauntlet/**`, no `.agents/research/**`, no `CURRENT_STATUS.yml` touched.
(`.agents/reports/doctor.json` and `.jolli/*` diffs in `git status` predate
or sit outside this slice — not written by B2.)

## COR-R2-1 RED→GREEN (M1 mutation of `probe_template`)

- sha256 BEFORE (also = the falsifier's round-2 recorded `794d0618…`):
  `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
- Mutation applied (exact): `probe_template`'s body replaced with
  `parse_template(source).map(|_| ())` (parse-only gutting, round-2 M1).
- sha256 MUTATED: `574114f811f00744890b40e658caa9db1d971398e1a0b187610ffed92fc63cb7`
- RED: `just test-package gauntlet-adapter-agent-cli` →
  `Summary: 56 tests run: 55 passed, 1 failed` — the ONE failure is exactly
  the discriminating leg:
  `FAIL gauntlet-adapter-agent-cli::probe_template_honesty parse_ok_render_impossible_template_is_refused`
  panic at `tests/probe_template_honesty.rs:30` — "a template whose argv[0]
  is the empty word must be REFUSED by the probe — an Ok here means the
  probe has regressed to parse-only (round-2 mutation M1): ()".
  Legs 2 and 3 PASSED under the mutation, and ALL 53 pre-existing
  package tests PASSED — independently re-proving F1 (the mutation
  survives every shipped suite; only the new tooth catches it).
- Restore: pristine copy over the file; sha256 AFTER re-verified
  `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
  (byte-identical).
- GREEN: `just test-package gauntlet-adapter-agent-cli` →
  `56 tests run: 56 passed, 0 skipped` (includes the 3 new tests).

## COR-R2-2 RED→GREEN (decode-locus plant)

- Plant (exact): transient fn appended to cli `template.rs` (text-level
  plant; the cli crate was NOT compiled during the plant window — the scan
  reads sources as text):
  ```rust
  // B2 TRANSIENT TOOTH PLANT (COR-R2-2 RED evidence — never committed):
  // a result-decoding call outside the runner's poll path.
  pub fn b2_tooth_plant(source: &str) -> usize {
      let decoded = read_result_contract(source, &todo!(), &todo!());
      decoded.map_or(0, |_| 1)
  }
  ```
  planted `read_result_contract(` at `template.rs:514` (outside poll/collect).
- sha256 PLANTED: `4c8fe3b6ece7587ba865288e97214b6395471d9d8cc547286ef16846eb225ff3`
- RED: `just boundaries` → exactly one failure of 228:
  `FAIL gauntlet-arch-tests delegation::delegation_tests::the_generic_cli_adapter_derives_no_result_outside_the_poll_path`
  with the violation naming file:line:
  `delegation-ban (rule A): crates/adapters/gauntlet-adapter-agent-cli/src/template.rs:514: result-decoding call "read_result_contract(" outside runner.rs — only the runner's poll path may derive a result from the contract (COR-R2-2, spec 10.5)`
  (The prime scan's test and the cli vendor-ban scan stayed green.)
- Restore: pristine copy over the file; sha256 AFTER re-verified
  `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
  (byte-identical). `git status --porcelain -- crates/adapters/` shows only
  the new `tests/probe_template_honesty.rs`.
- GREEN: `just boundaries` → `228 tests run: 228 passed, 0 skipped`.

## Design note (recorded, not a deviation from the brief's intent)

The cli sibling's decoding-fn set is `poll`, `collect`, `classify_contract`
(`CLI_DECODING_FNS`), not a literal poll/collect pair: the cli runner's one
decode locus is `fn classify_contract` (runner.rs:245, called only from
`poll`'s initial read at runner.rs:477 and its exit-triggered final re-read
at runner.rs:501), so a literal set would be RED on the pristine tree.
`classify_contract` is documented in delegation.rs as the poll-path
classifier; a decode call in ANY other fn (`invoke`, `cancel`, `stop_child`,
`forensics`, …) fails rule B, and any decode outside `runner.rs` fails rule
A — which is exactly the F4 exclusion demanded. A fixture control pins the
exemption AND the invoke-hit in one test.

## Verification battery (verbatim, run in the brief's order)

1. `just test-package gauntlet-adapter-agent-cli`
   `Summary [ 4.660s] 56 tests run: 56 passed, 0 skipped`
2. `just test-package gauntlet-arch-tests`
   `Summary [16.007s] 228 tests run: 228 passed, 0 skipped` (224 prior + 4 new)
3. `just boundaries`
   `Summary [16.012s] 228 tests run: 228 passed, 0 skipped`
4. `just lint`
   clippy `--workspace --all-targets -- -D warnings`: Finished, zero warnings;
   `cargo fmt --check`: clean (no output, exit 0)
5. `just ruler`
   `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)`
   (PASS at start AND at end.)

## Hygiene

- No probe children spawned by B2 directly; no live-probe children (both
  teeth are static/text-level). Final sweeps `pgrep -af` for sleeper-shaped
  children: one orphaned `sleep 121` (pid 1868328, reparented to `/init`,
  started 22:44:55 — inside the battery window; `sleep 121` matches the
  round-1 live battery's sleeper shape) was found. Attribution ambiguous:
  all 56 cli package tests passed INCLUDING their stop postconditions, so a
  test leak is not established — it may equally be another session's child.
  Killed it anyway (harmless sleeper) and confirmed gone; second sweep
  clean. Flagging for the orchestrator only as an observation, not a
  finding.
- Scratch `/tmp/gauntlet-b2-scratch/` removed (0 `/tmp/gauntlet-b2-*`
  remain). No other scratch created.
- Firewalled files not opened: no `b1-builder.md`/`b1-progress.md`,
  `critic-design.md`, `design-adjudication.md`, `IN-FLIGHT.md` (contents),
  preregistrations, `CURRENT_STATUS.yml`, `.wayfinder/answer-key.md`,
  `.gauntlet/**`.

## Deviations

None from the brief's slices. The two recorded notes: (a) the
`classify_contract` fn-set inclusion above (required for pristine-GREEN,
documented in-code); (b) the ambiguous orphaned sleeper, killed and
recorded under Hygiene. No scope widened, no shipped `src/` behavior
changed (template.rs and delegation.rs-adjacent sources digest-proven
untouched at close; delegation.rs gained tests-only machinery plus public
constants/fn used only by tests).
