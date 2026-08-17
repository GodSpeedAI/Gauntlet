# Critic B — blind adversarial review (evasion / vacuity / correctness), Task 5

Scope attacked: `crates/gauntlet-app/src/config/**`, `crates/gauntlet-app/src/doctor/**`,
`crates/gauntlet-cli/src/main.rs`, `crates/gauntlet-cli/src/spec_vocab.rs`, `GAUNTLET.md`,
`tests/fixtures/config/spec-example.md`, the `justfile` `doctor` recipe, and the sha2
edits (`Cargo.toml`, `crates/gauntlet-app/Cargo.toml`,
`crates/gauntlet-arch-tests/src/deps.rs`). Baseline `cargo nextest run -p gauntlet-app
-p gauntlet-cli` is green (48/48). All reproductions below were run against the built
`gauntlet-cli` binary with throwaway trees under `/tmp/t5probe` (report dir redirected;
the repository was not mutated).

## Verdict — residuals-found

8 residuals: 1 critical, 2 major, 5 minor.

## Residuals

### R1 — CRITICAL (E3/E2/E7): a UTF-8 BOM silently discards the entire GAUNTLET.md layer; doctor reports "parsed", exits 0

Evidence: `crates/gauntlet-app/src/config/gauntlet_md.rs:289-303` — `parse()` detects
the opening fence with `line.trim() == "---"`. U+FEFF is not `char::is_whitespace`, so a
BOM-prefixed first line `\u{feff}---` never matches, `opening` stays `None`, and the
function returns `Ok(FrontMatter::default())` — the whole file is reclassified as
documentation, with no error and no unknown-key report.

Reproduction (all in `/tmp/t5probe`): a healthy scratch project whose `GAUNTLET.md`
front matter sets `budget.max_rounds: 5`, with a UTF-8 BOM prepended:

```
gauntlet doctor  →  1. config  pass
    - front matter parses  pass
        contract found and parsed at /tmp/t5probe/proj/GAUNTLET.md
    budget evidence: rounds 8 parallel 2 ...   # the DEFAULT, not the file's 5
exit: OK   (exit code 0)
```

Control run with the identical file minus the 3 BOM bytes: `rounds 5`, exit 0. A
one-byte encoding artifact — the default output of several mainstream Windows editors —
silently inverts precedence (layer 2 loses to layer 1), makes check 1 assert a false
statement ("found and parsed"), and still exits 0. This simultaneously breaks the §8.1
precedence contract, §8.2's typo-surfacing promise (nothing is reportable because the
parser believes there is no front matter), and the honesty rule that missing evidence is
never pass.

### R2 — MAJOR (E7/E2): any failure to READ the config path is silently conflated with "no GAUNTLET.md found"; check 1 passes with false evidence and the run can exit 0

Evidence: `crates/gauntlet-app/src/doctor/mod.rs:108` —
`let config_text = std::fs::read_to_string(&config_path).ok();` discards the error kind;
`crates/gauntlet-app/src/doctor/checks.rs:64-65` then emits the evidence string
"no GAUNTLET.md found; the environment fully specifies the run (spec 8.2)" whenever
`config_text` is `None`.

Three confirmed reproductions, each exiting 0 with check 1 **pass** and that (false)
evidence line, while the front-matter layer is silently dropped and defaults win
(`rounds 8` instead of the file's `9`/`5`):

1. `--config /tmp/t5probe/proj/GAUNTLET.mdd` (typo in an explicitly requested path).
2. `GAUNTLET.md` containing a non-UTF-8 byte (`read_to_string` → `InvalidData`).
3. `GAUNTLET.md` with mode 000 (`PermissionDenied`).

Case 1 is the worst: the operator explicitly named a config file; a typo silently falls
back to defaults with a green preflight. Not-found, unreadable, and non-UTF-8 are three
different states flattened into one; per the error-handling contract, failure must be
explicit and preserve the locus.

### R3 — MAJOR (E3/E8/E2): duplicate keys inside an inline flow mapping are silently accepted, last-wins — violating the parser's own documented grammar, with a fingerprint collision across documents

Evidence: block mappings reject duplicates
(`crates/gauntlet-app/src/config/gauntlet_md.rs:399-408`, `DuplicateKey`), but
`read_flow_map` (`gauntlet_md.rs:627-683`) performs no duplicate check at all, and
`walk_runners` (`crates/gauntlet-app/src/config/resolve.rs:683-703`) assigns per field
occurrence so the last spelling silently wins. The module header
(`gauntlet_md.rs:45-46`) documents "duplicate keys in one mapping (ambiguous documents
are not admitted)" as a typed error — without qualification of mapping style.

