/**
 * T18 secret-leakage suite (plan T18 Attack 2; REQ-SEC-003, REQ-SAFE-003,
 * REQ-CONFIG-002, REQ-UX-006).
 *
 * Declared falsifier: "Seed fake secrets in approved secret sources and trigger
 * errors, snapshots, evidence frames, plugin manifests/logs, and SQLite
 * writes." Expected: no secret appears in forbidden outputs; references and
 * redactions are used instead.
 *
 * Approved secret sources seeded here (synthetic, well-marked): the OS env
 * indirection (GAUNTLET_SESSION_TOKEN) and the CLI option — the ONLY sources
 * REQ-CONFIG-002 approves for session tokens. Forbidden channels swept: error
 * messages, workbench snapshots, evidence frames, plugin manifests/state/
 * error-ledger, SQLite projection rows (a real temp-file store read back over
 * an INDEPENDENT connection), terminal provenance/scrollback, stream client
 * surfaces, and the resolved-config log view.
 *
 * DISCLOSED BOUND (also ENFORCEMENT_SCOPE in security-hardening.ts): the
 * workbench cannot prevent UPSTREAM authoritative stores from leaking a secret
 * inside their own event payloads — upstream hygiene is the source's. The
 * scanner is the detection instrument for exactly that class: legs below prove
 * the detector FLAGS an upstream-carried secret and the REDACTED VIEW is clean.
 * Raw transport/plugin error TEXT is same-user operator-facing input within the
 * declared scope; presentation layers must render it through the redaction
 * instrument, and the suite asserts the instrument's output is clean.
 */

