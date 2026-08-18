# Task 7 — D4 (provider.rs) verification report

**Reviewer:** read-only explore agent, dispatched 2026-08-18 by the orchestrator
as the delegation-health probe (delegation confirmed healthy this session —
ESC-002 spawn-death trigger absent). Report delivered inline (read-only mandate);
persisted verbatim by the orchestrator. Line numbers refer to the tree at
verification time (provider.rs mtime 2026-08-17 18:33).

**VERDICT: COMPLETE-WITH-GAPS.** The provider implements the D4 pipeline faithfully
(resolve → normalize → digest → freeze → verify_unchanged; EvidenceKind::File put+pin
freeze; typed Unavailable vs ReferenceMutated split; artifact-source seam; suite run
via real factory; mod wiring). One substantive brief sentence — per-block parse
failures surfaced through `normalize` as `NormalizeFailed` — is not implemented as
written; the deviation is documented and forced by the brief's own contract-suite
requirement, but its named compensating controls are not yet in the tree, and one
error branch lacks test coverage.

## Findings

1. **Major (documented deviation)** — brief lines 138-141: "`normalize(draft, _)`:
   parses manifest + answer key (per-block failures surface here as `NormalizeFailed`
   carrying the typed cause…)". Implementation: parsing happens in `resolve`
   (`provider.rs:187-194`) and `normalize` is only `Reference::new`
   (`provider.rs:198-207`). `parsed.failures` is computed then never read by the
   provider — only `parsed.units` reaches `build_definition` (`provider.rs:194`).
   `NormalizeFailed.cause` is a `ReferenceError` (`ports/reference_provider.rs:29`),
   so it can never carry per-block causes. The module header (`provider.rs:22-43`)
   records the rationale, and it is verifiably real: the frozen Task-4 suite feeds
   the plain phrase `"the standard text"` (`ports/contract/reference_provider.rs:103,126`)
   and expects `normalize` to succeed (`:66-67`) — a strict normalize would fail the
   suite by construction, so the brief is self-contradictory and the builder chose
   suite compatibility. Residual exposure: on provider paths alone, a key with one
   malformed block silently yields that Unit missing (the digest still covers the
   raw bytes, so the I1 tripwire is unaffected — bounded to what was read).
2. **Minor (doc ahead of tree)** — `provider.rs:36-38` cites three strict-admission
   callsites for the finding-1 deviation; only one exists: `parse_all_or_fail`
   (`parse.rs:269`). Doctor check-2 still reports `UNIT_SCHEMA_LATER`/`DIGEST_LATER`
   unavailable (`doctor/checks.rs:318-336`), and `gauntlet-cli/tests/` contains only
   `export_stack.rs`. Those are D6/D7 deliverables still pending; the header's claim
   is unsupported in the current tree.
3. **Minor (test coverage)** — brief requires missing/broken artifact → Unavailable,
   NOT ReferenceMutated, specifically for `verify_unchanged`. Code correct
   (`provider.rs:266-281`) but `a_missing_artifact_is_unavailable_not_reference_mutated`
   (`provider.rs:528-544`) only asserts `resolve` → `ResolveFailed`; the
   `verify_unchanged` Unavailable arms have no exercising test (the mutation test
   `provider.rs:478-484` covers only `ReferenceMutated`).
4. **Minor (observation, not a brief violation)** — `freeze` re-reads the artifact
   from source (`provider.rs:231-241`) and returns `reference.freeze()` (`:253`)
   without cross-checking the two: a mutation landing between `normalize` and
   `freeze` would store evidence whose digest differs from the returned
   `version.digest()`, silently. The brief does not require a guard, and the next
   `verify_unchanged` boundary would fire; recorded as an edge, not a defect.
5. **Minor (trivial)** — test temp-file naming uses pid + nanos (`provider.rs:454-462`)
   rather than the brief's Scratch idiom of pid + atomic counter. Tests-only;
   uniqueness holds.

## Specific confirmations

- **(a) Shared suite with production-path factory: CONFIRMED.** `provider.rs:443-450`
  runs `reference_provider_suite(&SuiteFactory)` and asserts findings empty, matching
  the `suite_runs.rs` idiom (`ports/contract/suite_runs.rs:26-36,251-256`).
  `SuiteFactory` (`provider.rs:427-441`) builds the real production type
  `FileReferenceProvider::from_sources` over the suite's shared artifact — not a
  stub. Test lives in-module under `reference::` as the gate filter requires.
- **(b) Freeze = put + pin, exact kind: CONFIRMED.** `provider.rs:237-252`:
  `normalize_bytes` → `put(&normalized, EvidenceKind::File, deadline)` →
  `pin(run_id, &evidence_ref, deadline)` → `reference.freeze()`. Port signatures
  match (`ports/evidence_store.rs:55-60,71-76`). Idempotency and normalized-bytes
  storage asserted (`provider.rs:494-521`).
- **(c) Error typing: CONFIRMED (code).** Read failure → `Unavailable`
  (`provider.rs:266-273`); digest failure (invalid UTF-8) → `Unavailable`
  (`:274-281`); mismatch → `ReferenceMutated { expected, actual }` carrying both
  digests (`:282-289`), exercised at `:478-484`. Coverage gap per finding 3.
- **(d) No unwrap/expect/panic on untrusted input: CONFIRMED within files read.**
  Production code `provider.rs:45-291` has zero panic-class macros (hits are all
  `#[cfg(test)]` or doc comments). `normalize.rs:111-114` `unreachable!` guards
  `Digest::new` over the const `DIGEST_ALGO` + 64-hex — an established invariant
  independent of input, documented; same pattern at `manifest.rs:1010-1012`.
- **(e) TODO/dead/unfinished: none found in provider.rs itself.**

## Correct D1/D3 integration (checked)

`resolve` digests via `digest_of` = UTF-8 validate → normalize → SHA-256
(`provider.rs:189`, `normalize.rs:118-120,95-97`); `build_definition(&manifest,
parsed.units, digest)` sets the definition digest to the answer-key digest
(`manifest.rs:986-994`). Mod wiring: `gauntlet-app/src/lib.rs:16`, `reference/mod.rs:18`.

## Bounded claims

Checked: `provider.rs` and `mod.rs` in full; `normalize.rs` in full;
`ports/reference_provider.rs`, `ports/evidence_store.rs` (trait region),
`ports/contract/{mod,reference_provider,suite_runs}.rs` in full; `manifest.rs` only
the `build_definition` region; `doctor/checks.rs:310-339`; directory listings;
`builder-checkpoints.md`. Did not check: compilation or any gate (prohibited; the
checkpoint records an orchestrator `cargo check -p gauntlet-app` PASS),
`parse.rs`/`manifest.rs` internals beyond signatures, D5 CLI subcommand, D6/D7/D8
status beyond the absence notes, and the fs evidence adapter. Absence of defects is
claimed only within the files and line ranges cited.
