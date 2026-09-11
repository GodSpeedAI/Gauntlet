# Gauntlet TUI — Interaction & Command Map

> **Status:** Draft implementation-facing interaction contract

> **Purpose:** Ensure that every mouse, keyboard, menu, palette, tab, window, composer, and contextual interaction in the Gauntlet TUI either changes presentation only or converges on one existing semantic/canonical operation.

> **Applies to:** React + OpenTUI, Hallmark redesign work, command palette, menu bar, mouse input, keyboard bindings, context menus, Trace interactions (Gauntlet execution history and RealityTrace), Prime/Jupyter controls, plugin-contributed actions, and GUI-grade terminal interaction.

> **Companion artifact:** `UI_ELEMENT_STATE_CATALOG.md`

> **Upstream authority:** Gauntlet workbench specification, `DESIGN.md`, canonical Gauntlet/Prime/DomainForge behavior.

> **Important:** Backend function names in this document are semantic command identities, not permission to invent new APIs. Any consequential command marked `BIND_EXISTING` MUST be bound to the repository's actual operation/adapter after inspection.

> **Trace terminology:** "Trace" means the Trace surface: one ordered, source-typed ledger over two sources.
> 1. **Gauntlet execution history** — canonical frames folded by `WorkbenchModel` into `trace_records` (identity `trace-<source_id>-<cursor>`). Always present.
> 2. **RealityTrace** — the SXR consequence ledger (`../sxr`), read through `workbench/src/sxr-adapter.ts`. Present only when explicitly bound.
>
> The sources are never merged. A RealityTrace record is not a Gauntlet Event. A RealityTrace settlement is not a Gauntlet Settlement. RealityTrace evidence becomes Gauntlet evidence only through `evidence.offer` and canonical admission. Spec basis: REQ-SXR-001..004, REQ-DATA-005, REQ-TUI-003. Detail: §16.0 here and `UI_ELEMENT_STATE_CATALOG.md` §16.0.

---

# 0. Core law
Every user interaction must terminate in exactly one of these outcomes:

```text
1. PRESENTATION_LOCAL
   Changes only disposable UI/workbench state.

2. NAVIGATION_LOCAL
   Changes SemanticLocation/Horizon/View/selection without mutating domain truth.

3. QUERY_ONLY
   Reads or materializes a projection without requesting consequential mutation.

4. RUNTIME_ADAPTER
   Requests a runtime-owned control through the owning structured adapter
   (Prime/Jupyter/filesystem/etc.), subject to Gauntlet governance where applicable.

5. CANONICAL_OPERATION
   Requests one consequential Gauntlet operation identity that owns
   validation, authority, idempotency, execution admission, and result semantics.

```
A React/OpenTUI component MUST NOT perform a sixth category of side effect.

The central implementation rule is:

```text
gesture
  ↓
interaction intent
  ↓
semantic command identity
  ↓
current affordance / policy check
  ↓
revalidation
  ↓
owning operation or local presentation reducer
  ↓
authoritative event/projection update
  ↓
Workbench snapshot
  ↓
renderer

```
Never:

```text
Button.onClick()
  ↓
component-specific business mutation

```

---

# 1. Specification crosswalk
This artifact operationalizes the following existing workbench requirements.

| Requirement | Interaction implication |
| --- | --- |
| REQ-AFF-001 | Every consequential visible action references one canonical Gauntlet operation identity. |
| REQ-AFF-002 | TUI, palette, keyboard, CLI/API, kernel, and eligible-agent projections converge on the same semantics. |
| REQ-AFF-003 | Available controls are revalidated at invocation; stale UI cannot bypass gates. |
| REQ-AFF-004 | Installed tools/providers are not automatically exposed as usable affordances. |
| REQ-AFF-005 | Unavailable affordances may remain inspectable with reasons. |
| REQ-FOCUS-001 | Selecting/opening a Focus preserves one stable identity across all Views. |
| REQ-FOCUS-002 | Multiple placements invoke actions against the same FocusRef. |
| REQ-FOCUS-003 | Hoist/de-hoist changes Horizon and may change queries/subscriptions/context/affordances. |
| REQ-FOCUS-004 | Interactions prefer horizon-local incremental materialization. |
| REQ-FOCUS-005 | Saved Horizons remain projections, never independent truth. |
| REQ-PRIME-003 | Spawn admission is not child completion. UI must display these separately. |
| REQ-PRIME-005 | Mailbox/message actions remain communication, not completion. |
| REQ-PRIME-006 | Kernel input/control returns through Prime/Jupyter mediation. |
| REQ-PRIME-009 | Closing TUI/presentation never implicitly stops admitted Prime work. |
| REQ-PRIME-010 | Reattachment uses Prime/Gauntlet truth, not stale UI. |
| REQ-DATA-002..004 | Reconnect interactions use generations/cursors and reject stale frames. |
| REQ-DATA-005 | Chat and Trace actions operate on distinct projections. |
| REQ-DATA-006 | One View cannot consume state such that another View misses it. |
| REQ-SXR-001 | When RealityTrace is bound, its committed evidence, ledger history/verification, and question/threshold context appear in Trace, Evidence, and Inspector with ledger ordinal, entry hash, and committed/pending status. Binding to Gauntlet IDs is explicit, never by label. |
| REQ-SXR-002 | RealityTrace interactions never mutate Gauntlet Settlement, Residual, or EvidenceRef. RealityTrace evidence reaches Gauntlet only through `evidence.offer` + canonical admission. |
| REQ-SXR-003 | RealityTrace refresh/verify are bounded `RUNTIME_ADAPTER` commands with side effects classified before invocation. Never run during render. Never append to the RealityTrace ledger or start its autonomous run. |
| REQ-SXR-004 | Every Trace interaction works in Gauntlet-only, RealityTrace-only, and combined binding modes. An absent source is shown with its reason. |
| REQ-TUI-001 | OpenTUI/React receives snapshots + callbacks, not business-state ownership. |
| REQ-TUI-002 | Interactions preserve Environment → Workspace → Surface → View → Focus semantics. |
| REQ-TUI-003 | Work, Chat, and Trace remain distinct interaction questions. |
| REQ-TUI-004 | Composer takeover is temporary and restores default draft state. |
| REQ-TUI-005 | Renderer replacement must not change command semantics. |

---

# 2. Command classes
Use these classes in code, tests, plugin APIs, and command registries.

## 2.1 `PRESENTATION_LOCAL`
Changes only local Workbench/OpenTUI state.

Examples:

- open/close tab;
- resize pane;
- open/close floating window;
- pin Inspector;
- toggle Bottom Surface;
- dismiss tooltip;
- change local density;
- focus a Surface.
It MUST NOT:

- mutate a Case;
- stop a Run;
- stop an Agent;
- admit Evidence;
- change authority;
- create Settlement.

---

## 2.2 `NAVIGATION_LOCAL`
Changes current perception/location.

Examples:

- select Focus;
- open Focus;
- back/forward;
- switch View;
- hoist/de-hoist;
- navigate relation;
- open related Event;
- return to current Case root.
A Horizon change may legitimately cause:

- new subscriptions;
- different SQLite query;
- different prefetch;
- different Prime context projection;
- different visible Affordances.
That consequence remains representational, not domain mutation.

---

## 2.3 `QUERY_ONLY`
Requests data/projection computation with no consequential mutation.

Examples:

- search current Case;
- load older Trace page;
- inspect Diff;
- retrieve Evidence details;
- query relations;
- compute current affordance reasons;
- load Domain concept neighborhood.
Queries MAY produce cached/read-model state. That state remains reconstructible.

---

## 2.4 `RUNTIME_ADAPTER`
Controls a runtime whose lifecycle is owned outside React/OpenTUI.

Examples:

