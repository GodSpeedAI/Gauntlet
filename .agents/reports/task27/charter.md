# Task 27 charter — Final conformance and handoff

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 27 (phase 7; the FINAL task).
**Ruler units in direct scope:** AK-067 (handoff six categories; frozen
locator `.agents/reports/handoff.md`, kind `file`) and AK-068 (§25
documentation set; frozen locator `tests/contract/docs_present`, kind
`test_report`). The sweep itself dispositions ALL 70 units including
AK-069 (baselines.json, recorded) and AK-070 (mutation.json, recorded).
**OUT OF SCOPE:** the operator's TUI workstream
(`workbench_document_review` in CURRENT_STATUS; `.agents/specs/
gauntlet-tui-prime-workbench.*`, its plan/reports) — the operator's,
never this task's. **Prior tasks are settled; no re-litigation.**

**Plan steps (verbatim):**

> 1. Run `gauntlet verify --all` against the frozen answer key; produce a
>    per-Unit disposition with evidence refs.
> 2. Write `docs/CONFORMANCE.md`: trust posture, sandbox backends and tier
>    mapping, which profile (Core/Extended/Self-improving) is met, and every
>    advisory-only enforcement.
> 3. Write `.agents/reports/handoff.md` with the six status categories; list
>    every `INCONCLUSIVE` Unit and why.
> 4. Ensure documentation set from spec §25 exists and no document claims a
>    status ahead of a passing test.
> 5. Export the run (`gauntlet export`) and pin the evidence bundle.

**Plan gate (verbatim — must run unchanged from the repo root):**

```bash
cargo run -q -p gauntlet-cli -- verify --all --report .agents/reports/conformance.json && \
python3 -c "import json;d=json.load(open('.agents/reports/conformance.json'));\
assert all(u['outcome'] in ('PASS','INCONCLUSIVE','FAIL') for u in d['units']);\
assert not [u for u in d['units'] if u['outcome']=='PASS' and not u['evidence_ref']]" && \
just ci && just ruler
```

**Plan Done-when (verbatim):** no Unit is `PASS` without a resolvable
evidence ref; the handoff report's categories match the conformance JSON;
`just ruler` still passes, proving the ruler never moved across the entire
build.

**Redesign trigger (verbatim):** none plausible. If this gate cannot pass,
report honestly which Units are `INCONCLUSIVE` and why — never adjust the
gate.

## Locator reconciliation (decided now)

- `verify --all --report .agents/reports/conformance.json` — plan spelling,
  achievable verbatim; extend the existing `verify` subcommand.
- `.agents/reports/handoff.md` (AK-067) — achievable verbatim.
- `tests/contract/docs_present` (AK-068) — achievable VERBATIM by the
  AK-001 precedent: a `[[test]]` target named `docs_present` of
  `gauntlet-arch-tests` with `path = "../../tests/contract/docs_present.rs"`
  (the crate already does exactly this for architecture_boundaries).
  **No divergence entry is owed for AK-068.**
- `.agents/reports/baselines.json` (AK-069) and
  `.agents/reports/mutation.json` (AK-070) — already at their frozen
  locators from tasks 26/24.
- Every OTHER unit whose frozen locator diverges from its real evidence
  target keeps its recorded `reference-candidates.md` citation in the
  manifest (see schema below); NEW divergences discovered while building
  the manifest are reported by B1 in its result file and written to
  reference-candidates.md by B3 (single writer).

## Orchestrator-owned design decisions (fog resolutions, binding)

1. **The sweep is a read-only instrument surface.** `gauntlet verify --all`
   never settles, never writes state, never dispatches agents. The existing
   `--run` mode is unchanged; `--all` and `--run` are mutually exclusive
   (usage error, exit 2). New application module `gauntlet_app::conformance`
   owns parsing/manifest/adjudication/report-assembly over an injectable
   `ConformanceExecutor` seam (the task-23/26 precedent); the CLI wires, it
   does not decide.
