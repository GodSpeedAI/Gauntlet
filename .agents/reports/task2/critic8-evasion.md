# Critic 8 — evasion lens — `crates/gauntlet-arch-tests/`

Blind review: no builder history, no attempt counts. Judged against the artifact
on disk, the frozen answer key (AK-001, AK-002, AK-014, AK-016, AK-019), spec
§4.1 / §8.1 / §24.3, and the crate's own threat model (`src/lib.rs` lines 42–116).
Every finding below was traced through the matcher code and grounded in the real
repository state (root manifest with literal member list, no `.cargo/` config, no
`rust-toolchain*` file anywhere, eleven adapter members, `tests/contract/`
locators present).

## Verdict

The suite is unusually evasion-aware — seven prior critic rounds have closed the
manifest, substitution, lexing, vacuity, and exemption channels, and the
fail-closed posture (I8) is real almost everywhere I pressed it. But the
evasion-hardening has concentrated on *quote* tricks and *manifest* spellings,
and two channels remain open at full strength. First, the escape matchers treat
whitespace and comments asymmetrically: rustc tokenizes `#[path /*c*/ = "…"]` and
`include! /*c*/ (…)` exactly like the compact spellings, but
`attribute_path_arguments` and `include_argument` skip only spaces between the
keyword, the `=`, and the `(` — a block comment at either position makes the
directive invisible, and a path literal containing a space is simultaneously not
"path-shaped" for the literal scanner, so an out-of-crate file compiles into a
core crate with every gate green. Second, the cargo-configuration key denylist
forbids `rustflags` at any table depth but does not know the `[env]` table, whose
values cargo applies to rustc invocations themselves — `[env] RUSTFLAGS =
"--extern shadow=…"` is the exact payload the `rustflags` ban exists to stop,
delivered under a key the walk never matches (the match is case-sensitive and
the child key is uppercase). Beyond those two criticals, the layer classifier
trusts `[package].name` while every scanner trusts directories, the timeout
class is blind to `return`/block-expression value positions (ordinary code), the
toolchain pin is checked only at the repo root while the same crate argues —
correctly, for `.cargo/config` — that cargo reads upward from the invocation
directory, and the setting-scoped defaults exemption can be smuggled past by
packing a second declaration onto a setting's line. The declared residuals
(runtime assembly, enumerated families, UNC for the literal class) are honestly
drawn and I do not count them except where the declared line does not cover the
gap actually found.

Severity rubric used: **critical** = infrastructure or a §8.1 literal class
actually reaches an inner layer with the gate green; **major** = reachable with
unusual-but-legitimate input; **minor** = latent or heavily constrained.
Trigger: **accidental** (ordinary coding mistake) / **adversarial** (requires an
actor controlling the repo or developer environment). In/out of threat model per
the declaration in `src/lib.rs` 42–116.

## Findings table

