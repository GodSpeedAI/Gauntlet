# Task 2 — blind verification round 3 (post-commit b69c095)

Two blind critics (evasion, vacuity/teeth lenses), read-only, non-compiling, blind to all
prior reports. **Verdict: 4 critical findings — Task 2 REOPENED.** The workflow stopped and
refused to start Task 3 rather than build on a gate with holes in it.

| Critic | Verdict | critical | major | minor |
|---|---|---|---|---|
| 1 | residuals-found | 4 | 6 | 3 |
| 2 | residuals-found | 0 | 4 | 2 |

## Critic 1

### V1 — **CRITICAL** — `crates/gauntlet-arch-tests/src/deps.rs:212`

**Summary.** Cargo target source paths (`[lib].path`, `[[bin]].path`, `[[test]].path`, `[[bench]].path`, `[[example]].path`) are never inspected, so a core crate can point its crate root at a file outside the crate directory. The walk then scans a file that is not compiled, and the compiler compiles a file that is never scanned — the exact inversion of walk.rs's stated governing rule.

**Concrete evasion.**

```
Append to `crates/gauntlet-domain/Cargo.toml`:

    [lib]
    path = "../../shared/core.rs"

and put anything you like in `<repo>/shared/core.rs` (`use rusqlite::Connection;`, `pub const ROOT: &str = "/home/agent/state";`). Trace: `deps::check_member` -> `declared_deps` only iterates `dependency_sections` (dependencies / dev- / build- / target.*), so `[lib]` is invisible; `denylist::scan_manifest` iterates the same sections; `denylist::scan_source` and `literals::scan_literals` see only the manifest text `path = "../../shared/core.rs"`, which carries no denied term and no literal class (find_absolute_path skips every `/` because each is preceded by `.` or an alphanumeric); `walk::core_tree` walks only `crates/gauntlet-domain/**` so `shared/core.rs` is never a file; `escapes::scan_escapes` runs on the manifest but its DIRECTIVES are `#[path` / `include!` / `include_str!` / `include_bytes!`, none of which appear in TOML. `crates/gauntlet-domain/src/lib.rs` stays clean and is scanned, but is no longer compiled. Every check reports green. The suite's own Cargo.toml uses this exact mechanism (`[[test]] path = "../../tests/contract/..."`), so the channel is known to the artifact and left unpoliced.
```

**Requirement.** walk.rs module doc: "what gets scanned must equal what compiles"; spec §24.3 (mechanical enforcement of §4.1); AK-001.

### V2 — **CRITICAL** — `crates/gauntlet-arch-tests/src/escapes.rs:26`

**Summary.** The `#[path]` matcher is the literal substring `"#[path"`, so the two ordinary spellings rustc also accepts — `#[cfg_attr(..., path = "...")]` and `#[ path = "..."]` — pull an out-of-tree file into a core crate without producing a scope-escape violation.

**Concrete evasion.**

```
In `crates/gauntlet-domain/src/lib.rs` add:

    #[cfg_attr(all(), path = "../../../vendor/store.rs")]
    mod store;

`all()` is unconditionally true, so rustc compiles `<repo>/vendor/store.rs` as `gauntlet_domain::store`. In `scan_escapes` the lowered unit is `#[cfg_attr(all(), path = "../../../vendor/store.rs")]`; `lowered.find("#[path")` returns None (the text between `#[` and `path` breaks the literal) and no `include*!` matches, so zero violations. The target lives outside `crates/gauntlet-domain`, so `walk::core_tree` never lists it and neither the denylist nor the literal scan ever reads it. The same miss occurs for the plain whitespace form `#[ path = "../../../vendor/store.rs"] mod store;`, which is also valid attribute syntax. The unit test `path_attribute_escaping_the_crate_is_flagged` only exercises the tight `#[path = ` spelling, so the gap is entrenched rather than covered.
```

**Requirement.** escapes.rs module doc claims to close "four ways to compile a file from outside the crate directory"; spec §24.3; AK-001.

### V3 — **CRITICAL** — `crates/gauntlet-arch-tests/src/escapes.rs:41`

**Summary.** `first_string_after` takes the first double-quoted string after the directive, so the standard build-script codegen form `include!(concat!(env!("OUT_DIR"), "/gen.rs"))` resolves to the harmless in-crate path `<crate>/src/OUT_DIR` and is reported as inside. Generated code compiled into a core crate is never scanned by anything.

**Concrete evasion.**

```
Create `crates/gauntlet-app/build.rs`:

    fn main() {
        let out = std::env::var("OUT_DIR").unwrap();
        let src = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        std::fs::copy(format!("{src}/../../wiring/gen.rs"), format!("{out}/wiring.rs")).unwrap();
    }

and in `crates/gauntlet-app/src/lib.rs`:

    include!(concat!(env!("OUT_DIR"), "/wiring.rs"));

Trace of `scan_escapes`: it finds `include!` at offset 0, then `first_string_after` scans forward past `(concat!(env!(` — no `;` or `{` intervenes — and returns the string `OUT_DIR`. `normalize_join("crates/gauntlet-app/src", "OUT_DIR")` yields `crates/gauntlet-app/src/OUT_DIR`, which `starts_with("crates/gauntlet-app/")`, so `inside == true` and nothing is reported. `build.rs` itself is walked and scanned but contains no denied term and no literal class (the `/` characters in `"{src}/../../wiring/gen.rs"` are all preceded by `}`, `.` or alphanumerics, so `find_absolute_path` never fires). `wiring/gen.rs` may contain `use gauntlet_adapter_state_sqlite::SqliteStore;` and every hardcoded path, model id and timeout you want, and it compiles into the application layer. The generated file lands under the workspace-root `target/`, outside every walk root.
```

**Requirement.** walk.rs module doc: "what gets scanned must equal what compiles"; spec §24.3; AK-001/AK-002/AK-016.

### V4 — **CRITICAL** — `crates/gauntlet-arch-tests/src/denylist.rs:273`

**Summary.** `scan_manifest` builds its candidate list from the member manifest's own dependency key and its own inline `package = "..."` rename, but never resolves `workspace = true` through the root `[workspace.dependencies]` table — and the root `Cargo.toml` is outside every scan scope. A workspace-inherited rename therefore compiles a denied vendor crate into `gauntlet-domain` with no violation anywhere.

**Concrete evasion.**

```
Root `Cargo.toml`, under `[workspace.dependencies]`:

    store = { package = "rusqlite", version = "0.32" }

`crates/gauntlet-domain/Cargo.toml`:

    [dependencies]
    store.workspace = true

Trace: `scan_manifest` on the domain manifest sees key `store` with spec `{workspace = true}`; `fields.get("package")` is None, so candidates = ["store"] and no term matches. `scan_source` on the same manifest sees the line `store.workspace = true` — clean. `deps::declared_deps` DOES resolve the rename (dep becomes `rusqlite`) but then discards it because `rusqlite` is not a workspace member, so no dependency-direction violation fires either. The root `Cargo.toml` — where the string `rusqlite` actually lives — is never text-scanned: `walk::core_tree` (walk.rs:93) walks only the four `crates/gauntlet-*` directories, and `check_dependency_direction` parses the root manifest structurally without ever running `scan_source` over it. `deps::workspace_dep_names` already computes exactly the alias->package map that would close this, and `check_denylist` (lib.rs:285) simply never passes it in.
```

**Requirement.** spec §24.3 ("provider, harness, database, browser, and sandbox identifiers appear only under adapters/ and configuration files"); AK-002.

### V5 — **MAJOR** — `crates/gauntlet-arch-tests/src/denylist.rs:239`

**Summary.** The composition-root carve-out skips the ENTIRE scan unit when it contains an adapter crate name, rather than masking the adapter name out and scanning the remainder — which is what the code's own doc comment (line 149: "what is masked out of its text before the vendor scan") says it does. Appending an adapter crate name in a trailing comment launders any line in `gauntlet-cli`.

**Concrete evasion.**

```
In `crates/gauntlet-cli/src/main.rs`:

    let conn = rusqlite::Connection::open(p)?; // gauntlet_adapter_state_sqlite

In `scan_source` the lowered unit contains `gauntlet_adapter_state_sqlite`, so `names_an_adapter_crate` returns true and the loop `continue`s before any term is tested — `rusqlite` and `sqlite` are both unreported. The same one-line comment defeats every vendor term (`docker`, `anthropic`, `chromium`, ...) on that line, and repeating it per line defeats the whole file. It also launders non-dependency manifest tables: `crates/gauntlet-cli/Cargo.toml` with `state_backend = "sqlite"  # gauntlet-adapter-state-sqlite` under `[package.metadata.gauntlet]` is skipped by `scan_source` and is not a dependency key, so `scan_manifest` never sees it. The unit test `composition_root_may_name_adapters_but_not_bare_vendors` only checks `rusqlite::Connection::open(p)?` on a line with NO adapter name, so the mixed-line case is untested and the carve-out reads as safe. The stated scope decision (cli outside the adapter-identifier rule, inside the vendor rule) is defensible; this implementation does not deliver the second half.
```

**Requirement.** denylist.rs module doc: "a bare `rusqlite::Connection` [in the cli] is not [legal]"; spec §24.3; AK-002.

### V6 — **MAJOR** — `crates/gauntlet-arch-tests/src/text.rs:62`

**Summary.** Numeric and unicode string escapes (`\x41`, `\u{41}`) are never decoded by `text::scan_units`, and `literals::string_literals` (line 761) drops the backslash and keeps the escape body verbatim. Any denied term or environment literal spelled with one escaped character passes both the denylist and the literal scan while compiling to the exact forbidden value.

**Concrete evasion.**

```
In `crates/gauntlet-domain/src/lib.rs`:

    pub const STORE: &str = "\x73qlite";                 // compiles to "sqlite"
    pub const ROOT: &str  = "/\x74mp/gauntlet/state";    // compiles to "/tmp/gauntlet/state"

In `logical_lines` the byte `\` is followed by `x`, not `\`, and `continuation_end` fails (the next byte is not a newline), so the backslash is pushed as ordinary content and the unit text remains `"\x73qlite"`. `contains_term(lowered, "sqlite")` is false. For the path, `find_absolute_path` reaches the `/` after the opening quote, tests `lowered[i+1..]` = `\x74mp/...` against every root and matches none; the later `/` characters are all preceded by alphanumerics and are skipped. `\u{73}qlite` and `\u{2f}tmp` behave identically. This defeats AK-002 and AK-016 with a single-character edit and leaves the compiled program byte-identical to the forbidden version.
```

**Requirement.** spec §8.1 ("No path, model, provider, command, port number, timeout, tier policy, or tool identity is a literal"); spec §24.3; AK-002/AK-016.

### V7 — **MAJOR** — `crates/gauntlet-arch-tests/src/denylist.rs:166`

**Summary.** `Denylist::derive` produces the full crate name, the `gauntlet-adapter-` suffix, the reversed suffix and their underscore spellings, but never the individual suffix segments. Selecting an adapter by its shortest distinguishing segment in application code is invisible to AK-014.

**Concrete evasion.**

```
In `crates/gauntlet-app/src/lib.rs`:

    pub fn runner_for(role: &str) -> &'static str {
        match role { "builder" => "prime", "critic" => "mock", _ => "cli" }
    }

Derived terms for `gauntlet-adapter-agent-prime` are `gauntlet-adapter-agent-prime`, `gauntlet_adapter_agent_prime`, `agent-prime`, `agent_prime`, `prime-agent`, `prime_agent` — none is a substring of `"prime"`, and `contains_term("prime", "agent-prime")` is false because the matcher tests haystack.contains(term), not the reverse. `"mock"`, `"cli"`, `"git"`, `"diff"`, `"fs"` behave the same. No vendor term matches; `find_tool_identity` does not list `mock`/`prime`/`cli`; `find_tier_policy` does not list them either. This is literal adapter selection compiled into the application layer, which is precisely what the artifact's own `derived_adapter_identifiers_are_flagged_in_inner_layers` test exists to catch — it just tests the two-segment spellings.
```

**Requirement.** AK-014 ("adapter selection resolves from configuration only; no adapter is chosen by a literal in domain or application code"); spec §24.3 second bullet.

### V8 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:409`

**Summary.** `find_absolute_path` requires the byte before `/` to not be a path-context byte and requires the root name to start immediately after it, so any absolute path with a `.` or an extra `/` as its first segment slips past — including roots that ARE on the list.

**Concrete evasion.**

```
In `crates/gauntlet-app/src/lib.rs`:

    pub const STATE_ROOT: &str = "/./home/agent/gauntlet/state";

resolves to `/home/agent/gauntlet/state` on every POSIX host. Trace: at the first `/`, the preceding byte is `"` so the guard passes, but `lowered[i+1..]` begins `./home/...` and no entry of ABSOLUTE_PATH_ROOTS starts with `.`; at the second `/`, `bytes[i-1] == b'.'` and `is_path_context_byte(b'.')` is true, so the loop `continue`s; the remaining slashes follow alphanumerics. Zero violations, and no other class fires (no scheme, no model family, no port). `"//home/agent/state"` fails the same way (second slash preceded by `/`, also a path-context byte). Separately, the root list is an enumeration with no documented residual: `"/scratch/gauntlet"` and `"/workdir/state"` are absolute paths that match nothing — but the `/./` form is the sharper defect because it defeats a listed root.
```

**Requirement.** spec §8.1 ("No path ... is a literal in domain or application code"); AK-016; literals.rs class table which promises "a `/`-rooted segment from ABSOLUTE_PATH_ROOTS".

### V9 — **MAJOR** — `crates/gauntlet-arch-tests/src/deps.rs:166`

**Summary.** `[patch]` and `[replace]` in the workspace root manifest are never read, although both accept `path = "..."` and both silently substitute an out-of-tree crate for a declared dependency across the whole workspace. deps.rs's own module doc claims every `path` dependency is resolved to a known member.

**Concrete evasion.**

```
Add a benign external dependency to `crates/gauntlet-domain/Cargo.toml` (`serde = "1"` — external deps are entirely unconstrained by the layer rules, which bound only workspace-member deps), then add to the root `Cargo.toml`:

    [patch.crates-io]
    serde = { path = "../gauntlet-infra/serde-shim" }

`check_workspace_dependency_paths` reads only `workspace.dependencies` and returns early on anything else; `dependency_sections` (line 119) enumerates only dependencies / dev-dependencies / build-dependencies plus `target.*`; `discover_package_dirs` walks only `crates/**` so the out-of-tree package is never discovered; and the root manifest text is never scanned by the denylist. Arbitrary out-of-tree code now compiles into `gauntlet-domain`. `[replace]` has the same shape and the same blind spot.
```

**Requirement.** deps.rs module doc: "every `path = \"...\"` dependency ... must resolve to a known member"; AK-001 ("zero compile-time dependencies on ... infrastructure").

### V10 — **MAJOR** — `crates/gauntlet-arch-tests/src/envfile.rs:130`

**Summary.** `parse_env_example` discards every line beginning with `#` before any credential check runs, so a real key committed as a commented-out example value satisfies AK-019's "no credentials" clause.

**Concrete evasion.**

```
Append to `.env.example`:

    # Example (works today):
    # GAUNTLET_ANTHROPIC_KEY=sk-ant-api03-REAL-KEY-MATERIAL

`parse_env_example` filters on `line.starts_with('#')` and returns None, so the line produces no `EnvEntry`; `check_env_example` iterates only over entries, so CREDENTIAL_SHAPES is never applied to it. The commented key is not in Annex E either, so no "unknown key" violation fires — commented lines simply do not exist to this instrument. The credential is committed to the repository and the gate is green. The scan should run CREDENTIAL_SHAPES over the raw file text, not only over parsed values.
```

**Requirement.** AK-019 (".env.example lists every recognized GAUNTLET_ variable with empty values and no credentials"); spec §8.1 ("Generated files never contain credentials").

### V11 — **MINOR** — `crates/gauntlet-arch-tests/src/text.rs:144`

**Summary.** `literal_join_end` joins fragments only across the exact byte shape `" ws* , ws* "`, so a raw-string fragment or a comment between fragments defeats the `concat!` normalization the module exists to provide.

**Concrete evasion.**

```
In `crates/gauntlet-app/src/lib.rs`:

    pub const STORE: &str = concat!("sq", r"lite");
    pub const MODEL: &str = concat!("claude", /* id */ "-3-5-sonnet");

