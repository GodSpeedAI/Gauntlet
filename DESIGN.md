# Gauntlet TUI Design System

> **Scope:** Terminal-native operator workbench for Gauntlet, implemented through React + OpenTUI on the Bun workbench substrate.
>
> **Design authority:** This file governs how the Gauntlet TUI should look, compose, navigate, expose state, and lower semantic interaction into terminal presentation. It does **not** redefine Gauntlet's normative execution, authority, evidence, or settlement semantics.
>
> **Source hierarchy:**
> 1. `../godspeed-growth/brand/visual/visual-language.md` — GodSpeed visual philosophy and cross-product visual authority.
> 2. `gauntlet-tui-prime-workbench.spec.yaml` — normative behavioral/system contract.
> 3. `DESIGN.md` — normative TUI visual/interaction design contract.
> 4. `gauntlet-tui-prime-workbench.plan.yaml` — settlement path for implementing and proving the above.
> 5. renderer/theme/component code — replaceable implementation projections.
>
> The SEA Forge semantic CSS is a precedent for naming presentation by **meaning rather than appearance**. Gauntlet carries that pattern into a terminal renderer: semantic state first, terminal representation second.

---

## Visual Theme & Atmosphere

### Design thesis

The Gauntlet TUI is an **operational control plane for governed developmental execution**. It should feel like serious infrastructure under active use: precise, dense when density is useful, quiet when judgment is required, and evidence-bearing at every consequential boundary.

The governing visual formula is:

```text
institutional authority
+ terminal-native operational precision
+ evidence-bearing artifacts
+ restrained cinematic contrast
+ semantic state visibility
+ controlled velocity
```

The product should communicate, before explanation:

```text
I know where I am.
I know what is happening.
I know what requires attention.
I know what evidence exists.
I know what I can legitimately do next.
```

It must **not** communicate:

```text
AI spectacle
chat-first assistant
busy dashboard
command-center cosplay
neon hacker aesthetic
status theater
```

### Design objective

The current world should become **perceptually coincident around the operator's Focus**.

When the operator focuses a WorkItem, Run, Agent, Evidence record, File, Domain concept, or other consequential object, the interface should make its relevant relations locally adjacent:

```text
Focus
  ├ semantic relations
  ├ execution relations
  ├ agents
  ├ files/artifacts
  ├ evidence
  ├ history
  ├ attention conditions
  └ current affordances
```

The operator should not manually reconstruct those joins from separate applications.

### Interaction ontology

The TUI uses the following semantic Atomic UI grammar:

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

The following operate orthogonally across that hierarchy:

```text
Cue
Attention
Relation
Affordance
Flow
```

These terms describe **meaning**, not React components.

#### Environment

The complete operating world available to the operator: Gauntlet, Prime Agent, DomainForge when present, repository/workspace, evidence, artifacts, tools, and installed plugins.

#### Workspace

A bounded activity field, normally a Gauntlet Case or a hoisted Horizon within a Case.

#### Surface

A stable perceptual region that hosts one or more Views. The primary surfaces are Navigator, Primary, Inspector, Bottom, Composer, and Overlay.

#### View

A projection that answers a specific operator question. A View never becomes canonical state.

#### Focus

The smallest semantically complete object of attention and action. Examples include Case, WorkItem, Run, Agent, Event, Evidence, File, AttentionItem, DomainConcept, KernelObject, Skill, and HarnessRevision.

#### Cue

A perceptible signal about a condition relevant to a Focus: blocked, running, verification pending, stale evidence, authority required, and similar states.

#### Attention

The operator's cognitive allocation across Foci. Attention is not the same thing as a canonical `AttentionItem`.

#### Relation

A typed connection between Foci: `requires`, `executed-by`, `produced`, `supports`, `contradicts`, `observed-by`, `derived-from`, and similar relations.

#### Affordance

A currently legitimate spendable move. It is not equivalent to a technically installed capability.

#### Flow

Movement through related Foci and operations toward consequence, correction, or settlement.

### Identity rule

Never collapse identity into representation.

```text
Focus identity
≠ FocusPlacement
≠ View
≠ React component
≠ OpenTUI renderable
```

A single WorkItem may appear under the plan, current Run, active Agent, Attention, and Evidence while remaining one underlying Focus.

React/OpenTUI should render `FocusRef` placements rather than copying domain state.

### Brand register

The TUI is primarily a product surface, so it uses the GodSpeed visual language's two registers as **density/posture modes**, not as separate brands.

#### Journey / Review posture

Use when trust, judgment, or consequence dominates:

- settlement review;
- approval/escalation;
- evidence inspection;
- reference review;
- governance and recovery;
- high-risk destructive actions.

Characteristics:

- more whitespace;
- fewer simultaneous cues;
- explicit provenance;
- longer labels;
- restrained motion;
- evidence objects given room to breathe;
- lower information compression.

#### Speed / Operational posture

Use when active execution and navigation dominate:

- Work view during implementation;
- Trace inspection;
- agent monitoring;
- terminal/kernel work;
- file/diff traversal.

Characteristics:

- compact rows;
- more visible technical metadata;
- faster navigation;
- more simultaneous but structured state;
- sharper contrast;
- denser relation surfaces.

The posture may change presentation density. It MUST NOT change semantic state, authority, or affordance meaning.

### Product references

Directional influences:

- Linear — precision, spacing, operational UI craft;
- Vercel — infrastructure clarity and technical hierarchy;
- Palantir — operational gravity and consequence;
- Bloomberg/control-room systems — live operational density, but not clutter;
- Stripe technical docs — proof-oriented clarity.

Do not imitate any reference literally. The GodSpeed visual language governs.

---

## Color Palette & Roles

