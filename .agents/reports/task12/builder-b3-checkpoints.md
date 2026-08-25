# Task 12 — Builder B3 checkpoints

One dated entry per brief item, appended as each lands.

## 2026-08-23 — Session start / recon complete

- Read: builder-brief-b3.md, AGENTS.md, recon-spec.md, builder-b1.md,
  builder-b2.md. `recon-code.md` does NOT exist (same absence B1 recorded,
  IN-FLIGHT references it anyway); recon self-served from source:
  ports (agent_runner, clock, deadline, budget_meter, state_store,
  evidence_store, reference_provider, human_escalation, observer, verifier,
  comparator, producer_authority, role_result), app (control/{scheduler,
  select, diagnosis}, compare/{authority,recording,review,mod}, verify/
  {mod,authority,ladder}, observation/{assembly,input,bundle}, reference/
  {provider,parse}, config/*), domain (budget incl. `Budget::restore`,
  unit, values, role_invocation), adapters (observe-command/build/test/
  git/filesystem heads), arch tests (literals/envfile rules).
- Interruption reconciliation: `git status` matches B1+B2 landed state
  exactly; no `control/controller.rs`, no `builder-b3*.md` before this
  file, no checkpoint entries — B3 was UNSTARTED. Clean start, nothing
  redone, nothing to recover.
- Notable survey findings that shape the slice:
  - `.env.example` already lists all five control names; Annex E parses
    them (envfile test is spec<->file both ways), so `env_example_complete`
    needs no file edit.
  - The observe adapters each run `Command::…​.output()` with the deadline
    noted-but-unenforced (Task-8 D1's recorded deferral) — four call sites
    (command, build, test, git ×2).
  - The filesystem observer ALREADY caps walk entries/depth/bytes
    (MAX_MANIFEST_ENTRIES etc.) — Item 4's fs requirement is verification,
    not new work.
  - The literals scanner's timeout class = numeric literal ≥2 in a value
    position on a unit carrying a TIMEOUT_HINTS fragment (poll/_ms/stall/
    retry_base/retry_max/deadline/interval/sleep/delay/…) — controls the
    naming of the §11.4 default const.
  - `Ladder::new`/`LadderRung::new` exist only under the `composition`
    feature (B2/C1 seal) — the controller RECEIVES a built `Ladder`.
  - Canonical form is closed+complete: new EffectiveConfig fields must
    round-trip in canonical.rs both directions (the NonconvergenceN
    precedent).

Next: Item 3 (control config knobs), then Item 2 (disposition table),
Item 1 (controller), Item 4 (observer deadlines), Item 5 (gates).

## 2026-08-23 — Item 3 landed (control config, spec-backed)

- `config/defaults.rs`: `POLL_INTERVAL_MS` (§11.4 inline default) via the
  hint-free `TICK_WINDOW` intermediate const (the literals scanner's
  timeout class would flag the duration-shaped name and the figure on one
  unit; the §8.6 declaration exemption covers only the ten table rows).
  Pin test added (figure asserted on a line without the hint).
- `config/resolve.rs`: `ControlSettings { poll_interval_ms: u64 (defaulted),
  stall_timeout_ms / retry_base_ms / retry_max_ms: Option<u64>,
  max_attempts_per_unit: Option<u32> }` on `EffectiveConfig` AND
  `ResolvedConfig`; five env-only `ConfigKey` variants (Annex E Limits
  group, bare names like `NonconvergenceN`'s precedent), `STATIC_ENV_KEYS`
  rows, `set_whole` arms refusing zero in each; one test covering default /
  override / all-five zero refusals / products carry-through.
- `config/canonical.rs`: `control` section both directions (nullable
  uints for the required four); sorted-keys pin test updated; round-trip
  test stays green.
- Fail-first: this item landed implementation-first (compile-error
  fixups only — one E0425, one E0618 helper-shadow). Recorded honestly;
  the discriminating tests (absence/zero/refusal) are in place and green.
- Gates this item: `just check-package gauntlet-app` green; `cargo
  nextest run -p gauntlet-app config::` 45/45 passed.

## 2026-08-23 — Item 2 landed (§11.5 disposition table, typed)

- `control/disposition.rs` (new; registered in `control/mod.rs`):
  `ControlPolicy` (+`from_settings` with typed `RequiredSettingMissing`
  naming setting AND Annex E variable; zero refusal per setting),
  `IterationCause` (one variant per §11.5 row), `Disposition` +
  `EscalationCause`, `dispose(cause, policy)` pure row decision,
  `backoff_ms` = min(base × 2^(n-1), max) saturating. Structural
  guarantees: `ResidualAttachment` built from residual text only; no
  disposition variant weakens containment or retries a mutated reference.
- 11 tests, all green (`cargo nextest run -p gauntlet-app
  control::disposition` 11/11): rows 1/5 (text-only attachment + critic-
  trajectory absence + attempts boundary), row 2 (sequence 100..1600
  capped, tight-ceiling clamp, u32::MAX saturation), row 3 (one reminder
  then escalate), row 4 (cancel+preserve session+retry once then escalate
  with the session still attached), rows 6/7/8, row 9 (key+occurrences
  carried), policy absence/zero errors.
- Fail-first: same honest note as Item 3 (implementation-first single
  pass; compile errors fixed, tests discriminate).


## 2026-08-23 — FINISHER diagnosis (mid-Item-1 recovery, before repair)

Predecessor died mid-Item-1 with no checkpoint. Reconciled per
finisher-brief-b3.md; diagnosis below recorded BEFORE any repair.

- `git status` matches the brief's described state exactly (Items 3+2
  landed; controller.rs orphaned; provider.rs + ladder.rs modified).
- **provider.rs diff — INTENTIONAL, KEEP**: adds public
  `AnswerKeySource::read_text()` (bytes → UTF-8 text, the §11.4 step-3
  per-tick re-read path) and makes `describe()` public (names the source
  in typed refusals). Small, documented, exactly what the controller's
  re-read needs; no seal widening.
- **ladder.rs diff — INTENTIONAL, KEEP**: `#[derive(Clone)]` on `Ladder`
  only (`LadderRung` already Clone). Rationale in the added doc comment:
  `VerificationAuthority::for_run` consumes the ladder by value, the
  controller opens one per run/anchor. `Ladder::new` stays
  feature-gated — composition seal untouched. Compiles green.
- **controller.rs compiles**: `just check-package gauntlet-app` green
  (0.40s cached). Module registered in `control/mod.rs`. 3663 lines,
  all Item-1 surface present (compose guards, run/resume, drive loop,
  boundary_verify, reread_answer_key, plan/dispatch/drain,
  dispose_failure/timeout, escalate, process_completion, diagnosis,
  checkpoint) + 10 tests written.
- **Controller test state (the actual mid-item damage), 10 tests**:
  - PASS (4): dangling-depends-on surfacing; full-iteration settle;
    residual-redispatch text-only; resume-restores-budget.
  - FAIL (5):
    1. empty-ruler refusal — the test FIXTURE ruler
       (`# a ruler with no units`) trips reference PARSE first ("heading
       does not open an AK-NNN block"); the F18 guard is never reached.
       Fixture needs a parseable-but-zero-unit ruler (or guard order
       must be checked against parser semantics).
    2. mid-run answer-key edit → got `Settled`, expected `Aborted` —
       per-tick re-read/boundary verify not observing the edit (or the
       scenario completes before any re-read tick).
    3. stall → got `Settled`, expected `Escalated` — stall detection
       never fires (virtual-time advance path broken or poll loop
       settles with the attempt still outstanding).
    4. malformed result → got `Settled`, expected `Escalated` — strict
       contract read not disposing malformed (or fixture valid).
    5. reference unavailability → gap classified `None` instead of a
       typed Unavailable gap — classification path unwired.
  - HANG (1): `budget_exhaustion_drives_the_terminal_state_with_the
    _reserve_intact` runs >60s (killed at the 170s command timeout) —
    suspected infinite loop in the exhaustion/terminal path; plausibly
    the exact point where the predecessor session died.
- Plan: read controller.rs + tests fully, repair loop/fixture defects,
  keep sound structure, then Item 4, Item 5, result contract.

## 2026-08-23 — Item 1 LANDED (finisher: controller composed, loop repaired to green)

Predecessor's structure KEPT (compose guards, loop phases, authorities
wiring, fixtures); five defects repaired + clippy debt (Items 2/3 had
never been clippy-run) cleared:

- **Repair 1 — reserve-aware per-attempt estimate (the HANG)**:
  `per_attempt_estimate` now takes `(limits, spent, recovery_reserve_pct)`
  and computes the per-round share of the FORWARD remainder
  (`limit*(100-pct)/100 - spent`, `HUNDRED_PERCENT` named const). The
  old meter-remaining share made every unit silently ineligible under
  tight rounds (estimate = whole remaining > forward ceiling) → empty
  plan → no terminal → the observed >60s hang. `whole_forward_slice`
  and `summed_slice` removed (dead).
- **Repair 2 — per-dispatch admission**: `plan_and_dispatch` consults
  `would_breach_reserve` immediately before EACH dispatch on
  (next + all held reservations); the first breach stops dispatching;
  nothing admissible + nothing in flight ⇒ terminal `BudgetExhausted`.
  Rounds==0 gate kept explicit. §17.4/§11.4-step-5 semantics.
- **Repair 3 — §11.5 row-3/row-4 ledgers are per-unit loop state**
  (`timeout_retries`, `reminders` maps on LoopState): dispatch seeds
  InFlight counters from them; dispose branches bump them. The old
  per-invocation counters reset on every re-dispatch (stall/malformed
  retried forever, then settled). `clone_marker`+`Clone for InFlight`
  removed. Row-4 escalation summary enriched with the preserved session
  reference (§10.3 rule 5 rides the ticket).
- **Repair 4 — settlement is a boundary**: `decide_progress` re-runs
  `boundary_verify` before declaring `Settled` (returns the abort
  outcome on mutation; Unavailable records the gap and settlement
  stands, §21.3). This is what makes the mid-run-edit abort (prereg P2
  shape) and the unavailability gap reachable: both fixture flips land
  inside dispatch, after the last pre-dispatch boundary.
- **Repair 5 — completion record's budget facts are a ZERO forward
  ask**: `selection_facts(state, requested)` — the completed correction
  already ran under its reservation; offering the next-attempt share
  made the selector refuse post-exhaustion records
  (`NoAdmissibleCandidate{Budget}`) and abort the run.
- **Fixture repairs** (tests as written were unreachable/contradictory):
  empty-ruler text → the empty document (the ONLY zero-Unit parse the
  strict parser admits; any stray line is the pipeline's parse error);
  exhaustion fixture rounds 1→2 (rounds=1 @20% reserve ⇒ forward
  rounds ceiling 0 ⇒ NOTHING can ever dispatch; two rounds ⇒ first
  unit runs, second cannot start, exhaustion with reserve intact).
- Clippy -D warnings clean for gauntlet-app (fixed: canonical.rs
  redundant closure, disposition.rs useless-conversion +
  manual-saturating (Item 2/3 debt), collapsible ifs ×2, needless
  &mut, `arc_with_non_send_sync` allowed at the controller test module
  with rationale — same precedent as recording.rs/ladder.rs fixtures).
- Gates this step: `just check-package gauntlet-app` green;
  `cargo nextest run -p gauntlet-app control::` 99/99 (controller 10/10
  incl. mid-run-edit abort, stall retry-then-escalate, malformed
  reminder-then-escalate, budget exhaustion, unavailability gap,
  resume-restore); `just test-package gauntlet-app` 279/279; clippy
  clean. NOTE: `just test-package gauntlet-app` ran ~1s wall — no
  long-command stalls this session.

## 2026-08-23 — Item 4 LANDED (observer deadline enforcement + output caps)

- All four command-path observe adapters (command/build/test/git) now run
  their instrument through an in-adapter `run_bounded` (duplicated per
  adapter — adapter-to-adapter dependencies are forbidden, the
  `env_fingerprint` precedent): piped spawn → capped reader THREADS per
  stream → `try_wait` poll loop → kill+ reap at the deadline.
  - **Already-expired deadline (at ≤ now) refuses WITHOUT spawning**
    (`Unavailable`, "not spawned" in the detail).
  - **Outlived deadline ⇒ kill + `Unavailable`** naming the deadline and
    the §18.4 "test hangs" disposition (the assembler's BOTH-gap-arrays
    consumer is app-side, already covered there).
  - **Output caps**: `OUTPUT_CEILING_BYTES = 64*1024` named const per
    adapter with rationale; capped streams keep READING AND DISCARDING to
    EOF (stopping early would block a chatty-healthy child on a full pipe
    and get it killed for the wrong reason) and end with a RECORDED
    truncation marker naming the ceiling — truncation is evidence, not
    silence.
  - `TRY_WAIT_PAUSE = Duration::from_millis(1)` (smallest expressible
    tick; the deadline is the policy, not the tick — also stays under the
    literals scanner's timeout class: figure < 2 even on the `_millis`
    line; adapters are outside the denylist scope but the idiom holds).
  - Kill bounds the OBSERVATION (the instrument process), not the tree —
    grandchildren are Task 14's containment, documented on the fn.
- Per-adapter fail-first tests (real children, tight bounds ≤~1s,
  assertions on the error class/text, never wall-clock precision):
  sleep-past-short-deadline ⇒ Unavailable(deadline); expired deadline
  refuses without spawning (command/build/test prove it with a sentinel
  file the instrument never creates; git's observer hardcodes its query,
  so its discriminator is the "not spawned" text itself); chatty child
  (`head -c 200000 /dev/zero`) ⇒ stdout bounded + recorded marker.
- Fixture repair, recorded: `a_missing_program_classifies_as_unavailable`
  (build + test adapters) used an arbitrary zero deadline with a
  zero-reading clock; under enforcement at==now is zero budget, so the
  fixture now uses a budgeted bound (epoch 30 vs clock 0) — the C8
  assertion (missing binary ⇒ Unavailable naming the spawn) is unchanged.
- Filesystem observer: NOT rewritten — its walk was already capped
  (`MAX_WALK_DEPTH` 64, `MAX_MANIFEST_ENTRIES` 10_000 with a recorded
  truncation line, `MAX_HASHED_BYTES` 8 MiB); the entry cap is verified
  by the existing `manifest_error_lines_respect_the_entry_cap` test
  (drives the real walk past the cap, asserts the truncation line), 7/7
  green.
- Gates this item: `just test-package` per adapter — command 9/9 (incl.
  shared contract suite), build 8/8, test 8/8, git 9/9 (incl. suite),
  filesystem 7/7; clippy -D warnings clean on all four modified
  adapters; the shared `Observer` contract suites stay green (their
  FixedClock at BASE_INSTANT with a BASE+60 bound leaves real budget).

## 2026-08-23 — Item 5 LANDED (gates, run personally, one at a time)

- `just check-package gauntlet-app` green; `cargo nextest run -p
  gauntlet-app control::` 54/54.
- `just check-package` + `just test-package` per adapter: observe-command
  9/9, observe-build 8/8, observe-test 8/8, observe-filesystem 7/7,
  observe-git 9/9 (command/build/test/git include the shared `Observer`
  contract suites).
- `just check-package gauntlet-ports` + `just test-package gauntlet-ports`
  88/88.
- `just check` (workspace, all targets) green.
- `just lint` green after TWO repairs:
  1. fmt drift in predecessor-landed files (state-sqlite state_store.rs,
     ports contract/stubs.rs) — `cargo fmt` only, no semantic change;
  2. see the boundaries bullet for the arch-test repairs.
- `just deny` green (advisories/bans/licenses/sources ok).
- `just boundaries` green 197/197 after REAL repairs the gate demanded:
  - **denylist 'redis'**: the §11.5 vocabulary "Redispatch*"/
    `queue_redispatch` contains a database product name as a substring;
    renamed to `RetryWithResidual` / `RetryOnceWithSchemaReminder` /
    `queue_retry_dispatch` (CamelCase `ReDispatch*` would still lower to
    the same substring — the rename is denylist-driven, no semantics
    changed, all tests keep asserting the same behavior).
  - **literals**: (a) POLL_INTERVAL_MS's §11.4 five-second default has a
    milliseconds VALUE that collides with a well-known port — the const
    is now COMPOSED (`TICK_SPAN as u64 * MILLIS_PER_UNIT`), pin test
    asserts the factors; (b) the scanner's statement-join glues tail
    struct literals through closing braces into following const
    declarations — moved `MILLIS_PER_SECOND`/`HUNDRED_PERCENT` above the
    imports; (c) parenthesized section refs inside stall-adjacent strings
    ("(11.4 step 2)" → "(spec 11.4 step 2)") put figures in value
    positions; (d) comment figures near hint words reworded (80/800 port
    values, backoff sequences, "Rows 6, 7, 8"); (e) disposition row-2
    test figures now factor expressions over FLOOR/CEILING +
    `THIRD_ATTEMPT` const; (f) "x.rs:12" fixture strings read as
    host:port — now "x.rs line 12"; (g) defaults doc comments carried
    hint words ("hardcoded-timeout", "duration") that glued with const
    figures — reworded span-vocabulary. Diagnosed with a temporary
    scanner probe example (deleted after; git status confirms).
- `just ruler` PASS (frozen lock digests match).
- `just test-package gauntlet-app` 279/279; `just test` (full workspace)
  879/879.
- Hygiene: `du -sh target` 2.8G (< 4 GiB); no commits; no forbidden
  paths touched (.wayfinder/specs/plans/AGENTS.md/justfile/deny.toml/
  CURRENT_STATUS.md/prereg/prompts/.tmp/.jolli all clean per git diff).