In the first, after the closing `"` of `"sq"` the scanner skips whitespace, finds `,`, skips whitespace, and then requires `"` but sees `r` — `literal_join_end` returns None and the fragments are never joined, so no unit contains `sqlite`. In the second, after the `,` the next non-whitespace byte is `/`, same result. Both compile to the denied term. (The plain `concat!("sq", "lite")` and multi-fragment `concat!("sq", "li", "te")` forms ARE correctly joined — this is only the raw-string and interleaved-comment variants.)
```

**Requirement.** text.rs module doc: "adjacent string-literal fragments joined ... the `concat!` shape"; spec §24.3; AK-002/AK-016.

### V12 — **MINOR** — `crates/gauntlet-arch-tests/src/envfile.rs:152`

**Summary.** The AK-019 anti-vacuity floor is the hardcoded constant 50 on both sides, while Annex E and `.env.example` each carry 92 variables today. Forty-two variables can be deleted from both files in lockstep and the check still reports clean — the exact "a constant that happens to equal today's count" failure mode lib.rs:247 warns against for the crate-set floor.

**Concrete evasion.**

```
Delete the `Tools` group (21 variables) and the `Limits` group (17) from Annex E of `.agents/specs/Core-spec.md` and the matching 38 lines from `.env.example`. `annex.len()` becomes 54 and `entries.len()` becomes 54, both clear the `< 50` guards; the two sides still agree, so no missing/unknown-key violation fires and `env_example_matches_annex_e` passes with a third of the runtime's configuration surface undocumented. The floor should be derived (e.g. the count of distinct `GAUNTLET_` tokens anywhere in the spec, or the count of `.env.example` entries) rather than a magic number chosen below the current value.
```

**Requirement.** lib.rs:247 principle ("the floor is derived from the discovered crate set, not a constant that happens to equal today's count"); I8 "missing evidence is not passing evidence"; AK-019.

### V13 — **MINOR** — `crates/gauntlet-arch-tests/src/lib.rs:330`

**Summary.** `check_adapter_selection` is the only repo-level scan that does not call `assert_scan_is_not_vacuous`, so the AK-014 instrument has no guard against a future narrowing of `walk::in_adapter_identifier_scope`.

**Concrete evasion.**

```
Change `walk::in_adapter_identifier_scope` (walk.rs:219) to `INNER_CRATES.iter().any(...) && !rel.contains("/generated/")`. `check_denylist` and `check_no_hardcoded_literals` both fail immediately, because `assert_scan_is_not_vacuous` walks every core file and reports any that the scope predicate excludes and that is not `DEFAULTS_MODULE`. `check_adapter_selection` checks only that the three inner-crate `Cargo.toml` files are in `report.scanned` and that `skipped == 0`; both still hold, so AK-014's locator stays green while `crates/gauntlet-app/src/generated/**` is exempt from the adapter-identifier rule. (The scope predicate is shared, so today the other two checks would catch this edit — but AK-014's own instrument carries no such guard, and the guard is what makes a scope change fail loudly rather than shrink silently.)
```

**Requirement.** lib.rs:209 ("Anti-vacuity for a repo-level scan (class C1)"); AK-014.

### Critic 1 — checked and found sound

SCOPE OF REVIEW: read-only source inspection of crates/gauntlet-arch-tests/{Cargo.toml,src/*.rs}, tests/contract/*.rs, the workspace Cargo.toml, all core/adapter manifests, justfile, deny.toml, .env.example, .agents/specs/Core-spec.md (4.1, 8.1, 24.3, 25, Annex E) and .wayfinder/answer-key.md. No cargo/just invocation; nothing under .agents/reports/ was opened.

VERDICT ON THE STATED DESIGN DECISION (cli carve-out): the SCOPE is implemented exactly as declared — walk::in_adapter_identifier_scope is INNER_CRATES only, and walk.rs's scope_tests assert that crates/gauntlet-cli files stay inside in_denylist_scope and in_literal_scope. That part is sound and matches spec 4.1. The MECHANISM is not: denylist.rs:239 skips the whole scan unit instead of masking the adapter name out of it (finding #5), which the code's own doc at line 149 says it does. So the carve-out is correctly scoped and incorrectly enforced — the fix is mask-and-rescan, not a scope change.

HOLES I SUSPECTED AND FOUND CLOSED (stated explicitly, per the brief):
- Symlinked FILES and symlinked DIRECTORIES inside a core crate: closed. walk.rs:129 tests file_type.is_symlink() before is_dir()/is_file(), so both are violations and neither is followed.
- Non-UTF-8 files, files over 1 MiB, unreadable files/directories, unclassifiable file types: closed, all violations rather than skips (walk.rs:104-192), and ScanReport.skipped is asserted zero (lib.rs:265).
- Undeclared package directories under crates/**: closed (deps::discover_package_dirs + undeclared_packages).
- `path = "../../vendor/x"` in a member manifest or in root [workspace.dependencies], including absolute paths (join_posix keeps them out of the member set): closed.
- Target-scoped dependency tables `[target.'cfg(unix)'.dependencies]`: closed (deps::dependency_sections).
- Inline `package = "..."` renames in a member manifest: closed. (The WORKSPACE-inherited rename is not — finding #4.)
- optional / feature-gated deps: closed — an optional dep still appears as a key in [dependencies] and is matched.
- `[features]` and `[package.metadata]` vendor strings in a core manifest: closed, because check_denylist runs scan_source over manifests as raw text as well as scan_manifest.
- Word-boundary evasion in vendor names (`libsqlite3-sys`, `anthropic_sdk`, `openai_api`): closed — contains_term is a bare case-insensitive substring with no boundary requirement.
- Rust line-continuation splitting a term, and plain `concat!("sq","lite")`: closed (text::logical_lines).
- Numeric port spellings `8_080` / `0x1F90` / `0o17620` / `8080u16`: closed (literals::numeric_literals).
- Scope-predicate tampering (adding `&& !rel.contains("/generated/")`): closed for the denylist and literal instruments by assert_scan_is_not_vacuous; NOT closed for AK-014 (finding #13).
- Vacuous defaults-module exemption: closed today. DEFAULTS_MODULE does not exist, so check_defaults_exemption takes the "must exempt nothing" branch and in_literal_scope == in_denylist_scope for every walked file.
- Unicode homoglyphs: considered and NOT reported. Crate names and dependency keys must be ASCII, and in a string literal a homoglyph changes the runtime value, so it buys nothing an attacker wants. The `\x` / `\u{}` escape path (finding #6) is the version of this that actually works.

RESIDUALS I JUDGED DISCLOSED RATHER THAN DEFECTIVE: the enumerated MODEL_FAMILIES, TOOL_IDENTITIES, TIER_TERMS and COMMAND_HEADS lists, and the const/static/default narrowing of the tier class, are written down in the literals.rs "Residual, recorded rather than hidden" section. TIMEOUT_HINTS is similarly enumerated (`pub const BUILDER_LIMIT: u64 = 5400;` is not flagged) and contains_hint's alphanumeric word boundary means `BUILDERTIMEOUT` evades where `BUILDER_TIMEOUT` does not — that one is NOT in the residual list and is worth adding there, but it needs unnatural naming so I did not raise it as a finding. ABSOLUTE_PATH_ROOTS being enumerated is also undisclosed; I raised the `/./` matcher defect instead because it defeats roots that ARE listed.

ONE COSMETIC DEFECT, NOT FILED: denylist.rs:292 computes the report line number with `l.to_lowercase().contains(candidate)` where `candidate` is not lowercased, so a dependency key containing any uppercase character loses its line number and reports the bare path. The violation still fires; only the locator degrades.

SEVERITY RATIONALE: findings 1-4 are critical because each one compiles attacker-chosen code or an attacker-chosen dependency into gauntlet-domain or gauntlet-app while all five answer-key locators report green — they defeat the gate, not merely a class within it. Findings 5-10 are major: each defeats one named clause of AK-002, AK-014, AK-016 or AK-019 with a one-line edit but leaves the other instruments intact.

## Critic 2

### V14 — **MAJOR** — `crates/gauntlet-arch-tests/src/denylist.rs:239`

**Summary.** The composition-root carve-out skips the ENTIRE scan unit when a line mentions an adapter crate, instead of masking the adapter name and scanning the rest. Any line in gauntlet-cli can therefore be exempted from the whole vendor denylist by appending a comment naming an adapter crate.

**Concrete evasion.**

```
Add to crates/gauntlet-cli/src/main.rs:

    let store = rusqlite::Connection::open(cfg.path)?; // gauntlet-adapter-state-sqlite

