# T19 Evidence — Headless Equivalence and Renderer Replaceability

Date: 2026-09-09
Task: T19
Proof Level: P3
Confirmation: independent
Scope: Headless replay/reconstruction of a recorded Run over authoritative sources; deletion-ordering invariance (renderer / kernel / store); renderer-independent minimal consumer over the Workbench Model; filesystem-derived architecture tests banning presentation imports in canonical layers.

## Settled Requirements

- **REQ-VERIFY-002**: Killing and restarting the TUI and Python REPL kernel independently MUST NOT lose Reference, authority, evidence, settlement, or other Gauntlet canonical truth.
- **REQ-STATE-004**: The same recorded Gauntlet Run MUST be reconstructible headlessly without OpenTUI, React, Bun SQLite, or an active notebook view.
- **REQ-GOAL-005**: OpenTUI MUST remain a replaceable rendering projection over React-free workbench state and MUST NOT own consequential business truth.
- **REQ-TUI-005**: Renderer replacement MUST NOT require rewriting Gauntlet canonical behavior or Prime execution semantics.
- **REQ-SAFE-001**: Loss of TUI, React, OpenTUI renderer, local SQLite, or kernel state MUST NOT alter canonical Gauntlet settlement/reference/authority truth.
- **DSN-011**: No React/OpenTUI component holds canonical truth; every semantic View could be rendered by another frontend without changing its meaning.

## Pinned interpretation: "without Bun SQLite" (REQ-STATE-004 / plan T19 "changes:")

REQ-STATE-004 forbids depending on the WORKBENCH's local SQLite read-model — or any UI over it (the "notebook UI") — as a truth source. NONGOAL-001 keeps the Bun SQLite database explicitly non-canonical. The interpretation cannot mean "without the `bun:sqlite` module": T06's `projection-store.ts` IS `bun:sqlite`, and the plan's own T19 steps point reconstruction at the `rebuildFromAuthoritativeLog` path (hot_context: "projection rebuild T06"). Accordingly, `headless-replay.ts` proves the stronger form:

1. the normalized projection is derivable from the BARE authoritative event log alone (`foldLogToNormalized` — a pure fold; zero SQLite, zero store, zero UI), AND
2. it is derivable from a cold `SqliteProjectionStore.rebuildFromAuthoritativeLog` into a fresh (possibly deleted-and-recreated) SQLite file, AND
3. both folds agree byte-for-byte in canonical JSON, and pre-existing local SQLite state is never an input to truth.

## Implementation

1. **`workbench/src/headless-replay.ts`** — the headless replay/reconstruction harness. Imports ONLY state-layer modules (`projection-store`, `horizon`, `workbench-model` types, `evidence-promotion` type, `node:crypto`); zero OpenTUI/React/presentation imports. Emits a normalized `Case/Run/Evidence/Attention/settlement` projection (`NormalizedRunProjection`, plain JSON, deterministic ordering, canonical-JSON sha256 digest). Canonical sections (`canonicalSections`) = Reference cursors, authority-bearing unit rows, evidence, settlement inputs, attention, and run/audit trace — the truth that must survive renderer/kernel/store loss. Kernel values are projected as availability only: `artifactized` objects survive via their durable locator; `pinned` and `ephemeral` values are marked UNAVAILABLE with typed reasons after kernel loss (`kernel_restarted_pinned_value_not_retained`, `ephemeral_kernel_state_not_reconstructible`) — never recreated, never faked (T05/T11, REQ-PRIME-006, REQ-SYNC-003, REQ-SAFE-004).
2. **`workbench/src/testing/minimal-projection.ts`** — the smallest renderer-independent consumer. Consumes WorkbenchModel snapshots + the canonical affordance resolver (T09) ONLY; emits plain-text lines and machine-checkable records. Zero imports of shell/views/inspector/composer/theme/render-tree/geometry/OpenTUI/React (policed by the boundary suite). No presentation vocabulary forks: raw status/reason strings pass through untouched.
3. **`workbench/src/renderer-boundary.test.ts`** — architecture tests. The state-layer file list is ENUMERATED FROM THE FILESYSTEM (no hand-maintained allowlist): presentation is classified by transitive import closure (any binding form: runtime, type-only, side-effect, dynamic) reaching the vocabulary seeds (`theme.ts`, `geometry.ts`, `render-tree.ts`) or an OpenTUI/React package; every other non-test/bench/barrel file is policed state-layer and must import no presentation module or OpenTUI/React package in any form. The 19 canonical modules of plan T19 (plus `evidence-promotion`, `kernel-affordances`, `security-hardening`, `terminal-session`, `headless-replay`) are pinned to stay state-classified; a violation fails with the full transitive import chain and the plan's redesign_trigger message. The import parser is itself tested against planted snippets (runtime/type/side-effect/dynamic/package/comment shapes) so a rotting parser fails loudly instead of passing a fake boundary.
4. **`workbench/src/headless-replay.test.ts`** — the recorded representative Run fixture (deterministic constants; evidence admission through the REAL canonical path: `EvidenceAdmissionGate` + `authoritativeEvidenceFrame`, the only writer path for evidence rows) plus both attacks below.

