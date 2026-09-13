/**
 * Planned Prime operation contracts (plan Phase C1,
 * workbench-intent-closure.md §4; REQ-OPS-003, D7).
 *
 * REQ-OPS-003 requires a versioned capability handshake that distinguishes
 * EXISTING operations (today's `operation-registry.ts` — read/refresh/
 * resolve/retry, already bound to canonical admission) from PLANNED ones
 * (launch, steering, cancellation, evidence admission, session operations)
 * the canonical surface does not yet serve. Every planned operation MUST
 * carry the full nine-field contract — application owner, allowed actors,
 * typed inputs/results, preconditions, authority, containment, budget,
 * idempotency, evidence contract — BEFORE any binding work starts (Phase C2).
 *
 * This module is declaration only. Nothing here is invocable:
 * `PLANNED_OPERATION_REGISTRY` is a SEPARATE catalog from
 * `operation-registry.ts`'s `OPERATION_REGISTRY` — a planned id is never
 * added to `OPERATION_IDS`/`OPERATION_REGISTRY` (that catalog is pinned by
 * `operation-isomorphism.test.ts` to its exact three real entries), so
 * `isOperationId`/`lookupOperation` (operation-registry.ts) and therefore
 * `resolveAffordance` (affordance.ts) treat every planned id exactly like any
 * other unregistered operation: `unsupported_operation`, never `available`.
 * REQ-OPS-003's "missing operations render unsupported, never a
 * shell-command fallback" (D7) stays true after this file exists.
 *
 * This is a canonical state-layer module (renderer-boundary.test.ts,
 * DSN-011/REQ-TUI-005): it must never import presentation, so
 * `ComposerModeName` below MIRRORS `composer.ts`'s `ComposerMode` literal
 * union by value rather than importing it — importing `composer.ts`, even
 * type-only, transitively taints this file as presentation (T19's BFS
 * follows every import edge, not just runtime ones; caught RED the first
 * time this file imported `ComposerMode` directly).
 */

import type { OperationId } from "./operation-registry";
import { operationIds } from "./operation-registry";

/** Mirrors composer.ts's `ComposerMode` values — see the file-level note above. */
export type ComposerModeName = "message" | "operation" | "prime" | "kernel";

export const PLANNED_OPERATION_IDS = {
  PRIME_LAUNCH: "prime.launch",
  PRIME_MESSAGE: "prime.message",
  PRIME_STEER: "prime.steer",
  PRIME_CANCEL: "prime.cancel",
  EVIDENCE_ADMIT: "evidence.admit",
  PRIME_SESSION: "prime.session",
} as const;

export type PlannedOperationId = (typeof PLANNED_OPERATION_IDS)[keyof typeof PLANNED_OPERATION_IDS];

/**
 * The nine REQ-OPS-003 fields every planned operation must name before
 * implementation. `io_contract` carries "typed inputs/results" as the single
 * comma-separated item REQ-OPS-003 names it as. `id`/`composer_mode` are
 * registry plumbing (identity + today's reachable composer surface, `null`
 * when the operation has no composer entry point yet), not part of the
 * required nine — `PLANNED_CONTRACT_FIELDS` below names exactly the nine.
 */
export interface PlannedOperationContract {
  readonly id: PlannedOperationId;
  readonly composer_mode: ComposerModeName | null;
  readonly owner: string;
  readonly actors: string;
  readonly io_contract: string;
  readonly preconditions: string;
  readonly authority: string;
  readonly containment: string;
  readonly budget: string;
  readonly idempotency: string;
  readonly evidence_contract: string;
  /**
   * Whether the contract declared here is actually bound to a live code path
   * (plan workbench-agent-interaction.md, increment P1) or still awaiting
   * implementation. This is deliberately NOT the same thing as
   * `isOperationId`/`OPERATION_REGISTRY` membership: Prime admission
   * (prime.launch/prime.message/prime.cancel) is a SEPARATE admission path
   * from Gauntlet's three canonical operations, and that registry stays
   * pinned to exactly its three entries (operation-isomorphism.test.ts).
   * `bound: true` only means "an operator submission through this
   * composer mode reaches real, tested behavior today" — never that the
   * canonical surface now serves it.
   */
  readonly bound: boolean;
}