- Prime session reattach;
- Jupyter interrupt/restart/input response;
- terminal PTY resize/input;
- open file externally;
- artifact preview load;
- RealityTrace ledger refresh / ledger verify / evidence query (`executeSxrRefresh`, side effects pre-classified in `SXR_SIDE_EFFECT_CLASSES`).
A runtime control that is consequential under Gauntlet governance MUST still pass through the appropriate Gauntlet admission boundary before reaching the adapter.

---

## 2.5 `CANONICAL_OPERATION`
Requests consequential system behavior.

Examples:

- create Case;
- bind resource;
- create Thread if persisted canonically;
- submit user intent to Orchestrator;
- freeze Reference;
- start/admit Run;
- cancel/stop admitted Run or Agent when supported;
- run Verification;
- answer authority/approval question;
- admit Evidence;
- settle WorkItem/Run when such an operation exists.
Every such action:

1. references one semantic operation identity;
2. revalidates current state at invocation;
3. routes through owning operation/admission semantics;
4. emits authoritative events/results from the owner;
5. is reflected back through projection snapshots.

---

# 3. Binding status vocabulary
Because this artifact is written before final repository binding, use these statuses.

| Binding status | Meaning |
| --- | --- |
| LOCAL_DEFINED | Workbench presentation/navigation behavior may be implemented directly. |
| BIND_EXISTING | Semantic operation is required, but exact existing backend command/function/API MUST be discovered. |
| CONDITIONAL_BIND_EXISTING | Expose only if the repository already supports/admits the operation or the normative implementation plan explicitly adds it. |
| ADAPTER_BIND_EXISTING | Bind to actual Prime/Jupyter/filesystem/etc. adapter; do not invent protocol. |
| DERIVED_QUERY | Implement against read-model/projection/query layer only. |
| PLUGIN_PROVIDED | Plugin contributes the action through the same registry/affordance contract. |

---

# 4. Semantic command envelope
Every command/action registered in the workbench SHOULD normalize to this logical shape:

```yaml
id: "semantic.command.id"

class:
  one_of:
    - PRESENTATION_LOCAL
    - NAVIGATION_LOCAL
    - QUERY_ONLY
    - RUNTIME_ADAPTER
    - CANONICAL_OPERATION

binding: "LOCAL_DEFINED | BIND_EXISTING | CONDITIONAL_BIND_EXISTING | ADAPTER_BIND_EXISTING | DERIVED_QUERY | PLUGIN_PROVIDED"

target:
  focus_ref: "optional"
  workspace_ref: "optional"
  semantic_location: "optional"

invocation_sources:
  - mouse
  - keyboard
  - command_palette
  - menu
  - context_menu
  - composer
  - cli_api_if_applicable
  - kernel_if_applicable
  - eligible_agent_if_applicable

availability:
  source: "presentation | derived query | AffordanceResolver"
  unavailable_behavior: "hide | disable_with_reason | inspectable_reason"

revalidation:
  required: true_or_false
  owner: "Workbench | Gauntlet | Prime/Jupyter adapter | other"

parameters:
  - "typed parameter"

expected_effect:
  presentation: "optional"
  canonical: "optional"

expected_result:
  kind: "local state | authoritative result | async admission handle | query result"

failure_behavior:
  - "how stale state/denial/error is represented"

telemetry_and_trace:
  - "what user intent and resulting authoritative events are linkable"

forbidden:
  - "component-local business mutation"

```

---

# 5. Interaction dispatch pipeline

## 5.1 Local presentation path

```text
mouse/key gesture
   ↓
InteractionIntent
   ↓
local command registry
   ↓
Workbench reducer
   ↓
new immutable snapshot
   ↓
OpenTUI render

```
Examples:

- `ui.tab.close`
- `ui.inspector.pin`
- `ui.bottom.resize`

---

## 5.2 Consequential operation path

```text
mouse/key/palette/composer gesture
   ↓
InteractionIntent
   ↓
semantic operation identity
   ↓
AffordanceResolver / current admission path
   ↓
REVALIDATE
   ├→ denied → authoritative/derived reason → snapshot → UI
   ↓
admitted
   ↓
canonical Gauntlet operation
   ↓
execution/runtime adapters as required
   ↓
authoritative event(s)
   ↓
projection reducer
   ↓
immutable Workbench snapshot
   ↓
all affected Views update

```
The originating component must not optimistically fabricate domain success.

---

# 6. Universal GUI interactions
These apply consistently wherever the target supports them.

## 6.1 Select Focus
**Semantic ID:** `nav.focus.select`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

| Projection | Behavior |
| --- | --- |
| Mouse | single click |
| Keyboard | focus traversal/arrows + selection |
| Palette | Go to / Select <Focus> where relevant |
| Result | Workbench selected Focus changes; Inspector follows unless pinned |

**Does not:**

- open the Focus;
- mutate canonical object;
- allocate authority;
- imply primary Attention if the design separates selection from attention.

---

## 6.2 Open Focus
**Semantic ID:** `nav.focus.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

| Projection | Behavior |
| --- | --- |
| Mouse | double-click or explicit open control |
| Keyboard | Enter |
| Context menu | Open |
| Palette | Open selected Focus |

**Effect:** Promote Focus into an appropriate Primary View/tab/window while preserving identity.

**Does not:** create/duplicate the underlying object.

---

## 6.3 Inspect Focus
**Semantic ID:** `nav.focus.inspect`

**Class:** `NAVIGATION_LOCAL` / `QUERY_ONLY` if details must load

**Binding:** `LOCAL_DEFINED` + `DERIVED_QUERY`

**Mouse:** single-click normally makes Inspector follow.

**Keyboard:** inspect shortcut / move selection.

**Effect:** Inspector shows local details/relations.

---

## 6.4 Open context menu
**Semantic ID:** `ui.context_menu.open`

**Class:** `PRESENTATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

**Mouse:** right-click

**Keyboard:** context-menu shortcut

**Effect:** materialize:

1. presentation actions;
2. navigation actions;
3. current semantic Affordances returned by the resolver.
Consequential menu items remain canonical operations.

---

## 6.5 Close presentation
**Semantic ID:** `ui.presentation.close`

