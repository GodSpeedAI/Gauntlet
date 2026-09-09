import { describe, expect, it } from "bun:test";
import type {
  OutOfProcessCapabilityRequest,
  OutOfProcessCapabilityResponse,
  PluginManifest,
} from "./plugins";
import {
  createShellViewRegistryCell,
  decideTrustPlacement,
  PluginBoundaryError,
  PluginCompositionManager,
  PluginLifecycleError,
  PluginManifestError,
  PluginTrustRefusalError,
  parsePluginManifest,
  serializePluginSurfaceFingerprint,
} from "./plugins";
import { createSamplePlugin, SAMPLE_PLUGIN_ID } from "./testing/sample-plugin";
import { coldModel } from "./testing/shell-harness";
import { defaultViewRegistry } from "./views-chat-trace";

/**
 * T15 plugin trust suites (REQ-PLUGIN-002, REQ-SEC-004): in-process trust is
 * EXPLICIT and never inferred; an untrusted module marked for in-process
 * loading is refused with a typed policy refusal (plan Attack 2); untrusted
 * code is confined to the narrow out-of-process capability boundary whose
 * requests and responses are serialized strings validated before admission.
 */

const TRUSTED_MANIFEST = {
  id: "sample.provenance-lens",
  name: "Provenance Lens (representative T15 plugin)",
  version: "1.0.0",
  trust: "trusted_in_process",
} as const;

function untrustedManifest(): {
  id: string;
  name: string;
  version: string;
  trust: "untrusted_out_of_process";
  capabilities: string[];
} {
  return {
    id: "external.witness",
    name: "External witness plugin",
    version: "0.9.0",
    trust: "untrusted_out_of_process",
    capabilities: ["render_artifact", "provide_cue"],
  };
}

function emptyManager(): PluginCompositionManager {
  return new PluginCompositionManager({
    snapshots: coldModel(),
    shellViews: createShellViewRegistryCell(defaultViewRegistry()),
  });
}

function recordingTransport(
  respond: (request: OutOfProcessCapabilityRequest) => OutOfProcessCapabilityResponse,
): {
  transport: (request: OutOfProcessCapabilityRequest) => Promise<OutOfProcessCapabilityResponse>;
  requests: OutOfProcessCapabilityRequest[];
} {
  const requests: OutOfProcessCapabilityRequest[] = [];
  return {
    requests,
    transport: (request) => {
      requests.push(request);
      return Promise.resolve(respond(request));
    },
  };
}

