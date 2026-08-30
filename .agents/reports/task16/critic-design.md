# Task 16 — pre-implementation blind design review (static-only)

**Critic:** independent fresh-context design critic (charter adopted verbatim).
**Mode:** STATIC. No cargo/build/test commands run. Allowed cheap gates run:
`just ruler` PASS, `just prereg-check 16` PASS (frozen `f522dbd3…`).
**Inputs:** the design brief (13 pinned decisions, verbatim in the dispatch),
`.agents/specs/Core-spec.md` (§4.1, §6.1–6.3, §7.1–7.3, §8.5, §10.3–10.6,
§17.3, §18.3, §21.1–21.2, §23.1, §24.3, Annex B/C), `.agents/plans/gauntlet-impl.md`
(Task 15/16, global gates), `crates/gauntlet-ports/src/agent_runner.rs`,
`crates/gauntlet-ports/src/contract/agent_runner.rs`, `crates/gauntlet-ports/src/contract/stubs.rs`,
`crates/gauntlet-ports/src/role_result.rs`, `crates/gauntlet-ports/src/deadline.rs`,
`crates/adapters/gauntlet-adapter-agent-prime/*` (runner, program, transport,
config, probe, tests), `crates/adapters/gauntlet-adapter-agent-mock/*`,
`crates/gauntlet-cli/src/main.rs`, `crates/gauntlet-app/src/doctor/{mod,checks}.rs`,
`crates/gauntlet-arch-tests/src/denylist.rs`, `.agents/CURRENT_STATUS.yml`,
`.agents/reports/task15/closure.md`, `.agents/reports/task14/closure.md`,
`.agents/research/preregistrations/task-16.prereg.yaml`, `.agents/reports/task16/IN-FLIGHT.md`.

VERDICT: NOT_CONFIRM

Residuals: 1 blocking · 5 major · 6 minor (12 total).

---

## Blocking

### C1 — The shape-2 suite case, added to the shared suite, cannot be passed by the settled prime adapter; "prime stays green, tests-only" is unimplementable as briefed