## Recorded representative Run (fixture)

Generation `gen-t19-recorded`, source `gauntlet`, cursors c-01..c-09, fixed recorded clock: three units (u-core running→fail with builder authority, u-edge running, u-far blocked with orchestrator authority → attention), one unverified source-log evidence row, two agent sessions (parent/child), one canonically admitted kernel-derived evidence row (verified, durable artifact locator), one authoritative focus_changed, and three recorded kernel objects (metrics: artifactized at `.agents/reports/artifacts/t19-metrics.json`; intermediate: pinned; scratch: ephemeral). Presentation ephemera (a placement with an operator label override, a selected focus) are recorded but projected NOWHERE.

## Teeth falsification & verification

### Attack 1 (prereg P1, falsifier 1): remove/disable all OpenTUI/React modules and reconstruct the recorded Run

- **Method**: the harness imports no presentation module (asserted statically); reconstruction runs through the pure log fold and a cold `rebuildFromAuthoritativeLog` with no renderer, no OpenTUI, no React, no pre-existing SQLite, and no active notebook view.
- **Outcome**: `via_log` and `via_store` are fully equal; their canonical sections equal the WITH-presentation live fold's (real `WorkbenchModel` carrying a placement, an operator label, a hoisted horizon, an operator selection, and a capability inventory — all proven present in the live snapshot, all absent from the normalized truth). Digests are reproducible run-over-run.
- **Tests**: `headless-replay.test.ts` — "headless replay equals the with-presentation fold in canonical content", "reconstruction is deterministic", "the harness itself imports no presentation module".
- **Result**: **PASS**.

### Deletion orderings (prereg transfer test L2, parametric variation)

- **(a) Renderer-state first**: all placements removed, hoist popped, operator selection moved to a different unit with no authoritative event behind it. The full snapshot's renderer state provably changed; the canonical sections are byte-identical before/after and identical to the headless fold.
- **(b) Kernel first**: headless reconstruction (kernel not alive) vs the kernel-alive fold — canonical sections digest-equal; only kernel-object availability moves, strictly in the typed direction (artifactized stays available at the same locator; pinned/ephemeral become unavailable with reasons and null locators; no unavailable record claims a value). Full-projection JSON differs; canonical digest does not.
- **(c) Store first, then rebuild**: the file-backed SQLite database (+WAL/SHM) is deleted from disk and `rebuildFromAuthoritativeLog` rebuilds at the same path — projection JSON and digest identical.
- **All orderings at once**: log-only fold equals every other ordering's canonical sections.
- **Tests**: `headless-replay.test.ts` — "deletion order (a)/(b)/(c)", "every deletion ordering leaves canonical truth identical".
- **Result**: **PASS** (REQ-VERIFY-002, REQ-SAFE-001).

### Attack 2 (prereg P2): minimal non-OpenTUI consumer derives the views' answers without a semantic fork

- **Method**: ONE `WorkbenchModel` snapshot consumed by both the minimal consumer and the OpenTUI views; representative queries compared, with the views' derivations computed independently via `renderInspector` / `workUnitOrder` / `materializeAffordances`.
  - Q1 unit status: `minimalUnitStatus` equals the inspector STATE section's status/authority fields and the snapshot rows; unknown unit → honest absence; the consumer's statuses classify units exactly as the Work view's escalation-first `workUnitOrder` orders them.
  - Q2 evidence presence: `minimalEvidenceFor` equals the inspector EVIDENCE records (identity/VERIFIED-UNVERIFIED/reference), including the admitted kernel-derived evidence with its durable locator and the honestly unverified source log; a unit with no evidence is empty on both sides.
  - Q3 affordance reasons: `minimalAffordanceReasons` equals the inspector AFFORDANCES records per operation (status, reason codes, required attention item); known canonical answers hold (u-core retry available; u-core resolve refused `no_open_ticket` under the disclosed strict policy; state.refresh `not_action_admitted`; u-far resolve `requires_attention` bound to ticket ESC-19-1).
  - Full-projection comparison: the minimal action bar equals the Work view's materialized affordances (same palette-only exclusions for the same canonical reasons), palette-only operations stay discoverable with reasons, every unit appears in the plain text with its snapshot status, blocked-unit attention is present.
