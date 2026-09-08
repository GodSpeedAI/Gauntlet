# Task 27 IN-FLIGHT ledger — final conformance + handoff

Charter: `.agents/reports/task27/charter.md` (binding).
Plan: `.agents/plans/gauntlet-impl.md` §Task 27.
Ruler units: AK-067, AK-068 direct; the sweep dispositions all 70.
Gate: verbatim in the charter; `just ruler` at every phase boundary.

## Slice ledger

| slice | scope | status | result contract |
|---|---|---|---|
| B1 | conformance sweep: `gauntlet_app::conformance` + `verify --all --report` + 70-unit manifest + help regen | DISPATCHED 2026-09-07, RUNNING | `.agents/reports/task27/b1-report.md` |
| B2 | §25 docs set (8 uppercase files) + `tests/contract/docs_present` (structure teeth) | LANDED 2026-09-07 — adjudicated accepted-pending-full-battery (scoped targets 5/5+3/3+1/1 green; teeth digests recorded; ruler PASS; 4 package-battery failures = B1's WIP arch-denylist hits, B1's to clear) | `.agents/reports/task27/b2-report.md` |
| — | orchestrator: first REAL sweep run + commit conformance.json (wave-2 precondition) | pending B1 | phase-gate action |
| B3 | handoff.md + consistency/cross-check tests + evidence-bundle pin + reference-candidates entries + UNK-002/003 | pending wave 2 | `.agents/reports/task27/b3-report.md` |
| critic | blind design-as-built critic (fresh context) | pending build | `.agents/reports/task27/critic-design.md` |
| falsifier | blind EXECUTING falsifier | pending critic | `.agents/reports/task27/falsifier-round1.md` |

## Phase transitions

- 2026-09-07 charter written; B1 + B2 dispatched (wave 1, parallel,
  file-disjoint). CURRENT_STATUS active=task27/dispatched.
