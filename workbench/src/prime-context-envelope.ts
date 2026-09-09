/**
 * Prime child context-envelope construction (plan T10, REQ-OPS-004, REQ-GOAL-003,
 * REQ-PRIME-004; prereg intervention class A1 — Attack 1's defense).
 *
 * A context envelope is the bounded, serializable artifact a Prime child receives
 * when work is delegated to it. It is constructed from a hoisted Horizon and its
 * planMaterialization output (relations, context candidates, and the MANDATORY
 * Reference/governance/evidence closure that hoisting can never remove), plus the
 * caller's explicit required dependencies.
 *
 * Core Invariants:
 *   - DEFAULT IS BOUNDED. `constructBoundedContextEnvelope` REQUIRES a Horizon and
 *     a plan that names that same horizon; no parameter of it can produce a
 *     whole-Case dump (Attack 1). A caller wanting "everything" must call
 *     `constructWholeCaseEnvelope` explicitly — a separate function whose output is
 *     marked `kind: "whole_case_disclosed"` and carries a required, recorded
 *     disclosure — never an implicit default.
 *   - Mandatory closure (REQ-OPS-004): every envelope keeps the plan's
 *     mandatory_reference, mandatory_governance, and mandatory_evidence candidates.
 *     Hoisting narrows the relation working set; it never removes the closure.
 *   - Every included item carries an explicit inclusion reason (the candidate's
 *     provenance detail, or the caller's declared dependency reason).
 *   - The envelope records its horizon identity and generation, so provenance of
 *     "what context this child was constructed from" is inspectable afterwards.
 *   - Envelopes are plain data (JSON-serializable); parsing re-resolves FocusRefs
 *     through the identity registry so deserialization can never fork identity.
 *   - Formal critic envelopes are NOT built here: critic context assembly belongs
 *     to Gauntlet comparison (fresh context, allowlist) — REQ-OPS-004 keeps chat,
 *     builder history, and Prime session memory out of formal comparison, and this
 *     module deliberately has no critic-envelope capability at all.
 */

import { FocusRef } from "./focus";
import type { ContextCandidate, Horizon, MaterializationPlan } from "./horizon";
import { isHorizon } from "./horizon";

/** The wire/schema identity of the envelope artifact. */
export const PRIME_CONTEXT_ENVELOPE_SCHEMA = "gauntlet.workbench.prime-context-envelope/1";

/** Why one focus is inside a child's context envelope. */
export type EnvelopeInclusionRole =
  | "horizon_root"
  | "relation"
  | "mandatory_reference"
  | "mandatory_governance"
  | "mandatory_evidence"
  | "declared_dependency";

/**
 * Per-role context budget, applied AFTER the mandatory closure (REQ-OPS-004:
 * "Per-role context budgets apply after this mandatory closure"). The budget
 * counts the two NON-structural roles — horizon-relation items and declared
 * required dependencies; the mandatory closure roles and the single horizon
 * root are exempt by definition. The budget is the defense against smuggling
 * whole-Case content through the declared-dependency channel (Correction
 * Round 1 MAJOR 1): exceeding it is a typed construction refusal — the
 * envelope is NEVER silently re-kinded.
 */
export interface EnvelopeRoleBudget {
  /** Maximum horizon-relation items (reached via declared relation traversals). */
  readonly relation: number;
  /** Maximum declared required dependencies. */
  readonly declared_dependency: number;
}

/**
 * The typed default role budget. A caller's role/config may raise it
 * explicitly (the raised budget is then recorded in the envelope's accounting);
 * there is no unbounded option short of the explicitly disclosed whole-case
 * constructor.
 */
export const DEFAULT_ENVELOPE_ROLE_BUDGET: EnvelopeRoleBudget = Object.freeze({
  relation: 16,
  declared_dependency: 4,
});

