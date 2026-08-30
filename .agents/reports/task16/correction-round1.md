# Task 16 — correction round 1 (orchestrator-executed)

**Context:** falsifier round 1 returned NOT_CONFIRM with findings F1 (major),
F2 (major), F3 (minor), F4 (major) — see `falsifier-round1.md`. The
falsifier agent died on the sub-agent usage window mid-round; the round was
completed and these corrections were implemented PERSONALLY by the
orchestrator under the operator's standing heal+continue rule (sub-agent
dispatch unavailable until 2026-08-30T12:39; recorded honestly —
orchestrator implementation is the recorded exception class, not a
settlement authority).

**Method:** tests-first. The falsifier's own finding-probes were flipped to
assert the corrected behavior and executed RED on the pre-correction tree
(exact run: 32 tests, 29 passed, 3 failed — `a4b_comment_only_template_is_a_typed_parse_refusal`,
`c_sigterm_ignoring_child_is_escalated_within_bound`,
`c_hang_after_write_completes_and_the_child_is_retired`), then the
corrections landed, then the same run returned 32/32 GREEN. The RED→GREEN
direction is the corrections' teeth: each corrected test fails on exactly
the defect class it pins.

## COR-F1 — a landed SIGKILL with a pending reap is a stopped child, not a failed cancel

- **Finding (F1, major):** `transport.rs`'s escalation leg waited for the
  graceful SIGTERM leg to consume the ENTIRE caller bound, then SIGKILLed
  and immediately required the reap within the already-expired bound —
  returning `CancelFailed{"not reaped within the bound"}` for a kill that
  had LANDED. Consequence: the runner left the invocation unpinned "for
  retry", and a retry-or-poll would classify the dead child as
  `Failed{missing_result}` — a deliberately cancelled run re-classified as
  a retryable transient agent failure (which the controller's row-2 arm
  would then re-dispatch).
- **Correction:** SIGKILL is uncatchable — once it lands the child IS
  stopped and only the reap observation is pending. The escalation leg now
  gives the reap a DEDICATED grace (`REAP_GRACE_SECS = 2`, measured from
  the start of the escalation leg, not the caller's bound) and expires as
  `Ok(None)` — "stopped; exit code unobserved; a later state probe reaps"
  — never `StopFailed`. `StopFailed`'s honest scope is a failed kill or a
  broken wait. `Ok(None)` flows through the runner's existing
  `stop_child` (`Ok(_)`) arm and pins `Cancelled`. Trait and module docs
  updated to match the type's actual meaning.
- **Locus:** `crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs`
  (escalation loop, `StopFailed` doc, `stop` trait doc, module doc).
- **Teeth:** `c_sigterm_ignoring_child_is_escalated_within_bound` RED on
  pre-correction code (the exact CancelFailed observed), GREEN after
  (cancel succeeds inside the bound; pid verified dead; pinned Cancelled).

## COR-F2 — a zero-word template is a typed parse refusal, and the doctor probe renders

- **Finding (F2, major):** a comment-only template parsed successfully
  (zero words), so doctor's configuration probe reported a USABLE runner
  that every admission would refuse (`EmptyProgram`); `'' {prompt_file}`
  shared the parse-clean-render-refuse class. False "measured clean" in
  §8.5's truth-telling surface.
- **Correction (two halves):**
  1. `parse_template` refuses a zero-word result as `TemplateError::Empty`
     — a comment-only source is no program, morally identical to a
     whitespace-only one.
  2. New `probe_template(source)` (exported): parse AND render against
     every declared placeholder with synthetic values. The doctor's cli
     leg (`measure_runner`) now probes through `probe_template`, so ANY
     render-level refusal (empty program, missing value) surfaces at
     doctor time as `measured: false` + honest note. Closing the whole
     class, not the single instance.
- **Locus:** `template.rs` (parse tail + `probe_template`), `lib.rs`
  (export), `crates/gauntlet-cli/src/main.rs` (doctor cli leg).
- **Teeth:** `a4b_comment_only_template_is_a_typed_parse_refusal` RED
  pre-correction; after: doctor executed end-to-end — comment-only and
  `'' {prompt_file}` FAIL check 3 honestly, a good template passes with
  the Configuration wording ("no binary probed and no version claimed").

## COR-F3 — lib.rs doc corrected to the actual (correct) collect behavior

- **Finding (F3, minor):** lib.rs claimed the result contract is read "in
  exactly two places — poll and collect". Executed observation (the F
  leg): `collect` serves the VALIDATED snapshot `poll` classified and
  never re-reads the file — post-completion tampering cannot change what
  the runtime collected. The BEHAVIOR is right (snapshot
  tamper-resistance); the DOC was wrong.
- **Correction:** lib.rs module doc now states the real shape: read at
  poll (initial + the exit-triggered final re-read); collect serves the
  validated snapshot, never the file.
- **Locus:** `crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs`.

## COR-F4 — no invocation leaves a live child behind

- **Finding (F4, major):** a terminal pin via the contract read
  (Completed, malformed, mismatch) left a LIVE child running forever; the
  child retains workspace authority after the verdict is fixed and cancel
  cannot reach it (`AlreadyTerminal`). Executed: the hang-after-write
  child outlived its Completed invocation.
- **Correction:** best-effort retirement at every terminal pin: a new
  `retire_child` (bounded by a dedicated short `RETIRE_SPAN_SECONDS = 5`,
  distinct from a cancellation's configured span) runs before pinning a
  terminal reached through the contract read (both read sites) — the
  TimedOut path already stopped the child itself. The stop's outcome is
  NEVER a state input: a failure surfaces at most as a control note and
  the verdict stands (post-terminal stability — B8's late-writer leg —
  is unaffected). Already-exited children make the retirement a no-op
  (kill-after-exit success), preserving B1's pinned semantics (process
  state never consulted on the contract-wins path; forensics untouched).
- **Locus:** `crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs`
  (`retire_child`, `fresh_retire_bound`, `RETIRE_SPAN_SECONDS`, two pin
  call sites).
- **Teeth:** `c_hang_after_write_completes_and_the_child_is_retired` RED
  pre-correction (leak observed); GREEN after (child retired within the
  bound; verdict stays Completed; stability holds).

## Verification after the corrections

- cli package: 53/53 (includes the 32 retained round-1 attack teeth).
- Affected packages (cli adapters + mock + prime + app): 507/507; proofs
  spot-run: 17/17; p8 gate line PASS (plus fail-closed and single-adapter
  selection legs).
- Full standing battery re-run on the corrected tree: workspace
  1182/1182, lint clean (after fmt on the retained test file), boundaries
  224/224, deny ok, doctor exit OK, ruler PASS, prereg-check 16 PASS
  (frozen hash untouched — corrections are code/tests only; the prereg's
  predictions are adjudicated in the closure record, not edited).
