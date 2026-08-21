# Task 10 correction round 1

**Date:** 2026-08-20  
**Scope:** correction of valid critic/audit residuals only. Reconciled against frozen Core spec §§9.5, 10.3, 13.2–13.5; AK-026, AK-027, AK-032, AK-033, AK-034; and Task-10 D1–D5 brief. Frozen files, `.gauntlet/reference.lock`, comparison-contract YAML, reference candidates, and CURRENT_STATUS were not modified.

## Residual dispositions

| Residual | Disposition and evidence |
|---|---|
| C1 / static P2 value channel | **Partially corrected; bounded blocker remains.** `assert_blind` now rejects deny tokens anywhere, including an allowlisted-string value; fail-first/green `a_deny_listed_name_hidden_in_an_allowlisted_value_fails_blindness`. Claims remain structurally absent. Proving arbitrary `reference_view` prose derives from bytes named by `reference_digest` needs a frozen-reference resolver/provenance type, absent from this port and cannot be fabricated from two caller strings. |
| Fresh context/workspace containment | **Corrected.** Model adapter no longer accepts caller workspace/permissions; it derives a per-invocation critic workspace and fixed envelope-only permission reference and refuses runner isolation below process. |
| M1/F3/F7 nested binding | **Corrected.** Model adapter rejects mismatch of run, iteration, reference, observation, envelope digest, contract version, requested identity dimension, and residual dimension. Cross-wired nested record added fail-first then green. |
| M2/F2 timeout | **Corrected within port capability.** `collect` receives supplied deadline after completion; all timeout/no-terminal paths call `cancel` with the same deadline before returning typed `TimedOut`. Poll has no deadline parameter by its fixed port contract; bounded nonblocking polling remains the only available wait mechanism. |
| M3/F1 adversarial absence | **Corrected.** High assurance returns typed `SourceFailed{adversarial}` rather than degrading. Budget-exhausted adversarial pass remains explicit unavailable evidence. |
| M4/F6 incomplete review | **Corrected.** Any dispatched-but-truncated review rewrites consequential record verdict to `Inconclusive` and includes canonical gap kinds in its rationale; detailed gaps remain in `ReviewOutcome`. Fully truncated review still creates no fabricated record. |
| M5 | **Corrected.** Prior `ResidualDoc.disputed` becomes inherited merge evidence; it cannot be erased by re-aggregation. |
| F4 B.4 loss | **Corrected for concrete B.4-producing adapters.** The Comparator port now offers an optional full-document method. Review uses it directly for diff/model adapters; evidence refs, gates, identity, unit refs, diagnosis, and corrections are not reboxed. Domain-only compatibility comparators retain the documented lossy fallback. |
| M7 canonicality | **Corrected.** Envelope serialization sorts facets, both gap arrays, constraints, open residuals, contract dimensions, and rubric declarations. Green ports test: `canonical_envelope_ignores_semantically_unordered_input_order`. |
| Static contract declarations | **Partially corrected.** Parsed `comparator_receives` and `residual_definition` are retained in the typed contract, not reduced to counts. Their natural-language semantics cannot be mechanically executed without a new structured frozen contract schema. |
| Static observed locator | **Corrected/reconciled.** Annex B.4 field `observed` is explicitly documented and used as the §13.5 `observed_locator`; no schema alteration was invented. |
| F5 parallel fanout | **Blocked precisely, not claimed compliant.** Core §13.3 requires fanout “in parallel.” Current `Comparator::compare(&self, ..., Deadline) -> Result` is synchronous and contains neither future/task handle nor executor; `ComparatorSource` returns one synchronous comparator at a time. Real parallel dispatch would require a port revision (async/futures or admission/collection split) and scheduler budget/deadline policy. This correction keeps serial behavior and makes no compliance claim. |
| M6 CLI reachability | **Out of this slice, already explicitly ledgered by Task-10 brief as no CLI surface beyond proof and by CLI Task-12/20 comments.** No new CLI command added. This is not represented as production reachability. |

## Explicit adjudications

- The spec critic’s request to remove `iteration_no` is rejected: D1 expressly requires it in the sanctioned envelope widening, and §13.2/D1 require the nested record binding it. It is a run identity/coupling field here, not silently changed to satisfy contradictory criticism.
- The request to treat a clean deterministic probe as conclusive pass is rejected: D4 explicitly says a clean deterministic probe settles nothing; the adaptive test retains model dispatch for such dimensions.

## Files changed in this correction

- `crates/adapters/gauntlet-adapter-compare-model/src/lib.rs`
- `crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs`
- `crates/gauntlet-app/src/compare/{aggregate,contract,envelope,review}.rs`
- `crates/gauntlet-ports/src/comparator.rs`
- `.agents/reports/task10/builder-checkpoints.md`
- `.agents/reports/task10/correction-round1.md`

## Fail-first → green proof

Each named test first failed against the pre-correction behavior, then passed after the smallest relevant change: nested model cross-wire, inherited dispute, absent adversarial source, truncated passing record, and deny token within an allowlisted value. Canonical input-order behavior is additionally covered by the ports test.

## Gates

- `cargo nextest run -p gauntlet-ports` — PASS (71)
- `cargo nextest run -p gauntlet-app compare::` — PASS (22)
- `cargo nextest run -p gauntlet-adapter-compare-model` — PASS (4)
- `cargo nextest run -p gauntlet-adapter-compare-diff` — PASS (5)
- `cargo nextest run -p gauntlet-cli --test p2_context_isolation` — PASS (2)
- `cargo fmt --all` — PASS
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- `just ruler` — PASS
- `just boundaries` — PASS (194)

Not rerun in this correction: workspace-wide nextest and `just deny`; no relevant red result is known from this bounded run. `target` was 1.8G.

## Remaining fog/blockers

1. **Frozen reference/contract binding:** the existing lock only names answer-key/manifest digests. Adding a comparison-contract or rendered-reference digest to it is a prohibited frozen-reference change. A candidate revision must define the source-of-truth and binding before implementation.
2. **Reference-view provenance:** an opaque pair of caller-supplied digest/text cannot establish that the text projects the frozen artifact. A `ReferenceProvider`-backed opaque rendered-view construction is required.
3. **Actual parallel fanout:** requires the port revision above; serial review is not claimed compliant.
4. **Textual contract semantics:** storing declarations stops discard, but executable residual semantics require a frozen structured vocabulary/candidate revision.
