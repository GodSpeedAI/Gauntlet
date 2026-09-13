/**
 * Discovery defaults for `project_root` / `gauntlet_binary` (operator request:
 * "set defaults ... so I don't have to").
 *
 * The invariant under test is that discovery either names a REAL Gauntlet
 * project / the REAL Gauntlet CLI, or stays null so every consumer renders
 * unavailable-with-reason. A wrong-but-plausible answer (a directory that
 * merely contains projects; the prime-agent binary in place of Gauntlet's own)
 * is worse than no answer: the first binds a child's write scope to unrelated
 * repositories, the second makes `run.start` exec `prime-agent run <intent>`.
 */

import { describe, expect, it } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { discoverGauntletBinary, discoverProjectRoot, resolveConfig } from "./config";

function scratch(): string {
  return mkdtempSync(join(tmpdir(), "gauntlet-config-discovery-"));
}

describe("project root discovery", () => {
  it("finds the project containing the launch directory, walking up", () => {
    const root = scratch();
    const project = join(root, "my-project");
    const nested = join(project, "src", "deep");
    mkdirSync(nested, { recursive: true });
    writeFileSync(join(project, "GAUNTLET.md"), "---\n---\n");

    expect(discoverProjectRoot(nested)).toBe(project);
    expect(discoverProjectRoot(project)).toBe(project);
  });

  it("refuses a directory that merely CONTAINS projects, rather than guessing one", () => {
    // The ~/projects shape: sibling projects under one parent. Returning the
    // parent would run `gauntlet run` where no reference exists and hand a
    // work-mode child every repository at once.
    const projects = scratch();
    for (const name of ["alpha", "beta"]) {
      const dir = join(projects, name);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "GAUNTLET.md"), "---\n---\n");
    }

    expect(discoverProjectRoot(projects)).toBeNull();
    // Launching inside one of them still resolves to THAT project.
    expect(discoverProjectRoot(join(projects, "beta"))).toBe(join(projects, "beta"));
  });

  it("stays null when no project encloses the launch directory", () => {
    const root = scratch();
    const plain = join(root, "not-a-project");
    mkdirSync(plain, { recursive: true });
    expect(discoverProjectRoot(plain)).toBeNull();
  });
});

describe("gauntlet binary discovery", () => {
  it("prefers an installed gauntlet on PATH", () => {
    expect(discoverGauntletBinary(null, () => "/usr/local/bin/gauntlet")).toBe(
      "/usr/local/bin/gauntlet",
    );
  });

  it("falls back to this checkout's debug build when PATH has none", () => {
    const project = scratch();
    const built = join(project, "target", "debug", "gauntlet");
    mkdirSync(join(project, "target", "debug"), { recursive: true });
    writeFileSync(built, "#!/bin/sh\n");

    expect(discoverGauntletBinary(project, () => null, existsSync, scratch())).toBe(built);
  });

  it("finds the checkout the workbench itself runs from, for a target project with no build", () => {
    // A demo or any other target project has no `gauntlet` build of its own;
    // the developer running the TUI out of the repository still has one.
    const checkout = scratch();
    const built = join(checkout, "target", "debug", "gauntlet");
    mkdirSync(join(checkout, "target", "debug"), { recursive: true });
    writeFileSync(built, "#!/bin/sh\n");
    const unrelatedProject = scratch();

    expect(
      discoverGauntletBinary(
        unrelatedProject,
        () => null,
        existsSync,
        join(checkout, "workbench", "src"),
      ),
    ).toBe(built);
  });

  it("stays null rather than naming any other binary when nothing is found", () => {
    // Specifically: never prime-agent. That is the AGENT binary; `run.start`
    // execs `<binary> run "<intent>"`, which only Gauntlet's own CLI serves.
    const nowhere = scratch();
    expect(discoverGauntletBinary(scratch(), () => null, existsSync, nowhere)).toBeNull();
    expect(discoverGauntletBinary(null, () => null, existsSync, nowhere)).toBeNull();
  });
});

describe("resolveConfig precedence over discovery", () => {
  it("an explicit env value always wins over discovery", () => {
    const project = scratch();
    mkdirSync(project, { recursive: true });
    writeFileSync(join(project, "GAUNTLET.md"), "---\n---\n");
    const elsewhere = scratch();

    const config = resolveConfig({
      env: { GAUNTLET_WORKBENCH_PROJECT_ROOT: elsewhere },
      cwd: project,
    });

    expect(config.project_root).toBe(elsewhere);
  });

  it("discovers from the launch directory when nothing is configured", () => {
    const project = scratch();
    const nested = join(project, "workbench");
    mkdirSync(nested, { recursive: true });
    writeFileSync(join(project, "GAUNTLET.md"), "---\n---\n");

    expect(resolveConfig({ env: {}, cwd: nested }).project_root).toBe(project);
  });

  it("an empty env value is an explicit 'unset', not a trigger for discovery", () => {
    const project = scratch();
    mkdirSync(project, { recursive: true });
    writeFileSync(join(project, "GAUNTLET.md"), "---\n---\n");

    const config = resolveConfig({
      env: { GAUNTLET_WORKBENCH_PROJECT_ROOT: "" },
      cwd: project,
    });

    expect(config.project_root).toBeNull();
  });
});
