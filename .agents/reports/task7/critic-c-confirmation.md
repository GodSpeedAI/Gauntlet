# Task 7 blind confirmation critic (critic C)

Date: 2026-08-18. Verifying the correction round against critic A and critic B findings. Read-only; no cargo/build. Report appended progressively.

## Method

Read in full: `crates/gauntlet-app/src/reference/manifest.rs` (1374 ln),
`crates/gauntlet-app/src/doctor/checks.rs` (1174 ln), `crates/gauntlet-app/src/doctor/mod.rs`
(1133 ln), `crates/gauntlet-app/src/reference/provider.rs` (647 ln),
`crates/gauntlet-cli/tests/p1_reference_integrity.rs` (377 ln),
`.agents/reports/reference-candidates.md`, `.gauntlet/reference.lock`,
`scripts/reference-digest.sh`, both critic reports, builder-checkpoints/round1,
CURRENT_STATUS (Task 7 region), escalations (diff). Read in part: domain
`reference.rs` (SettlementCriteria region), `normalize.rs` (signatures only),
`.agents/reports/doctor.json`. Read-only executions: `scripts/reference-digest.sh
.wayfinder/reference.yaml`, `sha256sum`, `od`, `git status/diff`, mtimes. No cargo,
no builds. All corrected files post-date critic-b-result.md (17:21); corrections at
17:25-17:27; doctor.json regenerated 17:26:46 (post-correction run, check 2 green,
both digest halves matching).

## Per-finding verdicts

