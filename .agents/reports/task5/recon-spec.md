# Task 5 recon — spec extraction (verbatim)

**Provenance:** extracted 2026-08-16 by a read-only explore agent from
`.agents/specs/Core-spec.md` (2156 lines); persisted by the orchestrator.
Verbatim quotes with line numbers. Authoritative for the Task-5 builder brief.

## 1. §8.1 Principles (lines 505–509)

```
505	### 8.1 Principles
506
507	- **Nothing hardcoded.** No path, model, provider, command, port number, timeout, tier policy, or tool identity is a literal in domain or application code. Conservative defaults live in one declared defaults table (§8.6) and are overridable.
508	- **Three layers, one order.** `built-in defaults` < `GAUNTLET.md` front matter < `.env` / process environment < CLI flags. Later wins. The effective config is digested into `Run.config_fingerprint`.
509	- **Secrets by indirection only.** Config values may reference `$VAR`; the runtime resolves them, validates presence, and never logs the value. Generated files never contain credentials. A `.env.example` with empty values is REQUIRED.
```

(Note: "Three layers" names four precedence sources — built-in defaults < GAUNTLET.md front matter < .env/process environment < CLI flags; later wins.)

## 2. §8.2 `GAUNTLET.md` (repository policy contract) — COMPLETE (lines 511–563)

```
511	### 8.2 `GAUNTLET.md` (repository policy contract)
512
513	Discovery: `--config <path>` → `$GAUNTLET_CONFIG_DIR/GAUNTLET.md` → `./GAUNTLET.md`. Missing file is not fatal if the environment fully specifies the run; it is an error only when a required value is unresolved.
514
515	Structure: OPTIONAL YAML front matter (runtime config) followed by OPTIONAL H2 sections used as documentation. Front matter MUST decode to a map. Unknown keys are ignored for forward compatibility, but `gauntlet doctor` lists them so typos surface.
516
517	```yaml
518	---
519	reference:
520	  manifest: .wayfinder/reference.yaml
521	  answer_key: .wayfinder/answer-key.md
522	  map: .wayfinder/map.md
523	  comparison_contract: .gauntlet/comparison-contract.yaml
524	  digest_algo: sha256
525	roles:
526	  dir: .agents/roles                  # role definitions (§10.1)
527	runners:
528	  default: $GAUNTLET_AGENT_RUNNER
529	  by_role:
530	    builder:  { runner: prime-agent, model: $GAUNTLET_BUILDER_MODEL }
531	    critic:   { runner: prime-agent, model: $GAUNTLET_CRITIC_MODEL }
532	    verifier: { runner: none }        # deterministic verifiers need no model
533	observation:
534	  facets: [versioning, filesystem, build, test]
535	review:
536	  mode: adaptive                      # ReviewMode
537	  max_critics: 4
538	verification:
539	  prefer_deterministic: true
540	sandbox:
541	  backend: $GAUNTLET_SANDBOX_BACKEND  # none | process | bwrap | seatbelt | mxc | microvm
542	  policy_dir: .gauntlet/policies      # one file per tier; irreversible MUST be absent
543	budget:
544	  max_rounds: 8
545	  max_parallel: 3
546	  max_depth: 2
547	  max_wall_seconds: 5400
548	  max_tokens: 2000000
549	  max_cost_usd: 25
550	  recovery_reserve_pct: 15
551	  diagnostic_budget: 2
552	workspace:
553	  root: $GAUNTLET_TMP_DIR/workspaces
554	  use_worktrees: true
555	state:
556	  backend: sqlite
557	  dir: $GAUNTLET_STATE_DIR
558	evidence:
559	  dir: $GAUNTLET_ARTIFACT_DIR
560	operator:
561	  surfaces: [cli]                     # + json-api, tui, dashboard
562	---
563	```
```

Documented front-matter keys (top level): `reference` (manifest, answer_key, map,
comparison_contract, digest_algo), `roles` (dir), `runners` (default,
by_role.{builder,critic,verifier}.{runner,model}), `observation` (facets), `review`
(mode, max_critics), `verification` (prefer_deterministic), `sandbox` (backend,
policy_dir), `budget` (max_rounds, max_parallel, max_depth, max_wall_seconds,
max_tokens, max_cost_usd, recovery_reserve_pct, diagnostic_budget), `workspace`
(root, use_worktrees), `state` (backend, dir), `evidence` (dir), `operator`
(surfaces). `$VAR` indirection appears in values.

