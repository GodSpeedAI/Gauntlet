# Task 2 — builder, round 6

Scope: `crates/gauntlet-arch-tests/src/**` only (7 files), plus this report.
Nothing under `.wayfinder/`, `.gauntlet/`, `.jolli/`, or `.agents/CURRENT_STATUS.md`
was touched. One pre-existing working-tree edit outside my scope:
`.agents/plans/gauntlet-impl.md` line 578 (a URL note on the Task 9 clone step) —
not mine, left as-is for the orchestrator.

All paths relative to `/home/sprime01/projects/gauntlet/`.

## What changed, in one paragraph

`text.rs` now owns ONE Rust-aware lexing pass (`text::regions`) that delimits
string literals (plain/byte/raw/byte-raw), char literals (`'"'` included), line
comments, nested block comments, and decodes `\xNN`/`\u{NN}` escapes. The
denylist, the literal scanner, and the escape scanner all consume that pass
(`text::string_literals`, `text::string_literal_spans`,
`text::attribute_spans`, `text::bracket_end`); the per-class private quote
counters are deleted (`literals::string_literals`, and escapes' own
`plain_string_end`/`char_literal_end`/`prefixed_string_end`/
`string_literal_open`/`read_string_literal`/`attribute_body_end`/
`ends_with_quote`, plus its inline whole-file state machine). The §8.6 defaults
exemption is setting-scoped instead of file-scoped. Unreadable
`.cargo/config*` and unparseable `rust-toolchain*` are I8 violations. The two
unpinned exemption inventories are pinned. `check_scope_integrity` measures its
own scan. The threat model in `lib.rs` was updated to match — strengthened, not
weakened.

**Honest deviation from the brief, stated rather than forced:** the brief lists
`deps` and `envfile` among the classes that must "consume this one pass's
output". Neither ever consumed `text.rs` (verified: no `text::` reference in
either file) — `deps` parses TOML with the `toml` crate, `envfile` parses spec
Annex E. There was nothing to migrate; forcing a Rust lexer into a TOML/markdown
parser would be wrong, so the consumers migrated are the three that scan
Rust-like text: `denylist`, `escapes`, `literals`.

## Per-finding closure map

