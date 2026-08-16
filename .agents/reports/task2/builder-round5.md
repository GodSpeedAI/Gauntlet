# Task 2 — builder, round 5

Scope: `crates/gauntlet-arch-tests/src/**` only. No core crate, adapter, justfile
recipe, `.env.example`, spec, `.wayfinder/**` or `.gauntlet/**` file was touched.
No `.cargo/config.toml` was created in the repo — that channel is tested with
temporary-directory fixtures.

All paths below are relative to `/home/sprime01/projects/gauntlet/`.

## Findings

| id | state | fix (file:line) | pinned by |
|---|---|---|---|
| C-A external-dependency policy | CLOSED (**already closed on arrival**, verified + extended) | `crates/gauntlet-arch-tests/src/deps.rs:109` (per-layer allowlists), `:256` (`dependency_sections` — normal/dev/build/target), `:380` (root `[workspace.dependencies]`), `:679` (member allowlist enforcement) | `lib.rs:1123 c1_external_dependencies_compiling_into_the_domain_fail_the_gate`; `deps.rs:1616 a_registry_dependency_compiling_into_the_domain_is_flagged`; **new** `deps.rs:1951 the_external_policy_covers_build_and_target_scoped_sections`; **new** `deps.rs:1579 allowlist_entries_state_a_reason` |
| C-B cargo-config rustflags/linker/registries anywhere | CLOSED | `deps.rs:918` (`FORBIDDEN_CARGO_CONFIG_KEYS`), `:1119` (`walk_cargo_config`, generic walk), `:969` (`[source.*]` + `[registries.*]`) | `deps.rs:1832 target_scoped_rustflags_and_other_compiler_channels_are_flagged`; `lib.rs:1330 c1_target_scoped_cargo_config_channels_fail_the_gate`; `deps.rs:1596 forbidden_cargo_config_keys_state_a_reason` |
| C-C allowlist admits (name, source) pair | CLOSED | `deps.rs:493` (`admissible_by_allowlist`), `:505` (`has_alternate_registry`), `:679` (pair check + typed message), `:365`/`:380` (root table classifies and rejects alternate registry), `:863` (`[patch]`/`[replace]` names the source kind) | `deps.rs:1983 an_allowlisted_name_from_a_non_default_source_is_flagged`; `deps.rs:2051 root_table_and_patch_entries_reject_alternate_registry_sources`; `lib.rs:1368 c2_an_allowlisted_name_from_a_hostile_source_fails_the_gate` |
| C-D string-aware attribute bracket scan | CLOSED | `escapes.rs:360` (`attribute_body_end`, string/raw/char aware — **present but untested on arrival**), `escapes.rs:237` (`scan_unterminated`, new: unterminated input is a violation) | **new** `escapes.rs:780 a_bracket_inside_a_literal_does_not_close_the_attribute_body`; `escapes.rs:811 literals_and_lifetimes_do_not_produce_phantom_escapes`; `escapes.rs:830 unterminated_input_is_a_violation_not_a_skip`; `lib.rs:1413 c3_a_bracket_in_a_literal_no_longer_hides_a_path_escape` |
| M-1 provider/endpoint class, scheme-less | CLOSED | `literals.rs:791` (`find_provider_endpoint`), `:167` (`ENDPOINT_TLDS`), `:176` (`ENDPOINT_EXEMPTIONS`), wired at `:622` | `literals.rs:1459 scheme_less_provider_endpoints_are_flagged`; `literals.rs:1495 endpoint_exemptions_state_a_reason`; `lib.rs:1445 critic5_literal_evasions_fail_the_gate` |
| M-2 relative paths widened + tested exemptions | CLOSED | `literals.rs:734` (`looks_like_relative_path`, extension requirement removed), `:198` (`RELATIVE_PATH_EXEMPTIONS` with reasons) | `literals.rs:1367 path_shaped_relative_literals_are_flagged_and_vocabulary_is_not`; `literals.rs:1437 relative_path_exemptions_state_a_reason`; `lib.rs:1445 critic5_literal_evasions_fail_the_gate` |
| M-3 statement window / split before `=` | CLOSED | `text.rs:244` (`declaration_units`: joins a `const`/`static`/`let` head to its `;`), `text.rs:140` (`MAX_DECLARATION_LINES = 500`), `text.rs:285` (`unjoinable_declarations`), reported at `literals.rs:488` | `literals.rs:1515 a_declaration_split_past_the_old_window_is_still_seen`; `lib.rs:1445 critic5_literal_evasions_fail_the_gate` |

