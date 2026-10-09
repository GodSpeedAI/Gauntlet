# Herdr Agent Catalog

Extracted from herdr source (commit 4e624cd): 24 agents with verified manifest presence and resume argument patterns.

| ID | Executables | Resume | Manifest | Level | Unverified Fields |
|---|---|---|---|---|---|
| `amp` | amp, amp-local | — | amp | 0 | prompt_mode, interrupt |
| `antigravity` | agy, antigravity, antigravity-cli | ✓ | antigravity | 0 | prompt_mode, interrupt |
| `claude` | claude, claude-code | ✓ | claude | 0 | prompt_mode, interrupt |
| `cline` | cline, .cline | — | cline | 0 | prompt_mode, interrupt |
| `codex` | codex | ✓ | codex | 0 | prompt_mode, interrupt |
| `cursor` | cursor, cursor-agent | ✓ | cursor | 0 | prompt_mode, interrupt |
| `devin` | devin, devin-cli | ✓ | devin | 0 | prompt_mode, interrupt |
| `droid` | droid | ✓ | droid | 0 | prompt_mode, interrupt |
| `gemini` | gemini | — | gemini | 0 | prompt_mode, interrupt |
| `github-copilot` | copilot, github-copilot, ghcs | ✓ | github-copilot | 0 | prompt_mode, interrupt |
| `grok` | grok, grok-build | ✓ | grok | 0 | prompt_mode, interrupt |
| `hermes` | hermes, hermes-agent | ✓ | hermes | 0 | prompt_mode, interrupt |
| `kilo` | kilo, kilo-code | ✓ | kilo | 0 | prompt_mode, interrupt |
| `kimi` | kimi, kimi-code | ✓ | kimi | 0 | prompt_mode, interrupt |
| `kiro` | kiro, kiro-cli | — | kiro | 0 | prompt_mode, interrupt |
| `letta` | letta, letta-code | ✓ | letta | 0 | prompt_mode, interrupt |
| `maki` | maki | — | maki | 0 | prompt_mode, interrupt |
| `mastracode` | mastracode, mastra-code | ✓ | — | 0 | prompt_mode, interrupt |
| `muse` | muse, muse-code, muse-cli | — | muse | 0 | prompt_mode, interrupt |
| `omp` | omp | ✓ | — | 0 | prompt_mode, interrupt |
| `opencode` | opencode, opencode2, open-code | ✓ | opencode | 0 | prompt_mode, interrupt |
| `pi` | pi | ✓ | pi | 0 | prompt_mode, interrupt |
| `qodercli` | qodercli, qoderclicn, qoder, qodercn | ✓ | qodercli | 0 | prompt_mode, interrupt |
| `qwen` | qwen, qwen-code | ✓ | qwen | 0 | prompt_mode, interrupt |

Multi-word aliases from the donor (`devin cli`, `kilo code`, `kimi code`, `letta code`, `mastra code`, `qwen code`) are not listed as executables: the validator rejects whitespace in executable names. `just sync-harness-catalog` skips them for the same reason.

## Legend

- **Resume**: ✓ = `session.resume_argv` backed by a `plan()` arm in src/agent_resume.rs; — = omitted (not resumable)
- **Manifest**: `lifecycle.detection_manifest`, backed by a bundled-manifest registry entry in src/detect/manifest.rs and a file in src/detect/manifests/; — = no manifest (`lifecycle.source = "process"`, backed by the registry block's absence)
- **Level**: `capabilities.support_level` as stored in the definition. 0 everywhere, because every `prompt_mode` is `unknown` and the validator only allows level 1+ with a known prompt mode.
- **Unverified Fields**: `launch.prompt_mode` and `control.interrupt` have no donor evidence, so they stay at defaults.

## Provenance audit (report-only sync against 4e624cd)

Every `[provenance] refs` entry is now `field=path:lines`, and each claimed field has exactly one ref. Checked mechanically against the donor: file exists, range in bounds, quoted executables / argv tokens / manifest name present in the range.

Before the audit there were 62 refs. Their problems:

- 22 `executables` claims pointed at `agent_label()` (src/detect/mod.rs:126–149), which maps each agent to its canonical name only. They did not list the aliases. Fixed to the `lookup_agent` arm.
- 2 resume claims pointed at the wrong lines: pi and omp both cited `agent_resume.rs:187-210`, which covers unrelated helpers and the claude arm. Fixed to 233–235 (pi) and 236–240 (omp).
- 22 manifest claims were whole-file paths (`src/detect/manifests/x.toml`). Replaced with the registry line in src/detect/manifest.rs that includes that file.
- 2 resumable agents had no resume ref: antigravity and github-copilot. Added 280–286 and 214–216.
- 2 process-only agents (mastracode, omp) had no evidence for `lifecycle.source = "process"`. Added a registry-block ref (240–262). This is absence-based evidence, the weakest in the catalog.

| Outcome | Count |
|---|---|
| Refs before audit | 62 |
| Kept unchanged (verified: 14 resume arms, 2 executable arms for antigravity and github-copilot) | 16 |
| Rewritten (imprecise: 22 executable, 22 manifest, 2 resume range) | 46 |
| Added (missing: 2 resume, 2 process lifecycle) | 4 |
| Removed (no donor support for a claimed field) | 0 |
| Refs after audit, all verified by `just sync-harness-catalog` | 66 |
| Support-level drops in definitions | 0 |

**Level correction (doc only).** The previous version of this table listed level 1 for 18 agents. The definitions had always carried level 0, which is the only legal value while `prompt_mode` is `unknown`. This table now matches the files. No definition changed.

## Summary

- Total agents: 24
- Agents with verified resume_argv: 18
- Agents with manifests: 22
- Refs: 66 across 24 definitions, all `field=path:lines`