| id | severity | trigger | threat model | one line |
|---|---|---|---|---|
| E1 | critical | adversarial | inside | A comment token between `path`/`=` or `include!`/`(` hides the directive; a space-bearing escape path hides from the literal class — out-of-crate code compiles into a core crate, gate green |
| E2 | critical | adversarial | inside | `.cargo/config.toml` `[env]` table is not a forbidden key and its uppercase `RUSTFLAGS` child escapes the case-sensitive key walk — cargo hands it to rustc |
| E3 | major | adversarial | inside | Layer classification trusts `[package].name`; a name/dir swap re-classes the domain directory as app/cli and legalizes ports/adapter deps there with AK-001 green |
| E4 | major | accidental | inside | The timeout class only accepts values introduced by `= ( , : > [` — `return 900_000;` and `= { 900_000 }` hide a §8.1 timeout in ordinary code |
| E5 | major | adversarial | inside (declared residual for the literal class; the escape-resolver half is undeclared) | UNC/verbatim Windows paths are not absolute to `is_absolute` and mis-resolve to *inside* the crate in `normalize_join` — no scope-escape, no literal hit |
| E6 | major | adversarial | inside | `rust-toolchain*` is checked only at the repo root; a per-crate `[toolchain] path` pin is invisible although rustup searches upward from the invocation directory |
| E7 | major | adversarial | inside | The defaults exemption skips the whole scan *unit*; a second declaration packed onto a setting's line smuggles any §8.1 literal inside `defaults.rs` |
| E8 | minor | adversarial | inside | Cargo-config discovery skips directories named `target`/`.git` at any depth and silently drops symlinked dirs, against the "ANYWHERE in the tree" claim |
| E9 | minor | adversarial | inside | `terminates_statement` has no char-literal case: a `';'` arm truncates the declaration join and the I8 unjoinable report stays silent |
| E10 | minor | adversarial | inside (latent) | The denylist's text scan of the root table matches only the exact `[workspace.dependencies]` header; the dotted `[workspace.dependencies.<alias>]` spelling is blanked |
| E11 | minor | adversarial | outside | The locator pin checks substrings (`#[test]`, `check_`), not assertions — locator bodies and the crate's own repo tests are unpinned against instrument tampering |
| E12 | minor | adversarial | inside (adjacent to a declared residual) | Whole-literal rules (adapter segments, tools, providers) read only `Str` regions — `&[b'p', b'r', …]` assembles a selector with no string literal |

Counts: 2 critical, 5 major, 5 minor.

---

## E1 (critical, adversarial, inside the threat model) — comment tokens defeat the escape matchers; combined with a spaced path, everything misses

**Channel.** `#[path = …]` / `include*!` scope escapes, AK-001/AK-002's
scan-equals-compile half.

**Gap 1 — the matchers skip spaces, rustc skips comments.**
`escapes.rs:338-343` (`attribute_path_arguments`):

```rust
let mut j = skip_space(bytes, at + 4);
if bytes.get(j) != Some(&b'=') { continue; }
```

`skip_space` (`escapes.rs:388-393`) matches only `space | tab | \r | \n`. A block
comment between `path` and `=` leaves `j` on `/`, the `=` check fails, and the
`path` key is never collected. Same shape in the include family:
`escapes.rs:195-203` requires `!` after only spaces, and `include_argument`
(`escapes.rs:351-358`) requires `(` after only spaces.

**Exact payload** (all valid rustc — comments are trivia between tokens):

```rust
// crates/gauntlet-domain/src/lib.rs
#[path/* wiring */= "my dir/../../../vendor/store.rs"]
mod store;
// or:
include!/* wiring */("my dir/../../../shared/wiring.rs");
```

**Gap 2 — the backstop literal class does not fire.** The relative-path class
(`literals.rs:793-835`) requires *every* `/`-separated segment to be
filename-shaped; a segment containing a space fails `is_path_segment`
(`literals.rs:825-835`), so the whole literal `"my dir/../../../vendor/store.rs"`
is not path-shaped and not flagged. It does not start with `/` (no
absolute-literal hit), the line-level root scan (`literals.rs:707-731`) sees
every `/` preceded by a path-context byte, and it is exempt-list-free but never
reaches the exemption check. The declaration join does not rescue anything: the
attribute line ends on `]` (not an open continuation token, `text.rs:163-168`)
and `directive_head_awaits` (`text.rs:184-199`) only joins when the *next* line
starts with `=` or `(`.

**Traced result.** `check_scope_integrity` walks the file
(`lib.rs:246-273`), `scan_escapes` finds no `(directive, argument)` pair, the
literal scan finds no path, the denylist sees no vendor term, and the target
file lives outside every walk root. `vendor/store.rs` compiles into
`gauntlet-domain` with every locator green. The variant with the comment between
the include word and the `!` (`include/*c*/!("…")`) fails the `!` check at
`escapes.rs:196-198` the same way.

**Threat model.** Inside: lib.rs 62–66 — "`#[path]`/`include!` targets that
escape a crate directory are all violations, never skips". The residual list
(lib.rs 106–116) names only whitespace-split directive spellings "beyond those
normalized here" — a comment is not whitespace to this matcher but is whitespace
to rustc, which is precisely the divergence the module claims to have closed
(`escapes.rs` docs: matchers "tolerate whitespace where rustc does").

