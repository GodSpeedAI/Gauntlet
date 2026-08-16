# critic6 — concrete evasion

VERDICT: residuals-found (3 critical / 3 major / 3 minor)

Lens: what exact edit compiles something that should not compile into
`gauntlet-domain` / `gauntlet-ports` / `gauntlet-app`, or hardcodes a literal
§8.1 forbids, while every gate stays green.

I did not run cargo, just, rustc or nextest. Every trace below is read off the
source and confirmed by re-implementing the relevant scanner functions
byte-for-byte in a throw-away script and running the fixtures through them.
Anything that would need a compile to confirm is labelled unconfirmed.

**The manifest and source-inclusion channels held.** I worked
`[patch]`/`[replace]`, target-scoped dependency tables, workspace inheritance
with `package =` renames, alternate registries, git sources in every spelling,
`path` deps escaping the member set, undeclared packages under `crates/**`,
in-repo `.cargo/config*` at every depth, `rust-toolchain.toml`, `[lib]`/`[[bin]]`
/`[[test]]`/`[[bench]]`/`[[example]]` `path` overrides, `[package] build`, a
bare `build.rs`, `#[path]`, `#[cfg_attr(.., path = ..)]`, `# [path …]`,
`include!`/`include_str!`/`include_bytes!` incl. `std::include!` and whitespace
spellings, `\x2f`/`\u{2f}`-escaped and line-continuation-split path arguments,
module-resolution depth under inline `mod` blocks, and symlinked files and
directories. I found no way through any of them. That part of the instrument is
genuinely good, and the `unfiltered_listing`-vs-walker anti-vacuity guard and
the empty-by-default external allowlists are the reasons why.

**Where it breaks is one layer down: the literal extractor.** All three
criticals are in `literals.rs`, and one of them reaches AK-002/AK-014 because
`denylist.rs` borrows the same extractor.

---

## C1 (critical) — `string_literals` mis-pairs quotes, so a `'"'` earlier on the line deletes every following literal from eight classes *and* from AK-014's adapter-segment rule

**File to edit:** `/home/sprime01/projects/gauntlet/crates/gauntlet-app/src/lib.rs`

**Exact text to add:**

```rust
/// Resolve the agent runner. `spec` may arrive quoted from the CLI.
pub fn agent_for(spec: &str) -> &'static str {
    match spec.trim_matches('"') { "fast" => "mock", _ => "prime" }
}
```