**Class:** `PRESENTATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Applies to:

- tab;
- floating window;
- dialog where cancellation is legitimate;
- panel;
- preview.
**Hard rule:** closing presentation never means delete/stop/cancel underlying object.

---

## 6.6 Back / Forward
**IDs:** `nav.history.back`, `nav.history.forward`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Restores:

- Case;
- Horizon;
- View;
- Focus;
- optional layout state.
Does not revert domain time.

---

## 6.7 Hoist / De-hoist
**IDs:** `nav.horizon.hoist`, `nav.horizon.dehoist`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Hoist may recompute:

- subscriptions;
- read-model query;
- prefetch;
- semantic graph neighborhood;
- visible Affordances;
- context for future Prime actions.
**Mouse:** breadcrumb / explicit focus action

**Keyboard:** shortcut/palette

**Effect:** update active Horizon.

---

# 7. Application chrome commands

## 7.1 File menu
Recommended semantic contents, only when supported:

| Command ID | Class | Binding | Notes |
| --- | --- | --- | --- |
| case.create | CANONICAL_OPERATION | BIND_EXISTING | Create new Case. |
| case.open | NAVIGATION_LOCAL / QUERY_ONLY | LOCAL_DEFINED + DERIVED_QUERY | Enter existing Case. |
| resource.bind | CANONICAL_OPERATION | BIND_EXISTING | Bind folder/repo/resource to Case. |
| ui.presentation.close_current | PRESENTATION_LOCAL | LOCAL_DEFINED | Close current tab/window only. |
| app.quit | PRESENTATION_LOCAL | LOCAL_DEFINED | Close TUI process; MUST NOT stop admitted Prime/Run work. |

Do not add `Save` unless there is an actual meaningful operation; canonical state should not depend on fake document-save metaphors.

---

## 7.2 View menu
| Command ID | Class | Binding |
| --- | --- | --- |
| view.activate.work | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.chat | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.trace | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.evidence | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.agents | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.files | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.reference | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.domain | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.kernel | NAVIGATION_LOCAL | LOCAL_DEFINED |
| view.activate.harness | NAVIGATION_LOCAL | LOCAL_DEFINED |
| ui.navigator.toggle | PRESENTATION_LOCAL | LOCAL_DEFINED |
| ui.inspector.toggle | PRESENTATION_LOCAL | LOCAL_DEFINED |
| ui.bottom.toggle | PRESENTATION_LOCAL | LOCAL_DEFINED |
| ui.density.review | PRESENTATION_LOCAL | LOCAL_DEFINED |
| ui.density.operational | PRESENTATION_LOCAL | LOCAL_DEFINED |

---

## 7.3 Run menu
Menu items MUST come from actual state/affordances.

| Semantic command | Class | Binding |
| --- | --- | --- |
| run.form_or_start | CANONICAL_OPERATION | BIND_EXISTING |
| run.inspect_reference | NAVIGATION_LOCAL | LOCAL_DEFINED |
| run.request_cancel | CANONICAL_OPERATION | CONDITIONAL_BIND_EXISTING |
| run.resume_or_reattach | CANONICAL_OPERATION / RUNTIME_ADAPTER | CONDITIONAL_BIND_EXISTING |
| verification.run_selected | CANONICAL_OPERATION | BIND_EXISTING where existing |
| run.open_trace | NAVIGATION_LOCAL | LOCAL_DEFINED |

Do not display unsupported lifecycle commands merely because desktop apps usually have them.

---

## 7.4 Agents menu
| Semantic command | Class | Binding |
| --- | --- | --- |
| agents.open_view | NAVIGATION_LOCAL | LOCAL_DEFINED |
| agent.inspect_selected | NAVIGATION_LOCAL | LOCAL_DEFINED |
| agent.open_trace | NAVIGATION_LOCAL | LOCAL_DEFINED |
| agent.request_stop | CANONICAL_OPERATION | CONDITIONAL_BIND_EXISTING |
| agent.message | CANONICAL_OPERATION / RUNTIME_ADAPTER | CONDITIONAL_BIND_EXISTING |
| agent.spawn | CANONICAL_OPERATION | AFFORDANCE-DRIVEN / CONDITIONAL_BIND_EXISTING |

**Important:** `agent.spawn` is not a generic always-visible button. Specialist delegation should normally emerge from Orchestrator/work needs and only materialize when useful/payable/governable.

---

## 7.5 Help menu
All presentation/query only.

| Command | Class |
| --- | --- |
| help.open | NAVIGATION_LOCAL |
| help.shortcuts | PRESENTATION_LOCAL |
| help.command_reference | PRESENTATION_LOCAL |
| help.about | PRESENTATION_LOCAL |
| help.diagnostics | QUERY_ONLY |

---

# 8. Home / Case interactions

## 8.1 Create Case
**Semantic ID:** `case.create`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

### Sources
- `+ New Case` button;
- File → New Case;
- command palette;
- keyboard shortcut if desired.

### Form steps
1. define name/purpose;
2. optionally select project/resource binding;
3. optionally identify existing purposeful artifacts;
4. optionally bind SEA world;
5. validate;
6. invoke canonical Case creation.

### Invocation behavior

```text
form valid
 ↓
user invokes Create
 ↓
revalidate inputs/resources
 ↓
canonical create operation
 ├→ denied/error → form remains with exact reason
 ↓
Case created
 ↓
navigate to CaseWorkspace

```
**Never:** create a local React-only Case placeholder that masquerades as canonical success.

---

## 8.2 Open Case
**ID:** `case.open`

**Class:** `NAVIGATION_LOCAL` + `QUERY_ONLY`

**Binding:** `LOCAL_DEFINED` + `DERIVED_QUERY`

Single click = select.

Double-click/Enter = open.

Opening hydrates the Case projection; it does not mutate the Case.

---

## 8.3 Bind project/resource
**ID:** `resource.bind`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Potential sources:

- Case creation;
- Case Overview;
- context menu;
- command palette.
After binding, resource state must derive from canonical config/filesystem projection.

---

# 9. Thread / Chat interactions

## 9.1 New Thread
**Semantic ID:** `thread.create`

**Class:** `CANONICAL_OPERATION` if Thread persistence is canonical; otherwise bind to actual owning substrate

**Binding:** `BIND_EXISTING`

### Sources
- `+ New Thread`;
- thread list context;
- palette;
- keyboard shortcut.

### Result
- create/persist Thread using actual Thread/Prime/Gauntlet mechanism;
- open ChatView;
- bind visible main conversational participant to the Orchestrator.
**Hard rule:** creating a Thread does not create a Run.

---

## 9.2 Open Thread
**ID:** `thread.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Restores Chat projection and relevant Case context.

---

## 9.3 Archive Thread
**ID:** `thread.archive`

**Class:** `CANONICAL_OPERATION` if archival is supported

**Binding:** `CONDITIONAL_BIND_EXISTING`

Closing a Thread tab is not archival.

---

## 9.4 Submit user message
**Semantic ID:** `thread.message.submit`

**Class:** `CANONICAL_OPERATION` / owning communication path

**Binding:** `BIND_EXISTING`

### Sources
- Composer submit button;
- keyboard submit shortcut;
- approved speech/input mechanism if added later.

### Path

```text
draft
 ↓ submit
user message intent
 ↓ owning conversation/Prime/Gauntlet path
Orchestrator receives
 ↓
message/execution events
 ↓
Chat + Trace projections update independently

```
**Hard rules:**

- message delivery ≠ work completion;
- Chat text is not the canonical Run state;
- Orchestrator may answer, shape purpose, perform bounded work, delegate, or propose Run formation.

---

## 9.5 Discuss instead of decide
**ID:** `attention.discuss`

**Class:** `NAVIGATION_LOCAL` plus message drafting

**Binding:** `LOCAL_DEFINED`

When an approval/question card offers `Discuss`, it should restore Chat/composer context with the unresolved AttentionItem linked, not silently answer it.

---

# 10. Composer interactions

## 10.1 Type/Edit draft
**ID:** `composer.draft.edit`

**Class:** `PRESENTATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Draft is local until submitted unless project explicitly persists drafts.

---

## 10.2 Takeover by question/approval/recovery/kernel input
**ID:** `composer.takeover.enter`

**Class:** `PRESENTATION_LOCAL` triggered by authoritative Attention condition

**Binding:** `LOCAL_DEFINED`

Must preserve prior draft.

---

## 10.3 Answer canonical question/approval
**Semantic family:** `attention.respond.*`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Examples:

- approve;
- reject;
- choose option;
- provide requested input.
Exact operation identity comes from the owning AttentionItem, not from hardcoded modal buttons.

---

## 10.4 Kernel stdin response
**ID:** `kernel.stdin.respond`

**Class:** `RUNTIME_ADAPTER`, potentially Gauntlet-governed

**Binding:** `ADAPTER_BIND_EXISTING`

Must return through Prime/Jupyter mediation.

---

# 11. Wayfinder interactions

## 11.1 Begin/continue Wayfinder
**ID:** `wayfinder.continue`

**Class:** `CANONICAL_OPERATION` / conversation-backed artifact formation

**Binding:** `BIND_EXISTING`

The UI may expose:

- unresolved question;
- current represented answer;
- why the distinction matters;
- prior answer/version.

---

## 11.2 Answer Wayfinder question
**ID:** `wayfinder.answer`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

The visible control may be:

- free text;
- explicit options;
- discuss in Chat.
Answer creates/updates candidate representation through the existing mechanism.

---

## 11.3 Review Wayfinder candidate
**ID:** `wayfinder.review`

**Class:** `NAVIGATION_LOCAL` / `QUERY_ONLY`

**Binding:** `LOCAL_DEFINED`

---

## 11.4 Freeze Wayfinder / purpose artifact
**ID:** `wayfinder.freeze`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Must revalidate unresolved required distinctions.

Frozen artifact becomes immutable revision.

---

# 12. Reference interactions

## 12.1 Open Reference
**ID:** `reference.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

