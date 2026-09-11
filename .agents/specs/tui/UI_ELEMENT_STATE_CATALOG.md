# Gauntlet TUI — UI Element & State Contract

> **Status:** Draft implementation-facing design contract

> **Purpose:** Make the Gauntlet TUI difficult to misimplement during a radical GUI-grade redesign.

> **Applies to:** React + OpenTUI workbench, mouse/keyboard interaction, traceability UI (the Trace surface over Gauntlet execution history and RealityTrace), Prime Python REPL projections, DomainForge projections, and plugin-contributed UI.

> **Upstream authority:** `gauntlet-tui-prime-workbench.spec.yaml`, `DESIGN.md`, the GodSpeed Visual Language, and canonical Gauntlet/Prime/DomainForge semantics.

> **Downstream consumers:** Hallmark redesign work, React/OpenTUI component design, workbench-model implementation, UI tests, interaction tests, plugin authors, and agent-generated frontend changes.

> **Trace terminology:** "Trace" means the Trace surface: one ordered, source-typed ledger over two sources.
> 1. **Gauntlet execution history** — canonical frames folded by `WorkbenchModel` into `trace_records` (identity `trace-<source_id>-<cursor>`). Always present.
> 2. **RealityTrace** — the SXR consequence ledger (`../sxr`), read through `workbench/src/sxr-adapter.ts`. Present only when explicitly bound.
>
> The sources are never merged. A RealityTrace record is not a Gauntlet Event. A RealityTrace settlement is not a Gauntlet Settlement. RealityTrace evidence becomes Gauntlet evidence only through `evidence.offer` and canonical admission. Spec basis: REQ-SXR-001..004, REQ-DATA-005, REQ-TUI-003. Detail: §16.0 here and `INTERACTION_COMMAND_MAP.md` §16.0.

---

## 0. Contract
This document defines **what UI elements exist, what kind of things they are, who owns their truth, what state machines or state vectors govern them, how they may be rendered, and what an implementation MUST NOT infer from presentation state**.

It is not a replacement for the Gauntlet specification. It does not redefine Run, Reference, Evidence, Settlement, Capability, Prime Agent, or DomainForge semantics. Where a canonical implementation already defines exact state names, the implementation MUST bind this design contract to those real states rather than inventing a competing enum.

The design goal is:

> **A GUI-grade, direct-manipulation, trace-native semantic workbench rendered through the terminal.**

A novice should be able to use visible controls, mouse interaction, tabs, panes, scrolling, context menus, dialogs, and familiar application conventions. An expert should retain the full keyboard path. Both paths MUST converge on the same semantic operations and admission rules.

---

# 1. Non-collapse laws
These are hard design invariants.

| Never collapse | Reason |
| --- | --- |
| Case ↔ project folder | A folder/repository is a resource binding, not the purposeful world. |
| Case ↔ Thread | A conversation is a projection into the Case, not the Case itself. |
| Thread ↔ Run | Conversation is not governed long-horizon action. |
| Orchestrator ↔ Builder | Controller/conversational lead is not the default specialist worker. |
| Orchestrator ↔ Researcher | Shaping/coordination is not specialist research. |
| Orchestrator ↔ Critic | Interpretation is not adversarial criticism. |
| Orchestrator ↔ Verifier | Coordination cannot self-supply independent verification. |
| Message ↔ Event | Communication is not occurrence. |
| Command ↔ Event | Intent is not consequence. |
| Event ↔ Evidence | Something happening is not automatically admissible proof. |
| Evidence ↔ Verification | Observation is not evaluation. |
| Verification ↔ Settlement | Passing a verifier is not itself consequential acceptance. |
| Settlement ↔ Capability | One settlement does not establish transferable/recoverable capability. |
| Capability ↔ installed tool | Access or availability is not capability. |
| Focus ↔ FocusPlacement | Identity is not a particular visual appearance. |
| Focus ↔ renderer | Semantic object is not its React/OpenTUI component. |
| Selected ↔ primary attention | UI selection is not cognitive allocation. |
| Cue ↔ AttentionItem | Perceptual signal is not the canonical cause. |
| close tab ↔ delete object | Closing a representation does not destroy the represented object. |
| close TUI ↔ stop Run | UI presence does not own long-running execution lifecycle. |
| close agent view ↔ stop agent | Presentation is not process control. |
| frozen Reference ↔ ongoing conversation | Execution contract is not mutable chat context. |
| kernel value ↔ Evidence | Computed state is not admitted evidence. |
| Prime mailbox message ↔ completion | Communication does not establish consequence. |
| agent completion ↔ WorkItem settlement | Worker termination does not establish settlement. |
| React state ↔ canonical state | Presentation/projection state is disposable. |
| Bun SQLite projection ↔ canonical state | Read model is reconstructible cache/projection. |
| RealityTrace record ↔ Gauntlet Event | An external consequence observation is not Gauntlet's own execution occurrence. |
| RealityTrace settlement ↔ Gauntlet Settlement | SXR settlement is source-typed; it never settles a WorkItem or Run. |
| RealityTrace evidence ↔ admitted Gauntlet Evidence | Admission requires `evidence.offer` + canonical admission against the frozen Reference. |
| pending RealityTrace record ↔ committed record | `spooled` / `queued_for_commit` is not committed ledger state. |
| label match ↔ RealityTrace binding | RealityTrace and Gauntlet identities join only through explicit bindings. |
| Trace interleaving ↔ causal order | Cross-source row order is a derived presentation; only per-source order is authoritative. |
| green color ↔ “success” | Multiple distinct semantic states may share a visual color. |
| hover ↔ availability | Pointer state does not establish affordance legitimacy. |
| enabled control ↔ guaranteed authorization | Every consequential invocation revalidates current admission. |

---

# 2. Semantic Atomic UI taxonomy
Gauntlet does not use atoms/molecules/organisms as its primary UI semantics.

```text
Environment
  ↓
Workspace
  ↓
Surface
  ↓
View
  ↓
Focus

```
Orthogonal to that hierarchy:

```text
Cue
Attention
Relation
Affordance
Flow
FocusPlacement
SemanticLocation

```

## 2.1 Element kinds
| Kind | Definition | Canonical? |
| --- | --- | --- |
| Environment | Complete operating world available to the operator. | No; composition/projection over several authorities. |
| Workspace | Bounded activity field, normally a Case or hoisted Horizon. | Projection. |
| Surface | Stable perceptual region that hosts Views. | Presentation. |
| View | Question-oriented projection over current world. | Projection. |
| Focus | Smallest semantically complete thing that can be attended to, inspected, related, or acted upon. | Often references canonical identity elsewhere. |
| FocusPlacement | One presentation of one Focus in one View/Surface. | Presentation. |
| Cue | Perceptible signal caused by a relevant condition. | Projection. |
| Attention | Allocation of operator cognition. | Workbench/operator state. |
| Relation | Typed connection between Foci. | May be canonical, authored, derived, or projected. |
| Affordance | Currently legitimate spendable move. | Derived from canonical state/gates. |
| Flow | Derived path through objects/actions toward consequence. | Projection. |
| SemanticLocation | Reproducible perceptual/navigation location. | Workbench navigation state. |
| Renderer primitive | Dock, tab, menu, window, button, splitter, etc. | Presentation only. |

---

# 3. Truth-ownership matrix
The UI MUST know which layer is allowed to change which truth.