/** The nine required field names, in REQ-OPS-003's own order. */
export const PLANNED_CONTRACT_FIELDS = Object.freeze([
  "owner",
  "actors",
  "io_contract",
  "preconditions",
  "authority",
  "containment",
  "budget",
  "idempotency",
  "evidence_contract",
] as const satisfies readonly (keyof PlannedOperationContract)[]);

/**
 * The frozen catalog. Every value is grounded in an existing REQ-PRIME-*
 * requirement or an already-built mechanism (PrimeDelegationDispatcher,
 * KernelInputBroker, substrate-inventory.ts) — nothing here is invented for
 * this file; it restates what Phase C2 must bind, not new policy.
 */
export const PLANNED_OPERATION_REGISTRY: Readonly<
  Record<PlannedOperationId, PlannedOperationContract>
> = Object.freeze({
  [PLANNED_OPERATION_IDS.PRIME_LAUNCH]: Object.freeze({
    id: PLANNED_OPERATION_IDS.PRIME_LAUNCH,
    composer_mode: "prime",
    owner:
      "PrimeDelegationDispatcher (prime-recursion.ts) driving a production RealPrimeHost adapter over Bun.spawn (plan Phase C2)",
    actors:
      "an authenticated session actor with settlement access to the target Case; no anonymous or unauthenticated launch",
    io_contract:
      "input: {focus: FocusRef, goal: string, budget_estimate: PaymentEstimate}; result: {child_session_id: string, receipt: DelegationReceipt} — a spawn/admission handle is never presented as the child result or completion (REQ-PRIME-003)",
    preconditions:
      "canonical admission for the target unit resolves available; Prime capability observed installed AND reachable (installed:true, available:true — D6, NONGOAL-004)",
    authority:
      "the ONE canonical Gauntlet admission path decides; PrimeDelegationDispatcher only sequences receipt-before-spawn, it never authorizes on its own",
    containment:
      "Bun.spawn child-process containment — NOT a security sandbox (NONGOAL-006); the kernel/child may compute but is never an admission path (REQ-OUT-004)",
    budget:
      "token/usd_micro estimate evaluated against AffordanceObservation.budget_remaining before admission, the same gate every consequential operation passes through",
    idempotency:
      "one child per (focus, idempotency_key); a second concurrent submit under the same child_session_id spawns nothing and is refused with zero side effects (C2 tooth 2)",
    evidence_contract:
      "the accepted receipt is the evidence row; a schema-valid result contract at a canonical locator is required before any mailbox 'done' can settle anything (REQ-PRIME-005, REQ-SAFE-004)",
    bound: true,
  }),
  [PLANNED_OPERATION_IDS.PRIME_MESSAGE]: Object.freeze({
    id: PLANNED_OPERATION_IDS.PRIME_MESSAGE,
    composer_mode: "message",
    owner: "Prime mailbox delivery (REQ-PRIME-005), addressed to a live child session's mailbox",
    actors: "an authenticated session actor whose focus names a live, admitted child session",
    io_contract:
      "input: {focus: FocusRef, child_session_id: string, body: string}; result: {delivered: boolean, mailbox_ref: string} — delivery is communication, never a completion signal",
    preconditions:
      "a live child session recorded under this focus (from prime.launch); mailbox reachable",
    authority:
      "canonical admission decides whether delivery is permitted; message content MAY update Chat/Trace/attention projections but MUST NOT alone establish Gauntlet completion (REQ-PRIME-005)",
    containment:
      "process containment via the same child boundary prime.launch established; not a sandbox",
    budget:
      "token estimate for message delivery, evaluated the same as any consequential operation",
    idempotency:
      "delivery is at-least-once with a deterministic key; a resend under the same key does not duplicate the mailbox entry",
    evidence_contract:
      "message content alone is never evidence; only an explicit, schema-valid result contract at a canonical locator promotes to settlement (REQ-DATA-005 keeps Chat and Trace separate projections)",
    bound: true,
  }),
  [PLANNED_OPERATION_IDS.PRIME_STEER]: Object.freeze({
    id: PLANNED_OPERATION_IDS.PRIME_STEER,
    composer_mode: "kernel",
    owner:
      "the Prime-owned Python REPL mediation path via KernelInputBroker (kernel-affordances.ts)",
    actors:
      "an authenticated session actor with settlement access; REQ-PRIME-004 forbids concurrent shell executions against one single-threaded REPL",
    io_contract:
      "input: {focus: FocusRef, control_input: string}; result: {kernel_object_ref: string, classification: 'ephemeral'|'pinned'|'artifactized'}",
    preconditions:
      "a live prime.launch session behind this focus; the REPL not already mid-turn (serialized, REQ-PRIME-004)",
    authority:
      "canonical admission decides every consequential action; the kernel compute path itself can never bypass a Gauntlet denial (REQ-OUT-004)",
    containment:
      "process containment only — NONGOAL-006 forbids describing this as a sandbox; input/control surfaces through explicit workbench Attention and returns through Prime's mediation path (REQ-PRIME-006)",
    budget: "token estimate for the REPL turn, evaluated the same as any consequential operation",
    idempotency:
      "serialized against the one live REPL session; concurrent submissions queue through workbench Attention, never interleave (REQ-PRIME-004, C2 tooth 9)",
    evidence_contract:
      "promotion of a kernel object to Gauntlet evidence is explicit and provenance-preserving; an ephemeral object never silently becomes evidence (REQ-PRIME-007)",
    bound: false,
  }),
  [PLANNED_OPERATION_IDS.PRIME_CANCEL]: Object.freeze({
    id: PLANNED_OPERATION_IDS.PRIME_CANCEL,
    composer_mode: null,
    owner:
      "PrimeDelegationDispatcher's stop/cancel path over the production RealPrimeHost adapter (Phase C2)",
    actors: "an authenticated session actor with settlement access to the delegating unit",
    io_contract:
      "input: {focus: FocusRef, child_session_id: string}; result: {cancelled: boolean, terminal_state: string}",
    preconditions: "a live, admitted child session recorded under this focus",
    authority:
      "canonical admission decides; an explicit canonical stop/cancel — or governing policy — is the ONLY thing that terminates admitted work. Closing the TUI MUST NOT (REQ-PRIME-009)",
    containment:
      "process containment; a forged or substituted binary path is refused, not resolved (T13 spawn-env bound, C2 tooth 4)",
    budget: "no additional payment; cancellation itself is not billed",
    idempotency:
      "idempotent: repeating cancel on an already-terminal session returns the same terminal_state as a no-op, never a second termination attempt",
    evidence_contract:
      "the terminal state is recorded at a canonical locator before the operation reports accepted",
    bound: true,
  }),
  [PLANNED_OPERATION_IDS.EVIDENCE_ADMIT]: Object.freeze({
    id: PLANNED_OPERATION_IDS.EVIDENCE_ADMIT,
    composer_mode: null,
    owner: "Gauntlet's evidence admission path, reached via KernelInputBroker object promotion",
    actors: "an authenticated session actor with settlement access to the target unit",
    io_contract:
      "input: {focus: FocusRef, kernel_object_ref: string, evidence_kind: string}; result: {locator: string, producer: string} matching the EvidenceRequiredFact shape",
    preconditions:
      "the kernel object is classified pinned or artifactized — never ephemeral (REQ-PRIME-007); the target unit's evidence_required names a kind this admission can satisfy",
    authority:
      "canonical admission decides; promotion is explicit, never inferred from kernel activity alone (REQ-PRIME-007)",
    containment:
      "not a process boundary — evidence promotion is a data-provenance discipline, not containment",
    budget:
      "token/usd_micro estimate for the promotion call, evaluated the same as any consequential operation",
    idempotency:
      "same idempotency-key discipline as unit.resolve/unit.retry: re-admission with the same key and payload is a no-op re-entry; a changed payload under a reused key is refused",
    evidence_contract:
      "this operation IS the evidence contract path — its output locator/producer become the unit's EvidenceRequiredFact/evidence_content",
    bound: false,
  }),
  [PLANNED_OPERATION_IDS.PRIME_SESSION]: Object.freeze({
    id: PLANNED_OPERATION_IDS.PRIME_SESSION,
    composer_mode: null,
    owner:
      "substrate-inventory.ts's three-way provenance map plus a production RealPrimeHost session directory (Phase C2)",
    actors:
      "an authenticated session actor; start/stop are consequential, attach/list are session-scoped reads",
    io_contract:
      "input: {focus: FocusRef, session_action: 'start'|'attach'|'stop'}; result: {session_ref: string, topology: unknown}",
    preconditions:
      "Prime capability observed installed; for attach, a QUALIFIED Prime/Gauntlet record naming the session — never inferred from stale UI or unvalidated session-directory presence (REQ-PRIME-010)",
    authority:
      "canonical admission decides start/stop; attach reconstructs read-only topology from qualified records only",
    containment:
      "process containment, not a sandbox; a restarted workbench recovers topology from qualified records, never synthesizes it",
    budget:
      "start/stop carry no direct payment; a launched session's ongoing work is billed at prime.launch",
    idempotency:
      "attach is idempotent — re-attaching an already-attached session returns the same session_ref; start is keyed like any admission",
    evidence_contract:
      "the session's topology facts (parent-child, model identity, lifecycle) ARE the evidence; synthetic parallel agent records are never the source of truth (REQ-PRIME-001)",
    bound: false,
  }),
} as Record<PlannedOperationId, PlannedOperationContract>);

