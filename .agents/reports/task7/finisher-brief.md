# Task 7 — FINISHER brief (round 1 recovery): ReferenceProvider — remaining deliverables

You are the FINISHER builder for plan Task 7 in `/home/sprime01/projects/gauntlet`.
The round-1 builder's session died mid-slice. Its D1 (normalize) and D2 (parse)
landed and are checkpointed; D3 (manifest.rs, 1270 lines) exists and compiles but
is uncheckpointed and MUST be verified; D4–D8 are missing. Your job: finish, prove,
report. You do NOT commit (the orchestrator commits).

## Read first (in this order)

1. `AGENTS.md` — rules that outrank this brief where they conflict (they won't).
2. `.agents/plans/gauntlet-impl.md` Task 7 section.
3. **`.agents/reports/task7/builder-brief.md`** — the full original brief; every
   deliverable spec below is described there in detail. Follow it exactly.
4. `scripts/reference-digest.sh` — the frozen bootstrap digest tool and its
   documented normalization rules (byte-for-byte reproduction is the task gate).
5. `.agents/reports/task7/builder-brief.md` D1–D8 deliverable sections again —
   the D1/D2/D3 guidance is the spec you are completing, not just reading.
6. The current tree: `crates/gauntlet-app/src/reference/{mod.rs,normalize.rs,parse.rs,manifest.rs}`,
   `crates/gauntlet-app/src/doctor/checks.rs`, `crates/gauntlet-cli/src/main.rs`,
   `crates/gauntlet-cli/tests/export_stack.rs`, `.agents/reports/reference-candidates.md`.

## Current state (verified by the orchestrator — do not re-derive)

- `cargo check -p gauntlet-app` PASSES with the reference module wired in lib.rs.
- `just ruler` PASS.
- D1 `normalize.rs` (286 lines) and D2 `parse.rs` (930 lines) landed; their
  checkpoints claim in-crate tests green (`reference::` gate), which the orchestrator
  will re-verify.
- D3 `manifest.rs` (1270 lines) exists and compiles but was never checkpointed.
- MISSING: D4 `provider.rs` (FileReferenceProvider + contract-suite run), D5 CLI
  `reference digest` subcommand + evidence-fs dev-dep, D6 `crates/gauntlet-cli/tests/p1_reference_integrity.rs`,
  D7 doctor check-2 closure (`UNIT_SCHEMA_LATER`/`DIGEST_LATER`), D8 the two
  reference-candidates entries, and the final builder report.
- `crates/gauntlet-app/src/reference/mod.rs` currently declares only
  `manifest`, `normalize`, `parse` — D4 must add `provider`.

## Hard rules

- **Never edit anything under `.wayfinder/`, never edit `.gauntlet/reference.lock`,
  never edit `scripts/reference-digest.sh`.** They are the ruler.
- No new external dependencies; all parsing hand-rolled strict (like Task 5).
- Arch suite must stay green: no absolute paths/hardcoded literals outside the
  defaults module; test temp dirs use the existing `Scratch` idiom.
- Typed errors carrying the failure locus. No `unwrap`/`expect` in production paths,
  no panics on untrusted input. edition-2024 idiom, `unsafe` forbidden.
- **Memory discipline (operator instruction, non-negotiable): the host OOMs on
  parallel cargo work.** Before ANY `cargo` or `just` command, run:
  `source /tmp/opencode/mem-safeguard.env` (sets CARGO_BUILD_JOBS=2,
  NEXTEST_TEST_THREADS=1, NEXTEST_JOBS=1). ALWAYS append `-j 1` to `cargo nextest`
  invocations is NOT needed — the env vars handle it; do not add extra parallel
  shells. Only you compile; the orchestrator holds nothing concurrently.

## Do this, in order (checkpoint after EVERY item — same file)

0. **FIRST ACTION after reading this brief: append one line**
   "`<timestamp> finisher session started; brief read`" to
   `.agents/reports/task7/builder-checkpoints.md` and nothing else. This proves the
   session is alive. Then proceed. If you cannot write that line, stop and report.
1. **Verify D3 manifest.rs is coherent and complete** per builder-brief §3 (all
   documented deferrals present as decisions, all keys from the frozen
   `.wayfinder/reference.yaml` handled, its in-module tests green). Fix any gaps you
   find as defects, but do NOT invent mappings the brief explicitly defers. Append
   a checkpoint line for D3 whether or not you changed anything.
2. **D4** — `crates/gauntlet-app/src/reference/provider.rs`: `FileReferenceProvider`
   per builder-brief §4 (resolve → normalize → digest identity → freeze via
   `Rc<dyn EvidenceStore>` put+pin with `EvidenceKind::File` → `verify_unchanged`
   returning `ReferenceMutated{expected,actual}` on mismatch; broken artifact must be
   typed Unavailable, NOT ReferenceMutated). Artifact-source seam (path for production,
   shared `Rc<RefCell<String>>` for the contract suite). Run the contract suite via the
   `suite_runs.rs` idiom — it must pass. Wire `pub mod provider;` into mod.rs.
3. **D5** — `gauntlet reference digest --path <path>` in `crates/gauntlet-cli/src/main.rs`
   following the existing `doctor` command's arg-loop idiom; exact output
   `sha256:<lowercase hex>\n`; non-zero+stderr on error. Add `gauntlet-adapter-evidence-fs`
   to `gauntlet-cli` `[dev-dependencies]`. The `diff` gate against
   `scripts/reference-digest.sh` must produce byte-identical output.
4. **D6** — `crates/gauntlet-cli/tests/p1_reference_integrity.rs`: `frozen_copy_outside_workspace`
   (AK-022), `builder_mutation_of_the_active_reference_aborts_with_reference_mutated`
   (AK-023), `digest_rechecked` (AK-024, boundary-call-site named helper), and
   `the_run_carries_the_frozen_version`. Real adapters only; `Scratch` temp dirs;
   never point tests at `.wayfinder/` itself.
5. **D7** — close doctor check-2: unit-schema deep sub-result parses the configured
   answer key (70/70 or per-block failures); digest-vs-lock reads `.gauntlet/reference.lock`
   (hand-rolled, strict, unknown fields are findings), recomputes normalized
   answer-key digest, mismatch = check failure. Keep existing doctor tests green;
   update only where the previously-unavailable sub-results now resolve.
6. **D8** — append the two dated entries to `.agents/reports/reference-candidates.md`
   (AK-021 locator + AK-022/023/024 locators); each explicitly says "additional
   instance of the existing locator-materialization class (first recorded 2026-08-17,
   AK-049)". Follow the existing entry format.
7. Run these gates verbatim (with mem-safeguard sourced in the SAME shell as the
   cargo command): `cargo nextest run -p gauntlet-app reference::`,
   `cargo nextest run --test p1_reference_integrity`, the digest diff gate against
   `scripts/reference-digest.sh`, then `just check && just lint && just deny &&
   just boundaries && just ruler`. **Do NOT run `cargo nextest run --workspace` —
   that gate is the orchestrator's, run alone afterward with jobs=1.** Also run
   `cargo run -q -p gauntlet-cli -- reference digest --path .wayfinder/answer-key.md`
   and paste its output in the report.

   **WRITE-ONLY VARIANT (dispatch 3+): DO NOT RUN ANY cargo/just command in this
   session.** The host OOMs whenever a delegated session compiles — the harness
   itself is killed, taking the session with it (three sessions lost this way).
   Write all code (D1–D8), checkpoint after every item, write the final report,
   and let the orchestrator run every gate alone afterward, jobs=1. Your gates
   compress to: write files + checkpoints + report. The orchestrator's later gate
   run is independent verification regardless, so nothing is lost by this split.
   If a compile test would help you check syntax, do NOT run cargo; instead
   re-read your code for obvious breaks and say so in the report's residuals.
8. Write the final report `.agents/reports/task7/builder-round1.md` with: Artifact,
   Decision log (every deliberate choice + why, esp. manifest deferrals,
   EvidenceKind::File, digest-identity semantics, artifact-source seam), Gate output
   (verbatim tails), Teeth proposals (brief §"Teeth"), Residuals/limits (honest,
   specific: AK-023/024 substrate-level only), Reference candidates (cross-checked).

## Report contract

- Append a dated checkpoint line to `.agents/reports/task7/builder-checkpoints.md`
  after EVERY numbered item above. Your final message may be lost — the file is the
  record.
- Completion means `builder-round1.md` exists and is complete, all gates green.
  Keep the final message to a short pointer; do not paraphrase the report in it.

## Out of scope

Tasks 8+, controller iteration-boundary wiring, `.agents/CURRENT_STATUS.md`,
the spec, the plan, `.wayfinder/**`, `.gauntlet/reference.lock`, `scripts/*`,
`deny.toml`, the arch suite. If you believe an arch-suite change is needed, STOP
and record it as a blocker in your report.