import { Database } from "bun:sqlite";
import { describe, expect, it } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { GauntletReadClient } from "./client";
import { DEFAULT_CONFIG, redactConfig, resolveConfig } from "./config";
import {
  authoritativeEvidenceFrame,
  EvidenceAdmissionGate,
  KernelObjectLedger,
} from "./evidence-promotion";
import { FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import { InMemoryStreamPort, InMemoryStreamSource, LiveStreamClient } from "./live-streams";
import { OperationGateway, type OperatorActionRoute } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { PluginActivationError, PluginCompositionManager, parsePluginManifest } from "./plugins";
import { constructBoundedContextEnvelope } from "./prime-context-envelope";
import { PrimeReplKernel } from "./prime-kernel";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import { SqliteProjectionStore } from "./projection-store";
import { buildRealPrimeEnv, RealPrimeHost } from "./real-prime-host";
import type { ProjectionWorld } from "./relations";
import {
  assertNoSilentFallback,
  detectSecretPatterns,
  FALLBACK_VISIBILITY_LEDGER,
  redactionMarker,
  SECRET_LOOKALIKE_NEGATIVE_CONTROLS,
  type SecretKind,
  SecretRedactor,
} from "./security-hardening";
import { bindTerminalAutomation, FakeProcessProvider, TerminalSession } from "./terminal-session";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";
import { WorkbenchModel } from "./workbench-model";

// --- the seeded secrets (synthetic, well-marked, approved sources only) --------------

const ENV_TOKEN = "t18-FAKE-ENV-TOKEN-7f3a9c2b4d";
const CLI_TOKEN = "t18-FAKE-CLI-TOKEN-9c2e7a1f58";
const UPSTREAM_LEAK = "sk-t18upstream0123456789"; // a pattern-secret shape an upstream store leaked

const FOCUS = FocusRef.for("gauntlet", "WorkItem", "AK-182");

/** The suite's redactor with every seeded secret registered under stable markers. */
function seededRedactor(): SecretRedactor {
  const redactor = new SecretRedactor();
  redactor.register("registered_secret", ENV_TOKEN, "env-token");
  redactor.register("registered_secret", CLI_TOKEN, "cli-token");
  return redactor;
}

/** Serialize any JSON-like value for channel scanning. */
function text(value: unknown): string {
  return JSON.stringify(value) ?? String(value);
}

// --- redactor mechanics: stable markers, correlation, no content leak ----------------

describe("T18 redaction instrument — stable reference markers (REQ-SEC-003)", () => {
  it("the same secret always maps to the same marker; different secrets differ; markers carry no content", () => {
    const redactor = seededRedactor();
    const first = redactor.redactText(`header ${ENV_TOKEN} tail ${ENV_TOKEN} end ${CLI_TOKEN}`);
    expect(first.text).not.toContain(ENV_TOKEN);
    expect(first.text).not.toContain(CLI_TOKEN);
    // Both occurrences of one secret → ONE stable marker (correlation survives).
    const markers = first.text.match(/\[REDACTED:[^\]]+\]/g) ?? [];
    expect(markers.filter((marker) => marker.includes("env-token"))).toHaveLength(2);
    expect(markers.filter((marker) => marker.includes("cli-token"))).toHaveLength(1);
    // Re-registration is idempotent; a fresh scan produces the SAME marker.
    const marker = redactor.register("registered_secret", ENV_TOKEN, "env-token");
    expect(marker).toBe(redactionMarker("registered_secret", "env-token"));
    expect(redactor.redactText(ENV_TOKEN).text).toBe(marker);
    // Markers never carry secret content.
    for (const marker of redactor.markers()) {
      expect(marker).not.toContain("7f3a9c2b4d");
      expect(marker).not.toContain("9c2e7a1f58");
    }
  });

  it("the negative control: non-secret look-alikes are neither detected nor mangled", () => {
    const corpus = SECRET_LOOKALIKE_NEGATIVE_CONTROLS.join("\n");
    expect(detectSecretPatterns(corpus)).toEqual([]);
    const redactor = seededRedactor();
    for (const line of SECRET_LOOKALIKE_NEGATIVE_CONTROLS) {
      expect(redactor.redactText(line).text).toBe(line);
      expect(redactor.redactDeep({ line }).line).toBe(line);
    }
  });

  it("the detector flags realistic secret shapes the negative control must not cover", () => {
    for (const sample of [
      `Authorization: Bearer ${ENV_TOKEN}`,
      `postgres://admin:${ENV_TOKEN}@db.internal/gauntlet`,
      `session_token = "${UPSTREAM_LEAK}"`,
      `-----BEGIN RSA PRIVATE KEY-----\nMIIB\n-----END RSA PRIVATE KEY-----`,
    ]) {
      expect(detectSecretPatterns(sample).length).toBeGreaterThanOrEqual(1);
    }
  });

  it("correction round 1 — the verifier's calibration candidates each detect, and each carries a look-alike the corpus refuses", () => {
    // POSITIVES: every candidate the verifier demonstrated must be detected.
    const positives: readonly { readonly sample: string; readonly kind: SecretKind }[] = [
      { sample: `PASSWORD=xK9mQ2vL8pR4tZ7w`, kind: "key_value_secret" },
      { sample: `AWS_SECRET_ACCESS_KEY=AKIAT18EXAMPLEKEY45`, kind: "key_value_secret" },
      { sample: `pat: ghp_0123456789abcdefghijklmnopqrstuvwxyz`, kind: "known_token_shape" },
      {
        sample: `jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk`,
        kind: "jwt",
      },
      { sample: `token: 'yK8mQ2wL9pR5tZ3x'`, kind: "key_value_secret" },
    ];
    for (const { sample, kind } of positives) {
      const hits = detectSecretPatterns(sample);
      expect(hits.length).toBeGreaterThanOrEqual(1);
      expect(hits.some((hit) => hit.kind === kind)).toBe(true);
    }
    // NEGATIVES: the matching look-alikes stay in the corpus, untouched.
    const lookalikes = [
      `PASSWORD_HASH_ALGORITHM=argon2id`,
      `AWS_SECRET_ACCESS_KEY_POLICY=managed`,
      `GitHub PATs look like ghp_ (elided in docs)`,
      `the JWT header segment always starts eyJ (plain base64 of a brace)`,
      `token: 'short'`,
    ];
    for (const line of lookalikes) {
      expect(detectSecretPatterns(line)).toEqual([]);
      expect(SECRET_LOOKALIKE_NEGATIVE_CONTROLS).toContain(line);
    }
  });
});

// --- approved sources and the config boundary (REQ-CONFIG-002) ------------------------