- **Outcome**: identical answers from identical snapshots with zero presentation imports in the consumer — OpenTUI is replaceable without a business-semantic fork (DSN-011, REQ-TUI-005).
- **Result**: **PASS**.

### Digest teeth

- **Test**: "teeth: the digest discriminates any consequential tampering with the recording" — rewriting a WINNING authority fact moves the digest and the projected authority; flipping a canonical verified bit moves the digest and the settlement inputs (verified_evidence_ids ↔ unverified_evidence_ids); tampering a SUPERSEDED frame legitimately moves nothing (the fold's last-write-wins semantics, matching the store's INSERT OR REPLACE); rewriting presentation-only recording fields moves nothing.
- **Boundary teeth** (sandbox demonstration on a throwaway copy, repo untouched): smuggling `import { SEMANTIC_TOKENS } from "./theme"` into canonical `relations.ts` fails the boundary suite, naming `relations.ts` and the full transitive blast radius (workbench-model, affordance, horizon, live-streams, prime-*, worker-folds, ...) with the redesign_trigger message.
- **Result**: **PASS**.

### Import-ban coverage (the policed file list)

Filesystem-derived at gate time (36 candidate files): **24 policed state-layer files** — affordance, client, config, evidence-promotion, focus, headless-replay, horizon, kernel-affordances, live-streams, operation-gateway, operation-registry, preflight, prime-context-envelope, prime-host, prime-kernel, prime-long-running, prime-recursion, prime-topology, projection-store, relations, security-hardening, terminal-session, workbench-model, worker-folds (all `.ts`). **12 presentation-classified** — shell, views-work, views-chat-trace, inspector, composer, theme, geometry, render-tree, preview, plugins, sxr-adapter, domainforge-adapter (the last via its type-only render-tree data-shape coupling, a pinned policy decision). Tests, benches, the `index.ts` composition-root barrel, and `src/testing/` helpers are outside the policed universe; `testing/minimal-projection.ts` is additionally asserted presentation-free by direct source scan. A new file that imports no presentation module is policed automatically; a canonical module that ever imports presentation fails the pin with the offending chain.

## Bun-SQLite interpretation decision

Recorded above ("Pinned interpretation"). In short: local SQLite is deletable cache, never truth; the plan's "without Bun SQLite" binds the truth-dependency, not the store module; the harness proves log-only reconstruction AND cold SQLite rebuild AND their agreement.

## Gate results

T19 landed during the PARALLEL wave (T20 release/SSH qualification and a concurrent inspector-lens builder actively editing `workbench/`). Shared gates over the whole tree therefore fluctuate with their in-flight files; the attribution below is per-file and re-verified.