/** Roles exempt from the budget: the mandatory closure (REQ-OPS-004) plus the single horizon root. */
export const BUDGET_EXEMPT_ROLES: readonly EnvelopeInclusionRole[] = Object.freeze([
  "horizon_root",
  "mandatory_reference",
  "mandatory_governance",
  "mandatory_evidence",
]);

/** Budget accounting recorded in the envelope's provenance. */
export interface EnvelopeBudgetAccounting {
  readonly limits: EnvelopeRoleBudget;
  /** Number of horizon-relation items counted against `limits.relation`. */
  readonly relation_count: number;
  /** Number of declared dependencies counted against `limits.declared_dependency`. */
  readonly declared_dependency_count: number;
  readonly exempt_roles: readonly EnvelopeInclusionRole[];
}

/** One included focus with its explicit inclusion reason. */
export interface EnvelopeItem {
  readonly focus: FocusRef;
  readonly role: EnvelopeInclusionRole;
  /** Explicit, human-inspectable inclusion reason (provenance of membership). */
  readonly reason: string;
  /** True for mandatory closure items and declared dependencies; false for horizon-relation items. */
  readonly mandatory: boolean;
}

/**
 * The bounded context artifact a Prime child receives. Plain serializable data:
 * `JSON.stringify` works directly (FocusRef carries toJSON), and
 * `envelopeFromJSON` re-admits an untrusted document through validation.
 */
export interface PrimeContextEnvelope {
  readonly schema: typeof PRIME_CONTEXT_ENVELOPE_SCHEMA;
  readonly envelope_id: string;
  /** `bounded` (the default) or `whole_case_disclosed` (explicit, degraded, recorded). */
  readonly kind: "bounded" | "whole_case_disclosed";
  /** Horizon identity this envelope was constructed from; null only for whole-case disclosure. */
  readonly horizon_id: string | null;
  /** Workbench generation the horizon projection was valid for. */
  readonly horizon_generation: string;
  readonly root_focus: FocusRef;
  readonly items: readonly EnvelopeItem[];
  /** Role/task instructions carried verbatim for the child (never comparison input). */
  readonly role_instructions: readonly string[];
  /** The dependencies the caller explicitly declared (provenance; also present in items). */
  readonly declared_dependencies: readonly FocusRef[];
  /** Degradation/disclosure records — non-empty for whole-case disclosure. */
  readonly disclosures: readonly string[];
  /**
   * Per-role budget accounting (Correction Round 1 MAJOR 1). Non-null for
   * bounded envelopes; null ONLY for whole-case disclosure (unbounded by
   * explicit, recorded disclosure — never by an implicit default).
   */
  readonly budget: EnvelopeBudgetAccounting | null;
  readonly constructed_at_ms: number;
}

/** A caller-declared required dependency: the child cannot do the work without it. */
export interface RequiredDependency {
  readonly focus: FocusRef;
  /** Why the child needs this (recorded verbatim as the item's inclusion reason). */
  readonly reason: string;
}

/** Typed construction failure carrying the envelope-construction locus. */
export class EnvelopeConstructionError extends Error {
  constructor(detail: string) {
    super(`prime context envelope construction: ${detail}`);
    this.name = "EnvelopeConstructionError";
  }
}

/** Typed parse failure for malformed untrusted envelope documents. */
export class MalformedEnvelopeError extends Error {
  constructor(detail: string) {
    super(`malformed prime context envelope: ${detail}`);
    this.name = "MalformedEnvelopeError";
  }
}

function requireNonEmpty(value: string, what: string): string {
  if (value.trim().length === 0) {
    throw new EnvelopeConstructionError(`${what} must be a non-empty string`);
  }
  return value;
}

/**
 * Map one plan context candidate to an envelope item, preserving the mandatory
 * closure roles verbatim and naming the horizon root explicitly.
 */
function itemFromCandidate(candidate: ContextCandidate, rootFocusId: string): EnvelopeItem {
  const role: EnvelopeInclusionRole =
    candidate.role === "relation" && candidate.focus.id === rootFocusId
      ? "horizon_root"
      : candidate.role;
  return Object.freeze({
    focus: candidate.focus,
    role,
    reason: candidate.detail,
    mandatory: candidate.mandatory,
  });
}