---

## 12.2 Add purposeful artifact to candidate Reference
**ID:** `reference.candidate.include_artifact`

**Class:** `CANONICAL_OPERATION` or canonical reference-builder operation

**Binding:** `BIND_EXISTING`

Examples:

- `.wayfinder`;
- spec;
- plan;
- DESIGN;
- `.sea`;
- research method;
- acceptance criteria.
Do not treat file presence alone as Reference inclusion.

---

## 12.3 Remove artifact from candidate Reference
**ID:** `reference.candidate.exclude_artifact`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Only candidate Reference may be changed.

---

## 12.4 Validate Reference candidate
**ID:** `reference.validate`

**Class:** `CANONICAL_OPERATION` / query according to current implementation

**Binding:** `BIND_EXISTING`

Returns explicit missing distinctions/reasons.

---

## 12.5 Freeze Reference
**ID:** `reference.freeze`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

### Path

```text
candidate
 ↓ invoke Freeze
revalidate
 ├→ deficient → remain candidate + reasons
 ↓
freeze
 ↓
immutable Reference identity/hash
 ↓
RunAdmission recomputed

```

---

## 12.6 Request Reference revision
**ID:** `reference.revision.begin`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Never edit a frozen Reference in place.

---

# 13. Run interactions

## 13.1 Start / Admit Run
**Semantic ID:** `run.start`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

### Availability
Source = current Run admission affordance.

States:

- unavailable with reason;
- available;
- stale UI available but denied at revalidation;
- admitted.

### Sources
- visible `Start Run`;
- Run menu;
- command palette;
- keyboard shortcut;
- eligible agent/kernel/CLI/API projection if supported.
All sources converge on same operation identity.

### Result
The immediate result may be an admission/run identity, not final completion.

---

## 13.2 Inspect why Run unavailable
**ID:** `run.admission.inspect`

**Class:** `QUERY_ONLY` / navigation

**Binding:** `DERIVED_QUERY`

May reveal:

- missing frozen Reference;
- missing authority;
- payment constraint;
- missing settlement access;
- unresolved recovery gate;
- other exact gate reason.

---

## 13.3 Open Run
**ID:** `run.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

May hoist Horizon to Run.

---

## 13.4 Open Run Trace
**ID:** `run.trace.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

---

## 13.5 Cancel/Stop Run
**ID:** `run.request_cancel`

**Class:** `CANONICAL_OPERATION`

**Binding:** `CONDITIONAL_BIND_EXISTING`

Only expose if actual Gauntlet operation/policy supports it.

Closing TUI never invokes this implicitly.

---

## 13.6 Resume/Reattach Run
**ID:** `run.reattach`

**Class:** `CANONICAL_OPERATION` + `RUNTIME_ADAPTER` as applicable

**Binding:** `CONDITIONAL_BIND_EXISTING`

Must reconstruct from Gauntlet/Prime truth.

---

# 14. WorkItem interactions

## 14.1 Select/Open/Hoist WorkItem
IDs:

- `nav.focus.select`
- `nav.focus.open`
- `nav.horizon.hoist`
All local navigation only.

---

## 14.2 Inspect current WorkItem affordances
**ID:** `work.affordances.inspect`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY`

May show:

```text
Inspect Diff           available
Run Verification       available
Settle                 unavailable — verification absent

```
Do not imply every example operation always exists.

---

## 14.3 Invoke WorkItem action
**ID:** `work.affordance.invoke` with canonical operation identity

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

The visual control does not own action semantics.

---

## 14.4 Run Verification
**ID:** `verification.run`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Possible sources:

- Work View button;
- context menu;
- command palette;
- keyboard shortcut;
- CLI/API/kernel/eligible-agent projection if supported.
Must converge on one operation.

---

# 15. Agent interactions

## 15.1 Inspect Agent
**ID:** `agent.inspect`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Inspector may show:

- purpose;
- role;
- parent;
- WorkItem;
- context scope;
- Prime session ID;
- model;
- status;
- cost/payment;
- latest activity;
- produced artifacts/evidence.

---

## 15.2 Open Agent trajectory
**ID:** `agent.trace.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Normal user experience is inspection of activity/trajectory, not an agent chatroom.

---

## 15.3 Spawn Specialist Agent
**ID:** `agent.spawn`

**Class:** `CANONICAL_OPERATION`

**Binding:** `CONDITIONAL_BIND_EXISTING`

The Orchestrator normally requests delegation.

If exposed to the operator, it MUST materialize only as an actual current Affordance.

Prime spawn/admission handle is displayed as admission, not result.

---

## 15.4 Request Agent stop/cancel
**ID:** `agent.request_stop`

**Class:** `CANONICAL_OPERATION`

**Binding:** `CONDITIONAL_BIND_EXISTING`

Closing Agent view/tab is never this command.

---

## 15.5 Message Agent
**ID:** `agent.message`

**Class:** `RUNTIME_ADAPTER` / canonical communication path

**Binding:** `CONDITIONAL_BIND_EXISTING`

This is advanced/secondary. Default UX should preserve the Orchestrator as the user-facing conversational boundary.

Mailbox delivery remains communication only.

---

# 16. Trace interactions

## 16.0 Trace sources, identity, and order
The Trace surface is one ordered ledger over two source-typed streams.

| Source | Truth owner | Row identity | Authoritative order | Presence |
| --- | --- | --- | --- | --- |
| Gauntlet execution history | Gauntlet canonical event log, folded by `WorkbenchModel` into `trace_records` | `trace-<source_id>-<cursor>` + projection generation | source cursor | always |
| RealityTrace ledger | RealityTrace (SXR), read via `sxr-adapter.ts` (`parseSxrLedgerExport` → `projectSxrEvidenceLens`) | `ledger_id` + `ordinal` + `entry_hash` | ordinal within `ledger_id` | only when explicitly bound |

Rules:

1. Every row carries its source. A renderer never styles one source as the other.
2. Per-source order is authoritative. Cross-source interleaving is a derived presentation order and is marked derived. Interleave by time only when both rows carry a recorded timestamp. The RealityTrace ledger export omits `committed_at`, so RealityTrace rows render in ordinal order in their own segment.
3. Cross-source adjacency comes only from explicit bindings (`admitSxrUnitBindings`), never from label matches.
4. Pending RealityTrace state (`spooled`, `queued_for_commit`) renders apart from committed records and is never offerable.
5. Unbound, absent (`no_ledger_export`), invalid (`export_invalid`), or incompatible RealityTrace renders as an inspectable source status, not as an empty ledger. Gauntlet-only Trace stays fully operable.
6. RealityTrace records arrive only through bounded refresh (§16.11). Rendering never invokes the adapter.
7. No Trace interaction mutates Gauntlet Settlement, Residual, or EvidenceRef, or appends to the RealityTrace ledger.

---

## 16.1 Select Trace row
**ID:** `trace.event.select`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Inspector follows the selected row unless pinned: a Gauntlet Event, or a RealityTrace record (kind, ordinal, entry hash, durability, payload fields, unknowns, export omissions, bound Gauntlet identities).

---

## 16.2 Open Trace row
**ID:** `trace.event.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Shows richer details without mutating either source. For a RealityTrace question, threshold, evidence, or settlement record it opens the verification path (§16.13).

---

## 16.3 Expand nested Trace step
**ID:** `trace.step.expand`

**Class:** `PRESENTATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Turn/step grouping is derived from Gauntlet execution events and marked derived. RealityTrace records are never folded into a Gauntlet turn.

---

## 16.4 Scroll away from live tail
**ID:** `trace.follow.suspend`

