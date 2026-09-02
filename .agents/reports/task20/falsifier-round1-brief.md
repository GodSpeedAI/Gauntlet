# Task 20 — executing falsifier round 1 commission (blind)

You are the BLIND EXECUTING FALSIFIER for plan Task 20 of the Gauntlet
runtime at /home/sprime01/projects/gauntlet. Fresh context; you execute real
attacks against the built surface and report what actually happened. You are
NOT the builder's friend and NOT the critic's echo: compose and run your own
attacks.

## Blindness rules (hard)

- Do NOT read: any `*report*.md` or `*progress*.md` under
  `.agents/reports/task20/`, any `*.log` there, `critic-design.md`,
  `design-adjudication.md`. Those are builder/critic history; reading them
  voids the round.
- You MAY read: `AGENTS.md`, `.agents/reports/task20/charter.md`,
  `b1-brief.md`…`b3-brief.md`, `cor-b3a-brief.md`, `cor-b3a-brief`-adjacent
  briefs, `.agents/plans/gauntlet-impl.md` (Task 20),
  `.agents/specs/Core-spec.md`, `.wayfinder/answer-key.md` (read-only!),
  and ALL CODE, TESTS, AND FIXTURES in the tree.
- Your only write targets: `.agents/reports/task20/falsifier-round1.md`
  (write FIRST, incrementally, most severe first) and a TEMPDIR you create
  under /tmp for probe material.

## What you are attacking

The uncommitted task20 tree on HEAD (git status shows it; ignore `.jolli/`
and report files). The commissioned mechanisms, with their load-bearing
locations (find more yourself):

1. `gauntlet run` composition (crates/gauntlet-cli/src/compose.rs,
   run_cmd.rs): fail-closed runner selection (D2: unset/unknown token, mock
   scenario hook); doctor preflight; dry-run (D1: zero writes, no runner
   required); wrap_builder_command production caller with fail-closed
   live-backend refusal (D3); ProducePolicy produced-artifact world.
2. Per-unit workspaces end-to-end (govern/gate.rs, controller.rs,
   gauntlet-adapter-workspace-fs, agent adapters): admission sees the
   per-unit workspace AND execution is anchored there (spec.workspace);
   carry semantics for parallel completions (at most one observation
   binding per (run, iteration); surplus completions carry to the next
   iteration; checkpoint/resume must not double-bind or strand).
3. Settlement single-writer (escalations_cmd.rs, settle service): CLI
   resolve/retry can NEVER settle without earned evidence (no bypass, no
   second write path); tickets stay open on refusal.
4. §20.1 surface honesty: every command real or typed not-wired; unknown
   args exit 2; usage/help truthful; report categories strictly
   record-derived; dry-run zero-write tooth hashes CONTENTS.
5. Roles/skills surface: roles list/read-only verify; skills
   install/verify/list with lock digests; lint intact.
6. New adapters: escalation-fs (durability, write-once resolve, tamper),
   id-entropy, workspace-fs (traversal refusal, per-subject isolation).
7. E2E discrimination: lying builder does NOT settle; happy path settles
   only with pinned instrument evidence.

## Attack ideas (a floor, not a ceiling — bring your own)

Make dry-run or a malformed config write/persist something; select a runner
the composition does not front; feed the mock a scenario that forges
instrument evidence and try to settle; escape the per-unit workspace (path
traversal, `..`-laden unit ids, symlinked workspace effects); force two
completions into one iteration and hunt double-binding, lost carries, or
resume divergence (kill between checkpoint steps if practical); drive
escalations resolve with forged PASS records and try to extract a
settlement row; tamper with the escalation queue mid-flight; feed a forged
roles.lock / skills.lock to list/verify; make the report claim
implemented-and-proven from a settlement row whose verification record is
absent or foreign-run; break the E2E teeth by making the lying builder
look truthful; hunt secrets or absolute paths in any new command's output;
try unknown tokens/args on every command; check GAUNTLET_CLOCK hook
refusals; check the dry-run content-hash tooth actually bites.

## Execution discipline

- Every cargo/just call: `flock .agents/reports/task20/compile.lock -c
  '<command>'`, and run `sccache --stop-server` INSIDE the same flock'd
  command afterward. Long runs: background + poll every 30-60s.
- Probe hygiene: probe files ONLY inside your own tempdir. NEVER create
  anything under `.gauntlet/policies/` or `.wayfinder/`. If you must
  temporarily modify a tracked file, record its sha256 before, restore
  byte-identically after, and prove the restore.
- You may run the existing tests as instruments, but a passing shipped test
  is not your evidence — your evidence is what YOUR attacks produce.
- Never weaken, delete, or "fix" any test, fixture, or source. You are
  read-only except your report and tempdir.
- Budget yourself: prefer breadth across mechanisms over depth in one.

## Report contract

`.agents/reports/task20/final verdict at the end: CONFIRM (0 blocking / 0
major / 0 minor) or NOT_CONFIRM (n blocking / n major / n minor /
n informational)`. Per finding: ID (F-1…), severity (BLOCKING = a
commission invariant is violated by execution; MAJOR = mechanism wrong or
misleading under attack; MINOR = narrow defect; INFORMATIONAL), the exact
reproduction (commands + observed output excerpts), and file:line locus.
Report honestly when an attack failed to land (that is evidence too).

Final message: the verdict line + finding counts, pointing at the report.
No attack details in the message.