### Principle

Color is semantic instrumentation. It is never decoration.

A component MUST NOT select a raw color because it “looks right.” It resolves a semantic token, and the active terminal theme lowers that token to truecolor, 256-color, or monochrome presentation.

```text
system meaning
    ↓
semantic visual token
    ↓
terminal theme
    ↓
glyph + color + emphasis + wording
```

Color alone MUST NOT carry consequential meaning.

### Core brand palette

| Token | Value | Role |
|---|---:|---|
| `brand.prussian` | `#003153` | structural identity, selected structure, major anchors |
| `brand.nearBlack` | `#05070A` | primary dark workspace field |
| `brand.white` | `#FFFFFF` | high-contrast primary text |
| `brand.ivory` | `#F7F3EA` | light-mode/editorial fallback when supported |
| `brand.gold` | `#D7A84A` | proof, sanctioned authority, confirmation, route highlight |
| `neutral.slate` | `#566978` | borders, secondary structure, metadata |
| `neutral.mist` | `#E7EBEE` | light dividers / high-lightness neutral |

### Gauntlet dark-surface projection

The TUI defaults to dark instrument-panel presentation.

| Token | Default | Use |
|---|---:|---|
| `surface.workspace` | `#05070A` | Environment/Workspace background |
| `surface.panel` | `#0B1220` | normal Surface background |
| `surface.elevated` | `#111827` | active/selected elevated region |
| `surface.overlay` | `#0F172A` | command palette, modal, takeover composer |
| `surface.code` | `#08101D` | code/diff/kernel/terminal framing |
| `surface.muted` | `#334155` | separators, inactive structure |
| `fg.primary` | `#FFFFFF` | primary content |
| `fg.secondary` | `#94A3B8` | supporting content |
| `fg.tertiary` | `#64748B` | metadata and de-emphasis |

These values are the Gauntlet TUI projection, not a redefinition of the cross-product palette.

### Utility states

Use the GodSpeed visual language's utility roles for state-bearing product UI.

| Semantic token | Default | Meaning |
|---|---:|---|
| `authority.allowed` | `#2F8F5B` | proposed transformation is authorized |
| `authority.denied` | `#9E2F2F` | proposed transformation is prohibited |
| `authority.escalated` | `#C58A2A` | explicit authority decision requires escalation |
| `authority.pending` | `#7C8790` | authority decision not yet established |
| `execution.running` | `#2563EB` | execution is live |
| `execution.waiting` | `#7C8790` | execution admitted but waiting |
| `execution.succeeded` | `#2F8F5B` | execution completed its observed operation |
| `execution.failed` | `#9E2F2F` | execution failed |
| `execution.cancelled` | `#7C8790` | execution was cancelled |
| `settlement.pending` | `#7C8790` | settlement has not been established |
| `settlement.evaluating` | `#2563EB` | settlement evaluation is active |
| `settlement.accepted` | `#2F8F5B` | settlement criteria were accepted |
| `settlement.rejected` | `#9E2F2F` | settlement criteria were not met |
| `settlement.escalated` | `#C58A2A` | settlement requires escalation |
| `evidence.complete` | `#2F8F5B` | required evidence exists for the represented claim |
| `evidence.missing` | `#C58A2A` | required evidence is absent |
| `evidence.failed` | `#9E2F2F` | evidence contradicts or fails the required check |
| `evidence.stale` | `#C58A2A` | evidence exists but fails freshness expectation |
| `readiness.ready` | `#2F8F5B` | represented work is ready under current gates |
| `readiness.degraded` | `#C58A2A` | usable with an explicit degraded condition |
| `readiness.blocked` | `#9E2F2F` | unavailable because a required gate is unsatisfied |
| `integrity.verified` | `#2F8F5B` | integrity/verification check is satisfied |
| `integrity.partial` | `#7C8790` | only partial verification exists |
| `integrity.pending` | `#C58A2A` | verification has not completed |
| `integrity.failed` | `#9E2F2F` | verification failed |
| `capability.attempted` | `#7C8790` | attempt exists but capability is not established |
| `capability.demonstrated` | `#2563EB` | capability demonstrated under limited conditions |
| `capability.proven` | `#2F8F5B` | capability meets current proof requirements |
| `capability.degraded` | `#C58A2A` | capability exists with a known degradation |
| `focus.primary` | `#2563EB` | current primary Focus |
| `focus.ring` | `#60A5FA` | keyboard/interaction focus boundary |
| `attention.required` | `#C58A2A` | operator attention is required |
| `danger` | `#9E2F2F` | irreversible/destructive risk |

### Gold discipline

`brand.gold` is scarce.

Gold means one of:

- proof mark;
- independently confirmed claim;
- sanctioned authority record;
- explicit decision marker;
- selected route/trace whose meaning is evidentiary or authoritative.

Gold MUST NOT mean:

- ordinary selection;
- generic importance;
- hover/focus;
- normal success;
- decoration;
- “premium” styling.

Examples:

```text
✓ tests passed                     → evidence/verification green
◆ independently confirmed          → gold proof marker
● active Focus                      → blue focus token
! authority escalation required    → amber authority token
```

### Same color does not mean same state

Several semantic tokens may intentionally resolve to the same default color. They remain distinct tokens.

```text
authority.escalated
settlement.escalated
evidence.missing
attention.required
```

may all be amber while remaining separate semantic identities.

### Terminal capability fallback

The theme layer MUST support at least:

1. truecolor terminals;
2. reduced/256-color terminals;
3. monochrome or unreliable-color mode.

Consequential state must remain intelligible through glyph + label + emphasis even when color is absent.

Recommended semantic glyph family:

