# Task 16 — falsifier round 2 (EXECUTING, fresh)

**VERDICT: CONFIRM**   (rationale and findings below; no BLOCKING and no in-mechanism
MAJOR defect stands after the executed attacks — F1 is MAJOR-with-COR teeth debt)

## Round provenance

Fresh blind executing falsifier, round 2. HEAD at start: 8550aaa (later than a1c15a2;
commit message states round-1 corrections COR-F1..F4 landed). I composed my own attacks
from the adapter source, the shared ports suite, the P8 proof, the doctor leg, and the
arch teeth, anchored on Core-spec §7.3/§10.3/§17.3/§18.3/Annex B and answer-key
AK-013/AK-014. I have NOT read and will not read any firewalled task16 report file,
any preregistration, or `.agents/CURRENT_STATUS.yml`. Round-1's NOT-ATTACKED list was
given to me as a warm-start hint; I independently re-derive every attack.

## Plan (live, updated as legs complete)

Legs (each gets a row in the outcomes table; findings appended below):

- L0  Hygiene baseline: `just ruler` PASS; inventory of tree state.
- L1  Source survey of `crates/adapters/gauntlet-adapter-agent-cli` (template, transport,
      runner, capabilities, config, prompt, clock, testing, lib docs).
- L2  Contract-suite shape-2 attack: clean exit + no result doc ⇒ `Failed{missing_result}`
      decided before deadline; same suite unmodified against mock + cli factories.
- L3  Template grammar / argv rendering / quoting / placeholder-class attacks.
- L4  Process lifecycle: exit codes, signals, SIGTERM-ignoring child, hang, orphan/reap,
      deadline boundary semantics.
- L5  Contract-file races: late writer, tamper after verdict, directory at result path,
      wrong-path exit, malformed JSON, envelope-body sibling channel.
- L6  Verdict classification + stability (terminal pins), cancel lifecycle (double cancel,
      stranger/forged handles).
- L7  Capabilities honesty vs actual transport behavior.
- L8  P8 fail-closed selection: bogus `GAUNTLET_P8_ADAPTER` value must FAIL LOUDLY;
      mock|cli|both selection; zero test-body changes claim.
- L9  Doctor cli-runner leg honesty (configuration probe over command template).
- L10 Arch teeth: vendor-name ban, await-ban scan — evasion attempts + teeth proof.
- L11 Targeted cheap mutation checks (single mechanism, not a full run).
- L12 Live probe leg (bounded, authorized): one trivial real child through the real spawn
      path under `GAUNTLET_CLI_AGENT_LIVE_PROBE=1`; my own trivial probe children only.
- L13 Hygiene close-out: stray sweep, scratch removal, planted-file restore proof,
      `just ruler` PASS again.

## Executed attacks and outcomes

Checkpoint 1 (survey complete, attacks not yet executed). Source-survey notes feeding
the attack design:

- `runner.rs` poll total order: contract read (strict codec + identity) → exited child
  (+ final re-read) ⇒ `Failed{missing_result}` → deadline ⇒ `TimedOut` → `Running`.
  A NON-NotFound contract read error (e.g. a DIRECTORY planted at the contract path)
  returns `AgentError::Transport` and is never terminal — attack candidate A (does a
  hostile child wedge the deadline verdict by poisoning the contract path?).
- `lib.rs` claims an executed post-completion tamper leg — I re-execute it myself
  (attack B): tamper the contract file after `Completed`, poll + collect must serve the
  validated snapshot, never the tampered text.
- `transport.rs` stop: SIGTERM via fixed-argv `sh -c 'kill -TERM "$1"' sh <pid>`, bounded
  graceful wait vs an absolute epoch-second bound, then SIGKILL + 2 s reap grace
  (`Ok(None)` = stopped, code unobserved). Attacks: SIGTERM-ignoring child under a short
  caller bound (C1); external signal-death collapse to `Failed{missing_result}` with
  forensics `Some(None)` (C2).
- `template.rs`: quotes never hide placeholders (documented); consequence — the grammar
  cannot express the literal argv word `{model}` (documented design, to be verified);
  UnterminatedQuote offsets are CHAR indices reported as "byte N" (candidate MINOR).
- P8 `selected_factories`: bogus `GAUNTLET_P8_ADAPTER` panics — to be executed, not
  just read (L8).
