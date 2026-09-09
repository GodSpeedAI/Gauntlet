/**
 * TEST-SUPPORT canonical operator surface stub (plan T09 gate suites).
 *
 * An in-process stand-in for the canonical Rust surface
 * (`DurableSurface::perform` → `operator_action::execute_operator_action` →
 * the settlement service) exposing the SAME shape vocabulary the real surface
 * serves, so the deny-bypass matrix can drive controlled denials through every
 * currently implemented route with zero network and zero Rust changes:
 *
 *   - Formation and refusal codes mirror ONLY the canonical vocabulary,
 *     pinned to the Rust sources: the settlement refusal tokens come from
 *     `refusal_code()` (crates/gauntlet-cli/src/operator_action.rs:500-528 —
 *     here `run_not_durable`, `unit_not_frozen`, `authority_not_permitted`,
 *     `operator_identity_required`, `blocking_residual_open`,
 *     `readiness_not_earned`), the anchoring codes from the same file's
 *     ticket-selection match (`no_open_ticket` :232, `ambiguous_ticket` :242,
 *     with :246-249 letting an explicitly-anchored action proceed with zero
 *     open tickets), and the surface's own auth/formation codes
 *     (`unit_unparsable`, `actor_authentication_required`,
 *     `invalid_session_credentials`, `client_asserted_authority_refused`).
 *     There is NO budget refusal: admission has no affordability gate
 *     (Correction Round 1 MAJOR 1). The §16.3 idempotent re-entry shape
 *     (`committed_now: false` over an already-durable settlement) is kept.
 *   - The denial knobs SEPARATE what the READS show from what ADMISSION
 *     enforces, modeling both a stale projection (denial visible before
 *     invoke) and a denial that races past revalidation (visible only at the
 *     admission boundary).
 *   - Side-effect counters make "zero side effect" a measurable assertion.
 *
 * This is NOT production code and is NOT a second canonical authority: every
 * route under test terminates HERE, the single admission point of the suite,
 * exactly as every real route terminates in the one Rust write path.
 */

import type { AffordanceObservation } from "../affordance";
import type {
  CanonicalActionCommand,
  CanonicalOutcome,
  OperatorActionRoute,
} from "../operation-gateway";
import type { CapabilityObservation } from "../workbench-model";

/** The canonical unit-id grammar (spec §5.2, gauntlet-domain `UnitId::new`). */
const UNIT_ID_PATTERN = /^[A-Z]{2,8}-[0-9]{3,6}$/;

export interface StubUnitRecord {
  readonly unit_id: string;
  readonly status: string;
  readonly tier: string;
  /** Durable settlement present (§16.3 re-entry shape applies when set). */
  settled: boolean;
  /** The settlement authority gate: granted (durable PASS + recorded identity) or revoked. */
  authority: "granted" | "revoked";
  /** Whether READS expose the revoked authority (admission enforces it regardless). */
  authority_visible_in_reads: boolean;
  /** An open governance/residual denial on the unit. */
  governance_denied: boolean;
  /** Whether READS expose the governance denial (admission enforces it regardless). */
  governance_visible_in_reads: boolean;
  /** The open escalation ticket id for this unit, or null. */
  open_ticket: string | null;
}

export interface StubSideEffects {
  /** §16.3 durable commits performed (fresh accepts only). */
  settlements_committed: number;
  /** Escalation tickets resolved AFTER acceptance. */
  tickets_resolved: number;
  /** Admission attempts that REACHED the decision body (pre-admission denials never increment this). */
  admission_attempts: number;
}

export interface StubOptions {
  readonly run_id: string;
  readonly bound_actor: string;
  readonly session_token: string;
}

export class CanonicalSurfaceStub {
  readonly sideEffects: StubSideEffects = {
    settlements_committed: 0,
    tickets_resolved: 0,
    admission_attempts: 0,
  };