| Concern | Truth owner | Workbench role |
| --- | --- | --- |
| Case identity/lifecycle | Gauntlet | Project and render. |
| Reference / frozen set | Gauntlet | Review, navigate, request canonical operations. |
| Run admission/lifecycle | Gauntlet | Project, invoke allowed operations, never infer. |
| WorkItem lifecycle | Gauntlet | Project and navigate. |
| Authority/admission | Gauntlet | Render result; invoke canonical admission path. |
| Evidence admission | Gauntlet | Render candidate/admitted/rejected/etc. |
| Verification records | Gauntlet / configured verifier mechanism | Project records; never synthesize pass. |
| Settlement | Gauntlet | Project immutable/append-only consequence. |
| Capability claim | Gauntlet developmental ledger | Project evidence-backed state. |
| Prime session lifecycle | Prime host | Project/reattach/request control through adapter. |
| Prime child lifecycle | Prime host | Project admission and later result separately. |
| Prime messages | Prime mailbox | Project communication, not completion. |
| Prime Python REPL kernel lifecycle | Prime Python REPL host | Project, request control through adapter. |
| Kernel variables | Kernel | Project ephemeral/pinned/artifactized references. |
| Domain semantic identity | DomainForge when present | Project concepts/relations/policies. |
| Files/repository | Filesystem/VCS | Project; mutations go through canonical operations/tools. |
| Local read models | Workbench/Bun SQLite | Reconstructible projections only. |
| Gauntlet execution history (Trace source 1) | Gauntlet canonical event log | Fold frames into `trace_records`; reject stale generations. |
| RealityTrace ledger records, ordinal, entry hash (Trace source 2) | RealityTrace (SXR) | Project through bounded refresh; never append; show absence/invalidity with reason. |
| RealityTrace durability (`committed` / `spooled` / `queued_for_commit`) | RealityTrace (SXR) command output | Render pending apart from committed; never promote. |
| RealityTrace settlement status | RealityTrace (SXR) | Render as a source-typed record; never as Gauntlet Settlement. |
| RealityTrace ↔ Gauntlet identity bindings | Explicit binding inputs (`admitSxrUnitBindings`) | Project; refuse label joins. |
| Trace cross-source row order | Workbench (derived presentation) | Own; per-source order stays authoritative. |
| Current selection | Workbench | Own. |
| Open tabs/windows/pane sizes | Workbench | Own. |
| Hover/pointer state | OpenTUI/workbench | Own. |
| Navigation history | Workbench | Own. |
| Theme/density/preferences | Workbench/user config | Own. |
| Mouse capability / color capability | Terminal runtime | Detect and adapt. |
| React component lifecycle | React | Own only renderer lifecycle. |

---

# 4. State modeling rules

## 4.1 Prefer state vectors over mega-enums
Do not encode:

```text
SELECTED_RUNNING_VERIFIED_STALE_PINNED

```
as one enum.

Represent independent dimensions independently.

Example:

```text
WorkItem T23
domain.lifecycle       = VERIFYING
interaction.selection  = SELECTED
attention              = PRIMARY
evidence.freshness     = CURRENT
placement.disclosure   = EXPANDED

```

## 4.2 State-machine categories
Every state below is one of:

- **canonical lifecycle** — owned by Gauntlet/Prime/DomainForge/etc.;
- **derived semantic state** — computed from canonical records;
- **interaction state** — selection, hover, focus, disclosure;
- **presentation state** — docked, floating, maximized, hidden;
- **capability state** — terminal/runtime support;
- **stream state** — hydration, live follow, reconnect;
- **developmental state** — evidence-backed capability progression.
A renderer MUST NOT promote interaction or presentation state into canonical lifecycle state.

---

# 5. Global application elements

## 5.1 `GauntletEnvironment`
**Kind:** Environment

**Truth owner:** Composite; no single canonical store.

**Purpose:** Presents the currently reachable Gauntlet + Prime + repository + DomainForge + plugin world.

**Contains:**

- Case inventory;
- active Case Workspace;
- Prime runtime status;
- kernel status;
- plugin registry;
- terminal capability profile;
- workbench model/projections.

### State vector

```text
readiness:
  INITIALIZING
  HYDRATING
  OPERATIONAL
  DEGRADED
  RECONNECTING
  FATAL

optional_capabilities:
  prime: AVAILABLE | DEGRADED | UNAVAILABLE
  kernel: AVAILABLE | RECONNECTING | UNAVAILABLE
  domainforge: AVAILABLE | UNAVAILABLE
  reality_trace: UNBOUND | BOUND_CURRENT | BOUND_ABSENT | BOUND_INVALID | BOUND_INCOMPATIBLE
  images: SUPPORTED | UNSUPPORTED
  mouse: SUPPORTED | UNSUPPORTED
  truecolor: SUPPORTED | REDUCED | MONOCHROME

```

### State machine

```text
INITIALIZING
     ↓
HYDRATING
     ↓
OPERATIONAL
   ↙    ↓       ↘
DEGRADED RECONNECTING FATAL
   ↑       │
   └───────┘

```
**Mouse:** application-level menus, visible controls when supported.

**Keyboard:** full fallback; environment must remain operable without mouse.

**Forbidden assumptions:**

- DomainForge absence does not imply total environment failure.
- Kernel reconnecting does not imply Run failure.
- RealityTrace being unbound or unavailable does not imply Trace failure.
- Workbench fatal state must not rewrite underlying canonical state.

---

## 5.2 `ApplicationChrome`
**Kind:** Surface/presentation shell

**Truth owner:** Workbench

**Purpose:** Stable application identity, menu bar, current Case/Horizon summary, global operational cues.

**May contain:**

- File / View / Run / Agents / Help menus;
- current Case identity;
- connection/runtime cues;
- global Attention count;
- back/forward controls.

### State vector

```text
visibility: VISIBLE | HIDDEN_BY_CONSTRAINT
density: REVIEW | OPERATIONAL
connectionCue: NONE | DEGRADED | RECONNECTING

```
**Forbidden assumptions:** application chrome is not a dashboard and must not become a second command authority.

---

# 6. Workspace elements

## 6.1 `CaseWorkspace`
**Kind:** Workspace

**Truth owner:** Case truth = Gauntlet; workspace projection = Workbench

**Purpose:** Primary bounded field around one Case.

### Lifecycle

```text
CLOSED
  ↓ open
OPENING
  ↓
ACTIVE
  ↓ close
CLOSED

```
Hoisting is orthogonal:

```text
horizonMode: CASE | RUN | WORK_ITEM | SAVED_HORIZON | OTHER_FOCUS

```
**Contains:** Navigator, Primary, Inspector, Bottom, Composer, Overlay surfaces.

**Forbidden assumptions:** `CaseWorkspace` state never determines whether the Case itself exists or is running.

---

## 6.2 `Horizon`
**Kind:** Workspace projection constraint

**Truth owner:** Workbench representation; selection may reference canonical identities

**Purpose:** Narrows what is queried, subscribed, prefetched, rendered, and supplied to agents without deleting the rest of the world.

### State machine

```text
CASE_HORIZON
  ↓ hoist
FOCUS_HORIZON
  ↓ dehoist
PARENT_HORIZON
  ↓ ...
CASE_HORIZON

```
**Must shape where appropriate:**

- query horizon;
- subscription horizon;
- semantic traversal;
- cache/prefetch;
- Prime context construction;
- visible affordances.
**Forbidden assumptions:** hoisting is not mutation, deletion, or authority narrowing unless Gauntlet separately says so.

---

## 6.3 `SemanticLocation`
**Kind:** Navigation projection

**Truth owner:** Workbench

**Purpose:** Addressable location such as Case + Horizon + View + Focus.

Conceptual shape:

```text
Case C17
Horizon WorkItem T23
Primary View Trace
Selected Focus Event EV-88

```
Possible serialized form:

```text
gauntlet://case/C17/work/T23?view=trace&focus=event:EV-88

```

### History state

```text
A → B → C

Back:    C → B → A
Forward: A → B → C

```
**Forbidden assumptions:** navigation history is not domain history or time travel.

