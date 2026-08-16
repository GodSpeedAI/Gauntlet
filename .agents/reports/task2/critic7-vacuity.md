# critic7 — vacuity, teeth, and self-exemption

VERDICT: residuals-found (2 critical / 3 major / 6 minor)

Lens: which checks cannot fail, and which exempt the thing they exist to police.
Read-only review; no build or test command was run. Any claim that would need a
compile is labelled UNCONFIRMED.

---

## Answers to the lens questions, up front

**Is the walker pinned to the real tree?** Yes — this class of criticism does not
apply. `walk.rs:396 the_independent_listing_agrees_with_the_walker_on_the_real_repo`
compares `unfiltered_listing` against `core_tree` on `repo_root()`;
`lib.rs:1760 every_scanned_file_gets_the_role_its_layer_requires` drives
`scan_core_tree` over `repo_root()` and asserts `skipped == 0`, a non-empty
result, and `inner > 0 && composition > 0`; and all four `tests/contract/*.rs`
locators plus the `#[cfg(test)] mod repo` blocks in `deps.rs`, `denylist.rs` and
`envfile.rs` call `repo_root()`. The suite is not fixture-only.

**Do the four AK locators invoke the checks, or merely mention them?** They
invoke them. `tests/contract/architecture_boundaries.rs` calls
`check_dependency_direction`, `check_scope_integrity`, `check_denylist`;
`adapter_registry.rs` calls `check_adapter_selection`;
`no_hardcoded_literals.rs` calls `check_no_hardcoded_literals` and
`check_defaults_exemption`; `env_example_complete.rs` calls `check_env_example`.
`lib.rs:1689` additionally pins the four `[[test]]` blocks by name and path.

**Are the anti-vacuity guards load-bearing?** Mostly yes. The walker-level guard
in `assert_scan_is_not_vacuous` (`lib.rs:358-371`) measures the walk against an
independent listing and would fire on a carve-out inside `collect_files`. The
`deny.adapter.is_empty()` guards (`lib.rs:449`, `lib.rs:497`), the expected-member
list (`lib.rs:168-186`), the derived AK-019 floor (`envfile.rs:281-298`) and the
declared-exemption warrant (`lib.rs:337-352`) all have real teeth. Two are close
to tautological — see m5 and the note under MJ3. The scope constants
(`CORE_CRATES`, `INNER_CRATES`, `COMPOSITION_ROOT`, `DEFAULTS_MODULE`) are pinned
by hard-coded paths in `walk::scope_tests`, so quietly dropping a crate from a
scope fails a unit test.

**Full inventory of exemptions, allowlists and skip paths** (each is assessed in
the findings that follow, or dismissed here):

| # | exemption / skip | where | assessment |
|---|---|---|---|
| 1 | `DEFAULTS_MODULE` — literal scan off for one file | `walk.rs:53`, `lib.rs:566` | **C1 / C2** |
| 2 | `DECLARED_SCAN_EXEMPTIONS` | `walk.rs:63` | m3 (unpinned, but not sufficient alone) |
| 3 | `RELATIVE_PATH_EXEMPTIONS`, `ENDPOINT_EXEMPTIONS` | `literals.rs:176,198` | **MJ2** (unpinned) |
| 4 | `DENIED_TERMS` | `denylist.rs:92` | content-pinned (`denylist.rs:536`) — OK |
| 5 | `RUNTIME_INJECTED` | `envfile.rs:55` | pinned to len 1 (`envfile.rs:637`) — OK |
| 6 | `TARGET_PATH_EXEMPT_CRATES` | `escapes.rs:76` | m2 (dead in production) |
| 7 | external allowlists (5 empty + `toml`) | `deps.rs:113-139` | reason-gated (`deps.rs:1579`) and source-gated (`deps.rs:493`) — OK |
| 8 | composition-root adapter mask | `denylist.rs:262` | declared; scoped exactly to `gauntlet-cli` (`walk.rs:277`, cross-checked at `lib.rs:544-555`) — as briefed, not reported |
| 9 | `CONFIG_WALK_SKIPPED_DIRS = ["target", ".git"]` | `deps.rs:1020` | m7 |
| 10 | unreadable `.cargo/config*` → `continue` | `deps.rs:978` | **MJ1 (I8)** |
| 11 | unparseable `rust-toolchain*` → `continue` | `deps.rs:1099` | **MJ1 (I8)** |
| 12 | `scan_unterminated` skips non-`.rs` | `escapes.rs:238` | declared; low value, not reported |
| 13 | adapters + `gauntlet-arch-tests` outside `CORE_CRATES` | `walk.rs:20` | declared self-exemption; the crate's own manifest *is* policed (`Layer::BoundaryTests`) and no layer may depend on it — OK |