Trace: lib.rs:192-196 assigns Role::CompositionRoot to every gauntlet-cli file. denylist.rs:239 tests names_an_adapter_crate(&lowered), which is a plain `lowered.contains(name)` over adapter_crate_names (denylist.rs:211-215) derived from all 11 workspace adapters -- the trailing comment matches -- and `continue`s before any vendor term is tested (denylist.rs:242-253). Both the physical and the logical scan unit contain the comment, so both are skipped. `rusqlite` and `sqlite`, database identifiers spec section 24.3 names explicitly, are not reported. crate::literals has no vendor class, so check_no_hardcoded_literals does not catch it either. Note this contradicts the module's own documentation: denylist.rs:135-136 says the composition root is scanned with "adapter identifiers masked out first" and denylist.rs:148-149 says crate names are "masked out of its text before the vendor scan" -- no masking is performed anywhere. The same whole-line skip applies to crates/gauntlet-cli/Cargo.toml when scanned as raw text (lib.rs:285-291), so a [features] or [package.metadata] vendor value there escapes too (scan_manifest only backstops dependency keys and `package =` renames).
```

**Requirement.** Spec section 24.3 fourth bullet: "a denylist test asserts that provider, harness, database, browser, and sandbox identifiers appear only under adapters/ and configuration files." gauntlet-cli is not under adapters/ and is not a configuration file; the artifact itself declares (walk.rs:39-43, denylist.rs:21) that the cli remains fully inside the vendor denylist.

### V15 — **MAJOR** — `crates/gauntlet-arch-tests/src/lib.rs:330`

**Summary.** check_adapter_selection -- the sole evidence locator for AK-014 -- never calls assert_scan_is_not_vacuous and never asserts a source file was scanned, and walk::in_adapter_identifier_scope is the one scope predicate with no anti-carve-out guard, even though it is also what selects Role::Inner. One added exclusion silently disables the adapter-identifier rule for a subtree with no test failing.

**Concrete evasion.**

```
Edit walk.rs:219-223 to `INNER_CRATES.iter().any(|c| rel == *c || rel.starts_with(&format!("{c}/"))) && !rel.contains("/registry/")`, then create crates/gauntlet-app/src/registry/select.rs containing:

    pub fn runner_for(role: &str) -> &'static str {
        match role { "builder" => "prime-agent", "critic" => "agent-mock", _ => "cli-agent" }
    }

