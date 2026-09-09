# T15 Evidence — Typed Plugin Composition, Trust, and Reversible Effects

Date: 2026-09-09
Task: T15 (plan `.agents/plans/gauntlet-tui-prime-workbench.plan.yaml`, block T15)
Proof Level: P3 (confirmation: independent_adversarial)
Scope: One typed slot/provider catalog covering the DESIGN.md (DSN-012) slot catalogue and contribution kinds, a plugin lifecycle manager whose registrations/subscriptions/effects all bind to plugin lifetime through deterministic disposers, explicit trust classification with a narrow serialized out-of-process capability boundary for untrusted modules, one representative multi-surface plugin, and the mount → exercise → unload → baseline reversibility harness with its teeth.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T15.prereg.yaml` — verified byte-identical this session (sha256 `7d546049…f060` via `just prereg-check gauntlet-tui-prime-workbench-T15`), never modified.

## Settled Requirements

- **REQ-PLUGIN-001** (plugins contribute through stable semantic slots/providers, never arbitrary cross-feature state mutation): `src/plugins.ts` defines the frozen DSN-012 slot union (`PLUGIN_SLOTS`: root.nav, case.view, trace.row, inspector.section, artifact.viewer, composer, bottom.panel, status.item, overlay) and the DESIGN.md contribution-kind union (`PLUGIN_CONTRIBUTION_KINDS`: view, focus_renderer, relation_provider, cue_provider, inspector_section, artifact_viewer, affordance_projection); each slot variant of the `PluginContribution` discriminated union carries its own contribution shape, and `validateContribution` slot-checks every registration at runtime (kind/slot pairing, function shapes, view targets). A plugin's ONLY host surface is `PluginContext` — register, onSnapshot, and a private state cell. No plugin ever receives the WorkbenchModel, shell state, composer state, another plugin's state, or the gateway, so cross-feature state mutation has no API. The relation-provider kind is namespace-locked (`plugin.<plugin-id>.<kind>`, re-validated at read time) so a plugin relation can never impersonate a canonical source-typed relation kind.
- **REQ-PLUGIN-002** (in-process modules MUST be explicitly trusted; untrusted code MUST execute out of process behind a narrower capability boundary): `mount()` runs the trust policy FIRST and refuses any manifest not explicitly declaring `trusted_in_process` with the typed `PluginTrustRefusalError` code `untrusted_in_process_refused` — trust is never inferred; a manifest without a trust field is refused at parse time with `trust_not_declared`. Untrusted plugins mount ONLY through `mountOutOfProcess()`, which admits NO module (nothing to activate, no live callback can enter the host); all interaction crosses `callCapability()`, which (1) checks the manifest's declared capability allowlist before contacting the transport, (2) serializes the payload to a JSON string before the boundary, and (3) validates the response shape (`{ok: boolean, payload: string}`) and parses it before any value is admitted into host state. `decideTrustPlacement()` exposes the same policy as a pure function. Boundary failures are typed: `capability_not_declared`, `boundary_transport_failed`, `malformed_boundary_payload`, `malformed_boundary_response`.
- **REQ-PLUGIN-003** (registrations, subscriptions, view contributions, and local effects MUST unwind on unload): every registration and subscription returns a `PluginDisposable` and joins the plugin's disposer ledger; `unload()` (and a failed `mount()`) runs disposers in exact reverse registration order. View extensions unwind through the SAME typed `registerShellView` seam by RECOMPOSING the view's registry entry from the anchored base renderer plus the remaining live extension stack (Correction Round 1 BLOCKING-1 — correct under any disposal order); snapshot subscriptions are fanout entries disposed by a deterministic delete (Correction Round 1 MINOR-1); plugin state cells die with the record. `mount()` is ATOMIC: a module that throws halfway is fully unwound before the typed `PluginActivationError` (or the original typed refusal) escapes, and `manager.unwindTrace` records the reverse-order unwinding as a deterministic instrument.
- **REQ-PLUGIN-004** (unload MUST be tested by mount → exercise → unload → baseline comparison): `fingerprint()` renders the whole plugin-visible surface (mounted ids, contribution {id, plugin, slot, kind} records, subscription records, per-plugin state keys) in canonical sorted order, and `serializePluginSurfaceFingerprint()` produces a byte-stable JSON document, so `plugin-lifecycle.test.ts` asserts byte-equality of the serialized fingerprint plus JSON-equality of the full WorkbenchModel snapshot plus shell-registry renderer identity, before mount vs. after unload — with functional probes (all 9 slot reads empty, no plugin line in the composed Work view, a post-unload event reaching no plugin listener) proving the counters are not lying.
- **REQ-SEC-004** (runtime-loaded modules are trusted in-process only under the trust policy; code outside it uses a separate process and narrower interface): same mechanism as REQ-PLUGIN-002 — the trust gate precedes module activation, and the out-of-process boundary's narrow surface is enforced at the seam (serialized requests only, allowlist-checked, validated responses).

**DSN-012** (plugin views/renderers/cues/relations/inspector sections/artifact viewers register and unregister cleanly; unload returns the host projection to baseline with no leaked subscription or UI state): covered by the reversibility suite (see Teeth) over every slot and kind below.

## Slot catalog coverage vs. DSN-012

| DSN-012 slot | Contribution kind | Shape (one per slot) | Host read |
| --- | --- | --- | --- |
| `root.nav` | view | `PluginNavEntryContribution` — label, target ViewId, optional activation handler | `navEntries()`, `activateNavEntry()` |
| `case.view` | view | `PluginViewExtensionContribution` — extension appended to an existing ViewId renderer THROUGH shell.ts `registerShellView` (DSN-015 guard applies to plugin views exactly like host views) | composed via the host's `ShellViewRegistryCell`; `viewExtensions()` |
| `trace.row` | focus_renderer | `PluginTraceRowContribution` — one TraceRecord → RenderedLine or null | `traceRows(record, snapshot)` |
| `inspector.section` | inspector_section | `PluginInspectorSectionContribution` — ({snapshot, focus}) → InspectorSection or null (reuses inspector.ts's section type) | `inspectorSections(snapshot, focus)` |
| `artifact.viewer` | artifact_viewer | `PluginArtifactViewerContribution` — per-artifact-kind viewer | `artifactViews(kind, artifact)` |
| `composer` | cue_provider | `PluginComposerCueContribution` — PRESENTATION-ONLY cue; composer state (mode/draft/takeovers) is host-owned and unreachable from the context | `composerCues(snapshot)` |
| `bottom.panel` | view | `PluginPanelContribution` — panel lines/records provider | `bottomPanels(snapshot)` |
| `status.item` | cue_provider | `PluginStatusCueContribution` — status cue provider | `statusCues(snapshot)` |
| `overlay` | view | `PluginOverlayContribution` — overlay content provider | `overlayContent(snapshot)` |
| (non-surface provider) | relation_provider | `PluginRelationProviderContribution` — ProjectionWorld → namespaced plugin edges | `pluginRelations(world)` |
| (non-surface provider) | affordance_projection | `PluginAffordanceProjectionContribution` — CANONICAL operation ID + presentation hint + optional estimate | `affordanceProjections()`, `projectionDescriptor()` |

All nine DESIGN.md slots and all seven contribution kinds are covered; both non-surface provider kinds are first-class catalog members.

## Implementation Details

1. **`workbench/src/plugins.ts`** (NEW): the slot/provider catalog, typed error hierarchy (`PluginError` base with `code`/`plugin` locus; manifest, trust-refusal, lifecycle, contribution, activation, and boundary subclasses), manifest parsing/validation (`parsePluginManifest` — untrusted documents are validated before admission; missing trust is `trust_not_declared`, never defaulted), `decideTrustPlacement`, the `PluginContext` typed seam, and `PluginCompositionManager` (mount / mountOutOfProcess / unload / slot reads / fingerprint / callCapability).
2. **`workbench/src/testing/sample-plugin.ts`** (NEW): the representative plugin `sample.provenance-lens` — 12 contributions spanning ALL nine slots plus both provider kinds (two affordance projections over `unit.resolve` and `unit.retry` with presentation estimates) plus 1 snapshot subscription (13 registration steps), each step observable on a deterministic probe. `createSamplePlugin({ failAfterRegistrations })` injects a halfway failure after exactly N steps (0 = before anything registers) for the Attack-1 sweep.
3. **`workbench/src/plugin-lifecycle.test.ts`** (NEW): 13 tests, 487 expect() calls — baseline reversibility, while-mounted slot coverage, the Attack-1 sweep, the deny-matrix re-run, double-mount/unload idempotence, remount cycles, and cross-plugin isolation.
4. **`workbench/src/plugin-trust.test.ts`** (NEW): 11 tests, 58 expect() calls — explicit trust declaration, the Attack-2 refusal, and the confined out-of-process boundary.
5. No existing file was modified. In particular `shell.ts` is untouched: the manager calls its public `registerShellView` guard and `ROADMAP_VIEW_IDS`/`VIEW_IDS` vocabulary as the typed seam, and `composer.ts` is untouched: the composer slot is cue-provider presentation only.

## Teeth Falsification & Verification

### Attack 1 (plan falsifier; REQ-PLUGIN-003/004): plugin throws halfway through load after registering one handler/subscription
- **Attack description**: `createSamplePlugin({ failAfterRegistrations: N })` for EVERY N in 0..13 — failure injection at every registration step (prereg transfer test L2), including before the first registration (0), after one handler (1), and after the subscription (13).
- **Defense outcome**: `mount` throws the typed `PluginActivationError` carrying the injected cause; exactly N steps had completed; the manager reports nothing mounted; the serialized surface fingerprint, the full model snapshot JSON, and the shell-registry renderer identity are byte-identical to the pre-mount baseline; `unwindTrace` equals the first N disposer labels in EXACT reverse order; a live event applied after the failed mount reaches zero plugin listeners; `unload` of the never-mounted plugin stays idempotent (`{unloaded: false, reason: "not_mounted"}`).
- **Test**: `src/plugin-lifecycle.test.ts` — "every injection point 0..N unwinds exactly its partial effects…", "the typed activation error preserves the failure locus…".
- **Result**: **PASS**.

### Attack 2 (plan falsifier; REQ-PLUGIN-002, REQ-SEC-004): untrusted module marked for in-process loading
- **Attack description**: mount an `untrusted_out_of_process` manifest together with a live module whose activation sets a probe flag.
- **Defense outcome**: typed `PluginTrustRefusalError` (`untrusted_in_process_refused`) BEFORE activation — the probe proves the module NEVER ran; the plugin is not mounted; the fingerprint is unchanged; `unload` reports `not_mounted`. The same manifest's pure policy decision is `route_out_of_process` with its capability list. The manifest parser independently refuses trust-less documents (`trust_not_declared`) and non-member trust classes (`invalid_trust_class`), and the boundary refuses trusted manifests in the other direction (`trust_boundary_mismatch`) — trust placement is explicit both ways.
- **Tests**: `src/plugin-trust.test.ts` — Attack-2 suite and the explicit-declaration suite.
- **Result**: **PASS**.

### Deny-matrix re-run over the plugin affordance-projection route (T09 trigger-records 1/2 re-check)
- **Allowed op**: `unit.resolve` via the sample plugin's projection, resolved through `projectionDescriptor` (the SAME pure T09 `resolveAffordance`: `requires_attention` with `required_attention_item: TKT-001`), then invoked through `OperationGateway` over the in-process canonical-surface route with the projection's presentation estimate: accepted, `committed_now: true`, one settlement, one ticket resolved, one admission attempt. Re-driving the SAME deterministic request re-enters the durable settlement with `committed_now: false` and NO second commit — T09 Correction Round 2 trigger-record 1's anchored-shape §16.3 re-entry behavior re-verified over the plugin route.
- **Denied ops**: `unit.resolve` on an authority-revoked unit and `unit.retry` on a readiness-not-earned unit, both through the plugin's projections: the projection resolves `unavailable` with the canonical reason first, the gateway refuses pre-admission with the typed `stale_affordance_denied` preserving the canonical class (`authority_not_permitted` / `readiness_not_earned`), and BOTH outcomes deep-equal the outcomes of identical requests driven through reference gateways with NO plugin anywhere — the same typed canonical denial. Zero side effects: `settlements_committed / tickets_resolved / admission_attempts` all 0 on every denied surface.
- **No bypass**: affordance projections must carry a registry operation (`plugin_operation_not_canonical` refusal otherwise — a plugin-defined operation cannot exist); the manager has no `invoke`/`perform` surface (asserted); after unload the projections are gone and `projectionDescriptor` refuses.
- **Tests**: `src/plugin-lifecycle.test.ts` — the deny-matrix describe block.
- **Result**: **PASS**.

### Reversibility, idempotence, and isolation (DSN-012)
- **Baseline**: mount → exercise (subscription initial emit, nav activation, every slot read) → unload → serialized fingerprint, model snapshot JSON, and shell-registry renderer identity all byte-identical to baseline; all nine slot reads empty; post-unload events reach no plugin listener while the instrument demonstrably detects the same event in the snapshot.
- **Idempotence**: double `mount` → typed `plugin_already_mounted`, second module never activated, surface unchanged; second `unload` → `{unloaded: false, reason: "not_mounted"}`; two full mount→exercise→unload cycles each land on the baseline with zero cross-cycle accumulation (prereg transfer test L1).
- **Isolation**: cross-plugin contribution-id collisions refuse the latecomer and leave the owner intact; unloading one plugin leaves another's contributions serving; post-activation registration/subscription from a stashed context is refused (`activation_closed`); plugin relation kinds must stay inside `plugin.<id>.*`; roadmap view targets are refused at the plugin seam (`roadmap_surface_refused`, DSN-015 defense in depth) and the failed mount unwinds the contributions made before the refusal.
- **Result**: **PASS**.

### Mutation spot-check (falsifiability of the harness itself)
- TEMP-RED (reverted before evidence): removing the sample plugin's failAfter injection made the Attack-1 loop fail ("Received function did not throw" for N=0), confirming the sweep detects a non-injecting plugin; removing the reverse-order unwind would fail the `unwindTrace` equality. The baseline suite detects orphan residue through three independent instruments (fingerprint, snapshot JSON, functional listener probe).

## Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T15` | PASS (exit 0) | Hash matches `7d546049ce8efd99a90be1d0616c0f61c0282fb876c74d4b82cddd91acd1f060`; prereg untouched |
| `just tui-test plugin-lifecycle` | PASS (exit 0) | 13 pass / 0 fail, 487 expect() calls |
| `just tui-test plugin-trust` | PASS (exit 0) | 11 pass / 0 fail, 58 expect() calls |
| `just tui-check` | PASS (exit 0) | `tsc --noEmit --strict` clean at final run |
| `just tui-lint` | PASS (exit 0) | 0 errors; 17 warnings / 308 infos — the pre-existing baseline classes (bench-file `noConsole` warnings, `useLiteralKeys` infos) in other files; T15's four files carry none of them |
| `just tui-preflight bootstrap` | PASS (exit 0) | Skeleton, pinned deps, bun.lock in sync, bun audit clean, spec/DESIGN digests match |
| `cd workbench && bun test` | PASS (exit 0) | **322 pass / 0 fail (1 gated skip)** across 38 files, 3730 expect() calls at the final run — 187 pre-wave baseline + T15's 24 + the parallel builders' suites that landed during this task |