---

## CRITICAL

### C1 — the declared-defaults exemption is file-scoped, but its warrant only checks six names

**What is wrong.** `check_no_hardcoded_literals` (`lib.rs:560-570`) passes
`walk::in_literal_scope` as its scope predicate. `scan_core_tree` skips an
out-of-scope file *before reading it* (`lib.rs:275-277`), so for
`crates/gauntlet-app/src/config/defaults.rs` **all thirteen literal classes are
off, not just the ones §8.6 needs**. The only thing that warrants that
blanket-off is `check_defaults_exemption` (`lib.rs:618-632`):

```rust
let text = std::fs::read_to_string(&path).unwrap_or_default();
let lowered = text.to_lowercase();
for setting in walk::DEFAULTS_TABLE_SETTINGS {
    if !lowered.contains(&format!("const {setting}")) { /* violation */ }
}
```

That is a **presence** test over six substrings. Nothing bounds what *else* the
file declares. The doc comment above it says the exemption is for "the single
place conservative default VALUES are permitted to be literals" and §8.1 says
"Conservative defaults live in one declared defaults table (§8.6)" — but the
implementation grants the exemption to a *file*, not to the §8.6 *settings*.

**The exact edit that keeps `just boundaries` green.**

`crates/gauntlet-app/src/lib.rs`: add `pub mod config;`
`crates/gauntlet-app/src/config.rs`: `pub mod defaults;`
`crates/gauntlet-app/src/config/defaults.rs`:

```rust
pub const MAX_ROUNDS: usize = 8;
pub const MAX_PARALLEL: usize = 2;
pub const MAX_DEPTH: usize = 1;
pub const MAX_WALL_SECONDS: u64 = 3600;
pub const RECOVERY_RESERVE_PCT: u8 = 15;
pub const DIAGNOSTIC_BUDGET: u64 = 2;

pub const MODEL: &str = "gpt-5-codex";                 // hardcoded-model-id
pub const AGENT_CMD: &str = "just verify --workspace"; // hardcoded-command
pub const STATE_ROOT: &str = "/var/lib/gauntlet/state";// hardcoded-absolute-path
pub const API_BIND: &str = "localhost:8080";           // network-endpoint + port
pub const REVIEW_MODE: &str = "adversarial";           // hardcoded-tier-policy
pub const STALL_TIMEOUT_MS: u64 = 900_000;             // hardcoded-timeout
pub const TOOL: &str = "kubectl";                      // hardcoded-tool-identity
```

**Trace.**
- `in_literal_scope` returns `false` (`walk.rs:270`), so `scan_core_tree`
  `continue`s at `lib.rs:275` and `literals::scan_literals` never sees the file.
  Every one of those seven strings is proven to fire *elsewhere* by the suite's
  own fixtures (`lib.rs:1024-1117`, `lib.rs:1607-1671`).
- `check_defaults_exemption` passes: the six `const <setting>` substrings are all
  present; the loop at `lib.rs:601-617` finds no *other* file that the two scopes
  treat differently.