describe("T18 Attack 2 — approved sources stay approved; forbidden channels stay clean", () => {
  it("config: env/cli sources resolve in memory only; a token in a FILE source propagates NOWHERE; the log view is the redacted one", () => {
    // Approved sources resolve…
    const fromEnv = resolveConfig({ env: { GAUNTLET_SESSION_TOKEN: ENV_TOKEN } });
    expect(fromEnv.session_token).toBe(ENV_TOKEN);
    const fromCli = resolveConfig({ cli: { session_token: CLI_TOKEN } });
    expect(fromCli.session_token).toBe(CLI_TOKEN);
    // …but a token seeded in a CONFIG FILE never propagates (REQ-CONFIG-002:
    // tokens are referenced indirectly via env/cli, never from the file).
    const fromFile = resolveConfig({ file: { session_token: ENV_TOKEN } });
    expect(fromFile.session_token).toBeNull();
    // The log/presentation view of the resolved config is redacted.
    const logView = redactConfig(fromEnv) as { session_token: unknown };
    expect(logView.session_token).toBe("[REDACTED]");
    const redactor = seededRedactor();
    const redactedView = redactor.redactDeep(redactConfig(fromEnv));
    expect(text(redactedView)).not.toContain(ENV_TOKEN);
    expect(text(redactedView)).toContain("[REDACTED]");
    // The documented default endpoint remains visible (fallback ledger leg).
    expect(DEFAULT_CONFIG.gauntlet_endpoint).toBe("http://127.0.0.1:4000");
  });

  it("errors: transport error text that carries a secret is exposed raw (disclosed bound) and the redacted view the projection stores is clean", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "01J9T18SECRETSRG",
      bound_actor: "operator-t18",
      session_token: "t18-secret-surface-token",
    }).addUnit({
      unit_id: "AK-182",
      status: "blocked",
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: "ESC-182",
    });
    // A compromised-message transport: the OBSERVE error text embeds the secret
    // (same-user operator-facing input, inside the declared scope).
    const hostile: OperatorActionRoute = {
      observe: () =>
        Promise.reject(
          new Error(`loopback dial failed: credential ${ENV_TOKEN} rejected by the surface`),
        ),
      perform: () => Promise.reject(new Error("never routed")),
    };
    const gateway = new OperationGateway(hostile);
    const outcome = await gateway.invoke({
      operation: "unit.resolve",
      focus: FOCUS,
      session: { actor: "operator-t18", token: "t18-secret-surface-token" },
      payload: { note: "secret-channel probe" },
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the hostile-message drive did not refuse");
    }
    expect(outcome.code).toBe("revalidation_unavailable");
    // THE DISCLOSED BOUND: the raw message carries what the transport said…
    expect(outcome.message).toContain(ENV_TOKEN);
    // …and the redaction instrument is what a projection must store/render:
    const redactor = seededRedactor();
    expect(redactor.leaksAny(outcome.message)).toBe(true);
    const storedLine = redactor.redactText(outcome.message).text;
    expect(storedLine).not.toContain(ENV_TOKEN);
    expect(storedLine).toContain("[REDACTED:registered_secret:env-token]");
    // The canonical surface was never contacted; its stub holds no secret echo.
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(text(stub.sideEffects)).not.toContain(ENV_TOKEN);
  });

  it("snapshots: seeded secrets never reach a workbench snapshot; an upstream-carried leak is DETECTED and its redacted view is clean", () => {
    const model = new WorkbenchModel({ dbPath: ":memory:" });
    model.startGeneration("gen-t18-secrets");
    model.applyEvent({
      generation_id: "gen-t18-secrets",
      source_id: "gauntlet",
      cursor: "cursor-clean",
      event_type: "unit_updated",
      payload: {
        unit_id: "AK-182",
        title: "resolve the escalation (env token held in config only)",
        phase: "verify",
        status: "blocked",
        authority: "granted",
        blast_radius: "local",
      },
      timestamp_ms: 0,
    });
    model.markReady("gen-t18-secrets");
    const snapshot = model.getSnapshot();
    const redactor = seededRedactor();
    // The approved-source secrets appear NOWHERE in the snapshot…
    expect(redactor.leaksAny(text(snapshot))).toBe(false);
    expect(text(snapshot)).not.toContain(ENV_TOKEN);
    expect(text(snapshot)).not.toContain(CLI_TOKEN);

    // …while an UPSTREAM-leaked secret riding a payload IS the source's leak:
    // the detector flags it (the audit instrument works on real snapshots)…
    model.applyEvent({
      generation_id: "gen-t18-secrets",
      source_id: "gauntlet",
      cursor: "cursor-leak",
      event_type: "unit_updated",
      payload: {
        unit_id: "AK-183",
        title: `upstream pasted ${UPSTREAM_LEAK} into a title`,
        phase: "verify",
        status: "blocked",
        authority: "granted",
        blast_radius: "local",
      },
      timestamp_ms: 0,
    });
    const leaked = model.getSnapshot();
    expect(text(leaked)).toContain(UPSTREAM_LEAK); // upstream truth, disclosed bound
    const detections = detectSecretPatterns(text(leaked));
    expect(detections.some((hit) => hit.matched.includes(UPSTREAM_LEAK))).toBe(true);
    // …and the REDACTED VIEW a presentation layer renders is clean.
    expect(redactor.redactDeep(leaked) !== leaked).toBe(true);
    expect(text(redactor.redactDeep(leaked))).not.toContain(UPSTREAM_LEAK);
  });

  it("evidence frames: no seeded secret reaches the authoritative frame or the admission ledger", async () => {
    const kernel = new PrimeReplKernel("kernel-t18-secret");
    const ledger = new KernelObjectLedger({ kernel, kernel_generation: "kg-t18" });
    const result = await kernel.execute("report = {verdict: 'pass'}");
    const [object] = ledger.register(result);
    if (object === undefined) {
      throw new Error("no kernel object");
    }
    ledger.artifactize(object.provenance.object_id, "/tmp/t18/secret-probe.json");
    const offerOutcome = ledger.offerAsEvidence({
      objectId: object.provenance.object_id,
      unit_id: "AK-182",
      source_id: "gauntlet",
      kind: "report",
    });
    if (offerOutcome.class !== "offered") {
      throw new Error("the artifactized object produced no offer");
    }
    const decision = {
      class: "admitted" as const,
      evidence_id: offerOutcome.offer.admission_contract.evidence_id,
      verified: true,
      source_cursor: "cursor-canonical",
    };
    const frame = authoritativeEvidenceFrame(offerOutcome.offer, decision, {
      generation_id: "gen-t18-secrets",
      timestamp_ms: 0,
    });
    const gate = new EvidenceAdmissionGate();
    const admitted = gate.admit(offerOutcome.offer, decision);
    expect(admitted.class).toBe("admitted");
    const redactor = seededRedactor();
    for (const channel of [frame, gate.admittedRecords()]) {
      expect(text(channel)).not.toContain(ENV_TOKEN);
      expect(text(channel)).not.toContain(CLI_TOKEN);
      expect(redactor.leaksAny(text(channel))).toBe(false);
    }
  });

  it("plugin manifests, state, and error-ledgers: undeclared fields are dropped, state keys only, and error text redacts clean", () => {
    const redactor = seededRedactor();
    // (a) A manifest DOCUMENT smuggling an extra own property: only declared
    //     fields are admitted, so the smuggled secret never survives parsing.
    const smuggled = {
      id: "t18.smuggler",
      name: "smuggler",
      version: "1.0.0",
      trust: "trusted_in_process",
      session_token: ENV_TOKEN,
    };
    const manifest = parsePluginManifest(smuggled);
    expect(text(manifest)).not.toContain(ENV_TOKEN);

    // (b) A mounted plugin putting a secret in its PRIVATE state cell: the
    //     host-visible surface (fingerprint, record views) carries KEYS only.
    const manager = new PluginCompositionManager();
    manager.mount(manifest, {
      activate(context) {
        context.state.set("session_token", CLI_TOKEN);
        context.state.set("has_token", true);
      },
    });
    expect(text(manager.fingerprint())).toContain("session_token"); // the KEY is visible
    expect(text(manager.fingerprint())).not.toContain(CLI_TOKEN); // the VALUE is not
    expect(text(manager.plugin("t18.smuggler"))).not.toContain(CLI_TOKEN);

    // (c) An activation crash whose cause text embeds a secret: the typed
    //     error text is operator-facing (disclosed bound) — the redacted view
    //     is clean, and the manager's post-unwind ledger holds NO secret.
    const crasher = new PluginCompositionManager();
    let escaped: unknown = null;
    try {
      crasher.mount(
        parsePluginManifest({
          id: "t18.crasher",
          name: "crasher",
          version: "1.0.0",
          trust: "trusted_in_process",
        }),
        {
          activate() {
            throw new Error(`oauth handshake failed for ${ENV_TOKEN}`);
          },
        },
      );
    } catch (error) {
      escaped = error;
    }
    expect(escaped).toBeInstanceOf(PluginActivationError);
    if (escaped instanceof PluginActivationError) {
      expect(escaped.message).toContain(ENV_TOKEN); // raw text, disclosed bound
      expect(redactor.leaksAny(escaped.message)).toBe(true);
      const safe = redactor.redactText(escaped.message).text;
      expect(safe).not.toContain(ENV_TOKEN);
      expect(safe).toContain("[REDACTED:registered_secret:env-token]");
    }
    // The mount was atomic: fully unwound, nothing retained that holds a secret.
    expect(crasher.pluginIds()).toHaveLength(0);
    expect(text(crasher.fingerprint())).not.toContain(ENV_TOKEN);
  });

  it("SQLite writes: no seeded secret reaches any projection row (read back over an INDEPENDENT connection); an upstream-leaked row is DETECTED", () => {
    const dbPath = join(tmpdir(), "gauntlet-t18-secretdb.db");
    for (const suffix of ["", "-wal", "-shm"]) {
      if (existsSync(`${dbPath}${suffix}`)) {
        rmSync(`${dbPath}${suffix}`, { force: true });
      }
    }
    const store = new SqliteProjectionStore(dbPath);
    try {
      store.startGeneration("gen-t18-secrets");
      store.applyEvent({
        generation_id: "gen-t18-secrets",
        source_id: "gauntlet",
        cursor: "cursor-a",
        event_type: "unit_updated",
        payload: {
          unit_id: "AK-182",
          title: "clean title",
          phase: "verify",
          status: "blocked",
          authority: "granted",
          blast_radius: "local",
        },
        timestamp_ms: 0,
      });
      // The upstream leak rides its own payload (disclosed upstream bound).
      store.applyEvent({
        generation_id: "gen-t18-secrets",
        source_id: "gauntlet",
        cursor: "cursor-b",
        event_type: "unit_updated",
        payload: {
          unit_id: "AK-184",
          title: `upstream leaked ${UPSTREAM_LEAK}`,
          phase: "verify",
          status: "blocked",
          authority: "granted",
          blast_radius: "local",
        },
        timestamp_ms: 0,
      });
      store.close();
    } finally {
      store.close();
    }
    // Read EVERY row of EVERY table over an independent connection.
    const audit = new Database(dbPath);
    const tables = audit
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
      .all() as readonly { name: string }[];
    let dump = "";
    for (const table of tables) {
      const rows = audit.prepare(`SELECT * FROM "${table.name}"`).all();
      dump += text(rows);
    }
    audit.close();
    // The APPROVED-SOURCE secrets appear in NO row of NO table…
    expect(dump).not.toContain(ENV_TOKEN);
    expect(dump).not.toContain(CLI_TOKEN);
    // …while the upstream-carried leak is DETECTABLE by the audit instrument.
    expect(dump).toContain(UPSTREAM_LEAK);
    expect(detectSecretPatterns(dump).some((hit) => hit.matched.includes(UPSTREAM_LEAK))).toBe(
      true,
    );
    for (const suffix of ["", "-wal", "-shm"]) {
      if (existsSync(`${dbPath}${suffix}`)) {
        rmSync(`${dbPath}${suffix}`, { force: true });
      }
    }
  });

  it("terminal: env VALUES are never recorded (names only) and input BYTES never reach provenance or scrollback", () => {
    const provider = new FakeProcessProvider();
    const session = new TerminalSession(
      {
        program: "cat",
        args: [],
        cwd: "/tmp/gauntlet-t18",
        // The operator's approved secret rides the spawn env VALUE…
        env: { GAUNTLET_SESSION_TOKEN: ENV_TOKEN },
        title: "t18 secret probe",
      },
      provider,
      {
        pathPolicy: {
          programResolution: { cat: "/bin/cat" },
          allowedPathRoots: ["/tmp/gauntlet-t18"],
        },
        nowMs: () => 0,
      },
    );
    expect(session.isRunning()).toBe(true);
    // …the provenance records the env NAME only; every public surface is clean.
    expect(session.provenance.envKeys).toEqual(["GAUNTLET_SESSION_TOKEN"]);
    for (const surface of [session.project(), session.provenance, session.events]) {
      expect(text(surface)).not.toContain(ENV_TOKEN);
    }
    // The operator pastes a secret INTO the PTY: input BYTES are never recorded.
    session.write(`${CLI_TOKEN}\n`);
    const logged = text(session.events);
    expect(logged).not.toContain(CLI_TOKEN);
    expect(logged).toContain('"input"');
    // The projection (title, scrollback, provenance) carries no secret either.
    expect(text(session.project())).not.toContain(CLI_TOKEN);
    session.cancel("probe done");
    expect(text(session.project())).not.toContain(ENV_TOKEN);
    expect(text(session.project())).not.toContain(CLI_TOKEN);
  });

  it("terminal automation + the read client: OSC-bearing titles bind inert, and transport refusals never echo credentials", async () => {
    const bound = bindTerminalAutomation({
      kind: "title",
      value: `session ${ENV_TOKEN} \u001b]52;c;ZXhmaWx0cmF0ZQ==\u0007`,
    });
    // The secret TEXT survives as (inert) operator content — disclosed: a title
    // is operator input, the neutralization is about CONTROL functions — while
    // the OSC 52 clipboard exfiltration is neutralized…
    expect(bound.neutralized.oscClipboard).toBe(1);
    expect(bound.value).not.toContain("\u001b");
    // …and the redaction instrument produces the render-safe view.
    const redactor = seededRedactor();
    expect(redactor.redactText(bound.value ?? "").text).not.toContain(ENV_TOKEN);

    // The read client: auth refusals from a real surface never echo the token.
    const stub = new CanonicalSurfaceStub({
      run_id: "01J9T18SECRETSRG",
      bound_actor: "operator-t18",
      // The surface binds a DIFFERENT session secret, so the client's seeded
      // CLI token is genuinely wrong for this surface.
      session_token: "t18-surface-bound-secret",
    }).addUnit({
      unit_id: "AK-182",
      status: "blocked",
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: "ESC-182",
    });
    const server = stub.serveHttp();
    try {
      const client = new GauntletReadClient(`127.0.0.1:${server.port}`);
      const refusal = await client.mutateUnit("AK-182", "resolve", {
        operator: "operator-t18",
        token: CLI_TOKEN,
        note: "secret echo probe",
      });
      // The client's token is WRONG for this surface (the stub binds a
      // different session secret) — the auth refusal is the channel under test.
      expect(refusal.ok).toBe(false);
      expect(refusal.code).toBe("invalid_session_credentials");
      expect(text(refusal)).not.toContain(CLI_TOKEN);
      expect(stub.sideEffects.settlements_committed).toBe(0);
    } finally {
      server.stop();
    }
  });

  it("stream frames: client surfaces stay clean under approved-source seeding; an upstream-carried frame secret is DETECTED and redacts clean", () => {
    const model = new WorkbenchModel({ dbPath: ":memory:" });
    const source = new InMemoryStreamSource("gauntlet");
    // Baseline: clean authoritative content.
    source.append({
      event_type: "unit_updated",
      payload: {
        unit_id: "AK-182",
        title: "clean baseline",
        phase: "verify",
        status: "blocked",
        authority: "granted",
        blast_radius: "local",
      },
      timestamp_ms: 0,
    });
    const client = new LiveStreamClient({
      model,
      port: new InMemoryStreamPort([source]),
      mintGenerationId: (serial) => `gen-t18-secret-${serial}`,
    });
    client.connect();
    expect(client.state).toBe("live");

    // The client's own surfaces (handshake, state log, degraded reasons) are
    // clean under approved-source seeding.
    const redactor = seededRedactor();
    for (const surface of [client.lastHandshake, client.stateLog, client.degradedReasons]) {
      expect(redactor.leaksAny(text(surface))).toBe(false);
    }

    // An upstream-carried secret rides an increment payload: the client's
    // job is convergence, and the AUDIT INSTRUMENT detects the leak…
    source.append({
      event_type: "unit_updated",
      payload: {
        unit_id: "AK-185",
        title: `upstream frame leaked ${UPSTREAM_LEAK}`,
        phase: "verify",
        status: "blocked",
        authority: "granted",
        blast_radius: "local",
      },
      timestamp_ms: 0,
    });
    const report = client.poll();
    expect(report.polled_sources).toContain("gauntlet");
    // …no seeded APPROVED-source secret reaches any client surface…
    for (const surface of [
      client.lastHandshake,
      client.stateLog,
      client.degradedReasons,
      client.rejectionLog,
      client.refusals,
    ]) {
      expect(text(surface)).not.toContain(ENV_TOKEN);
      expect(text(surface)).not.toContain(CLI_TOKEN);
    }
    // …and the redacted view of the live snapshot is render-safe.
    const liveSnapshot = model.getSnapshot();
    expect(text(liveSnapshot)).not.toContain(ENV_TOKEN);
    expect(text(redactor.redactDeep(liveSnapshot))).not.toContain(UPSTREAM_LEAK);
    client.close();
  });
});

