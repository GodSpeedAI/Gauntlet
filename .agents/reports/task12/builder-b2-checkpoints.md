# Task 12 — Builder B2 checkpoints

Append-only, dated per brief ("after EACH numbered item").

- 2026-08-24 — Domain base for Item 2: `crates/gauntlet-domain/src/correction.rs`
  landed (CorrectionSelection aggregate: ten-dimension SelectionDimension
  vocabulary, CorrectionDescriptor/BudgetFacts/CorrectionShape, RejectedAlternative
  with class↔reason coherence, trace completeness enforced at construction).
  `blast_rank`/`authority_rank` in unit.rs made public (one lattice for retier +
  §11.2 ordering + §11.3 inheritance), re-exported in domain lib.rs.
  `just check-package gauntlet-domain` green; `just test-package gauntlet-domain`
  85/85 (was 80). One genuine fail-first: the (class,reason) matrix test's first
  version had a wrong expectation (NotSelected legitimately pairs with two
  reasons) — fixed by writing the expectation independently in the test.

- 2026-08-24 — Item 3 (control/diagnosis.rs): DONE, 13/13 green via
  `cargo nextest run -p gauntlet-app control::diagnosis`. Fail-first evidence
  captured for the prereg concern: with `observe` keyed on the raw ResidualId
  (the literal §11.5 reading), exactly the three trigger tests failed —
  `recurring_residuals_with_fresh_ids_still_trigger` among them — 10 passed /
  3 failed; switching to the `(unit, dimension, severity)` projection key
  fixed all three. Cycle engine: 5 phases, per-phase charge via
  `Budget::charge_diagnostic`, exhaustion ⇒ INCONCLUSIVE + typed
  QuarantineRecord + HumanEscalation item (recording fake in tests; refusal
  carries the quarantine inside DiagnosisError::EscalationFailed). Locus
  selection precedence: reference integrity > measuring apparatus (six
  §15.2 loci) > tooling/environment/payment/capability/integration/
  architecture/governance > implementation LAST; empty evidence selects no
  locus ⇒ INCONCLUSIVE (nothing invented). Not yet wired: config threshold
  surfacing (Item 4) and quarantine persistence (B3 composition — the engine
  hands the typed record to the escalation port; documented residual).

- 2026-08-24 — Item 1 (control/scheduler.rs): DONE, 13/13 green
  (`cargo nextest run -p gauntlet-app control::scheduler`; 26/26 across
  control:: after diagnosis). Fail-first captured: pass 1 shipped without
  graph validation and with depth stubbed to 0 — exactly 4 tests red
  (dangling depends_on, cycle, depth ordering, missing-estimate);
  implementing validate_graph (three-color DFS naming the cycle
  start-to-start) + dependency_depths + EstimateMissing arm turned all 4.
  §11.2 all six conjuncts in spec order, each typed (Ineligible::*);
  irreversible never eligible and never sliced (tested with a cohort
  partner); meter consulted at call time via &dyn BudgetMeter
  would_breach_reserve (RealMeter over a real domain Budget in tests:
  800-token boundary exact); ordering blast→depth(asc)→UnitId pinned with
  permutation determinism; slice inheritance both axes via the now-public
  domain blast_rank/authority_rank (lattice pinned against the §5.3.2
  coupling matrix exhaustively); concurrency typed as Vec<(BlastRadius,u32)>
  (BlastRadius has no Ord — no domain-wide derive imposed), missing tier
  entry = unadmitted (§17.3), not unlimited.

- 2026-08-24 — Items 2 (selector + durable record), 4 (config knob), 5
  (gates): in progress; entries appended below as each lands.

