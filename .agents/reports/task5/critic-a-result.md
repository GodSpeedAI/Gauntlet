# Critic-A result — Task 5 spec conformance (blind critic)

Compared against `.agents/specs/Core-spec.md` §8 (8.1–8.6), §17.3, §20.1, Annex E,
`.wayfinder/answer-key.md` AK-015/017/018/019/049, and the Task 5 plan
requirements as stated in the dispatch. Gates re-run by this critic:
`cargo nextest run -p gauntlet-app config:: doctor::` (47/47 pass), `just doctor`
(exit 0), `just boundaries` (190/190 pass), plus live CLI probes of exit codes
(0 healthy, 1 required failure via a planted `irreversible.json`, 2 usage).

## Verdict — residuals-found

## Residuals

1. **major — S6 (§8.5 check 5 / §17.3 / AK-049)** —
   `crates/gauntlet-app/src/doctor/checks.rs:466` (`dir.join("irreversible.json")`),
   `:469-479` (presence probe), and the required-tier loop `:483-511`
   (`format!("{tier}.json")`). Fact: the forbidden-tier guard probes exactly one
   filename spelling, `irreversible.json`; the required-tier clause likewise
   recognizes only `<tier>.json`. A policy file for the `irreversible` tier under
   any other spelling (`irreversible.yaml`, `irreversible`, different case) is
   invisible to check 5 and doctor stays green. Spec §17.3: "The absence of an
   `irreversible` policy file is a requirement … writing one implies a path that
   must not exist"; §8.5 check 5: "**no policy exists for `irreversible`**". The
   canonical name IS probed for real (proven by the in-crate test and this
   critic's live CLI run: check 5 fail, exit 1), so the pass is not vacuous —
   but the guard has a concrete false-negative path against a safety check whose
   purpose is to catch a file that must not exist. (The required-tier direction
   fails safe: an oddly spelled existing policy reads as missing.)

2. **minor — S1 (§8.6/plan requirement 1: defaults module is the only literals
   site)** — `crates/gauntlet-app/src/config/resolve.rs:414`
   (`workspace_use_worktrees: true`). Fact: the bootstrap default for the
   documented §8.2 setting `workspace.use_worktrees` is a hardcoded boolean
   literal in application code, while every other non-§8.6 bootstrap default
   (`PREFER_DETERMINISTIC`, `MAX_CRITICS`, `MAX_TOKENS`, `MAX_COST_USD`,
   `DIGEST_ALGO`) lives as a named constant in `defaults.rs`
   (`crates/gauntlet-app/src/config/defaults.rs:128-142`). The value matches the
   §8.2 example and the boundary literal scan does not class bare booleans, so
   the gate stays green — but the plan requirement says literals are permitted
   only in the defaults module. Related hygiene fact, same class:
   `checks.rs:482` compares the backend token against the literal `"none"`
   (the §8.6 default value spelled outside the module) rather than
   `defaults::SANDBOX_BACKEND`.

3. **minor — S7 (skipped never rolls up to pass)** —
   `crates/gauntlet-app/src/doctor/report.rs:87-102` (`CheckOutcome::combine`
   orders Fail > Unavailable > Pass > Skipped) and the emitted
   `.agents/reports/doctor.json` check 5: status `"pass"` while its sub-clause
   "tier policies exist" is `"skipped"` with a recorded reason. Fact: an
   `unavailable` clause never rolls up to pass (verified: checks 2, 3, 4 report
   `unavailable` on this host despite passing sub-clauses), but a `skipped`
   sub-clause can sit under a check-level `pass`. The skip is the spec's own
   conditional ("when `backend != none`") and the pass rests on the genuinely
   probed forbidden-tier clause; recorded as a fact because the contract states
   skipped never rolls up to pass, and at sub-clause granularity it does.

## Satisfied

- **S1** — All ten §8.6 rows present with exact spec values:
  `defaults.rs:40-61` (8 / 2 / 1 / 3600 / 15 / 2 / `"adaptive"` / `"none"` /
  `InteractiveDefault::AutoDetect` / `"worktree"`), unit-pinned by
  `defaults.rs:279-298`; the boundary suite masks exactly those ten declarations
  (`gauntlet-arch-tests/src/walk.rs:89-100`, declaration-granular since round
  6.1) and `check_defaults_exemption` requires the module to declare each
  (`gauntlet-arch-tests/src/lib.rs:604-697`); `no_hardcoded_literals` green in
  this critic's `just boundaries` run. Residual 2 is the one exception found.
- **S2** — Precedence `defaults < GAUNTLET.md < env < flags` with the
  `.env`-under-process inside fold (§8.1/Annex E), every adjacent pair
  test-proven: `config::resolve::tests::an_empty_world_resolves_to_the_defaults_table`
  and `::gauntlet_md_beats_the_defaults_table` (defaults<file),
  `::the_environment_beats_gauntlet_md` (file<env),
  `::the_process_environment_beats_the_dotenv_file` plus
  `config::env_layer::tests::the_process_environment_wins_over_the_dotenv_file`
  (.env<process), `::cli_flags_beat_the_environment` (env<flags),
  `::the_full_chain_shows_each_layer_winning_in_turn` and `::precedence`
  (whole chain, each layer boundary moves the fingerprint);
  `::discovery_follows_the_spec_order` pins §8.2 `--config` → `$GAUNTLET_CONFIG_DIR`
  → `./GAUNTLET.md` (`resolve.rs:1706-1717`).
- **S3** — Secrets discipline holds on every surface that exists. `ConfigError`
  variants carry variable NAMES, keys, and kinds only
  (`config/error.rs:11-39` — Display audited variant by variant); presence is
  validated without printing (`resolve.rs:1373-1393` `push_expanded`); canonical
  form and fingerprint run over raw `$VAR` tokens with canary tests proving the
  value never appears (`config/canonical.rs:650-706`, `config/fingerprint.rs:94-116`);
  doctor evidence withholds resolved values behind indirections
  (`doctor/checks.rs:639-645` `display_path`) and the canary test covers
  doctor.json and the human summary (`doctor/mod.rs:519-543`). Hunt results: no
  print/`{:?}`/`dbg!` call site over resolved products in the artifact (grep);
  `ResolvedConfig` derives `Debug` while holding expanded values
  (`resolve.rs:1011`) — latent affordance only, no call site. Two deliberate,
  documented echoes that are not credential surfaces: `GAUNTLET_INTERACTIVE`
  mode words (`defaults.rs:100-118`) and literal (non-indirected) paths in
  check-7 evidence.
- **S4** — Unknown front-matter keys collected with line
  (`resolve.rs:431-436,610-613,636-639,697-708`), unknown `GAUNTLET_*` env names
  collected with source (`env_layer.rs:38-43,128-136`), both surfaced in check 1
  sub-clauses "unknown keys"/"unknown environment" evidence, non-fatal per §8.2
  forward compatibility; proven by `doctor::tests::unknown_capabilities_fail_and_unknown_names_are_reported`
  (`doctor/mod.rs:496-497`) asserting `GAUNTLET_TYPO_VAR` and `unknown key 'mcp'`.
- **S5** — Canonical serialization documented and deterministic: byte-sorted keys
  by `BTreeMap` construction, two-space indent, LF, minimal escaping
  (`canonical.rs:1-16, 85-153`), no timestamps, raw indirection tokens;
  round-trip and however-built-identity tests (`canonical.rs:579-647`);
  `parse_canonical` rejects shapes the writer never emits (`:159-191`);
  `config_fingerprint` is sha256 over `to_canonical()` (`fingerprint.rs:23-33`),
  sensitivity proven (`fingerprint.rs:71-91`, `resolve.rs:1751-1758`).
- **S6** — All eight §8.5 checks exist and the passing statuses are earned by
  real probes, with deferred machinery honestly `unavailable`, never silently
  passed: 1 = parse/merge/expand actually executed, unknowns listed, failures
  fail (`checks.rs:56-148`); 2 = manifest read + top-level-mapping shape +
  answer-key AK-block count cross-checked against the frozen lock's `unit_count`
  (`checks.rs:154-355`), §5.3.2 unit validation and digest-vs-lock `unavailable`
  (Task 7, recorded); 3 = configured-runner structure real, registry resolution
  `unavailable` (Tasks 15–16); 4 = `unavailable` (Task 14); 5 = filesystem
  probes both directions, tested (`doctor/mod.rs:382-424`) and live-probed by
  this critic; 6 = §8.4 vocabulary membership real and failing on unknown names,
  tool-binding resolution `unavailable` (Tasks 15–16); 7 = create/write/remove
  scratch probes per directory + answer-key lexical AND canonicalized prefix
  check with canonicalization failure treated as a finding, not a pass
  (`checks.rs:650-731`), inside-workspace failure tested (`doctor/mod.rs:427-434`);
  8 = domain `Limits` construction (zero ceilings refused by the domain) +
  reserve 0/≥100 and diagnostic-0 failures (`checks.rs:742-818`), tested through
  the domain (`doctor/mod.rs:502-516`). Residual 1 is the check-5 gap.
- **S7** — `doctor.json` lands at the resolved report dir (`GAUNTLET_REPORT_DIR`
  honored via the known-key table `resolve.rs:303`; default `.agents/reports`,
  `defaults.rs:217-219`) and is canonical JSON (verified in the emitted file);
  human summary printed by the CLI (`gauntlet-cli/src/main.rs:56-64`); exit
  codes verified live by this critic: 0 healthy (`just doctor`), 1 on a required
  failure (planted `irreversible.json` via `GAUNTLET_SANDBOX_POLICY_DIR` →
  check 5 fail, exit 1; unparsable GAUNTLET.md via `--config` → exit 1), 2 on
  usage errors; `unavailable`/`skipped` carry recorded reasons serialized as
  `reason` in JSON and echoed in evidence lines; `unavailable` never rolls up to
  pass (checks 2/3/4 on this host). Residual 3 is the sub-clause-skip nuance.
- **S8** — See AK units below; behavior implemented here is distinguished from
  machinery the plan defers to Tasks 7/14/15–16, and the locator question is
  recorded there.
- **S9** — The repo's own `GAUNTLET.md` parses through the real binary (emitted
  `doctor.json`: `config_origin` set to it, check 1 pass, "no unknown
  front-matter keys") and conforms to §8.2's structure (front-matter map, every
  key known, H2 documentation body, omissions documented in-body);
  `spec_vocab.rs` mirrors Annex E exactly — 92/92 names, mechanical diff against
  the spec text by this critic, zero both ways — and §8.4 exactly — 17/17
  capabilities in spec order (`spec_vocab.rs:112-130,137-147`);
  `tests/fixtures/config/spec-example.md` reproduces the §8.2 example verbatim
  (including its 3/2/5400 budget, distinct from §8.6 defaults) and its parse is
  pinned (`gauntlet_md.rs:923-957`).