Nothing fails. (a) check_adapter_selection (lib.rs:318-343) scans only in-scope files, so the new file is excluded; its only floors are "the three INNER_CRATES manifests are in report.scanned" and "skipped == 0", both still satisfied. (b) check_denylist still walks the file (in_denylist_scope is unchanged), but lib.rs:192-196 derives the role from the same broken predicate, so the file gets Role::CompositionRoot; terms_for(CompositionRoot) (denylist.rs:198-201) omits self.adapter entirely, and none of "prime-agent"/"agent-mock"/"cli-agent" is a curated vendor term. (c) assert_scan_is_not_vacuous's "no subtree may be carved out" loop (lib.rs:236-245) is only ever invoked with in_denylist_scope (lib.rs:293-298) and in_literal_scope (lib.rs:352-357), never with in_adapter_identifier_scope. (d) walk::scope_tests::adapter_identifier_scope_is_the_inner_layers_only (walk.rs:299-320) uses only src/lib.rs and Cargo.toml fixtures, none containing "/registry/". Result: the exact AK-002 evasion the artifact's own test at denylist.rs:409-452 was written to catch now passes at repo level.
```

**Requirement.** AK-002 ("no adapter identifier appears in domain or application source") and AK-014 ("no adapter is chosen by a literal in domain or application code", spec section 7.1); spec section 24.3 third bullet. The artifact states its own standard at walk.rs:200-203 -- an added exclusion must "fail the gate instead of silently shrinking it" -- and applies that standard to two of its three scope predicates.

### V16 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:805`

