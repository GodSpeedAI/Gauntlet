# T00 evidence — workbench package root + Biome/TypeScript toolchain wired

Date: 2026-09-08
Scope: executes the decision recorded in the plan's `verification.workbench_package_root`,
`workbench_toolchain`, and `workbench_gate_composition`. This is one slice of T00, not the
whole task — the surfaces-feature gate closure, Prime host pin, DomainForge/SXR inventory,
and the remaining T00 steps are still open.

## What exists now

```text
workbench/.gitignore        node_modules/
workbench/package.json      devDependencies pinned exact: @biomejs/biome@2.5.12, typescript@7.0.2, @types/bun@1.4.2
workbench/bun.lock          committed, text lockfile
workbench/biome.json        formatter (2-space, 100 col, matching rustfmt's default width) + linter
                             (recommended preset + noExplicitAny/noUnusedVariables/noUnusedImports
                             as errors, per AGENTS.md "no any") + import-organize assist
workbench/tsconfig.json     strict:true, noImplicitAny, noUncheckedIndexedAccess,
                             exactOptionalPropertyTypes, verbatimModuleSyntax, noEmit
workbench/src/index.ts      placeholder (real content lands with T00A/T12)
workbench/src/index.test.ts trivial bun:test so tui-test is non-vacuous
scripts/tui-preflight.sh    bootstrap mode: package-skeleton presence, exact-pin check,
                             bun.lock sync (--frozen-lockfile --dry-run), `bun audit`,
                             and spec/DESIGN.md hash-pin verification against the plan's
                             own source.spec.sha256/source.design.sha256; conformance mode
                             NOT_IMPLEMENTED (no runtime substrate exists to qualify yet)
justfile                    tui-check / tui-test / tui-lint / tui-preflight / tui-bench /
                             tui-build wired per workbench_gate_composition
```

## Commands run and results

```text
$ cd workbench && bun add --exact --dev typescript @biomejs/biome @types/bun
  installed typescript@7.0.2, @biomejs/biome@2.5.12, @types/bun@1.4.2 — exact, no range operators

$ just tui-check     -> tsc --noEmit -p tsconfig.json                    exit 0
$ just tui-lint       -> biome check .                                    exit 0
$ just tui-test        -> bun test                                         exit 0 (1 pass)
$ just tui-test all    -> NOT_IMPLEMENTED (no normative suite registered)  exit 2  [correct — not a fake pass]
$ just tui-preflight bootstrap -> scripts/tui-preflight.sh bootstrap       exit 0 (6/6 checks pass)
$ just tui-preflight conformance -> NOT_IMPLEMENTED (no runtime substrate) exit 2  [correct]
$ just tui-preflight   (no mode) -> just itself refuses: missing required argument, exit 1
$ just tui-bench <selector> -> NOT_IMPLEMENTED (no benchmark suite)        exit 2
$ just tui-build <mode>     -> NOT_IMPLEMENTED (no release target)         exit 2
$ just -n verify        -> unchanged Rust closure gate; no tui-* recipe appears in it
```

## Teeth exercised (adversarial, not just declared)

- **`any` type** in a scratch file → `biome check` fails on `lint/suspicious/noExplicitAny`.
- **Unused variable** in the same scratch file → fails on `lint/correctness/noUnusedVariables`
  (with an unsafe-fix suggestion, not auto-applied).
- **Formatting-only diff** (bad spacing) in the same scratch file → fails the formatter check,
  proving `tui-lint` enforces format, not only lint rules.
- **Real type error** (`const n: number = "not a number"`) → `tsc --noEmit` fails with `TS2322`.
- **One-byte mutation of `DESIGN.md`** → `tui-preflight bootstrap` fails with an explicit
  expected/actual hash mismatch naming the file; restoring the byte restores a PASS. This is
  the plan's own T00 teeth attack ("Change DESIGN.md by one byte and run the plan preflight"),
  now executable rather than prose.
- Scratch/mutation files were removed after verification; the committed tree is clean.

## What this does NOT prove

No OpenTUI, no React, no Prime/Bun runtime integration, no real workbench source beyond the
placeholder. `tui-test`'s "all" selector and `tui-preflight conformance` are honestly
NOT_IMPLEMENTED, not stubbed green. The rest of T00 (surfaces-feature gate closure, Prime/
OpenTUI version pins, DomainForge/SXR inventory) is unexecuted.