---

## E2 (critical, adversarial, inside the threat model) — `[env]` in an in-repo cargo config delivers compiler flags the key denylist never sees

**Channel.** `.cargo/config.toml` at any depth; `check_cargo_config`
(`deps.rs:972-1026`) + `walk_cargo_config` (`deps.rs:1176-1201`).

**Gap.** `walk_cargo_config` reports a hit only when a table key exactly equals
a `FORBIDDEN_CARGO_CONFIG_KEYS` entry (`deps.rs:1178-1181`,
`*forbidden == key.as_str()` — case-sensitive). The list (`deps.rs:918-957`)
contains `rustflags`, `rustdocflags`, `rustc`, `rustc-wrapper`,
`rustc-workspace-wrapper`, `linker`, `runner`, `paths` — it does not contain
`env`, and the `[env]` table's children are uppercase environment-variable
names, which the lowercase `rustflags` entry never matches. `[source.*]` and
`[registries.*]` are the only table-level bans (`deps.rs:1003-1021`).

**Exact payload** (committed `.cargo/config.toml` anywhere in the tree):

```toml
[env]
RUSTFLAGS = { value = "--extern shadow=/home/u/vendor/libshadow.rlib --cfg injected", force = true }
```

Cargo's own documentation: "The [env] section allows you to set additional
environment variables for build scripts, **rustc invocations**, cargo run and
cargo build" (cargo reference, config.html). rustc reads `RUSTFLAGS` from its
environment, so `pub use shadow::backdoor;` compiles inside `gauntlet-domain`
while its manifest declares nothing — the identical power to the
`[target.'cfg(all())'] rustflags` payload this suite explicitly tests and
bans (`lib.rs:1550-1583`, `deps.rs:919-923`). `[env] LD_PRELOAD` reaches the
same rustc invocations and the test binaries the gate itself reports on, which
is the stated rationale for banning `runner` (`deps.rs:946-951`).

**Traced result.** `discover_cargo_configs` finds the file, it parses, no
`[source]`/`[registries]` table, no forbidden key at any depth — zero
violations from `check_dependency_direction`. No manifest changed, so every
manifest check is green.

**Threat model.** Inside: lib.rs 51–56 — "every in-repo cargo configuration
(`.cargo/config.toml` and the legacy `.cargo/config`, ANYWHERE in the tree …)".
The residual list does not mention `[env]`; the crate's own docs claim the walk
is generic over "every key cargo honours" (`deps.rs:60-68`), and `[env]` is a
key cargo honours.

---

## E3 (major, adversarial, inside the threat model) — layer classification trusts `[package].name`, every scanner trusts directories

**Channel.** `check_dependency_direction` layer rules (AK-001).

**Gap.** `classify` (`deps.rs:239-251`) maps a member to a layer by
`[package].name` for five of the six layers (only `Adapter` is keyed by
directory, `crates/adapters/`). The walk, denylist roles, literal scope, and the
anti-vacuity presence check all key on *directories* (`walk.rs:20-44`,
`lib.rs:178-196`). The anti-vacuity check asserts only that the five *names*
exist among members — not that a name sits at its own directory.

**Exact payload** (three manifest edits, gate green):

1. `crates/gauntlet-domain/Cargo.toml`: `name = "gauntlet-app"`, plus
   `gauntlet-ports.workspace = true` — classified `App`, ports is an allowed
   internal dep (`deps.rs:209`).
2. `crates/gauntlet-app/Cargo.toml`: `name = "gauntlet-cli"` — its
   domain+ports+app deps are legal for `Cli` (`deps.rs:210`).
3. `crates/gauntlet-cli/Cargo.toml`: `name = "gauntlet-domain"`, deps removed
   (Domain allows none) — or the displaced name is parked on a new hollow
   member under `crates/adapters/` (classified `Adapter` by directory, name
   never consulted).

