# Task 7 — FINISHER brief (round 1 recovery, dispatch 2): remaining deliverables

You are the FINISHER builder for plan Task 7 in `/home/sprime01/projects/gauntlet`.
Round-1 state: D1 (normalize), D2 (parse), D3 (manifest), D4 (provider) have landed.
A read-only reviewer verified D4 as COMPLETE-WITH-GAPS (`.agents/reports/task7/d4-verify.md`
— READ IT FIRST). Your job: remediate the D4 findings that are yours to fix, land
D5–D8, and write the final report. You do NOT commit (the orchestrator commits)
and you do NOT run cargo/just (see Hard rules).

## Read first (in this order)

1. `AGENTS.md` — rules that outrank this brief where they conflict (they won't).
2. `.agents/plans/gauntlet-impl.md` Task 7 section.
3. `.agents/reports/task7/builder-brief.md` — the ORIGINAL full brief; its
   deliverable sections D5–D8 (§5–§8) and the Gates/Teeth/Report-contract
   sections are the spec you are completing. Follow them exactly.
4. `.agents/reports/task7/d4-verify.md` — the D4 verification report (findings
   F1–F5 below refer to it).
5. `scripts/reference-digest.sh` — the frozen bootstrap digest tool whose rules
   the Rust normalize must reproduce byte-for-byte.
6. The current tree: `crates/gauntlet-app/src/reference/{mod,normalize,parse,manifest,provider}.rs`,
   `crates/gauntlet-app/src/doctor/checks.rs`, `crates/gauntlet-cli/src/main.rs`,
   `crates/gauntlet-cli/tests/export_stack.rs`, `crates/gauntlet-cli/Cargo.toml`,
   `.agents/reports/reference-candidates.md`.

## Current state (verified by the orchestrator — do not re-derive)

- `cargo check -p gauntlet-app` PASSES with the whole reference module wired in
  (orchestrator-run). `just ruler` PASS. Delegation is healthy again this session.
- D1–D3 landed and checkpointed; D4 landed but never checkpointed or gate-run.
- MISSING: D5 (CLI `reference digest`), D6 (`crates/gauntlet-cli/tests/p1_reference_integrity.rs`),
  D7 (doctor check-2 closure), D8 (reference-candidates entries), the final
  builder report, and the D4 remediation below.

## Hard rules

- **WRITE-ONLY SESSION: do NOT run any `cargo`, `rustc`, `just`, or build command.
  Do not run tests.** The host OOM-killed five delegated sessions that compiled
  (see ESC-002 in `.agents/reports/escalations.md`). Write code, checkpoint, write
  the report; the orchestrator runs every gate alone afterward with jobs=1 — that
  run is independent verification regardless. To sanity-check syntax, re-read your
  code; list anything you are unsure compiles in the report's Residuals section.
- **Never edit anything under `.wayfinder/`, never edit `.gauntlet/reference.lock`,
  never edit `scripts/reference-digest.sh`.** They are the ruler.
- No new external dependencies; hand-rolled strict parsing only (like Task 5).
- Arch suite must stay green: no absolute paths/hardcoded literals outside the
  defaults module; test temp dirs use the existing `Scratch` idiom.
- Typed errors carrying the failure locus. No `unwrap`/`expect` in production
  paths, no panics on untrusted input. edition-2024 idiom, `unsafe` forbidden.
- Do not weaken any existing assertion or test to make something pass.

## Do this, in order (checkpoint after EVERY item — same checkpoints file)

0. **FIRST ACTION after reading this brief:** append one line
   "`2026-08-18 finisher-2 session started; brief read`" to
   `.agents/reports/task7/builder-checkpoints.md` and nothing else. Then proceed.
1. **D4R — remediate reviewer findings** (from `d4-verify.md`):
   - **F3 (must fix):** add a test exercising the `verify_unchanged` Unavailable
     arms (artifact removed / made non-UTF-8 after freeze → `Unavailable`, not
     `ReferenceMutated`).
   - **F4 (close if natural):** in `freeze`, digest the bytes you actually read
     and, if they differ from the digest captured at resolve time, return the
     existing `ReferenceMutated` variant (it already carries expected+actual) —
     no new error variant, no port change. If you judge this over-engineering,
     instead record it as a residual with rationale; either way checkpoint the
     decision.
   - **F5 (align):** use the brief's Scratch idiom (pid + atomic counter) for any
     test temp naming you touch; converting the existing provider.rs test helper
     is in scope only if trivial.
   - **F1 (no code change):** the documented deviation stands — per-block parse
     failures are surfaced by D7's doctor deep sub-result (the compensating
     control), not through provider `normalize`. Do not restructure the provider.
   - **F2 (resolve via D6/D7):** after D6/D7 land, make the module header's
     strict-admission callsite citations true (exactly the callsites that then
     exist — no forward references).