2. **The conformance manifest** (authored input, committed):
   `.agents/reports/conformance-manifest.yaml`, schema
   `gauntlet.conformance-manifest@v1`. One entry per ruler unit — ALL 70,
   set-equal with the parsed frozen answer key both directions or the sweep
   REFUSES (no report written, non-zero exit). Entry shape:

   ```yaml
   - id: AK-001
     kind: test_report          # MUST equal the unit's evidence_required kind; mismatch => refuse
     frozen_locator: tests/contract/architecture_boundaries
     evidence:
       execute: "cargo nextest run -p gauntlet-arch-tests --test architecture_boundaries"
       # OR, for kind file/metric:
       artifact: { path: .agents/reports/handoff.md, predicate: handoff_six_categories }
       # OR (unit with no mechanical basis today):
       none: { reason: "..." }
     bounded: false             # true => PASS adjudicates implemented_but_provisional
     status_hint: none          # non-PASS only: configured_but_untested|proposed|unknown
     basis: "one line: what actually runs/checks"
     divergence: none           # or "reference-candidates 2026-08-17" (the recorded entry)
   ```

   `execute` strings are NOT free-form shell: they are a closed command
   vocabulary (cargo nextest invocations) parsed and validated fail-closed;
   `file`/`metric` kinds use NAMED typed predicates in app code
   (e.g. `handoff_six_categories`, `baselines_keys_abcd`, `mutation_record_valid`),
   never arbitrary shell. Identical `execute` commands are grouped and run
   ONCE; results fan out to their units.
3. **Disposition semantics (fail-closed everywhere):**
   - `test_report` → execute; exit 0 ⇒ PASS; non-zero ⇒ FAIL.
   - `file`/`metric` → artifact exists AND named predicate holds ⇒ PASS;
     artifact absent ⇒ INCONCLUSIVE(`no_artifact`); predicate refutes ⇒ FAIL.
   - `evidence: none` ⇒ INCONCLUSIVE with the recorded reason (never PASS).
   - Executor cannot run a command (environmental, typed) ⇒ INCONCLUSIVE
     with that reason — never silently PASS, never FAIL-by-default.
   - ANY FAIL ⇒ sweep exits non-zero (the gate then cannot pass — correct);
     INCONCLUSIVE alone ⇒ exit 0 (honest reporting per the redesign trigger).
   - PASS is structurally impossible without a non-empty, on-disk,
     repo-relative `evidence_ref` (report assembly refuses otherwise —
     make the invalid state unrepresentable, then test the refusal).
4. **Evidence capture:** every executed command's stdout/stderr lands under
   `.agents/reports/conformance-evidence/<UTC-timestamp>/NN-<slug>.log`
   alongside a `MANIFEST` (command, exit, duration, sha256 per log).
   `evidence_ref` for executed units = that log path; for artifacts = the
   artifact path. The sweep writes ONLY the report + the evidence dir.
5. **conformance.json schema (frozen; `gauntlet.conformance@v1`):**

   ```json
   {
     "schema": "gauntlet.conformance@v1",
     "generated_at": "<RFC3339>",
     "tree": {"commit": "<head sha at sweep>"},
     "answer_key": {"path": ".wayfinder/answer-key.md", "sha256": "<hex>"},
     "manifest": {"path": ".agents/reports/conformance-manifest.yaml", "sha256": "<hex>"},
     "units": [{
       "id": "AK-001",
       "outcome": "PASS|INCONCLUSIVE|FAIL",
       "status_category": "implemented_and_proven|implemented_but_provisional|configured_but_untested|proposed|blocked|unknown",
       "evidence_kind": "test_report|file|metric",
       "frozen_locator": "<verbatim from the ruler>",
       "real_locator": "<from the manifest>",
       "divergence": "none|reference-candidates <date>",
       "evidence_ref": "<non-empty when PASS>",
       "basis": "<one line>",
       "reason": "<required non-empty when outcome != PASS>"
     }],
     "summary": {"total": 70, "pass": 0, "inconclusive": 0, "fail": 0},
     "_meta": {"tool": "...", "commands_executed": 0, "disclosures": ["..."]}
   }
   ```

   The `status_category` vocabulary is §26's six states VERBATIM; the
   mapping is mechanical: PASS+unbounded ⇒ `implemented_and_proven`;
   PASS+bounded ⇒ `implemented_but_provisional`; INCONCLUSIVE ⇒ the
   entry's `status_hint` (validated); FAIL ⇒ `blocked`. The python gate's
   two asserts must hold by construction.
6. **Command vocabulary honesty:** per-target `cargo nextest` invocations
   have no justfile equivalent (the recipes are package/workspace scoped);
   the sweep uses explicit nextest target filters under the repo's
   configured nextest parallelism — the scripts/run-mutation.sh precedent
   (raw tool where no recipe fits), recorded in `_meta`. All cargo
   invocations run under the repo compile-lock discipline. The sweep NEVER
   raises build/test parallelism (WSL memory rule).