- `check_denylist` *does* still scan the file — and none of those literals
  contains a `DENIED_TERMS` entry (`gpt`, `codex`, `kubectl`, `adversarial`,
  `localhost` are all absent from the list), and none of them equals an adapter
  segment (`denylist.rs:280`; the real workspace derives `mock, prime, cli,
  sqlite, fs, filesystem, git, build, test, command, diff`).
- `check_scope_integrity` finds no `#[path]`, no `include!`, no `build.rs`.

Net: eight of the nine classes §8.1 enumerates are hardcoded in the application
layer, and `just boundaries` is green.

**Would any existing test notice?** No. `v2_the_defaults_warrant_requires_
declarations_not_mentions` (`lib.rs:1270`) asserts the opposite direction only:
that a file *without* the six declarations is rejected, and that a file *with*
them is accepted (`lib.rs:1298-1302`) — its accepting fixture contains nothing
but the six consts, so the unbounded remainder is never exercised.

**Suggested direction** (not a required fix): make the exemption
setting-scoped rather than file-scoped — exempt only the *value* on a line whose
identifier is one of the §8.6 settings, and report every other literal in the
file normally. Note also that `DEFAULTS_TABLE_SETTINGS` (`walk.rs:73`) covers
six of §8.6's ten rows; `review.mode`, `sandbox.backend`, `interactive` and
`git_strategy` are absent (m4).

---

### C2 — `scan_escapes` matches per scan unit, and nothing joins a directive head split *before* its `=` / `(`

**What is wrong.** `escapes::scan_escapes` requires the directive **and** its
argument to appear in one `crate::text::scan_unit` (`escapes.rs:101-114`). The
only joiner that can produce such a unit is `text::statement_lines`
(`text.rs:182-216`), which joins a line only when

- `opens_continuation(trimmed)` — the last byte is one of `= ( , [ +`
  (`text.rs:146-151`), or
- `declaration_head(trimmed)` and the next line starts with `=` or `:`
  (`text.rs:157-167`, `text.rs:193-194`) — and `declaration_head` returns `false`
  for anything whose first token is not `pub`/`async`/`unsafe`/`extern`/`pub(…)`/
  `const`/`static`/`let`.

`include!` ends on `!`. `#[path` ends on `h`. Neither is a declaration head. So a
directive head placed on its own line is **never** joined to the line carrying
its argument, and `directive_arguments` (`escapes.rs:168-210`) sees two units,
neither of which contains a `(directive, argument)` pair:

- unit `include!` — `escapes.rs:186-190` finds the `!`, then
  `include_argument(raw, 8)` (`escapes.rs:490-497`) finds no `(` and returns
  `None`;
- unit `("../…/wiring.rs");` — contains no `include` token and no `#`, so both
  matcher loops produce nothing.

The `#[path`↵`= "…"]` spelling fails the same way: `attribute_body_end`
(`escapes.rs:360`) hits an unbalanced `[`, returns `bytes.len()`, and
`attribute_path_arguments("#[path")` (`escapes.rs:463-483`) requires `=`
immediately after `path`, which is on the next physical line. `scan_unterminated`
(`escapes.rs:237`) sees the `[` and `]` balanced *across the file* and reports
nothing. This is exactly the critic-4-M1 shape that was fixed for `const`
declarations (`text.rs:193`) and never generalised to attributes or macro calls.

**Why this is only a MAJOR on its own, and a CRITICAL in combination.** In the
four core crates the escape's *argument line* is normally caught by an unrelated
class in a different check: `literals::looks_like_relative_path`
(`literals.rs:734-761`) flags any literal starting `../`, so the gate goes red —
with a `hardcoded-relative-path` message, not a `scope-escape` one. That
backstop is turned off in exactly one file: the C1 defaults module. (It can also
be turned off for one exact literal by adding an entry to
`RELATIVE_PATH_EXEMPTIONS` — see MJ2.)

**The exact edit that keeps `just boundaries` green.** On top of C1's
`config/defaults.rs`, append:

```rust
include!
    ("../../../../shared/wiring.rs");
```

