# Task 20 — blind design-as-built critic report

Critic: design-as-built, fresh context, blind (no builder progress/report/log
files read; commission read = AGENTS.md, charter.md, b1/b2/b2a/b3/cor-b3a
briefs, plan Task 20, spec §8.5/§11/§13.2/§14/§16/§17/§20/§21/§23, answer key
AK-060/061, reference-candidates.md, and the code). Review surface = the
uncommitted tree on HEAD (code, fixtures, manifests, tests; `.jolli/` and
report files excluded).

Method note: every claim below carries file:line evidence read from the tree.
Where the commission asks "classify", the classification is stated with its
reason. Where a record is required, I state what the record must say; I cannot
verify the task report's prose from a blind context, so "unrecorded" below
means "not recordable from the tree or charter alone".

---

## RES-01 — MAJOR — §20.1 surface is one subcommand short: `roles list` does not exist

- Evidence: `crates/gauntlet-cli/src/roles_cmd.rs:10-26` dispatches only
  `install` and `verify` (no `list` arm; usage prints no list line). Spec
  §20.1 (Core-spec.md:1185): `gauntlet roles       # install | verify | list`.
  The root usage (`src/lib.rs:93-94`) and the report manifest
  (`src/report_manifest.rs:25-81`) likewise carry roles at command level only.
  `skills` got its full trio (`skills_cmd.rs:48-53`); roles did not.
- Commission vs tree: plan Task 20 step 1 = "Implement the commands of spec
  §20.1"; commission item 1 = "full §20.1 command surface". Charter D8 says
  "roles (exists)" — the orchestrator's recorded depth decision — which is in
  tension with §20.1's enumerated `list`. AK-060's frozen claim is command-level
  and is satisfied, so this is not a ruler breach; it is a spec-surface shortfall
  under an ambiguous charter shorthand. I am recording the conflict rather than
  silently picking a side.
- Correction direction: add `roles list` over the existing registry/lock reads
  (trivial, read-only), or record a deliberate §20.1 deviation against D8 in the
  task record for the operator to re-adjudicate.

## RES-02 — MAJOR — parallel completions collide at the write-once per-(run,iteration) observation binding; the fixture's `max_parallel: 1` workaround is unrecorded

- Evidence: the drain loop processes every completed in-flight invocation in the
  same iteration (`controller.rs:2124-2141`), each completion binding ONE
  observation bundle at `(run_id, iteration_no)`
  (`controller.rs:2656-2661` → `ObservationBindingAuthority::bind`,
  `ports/src/producer_authority.rs:347-380`). The store's binding is write-once
  per (run, iteration): a DIFFERENT bundle ref for the same key is a typed
  refusal, never absorbed
  (`adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:2078-2093`).
  The scheduler plans up to `max_parallel` dispatches into one iteration
  (`controller.rs:1479-1480`, fan-out designed and truncation-tested at
  `controller.rs:7196`). Therefore with `max_parallel >= 2` and 2+ eligible
  units, the second completion's bind refuses → the error propagates out of
  `drain`/`drive` as a run-level error — NOT one of §11.6's bounded terminals.
  The §8.6 default is `max_parallel: 2` (Core-spec.md:604), so the shipped
  composed surface cannot run its own default parallelism over a multi-unit
  reference. The E2E fixture pins `max_parallel: 1`
  (`tests/fixtures/target/GAUNTLET.md` budget block) and neither the fixture
  prose, the fixture reference, nor any in-code record states why.
- Classification: DEFECT plus an unrecorded spec tension. The write-once rule
  itself is a sound integrity anchor; the defect is that the loop's observation
  model admits exactly one completion per iteration while dispatch admits N,
  and the collision surface is a raw store error instead of a bounded terminal.
- What the record must say: (a) the fixture GAUNTLET.md body must state that
  `max_parallel: 1` exists because the observation binding is write-once per
  (run, iteration); (b) the task record must surface the binding-granularity
  question — §12/§14 keyed per-iteration vs §11.2 parallel completions — as a
  ruler-owner decision (re-key the binding, or process one completion per
  iteration and treat parallelism as across-iteration), not a fixture detail.
- Correction direction: either sequence completion processing (one per
  iteration, the others re-planned) or re-key the binding per invocation —
  the first is loop-local; the second is a spec change via candidate revision.

## RES-03 — MAJOR — R-1b's behavioral half is unclosed: runners do NOT anchor builder execution at the admitted per-unit workspace

