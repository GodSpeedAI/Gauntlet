# T00A Evidence — Bootstrap Dependency Admission List

Date: 2026-09-08
Task: T00A
Target Consumer: T03 Dependency-Admission Ledger

## Context & Authority

Per the plan's operating contract and Adversarial Finding F10, T00A admits the first runtime rendering dependencies under an explicit, bounded bootstrap carve-out ahead of T03's dependency-admission ledger.

## Admitted Dependencies

| Package | Version | Type | Purpose / Justification |
|---------|---------|------|-------------------------|
| `@opentui/core` | `0.5.11` | runtime | Terminal rendering engine and Zig-native TUI core (`REQ-TUI-001`, `REQ-TUI-005`). |
| `@opentui/react` | `0.5.11` | runtime | OpenTUI React renderer reconciler (`REQ-TUI-001`). |
| `react` | `19.2.8` | runtime | Component state and lifecycle substrate for OpenTUI views (`DESIGN.md`). |
| `react-reconciler` | `0.33.0` | runtime | Custom renderer reconciler dependency for OpenTUI React. |
| `@types/react` | `19.2.8` | dev | TypeScript types for React. |

## REQ-BUN-001 Overlap Set Exclusion Audit

All packages in the `REQ-BUN-001` forbidden overlap set were verified as **ABSENT**:

- **Local Database**: No SQLite or external DB driver (Bun native `bun:sqlite` will be qualified in T03).
- **PTY / Terminal Spawn**: No `node-pty`, `pty.js` admitted (deferred to T13/T18).
- **Process Management**: No `execa`, `cross-spawn` (Bun native `Bun.spawn` used).
- **HTTP / Sockets**: No Express, Fastify, WS (native `fetch` used for HTTP read path).
- **Config Parsing**: No YAML, TOML, dotenv libraries (Bun/native stdlib to be qualified in T02/T03).
- **Compression / Hashing**: No external libraries (native crypto/compression to be qualified).

Automated verification is enforced by `operator-preview.test.ts` (teeth test "package.json admits zero REQ-BUN-001 overlap dependencies") and `scripts/tui-preflight.sh bootstrap` (exact version pins and audit checks).
