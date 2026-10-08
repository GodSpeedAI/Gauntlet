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
refs = ["src/agent_resume.rs:207-213", "src/detect/mod.rs:202"]
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

## Validation rules (fail closed)

* Unknown keys rejected (`deny_unknown_fields`); `schema` must equal 1.
* Placeholders `{model}`, `{session}`, `{prompt}` are legal ONLY as (a) an entire argv element, or (b) the sole suffix of a long-flag element of the exact form `--[a-z][a-z0-9-]*={placeholder}` (Herdr's omp/copilot use `--resume={session}`; argv elements are never shell-interpreted). Any other fusion, any other `{...}`, shell metacharacters in executables, NUL, or a path separator in `executables` => reject.
* `resume_argv` / `launch.args` / `model_arg` are argv AFTER the executable; the executable is never repeated inside them.
* `resume_argv` requires `session.identity != "none"` and must contain exactly one `{session}`; `capabilities.resumable` must agree.
* `support_level` must be ≤ what the declared fields justify: 1 needs `prompt_mode != "unknown"`; 2 needs resumable; 3 needs `lifecycle.source = "structured"`; 4 is never legal in a definition (needs code).
* Definitions never carry env, shell strings, sandbox or authority settings.
* Lifecycle states are forensics only; nothing here can mark an invocation complete.
