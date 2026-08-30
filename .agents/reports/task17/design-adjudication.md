# Task 17 — design-as-built adjudication (round: critic-design.md)

Critic verdict NOT_CONFIRM (8 major / 6 minor, 0 blocking; positive core
confirmed: intersection invariant faithful, §10.1 field set exact, A1
reuse real, secrets discipline verified, idempotence deterministic,
re-forge teeth genuine, denylist entry precedent-consistent, role bodies
governance-free, advisory fallback honest). All four checkable factual
claims independently verified by the orchestrator before routing
(justfile placeholder, resolved `roles_dir`, §8.4 tools list, no
consumer outside roles/). Adjudication:

| Id | Severity | Disposition |
|---|---|---|
| RES-D01 empty forged lock passes verify | major | **ACCEPT → COR-D01** |
| RES-D02 roles.dir config ignored by CLI | major | **ACCEPT → COR-D02** |
| RES-D03 write_paths interior over-grant | major | **ACCEPT → COR-D03** |
| RES-D04 addition drift invisible | major | **ACCEPT → COR-D04** |
| RES-D05 write_atomic lacks fsync | minor | **ACCEPT → COR-D05** (file sync; parent-dir fsync recorded) |
| RES-D06 permission VALUE vocabularies open | major | **ACCEPT → COR-D06** |
| RES-D07 deny-set has no consumer | major | **ACCEPT (partial) → COR-D07** — consume-side seam in-task; composed wiring stays task20 |
| RES-D08 RuntimeProvided no per-role backstop | minor | **DEFER, trigger task20 composition** (R17-D08) |
| RES-D09 citation nits (§12.2→§17.1; §10.7 hatch) | minor | **ACCEPT → COR-D09** |
| RES-D10 vocabulary lacks map/units | minor | **DEFER, trigger task20 composition** (R17-D10) |
| RES-D11 pin reason overstates guard | minor | **ACCEPT → COR-D11** (reason text; guard needle unchanged) |
| RES-D12 roles verify ignores unknown args | minor | **ACCEPT → COR-D12** |
| RES-D13 contained_join + SourceMissing untested | major | **ACCEPT → COR-D13** |
| RES-D14 stale just roles-verify placeholder | major | **ACCEPT → COR-D14** |

RES-D07 adjudication note: the frozen prereg's D6 says deny-set
enforcement is "IN-TASK at the TYPE level … a compile-shaped
obligation" while composed dispatch stays task20. The critic's reading
(the envelope builder must ACCEPT the deny-set in-tree) is the better
one and satisfies the plan's step-2 words ("feeds the envelope builder")
without touching call sites: an additive, typed consume-side check in
`compare/envelope.rs` that refuses an input set carrying a category the
invocation's role denies. The builder's narrower reading (B1-D7,
documented API consumed at task20) is superseded.

Residuals opened: R17-D08 (per-role §10.2 admission rules for
RuntimeProvided categories — encode before/with the task20 composer),
R17-D10 (vocabulary needs `map`/`units` spellings for the builder
reading set — decide at composition), R17-note-1 (write_atomic
parent-dir fsync — crash-durability of the lock's directory entry).
