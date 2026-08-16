# Critic 9 — vacuity lens — `crates/gauntlet-arch-tests/`

Blind review, artifact-as-on-disk. Units graded: AK-001, AK-002, AK-014, AK-016, AK-019.

## Verdict

This suite is unusually hard to hollow out by accident: the shared scan path carries an anti-vacuity guard whose universe comes from an independent listing, every exemption inventory and term list is pinned by test, unreadable/unparseable input is a violation everywhere I traced it, and the evasion tests drive the real `check_*` entry points against synthetic workspaces rather than fixtures of the scanners' own construction. No check I read is a constant compared to itself in the way that matters (the "pins" compare an inventory to a literal duplicate and fire on any edit to the inventory). The declared residuals in `lib.rs` are real, not convenient: I verified that UNC paths, unlisted TLDs, unlisted model families/providers, and whitespace-split directive spellings (`#[cfg_attr(x`⏎`, path = "…")]`) genuinely pass, exactly as disclosed. Four majors remain. Two are undisclosed narrowings inside classes the threat model claims as covered (a command containing any of 37 English function words is invisible; a literal packed onto the same physical line as a §8.6 setting declaration escapes every class in the defaults module — the exemption is unit-granular, not declaration-granular, contradicting a specific threat-model sentence). One is a silent parse-drop in the AK-019 instrument that its own I8 posture elsewhere forbids. One is the locator pin, which verifies only `#[test]` + `check_` substrings and so cannot notice a locator body swapped to a different Unit's check. Zero criticals: nothing that claims to enforce an AK rule is without a firing path on the ordinary shapes of that rule's violations. One context note matters for settlement: the four core crates are today 1.9 KB of doc comments and a `println!` stub — every repo-level green, including all four answer-key locators, is currently evidence over stubs.

## Findings table

| id | severity | trigger | threat-model relation | one-line description |
|---|---|---|---|---|
| F1 | major | accidental + adversarial | contradicts "no environment hardcoded in core sources: … commands" (residual not declared) | any command string containing one of 37 stopwords passes the command class |
| F2 | major | adversarial | contradicts "only the declared default-setting declarations … are exempt, and every other literal class applies to the rest of that file" | §8.6 exemption skips the whole scan unit: extra literals on a setting line escape all classes |
| F3 | major | accidental + adversarial | concerns territory the threat model does not claim, but violates the suite-wide I8 posture envfile itself invokes | `.env.example` lines without `=` are silently dropped; unknown-key/duplicate/value checks never see them |
| F4 | major | accidental + adversarial | contradicts the instruments table: "each calling the `check_*` function beside it here" | locator pin checks only `#[test]` and `check_` substrings; a locator body swapped to another Unit's check stays green |
| F5 | minor | accidental + adversarial | disclosed in `literals` module docs, not in the lib.rs residual list | timeout class requires an enumerated `TIMEOUT_HINTS` fragment on the line |
| F6 | minor | adversarial | disclosed in `literals` module docs | tier-policy class only fires on `const`/`static`/`default` lines; function-body policy literals pass |
| F7 | minor | accidental + adversarial | disclosed by the rule statement itself | command/tool classes require an enumerated executable head; `"rm -rf workdir"` is clean |
| F8 | minor | accidental | territory not claimed; channel redundant today | root-table denylist scan blanks out unless the header is exactly `[workspace.dependencies]` |
| F9 | minor | accidental | weakens "measured against an independent listing" | walker and "independent" listing both `flatten()` away mid-iteration IO errors in lockstep |
| F10 | minor | adversarial | disclosed in a code comment at the derivation site | adapter selection by FIRST (capability) segment (`"state"`, `"agent"`, `"observe"`) passes AK-014 |
| F11 | minor | adversarial | check sold as measurement, is a pin | scope-agreement loops are true by construction (`in_literal_scope` delegates to `in_denylist_scope`) |
| F12 | minor (context) | n/a — planned build state | not contradicted; floors are structural | repo-level greens are currently evidence over doc-only stub crates |

## F1 — the command class's stopword guard launders real commands (major)