- **S10** — The sha2 edit is exactly the minimal recorded change: workspace
  `sha2 = "0.10"` with justification (`Cargo.toml` diff), `sha2.workspace = true`
  with justification (`crates/gauntlet-app/Cargo.toml` diff), one
  `APPLICATION_EXTERNAL_ALLOWLIST` entry carrying the recorded reason
  (`deps.rs:122-130`) per the documented procedure (`deps.rs:93-112`: entry +
  beside-it reason, gate fails until it exists, `allowlist_entries_state_a_reason`
  enforces reasons); the pinned assertions were updated to exactly the new truth
  (`assert_eq!(allowlist_names(...), vec!["sha2"])`, union `vec!["sha2", "toml"]`)
  and every other empty-list assertion (domain, ports, CLI, adapters, toml-only
  boundary dep, list count 6) is intact — nothing else weakened; `just
  boundaries` green (190/190) under this critic.

## AK units

- **AK-015** (precedence order + effective config digested onto the run) —
  **partial-here**: the four-layer order and fingerprint sensitivity are
  test-proven in-crate (`config::resolve::tests::precedence`, `resolve.rs:1726-1759`);
  "digested onto the run" stops at `DoctorReport.config_fingerprint` — no `Run`
  record exists yet to carry it; the locator `tests/contract/config::precedence`
  is an in-crate test name, not a repo-level target.
