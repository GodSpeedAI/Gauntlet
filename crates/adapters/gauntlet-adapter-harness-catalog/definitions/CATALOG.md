# Herdr Agent Catalog

Extracted from herdr source (commit 4e624cd): 24 agents with verified manifest presence and resume argument patterns.

| ID | Executables | Resume | Manifest | Level | Unverified Fields |
|---|---|---|---|---|---|
| `amp` | amp, amp-local | — | amp | 0 | prompt_mode, interrupt |
| `antigravity` | agy, antigravity, antigravity-cli | ✓ | antigravity | 0 | prompt_mode, interrupt |
| `claude` | claude, claude-code | ✓ | claude | 2 | interrupt |
| `cline` | cline, .cline | — | cline | 0 | prompt_mode, interrupt |
| `codex` | codex | ✓ | codex | 2 | interrupt |
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
| `pi` | pi | ✓ | pi | 1 | interrupt, resume argv (`--session` not in help) |
| `qodercli` | qodercli, qoderclicn, qoder, qodercn | ✓ | qodercli | 0 | prompt_mode, interrupt |
| `qwen` | qwen, qwen-code | ✓ | qwen | 0 | prompt_mode, interrupt |

Multi-word aliases from the donor (`devin cli`, `kilo code`, `kimi code`, `letta code`, `mastra code`, `qwen code`) are not listed as executables: the validator rejects whitespace in executable names. `just sync-harness-catalog` skips them for the same reason.

## Legend

- **Resume**: ✓ = `session.resume_argv` backed by a `plan()` arm in src/agent_resume.rs; — = omitted (not resumable)
- **Manifest**: `lifecycle.detection_manifest`, backed by a bundled-manifest registry entry in src/detect/manifest.rs and a file in src/detect/manifests/; — = no manifest (`lifecycle.source = "process"`, backed by the registry block's absence)
- **Level**: `capabilities.support_level` as stored in the definition. 21 agents stay at 0, because their `prompt_mode` is `unknown`. claude and codex are 2, pi is 1. Those levels rest on help-verified records (see Verification tiers), not on the donor. Level 3 and 4 are not used.
- **Unverified Fields**: `control.interrupt` has no donor or help evidence, so it stays at `none`. For pi, the donor resume argv (`--session`) is not in the installed help, which documents `--resume [path|id]`, so pi stays at level 1.

## Verification tiers

Evidence about a harness lives in `definitions/VERIFICATION.toml` as `[[record]]` entries. Each record names the harness, the field, the kind, the probe argv, a short literal `observed` string, the harness version, and a date. The tiers, weakest to strongest:

- **catalogued**: extracted from the pinned Herdr donor, with provenance refs audited by `just sync-harness-catalog`. It says what Herdr does, not what the binary does on this machine.
- **fixture-tested**: a parser, validator, or launch-argv builder test runs with no real binary. The probe argv names the test.
- **help-verified**: the real installed binary's own `--help` text documents the flag or behavior. The binary was run only with `--help` or `--version`, never with a prompt.
- **live-verified**: a real run exercised the claim. `--version` live-verifies only that the executable was discovered and reports that version. Nothing else is live-verified in this slice. A live-verified record must carry a non-empty probe argv and a real harness version.

A definition may only claim what its evidence supports. Probe-backed fields are cited in `[provenance] refs` as `field=verification`. Those refs are not donor refs. `builtin()` fails closed unless each probe ref has a help-verified or live-verified record for the same harness and field.

## Installed on this machine (slice 1b)

Live evidence exists only for executables that the operator installed in `~/.local/bin`: claude 2.1.295, codex 0.160.1, pi 0.8.0, prime-agent 0.9.8. Probes ran with `--version` and `--help` only, from an empty temp directory, under `timeout 20`.

Two facts to keep in mind:

- `pi` on PATH is a symlink to prime-agent's binary. Its help text is prime-agent's. Its version (0.8.0) differs from `prime-agent --version` (0.9.8).
- A PATH lookup also finds `agy` (antigravity), `cline` and `opencode`. They have no live or help records, so they stay catalogued and fixture-tested only.

The other harnesses have no records beyond the fixture test. `just harness-matrix` renders the current state.

The prime-agent row is adapter-owned and has no definition file. Its evidence comes from `--version` and `--help` (help-verified) and from the adapter's fixture tests in `crates/adapters/gauntlet-adapter-agent-prime`. The env-gated live probe (`tests/live_probe.rs`, which makes a model call) and `tests/mcp_bridge_live.rs` (ignored by default) were not run in this slice, so no live-verified record exists for prime beyond `--version`.

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
- Donor refs: 66 across 24 definitions, all `field=path:lines`, checked by `just sync-harness-catalog`
- Probe refs: 12 across claude (4), codex (5) and pi (3), all `field=verification`, checked against VERIFICATION.toml at load time