`literals.rs`, `find_command` (≈ line 1192):

```rust
for literal in literals {
    let tokens: Vec<&str> = literal.split_whitespace().collect();
    if tokens.len() < 2 { continue; }
    let head = tokens[0] ... ;
    if !COMMAND_HEADS.contains(&head.as_str()) { continue; }
    if tokens
        .iter()
        .any(|t| STOPWORDS.contains(&t.to_lowercase().as_str()))
    {
        continue;
    }
    return Some(literal.clone());
}
```

The stopword guard was built to keep English sentences out, but it applies to every token of the literal, not to a prose detector. Any hardcoded command whose arguments contain one of the 37 function words (`of`, `and`, `to`, `or`, `in`, `on`, `for`, `with`, `no`, `not`, `the`, …) is exempt in its entirety. The suite's own test `majors_that_defeated_a_class_fail_the_gate` proves `"cargo build --workspace 2>&1"` bites; the class cannot bite the same command with one more word. The module docs frame the guard's error direction entirely as false positives ("a sentence beginning with a tool name is excluded"; "An English sentence with no stopword … would flag") and never state that a genuine command containing a stopword passes; the lib.rs residual list (whitespace directives, UNC, unlisted ports/TLDs/providers) does not include it, while the mechanism list claims `literals` polices "commands".

Smallest mutation proving no teeth: in `crates/gauntlet-app/src/lib.rs` write `pub const BUILD_CMD: &str = "cargo build --workspace 2>&1 of";` — a real AK-016 command literal, structurally invisible. The un-flagged fixture differs from the flagged one by a single token. Accidental reach is real too: `git commit -m "fix of parser"` and `sh -c 'rm -rf /tmp and /scratch'` shapes pass.

## F2 — the §8.6 exemption is unit-granular, not declaration-granular (major)

`literals.rs`, `scan_literals` (≈ line 517):

```rust
for unit in crate::text::scan_units(text) {
    if defaults_scope && is_defaults_setting_declaration(&unit.text) {
        continue;
    }
```

`is_defaults_setting_declaration` tests only that the unit *begins* `… const/static <SETTING>`. Everything else in that unit is skipped with it. The threat model states, in "What it enforces": "the §8.6 defaults exemption is SETTING-scoped — only the declared default-setting declarations of the declared defaults module are exempt, and every other literal class applies to the rest of that file." The skip honors that sentence at file granularity and at line granularity, but not at token granularity: a second declaration packed onto the same physical line as a setting declaration is exempt in full. The warrant (`check_defaults_exemption`) uses the same shape test and is satisfied — the packed line still "declares" the setting — so nothing else fires.

Smallest mutation: create `crates/gauntlet-app/src/config/defaults.rs` declaring nine settings cleanly, plus one line `pub const MAX_ROUNDS: usize = 12; pub const BIND: &str = "localhost:8080";` — the network-endpoint and port classes never see the packed literal; `check_defaults_exemption` and `check_no_hardcoded_literals` are both green. (Vendor terms would still be caught by the denylist, which has no defaults carve-out; the literal-only classes — endpoint, port, URL, path, unlisted-family model id — are the blind ones.) The defaults module does not exist yet today; the hole goes live the day Task 5 lands it.

## F3 — `parse_env_example` silently drops lines without `=` (major)

`envfile.rs`, `parse_env_example` (≈ line 243):

```rust
let (key, value) = line.split_once('=')?;
```

A non-comment, non-blank line carrying no `=` produces `None` and vanishes before any check runs. The unknown-key check ("every `.env.example` key appears in Annex E, or is a NAMED exclusion"), the duplicate check, and the empty-value check all operate on parsed entries only. The module's own posture elsewhere is that malformed input is a violation, never a skip (`scan_manifest` in `denylist.rs` reports malformed TOML rather than returning empty); this is the one parse in the suite that fails open. The disclosed residuals in `envfile.rs` (RUNTIME_INJECTED growth, floor scoping) do not cover it, and the lib.rs residual "an un-prefixed secret token sitting in an `.env.example` comment" covers comments, not un-parsed entry lines.