---

# 7. Surface elements

## 7.1 Generic `Surface`
**Kind:** Surface

**Truth owner:** Workbench

**Presentation forms:**

- `DockedSurface`
- `FloatingSurface`
- `OverlaySurface`
- `ModalSurface`
- `PopoverSurface`

### State vector

```text
visibility: HIDDEN | VISIBLE
focus: UNFOCUSED | FOCUSED
size: COLLAPSED | NORMAL | MAXIMIZED
placement: DOCKED | FLOATING | OVERLAY | MODAL | POPOVER
interaction: STABLE | RESIZING | DRAGGING

```
**Forbidden assumption:** presentation form does not change semantic identity of its content.

---

## 7.2 `NavigatorSurface`
**Purpose:** Spatial orientation and direct manipulation of Cases, Threads, Work, Runs, Evidence, Files, Agents, etc.

**Primary behaviors:**

- click selects;
- double-click/Enter opens;
- disclosure control expands/collapses;
- scroll wheel/keyboard scroll;
- contextual menu derives from current affordances;
- breadcrumbs/hoist links may narrow Horizon.
**Must not:** duplicate Focus state per placement.

---

## 7.3 `PrimarySurface`
**Purpose:** Sustained work on one active View/object.

**Hosts:** Chat, Work, Trace, Evidence, Files, Reference, Wayfinder, etc.

**Behavior:** tabs or equivalent GUI-grade working-object navigation.

---

## 7.4 `InspectorSurface`
**Purpose:** Local detail and relationships for selected Focus.

### State machine

```text
EMPTY
 ↓ select Focus
FOLLOW_SELECTION
 ↓ pin
PINNED
 ↓ unpin
FOLLOW_SELECTION

```
**Inspector rule:** central ledger/view stays compact; rich detail is local to selection.

---

## 7.5 `BottomSurface`
**Purpose:** Supporting tools.

Likely tabs:

- Terminal;
- Problems;
- Agents;
- Kernel;
- Evidence;
- Changes.

### State vector

```text
visibility: HIDDEN | VISIBLE
size: NORMAL | MAXIMIZED
interaction: STABLE | RESIZING
activeTab: <tool view>

```
**GUI behavior:** draggable top boundary, visible close/hide, tabs, scrolling.

---

## 7.6 `ComposerSurface`
**Purpose:** User intent and temporary operator-input takeover.

### Draft machine

```text
EMPTY
 ↓ type
DRAFT
 ↓ submit
SUBMITTING
 ↓
EMPTY

```

### Takeover machine

```text
DEFAULT
 ├→ QUESTION
 ├→ APPROVAL
 ├→ RECOVERY
 └→ KERNEL_INPUT
        ↓ resolve/cancel
      DEFAULT

```
**Critical invariant:** preserve and restore pre-takeover draft where possible.

---

## 7.7 `OverlaySurface`
**Purpose:** Command palette, dialogs, temporary windows, menus, contextual interactions.

**Rule:** overlay elevation changes perception, not authority.

---

# 8. View elements
Generic View data state:

```text
UNLOADED
   ↓
LOADING
   ↓
READY
 ↙   ↘
STALE ERROR

```
Generic activation:

```text
INACTIVE ↔ ACTIVE

```
Activity and data state are independent.

## 8.1 View catalog
| View | Primary question | Main Foci |
| --- | --- | --- |
| HomeView | What Case should I create/open? | Case, ProjectBinding |
| CaseOverviewView | What is this purposeful world? | Case, Reference, Run summaries |
| ChatView | What was communicated between user and Orchestrator? | Thread, Message |
| WorkView | What are we trying to move? | WorkItem, Run, Evidence, Settlement |
| ReferenceView | What frozen representation governs long-horizon work? | Reference, purposeful artifacts |
| WayfinderView | What distinctions are still missing? | WayfinderArtifact, unresolved question |
| RunsView | What long-horizon attempts exist? | Run |
| TraceView | What actually happened? (Gauntlet execution history + RealityTrace consequence ledger when bound) | Event, ToolCall, RoleInvocation, Verification, RealityTraceRecord |
| EvidenceView | What supports/contradicts claims? | Evidence, Verification |
| AgentsView | Where is execution capacity allocated? | Orchestrator, SpecialistAgent, PrimeSession |
| FilesView | What project artifacts are involved? | File, Diff, Artifact |
| DomainView | What semantic world are we operating against? | DomainConcept, DomainRelation, Policy |
| KernelView | What live computation exists? | KernelObject, KernelExecution |
| HarnessView | (roadmap-only — DSN-015; must not register while the boundary holds) | Skill, HarnessRevision |
| AttentionView | What genuinely requires operator judgment? | AttentionItem |
| SettingsView | What operator/render configuration is active? | settings |
| HelpView | How can the current application be operated? | documentation/commands |

**Rule:** plugins MAY add Views but MUST preserve the same interaction contract and unload reversibly.

---

# 9. Focus infrastructure

## 9.1 `FocusRef`
**Kind:** Identity reference

**Truth owner:** Workbench reference to external canonical identity

**Purpose:** Stable identity handle passed through Views/renderers without copying domain objects.

Example conceptual values:

```text
case:C17
run:R31
work:T23
event:EV-88
evidence:EV991
agent:builder-03
file:src/runtime.rs

```
**Forbidden:** storing canonical mutable object truth inside the FocusRef.

---

## 9.2 `FocusPlacement`
**Kind:** Presentation identity

**Truth owner:** Workbench

**Purpose:** One appearance of a Focus.

Example:

```text
T23
├ plan placement
├ current Run placement
├ agent assignment placement
├ attention placement
└ evidence relation placement

```

### Interaction state

```text
render: NOT_RENDERED | RENDERED
pointer: NORMAL | HOVERED
selection: UNSELECTED | SELECTED
disclosure: COLLAPSED | EXPANDED | NOT_APPLICABLE

```
Destroying a placement MUST NOT destroy its Focus.

---

## 9.3 Generic Focus interaction vector

```text
visibility: HIDDEN | VISIBLE
pointer: NORMAL | HOVERED
selection: UNSELECTED | SELECTED
opening: CLOSED | OPEN
attention: LATENT | CONTEXTUAL | PRIMARY

```
These dimensions remain independent of domain lifecycle.

---

# 10. Core domain Foci

## 10.1 `Case`
**Kind:** Focus / canonical Gauntlet object

**Truth owner:** Gauntlet

**Meaning:** Durable purposeful world.

**May bind:** one or many repositories/folders/resources, semantic models, evidence, prior Runs, threads.

### UI state
Do not invent a Case lifecycle if Gauntlet already has one. UI may project:

```text
availability: OPENABLE | DEGRADED | UNAVAILABLE
activity: QUIET | ACTIVE_RUNS
attention: NONE | REQUIRES_ATTENTION

```
**Forbidden:** Case == project folder.

---

## 10.2 `ProjectBinding`
**Kind:** Focus/resource relation

**Truth owner:** Gauntlet config + filesystem/repository

**Meaning:** Resource bound into a Case.

### UI states

```text
BOUND
MISSING
STALE
UNREACHABLE

```
These are resource conditions, not Case lifecycle.

---

## 10.3 `Thread`
**Kind:** Focus / conversational projection

**Truth owner:** Gauntlet/Prime conversational persistence as implemented

**Meaning:** User ↔ Orchestrator conversational line into one Case.

### Lifecycle

```text
NEW
 ↓
ACTIVE
 ↕
IDLE
 ↓
ARCHIVED
 ↓ reopen
ACTIVE

```
A Thread may initiate zero, one, or many Runs.

**Forbidden:** Thread == Run; Thread == Case memory boundary.