describe("T15 trust — trust placement is explicit, never inferred (REQ-PLUGIN-002, REQ-SEC-004)", () => {
  it("a manifest without a declared trust class is refused (trust_not_declared)", () => {
    expect(() =>
      parsePluginManifest({ id: "external.witness", name: "Witness", version: "1.0.0" }),
    ).toThrow(PluginManifestError);
    try {
      parsePluginManifest({ id: "external.witness", name: "Witness", version: "1.0.0" });
    } catch (error) {
      expect((error as PluginManifestError).code).toBe("trust_not_declared");
    }
  });

  it("a manifest with a non-member trust class is refused (invalid_trust_class)", () => {
    expect(() =>
      parsePluginManifest({
        id: "external.witness",
        name: "Witness",
        version: "1.0.0",
        trust: "trusted",
      }),
    ).toThrow(PluginManifestError);
    try {
      parsePluginManifest({
        id: "external.witness",
        name: "Witness",
        version: "1.0.0",
        trust: "trusted",
      });
    } catch (error) {
      expect((error as PluginManifestError).code).toBe("invalid_trust_class");
    }
  });

  it("malformed manifests are refused before anything is admitted", () => {
    expect(() => parsePluginManifest(null)).toThrow(PluginManifestError);
    expect(() =>
      parsePluginManifest({ name: "no id", version: "1.0.0", trust: "trusted_in_process" }),
    ).toThrow(PluginManifestError);
    expect(() =>
      parsePluginManifest({
        id: "Bad Id",
        name: "n",
        version: "1.0.0",
        trust: "trusted_in_process",
      }),
    ).toThrow(PluginManifestError);
    // An untrusted manifest without a capability allowlist is refused.
    try {
      parsePluginManifest({
        id: "external.witness",
        name: "Witness",
        version: "1.0.0",
        trust: "untrusted_out_of_process",
      });
      throw new Error("unreachable");
    } catch (error) {
      expect((error as PluginManifestError).code).toBe("invalid_capability_list");
    }
  });

  it("the trust policy decision is pure and follows the declaration", () => {
    const trusted = parsePluginManifest(TRUSTED_MANIFEST);
    const trustedDecision = decideTrustPlacement(trusted);
    expect(trustedDecision.verdict).toBe("load_in_process");

    const untrusted = parsePluginManifest(untrustedManifest());
    const untrustedDecision = decideTrustPlacement(untrusted);
    expect(untrustedDecision.verdict).toBe("route_out_of_process");
    if (untrustedDecision.verdict === "route_out_of_process") {
      expect(untrustedDecision.capabilities).toEqual(["render_artifact", "provide_cue"]);
    }
  });

  it("a parsed trusted manifest enables the in-process path — the only path that activates modules", () => {
    const manager = emptyManager();
    const sample = createSamplePlugin();
    const view = manager.mount(parsePluginManifest(TRUSTED_MANIFEST), sample.module);
    expect(view.mode).toBe("in_process");
    expect(sample.probe.registrationOrder.length).toBe(13);
    manager.unload(SAMPLE_PLUGIN_ID);
  });
});

describe("T15 Attack 2 — an untrusted module marked for in-process loading", () => {
  it("is refused by trust policy; the module never activates and nothing registers", () => {
    const manager = emptyManager();
    const baselineFingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());

    let activated = false;
    const untrustedModule = {
      activate: () => {
        activated = true;
      },
    };

    const decision = decideTrustPlacement(parsePluginManifest(untrustedManifest()));
    expect(decision.verdict).toBe("route_out_of_process");

    expect(() => manager.mount(untrustedManifest(), untrustedModule)).toThrow(
      PluginTrustRefusalError,
    );
    try {
      manager.mount(untrustedManifest(), untrustedModule);
    } catch (error) {
      expect(error instanceof PluginTrustRefusalError).toBe(true);
      const refusal = error as PluginTrustRefusalError;
      expect(refusal.code).toBe("untrusted_in_process_refused");
      expect(refusal.plugin).toBe("external.witness");
    }

    // The untrusted module NEVER ran and left zero effects.
    expect(activated).toBe(false);
    expect(manager.isMounted("external.witness")).toBe(false);
    expect(manager.pluginIds()).toEqual([]);
    expect(serializePluginSurfaceFingerprint(manager.fingerprint())).toBe(baselineFingerprint);
    expect(manager.unload("external.witness")).toEqual({ unloaded: false, reason: "not_mounted" });
  });
});