## 3. §8.3 Environment variables (lines 565–569) + Annex E (lines 2082–2098)

§8.3:

```
565	### 8.3 Environment variables (canonical set)
566
567	Full table in Annex E. All are optional; all have defaults; none may be required for a bounded run to start safely.
568
569	Groups: workspace paths (`GAUNTLET_PROJECT_ROOT`, `_CONFIG_DIR`, `_STATE_DIR`, `_ARTIFACT_DIR`, `_PLAN_DIR`, `_REPORT_DIR`, `_TMP_DIR`); reference (`_REFERENCE_PATH`, `_REFERENCE_MANIFEST`, `_REFERENCE_DIGEST`, `_ANSWER_KEY_PATH`, `_MAP_PATH`, `_COMPARISON_CONTRACT`, `_ACCEPTANCE_PATH`, `_CONTEXT_PATHS`, `_CANON_PATHS`, `_HELD_OUT_PATH`, `_REGRESSION_PATH`); runners (`_AGENT_RUNNER`, `_<ROLE>_RUNNER`, `_<ROLE>_MODEL`); tools (`_MCP_CONFIG`, `_TOOL_REGISTRY`, `_MCP_ALLOWLIST`, `_MCP_REQUIRED`, `_TOOL_<CAPABILITY>`); limits (`_INTERACTIVE`, `_MAX_ROUNDS`, `_MAX_PARALLEL`, `_MAX_DEPTH`, `_MAX_WALL_SECONDS`, `_MAX_TOKENS`, `_MAX_COST_USD`, `_RECOVERY_RESERVE_PCT`, `_DIAGNOSTIC_BUDGET`, `_REVIEW_MODE`, `_ALLOW_INSTALL`, `_ALLOW_NETWORK`, `_ALLOW_DESTRUCTIVE`, `_GIT_STRATEGY`, `_ALLOW_ORCHESTRATOR_IMPLEMENTATION`); containment (`_SANDBOX_BACKEND`, `_SANDBOX_POLICY_DIR`); persistence (`_STATE_BACKEND`, `_EVIDENCE_BACKEND`); surfaces (`_OPERATOR_SURFACES`, `_API_BIND`, `_LOG_FORMAT`, `_LOG_SINK`).
```

Annex E — complete:

```
2082	# Annex E — Environment variable reference
2083
2084	All optional; all defaulted; none required for a safe bounded run.
2085
2086	| Group | Variables |
2087	|---|---|---|
2088	| Paths | `GAUNTLET_PROJECT_ROOT` `GAUNTLET_CONFIG_DIR` `GAUNTLET_STATE_DIR` `GAUNTLET_ARTIFACT_DIR` `GAUNTLET_PLAN_DIR` `GAUNTLET_REPORT_DIR` `GAUNTLET_TMP_DIR` |
2089	| Reference | `GAUNTLET_REFERENCE_PATH` `GAUNTLET_REFERENCE_MANIFEST` `GAUNTLET_REFERENCE_DIGEST` `GAUNTLET_ANSWER_KEY_PATH` `GAUNTLET_MAP_PATH` `GAUNTLET_COMPARISON_CONTRACT` `GAUNTLET_ACCEPTANCE_PATH` `GAUNTLET_CONTEXT_PATHS` `GAUNTLET_CANON_PATHS` `GAUNTLET_HELD_OUT_PATH` `GAUNTLET_REGRESSION_PATH` `GAUNTLET_DIGEST_ALGO` |
2090	| Runners | `GAUNTLET_AGENT_RUNNER` `GAUNTLET_ORCHESTRATOR_RUNNER` `GAUNTLET_ORCHESTRATOR_MODEL` `GAUNTLET_BUILDER_RUNNER` `GAUNTLET_BUILDER_MODEL` `GAUNTLET_CRITIC_RUNNER` `GAUNTLET_CRITIC_MODEL` `GAUNTLET_VERIFIER_RUNNER` `GAUNTLET_VERIFIER_MODEL` `GAUNTLET_OBSERVER_RUNNER` `GAUNTLET_OBSERVER_MODEL` `GAUNTLET_OPTIMIZER_RUNNER` `GAUNTLET_OPTIMIZER_MODEL` `GAUNTLET_CLI_AGENT_CMD` `GAUNTLET_ROLES_DIR` |
2091	| Tools | `GAUNTLET_MCP_CONFIG` `GAUNTLET_TOOL_REGISTRY` `GAUNTLET_MCP_ALLOWLIST` `GAUNTLET_MCP_REQUIRED` `GAUNTLET_TOOL_CODE_NAV` `GAUNTLET_TOOL_FILESYSTEM` `GAUNTLET_TOOL_VERSIONING` `GAUNTLET_TOOL_TEST` `GAUNTLET_TOOL_BUILD` `GAUNTLET_TOOL_BROWSER` `GAUNTLET_TOOL_VISUAL` `GAUNTLET_TOOL_WEB` `GAUNTLET_TOOL_RESEARCH` `GAUNTLET_TOOL_SECURITY` `GAUNTLET_TOOL_SANDBOX` `GAUNTLET_TOOL_MEMORY` `GAUNTLET_TOOL_EVIDENCE` `GAUNTLET_TOOL_DOMAIN` `GAUNTLET_TOOL_POLICY` `GAUNTLET_TOOL_BENCHMARK` `GAUNTLET_TOOL_SCHEMA` |
2092	| Limits | `GAUNTLET_INTERACTIVE` `GAUNTLET_MAX_ROUNDS` `GAUNTLET_MAX_PARALLEL` `GAUNTLET_MAX_DEPTH` `GAUNTLET_MAX_WALL_SECONDS` `GAUNTLET_MAX_TOKENS` `GAUNTLET_MAX_COST_USD` `GAUNTLET_RECOVERY_RESERVE_PCT` `GAUNTLET_DIAGNOSTIC_BUDGET` `GAUNTLET_REVIEW_MODE` `GAUNTLET_MAX_CRITICS` `GAUNTLET_MAX_ATTEMPTS_PER_UNIT` `GAUNTLET_NONCONVERGENCE_N` `GAUNTLET_POLL_INTERVAL_MS` `GAUNTLET_STALL_TIMEOUT_MS` `GAUNTLET_RETRY_BASE_MS` `GAUNTLET_RETRY_MAX_MS` |
2093	| Permissions | `GAUNTLET_ALLOW_INSTALL` `GAUNTLET_ALLOW_NETWORK` `GAUNTLET_ALLOW_DESTRUCTIVE` `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION` `GAUNTLET_VERSIONING_STRATEGY` |
2094	| Containment | `GAUNTLET_SANDBOX_BACKEND` `GAUNTLET_SANDBOX_BINARY` `GAUNTLET_SANDBOX_EXPERIMENTAL` `GAUNTLET_SANDBOX_POLICY_DIR` |
2095	| Persistence | `GAUNTLET_STATE_BACKEND` `GAUNTLET_EVIDENCE_BACKEND` `GAUNTLET_EXPORT_DIR` |
2096	| Surfaces | `GAUNTLET_OPERATOR_SURFACES` `GAUNTLET_API_BIND` `GAUNTLET_LOG_FORMAT` `GAUNTLET_LOG_SINK` `GAUNTLET_OTEL_ENDPOINT` |
2097	| Promotion | `GAUNTLET_PROMOTION_MIN_SETTLEMENTS` `GAUNTLET_PROMOTION_MARGIN` |
2098	| Test hooks | `GAUNTLET_CLOCK` `GAUNTLET_ID_SEED` (deterministic clock/ids for tests only) |
```

Annex E statements on defaults/validation: header only — "All optional; all defaulted;
none required for a safe bounded run." plus the test-hooks parenthetical. Naming
mismatch: §8.3 says `_GIT_STRATEGY` (limits); Annex E says
`GAUNTLET_VERSIONING_STRATEGY` (Permissions).

## 4. §8.4 Semantic tool capabilities (lines 571–582)