Cargo permits this (package names are unique; directories are not authoritative
to cargo). Result: the directory the spec's layering names *domain* — the one
`walk::CORE_CRATES` scans as the innermost layer — declares and compiles a
dependency on the ports layer, which AK-001's instrument claims is impossible;
with the `Cli` spelling it may declare *every* adapter crate
(`deps.rs:213-219`). The directory-keyed denylist backstops adapter *names* in
that directory's source (any `use` of an adapter spells its derived name), which
is why this is major rather than critical.

**Threat model.** Inside: lib.rs 68–71 — "membership comes from the filesystem
rather than from `[workspace].members`". The filesystem decides existence; the
*layer*, the thing the rules constrain, is decided by a manifest string.

---

## E4 (major, accidental, inside the threat model) — the timeout class is blind to `return` and block-expression value positions

**Channel.** `hardcoded-timeout` (AK-016, §8.1 "timeout").

**Gap.** `find_timeout` (`literals.rs:1260-1273`) requires the number's
*introducer* — the nearest non-space byte before it (`literals.rs:1023-1026`) —
to be one of `= ( , : > [`. Missing: `{` (block expressions, function bodies)
and letters (`return`, `as`, …).

**Exact payload** (ordinary Rust, either line alone):

```rust
pub const STALL_TIMEOUT_MS: u64 = { 900_000 };          // introducer '{'
fn stall_timeout_ms() -> u64 { 900_000 }                 // introducer '{'
fn retry_delay_ms() -> u64 { return 900_000; }           // introducer 'n'
```

The hint is present on the unit ("timeout" matches `TIMEOUT_HINTS`), the number
parses (`900_000`, value 900000 ≥ 2), and the introducer check discards it. The
same line carries no other literal class. Result: a §8.1 timeout hardcoded in an
inner-layer crate with AK-016 green — and via *ordinary* code shapes, not
laundering (a `return` of a duration constant inside a defaults helper is a
mistake a developer actually makes). The tier class's restriction to
`const/static/default` lines is a *declared* residual (`literals.rs:36-42`); the
timeout class's introducer narrowing is not — its residual note
(`literals.rs:69-74`) covers only unlisted hint words.

**Threat model.** Inside: lib.rs 30–33 lists timeouts among the enforced
literal classes; the crate claims the class exists and its posture is documented
(`literals.rs:21`).

---

## E5 (major, adversarial, inside the threat model as a declared residual for the literal class — the escape-resolver half is undeclared)

**Channel.** Absolute/UNC paths in escapes *and* literals (§8.1 path class).

