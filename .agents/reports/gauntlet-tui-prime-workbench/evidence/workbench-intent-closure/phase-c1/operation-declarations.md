# Phase C1 — declare the Prime operation contract (nothing executes)

Plan: `.agents/plans/workbench-intent-closure.md` §4 Phase C1.
Date: 2026-09-10. Prior phase: Phase B (projection integrity contract; evidence in
`evidence/workbench-intent-closure/phase-b/projection-integrity.md`). Ran in parallel with
(after) Phase B, per the plan's own ordering note.

## What changed

New file `workbench/src/planned-operations.ts`: a versioned capability handshake
(REQ-OPS-003) that distinguishes **existing** operations (today's `operation-registry.ts`
— `unit.resolve`/`unit.retry`/`state.refresh`, already bound to canonical admission) from
**planned** ones the canonical surface does not yet serve. `workbench-host.ts`'s `submit()`
now names the declared-but-unbound operation for `message`/`prime`/`kernel` composer
submissions instead of a bare "no live submission adapter is connected" (D7: still
unsupported, now with a reason).

## The six declared planned operations

REQ-OPS-003 names five categories (launch, steering, cancellation, evidence admission,
session operations); `prime.message` (Prime mailbox delivery, REQ-PRIME-005) is added as a
sixth because C2's own "Mode bindings" table names `message` as a composer-reachable bind
target alongside `prime`/`kernel`, and REQ-OPS-003 requires every *exposed* mutation
declared, not just the five named nouns.

| id | composer mode | REQ grounding |
|---|---|---|
| `prime.launch` | `prime` | REQ-PRIME-001/003 |
| `prime.message` | `message` | REQ-PRIME-005, REQ-DATA-005 |
| `prime.steer` | `kernel` | REQ-PRIME-004/006/007, REQ-OUT-004 |
| `prime.cancel` | *(none yet)* | REQ-PRIME-009 |
| `evidence.admit` | *(none yet)* | REQ-PRIME-007 |
| `prime.session` | *(none yet)* | REQ-PRIME-001/010 |

Three of the six (`prime.cancel`, `evidence.admit`, `prime.session`) are declared but have
no composer entry point yet — honestly typed `composer_mode: null`, not invented UI.

## The nine-field contract (REQ-OPS-003 verbatim, per entry)