  private readonly units = new Map<string, StubUnitRecord>();
  private readonly runId: string;
  private readonly boundActor: string;
  private readonly sessionToken: string;
  private reachable = true;
  private budgetExhausted = false;
  private budgetVisibleInReads = true;
  private runRetired = false;
  private capabilities: readonly CapabilityObservation[] = [];

  constructor(options: StubOptions) {
    this.runId = options.run_id;
    this.boundActor = options.bound_actor;
    this.sessionToken = options.session_token;
  }

  // --- scenario construction -------------------------------------------------

  addUnit(record: StubUnitRecord): this {
    this.units.set(record.unit_id, { ...record });
    return this;
  }

  /** Revoke a unit's settlement authority; `visibleInReads` controls whether reads expose it. */
  revokeAuthority(unitId: string, visibleInReads: boolean): this {
    const unit = this.requireUnit(unitId);
    unit.authority = "revoked";
    unit.authority_visible_in_reads = visibleInReads;
    return this;
  }

  /** Deny the unit under an open governance residual; `visibleInReads` as above. */
  denyGovernance(unitId: string, visibleInReads: boolean): this {
    const unit = this.requireUnit(unitId);
    unit.governance_denied = true;
    unit.governance_visible_in_reads = visibleInReads;
    return this;
  }

  /**
   * Exhaust the run budget; `visibleInReads` controls whether reads show
   * ~zero remaining. This is a READ-side knob only: the admission decision
   * body has NO budget gate (Correction Round 1 MAJOR 1 — the real
   * operator-action refusal vocabulary, operator_action.rs:500-528, carries
   * no budget code). Affordability is enforced workbench-side by the
   * affordance resolver.
   */
  exhaustBudget(visibleInReads: boolean): this {
    this.budgetExhausted = true;
    this.budgetVisibleInReads = visibleInReads;
    return this;
  }

  /** Make the surface unobservable (transport loss for the in-process route). */
  setUnreachable(): this {
    this.reachable = false;
    return this;
  }

  /** Make the anchored run stale (the anchor the projection held is no longer durable). */
  retireRun(): this {
    this.runRetired = true;
    return this;
  }

  declareCapabilities(capabilities: readonly CapabilityObservation[]): this {
    this.capabilities = capabilities;
    return this;
  }

  // --- canonical reads ---------------------------------------------------------

  /**
   * The fresh canonical observation (the stand-in for the surface's snapshot
   * read). Honors the read-visibility knobs ONLY; admission enforcement is
   * independent. Throws when the surface is unreachable, like a failed fetch.
   */
  observe(_target: { unit: string | null }): AffordanceObservation {
    if (!this.reachable) {
      throw new Error("the canonical surface is unreachable");
    }
    const records = [...this.units.values()];
    return Object.freeze({
      reachable: true,
      degraded: false,
      units: Object.freeze(
        records.map((record) => Object.freeze({ unit_id: record.unit_id, status: record.status })),
      ),
      open_escalations: Object.freeze(
        records
          .filter((record) => record.open_ticket !== null)
          .map((record) =>
            Object.freeze({ ticket_id: record.open_ticket ?? "", unit_id: record.unit_id }),
          ),
      ),
      budget_remaining: Object.freeze({
        usd_micro: this.budgetExhausted && this.budgetVisibleInReads ? 0 : 5_000_000,
        tokens: this.budgetExhausted && this.budgetVisibleInReads ? 0 : 1_000_000,
      }),
      governance_denied_units: Object.freeze(
        records
          .filter((record) => record.governance_denied && record.governance_visible_in_reads)
          .map((record) => record.unit_id),
      ),
      authority_revoked_units: Object.freeze(
        records
          .filter((record) => record.authority === "revoked" && record.authority_visible_in_reads)
          .map((record) => record.unit_id),
      ),
      capabilities: this.capabilities,
    });
  }

  // --- canonical admission -----------------------------------------------------