- Evidence: the controller allocates a fresh per-unit workspace and admits the
  dispatch against it (`controller.rs:8015-8095` teeth prove distinct refs at
  the gate/spec level). But the adapters anchor execution at the per-instance
  CANONICAL root, not `spec.workspace`: the mock applies its workspace effects
  under `self.workspace_root` — the root passed at construction
  (`adapters/gauntlet-adapter-agent-mock/src/runner.rs:279-288`, constructed
  with `state_dir.join(WORKSPACES_DIR_NAME)` at
  `crates/gauntlet-cli/src/compose.rs:289-292`); the generic CLI runner starts
  children with `cwd = self.workspace_root` and binds the template's `workspace`
  placeholder to the root
  (`adapters/gauntlet-adapter-agent-cli/src/runner.rs:362,391,406`; wired at
  `compose.rs:255-261`). The E2E's artifacts consequently sit at
  `workspaces/evidence/...` — directly under the shared root, no per-unit
  segment (`tests/e2e_full_loop_mock.rs:264-276`).
- Commission vs tree: the b2 brief's R-1b closure says the provider is wired
  "so per-unit workspaces are real in the composed path". They are real at
  ADMISSION and CLEANUP, vacuous at EXECUTION: §17.5.1 invariant 1 ("builder
  and critic processes run only inside their assigned workspace path") is not
  honored by either process-backed token the composition fronts. Exposure is
  bounded by the shipped `none`-backend/`contained`-only posture, and no
  corruption results — but the containment story the wiring implies is not the
  containment the execution path performs.
- Correction direction: anchors must follow `spec.workspace` (mock effects
  root, CLI runner cwd and template placeholder), or the record must scope
  R-1b's closure honestly to the gate/admission half and carry
  "runner execution anchor" as an open residual — with the E2E's containment
  wording corrected accordingly.

## RES-04 — MAJOR — teeth gap: the D2 fail-closed runner selection (and the mock scenario hook) have NO test

- Evidence: `select_runner_token`, the unknown-token refusal, and the
  mock-without-scenario refusal exist only in
  `crates/gauntlet-cli/src/compose.rs:186-219,267-293`. No test in the tree
  exercises any of them (`grep` over `crates/` for
  `select_runner_token|names no runner|no default runner|GAUNTLET_AGENT_RUNNER`
  in test files: zero hits). The E2E proves the happy selection works; nothing
  fails if the fail-closed refusal is removed or degraded to a default.
- Commission vs tree: charter D2 makes fail-closed selection a named,
  load-bearing governance mechanism; AGENTS.md and spec §23.1 make teeth a
  proof obligation ("disabling the defense makes the proof fail"). As built,
  the mechanism has no disabling-detection.
- Correction direction: three cheap library tests through
  `run_cli`/`compose_run`: unset token → typed configuration failure naming
  key+env; unknown token → same naming the fronted set; `mock-agent` without
  `GAUNTLET_MOCK_SCENARIO` → typed failure.

## RES-05 — MINOR — D3's composition-level live-wrap refusal is untested

- Evidence: `CompositionError::LiveContainmentDeferred`
  (`compose.rs:139,985-989`) has no test; the gate-level
  `wrap_builder_command` legs are tested (`p5_hard_gate.rs:1153-1193`), the
  composition's Sandboxed→refuse mapping is not. Fail-closed by inspection;
  unproven by instrument.
- Direction: one test composing a live-backend posture over the `cli-agent`
  token and asserting the typed refusal (can be a unit test of
  `wrap_checked_posture` with a stubbed env), or fold into RES-04's battery.

## RES-06 — MINOR — the `escalations resolve/retry` CLI glue is untested end-to-end

- Evidence: `escalations_cmd.rs:124-305` wires queue-read →
  `SettlementService::settle_operator_action` → queue-resolve-after-accept.
  The service's no-bypass half is well tested
  (`gauntlet-app/src/settle/mod.rs:3144-3240`, AK-062 service-level), but no
  test drives the CLI path: not the accepted resolve, not
  refusal-leaves-ticket-open, not the honest
  settle-committed-but-queue-write-failed branch (`escalations_cmd.rs:283-289`).
  Replacing `settle_operator_action` with a bare queue write would fail no
  test.
- Direction: one composed test: escalate via the adapter, `resolve` without
  PASS evidence → non-zero, ticket still open; with earned PASS → success,
  ticket resolved, one settlement row (D9's single-writer visible).

## RES-07 — MINOR — the dry-run zero-write tooth compares the file SET, not content

- Evidence: `e2e_full_loop_mock.rs:466-500` snapshots relative PATHS only
  (`walk` pushes names; `assert_eq!(before, after)`). A dry-run path that
  overwrote existing bytes in place (e.g. touching the fixture copy's
  GAUNTLET.md) would pass. The write-free claim is otherwise well founded by
  trace: `run_doctor` (no write; `doctor/mod.rs:94` vs
  `run_doctor_and_write:101`), `verify_lock` (read-only,
  `reference_lock.rs:69-130`), `plan_preview` (in-memory meter, no store).
- Direction: hash file contents into the snapshot (cost: one pass over a tiny
  tree).

## RES-08 — MINOR — help tree incoherent with behavior in two places

- Evidence: root usage says `gauntlet status [--run <run-id>]`
  (`lib.rs:88`, frozen into `tests/cli_help.txt:12`) but omitting `--run` is a
  usage error (`state_cmd.rs:50-58`); root usage says
  `gauntlet inspect --run <run-id> [--iteration <n>]` (`lib.rs:89`) while the
  command honors `--unit`, `--verifications`, `--settlements`
  (`state_cmd.rs:196-217`). The snapshot froze the under-reported tree —
  committed per procedure, but the procedure froze a divergence.
- Direction: fix both usage lines, regenerate the golden deliberately with the
  change recorded.

## RES-09 — MINOR — doctor §8.5 check 3 reports a stale locus for the mock token

- Evidence: `measure_runner` fronts only prime/CLI probes
  (`context.rs:131-192`, returns `None` for `mock-agent`); check 3 then lands
  in `(true, None)` → `Unavailable` with reason "runner adapters land with
  plan Tasks 15–16" (`doctor/checks.rs:815-827`) — stale (they landed) and
  mislocating (the truth: the composition fronts no measurement for this
  token). Exit-OK, so the E2E passes; the record is honest about
  unavailability but names the wrong reason.
- Direction: either give the mock token a configuration probe (scenario-hook
  presence) or reword the `(true, None)` reason to the token-neutral "no
  measurement fronted for this runner token".

## RES-10 — MINOR — the diagnostic allowance mapping is policy living in the composition root

- Evidence: `diagnostic_allowance` (`compose.rs:302-317`) fixes the §15.2
  spending SHAPE (cycles × forward-share formula) in the CLI crate. The plan's
  redesign trigger: "If the composition root starts making policy decisions,
  push them into the application layer." The formula is documented and
  reserve-safe, but its shape is a policy decision, not a wiring fact.
- Direction: move the mapping into `gauntlet_app` (control/disposition or
  budget), leaving the CLI passing the config scalar through.

## RES-11 — INFORMATIONAL — the ProducePolicy "two-world produce seam" is a legitimate compatibility seam, not a competing source of truth — with a record obligation

- Judgment: `ProducePolicy` (`controller.rs:696-750`, produce step
  `controller.rs:2999-3170`) preserves §14.3's three-way binding in BOTH
  worlds; a mismatched composition fails loudly at verification (fail-closed);
  the default preserves the frozen p4/p7 proofs' world byte-for-byte; the CLI
  opt-in (`compose.rs:908`) is documented at the module head. That is a
  compatibility seam, not a second authority: neither world can settle on
  claim bytes alone, and the instrument identity on the production is the
  observation instrument, honestly recorded (`controller.rs:3092-3097`).
- Record obligation: the task record must state that the CONTENT semantics of
  `evidence_required.locator` are composition-bound (facet-document world vs
  produced-artifact world) so the ruler owner can adjudicate whether §14.3's
  "the locator matches evidence_required.locator" should pin one world.
  Also note the BindLocatorContent world ingests builder-produced bytes at the
  locator — the rung contract (not the ingestion) is what discriminates.

## RES-12 — INFORMATIONAL — mock token in the production composition; E2E verification depth bounded by the mock's expressiveness

- Per charter D2 the mock token is operator-selected, fail-closed on the
  scenario hook, and kept out of `.env.example`/Annex E lockstep with the
  recorded AK-019 reason (`compose.rs:101-102`,
  `adapters/gauntlet-adapter-agent-mock/src/lib.rs:33-45`). §23.2 lens: not
  agent-reachable (runner comes from operator config; `config_fingerprint` is
  recorded at run start), so not a live gaming surface in this tree.
  Separately: the fixture's `observation.facets: [filesystem]` +
  `test_report` rungs mean the happy-path settlement rests partly on
  builder-produced report content the test-report rung trusts, with the
  filesystem marker as the one real-instrument cross-check; the lying tooth
  discriminates present-and-wrong, not fabricated-and-clean (the mock's
  single-line effect parser cannot express the two-line artifact contracts —
  recorded in the E2E header and scenario comments). Fabrication-class
  strategies (G3/G12) remain Task 24's suite; acceptable for Task 20, worth
  the ruler owner's notice.
- Adjacent verification (COR-b3a item 3): the isolation-none model-critic
  refusal is honest fail-closed governance at
  `adapters/gauntlet-adapter-compare-model/src/lib.rs:216-218` (§13.2 fresh-
  context posture, typed detail); the fixture's `max_critics: 0` avoids the
  refusal rather than disabling any gate. No change warranted.

## RES-13 — INFORMATIONAL — the arch-suite deviations are correctly decided and correctly recorded

- Composition-seal widening (`gauntlet-arch-tests/src/deps.rs:878-930`):
  narrow (same crate, unscoped sections only), justified by the Task 11 note's
  anticipated production caller, fail-first shape preserved (moved to another
  member, since the cli edge is now legal), recorded in-code and in the
  CLI manifest comment (`gauntlet-cli/Cargo.toml` `composition` feature edge).
  Correct.
- Single-writer inventory extension
  (`gauntlet-arch-tests/src/single_writer.rs` diff): new CLI write sites
  inventoried with named guards; the three new answer-key-surface pins
  (compose/init/reference_cmd) keep the unguarded-write scan over the new
  files. Correct treatment — inventory, not exemption.
- `.gitignore` un-ignore of `tests/fixtures/target/`: narrow, commented,
  reverses exactly one wholesale swallow; nested `target/` dirs stay ignored.
  Correct.
- AK-060 locator divergence: recorded in
  `.agents/reports/reference-candidates.md:1322-1357` in the established
  AK-063/AK-065 class, with the correct additional architectural reason
  (exempting a product crate would weaken the scan guarantee) and the ruler
  untouched. The entry honestly self-records the golden-path doc nit; I
  confirm it at `cli_surface.rs:17` ("golden/cli_help.txt" vs the actual
  `include_str!("cli_help.txt")`). Residual doc nit, one line.

## RES-14 — INFORMATIONAL — E2E assertion quality: one vacuous assertion; one brittle-to-prose assertion

- `e2e_full_loop_mock.rs:393-396` asserts the lying report
  `contains("configured-but-untested")` — vacuous: every report JSON carries
  all six category tokens unconditionally
  (`gauntlet-app/src/report/mod.rs:249-261`). The tooth's real load is carried
  by the per-unit negative (`!contains(...implemented-and-proven")`),
  `settled=0`, and the export zero-settlement count — those hold.
- `e2e_full_loop_mock.rs:247-255` asserts exact report basis prose
  (`"basis\":\"settlement row names verification "`); a wording change in the
  report module breaks the E2E without any behavior change. Record-derived,
  but brittle. Prefer asserting unit→category pairs from parsed JSON.

## RES-15 — INFORMATIONAL — `is_wired`'s doc claims a reader it does not have

- `report_manifest.rs:101-103` says "the `improve` command's own help reads
  this"; nothing outside the manifest and the cli_surface test reads
  `is_wired`. The exit/manifest agreement is pinned by test
  (`cli_surface.rs:185-188`), not by construction. Fix the doc line or make
  `improve_cmd` actually consult it.

---

## What the tree does well (positive findings, for the record)

- Dry-run (D1) is genuinely write-free by trace and by a before/after E2E leg;
  it requires no runner and verifies the frozen lock via the typed
  `just ruler` twin (`run_cmd.rs:189-255`, `reference_lock.rs`).
- One settlement writer everywhere: operator actions go through
  `SettlementService::settle_operator_action`
  (`escalations_cmd.rs:273-280`); the queue write is post-accept bookkeeping;
  `verify` never settles (`verify_cmd.rs:185-188`); the report module is
  read-only and deterministic (sorted rows, no clock —
  `report/mod.rs:368-481`).
- D7's cohort guardrail is pinned in both compose sites
  (`controller.rs:793-821,1486-1514`) and the fixture reference carries no
  cohort key; the fixture lock digests verify against the repository's own
  `scripts/reference-digest.sh` (recomputed in this review: both match).
- D10 closed with an honest mutation tooth (call-counter sensor with a
  documented reason a kernel check is impossible in-process —
  `roles/install.rs:1190-1269`).
- The escalation adapter is durably built (temp → file fsync → rename → dir
  fsync, strict decode refusing unknown fields, write-once resolve —
  `escalation-fs/src/queue.rs:245-269,396-472`) and the escalation READ lives
  with the adapter, not the StateStore.
- Deny-set (R2-N1) is enforced before envelope construction in the governed
  chain (`compare/authority.rs` diff, `compare/envelope.rs` wiring) with a
  composed-path teeth test (`gauntlet-app/tests/compare_deny.rs`).
- The clock hook is wall-clock-identical when unset and typed-refusing on an
  unknown token (`compose.rs:714-728`); the id generator is production ULID
  entropy with typed construction failure (`id-entropy/src/lib.rs`).
- All six §26 category spellings, record-driven population, manifest-declared
  capability rows that can never upgrade a not-wired declaration
  (`report/mod.rs`, `report_manifest.rs`, `improve_cmd.rs` honest not-wired
  exit).
- The p4/p5/p6/p7/p11/g7/g10 test diffs are purely mechanical constructor
  updates (empty deny set, `workspaces: None`, `none_backend()`); no assertion
  weakened.

---

## VERDICT: NOT_CONFIRM (0 blocking / 4 major / 6 minor / 5 informational)