**Gap.** `is_absolute` (`escapes.rs:487-495`) recognizes only `/`-rooted and
drive-letter paths; `\\server\share\x.rs` and the verbatim `\\?\C:\x.rs` are not
absolute to it. `normalize_join` (`escapes.rs:507-524`) then replaces `\` with
`/`, drops the empty segments, and *appends the segments to the declaring
directory* — `include!("\\\\server\\share\\w.rs")` in
`crates/gauntlet-app/src/lib.rs` resolves to
`crates/gauntlet-app/src/server/share/w.rs`, which is *inside* the crate: no
scope-escape violation, no line-level or literal-level absolute hit
(`literals.rs:747-752` requires a literal *starting* with `/`).

**Traced result.** On a Windows host rustc reads the UNC/verbatim path and
compiles out-of-tree bytes into a core crate with every gate green.

**Threat model.** lib.rs 109-110 declares "UNC paths (`\\\\server\\share`),
which the absolute-path class does not recognize" — that covers the *literal*
class miss. It does not cover the escape resolver deciding the target is
*in-crate* (the declared residual is a bounded miss of an enumerated list, "not
a hole in a channel"; here the channel's own resolver mis-answers "inside").
Filed major, adversarial, Windows-only.

---

## E6 (major, adversarial, inside the threat model) — the toolchain pin is policed only at the repo root

**Channel.** `rust-toolchain*` compiler substitution.

**Gap.** `check_toolchain_file` (`deps.rs:1111-1156`) reads only
`root.join("rust-toolchain.toml")` and `root.join("rust-toolchain")`. Rustup,
like cargo for `.cargo/config`, searches from the invocation directory upward —
the exact argument this crate itself makes for configs at any depth
(`deps.rs:1041-1049`: "Cargo reads `.cargo/config.toml` from the invocation
directory UPWARD … a config committed at `crates/gauntlet-domain/.cargo/config.toml`
therefore applies to every `cargo build` run from inside that crate").

**Exact payload:** `crates/gauntlet-app/rust-toolchain.toml` containing

```toml
[toolchain]
path = "../../toolchains/shadow-rustc"
```

Any `cargo`/`rustc` invocation from inside that crate directory (an ordinary
per-crate workflow) uses the substituted compiler — the `rustc` substitution
channel the suite bans by key (`deps.rs:929-933`) — and no check reads the file.
Today the repo has no toolchain file at all, so the root-only probe currently
observes nothing; the asymmetry with the config channel is the finding.

**Threat model.** Inside: lib.rs 56-57 names "the `rust-toolchain.toml` pin"
among the policed build inputs; a pin inside a crate directory is an in-repo pin
of the same build, by the crate's own invocation-directory reasoning.

---

## E7 (major, adversarial, inside the threat model) — the defaults exemption is unit-scoped, and a unit can hold more than one declaration

**Channel.** The §8.6 setting-scoped exemption in `literals.rs` (AK-016).

**Gap.** `scan_literals` skips an entire scan *unit* when
`is_defaults_setting_declaration(&unit.text)` holds
(`literals.rs:518-520`), and that test inspects only the leading tokens
(qualifiers, `const`/`static`, one identifier — `literals.rs:673-702`). A
physical line declaring a setting first and anything else second is skipped as a
whole; `declaration_units` never re-splits it because a head containing `;` is
skipped outright (`text.rs:317-320`), and `statement_lines` likewise skips lines
containing `;` (`text.rs:256-258`).

**Exact payload** (in `crates/gauntlet-app/src/config/defaults.rs`, with the
other nine settings declared normally so `check_defaults_exemption` holds):

```rust
pub const MAX_PARALLEL: usize = 2; pub const ROOT: &str = "/var/lib/gauntlet";
```

`ROOT`'s absolute path (or a model id, command, port, endpoint — any §8.1 class)
is inside the one exempted unit and is never scanned; the denylist backstops
only vendor *names*, not literal classes. Trigger is adversarial (rustfmt would
never pack the line), but the whole point of the round-6 setting-scoping
(critic-7 C1, `lib.rs:63-69`) was that nothing but the setting declarations may
escape — and the exemption's unit of skipping is larger than its unit of
warrant.

**Threat model.** Inside: the exemption is declared (lib.rs 66-69: "only the
declared default-setting declarations of the declared defaults module are
exempt"); the found behavior exempts *whatever shares a scan unit with one*.

---

## E8 (minor, adversarial, inside the threat model) — config discovery carve-outs contradict "ANYWHERE in the tree"

`CONFIG_WALK_SKIPPED_DIRS = ["target", ".git"]` (`deps.rs:1032`) is honored at
*any* depth: `crates/gauntlet-app/target/.cargo/config.toml` (a directory named
`target` inside a crate) is never discovered, while cargo invoked from inside it
would read it. Independently, `collect_cargo_configs`
(`deps.rs:1080-1092`) silently `continue`s past symlinked directories
(`file_type().is_dir()` is false for symlinks) — no discovery, no I8 violation —
so a `.cargo/config.toml` behind a top-level symlink (e.g. `dev -> /shared`)
is invisible; inside core crates `walk.rs:160-171` would at least flag the
symlink. Both sit against lib.rs 53-56: "ANYWHERE in the tree". Adversarial and
unusual (who runs cargo from inside `target/`?), hence minor.

## E9 (minor, adversarial, inside the threat model) — `terminates_statement` is not char-literal aware

`terminates_statement` (`text.rs:414-438`) handles `//` and `"…"` but not
`'…'`: a `;` inside a char literal returns "terminates". Payload:

```rust
pub const STALL_TIMEOUT_MS: u64 = match SEP {
    ';' => computed(),
    _ => 900_000,
};
```

