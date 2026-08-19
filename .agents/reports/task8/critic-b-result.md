# CRITIC-B REPORT — Adversarial review of the mock AgentRunner adapter

> **Persisted by the orchestrator, verbatim from the critic's final message.**
> The critic (read-only Explore agent) declared a delivery deviation: it could
> not write files under its mandate, so the report arrived in-message and is
> persisted here unchanged. File:line references are to the tree as it stood
> at review time (post lint-closure, pre correction round).

**Delivery deviation (declared):** this agent operates under a strict read-only mandate (no file creation of any kind, no shell redirection, no build lock access). The output contract's requested write to `.agents/reports/task8/critic-b-result.md` could not be performed; the full report is delivered here instead for the parent agent to persist verbatim.

**Bounded-claim discipline:** every verdict below comes from static reading of code paths, not execution. "SURVIVES" means *the specific attack I constructed* fails against the code as written — it never means the mechanism is unbreakable. Where a mechanism's behavior depends on runtime facts (OS path resolution, stack limits), the verdict is stated with the exact runtime probe that would settle it.

---

## Attack surface 1 — Parser termination / memory

**Attack 1a — non-terminating traversal.** Attempt to construct input where `parse_scenario` loops forever: a `mapping` that re-reads a line, a `block_sequence` that stalls, a `dash_mapping_item` that returns to a consumed line.
Path: `scenario.rs:808-847` (`value_of`), `852-875` (`mapping`), `905-954` (`block_sequence`), `961-996` (`dash_mapping_item`).
Analysis: every arm advances `*pos` by at least one line or breaks/errors — `value_of` increments before every return (`815-846`); `mapping`'s loop body always passes through `value_of`; `block_sequence`'s three item arms each increment (`919`, `944`, inside `dash_mapping_item` at `974`); `dash_mapping_item` advances past every cloned line (`983`). The root check (`1021-1027`) turns any leftover line into `UnplaceableLine`. **Verdict: SURVIVES — no non-terminating input exists within this traversal.**