- Doctor leg to inspect: which probe does it use — `parsed_template` (parse only) or
  `probe_template` (parse + render)? A parse-only doctor would report OK for a template
  that can never render (e.g. `'' {prompt_file}` ⇒ `EmptyProgram` at every admission) —
  clean-here-refused-at-admission, exactly what COR-F2 forbids (candidate MAJOR, L9).

| Id | Attack | Outcome |
|---|---|---|
| L0 | `just ruler` at round start | REPELLED — PASS (answer key + manifest digests match the frozen lock) |
| L2 | `just test-package gauntlet-adapter-agent-cli` (53 tests) and `gauntlet-ports` (112 tests) baselines | REPELLED — all green, incl. the in-tree round-1 falsifier battery and the shape-2 suites |
| L8a | `GAUNTLET_P8_ADAPTER=bogus` → P8 must fail loudly | REPELLED — FAIL with the typed message "is unknown — expected mock \| cli \| both; an unknown selection must fail loudly, never narrow the battery silently" (0 passed, 1 failed) |
| L8b | `GAUNTLET_P8_ADAPTER=mock` | REPELLED — PASS (single-factory debug leg) |
| L8c | `GAUNTLET_P8_ADAPTER=cli` | REPELLED — PASS (single-factory debug leg) |
| L8d | default (unset = both) | REPELLED — PASS; the two-factory + non-mock assertions bind |
| L8e | `GAUNTLET_P8_ADAPTER=BOTH` (case trick) | REPELLED — FAIL loudly; selection is exact-match, no case-insensitive silent widening |

Survey-sourced doctor finding: the composition root (`crates/gauntlet-cli/src/main.rs`
`measure_runner`, cli token leg) calls `generic_cli_runner::probe_template` — the
parse-AND-render probe — so the doctor's Configuration leg is honest by construction;
the earlier clean-here candidate died at inspection. BUT `probe_template` has ZERO
in-tree test callers (checked across all crates) — teeth finding F1 below, proven by
mutation in L11.

| L8e | `GAUNTLET_P8_ADAPTER=BOTH` (case trick) | REPELLED — FAIL loudly; selection is exact-match, no case-insensitive silent widening |
| L9a | `just doctor` current tree | REPELLED — exit OK; runners leg honest `UNK-003` open-unknown (no runner configured on this host) |
| L9b | `GAUNTLET_AGENT_RUNNER=cli-agent`, template unset | REPELLED — check 3 FAILS honestly: "GAUNTLET_CLI_AGENT_CMD is not set — the generic CLI runner is driven entirely by its configured command template" |
| L9c | template `'' {prompt_file}` (parses, can NEVER render) | REPELLED — check 3 FAILS with "argv[0] is empty after substitution — there is no program to run": the doctor does NOT report clean-here-refused-at-admission (COR-F2 honesty proven by execution) |
| L9d | template `agent {fabricated}` | REPELLED — check 3 FAILS with the typed vocabulary locus |
| L9e | valid template `agent --prompt … --out … --dir …` | REPELLED — doctor exit OK (the probe does not over-fail) |
| L3a | scratch r2_probe_template_shape_check (parse-ok/render-impossible, unknown, comment-only, synthetic model) | REPELLED — all shapes behave as documented (NOTE: no PERMANENT in-tree test covers probe_template — finding F1) |
| L3b | scratch r2_placeholder_binding_is_inescapable_in_every_quote_mode (`{model}`, `'{model}'`, `"{model}"`, `\{model}` all bind; `\{model}x` refused) | REPELLED — documented design confirmed; no grammar path yields a literal `{model}` word |
| L3c | scratch r2_unterminated_quote_offset (multibyte source `ää'`) | BROKE (minor) — the recorded offset 3 is a CHAR index while the Display text says "at byte 3"; the true byte position is 4 — finding F3 (MINOR, locus-label honesty only) |
| L5a | scratch r2_deadline_wedge: hostile child plants a DIRECTORY at the contract path and stays alive, spec deadline 2 s | BROKE (liveness, deliberate-posture cost) — poll returns the typed Transport error forever, the `TimedOut` verdict is UNREACHABLE past the deadline, and the hostile child was still ALIVE until an explicit `cancel` retired it (cancel then worked and pinned `Cancelled`) — finding F2 |
| L5b | scratch r2_sigkill_death: child `kill -9 $$` (external signal death) | REPELLED — `Failed{missing_result}` on the observed exit, forensics `exit_status = Some(None)` recorded, never obeyed |
| L5c | scratch r2_valid_foreign_tamper: after `Completed`, overwrite the contract with a fully VALID impostor document (foreign id + foreign digest), then poll | REPELLED — verdict stays `Completed` on every poll (no mismatch flip), `collect` serves the ORIGINAL validated snapshot (id = ID_A), the impostor never leaks |
| L4/L6 | cancel lifecycle, double cancel, stranger handles, SIGTERM-ignoring escalation, hang-after-write retirement, two-invocation cross-wiring, deadline boundary exactness, wrong-path exit, orphan handle file | REPELLED (already pinned in-tree by the round-1 battery — `falsifier_round1_live_attacks.rs` — re-verified green in L2; I read those pins and did not duplicate them) |