2. **D5** — per builder-brief §5: `gauntlet reference digest --path <path>` in
   `crates/gauntlet-cli/src/main.rs` following the existing `doctor` command's
   arg-loop idiom; exact output `sha256:<lowercase hex>\n`; non-zero + stderr on
   error. Add `gauntlet-adapter-evidence-fs` to `gauntlet-cli` `[dev-dependencies]`.
3. **D6** — per builder-brief §6: `crates/gauntlet-cli/tests/p1_reference_integrity.rs`
   with the four named test fns (`frozen_copy_outside_workspace`,
   `builder_mutation_of_the_active_reference_aborts_with_reference_mutated`,
   `digest_rechecked` with a NAMED boundary-call helper so the tooth is surgical,
   `the_run_carries_the_frozen_version`). Real adapters only (FsEvidenceStore,
   EmbeddedStateStore); Scratch temp dirs; copy the frozen key into scratch as
   the active artifact — NEVER point anything at `.wayfinder/` itself. Include the
   location-rationale comment the brief requires.
4. **D7** — per builder-brief §7: close doctor check-2's two honest-unavailables
   (unit-schema deep sub-result = parse the configured answer key, report 70/70
   or per-block failures — this is F1's compensating control, make it real;
   digest-vs-lock = hand-rolled strict read of `.gauntlet/reference.lock`,
   unknown fields are findings, recompute normalized digest, mismatch = loud
   reference-class failure). Keep existing doctor tests green by encoding the
   now-available truth, never by weakening.
5. **D8** — per builder-brief §8: append the TWO dated entries to
   `.agents/reports/reference-candidates.md` in the existing entry format, each
   explicitly marked "additional instance of the existing locator-materialization
   class (first recorded 2026-08-17, AK-049)".
6. **Report** — write `.agents/reports/task7/builder-round1.md` per builder-brief
   §Report contract, covering the WHOLE round (D1–D8, including the parts earlier
   sessions landed — mark clearly which items you verified only by reading, not by
   running). Sections: Artifact, Decision log (include the F1 deviation, the F4
   decision, EvidenceKind::File, digest-identity semantics, artifact-source seam,
   manifest deferrals), Gate output (state honestly that gates were NOT run in
   delegated sessions; paste nothing fabricated), Teeth proposals (brief §Teeth:
   (a) boundary-call removal → `digest_rechecked` fails; (b) per-line-trim
   normalize → digest diff gate fails + which determinism test fires; (c) any
   other tooth you see), Residuals/limits (honest: AK-023/024 substrate-level
   until Tasks 12–13; anything you could not compile-check), Reference candidates
   (cross-checked against the file you appended to).

## Report contract

- Checkpoint after EVERY numbered item above (same file). Your final message may
  be lost — the files are the record.
- Completion means `builder-round1.md` exists and is complete. Keep the final
  message to a short pointer.

## Out of scope

Tasks 8+, controller iteration-boundary wiring, `.agents/CURRENT_STATUS.md`,
the spec, the plan, `.wayfinder/**`, `.gauntlet/reference.lock`, `scripts/*`,
`deny.toml`, the arch suite. If you believe an arch-suite change is needed, STOP
and record it as a blocker in your report.