export interface BoundedEnvelopeOptions {
  /** The active (hoisted) Horizon the child context is constructed from. Required — there is no unbounded default. */
  readonly horizon: Horizon;
  /** The planMaterialization output FOR THAT HORIZON (plan.horizon_id must equal horizon.id). */
  readonly plan: MaterializationPlan;
  readonly role_instructions?: readonly string[] | undefined;
  readonly required_dependencies?: readonly RequiredDependency[] | undefined;
  /**
   * The per-role context budget applied AFTER the mandatory closure
   * (REQ-OPS-004). Defaults to `DEFAULT_ENVELOPE_ROLE_BUDGET`; raising it is an
   * explicit caller decision recorded in the envelope's accounting. Exceeding
   * any budgeted role's limit is a typed `EnvelopeConstructionError` — the
   * envelope is never silently re-kinded (Correction Round 1 MAJOR 1).
   */
  readonly context_budget?: EnvelopeRoleBudget | undefined;
  readonly envelope_id?: string | undefined;
  readonly constructed_at_ms?: number | undefined;
}

/**
 * Validate a caller-supplied budget shape (typed inputs, no stringly state).
 */
function validBudget(budget: EnvelopeRoleBudget): boolean {
  return (
    typeof budget === "object" &&
    budget !== null &&
    Number.isFinite(budget.relation) &&
    budget.relation >= 0 &&
    Number.isFinite(budget.declared_dependency) &&
    budget.declared_dependency >= 0
  );
}

/**
 * Construct a BOUNDED context envelope from a hoisted Horizon plus its
 * materialization plan and explicit required dependencies (the default and only
 * implicit path — bounded by construction, Attack 1's defense).
 *
 * Throws a typed `EnvelopeConstructionError` when the horizon is malformed, when
 * the plan does not belong to the horizon, or when a dependency/discriminator
 * field is malformed. There is deliberately NO option that widens the envelope
 * beyond `plan.context_candidates` + declared dependencies.
 */