and create `<repo>/shared/wiring.rs` containing anything at all — vendor
identifiers, adapter names, adapter selection by literal, absolute paths, model
ids, provider identities.

**Trace.**
- `check_scope_integrity` (`lib.rs:224-248`) reaches the file
  (`in_denylist_scope` is `true` for it) and calls `scan_escapes`, which as traced
  above reports nothing for either physical line; `logical_lines`
  (`text.rs:63-116`) collapses only `\`-continuations and `","`-adjacent string
  fragments, so it produces the same two units; `statement_lines` produces none;
  `declaration_units` produces none.
- `check_no_hardcoded_literals` never reads the file (C1), so
  `"../../../../shared/wiring.rs"` is never seen by the relative-path class.
- `check_denylist` reads the file but the only new text is
  `include!` and the path string — no `DENIED_TERMS` entry, no adapter segment.
- `<repo>/shared/wiring.rs` is under none of `walk::CORE_CRATES`, so
  `core_tree` never lists it, `unfiltered_listing` never lists it, and
  `discover_package_dirs` (`deps.rs:789`, walks `crates/**` only) never sees it.
  It compiles into `gauntlet-app` and no scanner reads a byte of it.

The `../` count above depends on rustc's module-directory rule for `include!`
inside a non-`mod.rs` file; the number is irrelevant to the finding, because the
scanner never gets as far as resolving the argument. UNCONFIRMED without a
compile: that rustc accepts the newline between `include!` and `(` — it does,
whitespace between tokens is insignificant, and the crate's own docs already
concede the `include !` spelling at `escapes.rs:55-56`.

**Would any existing test notice?** No. `v3_out_dir_codegen_and_build_scripts_
fail_the_gate` (`lib.rs:814`) and `v2_a_cfg_attr_path_attribute_fails_the_gate`
(`lib.rs:789`) both use single-line spellings, and the multi-line `cfg_attr(`
fixture that *is* covered works only because `(` is an `opens_continuation`
token.

**Suggested direction.** Run `directive_arguments` over a whole-file token
stream (or extend `statement_lines` to treat `#[`, `#![` and `ident!` as heads
that await their delimiter), and treat a directive head whose argument cannot be
located in the same unit as `Argument::Unresolvable` — the module already has the
right failure mode, it just never reaches it.

---

## MAJOR

### MJ1 — I8 violated: unreadable cargo configuration and unparseable toolchain pins are treated as clean

`deps.rs:978`:

```rust
let Ok(text) = std::fs::read_to_string(&path) else {
    continue;
};
```

`deps.rs:1099`:

```rust
let Ok(table) = parse_table(&text) else {
    continue;
};
```

Both are silent skips. This directly contradicts the crate's own threat model
(`lib.rs:65-69`: "Input the suite cannot read — an unreadable, over-sized,
non-UTF-8, or symlinked file … — is reported as a violation, never skipped") and
the sibling branch three lines above the first one, which *does* report malformed
TOML as a violation (`deps.rs:984`). It is also inconsistent with
`walk::read_scannable`, which treats exactly these conditions as violations.

The rationalisation at `deps.rs:1097-1098` ("the legacy `rust-toolchain` file may
be a bare channel name") is true for the extensionless spelling only; the
`continue` applies to `rust-toolchain.toml` as well.

**What would have to happen for it to hide a real violation:** a
`.cargo/config.toml` that `is_file()` but is unreadable to the test process
(mode/ACL, a mount point) or is not valid UTF-8. Cargo would very likely reject
such a file too, so I do not claim a live evasion — but "the input we could not
read is clean" is the exact pattern I8 exists to forbid, and here it is the
channel the suite calls its own highest-value one (`deps.rs:59-68`). No existing
test covers either branch: `deps.rs:1822` covers a *missing directory*, not an
unreadable file.

### MJ2 — `RELATIVE_PATH_EXEMPTIONS` and `ENDPOINT_EXEMPTIONS` are the only exemption lists with no content pin

Every other inventory in the suite is pinned to its exact contents:
`DENIED_TERMS` (`denylist.rs:536`), `RUNTIME_INJECTED` (`envfile.rs:637`),
`TARGET_PATH_EXEMPT_CRATES` (`escapes.rs:1060`). These two are gated only by
`relative_path_exemptions_state_a_reason` (`literals.rs:1437`) and
`endpoint_exemptions_state_a_reason` (`literals.rs:1495`), which assert that the
second tuple element is non-empty. A reason string is not a gate.

Matching is whole-literal equality (`literals.rs:737`,
`eq_ignore_ascii_case(literal.trim())`), so **one added row un-polices exactly one
literal, forever, with the gate green**:

```rust
("../../../../shared/wiring.rs", "the boundary suite's shared fixture root"),
```

That single line is the second route to C2 — it removes the relative-path
backstop for that literal in *every* core crate, not just in the defaults module.
Nothing would notice: no test enumerates the list's contents, and the "state a
reason" test iterates the list, so it grows in lockstep with it — the same
lockstep defect that `denied_terms_inventory_is_pinned` was written to close
(`denylist.rs:529-534`).

**Fix shape:** pin both lists with `assert_eq!` on their full contents, as
`DENIED_TERMS` already is.

### MJ3 — `check_scope_integrity` is the only repo-level check with no coverage floor

`check_scope_integrity` (`lib.rs:224-248`) walks `tree.files`, filters with
`in_denylist_scope`, and returns. It does not build a `ScanReport`, does not call
`assert_scan_is_not_vacuous`, and asserts nothing about how many files it
examined. If the walker or the predicate ever narrowed, this check would go quiet
and say so by saying nothing — the precise failure mode `scan_core_tree` was
restructured to prevent (`lib.rs:251-257`: "Folding it in makes deleting the
guard mean deleting it for every check at once").

Today the narrowing would still be caught, but only *indirectly*, by the guard
inside the other three checks. That is coverage by coincidence: `check_scope_
integrity` is the check that enforces "what gets scanned must equal what
compiles", and it is the one with no self-measurement. It should route through
`scan_core_tree` (or carry its own floor: manifest + ≥1 `.rs` per crate,
`skipped == 0`).

Related, and part of why this matters: the per-crate floor the other checks *do*
carry is "the crate's `Cargo.toml` was scanned and at least one `.rs` under it was
scanned" (`lib.rs:395-411`). All four core crates today are doc-comment-only
stubs (8 files, ~96 lines, one executable function in `gauntlet-cli`), so that
floor is satisfied by a one-line file and will stay satisfiable by a stub through
Tasks 3–20. The floor proves the walk reached the crate; it proves nothing about
how much of the crate exists.

---

## MINOR

**m1 — `walk.rs:196` binds an unused `err`.**

```rust
Err(err) if !dir.exists() => return,
Err(err) => panic!("{} is readable: {err}", rel_of(root, dir)),
```

The first arm never uses `err`, which should raise `unused_variables`. `just
boundaries` would still pass (nextest does not deny warnings) but `just lint`
runs `clippy … -D warnings`, so `just ci` cannot be green. UNCONFIRMED without a
compile. Rename to `_err`.

**m2 — `escapes::TARGET_PATH_EXEMPT_CRATES` is dead in production.**
`scan_cargo_target_paths` is reached only from `lib.rs:244`, inside a loop
guarded by `walk::core_crate_dir_of(file)` (`lib.rs:238`), which returns `Some`
only for the four `CORE_CRATES`. `crates/gauntlet-arch-tests` is not one, so
`target_path_rule_applies` returns `true` on every call the production path ever
makes. The suite's own `[[test]] path = "../../tests/contract/…"` relocation is
unpoliced because the crate is out of scope, not because of this exemption — but
the constant, its 10-line justification and its pin all read as if it were
load-bearing. Either police the boundary crate's manifest for target paths (so
the exemption becomes real), or delete the constant and state the scope fact.

**m3 — `walk::DECLARED_SCAN_EXEMPTIONS` has no count or content pin**, unlike
`RUNTIME_INJECTED`. Adding a row is not sufficient on its own — an exemption only
takes effect if `in_literal_scope` is *also* narrowed and the call site *also*
passes it, and `check_defaults_exemption`'s unconditional cross-check
(`lib.rs:601-617`) fires on the resulting scope divergence. So this is a
maintainability trap rather than a hole. Pin it to its single entry.

**m4 — `walk::DEFAULTS_TABLE_SETTINGS` covers six of §8.6's ten rows.**
`review.mode`, `sandbox.backend`, `interactive` and `git_strategy` are absent, so
a "defaults table" that declares only the six numeric settings satisfies the
warrant while omitting the four that actually carry policy vocabulary. Feeds C1.

**m5 — the first clause of `check_defaults_exemption` cannot fire.**
`lib.rs:591`: `if !walk::in_denylist_scope(walk::DEFAULTS_MODULE)`. Since
`in_denylist_scope` is exactly `is_core_path` (`walk.rs:261-263`) and
`DEFAULTS_MODULE` is a constant under `crates/gauntlet-app/`, the condition is
statically false. It is a guard against a change that `walk::scope_tests` already
pins (`walk.rs:341-346`). Harmless, but it is one of two places where the
anti-vacuity apparatus asserts something trivially true — the other is
`assert_scan_is_not_vacuous`'s `expected.len() != report.scanned.len()` check
(`lib.rs:374-381`), which for `CORE_CRATES` reduces to `skipped == 0`, already
asserted twelve lines later.

**m6 — the evidence-locator warrant is mention-shaped.** `lib.rs:1739-1746`
accepts a locator file if its text `contains("#[test]")` and `contains("check_")`
— both satisfiable by a comment. This is the same weakness the suite fixed for
the defaults-table warrant ("mentioning the table in prose is not being the
table", `lib.rs:588`). The four files are currently genuine; the pin does not
require them to stay so.

**m7 — `CONFIG_WALK_SKIPPED_DIRS = ["target", ".git"]`** (`deps.rs:1020`) skips
any directory with those names at any depth, so a committed
`crates/gauntlet-app/target/.cargo/config.toml` is never read even though cargo
would honour it for a build invoked from inside that directory. The file would
still be scanned by the denylist and literal scans (the walker has no such
exclusion, `walk.rs:336`), so this is narrowness rather than a hole; it is worth
skipping by *build-output identity* rather than by name, or documenting the
residual next to the constant as the module does elsewhere.

---

## Not reported, deliberately

- The `gauntlet-cli` adapter-identifier carve-out. As briefed, I checked only
  whether it is implemented more widely than the sentence describes. It is not:
  `in_adapter_identifier_scope` is `INNER_CRATES` membership only
  (`walk.rs:277-281`), the cli stays inside `in_denylist_scope` and
  `in_literal_scope`, and `check_adapter_selection` re-asserts that both ways
  over the walked tree (`lib.rs:544-555`). The mask (`denylist.rs:262-270`)
  replaces adapter crate names with a space rather than skipping the unit, and
  `role_tests` pins the role mapping on the real tree.
- `gauntlet-arch-tests` excluding itself from `CORE_CRATES`. This is the
  declared self-exemption, and it is not open-ended: the crate is a member, is
  classified `Layer::BoundaryTests` with an empty internal allowlist, its one
  external dependency is admitted only as a default-registry `toml`
  (`deps.rs:134`, `deps.rs:493`), and no other layer's `allowed_internal` names
  it, so making it a dependency of a product crate fails
  `check_dependency_direction`.
- `check_env_example`. Its floor is derived from every `GAUNTLET_` token in the
  whole spec, not from Annex E (`envfile.rs:170-181`, `envfile.rs:281-298`), so
  a lockstep deletion from annex + file still fails; a missing `# Annex E`
  heading makes `annex` empty and the `missing_from_annex` branch fires. I found
  no vacuity here.
