# Task 7 critic-B (blind adversarial) — findings

Method: source-only reasoning over normalize.rs, manifest.rs, parse.rs, provider.rs,
doctor/checks.rs, doctor/mod.rs, cli main.rs, p1_reference_integrity.rs, and
scripts/reference-digest.sh. No cargo runs. All claims are bounded to the inputs
and reasoning chains enumerated per finding.

## F1 — CRITICAL — panic on untrusted manifest: `>= 0 settlements` reaches `unreachable!`

- manifest.rs:1017-1019 (`build_definition`): `SettlementCriteria::new(...)` is
  `.unwrap_or_else(|err| unreachable!("the manifest parse already refused a reliability below one: {err}"))`.
- The premise is FALSE. Nothing in `parse_manifest`/`settlement_of` refuses a
  reliability of zero:
  - `settlement_of` (manifest.rs:897-901) takes `reliability_from(...)` directly.
  - `reliability_from` (manifest.rs:938-950) on `">= 0 settlements"`: marker found,
    digits = "0", `after` starts with "settlements", `"0".parse::<u32>()` = `Ok(0)`
    -> `Some(0)`. No `== 0` check exists anywhere in the file.
- `SettlementCriteria::new` (gauntlet-domain/src/reference.rs:293-297) returns
  `Err(MinimumReliabilityInvalid)` for 0 -> the `unreachable!()` fires -> PANIC.
- Triggering input: the frozen `.wayfinder/reference.yaml` with the single-token
  edit `>= 3 settlements` -> `>= 0 settlements` (also reachable by any hostile
  manifest; mod.rs declares these "untrusted external input, spec 21.2").
- Reachability chain: `FileReferenceProvider::resolve` (provider.rs:193-198) calls
  `parse_manifest` then `build_definition` BEFORE `Reference::new`, so the typed
  `ReferenceProviderError` channel never sees it; the process aborts on a panic
  instead. Every consumer of `resolve` (contract suite, doctor, CLI) inherits it.
- Fix: validate `minimum_reliability >= 1` in `settlement_of` as a typed
  `ManifestError` (e.g. a `ReliabilityBelowOne`/`BadShape` variant), or return the
  `SettlementCriteria::new` error as `ResolveFailed`. An `unreachable!` whose
  justification comment names a check that does not exist is the exact class of
  defect the comment was meant to prevent.

## F2 — MAJOR — the lock's `manifest_digest` is declared, whitelisted, and never verified

- checks.rs:453 lists `manifest_digest` in `LOCK_FIELDS` (and the frozen
  `.gauntlet/reference.lock` carries a real value for it), but
  `read_reference_lock` (checks.rs:475-532) extracts only `answer_key_digest`
  and `unit_count`; the string "manifest_digest" appears NOWHERE else in
  `crates/` except that whitelist entry (grep-verified).
- Consequence: an edit to the configured manifest (`.wayfinder/reference.yaml`
  or the contract's `manifest:` path) that keeps any column-zero `key:` shape
  passes doctor check 2 entirely — the manifest clause (checks.rs:210-233) only
  runs `top_level_mapping_keys`, the answer-key/schema/digest clauses do not
  read the manifest. `FileReferenceProvider::verify_unchanged`
  (provider.rs:273-307) also re-reads only the answer key; `freeze`
  (provider.rs:224-271) pins only the normalized answer-key bytes. The
  narrative half of the ruler (purpose, desired_state, invariants, success
  criteria, `minimum_reliability`, terminal conditions, unknowns) has NO
  integrity check anywhere in Task 7's machinery while the lock DECLARES one.
- Triggering input: in the doctor Scratch fixture, rewrite `reference.yaml`
  to `reference:\n  id: scratch\n  version: 999\n` — every check-2 clause
  still passes. (The doctor's own Scratch manifest at doctor/mod.rs:366 is
  already a manifest `parse_manifest` would REJECT — missing
  purpose/desired_state/settlement — yet check 2 passes it, demonstrating the
  gap from inside the test suite.)
- Fix: recompute `digest_of` over the manifest's raw bytes and compare with
  the lock's `manifest_digest` (a fifth sub-result or an extension of the
  digest clause), and/or have `verify_unchanged` cover the manifest digest.
  If manifest verification is out of scope, record it as a residual; the
  builder's round-1 residual list records only "doctor manifest clause still
  shape-only", not the unverified `manifest_digest` field.