describe("T15 — the out-of-process capability boundary confines untrusted code", () => {
  it("mounts with no live module and contributes nothing to any surface", () => {
    const manager = emptyManager();
    const transport = recordingTransport(() => ({ ok: true, payload: "{}" }));
    const view = manager.mountOutOfProcess(untrustedManifest(), transport.transport);

    expect(view.mode).toBe("out_of_process");
    expect(view.trust).toBe("untrusted_out_of_process");
    expect(view.contributionCount).toBe(0);
    expect(view.subscriptionCount).toBe(0);

    // No surface changed: an out-of-process plugin cannot register anything.
    const snapshot = coldModel().getSnapshot();
    expect(manager.navEntries()).toEqual([]);
    expect(manager.statusCues(snapshot)).toEqual([]);
    expect(manager.composerCues(snapshot)).toEqual([]);
    expect(manager.bottomPanels(snapshot)).toEqual([]);
    expect(manager.overlayContent(snapshot)).toEqual([]);
    expect(manager.affordanceProjections()).toEqual([]);
    const fingerprint = serializePluginSurfaceFingerprint(manager.fingerprint());
    expect(fingerprint).toContain('"mounted":["external.witness"]');
    expect(fingerprint).toContain('"contributions":[]');
    expect(fingerprint).toContain('"subscriptions":[]');

    expect(manager.unload("external.witness")).toEqual({ unloaded: true });
  });

  it("declared capability requests cross as serialized strings; responses are validated before admission", async () => {
    const manager = emptyManager();
    const transport = recordingTransport((request) => ({
      ok: true,
      payload: JSON.stringify({ rendered: [`artifact view for ${request.capability}`] }),
    }));
    manager.mountOutOfProcess(untrustedManifest(), transport.transport);

    const payload = { artifact: { id: "a-1", kind: "sample-log" }, rows: [1, 2] };
    const result = await manager.callCapability("external.witness", "render_artifact", payload);

    expect(result.ok).toBe(true);
    expect(result.value).toEqual({ rendered: ["artifact view for render_artifact"] });
    expect(transport.requests.length).toBe(1);
    const crossed = transport.requests[0];
    expect(crossed?.plugin).toBe("external.witness");
    expect(crossed?.capability).toBe("render_artifact");
    // The payload crossed SERIALIZED: a string, no live references.
    expect(typeof crossed?.payload).toBe("string");
    expect(JSON.parse(crossed?.payload ?? "null")).toEqual(payload);

    // A live function value cannot cross the boundary: serialization strips it.
    const functionPayload = { data: 1, callback: () => "never serialised" };
    await manager.callCapability("external.witness", "render_artifact", functionPayload);
    const second = transport.requests[1];
    expect(JSON.parse(second?.payload ?? "null")).toEqual({ data: 1 });
    expect(JSON.stringify(second?.payload)).not.toContain("never serialised");

    manager.unload("external.witness");
  });

  it("undeclared capabilities are refused before the transport is contacted", async () => {
    const manager = emptyManager();
    const transport = recordingTransport(() => ({ ok: true, payload: "{}" }));
    manager.mountOutOfProcess(untrustedManifest(), transport.transport);

    await expect(manager.callCapability("external.witness", "fs_read", {})).rejects.toThrow(
      PluginBoundaryError,
    );
    try {
      await manager.callCapability("external.witness", "fs_read", {});
    } catch (error) {
      expect((error as PluginBoundaryError).code).toBe("capability_not_declared");
    }
    // The boundary channel was never opened for the undeclared capability.
    expect(transport.requests.length).toBe(0);
    manager.unload("external.witness");
  });

  it("unknown plugins, in-process plugins, transport loss, and malformed responses are typed refusals", async () => {
    const manager = emptyManager();

    // Unknown plugin.
    await expect(manager.callCapability("no.such-plugin", "render_artifact", {})).rejects.toThrow(
      PluginLifecycleError,
    );

    // A trusted in-process plugin is not a boundary party.
    const sample = createSamplePlugin();
    manager.mount(sample.manifest, sample.module);
    await expect(manager.callCapability(SAMPLE_PLUGIN_ID, "render_artifact", {})).rejects.toThrow(
      PluginLifecycleError,
    );
    manager.unload(SAMPLE_PLUGIN_ID);

    // Transport loss is typed, never swallowed.
    const failing = recordingTransport(() => {
      throw new Error("child process died");
    });
    manager.mountOutOfProcess(untrustedManifest(), failing.transport);
    try {
      await manager.callCapability("external.witness", "provide_cue", {});
      throw new Error("unreachable");
    } catch (error) {
      expect((error as PluginBoundaryError).code).toBe("boundary_transport_failed");
      expect((error as PluginBoundaryError).message).toContain("child process died");
    }

    // A malformed response document is refused before admission.
    const malformed = recordingTransport(
      () => ({ ok: "yes", payload: "{}" }) as unknown as OutOfProcessCapabilityResponse,
    );
    manager.unload("external.witness");
    manager.mountOutOfProcess(untrustedManifest(), malformed.transport);
    try {
      await manager.callCapability("external.witness", "provide_cue", {});
      throw new Error("unreachable");
    } catch (error) {
      expect((error as PluginBoundaryError).code).toBe("malformed_boundary_response");
    }

    // A non-JSON response payload is refused before admission.
    manager.unload("external.witness");
    const nonJson = recordingTransport(() => ({ ok: true, payload: "not json" }));
    manager.mountOutOfProcess(untrustedManifest(), nonJson.transport);
    await expect(manager.callCapability("external.witness", "provide_cue", {})).rejects.toThrow(
      PluginBoundaryError,
    );
    manager.unload("external.witness");
  });

  it("a trusted manifest is refused at the boundary (trust placement is explicit in BOTH directions)", () => {
    const manager = emptyManager();
    const transport = recordingTransport(() => ({ ok: true, payload: "{}" }));
    expect(() => manager.mountOutOfProcess(TRUSTED_MANIFEST, transport.transport)).toThrow(
      PluginTrustRefusalError,
    );
    try {
      manager.mountOutOfProcess(TRUSTED_MANIFEST, transport.transport);
    } catch (error) {
      expect((error as PluginTrustRefusalError).code).toBe("trust_boundary_mismatch");
    }
    expect(manager.pluginIds()).toEqual([]);
  });
});