Threat-model deliverable: `lib.rs:42` — `# THREAT MODEL — what this suite
enforces, and where its authority ends`, in three parts (enforced surface;
what it cannot enforce, each with a one-line reason; residuals known and
accepted).

Also closed, from the round-5 vacuity critique, because both are one-file and
directly about whether these checks can go silent:

- vacuity M-1 (call-site exemption arguments were unchecked): `walk.rs:63`
  `DECLARED_SCAN_EXEMPTIONS` (path + reason) and `lib.rs:336` (the shared scan
  refuses an undeclared exemption). Pinned by `lib.rs:1503
  an_undeclared_call_site_exemption_fails_the_gate`, which also asserts the one
  declared exemption is still honoured.
- vacuity M-2 (AK-019's root entry point had no adversarial fixture): pinned by
  `lib.rs:1559 env_example_gaps_fail_through_the_root_entry_point` — missing
  variable, non-empty value, missing file, and a clean negative.

## Gate exit codes (run in this order, verbatim)

```
just check       0
just lint        0
just deny        0
just boundaries  0
just ruler       0
```

`just boundaries`: **159 tests run, 159 passed, 0 skipped.** (`cargo fmt` was
run before `just lint`; `just deny` prints four `license-not-encountered` and
one duplicate-`winnow` warning and exits 0.)

`du -sh target` at the end: **360M** (ceiling not approached; no `cargo clean`
was needed).

## Teeth verified by mutation, not by assertion

Each fix was reverted in place, the suite re-run, and the fix restored:

- string-awareness removed from `attribute_body_end` → 2 failures
  (`a_bracket_inside_a_literal_…`, `c3_…`);
- `admissible_by_allowlist` forced to `true` → 2 failures
  (`an_allowlisted_name_from_a_non_default_source_…`, `c2_…`);
- generic `walk_cargo_config` call removed → 4 failures (including the
  pre-existing `m5_patch_git_and_cargo_config_fail_the_gate`);
- relative-path rule reverted to extension-only, endpoint class disabled,
  unjoinable-declaration report disabled → 5 failures.

## Found by this builder, not by the critics

1. **Cargo configuration is read from the invocation directory upward, not only
   from the workspace root.** The C-B fix as specified would still have missed
   `crates/gauntlet-domain/.cargo/config.toml`, which applies to every
   `cargo build` started inside that crate — an ordinary thing for a developer
   to do. `check_cargo_config` now DISCOVERS every `.cargo/config{,.toml}` in
   the tree (`deps.rs:1038`, skipping `target/` and `.git/`, reporting an
   unreadable directory as a violation) and applies the same rules to each.
   Fixture in `deps.rs:1832`.
2. **`rust-toolchain.toml` is a declared build input with the same power as
   `rustc =` in a cargo config.** `[toolchain] path = "/opt/evil/toolchain"`
   replaces the compiler for every build of the workspace. Closed at
   `deps.rs:1091` (`check_toolchain_file`); a pinned `channel` stays clean,
   because a released channel is ordinary reproducible practice. Without this,
   the threat-model sentence "the repository's own declared build inputs" would
   have been an overclaim.
3. **A member manifest may narrow an inherited dependency to its own registry.**
   `dep = { workspace = true, registry = "mirror" }` inherits the root entry's
   name but resolves through a different index; the narrowing now decides the
   source kind (`deps.rs:590`), so inheritance cannot launder an alternate
   registry.
4. **The prompt's description of C-A was stale.** The external-dependency
   policy (per-layer deny-by-default allowlists, all sections, root table) was
   already implemented and tested when this round started — the round-4 builder
   closed it. What was missing was the source half (C-C), coverage of
   build/target-scoped sections in the tests, and a warrant on the allowlist
   itself. Reporting it as newly closed would have been a false claim, so it is
   marked "already closed on arrival".
5. **C-D was half-done on arrival too**: `attribute_body_end` was already
   string-aware in the working tree, with a `critic-5 C3` comment and **no test
   anywhere**. A fix with no pin is one refactor away from being undone, which
   is why the mutation probe above matters more than the diff.

## Deliberate behaviour changes a reviewer should see

- `literals.rs` negative fixtures `"a/tmpfile"`, `"run/state"`,
  `"config/defaults"` moved from "clean" to "flagged". That is the M-2 widening,
  not a regression: §8.1 says "no path", and an extension-less relative path
  resolves against the working directory exactly like an extension-ed one. The
  cases that are genuinely not paths (`and/or`, `input/output`, `read/write`,
  `n/a`, `24/7`, five IANA media types) are now exempt BY NAME with a stated
  reason each, and `relative_path_exemptions_state_a_reason` fails if an entry
  arrives without one.
- `MAX_STATEMENT_LINES` stays at 24 for the LOOSE join (any line ending on an
  open token); welding hundreds of lines of arbitrary code together would
  manufacture matches nobody wrote. The correct join is the new declaration
  join, which is driven by the language (`;`) rather than by a formatting
  heuristic, budgeted at 500 physical lines, and reports rather than skips when
  it runs out.
- Comments are stripped from the JOINED declaration form only (`text.rs:296`, applied at `text.rs:252`).
  Without that, critic-5 M1's filler made `//` the token introducing the value
  and the timeout class stayed silent even after the join. The physical lines
  are still scanned with their comments intact.

## NOT closed — stated plainly

- **Unbalanced brackets WITHIN a scan unit are not reported**, only unbalanced
  brackets within a whole `.rs` FILE (`escapes.rs:237`). A legitimately
  multi-line attribute looks unbalanced on a single physical line, so reporting
  per-unit would be a false-positive factory. The fail-safe path is still
  closed — an unbalanced body makes the scanner treat the entire remaining text
  as the attribute body, so a `path` key in the tail is read, not skipped — but
  the per-unit case is a "no silent skip" guarantee rather than an explicit
  violation. That is weaker than the letter of the C-D instruction and is the
  one place I traded a violation for precision.
- **The unterminated-input scan is `.rs`-only.** A Markdown or TOML file under
  a core crate with an unmatched `[` is prose or a table header, not structure
  this scanner understands. A `.md` file that ends inside a code fence is
  therefore not reported.
- **`string_literals` still mis-pairs quotes across a `'"'` char literal**
  (round-5 vacuity minor m-3). Untouched: it is a minor, it is in a function
  every class depends on, and changing it in the same round as four criticals
  is how a suite acquires a regression nobody notices.
- **The disclosed residuals remain open by instruction**: runtime string
  assembly; whitespace-split directive spellings beyond those normalized;
  spelling variants of a tool identity laundered through a const; UNC paths;
  well-known ports outside the curated list; un-prefixed secret tokens in
  `.env.example` comments. All are now enumerated in the threat model at
  `lib.rs:42` as accepted, not as unknowns.
- **Enumerated lists remain enumerated.** `ENDPOINT_TLDS`,
  `PROVIDER_IDENTITIES`, `MODEL_FAMILIES`, `COMMAND_HEADS`, `TIMEOUT_HINTS` and
  `WELL_KNOWN_PORTS` all miss by construction: an endpoint under an unlisted
  TLD (`api.acme.internal`), a provider nobody listed, a duration-shaped
  identifier nobody listed. Each list is pinned by an inventory test so it
  cannot shrink silently, but no test can make an enumeration complete.
- **Environment-side channels are out of reach and stated as such**:
  `$CARGO_HOME/config.toml`, `RUSTFLAGS`, `CARGO_*`, a locally modified
  toolchain binary. Closing these needs a hermetic build, which is an
  infrastructure decision rather than a test, and I did not attempt to fake
  coverage of them.
</content>
</invoke>
