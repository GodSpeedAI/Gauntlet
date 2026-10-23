# Gauntlet Crates Agent Guide

Durable operating instructions for the 30 Rust workspace crates under `crates/`. This directory contains the Gauntlet domain model, port definitions, orchestration application, composition root CLI, and 25 capability adapters. Follow root `AGENTS.md` for repository-wide invariants and retrieval tools.

## 1. Architectural Layering & Dependencies

Dependency direction points inward and is enforced mechanically by `crates/gauntlet-arch-tests` (`just boundaries`):

* **`gauntlet-domain`**: Pure domain entities, value objects, invariants. Zero internal and zero external dependencies.
* **`gauntlet-ports`**: Port traits and shared contract test suites (`src/contract/`). Depends only on `gauntlet-domain` (external allowlist: `sha2`).
* **`gauntlet-app`**: Orchestration controller, execution engines, and services. Depends only on `gauntlet-domain` and `gauntlet-ports` (external allowlist: `sha2`). Never depends on adapters or CLI.
* **`crates/adapters/*`**: 25 pluggable adapter crates implementing ports. Each adapter depends only on `gauntlet-domain` and `gauntlet-ports` (never on `gauntlet-app` or `gauntlet-cli`). External allowlist: `postgres` (for `state-postgres`), `sha2` (for `evidence-fs` and observe adapters).
* **`gauntlet-cli`**: Composition root binary and thin library. Wires domain, ports, app, and adapters. Enables `surface-json` via `--features surfaces`. External allowlist: empty.
* **`gauntlet-arch-tests`**: Test-only crate owning mechanical boundary scans. Depends on no internal crate (external allowlist: `toml`).

## 2. Mechanical Boundary Enforcement (`just boundaries`)

`crates/gauntlet-arch-tests` runs as `just boundaries` (`cargo nextest run -p gauntlet-arch-tests`). A violation is a defect, not a review suggestion:

* **Discovered membership**: Package directories are discovered from the filesystem; undeclared crates fail closed.
* **Source denylist**: Provider, model, harness, database, browser, and sandbox identifiers are forbidden in core crates (`domain`, `ports`, `app`, `cli`).
* **Adapter selection is config-driven**: No adapter identifier may appear as a literal in domain or application code.
* **No hardcoded literals**: Paths, URLs, model IDs, endpoints, commands, and timeouts outside the declared defaults table fail the scan.
* **Scope integrity**: No symlinks, unreadable files, build scripts in core crates, or `#[path]`/`include!` escapes.
* **Single-writer invariant**: Answer-key surface is single-writer/no-writer by construction.

## 3. Development Loop & Commands

Use package-scoped recipes for fast feedback before workspace-level gates:

```sh
just check-package <crate>   # type-check single crate (e.g. just check-package gauntlet-app)
just test-package <crate>    # test single crate via cargo-nextest
just check                   # workspace type-check (cargo check --workspace --all-targets)
just test                    # full test suite (cargo nextest run --workspace + surfaces tests)
just test-surfaces           # run operator JSON surface tests in gauntlet-cli
just boundaries              # run gauntlet-arch-tests boundary suite
just lint                    # cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
just deny                    # cargo deny check (licence, advisory, and ban audit)
```

## 4. Adapter & Port Discipline

* Every adapter port implementation must pass the unmodified shared contract test suite under `crates/gauntlet-ports/src/contract/`. Do not write bespoke adapter tests when a shared contract suite exists.
* `AgentRunner::invoke` returns an admission handle, never a result value. Completion is established only when a schema-valid result document exists at the declared result-contract path.
* Semantic port names: name ports after capabilities (`AgentRunner`, not `ClaudeRunner`). Vendor identifiers belong strictly in adapters or configuration.

## 5. Rust Coding Constraints

* **`unsafe_code = "forbid"`**: Enforced workspace-wide in `Cargo.toml`.
* Explicit domain types, enums, and newtypes. Typed errors with failure locus.
* No boolean conversion for three-state `Outcome` (`PASS`, `FAIL`, `INCONCLUSIVE`).
* Avoid production `unwrap()` and `expect()` unless impossibility is an established invariant.

## 6. Build & Resource Discipline

* **Memory-constrained WSL**: Respect configured Cargo and nextest concurrency; never increase concurrency flags to mask slow builds. Diagnose memory pressure on OOM failures.
* **Single compile writer**: Only one agent may compile against `target/` at a time. Never run `cargo clean` while compiling or as a routine response to failures.
* **Diagnosing storage**: Use `just timings`, `just cache-stats` (`sccache`), and `du -sh target`. Preserve incremental state.