| Finding | Verdict | Evidence (file:line) |
|---|---|---|
| B-F1 CRITICAL (unreachable! panic on `>= 0 settlements`) | **CLOSED** | Guard `if minimum_reliability == 0 -> ReliabilityBelowOne` in `settlement_of` (manifest.rs:926-933), reached via `parse_manifest` (manifest.rs:659) -> provider `read_manifest` (provider.rs:164-172) BEFORE `build_definition` (provider.rs:199); typed variant manifest.rs:81-89 with Display :153-157. Premise now true: domain `SettlementCriteria::new` (gauntlet-domain/src/reference.rs:286-305) has exactly ONE failure mode, `passes == 0` (:293-297) — nothing else can fail, so the rewritten `unreachable!` (manifest.rs:1049-1054) is sound for anything routed through parse_manifest. Discriminating test: manifest.rs:1252-1259 expects `ReliabilityBelowOne{line:11}` on `>= 0 settlements.`; guard removed -> parse_manifest returns Ok -> `expect_err` panics -> test fails. Residual nuance (not a defect): `ManifestDefinition` fields are pub, so a hand-built struct with passes==0 handed straight to `build_definition` would still panic; no such caller exists (grep: provider.rs:199 and manifest tests only). |
| A-M3 + B-F2 MAJOR (manifest_digest never verified) | **CLOSED** | `read_reference_lock` now REQUIRES `manifest_digest` (checks.rs:560-570): present, string, parses as domain Digest; missing -> "reference record carries no manifest_digest"; carried in `FrozenLock` (:513-517). Digest clause recomputes `digest_of(manifest_bytes)` (checks.rs:401) vs `lock.manifest_digest` (:406), mismatch -> Fail with "REFERENCE MOVED: the manifest's normalized digest ... reference-class failure (I1)" (:412-420); manifest read once and shared (:210, comment :207-209). Pinned by `a_moved_manifest_fails_check_two_as_a_reference_class_failure` (doctor/mod.rs:925-965): manifest rewritten to `version: 999`, shape clauses asserted Pass, digest clause asserted Fail, both halves asserted present ("answer-key half" :960-963) — removing the manifest comparison flips digest to Pass and fails the test. Real lock carries the field (.gauntlet/reference.lock:5) and I recomputed it read-only: `scripts/reference-digest.sh .wayfinder/reference.yaml` -> `sha256:95331d3e...5ba5e94` == lock (raw sha256sum identical — file already normalized, 1 trailing LF, 0 CR), so `digest_of`'s normalized recompute matches on the real repo; post-correction `.agents/reports/doctor.json` (17:26:46) shows check 2 pass with both halves matching. No lock shape broken: the only lock writers are the real frozen lock and `lock_text` (mod.rs:402-413), which now embeds the true manifest digest. |
| B-F3 MAJOR (dead tab guard in lex) | **CLOSED** | lex now measures the FULL leading whitespace run: `body_start = line.len() - line.trim_start().len()`; `line[..body_start].contains('\t')` (manifest.rs:222-228) — fires for tab-first, spaces-then-tab, and tab-between-spaces, since `trim_start` consumes the whole run. Byte-safety with multi-byte content after the indentation: `str::trim_start` advances by whole chars, so `body_start` is a char boundary; `line[..body_start]` spans only whitespace chars and `line[indent..]` starts at the first non-whitespace char — safe for any UTF-8 body. Discriminating test manifest.rs:1274-1292: tab-first root key (old code -> UnknownRootKey, variant assert fails), spaces-then-tab folded body (old code: the exact F3 silent-acceptance route -> parse Ok -> expect_err panics), tab-then-spaces sequence item. One adjacent lenience introduced — see NEW-1. |
| A-M1 MAJOR (AK-025 no recorded disposition) | **CLOSED** | reference-candidates.md:137-168: dated 2026-08-18 entry in the established format (Unit / Claim / Defect / Proposed correction / not-changed footer), cites "Task 7 correction C4, critic-A MAJOR-1", disposes BOTH claim halves honestly (cannot-adopt: P1 tripwire substrate; may-record: nothing yet, owned by Tasks 22/23, adversarial at Task 24). Ruler untouched: git status shows no `.wayfinder/` change; the entry records `just ruler` PASS verified in-session. |
| B-F6 MINOR (flow empties admitted) | **CLOSED** | `flow_value` rejects empty entries after trim with typed `MalformedItem` "carries an empty entry" (manifest.rs:490-500) — covers terminal_conditions, source_refs, success_criteria alike (same code path). Test manifest.rs:1296-1309: `[settled,, aborted]` and `[,]` both expect the typed error; refusal removed -> `[,]` parses as empty seq -> expect_err fails. |
| B-F17 MINOR (near-vacuous confinement assert) | **CLOSED (with note)** | p1_reference_integrity.rs:190-207 now asserts `object.starts_with(&store_root)` — exactly critic B's suggested honest form — plus `!store_root.starts_with(workspace)` and `!object.starts_with(workspace)` per workspace. Note: the root-containment assert is true by construction (object is built via `store_root.join(...)`), so the SUT-tied weight still rests on `object.is_file()` at the content address, the `store.get == normalized bytes` equality (:225-229), and the pin marker (:233-237); the workspace-exclusion asserts are the policy pin. No assertion weakened. |
| A-M2 MINOR-2 (header "exactly three callsites" overstates) | **CLOSED** | provider.rs:36-43 no longer claims three production callsites; it names the three surfaces and explicitly discloses `parse_all_or_fail`'s production caller as "the Task-12 controller, not yet built", with doctor check 2's unit-schema clause and the P1 clean-parse assertion as the live callsites. Verified: `parse_all_or_fail` exists (parse.rs:282), zero production callers — the disclosure is accurate. |

## Deferral records (verify-not-silently-dropped)

