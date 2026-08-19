# Task 8 Blind Critic Report — Mock AgentRunner and Fixtures (critic-a)

Reviewer stance: spec-conformance review of the artifact against frozen plan Task 8 and AK-059, with no knowledge of the builder's process or claims. Read-only analysis; nothing was compiled or executed. All verdicts below are static-analysis verdicts over the files listed in "Scope".

## Scope (bounded claims)

Read fully:
- `crates/adapters/gauntlet-adapter-agent-mock/src/{lib,runner,scenario,hooks,deterministic_clock,seeded_ids}.rs` (including embedded unit tests)
- `crates/adapters/gauntlet-adapter-agent-mock/tests/{contract_suite,scenario_playback}.rs`
- All 8 fixtures under `tests/fixtures/mock_runner_scenarios/`
- `crates/adapters/gauntlet-adapter-agent-mock/Cargo.toml`
- Ports: `crates/gauntlet-ports/src/role_result.rs`, `crates/gauntlet-ports/src/contract/agent_runner.rs`
- Spec: `.agents/specs/Core-spec.md` §6.1 (L408-438), §6.2 (L440-459), §6.3 (L461-474), §7.3 (L497-499), §8.6 (L599-613), §10.3 (L738-755), §10.6 (L796-800), Annex B.1/B.2 (L1902-1934), Annex E (L2082-2098)
- `.wayfinder/answer-key.md` AK-059 (L411-416)

Skimmed / grep-level:
- `/home/sprime01/projects/gauntlet/Cargo.toml` (workspace membership), `crates/gauntlet-ports/src/agent_runner.rs` (error/failure vocabularies, `is_terminal`)

Not done: no build, no test execution (read-only mandate). "The suite passes" is therefore evidenced by static consistency between the mock's `RunnerScript` mapping and the suite's cases, not by a green run.

## Requirement-by-requirement verdicts

| Requirement (plan Task 8 / AK-059) | Verdict | Anchor evidence |
|---|---|---|
| 1. Mock crate driven by fixtures at the AK-059 locator | Satisfied | `scenario.rs:1546-1568` (`scenario_fixtures_dir` = repo `tests/fixtures/mock_runner_scenarios`, stem/identity cross-check); exactly 8 yaml files present; `Cargo.toml` workspace member (`Cargo.toml` root L19) so its tests actually run |
| 2. Eight REQUIRED scenarios, each distinct and asserted | Satisfied (see MINOR-3, MINOR-5 for the two soft spots) | `scenario_playback.rs:27-36` pins the inventory; one test per scenario (L139-409), each with scenario-specific assertions beyond the terminal state |
| 3. Deterministic clock + seeded id generator selected by GAUNTLET_CLOCK / GAUNTLET_ID_SEED | Satisfied | `hooks.rs:79-114` (pure selectors + env wrappers), `deterministic_clock.rs:44-61`, `seeded_ids.rs:121-161` |
| 4. Full AgentRunner contract suite passes | Satisfied-by-static-analysis | `contract_suite.rs:19-34` runs the unmodified `gauntlet_ports::contract::agent_runner::agent_runner_suite` (all 10 cases, ports `contract/agent_runner.rs:809-825`) via `MockRunnerFactory`; `runner.rs:135-187` maps all 8 `RunnerScript` variants; probe arithmetic fits `ProbeBudget::INSTANT` (ceiling 24 ≥ 4 needed for the stall case; ports L42, L61-64) |
| Done-when: eight distinct asserted runtime states + suite passes (AK-059) | Met, static | See MINOR-3 for the honest caveat on "distinct" for the two rule-6 scenarios |
| Mock must NOT decide orchestration / encode agent reasoning | Satisfied | `scenario.rs:54-60` (child behavior only, the two time-ish knobs justified as child modeling); `runner.rs:12-16` ("returns results; it decides nothing"); the only verdicts it renders are port-mandated ones (TimedOut per §7.3 suite case; Failed classes per §10.3 via `failure_class`) |

## CRITICAL

None found — bounded to the files listed above. No requirement clause is violated, and I found no path where the Task-8 gate would pass while a listed requirement is unmet.

## MAJOR

### MAJOR-1: `workspace_effects` under `behavior: stall` is accepted by the parser, documented as legal, and silently never applied by the runner

