# Herdr Agent Catalog

Extracted from herdr source (commit 4e624cd): 24 agents with verified manifest presence and resume argument patterns.

| ID | Executables | Resume | Manifest | Level | Unverified Fields |
|---|---|---|---|---|---|
| `amp` | amp, amp-local | — | amp | 0 | prompt_mode, interrupt |
| `antigravity` | agy, antigravity, antigravity-cli | ✓ | antigravity | 1 | prompt_mode, interrupt |
| `claude` | claude, claude-code | ✓ | claude | 1 | prompt_mode, interrupt |
| `cline` | cline, .cline | — | cline | 0 | prompt_mode, interrupt |
| `codex` | codex | ✓ | codex | 1 | prompt_mode, interrupt |
| `cursor` | cursor, cursor-agent | ✓ | cursor | 1 | prompt_mode, interrupt |
| `devin` | devin, devin-cli, devin cli | ✓ | devin | 1 | prompt_mode, interrupt |
| `droid` | droid | ✓ | droid | 1 | prompt_mode, interrupt |
| `gemini` | gemini | — | gemini | 0 | prompt_mode, interrupt |
| `github-copilot` | copilot, github-copilot, ghcs | ✓ | github-copilot | 1 | prompt_mode, interrupt |
| `grok` | grok, grok-build | ✓ | grok | 1 | prompt_mode, interrupt |
| `hermes` | hermes, hermes-agent | ✓ | hermes | 1 | prompt_mode, interrupt |
| `kilo` | kilo, kilo-code, kilo code | ✓ | kilo | 1 | prompt_mode, interrupt |
| `kimi` | kimi, kimi-code, kimi code | ✓ | kimi | 1 | prompt_mode, interrupt |
| `kiro` | kiro, kiro-cli | — | kiro | 0 | prompt_mode, interrupt |
| `letta` | letta, letta-code, letta code | ✓ | letta | 1 | prompt_mode, interrupt |
| `maki` | maki | — | maki | 0 | prompt_mode, interrupt |
| `mastracode` | mastracode, mastra-code, mastra code | ✓ | — | 1 | prompt_mode, interrupt |
| `muse` | muse, muse-code, muse-cli | — | muse | 0 | prompt_mode, interrupt |
| `omp` | omp | ✓ | — | 1 | prompt_mode, interrupt |
| `opencode` | opencode, opencode2, open-code | ✓ | opencode | 1 | prompt_mode, interrupt |
| `pi` | pi | ✓ | pi | 1 | prompt_mode, interrupt |
| `qodercli` | qodercli, qoderclicn, qoder, qodercn | ✓ | qodercli | 1 | prompt_mode, interrupt |
| `qwen` | qwen, qwen-code, qwen code | ✓ | qwen | 1 | prompt_mode, interrupt |

## Legend

- **Resume**: ✓ = resume_argv verified in src/agent_resume.rs plan() function; — = omitted (not resumable or unverified)
- **Manifest**: Name of src/detect/manifests/*.toml (lifecycle.source='screen'); — = no manifest (source='process')
- **Level**: Support level (0=process, 1=interactive, 2+=unverified). Based on resumable + manifest.
- **Unverified Fields**: Fields set to defaults (unknown/none) due to lack of verification in donor source

## Summary

- Total agents: 24
- Agents with verified resume_argv: 18
- Agents with manifests: 22