**Class:** `PRESENTATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Triggered automatically by meaningful user scroll upward.

Effect:

- state → `INSPECTION_HOLD`;
- do not yank viewport;
- unseen row count increments for arrivals from either source (Gauntlet frames, or rows added by a RealityTrace refresh), counted per source.

---

## 16.5 Return to live
**ID:** `trace.follow.resume`

**Class:** `PRESENTATION_LOCAL` / navigation

**Binding:** `LOCAL_DEFINED`

Sources:

- click `↓ N new`;
- End/live shortcut;
- palette command.

---

## 16.6 Load older history
**ID:** `trace.history.load_older`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY`

Preserve current selection/scroll anchor. Applies per source: Gauntlet history pages from the projection; RealityTrace history is the last refreshed export, and newer or older ledger rows come only from a new `ledger_export` refresh.

---

## 16.7 Search Trace
**ID:** `trace.search`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY`

Search result navigation is `NAVIGATION_LOCAL`. Results keep their source.

---

## 16.8 Filter Trace
**ID:** `trace.filter.set`

**Class:** `PRESENTATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Possible filters:

- source (Gauntlet / RealityTrace);
- actor;
- type / RealityTrace record kind;
- WorkItem (Gauntlet Unit, or RealityTrace records explicitly bound to it);
- agent;
- evidence-producing;
- authority-changing;
- errors;
- verification;
- RealityTrace durability (committed / pending).
Filters are projections, not event deletion.

---

## 16.9 Open related Focus
**ID:** `trace.related.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

Examples:

- produced Evidence;
- parent WorkItem;
- Agent;
- File;
- authority record;
- Gauntlet Unit/Run explicitly bound to a RealityTrace record, and the reverse.

---

## 16.10 Replay from Event / Fork from Event
**IDs:** `trace.replay.from`, `trace.fork.from`

**Class:** `CANONICAL_OPERATION`

**Binding:** `CONDITIONAL_BIND_EXISTING`

Expose only when Gauntlet/Prime actually supports the corresponding governed operation. Not applicable to RealityTrace records: the workbench never launches a RealityTrace run and never uses its placeholder explain command.

Never fake replay/fork by changing UI location.

---

## 16.11 Refresh RealityTrace
**ID:** `trace.reality.refresh`

**Class:** `RUNTIME_ADAPTER`

**Binding:** `ADAPTER_BIND_EXISTING` → `executeSxrRefresh` with `{ kind: "ledger_export" }` over `SxrProcessRunner`

Materializes only when RealityTrace is explicitly bound. Side effects are declared before invocation (`SXR_SIDE_EFFECT_CLASSES.ledger_export`: may initialize local storage; never appends; never launches a run). Bounded by deadline and output cap; runs off the render loop. Outcome (`refreshed`, `producer_refused`, `deadline_exceeded`, `output_limit_exceeded`, `spawn_failed`) and parse status (`parsed`, `invalid`, `incompatible`, `unknown`) render as source status. A refresh result that completes after a newer one is discarded.

---

## 16.12 Verify RealityTrace ledger
**ID:** `trace.reality.verify`

**Class:** `RUNTIME_ADAPTER`

**Binding:** `ADAPTER_BIND_EXISTING` → `executeSxrRefresh` with `{ kind: "ledger_verify" }`

The export alone checks ordinal density (`ordinal_density_ok`); hash-chain linkage is checked producer-side by `ledger_verify`. Show which check ran. A verified RealityTrace ledger is not a Gauntlet Verification.

---

## 16.13 Inspect RealityTrace verification path
**ID:** `trace.reality.path.inspect`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY` → `sxrVerificationPath(lens, declarationRef, bindings)`

Shows declaration → unit binding → question → threshold → evidence → RealityTrace settlement, each with ordinal + entry hash, plus `chain_complete` and explicit `missing` links. No invented causal links.

---

## 16.14 Offer RealityTrace evidence to Gauntlet
**ID:** `evidence.offer` (source: RealityTrace)

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING` → `offerSxrEvidenceAsGauntletEvidence`, then the canonical admission decision applied through `SxrEvidenceAdmissionGate.admit`

Materializes only on committed RealityTrace evidence records bound to a Gauntlet Unit. Refusals render with their reason: `pending_evidence`, `evidence_not_in_qualified_ledger`, `repository_mismatch`, `settlement_is_sxr_only`. An offer is not admission; admission is not verification or settlement.

---

# 17. Evidence interactions

## 17.1 Inspect Evidence
**ID:** `evidence.inspect`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

---

## 17.2 Open Evidence artifact
**ID:** `evidence.artifact.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Uses terminal image/render capability where available, with fallback.

---

## 17.3 Compare Evidence
**ID:** `evidence.compare`

**Class:** `QUERY_ONLY` / navigation

**Binding:** `DERIVED_QUERY`

Does not change admission/verification.

---

## 17.4 Offer candidate Evidence for admission
**ID:** `evidence.offer`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Only if existing Gauntlet mechanism recognizes candidate→admission flow.

One command identity for every source; the source only selects the offer constructor:

- RealityTrace evidence record → §16.14 (`offerSxrEvidenceAsGauntletEvidence` + `SxrEvidenceAdmissionGate`);
- kernel artifact → §23.6.

---

## 17.5 Admit / Reject / Quarantine Evidence
**Semantic IDs:** owning evidence decision operations

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Never implement as card-local status edits.

---

# 18. Verification interactions

## 18.1 Run Verification
`verification.run` — covered above.

## 18.2 Inspect Verification
**ID:** `verification.inspect`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

---

## 18.3 Re-run / new verification round
**ID:** `verification.retry`

**Class:** `CANONICAL_OPERATION`

**Binding:** `CONDITIONAL_BIND_EXISTING`

Must preserve prior failed/inconclusive round rather than mutate it away.

---

# 19. Settlement interactions

## 19.1 Inspect Settlement readiness
**ID:** `settlement.readiness.inspect`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY`

---

## 19.2 Request Settlement evaluation
**ID:** `settlement.evaluate`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Only expose when the governing Gauntlet operation exists/currently materializes.

---

## 19.3 Settlement decision actions
If human authority is part of settlement:

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Visible options must come from the governing Attention/Affordance, not generic `Accept/Reject` buttons invented by the component.

---

# 20. Capability interactions

## 20.1 Inspect Capability evidence
**ID:** `capability.inspect`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

Shows:

- attempts;
- settlements;
- variation;
- transfer;
- recovery;
- orchestration burden evidence.

---

## 20.2 Quarantine/degrade capability
If supported, these are `CANONICAL_OPERATION` and `BIND_EXISTING`.

Do not allow a renderer to set capability state directly.

---

# 21. Attention interactions

## 21.1 Open AttentionItem
**ID:** `attention.open`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

May:

- select underlying Focus;
- open relevant View;
- trigger composer takeover.

---

## 21.2 Acknowledge
**ID:** `attention.acknowledge`

**Class:** `CANONICAL_OPERATION` if acknowledgement is canonical; otherwise local cue acknowledgement

**Binding:** inspect existing implementation before choosing.

Hard rule:

- acknowledge ≠ resolve underlying issue.

---

## 21.3 Resolve
No generic UI-local `resolve`.

Resolution occurs through the actual operation that resolves the underlying condition.

---

# 22. File / Diff interactions

## 22.1 Open File
**ID:** `file.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

---

