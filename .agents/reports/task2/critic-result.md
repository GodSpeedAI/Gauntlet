# Task 2 critic result

Comparison method: read-only inspection of the artifact tree (workspace manifests, all
crate sources, `crates/gauntlet-arch-tests/src/*`, `justfile`, `.env.example`,
`deny.toml`) against the Task 2 requirements and `.agents/specs/Core-spec.md`
§4.1, §8.1, §24.3, Annex E. `git diff`/`git status` used for frozen-path and
ruler-recipe checks. No file other than this one was written; no gates were executed
(read-only constraint — gate passage was not re-run by this critic).

## Verdict — residuals-found

## Residuals

1. **major — requirement 2 / R3 (dependency direction encodes "cli wires everything")**
   `crates/gauntlet-arch-tests/src/deps.rs:60` — `Layer::Cli => &["gauntlet-domain",
   "gauntlet-ports", "gauntlet-app"]`. The cli layer's allowed internal dependencies
   omit every adapter crate, so a dependency from `gauntlet-cli` on any
   `gauntlet-adapter-*` crate is reported as a violation by the implemented gate.
   Requirement 2 states "cli wires everything"; `crates/gauntlet-cli/Cargo.toml:10-14`
   itself says the CLI is "the only crate that may construct adapters and wire them"
   and plan Task 20 wires the composition root there. As implemented, the planned
   composition-root wiring would fail `just boundaries`.

2. **major — requirement 3(a) / R2 (workspace-inherited rename hides an internal edge)**
   `crates/gauntlet-arch-tests/src/deps.rs:111-139` — `internal_deps` resolves the
   dependency name only from the member manifest's own key, its own `package = "..."`
   field (line 123-125), and its own `path` (line 126-131). A rename declared in the
   root `[workspace.dependencies]` table (`Cargo.toml:42-60`, never inspected by
   `internal_deps`) is invisible: root `foo = { package = "gauntlet-app", path =
   "crates/gauntlet-app" }` plus member `foo.workspace = true` yields dep name `foo`,
   which matches no member and is not flagged — e.g. a forbidden ports→app edge
   declared this way passes the direction gate.

3. **major — requirement 3(c) / R2, R7 (literals whitelist wider than the declared
   defaults module)**
   `crates/gauntlet-arch-tests/src/walk.rs:23` — `CONFIG_DIR =
   "crates/gauntlet-app/src/config"` and `walk.rs:74-76` exempts every path with that
   prefix from the literal scan. The requirement exempts "the declared defaults
   module"; the implementation exempts the whole directory, so any future file placed
   under `src/config/` (URLs, model ids, endpoints included) is silently outside the
   scan. The directory does not exist yet (the exemption is currently vacuous), but it
   pre-authorizes a directory-wide lane.

4. **major — requirement 3(c) / R1, R2 (port numbers are not scanned as a literal
   class)**
   `crates/gauntlet-arch-tests/src/literals.rs:131-146` — a port is flagged only
   inside `host:port` endpoints (`localhost:`, `0.0.0.0:`, `127.0.0.1:`) or as an IPv4
   literal. The scanner test at `literals.rs:300` explicitly asserts
   `let port: u16 = 8080;` produces no violations. Requirement 3(c) names "port
   numbers" as a scan class; spec §8.1 says "No path, model, provider, command, port
   number, timeout, tier policy, or tool identity is a literal in domain or
   application code." A bare hardcoded port in a core crate passes `just boundaries`.

