# Task 2 — builder, round 6.1

Scope: `crates/gauntlet-arch-tests/src/**` only, plus this report. Nothing
under `.wayfinder/`, `.jolli/`, `.gauntlet/`, `.agents/CURRENT_STATUS.md`, or
`.agents/plans/*` was touched. Not committed — the orchestrator commits.

Exactly three findings closed: critic-8 E1, critic-8 E2, critic-9 F2. Every
other finding in the two critic reports (critic-8 E3–E12, critic-9 F1, F3–F12)
is a recorded deferral, out of this brief, left untouched. Note for
settlement: critic-8's E7 and critic-9's F2 are the same defect — the
declaration-granular fix below closes both, and the report says so wherever
"F2" appears.

All paths relative to `/home/sprime01/projects/gauntlet/`.

## Per-finding closure map

### E1 (critic-8, critical) — comment tokens defeat the escape matchers — CLOSED

**Fix, matcher half** (`escapes.rs`). Every gap the directive matchers step
over — include-word→`!`, `!`→`(`, `(`→argument, `#`→`[`, `#!`→`[`,
`path`→`=`, `=`→argument — now goes through one new helper, `skip_trivia`,
which skips whitespace AND `/* … */` block comments, taking the comment spans
from the existing lexing pass (`crate::text::regions`), so nesting is handled
and a comment inside a string/char literal stays data. The old space-only
`skip_space` is deleted. rustc's lexer treats comments as token trivia; the
matchers now skip exactly what the compiler skips, so
`#[path/*c*/= "my dir/../../../vendor/store.rs"]` and
`include!/*c*/("my dir/../../../shared/wiring.rs")` resolve to their real
targets and are reported as scope escapes. Line comments are deliberately
NOT skipped: a scan unit never contains the newline that ends one, so
"skip a line comment" would mean "skip to the end of the unit", which can
only lose evidence; an argument hidden behind one in a joined unit lands in
`Argument::Unresolvable` — the fail-closed direction, unchanged.

**Fix, backstop half** (`literals.rs`). `looks_like_relative_path` now flags
any string literal containing an explicit `..` parent-directory segment,
whatever its other segments look like. The critic's
`"my dir/../../../vendor/store.rs"` navigates the filesystem exactly like
`../vendor/store.rs`; the `..` IS the navigation, and one space-bearing
segment had made the whole literal read as prose to the
every-segment-filename-shaped rule. Escape detection no longer depends on
the path being well-formed single-segment path material: in the escape
scanner it never did (`normalize_join` splits on `/` and treats any segment
as a name); in the literal class it no longer does for the `..`-bearing
family.

**Named regressions.**

- `evasion_tests::e1_comment_tokens_between_directive_and_argument_fail_the_gate`
  (lib.rs, through `check_scope_integrity` / `check_no_hardcoded_literals`):
  the critic's EXACT payloads — `#[path/* wiring */= "my
  dir/../../../vendor/store.rs"]` and `include!/* wiring */("my
  dir/../../../shared/wiring.rs");` — both fail the gate; plus the
  `..`-bearing literal alone fails `check_no_hardcoded_literals`; plus an
  in-crate negative (the same comment forms with in-crate targets stays
  clean).
- `escapes::tests::comment_tokens_between_a_directive_and_its_argument_are_skipped`:
  the two exact payloads plus comments in every other gap (`#/*c*/[`,
  `#!/*c*/[`, `include/*c*/!`, `include!/*c*/(/*c*/ …)`,
  `path/*c*/=/*c*/`, `#[ path /*c*/ = …]`, cfg_attr bodies), with in-crate
  negatives for the same spellings.
- `literals::scanner_tests::parent_directory_navigation_is_path_shaped_regardless_of_other_segments`:
  the space-bearing `..` literals flag as relative paths; the prose negatives
  (`"either this / or that"`, `"key: value/other"`, `"and/or"`) stay clean.

### E2 (critic-8, critical) — `[env]` table delivers compiler flags — CLOSED

**Source for the semantics** (the brief required stating it): The Cargo Book,
`reference/config.html`, `[env]` section — "The `[env]` section allows you to
set additional environment variables for build scripts, rustc invocations,
cargo run and cargo build" (verified against the live doc this round). The
flag/binary names come from `reference/environment-variables.html`: RUSTFLAGS
and CARGO_ENCODED_RUSTFLAGS are "custom flags to pass to all compiler
invocations that Cargo performs"; CARGO_BUILD_RUSTFLAGS is the env spelling of
`build.rustflags`; RUSTDOCFLAGS/CARGO_ENCODED_RUSTDOCFLAGS the same for rustdoc
invocations; RUSTC substitutes the compiler binary; RUSTC_WRAPPER /
RUSTC_WORKSPACE_WRAPPER wrap every rustc invocation; CARGO_TARGET_<triple>_
RUSTFLAGS is the per-target rustflags spelling. LD_PRELOAD and
DYLD_INSERT_LIBRARIES are justified by the critic's own argument: the dynamic
loader injects them into every process cargo spawns — rustc, the linker, and
the test binaries this gate reports on — the channel the `runner` ban exists
to close.