- Dialect documentation promises effects are usable under either behavior: `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs:45-47` — "workspace_effects: # OPTIONAL under either behavior".
- The parser honors that promise: `scenario.rs:1200-1203` validates `workspace_effects` with no behavior-conditional refusal (contrast the explicit `BehaviorConflict` arms for `state`/`derailment`/`payload`/`probes` under stall at `scenario.rs:1157-1196`).
- The runner applies effects only in the `WriteResult` branch (`runner.rs:196-215`, `apply_effects` called once at write time); the `Stall` branch (`runner.rs:239-251`) never calls `apply_effects`, and neither does `invoke` (`runner.rs:318-337`).
- Consequence: a future fixture `behavior: stall` + `workspace_effects` would parse cleanly and silently encode a lie — the files it claims the child wrote are never written, with no typed error at parse or runtime.
- Offended clause: plan Task 8 item 1 (fixtures *drive* the runner; the dialect must not silently diverge from what a fixture claims to encode). This is exactly the "fixture dialect silently diverges" failure shape this review was asked to hunt.
- Mitigating facts (why MAJOR, not CRITICAL): none of the eight frozen fixtures uses the combination (`no-result.yaml` carries no effects), so no required scenario is mis-replayed, and the gate cannot pass on a false premise today. Bounded fix: either one `BehaviorConflict` arm refusing effects under stall, or applying effects at admission for stall children — either closes it in one place.
- Alternative reading, for fairness: a silent child plausibly writes nothing at all (including no files), in which case the runner is right and both the doc comment (`scenario.rs:45`) and the parser's permissiveness are the defect. Either way the three sites disagree; that disagreement is the finding.

## MINOR

1. **Wrong spec citation for the hooks.** `hooks.rs:1-2` cites "spec §8.6 / Annex E" for the test hooks; §8.6 is the *defaults table* (spec L599-613). The hooks actually live in Annex E (spec L2098, reached from §8.3 L565-567). Documentation nit only; the hook semantics themselves match Annex E (which names the two variables without a token vocabulary, so the invented exact-token `deterministic` with typed `UnknownClockToken` refusal — `hooks.rs:79-89` — is a compliant, fail-closed choice, not a divergence).
2. **The runner never consults any `Deadline`.** `RoleInvocationSpec.deadline` is stored but never read; `collect`'s deadline parameter is ignored (`runner.rs:350-354`); §10.3 rule 5's "no result by deadline" is modeled as `timeout_after_probes` poll counts (`scenario.rs:56-60`, `runner.rs:239-251`). This is documented honestly (`runner.rs:1-16`, time-free by design) and the port suite itself accepts a probe surrogate for scripted adapters, but it means the deadline fields are dead weight in this adapter and rule 5 is *modeled*, not *enforced*. Relatedly, the §10.3 rule-2 atomic file write (tmp+fsync+rename) is emulated by an in-memory map (`runner.rs:96-99`, `documents`), which is trivially "atomic" — honest emulation, noted for the record.
3. **The two rule-6 scenarios are indistinguishable at the runner boundary in the integration tests.** `scenario_playback.rs:256-281` asserts only `Failed{ContractMismatch}` for both `wrong-invocation-id` and `stale-envelope-digest` — identical asserted runtime states. The distinguishing assertions (`InvocationIdMismatch` vs `EnvelopeDigestMismatch`) exist only in the crate's unit tests (`scenario.rs:2018-2067`). This is spec-consistent — §10.3 rule 6 (spec L753) collapses both mismatches to one failure class, so "distinct runtime state" cannot mean a distinct class for this pair — but a reader of `scenario_playback.rs` alone cannot tell the two fixtures apart. Distinctness for the remaining six scenarios is carried by genuinely distinct assertions (identity echo; never-collectible malformed; TimedOut + `ResultNotReady{TimedOut}`; artifact/claims divergence; stand-in mutation + real-ruler byte comparison; critic role + empty evidence arrays).
4. **The critic fixture wears a builder's payload schema.** The renderer always emits `payload_schema: builder_result@v1` (`scenario.rs:529-531`), so `critic-pass-without-evidence` produces a document with `role: critic` and a builder payload schema. This is spec-literal — Annex B defines exactly one payload schema (B.2), and the ports reader rejects any other token (`role_result.rs:494-502`) — and the design note is recorded in-test (`scenario_playback.rs:369-375`). Still a semantic oddity a future schema split will have to revisit. Corollary: the contract reader performs no role-vs-dispatch check (only §10.3 rule 6's two fields, `role_result.rs:362-380`), so the playback harness dispatching the critic scenario under `role: Builder` (`scenario_playback.rs:86`) completes without objection — spec-frozen behavior, noted so nobody mistakes it for an enforcement gap in this adapter.
5. **The answer-key attempt targets a stand-in, not the literal ruler path.** The fixture's effect path is `ruler-standin/answer-key.md` (`answer-key-write-attempt.yaml:12,20`), not `.wayfinder/answer-key.md`. The attempt's *intent* is carried by `commands_run` ("edit the ruler…") and `changed_paths`, and the test does the containment proof properly: sentinel mutation asserted AND the real `.wayfinder/answer-key.md` byte-compared before/after (`scenario_playback.rs:322-366`). Defensible modeling — under the mock's workspace-root containment a literal path would also land in scratch — but it is a stand-in replay of the attempt, worth naming against the plan's wording "attempts to write the answer key".
6. **Crate name diverges from the plan's shorthand.** Plan says `crates/adapters/agent-mock`; the artifact is `crates/adapters/gauntlet-adapter-agent-mock` (`Cargo.toml:2`). This matches every sibling adapter and the workspace member list, and the locator that AK-059 actually freezes (the fixtures directory) is spelled exactly. No action suggested.
7. **Flow-sequence entries split on commas by definition.** A `commands_run`/`claims` scalar containing a comma inside `[...]` form would silently become two entries (`scenario.rs:879-899`). Documented dialect shape ("flow sequences of scalars"), no fixture affected (all multi-word values are comma-free). Noting for fixture authors.