5. **minor — requirement 3(c) / R2 (absolute-path class covers only five prefixes)**
   `crates/gauntlet-arch-tests/src/literals.rs:40-55` — needles are `/home/`,
   `/Users/`, `/root/`, `c:\`, `d:\` (on the lowercased line). `/var/...`, `/etc/...`,
   `/tmp/...`, `/srv/...`, `/opt/...`, `/usr/...`, drive letters `e:`–`z:`, and
   forward-slash drive paths (`C:/tools`) in a core crate are not flagged.

6. **minor — requirement 3(b)/(c) / R2 (hyphenless model ids evade both scanners)**
   `crates/gauntlet-arch-tests/src/literals.rs:103-108` requires a `-` after the
   family prefix; `crates/gauntlet-arch-tests/src/denylist.rs:72-73` treats a
   following digit as a word-continuation byte. Model ids written `glm4`, `qwen3`,
   `llama4`, `gpt5` match neither the literal scanner's pattern nor the denylist's
   word-boundary match, in `.rs` source and in dependency keys alike.

7. **minor — requirement 3(b) / R2 (compound identifiers embed denied terms
   undetected)**
   `crates/gauntlet-arch-tests/src/denylist.rs:59-80` — ASCII word boundaries mean an
   identifier like `SqliteConn`, `sqlite_conn`, or `ReqwestClient` in a core crate's
   source does not match `sqlite`/`reqwest` (the test at `denylist.rs:219-221`
   asserts embedding passes, by design). Casing alone is handled (`RusQLite` is
   flagged); underscore/PascalCase compounding is not.

8. **minor — requirement 3(b)/(c) / R7 (scanners read only two file extensions)**
   `crates/gauntlet-arch-tests/src/walk.rs:66-68` — denylist and literal scope is
   `.rs` and `/Cargo.toml` only under the four core crates. Any other file under
   `crates/gauntlet-domain/src/...` (e.g. `.json`, `.txt`, `.ron` loaded via
   `include_str!`/`include_bytes!`) containing vendor identifiers or environment
   literals escapes both scans.

9. **minor — requirement 3(b) vs 3(c) interplay / R7 (latent: denylist covers the
   declared configuration location)**
   `crates/gauntlet-arch-tests/src/walk.rs:63-68` — the denylist exempts nothing,
   and the scope test at `walk.rs:111-113` places `src/config/defaults.rs` inside
   denylist scope. Requirement 3(b) permits provider/harness identifiers "under
   crates/adapters/** and configuration files"; when the Task 5 defaults module lands,
   a vendor-family default value (e.g. a `claude-*` or `gemini-*` default model id)
   would fail the denylist even though it is configuration. Latent contradiction
   between mechanism (b) scope and the configuration carve-out; no such file exists
   yet.

## Checked

- **Requirement 1** — workspace exists with all named crates: 4 core crates,
  `crates/gauntlet-arch-tests`, and 11 adapter crates under `crates/adapters/`
  (agent-mock/prime/cli, state-sqlite, evidence-fs, observe-×6, compare-diff); all
  listed in the root `Cargo.toml:14-31` members list; adapter families the plan
  defers (verifier Task 11, sandbox Task 14) are absent but the plan schedules them
  later and `classify` accepts any `crates/adapters/` directory.
- **Requirement 2 (other edges)** — manifests declare exactly domain→∅, ports→domain,
  app→domain+ports, adapters→∅ (upper bound domain+ports), cli→domain+ports+app;
  `deps.rs` checks dev-/build-dependencies, target-scoped tables
  (`deps.rs:83-104`), member-level `package = ...` renames and member-level `path`
  resolution (`deps.rs:119-137`); adapters→app/cli produce a dedicated violation
  message (`deps.rs:162-166`); unclassified members fail loudly (`deps.rs:146-153`,
  test at `deps.rs:449-456`); `gauntlet-arch-tests` may depend on nothing internal
  and nothing may depend on it (absent from every allowed list, including the root
  `[workspace.dependencies]` table).
- **Requirement 3 mechanisms exist** — (a) manifest-parsing direction test with
  anti-vacuity member asserts (`deps.rs:244-284`); (b) denylist scanner, 33 terms,
  case-insensitive, whole-file including comments, dependency keys plus rename values
  in all dependency sections (`denylist.rs:19-146`), repo test with anti-vacuity
  asserts (`denylist.rs:153-189`); (c) four literal classes with per-class typed
  messages and self-tests (`literals.rs`). Teeth: `sqlite`/`rusqlite`/`reqwest` as
  dependency keys and `gpt-4o`-style literals in `crates/gauntlet-domain` are caught
  by inspection of the scanners (tests `denylist.rs:236-256`, `literals.rs:259-274`).
- **Arch-tests exclusion** — the crate is outside scanned scope by not being in
  `CORE_CRATES` (`walk.rs:11-16`), not by a path special-case; the exclusion is safe
  because the deps gate forbids any member from depending on it and forbids its own
  internal deps.
- **Requirement 4** — `justfile` recipes `check`, `test`, `lint`, `deny`,
  `boundaries`, `ruler`, `ci`, `doctor`, `schemas`, `roles-verify` all present;
  `ci: check test lint deny boundaries ruler` runs all six gates; `doctor`,
  `schemas`, `roles-verify` write to stderr and `exit 2`; the ruler recipe is
  byte-identical to `git show HEAD:justfile` (verified by byte diff of the pre-diff
  head of the file; the git diff is a single appended hunk after line 12).
- **Requirement 5** — `.env.example` contains exactly the 92 Annex E variables
  (mechanical diff against spec lines 2088-2098: 0 missing, 0 extra) with all values
  empty; `deny.toml` is a real policy (`yanked = "deny"`, `unknown-registry =
  "deny"`, `unknown-git = "deny"`, closed six-entry licence allowlist; the two
  deliberate `warn` looseners are documented in-file).
- **Constraints** — no LICENSE file at the root; external dependencies confined to
  `gauntlet-arch-tests` (`toml` and its transitive closure are the only non-workspace
  packages in `Cargo.lock`); core and adapter crates declare zero external
  dependencies; `git status` shows no modifications under `.wayfinder/`,
  `.gauntlet/` (incl. `reference.lock`), `scripts/`, `.agents/plans/`; the working
  tree diffs to `AGENTS.md` and `.agents/specs/Core-spec.md` are exactly the
  pre-existing operator edits (commit-cadence rule; facet rename git→versioning) and
  were ignored per instructions.
