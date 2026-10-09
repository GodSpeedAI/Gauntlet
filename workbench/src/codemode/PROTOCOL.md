# Codemode host protocol (v1)

Normative. The host is `src/codemode/cli.ts` (`bun run codemode-host`). The parent is a process that
spawns it and speaks over its stdin and stdout. One program runs per process lifetime.

## Framing

- Newline-delimited JSON (UTF-8). One JSON object per line, in both directions.
- Each line is at most 1 MiB (1,048,576 bytes) excluding the newline.
- stdout carries only protocol lines. stderr carries diagnostics and is not parsed.
- Every object has `"v": 1`. Any other version is a protocol fault.
- Unknown message types and unknown keys are protocol faults. Keys are required unless marked optional.

## Parent to host

### `run` (must be the first message, once)

```json
{"v":1,"t":"run","source":"...","admitted":["agents.list","ping"],
 "limits":{"timeout_ms":5000,"max_tool_calls":20,"max_parallel_calls":4,
           "max_result_bytes":100000,"max_source_bytes":100000}}
```

- `source`: the body of an async function whose only parameter is `tools`. The program's return
  value is its result. `return` is optional; no return means `null`.
- `admitted`: tool names. A name is dot-separated segments; each segment matches
  `^[A-Za-z][A-Za-z0-9_-]{0,63}$`, and no segment may be `then`, `constructor`, `prototype` or
  `__proto__`. Names must be unique, and a name may not also be a namespace of another name
  (`agents` together with `agents.list` is a fault). Names longer than 256 characters are a fault.
- `limits`: all five keys are required, each a non-negative safe integer. `timeout_ms` is at least
  1 and `max_parallel_calls` is at least 1. Limits are enforced by the host.

### `result` (answers a `call`)

```json
{"v":1,"t":"result","id":1,"outcome":{"kind":"ok","value":[{"id":"a1"}]}}
{"v":1,"t":"result","id":2,"outcome":{"kind":"refused","message":"not allowed"}}
```

- `id` must be the id of an outstanding `call`. Any other id is a protocol fault.
- `kind` is `ok` (requires `value`), or `refused`, `invalid`, `error` (`value` and `message` optional).
- A non-`ok` outcome reaches the program as a thrown `Error` with `name` `"ToolError"` and a
  `kind` property holding the outcome kind. Its `message` is the outcome message.

### `cancel`

```json
{"v":1,"t":"cancel"}
```

Stops the program at once, whether it is waiting on a call or spinning. Valid only after `run`.

## Host to parent

### `call`

```json
{"v":1,"t":"call","id":1,"tool":"agents.list","args":{"scope":"all"}}
```

Only admitted tools are sent. `args` is always present. Omitted tool arguments are `{}`. Ids are
assigned by the sandbox and are unique within the run.

### `done` (exactly one per run; the host then exits)

```json
{"v":1,"t":"done","status":"completed","value":{"n":1}}
{"v":1,"t":"done","status":"failed","value":null,"message":"TypeError: ..."}
{"v":1,"t":"done","status":"limit_exceeded","value":null,"message":"...","limit":"tool_calls"}
```

- `status`: `completed`, `failed`, `timed_out`, `limit_exceeded`, or `cancelled`.
- `value`: the JSON result when `completed`; `null` otherwise.
- `message`: present unless `completed`.
- `limit`: present only when `status` is `limit_exceeded`. One of `tool_calls`, `parallel`,
  `result_bytes`, `source_bytes`, `memory`.

## Limits and their meaning

- `timeout_ms`: wall-clock for the whole run, including time spent waiting for the parent. Expiry
  is `timed_out`.
- `max_tool_calls`: the budget counts every attempt. An admitted call counts when it is sent. An
  access to a name that is not admitted also counts, though it never reaches the parent. Exceeding
  the budget is `limit_exceeded` with `tool_calls`.
- `max_parallel_calls`: calls in flight (sent, not yet answered). A call beyond the cap fails the run
  with `limit_exceeded` and `parallel`. Calls are not queued.
- `max_source_bytes`: checked before the program runs. Exceeding it is `limit_exceeded` with `source_bytes`.
- `max_result_bytes`: the serialized result, in UTF-8 bytes. Exceeding it is `limit_exceeded` with `result_bytes`.
- Guest heap is capped at 64 MiB (not configurable in v1). Exhaustion is `limit_exceeded` with `memory`.

## Data rules

- Tool arguments and results are plain JSON: `null`, booleans, finite numbers, strings, arrays, and
  plain objects with data properties. Functions, symbols, bigints, `undefined` inside containers,
  non-finite numbers, cycles, accessors, class instances and holes are refused inside the program
  (a `TypeError` for arguments). A non-JSON program result is `failed` ("result is not JSON").
- A top-level `undefined` result is `null`.

## Protocol faults (fail closed)

A fault is a line that is not valid JSON, an empty line, a non-object, a wrong version, an unknown
`t`, unknown or missing keys, invalid fields, a line over 1 MiB, a `result` for an unknown id, a
second `run`, a message other than `run` as the first message, or EOF on stdin before `done`.

On a fault the host writes `{"t":"done","status":"failed","message":"protocol fault: ..."}`,
kills the sandbox, and exits with code 1. Program outcomes of any status exit with code 0.