`"mock"` and `"prime"` are the distinguishing segments of
`gauntlet-adapter-agent-mock` and `gauntlet-adapter-agent-prime`. This is
adapter selection by literal in application source — exactly AK-002 ("no adapter
identifier appears in domain or application source"), AK-014 ("adapter selection
resolves from configuration only") and spec §24.3 bullet 3. Without the
`trim_matches('"')` it fails the gate. With it, every check is green.

**Trace.**

1. `crates/gauntlet-arch-tests/src/literals.rs:1124` `string_literals(raw)` is a
   naive alternating-quote pairer. It has no state for char literals, raw
   strings, or comments: it walks bytes, and *every* `"` it meets either opens or
   closes a literal.
2. The char literal `'"'` in `trim_matches('"')` contributes one `"` byte. The
   match arms contribute six more. Seven is odd, so the pairing is shifted by one
   for the whole rest of the unit. Simulating `string_literals` on
   `    match spec.trim_matches('"') { "fast" => "mock", _ => "prime" }` yields:

   ```
   ["') { ", " => ", ", _ => ", " }"]
   ```

   The control line without `trim_matches('"')` yields `["fast", "mock", "prime"]`.
3. `denylist.rs:331` — inside `scan_source`, the AK-014 rule is
   `for literal in crate::literals::string_literals(&unit.text) { deny.segment_literal(&literal) }`.
   `segment_literal` (denylist.rs:280) is whole-literal equality against
   `adapter_segments` = `{mock, prime, cli, sqlite, fs, filesystem, git, build,
   test, command, diff}`. None of the four extracted strings equals a segment →
   no violation.
4. The other half of `scan_source` (denylist.rs:316) is the case-insensitive
   *substring* scan over `terms_for(Role::Inner)` = `DENIED_TERMS` ∪
   `Denylist::adapter`. `adapter` holds full crate names, the
   `gauntlet-adapter-` suffixes (`agent-mock`, `agent_mock`) and the reversed
   spellings (`mock-agent`, `mock_agent`) — **not** the bare segment. I ran the
   whole term set against the line: zero hits. The segment rule of step 3 is the
   *only* thing that ever catches bare `"mock"`/`"prime"`, and it is blind.
5. `check_adapter_selection` (lib.rs:495) uses the same `scan_source` with
   `vendor: Vec::new()`, so it has strictly fewer terms → also clean.
6. `literals.rs:476 scan_literals` — `find_relative_path`, `find_command`,
   `find_tool_identity`, `find_provider_identity`, `find_provider_endpoint` and
   `find_tier_policy` are all fed the same broken `literals` vector. Simulated:
   all `None`. The line-level classes (`find_absolute_path`, `find_url_scheme`,
   `find_model_id`, `find_drive_root`, endpoints, ports, timeout) do not match
   this line either.
7. **No other scan unit rescues it.** `text::scan_units` (text.rs:447) produces
   physical, logical, statement, declaration and escape-decoded forms:
   - `logical_lines` joins `"mock", "prime"` into `"mockprime"` (its
     `literal_join_end` fires at the `,`), producing
     `match spec.trim_matches('"') { "fast" => "mockprime" }` — still five
     quotes, still odd; simulated literals `["') { ", " => ", " }"]`. Clean.
   - `statement_lines` (text.rs:182) requires the line to end on `= ( , [ +` or
     to be a declaration head; this line ends on `}` and starts with `match` →
     no unit.
   - `declaration_units` (text.rs:244) requires `declaration_head` (`const` /
     `static` / `let` after qualifiers). First token is `match` → no unit. This
     matters: `declaration_units` is the one path that calls `strip_comments`
     and would restore quote parity.
   - `decode_escapes` changes nothing (no backslashes).
8. `check_scope_integrity` is clean: `escapes::scan_unterminated` *is*
   char-literal aware (`char_literal_end`, escapes.rs:414), so it correctly skips
   `'"'` and sees balanced quotes. The two scanners disagree about the same
   bytes, and only the weaker one feeds the denylist.

**Why this is the deep one.** `escapes.rs` was given a full string/raw/char/
comment-aware bracket scanner (`attribute_body_end`, escapes.rs:360, with the
`critic-5 C3` note). `literals.rs::string_literals` — which serves eight literal
classes *and* AK-014's only bare-segment rule — never got the same treatment.
Any construct putting an odd number of `"` bytes ahead of the payload works:
`'"'`, `b'"'`, a raw string containing one quote (`r#"pass a " here"#`), or a
block comment `/* " */`. Simulated and confirmed:
`pub fn db() -> &'static str { /* " */ "state/gauntlet.db" }` also passes
clean, hiding a relative path literal.

**Fix direction:** make `string_literals` share `escapes.rs`'s literal state
machine (skip `'…'`, `r#"…"#`, `b"…"`, `//`, `/* */`), and report a unit whose
quotes do not balance as a violation rather than returning a shifted vector —
missing evidence is not passing evidence (I8).

---

## C2 (critical) — the timeout class only sees values introduced by `=`, `(` or `,`, so a struct-field initialiser hides every duration

**File to edit:** `/home/sprime01/projects/gauntlet/crates/gauntlet-app/src/lib.rs`

**Exact text to add:**

```rust
pub struct Timeouts { pub stall_ms: u64 }

pub const POLICY: Timeouts = Timeouts { stall_ms: 900_000 };
```

This is a hardcoded timeout in application code — §8.1 names "timeout"
explicitly, AK-016 repeats it, and it is not in the declared §8.6 defaults
table. No trick, no comment, no unusual syntax; this is how a defaults struct is
ordinarily written.

**Trace.**

1. `literals.rs:1229 find_timeout` requires **both** a `TIMEOUT_HINTS` fragment
   on the line **and** a number satisfying
   `matches!(n.introducer, Some(b'=') | Some(b'(') | Some(b','))`.
2. `numeric_literals` (literals.rs:921) sets `introducer` to the nearest
   non-space/tab byte before the literal. Simulated on the line above:
   `[("900_000", 900000, ':')]`. The hint is found (`stall`, and `_ms`), the
   number is found, but `:` is not in the accepted set → `None`.
3. Compare the same value one keystroke away:
   `pub const STALL_TIMEOUT_MS: u64 = 900_000;` → introducer `'='` → reported.
   `Duration::from_millis(900_000)` → introducer `'('` → reported. The class is
   not narrow by value, it is narrow by *punctuation*.
4. `find_well_known_port` (literals.rs:982) does not gate on introducer, but
   `900000 ∉ WELL_KNOWN_PORTS`, so it is silent.
5. No other class matches: the line carries no string literal at all, so
   `find_relative_path` / `find_command` / `find_tool_identity` /
   `find_provider_identity` / `find_provider_endpoint` / `find_tier_policy` all
   return `None`; `find_absolute_path` finds no `/`; the denylist substring set
   (curated + derived) has zero hits — I ran the full set against the line.
6. No other scan unit: the line contains `;`, so `statement_lines` skips it
   (text.rs:187); `declaration_units` skips it too because
   `terminates_statement(head)` is true (text.rs:249). One unit, and it is silent.

The same gap covers `=>` (match arms: simulated introducer `'>'`) and `[`
(`= [900_000]`: simulated introducer `'['`). `:` and `>` are the two commonest
ways to introduce a value in Rust after `=`, `(` and `,`. The module docs
disclose *why* the introducer gate exists ("keeps `(spec §11.5)` and markdown
tables from flagging") but never disclose that struct-literal and match-arm
values are outside the class.

**Fix direction:** add `:`, `>` and `[` to the accepted introducer set, or —
better — drop the introducer test and keep the prose guard elsewhere (the hint
requirement already does most of the narrowing).

---

## C3 (critical) — `find_tier_policy` disables itself for any unit containing `#[`, including in a comment

**File to edit:** `/home/sprime01/projects/gauntlet/crates/gauntlet-app/src/lib.rs`

**Exact text to add:**

```rust
pub const REVIEW_MODE: &str = "adversarial"; // matches the #[review] attribute
```

`"adversarial"` is in `TIER_TERMS`. §8.1 and AK-016 both name "tier policy".

**Trace.**

1. `literals.rs:1242 find_tier_policy` opens with
   `if lowered.contains("#[") { return None; }`. That is a blanket disable of the
   class for the whole scan unit — not, as the module table claims, "on a
   `const`/`static`/`default` line **that is not an attribute**".
2. The `#[` here is inside a `//` comment. `scan_literals` (literals.rs:501)
   feeds `unit.text` — comments included — to both `lowered` and
   `string_literals`, so the comment reaches the guard.
3. Simulated: the line with the comment → `tier: None`. The identical line
   without the comment → `tier: adversarial`. One token in a comment turns the
   class off.
4. No other unit: the line contains `;`, so `statement_lines` skips it and
   `declaration_units` skips it (`terminates_statement(head)` true) — the one
   path that would `strip_comments` and restore the line never runs.
5. Nothing else fires: `"adversarial"` is not in `DENIED_TERMS`, not a
   `TOOL_IDENTITIES`/`COMMAND_HEADS`/`PROVIDER_IDENTITIES` entry, not path-shaped
   (single segment), not a model family. Simulated: zero hits across the whole
   term set and every other class.

The same guard is defeated by a real attribute on the same line
(`#[allow(dead_code)] pub const REVIEW_MODE: &str = "adversarial";` — though
`cargo fmt` would split that one, which `just boundaries` does not run), and by
any joined statement unit that happens to carry an attribute earlier in the
join.

**Fix direction:** the guard should ask whether the *literal* sits inside an
attribute body — the machinery already exists in
`escapes::attribute_body_end` — not whether `#[` appears anywhere in the unit.

---

## Majors

### M1 — an unreadable in-repo cargo config is silently skipped, in the one function whose whole thesis is I8

`deps.rs:978`:

```rust
let Ok(text) = std::fs::read_to_string(&path) else {
    continue;
};
```

Every other unreadable input in this suite is a violation:
`walk::read_scannable` (walk.rs:223), `collect_files` (walk.rs:122),
`collect_packages` (deps.rs:807), `collect_cargo_configs`'s own directory error
(deps.rs:1058), and malformed TOML two lines below this one (deps.rs:984) all
report. A `.cargo/config.toml` that `is_file()` accepts but `read_to_string`
rejects — non-UTF-8 content, a permission-restricted file, a symlink whose
target became unreadable between the `is_file()` and the read — passes as
"no forbidden keys". The file's own doc comment says these configs "redirect
what compiles without touching any manifest this suite reads". A `continue`
there contradicts the module's stated posture and the crate-level threat model.
I could not construct a spelling that cargo itself would still honour, so I am
not calling it critical — but the fail-open branch should be a violation
regardless.

### M2 — `Command::new(` reports `literals.first()`, which is positional, not the argument

`literals.rs:1169`:

```rust
if lowered.contains("command::new(")
    && let Some(first) = literals.first()
```

The class assumes the first string literal on the line *is* the program. It is
not: `Command::new(&cfg.shell).arg("--login")` reports `"--login"` (a false
attribution), and `Command::new(&cfg.shell)` with no literal on the line reports
nothing at all. Combined with C1 the whole `hardcoded-command` class is
bypassable; on its own it means the class's message can name the wrong string,
which is a maintainability trap for whoever has to act on the violation.

### M3 — "what is scanned equals what compiles" stops at `walk::CORE_CRATES`, but `gauntlet-cli` links every adapter

`check_scope_integrity` (lib.rs:224) iterates `walk::core_tree(root)`, which
walks only the four `CORE_CRATES` (walk.rs:20, walk.rs:111). Consequently
`crates/adapters/**` is scanned by *nothing*: not `scan_escapes`, not
`scan_cargo_target_paths`, not `build_script_violation`. An adapter may carry a
`build.rs`, an `include!(concat!(env!("OUT_DIR"), "/gen.rs"))`, a
`[lib] path = "../../../vendor/core.rs"`, or a symlinked source tree, and no
gate reads a byte of any of it — while `deps.rs` deliberately permits
`gauntlet-cli`, a policed core crate, to depend on every adapter member
(`Layer::allowed_internal`, deps.rs:213). The layer rules keep that code out of
domain/ports/app, so this is not a C-severity hole in AK-001; but the
composition root's binary is assembled from code the suite's central invariant
does not cover, and the crate docs assert the invariant without that
qualification. At minimum the scope note belongs in the threat model; better,
`build_script_violation` and `scan_escapes` should run over adapters too, with
the vendor-identifier rules left off.

---

## Minors

### m1 — `escapes::read_string_literal` decodes escapes differently from `text::decode_escapes`, and the redundancy that saves it is undocumented

`escapes.rs:543` turns `\x` into the literal char `x` (`out.push(bytes[i + 1] as char)`),
so `#[path = "..\x2f..\x2f..\x2fvendor\x2fevil.rs"]` resolves, in that function,
to the harmless in-crate segment `..x2f..x2f..x2fvendorx2fevil.rs` and
`report()` (escapes.rs:151) concludes "inside". The escape is caught only
because `text::scan_units` separately yields an escape-*decoded* copy of the
same unit and `scan_escapes` loops over all units. That redundancy is
load-bearing and nothing says so; deleting the decoded-form pass in `scan_units`
(which looks like a performance tidy-up) would reopen a scope escape. The same
function also does `out.push(bytes[i] as char)`, which mangles every non-ASCII
byte into a Latin-1 char — harmless today, wrong in principle for a path
resolver.

### m2 — the cargo-config walk carves out `target` and `.git` with no anti-vacuity guard

`CONFIG_WALK_SKIPPED_DIRS` (deps.rs:1020) is the one silent exclusion left in
the suite: every other scope decision is measured against an independent listing
(`walk::unfiltered_listing`) or declared in an inventory with a reason
(`walk::DECLARED_SCAN_EXEMPTIONS`, `escapes::TARGET_PATH_EXEMPT_CRATES`,
`literals::RELATIVE_PATH_EXEMPTIONS`). This one is a bare `const` in a `contains`
test. The reason given (build output holds no config cargo would read) is
plausible, but it is exactly the shape of carve-out the rest of the crate spent
its guards making impossible.

### m3 — `find_tier_policy`'s coverage of function bodies is an accident of `&'static str`

The gate `lowered.contains("const ") || contains("static ") || contains("default")`
matches `pub fn review_mode() -> &'static str { "adversarial" }` only because
`&'static str` happens to contain the substring `static ` with a trailing space.
Change the signature to `-> String { String::from("adversarial") }` and the class
goes silent — simulated and confirmed. The module discloses the
`const`/`static`/`default` restriction as a deliberate residual, so this is not
a hidden hole; it is worth recording that the class's apparent reach into
function bodies is a coincidence of Rust lifetime syntax, not a rule, and will
evaporate the first time someone returns an owned `String`.

---

## Suspicions (not traced to a green gate — do not treat as findings)

1. **An adapter build script mutating core sources between the compile and the
   scan.** Adapters may carry a `build.rs` (M3), it runs with full filesystem
   access, and in `just ci` the order is `check test lint deny boundaries ruler`
   — the scan reads on-disk state produced after three earlier stages already
   built the workspace. I did not establish an ordering that gives a green gate
   with dirty code compiled, so this is a suspicion, not a finding.
2. **`[alias]` in `.cargo/config.toml`.** It is not in
   `FORBIDDEN_CARGO_CONFIG_KEYS` (deps.rs:918). Cargo aliases take precedence
   over *external* subcommands, and `just boundaries` invokes the external
   subcommand `cargo nextest`. I could not construct an alias that both shadows
   it and exits 0, so this is unconfirmed — but the key belongs on the forbidden
   list next to `runner`, whose recorded reason ("replaces the program that
   actually executes the test targets this gate reports on") applies verbatim.
3. **The cargo config `[env]` table** is not checked. It sets variables for
   `rustc` and build-script invocations, which is the input side of `env!()` /
   `option_env!()` in compiled core code. Whether it can reach `RUSTFLAGS` as
   cargo computes it is version-dependent and I did not verify it.
4. Everything above is static reasoning plus a byte-faithful re-implementation
   of the scanners. **Nothing here has been compiled or executed against the
   real crate**, per the read-only constraint.