**Summary.** The hardcoded-command class silently disables itself for any string literal containing a character outside [A-Za-z0-9_.:/=+@%-]. A shell metacharacter -- a redirect, a pipe, a quote, a glob -- turns a hardcoded command line into a clean pass.

**Concrete evasion.**

```
Put in crates/gauntlet-app/src/lib.rs:

    pub const BUILD_CMD: &str = "cargo build --workspace 2>&1";

find_command (literals.rs:786-812) tokenizes to ["cargo","build","--workspace","2>&1"], passes the length, COMMAND_HEADS and STOPWORDS gates, then hits the character-class filter at literals.rs:805-810; '>' and '&' are not in the allowed byte set, so `continue` drops the literal and the function returns None. I traced every other class on that line: no '/' at an absolute-path boundary, no `://`, no MODEL_FAMILIES match, no host:port (the only ':' is `BUILD_CMD:` followed by a space), the numeric literals 2 and 1 are not in WELL_KNOWN_PORTS, no TIMEOUT_HINTS fragment, the literal is not exactly a TOOL_IDENTITIES entry, and no TIER_TERMS. scan_literals returns an empty vector. Same bypass for `"git log --pretty=%H | head"` (pipe) and `"sh -c 'cargo nextest run'"` (quote). A 13-token command also escapes via the `tokens.len() > 12` cap at literals.rs:788.
```

**Requirement.** Spec section 8.1: "No path, model, provider, command, port number, timeout, tier policy, or tool identity is a literal in domain or application code." AK-016 restates "command". The module doc (literals.rs:15) claims the class's only narrowing is "scoped to STRING LITERALS only" plus the stopword guard; the character-class filter is an undocumented third narrowing that inverts into a bypass.

### V17 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:826`