export function constructBoundedContextEnvelope(
  options: BoundedEnvelopeOptions,
): PrimeContextEnvelope {
  if (!isHorizon(options.horizon)) {
    throw new EnvelopeConstructionError("a valid hoisted Horizon is required");
  }
  if (options.plan.horizon_id !== options.horizon.id) {
    throw new EnvelopeConstructionError(
      `plan belongs to horizon '${options.plan.horizon_id ?? "<whole-Case>"}', not to the requested horizon '${options.horizon.id}'`,
    );
  }

  const seen = new Set<string>();
  const items: EnvelopeItem[] = [];
  for (const candidate of options.plan.context_candidates) {
    const item = itemFromCandidate(candidate, options.horizon.root_focus.id);
    seen.add(item.focus.id);
    items.push(item);
  }

  const declared: FocusRef[] = [];
  for (const dependency of options.required_dependencies ?? []) {
    requireNonEmpty(dependency.reason, "declared dependency reason");
    declared.push(dependency.focus);
    if (seen.has(dependency.focus.id)) {
      // Already present via the horizon plan (e.g. mandatory closure): the
      // existing item stays; the declaration is still recorded in provenance.
      continue;
    }
    seen.add(dependency.focus.id);
    items.push(
      Object.freeze({
        focus: dependency.focus,
        role: "declared_dependency" as const,
        reason: dependency.reason,
        mandatory: true,
      }),
    );
  }

  // Per-role context budget, applied AFTER the mandatory closure (REQ-OPS-004,
  // Correction Round 1 MAJOR 1): the budget counts the two non-structural roles
  // (horizon-relation items and declared dependencies); the mandatory closure
  // roles and the single horizon root are exempt. Exceeding a limit is a typed
  // refusal — never a silent re-kind, never silent truncation.
  const budget = options.context_budget ?? DEFAULT_ENVELOPE_ROLE_BUDGET;
  if (!validBudget(budget)) {
    throw new EnvelopeConstructionError(
      "context_budget requires finite non-negative relation and declared_dependency limits",
    );
  }
  const relationCount = items.filter((item) => item.role === "relation").length;
  const dependencyCount = items.filter((item) => item.role === "declared_dependency").length;
  if (relationCount > budget.relation) {
    throw new EnvelopeConstructionError(
      `per-role context budget exceeded: ${relationCount} relation items exceed the relation budget of ${budget.relation} (narrow the horizon's declared relation traversals or raise the role's budget explicitly); the envelope is never silently re-kinded`,
    );
  }
  if (dependencyCount > budget.declared_dependency) {
    throw new EnvelopeConstructionError(
      `per-role context budget exceeded: ${dependencyCount} declared_dependency items exceed the declared_dependency budget of ${budget.declared_dependency} (declare fewer required dependencies or raise the role's budget explicitly); the envelope is never silently re-kinded`,
    );
  }
  const budgetAccounting: EnvelopeBudgetAccounting = Object.freeze({
    limits: Object.freeze({ ...budget }),
    relation_count: relationCount,
    declared_dependency_count: dependencyCount,
    exempt_roles: BUDGET_EXEMPT_ROLES,
  });

  const envelopeId =
    options.envelope_id ??
    `envelope:${options.horizon.id}:c${options.plan.counters.context_candidates}`;

  return Object.freeze({
    schema: PRIME_CONTEXT_ENVELOPE_SCHEMA,
    envelope_id: envelopeId,
    kind: "bounded" as const,
    horizon_id: options.horizon.id,
    horizon_generation: options.horizon.generation,
    root_focus: options.horizon.root_focus,
    items: Object.freeze(items),
    role_instructions: Object.freeze([...(options.role_instructions ?? [])]),
    declared_dependencies: Object.freeze(declared),
    disclosures: Object.freeze([]),
    budget: budgetAccounting,
    constructed_at_ms: options.constructed_at_ms ?? Date.now(),
  });
}

export interface WholeCaseEnvelopeOptions {
  /** The whole-Case plan (planMaterialization(world, null)); horizon_id must be null. */
  readonly plan: MaterializationPlan;
  readonly generation: string;
  readonly root_focus: FocusRef;
  /** The recorded, inspectable reason whole-Case disclosure was authorized. Required and non-empty. */
  readonly disclosure_reason: string;
  readonly role_instructions?: readonly string[] | undefined;
  readonly envelope_id?: string | undefined;
  readonly constructed_at_ms?: number | undefined;
}

/**
 * Construct an EXPLICIT whole-Case envelope — never the default. The result is
 * marked `kind: "whole_case_disclosed"`, carries a non-empty disclosures record
 * naming the authorizing reason, and is flagged degraded. Callers delegating with
 * such an envelope must record it as degraded/disclosed (prime-recursion's
 * delegation records carry `envelope_kind` for exactly that provenance).
 */
export function constructWholeCaseEnvelope(
  options: WholeCaseEnvelopeOptions,
): PrimeContextEnvelope {
  if (options.plan.horizon_id !== null) {
    throw new EnvelopeConstructionError(
      "whole-case disclosure requires the whole-Case plan (plan.horizon_id === null); hoisted plans construct bounded envelopes",
    );
  }
  const reason = requireNonEmpty(options.disclosure_reason, "whole-case disclosure reason");
  const items = options.plan.context_candidates.map((candidate) =>
    Object.freeze({
      focus: candidate.focus,
      role: candidate.role,
      reason: candidate.detail,
      mandatory: candidate.mandatory,
    }),
  );
  return Object.freeze({
    schema: PRIME_CONTEXT_ENVELOPE_SCHEMA,
    envelope_id: options.envelope_id ?? `envelope:whole-case:${options.generation}`,
    kind: "whole_case_disclosed" as const,
    horizon_id: null,
    horizon_generation: options.generation,
    root_focus: options.root_focus,
    items: Object.freeze(items),
    role_instructions: Object.freeze([...(options.role_instructions ?? [])]),
    declared_dependencies: Object.freeze([]),
    disclosures: Object.freeze([
      `whole-Case context disclosed by explicit authorization: ${reason}`,
      "DEGRADED: unbounded context — recorded disclosure, never an implicit default",
    ]),
    budget: null,
    constructed_at_ms: options.constructed_at_ms ?? Date.now(),
  });
}