| Meaning | Preferred glyph |
|---|---|
| complete / accepted / allowed | `✓` |
| denied / failed / rejected | `×` |
| escalation / attention | `!` |
| pending / waiting | `○` |
| running / active | `●` |
| unknown / missing | `?` |
| stale / temporal issue | `◷` |
| proof / independent confirmation | `◆` |

Glyphs are renderer vocabulary, not canonical state.

---

## Typography Rules

### Principle

The terminal cannot reliably reproduce GodSpeed's web font stack, so Gauntlet preserves **typographic roles**, not font identity.

The cross-product roles remain:

```text
identity / ceremony
functional hierarchy
body clarity
evidence / machine values
```

OpenTUI lowers these roles into terminal emphasis: weight, dimming, underline, case, spacing, alignment, and structural framing.

### Semantic text roles

| Role | Purpose | Terminal projection |
|---|---|---|
| `text.environmentTitle` | Gauntlet / environment identity | bold, sparse, highest hierarchy |
| `text.workspaceTitle` | Case or Horizon title | bold |
| `text.focusTitle` | current Focus | bold; focus treatment when selected |
| `text.section` | section/group heading | bold or high contrast |
| `text.body` | normal readable prose | normal |
| `text.label` | field names, structured labels | medium emphasis; concise |
| `text.meta` | timestamps, counts, provenance context | dim/secondary |
| `text.machine` | IDs, hashes, paths, commands, code | terminal mono inherent; tabular alignment |
| `text.signal` | terse status/authority labels | uppercase permitted; restrained tracking analogue |

### Human vs machine values

Make the distinction visible through structure even though all terminal text is monospaced.

Preferred:

```text
Work Item    Retry safety
ID           T23
Reference    1d457ec9…
Verifier     verifier-02
Result       PASS
```

Do not visually flatten labels, prose, hashes, and statuses into one undifferentiated stream.

### Case and uppercase

ALL CAPS is reserved for:

- terse authority/status labels;
- proof/evidence object headers;
- very small technical section labels.

Do not render normal prose or long headings in all caps.

### Density

Typography and row density must adapt to the active posture and terminal width without changing meaning.

- Review/Journey: more vertical separation and explicit labels.
- Operational/Speed: tighter rows and more inline metadata.

---

## Component Stylings

### Component rule

React components are not semantic atoms. Components lower semantic UI objects into OpenTUI renderables.

The intended pipeline is:

```text
Gauntlet / Prime / DomainForge state
        ↓
WorkbenchModel
        ↓
Semantic Atomic UI
        ↓
React projection components
        ↓
OpenTUI renderables
        ↓
terminal cells
```

React MUST NOT become the authority for Case, Run, Reference, Evidence, Agent lifecycle, settlement, or affordance state.

### React application frame

Conceptual root composition:

```tsx
<GauntletEnvironment>
  <WorkspaceProvider>
    <WorkbenchShell>
      <Surface id="navigator">
        <ViewHost />
      </Surface>

      <Surface id="primary">
        <ViewHost />
      </Surface>

      <Surface id="inspector">
        <ViewHost />
      </Surface>

      <Surface id="bottom">
        <ViewHost />
      </Surface>

      <Surface id="composer">
        <ComposerView />
      </Surface>
    </WorkbenchShell>
  </WorkspaceProvider>
</GauntletEnvironment>
```

This is a lowering pattern, not a canonical component hierarchy.

### Surface styling

#### Navigator Surface

Purpose: answer **Where am I in the world?**

Contains:

- Case/Horizon outline;
- WorkItem and Run placements;
- saved Horizons;
- optionally Files, Agents, Attention placements.

Visual treatment:

- quiet structural surface;
- compact hierarchy;
- thin relation/indent guides;
- current Focus is clear but not glowing;
- cues are terse glyphs plus semantic color;
- same Focus in multiple locations uses a relation/alias indicator rather than duplicate visual identity.

#### Primary Surface

Purpose: host the primary question-oriented View.

Default Views:

- Work;
- Chat;
- Trace;
- Evidence;
- Agents;
- Files;
- Domain;
- Kernel;
- Harness;
- Attention.

Visual treatment depends on View but retains stable workspace framing.

#### Inspector Surface

Purpose: answer **What is relevant about the current Focus?**

Prefer structured sections:

```text
Identity
State
Relations
Authority
Evidence
Provenance
Affordances
```

Show only sections supported by the Focus kind and current state.

Avoid a giant property dump.

#### Bottom Surface

Purpose: supporting capability adjacent to current work.

Candidate Views:

- Terminal;
- Kernel;
- Agents;
- Evidence;
- Problems;
- Changes.

The Bottom Surface should be collapsible and must not permanently consume vertical space on small terminals.

#### Composer Surface

Purpose: universal operator-intent and required-input surface.

Composer variants:

```text
Default
Approval
Question
Recovery
```

Only one active composer variant occupies the primary input position at a time.

A Prime/Jupyter request for input or a Gauntlet approval condition may temporarily take over the composer. After resolution, normal composition returns.

#### Overlay Surface

Purpose: temporary navigation/action overlays such as the command palette.

Overlays use `surface.overlay`, explicit borders, minimal opacity tricks, and clear dismissal behavior.

### View grammar

A View answers a question rather than merely grouping widgets.

| View | Question | Primary Foci |
|---|---|---|
| Work | What are we trying to move? | WorkItem, Run, Evidence, Settlement |
| Chat | What was communicated? | Message, Turn, Agent |
| Trace | What actually happened? | Event, ToolCall, RoleInvocation, Verification |
| Evidence | What supports or contradicts settlement? | Evidence, Claim, Verification |
| Agents | Where is execution capacity allocated? | Agent, RoleInvocation |
| Files | What implementation artifacts are involved? | File, Diff, Artifact |
| Domain | What bounded world/model are we operating against? | DomainConcept, Relation, Policy |
| Kernel | What computation is currently alive? | KernelObject, Function, Result |
| Harness | What reusable agent capability has accumulated? | Skill, Memory, HarnessRevision |
| Attention | What genuinely requires operator resolution? | AttentionItem |