`declaration_units` (`text.rs:314-348`) stops the join at line 2, marks the
declaration `terminated` (so `unjoinable_declarations` reports nothing), and the
value lands in a unit with no duration hint. The timeout (or, same shape, a
tier-policy string) passes. The crate's own thesis — "no class carries its own
quote counting; every source consumer reads ONE lexing pass" (lib.rs 57-60) — is
violated by this one private delimiter pass in `text.rs`. Contrived but valid
Rust; minor, adversarial.

## E10 (minor, adversarial, inside the threat model, latent) — dotted root-table header blanks the denylist's root scan

`workspace_dependencies_text` (`denylist.rs:442-458`) keeps lines only while
`trimmed == "[workspace.dependencies]"`. The TOML-equivalent dotted spelling

```toml
[workspace.dependencies.store]
package = "rusqlite"
version = "0.32"
```

never sets `inside`, so the body is blanked and the denylist never text-scans
the root table's vendor names. Latent today: the structural half
(`check_workspace_dependency_table`, `deps.rs:380-441`) flags the same entry as
an unallowlisted registry dependency, and an inheriting member's
`scan_manifest` resolves the alias (`denylist.rs:390-397`) — so the content is
currently caught elsewhere. The two halves of the same guarantee disagree on
spelling, which is exactly the drift shape earlier rounds closed elsewhere.

## E11 (minor, adversarial, outside the threat model) — the locator pin checks substrings, not assertions

