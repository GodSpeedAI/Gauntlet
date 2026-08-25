# Task 12 — B4 FINISHER brief (recover mid-Item-1 death)

You are finishing a partially-landed B4. Read `AGENTS.md`,
`.agents/reports/task12/builder-brief-b4.md` (authoritative slice brief),
and `.agents/reports/task12/builder-b4-checkpoints.md` (predecessor recon —
three load-bearing findings already recorded there; honor them).

## Reconciled state (orchestrator-verified)

- Predecessor recon entry complete; NO implementation entries.
- `crates/gauntlet-cli/tests/p4_corrective_loop.rs` EXISTS — mid-Item-1
  death; completeness/compilation unknown. p6, teeth, gates: unstarted.

## Procedure

1. Diagnose p4's partial state (`just check-package gauntlet-cli`; read the
   file; note in checkpoints what you kept/repaired/removed and why) —
   write the diagnosis entry BEFORE repair work.
2. Complete Item 1 (p4) per the brief + predecessor findings: proof-local
   deterministic rung decoding `facet_evidence@v1` deciding by recorded
   exit status (documented deviation + integration finding in the report);
   proof-local scripted AgentRunner for fail-then-fix (documented why the
   mock cannot serve); Adaptive review mode so the real DiffComparator is
   on the path.
3. Item 2 (p6): real mock + REQUIRED lying-builder scenario; the
   composition-root MeterFactory supplement per predecessor finding 3 (REAL
   MeteredBudget for everything the port expresses + domain-Budget rounds
   shadow booking each admitted reservation's round at reserve — the
   "iteration boundaries" booking the B1 module doc delegates to the
   composition); document exhaustion-fires/diagnosis-pinned-in-crate and
   the diff-comparator attribution gap as recorded residuals.
4. Item 3 (teeth T1–T6 per the brief) — run each, record in
   `.agents/reports/task12/teeth.md`, revert byte-identical.
5. Item 4 (gates) incl. the exact plan gate. Report real locator spellings
   for the orchestrator's reference-candidates entries.
6. Checkpoint entry after EVERY step; result contract
   `.agents/reports/task12/builder-b4.md` covering the whole slice.

Hard rules: same as the B4 brief (no frozen/operator file edits, no
commits, single-compiler lock, package-scoped first, fail-first,
named-const literal idiom, no weakened gates — red gets reported loudly).