**Parallel-wave transient note (honest record):** this task ran inside the declared parallel wave (T11/T13/T14/T16 builders editing workbench/ concurrently). During the run, tree-wide `tui-check`/`tui-lint`/`bun test` transiently failed inside OTHER builders' files (`domainforge-adapter.test.ts`, `semantic-identity.test.ts`, `config.ts`, `workbench-model.ts`, `bun-substrate.bench.ts`, `config-preflight.test.ts`); each was retried after ~30–90 s per the wave protocol and cleared as those builders landed corrections. T15's own four files passed `tsc --noEmit --strict` and `biome check` file-scoped at every intermediate step while the tree-wide gates settled. All final-run gates above exited 0. No test was disabled, weakened, or skipped to reach green.

## Needed index exports (not applied — `src/index.ts` is outside this task's owned files)

- `export * from "./plugins";` — the slot catalog, lifecycle manager, trust policy, boundary types, and error classes are host-facing API. (`testing/sample-plugin.ts` follows the existing convention: test-support files are not barrel-exported.)

## Known Limitations (recorded honestly)

1. **The out-of-process boundary is a typed seam, not a spawned process.** `mountOutOfProcess` + `callCapability` + `OutOfProcessTransport` enforce the narrow surface (declared capability allowlist, serialized-string requests, validated responses, typed failures) and a real child-process adapter can implement the transport, but no actual second process is spawned or killed in these tests. Process isolation itself (REQ-SEC-004's "separate process") is claimed at the seam only.
2. **The deny-matrix re-run drives the in-process canonical-surface stub**, not the Rust binary — consistent with T09's declared scope; the plugin route adds no new authority surface to re-prove against the real admission path.
3. **Contribution ordering inside a slot is registration order** (insertion order), not a host-controlled priority; hosts needing deterministic interleaving with native surfaces get it via the composition functions, not a priority field.
4. **`callCapability` has no timeout/cancellation budget** — a transport that never settles stalls only its own caller (per-call await), never the host loop; bounded-concurrency budgets for untrusted transports are future hardening (same shape as T09's trigger-record 3).
5. **Plugin state cells are in-memory per mount**; durable plugin preferences (if ever needed) would be noncanonical host state under REQ-FOCUS-005-style rules, claimed nowhere here.
6. **The manager never hands plugins the gateway** — the structural no-bypass guarantee is type-level (no API) plus the registry-only projection rule; a malicious TRUSTED plugin could still call host modules it imports (that is what "trusted in-process" means under REQ-PLUGIN-002), which is why untrusted code has the boundary.

## What the adversarial verifier should attack hardest

1. **Unwind completeness under adversarial disposers**: register a module whose disposer THROWS, or that re-registers from inside a disposer, or that calls `unload` re-entrantly from inside its own activation; verify the ledger stays consistent, the unwind completes, and no half-disposed state survives.
2. **The shell-registry seam under interleaving**: dispose view extensions OUT of reverse order (plugin-disposer self-disposal is allowed by design) and verify the composed registry still returns to the exact baseline renderer identity for every ordering; verify two plugins decorating the same ViewId unwind to each other's content, not past it.
3. **Trust-policy bypass attempts**: manifests built to make `trust` truthy-but-wrong (`"trusted"`, numeric, nested), `capabilities` inherited from a prototype, a trusted plugin attempting `callCapability` via a second mount with the same id, and boundary responses carrying prototype-polluting JSON (`__proto__` keys) through `JSON.parse` admission.
4. **Fingerprint honesty**: contributions whose `render`/`cue` functions mutate host or manager state when the fingerprint's slot reads iterate them (the fingerprint is computed from ledgers, not from rendering — verify the two cannot diverge), and subscription counts vs. real listener delivery after exotic dispose orders.
5. **Deny-matrix divergence**: drive the plugin projection route with payloads whose digests collide with a prior key, with `resolution` estimates omitted vs. present, and with unanchored (run-less) re-drives after settlement — the anchored/unanchored refusal-shape asymmetry is DISCLOSED (T09 trigger-record 1); check the plugin route never mints a second authority for it.


## Correction Round 1 (adversarial verification findings)

Fresh independent adversarial verification of round 0 returned **NOT_CONFIRM** with 1 BLOCKING and 3 MINOR findings; its probe battery is `/tmp/t15-verify/probe.test.ts` (31 probes). Every fix below landed RED-first against the round-0 code, in the existing design language, with no frozen-document edits, no commits, and no files touched outside this task's owned set. The round's regression tests are the "T15 Correction Round 1" suites in `src/plugin-lifecycle.test.ts` and `src/plugin-trust.test.ts`; all six new tests failed RED before their fixes (verified: 4 fail in plugin-lifecycle, 2 fail in plugin-trust, every round-0 test green) and pass after.

### BLOCKING-1 — the case.view disposer restored its own previousRenderer, dropping/resurrecting other plugins' extensions (plugins.ts:1246-1269)

- **Finding (executed probes P3a/P3b/P3c)**: each view-extension disposer blindly wrote its OWN captured previous renderer back into the shell registry cell. With two plugins extending the same view (the DSN-012 composition case), unloading the FIRST plugin (1) silently dropped the SECOND plugin's live wrapper while `viewExtensions()` still claimed it served (ledger/live divergence) and (2) unloading the second then restored the FIRST plugin's wrapper — with zero plugins mounted, rendering still executed an unmounted plugin's closure. Baseline not restored; reachable through public `unload(pluginId)` with well-behaved plugins; same class via one plugin self-disposing its first same-view extension. This falsified the evidence's "correct under ANY disposal order" claim.
- **Fix** (`src/plugins.ts`): the manager now keeps a per-view STACK of live case.view contributions. The first extension for a view anchors the cell's current entry as the BASE renderer; registration pushes onto the stack and the registry entry is RECOMPOSED as base + live extensions in stacking order (applied through `registerShellView`, so the DSN-015 guard still guards every write). Disposal drops the contribution from the stack and recomposes — an emptied stack restores the exact base renderer identity, a non-empty stack rebuilds exactly the remaining composition. No previousRenderer pointers exist anymore; nothing can be dropped or resurrected under ANY disposal order. The composition reads the contribution ledger, so the new record is admitted into the ledger before composition runs (an interim defect found by the round's own regression tests: composing before ledger insertion silently skipped the freshly registered extension).
- **New tests**: "cross-plugin non-LIFO unload…" (render-identity assertions with per-extension invocation counters: after unloading A, B still serves and A's closure never executes; after unloading B, `registry["work"]` IS the baseline renderer object) and "one plugin self-disposes its FIRST same-view extension out of order…" (the P3c shape). Both failed RED pre-fix (EXT:b missing from the live render while the ledger claimed it served; a stale wrapper reachable after both unloads).

### MINOR-1 — a throwing host unsubscribe orphaned the ledger record and could leave a live listener reported as success (plugins.ts:1292-1301)

- **Finding (executed probes P2a/P2b)**: the subscription disposer called the host unsubscribe BEFORE the ledger delete; a throw skipped the delete (orphan fingerprint record, P2a) and unload still reported `{unloaded: true}` even when the listener remained attached (P2b).
- **Fix** (`src/plugins.ts`, structural): plugin subscriptions no longer depend on the host's unsubscribe closure for their disposal. Plugin listeners are FANOUT entries; the manager owns exactly ONE host subscription (a stable-identity dispatcher, attached while any plugin subscription is live — `source.count()` is 1 while mounted and 0 after a healthy unload, preserving P1's contract). A plugin subscription's disposer deletes its fanout entry and its ledger record deterministically — neither can throw — so a throwing host unsubscribe can never defeat a plugin unwind, keep a plugin listener alive, or orphan a ledger record. The host detach runs only at fanout-empty; if the host's unsubscribe THROWS there, the dispatcher is inert by construction (an empty fanout delivers to zero plugin listeners) and its stable identity makes the next attach safe (the host Set deduplicates it); the failure is recorded as a TYPED residual on `manager.detachResiduals` (`{kind: "snapshot_dispatcher_detach_failed", error}`) instead of being laundered into an untyped success. `PluginUnwindError` (typed, with structured `failures`) remains the unload-contract refusal for any unwind step that cannot fully dispose its effect.
- **Disclosed design consequence**: a plugin that subscribes while the shared dispatcher is already attached no longer receives the synchronous subscribe-replay the host model delivers on a fresh subscribe (it receives the next snapshot). No round-0 test, probe, or settled requirement observes the replay for late subscribers; recorded here rather than silently changed.
- **New tests**: "recoverable unsubscribe… ledger cleaned, unload reports success, residual recorded" and "stuck unsubscribe… the plugin listener is still fully disposed, the manager's inert dispatcher residual is typed and inspectable — never a silent orphan" (asserts clean unload, baseline fingerprint, exactly one typed residual, and post-unload host emits reaching zero plugin listeners). Both failed RED pre-fix.

### MINOR-2 — manifest fields were read through the prototype chain (plugins.ts:434-449, 487-517)

- **Finding (executed probe P4)**: `Object.create({id, name, version, trust: "trusted_in_process"})` parsed as a TRUSTED manifest — the `in`/bracket reads walked the prototype chain, so trust was "declared" by an object the manifest does not own.
- **Fix**: every manifest field is admitted from OWN properties only (`Object.hasOwn` guards in `manifestTrustOf`/`manifestStringOf`/`manifestCapabilitiesOf`). The trust declaration is checked FIRST (it is the security gate; a manifest with no own trust is `trust_not_declared`), and `validateManifestForMount` now passes the RAW object to the parser instead of reconstructing it via property reads — a reconstruction would have resurrected inherited fields as own properties and bypassed the gate. An own-property trust declaration still wins over an inherited decoy (pinned).
- **New tests**: "a manifest whose trust (and identity) live only on the PROTOTYPE is refused…" (RED: round-0 admitted and mounted it) and "an own-property manifest with an inherited DECOY trust still follows its own declaration" (control).

### MINOR-3 — mount(null) threw a raw TypeError (plugins.ts:1714-1724)

- **Finding (executed probe P4, classification "TypeError")**: `validateManifestForMount` dereferenced `manifest.id` before any type check.
- **Fix**: `parsePluginManifest` type-checks first and refuses non-object manifests with the typed `PluginManifestError`/`malformed_manifest`; both mount paths delegate to it.
- **New test**: "mount(null) and other non-object manifests throw PluginManifestError, never a raw TypeError" (RED: round-0 threw TypeError).

### Verifier probe battery verdict after fixes

`bun test /tmp/t15-verify/probe.test.ts`: **30 pass / 1 fail** (round 0: 26 pass / 5 fail).

The single failing probe, **P2b**, is UNSATISFIABLE BY CONSTRUCTION, and the orphan it targets is closed:

- **Proof**: the probe's own instrument (`countingSource({unsubscribeThrows: "before"})`, probe.test.ts:41-69) throws in its unsubscribe closure BEFORE `listeners.delete(listener)` (probe.test.ts:48-50) on EVERY invocation. That delete is the only statement in the entire fixture that shrinks the Set, and the guard precedes it unconditionally; `source.count()` reads `listeners.size`. The count therefore can never decrease from 1 after a subscribe — for ANY manager implementation — so the probe's pass condition (`count() === 0` after unload, probe.test.ts:388-391) is unreachably false by the fixture's own construction.
- **The orphan P2b targets is closed**: with the fanout fix, nothing PLUGIN-facing survives the unload — the plugin's listener entry is deleted deterministically (delivery and ledger both), the fingerprint returns to baseline, and a post-unload host emit reaches zero plugin listeners (asserted by the round's own stuck-unsubscribe regression). What the broken host retains is the MANAGER-owned dispatcher, which is inert (empty fanout) and recorded as a typed residual. The probe cannot distinguish that inert, typed-reported manager reference from the round-0 live plugin-listener orphan its title describes; the round-0 failure mode it was written for ("unload reported success but the listener remains attached", live plugin closure) no longer exists.
- P2a (the satisfiable sibling) is fully green: `{unloaded: true}`, `count() === 0`, baseline fingerprint.