- 2026-09-07 B2 landed + adjudicated (accepted-pending-full-battery).
  Noted for B3: docs claims are deliberately all
  `implemented_but_provisional` (weaker-side rule); the wave-2
  cross-check must FAIL ahead-of-evidence claims (the plan's rule) and
  mechanically upgrade behind-evidence ones where conformance.json says
  `implemented_and_proven` — claim-line parse shape
  `^- <status_word> — AK-NNN:`; UNK-001 stable phrase
  `cross_cutting containment — a declared substitution`.
  Charter decision 10 amended (export is run-bound; no repo state
  store — pinned bundle is the canonical export, recorded honestly).
- 2026-09-07 B1 landed + adjudicated (manifest 70/70 verified: 67
  execute / 3 artifact / 4 bounded exactly as declared; deviations D1-D6
  accepted — D1 structural command vocabulary forced by the frozen
  AK-016 scanner, verified live; D2 module in the pre-existing task-11
  adapter crate; D3 single_writer inventory per the suite's own remedy).
  Full wave-1 battery in the orchestrator's hands GREEN: workspace
  1664/1664, boundaries 245/245, lint/deny clean, ruler PASS.
- 2026-09-07 FIRST REAL SWEEP (orchestrator): FAILED honestly —
  70 units = 2 PASS (the two recorded artifacts) / 1 INCONCLUSIVE
  (handoff.md not yet — wave 2) / 67 FAIL, all 45 commands dying in
  ~50ms with `flock: failed to execute -w` exit 69. Root causes: (1)
  the adapter's flock argv is file-first (flock needs options before
  the operand — verified empirically both orders); (2) flock-level
  exec failure classified FAIL instead of the charter's environmental
  INCONCLUSIVE; (3) the adapter's unit test asserted the argv SHAPE and
  enshrined defect 1 — the missing behavioral tooth (task24/26
  recurrence class: composition never executed). The sweep machinery
  itself behaved correctly: honest FAILs + evidence logs + exit 1, no
  fabrication. COR-1 dispatched to B1 (argv order; 86/69 environmental
  with inner-69 remapped to 169; behavioral flock test with true/false/
  held-lock legs). Evidence retained at
  conformance-evidence/20260907T193758Z/.
- 2026-09-07 COR-1 landed (adapter 9/9 incl. 3 behavioral flock legs;
  arm-aware {86,69}->CannotRun; residual seam honestly recorded: the
  six-line CLI mapping has no direct core-scope test). SWEEP-2
  (orchestrator): 70 units = 68 PASS (64 implemented_and_proven + 4
  implemented_but_provisional — exactly the manifest's bounded four) /
  1 INCONCLUSIVE (AK-067 no_artifact — correct until B3 lands
  handoff.md; hint `proposed`) / 1 FAIL (AK-062 `command exited 4`).
  AK-062 root cause: the whole operator_actions_no_bypass test file is
  `#![cfg(feature = "surfaces")]` — zero tests under the default
  config; the manifest's closed vocabulary lacked a features
  dimension. The exit-4 FAIL classification itself is correct (zero
  tests ran is never PASS). COR-2 dispatched to B1 (features: field
  validated against the workspace's feature edges; AK-062 entry; sweep
  the manifest for other gated targets). Evidence:
  conformance-evidence/20260907T200600Z/.
- 2026-09-07 COR-2 landed (features dimension {composition,surfaces}
  discovered from the workspace [features] tables, pinned by a test;
  AK-062 entry carries features:[surfaces]; cli_surface.rs verified
  inner-cfg-only). SWEEP-3 (orchestrator, ~75 min): 70 units = 69
  PASS (65 proven + 4 provisional) / 1 INCONCLUSIVE (AK-067
  no_artifact — wave 2) / 0 FAIL, exit 0; AK-062 PASS through the
  surfaces config. WAVE-1 MILESTONE COMMITTED ad9c64f (174 files;
  evidence dirs + reports included; CURRENT_STATUS/TUI/.jolli
  excluded); ruler PASS post-commit.
- 2026-09-07 B3 (wave 2) DISPATCHED two-phase: phase 1 =
  reference-candidates dated entries for the 17 unrecorded divergences
  + manifest divergence re-pin + pin-evidence-bundle.sh (+--check);
  then orchestrator re-sweep (dated citations land in the json,
  AK-067 still INCONCLUSIVE); phase 2 = handoff.md at the frozen
  AK-067 locator (six categories, every INCONCLUSIVE named, UNK-002/
  003 terminal dispositions, no-repo-run export disclosure, bounded
  bases quoted) + handoff↔conformance consistency test + docs
  status-claim cross-check with mechanical upgrades of behind-evidence
  lines + the executed bundle pin + docs/HANDOFF.md counts.
- 2026-09-07 B3 PHASE-1 LANDED + COMMITTED: reference-candidates
  2026-09-07 batch entry (8 sub-entries, all 17 unrecorded units, no
  prior-entry collisions verified); manifest divergence re-pin 17+/17-
  (CLI manifest tooth re-proven); scripts/pin-evidence-bundle.sh
  (fixture-tested, refused honestly on the real tree pre-handoff).
  Orchestrator decision: SKIP the intermediate re-sweep (75 min to
  change citations handoff.md doesn't carry); committed json's
  manifest-sha goes stale in the window — honest evidence-of-the-moment,
  resolves at the settlement sweep. B3 phase 2 released against the
  committed json (AK-067 in the proposed section for now); anticipated
  post-settlement-sweep correction: AK-067's handoff row move + count
  bump (~5 lines).
- 2026-09-07 BLIND DESIGN-AS-BUILT CRITIC r1: CONFIRM (0 blocking /
  1 MAJOR settlement-precondition [committed json is not yet the final
  tree's report — decision 12 requires the settlement re-sweep; the
  self-consistent fixed point exists and the teeth enforce it] / 3
  MINOR [evidence_ref dual anchoring execute=report-parent-relative vs
  artifact=repo-relative — resolve by disclosure at settlement;
  structural-vocabulary deviation re-adjudicated ACCEPTED with the
  forcing claim verified against the AK-016 scanner source; CURRENT_
  STATUS uncommitted — settlement step] / 2 INFORMATIONAL). POSITIVE
  audit surface: both plan-gate asserts hold on the artifact by
  construction; all 67 manifest execute arms mechanically cross-checked
  REAL (targets exist, filters found in cited sources, features match
  the cfg gate, no hopeful mapping); teeth verified not trusted (row-
  move/ahead-of-evidence/flock/manifest); bundle 50/50; handoff
  re-derived 65/4/0/1/0/0 == json. Adjudication: MAJOR = the planned
  settlement sequence itself (final sweep -> handoff row move -> re-pin
  -> commit); minors 1-3 routed to the settlement correction.
- 2026-09-07 BLIND EXECUTING FALSIFIER r1: NOT_CONFIRM — 3 MAJOR /
  5 MINOR, every finding EXECUTED (report: falsifier-round1.md).
  MAJORS: F1 duplicate manifest ids accepted, first entry shadows
  (forged PASS repro'd); F2 test_report units may carry an artifact
  arm (PASS with zero commands repro'd); F10 signal-killed command
  fabricates exit=-1 FAIL instead of environmental INCONCLUSIVE
  (lock-held kill-9 repro'd — the OOM shape). MINORS: F3 handoff
  predicate accepts a six-word legend; F4 mutation date unvalidated
  (recorded-artifact posture — residual); F7 evidence_ref anchor
  arm-dependent (matches critic minor); F8 docs claim-check has five
  executed evasion classes (case/hyphen/split-line/lowercase-ak);
  F13 lib filters are loose substrings (over-determined fail-closed;
  drift risk). Attacks that HELD: the full refusal matrix (11
  structural manifest lies -> typed REFUSED), three-state discipline,
  grouping+sha256 evidence, COR-1 wrapper teeth 9/9 live, COR-2
  features both directions, handoff grouping teeth all 6 tamper
  classes, bundle tamper/missing/foreign-cwd, delivered suites green.
  Falsifier incident disclosed: its own first driver put sccache-stop
  before cargo inside its flock — wedged the compile lock, recovered;
  re-confirms the delivered wrapper's order. ADJUDICATION: F1/F2/F10/
  F3/F7/F13 -> COR-3 dispatched to B1 (dup-id typed refusal; kind-arm
  matrix exact; signal->CannotRun incl. the unopenable-lock class;
  predicate section grammar; two _meta disclosures — F13 adjudicated
  disclose-and-record, no re-map); F8 -> COR-B3 dispatched to B3
  (case-insensitive + separator-tolerant + split-line window scan,
  five RED teeth, over-rejection invariant test). F4 recorded as
  decision-11 residual. Both corrections in flight, file-disjoint.
- 2026-09-08 COR-B3 LANDED: five F8 evasion classes closed (case-
  insensitive canonicalizing match; primary line-anchored parse +
  vocabulary-keyed +/-1 window fallback binding split/separator
  shapes; over-rejection invariant pinned — verb-beside-id prose is
  not a claim); RED-first against the unchanged checkers observed,
  plus a mid-correction RED the new teeth caught themselves; a
  nextest fixture-dir race found+fixed (pid+nanos). Real-tree audit:
  exactly 33 window pairings = the 33 claim lines, zero incidental.
  Scoped battery green; the 2 --lib failures disclosed are B1's
  in-flight COR-3 files (task-23 wave-interference class). Honest
  residual: bare non-canonical status tokens split from their id
  stay outside both legs (window keys on vocabulary words only).
  AWAITING COR-3 (B1).
- 2026-09-08 B1 DIED on a usage-limit window mid-COR-3 (resets 14:42
  harness time). Recovery per the standing procedure: tree audited
  BEFORE any re-dispatch — ALL COR-3 code + tests landed (F1
  DuplicateId typed refusal + test; F2 exact kind<->arm matrix + test;
  F10 signal->CannotRun naming the signal + self-kill stub test; F3
  predicate ## -heading grammar + test; F7/F13 _meta disclosures).
  Only b1-report.md's COR-3 section was unwritten — completed by the
  orchestrator under the heal+continue exception (dispatch
  unavailable), labeled as orchestrator-verified, not builder claim.
  Full workspace battery over BOTH corrections running in the
  orchestrator's hands. B3's COR-B3 landed earlier (see above).
- 2026-09-08 COR-3 battery v1 FAILED at the arch scanners: B1's F2
  alien-kind test token "diff" is an adapter identifier under the
  denylist (AK-014 scan; task-26 COR-3 token class recurring).
  Orchestrator mechanical fix: token -> "astral_kind" + a comment
  stating the constraint (conformance.rs:1267 region). Verified:
  denylist --lib 3/3, docs_consistency+docs 14/14 (COR-B3), app
  conformance 25/25 incl. the fixed F2 test. Battery v2 (full,
  consecutive) running.
- 2026-09-08 FOCUSED FALSIFIER r2 (fresh agent): NOT_CONFIRM — 3
  MAJOR / 2 MINOR / 1 COSMETIC, each executed (falsifier-round2.md).
  R2-F1 inner exit-86 misattributed lock-busy INCONCLUSIVE (gate exit
  0 on a real failure); R2-F2 inner signal kills still FAIL with
  fabricated 128+n codes (the realistic OOM victim); R2-F3 PAYMENT:
  unconditional sccache --stop-server = ~2m14s/command with no daemon
  (~100 min lock-held per sweep; explains the 75-min sweeps and the
  269s adapter tests); R2-F4 log+artifact+bounded wider than the
  correction's contract; R2-F5 Cyrillic homoglyphs evade both docs
  legs; R2-F6 mangled whitespace in DuplicateId msg. HELD: F1
  byte-equal/padded/execute-artifact twins; F2 unbounded laundering;
  F10 direct kills; F3 grammar variants; F8 all ten shapes; no
  PASS-without-evidence path; committed-report staleness matches the
  recorded lifecycle (settlement re-sweep owed). ADJUDICATION: COR-4
  dispatched to B1 (inner-86->186 remap; 128+n -> 200+signal band
  remap -> CannotRun naming the signal; timeout-10 bounded sccache
  stop; bounded-artifact restricted to test_report; message fix;
  disclosure/comment corrections); COR-B3-2 dispatched to B3 (static
  ASCII-confusable fold + the two executed homoglyph teeth + honest
  residual). Focused r3 to follow on the reserved-code scheme.
- 2026-09-08 COR-B3-2 + COR-4 LANDED. COR-B3-2: static 19-pair
  ASCII confusable fold inside the matchers only; both executed
  homoglyph shapes RED-first then GREEN; table-pinning test; honest
  residual (exotic scripts + digit look-alikes outside the legs;
  1/l unfolded deliberately to protect the AK grammar).
  COR-4: inner-86->186 remap + classify_exit table; signal band
  remap 200+signal -> CannotRun naming it (teeth through the REAL
  wrapper: kill -15 -> 215, kill -9 -> 209); CORRECTION-OF-THE-
  CORRECTION executed by B1's own pass-through tooth — the r2-
  demanded 128..191 band consumed a genuine inner 169 (observed
  169->241), narrowed to 129..159 (standard signals exactly);
  RT-signal death 160..192 stays FAIL (disclosed); timeout-10
  sccache stop (adapter test-phase 269s -> 10.4s; per-command floor
  eliminated); bounded-artifact restricted to test_report (RED-
  first); R2-F6 string fix. Reserved set now {86,69} lock-level /
  {169,186} inner-remapped / {200..231} inner-signalled, all
  ambiguities disclosed. Scoped batteries green. FULL closure-grade
  battery running in the orchestrator's hands.
- 2026-09-08 FOCUSED FALSIFIER r3: CONFIRM — 0 MAJOR (0 blocking).
  The corrected exit-code scheme held under executed attack: band
  boundaries exact (kill -1/-31 -> signals named; RT kills -34/-40
  stay FAIL), the 10s stop bound reproduced the dirty 2m14s stall
  live and capped it exactly (sleep-2 sweep 12.04s wall, never
  +134s), daemon invariant held across ~15 runs, fuzz table exact,
  all regressions green, zero state collapses. 4 residuals: R3-F1
  chosen-exit-in-band disclosure gap; R3-F2 impossible "signal 0"
  reason facts; R3-F3 stale refusal text for log kinds; R3-F4
  artifact-only sweep into a nonexistent report parent dies at
  write (os error 2). All adjudicated -> COR-5 dispatched to B1
  (final round; settlement follows). Corrections commit landed:
  workspace 1697/1697, boundaries 261/261, ruler PASS.
- 2026-09-08 COR-5 LANDED + COMMITTED. SETTLEMENT SEQUENCE STARTED:
  step 2 = the settlement re-sweep via the plan gate command
  verbatim (expect 70 PASS incl. AK-067 flipping on the landed
  handoff.md). Then: handoff row-move per B3's playbook (orchestrator
  settlement action, mechanical 5-line edit B3 documented);
  conformance_cli pinned-counts update 3/67 -> 4/66 (same re-pin the
  interim state required at each artifact landing); docs_present
  verify; bundle re-pin + --check; just ci; ruler; the four personal
  teeth; closure records + settlement commit.
- 2026-09-08 SETTLEMENT SEQUENCE EXECUTED. Sweep-4 (plan gate
  verbatim): 70 units = 70 PASS / 0 INCONCLUSIVE / 0 FAIL, 45
  commands, exit 0 (AK-067 flipped on the landed handoff; gate python
  asserts hold). Handoff row-move: BOTH teeth caught the
  orchestrator's own two errors live (AK-067 bullet initially placed
  in the provisional section [AK-066 is bounded-provisional]; a wrong
  4/66 pinned-count expectation — the validation-only executor's
  split stays 67/3 since AK-067 already counted among the artifact
  PASSes). Fixed; consistency 7/7 + conformance_cli 10/10; bundle
  re-pinned (50 files) + --check from root. Closure battery v1: one
  recorded load-flaky test failed under load (d_env_and_prompt —
  task-16-era adapter test, NOT task27 code), green in isolation
  (0.055s), /tmp healthy; battery v2 GREEN: check 0, workspace
  1700/1700, lint/deny 0, boundaries 261/261, ruler PASS. Four
  personal teeth RED -> byte-identical restore -> GREEN (T1 manifest
  kind-damage; T2 baselines.json removal; T3 docs/SECURITY.md
  removal = 4 RED; T4 fixture row-move = 4 violations).
- 2026-09-08 INCIDENT (orchestrator, disclosed): a CURRENT_STATUS.yml
  bulk edit matched the operator's workbench next_action line instead
  of the real one and truncated the metadata/execution/active/
  next_action middle. Reconstructed same-session from git HEAD + the
  workbench block (exact text preserved) + this session's known
  edits, then re-verified section-by-section. Known delta: the
  task26-era uncommitted updated_by long text is carried as a summary
  line (the full text survives verbatim in commit a98b624's message
  and the task26 closure record). Lesson filed: bulk edits to
  multi-occurrence keys require unique anchors.
