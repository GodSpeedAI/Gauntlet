# Task 13 — arch-suite re-litigation record (standing "next arch-suite touch" obligation)

Source: read-only research agent, 2026-08-26, against `62d1a5f` + the B2
heal delta (two files, shape-neutral for these classes). Landed by the
orchestrator from the agent's return (bounded claims preserved verbatim in
spirit; procedure statements retained). Trigger: B3 touched
`gauntlet-arch-tests` (new `single_writer.rs` check, boundaries 197→215).

## (a) E4/F1/F3 closures (Task 4) — **HOLD**

All three closures are mechanically present, each with its matcher and
named pin test:
- **E4** (timeout class accepts `return`/block-value positions):
  `find_timeout` literals.rs:1404-1423 (+`is_return_prefixed` :1430-1435);
  pin `block_values_and_returns_introduce_a_timeout` :2699-2722; false-
  positive shape (count from a duration-hint-named fn) documented
  :1396-1403.
- **F1** (stopword exemption yields to command SHAPE): `find_command`
  literals.rs:1300-1330 + `command_shaped` :1338-1349; pin
  `commands_carrying_stopwords_are_still_flagged` :1949-1977 (critic-9
  F1's exact mutations); false-positive shape (prose citing a flag)
  documented :75-82.
- **F3** (`=`-less `.env.example` lines are findings): `malformed_entry_lines`
  envfile.rs:272-288; pin `a_line_without_equals_is_a_violation_not_a_skip`
  :534-557; future-loader caveat documented :261-271.
Bounded claim: pins exist and encode the behavior; last recorded full run
green (now 215/215 after B3, personally verified). No fresh execution by
the research agent (read-only).

## (b) Task-9 B.5a masked codec spellings — **HOLD**

All pins intact: module list `ANNEX_B_CONTRACT_PARSERS` walk.rs:118-125
(grew 4→6 at Task 10, recorded+pinned `annex_b_contract_parsers_are_pinned`
walk.rs:528-548); mask corners incl. split fn-arg / purpose-blind element /
receiver-invisible `.get` pinned by
`annex_b_mask_corners_are_pinned_to_the_documented_residual`
denylist.rs:1465-1556 (outside-module control proves detectability);
maskable intersection still `["command"]` pinned by
`the_maskable_wire_key_intersection_is_pinned` denylist.rs:1571-1603
(recomputed from the real member list when compare-model landed).
Bounded scan of the six codec modules (whole-literal segment spellings +
`select|choose|pick` calls): only schema-conforming `command` spellings —
no NEW selection-shaped spellings within that scope.

## (c) Task-6 A5.2 adapter-allowlist granularity — **HOLD; not newly
load-bearing**

Still name + source-class only (`check_member` deps.rs:760-766; Task-11
F2's feature seal covers INTERNAL edges only, deps.rs:823-825). Among the
17 adapter manifests: exactly one features-bearing external declaration
(`rusqlite`/`bundled`, state-sqlite dev-deps — matches the recorded
justification verbatim); all other external edges are featureless
`sha2.workspace = true` inheritance. No adapter now exercises the
residual. Bounded to `crates/adapters/**` + the root table; patch/config
channels separately gated.

## (d) Task-7 B-F16 scanner residual shapes — **HOLD, with a re-record
note**

Both residuals still pass as documented (tier class declaration-only,
`find_tier_policy` literals.rs:1446-1468 + `TIER_TERMS` :427-436;
relative-path class literals-only, residual :52-60). NEW production
instances since Task 7, found by bounded scan and dated by blame:
- tier-in-expression: `compare/review.rs:346,:433` — `which: "adversarial"`
  struct-literal tags (Task 10). Not policy; typed-vocabulary error tags.
- slash-assembled format strings: `comparator.rs:1219` (Task 10),
  `verify/ladder.rs:652` (Task 11) — run/iteration and role LABELS, not
  filesystem paths.
None is a path or policy leak; no widening forced. **Re-record:** the
ledger's "production doctor code used the shape" note (CURRENT_STATUS,
Task-7 deferrals) understates the current count — the closure record
updates it to this inventory. Any future instance that assembles a real
path or encodes tier policy re-opens the widening decision.

## Summary

All four items hold against the current tree within the stated bounded
procedures. One re-record (d's instance inventory); no widening; no
exemption edits. B3's own growth (single_writer.rs) follows the
central-declaration + anti-vacuity conventions and its masked-spelling
residuals are recorded in its module docs.