| Gate command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T19` | PASS | Frozen hash `55d81c3796d12c719f28accaa8f77ac76bcadd693271f64517ee9700aaca29d3` verified before work began |
| `just tui-test headless-replay` | PASS (exit 0) | 16/16 (three consecutive runs, deterministic) |
| `just tui-test renderer-boundary` | PASS (exit 0) | 9/9 |
| `just tui-check` | PASS (exit 0) at recording, re-checked | TypeScript strict. Exit 0 once the wave's first edit batch stabilized; later re-failed ONLY on T20's in-flight `src/ssh-target.test.ts` (4 TS diagnostics, all in that file). Zero diagnostics in any T19 file at every observation. |
| `just tui-preflight bootstrap` | PASS (exit 0) | all 6 preflight checks; bun.lock frozen and unmodified |
| `cd workbench && bun test` | PASS at recording: 516 pass / 1 gated skip (T13 real-PTY, env-gated) / 0 fail across 48 files | Later full-suite runs fluctuate (T20's tmux/SSH legs are environment-dependent and mid-qualification). Both T19 suites: 25/25 pass inside the full run at every observation. |
| `just tui-lint` | PASS for all T19 files: `biome check` exit 0 on each of the four T19 files individually, re-verified after final edits | Workspace-wide gate fails ONLY on parallel-wave in-flight files (`inspector.ts` and `workbench-model.ts` modified by the wave; `planner-swap.test.ts`, `inspector-lens-wiring.test.ts`, `transport-equivalence.test.ts`, `main.ts`, `ssh-target.test.ts` new from the wave). None are T19-owned; none were modified by T19. Retried five times over ~15 minutes per the wave-conflict guidance. |
| `just ruler` | PASS | Answer key and manifest digests match the frozen lock |

## Honest gaps / scope bounds

- The non-OpenTUI consumer is a minimal test projection, not a product renderer (prereg blindspot); no libvaxis or second-production-renderer claim is made (plan `does_not_prove`).
- Recorded-Run reconstruction is bounded to the representative Run recorded within this task's fixture (prereg blindspot); it does not bound search over all possible Run shapes.
- Kernel values are projected as availability + durable locators, not contents; promotion mechanics themselves are T11's proof — T19 records their durable outcomes.
- The trace fan-out equality is proven by parsing the model's own trace summaries (`[source] event_type (cursor)`) back into fields; a format change in `workbench-model.ts` would fail loudly (typed throw), not silently pass.
- The architecture test's parser is regex-based, with parser self-tests pinning every import shape used in this repository; an exotic future import shape would fail safe into the parser self-tests rather than silently pass a violation.

## What the independent confirmer should attack

1. Re-run `replayHeadless` with a fixture of your own construction (any event sequence the T06 store admits) and check `via_log` vs `via_store` vs a live `WorkbenchModel` fold — canonical sections must agree and digests must be stable across processes.
2. Try to make the normalized projection depend on renderer state: select focuses, place placements, hoist/dehoist, declare capabilities BEFORE normalizing the live fold; canonical sections must not move.
3. Attempt a reconstruction that resurrects the pinned/ephemeral kernel values (e.g., feed `kernelAlive: true` selectively) — the suite's typed unavailability reasons and null-locator invariants are the contract to attack.
4. Smuggle a presentation import (runtime OR type-only) into any of the 24 policed state files, or add a new state-layer file, and verify `renderer-boundary.test.ts` catches it with the transitive chain; then verify a presentation-free new file is policed automatically.
5. Verify the minimal consumer answers still match `renderInspector`/views-work on a snapshot this fixture did not produce (e.g., a degraded observation, an authority-revoked unit, a Case-scoped focus).

## Confirmation findings (trigger-record)

Independent verdict: **CONFIRM_HEADLESS_EQUIVALENCE** — 0 blocking, 0 major, 1 MINOR (trigger-record), 2 informational wording nits. Recorded here verbatim-fair; no code changes made under this verdict.

- **MINOR (trigger-record) — `normalizeLiveFold`/`normalizeSnapshot` accept any `WorkbenchSnapshot`**: `workbench/src/headless-replay.ts` types both entry points with the BASE `WorkbenchSnapshot`, but `FullWorkbenchSnapshot` is a structural subtype, so a caller passing the FULL model snapshot would leak the operator's visual selection into the canonical `focus` section. The doc comment's claim that operator selection/placements/horizons "cannot enter by construction" is convention, not type enforcement; all shipped call sites pass the base store snapshot (`model.store.getSnapshot(null)` / the rebuild store's `getSnapshot`). Owning scope: T19 module — the fix shape is a narrower parameter type (e.g., an explicit base-projection input type that a `FullWorkbenchSnapshot` does not satisfy, or a store-snapshot brand) to be applied at the next touch of this module; trigger-recorded here so the seam cannot silently widen.

- **Nit (informational) — `kernelRecords` reason string imprecision**: an artifactized-recorded object whose recording lacks its locator (inconsistent recording) falls through to the ephemeral/pinned branch and, when the kernel is not alive, reports `ephemeral_kernel_state_not_reconstructible` even though its recorded `persistence_class` is `artifactized`. Fail-safe direction (marked unavailable, null locator, nothing fabricated), but the reason string does not name the actual anomaly (artifactized-with-missing-locator).

- **Nit (informational) — boundary parser treats import-shaped string-literal text as imports**: `renderer-boundary.test.ts` strips comments but not string-literal contents, so import-shaped text inside a string literal parses as an import. Direction is fail-safe: false detections surface loudly (a canonical module falsely classified trips the pin test rather than passing a real violation); they never green-light an actual presentation import in a pinned canonical module.

Confirmation: BUILDER evidence pending independent confirmation.
