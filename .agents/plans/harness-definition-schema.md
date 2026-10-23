# HarnessDefinition TOML schema (v1) — frozen for Phase C

One file per harness. Built-in catalog: `crates/adapters/gauntlet-adapter-harness-catalog/definitions/<id>.toml` (embedded). Custom: `.gauntlet/harnesses/*.toml` (strict validation, may never raise a built-in's support level or relax sandbox policy).

```toml
schema = 1
id = "claude"                      # [a-z][a-z0-9-]{0,31}, unique
label = "Claude Code"
executables = ["claude"]           # [0] canonical; rest are aliases; basenames only, no path separators

[provenance]                       # required for catalog-derived entries
source = "herdr"
commit = "4e624cd"
refs = ["executables=src/detect/mod.rs:202", "session.resume_argv=src/agent_resume.rs:204-210", "lifecycle.detection_manifest=src/detect/manifest.rs:243"]
license = "Apache-2.0"

[launch]
args = []                          # base argv after executable; array of literal strings
model_arg = ["--model", "{model}"] # optional; omit if unverified (=> no model selection)
prompt_mode = "unknown"            # "argv-tail" | "flag" | "stdin" | "pty-paste" | "unknown"
prompt_flag = "-p"                 # only when prompt_mode = "flag"

[session]
identity = "id"                    # "id" | "path" | "none"
resume_argv = ["--resume", "{session}"]   # argv AFTER executable; omit if not resumable/unverified

[lifecycle]
source = "screen"                  # "structured" | "process" | "screen" | "none"
detection_manifest = "claude"      # name of herdr-derived manifest (provenance pointer only in Phase C)

[control]
interrupt = "ctrl-c"               # "sigint" | "ctrl-c" | "esc" | "none"

[capabilities]
interactive = true
resumable = true
model_selection = false
native_subagents = false
structured_events = false
support_level = 1                  # 0 process | 1 interactive | 2 session | 3 structured | 4 native
```

## Probe refs (second source kind, schema 1)

Some fields are established by help-verified probes of the installed binary, not by the Herdr donor. Those claims cite `refs` entries of the form `field=verification`, with no path and no lines. The allowed probe fields are `launch.args`, `launch.prompt_mode`, `launch.model_arg`, `capabilities.model_selection` and `capabilities.structured_events`. Each probe ref must match a `[[record]]` in `definitions/VERIFICATION.toml` with the same harness and field and kind `help-verified` or `live-verified`. The loader fails closed otherwise, and it also fails when a claimed probe field has no probe ref. `[provenance]` keeps `source = "herdr"` and the pinned commit, because the definition is still Herdr-derived. Probe refs are counted separately and are not checked against the donor.

Evidence records are strict: `schema = 1`, `[[record]]` tables with exactly `harness`, `field`, `kind`, `probe_argv`, `observed`, `harness_version` and `date`. Kinds are `catalogued`, `fixture`, `help-verified` and `live-verified`. A harness id must be a definition id or an adapter-owned id (`prime-agent`). `live-verified` requires a non-empty `probe_argv` and a real `harness_version`. Tier definitions are in `definitions/CATALOG.md`.

## Provenance refs (schema 1, unchanged version)

Each `refs` entry is `field=path:start[-end]` (or `field=path:line`), 1-based inclusive lines in the Herdr donor checkout pinned by `commit`. The field names the claim the lines support, and every claimed field needs exactly one ref:

| field | donor file | lines must contain |
|---|---|---|
| `executables` | `src/detect/mod.rs` | the `lookup_agent` arm with every executable, quoted |
| `session.resume_argv` | `src/agent_resume.rs` | the `plan()` arm with every literal argv token |
| `lifecycle.detection_manifest` | `src/detect/manifest.rs` | the bundled-manifest registry entry for `manifests/<name>.toml` |
| `lifecycle.source` (only for `"process"`) | `src/detect/manifest.rs` | the bundled registry block, and no entry for this definition |

Refs are free-form strings to the existing parser, so the prefix is tolerated without a version bump. Whole-file refs and unlabelled refs are rejected by the catalog tests. `just sync-harness-catalog` re-checks every ref (existence, range, keyword) against a donor checkout, report-only. Per-ref content fingerprints were considered and deferred: they would need a hashing dependency in the catalog crate.

## Validation rules (fail closed)

* Unknown keys rejected (`deny_unknown_fields`); `schema` must equal 1.
* Placeholders `{model}`, `{session}`, `{prompt}` are legal ONLY as (a) an entire argv element, or (b) the sole suffix of a long-flag element of the exact form `--[a-z][a-z0-9-]*={placeholder}` (Herdr's omp/copilot use `--resume={session}`; argv elements are never shell-interpreted). Any other fusion, any other `{...}`, shell metacharacters in executables, NUL, or a path separator in `executables` => reject.
* `resume_argv` / `launch.args` / `model_arg` are argv AFTER the executable; the executable is never repeated inside them.
* `resume_argv` requires `session.identity != "none"` and must contain exactly one `{session}`; `capabilities.resumable` must agree.
* `support_level` must be ≤ what the declared fields justify: 1 needs `prompt_mode != "unknown"`; 2 needs resumable; 3 needs `lifecycle.source = "structured"`; 4 is never legal in a definition (needs code).
* Definitions never carry env, shell strings, sandbox or authority settings.
* Lifecycle states are forensics only; nothing here can mark an invocation complete.
