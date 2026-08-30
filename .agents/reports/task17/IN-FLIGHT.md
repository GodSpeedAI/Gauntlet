# IN-FLIGHT — Task 17 (Role artifacts + harness installer)

- **Started:** 2026-08-30T03:3xZ (orchestrator session; task16 settled at
  072ce00/f8d4f3f immediately prior)
- **Phase:** design critic (static-only, blind) + builder B1 dispatched
  CONCURRENTLY (critic runs no cargo — the builder holds the compile
  lock); critic findings routed mid-flight per the task15/16 pattern.
- **Prereg:** frozen `b3616216df3763407b58c624fb2f46b45f3cbe70771ed6cb7b7ede698bfd571a`
  (`just prereg-check 17` PASS at freeze; written before any implementation).
- **Baseline:** HEAD f8d4f3f at dispatch; `just ruler` PASS at dispatch.
- **Slice plan (B1):**
  - `.agents/roles/{orchestrator,builder,critic,verifier,observer,
    optimizer}.md` — six role artifacts with spec §10.1 front matter
    (role, context.allow, context.deny, permissions); bodies describe the
    contract and cite the spec, NO governance logic in prose (skill-lint
    class); critic per-dimension bodies (functional, architecture,
    security, evidence-completeness) for fan-out.
  - Loader in `gauntlet-app` (`roles::` module): typed RoleDefinition via
    validating constructor; REUSES task5's front-matter + $VAR machinery
    (config::gauntlet_md / env_layer) — no forked grammar; context.deny
    becomes typed data the envelope builder consumes (intersection
    semantics: a role file can NARROW, never GRANT beyond what the
    builder permits).
  - `gauntlet roles install --harness <id>` — semantic harness registry
    (prime-agent: writable role surface; generic-cli: NO surface ⇒
    per-invocation prompt injection with `enforcement: advisory`
    recorded); installed state under the repo-configured state root
    (never operator dirs, never outside workspace); writes
    `.gauntlet/roles.lock` (content digests + identifiers).
  - `gauntlet roles verify` — content-digest drift detection; drift ⇒
    typed configuration failure, non-zero exit.
  - Teeth (RED before GREEN): install idempotence (byte-identical lock),
    drift detection (out-of-band edit ⇒ verify RED), lock tamper
    (hand-edited lock ⇒ verify RED), over-grant refusal (typed error).
  - AK-063 evidence; locator divergence (`tests/contract/roles::
    install_and_verify`) handled via reference-candidates entry at
    closure (D5) — never the ruler.
- **Recorded design decisions (pre-dispatch, operator-fog recorded not
  silently picked):** D1 roles/ vs agents/ surfaces distinct; D2
  roles.lock is runtime-owned (not reference.lock; not writable by graded
  roles); D3 unset GAUNTLET_AGENT_RUNNER fails closed with a typed
  message, gate evidence runs an explicit registered id (D-dev class);
  D4 registry is a capability map, no vendor names; D6 enforcement is
  type-level in-task, composed dispatch stays task20 (R-1b).
- **Checkpoints:** builder writes `b1-progress.md` as it goes; final
  report `builder-b1.md` (result contract: file exists + gates run +
  touch surface). Critic reports `critic-design.md`; adjudication in
  `design-adjudication.md`.
- **Hard boundaries:** NO composed role dispatch (R-1b; task20); no
  envelope-builder rewiring beyond the typed consume-side deny-set; no
  writes outside the repo state root; UNK-003 untouched; answer key
  never opened for edit; `just ruler` PASS before/after every phase.