## 22.2 Open Diff
**ID:** `diff.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `LOCAL_DEFINED`

---

## 22.3 Edit File
**ID:** `file.edit` or actual canonical operation identity

**Class:** `CANONICAL_OPERATION` / runtime tool action

**Binding:** `BIND_EXISTING`

The TUI should not introduce an independent editor mutation path unless the architecture explicitly supports it through the same governance/action substrate.

---

## 22.4 Open externally
**ID:** `artifact.open_external`

**Class:** `RUNTIME_ADAPTER`

**Binding:** `ADAPTER_BIND_EXISTING`

Presentation convenience only unless governance requires admission.

---

# 23. Kernel/Jupyter interactions

## 23.1 Run computation
**ID:** `kernel.execute`

**Class:** `RUNTIME_ADAPTER`, potentially canonical-operation-wrapped depending on use

**Binding:** `ADAPTER_BIND_EXISTING`

Hard rule:

- no concurrent shell execution against one single-threaded kernel;
- deterministic bounded computation preferred where suitable.

---

## 23.2 Interrupt
**ID:** `kernel.interrupt`

**Class:** `RUNTIME_ADAPTER`

**Binding:** `ADAPTER_BIND_EXISTING`

Return through Prime-owned Jupyter control mediation.

---

## 23.3 Restart
**ID:** `kernel.restart`

**Class:** `RUNTIME_ADAPTER` / governed destructive action

**Binding:** `ADAPTER_BIND_EXISTING`

Should warn about ephemeral-state loss and require appropriate authority/confirmation.

---

## 23.4 Pin Kernel object
**ID:** `kernel.object.pin`

**Class:** `RUNTIME_ADAPTER` / owning kernel state action

**Binding:** `ADAPTER_BIND_EXISTING`

---

## 23.5 Artifactize Kernel object
**ID:** `kernel.object.artifactize`

**Class:** `RUNTIME_ADAPTER` / canonical artifact operation

**Binding:** `BIND_EXISTING`

---

## 23.6 Offer Kernel artifact as Evidence
**ID:** `evidence.offer`

**Class:** `CANONICAL_OPERATION`

**Binding:** `BIND_EXISTING`

Explicit provenance-preserving transition.

---

# 24. DomainForge interactions
Only materialize for SEA-bound Cases.

## 24.1 Open semantic concept
**ID:** `domain.concept.open`

**Class:** `NAVIGATION_LOCAL` / query

**Binding:** `DERIVED_QUERY`

---

## 24.2 Traverse semantic relation
**ID:** `domain.relation.traverse`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

---

## 24.3 Hoist semantic neighborhood
**ID:** `domain.horizon.hoist`

**Class:** `NAVIGATION_LOCAL`

**Binding:** `LOCAL_DEFINED`

May narrow graph traversal/context but never rewrite semantic model.

---

## 24.4 Modify semantic model
Any authoring operation is consequential and MUST bind to actual DomainForge/Gauntlet operation semantics. Never implement as local graph editing.

---

# 25. Harness interactions

## 25.1 Inspect Skill / HarnessRevision
Navigation/query only.

## 25.2 Propose refinement
**ID:** `harness.refinement.propose`

**Class:** `CANONICAL_OPERATION` / Prime refinement adapter

**Binding:** `BIND_EXISTING`

## 25.3 Evaluate refinement
Must create/consume evidence through the governed evaluation path.

## 25.4 Retain/Rollback refinement
Consequential; `BIND_EXISTING`.

Prime refinement output alone cannot self-certify improvement.

---

# 26. Plugin interactions

## 26.1 Load plugin
**ID:** `plugin.load`

**Class:** `RUNTIME_ADAPTER` / governed local capability

**Binding:** `ADAPTER_BIND_EXISTING`

## 26.2 Unload plugin
**ID:** `plugin.unload`

**Class:** `RUNTIME_ADAPTER`

**Binding:** `ADAPTER_BIND_EXISTING`

Must unwind registrations/subscriptions/effects.

## 26.3 Plugin-contributed action
Every plugin action MUST declare:

- semantic command identity;
- command class;
- truth owner;
- Affordance source;
- teardown behavior.
A plugin cannot bypass the main command registry for consequential behavior.

---

# 27. GUI presentation commands
These are safe local commands and should remain visually rich.

## 27.1 Tabs
| ID | Meaning |
| --- | --- |
| ui.tab.open | Open View/Focus placement. |
| ui.tab.activate | Activate existing tab. |
| ui.tab.close | Close placement only. |
| ui.tab.close_others | Close other placements only. |
| ui.tab.reorder | Presentation ordering only. |

---

## 27.2 Windows
| ID | Meaning |
| --- | --- |
| ui.window.open | Open floating presentation. |
| ui.window.close | Close presentation only. |
| ui.window.focus | Raise/focus. |
| ui.window.move | Drag location. |
| ui.window.resize | Change presentation dimensions. |

No window action mutates underlying Focus.

---

## 27.3 Panes
| ID | Meaning |
| --- | --- |
| ui.pane.resize | Resize pane. |
| ui.pane.collapse | Collapse. |
| ui.pane.expand | Expand. |
| ui.pane.maximize | Temporary presentation maximization. |
| ui.pane.restore | Restore prior layout. |

---

## 27.4 Inspector
| ID | Meaning |
| --- | --- |
| ui.inspector.follow | Follow current selection. |
| ui.inspector.pin | Keep current Focus despite later selection. |
| ui.inspector.unpin | Resume follow-selection. |
| ui.inspector.toggle | Show/hide. |

---

## 27.5 Navigator
| ID | Meaning |
| --- | --- |
| ui.tree.expand | Expand placement. |
| ui.tree.collapse | Collapse placement. |
| nav.focus.select | Select semantic Focus. |
| nav.focus.open | Open. |
| nav.horizon.hoist | Narrow Horizon. |

---

# 28. Command Palette contract
The palette is a projection over registries, not a separate command system.

Search domains may include:

- open Views;
- current Foci;
- recent locations;
- presentation commands;
- navigation commands;
- current Affordances;
- help/settings.
Each result MUST expose:

- title;
- semantic command ID;
- target;
- current availability;
- shortcut if any;
- unavailable reason where appropriate.
Consequential palette result:

```text
Palette row
  ↓ invoke
same semantic operation ID as button/context menu/shortcut
  ↓
revalidate

```
Never duplicate implementation.

---

# 29. Context-menu contract
For a Focus:

```text
contextMenu(FocusRef)
  =
presentation_actions(FocusRef)
+ navigation_actions(FocusRef)
+ AffordanceResolver(FocusRef)

```
Example for WorkItem T23:

```text
Open
Inspect
Hoist

────────

Inspect Diff
Run Verification
Ask Critic

────────

Copy ID

```
`Run Verification` and `Ask Critic` appear only if truly materialized.

Unavailable affordances may appear disabled with a reason if that teaches the operator what is missing.

---

# 30. ActionControl state contract
Controls render one of:

```text
HIDDEN
VISIBLE_DISABLED(reason)
VISIBLE_ENABLED
BUSY_SUBMISSION

```
`BUSY_SUBMISSION` means the UI is awaiting admission/result appropriate to that operation; it MUST NOT claim domain success.

Invocation:

```text
VISIBLE_ENABLED
 ↓ user activates
BUSY_SUBMISSION
 ↓ revalidate
 ├→ denied → VISIBLE_DISABLED/ENABLED + exact reason
 ↓ admitted
await authoritative projection update

```
Avoid permanent optimistic domain state.

---

# 31. Destructive-action interaction contract
Applies to operations such as:

- cancel active Run;
- stop Agent;
- restart kernel;
- discard candidate artifact;
- unbind resource;
- other irreversible/high-cost operations.
Pattern:

```text
invoke destructive Affordance
 ↓
revalidate
 ↓
if confirmation required
ModalDialog
 ↓ user confirms
canonical operation
 ↓
authoritative result

```
Dialog copy MUST identify:

- target;
- consequence;
- recovery/irreversibility;
- what is not affected.
Example principle:

```text
"Close window"          presentation-local, no confirmation.
"Stop builder-03"       canonical operation, may require confirmation.
"Cancel Run R31"        canonical operation, consequence explicit.

```

---

# 32. Denial and stale-affordance behavior
A stale visual projection is expected in distributed/live systems.

When invocation is denied after appearing available:

1. do not treat it as UI corruption;
2. do not retry through another path;
3. surface the authoritative/derived reason;
4. refresh relevant affordance projection;
5. preserve operator context.
Example:

```text
[Run Verification]  ← looked available

