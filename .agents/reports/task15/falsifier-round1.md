# Task 15 — Falsifier round 1 (fresh blind EXECUTING round)

Date: 2026-08-29. Role: Falsifier (`.agents/agents/falsifier.agent.md` charter).
Scope: `crates/adapters/gauntlet-adapter-agent-prime/`, arch await-ban scan
(`crates/gauntlet-arch-tests/src/delegation.rs`), doctor check-3/4 runner wiring
(`crates/gauntlet-app/src/doctor/`, `crates/gauntlet-cli/src/main.rs`).
Blind round: no builder history read (no `.agents/reports/task15/` contents, no
preregistrations, no CURRENT_STATUS.yml). Ruler lines: AK-008..AK-012 (AK-013/P8
out of scope, Task 16).

Baseline hygiene (taken BEFORE any mutation):

- `~/.cache/gauntlet-falsifier/baseline.sha256`: 191 files under `crates/`.
- Git HEAD at start: `6f133a82` — frozen suite
  `crates/gauntlet-ports/src/contract/agent_runner.rs` verified byte-identical to
  HEAD via `git diff --exit-code` (PASS, no output).
- Start-state git status snapshot: `~/.cache/gauntlet-falsifier/git-status-start.txt`
  (Task 15 work present as uncommitted modifications + untracked files; those are
  the restore target, not strays of this round).

## 1. Default battery (exact results)

| Command | Result |
|---|---|
| `just test-package gauntlet-adapter-agent-prime` | 27 tests run: 27 passed, 0 skipped (live_probe test honestly skipped itself: no `GAUNTLET_PRIME_LIVE_PROBE`) |
| `just test-package gauntlet-app` | 350 tests run: 350 passed, 0 skipped |
| `just boundaries` | 220 tests run: 220 passed, 0 skipped |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean |
| `just ruler` | `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock` |

## 2. Executable attacks (probe: `~/.cache/gauntlet-falsifier/probe/`)

Standalone cargo probe (edition 2024) referencing the workspace by path; drives
the PUBLIC adapter API: `PrimeAgentRunner` over in-memory
`KernelTransport`/`HarnessControl` doubles and a virtual `Clock`. Final run:
**ALL 13 ATTACKS REPELLED** — no defect found by any attack below. Exact output:

```text
REPELLED | A1 late-writer post-terminal pin | TimedOut pinned through 3 late-write polls; collect refused; session_ref kept; stops=["sub-probe01"]
REPELLED | A2 deadline ownership | spec deadline is the only TimedOut trigger; earlier and later deadlines govern independently
REPELLED | A3 mailbox-ignored | listing status 'completed' + mailbox-shaped sibling artifact never complete; only the contract file counts
REPELLED | A4 liveness-ignored | dead child + pending contract + standing deadline stays Running (never Failed/TimedOut early)
REPELLED | A5 strict handle decode | 16 hostile handle documents refused by the strict parse; valid document accepted
REPELLED | A5b handle decode end-to-end | hostile handle document (answer-shaped unknown field) refused at admission
REPELLED | A6 contract-path leaf anchor | 7 traversal/nested/relative tokens refused with no directory side effects; clean leaf anchors under runs/<run>/inv/<inv>/
REPELLED | A7 duplicate admission refusal | duplicate admission refused while Completed and while Running; original entry and document untouched
REPELLED | A8 cancel semantics | cancel idempotent (1 stop, no second effect), strangers refused everywhere, failed stop retryable
REPELLED | A9 capabilities honesty | honest: isolation=None streaming=false delivery=File depth=1
REPELLED | A10 result classification | 5 malformed/mismatch shapes classify whole (malformed_result vs contract_mismatch), stay pinned, never collectible, and write no settlement state
REPELLED | A11 unreadable contract | directory-at-contract-path is a typed Transport error both before and after the deadline — uncertainty never becomes a verdict
REPELLED | A12 collect locus | collect before completion refuses with ResultNotReady{running}
```

Attack details, locators, and what each proves:

- **A1 late writer (AK-010, post-terminal pin)** — locator:
  `runner.rs` `poll` post-terminal pin (`entry.terminal` early return).
  Sequence: admit (deadline BASE+30) → advance virtual clock to +40 → poll ⇒
  `TimedOut` → NOW write a schema-valid, identity-matching document at the
  declared contract path → poll ×3 all `TimedOut`; `collect` ⇒
  `ResultNotReady{TimedOut}`; `session_ref` preserved; exactly one rule-5 stop.
  Attack repelled.