**Fix** (`deps.rs`). A new pinned inventory `FORBIDDEN_ENV_KEYS` (14 names,
each with its stated reason, `forbidden_env_keys_state_a_reason` pins both
the reason bar and the six brief-mandated names), a helper
`forbidden_env_reason` that adds the unlistable `CARGO_TARGET_*_RUSTFLAGS`
family by prefix+suffix, and `env_table_violations(&table, rel)` wired into
`check_cargo_config` beside the `[source]`/`[registries]` checks. An
`[env]` entry is denied wherever the config sits in the tree (discovery is
unchanged and already walks everywhere). The `[env]` table itself is NOT
denied — a plain `GAUNTLET_*` (or `OPENSSL_DIR`) entry stays clean; only the
names that reach the compiler are.

**Case rule** (the brief: "key matching follows cargo's real case rules").
Cargo's config KEYS are exact-match TOML lookups — `[build] Rustflags` is not
read as `rustflags` — so the generic `walk_cargo_config` stays case-sensitive,
and that is now documented at the `[env]` matcher. `[env]` CHILD keys are
environment-variable names, and environment-variable lookup is
case-insensitive on Windows (a host this suite acknowledges in its
drive-letter path rules) and case-sensitive on Unix; the fail-closed union is
ASCII-case-insensitive matching, so `rustflags`, `RustFlags`, and `RUSTFLAGS`
are all denied. Denying the channel, not one spelling, introduces no false
positives: an `[env]` var named `rustflags` in any case IS the compiler
channel.

**Named regressions.**

- `evasion_tests::e2_a_compiler_affecting_env_entry_fails_the_gate` (lib.rs,
  through `check_dependency_direction`): the critic's EXACT payload —
  `[env] RUSTFLAGS = { value = "--extern shadow=/home/u/vendor/libshadow.rlib
  --cfg injected", force = true }` — fails the gate; a benign
  `[env] GAUNTLET_LOG = { value = "debug", force = true }` config stays
  clean.
- `deps::scanner_tests::compiler_affecting_env_entries_are_flagged_and_benign_ones_are_not`:
  the exact payload plus lowercase/mixed-case smuggling, the encoded and
  build-section spellings, `RUSTC`/wrappers, rustdoc flags, the per-target
  family, both dynamic-loader channels; benign `GAUNTLET_LOG` /
  `OPENSSL_DIR` / `relative`-flag entries stay clean; no `[env]` table at all
  reports nothing.
- `deps::scanner_tests::forbidden_env_keys_state_a_reason` (inventory pin).

### F2 (critic-9 F2 = critic-8 E7, major) — exemption unit-granular, not declaration-granular — CLOSED

**Fix** (`literals.rs`). `scan_literals` no longer SKIPS a defaults-scope
unit that declares a setting; it MASKS it. New helpers
(`setting_declaration_spans`, `mask_setting_declarations`,
`push_if_setting_declaration`) split the unit at CODE semicolons — a `;`
inside a string/char/comment region of the one lexing pass is data and never
ends a declaration — and blank out exactly the chunks that pass the same
`is_defaults_setting_declaration` shape test the warrant uses, replacing each
declaration's bytes with spaces (length and line numbers unchanged, so no
reported position moves). The setting's own declaration, head to its
terminating `;`, is the exemption; everything else in the unit — including a
second declaration packed onto the same physical line — is policed by every
class. The warrant and the exemption still share one predicate, so they
cannot drift apart, and `check_defaults_exemption` itself needed no change:
the packed line still DECLARES the setting, which is precisely why the old
unit-scoped skip was a smuggle.

**Named regression.**

- `evasion_tests::f2_the_defaults_exemption_covers_only_the_setting_declaration`
  (lib.rs, through `check_defaults_exemption` +
  `check_no_hardcoded_literals`): the critic's exact payload — nine clean
  settings plus `pub const MAX_ROUNDS: usize = 12; pub const BIND: &str =
  "localhost:8080";` on one line — BIND flagged
  (`hardcoded-network-endpoint` … `localhost:8080`), MAX_ROUNDS clean (no
  violation mentions it), and the warrant itself still green.
- `literals::scanner_tests::the_defaults_exemption_covers_only_the_setting_declaration`:
  the packed line; the reversed order (smuggled declaration leads — the mask
  is positional, not line-wide); three declarations on one line (the two
  non-settings flagged by their own classes); a data semicolon inside the
  setting's own value string (does not end the exempt declaration); and the
  setting declaration alone stays clean.
- The round-6 c7-C1 regressions
  (`c7c1_the_defaults_exemption_is_setting_scoped_not_file_scoped`,
  `defaults_settings_exempt_only_their_own_declarations`,
  `v2_the_defaults_warrant_requires_declarations_not_mentions`,
  `walk::scope_decisions_match_the_declared_exclusions`) pass UNCHANGED.

