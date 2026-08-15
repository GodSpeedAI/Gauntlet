# Task 2 — blind re-critique of the corrected artifact

Three independent blind critics (lenses: evasion, conformance, vacuity), read-only
source inspection, no compilation. Each was blind to the builder reasoning and to the
earlier critique. Findings below are pooled and ordered by severity.

| Critic | Verdict | critical | major | minor |
|---|---|---|---|---|
| 1 | residuals-found | 3 | 8 | 2 |
| 2 | residuals-found | 1 | 4 | 4 |
| 3 | residuals-found | 0 | 7 | 5 |

## Critic 1

### R1 — **CRITICAL** — `crates/gauntlet-arch-tests/src/deps.rs:254`

**Summary.** read_members reads only the literal [workspace].members array, so any crate Cargo auto-includes as a member (or a path dep outside the workspace dir) is invisible to the dependency-direction gate — the domain layer can take a compile-time dependency on an arbitrary infrastructure crate and pass.

**Concrete evasion.**

```
Create crates/gauntlet-infra/{Cargo.toml,src/lib.rs} (package name gauntlet-infra, [dependencies] rusqlite = "0.32", reqwest = "0.12") and do NOT add it to [workspace].members. Then add to crates/gauntlet-domain/Cargo.toml:

    [dependencies]
    infra = { path = "../gauntlet-infra" }

Cargo builds this fine ("all path dependencies residing in the workspace directory automatically become members"), but read_members (deps.rs:252-258) enumerates only the declared members array, so gauntlet-infra is never a Member. In internal_deps, deps.rs:174-179 resolves path -> "crates/gauntlet-infra", finds no matching Member, leaves dep = "infra", and deps.rs:188 (`if members.iter().any(|m| m.name == dep)`) discards it as external. walk.rs:11-16 never scans crates/gauntlet-infra because it is not in CORE_CRATES, so its rusqlite/reqwest identifiers are also unpoliced. `just boundaries` reports zero violations. The same works with path = "../../vendor/infra" (outside the workspace dir, so not even auto-included).
```

**Requirement.** AK-001 (domain has zero compile-time dependencies on ports, adapters, or infrastructure); Task 2 step 3(a)

### R2 — **CRITICAL** — `crates/gauntlet-arch-tests/src/denylist.rs:27`

**Summary.** The denylist is a hand-curated vendor-word list; it never derives the actual adapter identifiers from the workspace, and 10 of the 11 adapter crate names contain no denied term, so adapter selection by string literal in domain/application source passes.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs:

    pub fn runner_for(role: &str) -> &'static str {
        match role { "builder" => "prime-agent", "critic" => "agent-mock", _ => "cli-agent" }
    }
    pub const OBSERVER: &str = "gauntlet-adapter-observe-git";