## F3 — MAJOR — the tab-in-indentation guard in `lex` is dead code

- manifest.rs:209 computes `indent` as the count of leading SPACES
  (`line.len() - line.trim_start_matches(' ').len()`), so `line[..indent]`
  is by construction all spaces; `line[..indent].contains('\t')`
  (manifest.rs:214) can never be true. The advertised invariant at
  manifest.rs:204 ("A tab in the indentation is drift, not style") is
  unreachable.
- Silent-acceptance route (bounded to this one I traced): a folded body line
  whose indentation is spaces-then-tab, e.g.
  `  purpose: >\n  \tone paragraph.` — wait, indent 0 breaks folding; use
  `    \tone paragraph.` under a 2-space key: indent 4 > 2, body `\tone
  paragraph.` → `folded` trims it (manifest.rs:331) → silently accepted as
  prose "one paragraph.". Block-sequence scalar continuation lines
  (manifest.rs:407-421) absorb tab-led deeper lines the same way. A
  spaces-then-tab line that hits `mapping` fails loudly instead
  (`UnknownKey`/`NotAMapping`/`MalformedItem`), so most routes are loud; the
  folded/continuation routes are not.
- Fix: check `body.starts_with('\t')` (or scan the original raw prefix) and
  return the typed error, or delete the claim and the dead branch. A guard
  that cannot fire is worse than no guard: it passes review as enforcement.

## F4 — MINOR — `key_line` accepts far more than the documented "nothing or exactly one space"

- manifest.rs:249-260 strips at most ONE leading space from `rest`. Inputs:
  `purpose:  two spaces` → scalar `" two spaces"` (leading space survives,
  narrative check at :707 only trims for the emptiness TEST, so the space is
  stored); `purpose:\ttabbed` → scalar `"\ttabbed"` accepted as narrative;
  `id:\tref-1` → `ReferenceId::new("\tref-1")` fails loudly only because the
  domain refuses whitespace. Same one-space-only strip in parse.rs:312
  (`claim:  x` → claim `" x"`, and `Unit::new` only refuses claims that are
  whitespace-ONLY, so the leading space is stored).
- Wrong-but-plausible, silent; frozen files unaffected. Fix: reject any
  value whose first char is not exactly one space-separated boundary, or
  document the leniency.

## F5 — MINOR — `digest:` YAML-null spelling rejected despite header claiming "null or absent"