### Focus renderer registry

Do not implement one giant renderer per domain type.

Resolve rendering by Focus kind × View:

```text
FocusRef
   ↓
View
   ↓
renderer registry
```

Example conceptual registrations:

```text
work_item × outline   → WorkItemOutlineRenderer
work_item × work      → WorkItemWorkRenderer
work_item × inspector → WorkItemInspectorRenderer

evidence × outline   → EvidenceOutlineRenderer
evidence × evidence  → EvidenceRecordRenderer
evidence × inspector → EvidenceInspectorRenderer
```

A renderer is a projection. It MUST NOT own the underlying state.

### FocusPlacement

Every rendered occurrence of a Focus is a `FocusPlacement` or equivalent projection.

A WorkItem may appear under:

```text
Plan
Current Run
builder-03
Attention
Evidence
```

Selecting any placement resolves the same underlying Focus identity.

### Cues

Cues are resolved independently from Focus renderers.

Examples:

```text
running
blocked
verification_pending
new_evidence
authority_required
stale
uncertain
```

A single Cue may lower differently by View:

| View | `verification_pending` representation |
|---|---|
| Navigator | terse glyph on Focus row |
| Work | compact state banner/label |
| Inspector | reason + missing requirement |
| Attention | ranked AttentionItem relation |
| Status line | aggregate count only |

### Affordance presentation

Affordances MUST come from the Gauntlet affordance resolution path, not component-local conditionals.

Do not do this:

```tsx
if (work.status === "done") showSettleButton()
```

The intended pattern is:

```text
FocusRef
  ↓
AffordanceResolver
  ↓
materialized operations
  ↓
AffordanceBar / command palette / key binding / kernel wrapper
```

The same operation may have multiple projections:

| Projection | Example |
|---|---|
| TUI action | `[Run Verification]` |
| Command palette | `Verify T23` |
| Keyboard | centrally bound shortcut |
| Kernel | `aff.run_verification()` |
| CLI/API | canonical command projection |
| eligible Prime context | admitted operation capability |

All representations MUST converge on the same Gauntlet command/admission semantics.

### Affordance state

Do not show every impossible action as a disabled control by default.

Prefer Progressive Affordance Materialization:

- visible + legitimate → materialize;
- important but currently blocked and informative → materialize as blocked with reason;
- irrelevant/unreachable → do not clutter the View.

Example:

```text
Inspect Diff        ✓
Run Verification   ✓
Ask Critic         ✓
Settle             × verification absent
```

A blocked affordance must explain the blocking gate when inspected.

### Evidence objects

Evidence is a first-class visual object, not a green badge.

Preferred compact evidence record:

```text
┌─ VERIFICATION ─────────────────────────────
│ T23 · restart safety
│
│ claim       completed work will not rerun
│ verifier    verifier-02
│ result      PASS
│ evidence    EV-991
│ reference   1d457ec…
│ confirmed   independent
└────────────────────────────────────────────
```

Evidence objects should expose:

- claim/target;
- producer/verifier;
- result;
- evidence identity;
- reference/provenance where applicable;
- freshness or confirmation state where meaningful.

Do not imply that one passing check establishes capability unless Gauntlet state actually supports that claim.

### Authority records

Authority should be represented as a proof-bearing operational record.

Preferred compact shape:

```text
┌─ AUTHORITY ────────────────────────────────
│ actor       builder-03
│ operation   write
│ resource    src/runtime/**
│ decision    ALLOWED
│ basis       Case C17 / Run R17
└────────────────────────────────────────────
```

Allow/deny/escalate must be visually distinct and must not be conflated with execution success/failure.

### Trace rows

Trace is execution history, not merely conversation history.

Rows should fold related low-level pairs into semantic units where safe, while preserving provenance and inspectability.

Examples:

```text
Turn
├ Assistant
├ Tool
├ Subtool
├ Agent
├ Approval
├ Evidence
└ Result
```

Tool call/result may appear as one folded row, but raw provenance remains inspectable.

Trace presentation should support:

- chronological ordering;
- live follow;
- backward paging;
- folding;
- filtering;
- selected-row Inspector detail;
- distinction between Gauntlet and Prime/Jupyter event provenance.

### Agent rows

Agents represent allocated execution capacity, not chat rooms.

Preferred representation:

```text
builder-03      RUNNING
T23 retry implementation
parent: orchestrator
```

Inspector may expose:

- purpose;
- scope/context envelope;
- parent and children;
- model/session identity;
- usage/payment;
- messages;
- artifacts;
- result contract.

Do not infer completion from agent prose.

### Kernel objects

Kernel state is computational working state, not canonical truth.

Represent at least these maturity classes where available:

```text
EPHEMERAL
PINNED
ARTIFACTIZED
```

Possible actions:

```text
Inspect
Rename
Pin
Export Artifact
Attach to Focus
Offer as Evidence
```

`Offer as Evidence` is an affordance into Gauntlet's evidence admission path; it does not directly turn a Python value into evidence.

### Terminal

The embedded terminal should feel native, not like a decorative console widget.

Use Bun/OpenTUI native PTY/terminal capabilities where qualified by the implementation plan.

Terminal is a supporting Surface/View, not the canonical interaction metaphor for every operation.

### Code and diff

Use OpenTUI's native code/diff renderers before creating custom syntax highlighters.

