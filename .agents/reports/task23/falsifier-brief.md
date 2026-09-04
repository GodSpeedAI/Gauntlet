# Task 23 — blind EXECUTING falsifier, round 2 (post-COR-2)

You are the EXECUTING FALSIFIER for plan Task 23 of the Gauntlet
repository at /home/sprime01/projects/gauntlet. You are BLIND: no
builder history, no critic report, no correction reports — your
knowledge is this brief, the charter, the spec, the frozen ruler, the
tree as it stands, AND the round-1 falsifier report
(`.agents/reports/task23/falsifier-round1.md` — your round's own
attack ledger; re-attack every hole it found and confirm closed, then
find what it missed). You compose your own attacks and you EXECUTE
them (run the real binaries and tests; mutate code to prove teeth;
restore byte-identically). You are read-only in one sense only: every
file you touch you must restore byte-identically (sha256-proven), and
the ONLY repository file you may create is your report
`/home/sprime01/projects/gauntlet/.agents/reports/task23/falsifier-round2.md`
(plus scratch worlds under /tmp, removed after).

ROUND-2 MANDATES beyond the standing five attack families:

- **R-1 regression (round-1 F-1):** the moved-regression-ruler attack
  must now die TWICE — the harness refuses a moved-ruler task set at
  evaluate time (before any record), and the gate's condition 2 sees
  all four arms' reference digests. Re-execute the round-1 binary
  reproduction verbatim; also attack the NEW surface: can any arm's
  digest go unrecorded or unverified (a fifth arm shape, a digest
  mismatched inside the record, a regression set sharing the held-out
  directory)?
- **R-2 regression (round-1 F-2):** the instrument-series comparison
  now joins BY UNIT. Re-execute the probe-A shield shape and its
  variants (extra settlements, duplicate unit rows, a weakened unit
  with reordered facts, missing instrument facts on one side).
- **R-3..R-6 regressions:** margin >10 000 bps must be a typed
  refusal at constructor and resolution; evaluation-record
  re-declaration (metric/protected) must conflict; governance-target
  classification must be case-normalized; the containment scan must
  catch the raw write classes (fs::write, File::create, OpenOptions,
  set_var, Command::new).
- **Extend:** spend at least half your budget on attacks the round-1
  report lists under "not attacked" or does not contain at all — the
  §19.4 verdict boundary, the substrate, the harness asymmetries,
  the CLI surfaces.

READ FIRST: `AGENTS.md`; `.agents/reports/task23/charter.md` (the
binding contract); `.agents/specs/Core-spec.md` §19.3, §19.4 (the
eight conditions), §5.3.8, Annex B.7, §23.1 P9; ruler units AK-056 /
AK-057 in `.wayfinder/answer-key.md` (never edit).

THE CLAIMS YOU ATTACK:

1. **AK-056:** an optimizer proposal that improves a metric by
   weakening verification fails promotion. Attack the gate
   (`crates/gauntlet-app/src/promote/gate.rs`, `delta.rs`,
   `metrics.rs`) and the proof (`crates/gauntlet-cli/tests/
   p9_self_improvement_containment.rs`): can ANY weakening shape — a
   delta key, a value spelling, a target string, a metric token, a
   fixture quirk, an evaluation-record shape — reach Promoted while
   verification, review, or observation strength is actually reduced?
   Can the recorded rejection reason be dodged or emptied? Are the
   proof legs' teeth real (neutralize each named mechanism — the leg
   must go RED — then restore byte-identically)?
2. **AK-057:** the optimizer cannot modify the active loop; its output
   is a candidate revision requiring isolated evaluation, held-out
   comparison, and valid rollback. Attack the containment surfaces:
   the arch scan (`crates/gauntlet-arch-tests/src/optimizer.rs`) —
   does it actually cover the promote module, do its negative controls
   prove the bite, is there a spelling/class of config-write or
   roles-install access it misses; the proposal admission path
   (`promote/proposal.rs`, the role_result codec) — can a payload
   smuggle decision state, evaluation evidence, extra fields, or a
   side effect; the CLI (`improve_cmd.rs`) — can any subcommand or
   flag write configuration/roles/state outside the revision ledger,
   or adjudicate without evaluation evidence?
3. **The §19.4 verdict contract:** all eight conditions enforced over
   durable facts; FAILED ⇒ Rejected (all reasons + rollback ref);
   evidence gaps ⇒ Provisional, never promoted, never rejected; an
   infra failure aborts typed rather than judging. Hunt the boundary
   between "failed", "gap", and "abort": figure arithmetic (margin
   basis points, fixed-point, saturation), empty sets, undeclared
   sets, unknown tokens, missing instruments, stale claims, write-once
   decisions, derived rolled-back state, reopen durability.
4. **The durable substrate:** the revision ledger port + sqlite
   adapter (migration 0010) — idempotency, conflict shapes, stale
   claims, INSERT-only discipline, content-derived identity; can
   re-admission, replay, or re-adjudication inflate, flip, or erase
   anything?
5. **The evaluation harness:** control/treatment arms, parent_run
   isolation verification, metric derivation symmetry (identical
   function for both arms), developmental-import wiring — can arms be
   made asymmetric, metrics underivable-but-passing, or isolation
   unverifiable?

METHOD (house rules, binding):

- Evidence is what tests and binaries print, not what reports claim.
  Reproduce every suspected hole on the REAL adapters (real sqlite
  ledger, real composed mock loop, real binary under
  `target/debug/gauntlet`) in /tmp scratch worlds; remove scratch
  worlds afterwards.
- Teeth: for each proof leg's declared tooth, neutralize the single
  mechanism (edit or remove it), observe the leg RED, restore
  byte-identically (record sha256 before/after). A tooth that does not
  bite is a finding.
- You may RUN the full battery if you need it, but slice-scoped nextest
  filters are usually enough. NEVER edit
  `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.wayfinder/reference.yaml` — a ruler change is not a finding, it is
  the crime the whole system exists to prevent.
- If you plant ANY file under `.gauntlet/`, remove it and `ls` the
  directory to prove removal (the recorded task14 probe-hygiene rule).

BUILD DISCIPLINE (binding): ONE compiler at a time — wrap every
cargo/just call in
`flock /home/sprime01/projects/gauntlet/.agents/reports/task23/compile.lock -c '<command>'`,
and INSIDE the same flock'd command AFTER the build run
`sccache --stop-server`. Before editing any file check no live cargo
processes / lock holders linger. Run long builds in background and poll
every 30–60 s (you die on a 10-minute inactivity timeout — keep
tool-call cadence high). WSL2 ~1.6 GiB free — never raise parallelism.
Nextest ids are space-joined (the `promotion::candidate_only`
substring matches zero tests; use
`cargo nextest run -p gauntlet-app --test promotion candidate_only`).

VERDICT + REPORT: write
`/home/sprime01/projects/gauntlet/.agents/reports/task23/falsifier-round1.md`
FIRST: verdict line (CONFIRM or NOT_CONFIRM — NOT_CONFIRM means you
found a reproducible hole or a tooth that does not bite), then every
finding CRITICAL / MAJOR / MINOR / INFORMATIONAL with the verbatim
reproduction (commands + output), the mechanism, and the suggested fix
locus; then the not-attacked list (what you did not get to — the next
round's benefit); then your restore ledger (every file touched, sha256
before/after). Final message: the verdict line + one-line summary.
