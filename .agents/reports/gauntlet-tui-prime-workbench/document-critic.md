# Workbench document critic

Review contract: bounded, independent read-only examination of the proposed specification and plan for internal contradictions, implementability, source ownership, operation journeys and gates. Target documents are not edited. Findings concern the current document contents; no implementation or settlement verdict is claimed.

## Findings recorded during review

### D1 — Conflicting canonical ownership of navigation and input projections (material)

The specification explicitly defines Case, WorkItem and AttentionItem as projections at `.agents/specs/gauntlet-tui-prime-workbench.spec.yaml:677`, `:682` and `:683`, and makes Prime input requests separately owned. But `core_domain_model.scope_rule` calls all three canonical Gauntlet entities at `:694`; REQ-STATE-001 requires Gauntlet authority over Case and AttentionItem at `:1314`; the Core component claims canonical Case ownership at `:557`. The plan repeats the canonical Case/attention claim at `.agents/plans/gauntlet-tui-prime-workbench.plan.yaml:677` and `:685` while expressly rejecting new canonical aggregates at `:701`.

This is not merely terminology: an implementer following the canonicality test may persist Prime input or navigation lifecycle as new Gauntlet truth, contradicting the intended boundary. Existing `crates/gauntlet-ports/src/operator_surface.rs:53` projects RunId, TicketId, InvocationId and UnitSnapshot; `:44` defines UnitSnapshot. Core specification `:459` describes the OperatorSurface port and contains no canonical Case/WorkItem/AttentionItem definitions.

Correction: consistently distinguish canonical Reference/Run/Unit/Ticket/evidence state from workbench Case/WorkItem/AttentionItem projections and Prime-owned input request state. Rewrite the contradictory normative clauses and plan proof/preregistration claims, rather than expecting vocabulary-binding precedence to repair them implicitly.

### D2 — T00 gate contract can require later capabilities before they can be built (material)

At `.agents/plans/gauntlet-tui-prime-workbench.plan.yaml:142`, unqualified `tui-preflight` verifies actual required tests and configured source compatibility. At `:124` missing artifacts/skipped required integrations must fail. Yet T00 requires that same unqualified preflight and all narrow gates to exit zero at `:608` and `:619`, before T00A implements the first renderer journey and T01+ implement the new protocols. The prose at `:124` says preflight distinguishes document-integrity from runtime qualification, but the declared recipe signature and T00 invocation supply no mode or milestone scope. The plan does expressly distinguish default implemented suites from `all` for tui-test at `:136`, so preflight needs an equally explicit contract.

Correction: declare explicit document/bootstrap and runtime/full-conformance preflight modes, including required artifact sets and failure semantics. Bind T00 to bootstrap checks plus qualification probes for available runtimes, and bind final settlement to complete runtime conformance. A missing future integration must block its capability claim without making the bootstrap milestone impossible or silently skipping a required global check.

### D3 — Early journey needs an explicit source contract for explanations/evidence (material planning gap)

T00A claims a cold operator can inspect a Unit and find evidence/denial reasons using the actual HTTP surface (`.agents/plans/gauntlet-tui-prime-workbench.plan.yaml:633`, `:635`, `:644`). It precedes T01's read-boundary work (`:661`). The source surface is narrower: `crates/gauntlet-ports/src/operator_surface.rs:44` offers only Unit id/status/tier, and `crates/adapters/gauntlet-adapter-surface-json/src/views.rs:170` offers UnitSnapshot, verification facts and settlement existence. Verification facts carry evidence references (`views.rs:24`), not an evidence-content endpoint. The existing route table at `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:269` has state/run/unit/escalated reads and refresh/resolve/retry only.

Global sandbox denial strings and evidence references can be displayed, but they do not establish a selected Unit's current unavailable-action explanation or the evidence-content journey required at spec `:1301`. No T00A step identifies another authoritative read source or permits the minimal upstream read extension needed to supply missing fields. A builder could otherwise invent explanations from status or secretly widen this preview into T01.

Correction: explicitly bind the preview's evidence view to verification metadata/references if that is the intended early scope, label unavailable explanatory fields as absent, and reserve full evidence/requirement/denial inspection for the later journey. Alternatively authorize a minimal canonical read extension as a distinct prerequisite slice before T00A, with exact source owners and no inferred denial logic. Keep the final full journey requirement intact.

## Final bounded verdict

Changes requested for D1–D3 before calling the documents implementation-ready. D1 is a normative ownership contradiction; D2 is an ambiguous bootstrap/closure gate; D3 is a missing data-source/dependency contract for the first UX milestone.

Scope examined: current specification sections covering substrate claims, architecture/domain identity, operations, UX, synchronization, lifecycle, security, reconstruction and evidence; plan gates, early milestones, task/dependency excerpts and terminal/kernel integration; Core vocabulary/OperatorSurface clauses; concrete OperatorSurface and JSON surface definitions/routes. No target documents or implementation source were changed. No compilation, runtime integration, external protocol qualification, complete line-by-line 5,351-line review, or exhaustive DAG/requirement validation was performed. External substrate claims remain subject to the plan's pinned-version qualification. These findings are a bounded document review, not a universal design-quality claim or settlement decision.