  /**
   * The API-shape entry (the surface's action endpoint: `?run=` anchor or the
   * disclosed derivation; the run's open ticket is resolved). Mirrors
   * `DurableSurface::perform`.
   */
  perform(
    kind: "resolve" | "retry",
    unitId: string,
    operator: string,
    _note: string,
    run: string | null,
  ): CanonicalOutcome {
    this.sideEffects.admission_attempts += 1;
    if (!UNIT_ID_PATTERN.test(unitId)) {
      return failed("unit_unparsable", `${unitId} is not a valid unit id`);
    }
    if (operator.trim().length === 0) {
      return refused(
        "operator_identity_required",
        "every action carries the operator's identity (spec 16.1 condition 3); missing or blank is refused",
      );
    }
    if (run !== null && (run !== this.runId || this.runRetired)) {
      return refused(
        "run_not_durable",
        `run ${run} is not durable in this state store; the anchored view is stale`,
      );
    }
    if (run === null && this.runRetired) {
      return refused(
        "run_not_durable",
        "no active run is derivable from the durable artifacts; the anchored view is stale",
      );
    }
    const openTickets = [...this.units.values()].filter((record) => record.open_ticket !== null);
    let tickets: string[];
    if (run !== null) {
      tickets = openTickets.map((record) => record.open_ticket ?? "");
    } else {
      switch (openTickets.length) {
        case 1: {
          const only = openTickets[0];
          tickets = only === undefined ? [] : [only.open_ticket ?? ""];
          break;
        }
        case 0:
          return refused(
            "no_open_ticket",
            `run ${this.runId} holds no open escalation ticket — the operator action has nothing to resolve`,
          );
        default:
          return refused(
            "ambiguous_ticket",
            `run ${this.runId} holds ${openTickets.length} open escalation tickets — the API action cannot choose one`,
          );
      }
    }
    return this.decide(kind, unitId, tickets);
  }

  /**
   * The CLI-shape entry (the operator names the ticket; the ticket's own run
   * anchors the action). Mirrors the `escalations resolve|retry` command path.
   */
  performViaCli(
    kind: "resolve" | "retry",
    unitId: string,
    operator: string,
    _note: string,
    ticket: string | null,
  ): CanonicalOutcome {
    this.sideEffects.admission_attempts += 1;
    if (!UNIT_ID_PATTERN.test(unitId)) {
      return failed("unit_unparsable", `${unitId} is not a valid unit id`);
    }
    if (operator.trim().length === 0) {
      return refused(
        "operator_identity_required",
        "every action carries the operator's identity (spec 16.1 condition 3); missing or blank is refused",
      );
    }
    if (ticket === null) {
      return failed("action_failed", "the CLI action names no ticket to anchor");
    }
    const owner = [...this.units.values()].find((record) => record.open_ticket === ticket);
    if (owner === undefined) {
      return failed("action_failed", `ticket ${ticket} is not open in the durable queue`);
    }
    if (this.runRetired) {
      return refused(
        "run_not_durable",
        `run ${this.runId} anchored by ticket ${ticket} is no longer durable; the anchor is stale`,
      );
    }
    return this.decide(kind, unitId, [ticket]);
  }