Reproduction (identical report dir so `report_dir` does not move the digest):

```
builder: { runner: aaa, runner: bbb }  →  config_fingerprint sha256:9943355f6860…96d
builder: { runner: bbb }               →  config_fingerprint sha256:9943355f6860…96d  (identical)
builder: { runner: aaa }               →  config_fingerprint sha256:7ee9502ea8cc…8a68 (differs)
```

No `DuplicateKey` error, no unknown-key report, no mention anywhere in doctor.json or
the human summary. The ambiguity the grammar exists to reject is resolved silently, and
two different documents (one inadmissible by the documented grammar) collapse onto the
same effective config, canonical bytes, and fingerprint. The same collapse exists one
level up for duplicate role names in `by_role: {builder: {...}, builder: {...}}`.

### R4 — MINOR (E5): AK-049 detection is anchored solely to the resolved policy dir and to `Path::exists()`; two escape hatch

Evidence: `crates/gauntlet-app/src/doctor/checks.rs:462-471` — the only probe is
`resolved.sandbox_policy_dir.join("irreversible.json").exists()`.

1. Override-away: with `irreversible.json` present in the **default**
   `.gauntlet/policies/` directory and the config overriding
   `sandbox.policy_dir: policies2`, check 5 reports "no top-tier policy **pass**" and
   the run exits 0. Spec §17.3 states the requirement absolutely ("**No policy
   exists** … `gauntlet doctor` fails if one is present"), and this repository's own
   `GAUNTLET.md` names `.gauntlet/policies` as the policy location; a forbidden file
   sitting there escapes detection whenever the dir is redirected. (Checking the
   resolved dir is defensible for the run itself — the stray file is inert — hence
   minor, but the spec text and repo contract name the location, not just the
   resolution.)
2. Broken symlink: a symlink named `irreversible.json` pointing at a nonexistent target
   reads as `exists() == false` → pass, exit 0, while the directory entry named exactly
   `irreversible.json` is present (`symlink_metadata` would see the name).

Survived the other direction: forbidden file in the *overridden* dir is caught; the
forbidden check runs even with `backend: none`; a project with no `sandbox` section at
all correctly passes; tier names match spec §17.3 (`contained`, `local`,
`cross_cutting`).

### R5 — MINOR (E6): vacuous assertion in `canonical::tests::secrets_never_leaked`

Evidence: `crates/gauntlet-app/src/config/canonical.rs:696-701`:

```rust
for text in [&canonical, &digest] {
    assert!(text.contains("TASK5_CANARY") || digest.contains("sha256:"), "{text}");
}
```

`digest` is the fingerprint string, which by construction always begins with
`"sha256:"` (`fingerprint.rs:18,30`; the shape is even re-asserted at line 705), so the
second disjunct is tautologically true and this loop cannot fail for either `text` — an
assertion that looks like a leak check but verifies nothing. The real assertions follow
it (lines 702-705). No other test in the artifact is vacuous: the defaults-table pins
and the allowlist pins can both fail on a const/list edit, and the doctor/parse tests
assert against real observable output.

### R6 — MINOR (E9): three `.env` layer quirks silently swallow operator intent, none surfaced as unknown

Evidence: `crates/gauntlet-app/src/config/env_layer.rs:149-179`
(`parse_dotenv`, `is_env_byte`, `strip_quotes`) and `is_gauntlet_name`
(`env_layer.rs:140-142`). All three confirmed at runtime:

1. `export GAUNTLET_MAX_ROUNDS=5` — the key `export GAUNTLET` contains a space, fails
   `is_env_byte`, and the line is skipped: no override applied **and** no unknown-name
   report (the Annex E typo surface the module promises misses the single most common
   dotenv spelling). File value `9` survived; `rounds 9`.
2. Inline comments are not comments: `GAUNTLET_STATE_DIR=/tmp/t5probe/state # inline
   note` yields the value `/tmp/t5probe/state # inline note` — confirmed verbatim in
   check 7 evidence — and doctor then `create_dir_all`s a directory literally named
   `state # inline note` and passes. Silent value corruption for every Text key.
3. Lowercase `gauntlet_max_rounds=5` is silently inert and unreported
   (`is_gauntlet_name` is case-sensitive, and the Annex E table matches exact case).

### R7 — MINOR (E4/E7): malformed `max_cost_usd` surfaces under the wrong failure locus

Evidence: money parsing happens in `parse_dollars`
(`crates/gauntlet-app/src/config/resolve.rs:1289-1314`) inside `resolve_against`; the
resulting `ConfigError::ValueOutOfRange` is carried as `products_error` and reported by
`check_config` under the "indirections" sub-check with evidence "indirection failed:
configuration: 'budget.max_cost_usd' carries a value outside its setting's domain"
(`crates/gauntlet-app/src/doctor/checks.rs:96-115,137-144`). Confirmed for `1e3`,
`-1`, `25.1.2`: each fails the run (exit 1 — correct) but is labeled an indirection
failure. Per the error-handling contract the locus should be preserved, not re-badged.
Related harmless quirk in the same function: `"25."` is accepted as exactly 25 (empty
fraction passes the `all()` vacuously) — no truncation or rounding was found anywhere
(7-digit fractions are rejected, overflow is `checked_*`).

### R8 — MINOR (E5, evidence): AK-049's declared evidence locator has no test at it

Evidence: `.wayfinder/answer-key.md:341-345` requires AK-049's evidence at
`tests/e2e/doctor::irreversible_policy_is_error`; `tests/e2e/` contains only
`.gitkeep`. The behavior itself is implemented and unit-tested
(`crates/gauntlet-app/src/doctor/mod.rs:381-404`), so this is an unmet evidence
locator — plausibly scheduled for the later e2e tasks — recorded here because missing
evidence is not passing evidence.

## Held — attacks the implementation survived

- E1: no channel from an expanded `$VAR` value into any `Display`/`Debug` string,
  `doctor.json`, canonical bytes, fingerprint input, human summary, or panic text —
  canary runs clean; every `ConfigError` variant carries names/kinds/keys only, and
  canonical/fingerprint digest raw tokens by design.
- E1: `UnresolvedIndirection` for an unclosed `${` echoes only the raw remainder
  starting at the `$` (raw config text), never an environment value; recursive
  indirection `A=$B B=$A`, lone `$` at EOF, and `${`-with-garbage all produce typed
  errors or literal `$` without echoing values.
- E2: all four layer boundaries hold at runtime (defaults < GAUNTLET.md < env < flags,
  process over `.env`), proven with the same report dir; cross-layer kind mismatch
  (string where a number is required) is a typed error, never coercion.
- E2: case-mismatched (`Budget:`), whitespace-padded (`budget :`), dotted
  (`roles.dir:`) keys and duplicate block keys are typed errors with correct
  line:col — none silently wins a layer.
- E2/E9: empty-string env var and unset env var behave identically (documented
  decision, `resolve.rs:900-905`) — confirmed at runtime; no divergence to exploit.
- E3: CRLF front matter parses correctly (confirmed: `rounds 7` through a CRLF doc);
  tabs, block scalars, anchors/aliases/tags, non-map top levels, unclosed quotes and
  brackets, trailing content, mixed sibling indentation, and unterminated fences are
  each their own typed error; empty fences → empty map; no panics found in any probe
  (byte-index arithmetic in `read_quoted`/`expand_indirections` is boundary-safe).
- E4: `25.999999` exact to micro-dollars; `0` reaches the domain `LimitZero` refusal;
  non-ASCII digits (`٢٥`), `+25`/`-1`, `1e3`, `25.1.2`, untrimmed ` 25` (process env),
  and >u64/1e6-overflow values are typed errors failing the run; no silent truncation,
  sign coercion, or rounding (7+ fractional digits rejected).
- E5: forbidden file in the overridden policy dir is caught; the forbidden check runs
  regardless of backend (incl. `backend: none`); no `sandbox` section at all → pass;
  uppercase `IRREVERSIBLE.json` rejected as an attack (not a runtime spelling).
- E7: `CheckOutcome::combine` ordering Fail > Unavailable > Pass > Skipped is airtight
  — no sub-check mixture lets a failed required check exit 0; all eight checks always
  appear; the degraded path concentrates the failure in check 1 with exit 1.
- E7: every `unavailable` reason names a real plan task (6, 7, 12, 14, 15–16 all exist
  in `.agents/plans/gauntlet-impl.md`) — references, not placeholders.
- E8: the canonical form is total (every known key emitted), byte-sorted, round-trip
  stable; `Int`-vs-`Str`, `None`-vs-`""`, and list order all survive serialization;
  two routes to the same effective config digest identically (the correct direction).
- E9: process environment beats `.env` for the same name (confirmed); duplicate keys
  within one `.env` last-win (conventional); CRLF `.env` handled by `lines()`.
- E10: the sha2 grant is application-layer only; the pin test asserts the exact
  allowlist (`["sha2"]`) and union (`["sha2", "toml"]`) plus a reason per entry;
  `package = "…"` renames are resolved before the allowlist check
  (`deps.rs:554`), and normal/dev/build/target-scoped sections are all covered — I
  found no way for another external crate to enter `gauntlet-app` unnoticed.
