# Critic 1 (spec conformance) — 2026-08-16, artifact: crates/gauntlet-app/src/config + src/doctor, gauntlet-cli, GAUNTLET.md, justfile, arch-test allowlists

Status: in progress — sections appended as evidence is gathered. Do not settle on a partial read of this file.

## Ruling documents used

- `.agents/specs/Core-spec.md` §8 (all), §17.3, §20.1, Annex E.
- `.wayfinder/answer-key.md` AK-015, AK-017, AK-018, AK-019, AK-049.

## Evidence gathered so far

### §8.6 defaults table vs `config/defaults.rs`

All ten rows present with exact values (defaults.rs:40-61):
- max_rounds=8, max_parallel=2, max_depth=1, max_wall_seconds=3600 (u64), recovery_reserve_pct=15 (u8), diagnostic_budget=2, review.mode="adaptive", sandbox.backend="none", interactive=InteractiveDefault::AutoDetect (enum, not literal — faithful to "auto-detect TTY"), git_strategy="worktree". Conforms. Internal tests pin all ten (defaults.rs:279-298).

### §8.1 precedence vs `config/resolve.rs`

- Four layers applied in order defaults → front matter → env layer → flags (resolve.rs:493-505); adjacent-pair tests plus an umbrella `precedence` test prove each boundary (resolve.rs:1504-1549, 1726-1759).
- `.env` under process env (layer 3 internal order) tested at resolve.rs:1520-1526.
- `config_fingerprint` digests the canonical serialization; fingerprint moves at every layer boundary (resolve.rs:1751-1758; fingerprint.rs:23-33).
- Secrets: EffectiveConfig holds RAW `$VAR` tokens (resolve.rs:342-346); expanded products live in ResolvedConfig, documented never-serialized (resolve.rs:1006-1010). ConfigError variants carry variable/key names only (error.rs:18, 29, 32; display strings never echo values). Missing indirection is UnresolvedIndirection naming var+key (resolve.rs:1388-1391), tested with canary at resolve.rs:1625-1675.

(remaining sections: env layer, canonical form, doctor §8.5 walkthrough, output contract, CLI/justfile teeth, arch-test alignment, AK unit-by-unit, Annex E names, residuals)