```
571	### 8.4 Semantic tool capabilities
572
573	Tools bind by **semantic capability**, never by product:
574
575	```text
576	code_nav · filesystem · versioning · test · build · browser · visual · web · research
577	security · sandbox · memory · evidence · domain · policy · benchmark · schema
578	```
579
580	`GAUNTLET_TOOL_CODE_NAV=rust-analyzer` and `GAUNTLET_TOOL_CODE_NAV=<some MCP server>` are the same decision at different addresses. A binding may resolve through an MCP server, a CLI, a local library, or a harness-native tool. **Do not couple semantic capabilities to MCP**; MCP is one projection of a tool interface.
581
582	If `GAUNTLET_MCP_REQUIRED` names a capability that cannot be resolved, `gauntlet doctor` fails and the run does not start.
```

## 5. §8.5 Validation and preflight (`gauntlet doctor`) — COMPLETE (lines 584–597)

```
584	### 8.5 Validation and preflight (`gauntlet doctor`)
585
586	REQUIRED checks before any run starts, and re-checked at every iteration boundary for the subset that can drift:
587
588	1. Config parses; unknown keys reported; `$VAR` indirections resolve; secrets present but unprinted.
589	2. Reference manifest and answer key parse; every Unit validates against §5.3.2 including the tier/authority coupling rule.
590	3. A runner resolves for every role the configured `ReviewMode` can dispatch.
590	3. A runner resolves for every role the configured ReviewMode can dispatch.
591	4. Runner capabilities satisfy configured tiers (§17.3); otherwise `sandbox_capability_mismatch`.
592	5. Sandbox policies exist for `contained`, `local`, `cross_cutting` when `backend != none`; **no policy exists for `irreversible`** (its presence is a config error, §17.3).
593	6. Every semantic capability in `GAUNTLET_MCP_REQUIRED` resolves.
594	7. State dir, evidence dir, workspace root writable; answer key path **not** inside any builder-writable workspace.
595	8. Budget dimensions parse and are non-zero; recovery reserve < 100%.
596
597	Output is a machine-readable report at `$GAUNTLET_REPORT_DIR/doctor.json` plus a human summary. Exit non-zero on any REQUIRED failure.
```

(Note: line 590 appears twice above due to a transcription artifact in the agent's
extraction; the canonical list is eight checks, lines 588–595. If in doubt, read the
spec directly.)

## 6. §8.6 Defaults table — COMPLETE (lines 599–612)

```
599	### 8.6 Defaults table (conservative bootstrap)
600
601	| Setting | Default | Rationale |
602	|---|---|---|
603	| `max_rounds` | 8 | bounded even if unset |
604	| `max_parallel` | 2 | parallel writers corrupt each other's assumptions |
605	| `max_depth` | 1 | recursion is raised deliberately, never by default |
606	| `max_wall_seconds` | 3600 | |
607	| `recovery_reserve_pct` | 15 | a run must always afford to stop cleanly |
608	| `diagnostic_budget` | 2 | diagnosis is bounded too |
609	| `review.mode` | `adaptive` | pay for judgment only where instruments cannot decide |
610	| `sandbox.backend` | `none` | with tier `contained` only; any higher tier without a backend refuses to dispatch |
611	| `interactive` | auto-detect TTY | unattended runs record decisions instead of asking |
612	| `git_strategy` | `worktree` | isolation without cloning cost |
```

`git_strategy` vs `versioning` naming — current states:
- Line 612 (§8.6): setting still named `git_strategy`, default `worktree`.
- Line 569 (§8.3): env var still listed as `_GIT_STRATEGY`.
- Line 2093 (Annex E): env var is `GAUNTLET_VERSIONING_STRATEGY` (Permissions group).
- Line 576 (§8.4 capability list): `versioning` (renamed from `git`).
- Line 534 (§8.2 example): `facets: [versioning, filesystem, build, test]`.
- Line 886 (§12.2 facet list): `versioning · filesystem · build · test · lint/static · benchmark · runtime_probe`.
- Stale leftover: line 1369 (§25 layout) still lists an adapter named `git`.

## 7. Whole-spec keyword hits

`EffectiveConfig` — zero hits. `canonical serialization` — zero hits. Spec term:
"The effective config is digested into `Run.config_fingerprint`" (line 508).