## Positive conformance evidence (what was checked and held)

- **§10.3 rules 3/4/6 enforcement is single-sourced.** Completion is admitted only through `read_result_contract` (strict decode + rule-6 identity check) — `runner.rs:221-237`; decode failures map to `Failed{class}` via `RoleResultError::failure_class` (`role_result.rs:166-174`: only the two rule-6 mismatches → `ContractMismatch`, everything else → `MalformedResult`). The runner adds no parallel validation to drift against.
- **Annex B.1 field-for-field rendering.** The renderer emits exactly the 12 envelope fields (`scenario.rs:502-543`) matching `ENVELOPE_FIELDS` (`role_result.rs:41-54`); payment carries the five B.1 keys zeroed (`scenario.rs:670-694` vs reader `role_result.rs:879-944` — `cost_usd` as `Uint(0)` is accepted where the annex example shows `0.0`); payload emits exactly the 7 B.2 fields (`scenario.rs:624-647`); child-declared runtime verdicts (`timed_out`) are refused as states by both dialect and reader (`scenario.rs` unit test L1806-1813; `role_result.rs:182-207`).
- **Derailment mutations are value-safe by construction.** Foreign invocation id and stale digest flip in-alphabet characters and differ on either branch (`scenario.rs:551-572, 701-716`); unknown-field derailments inject exactly the fields the strict reader rejects (`scenario.rs:537-543, 607-623`), with unit tests pinning each rejection mode (`scenario.rs:1998-2101`).
- **Contract-suite mapping is complete and budget-fits.** All eight `RunnerScript` variants map (`runner.rs:135-187`); suite probe arithmetic: `CompleteAfterProbes{1}` terminal on poll 2, `NeverWriteResult` (stall, 4 probes) terminal on poll 4, both well inside `INSTANT`'s 24-probe ceiling (ports `contract/agent_runner.rs:42, 61-64, 169-218`).
- **Hooks fail closed.** Unknown `GAUNTLET_CLOCK` tokens are typed errors, never silent fallbacks (`hooks.rs:79-89`, tested L160-175); empty/unset reads as "no hook" for both; clock and id hooks cannot be cross-routed (separate selectors).
- **Containment is honest.** `capabilities()` reports `isolation: none`, `max_depth: 0`, `result_delivery: file` (`runner.rs:397-409`), exactly the §6.1 posture that lets Task 14's scheduler refuse it for contained tiers; effect paths are guarded twice (parse-time lexical containment `scenario.rs:1508-1537`, resolution-time prefix re-check `runner.rs:258-274`).
- **The mock decides nothing and encodes no reasoning.** Fixture vocabulary is child behavior only (`scenario.rs:54-60`); claims/commands are scripted strings, never logic; the timeout and failure verdicts the runner renders are the port's own required semantics (§7.3 suite case list, spec L499).

## What this review does not establish

- That the tests actually pass (no execution permitted). Static consistency is strong — every suite case has a mapping whose probe arithmetic fits the budget, and the renderer/reader pair is mutually consistent by construction — but a green run is a different agent's evidence.
- Behavior of `RoleId::parse`/`Digest::parse` internals (domain `values.rs` was located, not read).
- Whether any consumer outside this crate (e.g., an app-level teeth run) also drives `MockRunnerFactory`; Task 8 does not require one.

Severity totals: CRITICAL 0 · MAJOR 1 · MINOR 7.