`owner`, `actors`, `io_contract` (REQ-OPS-003's "typed inputs/results" as one item),
`preconditions`, `authority`, `containment`, `budget`, `idempotency`,
`evidence_contract` — `PLANNED_CONTRACT_FIELDS` names exactly these nine.
`contractIsComplete()` is a **runtime** check (D1 discipline applied to declarations, not
only observations): TypeScript's structural typing alone accepts `""` for any string field;
the runtime check requires every field non-blank, with a negative control proving it
actually rejects a hollow entry.

Every field's content is grounded in an existing REQ-PRIME-*/D-rule/mechanism already in the
tree (`PrimeDelegationDispatcher`, `KernelInputBroker`, `substrate-inventory.ts`,
T13's spawn-env bound) — nothing invents new policy; the contracts restate what Phase C2
must bind.

## D7 enforcement: planned never means invocable

`PLANNED_OPERATION_REGISTRY` is a catalog **separate** from `operation-registry.ts`'s
`OPERATION_REGISTRY`. No planned id is ever added to `OPERATION_IDS`/`OPERATION_REGISTRY`
(pinned to its exact three real entries by `operation-isomorphism.test.ts:107-121`, and
re-pinned locally in `operation-registry.test.ts`). Consequently `isOperationId` returns
`false` and `resolveAffordance` (`affordance.ts`) resolves every planned operation to
`unavailable` / `unsupported_operation` — proven directly in
`operation-registry.test.ts`'s "planned never means invocable" describe block, over all six
ids, not just one.

## Versioned handshake

`CAPABILITY_HANDSHAKE_VERSION = 2` (version 1 is the implicit existing-only predecessor —
today's `operation-registry.ts` surface, unversioned because nothing needed distinguishing
from it before this file). `capabilityHandshake()` returns
`{version, existing_operations, planned_operations}`; `existing_operations` is exactly
`operationIds()` — an old client reading only that field sees no change (additive
versioning), proven in `operation-registry.test.ts`.

## The RED round (preserved, D4)

First version of `planned-operations.ts` imported `type { ComposerMode } from "./composer"`
so `PlannedOperationContract.composer_mode` could reuse the presentation layer's own type.
`renderer-boundary.test.ts` failed immediately:

```
canonical state-layer module(s) now import presentation — presentation has become
canonical and must be unwound (plan T19 redesign_trigger): planned-operations.ts:
-> composer.ts (imports presentation vocabulary './render-tree' (type))
```

Root cause: T19's state/presentation BFS follows every import edge (runtime or type-only)
when propagating the presentation taint — a canonical state module referencing a
presentation-owned type is exactly the coupling REQ-TUI-005/DSN-011 forbid, even when the
import compiles to nothing at runtime. Fixed by defining a local `ComposerModeName`
literal-union type in `planned-operations.ts` that mirrors `ComposerMode`'s four values by
value instead of importing it — `workbench-host.ts` passes its real `ComposerMode` values
into `plannedOperationForComposerMode()` unchanged (structurally identical literal unions,
zero casts, `bun run check` clean). `planned-operations.ts` was then added to
`renderer-boundary.test.ts`'s `CANONICAL_STATE_MODULES` so this class of regression is
policed going forward, not just caught once by accident.

## Teeth (plan §4 Phase C1, all landed and green)

All in `workbench/src/operation-registry.test.ts` (new file) plus one in
`workbench-host.test.ts`:

1. **Nine-field completeness** — every planned operation carries all nine non-empty fields;
   a hollow-field negative control fails `contractIsComplete()`.
2. **Planned never invocable** — no planned id registers as an existing operation id;
   `OPERATION_REGISTRY`'s exact three-entry shape is re-pinned; `resolveAffordance` resolves
   every planned id to `unavailable`/`unsupported_operation`, never `available` — the tooth
   that "an operation exposed as invocable while still classed planned fails the suite."
3. **Versioned handshake** — version ≥ 2; `existing_operations` unchanged from
   `operationIds()` (old-client compatibility); `planned_operations` disjoint from
   `existing_operations`.
4. **Composer modes report why** — `prime`/`message`/`kernel` resolve to their declared
   operation; `operation` mode (existing, not planned) always resolves `null`.
5. **End-to-end host behavior** (`workbench-host.test.ts`, new `it`) — `host.submit()` for
   each of the three composer-reachable planned modes produces a notice naming the specific
   declared operation id and "declared (planned, not yet bound...)", still ending in
   "Message was not sent." (the pre-existing assertion in the first `workbench-host.test.ts`
   case, unchanged, still passes).

## Gate results

| Gate | Result |
|---|---|
| `bun test src/operation-registry.test.ts` (new) | 12 pass / 0 fail |
| `just tui-test operation-registry` | matches the new file; 12 pass / 0 fail (previously matched zero files) |
| `just tui-test cross-surface-equivalence operation-isomorphism` | 22 pass / 0 fail, unchanged |
| `just tui-test renderer-boundary` | 9 pass / 0 fail (after the RED-round fix; `planned-operations.ts` now policed) |
| `just tui-test workbench-host production-entry` | 3 files, unchanged + 1 new tooth, all green |
| `just tui-check` | clean |
| `bun run lint` (`biome check --write .` then re-run) | exit 0; only pre-existing `bun-substrate.bench.ts` `noConsole` warnings remain |
| `bun test` (full, unfiltered) | 637 pass / 0 fail / 13 skip (was 624/0/13 after Phase B; +13 new tests) |
| `just tui-test all` (49 registered suites) | 537 pass / 0 fail / 10 skip — unchanged from Phase B (C1 declares a new non-T-numbered file; it does not touch a registered normative selector, so the suite count and shape are correctly unchanged) |

## Closes / Does not close

**Closes**: REQ-OPS-003's declaration half in full — every planned mutation now names its
owner, actors, typed inputs/results, preconditions, authority, containment, budget,
idempotency, and evidence contract, and the versioned handshake distinguishes existing from
planned. D7 ("missing operations render unsupported, never a shell-command fallback") is
verified to still hold with the declarations present.

**Does not close**: any binding. Every planned operation stays exactly as unsupported as it
was before this file existed — `resolveAffordance` proves it. Phase C2 is required to bind
`prime.launch`/`prime.message`/`prime.steer`/`prime.cancel`/`evidence.admit`/`prime.session`
to a real `RealPrimeHost` substrate; this phase produces the contract artefact C2 binds
against, per the plan's "Value if C2 never lands" note (a declared roadmap with a contract).

**Value if C2 never lands**: this phase's own stated fallback — the six contracts stand as a
settled, REQ-OPS-003-satisfying declaration on their own, independent of whether C2 proceeds.