**Summary.** The timeout and tier-policy classes require the identifier and the value on the same scan unit, and text::scan_units joins only string continuations and adjacent literal fragments -- never a statement split across lines. An ordinary rustfmt line break after `=` defeats both classes.

**Concrete evasion.**

```
Put in crates/gauntlet-app/src/lib.rs (the shape rustfmt produces once the declaration exceeds the line width):

    pub const GAUNTLET_BUILDER_INVOCATION_STALL_TIMEOUT_MILLISECONDS: u64 =
        900_000;

    pub static REVIEW_MODE: &str =
        "adversarial";

Traced: text::scan_units (text.rs:108-116) emits two physical units per statement and logical_lines (text.rs:50-103) pushes a unit at every newline (text.rs:85-91), so the halves are never joined. For the timeout: unit 1 has the hint but numeric_literals (literals.rs:547-604) finds no candidate (the '6'/'4' of `u64` are rejected by the `is_word_byte(bytes[i-1])` guard at literals.rs:556); unit 2 is "    900_000;", which contains_hint (literals.rs:270-287) matches against no TIMEOUT_HINTS entry, so find_timeout returns None at literals.rs:827-830. For the tier: unit 1 satisfies the `static ` guard (literals.rs:843) but string_literals finds nothing; unit 2 carries the literal but fails the const/static/default guard and returns None immediately. Both statements produce zero violations. The one-line forms are the only ones the tests exercise (literals.rs:1134-1155 and 1171-1186), and the recorded-residual section (literals.rs:20-37) does not name this gap.
```

