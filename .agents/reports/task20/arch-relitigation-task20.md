# Task 20 — arch-suite re-litigation record (standing trigger REBORN → discharged)

Trigger: `.agents/CURRENT_STATUS.yml` — "Arch-suite standing trigger REBIRTHS at
the next arch-suite touch … E4/F1/F3, B.5a masked spellings, A5.2 allowlist
granularity, B-F16 shapes must be re-litigated and re-recorded when touched
again." Discharged 2026-08-27 at Task 13; reborn by plan Task 20's two
arch-suite edits.

Tree state: uncommitted Task-20 delta on `main`. The arch-suite delta is
exactly two files (`git diff HEAD -- crates/gauntlet-arch-tests/`):
`deps.rs` (+95/−25) and `single_writer.rs` (+175). `literals.rs`, `envfile.rs`,
`escapes.rs`, `denylist.rs`, `walk.rs` are byte-identical to the
Task-13-adjudicated state. Boundaries on the final tree: 228/228 (215/215 at
Task 13). Adjudication performed read-only against the settled tree
(2026-09-01); record landed by the orchestrator.

## (a) E4/F1/F3 closures (Task 4) — **HOLD**

Modules untouched by Task 20; all anchors intact at their recorded sites:
- **E4**: `find_timeout` literals.rs:1404-1423 (+ `is_return_prefixed`
  :1430-1435; false-positive shape documented :1396-1403); pin
  `block_values_and_returns_introduce_a_timeout` :2699-2722 (incl. the
  `xreturn` negative :2715-2721).
- **F1**: `find_command` literals.rs:1300-1330 + `command_shaped` :1338-1349;
  doc :75-82 and :1290-1299; pin
  `commands_carrying_stopwords_are_still_flagged` :1949-1977.
- **F3**: `malformed_entry_lines` envfile.rs:272-288; future-loader caveat
  :261-271; pin `a_line_without_equals_is_a_violation_not_a_skip` :534-557.

New-instance check: Task 20's new production text (gauntlet-cli `compose.rs`,
`init_cmd.rs`, `state_cmd.rs`, `skills_cmd.rs`, `reference_cmd.rs`,
`run_cmd.rs`, `verify_cmd.rs`, `writer_lock.rs`, etc.) is inside the
scanners' declared scope ("four core crates, including gauntlet-cli",
literals.rs:95-97). Bounded scan: no std `Command::new` in the new CLI code —
only typed wrappers (`TestCommand::new` compose.rs:339, `BuildCommand::new`
compose.rs:366); no command-shaped literal leaning on the F1 stopword
residual; full suite green on this tree. **No new instance.**

## (b) Task-9 B.5a masked codec spellings — **HOLD**

`walk.rs`/`denylist.rs` untouched. Module list `ANNEX_B_CONTRACT_PARSERS`
walk.rs:118-125 (still 6) + pin `annex_b_contract_parsers_are_pinned`
walk.rs:529-548; mask-corners pin
`annex_b_mask_corners_are_pinned_to_the_documented_residual`
denylist.rs:1672-1763 (line drift only; split fn-arg D4 residual,
purpose-blind element, receiver-invisible `.get`, and the outside-module
detectability control all intact); maskable-intersection pin
`the_maskable_wire_key_intersection_is_pinned` denylist.rs:1778-1810, still
`["command"]`.

Task-20 interaction was live here: three workspace members added (root
Cargo.toml:24-26: `workspace-fs`, `escalation-fs`, `id-entropy`), and the pin
recomputes from the real member list. Derivation uses the LAST distinguishing
segment (denylist.rs:273ff), so the new last segments are `fs` (pre-existing
via `evidence-fs`/`policy-fs`) and `entropy`; `SPEC_VOCABULARY_SEGMENTS`
(denylist.rs:94) is unchanged and neither segment is a declared Annex-B wire
key — intersection stayed `["command"]`, confirmed by the green pin in the
final run. No new masking surface created. **HOLD.**

## (c) Task-6 A5.2 adapter-allowlist granularity — **HOLD**

`check_member` deps.rs:703-814 unchanged in granularity: admission is (name
on layer allowlist) AND source-class admissible (deps.rs:760-766; failure
messages :772-793) — still a (crate, source) PAIR, never a bare name. Adapter
layer grew to 23 members; the three new manifests are std-only
(`[dependencies]` = `gauntlet-domain` + `gauntlet-ports` only, each declaring
"the adapter external allowlist is untouched"; no `features` key anywhere).
Features-bearing external declarations among adapters: still exactly
`rusqlite`/`bundled` in state-sqlite. `ADAPTER_EXTERNAL_ALLOWLIST` unchanged
{rusqlite, sha2}; root table carries no features key.