  /**
   * The ONE decision body both entries share (the suite's stand-in for the one
   * Rust write path): authority gate → §16.3 re-entry → governance → budget →
   * operation preconditions → accept + ticket resolution.
   */
  private decide(kind: "resolve" | "retry", unitId: string, tickets: string[]): CanonicalOutcome {
    const unit = this.units.get(unitId);
    if (unit === undefined) {
      return refused(
        "unit_not_frozen",
        `unit ${unitId} is not in the frozen ruler — settlement refuses it`,
      );
    }
    // The settlement authority gate speaks FIRST (charter D4 ordering).
    if (unit.authority === "revoked") {
      return refused(
        "authority_not_permitted",
        `unit ${unitId} has no durable earned authority for an operator action`,
      );
    }
    // §16.3 idempotent re-entry: an already-durable settlement reports the
    // durable truth (committed_now=false) and resolves still-open tickets,
    // never committing twice.
    if (unit.settled) {
      const resolved = this.resolveTickets(tickets);
      return accepted(this.runId, unitId, resolved, false);
    }
    if (unit.governance_denied) {
      return refused(
        "blocking_residual_open",
        `unit ${unitId} carries an open blocking residual; governance denies the action`,
      );
    }
    // NO budget gate here (Correction Round 1 MAJOR 1): the real operator
    // action admission path (crates/gauntlet-cli/src/operator_action.rs,
    // execute_operator_action → the settlement service) has no affordability
    // refusal — `budget_exhausted` appears nowhere in `refusal_code()`
    // (operator_action.rs:500-528) nor in the SettleError vocabulary. A
    // budget fact exists only as a READ (RuntimeSnapshot.budget_remaining).
    // Affordability is a workbench RESOLVER policy: the gateway refuses
    // unaffordable resolutions pre-admission and the canonical surface never
    // sees a gateway-denied request.
    if (kind === "retry" && unit.status !== "fail") {
      return refused(
        "readiness_not_earned",
        `unit ${unitId} has no failed verification to retry; readiness is not earned`,
      );
    }
    // Accept: the §16.3 commit, then (only then) the ticket resolution.
    this.sideEffects.settlements_committed += 1;
    unit.settled = true;
    const resolved = this.resolveTickets(tickets);
    return accepted(this.runId, unitId, resolved, true);
  }

  private resolveTickets(tickets: string[]): string[] {
    const resolved: string[] = [];
    for (const ticket of tickets) {
      const owner = [...this.units.values()].find((record) => record.open_ticket === ticket);
      if (owner !== undefined) {
        const record = owner as { open_ticket: string | null };
        record.open_ticket = null;
        this.sideEffects.tickets_resolved += 1;
        resolved.push(ticket);
      }
    }
    return resolved;
  }

  private requireUnit(unitId: string): StubUnitRecord {
    const unit = this.units.get(unitId);
    if (unit === undefined) {
      throw new Error(`stub has no unit '${unitId}'`);
    }
    return unit;
  }

  // --- routes over the ONE admission -------------------------------------------