| Deferral | Recorded? | Where |
|---|---|---|
| Doctor manifest clause stays shallow (deep parse_manifest wiring) | **PARTIALLY** | builder-round1.md:246-248 (residuals, pre-correction) + in-code comment doctor/mod.rs:392-395 ("deep parse_manifest validation is a recorded deferral"). NO upgrade trigger is named anywhere — neither file names the task/event that wires the strict parser into check 2. Gap vs the repo's own deferral-ledger discipline (CURRENT_STATUS:280 "upgrades to blocking the moment a task walks into its trigger"). |
| Harness-only boundary teeth -> Tasks 12/13 (A-M4) | **Yes** | p1 header (p1_reference_integrity.rs:22-26), candidates entries (:127-131), provider header (Task-12 controller note), builder-round1.md:214-219. Correctly deferred — the plan itself sequences controller/settlement at Tasks 12/13. |
| ReferenceVersion.evidence_ref conventional linkage (A MINOR-1) | **NO** | Not recorded anywhere post-correction; survives only as the finding text in critic-a-result.md:116-124. Critic A's specific ask (note it in provider header or candidates) was not done. Dropped, not recorded-as-deferred. |
| B's remaining minors F4/F5/F7/F8/F9/F10/F11/F12/F13/F18/F19 | **PARTIALLY** | No correction-round disposition ledger exists (task6 has correction-round1.md; task7 has none — only reference-candidates.md and this file postdate the critics). The findings survive in-tree inside critic-b-result.md only. Mitigations observed in-repo: F11 (all-indented manifest passing shape clause) is now largely neutralized — such a rewrite fails the new manifest digest clause unless the lock is re-frozen; F12 (lock path fields unverified) stands. None of these minors should have blocked: all are leniency/diagnostics/doc nits, frozen artifact unaffected, and the manifest's integrity now has a real tripwire. The one that must not stay unrecorded: NONE at blocking severity — but a correction-round ledger naming dispositions + upgrade triggers should be appended before commit (the repo's own rule: "recorded, never silent"). |

## NEW findings (bounded to what I read)

- **NEW-1 MINOR — lex rewrite silently admits non-tab Unicode whitespace as indentation.**
  manifest.rs:222-230. Old code measured indentation as leading SPACES only
  (`trim_start_matches(' ')`); a line indented with, e.g., two U+00A0 NBSPs kept the
  NBSPs in its body and failed loudly (`UnknownRootKey` / `NotAMapping`). New code's
  `trim_start()` counts every `char::is_whitespace` rune (NBSP, U+3000, VT, FF...) as
  valid indentation, so `"\u{a0}\u{a0}id: scratch"` is now silently accepted as a
  2-indent mapping entry. Strict frozen-artifact reader accepts a new whitespace-drift
  class where it previously refused. Frozen manifest unaffected (spaces only);
  direction is silent-acceptance, the exact class F3 was about. Fix: restrict the
  indentation run to space-or-tab (e.g. `trim_start_matches([' ', '\t'])`) — the tab
  is then still caught by the contains check and the run is exactly the guarded set.
- **NEW-2 MINOR (doc-only) — misplaced doc comment on `read_manifest`.**
  provider.rs:162-163: "The normalized answer-key digest the ruler covers (spec
  §6.2). The digest is computed over the normalized answer-key bytes." sits on
  `read_manifest`, which returns a `ManifestDefinition` and computes no digest.
  File edited 17:26 (post-critics); may predate the correction — cannot bound
  further read-only.
- **NEW-3 OBSERVATION (process) — final checks.rs state postdates the last gate evidence.**
  `.agents/reports/doctor.json` was written 17:26:46 by a run whose check-2 evidence
  already shows BOTH digest halves (the corrected dual clause), but checks.rs mtime is
  17:27:11 — the file's final 25-second edit has no run in the tree witnessing the
  exact final bytes. The checkpoints' full gate record (builder-checkpoints.md:19)
  predates the critics entirely. Not a code defect; the commit gate must re-run on
  the final state.

Hunt specifics requested: constructor/test regressions — none found (`minimal()`
fixture uses `>= 1 settlements`, consistent with the guard; the frozen-manifest test
asserts passes==3 against the real ruler; `build_definition` test uses parse output
only). Borrow/move — the triple `match (&lock, &key_bytes, &manifest_bytes)` binds by
reference throughout; `digest_of` takes `&[u8]` (normalize.rs:120) so `&Vec<u8>`
coerces; nothing moved twice. Evidence strings — the new digest evidence carries
digests (printable, established practice), no resolved paths or withheld values.
Wrong-unit-count test still isolates the count locus (doctor/mod.rs:843-873: digests
true via `lock_text`, count 5 vs 2, digest clause asserted Pass). Healthy-path
fixture carries a true manifest digest (`lock_text` recomputes `digest_of` over the
actual manifest bytes, mod.rs:405-406). Absence of further defects claimed only
within the files and ranges listed under Method.

## Overall verdict

**CONFIRM** — all seven corrected findings (B-F1, A-M3+B-F2, B-F3, A-M1, B-F6, B-F17,
A-M2-minor) are genuinely closed with mechanisms and discriminating tests, no
assertion weakened and no adjacent behavior broken within what I read; the new
defects I found are minor (one real lex lenience regression NEW-1, one doc nit NEW-2,
one stale-gate-evidence observation NEW-3) plus a process gap — no correction-round
deferral ledger exists, evidence_ref was dropped rather than recorded, and the
doctor-manifest-clause deferral names no upgrade trigger — all closable with a ledger
append and one lex tweak before commit.
