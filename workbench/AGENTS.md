# Gauntlet Workbench Agent Guide

Durable operating instructions for the terminal-native operator workbench in `workbench/`. Built with Bun, TypeScript, and OpenTUI (`@opentui/core`, `@opentui/react`). Follow root `AGENTS.md` for repository-wide invariants and retrieval tools.

## 1. Toolchain & Commands

This package uses Bun >= 1.4.0, TypeScript 7, and Biome. Invoke commands via the repository root `justfile`:

```sh
just tui-check                # type-check (bun run check -> tsc --noEmit)
just tui-test [selector]      # run Bun test suite with optional test filter
just tui-lint                 # Biome check + format verification (biome check .)
just tui-preflight <mode>     # scripts/tui-preflight.sh (bootstrap or conformance)
just tui-bench <selector>     # benchmark runner (bun-substrate, sqlite-replay)
```

## 2. Gate Decoupling & Boundary Invariant

* **Sibling boundary**: `workbench/` is a sibling of `crates/`, never nested inside a Cargo crate. `crates/gauntlet-arch-tests` scans core crates and treats oversized files or symlinks (such as `node_modules/`) as hard violations.
* **Gate decoupling**: The Rust closure gate (`just verify` / `just ci`) does NOT run `workbench/` tests. Any task modifying files under `workbench/` must run the TUI gates (`just tui-check`, `just tui-test`, `just tui-lint`, `just tui-preflight bootstrap`) in addition to any required Rust checks.

## 3. Canonical Boundary & Operator Surface

* **Loopback JSON surface**: Workbench communicates with the Gauntlet core via the loopback HTTP/JSON surface (`crates/adapters/gauntlet-adapter-surface-json`, enabled via `gauntlet-cli --features surfaces`).
* **Router-not-authority (`operation-gateway.ts`)**:
  - The gateway is a router, not an authority. Every drive re-validates pre-admission against a fresh canonical observation.
  - Operations bind actor, session, target, payload digest, and idempotency key. Changed-payload key reuse is refused.
  - Skip authorization only from a durable canonical record (`{outcome_unknown, accepted}`). Refused and failed operations arm nothing.
  - Every drive fails closed when the surface is unobservable.
* **Capability is inventory, not availability**: Installed capability in `operation-registry.ts` represents inventory. It is never available until resolved against the active Focus and Horizon with explicit reason codes.

## 4. State & View Invariants

* **Projection store (`projection-store.ts`)**: SQLite projection generations and cursors support cold rebuilds. Stale frames are rejected.
* **Views are read-only**: Views derive state strictly from immutable snapshots and projections. Protected mutation goes through the canonical gateway; views never apply optimistic updates.
* **OpenTUI shell**: Maintain keyboard accessibility, focus identity, and terminal geometry bounds (e.g. 120x40 / 80x24 minimum thresholds). Do not use browser DOM conventions.