- manifest.rs:36-38 says the manifest's own `digest` key must be "`null` or
  absent"; manifest.rs:587-602 accepts only the literal scalar `null`. But
  `digest:` with nothing after it parses to an EMPTY SEQUENCE
  (value_of's empty-rest fallback, manifest.rs:304-311) → `BadShape`, i.e.
  a valid YAML null spelling is refused while the header promises it is
  allowed. Fail-loud direction, doc/behavior mismatch only.

## F6 — MINOR — flow-sequence empties are silently admitted

- `flow_value` (manifest.rs:464-484) splits on ',' and trims entries without
  rejecting empty ones: `source_refs: [a,,b]` → `["a", "", "b"]` with an
  empty source ref accepted; `success_criteria: [,]` → two empty criteria
  accepted into `SettlementCriteria` (the domain constructor does not check
  criteria text). Contrast: block-sequence and `parse_depends_on`
  (parse.rs:482-489) DO reject empties. Fix: reject empty entries after trim.

## F7 — MINOR — dangling and cyclic `depends_on` accepted; comment overclaims

- parse.rs:298-302's doc ("a `depends_on` target could never resolve to a
  block outside it") is about the AK prefix, but nothing checks EXISTENCE:
  `depends_on: [AK-999]` with no AK-999 block parses clean (close_block
  parse.rs:496-584 checks duplicates and required fields only), and cycles
  (A→B→A) are unchecked anywhere in the chain. Bounded claim: no existence or
  acyclicity check exists in parse.rs/provider.rs/manifest.rs.

## F8 — MINOR — folded/continuation lines starting with `#` are silently dropped

- `lex` (manifest.rs:211-213) drops any line whose body starts with `#` as a
  full-line comment, including lines INSIDE a folded block where YAML would
  treat them as content: `purpose: >\n    # part of the narrative` → line
  lost (and if it is the only body line, a confusing `EmptyFolded` instead
  of "content starts with #"). Silent content loss; the module header's
  stance ("the frozen manifest carries comments only at line start") is a
  frozen-shape claim, not an enforcement.

## F9 — MINOR — a bare `-` item line errors through a misleading route; `"- "` vs `"-"` asymmetry

- value_of dispatches to block_sequence on `next.text == "-"` but
  block_sequence's own `strip_prefix("- ")` (manifest.rs:357) cannot match
  "-", so it breaks with an EMPTY Seq and leaves pos on the dash; the
  enclosing mapping then errors with `MalformedItem "line sits deeper ..."`
  (manifest.rs:441-448) instead of "a bare dash". Loud but misleading; no
  hang (hand-traced: every path advances pos ≥ 1 or errors).

## F10 — MINOR — lock-count diagnostics: dead negative branch, misleading message

- checks.rs:515-516: `usize::try_from(u64)` cannot fail on 64-bit targets,
  so `"unit_count is negative"` is dead code there; meanwhile a NEGATIVE,
  float, or string count (`-2`, `2.0`, `"2"`) all fall into the
  `match field`'s `_ => None` at checks.rs:508-514 and report "carries no
  unit_count" — false ("carries" something malformed). Fail-loud preserved;
  message wrong. Duplicate JSON keys ARE rejected by the ports parser
  (json.rs:68) — verified, not a hole. Unicode-escaped field names decode
  before the whitelist check, so `\u0078` → "x" → unknown-field finding:
  correct.

## F11 — MINOR — doctor manifest clause passes an all-indented manifest as "0 keys"

- `top_level_mapping_keys` (checks.rs:420-444) `continue`s past every line
  that starts with whitespace (checks.rs:427-429); a file of ONLY indented
  lines yields `Ok(0)` → "manifest exists; top level is a mapping (0 keys)"
  → Pass. Zero top-level keys is not a mapping; the clause was already
  shape-only (recorded residual) but this specific pass-should-fail shape is
  unbounded.

## F12 — MINOR — lock path fields are never cross-checked

- `answer_key_path`/`manifest_path` in the lock are whitelisted but ignored;
  check 2 validates the CONFIGURED key/manifest against a lock that may name
  different paths. The digest comparison mostly saves this (a foreign lock
  fails unless the digest matches), but a lock naming `other-key.md` with
  the true digest passes — the record's path claims are unverified metadata.

## F13 — MINOR — CLI `--path --path` treats the second flag as the value

- main.rs:105-114: `["--path", "--path"]` → value "--path" accepted, fails
  later at read. Also `--path -x` accepted as value. Fail-loud end state;
  argument-hygiene nit only.

## F14 — BOUNDED POSITIVE — normalize.rs vs scripts/reference-digest.sh: no divergence found

- Battery hand-traced through both implementations: "", "\n", "\r", "\r\n",
  "a", "a\n", "a\n\n", "a\r\rb\n", lone-\r runs, "alpha\n  \n" (whitespace
  tail is content), interior blank lines, no-trailing-newline, UTF-8 BOM
  (BOTH keep U+FEFF as content — agree), NUL bytes, U+2028/U+0085 (neither
  treats as a line break), invalid UTF-8 (both refuse; Python traceback vs
  typed error, both non-zero). Python `str.split("\n")` and
  `while lines[-1]==""` match Rust `split`/`pop` semantics exactly; output
  spelling `sha256:<lowercase hex>` matches Digest's Display
  (values.rs:583-586). Claim is bounded to this battery; I did not execute
  either implementation (read-only task).

## F15 — BOUNDED POSITIVE — manifest traversal terminates on every input I could construct

- Hand-traced after the builder's runaway fix: value_of advances pos ≥ 1 on
  every arm (">": +1 then folded; non-empty rest: +1; empty rest: +1 before
  recursion); mapping errors on too-deep lines and otherwise only calls
  value_of; block_sequence's dash/mapping/scalar arms each advance; the
  bare-"-" break (F9) still nets +1 from value_of. I could not construct a
  non-progress cycle. Bounded: hand analysis only, no fuzzing.

## F16 — surface-6 verdicts on the three scanner accommodations

1. `field_keys()` const→fn (parse.rs:36-46): NOT a semantics change (same
   six strings; only used in the UnknownField message, parse.rs:103-108).
   The tier-class residual that sanctions it (literals.rs:36-39, "typed
   vocabulary, not policy") is real but OPEN: any function body could carry
   `"cross_cutting"` in expression position past the tier class the same
   way — the accommodation demonstrates the blind spot, it does not widen
   it. Residual drift risk: no test asserts `field_keys()` stays equal to
   the `match` arms in `field_line`, so the diagnostic list can silently
   lie (the match is the authority; the fn is a copy).
2. `format!("{}/{}", 1, 2)` needle (doctor/mod.rs:965-971): test-side only;
   the production fraction strings are ALSO built by format!
   (checks.rs:320-330 `"{total}/{total}"`, `"{}/{}"`), so no literal exists
   to flag. The relative-path class is structural-on-literals only; the
   same format! construction would smuggle a real multi-segment path
   (`format!("{}/{}", "etc", "passwd")`). Scanner residual, not an artifact
   defect; worth a recorded residual note since production code now uses
   the shape.
3. "just" reworded: "just" is in COMMAND_HEADS (literals.rs:300+), so an
   expect message headed by "just" with no stopword flags as
   hardcoded-command. Rewording is cosmetic, zero semantic content, and the
   abuse direction is the scanner's documented stopword residual
   (literals.rs:1290-1300), not this change.
None of the three smuggles a policy literal; none changes behavior.

## F17 — MINOR — near-vacuous assertion in the P1 workspace test

- p1_reference_integrity.rs:190-195: `!object.starts_with(workspace)` is
  guaranteed by the test's own layout (store under `evidence/`, workspaces
  as siblings); the only SUT-behavior assertion in the block is
  `object.is_file()` at the content address (line 196-199), which already
  implies confinement. Decorative, not wrong: asserting
  `object.starts_with(&store_root)` would test the same thing honestly.

## F18 — MINOR — an all-empty answer key is a legal ruler at provider level

- `parse_answer_key("")` → 0 units, 0 failures; `Reference::new` requires
  no units; freeze pins sha256(empty) happily. Compensating control exists
  and is real (doctor check 2's count + zero-blocks schema clause,
  checks.rs:261-279, 308-316), and the port-level leniency is the recorded
  contract-suite decision (provider.rs:23-48) — recorded, therefore not a
  silent weakening; listed for completeness with the note that
  `verify_unchanged` alone cannot distinguish "emptied ruler" from
  "legitimately blockless artifact" if a controller ever trusted it solo.

## F19 — MINOR — `version: +1` parses; duplicate-root error mislabels

- `" +1".parse::<u32>()` accepts the sign, so `version: +1` is admitted
  (manifest.rs:579-586) — lenient, not wrong-but-plausible. A second
  `reference:` root reports `NotAMapping { line: 1 }` (manifest.rs:243-246)
  — the message ("not a `key:` mapping") misdescribes a duplicate-root
  document and names line 1 instead of the second root's line.

Severity tally: 1 CRITICAL (F1), 2 MAJOR (F2, F3), 14 MINOR/bounded
observations. Strongest composite attack: a two-token edit of the frozen
manifest (">= 3" → ">= 0") panics the provider (F1); a whole-half rewrite of
the manifest passes doctor and every provider wire (F2).