`config_fingerprint` / `fingerprint`:
- 308: `config_fingerprint: # digest of effective config (§8.5)` (§5.3.3 Run contract; 309: environment_fingerprint)
- 508 (§8.1). 890 (§12.2 facet records environment fingerprint). 1940 (Annex B.3 `"environment_fingerprint": ""`).

`precedence`:
- 508 (§8.1).
- 1504 (AK-015): `claim: Configuration resolves in the order defaults < GAUNTLET.md < environment < CLI flags, and the effective configuration is digested onto the run.` / evidence_required `{kind: test_report, locator: tests/contract/config::precedence, producer: instrument}` (verify_by: test; blast_radius: local; authority: critic).

`doctor` hits outside §8:
- 438 (§7): runner `isolation: none` MUST NOT be selected for a tier requiring containment; configuration error surfaced by doctor.
- 815 (§11.1): `init → doctor → freeze reference → plan slices → [iterate]* → terminal → report`.
- 1076 (§17.3): absence of `irreversible` policy file is a requirement; doctor fails if one is present.
- 1176 (§20.1, below).
- 1357 (§25): `reports/ # doctor, run, handoff reports`.
- 1518 (AK-017): doctor performs every preflight check and exits non-zero on required failure; locator `tests/e2e/doctor`.
- 1745 (AK-049): sandbox policy file for irreversible tier causes doctor failure; locator `tests/e2e/doctor::irreversible_policy_is_error`.
- 1822 (AK-060): CLI command set includes doctor; locator `tests/e2e/cli_surface`.
- 1993 (Annex C): re-verify with `gauntlet doctor --probe-runners` before trusting.
- 2104/2133 (Annex F): policy shapes illustrative, verify at doctor time; irreversible: no policy file, presence is a doctor failure (AK-049).

`preflight` outside §8: 1117 (§18.3 resume step 5): "Re-run preflight (§8.5) and continue."

§25 layout (1349–1351): `GAUNTLET.md # runtime policy contract (config front matter)`; `.env.example # every GAUNTLET_* var, empty values`.

## 8. §20.1 CLI (lines 1171–1189)

```
1173	### 20.1 CLI (reference surface, REQUIRED)
1175	```text
1176	gauntlet doctor      # preflight; machine-readable report; non-zero on failure
1177	gauntlet init        # scaffold config, roles, skills, policies, .env.example
1178	gauntlet reference   # form | freeze | show | digest | diff | verify
1179	gauntlet run "<desired transformation>"
1180	gauntlet status      # runs, units by tier, in-flight invocations, budget burn
1181	gauntlet inspect     # run/iteration/invocation/unit/evidence detail
1182	gauntlet verify      # re-run verification for a unit or a run
1183	gauntlet resume <run_id>
1184	gauntlet escalations # list | resolve | retry
1185	gauntlet roles       # install | verify | list
1186	gauntlet skills      # install | verify | list
1187	gauntlet improve     # optimizer proposals; evaluate; promote/rollback
1188	gauntlet export      # deterministic JSONL of a run
1189	```
```

## 9. Secrets — every hit

- 509 (§8.1): "Secrets by indirection only... validates presence, and never logs the value. Generated files never contain credentials. A `.env.example` with empty values is REQUIRED."
- 588 (doctor check 1): "secrets present but unprinted."
- 1211 (§20.3 Logging): "...secrets never printed. Log sinks are configuration (`GAUNTLET_LOG_SINK`)..."
- 1247 (§22 item 4): "**Secrets** via `$VAR` indirection; presence validated without printing; never written into generated files, prompts, envelopes, or evidence."
- 1525 (AK-018): `claim: Secrets are referenced by indirection, validated for presence without printing, and never appear in logs, prompts, envelopes, evidence, or generated files.` (verify_by: test; blast_radius: cross_cutting; authority: lead_agent; locator 1529: `tests/contract/secrets::never_leaked`).
- 1248 (§22 item 5): "**The grader is not writable by the graded.** Answer key, frozen reference, evidence store, and event log are outside every builder-writable path."
- AK-019 (1532): `.env.example` lists every recognized `GAUNTLET_` variable, empty values, no credentials (locator `tests/contract/env_example_complete`).
