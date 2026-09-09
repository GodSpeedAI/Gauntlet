# Gauntlet Prime-Native Terminal Workbench review

Date: 2026-09-07
Disposition: approved design revision; implementation and runtime conformance remain open.

This review inspected the workbench specification and plan against the repository's
Core spec, frozen ruler, current OperatorSurface/JSON surface, CLI operator-action path,
Prime runtime documentation, and checked-out DomainForge and Reality Trace (SXR)
implementations. The working tree was already dirty; unrelated task26 changes were
preserved and are not treated as evidence for this design.

The original draft was not implementation-ready. The material defects were corrected:

1. It referred to `.agents/spec/` instead of `.agents/specs/`, and its source hash was
   stale. The plan now binds the correct approved spec revision and records the current
   hash.
2. It treated the existing seven-route, feature-gated loopback JSON surface as a general
   event/session API. The revised plan makes baseline/watermark, capability handshake,
   durable invocation ownership, bounded synchronization and any new mutation contract
   explicit implementation work. Existing `execute_operator_action` remains the owner
   for resolve/retry semantics.
3. It allowed canonical ownership language for Case, WorkItem and AttentionItem to
   conflict with the intended projection boundary. They are now explicitly workbench
   projections; Reference/Run/Unit/evidence/escalation/settlement remain Gauntlet-owned,
   and Prime owns its input/session state.
4. It assumed Jupyter socket semantics. The current Prime runtime evidence describes a
   persistent Python REPL with serialized execution and JSON-lines stdio host requests;
   the plan now requires a version-qualified host adapter and forbids a second kernel
   client.
5. It assumed DomainForge and SXR compose. The checked-out DomainForge envelope CLI and
   SXR Slice 0B CEP consumer currently have incompatible command/schema contracts. The
   revised spec makes compatible producer/consumer qualification an explicit blocked
   milestone and keeps semantic enrichment optional. SXR evidence is source-qualified,
   provenance-preserving, and never imported as Gauntlet settlement.
6. The plan had no early testable UX journey and its first journey promised evidence and
   denial details unavailable from the current HTTP surface. T00A now proves only the
   metadata/reference journey supported by existing views, labels absent fields, and
   reserves full explanations for T01's explicit read contract. T12 adds real-PTY
   keyboard, resize, attention, draft, stale-reply, scroll and terminal-restoration
   journeys.
7. Bootstrap preflight was ambiguous and could require future capabilities before they
   were built. `tui-preflight bootstrap` and `tui-preflight conformance` now have separate
   contracts; missing required future capability blocks only the later claim. Gate
   placeholders and empty selectors cannot report green.
8. Synchronization and operation races were underspecified. The revised contracts cover
   source epochs, inclusive cursors, gaps, retention, opaque cursor handling, idempotency
   payload binding, outcome-unknown reconciliation, concurrent escalation answers,
   admission-to-dispatch loss, and bounded queues/output/deadlines.

Useful sibling capabilities are deliberately incorporated at their real boundaries:
DomainForge declarations, pack/scope/namespace and digest-qualified semantic envelopes;
SXR committed ledger/evidence/history/question/threshold provenance; and no invented
graph or explanation service. SXR query/context side effects and placeholder `explain`
behavior are explicitly excluded from render-time or causal claims.

The revised plan contains 28 dependency-ordered tasks, including T00A (early read-only
rendered journey) and T14B (qualified SXR/semantic composition). It maps all 104 indexed
spec requirement IDs to tasks and adds section-path clause ownership so ID counting alone
cannot masquerade as coverage. The plan's fresh critic/verifier contract excludes builder
identity, reasoning, history and originating conversation; only the orchestrator may
settle the whole plan.

Validation performed:

- Both YAML documents parse successfully.
- Spec status is `approved`; plan status is `approved` for design execution.
- Plan source hash equals the current spec SHA-256.
- Dependency graph was regenerated from task prerequisites; no cycle or unknown
  dependency was found.
- Requirement count and mapping both equal 104; unmapped IDs are empty.
- `just tui-*` gates are proposed contracts only; implementation has not begun, so no
  runtime or test pass is claimed.

Known limits remain intentionally visible: no TUI implementation exists yet, OpenTUI and
Prime target qualification is pending, checked-out DomainForge/SXR composition is not
currently compatible, no performance threshold has been measured, and the existing dirty
worktree prevents treating a repository-wide gate as a clean baseline. These are open
implementation conditions, not silently resolved assumptions.

Document hashes at review completion:

```text
spec  .agents/specs/gauntlet-tui-prime-workbench.spec.yaml  6f1cac0b2b6e88149dc2ed12629a718f73948935e847c10f30018665f5b62c7b
plan  .agents/plans/gauntlet-tui-prime-workbench.plan.yaml  70e0f39b8f75a815032b08966a1dc7d2cb6bc5eadc2954835b5d973051196964
```
