# Gauntlet handoff report — final conformance (task 27)

This is the canonical handoff at the frozen AK-067 locator
(`.agents/reports/handoff.md`, `kind: file`, predicate
`handoff_six_categories`). Per-unit truth lives here and in
`.agents/reports/conformance.json` — nowhere else (`docs/HANDOFF.md` is
a thin pointer).

**Provenance:** the grouping below is derived mechanically from the
committed conformance sweep `.agents/reports/conformance.json`
(`gauntlet.conformance@v1`, generated `2026-09-07T21:19:36Z`, tree
`a98b6249f9ce5edcc4f6b0ae50fdfb9522838865`; summary 70 units = 69
PASS / 1 INCONCLUSIVE / 0 FAIL) — every unit in the category the
sweep's `status_category` field assigns it, counts included. The
`tests/contract/docs_present` target's consistency leg asserts this
equality mechanically at test time against whatever conformance.json is
committed, so this report can never silently drift from the sweep.

**Evidence-of-the-moment (honest window):** the sweep this report was
derived from ran BEFORE this file existed — that is why AK-067 below
sits in the proposed section with its `no_artifact` reason. The task's
settlement re-sweep re-adjudicates AK-067 against this file (the
six-category predicate is satisfied by the sections below) and the
committed conformance.json is regenerated at settlement; the
accompanying row move (AK-067 → implemented_and_proven, count bump) is
the anticipated, mechanical follow-up. The manifest's seventeen
`divergence` flags that were `unrecorded` at sweep time are already
flipped to dated `reference-candidates 2026-09-07` citations (the
entries live in `.agents/reports/reference-candidates.md`); the
settlement re-sweep's json will carry the dated spellings.

## implemented_and_proven — 65 units

Every unit below PASSed its executed or adjudicated evidence in the
sweep with no bounded basis recorded.

