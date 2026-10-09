# Codemode host

Headless Bun host that runs one generated tool program in a QuickJS sandbox and speaks JSONL on
stdio. Protocol: [PROTOCOL.md](./PROTOCOL.md). Run it with `bun run codemode-host`.

## Layout

- `cli.ts`: stdio framing, 1 MiB line cap, fail-closed protocol faults, exit codes.
- `protocol.ts`: strict parse and encode of v1 messages, admitted-name validation.
- `run.ts`: per-run state (worker, timer, tool budget, in-flight cap, single `done`).
- `sandbox-worker.ts`: worker thread that owns one QuickJS VM per run.
- `sandbox-prelude.ts`: guest-side code. Builds the frozen `tools` tree, checks JSON, settles calls.
- `sandbox-messages.ts`: host and worker message types.

## Engine choice

The engine is `quickjs-wasi@3.6.2` (exact pin, MIT, no transitive dependencies), added with
`bun add --exact`. It is the same engine the pi codemode donor (`@earendil-works/pi-codemode`
1.0.4, MIT, commit f10993b) uses. The donor was studied for its design, and nothing was copied: the
host logic here is re-expressed, so there is no donor source in the tree and no NOTICE entry is
needed. The engine is a regular dependency; nothing is fetched at runtime.

## Sandbox surface

The guest sees ECMAScript intrinsics, the `tools` tree, and nothing else. There is no `fetch`,
`process`, `require`, `Bun`, `console`, timers or filesystem. The only bridge to the host is a
closure in the prelude. A worker keeps the host thread free to read `cancel` and to time out a
spinning guest; a shared flag read by the VM interrupt handler stops it.

Known residual surface: the guest can still use the full language (including `Function` and
`eval`) inside the same realm, and can allocate up to the 64 MiB heap cap. Neither reaches the host.