- **A2 deadline ownership (AK-010)** — 20 polls across +0..+19 s with deadline
  +30 all `Running` (no adapter-internal timeout fires); first poll past the
  spec deadline is `TimedOut`; two invocations with deadlines +30/+100 polled at
  +40 report (TimedOut, Running), the second times out only past +100. The
  spec's deadline, not an adapter constant, governs. Attack repelled.
- **A3 mailbox-ignored (§10.5, AK-009)** — `HarnessControl::list` reports the
  child `status: "completed"` AND a mailbox-shaped sibling artifact
  (`<leaf>.mailbox` containing an agent-message) exists, with NO result
  document: polls at +1/+5/+20 all `Running`. No non-contract signal can
  complete an invocation. Attack repelled. (Structural complement: the
  transport return type `AdmissionHandle` has exactly four string fields — no
  channel an answer could occupy.)
- **A4 liveness-ignored (Annex C.4)** — `list` ⇒ `[]` (no live child),
  contract pending, deadline standing: polls +1..+20 all `Running`, never
  `Failed`/`TimedOut`. Attack repelled.
- **A5 strict handle decode** — `AdmissionHandle::parse` (transport.rs:55)
  attacked with 16 hostiles: 1 unknown field, 4 missing-field, 2 empty-string,
  3 wrong-typed values (number/null/object), 6 wrong shapes (array, string,
  number, null, `{}`, non-JSON). All typed `HandleUnparseable` refusals; the
  valid document parses. Attack repelled (mechanism holds) — but see M2: no
  in-tree test pins this.
- **A5b end-to-end** — a transport double writes a handle document carrying an
  answer-shaped unknown field (`"answer":"...the result is 42"`); `invoke`
  refuses with `AgentError::AdmissionRefused`. Attack repelled.
- **A6 leaf anchor (RES-D3 guard)** — `invoke` with
  `ResultContractPath`-admitted hostile tokens `../outside.json`, `a/b.json`,
  `sub/dir/x.json`, `x/../y.json`, `..`, `.`, `./here.json`: every one is
  `AdmissionRefused` with NO `runs/` tree created; absolute `/etc/passwd` is
  refused by the domain type; a clean leaf anchors to
  `runs/<run>/inv/<inv>/<leaf>` with the envelope beside it. Attack repelled.
- **A7 duplicate admission** — re-`invoke` of a known id refused while the
  entry is `Completed` (original document still collected unchanged, terminal
  state intact) and while `Running`. Attack repelled.
- **A8 cancel semantics** — first cancel issues exactly ONE stop; second
  cancel is the typed no-op with NO second stop; poll stays `Cancelled`;
  stranger handle refused by poll/collect/cancel; a genuinely failed stop ⇒
  `CancelFailed` with the state UNpinned (poll still `Running`) and the retry
  cancel succeeds. Attack repelled.
- **A9 capabilities honesty (§10.5 trust row)** — runtime values:
  `isolation=None, supports_streaming_events=false, result_delivery=File,
  max_depth=configured(1)`. Static scan: the ONLY `RunnerCapabilities`
  construction in the adapter is the honest literal (`runner.rs:588`); no
  `Isolation::Process|Container|Vm` token exists anywhere under the adapter
  `src/`; `supports_streaming_events` appears exactly once, as `false`.
  Attack repelled.
- **A10 classification (AK-011/AK-012)** — not-JSON, unknown envelope field,
  unknown field nested in `self_checks[0]` ⇒ `Failed{malformed_result}`;
  foreign `invocation_id`, stale digest ⇒ `Failed{contract_mismatch}`; child
  declaring `state:"timed_out"` (a RUNTIME verdict) ⇒ refused as malformed.
  Each stays pinned across polls, is never collectible, and the runner writes
  nothing beyond the invocation's own files (no settlement-bearing state from
  a rejected document). Attack repelled.
- **A11 unreadable contract (§21.2)** — a DIRECTORY at the contract path ⇒
  `AgentError::Transport` (never a coerced Running/terminal), and it stays an
  error even past the deadline. Uncertainty never becomes a verdict. Observed
  honest behavior.
- **A12 collect locus** — collect before completion ⇒
  `ResultNotReady{Running}`. Attack repelled.

AK-008 (non-blocking invoke) is evidenced by the live probe's D1 forensics
(section 4) — in-memory doubles cannot prove non-blocking against a real
process.

## 3. Teeth mutations

Hand-targeted mutations of MECHANISMS (not tests), one compiler at a time; after
each: protecting-test evidence, byte-identical restore (sha256), GREEN
re-observation. Full-tree baseline:
`~/.cache/gauntlet-falsifier/baseline.sha256` (191 files). All mutations
REVERTED; end-of-round `crates/` sha256 diff vs baseline: **EMPTY**.

