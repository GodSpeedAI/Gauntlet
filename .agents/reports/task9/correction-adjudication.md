# Task 9 — correction adjudication (2026-08-20)

Round-5 threat-model rule applied: 0 criticals across both critics. Majors
triaged into fix-now (load-bearing and cheap) vs. ledgered deferrals (later
task owns the mechanism). Minors triaged individually. Every fix item gets a
fail-first discriminating test. The deferral ledger discipline applies: a
deferred finding upgrades to blocking the moment a task walks into its
trigger.

## Fix in this round (C1–C10)

- **C1 ← B-6a + B-6b (MAJOR×2): workspace containment.** All five adapters:
  refuse refs containing `..` (or `.`) components; canonicalize the resolved
  workspace and require it to stay under the canonical root; a symlinked
  workspace pointing outside (or any containment failure) →
  `ObserverError::Unavailable` with a containment-naming detail. Tests:
  `../sibling` ref refused; symlink-to-outside workspace refused.
- **C2 ← B-2a: mismatch also populates `unavailable_evidence`.** The
  provenance cross-check's failure site pushes BOTH arrays (§21.3 alignment
  the module doc already claims). Test: garbage-evidence facet → both arrays
  name the kind.
- **C3 ← B-3a (MAJOR): cross-check agreement.** Compare instrument
  name+version, command, exit status, and timing window between the
  `FacetRecord` and the strict-decoded `FacetEvidenceDoc`. Disagreement on
  any → `provenance_mismatch` (→ both arrays per C2). Exit-status `None` on
  the facet while evidence carries one → mismatch (closes the None-bypass).
  Window: compare only fields the facet actually carries (Some). Test:
  cross-wired evidence (facet A citing facet B's evidence, equal exits)
  → mismatch recorded.
- **C4 ← B-3b(partial) + A-m4 + A-m5: plan validation.** `assemble` refuses
  (typed `AssemblyError`): empty observer list; `environment_fingerprint`
  not shaped `sha256:` + 64 lowercase hex; empty `project_revision`.
  Tests for each refusal. Document the snapshot semantics honestly (below).
- **C5 ← B-6c: manifest error lines respect the entry cap** (unreadable-dir
  and iteration-error lines count against `MAX_MANIFEST_ENTRIES`). Test:
  fixture with unreadable subdirs keeps total manifest lines ≤ cap+1
  (truncation line included).
- **C6 ← B-6d: manifest determinism under non-UTF-8 names.** Escape
  non-UTF-8 byte sequences in manifest path spellings (documented escape
  rule, e.g. `\xNN`) and sort by the escaped spelling (total order). Tests:
  two distinct lossy-colliding names → two distinct lines, stable across
  creation orders.
- **C7 ← B-5b': pin the maskable intersection.** New arch-test pinning
  `adapter_segments(denylist) ∩ ANNEX_B_WIRE_KEYS == ["command"]` computed
  from the REAL member list — a future adapter whose name's last segment
  collides with a declared wire key fails the pin and forces a deliberate
  recorded decision instead of silently widening the mask.
- **C8 ← B-7b + A-m1: spawn-failure taxonomy unified.** build/test/command
  map spawn failure → `Unavailable` (matching git; the port's own doc says
  "could not run at all"). Update the g6 pin that asserted
  `InstrumentFailed` for the missing program. The shared contract suite
  accepts either variant, so no suite edit.
- **C9 ← B-5a: residual visibility.** Extend the denylist residual test
  with the critic's split-argument selection fixture, ASSERTING it masks
  today (pinned visibility of the accepted residual, not silent
  acceptance). Do NOT attempt a shape hack — the Task-4 adjudication
  accepted this class for one module; the ledger entry (D4) carries the
  re-litigation trigger for four.
- **C10 ← B-1a/1b (adjudicated not-defects) + doc note.** The command field
  is the instrument-invocation record AK-028 itself requires; failure
  details are diagnostic context. Neither is a verdict carrier — the exit
  status is. Add a short doc note in `input.rs` saying exactly that, so a
  future reader does not mistake `command`/`detail` text for
  builder-claim influence.

## Deferred to the ledger (triggers named)

- **D1 ← B-7a + A-M2 (MAJOR):** observer process timeouts and subprocess
  output-size caps. Extends the standing Task-8 "deadlines noted, not
  consulted" deferral to `observe()` explicitly. Trigger: **Task 12**
  (controller deadline composition — §18.4 hang disposal needs the
  controller anyway).
- **D2 ← B-3b core (MAJOR):** child env inheritance vs fingerprint
  (PATH resolution outside the fingerprint; from_parts declares rather than
  captures). Trigger: **Tasks 15–16** (tool binding — deciding which env
  instruments may see is configuration, not observer, business).
- **D3 ← B-3c + A-M3 (MAJOR):** bundle digest recompute at the consumption
  boundary; ComparisonEnvelope carries the domain bundle (no facets) and
  will need widening for comparison to operate on recorded reality.
  Trigger: **Task 10** (comparator + envelope builder — its own plan
  section).
- **D4 ← B-5a (MINOR–MAJOR):** masked selection-shaped spellings in the
  four codec modules (split fn-arg, purpose-blind array element,
  receiver-invisible `.get`). Trigger: **next arch-suite touch** —
  re-litigate alongside E4/F1/F3 per the standing rule.
- **D5 ← B-8a (MINOR):** domain view drops facets/identity/revision.
  Trigger: **Task 10 / ports revision** (comparator needs facet-level
  reality).
- **D6 ← A-m7 (MINOR):** port permits incomplete facets (`Option` fields).
  Trigger: **ports revision**.
- **D7 ← A-m8:** evidence timestamps digest-covered only transitively via
  content-addressed refs — inherent, documented; no trigger needed.
- **D8 ← A-m9:** FacetKind vocabulary narrower than §12.2's wider class
  list (pre-existing port vocabulary). Trigger: **ports revision**.
- **D9 ← A-m10:** command adapter's kind is caller-bound. Trigger:
  **production wiring (Task 12/15)** must bind kind↔command correctly.
- **D10 ← A-M1:** AK-028 locator unmaterializable — ALREADY dispositioned
  by the dated reference-candidates entry (the blind critic confirmed the
  gap without seeing the ledger; the entry cites the real per-adapter
  provenance tests). No code action.
- **A-m2, A-m3, A-m5's request-shape note, B-7's git-exception note
  (A-m2-convergent), B-6's TOCTOU/shrink nits:** documented; recorded in
  this adjudication; no independent action beyond C-items above.