7. **The §25 documentation set is ADDITIVE and uppercase:** the eight
   files `docs/ARCHITECTURE.md`, `docs/CONFORMANCE.md`,
   `docs/CONFIGURATION.md`, `docs/SECURITY.md`, `docs/FAILURE-RECOVERY.md`,
   `docs/OPERATOR-GUIDE.md`, `docs/SELF-IMPROVEMENT.md`,
   `docs/HANDOFF.md` (spec §25 spelling). The operator's existing lowercase
   docs tree is NEVER modified, moved, or deleted — no competing source of
   truth is created: per-unit outcome truth lives ONLY in
   `conformance.json` + `.agents/reports/handoff.md`; the eight docs state
   capability prose and REFERENCE those artifacts. `docs/HANDOFF.md` is a
   thin pointer (six-category summary table + canonical locator), never a
   second handoff. Each of the eight carries a "Proven vs provisional"
   section whose unit-level claims are machine-checkable mentions (an
   `AK-NNN` id adjacent to a status word must match conformance.json).
8. **`docs/CONFORMANCE.md` must contain (binding content list):** trust
   posture (kernel is a control environment, not a security sandbox;
   authority gates whether something runs, containment gates how
   contained; honest capability reporting); sandbox backends and the §17.3
   tier mapping (bwrap-only host; artifact-declared substitutable backend;
   shipped default `none` posture refuses rather than degrades); the §24.1
   profile adjudication (Core / Extended / Self-improving — each item met,
   partial, or absent, with its conformance.json backing); EVERY
   advisory-only enforcement enumerated with its trigger (at minimum: G9
   critic write-scope advisory; F6 cli-agent run_id echo informational;
   the recorded structural boundaries from task26 reference-candidates 6);
   and **UNK-001's terminal disposition** (backend is an
   artifact-declared, substitutable choice; this host is bwrap-only at an
   honest Isolation::Process rung; the Annex-F expectation of
   microVM/container grade for typical cross_cutting use vs the delivered
   process grade is stated as a weakening; no production deployment target
   is named anywhere in the tree; the frozen manifest is never edited to
   record this — cite
   `.agents/reports/task14/structural-review-unk001.md`).