/** Planned operation ids that are actually bound to live code today (see `PlannedOperationContract.bound`). */
export function boundPlannedOperationIds(): readonly PlannedOperationId[] {
  return plannedOperationIds().filter((id) => PLANNED_OPERATION_REGISTRY[id].bound);
}

/** All planned operation IDs in deterministic (catalog) order. */
export function plannedOperationIds(): readonly PlannedOperationId[] {
  return Object.values(PLANNED_OPERATION_IDS);
}

/** Type guard: whether a raw string is a declared PLANNED (not bound) operation identity. */
export function isPlannedOperationId(value: string): value is PlannedOperationId {
  return Object.hasOwn(PLANNED_OPERATION_REGISTRY, value);
}

/** Planned-operation lookup; null when the value names no declared planned operation. */
export function lookupPlannedOperation(value: string): PlannedOperationContract | null {
  return isPlannedOperationId(value) ? PLANNED_OPERATION_REGISTRY[value] : null;
}

/**
 * The declared-but-unbound operation a composer mode's submission would
 * reach once Phase C2 binds it, or `null` when that mode has no composer
 * entry point among the planned operations yet (`prime.cancel`,
 * `evidence.admit`, `prime.session` are declared but not composer-reachable
 * today). `"operation"` mode is EXISTING, not planned, and always returns
 * `null` here — its unsupported/available resolution comes from
 * `operation-registry.ts`/`affordance.ts`, not this module.
 */