Smallest mutation: append `GAUNTLET_LEGACY_TOKEN` (no `=`), or a bare hex line like `8f14e45fceea167a5a36dedd4bea2543`, to `.env.example` — `env_example_complete` stays green. Credential-shape prefixes (`sk-`, `eyj`, …) are still caught by the raw-text scan; everything else on such a line is invisible.

## F4 — the evidence-locator pin checks substrings, not the mapping (major)

`lib.rs`, `mod evidence_locators` (≈ line 1984):

```rust
assert!(
    text.contains("#[test]"),
    "locator {name} carries at least one test"
);
assert!(
    text.contains("check_"),
    "locator {name} drives a check_* entry point"
);
```

The crate docs and the instruments table claim: "Those locators are real cargo test targets … each calling the `check_*` function beside it here", with a table binding each locator file to the function for its Unit. The pin verifies name, path, existence, one `#[test]`, and the substring `check_` — which any `check_*` call satisfies, including a comment mention. Nothing verifies that `tests/contract/no_hardcoded_literals.rs` calls `check_no_hardcoded_literals` rather than `check_env_example`. The answer key's `evidence_required` locators point at these files; a swapped body leaves the AK locator producing the wrong Unit's evidence with the pin green. (The crate-internal `mod repo` tests duplicate the real checks, so the suite as a whole would still run them — but the evidence *address* recorded in the frozen ruler is silently wrong.)

Smallest mutation: in `tests/contract/no_hardcoded_literals.rs`, replace `arch::check_no_hardcoded_literals` with `arch::check_env_example`. The pin passes; the AK-016 locator no longer tests AK-016.

## F5 — timeout class requires an enumerated hint on the line (minor, disclosed)

`literals.rs`, `find_timeout` (≈ line 1260):

```rust
let hint = TIMEOUT_HINTS
    .iter()
    .find(|hint| contains_hint(lowered, hint))?;
```

A duration value on a line with no listed hint fragment is invisible: `pub const BUILDER_LIMIT: u64 = 900_000;` passes. This is recorded in the `literals` module residual section ("a duration-shaped name nobody listed (`BUILDER_LIMIT`) is not either"), which the lib.rs threat model delegates to ("each is recorded beside the class it belongs to in `literals` …"), though the lib.rs residual list itself names only ports/TLDs/providers. Mutation: rename `STALL_TIMEOUT_MS` to `STALL_LIMIT` — the timeout class goes silent on a real AK-016 timeout literal.

## F6 — tier-policy class is gated to declaration lines (minor, disclosed)

`literals.rs`, `find_tier_policy` (≈ line 1285):

```rust
if !(lowered.contains("const ") || lowered.contains("static ") || lowered.contains("default")) {
    return None;
}
```

A review/blast-radius policy literal in a function body — `fn policy() -> &'static str { "irreversible" }` — passes. Disclosed in the module docs ("tier policy is restricted to `const`/`static`/`default` lines … needs a real Rust parser"), with the `match`-pattern collision honestly argued. Mutation: move a flagged `pub const MODE: &str = "adversarial";` into a function body.

## F7 — command/tool classes require an enumerated executable head (minor, disclosed by rule statement)

`literals.rs`, `find_command` (≈ line 1209):

```rust
if !COMMAND_HEADS.contains(&head.as_str()) {
    continue;
}
```

`COMMAND_HEADS` has 57 entries; `rm`, `mv`, `cp`, `ln`, `chmod`, `kill`, `env`, `dotnet`, `ruby`, `perl` are absent. `pub const CLEAN: &str = "rm -rf workdir";` is clean under every class (head unlisted; not a whole-literal tool identity; no path/port/endpoint markers). The module table states the rule as "first token is an executable from `COMMAND_HEADS`", so the enumeration is on the page even though the lib.rs residual list does not name command heads. Mutation: the line above, as written.

## F8 — root-table denylist scan requires an exact header match (minor)

`denylist.rs`, `workspace_dependencies_text` (≈ line 446):

```rust
if trimmed.starts_with('[') {
    inside = trimmed == "[workspace.dependencies]";
```

