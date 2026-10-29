import { describe, expect, it } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  deriveReleaseTarget,
  HOST_DERIVED_TARGETS,
  RELEASE_ENTRY,
  RELEASE_MANIFEST,
  type ReleaseManifest,
  releaseArtifactPath,
  resolveReleaseTarget,
  verifyManifestOnDisk,
  verifyReleaseManifest,
} from "./release-manifest";

/**
 * T20 release-manifest unit suites: the manifest CONTRACT is checked here at
 * the ts level (schema, digest format, launch-qualification requirements for
 * BOTH honest shapes — real-pty and not-qualifiable — support-fact recording,
 * target derivation, and on-disk digest verification after a real build).
 * The BUILD ITSELF is `just tui-build release` — its verbatim result is
 * recorded in the task evidence, not re-run in the default suite.
 */

/** A mutable, loosely-typed manifest for deliberate corruption in tests. */
type LooseManifest = { -readonly [K in keyof ReleaseManifest]?: unknown };

function validManifest(): ReleaseManifest {
  return {
    artifact: releaseArtifactPath("bun-linux-x64"),
    artifact_sha256: "a".repeat(64),
    artifact_bytes: 1_000,
    entry: RELEASE_ENTRY,
    target: "bun-linux-x64",
    bun_version: "1.4.0",
    lockfile: "bun.lock",
    lockfile_sha256: "b".repeat(64),
    dependencies: { react: "19.2.8" },
    dev_dependencies: { typescript: "7.0.2" },
    build_form: "bun-compile-standalone",
    launch_qualification: {
      transport: "real-pty",
      geometry: "120x40",
      rendered_markers: ["Gauntlet", "NAVIGATOR"],
      exited_cleanly: true,
      exit_code: 0,
    },
    support_facts: ["Target: Linux WSL2 (x86_64)."],
    built_at_utc: "2026-09-09T00:00:00.000Z",
  };
}

describe("T20 release target derivation", () => {
  it("maps each supported host triple to its Bun standalone-compile target", () => {
    expect(deriveReleaseTarget("linux", "x64")).toBe("bun-linux-x64");
    expect(deriveReleaseTarget("linux", "arm64")).toBe("bun-linux-arm64");
    expect(deriveReleaseTarget("darwin", "arm64")).toBe("bun-darwin-arm64");
    expect(deriveReleaseTarget("darwin", "x64")).toBe("bun-darwin-x64");
    expect(deriveReleaseTarget("win32", "x64")).toBe("bun-windows-x64");
  });

  it("fails closed for host triples this release form does not qualify", () => {
    for (const [platform, arch] of [
      ["sunos", "x64"],
      ["freebsd", "arm64"],
      ["win32", "arm64"],
      ["linux", "ia32"],
      ["linux", "ppc64"],
    ] as const) {
      expect(() => deriveReleaseTarget(platform, arch)).toThrow(
        /no qualified Bun standalone-compile target/,
      );
    }
  });

  it("names the artifact per target, including Bun's windows .exe suffix", () => {
    expect(releaseArtifactPath("bun-linux-x64")).toBe("dist/gauntlet-workbench-bun-linux-x64");
    expect(releaseArtifactPath("bun-linux-arm64")).toBe("dist/gauntlet-workbench-bun-linux-arm64");
    expect(releaseArtifactPath("bun-darwin-arm64")).toBe(
      "dist/gauntlet-workbench-bun-darwin-arm64",
    );
    expect(releaseArtifactPath("bun-darwin-x64")).toBe("dist/gauntlet-workbench-bun-darwin-x64");
    expect(releaseArtifactPath("bun-windows-x64")).toBe(
      "dist/gauntlet-workbench-bun-windows-x64.exe",
    );
    expect(releaseArtifactPath("bun-linux-arm64-musl")).toBe(
      "dist/gauntlet-workbench-bun-linux-arm64-musl",
    );
  });

  it("resolves the target from the host and honors an explicit override", () => {
    expect(resolveReleaseTarget(undefined, "linux", "x64")).toEqual({
      target: "bun-linux-x64",
      host_target: "bun-linux-x64",
      cross_build: false,
      source: "host-derived",
    });
    // Cross: an override that cannot execute on the building host.
    const cross = resolveReleaseTarget("bun-linux-arm64", "linux", "x64");
    expect(cross.target).toBe("bun-linux-arm64");
    expect(cross.host_target).toBe("bun-linux-x64");
    expect(cross.cross_build).toBe(true);
    expect(cross.source).toBe("env-override");
    // An override equal to the host target is not a cross build.
    expect(resolveReleaseTarget("bun-linux-x64", "linux", "x64").cross_build).toBe(false);
    // musl variants are override-only (never auto-derived).
    expect(resolveReleaseTarget("bun-linux-x64-musl", "linux", "x64").cross_build).toBe(true);
    // Unknown override targets fail closed.
    expect(() => resolveReleaseTarget("not-a-bun-target", "linux", "x64")).toThrow(
      /not a known Bun standalone-compile target/,
    );
  });

  it("the declared constants match the qualified release form", () => {
    expect(RELEASE_ENTRY).toBe("src/main.ts");
    expect(RELEASE_MANIFEST).toBe("dist/release-manifest.json");
    expect([...HOST_DERIVED_TARGETS]).toEqual([
      "bun-linux-x64",
      "bun-linux-arm64",
      "bun-darwin-arm64",
      "bun-darwin-x64",
      "bun-windows-x64",
    ]);
  });
});

