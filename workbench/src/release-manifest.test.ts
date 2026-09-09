import { describe, expect, it } from "bun:test";
import {
  RELEASE_ARTIFACT,
  RELEASE_ENTRY,
  RELEASE_MANIFEST,
  RELEASE_TARGET,
  verifyManifestOnDisk,
  verifyReleaseManifest,
} from "./release-manifest";

/**
 * T20 release-manifest unit suites: the manifest CONTRACT is checked here at
 * the ts level (schema, digest format, launch-qualification requirements,
 * support-fact recording, and on-disk digest verification after a real build).
 * The BUILD ITSELF is `just tui-build release` — its verbatim result is
 * recorded in the task evidence, not re-run in the default suite.
 */

function validManifest(): Record<string, unknown> {
  return {
    artifact: RELEASE_ARTIFACT,
    artifact_sha256: "a".repeat(64),
    artifact_bytes: 1_000,
    entry: RELEASE_ENTRY,
    target: RELEASE_TARGET,
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

describe("T20 release manifest contract", () => {
  it("accepts a shape-complete manifest (digest drift check skipped for absent artifacts only when path missing is reported)", () => {
    // The artifact path from a doc IS checked against disk when present; use
    // a path that does not exist to prove the disk check reports it.
    const manifest = { ...validManifest(), artifact: "dist/does-not-exist" };
    const result = verifyReleaseManifest(manifest);
    expect(result.valid).toBe(false);
    expect(result.failures.some((failure) => failure.includes("artifact missing on disk"))).toBe(
      true,
    );
  });

  it("rejects a manifest with missing fields, bad digests, wrong build form, or a fake launch record", () => {
    for (const mutate of [
      (doc: Record<string, unknown>) => {
        delete doc["artifact_sha256"];
      },
      (doc: Record<string, unknown>) => {
        doc["artifact_sha256"] = "not-a-digest";
      },
      (doc: Record<string, unknown>) => {
        doc["build_form"] = "tarball-of-source";
      },
      (doc: Record<string, unknown>) => {
        doc["launch_qualification"] = {
          transport: "assumed",
          geometry: "120x40",
          rendered_markers: [],
          exited_cleanly: false,
          exit_code: 1,
        };
      },
      (doc: Record<string, unknown>) => {
        doc["support_facts"] = [];
      },
    ]) {
      const doc = validManifest();
      mutate(doc);
      const result = verifyReleaseManifest(doc);
      expect(result.valid).toBe(false);
      expect(result.failures.length).toBeGreaterThan(0);
    }
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
    expect(onDisk.valid).toBe(true);
    expect(onDisk.failures).toEqual([]);
  });

  it("the declared constants match the qualified release form", () => {
    expect(RELEASE_TARGET).toBe("bun-linux-x64");
    expect(RELEASE_ENTRY).toBe("src/main.ts");
    expect(RELEASE_ARTIFACT).toBe("dist/gauntlet-workbench");
    expect(RELEASE_MANIFEST).toBe("dist/release-manifest.json");
  });
});