**Requirement.** Spec section 8.1 and AK-016 name "timeout" and "tier policy" explicitly. Section 8.6 fixes max_wall_seconds at 3600 as a defaults-table value, so a 900_000 ms stall budget compiled into the application layer is exactly the leak the class exists to stop. The artifact's own stated standard (text.rs:1-27) is that a term "split by a Rust line-continuation or by concat!" must still be found; a plain statement split is the same evasion at lower effort.

### V18 — **MINOR** — `crates/gauntlet-arch-tests/src/deps.rs:353`

**Summary.** discover_package_dirs turns filesystem read failures into "no undeclared packages found", and unconditionally excludes any directory named `target` from package discovery -- both contrary to the crate's own I8 posture and to how walk::collect_files treats the identical conditions.

**Concrete evasion.**

```
deps.rs:353-355 `let Ok(entries) = std::fs::read_dir(dir) else { return; };` returns before the `dir.join("Cargo.toml").is_file()` test at deps.rs:356, so an unreadable directory under crates/ contributes nothing to the discovered set and undeclared_packages (deps.rs:379-392) reports nothing -- a read failure reads as a pass. deps.rs:365-367 `let Ok(file_type) = entry.file_type() else { continue; };` swallows the same failure per-entry. deps.rs:370 `entry.file_name() != "target"` means a package at crates/target/Cargo.toml or crates/adapters/target/Cargo.toml is never reported as undeclared. Compare walk.rs:104-116 and walk.rs:121-128, which report exactly these two conditions as scope-integrity violations with the message "missing evidence, not a pass". The impact is bounded -- check_member (deps.rs:272-282) and check_workspace_dependency_paths (deps.rs:166-194) still flag any `path =` reaching such a package, so it cannot be depended on silently -- which is why this is minor rather than major, but the class-A1 discovery check is weaker than it reads.
```

**Requirement.** The crate's stated invariant I8, restated at walk.rs:8-13 and lib.rs:266-270: "missing evidence is not passing evidence". AK-001's discovery half (deps.rs:27-40) exists precisely because "[workspace].members is a claim, not the truth"; a discovery walk that silently drops unreadable subtrees reintroduces a smaller version of the same trust.

### V19 — **MINOR** — `crates/gauntlet-arch-tests/src/literals.rs:74`

**Summary.** ABSOLUTE_PATH_ROOTS is an enumerated list, so an absolute path rooted at any directory not on it passes cleanly -- and unlike the model-id, tier-policy, and tool-identity classes, this incompleteness is not recorded in the module's residual section.

**Concrete evasion.**

```
Put in crates/gauntlet-app/src/lib.rs:

    pub const CONFIG: &str = "/app/config.toml";

find_absolute_path (literals.rs:409-433) finds the leading '/' at a valid boundary but no ABSOLUTE_PATH_ROOTS entry prefixes "app/config.toml" ("applications" does not match), and the second '/' is preceded by 'p', a path-context byte, so it is skipped. find_drive_root, find_url_scheme, find_model_id, the endpoint/port/timeout/command/tool-identity/tier classes all return None on that line. scan_literals returns empty. The same holds for /gauntlet/..., /code/..., /build/..., /cache/... -- /app in particular is the standard container working directory. The module's "Residual, recorded rather than hidden" section (literals.rs:20-37) names exactly three incomplete classes (model id, tier policy, tool identity) and asserts the absolute-path class's only posture issue is over-flagging in comments (literals.rs:10), so a reader is told this class is complete when it is enumerated. A boundary-based rule (a string literal whose first byte is '/' followed by a name segment) would be mechanical and complete.
```

**Requirement.** Spec section 8.1 lists "path" first among the things that may not be a literal; AK-016 restates it. The artifact's own discipline -- literals.rs:5-6, "a class whose posture is not written down is a class that gets quietly weakened" -- requires an enumerated class to be recorded as a residual.

### Critic 2 — checked and found sound