---

## 10.4 `Message`
**Kind:** Focus / communication object

**Truth owner:** appropriate conversational/message substrate

**Meaning:** Actor communicated content.

### UI state

```text
delivery: PENDING | DELIVERED | FAILED
render: STREAMING | COMPLETE

```
**Forbidden:** delivered/complete message == task completion.

---

## 10.5 `Orchestrator`
**Kind:** Focus / persistent user-facing controller

**Truth owner:** Prime session + Gauntlet orchestration contract

**Meaning:** Conversational interface and work shaper.

It is not the default Builder/Researcher/Critic/Verifier.

### Behavior state machine

```text
IDLE
 ↓ user input
INTERPRETING
 ├→ RESPOND
 ├→ BOUNDED_ACTION
 ├→ SHAPE_PURPOSE
 ├→ REQUEST_ATTENTION
 ├→ DELEGATE
 └→ PROPOSE_RUN
         ↓
REPORT_OR_CONTINUE
         ↓
IDLE

```

### Delegation heuristic

```text
deterministic function
        ↓ if insufficient
bounded Orchestrator action
        ↓ if insufficient/long-horizon/specialized
specialist agent
        ↓ if judgment/authority required
human Attention

```
**Allowed:** light coding, inspection, small tests, small edits, drafting candidate artifacts.

**Not equivalent to:** long-horizon builder, independent verifier, specialist critic, dedicated researcher.

---

# 11. Purpose and Reference formation

## 11.1 `WayfinderArtifact`
**Kind:** Focus / purposeful representation artifact

**Truth owner:** Gauntlet artifact/reference formation mechanism

**Meaning:** Structured output of conversational elicitation when purpose is insufficiently represented.

### Lifecycle

```text
INACTIVE
   ↓
ELICITING
   ↔
CLARIFYING
   ↓
CANDIDATE
   ↓
REVIEW
 ┌─┴───────┐
 ↓         ↓
REVISE    FREEZE
 ↑          ↓
 └────── FROZEN

```
Once frozen, changes produce a new revision/candidate.

---

## 11.2 `Reference`
**Kind:** Focus / canonical Gauntlet object

**Truth owner:** Gauntlet

**Meaning:** Frozen set of purposeful artifacts/representations against which a Run proceeds.

The specific artifact set is contextual. It may include `.wayfinder`, spec, plan, DESIGN, `.sea`, acceptance criteria, research method, governance constraints, or other purposeful artifacts.

### Lifecycle

```text
COLLECTING
    ↓
CANDIDATE
    ↓
VALIDATING
 ┌──┴─────────┐
 ↓            ↓
DEFICIENT    READY
 ↑            ↓
 └─ revise   FREEZE
              ↓
           FROZEN

```
If a frozen Reference is discovered to be defective:

```text
FROZEN
  ↓ defect
REVISION_REQUIRED
  ↓
NEW_CANDIDATE

```
Never silently mutate a frozen Reference governing an active Run.

---

## 11.3 `ReferenceReadiness`
**Kind:** Derived semantic UI object/Cue group

**Truth owner:** Derived from Gauntlet reference requirements

**Purpose:** Make pre-Run missing distinctions visible.

May show:

- purpose;
- scope;
- constraints;
- settlement criteria;
- authority;
- recovery;
- required artifacts;
- unresolved questions.
**Forbidden:** fake percentage certainty not grounded in explicit readiness conditions.

---

# 12. Run admission and long-horizon work

## 12.1 `RunAdmission`
**Kind:** Affordance/derived gate state

**Truth owner:** Gauntlet

**Meaning:** Whether long-horizon action is currently admissible.

### State machine

```text
NO_SUFFICIENT_REFERENCE
        ↓
RUN_UNAVAILABLE
        ↓ frozen sufficient Reference
RUN_AVAILABLE
        ↓ requested
REVALIDATING
     ┌──┴─────┐
     ↓        ↓
   DENIED   ADMITTED

```
`Start Run` MUST derive from this path.

An unavailable Run action should communicate *why* it is unavailable rather than simply vanish where doing so helps representation formation.

---

## 12.2 `Run`
**Kind:** Focus / canonical Gauntlet object

**Truth owner:** Gauntlet

**Meaning:** Governed long-horizon attempt under frozen Reference.

### Lifecycle

```text
ADMITTED
   ↓
INITIALIZING
   ↓
EXECUTING
 ┌─┼──────────────┐
 │ ↓              ↓
 │ BLOCKED      RECOVERING
 │ ↓              ↓
 │ WAITING_     EXECUTING
 │ ATTENTION
 │ ↓
 └─→ EXECUTING
       ↓
FINAL_VERIFICATION
       ↓
SETTLEMENT_PENDING
       ↓
SETTLED

```
Exceptional terminal conditions may include:

```text
CANCELLED
TERMINATED
FAILED_UNRECOVERABLE

```
Bind exact names to canonical implementation.

**Forbidden:** all workers stopping == Run settled.

---

## 12.3 `WorkItem`
**Kind:** Focus / canonical Gauntlet settlement unit

**Truth owner:** Gauntlet

**Meaning:** Requirement(s) → transformation → evidence → falsification → confirmation.

### Lifecycle

```text
PLANNED
   ↓ dependencies/affordances satisfied
READY
   ↓ admitted
ACTIVE
 ├→ BLOCKED
 │    ↓ condition clears
 │  ACTIVE
 │
 ↓ result produced
VERIFICATION_PENDING
 ↓
VERIFYING
 ├→ FAILED
 │     ↓ correction
 │   ACTIVE
 │
 ↓
SETTLEMENT_PENDING
 ↓
SETTLED

```
**Hard rule:** coding done ≠ verified ≠ settled.

---

# 13. Specialist execution

## 13.1 `SpecialistAgent`
**Kinds:** Builder, Researcher, Critic, Verifier, other bounded role

**Truth owner:** Prime child lifecycle + Gauntlet role/work relation

**Meaning:** Admitted execution capacity for bounded work.

### Lifecycle

```text
NOT_MATERIALIZED
       ↓ needed + affordable + governable
ELIGIBLE
       ↓
SPAWN_REQUESTED
       ↓ Prime admission
SPAWNING
       ↓
ACTIVE
 ├→ WAITING
 ├→ BLOCKED
 ├→ COMPLETED
 ├→ FAILED
 └→ CANCELLED

```
Attachment is orthogonal:

```text
ATTACHED ↔ DETACHED ↔ REATTACHING

```
**UI default:** show activity/result, not the agent's private conversation as a first-class user thread.

---

## 13.2 `RoleInvocation`
**Kind:** Focus / execution relation

**Truth owner:** Gauntlet + Prime adapter

**Meaning:** One bounded invocation of a role for a purpose.

Useful UI facts:

- role;
- purpose;
- parent/orchestrator;
- WorkItem;
- Reference scope;
- context boundary;
- current status;
- cost/payment;
- produced artifacts/evidence.

---

## 13.3 `PrimeSession`
**Kind:** Focus/runtime projection

**Truth owner:** Prime host

**State:**

```text
CREATED
 ↓
ACTIVE
 ↔ IDLE
 ↓
DETACHED
 ↓ reattach
ACTIVE

```
Other runtime outcomes may include failed/terminated according to Prime.

**Forbidden:** stale session directory == authoritative active session.

---

# 14. Affordance and command path

## 14.1 `Affordance`
**Kind:** Orthogonal semantic element

**Truth owner:** Gauntlet resolver/gates

**Meaning:** Visible, reachable, payable, governable, settleable/recoverable next move.

### State machine