- AK-001 — the boundary suite at its verbatim locator (dependency direction + scope escapes)
- AK-002 — the denylist case of the same target (no adapter identifier in domain or app source)
- AK-003 — the domain values wire-enum suite (each fixed vocabulary a distinct typed enum) over gauntlet-domain
- AK-004 — the strict answer-key parser suite (unknown enum values are typed errors never coerced)
- AK-005 — the P12 composition-root proof (absent or stale evidence is INCONCLUSIVE at every layer)
- AK-006 — the parser suite (irreversible-without-human is a UnitInvalid block failure)
- AK-007 — the exhaustive tier x authority x verify_by coupling matrix over both construction paths
- AK-008 — the shared agent-runner suite on the real cli adapter (admission before completion)
- AK-009 — the shared suite (Completed only with a schema-valid result at the contract path)
- AK-010 — the shared suite (missing result is TimedOut never Completed)
- AK-011 — the shared suite (malformed result document is Failed malformed_result)
- AK-012 — the shared suite (foreign invocation id or stale digest is Failed contract_mismatch)
- AK-013 — the P8 proof (full unmodified suite over mock and cli through one loop body)
- AK-014 — the registry case at its verbatim locator (selection resolves from configuration only)
- AK-015 — the config resolve precedence suite (defaults below GAUNTLET.md below env below flags)
- AK-016 — the literal scanner at its verbatim locator (plus its vacuous-exemption warrant test)
- AK-017 — the doctor check suite (every preflight check; required failures exit non-zero)
- AK-018 — the secrets never-leaked case (indirection resolves; values never surface)
- AK-019 — the env example completeness target at its verbatim locator
- AK-020 — the empty-world resolution onto the conservative defaults table (config absent)
- AK-021 — the normalization determinism suite (one digest across spellings; edits move it)
- AK-022 — the P1 composed proof (frozen copy outside every builder-writable path)
- AK-023 — the P1 composed proof (builder mutation aborts the run with reference_mutated)
- AK-024 — the P1 composed proof (digest re-verified at boundaries and before settlement)
- AK-025 — the antigaming suite (the G2 reference-change tripwire among its nineteen tests)
- AK-026 — the P2 composed proof (allowlist envelope; six canaries absent from canonical bytes)
- AK-027 — the P2 teeth leg (a deny-listed field fails the blindness proof and names itself)
- AK-028 — the observation assembly cross-checks (facet provenance versus stored evidence)
- AK-029 — the P3 composed proof (claims facet excluded from verification inputs by construction)
- AK-030 — the G6 proof (failures populate both gap arrays; hidden edits are tamper-evident)
- AK-031 — the P11 composed proof (comparators produce residuals only; nothing settles on them)
- AK-032 — the compare suite (deterministic aggregation; blocking residual forces fail; no model)
- AK-033 — the review-mode suite (deterministic probes first; model critics only where undecided)
- AK-034 — the review-mode suite (budget-truncated dimensions recorded as unavailable evidence)
- AK-035 — the P12 composed leg (the deterministic rung decides; zero model dispatches recorded)
- AK-036 — the P12 binding classes (absent stale and foreign evidence each refuse distinctly)
- AK-037 — the P4 composed proof (correcting records name rejected alternatives with reasons)
- AK-038 — the P4 composed loop (injected defect exposed compared and closed by routed correction)
- AK-039 — the P4 diagnosis leg (recurring residual routes to bounded diagnosis; exhaustion is INCONCLUSIVE)
- AK-040 — the settlement precondition matrix (each refused condition a distinct typed refusal)
- AK-041 — the P7 commit_order leg (durable event before any status write or in-memory advance)
- AK-042 — the P7 crash legs (kill mid-settlement; resume completes exactly once; evidence kept)
- AK-043 — the G10 composed legs (builder writes refused by containment; tamper aborts pinned)
- AK-044 — the P5 leg (irreversible never dispatches; zero durable invocations; queue ticket)
- AK-045 — the P5 merit-vs-conjunct leg (one violated hard requirement refuses outright)
- AK-046 — the G7 legs (agent lattice moves refused both directions; mid-run downgrade aborts)
- AK-047 — the P5 escalation leg (failed validation escalates; no unsandboxed fallback)
- AK-048 — the P5 wrap leg (containment from process start; confined session over shipped policies)
- AK-049 — the doctor check-5 suite (irreversible-tier policy fails the run; removal is green)
- AK-050 — the P5 capability leg (isolation-none runner refused above contained)
- AK-051 — the P6 proof (terminal at the declared budget with the recovery reserve unspent)
- AK-052 — the P6 meter legs (reserve-consuming dispatch refused and booked as nothing)
- AK-054 — the composed export stack (double export byte-identical and canonical)
- AK-055 — the P10 proof (one run never promotes; identical variation refused; revoke sticky)
- AK-056 — the P9 proof (weakening candidates rejected with the reason as its own tooth)
- AK-057 — the promotion suite (optimizer output is a candidate revision only; rollback valid)
- AK-058 — the antigaming suite (sixteen strategy tests over real adapters; manifest validator)
- AK-059 — fixture playback in the mock adapter (the frozen locator holds the scenario fixtures)
- AK-060 — the surface snapshot and dispatch smokes over the REAL binary
- AK-061 — the mock full loop through the CLI-only default build (task-21 re-evidence)
- AK-062 — the operator-action proof under the surfaces config (the whole target is feature-gated; the default config compiles zero of its tests)
- AK-063 — the roles suite (install from repo files; drift reported in every direction)
- AK-064 — the wayfinder-gauntlet conformance suite (provenance; semantics preserved)
- AK-065 — the formation fog invariant (fog recorded; no fog entry reaches the answer key)
- AK-068 — the docs-present teeth (eight documents exist; proven-vs-provisional section present; the arch-target precedent)

## implemented_but_provisional — 4 units

Each PASSed with a recorded bounded basis (`bounded: true` in the
conformance manifest) — the basis lines quoted verbatim from
conformance.json in the Disclosures section below.

- AK-053 — controller checkpoint and resume fidelity (§18.3 steps 4–5 absent; recorded RES-A4)
- AK-066 — the wrap-and-settle leg (recovery and budget legs corroborate; settleability tension recorded)
- AK-069 — the recorded task-26 settlement artifact (measured on the task-26 tree; D-does-not-win included)
- AK-070 — the recorded task-24 settlement artifact (73 examined 63 caught 10 unviable missed 0)

## configured_but_untested — 0 units