9. **handoff.md:** the frozen AK-067 locator; six §26 categories; EVERY
   INCONCLUSIVE unit named with its reason; category counts MATCH
   conformance.json mechanically (a test asserts it). **UNK-003's terminal
   disposition** (runners.default remains the operator's standing decision;
   the repo-root GAUNTLET.md deliberately names none; fixture-level
   delegation is the recorded pattern — cite task20/25/26) and **UNK-002's
   terminal disposition** (LICENSE: the operator's call, still unknown)
   are stated in handoff.md's unknown section.
10. **Export + pinned bundle (step 5), reconciled honestly:** `gauntlet
    export` is run-bound (`--run <run-id>`, fails on empty history) and the
    repo root has NO durable state store — the build's own runs lived in
    scratch worlds (the recorded task20+ dogfooding posture). B3 verifies
    this fact by execution, records it verbatim in handoff.md's disclosures
    (the plan's "export the run" step lands as: no repo-level run exists to
    export; the honest export of THIS build is its evidence tree), and the
    pinned bundle IS the canonical export: a committed
    `.agents/reports/evidence-bundle.sha256` over conformance.json, the
    conformance-evidence dir contents, handoff.md, baselines.json, and
    mutation.json — verifiable with `sha256sum -c`. Delivered as a small
    boring script `scripts/pin-evidence-bundle.sh` + executed once; digests
    recorded in the closure. If a durable run DOES exist in the repo store
    at B3 time (verified, not assumed), export it with the real command into
    the evidence dir and include it in the pin.
11. **No fresh mutation re-run at this task** (orchestrator decision,
    recorded): AK-070's evidence basis is the recorded task-24 settlement
    artifact (dated, manifest-pinned) adjudicated `implemented_but_
    provisional` (bounded: true) with the basis line saying exactly that.
    The full-battery green on the final tree is `just ci`'s claim; the
    teeth defensibility is the recorded 73/63/10/missed-0 run. Nothing
    here weakens AGENTS.md's expensive-evidence policy: this IS the
    recorded-artifact posture, disclosed.
12. **Committed conformance.json lifecycle:** the first real sweep runs in
    the ORCHESTRATOR's hands after B1 lands; its output is committed at
    that phase boundary (evidence-of-the-moment, honest at each commit)
    and re-generated at settlement so the committed report is the final
    tree's report. The sweep is deterministic in dispositions given the
    tree state (timestamps/durations aside).

## Slices (file-disjoint; result contracts under `.agents/reports/task27/`)

- **B1 — the conformance sweep (code + manifest)** — wave 1. Owns
  `crates/gauntlet-app/src/conformance/**` + the `lib.rs` module line,
  `crates/gauntlet-app/tests/conformance.rs`,
  `crates/gauntlet-cli/src/verify_cmd.rs` (+ lib.rs usage line if
  separate), `crates/gauntlet-cli/tests/conformance_cli.rs`,
  the `cli_help.txt` golden regen (documented procedure in
  cli_surface.rs), `.agents/reports/conformance-manifest.yaml` (ALL 70
  entries, derived from the frozen ruler + recorded reference-candidates
  real locators + task closures; report any NEW divergence in
  `b1-report.md`, do not write reference-candidates.md), and the
  `report_manifest.rs` verify row reconciliation if it enumerates args.
  RED-first unit tests over a FIXTURE world (tiny fake answer key +
  manifest + echo/true/false commands in a temp dir) — never the real
  battery inside tests. The REAL sweep is the orchestrator's phase-gate
  action, not a test.
- **B2 — the §25 docs set + docs_present** — wave 1, parallel with B1.
  Owns the eight `docs/<UPPERCASE>.md` files,
  `tests/contract/docs_present.rs`, the `[[test]] docs_present` entry in
  `crates/gauntlet-arch-tests/Cargo.toml`, and (only if the crate pattern
  requires it) the minimal `gauntlet-arch-tests` lib surface for the
  doc checks. `docs_present` covers: all eight exist non-empty; the
  proven-vs-provisional section present in each; `docs/HANDOFF.md`
  pointer integrity (names the canonical handoff locator); the
  CONFORMANCE.md binding content list (decision 8) spot-checked
  mechanically (UNK-001 disposition present; profile section present;
  advisory section present). Teeth RED-first: delete a doc ⇒ RED; strip
  the status section ⇒ RED. The conformance-cross-check leg is B3's
  (wave 2) — B2 does NOT read conformance.json.
- **B3 — handoff + consistency + bundle** — wave 2, dispatched after the
  orchestrator commits the first real conformance.json. Owns
  `.agents/reports/handoff.md`, the handoff↔conformance consistency test
  + the docs status-claim cross-check leg (added to
  `tests/contract/docs_present.rs` or a sibling target in
  gauntlet-arch-tests — B3 coordinates the file, B2 is done by then),
  `scripts/pin-evidence-bundle.sh`, the `gauntlet export` execution
  artifacts, reference-candidates.md new entries (single writer), and the
  UNK-002/003 terminal dispositions in handoff.md.

## Battery, teeth, and discipline (all slices)

- RED-first where a test can exist before the mechanism; never weaken an
  assertion to reach green; INCONCLUSIVE is never PASS; three states
  stay three states.
- Task teeth (orchestrator re-verifies each personally: RED ⇒
  byte-identical sha256 restore ⇒ GREEN): **T1** manifest tooth (delete
  or damage one manifest entry ⇒ sweep refuses / unit goes INCONCLUSIVE,
  never silently PASS); **T2** evidence tooth (delete a committed PASS
  unit's evidence_ref artifact ⇒ that unit cannot stay PASS); **T3**
  docs tooth (remove one §25 doc ⇒ docs_present RED); **T4** handoff
  tooth (move a unit between categories in handoff.md ⇒ the consistency
  test RED).
- Every `flock` invocation is its OWN top-level shell command, never
  nested; one compile at a time repo-wide; `sccache --stop-server`
  INSIDE the same flock'd command; package-scoped checks before
  workspace ones; WSL memory is tight — never raise parallelism.
- Never `git restore`/`git checkout --` a tracked path carrying
  uncommitted work without `git status` + sha256 first. NEVER stage
  `.jolli/`, `.tmp/`, `.agents/prompts/`, `.agents/CURRENT_STATUS*`,
  the TUI workstream files, or operator-owned files.
- `just ruler` PASS at every phase boundary; `.wayfinder/answer-key.md`
  and every reference.lock are NEVER edited. The manifest cites
  divergences; it never moves the ruler.
- Progress checkpoints: each slice writes
  `.agents/reports/task27/<slice>-progress.md` as it goes and
  `<slice>-report.md` as its completion contract (a claim, carrying NO
  settlement authority). A builder's "done" without the report file is
  not done.
- Plan-gate watch item: `b1_contract_wins_over_exit7` (task25/26) is
  recorded load-flaky under full-workspace load — if it fails in a full
  battery, re-run in isolation before diagnosing.