- **Post-implementation:** blind EXECUTING falsifier (fresh context,
  charter verbatim, warm start = its round's not-attacked list), then
  corrections, closure records, outcome file, CURRENT_STATUS, two-commit
  settlement.
- **Recovery note:** reconcile `git status` vs this file before
  re-dispatching (tree outranks record). One compiler at a time; WSL2
  memory tight (~1.6 GiB free).
- **INTERRUPTION 2026-08-30 ~03:1xZ (22nd overall; 23rd event counting
  the double dispatch failure):** both concurrent dispatches FAILED at
  2026-08-30 ~03:11-03:15Z — the design critic on a transient network
  error (PRE-launch; no critic-design.md, no artifacts), builder B1 on
  the sub-agent 5-hour usage window (reset 2026-08-30 14:06:59 local)
  AFTER launching and working: `b1-progress.md` checkpointed through its
  eleven design decisions B1-D1..D11 (all within-brief, recorded) and
  the tree holds recovered work — `.agents/roles/` (10 role files),
  `crates/gauntlet-app/src/roles/`, `crates/gauntlet-app/tests/`,
  additive edits to `lib.rs` + `config/resolve.rs` (B1-D8 visibility).
  NO `builder-b1.md`, NO CLI wiring in `main.rs`, NO gate runs, NO RED
  teeth evidence — completion state UNKNOWN until inventoried.
  Resumption decision (2026-08-30 ~07:0x EDT, window still closed): per
  the operator's standing heal+continue instruction the ORCHESTRATOR
  completes the recovered slice personally, recorded as
  ORCHESTRATOR-COMPLETED with degraded independence. Compensations
  owed BEFORE settlement, dispatched after the window resets (≥
  14:06:59): (1) a fresh-context STATIC critic doing a design-as-built
  review (the pre-implementation design round never ran), and (2) the
  blind EXECUTING falsifier. Neither sees the orchestrator's
  completion narrative; both see the tree + frozen prereg + spec.
  Teeth RED evidence owed personally either way.
- **ORCHESTRATOR COMPLETION (2026-08-30 morning, window still closed):**
  the recovered slice was completed personally per the standing
  heal+continue rule. Substrate first (validated post-interruption:
  builder's recovered files intact; the orchestrator's own pre-death
  edits — deny-list line numbers, eprintln removal — verified present;
  no CLI wiring existed). Found and fixed by RUNNING the never-run
  tests: the lock serialize/parse digest-prefix asymmetry (COR-O-fix-1),
  verify's bare-vs-prefixed compare (fix-2), the reforge test's own
  unprefixed self-digest line (fix-3), the deny-set collation assertion
  (fix-4), the shadow-refusal line-0 locus (fix-5), a debug eprintln
  (fix-6). COR-O1: the canonical-closure check had ZERO discriminating
  tests (gutted → all 21 green) — tooth added, RED→GREEN proven. All
  four owed teeth proven RED→GREEN personally. CLI wired
  (roles install|verify; registry via RUNNER_TOKEN constants; doctor-
  mirroring config discovery). Arch integrations: `filesystem` joined
  SPEC_VOCABULARY_SEGMENTS (Task 10/11/14 precedent, pins grown); the
  installer's four write sites pinned in WRITE_API_INVENTORY (pins
  grown); roles.lock gitignored. Live binary verification: install/
  verify/idempotence(byte-identical lock)/drift-RED/repair/advisory/
  fail-closed-ids/indirection-absence/doctor-D2 — all through the REAL
  command surface. Battery: 1211/1211, boundaries 228/228, lint clean,
  deny ok, doctor OK, ruler PASS, prereg 17 PASS. Full record:
  `correction-orchestrator.md`. Two deviations recorded there
  (untracked-file git-checkout restore no-op; orchestrator CLI wiring).
- **COMPENSATION ROUNDS OWED BEFORE SETTLEMENT (dispatch ≥ 14:06:59):**
  (1) fresh-context STATIC design-as-built critic (the pre-implementation
  design round never ran — the critic dispatch died pre-launch on a
  network error); (2) blind EXECUTING falsifier (fresh context, charter
  verbatim). Neither receives the orchestrator's completion narrative;
  envelope = the tree + the frozen prereg + spec + plan. Settlement only
  after both rounds adjudicated.