None. Every mechanism the repository configures was dispositioned by
the sweep — executed evidence (proven) or adjudicated recorded
artifacts with a disclosed bounded basis (provisional). No unit landed
between: nothing is configured and merely asserted.

## proposed — 1 unit

- AK-067 — INCONCLUSIVE (no_artifact): the sweep's reason, verbatim:
  "no_artifact: .agents/reports/handoff.md is absent". This file is
  that artifact; it did not exist when the derived-from sweep ran (see
  the evidence-of-the-moment note above — the settlement re-sweep
  re-adjudicates it against this file's six-category predicate).

## blocked — 0 units

None. The sweep recorded zero FAIL — no unit's evidence refuted its
claim on the final tree.

## unknown — 0 units

No ruler unit landed in unknown. Two standing operator questions are
recorded here (fog dispositions, not units — they carry no AK id):

- **UNK-002 — LICENSE (terminal disposition): whether this repository
  should carry a LICENSE, and which, is the operator's call and is
  STILL UNKNOWN — status honestly `unknown`.** No licence was invented
  by the build; none is asserted here.
- **UNK-003 — `runners.default` (terminal disposition): which concrete
  runner `runners.default` should name remains the operator's standing
  decision.** The repository root's GAUNTLET.md deliberately names
  none; fixture-level delegation of the runner choice is the recorded
  pattern (task 20's scratch-world E2E fixtures, task 25's real-runner
  E2E legs, task 26's baseline gadget child — each run declared its
  own runner, none promoted to the repo default). Nothing in this
  build narrows that decision.

## INCONCLUSIVE units and reasons (summary)

Exactly one: **AK-067** — `no_artifact: .agents/reports/handoff.md is
absent` at the derived-from sweep. Missing evidence is not passing
evidence; the artifact now exists (this file) and the settlement
re-sweep decides it.

## Disclosures

1. **No repo-level durable run exists to export (verified by
   execution, not assumed).** `gauntlet export` is run-bound
   (`--run <run-id>`); the durable state store's default location
   resolves under the host scratch area (`/tmp/gauntlet/state/`),
   OUTSIDE the repository, and is currently empty; the repository root
   carries no state store at all (`.gauntlet/` holds the policies, the
   comparison contract, and the frozen reference lock only). The
   build's runs lived in scratch worlds — the recorded task-20+
   dogfooding posture. The honest export of THIS build is its evidence
   tree: the pinned evidence bundle
   `.agents/reports/evidence-bundle.sha256` (written by
   `scripts/pin-evidence-bundle.sh`, verifiable from the repo root
   with `sha256sum -c`) over conformance.json, the sweep's
   conformance-evidence directory, this handoff, baselines.json, and
   mutation.json IS the canonical export.
2. **The four bounded units' bases (quoted verbatim from
   conformance.json):** AK-053 — "controller checkpoint and resume
   fidelity (18.3 steps 4-5 absent; recorded RES-A4)"; AK-066 — "the
   wrap-and-settle leg (recovery and budget legs corroborate;
   settleability tension recorded)"; AK-069 — "the recorded task-26
   settlement artifact (measured on the task-26 tree; D-does-not-win
   included)"; AK-070 — "the recorded task-24 settlement artifact (73
   examined 63 caught 10 unviable missed 0)".
3. **Mutation-record basis:** AK-070's evidence is the DATED task-24
   settlement artifact (`mutation.json`, manifest-pinned) — mutation
   testing was NOT re-executed on the final tree (task-27 charter
   decision 11, the recorded-artifact posture). The full-battery green
   on the final tree is `just ci`'s claim; the teeth defensibility is
   the recorded 73/63/10/missed-0 run.
4. **Divergences and the ruler:** 61 of the 70 units' frozen locators
   diverge from their real evidence targets, every one now carrying a
   dated `.agents/reports/reference-candidates.md` citation (44
   recorded across tasks 5–26; 17 recorded 2026-09-07 by this wave);
   9 sit at their verbatim locators. The ruler itself never moved —
   `just ruler` PASSed at every phase boundary of this task.
5. **Per-unit truth is single-sourced:** the eight §25 documents state
   capability prose and reference this report and conformance.json;
   their unit-level claim lines are mechanically kept at or behind the
   sweep's adjudication by the `docs_present` target's cross-check
   leg.
