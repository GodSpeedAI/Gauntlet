# Task 18 — Falsifier Round 2 (independent, blind)

## Scope

Independently re-attacks `crates/gauntlet-app/src/formation/mod.rs`'s
`check_reference_invariants`, the enforcement mechanism behind AK-065
("no fog entry appears in the answer key," Core-spec §9.2). This module was
just corrected for a class of bug found in round 1: it failed OPEN on
non-canonical but plausible `map.md` input (ASCII `->` arrow, case-variant
or wrong-depth section headings). The fix added `heading_text()` /
`section_of()` (case-insensitive, any heading depth, exact match against
the three canonical titles after `.trim()`) and `find_arrow()` (accepts
both `→` and `->`).

Method: added a throwaway `#[cfg(test)]` probe block to
`formation::tests` (13 new `zz_probe_*` tests using `eprintln!` to print
the actual `Result` for each hand-built `map_text`), ran them with
`cargo test -p gauntlet-app --lib formation::tests -- --nocapture`, then
restored the file byte-for-byte from a pre-edit backup (sha256
`6ed3d885e2886cf23b6802c6e26770f48a41f510afbc379136b6119243376bc0`,
confirmed identical before and after via `diff` + `sha256sum`; `git
status` shows the file only as the pre-existing untracked `??`, no
tracked diff exists since this module was never committed).

## Attack results

### 1. ASCII arrow spacing variants (fog section)

| Input | Result | Correct? |
|---|---|---|
| `- [F1] Leaked.->AK-101` (no space) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |
| `- [F1] Leaked. ->  AK-101` (two spaces) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |
| `- [F1] Leaked. ->\tAK-101` (tab) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |

All three refused correctly. `find_arrow` only needs to locate the `->`
byte offset; the trailer is recovered with `.trim()`, which absorbs any
amount/kind of intervening whitespace. No gap.

### 2. Case + depth combined heading

| Input | Result | Correct? |
|---|---|---|
| `#### NOT YET SPECIFIED` (depth 4, all-caps) then `- [F1] Leaked. → AK-101` | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |

Refused correctly. `heading_text` strips any run of leading `#`s and
`section_of` does `eq_ignore_ascii_case`, so depth and case are fully
orthogonal and both variants compose. No gap.

### 3. Heading trailing whitespace / trailing colon

| Input | Result | Correct? |
|---|---|---|
| `## Not yet specified   ` (trailing spaces) then `- [F1] Leaked. → AK-101` | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |
| `## Not yet specified:` (trailing colon) then `- [F1] Leaked. → AK-101` | **`Ok(())`** | **No — GAP** |

Trailing whitespace is handled: `heading_text`'s final `.trim()` strips
it before `section_of` compares. **Trailing colon is not handled**: the
compared string becomes `"Not yet specified:"`, which
`eq_ignore_ascii_case("Not yet specified")` rejects, so `section_of`
returns `None`. Per `parse_sections`, an unrecognized heading does **not**
close the current section — it "leaves the current section unchanged."
Since the immediately preceding section was `Decisions so far`, the `[F1]`
bullet carrying `AK-101` gets silently filed into `bullets.decisions`
instead of `bullets.not_yet_specified`. In the decisions loop it has a
non-empty `ids` list containing a real, known id, so it passes both the
`DecisionWithoutUnit` and `DanglingReference` checks cleanly. **The fog
item is silently accepted as a validated decision and `check_reference_invariants`
returns `Ok(())`** — the exact failure mode (fail-open via an unrecognized
heading spelling, item misfiled into the prior section) that round 1's
fix was written to close, just triggered by a heading variant (`:` suffix)
outside the fix's own test coverage.

### 4. Out-of-scope section — same variants

| Input | Result | Correct? |
|---|---|---|
| `## Out of scope` + `- [S1] Leaked.->AK-101` (ASCII arrow, no space) | `Err(OutOfScopePromoted { marker: "[S1]", ids: ["AK-101"] })` | Yes |
| `#### OUT OF SCOPE` (case+depth) + `- [S1] Leaked. → AK-101` | `Err(OutOfScopePromoted { marker: "[S1]", ids: ["AK-101"] })` | Yes |
| `### Out of scope:` (trailing colon) + `- [S1] Leaked. → AK-101` | **`Ok(())`** | **No — GAP** |

The fix's arrow/case/depth handling applies uniformly to all three
canonical sections as documented. But the trailing-colon gap reproduces
identically for Out of scope, for the same reason: unmatched heading →
section stays `Decisions` → the out-of-scope leak is misfiled as an
accepted decision referencing a real id.