```text
NOT_RELEVANT
      ↓ world/horizon changes
REPRESENTED_UNAVAILABLE
      ↓ gates satisfied
AVAILABLE
      ↓ invoked
REVALIDATING
   ┌──┴───────┐
   ↓          ↓
DENIED      ADMITTED

```
At `ADMITTED`, execution is represented by a Command/Run/etc.; do not turn Affordance into a task lifecycle.

### Interaction projections
The same affordance MAY appear as:

- visible button;
- context-menu entry;
- command-palette action;
- keyboard shortcut;
- CLI/API command;
- kernel callable;
- eligible agent capability.
All MUST converge on one canonical operation/admission meaning.

---

## 14.2 `Command`
**Kind:** Focus / intent-to-action record

**Truth owner:** Gauntlet operation path

**Meaning:** An actor requested something consequential.

### Lifecycle

```text
INTENT
  ↓
REVALIDATING
 ├→ DENIED
 ↓
ADMITTED
 ↓
DISPATCHED
 ↓
EXECUTING
 ├→ FAILED
 ├→ CANCELLED
 ↓
COMPLETED
 ↓
OUTCOME_EVENT(S)

```
**Forbidden:** Command == Event.

---

## 14.3 `AuthorityDecision`
**Kind:** Focus / governance record

**Truth owner:** Gauntlet

**Lifecycle:**

```text
PROPOSED
   ↓
PENDING
 ┌─┼───────────┐
 ↓ ↓           ↓
ALLOW DENY   ESCALATE

```
An escalated decision should remain historically escalated; later resolution creates a linked new decision rather than rewriting history.

---

# 15. Attention system

## 15.1 `AttentionItem`
**Kind:** Focus / canonical or Gauntlet-owned cause requiring judgment

**Truth owner:** Gauntlet

**Meaning:** Real condition requiring operator resolution.

Conceptual lifecycle:

```text
CREATED
  ↓
ACTIVE
  ↓ surfaced
PRESENTED
  ↓
ACKNOWLEDGED
  ↓ actual issue resolved
RESOLVED

```
Bind exact statuses to canonical implementation.

**Hard rule:** dismissing a Cue does not resolve the AttentionItem.

---

## 15.2 `Attention`
**Kind:** Orthogonal workbench allocation

**Truth owner:** Workbench/operator

**State vector:**

```text
LATENT
CONTEXTUAL
PRIMARY

```
Conceptual movement:

```text
LATENT
 ↓ Cue/user selection
CONTEXTUAL
 ↓ raise
PRIMARY
 ↓ move away
CONTEXTUAL
 ↓
LATENT

```

---

## 15.3 `Cue`
**Kind:** Orthogonal presentation signal

**Truth owner:** Derived

**Lifecycle:**

```text
ABSENT
 ↓ relevant condition
VISIBLE
 ↓ condition no longer relevant
ABSENT

```
May also be acknowledged without resolving its cause.

Examples:

- blocked;
- verification pending;
- stale evidence;
- authority required;
- running;
- new evidence.

---

# 16. Traceability system
Trace is execution and consequence history, not merely chat or logs. The Trace surface is one ordered ledger over two source-typed streams: Gauntlet execution history (always) and the RealityTrace consequence ledger (when explicitly bound). It builds on the existing pipelines — `WorkbenchModel.trace_records` for Gauntlet, `sxr-adapter.ts` for RealityTrace — and adds no parallel trace model or renderer-side shadow state beside them.

## 16.0 Trace sources
| Source | Truth owner | Identity | Authoritative order | Presence |
| --- | --- | --- | --- | --- |
| Gauntlet execution history | Gauntlet canonical event log → `WorkbenchModel.trace_records` | `trace-<source_id>-<cursor>` + projection generation | cursor | always |
| RealityTrace ledger | RealityTrace (SXR) → `parseSxrLedgerExport` → `projectSxrEvidenceLens` | `ledger_id` + `ordinal` + `entry_hash` | ordinal within `ledger_id` | when bound |

Composition rules:

- rows keep their source; one source is never styled as the other;
- per-source order is authoritative; cross-source interleaving is derived and marked derived; interleave by time only when both rows carry a recorded timestamp (the RealityTrace export omits `committed_at`);
- cross-source relations come only from explicit bindings (`admitSxrUnitBindings`);
- RealityTrace pending state renders apart from committed records;
- absent / invalid / incompatible RealityTrace is a visible source status; Gauntlet-only Trace stays fully usable;
- rendering never invokes the RealityTrace adapter; refresh is an explicit bounded `RUNTIME_ADAPTER` command (`INTERACTION_COMMAND_MAP.md` §16.11).

## 16.1 `TraceView`
**Primary representation:** turn-aware/event-aware ledger with compact central rows, a source marker on every row, and rich local Inspector detail.

The primary ledger should favor:

- index/identity;
- source;
- event kind/actor (or RealityTrace record kind);
- concise content/result;
- critical semantic Cue.
Secondary information belongs in Inspector:

- full input/output;
- duration;
- payment/tokens;
- provenance;
- WorkItem relation;
- parent/child relation;
- files/artifacts;
- evidence;
- authority;
- settlement relevance;
- for RealityTrace records: kind, ordinal, entry hash, durability, verification path, unknowns, export omissions, explicit bindings.

---

## 16.2 `TraceStream`

### Stream machine

```text
COLD
 ↓
HYDRATING
 ↓
LIVE_FOLLOW
 ↓ user scrolls upward
INSPECTION_HOLD
 │
 │ new events arrive
 │ unseen_count++
 │
 ↓ Return to live
LIVE_FOLLOW

```

### Reconnect machine

```text
LIVE
 ↓ disconnect
RECONNECTING
 ↓ establish new projection generation
HYDRATING
 ↓ baseline + cursor + increments
LIVE

```
Frames from an older generation after a newer generation is active MUST be rejected.

The stream and reconnect machines above govern the Gauntlet source. The follow machine also governs rows added by RealityTrace refresh.

### RealityTrace source machine

```text
UNBOUND
   ↓ explicit binding
BOUND_ABSENT (no_ledger_export)
   ↓ trace.reality.refresh
REFRESHING
 ┌─┼──────────┬──────────────┬────────────────────────────────────────────┐
 ↓ ↓          ↓              ↓
CURRENT    INVALID    INCOMPATIBLE    REFUSED | DEADLINE_EXCEEDED | OUTPUT_LIMIT | SPAWN_FAILED
   ↓ later refresh
REFRESHING

```
A refresh that completes after a newer refresh is discarded. A failed refresh keeps the last `CURRENT` lens visible and marks it stale; it never clears recorded rows or invents replacements.

**Attention rule:** operator inspection outranks streaming novelty.

---

## 16.3 `TraceTurn`
**Kind:** Derived grouping

**Purpose:** Human-comprehensible conversational/execution turn boundary.

**Contains:** TraceSteps/TraceRows.

**Forbidden:** turn boundary must not fabricate canonical event grouping if the underlying event stream does not support it; derived grouping must remain identifiable as derived. RealityTrace records are never grouped into a Gauntlet turn; they may appear adjacent to one only through an explicit binding.

---

## 16.4 `TraceStep`
**Kind:** Derived sub-group inside a Turn

**Purpose:** Preserve nested tool/subagent/activity structure without flooding the main ledger.

---

## 16.5 `TraceRow`
**Kind:** FocusPlacement for Event/ToolCall/etc.

**State vector:**

```text
source: GAUNTLET | REALITY_TRACE
visibility: VISIBLE | VIRTUALIZED
selection: UNSELECTED | SELECTED
disclosure: COLLAPSED | EXPANDED_WHERE_ALLOWED

```

---

## 16.6 `Event`
**Kind:** Focus / durable occurrence

**Truth owner:** authoritative event store appropriate to source

**Lifecycle:**

```text
CREATED
 ↓
COMMITTED

```
Then immutable.