| finding | state | fix | pinned by (named regression in bold) |
|---|---|---|---|
| **c7-C1** defaults exemption is file-scoped | CLOSED | `walk::in_literal_scope` == denylist scope (no file exclusion); exemption applied INSIDE `literals::scan_literals` to scan units that DECLARE one of the ten §8.6 settings (`literals::is_defaults_setting_declaration`); `DECLARED_SCAN_EXEMPTIONS` emptied; warrant (`check_defaults_exemption`) now requires all TEN §8.6 settings, reads via `read_scannable` (the old `unwrap_or_default` treated an unreadable warrant as an empty one — an I8 hole closed in passing), and pins scope agreement file-for-file | **`c7c1_the_defaults_exemption_is_setting_scoped_not_file_scoped`** (lib.rs; the critic's exact edit: warranted table + model-id/command/absolute-path/endpoint/timeout/tool literals in the same file, each must fail); **`defaults_settings_exempt_only_their_own_declarations`** (literals.rs; other files not exempt, `MAX_ROUNDS_LOG` not a setting, unrelated literals in the defaults file policed); `walk::scope_decisions_match_the_declared_exclusions` (scope flip pinned) |
| **c7-C2** directive head split before `=`/`(` never joined | CLOSED | `text::statement_lines` gains `directive_head_awaits`: an include-family word as the line's last word (with/without `!`, path prefixes allowed) awaiting `(`/`!`, or an attribute head (`#[`, `#![`, whitespace-tolerant) awaiting `=`; joined under the existing 24-line loose budget. Also: `directive_arguments` now accepts `#![` inner attributes (found while testing — the matcher had the same blind spot) | **`directive_heads_split_before_their_argument_are_joined`** (text.rs); **`a_directive_head_split_before_its_delimiter_is_still_seen`** (escapes.rs; include/include_str/std::include/attribute/cfg_attr/inner-attribute spellings + in-crate negatives); **`c7c2_a_directive_head_split_before_its_argument_fails_the_gate`** (lib.rs, through `check_scope_integrity`) |
| **c6-C1** `string_literals` mis-pairs quotes | CLOSED | the one lexing pass reads `'"'`, raw strings, byte strings, and comments as data; `denylist`'s AK-014 segment rule and every literal class consume `text::string_literals` | **`a_quote_that_is_data_no_longer_deletes_following_literals`** (literals.rs; critic's exact fixtures + a same-line variant); **`adapter_segments_survive_a_char_literal_quote`** (denylist.rs; `trim_matches('"')` + `"mock"`/`"prime"`); **`c6c1_a_quote_that_is_data_does_not_hide_following_literals`** (lib.rs, block-comment form through `check_no_hardcoded_literals`); `text::quotes_that_are_data_do_not_shift_the_extraction` |
| **c6-C2** timeout class narrow by punctuation | CLOSED | introducer set extended to `:`, `>`, `[` (struct-field initialisers, match arms, array values); the hint requirement remains the real narrowing, and the module table now says so | **`struct_field_and_match_arm_values_introduce_a_timeout`** (literals.rs; `Timeouts { stall_ms: 900_000 }`, match arm, `[900_000]`, plus the prose negatives) |
| **c6-C3** `find_tier_policy` disabled by any `#[` | CLOSED | blanket `lowered.contains("#[")` replaced by a structural check: a literal is exempt only when it sits INSIDE an attribute body (`text::attribute_spans`, which is comment/string aware). A `#[` in a comment is not an attribute; an attribute earlier on the line no longer disables the class either | **`a_hash_in_a_comment_does_not_disable_the_tier_class`** (literals.rs; comment form, real-attribute-on-line form, and the `#[serde(rename = "cross_cutting")]` narrowing preserved); `text::attribute_spans_come_from_the_lexing_pass_not_substrings` |
| **c6-M1 / c7-MJ1** unreadable cargo config / unparseable toolchain pin silently skipped | CLOSED | `check_cargo_config`: read failure → violation; `check_toolchain_file` rewritten: absent → clean, unreadable → violation, `rust-toolchain.toml` unparseable → violation, legacy extensionless `rust-toolchain` → TOML, else a bare channel name (`stable`, `1.92.0`, `nightly-…`) is the one documented non-TOML form, anything else unparseable → violation | **`an_unreadable_or_unparseable_declared_build_input_is_a_violation`** (deps.rs; non-UTF-8 `.cargo/config.toml`, malformed `rust-toolchain.toml`, non-UTF-8 and garbage legacy `rust-toolchain`, bare-channel and pinned-channel negatives) |
| **c7-MJ2** `RELATIVE_PATH_EXEMPTIONS`/`ENDPOINT_EXEMPTIONS` unpinned | CLOSED | both pinned to exact contents with `assert_eq!`, exactly like `DENIED_TERMS` | **`exemption_inventories_are_pinned`** (literals.rs) |
| **c7-MJ3** `check_scope_integrity` has no coverage floor | CLOSED | the check now builds a `ScanReport` during its own loop and runs the shared `assert_scan_is_not_vacuous` — the same independent-listing floor, per-crate manifest+source floor, and skipped==0 assertion the other three repo-level checks carry, derived from the real tree, not a constant | **`scope_integrity_measures_its_own_scan`** (lib.rs; a core crate with no `.rs` and a non-UTF-8 core file both fail through THIS check) |

Folded in from the minors because the briefed work could not be honest without
them:

- **critic-7 m4** — `DEFAULTS_TABLE_SETTINGS` now lists all TEN §8.6 rows
  (`review_mode`, `sandbox_backend`, `interactive`, `git_strategy` added). The
  setting-scoped exemption cannot work without them: §8.6's own default for
  `review.mode` is `adaptive`, which is tier vocabulary and would have failed
  its own defaults table.
- **critic-7 m3** — `DECLARED_SCAN_EXEMPTIONS` pinned EMPTY in
  `walk::scope_tests`.
- **critic-6 m1** — closed as a side effect: `escapes`' argument reading now
  uses the shared decoder, so `#[path = "..\x2f..\x2fvendor\x2fevil.rs"]`
  resolves to the real path in the one place that reads it (previously only the
  separate escape-decoded unit pass caught it — that redundancy is no longer
  load-bearing, though the pass remains for escapes in code contexts). The new
  extractor is also UTF-8-safe (no Latin-1 mangling).
- Found while testing, no critic had it: `#![` inner attributes were invisible
  to the escapes directive matcher (`#` ws `[` only) — fixed and covered by the
  c7-C2 tests; and a `]` inside a comment inside an attribute body closed the
  body early — `bracket_end` is comment-aware, covered by
  `a_bracket_inside_a_comment_does_not_close_the_attribute_body` (escapes.rs).

## Existing tests: 159 → 175 (16 new), zero deleted

Three existing tests changed content, each a direct consequence of a briefed
design change, none weakened:

1. `walk::scope_decisions_match_the_declared_exclusions` — the assertion
   `!in_literal_scope(defaults.rs)` became `in_literal_scope(defaults.rs)`.
   That flip IS the c7-C1 fix; leaving the old assertion would have pinned the
   hole shut. Pins for the empty exemption inventory and the ten settings were
   added.
2. `an_undeclared_call_site_exemption_fails_the_gate` — the negative half
   ("the one declared exemption is honoured") pinned the OLD design; with the
   inventory empty, the test now asserts the stronger fact: even
   `DEFAULTS_MODULE` is refused as a file-level exemption, plus a
   no-exemption-passed negative. Guard machinery unchanged.