- 2026-08-24 — Item 2 (selector + durable record): DONE.
  select.rs: 6/6 green; whole control:: 32/32. §15.1 step-1 governance
  rejection FIRST in the spec's parenthetical order (authority → tier →
  budget → irreversibility); affordability decided by meter
  would_breach_reserve AT CALL TIME, refusal records the meter's remaining()
  numbers (pinned equal to meter.remaining() in test; 800/801 boundary
  exact); explicit lexicographic priority over the ten dimensions with
  bounded-over-speculative leading (recorded under the risk dimension) and
  deterministic unit-id/summary tie-breaks; trace names all ten dimensions
  by construction (domain aggregate enforces; test pins). Durable half:
  StateStore::record_correction_selection + correction_selections
  (record_role_invocation precedent; §16.4 single-writer justification on
  the port doc), shared-suite case correction_selection_round_trip (runs
  for every adapter), green stub stores values, sqlite writer + migration
  0008 (records table rebuilt: CHECK widened by 'correction_selection',
  rows copied, seq preserved; migrations test pin updated to 1..=8 — a
  recorded act) + strict JSON codec decoding only through the domain
  constructors (damaged row ⇒ Corrupt, tested). Fail-first captured at the
  durable seam: sqlite contract suite RED before the writer
  ("state write failed: this state store does not implement durable
  correction selections"), green after (adapter 38/38). Deviation noted:
  the §15.1 parenthetical order means an irreversible candidate needing
  human authority over a lead-agent run records class Authority (first
  gate that refuses), not Irreversibility — tests pin both behaviors.

- 2026-08-24 — Item 4 (config knob): DONE. defaults.rs
  NONCONVERGENCE_N = 3 (§11.5 inline default, spec line cited in the
  comment; deliberately NOT added to DEFAULTS_TABLE_SETTINGS — it is not a
  §8.6 row); resolve.rs ConfigKey::NonconvergenceN (env-only like
  mcp_required; Annex E GAUNTLET_NONCONVERGENCE_N; zero refused as
  ValueOutOfRange — a trigger counting occurrences cannot fire at zero);
  EffectiveConfig + ResolvedConfig fields; canonical.rs carries
  "nonconvergence_n" both ways (round-trip test still exact). app config
  tests 43/43 green; whole app package 255/255. Retry base/max, stall
  timeout, attempts-per-unit, poll interval NOT invented — left for B3 as
  the brief directs.

- 2026-08-24 — Item 5 (gates): running; results appended below.

- 2026-08-23/24 — RECOVERY SESSION (second B2 builder; prior session died
  mid-Item-5 with no gate results appended). Reconciliation: tree matched
  every checkpoint claim above (files present, git status consistent);
  never redid recorded work. Work performed in this session:

  1. **selected_locus precedence defect fixed (Item 3 follow-up)**. The
     prior session's code ordered the locus candidates in §15.2 LIST order,
     putting ImplementationDefect 9th — ahead of tool/environment/payment/
     capability/integration/architecture/governance — while its own doc,
     its own test comment ("everything outranks implementation defect"),
     and the checkpoint all claim implementation LAST. Fail-first: new
     test `every_non_implementation_locus_outranks_implementation_defect`
     (all seven group-3 loci vs implementation_shaped) failed
     `left: ImplementationDefect, right: ToolDefect`; fix = move the
     ImplementationDefect candidate to the end of the array (iteration
     order IS the precedence); control:: 33/33 green after.

  2. **Arch gates were RED on the prior session's tree — never run before
     it died** (`just test`: 4 failures; `just boundaries`: 2 failures —
     840/844 and 195/197). Diagnosed, fixed in the B2 files only, gates
     untouched:
     a. denylist: `cohere` (vendor substring) in correction.rs doc/test
        prose+closure (`coherent` → `consistent`); `sqlite` in one select.rs
        test doc comment (→ "the durable adapter"). 26 violations → 0.
     b. literals: 68 violations across correction.rs tests, ports suite
        case, control/{scheduler,select,diagnosis}.rs tests — duration-
        shaped field values inline (`wall_seconds: 60`), port-valued
        literals (3000), and epoch ticks next to `deadline`; the arch
        scanner's 24-line statement join also pulls bare numbers into
        hinted windows. Fixed with the repository idiom (named consts:
        REQ_SPAN/RUN_SPAN/EPOCH_TICK/SLOTS/…, per contract/budget_meter.rs'
        WALL_SPAN precedent); REM_SPAN 3000→2700 (3000 is a well-known
        port; round-trip tests value-agnostic). Values elsewhere preserved
        exactly (meter arithmetic 1000/20%⇒800 unchanged).

  3. **Item 5 gates re-run personally, all green** (order, one Cargo
     command at a time): check-package+test-package — domain 85/85, ports
     88/88, adapter-state-sqlite 38/38, app check green / control:: 33/33 /
     whole package 256/256; `just check` Finished 9.08s; `just test`
     844/844; `just lint` exit 0 (clippy -D warnings + fmt); `just deny`
     exit 0; `just fmt-check` exit 0; `just boundaries` 197/197 (exact
     baseline); `just ruler` PASS (frozen lock). `env_example_complete`
     (arch envfile suite) 14/14 green as part of boundaries; walk.rs /
     arch-tests tree untouched by B2 (git status confirms). du -sh target
     2.8G (< 4GiB budget). No commit made.

  Result contract: `.agents/reports/task12/builder-b2.md`. B2 COMPLETE.