The Task-20 seal widening (deps.rs:905-907) is an INTERNAL-edge feature seal
(gauntlet-cli→gauntlet-app), not an external-allowlist act — no interaction
with A5.2's granularity; new adapters' internal edges remain domain+ports-
only, enforced at deps.rs:735-755. **HOLD.**

## (d) Task-7 B-F16 scanner residual shapes — **HOLD; instance inventory unchanged**

Tier class still declaration-only: `find_tier_policy` literals.rs:1446-1468 +
`TIER_TERMS` :427-436 unchanged; relative-path class still literals-only
(residual ledger literals.rs:52-60; runtime-assembly residual :88-93).
Bounded scan of Task 20's new/modified production text:
- Tier-in-expression: NO string-literal tier term in expression position in
  any new file. Nearest shapes are typed identifiers, not literals
  (compose.rs:830 `adversarial: None`; `blast_radius()` calls/fields at
  compose.rs:973, state_cmd.rs:120, reference_cmd.rs:374, run_cmd.rs:306/363
  — same "typed vocabulary, not policy" class task13 recorded);
  govern/gate.rs:462's `"…is irreversible…"` is an error-message format
  string (not a whole-literal term, not a declaration line). Nothing fires;
  no widening pressure.
- Slash-assembled strings: zero `format!("…/…")` / `"/"` path assembly in the
  new cli files and `gauntlet-app/src/report/`; paths are built with
  `PathBuf::join`, matching the inventory's own reasons.

The task13 inventory (compare/review.rs:346,433; comparator.rs:1219;
verify/ladder.rs:652) is unchanged — those files untouched by Task 20. The
re-open condition (a real assembled path or encoded tier policy) is not met.
**HOLD.**

## The two Task-20 arch-suite edits

1. **deps.rs composition-seal widening** — within the suite's documented
   flows. Deliberate, recorded inventory edit: module doc updated to "TWO
   composition seams" (deps.rs:881-899); the legal predicate widened only for
   `owner == Some("gauntlet-cli")` adding `[dependencies]` beside
   `[dev-dependencies]` (deps.rs:905-907); the pin renamed and extended
   (`the_composition_seal_accepts_only_the_recorded_cli_edges`, deps.rs:2804)
   with the production-edge positive AND fail-first shapes re-pointed at a
   foreign member's production/bare-string edges (still exactly 1 violation
   each, deps.rs:2840-2882); foreign-member and root-table negatives retained
   (:2884-2931). The real manifest act is recorded in
   gauntlet-cli/Cargo.toml:14-20. Still fail-closed for every other owner,
   section, and the root table. No standing-item interaction (see (c)).

2. **single_writer.rs inventory extension** — within the module's own
   three-rule discipline (central declaration, fail-closed, growth/stale
   pins, guard clause). The +3 surface entries (single_writer.rs:161-172,
   pins :939-944) name files that genuinely carry a surface token; the +9
   write pairs (:244-276, :366-391; pins :956-985, :1039-1052) match every
   raw write-API token in `gauntlet-cli/src` + `gauntlet-app/src` production
   text — all other matches sit inside `#[cfg(test)]` regions the blanker
   removes; every guard needle is present (`fn ensure_configured_dir`
   compose.rs; "refusing to overwrite" init_cmd.rs; `fn installed_root`
   skills_cmd.rs; `--out` state_cmd.rs ×6; `fn acquire` ×2 and
   `impl Drop for WriterLock` in writer_lock.rs). Rule C compares (file,
   token) pairs (:534-566), so compose.rs's two `create_dir_all` sites
   (:419, :524) are correctly one pair. Fixture repo updated (:1257-1300),
   keeping the check non-vacuous. No standing-item interaction: writer_lock
   guards the state dir, not the ruler; the reference side remains
   write-free in production.

## Operator attention (non-blocking, next-touch class)

- **Stale manifest prose contradicting the widened seal** (docs only;
  mechanisms correct and green): (1) gauntlet-app/Cargo.toml:16-17 — "no
  production dependency edge of this workspace enables it" is now false
  (gauntlet-cli/Cargo.toml:20 enables it); (2) gauntlet-cli/Cargo.toml:92-94 —
  the dev-deps comment still claims "the ONLY edge in the workspace … no
  production dependency does". Update both at the next manifest touch.
- **Stale count prose** in single_writer.rs:122-134 ("Six of the seven"
  surface files; "the seventh, skills/mod.rs") — the inventory is now ten
  entries after Task 20's +3. Same re-record class as task13's item (d) note.

## Summary verdict

**4 HOLD, 0 AMEND, 0 REOPEN** — both Task-20 arch-suite edits are within the
suite's own documented flows; 3 prose-staleness items flagged for operator
attention (next-touch class, trigger-recorded in the closure).