// --- Correction Round 1 (adversarial verification findings) -------------------------

describe("T15 Correction Round 1 MINOR-2 — manifest fields are read from OWN properties only", () => {
  it("a manifest whose trust (and identity) live only on the PROTOTYPE is refused, never admitted as trusted", () => {
    const inherited = Object.create({
      id: "external.inherited",
      name: "Inherited",
      version: "1.0.0",
      trust: "trusted_in_process",
    }) as PluginManifest;

    expect(() => parsePluginManifest(inherited)).toThrow(PluginManifestError);
    try {
      parsePluginManifest(inherited);
    } catch (error) {
      expect((error as PluginManifestError).code).toBe("trust_not_declared");
    }

    // The same holds at the mount boundary: the module is never activated.
    const manager = emptyManager();
    let activated = false;
    expect(() =>
      manager.mount(inherited, {
        activate: () => {
          activated = true;
        },
      }),
    ).toThrow(PluginManifestError);
    expect(activated).toBe(false);
    expect(manager.pluginIds()).toEqual([]);
  });

  it("an own-property manifest with an inherited DECOY trust still follows its own declaration", () => {
    const base = {
      trust: "trusted_in_process",
    };
    const manifest = Object.assign(Object.create(base), {
      id: "external.own-trust",
      name: "Own trust",
      version: "1.0.0",
      trust: "untrusted_out_of_process",
      capabilities: ["cap"],
    });
    const parsed = parsePluginManifest(manifest);
    expect(parsed.trust).toBe("untrusted_out_of_process");
  });
});

describe("T15 Correction Round 1 MINOR-3 — mount refuses non-object manifests with the typed formation error", () => {
  it("mount(null) and other non-object manifests throw PluginManifestError, never a raw TypeError", () => {
    const manager = emptyManager();
    for (const garbage of [null, 42, "manifest", true]) {
      let thrown: unknown = null;
      try {
        manager.mount(garbage as unknown as PluginManifest, { activate: () => undefined });
      } catch (error) {
        thrown = error;
      }
      expect(thrown).toBeInstanceOf(PluginManifestError);
      expect((thrown as PluginManifestError).code).toBe("malformed_manifest");
    }
    expect(manager.pluginIds()).toEqual([]);

    let boundaryThrown: unknown = null;
    try {
      manager.mountOutOfProcess(null as unknown as PluginManifest, () =>
        Promise.resolve({ ok: true, payload: "{}" }),
      );
    } catch (error) {
      boundaryThrown = error;
    }
    expect(boundaryThrown).toBeInstanceOf(PluginManifestError);
    expect(manager.pluginIds()).toEqual([]);
  });
});