- **AK-017** (doctor performs every §8.5 check, non-zero on required failure) —
  **partial-here**: all eight checks present, required failures proven to fail
  the run and exit non-zero (in-crate tests + this critic's live CLI probes);
  checks 2 (§5.3.2 unit validation, digest-vs-lock), 3 (runner resolution),
  4 (capability-vs-tier), and 6's tool-binding clause defer machinery to Tasks
  7/14/15–16 with recorded reasons; the locator `tests/e2e/doctor` is not
  materialized (`tests/e2e/` holds only `.gitkeep`).
- **AK-018** (secrets by indirection, presence-validated, never printed) —
  **partial-here**: fully implemented and canary-tested on every surface that
  exists (error strings, canonical form, fingerprint input, doctor.json, human
  summary); "logs, prompts, envelopes, evidence" surfaces do not exist yet
  (later tasks), so coverage there is by-construction, not by-test.
- **AK-019** (`.env.example` lists every recognized `GAUNTLET_` variable, empty,
  no credentials) — **not-this-task** (pre-existing, verified satisfied now):
  exactly 92/92 Annex E names, all `VAR=` empty, mechanical diff against the
  spec text clean; pinned by the pre-existing `tests/contract/env_example_complete`.
- **AK-049** (an irreversible-tier policy file causes doctor to fail) —
  **satisfied-here**: forbidden-file presence fails check 5 and the exit code,
  proven by `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run`
  (which also asserts "AK-049" appears in both the human summary and the JSON)
  and by this critic's live CLI run (planted `irreversible.json`, exit 1);
  bounded by residual 1 (the probe is filename-exact) and the unmaterialized
  `tests/e2e/doctor::irreversible_policy_is_error` locator.