Code/diff presentation should prioritize:

- line identity;
- changed region;
- provenance;
- relation to current Focus;
- evidence or verification relevance.

### Domain view

When DomainForge is available, domain/model content should be presented as another View over the same Workspace, not a separate application.

Selecting a DomainConcept should make related code, policy, evidence, events, and Foci reachable through explicit relations when those bindings exist.

No DomainForge capability should be fabricated when the integration is absent.

### Harness view

The Harness View exposes reusable Prime/Gauntlet developmental artifacts such as skills, prompt notes, memories, subagent specifications, and harness revisions.

Refinement UI must distinguish:

```text
candidate refinement
applied refinement
verified refinement
rolled-back refinement
```

A refinement is not proven merely because Prime can apply it.

### Plugin slots

Plugins contribute semantic capabilities through stable slots rather than arbitrary component insertion.

Preferred slots:

```text
root.nav
case.view
trace.row
inspector.section
artifact.viewer
composer
bottom.panel
status.item
overlay
```

A plugin may contribute:

- View;
- Focus renderer;
- Relation provider;
- Cue provider;
- Inspector section;
- artifact viewer;
- affordance projection.

Plugin unload must remove its registrations and return the host projection to baseline without leaving leaked subscriptions or UI state.

### Motion and state transitions

Motion is allowed only when it clarifies state.

Good terminal motion:

- restrained running indicator;
- route/progress update tied to real execution;
- brief evidence/authority state transition;
- controlled overlay enter/exit;
- live trace movement.

Avoid:

- animated borders;
- decorative spinners;
- constant pulsing;
- ASCII fireworks;
- “hacker” effects;
- animation competing with reading.

If motion is removed, the interface must remain fully understandable.

---

## Layout Principles

### Overall shell

Preferred wide-screen structure:

```text
┌ Gauntlet · Case C17 · Horizon T23 ─────────────────────────────────┐
│                                                                    │
│ NAVIGATOR       PRIMARY WORKSPACE                 INSPECTOR         │
│                 [WORK] [CHAT] [TRACE]                               │
│ Case                                                               │
│ └ Run R17       current Focus / View              identity          │
│   └ T23 ●                                           relations        │
│     ├ agents                                       evidence          │
│     ├ evidence                                     provenance        │
│     └ files                                        affordances       │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ TERMINAL │ KERNEL │ AGENTS │ EVIDENCE │ PROBLEMS │ CHANGES         │
├────────────────────────────────────────────────────────────────────┤
│ @T23  Have a critic challenge the retry path…                       │
└────────────────────────────────────────────────────────────────────┘
```

This is a layout projection, not a mandatory fixed grid at every width.

### Hierarchy of visual weight

Highest visual weight:

```text
current Focus
current consequence
current blocker
current legitimate next move
```

Medium visual weight:

```text
evidence
authority
relations
agents currently affecting Focus
```

Lower visual weight:

```text
metadata
IDs
timestamps
background history
inactive capacity
```

### Negative space

Negative space is an authority signal. Do not fill empty terminal cells merely because space exists.

Review/Journey posture should intentionally leave more empty area around evidence, approval, and settlement objects.

Operational/Speed posture may use denser rows while preserving clear grouping.

### Grid and spacing

Terminal spacing is measured in character cells, not pixels.

Use a small semantic spacing scale:

```text
space.0 = no separation
space.1 = one cell / one line micro-gap
space.2 = compact group separation
space.3 = normal section separation
space.4 = major region separation
```

Do not mechanically translate web pixel spacing into terminal cells.

### Row density

Use semantic density classes:

```text
compact   → traces, agent lists, file lists
standard  → work/navigation rows
review    → evidence, authority, settlement records
```

A density class describes information posture, not a fixed pixel height.

### Borders and linework

Use thin box-drawing lines and separators to create structure.

Preferred:

- single-line borders;
- subtle Prussian/slate structure;
- borders only where they clarify containment or authority/evidence object boundaries;
- route lines where relation/flow is meaningful.

Avoid:

- every region boxed simultaneously;
- double-line “retro terminal” framing everywhere;
- neon outlines;
- decorative ASCII texture.

### Relation visualization

The semantic hierarchy is a DAG, not necessarily a tree.

Use indentation for containment/hierarchy and explicit relation markers for alternate placements.

Example:

```text
CASE · Journey Integrity
▾ Direction
▾ Reference
▾ Plan
  ├ ✓ T20
  ├ ✓ T21
  └ ● T23 Trace projection
▾ Current Run R17
  └ ● builder-03
      └ ● T23 Trace projection ↗
▾ Attention
  └ ! T23 verification required ↗
▾ Evidence
  └ EV-991 ↗
```

The arrow indicates another placement of an existing identity, not a copy.

### Horizon and hoist

Hoisting narrows the visible working world.

```text
Case C17
   ↓ hoist
Run R17
   ↓ hoist
WorkItem T23
```

The breadcrumb/location line must make the current Horizon obvious and make de-hoisting cheap.

Hoist changes representation and context construction; it does not delete or rewrite the rest of the Case.

### Saved Horizons

Saved Horizons are reusable projections such as:

```text
Main
Current Work
Verification
Agents
Research
Attention
Release
```

They are not separate copies of the Case.

### Semantic location / route-like addressing

The TUI should use a renderer-independent `SemanticLocation` (or equivalent) rather than React Router as the conceptual navigation authority.

Conceptual shape:

```ts
type SemanticLocation = {
  workspace: { kind: "case"; id: string }
  horizon?: FocusRef
  surface?: SurfaceId
  view?: ViewId
  focus?: FocusRef
}
```

Example serialized locations:

```text
gauntlet://cases
gauntlet://case/C17
gauntlet://case/C17/work/T23
gauntlet://case/C17/run/R17
gauntlet://case/C17?view=trace
gauntlet://case/C17/work/T23?focus=evidence:EV991
gauntlet://horizon/verification?case=C17
```

A semantic location identifies a reproducible projection/horizon. It does not create canonical domain state.

### React implementation structure

Prefer semantic responsibility over generic “pages/components” organization.

```text
src/tui/
├── environment/
├── workspace/
├── surfaces/
├── views/
│   ├── work/
│   ├── chat/
│   ├── trace/
│   ├── evidence/
│   ├── agents/
│   ├── files/
│   ├── domain/
│   ├── kernel/
│   ├── harness/
│   └── attention/
├── focus/
├── cues/
├── relations/
├── affordances/
├── attention/
├── composer/
├── navigation/
├── plugins/
└── opentui/
```

Avoid generic web conventions that obscure the semantic model:

```text
pages/
components/atoms/
components/molecules/
components/organisms/
```

unless repository constraints require them as purely mechanical folders.

### React state boundary

React consumes WorkbenchModel snapshots/read models. It must not become the owner of streaming business state.

Preferred conceptual flow:

```text
Gauntlet / Prime / DomainForge
         ↓
Bun workbench adapters + local projection model
         ↓
immutable/read-model snapshots
         ↓
React
         ↓
OpenTUI
```

The exact store/subscription library is implementation-defined. The design invariant is that React rendering state is replaceable and reconstructible.

### Progressive data adjacency

The current Horizon should shape what becomes locally adjacent:

```text
CognitiveOS horizon
        ↓
TUI Horizon
        ↓
subscription horizon
        ↓
query/read-model horizon
        ↓
cache horizon
        ↓
Prime context horizon
```

Selecting T23 should not require eagerly loading the entire Case/repository into every View.

---

## Depth & Elevation

### Principle

Terminal depth comes from hierarchy, contrast, containment, and z-order—not shadows or pseudo-3D effects.

### Depth levels

| Level | Use | Treatment |
|---|---|---|
| Workspace | global field | `surface.workspace` |
| Panel | normal Surface | `surface.panel` |
| Elevated | selected/active inspector or focused panel | `surface.elevated` + structural edge |
| Overlay | command palette/modal/composer takeover | `surface.overlay` + explicit border |
| Code | terminal/code/diff/kernel content | `surface.code` |

### Focus vs elevation

Do not use elevation as a substitute for Focus.

The primary Focus should have a clear semantic focus treatment. An elevated panel may contain the Focus but is not itself necessarily focused.

### Evidence/authority containment

Evidence and authority records may use restrained boxes because containment communicates record identity and provenance.

Do not render every WorkItem as a card. The product should not become a tile dashboard.

### Overlays

Overlays should:

- block or capture input only when the interaction requires it;
- preserve context around the overlay where useful;
- have an obvious exit/cancel path;
- never conceal an irreversible consequence behind ambiguous controls.

### Glass, blur, gradients

Not applicable as primary terminal techniques.

Do not imitate web glassmorphism with noisy character shading or gradients. Preserve the underlying hierarchy through terminal-native contrast and linework.

---

## Do's and Don'ts

### Do

- Make the current Focus obvious.
- Make consequential state readable without color.
- Name visual tokens by system meaning.
- Use Prussian Blue for structural identity, not generic status.
- Reserve gold for proof and sanctioned authority.
- Show evidence as inspectable objects, not badges alone.
- Distinguish authority, execution, evidence, settlement, and capability visually even when they share colors.
- Let the current Horizon reduce visible and computational clutter.
- Keep Chat as one View among several, never the primary product metaphor.
- Treat Trace as a projection of execution history.
- Show Agent state as execution capacity, not chat presence.
- Keep operator interruption driven by AttentionItems rather than raw agent chatter.
- Let affordances materialize from Gauntlet state.
- Provide the reason when an important action is blocked.
- Make provenance reachable from evidence, authority, and settlement objects.
- Use OpenTUI native code/diff/terminal capabilities before reimplementing them.
- Preserve renderer replaceability.
- Keep plugin contributions semantically scoped and unloadable.
- Prefer calm operational clarity over spectacle.

### Don't

- Do not create generic SaaS cards for every object.
- Do not create vanity dashboards or fake metrics.
- Do not use neon green/cyan “hacker” styling.
- Do not use gradients, glows, particle effects, or animated ASCII decoration.
- Do not make gold a generic accent.
- Do not let green mean all of “allowed,” “executed,” “verified,” “settled,” and “capable” without labels/state identity.
- Do not let React components infer settlement or authority locally.
- Do not let a mailbox message or agent sentence visually imply completion.
- Do not duplicate Focus state for each placement.
- Do not make filesystem hierarchy the semantic navigation model.
- Do not expose every installed tool as a permanent button/tab.
- Do not show unavailable actions merely to demonstrate product capability.
- Do not make React Router the canonical state/navigation model.
- Do not create multiple implementations of the same affordance across UI/kernel/CLI/agent surfaces.
- Do not turn Jupyter/IPython outputs directly into evidence without admission.
- Do not make the terminal panel the only way to operate Gauntlet.
- Do not create a second Prime agent model in the UI.
- Do not hide failure behind automatic fallback.
- Do not overload the operator with every event from recursive agents.
- Do not use a visual treatment that would fit unchanged on a generic AI startup dashboard.

### Banned visual motifs

For consistency with the GodSpeed Visual Language, never introduce:

- glowing AI brains;
- humanoid robots;
- generic node clouds;
- cyberpunk neon;
- crypto/gaming aesthetics;
- gears/cogs as automation metaphors;
- generic AI sparkles;
- rocket/explosion imagery;
- decorative “AI magic” iconography.