click
 ↓
revalidation
 ↓
DENIED: Reference revision is pending

UI:
Verification was not started.
This Run is paused behind Reference revision R4.

[Open Reference]

```
No bypass.

---

# 33. Availability reason grammar
Unavailable affordances SHOULD use structured reasons where helpful.

Conceptual shape:

```yaml
operation: "verification.run"
available: false

reason:
  class: "missing_evidence | missing_authority | payment | dependency | recovery | reference | runtime | other"
  summary: "Reference revision is pending."
  related_focus: "reference:R4"
  next_affordance: "reference.open"

```
Do not reduce every denial to `"disabled"`.

---

# 34. Keyboard binding rules
The existing keyboard system is earned capability.

Rules:

1. preserve existing bindings unless there is a demonstrated conflict;
2. visible GUI controls gain keyboard equivalents where meaningful;
3. shortcuts invoke semantic command IDs, never component functions;
4. focus traversal remains predictable;
5. modal/overlay focus is scoped but reversible;
6. `Esc` means presentation dismissal/cancel only when semantically legitimate;
7. keyboard users retain access to every important mouse-discoverable operation.
Recommended binding layer:

```text
KeyGesture
 ↓
Keymap resolver
 ↓
semantic command ID
 ↓
same dispatch path

```

---

# 35. Mouse binding rules
Rules:

1. single-click selects;
2. double-click/Enter opens;
3. right-click opens context actions;
4. scroll wheel scrolls the region under pointer;
5. drag only when a visible element genuinely supports movement/resize/semantic drag;
6. closing `×` closes representation unless explicitly labeled destructive;
7. hover never becomes the only way to access critical information;
8. pointer events invoke semantic commands, not inline business mutations.

---

# 36. Drag contract
There are two kinds of drag.

## 36.1 Presentation drag
Examples:

- resize pane;
- move floating window;
- reorder tab.
Class = `PRESENTATION_LOCAL`.

## 36.2 Semantic drag
Examples might eventually include moving/rebinding an object.

Class = `CANONICAL_OPERATION`.

Semantic drag MUST NOT be implemented merely because drag-and-drop looks attractive. It requires an actual meaningful operation and admission semantics.

---

# 37. Scroll contract

## Static/document surfaces
Normal viewport movement.

## Live surfaces
Use:

```text
LIVE_FOLLOW
  ↓ user scrolls away
INSPECTION_HOLD
  ↓ new items
unseen_count++
  ↓ explicit return
LIVE_FOLLOW

```
Never steal attention simply because new output arrives.

In Trace this applies to arrivals from both sources: Gauntlet frames and rows added by a RealityTrace refresh. The unseen count is kept per source.

---

# 38. Responsive interaction mapping

## Wide terminal

```text
Navigator | Primary | Inspector
Bottom Surface optional
Composer persistent

```
Mouse and keyboard both first-class.

## Medium

```text
Navigator | Primary
Inspector becomes drawer/floating Surface

```
Commands do not change.

## Narrow

```text
Primary Surface
Navigator/Inspector become overlays
Bottom tools become switchable View

```
Same semantic commands, different render placement.

## Very narrow / no mouse
Keyboard remains full recovery path.

Responsive layout MUST NOT produce different authorization or affordance semantics.

---

# 39. Image/rich-preview interaction
If image capability supported:

- click artifact → select;
- double-click/open → image preview;
- zoom/pan only presentation-local;
- context menu may offer external open.
If unsupported:

- show textual metadata/fallback;
- operation remains reachable.
Image capability never changes Evidence validity.

---

# 40. Reconnect interactions

## 40.1 Automatic reconnect
UI enters:

`RECONNECTING`.

Workbench establishes a new ProjectionGeneration and recovers baseline/cursors.

No user action should infer Run/Agent death.

## 40.2 Manual retry
**ID:** `runtime.reconnect.retry`

**Class:** `RUNTIME_ADAPTER` / query

**Binding:** `ADAPTER_BIND_EXISTING`

Late frames from prior generation are rejected.

## 40.3 Reattach Prime execution
`run.reattach` / `agent.reattach` only against actual Prime/Gauntlet identities.

---

# 41. Search interactions

## Global search
**ID:** `search.global`

**Class:** `QUERY_ONLY`

**Binding:** `DERIVED_QUERY`

May search:

- Cases;
- Foci;
- Files;
- Events;
- Evidence;
- Threads;
- Runs;
- Domain concepts.
Selecting result navigates.

Search does not create duplicated canonical objects.

---

# 42. Copy interactions
Examples:

- Copy ID;
- Copy path;
- Copy hash;
- Copy event reference;
- Copy evidence reference.
Class = `PRESENTATION_LOCAL` / OS clipboard adapter.

Copying content never changes provenance or canonical identity.

---

# 43. Default behavior by Focus type
| Focus | Single click | Double click / Enter | Right click |
| --- | --- | --- | --- |
| Case | select | open Workspace | open/inspect/bind actions |
| Thread | select | open Chat | open/archive if afforded |
| Reference | inspect | open Reference View | validate/revise/freeze if afforded |
| Run | inspect | hoist/open Run | trace/cancel/etc. if afforded |
| WorkItem | inspect | open/hoist | current Work affordances |
| Agent | inspect | open Agent detail/Trace | stop/message only if afforded |
| Event | inspect | open Event detail | related/replay/fork if afforded |
| RealityTraceRecord | inspect | open record / verification path | bound Gauntlet Focus, refresh, copy ordinal + entry hash, `evidence.offer` on committed bound evidence only |
| Evidence | inspect | open Evidence | compare/open artifact/etc. |
| Verification | inspect | open Verification detail | retry if afforded |
| Settlement | inspect | open record | normally immutable/query actions |
| File | inspect | open file | diff/open external/edit if afforded |
| Artifact | inspect | open preview | external/copy/evidence actions |
| DomainConcept | inspect | open Domain view | traverse/hoist |
| KernelObject | inspect | open Kernel detail | pin/artifactize/offer if afforded |
| AttentionItem | select | open cause | legitimate response actions |

---

# 44. Default behavior by Surface
| Surface | Primary interaction |
| --- | --- |
| Navigator | select/open/hoist/contextual actions |
| Primary | sustained object/View work |
| Inspector | local detail/relation traversal; pin/unpin |
| Bottom | supporting tools; resize/hide/maximize |
| Composer | submit intent or answer temporary request |
| Overlay | command/action discovery and temporary interaction |
| ApplicationChrome | global menus/navigation/status |

---

# 45. Telemetry and traceability of user commands
Every consequential user invocation SHOULD be linkable to:

```text
interaction source
  ↓
semantic command ID
  ↓
target FocusRef
  ↓
admission/result identity
  ↓
resulting authoritative event(s)

```
This enables Trace to answer:

> What did the operator ask the UI to do, what operation actually ran, and what happened?

When bound, RealityTrace separately answers what was observed about consequence. The two answers link only through explicit bindings, never by label or adjacency.

Presentation-only actions need not pollute canonical Gauntlet Trace but may be locally diagnosable if useful.

---

# 46. Plugin command registration schema
Plugin actions MUST register with a contract like:

```yaml
id: "plugin.namespace.semantic_action"
class: "QUERY_ONLY | NAVIGATION_LOCAL | CANONICAL_OPERATION | ..."

target_kinds:
  - "DomainConcept"

availability:
  source: "plugin provider + main AffordanceResolver if consequential"

binding:
  owner: "plugin"
  canonical_operation: "required for consequential action"

render:
  context_menu: true
  command_palette: true
  button: false

teardown:
  unregister_command: true
  remove_provider: true
  remove_subscription: true

```
Plugin unload must remove every interaction registration.

---

# 47. Command naming conventions
Semantic IDs describe intent, not UI placement.

Good:

```text
verification.run
reference.freeze
nav.focus.open
trace.follow.resume
ui.inspector.pin