## Existing tests: 175 → 183 (8 new), zero deleted, zero weakened

No existing test changed content. The only edits to existing code are the
mechanisms themselves plus their docs (module docs in `escapes.rs`,
`literals.rs`, `deps.rs`, the lib.rs threat model, `walk.rs`
`DEFAULTS_MODULE` docs) — all strengthened to describe the new behaviour.

New tests (8): the two E1 scanner tests, the two E2 deps tests, the F2
literals test, and the three entry-point regressions `e1_…`, `e2_…`, `f2_…`.

## Teeth, verified by mutation (each: break → named test FAILS → revert → green)

**1. E1 — comment-skipping reverted.** Mutation: `skip_trivia` body replaced
with the pre-fix space-only loop.

```
mutated → FAIL escapes::tests::comment_tokens_between_a_directive_and_its_argument_are_skipped
mutated → FAIL evasion_tests::e1_comment_tokens_between_directive_and_argument_fail_the_gate
          (panicked: "scope-integrity: no violations")
mutated → PASS literals::parent_directory_navigation_is_path_shaped_…
          (correct: that test pins the independent literal-class backstop)
revert  → green (183/183)
```

**2. E2 — the `[env]` entry removed.** Mutation: the
`out.extend(env_table_violations(&table, rel));` call commented out in
`check_cargo_config`.

```
mutated → FAIL evasion_tests::e2_a_compiler_affecting_env_entry_fails_the_gate
          (panicked: "dependency-direction: no violations" — the critic's payload sails through)
mutated → PASS deps::compiler_affecting_env_entries_are_flagged_…
          (correct: the mutation is in the WIRING; the unit test drives the
           helper directly — the entry-point test is the one that bites)
revert  → green (183/183)
```

**3. F2 — exemption widened back to unit scope.** Mutation: the masking
branch replaced with the round-6 `continue` on
`is_defaults_setting_declaration(&unit.text)`.

```
mutated → FAIL literals::the_defaults_exemption_covers_only_the_setting_declaration
mutated → FAIL evasion_tests::f2_the_defaults_exemption_covers_only_the_setting_declaration
          (panicked: "hardcoded-literal: no violations" — BIND sails through)
mutated → PASS evasion_tests::c7c1_the_defaults_exemption_is_setting_scoped_not_file_scoped
mutated → PASS literals::defaults_settings_exempt_only_their_own_declarations
          (correct and important: the round-6 regressions pin the weaker
           setting-scoped property, which the mutation also satisfies — the
           new declaration-granular regressions are what carry the teeth)
revert  → green (183/183)
```

## Gate output tail (run in this order, after all teeth reverts)

```
just check       → Finished `dev` profile [unoptimized + debuginfo] target(s) (exit 0)
just lint        → clippy -D warnings + cargo fmt --check, zero warnings/errors (exit 0)
just deny        → advisories ok, bans ok, licenses ok, sources ok (exit 0)
just boundaries  → Summary: 183 tests run: 183 passed, 0 skipped (exit 0)
just ruler       → verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (exit 0)
cargo nextest run --workspace → 183 tests run: 183 passed, 0 skipped
```

`just ruler` was also run FIRST (PASS, before any edit). No `cargo clean`; no
parallel cargo invocations.

## Honest gaps / not done

- **Split directive heads with trailing comments** (declared, added to the
  lib.rs residual list and the `escapes.rs` module docs): a block comment at
  the END of a directive head split across lines — `include! /*c*/`⏎`("…")`
  or `#/*c*/[path`⏎`= "…"]` — still defeats the round-6 head-join, because
  the join keys on the head line's LAST WORD and a trailing comment replaces
  it. Same-unit comment gaps (the critic's payloads, and every gap the
  matchers step over) are closed; the cross-line-with-comment corner is not.
- **`[env]` names that reach the compiler only transitively** are not denied:
  `PATH` and `LD_LIBRARY_PATH` redirection change which `cargo`/`rustc` and
  which libraries the build resolves, but both have ordinary non-compiler
  uses inside an `[env]` table; denying them is a policy call recorded here
  rather than taken. The 14 exact names plus the `CARGO_TARGET_*_RUSTFLAGS`
  family are the direct channels.
- **`CARGO_BUILD_RUSTDOCFLAGS`, `CARGO_BUILD_RUSTC*`** ARE denied (they are
  the documented env spellings of already-denied keys) — noted here only so
  the inventory growth beyond the brief's six names is visible as deliberate.
- The `[env]` matcher consults the configuration-ROOT `env` table only, which
  is where cargo reads it; a table named `env` nested under some other
  section is not cargo's environment table (the generic key walk still
  polices its contents for forbidden config keys).
- E3, E4, E5, E6, E8–E12 (critic-8) and F1, F3–F12 (critic-9): out of this
  brief, untouched, recorded as deferrals by the orchestrator.