### 5. Original round-1 attacks, re-run as sanity baseline

| Input | Result | Correct? |
|---|---|---|
| `- [F1] Leaked. -> AK-101` (ASCII arrow, normal spacing) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |
| `## Not Yet Specified` (title case) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |
| `### Not yet specified` (depth 3, right after `## Decisions so far`) | `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })` | Yes |

All three original attacks are now correctly refused, confirming the
round-1 fix holds for the exact cases it targeted.

### Bonus finding (not in the assigned list): leading whitespace before `#`

Probed one further variant in the same spirit — a single leading space
before the `#`:

```
 ## Not yet specified
- [F1] Leaked. → AK-101
```

Result: **`Ok(())`** — also a gap, and the same root cause.
`heading_text` calls `line.trim_start_matches('#')` directly on the raw
line; if the first character isn't `#`, nothing is stripped
(`after_hashes.len() == line.len()`), so the line is not heading-shaped at
all and the fog bullet stays filed under `Decisions`. This one is weaker
evidence than the colon case — the module's own doc comment scopes it to
"fixture-grade markdown, not CommonMark," and the shipped fixtures
(`tests/fixtures/formation/map.md`, `map-fog-leak.md`) and the
`wayfinder-gauntlet` skill template are all flush-left with no leading
indentation — so this may be legitimately out of the documented input
contract. Reported for completeness, not weighted into the verdict below.

## Regression / gate results

- `cargo test -p gauntlet-app --lib formation::tests` (13 shipped +
  9 probe variants + 4 baseline probes = 22 tests): **22 passed, 0
  failed.**
- `just check-package gauntlet-app`: **PASS** (cargo check clean).
- `just test-package gauntlet-app` (full package suite, post file
  restore): **405/405 passed**, including
  `gauntlet-app::formation fog_not_in_answer_key` and
  `gauntlet-app::skills wayfinder_gauntlet_conformance`.
- `just boundaries`: **228/228 passed.**
- `just ruler`: **PASS** — "answer key and manifest digests match the
  frozen lock."
- `crates/gauntlet-app/src/formation/mod.rs` restored to byte-identical
  pre-probe state (sha256 `6ed3d885e2886cf23b6802c6e26770f48a41f510afbc379136b6119243376bc0`
  before and after; `diff` against the pre-edit backup was empty).
- `.wayfinder/answer-key.md` sha256
  `ccc7c99be3d3b60488930eed9975dc6d67378a84bf3b13910b48511e9c228859` and
  `.gauntlet/reference.lock` sha256
  `006facaadaa1ceb84ce6189eff6bce5c034802ebda3e9042c8db2e5675812675`
  unchanged across the whole session (checked before and after; `just
  ruler` independently confirms the lock still matches).

## Verdict: NOT_CONFIRM

The round-1 fix genuinely closes the three cases it was built for (ASCII
arrow, case-variant heading, wrong-depth heading — individually and in
combination, and uniformly across all three §9.2 sections). But it did so
by hardening the match to a narrow set of known variants (case-fold +
depth-strip, then exact string equality) rather than by making the
underlying failure mode safe. The **same class of bug is still present**:
any heading spelling not in `{case-insensitive, any depth}` × `{exact
canonical title}` — e.g. a trailing colon, which is an entirely plausible
thing for an LLM-authored `map.md` to include before an itemized list —
causes `section_of` to return `None`, which `parse_sections` treats as
"leave the current section unchanged" rather than "unknown section,"
silently misfiling the fog/out-of-scope bullet into whatever section
preceded it. When that bullet references a real `AK-` id (the exact
scenario AK-065 exists to catch), the result is `Ok(())` — a fog or
out-of-scope leak into the answer key goes completely undetected. This
reproduces for both `Not yet specified:` and `Out of scope:`.

**Recommended fix direction**: don't grow the exact-match allowlist
further (colon today, closing `##` tomorrow, em-dash suffix after that).
Instead, either (a) strip trailing punctuation (`:`, `-`, `—`) in
`section_of` before the exact-match compare, or (b) — more robust —
change `parse_sections`'s "no match" branch from "current unchanged" to
an explicit `None`/`Unrecognized` state whenever a line is heading-shaped
but doesn't match a canonical title, so an ambiguous heading can never
silently continue a fog/out-of-scope leak's bullets in the wrong section.