/** Plain serializable form (JSON.stringify works directly too; this is the named path). */
export function envelopeToJSON(envelope: PrimeContextEnvelope): string {
  return JSON.stringify(envelope);
}

/**
 * Re-admit an untrusted envelope document: validate the declared schema and every
 * item, and re-resolve all FocusRefs through the identity registry so parsing can
 * never fork identity (model output is untrusted external input).
 */
export function envelopeFromJSON(json: string): PrimeContextEnvelope {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (error) {
    throw new MalformedEnvelopeError(`payload is not valid JSON: ${String(error)}`);
  }
  if (typeof parsed !== "object" || parsed === null) {
    throw new MalformedEnvelopeError("expected an object");
  }
  const doc = parsed as Record<string, unknown>;
  if (doc["schema"] !== PRIME_CONTEXT_ENVELOPE_SCHEMA) {
    throw new MalformedEnvelopeError(`unsupported schema '${String(doc["schema"])}'`);
  }
  const kind = doc["kind"];
  if (kind !== "bounded" && kind !== "whole_case_disclosed") {
    throw new MalformedEnvelopeError("kind must be 'bounded' or 'whole_case_disclosed'");
  }
  const envelopeId = doc["envelope_id"];
  const generation = doc["horizon_generation"];
  const constructedAt = doc["constructed_at_ms"];
  if (typeof envelopeId !== "string" || typeof generation !== "string") {
    throw new MalformedEnvelopeError("requires string envelope_id and horizon_generation");
  }
  if (typeof constructedAt !== "number") {
    throw new MalformedEnvelopeError("requires numeric constructed_at_ms");
  }
  const horizonId = doc["horizon_id"];
  if (horizonId !== null && typeof horizonId !== "string") {
    throw new MalformedEnvelopeError("horizon_id must be a string or null");
  }
  if (kind === "bounded" && typeof horizonId !== "string") {
    throw new MalformedEnvelopeError("a bounded envelope names its horizon");
  }
  if (kind === "whole_case_disclosed" && horizonId !== null) {
    throw new MalformedEnvelopeError("a whole-case disclosure has no horizon");
  }
  let root: FocusRef;
  try {
    root = FocusRef.fromJSON(doc["root_focus"]);
  } catch (error) {
    throw new MalformedEnvelopeError(
      `root_focus: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const rawItems = doc["items"];
  if (!Array.isArray(rawItems)) {
    throw new MalformedEnvelopeError("items must be an array");
  }
  const items: EnvelopeItem[] = rawItems.map((rawItem) => {
    if (typeof rawItem !== "object" || rawItem === null) {
      throw new MalformedEnvelopeError("each item must be an object");
    }
    const record = rawItem as Record<string, unknown>;
    const role = record["role"];
    const reason = record["reason"];
    const mandatory = record["mandatory"];
    if (
      role !== "horizon_root" &&
      role !== "relation" &&
      role !== "mandatory_reference" &&
      role !== "mandatory_governance" &&
      role !== "mandatory_evidence" &&
      role !== "declared_dependency"
    ) {
      throw new MalformedEnvelopeError(`unknown item role '${String(role)}'`);
    }
    if (typeof reason !== "string" || typeof mandatory !== "boolean") {
      throw new MalformedEnvelopeError("each item requires string reason and boolean mandatory");
    }
    let focus: FocusRef;
    try {
      focus = FocusRef.fromJSON(record["focus"]);
    } catch (error) {
      throw new MalformedEnvelopeError(
        `item focus: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
    return Object.freeze({ focus, role, reason, mandatory });
  });
  const rawInstructions = doc["role_instructions"];
  if (
    !Array.isArray(rawInstructions) ||
    rawInstructions.some((entry) => typeof entry !== "string")
  ) {
    throw new MalformedEnvelopeError("role_instructions must be an array of strings");
  }
  const rawDisclosures = doc["disclosures"];
  if (!Array.isArray(rawDisclosures) || rawDisclosures.some((entry) => typeof entry !== "string")) {
    throw new MalformedEnvelopeError("disclosures must be an array of strings");
  }
  if (kind === "whole_case_disclosed" && rawDisclosures.length === 0) {
    throw new MalformedEnvelopeError(
      "a whole-case disclosure carries a non-empty disclosure record",
    );
  }
  const rawDeclared = doc["declared_dependencies"];
  if (!Array.isArray(rawDeclared)) {
    throw new MalformedEnvelopeError("declared_dependencies must be an array");
  }
  const declared = rawDeclared.map((rawRef) => {
    try {
      return FocusRef.fromJSON(rawRef);
    } catch (error) {
      throw new MalformedEnvelopeError(
        `declared dependency: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  });
  // Budget accounting must round-trip: a bounded envelope names its budget and
  // counts; a whole-case disclosure is unbounded (budget null) BY DISCLOSURE.
  const rawBudget = doc["budget"];
  let budget: EnvelopeBudgetAccounting | null;
  if (kind === "whole_case_disclosed") {
    if (rawBudget !== null) {
      throw new MalformedEnvelopeError(
        "a whole-case disclosure carries budget: null (unbounded by disclosure)",
      );
    }
    budget = null;
  } else {
    if (typeof rawBudget !== "object" || rawBudget === null) {
      throw new MalformedEnvelopeError("a bounded envelope carries its budget accounting");
    }
    const budgetDoc = rawBudget as Record<string, unknown>;
    const limits = budgetDoc["limits"];
    if (typeof limits !== "object" || limits === null) {
      throw new MalformedEnvelopeError("budget accounting requires limits");
    }
    const limitsDoc = limits as Record<string, unknown>;
    const relationLimit = limitsDoc["relation"];
    const dependencyLimit = limitsDoc["declared_dependency"];
    if (typeof relationLimit !== "number" || typeof dependencyLimit !== "number") {
      throw new MalformedEnvelopeError(
        "budget limits require numeric relation and declared_dependency",
      );
    }
    const relationCount = budgetDoc["relation_count"];
    const dependencyCount = budgetDoc["declared_dependency_count"];
    if (typeof relationCount !== "number" || typeof dependencyCount !== "number") {
      throw new MalformedEnvelopeError("budget accounting requires numeric counts");
    }
    const exempt = budgetDoc["exempt_roles"];
    if (!Array.isArray(exempt) || exempt.some((entry) => typeof entry !== "string")) {
      throw new MalformedEnvelopeError("budget exempt_roles must be an array of strings");
    }
    budget = Object.freeze({
      limits: Object.freeze({ relation: relationLimit, declared_dependency: dependencyLimit }),
      relation_count: relationCount,
      declared_dependency_count: dependencyCount,
      exempt_roles: Object.freeze([...exempt]) as readonly EnvelopeInclusionRole[],
    });
  }
  return Object.freeze({
    schema: PRIME_CONTEXT_ENVELOPE_SCHEMA,
    envelope_id: envelopeId,
    kind,
    horizon_id: horizonId,
    horizon_generation: generation,
    root_focus: root,
    items: Object.freeze(items),
    role_instructions: Object.freeze([...rawInstructions]),
    declared_dependencies: Object.freeze(declared),
    disclosures: Object.freeze([...rawDisclosures]),
    budget,
    constructed_at_ms: constructedAt,
  });
}