| L11a | M1: `probe_template` gutted to parse-only (digests recorded) | BROKE (proves F1) — my scratch shape-check went RED (`Ok(())` vs `Err(EmptyProgram)`), but ALL 58 in-tree adapter tests and ALL 62 gauntlet-cli tests (incl. P8) stayed GREEN: the mutation survives every shipped suite. Restored byte-identical (sha256 `794d0618…` re-verified) |
| L11b | M2: shape-2 verdict removed from `runner.rs` poll (exited child falls through to the deadline) | REPELLED (teeth proven) — P8 FAILS immediately (the shared `exit_without_result_suite` catches it) AND the round-1 live battery FAILS (b4/b8/d legs). Restored from pristine copy byte-identical (sha256 `421cb654…` re-verified), P8 and the 18 live tests GREEN again |
| L10a | vendor-ban teeth: "claude anthropic openai gemini" planted in the cli crate's `lib.rs` | REPELLED (teeth proven) — `just boundaries` FAILS with the typed violation `denied-cli-adapter-identifier: …/src/lib.rs:68: 'claude' — … selection is configuration, not code`. Restored byte-identical (sha256 `1df6737d…` re-verified); all 224 arch tests GREEN |
| L10b | await/decode-locus ban scope inspection | BROKE (minor, teeth gap) — the delegation await-ban (`gauntlet-arch-tests/src/delegation.rs`, decode-call loci + poll/collect exclusivity) scopes ONLY `crates/adapters/gauntlet-adapter-agent-prime/src`; the cli crate is scanned for vendor terms (`check_generic_cli_adapter_sources`) but NOTHING enforces that `read_result_contract(`/`RoleResult::decode(` stay inside the cli runner's `poll`/`collect` — finding F4. Currently correct by inspection (runner.rs `classify_contract` is the only decode locus) |
| L12 | authorized live probe: `GAUNTLET_CLI_AGENT_LIVE_PROBE=1` — ONE real child through the REAL `ProcessChildTransport` spawn path (sh + sed parsing the real prompt file, writing the real contract) | REPELLED — PASS in 0.255 s; Completed via the file, collect matches, cancel idempotent, stranger UnknownHandle; scratch roots removed by the test itself |

## Findings

### F1 — MAJOR-with-COR (missing teeth for a correct mechanism)
`probe_template` (`crates/adapters/gauntlet-adapter-agent-cli/src/template.rs:293`) —
the doctor's parse-AND-render honesty probe, the mechanism COR-F2 relies on to keep the
doctor from reporting clean-here-refused-at-admission — has ZERO in-tree test callers
(verified: only definition, re-export, and the composition-root call in
`crates/gauntlet-cli/src/main.rs:155`). The doctor leg is honest by execution today
(L9c), but nothing fails if the probe regresses to parse-only. Proven by mutation M1:
gutting `probe_template` to `parse_template(source).map(|_| ())` leaves the ENTIRE
adapter package battery AND the P8 substitution proof GREEN (mutation survives; see
Teeth). The correct mechanism is unguarded.