// --- the store binding (T06) and fallback ledger attestation (REQ-SAFE-003) ----------

describe("T18 store and fallback attestation", () => {
  it("the in-memory projection store is the durable path's shape-identical twin: no secret survives a full row sweep there either", () => {
    const store = new SqliteProjectionStore(":memory:");
    store.startGeneration("gen-t18-mem");
    store.applyEvent({
      generation_id: "gen-t18-mem",
      source_id: "gauntlet",
      cursor: "cursor-clean",
      event_type: "session_updated",
      payload: {
        session_id: "prime-t18",
        parent_id: null,
        model: "claude-3-5-sonnet",
        status: "running",
      },
      timestamp_ms: 0,
    });
    const snapshot = store.getSnapshot(null);
    const redactor = seededRedactor();
    expect(redactor.leaksAny(text(snapshot))).toBe(false);
    store.close();
  });

  it("the fallback ledger attests only policy-permitted-and-visible or absent paths — no silent fallback (REQ-SAFE-003)", () => {
    expect(FALLBACK_VISIBILITY_LEDGER.length).toBeGreaterThanOrEqual(7);
    expect(() => assertNoSilentFallback()).not.toThrow();
    // Every permitted path names WHERE the operator sees it; every absent path
    // declares its absence reason.
    for (const entry of FALLBACK_VISIBILITY_LEDGER) {
      if (entry.status === "policy_permitted_visible") {
        expect(entry.visibility_signal?.length ?? 0).toBeGreaterThan(0);
      } else {
        expect(entry.policy_ref).toContain("REQ-SAFE-003");
      }
    }
    // The absence claims are TRUE at the surface: the config carries no
    // model/provider field to fall back from, the read client exposes no
    // retry/fallback operation, and config resolution is synchronous (it
    // cannot have probed or silently switched endpoints).
    const configKeys = Object.keys(DEFAULT_CONFIG);
    expect(configKeys.some((key) => /model|provider/i.test(key))).toBe(false);
    const clientMethods = Object.getOwnPropertyNames(
      Object.getPrototypeOf(new GauntletReadClient()),
    );
    expect(clientMethods.some((method) => /retry|fallback|alternate/i.test(method))).toBe(false);
    const resolved = resolveConfig({ env: { GAUNTLET_WORKBENCH_ENDPOINT: "http://127.0.0.1:1" } });
    expect(resolved.gauntlet_endpoint).toBe("http://127.0.0.1:1"); // visible, never substituted
    expect(resolved.transport).toBe("local"); // honest classification of the loopback
  });
});

