# Task 7 blind critic (spec-conformance) — critic A

Date: 2026-08-18. Role: blind critic; judged only against the ruler, never the reverse.
Ruler: `.agents/plans/gauntlet-impl.md` Task 7 section (lines 285–311); Core-spec §5.3.1, §6.2,
§7, §8.5, §9.4; `.wayfinder/answer-key.md` AK-021–AK-025 (read verbatim, lines 142–176);
`scripts/reference-digest.sh` header rules (the normalization authority the plan points at).

## Verdict summary

**No CRITICAL findings.** The core Task 7 obligations are met and well-pinned: the Rust
normalization is a rule-for-rule reproduction of `scripts/reference-digest.sh` (verified by
inspection line by line, and the five SHA-256 constants pinned in `normalize.rs` tests were
independently recomputed with `sha256sum` — all match); the strict answer-key parser has real
per-block isolation and typed strictness wired through the domain `Unit::new` coupling rules;
freeze writes the normalized bytes into the content-addressed evidence store and pins them to
the run, with a beyond-plan guard against a normalize→freeze mid-flight edit; `verify_unchanged`
re-derives the digest and separates `ReferenceMutated` from `Unavailable`; doctor check 2's two
honest-unavailable clauses (unit schema, digest-vs-lock) are closed loudly on both surfaces with
a strict lock read; the CLI subcommand mirrors the script's output contract; the ruler itself was
not touched (git status shows `.wayfinder/` and `.gauntlet/reference.lock` unmodified, and my own
run of the script reproduces the lock's `answer_key_digest` exactly).

Four MAJOR residuals, all "partially met / overreaches evidence" shaped, none a flat violation:
(A) AK-025 is absent from the artifact with no recorded deferral although the plan's done-when
names AK-021–AK-025; (B) doctor check 2's manifest clause remains Task-5 shallow even though the
strict manifest parser now exists in the same crate; (C) the freeze record's manifest half
(`manifest_digest`) is accepted but never verified by anything, and no integrity tripwire covers
the manifest at all; (D) the done-when "teeth" condition can only hold inside the P1 test's own
harness loop — no production iteration-boundary callsite exists yet (deferred to Tasks 12–13,
honestly documented, but the done-when is not dischargeable as written today).

## Obligation-by-obligation

| Obligation (ruler) | Verdict | Evidence |
|---|---|---|
| Step 1: normalization = script's documented rules, byte-for-byte (plan l.293; script header) | **Met** | `normalize.rs:63-97` — identical rule order (`\r\n` then `\r` fold; pop only trailing `""` split elements; rejoin + one LF or zero bytes; no per-line trim; no sort). UTF-8 refusal typed with byte locus (`NotUtf8`), matching the script's stderr+exit-1. Test pins at `normalize.rs:129-138` recomputed independently: `e49c81e2…`, `5518ad7a…`, `0e937c34…`, `3588d4ce…`, empty-string `e3b0c442…` all genuine. Whitespace-only last line kept as content (`normalize.rs:205-213`) — matches script semantics (only `""` is popped). |
| Done-when: both digest implementations agree (plan l.308; gate l.301-306) | **Partially met — evidence pending, honestly deferred** | Script side witnessed by me: `scripts/reference-digest.sh .wayfinder/answer-key.md` → `sha256:ccc7c99b…9c228859` == lock. Rust side NOT witnessed (cargo forbidden in this review); equivalence holds by code inspection and by the verified test pins. `reference-candidates.md:95-97` honestly states the gate was not run in the builder session and names the orchestrator's post-land gate as the confirming evidence. CLI output format matches the script's single `sha256:<lowercase hex>` line (`main.rs:138-141`; `Digest` Display at `values.rs:583-586`). |
| Step 2: freeze writes normalized bytes to evidence store, pins, records version on run (plan l.294; §9.4 step 4) | **Met at provider level** | `provider.rs:224-271`: re-reads, re-normalizes, re-digests and compares before put (mid-flight guard, a strengthening), `put(EvidenceKind::File)` + `pin(run_id, ref)`; returns `Reference.freeze()`. "Record on the run" is the caller's act, demonstrated through the real embedded state store in `p1_reference_integrity.rs:327-365`. P1 asserts the stored object is exactly the normalized bytes at the content address, and the pin marker exists (`p1_reference_integrity.rs:148-226`, matching the store's documented layout `objects/<b0-2>/<b2-4>/<hex>` + `pins/<run-id>/<ref>`). |
| Step 3: verify_unchanged at every boundary + before settlement; mismatch ⇒ terminal aborted{reference_mutated}, both digests preserved (plan l.295; §6.2 l.444; §9.4 l.666) | **Primitive met; boundary wiring not-yet (Tasks 12–13)** | `provider.rs:273-307` re-reads/re-normalizes/re-digests and returns `ReferenceMutated { expected, actual }` carrying both digests. No production iteration-boundary or settlement callsite exists (only the Task-4 contract-suite stubs and tests). "Sets terminal state to aborted" and "preserves both digests as evidence" (persisted, not just in the error) are controller responsibilities — see MAJOR-4. |
| Step 4: parser, strict enum validation, per-block isolation (plan l.296; §8.5 check 2 l.589; §5.3.2 l.293-298) | **Met** | `parse.rs:227-278` isolates block failures (one malformed block → that Unit fails, parsing continues); anomalies outside blocks carried as failures, never dropped (`parse.rs:254-262`). Strictness: unknown field/duplicate/missing/malformed/unknown-enum/invalid-locator all typed with line loci; tier/authority coupling + producer rule enforced via `Unit::new` (error variants confirmed at `gauntlet-domain/src/unit.rs:98-108`); pinned by `parse.rs:904-923`. Frozen key parses 70/70 clean, in id order (`parse.rs:628-664`). Doctor clause surfaces per-block loci and fails the run (`checks.rs:288-336`; `doctor/mod.rs:937-972`). |
| Step 5: P1 proof — builder writes key mid-run; abort, frozen copy unchanged, outside workspaces (plan l.297; AK-021–024) | **Met at substrate level, scope honestly declared** | `p1_reference_integrity.rs`: `frozen_copy_outside_workspace` (AK-022), `builder_mutation_of_the_active_reference_aborts_with_reference_mutated` (AK-023, both digests asserted), `digest_rechecked` (AK-024, fires at first boundary after the edit, plus the settlement pre-check), `the_run_carries_the_frozen_version` (durability). Header (l.22–26) states plainly that AK-023/024 remain substrate-level until Tasks 12–13. Path deviation from `tests/proofs/` documented (l.7–14) + recorded in candidates (the repo's own arch rule forbids the spelled locator; AK-049 precedent). |
| §8.5 check 2: "Reference manifest and answer key parse; every Unit validates against §5.3.2 incl. tier/authority coupling" | **Partially met** | Key half closed (unit schema + digest clauses, Task 7 diff confirmed via `git diff HEAD`). Manifest half remains existence + top-level-mapping-ness only — see MAJOR-2. |
| reference.lock handling strictness (task brief) | **Met, with one dead field** | `read_reference_lock` (`checks.rs:475-532`): unknown fields are findings; `answer_key_digest` required and parsed as a domain `Digest`; `unit_count` required `Uint`; `algo` must equal the defaults token. Pinned by `doctor/mod.rs:917-931`. `manifest_digest` never verified — see MAJOR-3. |
| §9.4 placement: frozen copy outside every builder-writable path; live key outside workspaces | **Met at substrate level** | P1 asserts the frozen object sits outside two workspace roots and holds exactly the normalized bytes; doctor check 7 (Task 5) covers the answer-key path not inside the workspace (`doctor/mod.rs:826-833`). Full "every builder-writable path" enforcement needs WorkspaceProvider/policy (later tasks). |
| AK-025 (plan done-when l.308: "AK-021–AK-025 have passing evidence") | **Not met, no deferral recorded** | See MAJOR-1. |

## Residuals

### MAJOR-1 — AK-025 is absent from the artifact, with no recorded deferral
- **Ruler:** plan l.308 ("AK-021–AK-025 have passing evidence"); AK-025 (answer-key l.171-176):
  "A run may record a candidate reference revision but cannot adopt one mid-run"; locator
  `tests/antigaming/g2_change_reference` (suite owned by plan Task 24, l.719-733).
- **Artifact:** nothing. No test, no `CandidateRevision` substrate use (the domain type exists
  from Task 3, `gauntlet-domain/src/capability.rs:316`), no mention in
  `p1_reference_integrity.rs`, and — the sharpest edge — no entry in
  `.agents/reports/reference-candidates.md`, whose two 2026-08-18 entries cover AK-021 and
  AK-022/023/024 and then stop. The artifact's own chosen mechanism for exactly this situation
  (record the locator/deferral, don't touch the ruler) was used for four Units and silently
  skipped for the fifth.
- **Fairness:** the "cannot adopt mid-run" half is arguably substrate-proven by the P1 tripwire
  (any mid-run reference move aborts rather than being adopted); the "may record a candidate"
  half has nothing. Full AK-025 evidence is legitimately Task 24's; what is missing is the
  recorded disposition, not the code.
- **Severity:** MAJOR (done-when obligation partially met; the omission is undocumented).

### MAJOR-2 — Doctor check 2's manifest clause is still shallow now that the strict parser exists
- **Ruler:** §8.5 check 2 (spec l.589): "Reference manifest and answer key parse".
- **Artifact:** `checks.rs:206-233` checks only readability and `top_level_mapping_keys`
  (l.420-444, itself labeled "Full YAML is NOT claimed"). `parse_manifest`
  (`manifest.rs:558`), in the same crate since this task, is never consulted by check 2.
  Consequence is an internal inconsistency, not just depth: doctor's own scratch fixture
  manifest `"reference:\n  id: scratch\n  version: 1\n"` (`doctor/mod.rs:364-368`) would FAIL
  `parse_manifest` (missing `purpose`, `desired_state`, `settlement`), i.e. the fixtures only
  pass because the strict parser is not wired in. A manifest doctor green-lights can be one the
  provider's `resolve` refuses outright (`provider.rs:163-171` → `ResolveFailed`), so preflight
  and pipeline disagree on what "parses" means.
- **Severity:** MAJOR (spec obligation partially met; strictness built but not admitted at the
  door it is required at).

### MAJOR-3 — The freeze record's manifest half is decorative; no integrity tripwire covers the manifest
- **Ruler:** §9.4 step 1 (spec l.657) includes the manifest in `resolve`; §5.3.1 (spec l.269)
  makes the whole `Reference` immutable once frozen; the lock's own note declares the record
  "the ground truth digest is checked against"; §8.5 check 2 covers the manifest's parse.
- **Artifact:** `read_reference_lock` lists `manifest_digest` as a known field
  (`checks.rs:453`) but never verifies it; the digest-vs-lock clause recomputes only the
  answer-key digest (`checks.rs:341-377`); `verify_unchanged` re-reads only the answer key
  (`provider.rs:283-298`). A builder (or accident) mutating `.wayfinder/reference.yaml` — the
  ruler's narrative half carrying the invariants, constraints, and settlement criteria — is
  detected by neither doctor nor the I1 tripwire. The choice to pin THE reference digest to the
  answer key is plan- and script-consistent (the plan gate digests the key only), so this is not
  a normalization divergence; it is that the manifest's own recorded digest is accepted,
  parsed-adjacent, and then ignored, with no rationale recorded anywhere in code or the
  candidates file.
- **Severity:** MAJOR (ruler's integrity envelope partially met; recorded-but-unverified freeze
  field; the "freeze record is as frozen as the key" comment at `checks.rs:447-448` overreaches
  what is actually checked).

### MAJOR-4 — The done-when "teeth" holds only inside the test's own harness; both-digests-as-evidence not yet persisted
- **Ruler:** plan l.308: "removing the `verify_unchanged` call from the iteration boundary makes
  P1 fail (teeth)"; §9.4 l.666: mismatch "sets the run terminal state to `aborted` … and
  preserves both digests as evidence".
- **Artifact:** the only iteration-boundary callsites are inside the P1 test itself
  (`p1_reference_integrity.rs:137-142`, loop at l.285-320) and the Task-4 contract-suite stubs;
  grep shows no production caller of `verify_unchanged`. Deleting the helper call from the
  test's loop does fail the test (l.307-312), but that is a harness-internal, self-referential
  tooth — trivially true of any test calling a helper. Likewise both digests live in the typed
  error (`gauntlet-ports/src/reference_provider.rs:37`) but are not persisted as evidence
  anywhere (no event, no evidence object; the P1 test asserts the error variant, not an evidence
  write). The artifact is honest about this (test header l.22-26: "AK-023/AK-024 claims remain
  substrate-level until the controller callsites exist"), and the plan itself sequences the
  controller at Task 12 and settlement at Task 13 — so this is a documented, owner-assigned
  deferral, but the done-when clause is not dischargeable as written at Task 7 and must not be
  read as discharged.
- **Severity:** MAJOR (obligation partially met; credit for the explicit scope note).

### MINOR-1 — `ReferenceVersion` lacks the spec's `evidence_ref` field; linkage is by convention
- §9.4 step 4 (spec l.660) records `ReferenceVersion{id, version, digest, evidence_ref}` on the
  run. The domain type is `{reference_id, version, digest}` (`gauntlet-domain/src/reference.rs:117-121`),
  so the run's recorded version carries no evidence link; the connection exists only because the
  content-addressed ref string happens to equal `"sha256:" + digest.hex`, a convention the P1
  test itself relies on when reconstructing the ref (`p1_reference_integrity.rs:211-212`). No
  information is lost (the ref is a pure function of the digest), but the spec-named linkage is
  conventional rather than typed, and neither the provider header nor the candidates file records
  the deviation. (Domain shape is Task 3's artifact; the workaround is Task 7's.)

### MINOR-2 — "Exactly three strict-admission callsites" overstates by one
- `provider.rs:36-42` names `parse_all_or_fail` in `reference/parse.rs` as one of the three
  strict-admission callsites; grep shows `parse_all_or_fail` has zero production callers (only
  its own unit tests). The real admission sites are doctor check 2 and the P1 proof's
  clean-parse assertion. Documentation-level overstatement of the strictness net.

### MINOR-3 — Deleted / non-UTF-8 answer key classified `Unavailable`, not `ReferenceMutated`
- `provider.rs:279-298` deliberately reports a broken artifact as `Unavailable`
  ("a broken artifact is not proof of mutation"), pinned at `provider.rs:531-568`. The spec is
  silent on the broken-artifact class, the decision is documented, and conflating them would be
  worse — but the flip side is unstated anywhere: a builder who cannot edit the ruler undetected
  could delete or corrupt it and receive a different error class whose boundary behavior
  (abort? retry?) is undefined until Task 12. Worth an explicit controller rule then.

### MINOR-4 — Count clause counts raw `## AK-` lines, so a malformed heading can satisfy a lying count
- `checks.rs:261-265` counts lines starting with `## AK-` while the schema clause parses
  headings strictly, so `## AK-9` (a `BadHeading` to the parser) still increments the count.
  Harmless today (the schema clause fails on the same line) — robustness only.

### MINOR-5 — Locator divergences: correctly recorded, awaiting operator settlement
- The plan's `tests/proofs/p1_reference_integrity` path and AK-021–AK-024 locators are
  unmaterializable under the repository's own cargo-target-path rule; the artifact deviates
  with an in-file rationale (`p1_reference_integrity.rs:7-14`) and dated candidates entries
  proposing corrected locators, without touching the ruler. This is the discipline the lock
  demands; the residual (unsettled candidates) belongs to the ruler's owner, not this task.

## Scope statement (what this review is and is not)

Read in full: the five `crates/gauntlet-app/src/reference/` files;
`crates/gauntlet-ports/src/reference_provider.rs`; `crates/gauntlet-cli/src/main.rs`;
`crates/gauntlet-cli/tests/p1_reference_integrity.rs`;
`.agents/reports/reference-candidates.md`; `scripts/reference-digest.sh`; the ruler texts named
above. Read in part: `doctor/checks.rs` lines 195–533 (the check-2 region and lock reader) plus
`git diff HEAD` over the same file; `doctor/mod.rs` test region (lines 300–520 and 780–980);
`gauntlet-domain` `values.rs`/`unit.rs`/`reference.rs`/`capability.rs` via targeted greps only;
`gauntlet-adapter-evidence-fs` layout docs only.

Executed (read-only): `scripts/reference-digest.sh .wayfinder/answer-key.md` (output matches the
lock), `sha256sum` over the five pinned test inputs, `git status`/`git log`/`git diff` (confirms
`.wayfinder/` and `.gauntlet/reference.lock` unmodified by this task).

NOT done, and therefore NOT claimed: no cargo, no test execution, no build — every "test passes"
statement above is a statement about what the test asserts, not a witnessed run; the contract
suite `ports/contract/reference_provider.rs` was skimmed (function names only), not audited; the
full domain `Unit::new`/`Reference::new` bodies, the fs evidence store internals, the doctor
report JSON, and `.agents/reports/task7/*` builder checkpoints were not examined. Absence of
further issues is claimed only within the files and line ranges listed here.