### F2 — MINOR (deliberate-posture liveness cost; deadline verdict unreachable under contract-path poisoning)
A hostile child can plant a directory (or otherwise unwritable/unreadable file) at its
own `GAUNTLET_RESULT_PATH` and stay alive: `classify_contract`
(`runner.rs:267-277`) maps non-NotFound read errors to `AgentError::Transport`, poll
never reaches the step-3 deadline verdict (`runner.rs:521-527`), so `TimedOut` — and
with it the runner's own stop of the child — is unreachable for as long as the poison
sits there. Executed evidence: L5a (child ALIVE past its 2 s deadline, polls erroring,
no pin; explicit cancel worked and reaped). Graded MINOR, not MAJOR: the posture is
deliberate and documented (§21.2 uncertainty-never-success; b7 in the round-1 battery
pins error-not-running), the harm is bounded by the operator/runtime cancel path, and
silent corruption is impossible (typed error, never a coerced state). Recording as the
honest cost of that posture: the adapter never self-terminates a wedged invocation.

### F3 — MINOR (locus-label honesty)
`TemplateError::UnterminatedQuote`'s offset (`template.rs:219`, `251`) is a CHAR index,
but its Display text (`template.rs:88`) says "at byte {offset}". With multibyte
prefixes the labeled locus is wrong (executed: `ää'` reports offset 3; the true byte
position is 4). Typed and diagnosable, so MINOR; a one-word doc fix or a byte-offset
computation repairs it.