### Correction Round 1 Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T15` | PASS (exit 0) | prereg still byte-identical (`7d546049…f060`) |
| `just tui-test plugin-lifecycle` | PASS (exit 0) | 17 pass / 0 fail (13 round-0 + 4 correction), 527 expect() calls |
| `just tui-test plugin-trust` | PASS (exit 0) | 14 pass / 0 fail (11 round-0 + 3 correction), 75 expect() calls |
| `just tui-check` | PASS (exit 0) | tsc --noEmit --strict clean |
| `just tui-lint` | PASS (exit 0) | 0 errors; 17 warnings / 308 infos — both pre-existing baseline classes (unchanged) |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged |
| `cd workbench && bun test` | PASS (exit 0) | **330 pass / 0 fail (1 gated skip)** across 38 files, 3808 expect() calls |
| verifier probe battery | **30 pass / 1 fail** | P2b unsatisfiable by construction (proof above); all other 30 probes green, including the P3a/P3b/P3c orphan repros, the P4 prototype-trust bypass, and the full deny-matrix/fingerprint families |

RED-first record: all six correction tests failed against the round-0 code before any fix landed (4 plugin-lifecycle failures on the BLOCKING-1/MINOR-1 assertions, 2 plugin-trust failures on MINOR-2/MINOR-3), with every round-0 test green; each passed after its fix. Two interim defects introduced and caught by the round's own tests during the fix (compose-before-ledger-insert skipping the fresh extension; compose-after-base-cleanup dropping the view key) were corrected within the round and are covered by the final assertions. No test was disabled, weakened, or skipped; no dependency, frozen document, status anchor, justfile recipe, or file outside this task's owned set was touched.

Confirmation: BUILDER evidence (Correction Round 1) pending independent adversarial confirmation — the BLOCKING and all three MINORS are closed RED-first; the single remaining probe failure is proven unsatisfiable by its own fixture; settlement authority remains with the orchestrator's evidence review.

