# T00 Evidence — Baseline Inventory & Environment Qualification

Date: 2026-09-08
Task: T00
Status: Settled

## 1. Source Authority Hashes Bound

| Artifact | Path | Pinned SHA-256 | Live SHA-256 | Status |
|----------|------|----------------|--------------|--------|
| Source Spec (0.3.0) | `.agents/specs/gauntlet-tui-prime-workbench.spec.yaml` | `d66203651461fd1189a8d10b6649b09473b423d8f2f501e3508034d52cb9f00b` | `d66203651461fd1189a8d10b6649b09473b423d8f2f501e3508034d52cb9f00b` | MATCH |
| Design Contract | `DESIGN.md` | `02ce58a257ec1240840338535cab218be77e0507abe22ea836dda77cad9944e3` | `02ce58a257ec1240840338535cab218be77e0507abe22ea836dda77cad9944e3` | MATCH |
| Core Ruler | `.wayfinder/reference.yaml` / answer key | Verified via `just ruler` | Unchanged | PASS |

## 2. Target Environment Runtime & Toolchain Inventory

- **Bun**: 1.4.0 (`/home/sprime01/.local/bin/bun`). Native runtime and package manager for `workbench/`.
- **TypeScript**: 7.0.2 pinned devDependency in `workbench/package.json`.
- **Biome**: 2.5.12 pinned devDependency in `workbench/package.json`.
- **Python**: 3.14.4 (`/usr/bin/python3`).
- **Rust toolchain**: 1.98.0 (`rustc 1.98.0`, `cargo 1.98.0`, `cargo-nextest 0.9.143`).
- **Prime Agent**: 0.8.0 (`/home/sprime01/.local/bin/prime-agent`). Protocol verified as Python REPL stdio JSON-lines, not Jupyter sockets.
- **DomainForge**: 0.16.0 (`/home/sprime01/.cargo/bin/domainforge`).
- **SXR / RealityTrace**: Incompatible CLI schema contracts between checked-out DomainForge envelope and SXR Slice 0B CEP consumer. Classified as optional/unsupported mode until T14B compatibility qualification.

## 3. Stable Gate Aliases Verified

All six `tui-*` gate recipes are wired in `justfile` and verified:

```text
$ just tui-check               -> tsc --noEmit -p tsconfig.json                   exit 0
$ just tui-lint                -> biome check .                                   exit 0
$ just tui-test                -> bun test                                        exit 0 (1 pass)
$ just tui-test all            -> NOT_IMPLEMENTED (no normative suite registered) exit 2
$ just tui-preflight bootstrap -> scripts/tui-preflight.sh bootstrap              exit 0 (6/6 pass)
$ just tui-preflight conf...   -> NOT_IMPLEMENTED (no runtime substrate yet)     exit 2
$ just tui-bench <selector>    -> NOT_IMPLEMENTED (no benchmark suite)            exit 2
$ just tui-build <mode>        -> NOT_IMPLEMENTED (no release target yet)         exit 2
$ just test-surfaces           -> cargo nextest run surfaces tests (17 tests)     exit 0
```

## 4. Verified Teeth & Falsification Attacks

1. **Spec SHA Mutation Attack**: Preflight fails with explicit mismatch message.
2. **DESIGN.md SHA Mutation Attack**: Preflight fails with explicit mismatch message.
3. **Surfaces Gate Test Mutation Attack**: Deliberate failure in `cli_surface` turns `just test-surfaces` RED (exit 100); byte-identical restore restores GREEN (exit 0).
4. **TypeScript Strict Type Violation**: `any` type or invalid type assignment rejected by `tsc --noEmit` and `biome check`.
5. **Formatting Diff Attack**: Bad indentation or spacing rejected by `tui-lint`.
6. **Core Crate Nesting Scan**: Validated that `workbench/` is outside `crates/` to prevent breaking `crates/gauntlet-arch-tests` file scanners.
7. **Lockfile Integrity Attack**: Tampered `bun.lock` or unpinned version rejected by `tui-preflight bootstrap`.