UI interaction:

```text
UNSEEN → VISIBLE → SELECTED → INSPECTED

```
Interaction does not mutate history.

---

## 16.7 `ToolCall`
**Kind:** Focus / execution event specialization

**Truth owner:** execution trace

**May show:** tool, concise input, concise result, duration, artifacts, errors.

**Forbidden:** successful tool return == WorkItem settlement.

---

## 16.8 `RealityTraceRecord`
**Kind:** Focus / external consequence-observation record

**Truth owner:** RealityTrace (SXR) ledger

**Identity:** `ledger_id` + `ordinal` + `entry_hash` (`SxrOrdinal`, `SxrEntryHash`)

**Kinds the lens interprets:** question, threshold, semantic snapshot, run, evidence, settlement. Other kinds are preserved with `kind_known: false` and never interpreted.

### State vector

```text
durability: COMMITTED | SPOOLED | QUEUED_FOR_COMMIT
kind:       KNOWN | UNKNOWN_PRESERVED
binding:    BOUND(<Gauntlet FocusRef>) | UNBOUND
lens:       CURRENT | STALE

```
Committed records are immutable. Pending durability is never rendered as committed and never offered.

**Forbidden:** RealityTrace record == Gauntlet Event; RealityTrace settlement == Gauntlet Settlement; label match == binding.

---

## 16.9 `RealityTraceVerificationPath`
**Kind:** Derived relation projection

**Truth owner:** Derived from RealityTrace records + explicit bindings (`sxrVerificationPath`)

Steps: declaration → unit binding → question → threshold → evidence → RealityTrace settlement, each carrying ordinal + entry hash.

```text
chain_complete: TRUE | FALSE(missing[])

```
Completeness is an observation about RealityTrace records, never a Gauntlet Verification result. Missing links are shown, never invented.

---

## 16.10 `RealityTraceBinding`
**Kind:** Relation

**Truth owner:** explicit binding inputs admitted by `admitSxrUnitBindings`

Binds RealityTrace repository/run/entity/question identities to Gauntlet workspace/reference/Run/Unit identities. A declaration name is never an identity. Unbound records stay in the RealityTrace segment with no Gauntlet adjacency.

---

# 17. Evidence, verification, settlement, capability

## 17.1 `Evidence`
Evidence uses orthogonal dimensions rather than one mega-lifecycle.

### Existence

```text
MISSING → PRESENT

```

### Admission

```text
CANDIDATE
 ├→ ADMITTED
 ├→ REJECTED
 └→ QUARANTINED

```

### Freshness

```text
CURRENT → STALE

```

### Claim relation

```text
SUPPORTS
CONTRADICTS
INCONCLUSIVE

```
Example coherent state:

```text
EV-17:
  present
  admitted
  stale
  supports claim C4

```
Historical evidence MUST NOT be rewritten merely because fresher evidence exists.

### Source

```text
GAUNTLET
REALITY_TRACE

```
RealityTrace evidence records keep their source and ledger identity (§16.8). They enter Gauntlet admission only through `evidence.offer` (`INTERACTION_COMMAND_MAP.md` §16.14). Offer ≠ admission ≠ verification ≠ settlement.

---

## 17.2 `Verification`
**Kind:** Focus / evaluation record

**Truth owner:** configured verifier/Gauntlet verification mechanism

### Lifecycle

```text
NOT_STARTED
   ↓
RUNNING
 ┌─┼─────────────┐
 ↓ ↓             ↓
PASS FAIL    INCONCLUSIVE

```
Retries should normally create a new Verification record/round, preserving failed history.

---

## 17.3 `Settlement`
**Kind:** Focus / consequential record

**Truth owner:** Gauntlet

### Lifecycle

```text
PENDING
  ↓
EVALUATING
 ┌─┼───────────────┬──────────────┐
 ↓ ↓               ↓              ↓
ACCEPTED       REJECTED       ESCALATED     QUARANTINED

```
Correction creates later linked settlement/correction evidence, not silent history mutation.

RealityTrace settlement records (`settled`, `settled_with_debt`, other producer words) are a separate source-typed channel rendered as `RealityTraceRecord` (§16.8), never as this Focus.

---

## 17.4 `Capability`
**Kind:** Focus / developmental ledger state

**Truth owner:** Gauntlet developmental memory

### Developmental state

```text
NOT_ESTABLISHED
       ↓ attempt
ATTEMPTED
       ↓ successful settlement
DEMONSTRATED
       ↓ repeated variation + transfer + recovery + reduced orchestration
PROVEN

```
Loss/risk states:

```text
PROVEN → DEGRADED
any established state → QUARANTINED

```
Recovery must be earned with evidence.

---

# 18. Artifact and resource elements

## 18.1 `File`
**Kind:** Focus/resource

**Truth owner:** filesystem/VCS

**Interaction:** select, inspect, open, diff, context menu.

**Forbidden:** open file == canonical WorkItem focus unless explicitly related.

---

## 18.2 `Diff`
**Kind:** Focus/derived artifact view

**Truth owner:** VCS/tool output

**Presentation:** OpenTUI diff renderer where available.

State dimensions may include:

```text
source: WORKTREE | COMMIT | ARTIFACT | GENERATED
review: UNREVIEWED | VIEWED

```
Do not invent "approved" from viewed.

---

## 18.3 `Artifact`
**Kind:** Focus/resource

**Truth owner:** artifact store/filesystem/Gauntlet evidence record relation

**May include:** reports, screenshots, generated files, preregistrations, logs.

---

## 18.4 `RichArtifactPreview`

### Capability machine

```text
CAPABILITY_UNKNOWN
   ↓ detect
 ┌─┴─────────────┐
 ↓               ↓
SUPPORTED     UNSUPPORTED
 ↓               ↓
LOADING         FALLBACK
 ↓
RENDERED
 └→ ERROR → FALLBACK

```
Image support is enrichment only. Understanding/operation must survive without it.

---

# 19. Kernel elements

## 19.1 `KernelObject`
**Kind:** Focus / computational working object

**Truth owner:** persistent Prime Python REPL kernel until artifactized/admitted

### Classification machine

```text
EPHEMERAL
   ↓ pin
PINNED
   ↓ serialize
ARTIFACTIZED
   ↓ offer
EVIDENCE_CANDIDATE
   ↓ Gauntlet admission
EVIDENCE

```
**Hard rule:** no kernel variable is canonical merely because it exists.

---

## 19.2 `KernelExecution`
**Kind:** Focus / computation event

**Truth owner:** Prime host (Python REPL)

Possible states:

```text
QUEUED
 ↓
RUNNING
 ├→ WAITING_FOR_STDIN
 ├→ COMPLETED
 ├→ ERROR
 └→ INTERRUPTED

```
One kernel shell is serialized; do not create faux concurrent shell-execution semantics.

---

## 19.3 `KernelInputRequest`
**Kind:** Attention-triggering interaction

**Presentation:** Composer takeover or modal where appropriate.

```text
REQUESTED
 ↓
PRESENTED
 ├→ ANSWERED
 └→ CANCELLED/INTERRUPTED

```

---

# 20. DomainForge elements
Only materialize when a Case is bound to relevant SEA semantics.

## 20.1 `SemanticModel`
**Kind:** Focus

**Truth owner:** DomainForge

**Meaning:** Bounded purposeful semantic world.

## 20.2 `DomainConcept`
**Kind:** Focus

**Truth owner:** DomainForge

## 20.3 `DomainRelation`
**Kind:** Relation/Focus depending on projection

**Truth owner:** DomainForge/authored model where canonical

## 20.4 `Policy`
**Kind:** Focus

**Truth owner:** DomainForge/SEA model where applicable