| # | Mechanism mutated (locator) | Mutation | Protecting evidence RED | In-tree battery under mutation | Restore sha256 == baseline | GREEN re-evidence |
|---|---|---|---|---|---|---|
| M1 | Post-terminal pin — `adapter/src/runner.rs` `poll` early-return of `entry.terminal` | pin block deleted | probe A1: late-written matching doc resurrected `TimedOut → Completed`; probe A8: post-cancel poll slid `Cancelled → Running` | **RED** — 26/27, frozen shared suite failed (via cancel-then-poll leg) | `37495c1a17a15c8f…` ✓ | battery 27/27; probe 13/13 REPELLED |
| M2 | Strict handle decode — `adapter/src/transport.rs` `AdmissionHandle::parse` unknown-field refusal | refusal replaced with silent ignore | probe A5: unknown-field doc ADMITTED; probe A5b: answer-shaped extra field admitted END-TO-END (`invoke` Ok) | **GREEN (27/27) — NO IN-TREE TEETH** (major finding F1) | `55f6411e76874aba…` ✓ | probe 13/13 REPELLED |
| M3 | TimedOut-at-deadline — `runner.rs` `poll` `entry.spec.deadline.exceeded_at(now)` | `if false && …` (never times out) | probe A2: past-deadline poll stayed `Running`; frozen suite `missing_result_is_timed_out` | **RED** — 26/27, frozen shared suite failed | `37495c1a17a15c8f…` ✓ | battery 27/27; probe 13/13 REPELLED |
| M4 | Leaf-anchor guard — `runner.rs` `invocation_dir` `components().count() == 1` | count check removed | probe A6: nested token `a/b.json` admitted and created the invocation tree; in-tree unit test | **RED** — `runner::tests::the_contract_token_is_a_leaf_inside_the_invocation_dir` FAILED (20/27) | `37495c1a17a15c8f…` ✓ | battery 27/27; probe 13/13 REPELLED |
| M5 | Arch await-ban scan — REAL violation planted in `transport.rs` `read_admission` (a `read_result_contract(` call), not a fixture | planted, then removed | arch scan fired with exact locator: `delegation-ban (rule A): …/transport.rs:201`; 4 negative controls still passed (4/5) | **RED** — `the_prime_adapter_derives_no_result_from_the_delegation_call` FAILED | `55f6411e76874aba…` ✓ | arch delegation tests 5/5 |
| M6 | Doctor failed-probe ⇒ Fail — `gauntlet-app/src/doctor/checks.rs` `(true, Some(d)) if !d.measured` arm | arm disabled (`if false && …`) so a failed probe falls to Pass | in-tree test `check_three_fails_a_failed_measured_probe_and_so_does_check_four` FAILED (188/350) | **RED** | `dda79a49405af8bb…` ✓ | app battery 350/350 |
| M7 | Cancel no-second-effect — `runner.rs` `cancel` `if entry.terminal.is_some()` typed no-op | no-op disabled: second cancel re-stops the child | probe A8: stop count 2 (`["sub-probe01","sub-probe01"]`) — a second control effect | **GREEN (27/27) — NO IN-TREE TEETH for the second effect** (major finding F2; the frozen suite accepts `Ok` on the second cancel and never counts stops) | `37495c1a17a15c8f…` ✓ | probe 13/13 REPELLED |

