# Task 16 — Builder B2 progress checkpoint (COR-R2-1 + COR-R2-2, tests-only)

Started: 2026-08-29, HEAD 8550aaa. Working tree clean under crates/.

## Baseline digests (pre-mutation)

- `crates/adapters/gauntlet-adapter-agent-cli/src/template.rs`
  sha256 `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
  (matches the falsifier's round-2 recorded digest `794d0618…`)
- `crates/gauntlet-arch-tests/src/delegation.rs`
  sha256 `8d366819e84477c0f3fd05f5b92c9092a6654a182a975da92d4f911e5dd22b49`

## Gates so far

- `just ruler` (start): PASS — "verify-ruler.sh: PASS — answer key and manifest
  digests match the frozen lock".

## Survey notes

- `probe_template` is re-exported at the crate root
  (`lib.rs`: `pub use template::{…, probe_template, …}`) — the new test uses
  `gauntlet_adapter_agent_cli::probe_template`.
- The cli crate's ONLY decode locus is `runner.rs:248` (`read_result_contract(`)
  inside `fn classify_contract` (runner.rs:245), a private classifier whose only
  callers are `fn poll`'s initial read (runner.rs:477) and its exit-triggered
  final re-read (runner.rs:501). `fn collect` serves the validated snapshot and
  never re-reads. So the cli sibling scan's allowed decoding-fn set is
  `poll`, `collect`, `classify_contract` — the last documented as the poll-path
  classifier. (Deviation from a literal poll/collect fn set is REQUIRED for the
  scan to be GREEN on the pristine tree; recorded in delegation.rs docs.)
- `scan_delegation_ban` (prime) is enforced by in-crate `#[cfg(test)]` tests in
  delegation.rs; `just boundaries` = `cargo nextest run -p gauntlet-arch-tests`
  (currently 224 tests). The cli sibling follows the same wiring: scan fn +
  repo-scan test + fixture negative controls, all inside delegation.rs. No
  lib.rs changes (out of the declared write surface).

## Leg status

- [x] Start gate `just ruler` PASS.
- [x] COR-R2-1: wrote `crates/adapters/gauntlet-adapter-agent-cli/tests/probe_template_honesty.rs`
      (3 legs: leg 1 = discriminating parse-ok/render-impossible ⇒ typed
      `EmptyProgram`; leg 2 = all-four-placeholders valid ⇒ Ok; leg 3 =
      unknown placeholder ⇒ typed `UnknownPlaceholder`).
- [x] COR-R2-1 RED→GREEN executed:
      - M1 mutation applied (probe body → `parse_template(source).map(|_| ())`);
        mutated digest `574114f811f00744890b40e658caa9db1d971398e1a0b187610ffed92fc63cb7`.
      - `just test-package gauntlet-adapter-agent-cli`: 56 run, 55 passed,
        1 failed — FAILED exactly `parse_ok_render_impossible_template_is_refused`
        (leg 1, panic: "…an Ok here means the probe has regressed to
        parse-only…"); legs 2/3 passed; ALL 53 pre-existing tests passed
        (independently re-proving F1: the mutation survives every shipped suite).
      - Restored from pristine scratch copy; sha256 re-verified
        `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
        (byte-identical); `git diff` on the crate src: empty.
      - GREEN: 56 run, 56 passed (3 new probe_template_honesty tests green).
- [x] COR-R2-2: cli decode-locus sibling scan landed in delegation.rs
      (`CLI_ADAPTER_SRC`, `CLI_DECODING_FNS` = poll/collect/classify_contract,
      `scan_cli_delegation_ban` with rules A+B, repo-scan test
      `the_generic_cli_adapter_derives_no_result_outside_the_poll_path`, and
      three negative controls: cli rule A fixture, cli rule B fixture
      (invoke hit + poll-path exemption clean), missing-cli-crate control).
      `just check-package gauntlet-arch-tests` clean;
      `just test-package gauntlet-arch-tests` 228/228 green (224 + 4 new).
- [x] COR-R2-2 RED→GREEN executed:
      - Plant: appended transient fn `b2_tooth_plant` to cli `template.rs`
        containing `read_result_contract(` at template.rs:514 (outside
        runner.rs poll path); planted digest
        `4c8fe3b6ece7587ba865288e97214b6395471d9d8cc547286ef16846eb225ff3`.
      - `just boundaries`: RED — exactly one failure,
        `the_generic_cli_adapter_derives_no_result_outside_the_poll_path`,
        with "delegation-ban (rule A): crates/adapters/gauntlet-adapter-agent-cli/
        src/template.rs:514: result-decoding call \"read_result_contract(\"
        outside runner.rs — only the runner's poll path may derive a result
        from the contract (COR-R2-2, spec 10.5)".
      - Restored from pristine scratch copy; sha256 re-verified
        `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
        (byte-identical); `git status --porcelain -- crates/adapters/` shows
        only the new tests/ file.
- [x] Battery (brief's order, verbatim results in b2-builder.md):
      1. `just test-package gauntlet-adapter-agent-cli` — 56/56 passed
      2. `just test-package gauntlet-arch-tests` — 228/228 passed
      3. `just boundaries` — 228/228 passed
      4. `just lint` — clippy -D warnings clean, `cargo fmt --check` clean
      5. `just ruler` — PASS (start AND end)

SLICE COMPLETE. Final report: `.agents/reports/task16/b2-builder.md`.
Scratch removed; one orphaned sleeper-shaped `sleep 121` killed in the
final sweep (attribution ambiguous — recorded in b2-builder.md Hygiene).