**Hard rule:** semantic identity/fingerprint is not interchangeable with file path, cache key, Run ID, or UI identity.

---

# 21. Harness/runtime elements

**Status:** `Skill` and `HarnessRevision` (21.1–21.2) are roadmap-only per `DESIGN.md` DSN-015 (`prime_harness_metabolization_ui`); no harness View or harness interaction may be registered until that boundary is lifted. `Plugin` (21.3) is not roadmap-bound.

## 21.1 `Skill`
**Kind:** Focus / reusable agent capability representation

**Truth owner:** Prime/Gauntlet harness as implemented

**Forbidden:** installed Skill == proven Gauntlet Capability.

---

## 21.2 `HarnessRevision`
**Kind:** Focus / candidate reusable harness change

### Lifecycle

```text
CURRENT
   ↓ repeated failure / candidate improvement
CANDIDATE_REVISION
   ↓
EVALUATING
 ├→ RETAINED
 └→ ROLLED_BACK

```
Prime must not self-certify the revision.

---

## 21.3 `Plugin`
**Kind:** Focus/runtime extension

**Truth owner:** Workbench/plugin host

### Lifecycle

```text
DISCOVERED
   ↓
LOADING
   ├→ FAILED
   ↓
ACTIVE
   ↓
UNLOADING
   ↓
UNLOADED

```
Settlement check:

```text
mount
→ exercise
→ unload
→ compare baseline

```
No leaked slots, handlers, subscriptions, read-model state, or renderers.

---

# 22. GUI-grade renderer primitives
These are presentation mechanics, not new domain ontology.

## 22.1 `Tab`

```text
CLOSED
  ↓ open
OPEN_INACTIVE
  ↓ select
ACTIVE
  ↓ another selected
OPEN_INACTIVE
  ↓ close
CLOSED

```
Hard rule:

```text
close tab ≠ delete Focus ≠ stop Run ≠ stop Agent

```

---

## 22.2 `FloatingWindow`

```text
CLOSED
 ↓
OPEN_UNFOCUSED
 ↓
FOCUSED
 ├→ DRAGGING → FOCUSED
 ├→ RESIZING → FOCUSED
 ↓
CLOSED

```
Use for temporary independent inspection, not as the dominant sustained-work model.

---

## 22.3 `ModalDialog`

```text
CLOSED
 ↓
OPEN
 ├→ CONFIRM
 ├→ CANCEL      when legitimate
 └→ VALIDATION_BLOCKED
 ↓
CLOSED

```
A dialog may request an authority operation; it is never the authority itself.

---

## 22.4 `Popover`

```text
CLOSED
 ↓ anchor interaction
OPEN
 ├→ HOVER_ITEM
 ├→ INVOKE_ACTION
 └→ DISMISS
 ↓
CLOSED

```

---

## 22.5 `ContextMenu`
Specialized Popover whose actions derive from `AffordanceResolver(FocusRef)`.

Never maintain a second static command list for consequential actions.

---

## 22.6 `Tooltip`

```text
HIDDEN
 ↓ hover delay
ARMED
 ↓
VISIBLE
 ↓ leave
HIDDEN

```
Never hide required information exclusively in a tooltip.

---

## 22.7 `MenuBar`

```text
CLOSED
 ↓ click/shortcut
MENU_OPEN
 ↔ switch menu
 ↓ invoke/dismiss
CLOSED

```
Only expose real commands. Do not add desktop cosplay.

---

## 22.8 `CommandPalette`

```text
CLOSED
 ↓
OPEN
 ↓ query
FILTERING
 ↓ selection
SELECTED
 ↓ invoke
CLOSED

```
May surface:

- navigation;
- Views;
- Focus search;
- current Affordances;
- settings;
- recent locations.

---

## 22.9 `ActionControl`
Presentation state:

```text
HIDDEN
VISIBLE_DISABLED
VISIBLE_ENABLED

```
Interaction:

```text
VISIBLE_ENABLED
 ↓ click/key
PRESSED
 ↓
REVALIDATE_AFFORDANCE

```
Visible enabled state is never the final authority check.

---

## 22.10 `TreeItem`
State vector:

```text
disclosure: COLLAPSED | EXPANDED | N/A
pointer: NORMAL | HOVERED
selection: UNSELECTED | SELECTED
availability: ENABLED | DISABLED
attention: NONE | CUED

```
Behaviors:

- row click = select;
- Enter/double-click = open;
- disclosure glyph = expand/collapse;
- right click = contextual Affordances.

---

## 22.11 `SplitPane`

```text
STABLE
 ↓ drag splitter
RESIZING
 ↓ release
STABLE

```
Pane size is user/workbench preference, never Case truth.

---

## 22.12 `ScrollRegion`
Generic:

```text
IDLE ↔ SCROLLING

```
Live streams use the explicit `LIVE_FOLLOW ↔ INSPECTION_HOLD` model.

---

## 22.13 `Toast`

```text
QUEUED
 ↓
VISIBLE
 ├→ EXPIRES
 └→ DISMISSED

```
If the condition matters after expiry, a durable Cue/AttentionItem must exist elsewhere.

---

## 22.14 `Breadcrumb`
**Kind:** navigation projection

**Behavior:** click ancestor to dehoist/navigate.

A breadcrumb is not a domain containment claim unless the underlying relationship is canonical.

---

## 22.15 `StatusBadge`
**Kind:** renderer primitive for one semantic state token

**Rule:** label/glyph + semantic color where appropriate.

Never create a generic `success`, `warning`, `error` vocabulary when a more precise semantic token exists.

---

# 23. Visual semantic state families
These are semantic presentation tokens; they are not a replacement ontology.

```text
authority:
  allowed
  denied
  escalated
  degraded
  pending

execution:
  running
  waiting
  succeeded
  failed
  cancelled

settlement:
  pending
  evaluating
  accepted
  rejected
  escalated
  quarantined

evidence:
  complete
  missing
  failed
  stale
  quarantined

readiness:
  ready
  degraded
  blocked

freshness:
  current
  stale

integrity:
  verified
  partial
  pending
  failed

capability:
  attempted
  demonstrated
  proven
  degraded
  quarantined

focus:
  primary
  ring

attention:
  required

danger:
  destructive

proof:
  independently_confirmed
  sanctioned_authority

```
Same color MAY represent different semantic tokens. They must remain independently addressable.

Gold is reserved for proof/sanctioned authority/earned confirmation, not generic selection.

---

# 24. Mouse–keyboard isomorphism
The GUI redesign MUST add interaction projections, not parallel behavior systems.

| User intent | Mouse projection | Keyboard projection | Canonical meaning |
| --- | --- | --- | --- |
| Select Focus | click | arrows/navigation | Workbench selection |
| Open Focus | double-click | Enter | SemanticLocation/open presentation |
| Context actions | right-click | context shortcut/palette | current Affordances |
| Switch View | click tab | shortcut | View activation |
| Close presentation | click × | close shortcut | close Tab/Surface only |
| Resize pane | drag | keyboard resize if provided | presentation preference |
| Scroll | wheel/drag scrollbar | arrows/PageUp/PageDown | viewport movement |
| Invoke operation | visible button/menu | shortcut/palette | one Gauntlet operation |
| Return to live Trace | click new-events marker | end/live shortcut | Trace stream state |
| Refresh RealityTrace | click source-status refresh control | shortcut/palette | bounded RealityTrace adapter refresh |
| Answer approval/question | click visible choice | keyboard selection | canonical response operation |

A keyboard expert must not lose capability. A mouse-first novice must not need undocumented shortcuts.

---

# 25. Run-formation composition
The pre-Run experience is a first-class composition.

Elements:

- `PurposeSummary`
- `ReferenceReadiness`
- `PurposefulArtifactInventory`
- `MissingDistinctions`
- `WayfinderQuestion`
- `FrozenSetPreview`
- `RunAdmissionStatus`
- `StartRunAffordance`

### Composition state

```text
EXPLORATORY
 ↓
SHAPING
 ↓
REFERENCE_INCOMPLETE
 ↕ Wayfinder / artifact creation
REFERENCE_READY
 ↓
REVIEWING
 ↓
FROZEN
 ↓
RUN_AVAILABLE

```
This composition should make it obvious that conversation can continue before a Run exists.

---

# 26. New Case composition
Elements:

- Case name;
- project/resource binding;
- existing purposeful artifacts;
- optional SEA model/world;
- creation action.

### State machine

```text
NEW_CASE_REQUEST
       ↓
DEFINE_CASE
       ↓
BIND_RESOURCES
       ↓
VALIDATE
    ↙      ↘
INVALID   READY
   ↑        ↓
   └── revise
          CREATE
            ↓
       CASE_ACTIVE

```
The UX may feel like "create project," but the underlying identity remains a Case.

---

# 27. Primary user journey state machine

```text
NO_CASE
   ↓
CREATE_OR_OPEN_CASE
   ↓
CASE_WORKSPACE
   ↓
START_OR_OPEN_THREAD
   ↓
CONVERSE_WITH_ORCHESTRATOR
   │
   ├──── bounded/light work ─────────┐
   │                                 │
   ↓                                 │
SHAPE_PURPOSE                        │
   ↓                                 │
FORM_REFERENCE                       │
   ↓                                 │
FREEZE                               │
   ↓                                 │
RUN_AVAILABLE                        │
   ↓                                 │
ADMIT_RUN                            │
   ↓                                 │
LONG_HORIZON_EXECUTION               │
   ↓                                 │
TRACE_AND_EVIDENCE                   │
   ↓                                 │
VERIFICATION                         │
   ↓                                 │
SETTLEMENT                           │
   ↓                                 │
METABOLIZATION ──────────────────────┘
   ↓
CONTINUE_THREAD / NEW_THREAD / NEW_RUN

```
This is the highest-level behavioral model the GUI must make legible without forcing the user to learn internal jargon first.

---

# 28. Trace-native interaction contract
The workbench is spatial, but execution history is a first-class operative surface.

## Primary representation order

```text
1. ordered/turn-aware ledger
2. local Inspector relationships/details
3. graph/other relation visualization only when it answers a question

```
Do not default to a giant node graph.

## Interaction
- single click Event → select, Inspector follows;
- Enter/double-click → open detail where useful;
- right click → current legitimate actions such as inspect, related objects, replay/fork if truly afforded;
- scroll upward → freeze live-follow;
- new events while inspecting → increment unseen counter, do not steal attention;
- return-to-live → resume live-follow;
- relation traversal → navigate without losing history/back capability;
- source marker on every row; source filter (Gauntlet / RealityTrace);
- RealityTrace source status (unbound / absent / invalid / incompatible / stale) is visible and clickable → Inspector reason + refresh when afforded;
- selecting a RealityTrace record → Inspector shows ordinal, entry hash, durability, verification path, bindings.

---

# 29. Element implementation record schema
Every new element added to the TUI SHOULD be documented using this record before or with implementation:

```yaml
id: "<stable element id>"
name: "<human name>"
kind: "<Environment|Workspace|Surface|View|Focus|Cue|Attention|Relation|Affordance|Flow|RendererPrimitive>"

meaning: >
  What kind of thing this is.

truth_owner: "<Gauntlet|Prime|DomainForge|Kernel|Filesystem|Workbench|Derived>"

canonical_identity:
  type: "<if applicable>"
  source: "<where identity comes from>"

parents:
  - "<allowed semantic parent/container>"

children:
  - "<allowed semantic children>"

state:
  canonical_dimensions: []
  derived_dimensions: []
  interaction_dimensions: []
  presentation_dimensions: []

events_consumed:
  - "<event or projection update>"

events_emitted:
  - "<user intent / renderer event; not canonical event unless admitted upstream>"

affordances:
  source: "<canonical resolver or presentation-only action>"
  examples: []

render_forms:
  - "<row|card|tab|inspector|window|ledger-row|etc>"

mouse:
  click: "<behavior>"
  double_click: "<behavior>"
  right_click: "<behavior>"
  hover: "<behavior>"
  drag: "<behavior>"

keyboard:
  focus: "<behavior>"
  open: "<behavior>"
  actions: "<behavior>"

accessibility_and_fallback:
  - "<color-independent meaning>"
  - "<mouse-independent path>"
  - "<narrow-terminal behavior>"

forbidden_assumptions:
  - "<what this element must never imply>"

```

---

# 30. Implementation gate questions
Before implementing or redesigning any component, the agent MUST answer:

1. **What kind of thing is this?**Environment, Workspace, Surface, View, Focus, Cue, Attention, Relation, Affordance, Flow, or renderer-only primitive?
2. **Who owns its truth?**Gauntlet, Prime, DomainForge, kernel, filesystem/VCS, Workbench, or derived projection?
3. **Which dimensions of state are canonical versus interaction/presentation?**
4. **What event or condition legitimately causes each state transition?**
5. **Does this UI action invoke a canonical operation or merely manipulate presentation?**
6. **Does mouse behavior converge with the existing keyboard behavior?**
7. **Could closing/hiding this component accidentally stop or mutate something it does not own?**
8. **Could the component accidentally infer evidence, verification, settlement, capability, or authority from a visual condition?**
9. **If OpenTUI/React state disappears, can canonical state be reconstructed?**
10. **If mouse/images/truecolor are absent, does the operation remain understandable and reachable?**
If any answer is unclear, stop and inspect the governing source before implementing.

---

# 31. Hallmark redesign guardrail
Hallmark or any visual redesign agent has broad freedom to change:

- composition;
- pane layout;
- tabs;
- menus;
- visible controls;
- spacing;
- density;
- mouse interactions;
- windows;
- dialogs;
- cards;
- navigation presentation;
- typography/emphasis;
- responsive layout;
- visual hierarchy.
It does **not** have authority to redefine:

- Case;
- Thread;
- Orchestrator role;
- Wayfinder;
- Reference;
- frozen set;
- Run;
- WorkItem;
- specialist-agent roles;
- Evidence;
- Verification;
- Settlement;
- Capability;
- canonical command/admission path;
- Prime Python REPL lifecycle semantics;
- Focus identity;
- affordance legitimacy;
- truth ownership.
A radical visual redesign is acceptable. A semantic rewrite disguised as frontend cleanup is not.

---

# 32. Final design compression
The complete interaction model can be remembered as:

```text
Case gives the world.
Thread gives the conversation.
Orchestrator gives conversational control.
Wayfinder gives missing distinctions a representation.
Reference gives long-horizon work a frozen contract.
Run gives long-horizon action a governed substrate.
Specialist agents give bounded execution capacity.
Trace gives history an inspectable ledger: Gauntlet execution history plus RealityTrace consequence observations, source-typed.
Evidence gives consequence something observable.
Verification evaluates claims.
Settlement determines what becomes consequential.
Capability records what survives repetition, variation, transfer, and recovery.

Focus makes one thing locally central.
Relations make its neighborhood visible.
Affordances expose only legitimate next moves.
Attention protects operator judgment.
OpenTUI renders all of it without owning any of it.

```
The desired user experience is therefore:

> **Open a Case → start a Thread → talk naturally to Gauntlet → shape purpose until there is something worth acting against → freeze the Reference → start a governed Run → watch/inspect work through a trace-native workbench → intervene only where judgment is needed → settle against evidence → continue the conversation from a more capable Case.**