### F4 — MINOR-with-COR (missing teeth; decode-locus exclusivity unenforced for the cli crate)
The delegation await-ban scan (`crates/gauntlet-arch-tests/src/delegation.rs`) pins
result-decoding exclusivity (`read_result_contract(` / `RoleResult::decode(` only in
`poll`/`collect`) — but its subject is `PRIME_ADAPTER_SRC` only. The cli adapter has the
vendor-term scan (`check_generic_cli_adapter_sources`, teeth proven in L10a) and NO
decode-locus scan. Today the mechanism is correct by inspection: the cli crate's only
decode locus is `runner.rs` `classify_contract` (poll path). A future transport-side or
template-side decode in the cli crate would be a structural violation of the same
principle (§10.5's "the seam must never derive a result from anything it observes")
with nothing to catch it — the shared suite cannot distinguish a decoding transport
from a live one, because decoding the real file yields the same verdicts. Same class
as F1, smaller blast radius; a delegation-scoped sibling of the prime scan over
`crates/adapters/gauntlet-adapter-agent-cli/src` closes it.

## Teeth

(pending)

## Teeth

How each gate's sensitivity was proven (RED evidence → byte-identical restore → GREEN):

1. **P8 shape-2 teeth (mutation M2).** RED: with the exited-child
   `Failed{missing_result}` pin removed from `runner.rs` poll, P8 fails immediately
   (the shared `exit_without_result_suite` reports the exited child never reaching the
   pinned verdict) and the round-1 live battery fails alongside it. Restore: pristine
   copy over the file, sha256 `421cb65498b822bd2c266622170647c38277c62be2c2fd2ed3cd1a05771fe043`
   re-verified identical. GREEN: P8 passes; 18/18 live attacks pass.
2. **Arch vendor-ban teeth.** RED: four vendor terms planted in the cli crate's
   `lib.rs` — `just boundaries` fails with
   `denied-cli-adapter-identifier: …/src/lib.rs:68: 'claude'`. Restore: pristine copy,
   sha256 `1df6737da229ef8336d7382f49ab34bc7a9ade8b7b53b2b0c300ea3895406119` re-verified.
   GREEN: 224/224 arch tests pass.
3. **Doctor honesty teeth (the gap).** RED-inversion under mutation M1: with
   `probe_template` gutted to parse-only, EVERY shipped suite stays green (58/58
   adapter in-tree, 62/62 gauntlet-cli incl. P8) — no shipped test catches a dishonest
   configuration probe. My temporary scratch shape-check went RED under the same
   mutation, then passed after the byte-identical restore (sha256 `794d061858479c5f84cb4c07a49cd1912e5aaac956d0022b604d8a283340ea89`
   re-verified), proving the check itself discriminates. This is F1's evidence.
4. **P8 fail-closed selection teeth.** Executed, not read: `bogus` and `BOTH` FAIL
   loudly; `mock`/`cli`/unset pass; the default run binds the two-factory + non-mock
   assertions.
5. **Post-completion tamper.** Executed fresh variant (valid impostor document,
   foreign id AND digest, followed by poll, not just collect): the pin holds and
   `collect` serves the original snapshot.

## Not attacked (honest scope)

- Full workspace mutation suite (task 24's instrument; out of scope per brief). Two
  targeted single-mechanism mutations only (M1, M2).
- Daemonized grandchild / process-group exotica (recorded blindspot; my stop-path
  attacks stayed at parent-child granularity).
- Real agent binary or model (explicit non-claim, kept).
- Dispatch-gate composition (R-1b fence — no Unit ever pushed through a gate).
- BudgetMeter / observer / comparator surfaces (untouched by task 16).
- Concurrency >2 invocations; OS pid reuse against the transport's pid-keyed child map
  (exotic; the early-exit check in `stop` closes the practical window, but the
  BTreeMap is pid-keyed — noted, not attacked).
- The SIGKILL-landed-but-unreapable `Ok(None)` reap-grace expiry path: not reachable
  with a normal child on this host (SIGKILL always lands; only a D-state process could
  stall the reap), so it remains pinned only by code reading, not by any test — folded
  into the F1 family of observation (noted; no finding raised beyond F1/F4).
- AK-013's logical evidence locator `tests/proofs/p8_adapter_substitution` vs the
  physical file `crates/gauntlet-cli/tests/p8_adapter_substitution.rs`: name-level
  match verified; mapping bookkeeping belongs to the orchestrator's settlement review.

## Hygiene

- **Probe children:** every spawned child was a trivial `sh` script under my own
  scratch roots, bounded (≤ 20 s sleepers), and reaped or killed by the tests; the
  dirhang sleeper was explicitly killed and waited gone before cleanup. Final sweep
  `pgrep -af 'sleep 20|sleep 30|sleep 121|probe-child'` → empty. One transient system
  `sleep 1` (pid 1846759) appeared in a broad sweep and was gone on attribution; not
  mine.
- **Scratch:** `/tmp/gauntlet-falsifier-r2-scratch` removed; all
  `/tmp/gauntlet-falsifier-*`, `/tmp/gauntlet-cli-adapter-*`, `/tmp/gauntlet-cli-live-*`
  dirs created by my test runs removed (0 remaining). The temporary scratch test file
  `crates/adapters/gauntlet-adapter-agent-cli/tests/falsifier_r2_scratch_attacks.rs`
  was REMOVED; `git status --porcelain -- crates/` is empty.
- **Planted mutations restored byte-identical, digest-verified:**
  `template.rs` `794d0618…`, `runner.rs` `421cb654…`, `lib.rs` `1df6737d…` — each
  re-hashed after restore against pre-mutation digests. DEVIATION RECORDED: the first
  restore used `git checkout -- <file>` (a working-tree restore from the index, no
  history/index write); all later restores used pristine file copies, and all three
  files were digest-verified identical afterward.
- **Nothing planted under `.gauntlet/`:** `git status --porcelain -- .gauntlet/` is
  empty. The doctor's configured `/tmp/gauntlet` state/evidence/workspace roots are its
  own defaults-table behavior, not plants.
- **`just ruler`:** PASS at round start and PASS at round end.
- **`just doctor`:** OK on the current tree (exit 0); the hostile-template legs were
  run with environment variables only and left no residue.
- **Firewall:** none of the firewalled files (`b1-builder.md`, `b1-progress.md`,
  `critic-design.md`, `design-adjudication.md`, `IN-FLIGHT.md`, `correction-round1.md`,
  `falsifier-round1.md`, anything under `.agents/research/preregistrations/`,
  `.agents/CURRENT_STATUS.yml`) was opened; no incidental exposure occurred. In-tree
  round-1 TEST files were read (explicitly allowed as source/test material).
- **Final battery:** `just test-package gauntlet-adapter-agent-cli` 53/53 green after
  scratch removal.

## Verdict rationale

CONFIRM. Every executed attack against the shipped mechanism was repelled: the
classification total order (contract wins, missing/malformed/mismatch classes,
shape-2-before-deadline), post-terminal stability against a valid impostor tamper,
cancel lifecycle and stop escalation, capabilities honesty, template fail-closed
grammar, P8's fail-closed selection, the doctor's configuration-probe honesty
(executed), and the arch vendor ban (teeth proven RED→GREEN). The two structural
findings are teeth debt, not mechanism defects: F1 (MAJOR-with-COR — the doctor's
`probe_template` honesty check has no permanent in-tree guard; mutation-proven
survivor) and F4 (MINOR-with-COR — decode-locus exclusivity unenforced for the cli
crate). F2 and F3 are MINOR. No BLOCKING or in-mechanism MAJOR defect stands.

**VERDICT: CONFIRM**
