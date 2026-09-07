# B2 progress — §25 docs set + docs_present (task27)

Charter: `.agents/reports/task27/charter.md` slice B2. Result contract:
this file's sibling `b2-report.md`.

## Checkpoints (written as work proceeds)

- 2026-09-07 (start): sources read in charter order — AGENTS.md, charter,
  Core-spec §8/§17/§18/§19/§20/§21/§22/§23/§24/§25/§26 + Annexes A/C/F,
  plan §Task 27, CURRENT_STATUS.yml (settled summaries + observations +
  unresolved_summary), reference-candidates entries 6-7 (task26 boundary
  batch) and the locator-divergence entries, task14
  structural-review-unk001.md (R1c seed wording for the UNK-001
  disposition), task26 closure.md, GAUNTLET.md, cli_help.txt snapshot,
  justfile. Evidence targets for every planned unit claim verified in-tree
  (p1-p12/g6/g7/g10 targets under crates/gauntlet-cli/tests/, app config
  unit tests, arch-tests targets, baselines.json, mutation.json).
- Status-word policy decided: every unit-level claim in the eight docs uses
  `implemented_but_provisional` — the working ledger's own convention
  ("55 units provisional through Task 26", CURRENT_STATUS observations) —
  per the charter's honesty rule (weaker status when unsure; the sweep's
  conformance.json is the only per-unit truth and does not exist yet).
  AK-067 gets no status word adjacent (handoff.md is wave-2 B3's file at
  authoring time); AK-068 claims implemented_but_provisional (this slice's
  own test, RED-first proven). Writing the eight docs now.
- Eight docs written (docs/{ARCHITECTURE,CONFORMANCE,CONFIGURATION,SECURITY,
  FAILURE-RECOVERY,OPERATOR-GUIDE,SELF-IMPROVEMENT,HANDOFF}.md — all NEW
  uppercase; the lowercase operator tree untouched). Binding CONFORMANCE.md
  content (charter decision 8) covered by sections: Trust posture / Sandbox
  backends and tier mapping / Profile adjudication / Advisory-only
  enforcement / UNK-001 terminal disposition (stable phrase:
  "cross_cutting containment — a declared substitution").
- Test infrastructure landed: tests/contract/docs_present.rs (address
  file, 5 tests) + crates/gauntlet-arch-tests/src/docs.rs (checking logic;
  `pub mod docs;` + locator-table row + the [[test]]-enumeration test's
  expected set extended to five entries — the existing
  evidence_locators test pins the manifest's [[test]] blocks, so the new
  entry had to be added there) + the [[test]] docs_present entry in
  crates/gauntlet-arch-tests/Cargo.toml. check-package green after one
  closure-deref fix (.copied()).
- First full package run: 4 failures. ONE mine (evidence_locators
  expected-set, fixed above). THREE over crates/gauntlet-app/src/
  conformance/commands.rs — B1's UNTRACKED in-progress file (adapter-token
  + literal violations); not touched by B2, recorded as wave-1
  interference. My scoped runs green: docs_present 5/5, docs:: unit
  tests 3/3, evidence_locators 1/1.
- Teeth proven RED-first (sha256 before == after, byte-identical
  restores): T1 delete docs/SECURITY.md ⇒ docs_set_present RED exit 100
  ("SECURITY.md unreadable") ⇒ restore b84d440a… ⇒ GREEN; T2 strip the
  status-section heading from docs/CONFIGURATION.md ⇒
  status_section_in_every_doc RED ⇒ restore b64f7a8c… ⇒ GREEN; T3 bogus
  status word ("- proven — AK-060:") in docs/OPERATOR-GUIDE.md ⇒
  status_claims_use_canonical_vocabulary RED ("non-canonical status
  token (proven)") ⇒ restore dd31b4ec… ⇒ GREEN (full target 5/5).
- Post-format re-verification: clippy -D warnings exit 0; docs_present
  5/5; docs:: 3/3. `just ruler` PASS. Full package battery 245 tests:
  230 passed / 4 failed — every failing violation located in B1's
  untracked crates/gauntlet-app/src/conformance/{commands,manifest}.rs
  (wave-1 interference, disclosed in the report; zero violations in B2
  scope).
- DONE — completion contract written to b2-report.md (eight files +
  summaries, the 33-claim status inventory with backing records, teeth
  sha256 evidence, deviations, battery results, B3 handoff notes).