None of "prime-agent", "agent-mock", "cli-agent", "gauntlet-adapter-observe-git", "gauntlet-adapter-compare-diff", "gauntlet-adapter-evidence-fs", "gauntlet-adapter-observe-{build,test,command,filesystem}" contains any DENIED_TERMS entry (only gauntlet-adapter-state-sqlite does, via "sqlite"), and literals.rs has no adapter-name class. scan_source (denylist.rs:105) and scan_literals (literals.rs:62) both return empty. The workspace member list is already available to the test crate (deps.rs::read_members returns every crates/adapters/* name), but the denylist never consults it.
```

**Requirement.** AK-002 (no adapter identifier appears in domain or application source); AK-014 (no adapter is chosen by a literal in domain or application code); spec §24.3

### R3 — **CRITICAL** — `crates/gauntlet-arch-tests/src/walk.rs:68`

**Summary.** collect_files drops symlinked entries entirely (neither is_dir() nor is_file() is true for a symlink), but rustc follows symlinks — a symlinked source file or module directory inside a core crate compiles into the crate while escaping both scanners.

**Concrete evasion.**

```
`ln -s ../../../../vendor/store.rs crates/gauntlet-app/src/store.rs` and add `mod store;` to crates/gauntlet-app/src/lib.rs. DirEntry::file_type() does not traverse symlinks, so at walk.rs:66-70 file_type.is_dir() is false and file_type.is_file() is false and the entry is never pushed to `out`. store.rs can contain `pub const MODEL: &str = "claude-3-5-sonnet";`, `pub const API: &str = "https://api.example/v1";`, `pub const ROOT: &str = "/home/agent/state";` and any denied identifier; none of it is ever read. The `mod store;` line itself is clean text. A symlinked *directory* (crates/gauntlet-app/src/config -> ../../../shared/config) removes an entire subtree the same way. The anti-vacuity asserts (denylist.rs:180-191, literals.rs:314-322) only require Cargo.toml/lib.rs to be present, which is unaffected. The walk.rs:44-45 comment presents this as a safety property; it is the opposite.
```

**Requirement.** AK-002, AK-016; Task 2 step 3(b)(c) — the scan scope must cover what compiles into the core crates

### R4 — **MAJOR** — `crates/gauntlet-arch-tests/src/walk.rs:11`

**Summary.** Scope is decided by four hardcoded directory prefixes, so core-crate code pulled in via #[path], include!, or include_str! from outside those directories is compiled but never scanned.

**Concrete evasion.**

```
In crates/gauntlet-app/src/lib.rs:

    #[path = "../../../shared/wiring.rs"]
    mod wiring;

or `include!("../../../shared/wiring.rs");`. The attribute line contains no denied term and no literal pattern (relative paths are explicitly not flagged — literals.rs:367-370 asserts relative paths pass), and shared/wiring.rs is outside CORE_CRATES so in_denylist_scope (walk.rs:96) and in_literal_scope (walk.rs:104) both return false for it. wiring.rs may contain `use rusqlite::Connection;`, hardcoded model ids, URLs, and absolute paths, all compiled into gauntlet-app. Same for `const KEY: &str = include_str!("../../../shared/model-id.txt");`.
```

**Requirement.** AK-002, AK-016; spec §8.1 ("no literal in domain or application code")

### R5 — **MAJOR** — `crates/gauntlet-arch-tests/src/walk.rs:82`

**Summary.** read_scannable silently returns None for any file over 1 MiB and both repo tests `continue` on None, so an oversized but perfectly valid Rust source file in a core crate is never scanned.

**Concrete evasion.**

```
Create crates/gauntlet-domain/src/big.rs containing 1.1 MiB of doc-comment padding plus `pub const MODEL: &str = "claude-3-5-sonnet";` and `pub const ROOT: &str = "/home/agent/state";`, and add `mod big;` to lib.rs. walk.rs:82-87 sees len() > MAX_SCAN_BYTES (walk.rs:28) and returns None; denylist.rs:198-200 and literals.rs:326-328 both `continue` with no record that a file was skipped. The anti-vacuity asserts count entries in the *file list*, which still contains big.rs, so they do not fire. `just boundaries` passes. Nothing in the gate distinguishes "a generated blob" (the stated justification) from "a large hand-written module", and no violation is emitted for the skip itself.
```

**Requirement.** AK-016, AK-002; "missing evidence is not passing evidence" (I8)

### R6 — **MAJOR** — `crates/gauntlet-arch-tests/src/denylist.rs:107`

**Summary.** Both scanners match line-by-line on raw text, so Rust's backslash-newline string continuation (and concat!/\u escapes) splits any denied term or literal across lines and neither half matches.

**Concrete evasion.**

```
In crates/gauntlet-app/src/lib.rs:

    pub const MODEL: &str = "claud\
    e-3-5-sonnet";
    pub const ROOT: &str = "/hom\
    e/agent/state";
    pub const STORE: &str = concat!("sq", "lite");

scan_source iterates text.lines() (denylist.rs:107) and scan_literals iterates text.lines() (literals.rs:64). Line 1 contains "claud" (not "claude"), line 2 contains "e-3-5-sonnet" (no MODEL_FAMILIES prefix at a word boundary). Line 1 contains "/hom", line 2 "e/agent/state" — ABSOLUTE_PATH_PREFIXES uses line.contains("/home/") and find_drive_root needs [alpha]:[\\/], so neither fires. concat!("sq", "lite") never puts "sqlite" contiguously on a line. Rust's line-continuation strips the leading whitespace, so the compiled constants are exactly "claude-3-5-sonnet" and "/home/agent/state". The edit reads as ordinary line-wrapping in review.
```

**Requirement.** AK-002, AK-016; spec §8.1

### R7 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:62`

**Summary.** AK-016 names six literal categories; scan_literals implements only path, model id, URL, endpoint, and port. Command, timeout, and tier-policy literals have no scanner at all, so they cannot fail the build.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs:

    pub const BUILDER_TIMEOUT_SECS: u64 = 5400;
    pub const POLL_INTERVAL_MS: u64 = 250;
    pub const AGENT_CMD: &str = "npx --yes agent-runner --print";
    pub const TIER_POLICY: &str = "irreversible";

All pass: 5400 and 250 are not in WELL_KNOWN_PORTS (literals.rs:44-47) so find_well_known_port returns None; "npx --yes agent-runner --print" contains no DENIED_TERMS entry and no MODEL_FAMILIES prefix; "irreversible" is matched by nothing. Spec §8.1 and AK-016 both name "command", "timeout", and "tier policy" explicitly, and the literals.rs module header (lines 1-22) declares only five classes without acknowledging the three uncovered ones.
```

**Requirement.** AK-016 (no path, model, provider, command, timeout, or tier policy is a hardcoded literal outside the declared defaults table); spec §8.1

### R8 — **MAJOR** — `crates/gauntlet-arch-tests/src/denylist.rs:44`

**Summary.** DENIED_TERMS omits sandbox and database identifiers the spec itself names as backends, so hardcoding them in the application layer passes the denylist.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs:

    pub const DEFAULT_SANDBOX: &str = "seatbelt";   // spec §8.2 backend enum
    pub const FALLBACK_SANDBOX: &str = "mxc";       // spec §4.1 diagram, §8.2
    pub const HEAVY_SANDBOX: &str = "microvm";      // spec §8.2
    pub const STATE_BACKEND: &str = "duckdb";

None of "seatbelt", "mxc", "microvm", "duckdb", "sled", "libsql", "sqlx", "ureq", "hyper", "nsjail", "gvisor", "bedrock", "vertex", or "groq" is in DENIED_TERMS (denylist.rs:27-65), while "bwrap" from the same spec §8.2 enum is. contains_term returns false and scan_source emits nothing. §24.3 requires that *sandbox* and *database* identifiers appear only under crates/adapters/**; an enumerated list that omits three of the five backend ids the spec prints verbatim cannot enforce that.
```

**Requirement.** AK-002; spec §24.3 ("provider, harness, database, browser, and sandbox identifiers appear only under adapters/ and configuration files")

### R9 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:27`

**Summary.** find_model_id is anchored to an enumerated MODEL_FAMILIES list, so any model id from a family not on the list — including Anthropic's own ids written without the "claude" prefix — is not a hardcoded model id as far as the gate is concerned.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs:

    pub const BUILDER_MODEL: &str = "sonnet-4-5";
    pub const CRITIC_MODEL: &str = "kimi-k2-0905";
    pub const CHEAP_MODEL: &str = "phi-4";

find_model_id (literals.rs:147-186) requires a prefix from MODEL_FAMILIES at an ASCII word boundary; "sonnet", "kimi", "minimax", "phi", "nova", "command-r" are absent from both MODEL_FAMILIES (literals.rs:27-30) and DENIED_TERMS, so all three lines produce zero violations while being exactly the hardcoded model ids AK-016 forbids.
```

**Requirement.** AK-016 (no model is a hardcoded literal outside the declared defaults table); spec §8.1

### R10 — **MAJOR** — `crates/gauntlet-arch-tests/src/deps.rs:86`

**Summary.** classify() assigns the Adapter layer purely by directory prefix and Layer::Cli auto-admits every member under crates/adapters/, so application logic can be relocated into the unscanned adapter region and wired from the composition root with no violation.

**Concrete evasion.**

```
Create crates/adapters/gauntlet-adapter-runtime (add to members), move RunController/IterationEngine there, and add `gauntlet-adapter-runtime.workspace = true` to crates/gauntlet-cli/Cargo.toml. deps.rs:86-88 classifies it Adapter on the path prefix alone (no content check), deps.rs:73-80 automatically adds it to the cli's allowed set because its dir starts with "crates/adapters/", and walk.rs:11-16 never scans it. The relocated application code may then contain hardcoded model ids, URLs, absolute paths, and any vendor identifier, all legal in that directory. The unscanned region is self-service: a future agent extends it by creating a directory.
```

**Requirement.** AK-002, AK-016; spec §4.1 (application layer), §24.3

### R11 — **MAJOR** — `.env.example:1`

**Summary.** AK-019 requires a test_report from an instrument, but no test anywhere reads .env.example — the file's completeness and credential-freedom are asserted by nothing.

**Concrete evasion.**

```
Delete the `GAUNTLET_OTEL_ENDPOINT=` line from .env.example, or add `GAUNTLET_API_KEY=sk-live-abc123`, and every gate stays green: `just boundaries` runs only `cargo nextest run -p gauntlet-arch-tests`, whose modules are exactly denylist/deps/literals/walk (crates/gauntlet-arch-tests/src/lib.rs:24-27); grep for "env.example", "env_example", or "GAUNTLET_" across crates/**/*.rs returns nothing. tests/contract/ contains only .gitkeep, so the declared locator tests/contract/env_example_complete does not exist. (I verified by script that the file's current content does match Annex E exactly — 92 of 92 variables, all empty — so the content is right; the instrument is absent.)
```

**Requirement.** AK-019 (a .env.example exists listing every recognized GAUNTLET_ variable with empty values and no credentials; evidence_required: test_report at tests/contract/env_example_complete, producer: instrument)

### R12 — **MINOR** — `crates/gauntlet-arch-tests/src/walk.rs:15`

**Summary.** gauntlet-cli is in the denylist scope with no carve-out, while deps.rs explicitly permits cli -> every adapter; the composition root therefore cannot name the adapter it is required to wire, and the predictable remedy is to weaken the denylist.

**Concrete evasion.**

```
When Task 20 wires the CLI, adding `gauntlet-adapter-state-sqlite.workspace = true` to crates/gauntlet-cli/Cargo.toml fails scan_manifest: contains_word("gauntlet-adapter-state-sqlite", "sqlite") is true (preceded by '-', ends at end-of-string, so both ASCII word boundaries hold). Likewise `use gauntlet_adapter_state_sqlite::SqliteStore;` in main.rs fails scan_source. deps.rs:73-80 says this dependency is allowed; walk.rs:15 says the identifier is not. The gate makes the required end state unreachable, and the cheapest fix — removing "crates/gauntlet-cli" from CORE_CRATES or adding a cli carve-out to DENIED_TERMS — deletes the only source-level gate on the composition root. Spec §24.3's second bullet scopes the type-name ban to "domain or application source", so the stricter reading is a choice, not a requirement.
```

**Requirement.** AK-002; spec §24.3; Task 2 step 2 ("cli wires everything")

### R13 — **MINOR** — `crates/gauntlet-arch-tests/src/lib.rs:14`

**Summary.** The three AK locators point at tests/contract/, which contains only .gitkeep; the instruments live in crates/gauntlet-arch-tests and no CandidateRevision records the relocation.

**Concrete evasion.**

```
AK-001 requires evidence at tests/contract/architecture_boundaries, AK-002 at tests/contract/architecture_boundaries::denylist, AK-016 at tests/contract/no_hardcoded_literals. `find tests -type f` returns five .gitkeep files and nothing else. .wayfinder/reference.yaml:101 still lists evidence_required: [tests/contract/architecture_boundaries], and grep for "candidate", "revision", or "relocat" in reference.yaml finds no CandidateRevision for the move. A grader following the ruler's locators verbatim finds an empty directory. Annex A states a locator may be relocated "only by recording a CandidateRevision against the reference — never by silent edit."
```

**Requirement.** AK-001, AK-002, AK-016 evidence_required locators; spec Annex A locator-relocation rule

### Critic 1 — checked and found sound

Source-inspection only; nothing compiled, nothing written. Read: Cargo.toml (root), all 16 member manifests, all core/adapter sources, all five arch-test modules line by line, justfile, .env.example, deny.toml, spec §4.1/§8.1/§8.2/§8.3/§24.2/§24.3/§25/Annex E, .wayfinder/reference.yaml. I did not open anything under .agents/reports/task2/.\n\nWhat I checked and found genuinely sound:\n\ndeps.rs — the manifest parser is the strongest part of the artifact. It resolves plain keys, locally-declared `package = \"...\"` renames, `path = \"...\"` deps normalized through `..` segments against the member dir, `foo.workspace = true` inheritance resolved through the root [workspace.dependencies] table *including* an inherited `package` rename (deps.rs:131-149, 180-186), and it walks dependencies / dev-dependencies / build-dependencies plus every target-scoped variant of all three (deps.rs:102-123). Dev/build deps count, so a test-only leak is a leak. An unclassified member is itself a violation (deps.rs:209-215) rather than a silent pass, and malformed TOML is a violation too. Anti-vacuity asserts require all five named members, at least one crates/adapters/* member, and a non-empty root [workspace.dependencies] (deps.rs:315-343). I checked one apparent hole and found it closed by Cargo rather than by the scanner: a root `[workspace.dependencies]` entry that renames via `path` alone (e.g. `infra = { path = \"crates/gauntlet-app\" }`) is not resolved by workspace_dep_names, but Cargo rejects a path dep whose key does not match the package name unless `package = \"...\"` is present — and `package` *is* resolved. So that one is not reportable. The declared layer table matches the workspace manifests exactly: domain has no [dependencies], ports has domain, app has domain+ports, cli has domain+ports+app, all 11 adapters declare nothing, arch-tests depends only on `toml`.\n\ndenylist.rs — substring (not word-boundary) matching in source text correctly defeats the obvious rename evasions: SqliteConn, sqlite_conn, REQWEST_CLIENT, r#sqlite, and RusQLite (two hits) all flag, and the test at denylist.rs:239-259 pins that behavior. There is deliberately no defaults-module or config carve-out in the denylist. Manifests are checked on dependency keys *and* rename values across all dependency sections. Malformed manifests are flagged, not swallowed.\n\nwalk.rs — every regular file is in scope regardless of extension, so relocating a leak into a .txt/.json/.md inside a core crate does not help. The literal whitelist is exactly one file path (crates/gauntlet-app/src/config/defaults.rs), not the config/ directory, so config/parser.rs stays policed — the test at walk.rs:149-158 pins that. An unreadable in-scope file panics rather than being skipped (walk.rs:88). The non-UTF-8 skip is not exploitable for Rust sources, since rustc requires UTF-8; I did not report it. This crate is outside its own scope by construction, not by special case.\n\nliterals.rs — find_ipv4 correctly rejects `[127, 0, 0, 1]` and `\"0.9.12+spec\"`; find_drive_root is word-boundary anchored so `https://` and `serde::fmt` do not false-positive; the port check excludes colon-attached digits to avoid double-reporting an endpoint. Manifests are in the literal scope too, so a `git = \"https://...\"` source in a core Cargo.toml is caught as a URL.\n\njustfile — all ten required recipes exist; doctor/schemas/roles-verify exit 2 rather than exit 0, so they cannot be mistaken for green. `ci` runs ruler last. `boundaries` is `cargo nextest run -p gauntlet-arch-tests`, which does pick up the `#![cfg(test)]` unit tests.\n\n.env.example — verified by script against Annex E: 92 variables in the annex, 92 in the file, symmetric difference empty, all values empty, no credentials. deny.toml exists with a closed licence allowlist and unknown-registry/unknown-git denied.\n\nThe findings above are ordered by how easily a future agent could use them. The first three are the ones I would expect to be used without any intent to deceive: a path dependency to a helper crate, an adapter name in a config match arm, and a symlinked module.

## Critic 2

### R14 — **CRITICAL** — `crates/gauntlet-arch-tests/src/deps.rs:188`

**Summary.** The dependency-direction gate only considers dependencies that resolve to a workspace member, and the manifest denylist only inspects dependency keys and `package =` renames. An infrastructure dependency added to gauntlet-domain or gauntlet-app therefore passes every gate unless its crate name happens to be one of the 34 words in DENIED_TERMS — and a `path`/`git` dependency evades even that.

**Concrete evasion.**

```
Add to crates/gauntlet-app/Cargo.toml: `store = { path = "../../vendor/rusqlite-store" }`. deps.rs:188 (`if members.iter().any(|m| m.name == dep)`) drops it because the path target is not a workspace member; denylist.rs:137-144 only examines the key (`store`) and a `package` rename (absent), never the `path` value; literals.rs sees only a relative path. `just check && just lint && just deny && just boundaries` all stay green while the application layer compiles against a SQLite-backed crate. The simplest form needs no path at all: `tokio = "1"` or `ureq = "2"` in crates/gauntlet-domain/Cargo.toml is an infrastructure dependency in the domain layer that no gate reports. Note `libsqlite3-sys = "0.30"` also passes, because contains_word (denylist.rs:86-100) requires a word boundary and the `sqlite` inside `libsqlite3-sys` is preceded by `b`.
```

**Requirement.** AK-001 ("zero compile-time dependencies on ports, adapters, or infrastructure"), AK-002, spec §24.3 ("the domain crate/module declares zero dependencies on port, adapter, or infrastructure crates")

### R15 — **MAJOR** — `crates/gauntlet-arch-tests/src/lib.rs:24`

**Summary.** AK-019 has no producer at all: no test, script, or just recipe anywhere in the repo reads `.env.example`. The file's content is currently correct, but the gate that would keep it correct does not exist, so the Unit cannot settle on any evidence this artifact produces.

**Concrete evasion.**

```
Delete the line `GAUNTLET_MAX_DEPTH=` from .env.example, or change line 42 to `GAUNTLET_CRITIC_MODEL=sk-ant-api03-REAL-KEY`. `just ci` (check, test, lint, deny, boundaries, ruler) stays green — grep for `env.example`/`env_example` across *.rs, justfile, *.sh and *.toml returns nothing. lib.rs declares three mechanisms (denylist, deps, literals); there is no fourth that parses spec Annex E and diffs it against the file, and no assertion that every value is empty and credential-free.
```

**Requirement.** AK-019 (evidence_required locator tests/contract/env_example_complete, kind test_report, producer instrument); spec §8.1 ("A .env.example with empty values is REQUIRED")

### R16 — **MAJOR** — `crates/gauntlet-arch-tests/src/deps.rs:306`

**Summary.** None of the AK evidence locators resolves to anything. The implemented gates are in-crate unit tests (gauntlet-arch-tests::deps::repo::dependency_direction_holds, ::denylist::repo::core_crates_contain_no_denied_identifiers, ::literals::repo::core_crates_contain_no_hardcoded_literals); `tests/contract/` contains only a `.gitkeep`, is not a cargo target of any crate, and no gate emits a machine-readable test report. The relocation is silent — Annex A's preamble permits relocating a locator only by recording a CandidateRevision, and `.agents/reports/reference-candidates.md` (the file .gauntlet/reference.lock names for exactly this) does not exist.

**Concrete evasion.**

```
The unbacked claim: `just boundaries` is presented (Cargo.toml:4, justfile:37-38, deps.rs:1-3) as the mechanical enforcement of §24.3, but a verifier resolving AK-001's `evidence_required.locator: tests/contract/architecture_boundaries` finds an empty directory and would have to settle the Unit on an address that does not exist. For these Units to settle with resolvable evidence, one of two things must exist: (a) real test targets named at the locator paths — tests/contract/architecture_boundaries.rs, .../architecture_boundaries::denylist, .../no_hardcoded_literals — or (b) a dated CandidateRevision in .agents/reports/reference-candidates.md mapping each locator to its actual test id, plus a producer that writes a `test_report` artifact (e.g. `cargo nextest run -p gauntlet-arch-tests --message-format libtest-json` captured into $GAUNTLET_REPORT_DIR), since `kind: test_report, producer: instrument` requires a durable report, which `just boundaries` does not currently produce.
```

**Requirement.** AK-001, AK-002, AK-016, AK-019 evidence_required locators; spec Annex A preamble (relocation only via a recorded CandidateRevision)

### R17 — **MAJOR** — `crates/gauntlet-arch-tests/src/walk.rs:68`

**Summary.** Scan scope is a directory-tree walk of four fixed paths rather than what the core crates actually compile, and every file the walker cannot handle is silently skipped instead of failing. Code that is compiled into the domain/application/cli layers can therefore sit outside both the denylist and the literal scan.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs: `#[path = "../../../vendor/store.rs"] mod store;` and put `const STORE: &str = "sqlite"; const API: &str = "https://api.example.com/v1";` in vendor/store.rs. The `#[path]` line itself is clean text (no denied term, no absolute path, no URL), the included file is outside CORE_CRATES (walk.rs:11-16, is_core_path walk.rs:108-112), so neither scan_source nor scan_literals ever sees it, and `just boundaries` passes. Two further silent-skip vectors with the same root cause: a symlinked directory inside a core crate is dropped entirely by collect_files (walk.rs:66-73 — a symlink is neither is_dir nor is_file, so it is neither traversed nor collected) while rustc compiles straight through it; and any source file over MAX_SCAN_BYTES returns None at walk.rs:82-87 and is skipped. In all three cases the scanner returns "no violations" for content it never read, which is missing evidence being counted as passing evidence (I8).
```

**Requirement.** AK-002, AK-016; spec §24.3 (denylist asserts identifiers appear only under adapters/ and configuration files)

### R18 — **MAJOR** — `crates/gauntlet-arch-tests/src/literals.rs:1`

**Summary.** The literal scanner's module doc claims spec §8.1 ("the environment is discovered and configured, never compiled in"), but implements four of the eight classes §8.1 and AK-016 enumerate. Commands, timeouts, and tier policy are not scanned at all, and the scanner's own unit test entrenches the timeout gap.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs: `pub const DEADLINE_SECS: u64 = 3600;` and `pub fn builder() -> std::process::Command { std::process::Command::new("git") }`. Both pass `just boundaries`: 3600 is not in WELL_KNOWN_PORTS (literals.rs:44-47) and literals.rs:463 asserts explicitly that `let ttl = 3600;` produces no flag — yet 3600 is precisely the spec §8.6 `max_wall_seconds` default, i.e. a defaults-table value compiled into the application layer. `git`, `cargo`, `bash` are in neither DENIED_TERMS nor any literal class, so a hardcoded builder command in application code is invisible. Plan Task 2 step 3(c) does enumerate only "absolute paths, model ids, URLs, port numbers", so the mechanism meets the plan; the gap is against the frozen Unit, and AK-016 must not be marked settled on this evidence while `command`, `timeout`, and `tier policy` literals are unpoliced.
```

**Requirement.** AK-016 ("No path, model, provider, command, timeout, or tier policy is a hardcoded literal outside the declared defaults table"); spec §8.1

### R19 — **MINOR** — `crates/gauntlet-domain/Cargo.toml:11`

**Summary.** The manifest comment claims more than the gate enforces: "the domain depends on nothing internal — no ports, no application, no infrastructure. Adding any dependency here fails `just boundaries`." The second sentence is false for any dependency whose name is not in the 34-term denylist.

**Concrete evasion.**

```
Add `tokio = { version = "1", features = ["full"] }` to crates/gauntlet-domain/Cargo.toml. `just boundaries` passes, directly contradicting the comment on lines 11-12. The same overclaim appears at crates/gauntlet-app/Cargo.toml:14-15 ("depends on domain and ports only — never on adapters") for a `path`-declared adapter-equivalent crate. The claim would be accurate only if it read "any dependency on an internal crate, or on an external crate whose name matches the denylist".
```

**Requirement.** AK-001; spec §24.3 — a comment asserting mechanical enforcement that the mechanism does not provide

### R20 — **MINOR** — `crates/gauntlet-arch-tests/src/walk.rs:15`

**Summary.** The denylist scope and the dependency-direction rule contradict each other for the composition root: deps.rs explicitly allows gauntlet-cli to depend on every adapter crate, but gauntlet-cli is in CORE_CRATES, so the manifest edit that exercises that allowance trips the denylist. The gate as built cannot stay green once the CLI wires the adapters, which pressures a future carve-out — a weakened boundary test that still looks like coverage.

**Concrete evasion.**

```
Add `gauntlet-adapter-state-sqlite.workspace = true` to crates/gauntlet-cli/Cargo.toml. dependency_direction_holds passes (Layer::Cli extends its allowed set with every member under crates/adapters/, deps.rs:73-80), but scan_manifest flags the dependency key: contains_word("gauntlet-adapter-state-sqlite", "sqlite") is true because the match is preceded by `-` and ends the string. So `just boundaries` fails on a manifest edit that spec §4.1 ("cli wires everything") mandates. denylist.rs:16-19 states the denylist "intentionally has NO defaults-module or config carve-out", which anticipates a defaults exemption but not this one.
```

**Requirement.** Task 2 step 2 ("cli wires everything"), spec §4.1; AK-002's gate must not have to be weakened to let Task 20 land

### R21 — **MINOR** — `crates/adapters/gauntlet-adapter-state-sqlite/Cargo.toml:12`

**Summary.** Task 2 step 2 requires the dependency direction to be declared in each Cargo.toml, but all 11 adapter manifests declare no `[dependencies]` section at all, and gauntlet-cli declares no adapter. The direction "adapters depend on domain + ports" and "cli wires everything" is enforced only as an upper bound, never actually declared anywhere in the artifact.

**Concrete evasion.**

```
grep across crates/adapters/*/Cargo.toml returns zero `[dependencies]` sections; the direction the requirement asks to declare exists only in comments (e.g. this file's lines 10-13) and in the allowed-set table in deps.rs:65-82. An adapter crate that never depends on gauntlet-ports — i.e. that implements no port and is an adapter in name only — is indistinguishable from a correct one to every gate in `just ci`.
```

**Requirement.** Task 2 requirement 2 ("adapters depend on domain + ports; cli wires everything")

### R22 — **MINOR** — `crates/gauntlet-arch-tests/src/literals.rs:224`

**Summary.** The well-known-port class is defeated by an idiomatic Rust numeric separator or any non-decimal literal, because find_well_known_port only matches maximal decimal runs with ASCII word boundaries and `_` counts as a word byte.

**Concrete evasion.**

```
`const API_PORT: u16 = 8_080;` in crates/gauntlet-app/src/lib.rs passes `just boundaries`: the digit run `8` is followed by `_`, so bounded_after (literals.rs:224) is false, and the run `080` is preceded by `_`, so bounded_before is false. `0x1F90` and `0o17620` evade for the same reason. Plan Task 2 step 3(c) names port numbers as a required class, so this is a class that exists but can be stepped over with one underscore.
```

**Requirement.** AK-016; Task 2 requirement 3(c) ("a no-hardcoded-literals scan (absolute paths, model ids, URLs, port numbers)")

### Critic 2 — checked and found sound

Read-only source inspection; nothing compiled, nothing written, no git mutation, and nothing under .agents/reports/task2 was opened.

What I checked and found sound:

1. Layering vs spec §4.1 (read at Core-spec.md:140-170). All 16 member manifests match the mandated direction exactly: gauntlet-domain declares no [dependencies]; gauntlet-ports -> gauntlet-domain; gauntlet-app -> domain + ports; gauntlet-cli -> domain + ports + app; the 11 adapters declare nothing (see the minor finding); gauntlet-arch-tests -> `toml` only. No manifest declares an inward violation. The members list matches the tree; resolver 3, edition 2024, unsafe_code = "forbid" workspace-wide.

2. The internal dependency-direction scanner is genuinely hard to evade. deps.rs resolves `package =` renames (both locally declared and inherited through the root [workspace.dependencies] alias table), `path =` entries joined and normalized against the member directory, target-cfg-scoped sections, and dev-/build-dependencies. Unclassified members are themselves a violation, and the repo test carries anti-vacuity asserts (deps.rs:315-343) for all five named crates plus at least one adapter and a non-empty workspace-dependency table. I could not construct an internal-crate evasion; every hole I found is about external or out-of-tree things, not internal edges.

3. .env.example vs spec Annex E (Core-spec.md:2082-2100): I extracted both variable sets programmatically. 92 in Annex E, 92 in the file, set difference empty in both directions — zero missing, zero extra. Every entry ends in `=` with nothing after it (zero non-empty lines), so no credential and no default is embedded. The §8.3 group list is also fully covered. AK-019's file requirement is met; only its producer is missing.

4. deny.toml expresses a real policy, not a stub: `[advisories] version = 2` (deny-by-default in the v2 format) with `yanked = "deny"`; a closed six-entry license allowlist; `[sources] unknown-registry = "deny"` and `unknown-git = "deny"`. `private = { ignore = true }` scopes to the workspace's own publish=false crates and does not weaken dependency license checking; `unused-allowed-license = "warn"` only affects unused allowlist entries; `[bans] multiple-versions = "warn"` is documented looseness and nothing in the spec forbids duplicate versions. No setting silently allows what the spec forbids.

5. justfile: `ci: check test lint deny boundaries ruler` runs every global gate the plan lists, in order, with ruler last so nothing can claim green while the frozen reference has moved. doctor, schemas, and roles-verify each echo to stderr and `exit 2` — they fail loudly rather than pretending success, and they are correctly not in `ci` (they would make it permanently red). `just ruler` calls scripts/verify-ruler.sh, which is `set -euo pipefail`, exits non-zero on a missing lock file, a missing digest tool, a missing source file, or a digest mismatch, and prints expected/actual — silence is not treated as agreement there.

6. AK-014: nothing in this artifact selects an adapter by a literal. crates/gauntlet-cli/src/main.rs contains no registry and no adapter identifier; gauntlet-cli/Cargo.toml declares no adapter dependency; grepping domain/ports/app/cli sources for adapter, sqlite, prime, mock returns only the generic English word "adapter" in two doc comments. The adapter path aliases in the root Cargo.toml [workspace.dependencies] are a manifest, not domain or application code. AK-014 is clean here, though also unenforced (no tests/contract/adapter_registry) and out of Task 2's scope.

7. The scanner unit tests are real teeth rather than decoration: every DENIED_TERM is exercised, case-insensitivity and compound-identifier matching are asserted (SqliteConn, sqlite_conn, REQWEST_CLIENT all flagged), each of the five literal classes has positive and negative fixtures, malformed manifests are flagged rather than swallowed, and both repo-level tests carry anti-vacuity asserts on scope size. The plan's own "Done when" teeth check does hold: `rusqlite`/`reqwest` as dependency keys in crates/gauntlet-domain/Cargo.toml are caught by the manifest denylist, and a model-id literal in domain sources is caught by literals.rs.

8. Status claims elsewhere are honest: .agents/CURRENT_STATUS.md:121 still records all 70 Units as `proposed`, and the reference manifest's QD-architecture entry names the same locator the ruler does. No comment or recipe in the artifact claims a Unit is settled.

Calibration: findings 1 and 4 are the two that let real coupling into the core layers today with a single ordinary-looking edit. Findings 2 and 3 are about evidence that cannot be produced rather than layering that is broken. The rest are narrower. Nothing here argues for new features — every item is a gap between what the artifact enforces and what the frozen Units and §4.1/§8.1/§24.3 already require.

## Critic 3

### R23 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/literals.rs:225`

**Summary.** The `colon_attached` suppression in `find_well_known_port` defers `host:port` to `find_network_endpoint`, but that function only recognizes three hardcoded host forms (`localhost:`, `0.0.0.0:`, `127.0.0.1:`) plus bare IPv4 — so every other `host:port` literal is flagged by neither class.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs: `pub const API_BIND: &str = "api.internal:8443";`. Traced by hand: no ABSOLUTE_PATH_PREFIXES hit; find_drive_root needs `[alpha]:[\\/]` and the byte after `:` is `8`; no `http(s)://`; no MODEL_FAMILIES stem; find_network_endpoint matches none of its three host prefixes and match_ipv4_at fails; find_well_known_port finds the digit run `8443` but `bytes[start-1] == b':'` sets colon_attached, so the `!colon_attached` guard at line 226 skips it. Zero violations. Same for `"db.prod:5432"` and `"[::1]:8080"`.
```

**Requirement.** AK-016 (no port/path literal outside the declared defaults table); Task 2 req 3(c) "port numbers"

### R24 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/denylist.rs:92`

**Summary.** `contains_word` treats digits and `_` as word bytes, and it is the ONLY matcher applied to Cargo.toml dependency keys (source text gets the stronger substring matcher). A denied vendor stem followed by a digit or underscore is therefore invisible in a core manifest, and deps.rs ignores it too because it is not a workspace member.

**Concrete evasion.**

```
Add to crates/gauntlet-domain/Cargo.toml: `[dependencies]` / `libsqlite3-sys = "0.30"`. contains_word("libsqlite3-sys", "sqlite") finds the term at index 3 but bytes[2]=='b' is a word byte so boundary_before is false; the loop restarts at index 4 and finds nothing. "rusqlite" is not a substring. scan_literals on the manifest finds no class (`0.30` is not a well-known port, `30` is not either). deps.rs internal_deps drops it because no member is named `libsqlite3-sys`. `just check`/`just boundaries`/`just deny` all stay green with the DOMAIN layer compiling against SQLite. `anthropic_sdk = "1"` and `openai_api = "1"` evade identically (boundary_after defeated by `_`).
```

**Requirement.** AK-001 (domain has zero compile-time dependencies on ports, adapters, or infrastructure); AK-002 (no adapter identifier in domain/application); Task 2 req 3(b)

### R25 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/walk.rs:46`

**Summary.** `core_tree_files` walks only the four literal directories in CORE_CRATES and skips symlinks by file type, but rustc's `#[path]` attribute and symlinks both compile out-of-tree files INTO a core crate. Any such file is invisible to both the denylist and the literal scan while being part of the domain/app binary.

**Concrete evasion.**

```
Add to crates/gauntlet-domain/src/lib.rs: `#[path = "../../../vendor/leak.rs"] mod leak;` and put `pub const DB: &str = "sqlite"; pub const M: &str = "claude-sonnet-4-5"; pub const U: &str = "https://api.example.com";` in /vendor/leak.rs. collect_files (walk.rs:55-74) never descends outside the four crate dirs, so leak.rs is never in `files`. The attribute line itself carries only a relative path (not in ABSOLUTE_PATH_PREFIXES) and the word "vendor" (not in DENIED_TERMS). All three scans report clean. The symlink variant (`ln -s ../../../vendor/leak.rs crates/gauntlet-domain/src/leak.rs` + `mod leak;`) is explicitly pre-authorized by the comment at walk.rs:44-45: entry.file_type().is_file() is false for a symlink, so it is dropped from `out`.
```

**Requirement.** AK-001, AK-002, AK-016; Task 2 req 3(b) and 3(c) scope

### R26 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/walk.rs:89`

**Summary.** `read_scannable` returns None for oversized (>1 MiB) and non-UTF8 files, and both repo-level tests turn that None into a silent `continue` with no record and no counter. This converts missing evidence into passing evidence — contradicting the function's own doc comment at walk.rs:77-79 ("A file that cannot be read at all still panics — that is missing evidence, not a pass").

**Concrete evasion.**

```
Create crates/gauntlet-app/src/big.rs whose first 1_048_577 bytes are a `//` comment block, ending with `pub const MODEL: &str = "gpt-4o"; pub const DB: &str = "sqlite";`, and add `mod big;` to lib.rs. walk.rs:82-87 sets oversized=true and returns None; denylist.rs:198-200 and literals.rs:326-328 each hit `else { continue; }`. The file is compiled into gauntlet-app and scanned by nothing. No assertion anywhere counts skipped files or requires skips==0.
```

**Requirement.** AK-002 and AK-016 (the denylist and no-hardcoded-literals instruments must cover core-crate sources); "missing evidence is not passing evidence"

### R27 — **MAJOR** — `/home/sprime01/projects/gauntlet/.env.example:1`

**Summary.** AK-019 requires a test_report produced by an instrument at tests/contract/env_example_complete. No such instrument exists anywhere in the deliverable: grep for `env_example`, `GAUNTLET_`, or `Annex E` across all *.rs returns zero hits, and tests/contract/ contains only .gitkeep. The .env.example is asserted correct by inspection only.

**Concrete evasion.**

```
Delete the line `GAUNTLET_MAX_ROUNDS=` from .env.example, or add `GAUNTLET_API_KEY=sk-live-abc123`. `just ci` (check test lint deny boundaries ruler) stays green — no gate reads this file. The gap is already live, not hypothetical: `GAUNTLET_RESULT_PATH` (spec §10.3 line 2015, §9 line 748) is a recognized GAUNTLET_ variable and is absent from .env.example, and nothing in the repo noticed.
```

**Requirement.** AK-019 (a .env.example exists listing every recognized GAUNTLET_ variable, empty values, no credentials); Task 2 req 5

### R28 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/lib.rs:14`

**Summary.** Every AK unit names an evidence locator under tests/contract/, but that directory holds only .gitkeep. The three instruments Task 2 actually built live at crates/gauntlet-arch-tests/src/{deps,denylist,literals}.rs and are not addressable at, aliased to, or mapped onto the declared locators by any file, just recipe, or nextest config (there is no .config/nextest.toml).

**Concrete evasion.**

```
A verifier resolving AK-001's `locator: tests/contract/architecture_boundaries`, AK-002's `tests/contract/architecture_boundaries::denylist`, or AK-016's `tests/contract/no_hardcoded_literals` finds an empty directory. There is no test named `architecture_boundaries`, `denylist` (the module is `denylist`; its repo test is `repo::core_crates_contain_no_denied_identifiers`), or `no_hardcoded_literals` in the binary that `cargo nextest run -p gauntlet-arch-tests` produces. Nothing fails; the evidence simply is not where the frozen ruler says it must be.
```

**Requirement.** AK-001, AK-002, AK-016, AK-019 evidence_required locators

### R29 — **MAJOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/literals.rs:62`

**Summary.** `scan_literals` implements five classes (absolute path, URL, model id, network endpoint, well-known port). Spec §8.1 — which outranks the plan text — enumerates more: "No path, model, provider, command, port number, timeout, tier policy, or tool identity is a literal in domain or application code." Command, timeout, tier policy, and tool identity have no scanner at all.

**Concrete evasion.**

```
Add to crates/gauntlet-app/src/lib.rs: `pub const TEST_CMD: &str = "cargo nextest run"; pub const STALL_MS: u64 = 900_000; pub const TIER: &str = "tier-2"; pub const TOOL: &str = "ripgrep";`. None of the five classes matches (900_000 is not in WELL_KNOWN_PORTS and the `_` separator breaks the digit run anyway; "tier-2" has no MODEL_FAMILIES stem; no denied term appears). `just boundaries` passes. In mitigation: Task 2 req 3(c) narrows the scan to "absolute paths, model ids, URLs, port numbers", so the builder met the plan text — but AK-016 and §8.1 are broader and are the standard being graded.
```

**Requirement.** AK-016 (no path, model, provider, command, timeout, or tier policy is a hardcoded literal); spec §8.1

### R30 — **MINOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/denylist.rs:180`

**Summary.** The denylist repo test's anti-vacuity block asserts against the PRE-filter `files` vector, not the post-filter set it actually scans, and the scan loop has no floor. The literals test (literals.rs:308-322) does the stronger thing — it filters first and asserts on `scoped`. The two paths are asymmetric, and the weaker one guards the denylist.

**Concrete evasion.**

```
Add `&& !rel.contains("/generated/")` to `walk::in_denylist_scope` (walk.rs:96). The anti-vacuity asserts at denylist.rs:180-191 still pass (they only check that the 4 Cargo.toml files and 3 lib.rs files appear in `files`, which is computed before the predicate runs), the `continue` at denylist.rs:196 silently drops the whole subtree, `violations` stays empty, and the test reports PASS. The walk.rs scope unit test exercises no `/generated/` path, so it does not catch it either. Separately, the literals floor `>= 4` (literals.rs:320) equals today's exact .rs count, so it can never detect a future exclusion.
```

**Requirement.** Anti-vacuity requirement of the lens: a repo-level scan must assert it scanned a non-empty set; Task 2 req 3(b)

### R31 — **MINOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/walk.rs:24`

**Summary.** DEFAULTS_MODULE names a whole-file literal-scan exemption for a path that does not exist (crates/gauntlet-app/src/config/ is absent — the crate contains only src/lib.rs). The exemption is vacuous today and pre-authorizes a hole: no test asserts the exempted file exists, that it is actually the §8.6 defaults table, or that anything in it is reachable as an overridable default.

**Concrete evasion.**

```
Creating crates/gauntlet-app/src/config/defaults.rs at any future point instantly exempts its entire contents from all five literal classes (absolute paths, URLs, model ids, endpoints, ports) with no size cap and no content check. The only guard is the string-equality unit test at walk.rs:152-154, which asserts the exemption applies — never that it is warranted. AK-016's "declared defaults table" is thus a self-declared path, not a verified one.
```

**Requirement.** AK-016 ("outside the declared defaults table"); lens item: vacuous carve-outs that pre-authorize a future hole

### R32 — **MINOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/literals.rs:50`

**Summary.** ABSOLUTE_PATH_PREFIXES is a case-SENSITIVE allowlist that requires a trailing slash, so common absolute paths are unflagged, and the URL class covers only http:// and https://.

**Concrete evasion.**

```
`let dir = "/tmp";` in crates/gauntlet-app/src/lib.rs passes — the needle is `"/tmp/"` and the line contains `/tmp"`, no trailing slash. Also unflagged: `"/workspace/repo"`, `"/dev/shm/gauntlet.sock"`, `"/proc/self/cwd"` (roots absent from the list), and `"/HOME/alice"` (line 69 matches on the raw `line`, not `lowered`, while `/Users/` is the only capitalized entry). URL class: `pub const EP: &str = "wss://api.example.com/v1";` and file:// / ftp:// schemes are not in the needle list at line 83.
```

**Requirement.** AK-016 ("no path ... is a hardcoded literal"); Task 2 req 3(c) "absolute paths, URLs"

### R33 — **MINOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/denylist.rs:135`

**Summary.** `scan_manifest` iterates only crate::deps::dependency_sections (dependencies / dev-dependencies / build-dependencies, plus target-scoped), checking dependency keys and `package = "..."` values. Every other table in a core crate's Cargo.toml is unscanned by the denylist, while non-manifest files get a whole-text scan.

**Concrete evasion.**

```
Add to crates/gauntlet-app/Cargo.toml: `[features]` / `default = ["sqlite"]` / `sqlite = []`, or `[package.metadata.gauntlet]` / `state_backend = "sqlite"`. dependency_sections returns nothing for [features] or [package.metadata], so scan_manifest emits no violation; scan_literals has no vendor-identifier class, so it emits none either. The vendor identifier sits in the application crate's manifest and every gate is green. Mitigating: §24.3 exempts "configuration files", and a manifest arguably is one — but the builder chose to police manifests, and the policing is partial in a way no test documents.
```

**Requirement.** AK-002 (no adapter identifier in domain or application); Task 2 req 3(b)

### R34 — **MINOR** — `/home/sprime01/projects/gauntlet/crates/gauntlet-arch-tests/src/lib.rs:24`

**Summary.** AK-014 has no instrument of any kind in this deliverable. The crate declares exactly three mechanisms (denylist, deps, literals); there is no adapter-registry or selection check, and tests/contract/adapter_registry does not exist.

**Concrete evasion.**

```
The claim "adapter selection resolves from configuration only" is currently vacuously true because no selection mechanism exists — crates/gauntlet-cli/src/main.rs is a 9-line stub that constructs nothing and crates/gauntlet-cli/Cargo.toml declares zero adapter dependencies. Nothing in the deliverable would fail if Task 20 wired `let store = gauntlet_adapter_state_sqlite::open();` into main.rs by a literal. This also surfaces a latent contradiction: because gauntlet-cli is inside CORE_CRATES, the composition root cannot name any adapter crate whose name contains a denied term, so the dependency key `gauntlet-adapter-state-sqlite` in cli/Cargo.toml will trip contains_word at denylist.rs:148 the moment Task 20 lands — the denylist is on a collision course with its own composition root and will have to be relaxed.
```

**Requirement.** AK-014 (adapter selection resolves from configuration only; locator tests/contract/adapter_registry::selection_is_config_driven)

### Critic 3 — checked and found sound

Read-only source inspection; no cargo/just invoked, nothing written to the repo, and nothing under .agents/reports/ was opened.

WHAT I TRACED AND FOUND SOUND:

1. Wiring. crates/gauntlet-arch-tests/src/lib.rs declares all four modules (denylist, deps, literals, walk) with no #[ignore], no should_panic, no cfg(feature) gates, and no .config/nextest.toml filter profile. `just boundaries` (cargo nextest run -p gauntlet-arch-tests) and `just test` (--workspace) both reach the repo-level tests. `just ci` sequences check/test/lint/deny/boundaries/ruler with ruler last. `#![cfg(test)]` at lib.rs:22 is harmless: `just check` uses --all-targets so the crate still compiles.

2. Error swallowing. I grepped for unwrap_or / .ok() / is_err / else-continue across the crate. Three hits. Two are benign: walk.rs:84 (metadata failure falls through to a panicking fs::read at line 88) and deps.rs:145 (alias fallback, correct semantics). The third, walk.rs:89 String::from_utf8().ok(), IS a missing-evidence-as-pass path and is reported as a finding. Parse errors are genuinely not swallowed: deps.rs:164 turns invalid TOML into a violation string, denylist.rs:128-133 does the same, and both are covered by negative unit tests (deps.rs:618-626, denylist.rs:325-329).

3. Scanner teeth (unit path). The three scanners are pure (path, text) -> Vec<String> functions with fixture tests that assert on violation COUNT and message CONTENT, not just non-emptiness. denylist.rs:220-237 loops over every entry in DENIED_TERMS and asserts each is flagged — genuinely non-vacuous. literals.rs has paired positive and negative fixtures per class. deps.rs proves the interesting evasions are closed: `package = "..."` renames (line 547), path deps resolved through join_posix (line 561), workspace-inherited renames resolving to the real crate name and NOT the alias (line 577), dev-dependencies counted as leaks (line 452), unclassified members flagged (line 608).

4. Scanner teeth (repo path). Critically, the repo-level tests call the SAME functions the unit tests exercise (check_member, scan_source, scan_manifest, scan_literals). There is no weaker parallel implementation — this was the specific asymmetry I went looking for and it is absent. My findings are about what reaches those functions (walk scope, read_scannable skips) and about gaps inside the matchers, not about a second, weaker code path.

5. Dependency-direction anti-vacuity. deps.rs:315-343 is real: it asserts all five named members are present, that at least one crates/adapters/ member exists, and that root [workspace.dependencies] resolved to something non-empty. A glob that matched nothing would fail here. read_members (deps.rs:250-278) propagates every missing or malformed manifest as an Err the caller panics on, so the members list cannot silently shrink. I confirmed the root Cargo.toml enumerates all 15 members explicitly (no globs), so read_members' literal path join is correct today.

6. repo_root(). walk.rs:166-178 asserts the resolved root contains both Cargo.toml and .wayfinder/answer-key.md, so a path-resolution regression fails loudly rather than scanning an empty tree.

7. Annex E coverage. I diffed all GAUNTLET_ tokens in spec Annex E (line 2082 onward) against .env.example: 92/92 present, all with empty values, no credentials, correct grouping. The file itself is correct — my AK-019 finding is strictly that nothing mechanically holds it that way.

8. Layer manifests. domain declares no [dependencies]; ports -> domain; app -> domain+ports; cli -> domain+ports+app; all 11 adapters declare nothing yet; arch-tests depends only on external `toml`. All correct per Task 2 req 2 and correctly enforced by check_member. The justfile has all ten required recipes; the three unimplemented ones (doctor, schemas, roles-verify) exit 2 rather than silently succeeding, which is the right posture.

CONTEXT THAT SHAPES SEVERITY: the four core crates currently hold four stub .rs files totalling 34 lines. The instruments are not vacuous — they scan a real, asserted-non-empty set and their unit tests prove they flag real violations — but they have not yet met any substantial body of code, so most gaps I list are holes that open as soon as Tasks 4/5/20 write into these crates rather than failures visible today. The two I would fix first are the libsqlite3-sys / underscore word-boundary hole (it lets infrastructure into the domain layer with every gate green, today) and the absent AK-019 instrument (there is no mechanism at all, so there is nothing that could fail).