3. `v2_the_defaults_warrant_requires_declarations_not_mentions` — fixture
   extended from six consts to the full ten-row §8.6 table (the warrant got
   stronger); the mention-shaped rejection and the policed-sibling assertions
   are unchanged and still pass.

## Teeth, verified by mutation (each: break → named test FAILS → revert → green)

**1. The lexer (c6-C1).** Mutation: `text::regions` char-literal arm replaced
with an ordinary byte advance — the pre-round-6 quote counting.

```
mutated → FAIL denylist::scanner_tests::adapter_segments_survive_a_char_literal_quote
          (panicked: no 'mock'/'prime' among violations)
mutated → FAIL literals::scanner_tests::a_quote_that_is_data_no_longer_deletes_following_literals
          (panicked: same-line "state/gauntlet.db" literal not flagged)
revert  → PASS both + text::quotes_that_are_data… + evasion c6c1… (4/4)
```

Note on honesty: the literals-side test as first written had its literals on
OTHER lines than the char literal, so the mutation did NOT make it fail — only
the denylist test bit. Per the brief ("a mutation that fails to make a test
fail means the test has no teeth — fix the test") a same-line variant was added
before recording this as closed.

**2. The setting-scoped exemption (c7-C1).** Mutation: the unit skip widened
from `defaults_scope && is_defaults_setting_declaration(...)` to `defaults_scope`
(file scope again).

```
mutated → FAIL literals::defaults_settings_exempt_only_their_own_declarations (panicked: [])
mutated → FAIL evasion_tests::c7c1_the_defaults_exemption_is_setting_scoped_not_file_scoped
revert  → PASS both (2/2)
```

**3. The inventory pin (c7-MJ2).** Mutation: one row
`("../../shared/wiring.rs", "…")` prepended to `RELATIVE_PATH_EXEMPTIONS` — the
critic's exact "one added row un-polices a literal forever" edit (the row even
satisfies the reason-warrant test, which is exactly why a reason is not a gate).

```
mutated → FAIL literals::exemption_inventories_are_pinned
          (assertion `left == right` failed at the pin)
revert  → PASS
```

## Gate output tail (run in this order, after all teeth reverts)

```
just check       → Finished `dev` profile … 0
just lint        → clippy … -D warnings clean; cargo fmt --check clean; 0
just deny        → advisories ok, bans ok, licenses ok, sources ok; 0
just boundaries  → Summary: 175 tests run: 175 passed, 0 skipped
just ruler       → verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
```

`just ruler` was also run FIRST (PASS, before any edit). `du -sh target`:
367M (ceiling not approached; no `cargo clean`).

## Threat model (lib.rs), updated to match reality — strengthened only

Additions: the one-lexing-pass sentence (quotes that are data cannot shift what
scanners see; no class carries its own quote counting); directive-head splits
are joined; the defaults exemption is setting-scoped; unreadable cargo
configurations and toolchain pins join the I8 list. No existing claim was
removed or weakened. The residual list stands (see below).

## Known gaps / not done — stated plainly

- **critic-6 M2** (`Command::new(` reports `literals.first()`, positional not
  the argument): NOT briefed (round-6 majors were MJ1/MJ2/MJ3) and not done.
  The class still catches the laundering half via tool-identity, but its
  message can still name the wrong string. Recorded, not silently dropped.
- **critic-6 M3** (adapters + `gauntlet-arch-tests` outside
  `walk::CORE_CRATES`, so `scan_escapes`/`build_script_violation` never read
  them): not briefed, not widened. The threat model's "every in-tree file under
  the four core crates" remains the honest scope statement.
- **critic-6 m2** (`CONFIG_WALK_SKIPPED_DIRS` name-based carve-out) and
  **critic-6 m3** (tier coverage of function bodies is an accident of
  `&'static str`): unchanged, both already disclosed in module docs.
- **critic-6 suspicion 2** (`[alias]` absent from `FORBIDDEN_CARGO_CONFIG_KEYS`):
  an unconfirmed suspicion in the critic's own words; left for the next critic
  round rather than half-closed here.
- **Lexer residual, documented beside `text::Region`**: in NON-Rust text
  (TOML/MD/JSON are scanned too), a `//` directly preceded by `:` is treated as
  a URL divider, so a line comment in non-Rust text spelled `x://c` skips the
  rest of that one line for string extraction — a bounded lose-only residual.
- **The tier class is still line-scoped** (`const`/`static`/`default` gate):
  the complete §8.1 reading still needs a real parser; the module's residual
  note stands.
- **Enumerated lists remain enumerated** and runtime string assembly remains
  undetectable — unchanged, as recorded in the threat model.