```
Bad:

```text
rightSidebarButton3
verifyButtonOnClick
ctrlShiftVHandler
contextMenuItem42

```
UI events map to semantic IDs.

---

# 48. Command registry validation
At startup/test time, validate:

1. every consequential command has a truth owner;
2. every consequential command has a binding status;
3. no `CANONICAL_OPERATION` is implemented only as component callback side effect;
4. no two command IDs claim different semantics for the same canonical operation without explicit aliasing;
5. shortcut/menu/context/button aliases converge;
6. plugin commands unregister on unload;
7. unavailable actions have deterministic representation behavior;
8. presentation commands cannot call canonical stores directly.

---

# 49. Interaction test matrix
For every high-value canonical operation, tests SHOULD prove:

| Test | Expected |
| --- | --- |
| Button invocation | reaches operation identity X |
| Keyboard invocation | reaches operation identity X |
| Palette invocation | reaches operation identity X |
| Context-menu invocation | reaches operation identity X |
| Stale availability | invocation revalidates and safely denies |
| Denied operation | no side effect through any projection |
| TUI close | admitted Run/Prime work continues |
| Reopen | projections reconstruct and reattach |
| Narrow terminal | operation remains reachable |
| Mouse unavailable | keyboard path remains reachable |

For presentation-only commands:

| Test | Expected |
| --- | --- |
| Close tab | underlying Focus remains |
| Hide Agents view | Agents keep running |
| Close TUI | Run keeps running |
| Pin Inspector | later selection does not change pinned detail |
| Resize panel | no canonical event/state mutation |
| Back navigation | canonical world unchanged |

For Trace sources:

| Test | Expected |
| --- | --- |
| Gauntlet-only mode | Trace fully operable; RealityTrace shows `UNBOUND` with reason |
| RealityTrace-only and combined modes | every row keeps its source; per-source order preserved |
| RealityTrace refresh during keyboard/mouse navigation | render loop stays responsive; bounded outcome shown |
| Refresh completes after a newer refresh | older result discarded |
| Pending RealityTrace evidence | rendered as pending; `evidence.offer` refused `pending_evidence` from every projection |
| RealityTrace settlement record | never rendered as Gauntlet Settlement; not offerable |
| Label-only RealityTrace ↔ Unit match | no adjacency, no binding |

---

# 50. High-risk adversarial tests

## 50.1 Visual bypass attack
Make a stale button appear enabled after authority is revoked.

Expected:

- invocation denied by canonical revalidation;
- no side effect;
- UI refreshes exact reason.

## 50.2 Keyboard bypass attack
Disable a mouse button but invoke its old shortcut.

Expected:

- same denial semantics.

## 50.3 Palette bypass attack
Search for a command that is not currently afforded.

Expected:

- hidden or disabled/inspectable reason;
- invocation cannot bypass.

## 50.4 Direct component mutation attack
Attempt to set WorkItem state from React reducer.

Expected:

- architectural/test failure; no legitimate API supports it.

## 50.5 Close-as-cancel attack
Close active Run/Agent tab/window.

Expected:

- execution continues.

## 50.6 Message-as-completion attack
Receive "done" from child mailbox.

Expected:

- Chat/Trace may update;
- WorkItem/Run completion does not change without Gauntlet evidence/settlement.

## 50.7 Spawn-handle-as-result attack
Prime returns child admission handle.

Expected:

- Agent shows admitted/spawning;
- not completed/result.

## 50.8 Trace live-tail attention attack
New events arrive while operator inspects older Event.

Expected:

- viewport does not move;
- unseen count increases.

## 50.9 RealityTrace settlement-as-Gauntlet-settlement attack
A bound RealityTrace ledger records `settled` for a question bound to Unit T23.

Expected:

- the record renders as a RealityTrace settlement record;
- T23's Gauntlet lifecycle, Settlement, and Evidence do not change;
- `evidence.offer` on it is refused `settlement_is_sxr_only`.

## 50.10 Pending-as-committed attack
RealityTrace output carries `spooled` or `queued_for_commit` durability.

Expected:

- rendered as pending, apart from committed rows;
- no projection offers it; `evidence.offer` refused `pending_evidence`.

## 50.11 Label-join attack
A RealityTrace declaration name equals a Gauntlet Unit label with no explicit binding.

Expected:

- no binding, no adjacency, no Inspector relation.

## 50.12 Render-time refresh attack
Force a render path to invoke the RealityTrace adapter, or hang the producer during navigation.

Expected:

- render never invokes `executeSxrRefresh`;
- deadline/output bounds end the refresh; navigation stays responsive.

---

# 51. Hallmark redesign restrictions
Hallmark may redesign how commands are exposed:

- buttons;
- menus;
- tabs;
- panes;
- cards;
- popovers;
- context menus;
- command palette;
- breadcrumbs;
- visible controls;
- windows;
- scrolling;
- layout.
It MAY change interaction discoverability and presentation.

It MUST NOT:

- introduce alternate business command paths;
- bypass the command registry;
- hardcode consequential behavior into components;
- infer authority from disabled/enabled visuals;
- treat close controls as process lifecycle controls;
- replace canonical operation IDs with component methods;
- duplicate keyboard and mouse behavior implementations;
- expose installed capability as an affordance without gates;
- hide loss of capability behind aesthetic redesign.

---

# 52. Implementation checklist for each interaction
Before adding any interactive element, answer:

```text
1. What is the user trying to do?

2. Is that:
   - presentation,
   - navigation,
   - query,
   - runtime control,
   - or canonical operation?

3. What semantic command ID represents the intent?

4. What Focus/Workspace is targeted?

5. Who owns the truth/change?

6. Where does availability come from?

7. Does invocation require revalidation?

8. What happens when the UI's availability projection is stale?

9. What is the equivalent keyboard path?

10. What is the equivalent mouse path?

11. Should it appear in the command palette?

12. Should it appear in the context menu?

13. Does another surface already expose the same operation?

14. If yes, do they converge on the same semantic ID?

15. What authoritative event/result confirms actual effect?

16. What may the UI optimistically change?
    Normally only presentation state.

17. What must never be inferred from visual state?

```
If any consequential command cannot answer these questions, it is not ready to implement.

---

# 53. Compact command architecture

```text
                       USER
                         │
      ┌──────────────────┼──────────────────┐
      │                  │                  │
     Mouse             Keyboard           Palette
      │                  │                  │
      └──────────────────┼──────────────────┘
                         ▼
                Interaction Intent
                         │
                         ▼
                 Semantic Command ID
                         │
          ┌──────────────┼───────────────┐
          │              │               │
 Presentation/Nav      Query        Consequential
          │              │               │
          ▼              ▼               ▼
    Workbench Model   Read Model   AffordanceResolver
          │              │               │
          │              │          revalidation
          │              │          ┌────┴─────┐
          │              │          │          │
          │              │        DENY       ADMIT
          │              │                     │
          │              │                     ▼
          │              │            Canonical Operation
          │              │                     │
          │              │         Prime/Jupyter/tools as needed
          │              │                     │
          └──────────────┴─────────────────────┘
                         │
                authoritative/projection
                     updates
                         │
                         ▼
               Immutable Snapshot
                         │
                         ▼
                    OpenTUI

```

---

# 54. Final interaction law
The workbench should make GUI behavior feel ordinary while keeping consequence rigorous:

> **Clicking, typing, dragging, selecting, or pressing a shortcut may change what the user sees immediately. It may change the world only by invoking the one operation that legitimately owns that change.**

And:

> **Mouse and keyboard are two representations of intent, not two implementations of behavior.**

The desired experience is:

```text
visible object
  → familiar GUI interaction
  → semantic command
  → current affordance
  → governed operation
  → traceable consequence
  → evidence
  → settlement

```
That is how Gauntlet can behave like a desktop-class application without allowing the redesigned terminal UI to become a second, less-governed execution system.