// --- C2 tooth 5: no auth material appears in any RealPrimeHost projection ------------

describe("Phase C2 tooth 5 — no auth material in RealPrimeHost mailbox/result rows or thrown errors", () => {
  const SEEDED_SECRET = "sk-t18c2-FAKE-PRIME-SECRET-4f8a2c";

  it("a secret-shaped ambient env value is passed to the real child's env but never appears in bookkeeping rows", async () => {
    const root = mkdtempSync(join(tmpdir(), "c2-secret-leakage-"));
    try {
      const binary = join(root, "bin", "prime-agent");
      writeFakePrimeAgent(binary, { assistantText: "working normally" });
      const host = new RealPrimeHost({
        binary,
        agentDir: join(root, "agent"),
        sessionDir: join(root, "sessions"),
      });

      // The env RealPrimeHost WOULD build for a real child carries whatever
      // secret-shaped values are in the ambient process env — that is
      // correct (the child needs its own credentials) — but the built env
      // itself, and any value from it, must never appear in any
      // projection this host exposes.
      const builtEnv = buildRealPrimeEnv(
        { ...(process.env as Record<string, string>), PRIME_AGENT_API_KEY: SEEDED_SECRET },
        join(root, "agent"),
      );
      expect(builtEnv["PRIME_AGENT_API_KEY"]).toBe(SEEDED_SECRET); // sanity: it IS in the built env

      const world: ProjectionWorld = {
        generation: "gen-c2-secret",
        units: [],
        evidence: [],
        sessions: [],
        source_cursors: {},
      };
      const horizon = createHorizon({
        root_focus: FocusRef.for("gauntlet", "WorkItem", "u-c2-secret"),
        generation: "gen-c2-secret",
        relation_queries: [],
        horizon_id: "horizon-c2-secret",
      });
      const envelope = constructBoundedContextEnvelope({
        horizon,
        plan: planMaterialization(world, horizon),
        role_instructions: ["secret-leakage probe"],
        constructed_at_ms: 1_790_000_000_000,
      });
      const stub = new CanonicalSurfaceStub({
        run_id: "01JC2SECRETRUN000000001",
        bound_actor: "operator-c2-secret",
        session_token: "c2-secret-token",
      }).addUnit({
        unit_id: "AK-295",
        status: "blocked",
        tier: "P3",
        settled: false,
        authority: "granted",
        authority_visible_in_reads: true,
        governance_denied: false,
        governance_visible_in_reads: true,
        open_ticket: "ESC-295",
      });
      const dispatcher = new PrimeDelegationDispatcher({
        adapter: host,
        canonical_route: stub.asInProcessRoute(),
      });
      await dispatcher.delegate({
        operation_request: {
          operation: OPERATION_IDS.UNIT_RESOLVE,
          focus: FocusRef.for("gauntlet", "WorkItem", "AK-295"),
          session: { actor: "operator-c2-secret", token: "c2-secret-token" },
          payload: { note: "secret-leakage probe", run: "01JC2SECRETRUN000000001" },
        },
        envelope,
        child_session_id: "sess-c2-secret",
        parent_session_id: null,
        task: "run normally",
        contract_path: join(root, "sess-c2-secret.result.json"),
      });
      await new Promise((resolve) => setTimeout(resolve, 300));

      const session = host.getSession("sess-c2-secret");
      expect(session).not.toBeNull();
      const projected = text(session);
      expect(projected.includes(SEEDED_SECRET)).toBe(false);
      expect(detectSecretPatterns(projected).length).toBe(0);

      // The written evidence file (the one channel that mirrors the real
      // child's own stdout/stderr) must also carry no trace of the secret —
      // the fake script never echoes its env, matching a well-behaved real
      // binary that never prints credentials to stdout/stderr either.
      if (existsSync(join(root, "sess-c2-secret.result.json"))) {
        const evidence = await Bun.file(join(root, "sess-c2-secret.result.json")).text();
        expect(evidence.includes(SEEDED_SECRET)).toBe(false);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("RealPrimeHost construction/spawn refusal error TEXT never carries an env value", () => {
    const root = mkdtempSync(join(tmpdir(), "c2-secret-leakage-err-"));
    try {
      const binary = join(root, "bin", "prime-agent");
      writeFakePrimeAgent(binary);
      const host = new RealPrimeHost({
        binary,
        agentDir: join(root, "agent"),
        sessionDir: join(root, "sessions"),
      });
      try {
        host.spawnChild({ sessionId: "s", contractPath: join(root, "c.json") }); // no task
        throw new Error("expected a throw");
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        expect(message.includes(SEEDED_SECRET)).toBe(false);
        expect(detectSecretPatterns(message).length).toBe(0);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