Teeth-map nuance from M1 (recorded, minor): the frozen suite caught pin REMOVAL,
but through the cancel-then-poll leg — no committed test drives the actual
late-writer resurrection shape (a document arriving after a terminal verdict)
against the prime adapter; the mock adapter has such a unit test
(`gauntlet-adapter-agent-mock/src/runner.rs` "the terminal entry must never
resurrect"), the prime adapter does not. The shape is covered by the external
probe only.

## 4. Live probe (authorized once)

```text
GAUNTLET_PRIME_LIVE_PROBE=1 cargo nextest run -p gauntlet-adapter-agent-prime \
    one_real_admission_completes_through_the_result_contract
    PASS [  40.369s] (1/1) gauntlet-adapter-agent-prime::live_probe
          one_real_admission_completes_through_the_result_contract
    Summary [  40.369s] 1 test run: 1 passed, 26 skipped
```

One real admission against the installed `prime-agent` harness completed
through the result contract in 40.4 s. The test internally asserts the D1
split evidence (forensic note contains `handle parsed while
parent.try_wait()=NotExited` and `reaper dispatched`), AK-008's non-blocking
invoke against a REAL process, file-derived `Completed`, matching `collect`,
and idempotent cancellation — it passed.

## 5. Hygiene proofs

- Frozen suite untouched and byte-identical to git HEAD:
  `git diff --exit-code -- crates/gauntlet-ports/src/contract/agent_runner.rs`
  → clean; sha256 `bd56991a…` matches baseline.
- End-of-round `crates/` sha256 re-cut (`after.sha256`) vs pre-attack baseline:
  **diff empty** (191 files).
- `git status --porcelain` end-state vs start-state snapshot: **diff empty** —
  no new strays, nothing lost (`.agents/reports/doctor.json` was already
  modified at start; rewritten by the mandated `just doctor` runs only).
- No transient files anywhere in the repo; all scratch (probe, docfix configs,
  hostile PATH shadow binary, sha256 manifests) lives under
  `~/.cache/gauntlet-falsifier/`. Nothing written to `.gauntlet/policies/`.
- `just doctor` on the tree: **exit 0** ("exit: OK"; check 3 runners honestly
  `unavailable` — no runner configured in the shipped config).
- `just ruler`: **PASS** (answer key and manifest digests match the frozen
  lock).
- Full `cargo-mutants` NOT run (Task 24's instrument). All mutations above are
  hand-targeted and reverted.
- No commits made.

### Doctor legs (end-to-end, via `--config` overrides into scratch, no shipped config edited)

- Real token named (`runners.default: prime-agent`): composition runs the real
  probe → check 3 `pass` ("runner 'prime-agent' resolves…"), exit OK.
- Mistokened config token (not fronted): honest `unavailable`, exit OK —
  correct posture for an unfronted token.
- Mistokened DESCRIPTOR (measured token ≠ resolved token): in-tree
  `check_three_fails_a_mistokened_descriptor` PASS ⇒ `Fail` +
  `runner_resolution_mismatch` (all four `check_three*` tests green).
- Broken harness binary (hostile PATH-shim printing `beep boop` to stderr):
  end-to-end `runner_probe_failed` ⇒ check 3 **fail**, doctor **exit 1**.

## 6. Findings

"Attack repelled" is not a finding. Findings from teeth mutations:

- **F1 (major, missing teeth):** the strict admission-handle decode
  (`AdmissionHandle::parse`) has no committed test. Mutation M2 — accepting
  unknown handle fields — passed the ENTIRE default battery (adapter package,
  app package, arch boundaries, lint) while admitting an answer-shaped extra
  field on the untrusted, model-written handle document end-to-end. The
  arch rule C pin guards the Rust struct's fields, not the parse; the frozen
  suite's doubles only ever write valid handle documents. The mechanism is
  correct today (A5/A5b repelled); only an external adversarial probe would
  catch its regression. Suggested repair (for the owner, not applied): a small
  in-tree unit test feeding hostile handle documents through
  `AdmissionHandle::parse`/`read_admission`.
- **F2 (major, missing teeth):** cancel's "never a second effect" guarantee is
  untested in-tree. Mutation M7 — a second cancel re-issuing the harness stop —
  passed the entire battery; no committed test counts control effects. Visible
  state semantics (Cancelled pinned, typed no-op allowed) are covered; the
  side effect is not.
- **F3 (minor, coverage nuance):** the late-writer resurrection shape (AK-010
  pressure: a valid matching document appearing AFTER `TimedOut`) is not driven
  by any committed test against the prime adapter (mock has one; the frozen
  suite's stability case cannot see it because its document never arrives
  late). Pin removal IS caught in-tree via a different leg (M1), so the
  invariant is not unprotected — the exact attack shape is.

No blocking findings. No contract divergence, invariant violation, or broken
gate found: all pinned semantics (AK-008..AK-012) held under every attack
executed, the live probe passed against the real harness, and the doctor gates
fire in the correct direction.

## 7. What was NOT attacked

- Task 16 territory: recovery/resume semantics, `orphaned_by_restart`
  classification, workspace resolution (explicitly R-1b), AK-013/P8.
- The rendered kernel program's real execution beyond the single live probe
  (partial handle writes, SIGTERM/grace/reaper races in
  `ProcessKernelTransport`, `parse_listing` against the real `list --json`):
  code-read + in-tree fixture tests + live-probe evidence only.
- Budget enforcement (runtime `BudgetMeter` side), observer/comparator paths,
  concurrency/thread-safety of `PrimeAgentRunner` (single-threaded posture
  asserted by suite comments, not proven here), `last_event_at` mtime
  correctness (only the fallback path was exercised).
- Full `cargo-mutants` run (Task 24's instrument; hand-targeted mutations only).
- Model-hint forwarding against the real harness; the live probe ran unhinted.