`the_manifest_declares_exactly_the_four_answer_key_locators`
(`lib.rs:1934-1993`) pins the four `[[test]]` blocks and requires each locator
file to contain the substrings `#[test]` and `check_` — the assertions
themselves (in the locators and in the crate's own `mod repo` tests) are
unpinned. An actor who can edit `tests/contract/*.rs` or `src/*.rs` can gut any
`check_*` call while every pin stays green. This is instrument tampering, not a
build input, so it is outside the declared threat model ("this repository's own
declared build inputs", lib.rs 50) — recorded as an authority boundary, not an
evasion of the model.

## E12 (minor, adversarial, inside the threat model, adjacent to a declared residual) — whole-literal rules read only `Str` regions

`segment_literal` / `find_tool_identity` / `find_provider_identity`
(`denylist.rs:280-286, 333-347`; `literals.rs:1230-1248`) inspect only string
literals. Compile-time assembly that never forms a `Str` region —
`const SEG: &[u8] = &[b'p', b'r', b'i', b'm', b'e'];` compared as bytes —
selects an adapter with no literal to match. The declared residual covers
*runtime* assembly ("`Command::new(var)`, `["mo", "ck"].join("")`", lib.rs
101-104); this is the *const/byte-array* shape, adjacent and undeclared.
Contrived; minor.

---

## Channels worked and found HOLDING

- Member dependency sections — normal, dev, build, target-scoped, dotted-key
  spellings, bare-string specs, `package =` renames: all resolve through
  `declared_deps` to the (crate, source) allowlist, which is empty except
  `toml`; git and alternate-registry sources rejected even when allowlisted
  (`deps.rs:528-618, 675-729`).
- Root `[workspace.dependencies]`: non-member `path`, any `git`, any
  `registry`/`registry-index`, and any unallowlisted registry entry are
  violations before anyone inherits (`deps.rs:380-441`); workspace-inherited
  renames resolve to the real package in both scanner halves
  (`denylist.rs:390-397`, fixtures in `lib.rs:907-935`).
- `[patch]`/`[replace]`: root-only matches cargo's own semantics; every entry
  must be a member `path`, git and registry substitutions are violations
  outright (`deps.rs:855-904`).
- `.cargo/config` `[source.*]` replacement and `[registries.*]` indexes: any
  entry at any discovered depth is a violation (`deps.rs:1003-1021`).
- The eight forbidden config keys at any table depth, including
  `[target.'cfg(all())']` and triple-scoped spellings (`deps.rs:1176-1201`,
  fixtures `lib.rs:1550-1583`) — only the `[env]` table escapes (E2).
- External dependencies compiling into `gauntlet-domain`/`ports`/`app` fail
  closed through the empty layer allowlists — registry, git, scp-like URLs,
  workspace inheritance, path-out-of-tree (`deps.rs:109-149, 622-733`,
  fixtures `lib.rs:1184-1325`).
- Undeclared package directories under `crates/**`, unreadable subtrees,
  unclassifiable member names, and glob `members` entries all fail closed
  (`deps.rs:789-840, 1206-1219, 239-251, 738-766`).
- Symlinks inside core crates are violations, never follows
  (`walk.rs:160-171`); unreadable, oversized, non-UTF-8 files are violations,
  not skips (`walk.rs:238-265`); the anti-vacuity guard's universe is an
  independent listing that panics rather than guesses
  (`walk.rs:199-226`, `lib.rs:355-455`).
- `#[path]`/`include*!` escape detection across the compact, whitespace-split,
  `#[ path ]`, `cfg_attr`, `# [`, head-before-argument, `]`-as-data (string,
  char, raw, comment), and `\x`/`\u`-escaped-path spellings; computed arguments
  fail closed as Unresolvable (`escapes.rs:105-223, 320-410`, fixtures
  `lib.rs:826-905, 1633-1659`) — the comment-token position is the one hole
  (E1).
- Cargo target relocation `[lib]`/`[[bin]]`/`[[test]]`/`[[bench]]`/`[[example]]`
  incl. absolute targets, and the arch-tests exemption pinned to exactly that
  crate (`escapes.rs:415-471, 83-89`).
- Build scripts in core crates: both the bare `build.rs` file and any non-false
  `[package].build` key (`escapes.rs:461-484`).
- The composition-root carve-out is a mask, not a skip: adapter names in
  comments cannot launder vendor calls, and masking cannot splice terms
  (`denylist.rs:262-270`, fixtures `denylist.rs:735-787`).
- Adapter-identifier derivation: full crate names, `_` spellings, suffixes,
  reversed segments, and the whole-literal distinguishing-segment rule for
  AK-014 (`denylist.rs:184-286, 333-347`).
- Scan-unit normalization: string line-continuations, `concat!` fragment joins
  including raw-string fragments and interleaved block comments, `\x`/`\u{}`
  decoding, rustfmt `=`-splits, split-before-`=`, colon-first splits, and
  directive-head splits (`text.rs:80-133, 251-286, 929-968`).
- The single lexing pass: char literals incl. `'"'`/`'\''`, lifetimes vs
  literals, raw strings with hashes, byte strings, nested block comments,
  attribute spans from regions, bracket matching through data
  (`text.rs:585-924, 684-747`); whole-file unterminated strings/comments and
  unbalanced brackets fail closed for `.rs` (`escapes.rs:252-302`).
- The setting-scoped defaults exemption: warrant requires all ten §8.6 settings
  declared, declaration-shaped, by the same test the exemption uses; no
  file-level exemption exists anywhere; every call-site exemption is refused
  (`lib.rs:611-695`, `walk.rs:73`, fixtures `lib.rs:1724-1798`) — the unit-scope
  smuggle (E7) is the residual.
- `.env.example`: raw-file credential shapes including comments, commented
  entries with values, per-variable Annex E diff, spec-wide non-vacuity floor,
  runtime-injected exclusion enforced both ways and pinned
  (`envfile.rs:263-401, 55-59`).
- Evidence locators: manifest-declared `[[test]]` blocks match the frozen
  answer-key addresses, with real files behind them (`lib.rs:1934-1993`) —
  assertion pinning is the separate boundary (E11).
- Declared residuals I pressed and confirm are declared, not hidden: runtime
  string assembly; unlisted model families, providers, ports, tool heads, and
  TLDs; tier literals outside `const`/`static`/`default` lines; absolute paths
  only in comments rooted at unlisted names; the `://`-after-identifier comment
  residual in non-Rust text (which can only lose matches on comment content,
  and per-unit lexing resyncs at line boundaries).