export function plannedOperationForComposerMode(
  mode: ComposerModeName,
): PlannedOperationContract | null {
  if (mode === "operation") return null;
  for (const contract of Object.values(PLANNED_OPERATION_REGISTRY)) {
    if (contract.composer_mode === mode) return contract;
  }
  return null;
}

/**
 * Runtime completeness check (D1 discipline applied to declarations, not
 * just observations): every one of the nine REQ-OPS-003 fields must be a
 * non-empty, non-whitespace string. TypeScript's structural typing alone
 * would accept `""` for any field; this is the check the suite runs so a
 * future entry cannot land with a hollow placeholder.
 */
export function contractIsComplete(contract: PlannedOperationContract): boolean {
  return PLANNED_CONTRACT_FIELDS.every((field) => contract[field].trim().length > 0);
}

/**
 * The versioned capability handshake (REQ-OPS-003): distinguishes EXISTING
 * (bound, invocable) operations from PLANNED (declared, not yet bound) ones.
 * `version` starts at 2 — version 1 is the implicit predecessor shape that
 * carried `existing_operations` alone (today's `operation-registry.ts`
 * surface, unversioned because nothing needed distinguishing from it yet).
 * The bump is additive: an old client that reads only `existing_operations`
 * sees exactly what it always saw (C1 tooth 3).
 */
export const CAPABILITY_HANDSHAKE_VERSION = 3;

export interface CapabilityHandshake {
  readonly version: number;
  readonly existing_operations: readonly OperationId[];
  readonly planned_operations: readonly PlannedOperationContract[];
  /**
   * Declared operations whose composer path is bound to live code today
   * (plan workbench-agent-interaction.md P1) — additive field, version 3.
   * A client reading only `planned_operations` (version 2 shape) still sees
   * every one of these ids there too (`bound: true` on the contract),
   * unchanged (D1: additive, never a breaking reinterpretation).
   */
  readonly bound_operations: readonly PlannedOperationId[];
}

export function capabilityHandshake(): CapabilityHandshake {
  return Object.freeze({
    version: CAPABILITY_HANDSHAKE_VERSION,
    existing_operations: operationIds(),
    planned_operations: Object.values(PLANNED_OPERATION_REGISTRY),
    bound_operations: boundPlannedOperationIds(),
  });
}