A trailing comment on the header — `[workspace.dependencies] # inherited` — makes `inside` false forever after, and the one root table core crates inherit from is scanned as blank lines: a clean pass. I verified the channel is *today* redundant: a `store = { package = "rusqlite", version = "0.32" }` smuggled in there is still flagged by `check_workspace_dependency_table` as an unallowlisted registry entry, so the gate stays red. But the denylist half of AK-002's coverage of that table fails silently on an ordinary comment. Mutation: add ` # deps` to the header line of the real root manifest and observe the `workspace_dependencies_text` unit test's fixture set no longer represents the shape it claims to police.

## F9 — the "independent" listing shares the walker's silent-drop failure mode (minor)

`walk.rs`, `collect_files` (line 148) and `list_unfiltered` (line 214):

```rust
let mut sorted: Vec<_> = entries.flatten().collect();   // walker
for entry in entries.flatten() {                        // "independent" listing
```

Both drop an `Err` from the directory iterator without recording anything. The listing is documented as having "no exclusion logic to shrink" — true of exclusion logic, but a mid-iteration IO error shrinks the walker and its guard's universe in lockstep, invisibly to every check. Narrow and exotic (the walker does record unreadable-directory and unreadable-file-type errors; only iteration-time errors vanish), but the independence claim is weaker than stated. Mutation (conceptual): an IO error injected between two entries of the same directory is absent from `tree.files`, absent from `tree.violations`, and absent from `unfiltered_listing` — no guard can see it.

## F10 — adapter selection by the first (capability) segment passes AK-014 (minor)

`denylist.rs`, `Denylist::derive` (≈ line 208):

```rust
// … the first (`agent`, `state`, `observe`, `compare`, `evidence`) is
// the capability the spec itself uses, so it is deliberately not derived.
if let Some(last) = reversed.first() {
    push_unique(&mut segments, (*last).to_string());
}
```

Only the LAST segment of an adapter suffix becomes a whole-literal segment term. `match backend { _ => "state" }` in `gauntlet-app` — selecting among the state adapters by their shared first segment — passes `check_adapter_selection`, while `"sqlite"` or `"fs"` would fail. The exclusion is deliberate and argued in the comment (first segments are spec capability vocabulary), so this is disclosed territory, but it is a code comment, not a residual in the lib.rs list. Mutation: the match arm above.

## F11 — the scope-agreement checks are pins, not measurements (minor)

`walk.rs` (≈ line 289):

```rust
pub fn in_literal_scope(rel: &str) -> bool {
    in_denylist_scope(rel)
}
```

against `check_defaults_exemption` (lib.rs ≈ line 642):

```rust
for file in walk::core_tree(root).files {
    if walk::in_denylist_scope(&file) != walk::in_literal_scope(&file) {
```

and the parallel loop in `check_adapter_selection` (`!walk::in_denylist_scope(&file) || !walk::in_literal_scope(&file)` for composition-root files). Because one predicate is defined as the other, the comparison is `f(x) != f(x)` — true by construction today. It does have teeth against exactly one mutation (editing `in_literal_scope` to exclude a file), so it is not vacuous in the strict sense; but it is sold as a measurement ("the literal scope and the denylist scope agree on EVERY walked file … a reintroduced one fails here") when it can only ever fire on an edit to the predicate itself, never on drift introduced anywhere else. Any divergence introduced at a call site or via data (rather than via the two functions) leaves it green. Companion coverage exists (`assert_scan_is_not_vacuous` polices the predicates over the independent listing), which is why this is minor.

## F12 — today's repo-level greens are evidence over stubs (minor, context)

The four core crates contain, in total, 1,883 bytes: `gauntlet-domain/src/lib.rs`, `gauntlet-ports/src/lib.rs`, and `gauntlet-app/src/lib.rs` are doc comments only; `gauntlet-cli/src/main.rs` is a two-`println!` stub. The declared defaults module does not exist. Every repo-level assertion — including all four answer-key locators — currently asserts `violations.is_empty()` over files that cannot violate most classes. The suite's floors (manifest present, ≥1 `.rs` per crate, member set, adapter set) are satisfied by the stubs, so a permanent stub state would keep all AK-001/002/014/016 evidence green forever with zero product code. This is the planned state of a multi-task build, not a defect of the instrument, and the instrument's teeth are proven by the synthetic-workspace evasion tests; but a settlement decision that cites this suite as evidence those Units *hold* should know the evidence is presently stub-grade.