### Semantic anti-patterns

#### Generic status token

Bad:

```text
status.success
status.warning
status.error
```

when the domain state is actually known.

Prefer:

```text
authority.allowed
evidence.failed
settlement.rejected
capability.degraded
```

Generic fallback tokens may exist only where domain semantics are truly unknown.

#### Local component authority

Bad:

```tsx
if (testsPassed) <Button>Settle</Button>
```

Good:

```text
Gauntlet resolves Settle as an affordance
→ UI projects it
```

#### Conversation-as-history

Bad:

```text
Chat transcript = what happened
```

Good:

```text
Durable execution/event history
→ Chat projection
→ Trace projection
```

#### Installed capability-as-affordance

Bad:

```text
RealityTrace installed
→ always show Compare Reality
```

Good:

```text
capability installed
+ relevant representation
+ gates satisfied
→ Compare Reality materializes
```

---

## Responsive Behavior

### Principle

“Responsive” in Gauntlet means adapting to terminal geometry, capability, transport, and cognitive load while preserving semantic identity and interaction reachability.

The same Workspace must remain usable locally, over SSH, and in constrained terminal windows.

### Width classes

These are design defaults and may be tuned by evidence without changing the semantic model.

#### Wide: `>= 140 columns`

Preferred layout:

```text
Navigator | Primary | Inspector
Bottom Surface
Composer
```

- Navigator and Inspector may remain visible simultaneously.
- Primary may show richer inline evidence/relations.
- Operational posture can use denser metadata.

#### Standard: `100–139 columns`

Preferred layout:

```text
Navigator | Primary
Inspector as toggle/sidecar
Bottom Surface
Composer
```

- Inspector is demand-driven.
- Metadata moves from inline to Inspector when necessary.

#### Compact: `80–99 columns`

Preferred layout:

```text
Primary
Navigator / Inspector as switchable side surfaces
Bottom Surface collapsed by default
Composer
```

- Preserve breadcrumbs/Horizon at all times.
- Replace wide tab strips with compact selector/command palette where needed.
- Favor one primary perceptual task at a time.

#### Narrow: `< 80 columns`

The TUI MUST degrade intentionally rather than crush the full dashboard into unreadable columns.

Preferred behavior:

```text
single Surface/View
persistent semantic location
command palette for navigation
overlay Inspector
compact composer
```

If a View cannot be made usable at the current dimensions, report the constraint explicitly and offer a reachable alternative rather than rendering corrupted layout.

### Height constraints

Low-height terminals should collapse the Bottom Surface first, then reduce nonessential metadata, while preserving:

- current Focus;
- current View;
- Horizon/location;
- Attention cue;
- composer or required approval input;
- current consequential status.

### Color capability

Theme lowering must detect or configure terminal color capability.

Do not assume truecolor over SSH.

Fallback behavior must preserve:

```text
glyph
label
emphasis
ordering
```

when colors degrade.

### Input modality

Keyboard is first-class. Mouse support is additive.

Every consequential operation must be reachable without a mouse.

Exact key bindings are centrally configured and implementation-defined. Components must bind semantic actions, not hardcode independent shortcuts.

### Reduced motion

A reduced-motion mode must suppress nonessential animation while retaining state-transition clarity.

### Reconnect / resume

After TUI restart or reconnection:

- restore/reconstruct the Workspace projection;
- restore the semantic location when valid;
- reattach to authorized long-running Prime work;
- do not imply that work stopped merely because the TUI disappeared;
- reject stale-generation updates from prior connections;
- surface any reconstruction discrepancy explicitly.

The UI should feel resumable, not session-fragile.

### Headless equivalence

No design behavior may require the TUI to exist for Gauntlet's canonical execution or settlement semantics to remain valid.

The workbench is a projection. The projection can disappear.

---

## Agent Prompt Guide

### Mandatory reading order

Before implementing or materially changing Gauntlet TUI code, an agent should read:

1. `gauntlet-tui-prime-workbench.spec.yaml` for normative behavioral requirements;
2. this `DESIGN.md` for TUI visual and interaction design;
3. the current task from `gauntlet-tui-prime-workbench.plan.yaml`;
4. repository-native implementation context and existing components;
5. the GodSpeed visual-language source when a design question is not resolved here.

The plan does not override the spec or this design contract.

### Core prompt

When generating or modifying Gauntlet TUI UI, follow this operating frame:

> Build a terminal-native operational control plane, not a generic dashboard or AI chat application. Begin from the current semantic Focus, View, Horizon, and materialized Gauntlet affordances. Preserve identity across placements. Present authority, execution, evidence, settlement, capability, and attention as distinct semantic states. Use the GodSpeed visual language: restraint, precision, evidence objects, authority paths, control aesthetics, Prussian structure, near-black field, and scarce gold proof signals. React/OpenTUI are rendering projections only; do not move canonical state or admission logic into components. Prefer native OpenTUI/Bun capabilities before adding overlapping UI infrastructure. If a visual decision is not covered, choose the option that makes consequence, proof, provenance, or the next spendable move clearer with the least visual noise.

### Agent decision procedure

For every new UI element, ask in this order:

```text
1. What semantic thing is this?
   Focus, Cue, Relation, Affordance, Attention, Flow, or renderer-only primitive?

2. Which View question does it answer?

3. Does it already have a stable identity elsewhere?
   If yes, render a placement/reference; do not copy state.

4. What is the source of truth?
   Gauntlet, Prime, DomainForge, kernel, artifact store, or projection state?

5. Is this state consequential?
   If yes, do not infer it locally in React.

6. What semantic visual token describes the state?
   Do not choose a raw color first.

7. Does this need operator attention or merely visibility?
   Do not interrupt for ordinary stream activity.

8. Is the proposed action an actual current affordance?
   Installed capability alone is insufficient.

9. Can the same behavior work at narrow terminal width and without color?

10. Is an OpenTUI/Bun native primitive already sufficient?
```