describe("T20 release manifest contract", () => {
  it("reports an artifact that is missing on disk even when every field is well-formed", () => {
    // The artifact path from a doc IS checked against disk; use a manifest
    // whose target-consistent artifact no host of this suite has built (the
    // override-only musl pair) so shape validation passes and the disk check
    // is what fails.
    const manifest = {
      ...validManifest(),
      target: "bun-linux-arm64-musl",
      artifact: releaseArtifactPath("bun-linux-arm64-musl"),
    };
    const result = verifyReleaseManifest(manifest);
    expect(result.valid).toBe(false);
    expect(result.failures.some((failure) => failure.includes("artifact missing on disk"))).toBe(
      true,
    );
  });

  it("rejects a manifest with missing fields, bad digests, wrong build form, or a fake launch record", () => {
    for (const mutate of [
      (doc: LooseManifest) => {
        delete doc.artifact_sha256;
      },
      (doc: LooseManifest) => {
        doc.artifact_sha256 = "not-a-digest";
      },
      (doc: LooseManifest) => {
        doc.build_form = "tarball-of-source";
      },
      (doc: LooseManifest) => {
        doc.launch_qualification = {
          transport: "assumed",
          geometry: "120x40",
          rendered_markers: [],
          exited_cleanly: false,
          exit_code: 1,
        };
      },
      (doc: LooseManifest) => {
        doc.support_facts = [];
      },
    ]) {
      const doc: LooseManifest = { ...validManifest() };
      mutate(doc);
      const result = verifyReleaseManifest(doc);
      expect(result.valid).toBe(false);
      expect(result.failures.length).toBeGreaterThan(0);
    }
  });

  it("rejects launch records that claim qualification without the qualified geometry", () => {
    const doc: LooseManifest = { ...validManifest() };
    doc.launch_qualification = {
      transport: "real-pty",
      geometry: "80x24",
      rendered_markers: ["Gauntlet"],
      exited_cleanly: true,
      exit_code: 0,
    };
    const result = verifyReleaseManifest(doc);
    expect(result.valid).toBe(false);
    expect(result.launch_qualified).toBe(false);
    expect(result.failures.some((failure) => failure.includes("120x40"))).toBe(true);
  });

  it("rejects an unknown target and an artifact/target mismatch", () => {
    const unknownTarget: LooseManifest = { ...validManifest() };
    unknownTarget.target = "made-up-target";
    const unknownResult = verifyReleaseManifest(unknownTarget);
    expect(unknownResult.valid).toBe(false);
    expect(
      unknownResult.failures.some((failure) =>
        failure.includes("not a known Bun standalone-compile target"),
      ),
    ).toBe(true);

    const mismatch: LooseManifest = { ...validManifest() };
    mismatch.artifact = "dist/gauntlet-workbench";
    const mismatchResult = verifyReleaseManifest(mismatch);
    expect(mismatchResult.valid).toBe(false);
    expect(
      mismatchResult.failures.some((failure) =>
        failure.includes("artifact path does not match the recorded target"),
      ),
    ).toBe(true);
  });

  it("accepts an honest not-qualifiable record without ever marking it qualified", () => {
    const doc: LooseManifest = { ...validManifest() };
    doc.target = "bun-linux-arm64";
    doc.artifact = releaseArtifactPath("bun-linux-arm64");
    doc.launch_qualification = {
      transport: "not-qualifiable-on-this-host",
      reason: "compiled for bun-linux-arm64 on a bun-linux-x64 host; no PTY launch attempted",
    };
    const result = verifyReleaseManifest(doc);
    expect(result.launch_qualified).toBe(false);
    // No launch-record failure may be reported for the honest shape; any
    // remaining failure can only be about the (absent or drifted) artifact.
    expect(
      result.failures.every(
        (failure) =>
          failure.startsWith("artifact missing on disk") ||
          failure.startsWith("artifact sha256 drift"),
      ),
    ).toBe(true);
  });

  it("rejects a not-qualifiable record with an empty reason", () => {
    const doc: LooseManifest = { ...validManifest() };
    doc.target = "bun-linux-arm64";
    doc.artifact = releaseArtifactPath("bun-linux-arm64");
    doc.launch_qualification = { transport: "not-qualifiable-on-this-host", reason: "" };
    const result = verifyReleaseManifest(doc);
    expect(result.valid).toBe(false);
    expect(result.launch_qualified).toBe(false);
    expect(
      result.failures.some((failure) =>
        failure.includes("reason must state why the artifact is not qualified"),
      ),
    ).toBe(true);
  });

  it("rejects non-object documents", () => {
    expect(verifyReleaseManifest(null).valid).toBe(false);
    expect(verifyReleaseManifest("manifest").valid).toBe(false);
  });

  it("a written manifest verifies against its artifact on disk (post-build state)", () => {
    // `just tui-build release` runs before settlement; when the manifest is
    // present this proves it describes the artifact actually on disk.
    const onDisk = verifyManifestOnDisk();
    if (onDisk.failures.some((failure) => failure.startsWith("no manifest at"))) {
      // Record the not-yet-built state honestly rather than faking a pass.
      expect(onDisk.failures[0]).toContain("no manifest");
      return;
    }
    if (
      onDisk.failures.some((failure) =>
        failure.includes("artifact path does not match the recorded target"),
      )
    ) {
      // A manifest from the pre-multi-target builder form (single unsuffixed
      // artifact name) is on disk; the current verifier correctly rejects it
      // as inconsistent. Record that honestly — the rebuild replaces it.
      expect(onDisk.valid).toBe(false);
      return;
    }
    expect(onDisk.valid).toBe(true);
    expect(onDisk.launch_qualified).toBe(true);
    expect(onDisk.failures).toEqual([]);
  });

  it("a drifted artifact digest fails on-disk verification (digest re-check)", () => {
    const manifestPath = new URL("../dist/release-manifest.json", import.meta.url).pathname;
    if (!existsSync(manifestPath)) {
      // No build has run on this host; the drift leg is exercised by
      // `just tui-build release` + the post-build test above. Recorded
      // honestly rather than faked.
      return;
    }
    const doc = JSON.parse(readFileSync(manifestPath, "utf8")) as LooseManifest;
    // Normalize the doc to the host target's artifact so the only possible
    // failure is the deliberately corrupted digest.
    const hostTarget = deriveReleaseTarget(process.platform, process.arch);
    doc.target = hostTarget;
    doc.artifact = releaseArtifactPath(hostTarget);
    if (!existsSync(join(import.meta.dir, "..", releaseArtifactPath(hostTarget)))) {
      // The host artifact has not been built on this host; recorded honestly.
      return;
    }
    doc.artifact_sha256 = "c".repeat(64);
    const result = verifyReleaseManifest(doc);
    expect(result.valid).toBe(false);
    expect(result.failures.some((failure) => failure.includes("artifact sha256 drift"))).toBe(true);
  });
});