## Checks verified to bite

- `check_dependency_direction` — member floor (five named members + ≥ one adapter), unclassified-member rule, and `[workspace.dependencies] resolved to nothing` guard.
- External channel fails closed: registry/git/alternate-registry/`path`-outside-members/workspace-inheritance in normal, dev, build, and target-scoped sections; empty pinned allowlists with a stated-reason gate; allowlist admits (crate, default-registry) pairs only.
- Substitution channel: `[patch]`/`[replace]` non-member and non-path sources; `.cargo/config*` discovered anywhere in-tree (source replacement, `[registries.*]`, forbidden keys at any table depth); `rust-toolchain*` `path` key; unreadable/unparseable build inputs are violations.
- `check_denylist` — derivation-non-empty guard, case-insensitive substring matching over compounds, composition-root masking (not skipping), workspace-inherited rename resolution, root `[workspace.dependencies]` text scan, malformed-manifest violation.
- `check_adapter_selection` — derivation guard, `INNER_CRATES` scope with the shared anti-vacuity guard, segments-as-whole-literals rule (fires on `"prime"`, `"mock"`, `"sqlite"`, `"fs"`, `"git"`, `"diff"`, `"build"`, `"command"`, `"test"`).
- `check_no_hardcoded_literals` — each class fired by an evasion fixture through the real entry point: escaped/concat/continuation/split values, char-literal quotes, struct-field and match-arm timeouts, comment-`#[` tier carve-out closed, absolute/relative path structure, TLD endpoints, ports in every spelling.
- `assert_scan_is_not_vacuous` — undeclared call-site exemption refused, walker carve-out caught via the independent listing, scope-predicate carve-out caught, per-crate manifest + source floors, `skipped != 0` (I8).
- `check_defaults_exemption` — mention-shaped warrant rejected, sibling of the defaults module policed, defaults module inside the denylist (setting-scoped exemption is scanner-side).
- `check_scope_integrity` — `[lib]`/`[[bin]]` path escapes, `#[path]`/`cfg_attr` spellings, unresolvable `include!`, build scripts by file and by manifest key, unterminated string/comment/bracket I8 reports, no-Rust-source floor, unreadable/non-UTF-8/oversized files as violations, symlinks as violations.
- `check_env_example` — bidirectional annex↔file diff, derived non-vacuity floor (lockstep deletion caught), raw-text credential scan including comments, commented-entry values, runtime-injected exclusion enforced in both directions, `RUNTIME_INJECTED` pinned singleton.
- `text` machinery — the one lexing pass (`regions`) survives quotes-as-data in char literals, raw strings, and comments; join families (continuation, `concat!` with raw/comment fragments, statement splits, declaration joins with I8 on unjoinable).
- Inventory pins that fire on edits: `DENIED_TERMS`, model/provider/port/root/command/tool/timeout/tier/stopword lists, `RELATIVE_PATH_EXEMPTIONS`/`ENDPOINT_EXEMPTIONS` exact contents plus their stated-reason gates, `DECLARED_SCAN_EXEMPTIONS` empty, `DEFAULTS_TABLE_SETTINGS` ten rows, `TARGET_PATH_EXEMPT_CRATES` singleton, allowlist contents.
- `role_tests` — role assignment pinned over the real repo (inner > 0 and composition > 0, per-file role asserted).
- Declared residuals spot-checked as true (boundary honestly drawn): UNC paths pass; `api.acme.internal` (unlisted TLD) passes; `"acme-1"` model family and `"acme-cloud"` provider pass; `#[cfg_attr(x`⏎`, path = "…")]` (non-joined whitespace split) passes; runtime assembly (`["mo","ck"].join("")`) passes.