### Component authoring rules

- Components SHOULD receive `FocusRef`, projection/read-model data, or semantic props rather than canonical mutable objects.
- Components MUST NOT make independent authority or settlement decisions.
- Components MUST use semantic theme tokens; avoid raw color literals outside theme/token adapters and tests.
- Components SHOULD render states with glyph + label + color where consequence matters.
- Components SHOULD remain small around one semantic responsibility.
- View components MAY orchestrate multiple renderers but should not become state stores.
- Renderer registries SHOULD resolve Focus kind × View rather than large type-switch components.
- Plugin-provided renderers MUST register/unregister cleanly.
- Native OpenTUI Code, Diff, Markdown, ScrollBox, terminal, input, focus, and layout primitives SHOULD be qualified before custom alternatives.

### Navigation authoring rules

- Navigation operates on `SemanticLocation`/Horizon concepts, not file paths or React route objects as the primary model.
- Hoist/de-hoist must preserve underlying Case identity.
- Back/forward history should reproduce perceptual locations without mutating domain state.
- Deep links/bookmarks should resolve to semantic locations when referenced Foci still exist.
- Broken/stale locations should fail visibly and recover to the nearest valid parent Horizon.

### Affordance authoring rules

- Never add a button merely because an underlying API exists.
- Never implement the same operation independently in UI and CLI/kernel/Prime surfaces.
- Important blocked actions may appear with a reason; irrelevant impossible actions should remain absent.
- Destructive/irreversible actions require explicit consequence framing and the governing Gauntlet authorization path.
- A visual “success” state must name what succeeded: execution, verification, settlement, or capability.

### Evidence and authority authoring rules

- Prefer record-like proof objects over decorative badges.
- Preserve provenance and allow inspection.
- Do not erase contradictory evidence when newer evidence passes.
- Do not use gold unless the state carries proof/sanctioned-authority meaning.
- Do not present an agent message as proof of completion.

### Attention authoring rules

- Raw agent messages and events do not automatically become operator interruptions.
- Canonical or derived AttentionItems generate Cues and may cause composer takeover.
- Composer takeover must be reversible and return to the prior interaction context.
- Rank attention by consequence, not recency alone.

### Terminal authoring rules

- Do not imitate browser UI unnecessarily.
- Do not fake shadows, glass, or animation with ASCII noise.
- Favor thin structural lines, semantic grouping, readable density, and direct interaction.
- Preserve usability under SSH and reduced color.
- Do not assume fonts beyond a competent monospaced terminal font.

### Visual acceptance check

Before considering a screen/view complete, answer all of these:

1. Is the current Focus unmistakable?
2. Can the operator tell what changed and what is merely selected?
3. Are authority, execution, evidence, settlement, and capability distinguishable?
4. Is proof inspectable rather than decorative?
5. Are blocked consequential actions explained?
6. Is gold carrying proof/authority rather than ornament?
7. Does the screen still work without color?
8. Does it remain coherent at the required terminal width?
9. Is Chat subordinate to the actual work rather than becoming the product shell?
10. Does the layout expose real operational state rather than status theater?
11. Could the same semantic View be rendered by another frontend without changing its meaning?
12. Has any React/OpenTUI component accidentally become a source of canonical truth?
13. Is there any element that would fit unchanged on a generic AI startup dashboard?
14. Is there an existing Bun/OpenTUI/Prime/Gauntlet capability being needlessly reimplemented?

If 11 or 12 is false/yes respectively, the design boundary is wrong even if the screen looks polished.

### Implementation mapping quick reference

| Semantic UI | React/OpenTUI role | Example |
|---|---|---|
| Environment | root providers/adapters | whole Gauntlet operating world |
| Workspace | workspace provider + WorkbenchModel scope | Case C17 |
| Surface | stable layout host | Navigator, Primary, Inspector |
| View | question-oriented projection | Work, Trace, Evidence |
| Focus | identity-aware projection | WorkItem T23 |
| FocusPlacement | one rendered occurrence | T23 under Agent and Plan |
| Cue | semantic decorator/signal | verification pending |
| Relation | graph/query binding | T23 `executed-by` builder-03 |
| Affordance | canonical-operation renderer | Run Verification |
| Attention | operator-state projection | primary/contextual/latent |
| AttentionItem | canonical Focus | approval required |
| Flow | event/navigation relation | attempt → evidence → settlement |
| Horizon | current bounded world | hoisted T23 |
| SemanticLocation | reproducible navigation projection | `gauntlet://case/C17/work/T23` |

### Visual token quick reference

```text
STRUCTURE
brand.prussian
surface.workspace
surface.panel
surface.elevated
surface.overlay
surface.code

TEXT
fg.primary
fg.secondary
fg.tertiary

SEMANTIC STATE
authority.*
execution.*
settlement.*
evidence.*
readiness.*
integrity.*
capability.*
focus.*
attention.required
danger

SCARCE PROOF
brand.gold
```

### Final design formula

```text
Gauntlet TUI =
semantic Atomic UI
+ GodSpeed visual language
+ Focus-centered perceptual coincidence
+ evidence-bearing operational objects
+ isomorphic affordances
+ terminal-native restraint
+ OpenTUI/Bun pragmatic leverage
```

Or shorter:

> **Current world visible. Consequence legible. Proof adjacent. Next move spendable.**