  /**
   * The HTTP surface route wrapper: serves the stub's reads and admission over
   * loopback HTTP with the REAL surface's wire conventions
   * (crates/adapters/gauntlet-adapter-surface-json/src/http.rs): accepted →
   * 200; typed refusal → 409 {code,message}; infrastructure failure → 500;
   * formation/auth → 400/401/403 {code,message}.
   */
  serveHttp(): ReturnType<typeof Bun.serve> {
    const stub = this;
    return Bun.serve({
      port: 0,
      async fetch(req) {
        const url = new URL(req.url);
        const path = url.pathname;

        if (req.method === "GET" && path === "/api/v1/state") {
          if (!stub.reachable) {
            return Response.json(
              { code: "snapshot_failed", message: "the canonical surface is unreachable" },
              { status: 500 },
            );
          }
          const observation = stub.observe({ unit: null });
          return Response.json({
            run: stub.runId,
            escalated: observation.open_escalations.map((ticket) => ticket.ticket_id),
            units: observation.units,
            budget_remaining: observation.budget_remaining,
            open_escalations: observation.open_escalations,
            governance_denied_units: observation.governance_denied_units,
            authority_revoked_units: observation.authority_revoked_units,
            status: "connected",
            run_derivation: "derived from the open escalation tickets of the stub queue",
          });
        }

        const actionMatch = /^\/api\/v1\/units\/([^/]+)\/(resolve|retry)$/.exec(path);
        if (req.method === "POST" && actionMatch !== null) {
          const unitId = decodeURIComponent(actionMatch[1] ?? "");
          const kind = actionMatch[2] === "resolve" ? "resolve" : "retry";
          if (!UNIT_ID_PATTERN.test(unitId)) {
            return Response.json(
              { code: "unit_unparsable", message: `${unitId} is not a valid unit id` },
              { status: 400 },
            );
          }
          const operatorHeader = req.headers.get("X-Gauntlet-Operator");
          const operator = operatorHeader === null ? "" : operatorHeader.trim();
          if (operator.length === 0) {
            return Response.json(
              {
                code: "operator_identity_required",
                message:
                  "every action carries the operator's identity in the X-Gauntlet-Operator header (spec 16.1 condition 3); missing or blank is refused",
              },
              { status: 400 },
            );
          }
          const authHeader = req.headers.get("Authorization");
          if (authHeader === null || authHeader.replace(/^Bearer\s+/i, "").trim().length === 0) {
            return Response.json(
              {
                code: "actor_authentication_required",
                message:
                  "mutation requests require authentication; provide bearer credentials in the Authorization header",
              },
              { status: 401 },
            );
          }
          const token = authHeader.replace(/^Bearer\s+/i, "").trim();
          if (token !== stub.sessionToken) {
            return Response.json(
              {
                code: "invalid_session_credentials",
                message: "the provided session token is invalid or expired",
              },
              { status: 401 },
            );
          }
          if (operator !== stub.boundActor) {
            return Response.json(
              {
                code: "client_asserted_authority_refused",
                message: `client-asserted operator '${operator}' does not match authenticated session actor '${stub.boundActor}'`,
              },
              { status: 403 },
            );
          }
          const runParam = url.searchParams.get("run");
          const note = await req.text();
          const outcome = stub.perform(kind, unitId, operator, note, runParam);
          if (outcome.kind === "accepted") {
            return Response.json({
              action: kind,
              run_id: outcome.run_id,
              unit_id: outcome.unit_id,
              committed_now: outcome.committed_now,
              tickets_resolved: outcome.tickets_resolved,
            });
          }
          return Response.json(
            { code: outcome.code, message: outcome.message },
            { status: outcome.kind === "refused" ? 409 : 500 },
          );
        }

        return Response.json(
          { code: "not_found", message: `No route for ${path}` },
          { status: 404 },
        );
      },
    });
  }

  /** The in-process route (the workbench-internal adapter shim's transport). */
  asInProcessRoute(): OperatorActionRoute {
    return {
      observe: (target) => Promise.resolve(this.observe(target)),
      perform: (command: CanonicalActionCommand) => {
        // Session re-validation at the surface boundary (the shim route is
        // loopback-trusted for transport but STILL checks the session, like
        // the real surface's authenticator): wrong token → typed auth refusal.
        if (command.session_token !== this.sessionToken) {
          return Promise.resolve(
            command.session_token.length === 0
              ? refused(
                  "actor_authentication_required",
                  "mutation requests require authentication; provide bearer credentials in the Authorization header",
                )
              : refused(
                  "invalid_session_credentials",
                  "the provided session token is invalid or expired",
                ),
          );
        }
        if (command.operator !== this.boundActor) {
          return Promise.resolve(
            refused(
              "client_asserted_authority_refused",
              `client-asserted operator '${command.operator}' does not match authenticated session actor '${this.boundActor}'`,
            ),
          );
        }
        return Promise.resolve(
          this.perform(
            command.action_kind,
            command.unit,
            command.operator,
            command.note,
            command.run,
          ),
        );
      },
    };
  }
}

function accepted(
  runId: string,
  unitId: string,
  tickets: string[],
  committedNow: boolean,
): CanonicalOutcome {
  return {
    kind: "accepted",
    run_id: runId,
    unit_id: unitId,
    tickets_resolved: Object.freeze(tickets),
    committed_now: committedNow,
  };
}

function refused(code: string, message: string): CanonicalOutcome {
  return { kind: "refused", code, message };
}

function failed(code: string, message: string): CanonicalOutcome {
  return { kind: "failed", code, message };
}
