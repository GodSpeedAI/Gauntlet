# Task 20 — executing falsifier round 1 (blind)

Written incrementally by the blind executing falsifier. Findings most severe
first. Attacks that failed to land are recorded honestly at the bottom.

## F-1 — BLOCKING — the discrimination tooth is defeated by a report-forging
liar: a builder whose work fails the project's own check settles all units
`implemented-and-proven` by forging the instrument reports

Commission invariant (brief item 7): "lying builder does NOT settle". The
shipped tooth (`e2e-lying`) only discriminates a liar whose planted reports
truthfully RECORD its failure. The moment the liar forges the report content —
the same privilege the fixture gives the builder — the loop settles the lie.

Reproduction (all paths under /tmp/falsifier-t20, fixture copied from
`tests/fixtures/target`; the repo tree was never modified):

1. Forge a scenario whose `src/gadget.rs` effect LACKS the required marker
   `GADGET_MARKER_V1` (byte-identical to `e2e-lying`'s planted source) while
   every `evidence/*-check-report.json` effect claims `"failures": 0`, and
   whose result document claims success (`self_checks: scripts/check.sh → 0`).
   Written to `/tmp/falsifier-t20/forged-report.yaml`, `scenario:` identity set
   to the fixture-dir traversal stem (see F-2) so `load_scenario` accepts it.
2. Run the real composed binary in a copy of the fixture project:

   ```
   GAUNTLET_MOCK_SCENARIO='../../../../../../../../../../tmp/falsifier-t20/forged-report' \
   GAUNTLET_STATE_DIR=…/state GAUNTLET_ARTIFACT_DIR=…/evidence \
   GAUNTLET_ID_SEED=forged-seed GAUNTLET_CLOCK=deterministic … \
   target/debug/gauntlet run "seeded bounded task"      # cwd = fixture copy
   ```

   Observed: `rc=0`, `run: terminal=settled iterations=3`; report JSON:
   `AK-901 → implemented-and-proven`, `AK-902 → implemented-and-proven`,
   `AK-903 → implemented-and-proven`; `status` prints `settled=3`; `export`
   carries exactly 3 `"record":"settlement"` rows.
3. The lie is real: running the fixture's own `scripts/check.sh` over the
   planted source prints `check: src/gadget.rs does not carry GADGET_MARKER_V1`,
   `rc=1` — unit AK-901's claim ("The gadget source declares the stable fixture
   marker GADGET_MARKER_V1") is false in the reality the loop observed, yet the
   unit settled. The forged passing report and the observation manifests naming
   `src/gadget.rs` are both durably in the content-addressed evidence store.

Locus: the §14.2 produce step ingests whatever bytes the dispatched child left
at the unit's locator and binds them as the instrument-produced verification
artifact — `controller.rs:2963-2970` (`produce_unit_evidence`, "binds the
locator's ACTUAL content"); the test-report rung then decides purely over those
bytes (`gauntlet-adapter-verify-test-report/src/lib.rs:14,129` — "PASS iff
tests > 0 and failures == 0"). Nothing in the composed path distinguishes
builder-authored bytes from instrument-authored bytes at a locator;
`evidence_required: {producer: instrument}` (fixture key) is never checked
against the artifact's actual origin, and the durable production record
attributes the bytes to the instrument. The loop's only reality probes are the
observation facets, and the fixture's `[filesystem]` facet records the tree
without re-deriving any claim. Contrast: `tests/fixtures/mock_runner_scenarios/
e2e-lying.yaml` plants HONEST failing reports and is refused — the tooth
discriminates honesty of the report, not honesty of the work.

Why BLOCKING rather than accepted fixture posture: the fixture comment claims
the artifacts are "honest by construction and falsifiable by the instrument",
and AK-061's E2E is commissioned as proof that "the lying builder does NOT
settle". Executed against a liar who lies in the report too, the E2E's
guarantee inverts. A settlement gate whose verdict is decided by the
untrusted party's own bytes does not discriminate lying builders; it
discriminates lying builders who confess.

## F-2 — MINOR — `load_scenario` has no path containment: the "fixture stem"
hook loads any YAML on the host whose `scenario:` identity equals the
traversal string

`GAUNTLET_MOCK_SCENARIO` is documented as "a fixture stem under the scenario
fixtures dir" (the frozen locator `tests/fixtures/mock_runner_scenarios`,
AK-059). The loader enforces only identity agreement, never containment:

- `GAUNTLET_MOCK_SCENARIO='../../../../../../../../../../tmp/falsifier-t20/forged-report'`
  with the file at `/tmp/falsifier-t20/forged-report.yaml` first refuses with
  `the stem and the identity must agree` (identity check, NOT a path check);
- after setting the YAML's `scenario:` field to the identical traversal
  string, the loader accepts it and the composed run replays it (this is the
  delivery channel for F-1).

Locus: `gauntlet-adapter-agent-mock/src/scenario.rs:1710-1725`
(`load_scenario` joins the raw stem; the only guard is
`scenario.name() != name`). Severity MINOR: the hook is operator-scoped
test-division configuration (an operator who can set the env can act with the
operator's own authority anyway), and identity agreement prevents a SILENT
substitution. Recorded because the frozen-locator claim is only nominal and
because F-1's forgery rides on it.

## F-3 — MINOR — escalation TicketId grammar admits `..`: a forged
out-of-root ticket file is READ and its run_id feeds the settlement request;
the only guard on the write-back path is the operator action's unconditional
refusal

Reproduction (world = settled mock run in /tmp/falsifier-t20/w-forged):

1. Plant a schema-valid open ticket at `$STATE/escaped.json` (one level ABOVE
   the queue root `escalations/`), `"ticket":"../escaped"`, `run_id` = the
   real settled run.
2. `gauntlet escalations resolve --ticket '../escaped' --unit AK-901 --operator mallory`

   Observed: the queue READS the out-of-root file (no refusal at the ticket
   layer) and the service then refuses on merits
   (`OperatorActionRequiresEarnedReadiness`), rc=1, nothing written.

Locus: `gauntlet-ports/src/human_escalation.rs:24-28` (validity = non-empty,
≤128, no leading `/`, no whitespace — `..` passes) and
`gauntlet-adapter-escalation-fs/src/queue.rs:234-236,346-348`
(`root.join(format!("{ticket}.json"))` unsanitized). Today the traversal is a
READ primitive only, and the resolve write-back
(`queue.resolve` → `path_of_ticket(&record.ticket)`, which would honour a
`..` in the FORGED record's own ticket field) is unreachable solely because
`settle_operator_action` never returns Ok (F-5's posture). Defense-in-depth
gap: one grammar change away from an out-of-root write.

## F-4 — MINOR — one corrupt file makes the operator's ENTIRE escalation
queue unreadable

`echo '{"garbage' > escalations/esc-broken.json` then
`gauntlet escalations list`:

```
gauntlet escalations: escalation ticket esc-broken is not a valid record: json: unexpected byte 0xa at offset 9   rc=1
```

The operator sees NO tickets at all (valid open tickets are not listed). The
refusal is typed and fail-closed (good), but a single truncated/corrupt file
— e.g. from a crash or a tamper attempt — denies the whole queue view with no
per-ticket quarantine. Locus: `escalation-fs/src/queue.rs:155-190` (`list`
fails the whole call on the first `RecordInvalid`).

## F-5 — INFORMATIONAL — `escalations resolve`/`retry` cannot ever succeed
on this tree (documented posture; empirically confirmed)

With a schema-valid open ticket naming a real settled run whose unit holds a
durable PASS verification record, and a non-blank operator identity, the
settlement service still refuses:
`the operator action for unit AK-901 of run … carries no chain-earned
readiness; settlement is earned through the governed verification chain…`
(rc=1, ticket stays open, zero writes). A ticket naming a foreign run refuses
`RunNotDurable`. This matches the shipped design (`settle/mod.rs:742-793` —
every path returns `Err`; the Task-21 operator surface will re-enter through
the governed chain) and the shipped test. It is honest — the CLI never fakes
success — but note the tension with charter D8, which classes `escalations
resolve/retry` as "real" commands: on this tree their success path is
unreachable, so the §20.1 claim rests entirely on the typed refusal being the
honest posture.

## F-6 — MAJOR — `gauntlet skills verify` can never succeed after a real
`gauntlet skills install`: the lock writer and reader disagree on the schema
(the round-trip is broken, and no shipped test exercises it)

Reproduction (isolated world: fixture copy + all eight `REPO_OWNED_SKILLS`
copied into `.agents/skills/`; ran entirely under /tmp):

```
$ gauntlet skills install
skills: installed 8 skill(s) for the local harness (lint: pass)
skills: lock: …/.gauntlet/skills.lock        rc=0
$ gauntlet skills verify
gauntlet skills verify: …/.gauntlet/skills.lock carries no skills array   rc=1
```

The written lock is `{"schema":"gauntlet-skills-lock@v1","count":8,
"skills":{"skills":[…]}}` — a doubly-nested object — while the reader
requires the root `skills` field to BE the array. Locus:
`crates/gauntlet-cli/src/skills_cmd.rs:174-201` (`install` wraps the array in
an extra `JsonValue::Object(vec![("skills", …)])` and then binds THAT under
the key `"skills"` again at line 200) versus `skills_cmd.rs:88-91`
(`read_lock`: `parsed.get("skills")` must be `JsonValue::Array`). Consequence:
`install → verify` — the documented pair — always fails typed; `verify` can
only "pass" against a hand-written lock with the array at the root, i.e. a
shape no shipped writer produces. Drift detection, lint gating, and repair
semantics are all unreachable through the real surface. No shipped test
covers the round-trip (`cli_surface.rs` only exercises help/unknown-token
shapes; no test anywhere reads `skills.lock`). Honest failure (never a fake
pass), but charter D8's "skills install/verify real" is falsified for verify
under normal use.

## F-7 — MINOR — `gauntlet resume` of an already-settled run re-enters the
loop, burns an empty iteration, and flips the terminal to
`budget_exhausted` (rc=1) instead of an honest settled no-op

A world whose run ended `terminal=settled iterations=3` (all three units
settled, durable records intact): `gauntlet resume <run-id>` prints
`run: terminal=budget_exhausted iterations=4` and exits 1; a second resume
prints `iterations=5`. Observed durable effect: NONE — `role_invocation`,
`verification_record` and `settlement` record counts are unchanged (3/3/3),
so no re-dispatch, no double-binding, no record mutation. The loop simply
re-enters, plans zero dispatches, and iterates until the round budget
refuses, reporting a terminal that misrepresents a fully settled run.
Locus: the resume path `run_cmd.rs:142-152` → `RunController::resume_run`
(the entry does not short-circuit on an all-settled durable projection).
Integrity is intact; the operator signal is wrong.

## Attacks that FAILED to land (recorded honestly)

- **Escalation queue tampering bought nothing**: forged tickets (open,
  foreign-run, traversal, unknown-field) all end in typed refusals; refused
  actions write nothing anywhere (verified by directory listing and file
  content before/after); a resolved ticket cannot be re-resolved through the
  CLI (the service refuses before the queue is consulted, and an already-
  resolved record is refused by the queue's `AlreadyResolved`).
- **Forged PASS extraction**: a forged open ticket whose run carries three
  durable PASS verification records did NOT yield a settlement row — the
  export stream of the world was unchanged by every resolve/retry attempt.
- **D1 dry-run zero-write held**: a full dry-run in a snapshot world changed
  NOTHING (before/after sha256 over every file; only my own shell redirects
  appeared); dry-run with NO runner configured exits 0 — no runner required.
- **The dry-run content tooth bites**: an appended comment line to a world
  copy of the answer key (content change, same path set) fails the doctor
  reference check (`digest fail — REFERENCE MOVED … normalized digest … but
  the frozen record declares …`) and `gauntlet reference verify` names the
  mutation with both digests (I1). The lock check digests CONTENTS, not the
  path set.
- **D2 runner selection fail-closed held**: unset token ⇒ typed
  configuration failure naming key+env and ZERO files created in
  state/evidence (composition fails before stores are ensured); unknown
  token ⇒ typed failure listing the three fronted tokens; `mock-agent`
  without `GAUNTLET_MOCK_SCENARIO` ⇒ typed failure naming the hook. Env-vs-
  config precedence is correct through the §8.2 merge (env wins: config
  `mock-agent` + env `cli-agent` composed the CLI runner).
- **Annex E hooks refuse instead of failing open**: `GAUNTLET_CLOCK=bogus`
  ⇒ typed `test hook failure` naming the only supported token; empty reads
  as unset. `GAUNTLET_ID_SEED` is documented as hash-derived (any non-empty
  seed is valid — not a fail-open path).
- **Per-unit workspace escapes refused**: a `..`-laden effect path dies at
  scenario parse (`an effect path must not traverse upward (no '..')`), a
  second lexical guard exists at effect application
  (`runner.rs:297-330`); a `../../escaped.json` evidence locator in a custom
  answer key (lock regenerated in my world) is refused AT THE WRITE BOUNDARY
  mid-run — `the evidence locator … does not resolve inside the run's
  workspace root <per-unit workspace> and is refused at the write boundary`
  — with the refusal pinned as evidence and nothing written outside.
  Symlink-seeded escapes are unreachable through the CLI (per-unit
  workspaces are created empty; the provider's resolved-containment check is
  covered by its own suite).
- **Parallel-completion carry held under my own leg** (`GAUNTLET_MAX_PARALLEL=2`,
  three units): terminal=settled, exactly ONE observation binding per
  iteration across three iterations (sqlite `GROUP BY iteration_no` = 1/1/1),
  all bundle refs pairwise distinct — no double-binding, no lost carry.
- **Kill -9 mid-run → resume held**: killed the process during a slow
  (4 s) test-facet observation; `gauntlet resume` completed the run
  (terminal=settled), one binding per iteration, zero iterations with >1
  binding, all three units settled exactly once — no stranded units, no
  orphan bindings from the killed iteration.
- **Report categories are strictly record-derived**: deleting a settlement
  ROW drops the unit from `implemented-and-proven` to `provisional` with a
  basis naming the divergence ("verification record … is PASS but no
  settlement record commits it; the latest checkpoint projects settled but no
  settlement record exists"); a FORGED settlement row pointing at a foreign
  verification ref drops to `provisional` naming both mismatches; deleting
  ALL verification records makes `gauntlet verify` report INCONCLUSIVE for
  every unit and settle nothing (`pass=0 … NOTHING settled`). No tampered or
  forged state ever produced a `implemented-and-proven` claim without the
  full record chain behind it. Foreign-run reads are typed everywhere:
  `status` ("not durable in this state store"), `verify` (verification
  authority cannot open), `export` (no durable history; no file written).
- **Wrap posture is fail-closed (D3/R-3)**: with `sandbox.backend: bwrap`
  (real bubblewrap 0.11.1 on this host), `runners.default: cli-agent`, and a
  parsing template, the composed run refuses at the composition-time wrap —
  `the composition-time wrap was refused: sandbox policy tier-contained-v1
  failed validation for unit AK-901 (1 finding(s)) — escalation, never an
  unsandboxed fallback (spec 21.3, AK-047)`; zero settlements, no dispatch.
  Bypass attempts: an unanchored absolute writable root in the tier policy is
  refused by the policy grammar itself ("not an anchored token path …
  hardcodes an operator-machine path"); an unknown backend token and a
  backend whose identity mismatches are typed refusals. The
  `Ok(Sandboxed) → LiveContainmentDeferred` arm documented in the charter was
  not reachable on this host (the live backend's policy validation refuses
  first), so that specific arm is untested by execution — but every route
  that COULD run a builder unwrapped under a live posture refused instead.
- **AK-049 top-tier guard fires**: a policy file named `irreversible.json`
  (any casing per the check) in the policy dir fails doctor check 5
  ("no top-tier policy fail"); the check keys on the file stem, does not
  follow symlinks, and treats an unreadable dir as failure.
- **Irreversible units are never dispatch candidates**: a custom answer key
  with one `blast_radius: irreversible, authority: human` unit yields a plan
  preview of exactly 2 dispatches — the irreversible unit is never planned.
- **Secrets hunt clean**: a full mock run executed with canary env values
  (`GAUNTLET_CANARY_SECRET`, `AWS_SECRET_ACCESS_KEY`, `ANTHROPIC_API_KEY`,
  generic token) leaves ZERO values in state, evidence objects, stdout, or
  stderr; the environment fingerprint hashes env NAMES only
  (`compose.rs:927-939`).
- **Roles surface held**: unregistered harness fails closed; forged lock
  (one edited digest) ⇒ `the lock body does not digest to its own
  self-digest — the lock was edited after it was written` (verify AND list);
  source drift after install ⇒ typed drift naming the role and both digests;
  `roles list` is honestly lock-derived (reads the lock, checks nothing
  against reality, as documented).
- **§20.1 argument discipline held** (attack A battery): unknown args on
  every command and subcommand exit 2 with usage; missing required args exit
  2; malformed ULIDs/units exit 1 with typed messages; `--help`/`-h` exit 0;
  `improve <real-subcommand>` is the honest typed not-wired posture (D8);
  traversal-flavored values (`--ticket ../../…`, `--unit ../evil`) are
  refused at their id grammars before any store is opened.

## Coverage notes and limits

- All executions ran in /tmp sandbox worlds (copies of
  `tests/fixtures/target`) with per-world state/evidence dirs; the repo tree
  was never modified (verified: `git status` over `.wayfinder/`, `.gauntlet/`,
  `tests/fixtures/` shows only the pre-existing untracked task20 fixtures).
- The `e2e-happy`/`e2e-lying` scenarios and the shipped E2E were NOT
  modified; F-1 used my own scenario file outside the fixtures dir (see F-2).
- `Ok(Sandboxed) → LiveContainmentDeferred` (charter D3's recorded arm) is
  untested by execution on this host — see the wrap-posture note above.
- The load-flaky live-process attack test noted in the charter was not
  reproduced nor relied upon by any finding here.

VERDICT: NOT_CONFIRM (1 blocking / 1 major / 4 minor / 1 informational)