Scope and method: read-only inspection of crates/gauntlet-arch-tests/src/{lib,deps,denylist,literals,walk,text,escapes,envfile}.rs, tests/contract/*.rs, crates/gauntlet-arch-tests/Cargo.toml, the workspace Cargo.toml, all 4 core + 11 adapter manifests, .env.example, deny.toml, justfile, .agents/specs/Core-spec.md sections 4.1/7.1/8.1/8.3/8.4/8.6/24.3/Annex E, and .wayfinder/answer-key.md. Nothing under .agents/reports/ was opened. No cargo/just invocation. Evasion traces were done by hand against the source, not by execution.

WHAT HOLDS UP (verified, not assumed):

1. AK-019 is sound end to end. I extracted Annex E independently (92 variables) and parsed .env.example independently (92 keys): zero missing, zero extra, zero non-empty values, zero duplicates, zero credential shapes. I also grepped the entire spec for GAUNTLET_ tokens outside Annex E; the only one is GAUNTLET_RESULT_PATH, which envfile.rs:41-45 names, justifies against sections 10.3/Annex C, and asserts absent from BOTH the annex and the file (envfile.rs:169-185), so the exclusion cannot go stale silently. The non-vacuity floors (envfile.rs:152, 160) are 50 against an actual 92 -- a real floor, not today's count. This instrument has teeth and I found no way past it.

2. All five AK locators resolve. Cargo.toml:22-36 declares [[test]] targets named architecture_boundaries, adapter_registry, no_hardcoded_literals, env_example_complete, each pointing at ../../tests/contract/<name>.rs; the fn names denylist and selection_is_config_driven match the ::-suffixed locators in .wayfinder/answer-key.md exactly. Deleting any of those files is a hard build failure, so the locators cannot rot into empty directories.

3. The repo-level scans do assert on the post-filter set. assert_scan_is_not_vacuous (lib.rs:213-273) compares report.scanned.len() against the in-scope core-file count, asserts report.skipped == 0, requires each core crate's manifest plus at least one .rs, and iterates ACTUAL tree files to catch a narrowed scope predicate -- I confirmed that adding `&& !rel.contains("/vendored/")` to in_denylist_scope plus a real crates/gauntlet-app/src/vendored/store.rs does fire the lib.rs:240-244 violation. read_scannable (walk.rs:165-192) converts unreadable, oversized, and non-UTF-8 files into violations rather than skips, and symlinks are reported rather than silently dropped (walk.rs:129-140). check_defaults_exemption's unwrap_or_default (lib.rs:376) fails SAFE -- an unreadable defaults.rs yields an empty string, which then fails all six DEFAULTS_TABLE_SETTINGS checks. I looked specifically for default-empty-on-failure and found only the deps.rs:353 case reported above.

4. The DEFAULTS_MODULE exemption is not vacuous today: crates/gauntlet-app/src/config/defaults.rs does not exist, so check_defaults_exemption takes the inactive branch (lib.rs:389-400) and requires in_denylist_scope == in_literal_scope over every walked file. The exemption is one exact path, not the config/ directory, and the vendor denylist still applies to it.

5. Directory renames are caught. I checked whether the CORE_CRATES constant makes the floor circular: renaming crates/gauntlet-app fires "scope is missing crates/gauntlet-app/Cargo.toml" (lib.rs:248-252), and editing the constant itself breaks walk::scope_tests (walk.rs:244-320), which pins all four crate paths by literal. Note the comment at lib.rs:246-247 ("derived from the discovered crate set, not a constant") is nonetheless inaccurate -- the loop iterates walk::CORE_CRATES, a constant; the property holds by a different mechanism than the comment claims.

6. A new non-adapter workspace member cannot become a hiding place: deps::classify (deps.rs:101-113) rejects any unrecognized package name, and Layer::allowed_internal (deps.rs:81-98) returns hardcoded names, so a newly classified crate can never legally be depended on by anything. The intended escape hatch (crates/adapters/**) is the one section 4.1 sanctions.

ON THE DELIBERATE CLI CARVE-OUT YOU ASKED ME TO JUDGE: the SCOPE decision is correct and matches section 4.1 -- walk.rs:31-44 and walk.rs:219-223 confine the adapter-identifier rule to domain/ports/app while walk.rs:203-213 keep gauntlet-cli inside both the vendor denylist and the literal scan, and I confirmed by tracing that the cli really is still scanned for both. The IMPLEMENTATION of the carve-out is what is exploitable: it is a whole-unit `continue` rather than the substring mask its own doc comments describe (finding 1), and the predicate that drives it has no anti-carve-out assertion (finding 2).

ONE SPEC-LEVEL AMBIGUITY, NOT A DEFECT IN THE ARTIFACT: spec section 8.3 names `_GIT_STRATEGY` in the limits group while Annex E lists GAUNTLET_VERSIONING_STRATEGY and section 8.6 lists git_strategy. envfile.rs treats Annex E as canonical, which section 8.3 itself authorizes ("Full table in Annex E"). Worth flagging to whoever owns the spec, but the instrument's reading is defensible and I did not count it as a finding.

ONE UNTESTED WIRING WORTH KNOWING ABOUT (subsumed by findings 1 and 2, so not listed separately): the role assignment at lib.rs:192-196 is exercised by no test. Inverting the two branches -- Inner for the cli, CompositionRoot for domain/ports/app -- leaves every test in the suite green today, because no core file currently contains any vendor or adapter identifier. The scope predicates are unit-tested; the code that consumes them is not.