**Attack 1b — unbounded recursion depth (stack exhaustion).** A staircase document (`a:\n b:\n  c:\n ...`, each key's value block one indent deeper) recurses `mapping → value_of → mapping` once per line. Depth is bounded only by line count; there is no `MAX_DEPTH` analog. Contrast: the ports' own JSON reader caps nesting at 64 with a typed `DepthExceeded` (`crates/gauntlet-ports/src/json.rs:28,39,250-252`) — the workspace's own standard for hostile-input parsers, which this parser does not meet. A deep-enough fixture aborts the process on stack overflow — not a typed error.
Path: `scenario.rs:831-837` (recursion site), no depth counter anywhere in `808-996`. **Verdict: BREAKS under adversarial input — MAJOR-2. Runtime probe: feed `parse_scenario` a 10⁵-line staircase; observe abort vs typed error. Mitigation as shipped: fixtures are repo-trusted.**

**Attack 1c — quadratic memory.** Nested dash-mapping staircases (`- a: 1` at ever-deeper indents) make each `dash_mapping_item` clone its whole deeper tail (`scenario.rs:969-985`) before re-parsing it; a line at nesting depth *d* is copied *d* times → Θ(n²) bytes. **Verdict: BREAKS under adversarial input (same trust-model caveat) — folded into MAJOR-2.**

**Probe knobs:** `PROBE_CEILING = 64` (`scenario.rs:86`) bounds both knobs; `optional_probe` (`1240-1272`) parses ASCII digits only into u32 then range-checks — no overflow, no unbounded loop in the runner (`advance` stops incrementing once terminal, `runner.rs:190-193`). `lex` is O(input), no backtracking. The crate's lesson claim ("every traversal... terminates by construction, and its bound is named", `scenario.rs:82-86`) holds for *termination* but not for *memory/depth* — the named bounds cover only the probe knobs.

## Attack surface 2 — Classification confusion

**Attack 2a — flip reproducing the original.** `flip_char_at` (`scenario.rs:701-711`) maps `'0'→'1'`, any other ASCII→`'0'`, non-ASCII untouched. For `ForeignInvocationId` it flips the dispatched ULID's last char (`551-557`); for `StaleEnvelopeDigest` the first hex char after `':'` (`564-572`). Domain grammar check (`values.rs:362-429`): ULID = 26 chars of `0-9A-HJKMNP-TV-Z` (ASCII — the non-ASCII escape hatch is unreachable); Digest = `algo:hex` where algo cannot contain `':'` (`values.rs:526-570`), so `find(':')` always hits the true separator and hex is non-empty lowercase hex. Flip output is `'0'` or `'1'` — in-alphabet for both, and always different from the input. **Verdict: SURVIVES — the "mutated" document can never equal the original, and stays grammar-valid so the failure is `contract_mismatch`, never accidentally `malformed_result`.**

**Attack 2b — fixture text poisoning the renderer.** A claim/artifact/command containing `"`, `\`, control characters, or `:` — can a `Derailment::None` scenario render invalid JSON and misreport `malformed_result`? The renderer wraps all fixture strings in `JsonValue::Str` and serializes through `to_json_text` → `write_escaped` (`json.rs:202-220`), which escapes quotes, backslashes, and all C0 controls. **Verdict: SURVIVES.**

**Attack 2c — Completed when it should be Failed.** The only path to `Completed` is `read_result_contract` strict decode + identity check (`runner.rs:221-231`); every decode failure maps through `failure_class()` (`role_result.rs:166-174`). A `state: failed` document that passes identity → `InvocationStatus::Completed` with `RoleResult.state() == Failed` — by design: the *invocation* completed its contract; the *role's* failure travels in the document. No fixture or derailment bypasses this. **Verdict: SURVIVES.**

**Attack 2d — derailment reading as valid.** `UnknownEnvelopeField`/`UnknownNestedField` inject fields rejected by the field tables (`role_result.rs:41-66, 444-458`); the nested variant self-materializes a placeholder check when the scenario scripted none (`scenario.rs:607-623`) so it can never silently degrade to a valid document. `RawMalformed` emits prose (`scenario.rs:493-499`). **Verdict: SURVIVES.**

## Attack surface 3 — Containment via workspace_effects

**Attack 3a — prefix collision `/root-evil` vs `/root`.** `apply_effects` uses `Path::starts_with` (`runner.rs:261`), which is **component-wise**, not string-prefix. `/tmp/root-evil/x`.starts_with(`/tmp/root`) is false. **Verdict: SURVIVES.**

**Attack 3b — lexical escape from a fixture.** Parser refuses absolute (`scenario.rs:1518-1522`), any `ParentDir` component (`1528-1535`), whitespace (`1523-1527`), empty. A path of `.` writes at the root itself → `fs::write` on a directory → typed `Failed{agent_execution}` (`runner.rs:271-272, 202-205`), no escape. NUL bytes and unicode look-alikes (`…` for `..`) fail or create literal names; no re-normalization trick survives. **Verdict: SURVIVES — no fixture text passes both guards and escapes lexically.**

**Attack 3c — symlink pre-placement.** Both guards are lexical; neither `canonicalize`s. Test code pre-placing `workspace_root/link → /etc` with effect path `link/passwd` passes parse (`validate_effect_path`) and resolution (`starts_with`), then `create_dir_all`/`fs::write` follow the symlink outside the root. Path: `runner.rs:258-274`. **Verdict: BREAKS the mechanism *only* under an adversary who controls the filesystem under the caller-provided root — which the adapter's own docs scope out (fixtures never name absolute paths; the root is test-provided). MINOR-3. Runtime probe: create the symlink, run the effects fixture, check for the out-of-root file.**

## Attack surface 4 — State-machine holes

**Attack 4a — poll after cancel.** `cancel` sets `terminal = Cancelled` (`runner.rs:393`); `advance` returns immediately on any terminal (`190-193`); second cancel is the typed `AlreadyTerminal` no-op (`386-392`). **SURVIVES.**

**Attack 4b — collect on terminal-but-not-Completed / non-terminal.** Both arms return typed `ResultNotReady` with the coarse state (`runner.rs:361-371`); `Completed` with `result == None` is unreachable (set together at `226-231`). **SURVIVES.**

**Attack 4c — two invokes with the same `invocation_id`.** `invoke` does an unconditional `entries.borrow_mut().insert(...)` (`runner.rs:326-335`) — a second admission of an already-terminal id **silently replaces the entry** (probes reset, `terminal = None`). The old handle (same id) then polls `Running` again: a resurrected terminal, violating the pinned "every later poll returns the SAME terminal status" (`agent_runner.rs:416-424`). No typed `AdmissionRefused`, no preservation of the terminal. The shared suite has no duplicate-invoke case (`contract/agent_runner.rs:809-825`), and no crate test covers it. Path: `runner.rs:318-337`. **Verdict: BREAKS the pinned stability semantics through the public API — MAJOR-1. The overwrite behavior is unambiguous from the code; a runtime probe would only confirm the poll sequence `Completed → (re-invoke) → Running`.**

**Attack 4d — entries-map keying / forensic cross-talk.** `documents` is keyed by `result_contract_path` string (`runner.rs:99, 132, 209-219, 217`): two invocations sharing one contract path clobber each other's raw text, and the effect-failure placeholder (`209-212`) overwrites a prior real document at the same path. Classification is unaffected (the validated `RoleResult` lives per-entry); only the forensic surface lies. **Verdict: SURVIVES classification; MINOR-6 forensics.**

## Attack surface 5 — Determinism

Constructed attack: replay same scenario + same spec twice; hunt for HashMap-order, pid, wall-time, or RNG dependence. The renderer is a pure function of typed parts (`scenario.rs:492-648`); `Running` cites a constant epoch (`runner.rs:59`); neither map is ever iterated (only `get`/`insert`); effects replay in `Vec` order; `scratch_root()`'s pid/counter (`295-303`) only names a directory that suite scenarios never write to (all `from_script` scenarios carry empty effects, `279-291`). `SeededIdGenerator` is a pure function of seed + counter (`seeded_ids.rs:99-145`). **Verdict: SURVIVES.**

## Attack surface 6 — Hook selection

Edge battery: `GAUNTLET_CLOCK` unset/`""` → production (documented); `"  "` / `"Deterministic"` / `"deterministic\n"` → typed `UnknownClockToken` (`hooks.rs:79-89`, pinned by tests at `hooks.rs:160-175`); exact `"deterministic"` → epoch-0 clock. `GAUNTLET_ID_SEED` unset/`""` → production; any non-empty → seeded. No silent misroute found. One asymmetry: a whitespace-only seed silently selects a generator seeded with whitespace, while a whitespace clock token is an error (`hooks.rs:102-108` vs `79-89`) — deterministic and documented, cosmetic only. **Verdict: SURVIVES; MINOR-7 asymmetry.**

## Attack surface 7 — The eight fixtures

Traced each fixture through parser → runner → playback assertion:

| Fixture | Encodes | Assertion pins it | Verdict |
|---|---|---|---|
| `happy.yaml` | 1 in-flight probe then valid doc | `scenario_playback.rs:140-181` (Running first, identity echo, repeat-collect equal) | Correct |
| `malformed.yaml` | prose instead of JSON → `Failed{malformed_result}` | `184-213` (+ 3-poll stability) | Correct |
| `no-result.yaml` | stall, timeout 3 → exactly 2 Running then `TimedOut` | `216-253` | Correct |
| `wrong-invocation-id.yaml` | foreign-id doc → `contract_mismatch` | `256-268`; flip of ULID tail `V→0` verified different | Correct |
| `stale-envelope-digest.yaml` | stale digest → `contract_mismatch` | `271-281`; `a…→0…` verified valid-but-different | Correct |
| `lying-builder.yaml` | valid doc claiming exit 0 vs on-disk `FAIL` artifact | `284-319` asserts both sides coexist | Correct |
| `answer-key-write-attempt.yaml` | mutation attempt against a stand-in under the root | `322-366` (stand-in mutated, real ruler byte-compared, `changed_paths` owns up) | Correct |
| `critic-pass-without-evidence.yaml` | valid envelope, pass claim, empty evidence arrays | `369-409` (design note recorded in-test) | Correct |

No fixture passes while encoding the wrong state. Two dialect/test tensions found: (i) `timeout_after_probes` is documented `<1..=64>` (`scenario.rs:28`) but `optional_probe` accepts `0` — a stall child that times out on the *first* poll parses fine (**MINOR-4**); (ii) `PROBE_CEILING = 64` exceeds both drivers' budgets (suite ceiling 24 at `contract/agent_runner.rs:42`; playback bound 24 at `scenario_playback.rs:116`), so a legal fixture with `probes_before_write: 30..=64` parses but its playback panics loudly — a defect signal, never a silent pass (**MINOR-5**).

## Residual table

| # | Severity | Finding | Locus | Conditions to fire |
|---|---|---|---|---|
| 1 | **MAJOR** | Duplicate `invoke` with the same id silently resets a terminal entry — resurrection violates pinned terminal stability; no typed refusal | `runner.rs:326-335` | Any caller reusing an invocation_id after terminal |
| 2 | **MAJOR** | Scenario parser has no depth bound (stack overflow = abort) and Θ(n²) dash-subtree copying — no `MAX_DEPTH` analog to `json.rs:28`, contradicting the crate's own runaway-memory lesson | `scenario.rs:808-996` | Adversarial/oversized input to public `parse_scenario`; repo fixtures are trusted today |
| 3 | MINOR | Both containment guards are lexical; pre-placed symlinks under the workspace root defeat them | `runner.rs:258-274`, `scenario.rs:1508-1537` | Hostile filesystem control under the caller-provided root |
| 4 | MINOR | `timeout_after_probes: 0` accepted despite documented range starting at 1 | `scenario.rs:1240-1272` vs `:28` | Fixture author writes 0 |
| 5 | MINOR | `PROBE_CEILING` (64) exceeds every driver's probe budget (24) — legal-but-undrivable scripts | `scenario.rs:86` vs `contract/agent_runner.rs:42`, `scenario_playback.rs:116` | Fixture probe knob in 25..=64 |
| 6 | MINOR | `documents` keyed by contract path: cross-invocation forensic clobber; effect-failure placeholder overwrites prior real text | `runner.rs:99,209-219` | Two invocations sharing a result_contract_path |
| 7 | MINOR | Hook asymmetry: whitespace seed silently accepted where whitespace clock token errors | `hooks.rs:102-108` | `GAUNTLET_ID_SEED="  "` |

**Counts: CRITICAL 0 · MAJOR 2 · MINOR 6 (7 findings, 3 surfaces fully SURVIVE: 2, 5, and classification core).**