- **dimension:** functional / architecture
- **severity:** blocking
- **locus:** brief decision 3 (and prereg D1/transfer-test L1, IN-FLIGHT slice plan) vs
  `crates/gauntlet-ports/src/contract/agent_runner.rs:809-825` (`agent_runner_suite` runs
  every case against every factory), `crates/adapters/gauntlet-adapter-agent-prime/tests/contract_suite.rs:296-301`
  (prime's battery calls `agent_runner_suite` directly),
  `crates/adapters/gauntlet-adapter-agent-prime/src/runner.rs:17-26, 445-533`,
  `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs:24` (same call shape).

**Reasoning.** The new case pins: clean exit, no valid result ⇒ terminal
`Failed{missing_result}` (not Completed, not TimedOut), collect refuses. The prime
adapter's poll is a deliberate ONE-input state machine — the result-contract file plus
the spec deadline, nothing else. Its own module docs and the task15 closure pin that
liveness/process state is NEVER a state input, on spec ground: Annex C.4 / spec §10.5
(persistence row) — prime children idle-unload and reload, so an "exited" child is not
observably dead and may still write the contract later. Prime can therefore reach only
`Completed`, `Failed{malformed_result}`, `Failed{contract_mismatch}`, `TimedOut`,
`Cancelled`. For the new script the prime double can express "no document" only
(`document_for(script) -> None`, tests/contract_suite.rs:139-150), after which the
virtual clock passes the past-deadline suite instant and the runner reports
`TimedOut` — which prereg D1 itself classifies as a shape-2 violation. Adding the case
to `agent_runner_suite` (the only mechanism the brief names: "shared-suite extension")
makes the case universal: prime's contract-suite test goes RED with a finding, and no
tests-only change can fix it:

- (i) Change prime's runner to consult process exit — a semantic change to a SETTLED
  adapter (task15, commit a869556), contradicting its documented Annex C.4 posture and
  spec §10.5. Not tests-only. Rejected.
- (ii) Map the script to a document whose codec class is `MissingResult` — no honest
  mapping exists: `RoleResultError::failure_class()` (role_result.rs:166-171) yields
  only `MalformedResult`/`ContractMismatch`, and an empty/garbage file is `malformed`
  under §10.3 rule 4. Rejected.
- (iii) Scope the shape-2 case as a SEPARATE exported contract function (not inside
  `agent_runner_suite`), run by the factories whose adapters actually observe process
  exit (mock, cli) — p8 runs `agent_runner_suite` + the shape-2 case over both, so
  AK-013's "full suite" still covers its two adapters; prime's battery stays byte-green
  because the process-exit rule is plan-Task-16-specific (§10.6 "poll checks process
  state AND the result contract"), not a spec-universal §7.3 rule.

The brief must pin (iii) (or an explicit capability declaration on
`AgentRunnerFactory`) BEFORE implementation. As briefed, the builder hits an
unresolvable red suite and is then forced to either weaken prime (spec-divergent),
weaken the new case (making it toothless), or quietly special-case the suite
(I12 leak). Note also the "ALL THREE factories" count is wrong — see C4.

**Reproduction.** Add `exit_without_result_is_failed` to `agent_runner_suite`; run
prime's `the_prime_runner_passes_the_unmodified_shared_contract_suite`. The case's
`probe_until_terminal` observes `TimedOut` for the new script's `None` document and
emits a finding. No double-side change can alter the runner's verdict.

---

## Major

### C2 — The prompt-file content list omits the context envelope BODY (prime's sibling-file precedent is lost)

- **dimension:** functional
- **severity:** major
- **locus:** brief decision 8 vs `crates/adapters/gauntlet-adapter-agent-prime/src/program.rs`
  (envelope body written to `<leaf>.envelope.json` sibling; Annex C.1 step 1) and
  `crates/gauntlet-ports/src/agent_runner.rs:224-248` (`ContextEnvelope` docs: "the
  runner passes the body to the child and never computes the digest").

**Reasoning.** Decision 8's prompt file carries `invocation_id`, `envelope digest`,
`role spec body`, `result path` — but never `context_envelope.body()`, nor a reference
to any file that holds it. `RoleInvocationSpec` carries `prompt` AND `context_envelope`
as distinct §6.1 fields; a generic CLI child that never receives the envelope body has
no allowlisted context to work on and echoes a digest of bytes it never saw (the
§10.3 rule-6 tripwire degenerates into a constant). The digest itself is stable —
it is computed by the runtime's envelope builder and carried on the spec; the adapter
must write the exact body bytes and echo the carried digest verbatim, never recompute
(prime precedent). The design must pin where the body goes: inside the prompt file
(verbatim, machine-delimited) or as a sibling file under the invocation directory
(`<leaf>.envelope.json`, prime's exact precedent).

**Reproduction.** Walk decision 8's content list against the §6.1 spec fields: no
element delivers `context_envelope.body()`.

### C3 — §10.3 rule 1's environment channel (`GAUNTLET_RESULT_PATH`) is missing from the design

- **dimension:** functional / spec divergence
- **severity:** major
- **locus:** brief decisions 4, 6, 8 vs spec §10.3 rule 1: "The runtime creates the
  invocation directory and passes the path in the prompt **and the environment
  (`GAUNTLET_RESULT_PATH`)**."

**Reasoning.** The template placeholders and the prompt file cover the prompt half
only. The brief never mentions constructing the child's environment at all. A
contract-conformant child that reads only `GAUNTLET_RESULT_PATH` — the exact consumer
rule 1 anticipates — never finds its contract path, and the adapter silently
under-delivers a REQUIRED mechanism. Prime was structurally excused (harness kernel
children have no injectable environment); a direct `std::process::Command` subprocess
has no such excuse — `env()` is one call. The design should pin: the child environment
carries `GAUNTLET_RESULT_PATH` (and should state what else it carries or that it
inherits the parent otherwise; cf. task14 round5-F1's environment-inheritance
residual, recorded for the sandbox path).

**Reproduction.** Quote rule 1 against decisions 4/6/8: no design element sets any
environment variable for the child.

### C4 — The ports revision forces two consumer edits the brief miscounts or leaves semantically unpinned: the controller's failure disposition, and the ports' own suite stub

- **dimension:** architecture / functional
- **severity:** major
- **locus:** brief decisions 2 and 13 vs
  `crates/gauntlet-app/src/control/controller.rs:2106-2192` (`dispose_failure` —
  exhaustive `InvocationFailure` match: ContractMismatch ⇒ abort; MalformedResult ⇒
  reminder-or-escalate; OrphanedByRestart | Execution ⇒ §11.5 row-2 backoff retry) and
  `crates/gauntlet-ports/src/contract/stubs.rs:257-305` (exhaustive `RunnerScript`
  match feeding `AgentRunnerGreen`; factories at 407/471/537).

**Reasoning.** Adding `InvocationFailure::MissingResult` does not compile through
`gauntlet-app` without a new arm in `dispose_failure`, and that arm IS a domain
decision: is a clean-exit-no-result failure transient (backoff retry, like
`Execution`), a reminder-class failure, or escalate-only? The brief neither pins the
disposition nor reconciles it with decision 13's "no controller wiring" fence — the
builder must either invent a §11.5 classification (a silently picked domain decision)
or exceed the fence. Similarly, "ALL THREE factories learn the new script" is a wrong
count: the ports crate's own green stub (`stubs.rs`) matches `RunnerScript`
exhaustively and breaks the ports build until it learns the variant (likely harmless
to fix, but the brief's touch-surface enumeration is what the builder is fenced to).
The design must pin: (a) the disposition (with a spec anchor — e.g. row-2 transient
with its recorded rationale, or escalate-only with the budget argument: retrying a
child that exited cleanly without ever writing the contract re-pays for a likely
misconfiguration), and (b) the correct consumer list (mock script table + scenario
vocabulary, cli factory, prime test double, ports stubs).

**Reproduction.** Add the variant; `just check-package gauntlet-app` and
`just check-package gauntlet-ports` fail on non-exhaustive matches; the disposition
semantic is undocumented anywhere in the brief.

### C5 — The additive arch scan over all adapters' `lib.rs` pub items is red-at-birth on settled adapters, or forces new exemption machinery that collides with the REBIRTH re-litigation items

- **dimension:** security / architecture
- **severity:** major
- **locus:** brief decision 11 vs `crates/gauntlet-arch-tests/src/denylist.rs:135-196`
  (`DENIED_TERMS` includes "bwrap", "bubblewrap", "sqlite"),
  `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs:177,789,796`
  (`pub struct BwrapSandbox`, `pub use BwrapSandbox as ProcessIsolationBackend`,
  `pub struct BwrapSandboxFactory`),
  `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs:36-39`
  (`pub use state_store::SqliteStateStore as EmbeddedStateStore`).

**Reasoning.** "Walking `crates/adapters/*/src/lib.rs` pub items for curated vendor
terms" cannot pass on the current tree: `BwrapSandbox`/`BwrapSandboxFactory` are
task14's settled, legitimate public API and contain the denied term "bwrap"; the
state-sqlite re-export line carries "SqliteStateStore" ("sqlite"). The builder must
therefore either (a) narrow the scan (to what? the brief says all adapters), (b) grow
an exemption/allowlist ledger for settled offenders — new mask machinery of exactly
the class the arch suite's own docs warn about, firing the recorded REBIRTH
re-litigation items (E4/F1/F3, B.5a masked spellings, A5.2 allowlist granularity,
B-F16; CURRENT_STATUS line 431) — or (c) curate a SECOND vendor term list parallel to
the pinned `DENIED_TERMS` (which is inventory-pinned by
`denied_terms_inventory_is_pinned`), guaranteeing drift. The minimal sound design is
already licensed by plan Task 16 step 1 — "no vendor name **anywhere in the
[agent-cli] crate**": a whole-crate text scan of `crates/adapters/gauntlet-adapter-agent-cli/**`
over the EXISTING `DENIED_TERMS` satisfies the done-when teeth (plant a vendor
identifier anywhere — including the public API — and `just boundaries` goes red) with
zero false positives and zero new term lists. Also pin: the scan reads whole files, so
the "lib.rs only misses submodule pub items" concern dissolves.

**Reproduction.** Apply the scan as worded to HEAD: bwrap's pub items flag; the new
test is born RED (or is born with an undocumented bwrap/sqlite exemption list).

### C6 — The doctor mirror would report `measured: true` with evidence strings that claim a version probe ran, for a probe that measures only configuration presence

- **dimension:** evidence
- **severity:** major
- **locus:** brief decision 9 vs `crates/gauntlet-app/src/doctor/checks.rs:642-655`
  (`RunnerDescriptor.measured` doc: "the binary was found, its version parsed, and the
  version sits inside the supported range"), `checks.rs:746-753` and `checks.rs:958-971`
  (hardcoded evidence: "the harness API-surface version probe measured clean (version
  compatibility only)").

**Reasoning.** The CLI probe ("env var present + template renders") resolves no
binary, parses no version. Under the current `RunnerDescriptor` contract and the
hardcoded check-3/check-4 evidence strings, an honest outcome is impossible without
app-layer edits the brief does not budget: either the descriptor gains a
what-was-measured dimension and the check strings render it, or the CLI descriptor
returns `measured: false` with an honest note — which check 3/4 convert into
`runner_probe_failed` FAIL, wrongly failing doctor for a correctly configured CLI
runner. As briefed, the mirror emits false evidence ("version probe measured clean")
for a runner whose probe measured no version — an I8-adjacent honesty defect in the
exact surface (§8.5) whose job is truthful preflight. The design must pin the
app-layer change (a probe-kind/what-was-measured field on `RunnerDescriptor` and the
two evidence-string sites) or an honest-note form, and say which strings change.

**Reproduction.** Construct the CLI descriptor per decision 9 and trace it through
`check_runners`/`check_capabilities`: the pass evidence asserts a version probe that
never ran.

---

## Minor

### C7 — Decision 7's parenthetical mis-states the isolation-vs-tier rule and could produce a spec-divergent doctor refusal

- **dimension:** architecture
- **severity:** minor
- **locus:** brief decision 7 ("capability-vs-tier check must be able to refuse this
  runner for contained tiers") vs spec §6.1 rule 3 ("a tier requiring containment"),
  §17.3 (only cross_cutting names a runner rung: isolation ≥ process; `none` is
  permitted at contained for the SANDBOX backend), and
  `crates/gauntlet-app/src/doctor/checks.rs:966-970` (doctor's runner half explicitly
  defers isolation-vs-tier to the dispatch gate).

**Reasoning.** "Refuse for contained tiers" is not the rule: the runner's
`isolation: none` is honest and correct, external wrapping is the Sandbox port's job
(Annex C trust row), and the dispatch gate composes runner isolation with sandbox wrap
per invocation. Implemented as worded, the doctor would refuse exactly the tier where
an unwrapped real runner is least problematic and diverge from the shipped division of
labor. The actionable content of decision 7 (honest capabilities, truth-telling
descriptor) is right; the parenthetical must be corrected to "honest capabilities; the
dispatch gate refuses where §17.3 requires" before the builder treats it as a
work item.

### C8 — Classification order, exit-status recording locus, and shell-word edge cases are unpinned

- **dimension:** functional
- **severity:** minor
- **locus:** brief decisions 4/5/1 vs suite constants
  (`contract/agent_runner.rs:80-83`: suite deadlines are BASE_INSTANT+SPAN, Nov 2023 —
  already past for any wall-clock adapter) and Annex B.1 (strict schema, no
  `exit_status` field).

**Reasoning.** Four pins are missing that cost a builder round each if guessed:
(a) The poll classification must be a total order — contract read (Completed /
malformed / mismatch) → child exited without valid contract (`Failed{missing_result}`)
→ deadline exceeded (`TimedOut`) → `Running` — so the suite's past deadline yields
Failed for the exit script but TimedOut for the still-running script. The brief pins
"decided BEFORE the deadline" but not the ordering vs malformed/mismatch, nor what
happens when a valid/matching document appears for an already-exited child (Completed
— contract wins; state that). (b) "Exit status recorded, not obeyed" names no locus:
Annex B.1's strict codec rejects unknown fields, so exit status cannot live in the
result document — pin adapter-local forensics/log as the recording locus. (c) A child
that died by SIGNAL or nonzero exit without a result also lands in
`missing_result` under decision 1(b) as written; decide whether exit detail maps any
case to the existing `Execution` class or record the deliberate collapse (it matters
to C4's disposition: a crashed child is transient, a cleanly-exiting non-writer may not
be). (d) Template parsing edge cases to refuse as the typed configuration error:
empty/whitespace template, unbalanced quote, trailing backslash, argv[0] empty after
substitution, `{model}` present with no hint (empty word), unknown placeholder;
explicitly pin NO expansion (tokenize only — no `$VAR`, `$(…)`, globs; `#` starts a
word only outside quotes; embedded newlines flatten rather than fork commands) and
that substituted values are single literal argv words (spaces/`;` inside a model hint
or path never become grammar). A template that is ONLY `{prompt_file}` is legal-but-
odd; document, don't refuse.

### C9 — Clock injection posture is absent for the CLI runner

- **dimension:** architecture
- **severity:** minor
- **locus:** brief decision 6 (seam list: spawn/poll/kill) vs
  `crates/adapters/gauntlet-adapter-agent-prime/src/runner.rs:41-45` ("Time comes from
  the INJECTED `Clock` port — never the system clock (the shared suite's pinned
  deadlines live in the past; only an injected clock drives them deterministically)")
  and `crates/adapters/gauntlet-adapter-agent-mock/src/runner.rs:428-432` (mock
  refuses direct system-clock reads).

**Reasoning.** The runner must read `now` on every poll to decide shape-1 vs shape-2
ordering. Both in-tree precedents normatively require the injected `Clock` port; a
`SystemTime::now()` poll would pass the suite only by accident of the past suite
deadlines, contradicts both adapters' documented posture, and leaves the live leg and
any future controller use with no deterministic time seam. Pin `Arc<dyn Clock>` in the
constructor, mirroring prime's shape.

### C10 — Deliverables omit the AK-013 reference-candidates entry, and the p8 factory's template source is unpinned

- **dimension:** evidence
- **severity:** minor
- **locus:** prereg D5 + IN-FLIGHT hard boundaries (locator divergence
  `tests/proofs/p8_adapter_substitution` vs delivered
  `crates/gauntlet-cli/tests/p8_adapter_substitution.rs`, handled "via a
  reference-candidates entry (D5)", task14 AK-049 precedent) vs the brief's
  deliverables list (entry absent). Factory template source: brief decision 10 vs
  §8.1 (config from environment, never process-env reads inside tests).

**Reasoning.** The location precedent itself checks out (all real proof tests live in
`crates/gauntlet-cli/tests/`; the repo-root `tests/proofs/` is an empty `.gitkeep` —
D5's factual claim is accurate), but the owed candidates entry is not in the
deliverable list and will otherwise surface only at closure. Separately: selecting
batteries via `GAUNTLET_P8_ADAPTER` is fine (a recorded, acceptable deviation from the
plan's literal `--adapter`); but the TEMPLATE the cli factory renders must be a
constructor-injected test literal, never read from the operator's process environment
(`GAUNTLET_CLI_AGENT_CMD` in the test host makes the battery environment-sensitive and
nextest-order-dependent). Pin: template = builder parameter; env var only for battery
selection.

### C11 — The R-2 re-binding contradicts two recorded closure triggers and must itself be recorded, not just decided

- **dimension:** evidence / architecture
- **severity:** minor
- **locus:** prereg D3 + brief decision 13 vs `.agents/reports/task14/closure.md` R-2
  row ("The ports revision at Tasks 15/16, before any real runner consumes a wrapped
  command") and `.agents/reports/task15/closure.md` §4 ("R-2 now due at Task 16").

**Reasoning.** The fog decision's rationale is sound — this adapter consumes a command
template and never constructs or consumes a `SandboxedCommand`, so R-2's operative
invariant ("before any real runner consumes a wrapped command") is not approached; the
alternative reading (any ports touch fires R-2) would be scope invention. But the
re-binding currently exists only in the prereg/brief; two closure records name Task 16.
The task16 closure report and CURRENT_STATUS's unresolved_summary must explicitly
re-record the trigger as "first wrap-consuming composition (task20)" with the D3
rationale, and the builder's touch surface must stay template-only (no governance
type construction) or the rationale evaporates. Also state explicitly that prime is
touched tests-only, so T15-R1..R5 (which trigger on task16/20 touches of the affected
prime code) fire on none of them.

### C12 — Orphaned-child forensics and post-terminal prompt-file posture are unpinned

- **dimension:** architecture
- **severity:** minor
- **locus:** brief decisions 6/8 vs spec §18.3 (resume: anything without a valid
  contract is `Failed{orphaned_by_restart}`) and the prime precedent
  (`handle.json` sibling + RES-D4 orphan sweep by deterministic invocation name).

**Reasoning.** If the runtime process dies between invoke and poll, a real CLI child
keeps running, reparented, with no registry any resume can search (prime's daemon
answers `list`; `init` does not). Cheap design pin now, while the invocation-directory
layout is being decided: record the child PID + invocation identity as a forensic
sibling file (prime's `handle.json` analog), and pin what happens to the prompt file
and directory after terminal states (retained for forensics is the consistent answer;
§10.3 rule 5's preserve-for-forensics posture). Cancel idempotence across the seam
(SIGTERM → bounded wait → SIGKILL; second cancel = typed no-op; kill-after-exit maps
to success, as prime's `stop_child` does) and non-blocking reaping via `try_wait` in
poll are implied by the port but worth one sentence each in the design so the
transport seam's contract is testable. Prereg P3 anticipates exactly this class; the
brief should not leave it to the falsifier to discover unpinned.

---

## Checked and found sound (no residual)

- **Decision 5's direction (exit status never completion evidence).** Spec-conform:
  §10.3 rule 3 is a NECESSARY condition ("only when"); rule 2 makes exit codes an
  optional SIGNAL; §10.5 pins "mailbox is a signal; the file is the contract".
  Nonzero exit + valid matching contract ⇒ Completed has no contradicting rule, and
  prereg D2 records it correctly. Only the recording locus was unpinned (C8b).
- **Envelope-digest stability at render time.** The digest is computed by the
  runtime's envelope builder and carried immutably on the spec; the adapter echoes
  the carried value and never recomputes (port docs; prime passes
  `spec.context_envelope.digest().to_string()`). Stable.
- **`AdmissionRefused` for missing `GAUNTLET_CLI_AGENT_CMD`.** Correct port class:
  `AgentError::AdmissionRefused`'s own doc names "runner not configured" (agent_runner.rs:40-41);
  a doctor-only resting place would leave admission unprotected.
- **Shape-2 implementability through the existing `poll` signature.** `poll` takes
  only the handle and the spec carries the deadline; the adapter memoizes the spec at
  invoke (prime precedent), so no signature change and no adapter-foreign semantics
  are needed for either shape — placement of the new CASE is the problem (C1), not
  the signature.
- **ProbeBudget semantics for a clean-exit double.** `ProbeBudget::INSTANT` works:
  an in-memory double reports exit state on first poll; no real-time waiting is
  introduced by the new case.
- **R-1b fence.** Sound: suite over process doubles; the env-gated live leg's trivial
  probe child (shell script parsing prompt-file markers, writing a valid contract; no
  Unit, no gate, no model) mirrors the task15 precedent exactly
  (`GAUNTLET_PRIME_LIVE_PROBE` gating in `tests/live_probe.rs`) and does not approach
  the "first real builder execution" trigger.
- **`--adapter` → env-var deviation (decision 10).** Acceptable and properly recorded
  (prereg P5 anticipated the plan's flag was unimplementable as written; the plan is
  subordinate to the spec and the deviation is test-harness-local).
- **Locator precedent (decision 10 / D5).** `crates/gauntlet-cli/tests/` is the
  established proof location (p1–p7, p11, p12, g6/g7/g10 all live there; repo-root
  `tests/proofs/` is empty); the delivered filename matches the precedent. Only the
  candidates-entry bookkeeping was missing (C10).
- **Frozen artifacts.** Nothing in the brief touches `.wayfinder/answer-key.md`,
  `.gauntlet/reference.lock`, the frozen prereg, or the shared suite's existing case
  bodies (additive-only claim is consistent with the byte-identity discipline of
  tasks 13/15 — keep the `git diff gauntlet-ports` empty-diff evidence for existing
  case bodies at closure).
- **Naming.** `RunnerScript` variant, `InvocationFailure::MissingResult` (token
  `missing_result`), capability-named public items, `RUNNER_TOKEN = "cli-agent"`
  pattern — all consistent with §1 vocabulary and §24.3; no vendor names anywhere in
  the planned surface.

## Verdict basis

NOT_CONFIRM: C1 is a blocking design contradiction (the additive suite case as placed
is unsatisfiable by the settled prime adapter, and the brief's "prime stays green,
tests-only" resolution is not implementable), and C2–C6 are unresolved majors against
the design as briefed. All findings are design-level and correctable by adjudication
before or mid-flight at one-message cost — the report deliberately includes the
reconciliation options where the choice is the orchestrator's (C1 option iii, C5
whole-crate scan, C6 descriptor extension), not patches.
