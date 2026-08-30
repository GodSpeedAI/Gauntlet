# Task 17 DESIGN-AS-BUILT critic report — role artifacts + harness installer

- Critic: independent design critic (fresh context, static-only)
- Date: 2026-08-29
- Subject: Task 17 built surface, reviewed design-as-built against the frozen envelope
- Mode: read-only source inspection; NO cargo/just compile or test commands were run;
  `just ruler` not needed (tree-only review)

## VERDICT

**NOT_CONFIRM** — no blocking residual found, but eight MAJOR residuals stand (RES-D01,
RES-D02, RES-D03, RES-D04, RES-D06, RES-D07, RES-D13, RES-D14). The load-bearing core
(the §13.2 intersection invariant, §10.1 field fidelity, install idempotence, the
re-forgery teeth) is sound and well-tested; the failures concentrate in (a) two
fail-open paths in the verify/tamper story, (b) config/permission-vocabulary refusals
the fail-closed surface claims but does not have, (c) an unfulfilled plan/prereg
obligation (deny-set consumption), and (d) stale external wiring (justfile, `roles.dir`).

## Envelope reviewed against

1. `.agents/specs/Core-spec.md` §10.1, §10.2, §10.3, §13.2, §8.1/§8.2, §8.5, §17.1–§17.5
2. `.agents/plans/gauntlet-impl.md` Task 17 (goal, steps 1–5, gate, done-when, redesign trigger)
3. `.agents/research/preregistrations/task-17.prereg.yaml` — frozen prereg (D1–D6, A1–A4,
   P1–P5, R1–R4, transfer tests, explicit nonclaims)
4. `.wayfinder/answer-key.md` AK-063

Spec §10.1, quoted verbatim (the front-matter contract this task implements):

> Each role is a versioned file at `${roles.dir}/<role>.md` with YAML front matter:
>
> ```yaml
> ---
> role: critic
> description: Compares an observation bundle against the frozen reference and emits residuals.
> runner: $GAUNTLET_CRITIC_RUNNER
> model: $GAUNTLET_CRITIC_MODEL
> permissions:
>   filesystem: read-only
>   write_paths: ["$RESULT_CONTRACT_DIR"]
>   network: deny
>   tools: [code_nav, filesystem]
> context:
>   allow: [reference, observation_bundle, comparison_contract, applicable_constraints,
>           open_residuals]
>   deny:  [builder_trajectory, builder_reasoning, attempt_count, builder_identity,
>           prior_verdicts, operator_chat]
> result_contract: comparison_record@v1
> deadline_seconds: 900
> ---
> <the role prompt body>
> ```
>
> Rationale: harness-native role state (for example Prime-Agent's refinable subagent
> specs) is session-local and mutable by the agent. Making the file the source of truth
> and *installing* it into the harness keeps role definitions reviewable, diffable, and
> reproducible across harnesses. `gauntlet roles install` writes them into whichever
> harness is configured (§10.5, §10.6); `gauntlet roles verify` re-reads them back and
> reports drift.
>
> The `context.deny` list is enforced by the runtime when it builds the envelope (§13.2).
> It is a structural allowlist, not an instruction to the model.

Plan invariant, quoted verbatim: "context.deny feeds the envelope builder's allowlist
(Task 10) — **the file cannot grant a role more context than the builder permits**."

## Built surface reviewed

- `.agents/roles/*.md` — all 10 artifacts read in full
- `crates/gauntlet-app/src/roles/{mod,category,definition,harness,install}.rs` — read in full
- `crates/gauntlet-app/tests/roles.rs` — read in full (918 lines)
- `crates/gauntlet-cli/src/main.rs` — roles wiring (lines 269–473) plus doctor/reference
  wiring for parity comparison
- `crates/gauntlet-arch-tests/src/denylist.rs` (`filesystem` SPEC_VOCABULARY_SEGMENTS entry
  + pin test), `crates/gauntlet-arch-tests/src/single_writer.rs` (roles write-site entries
  + pins)
- Supporting reuse surface (to verify A1/§8.1 claims): `config/gauntlet_md.rs`,
  `config/resolve.rs`, `config/error.rs`, `config/defaults.rs`, `doctor/mod.rs`,
  `compare/envelope.rs`, `gauntlet-domain` `RoleId`, `justfile`, `GAUNTLET.md`,
  `.env.example`

## Firewall compliance

The firewalled files (`.agents/reports/task17/IN-FLIGHT.md`, `correction-orchestrator.md`,
`b1-progress.md`, `.agents/CURRENT_STATUS.yml`, `.agents/reports/task15|16/**`,
`.agents/agents/**`) were NOT opened. `ls` of the task17 reports directory and of
`.gauntlet/`/`.wayfinder/` revealed filenames only; no firewalled content was read. No
builder or orchestrator narrative informed this review. One consequence is recorded under
"Not reviewed": deviations the prereg requires to be *recorded* may exist in firewalled
files; I graded on the tree.

## What held (positive findings, for settlement context)

1. **The intersection invariant is faithful.** `ContextCategory::ENVELOPE_PERMITTED` is
   exactly the five §13.2 allowlist lines (`reference`, `observation_bundle`,
   `comparison_contract`, `applicable_constraints`, `open_residuals` — matching
   `EnvelopeInputs`' members `frozen_reference`/`bundle`/`contract`/`constraints`/
   `open_residuals`). `EnvelopeDenied` carries ALL SEVEN §13.2 not-received items
   (`builder_trajectory`, `builder_reasoning`, `implementation_rationale`,
   `attempt_count`, `builder_identity`, `prior_verdicts`, `operator_chat`). No denied
   category is missing (nothing an allow could name slips through), no permitted member
   is missing (no legal allow is refused), and `envelope_grant() == allow ∩ permitted`
   exactly (`definition.rs:334-340`).
2. **§10.1 field fidelity.** The loader's field set is exactly the spec's eight keys plus
   the four permission sub-keys; no invented required keys, none dropped. `result_contract`
   values in all 10 artifacts name real Annex B schemas (B.1–B.5, B.7 all verified
   present in the spec). The closed-field posture (unknown key = typed error, unlike
   GAUNTLET.md's forward-compatible ignore) is a justified divergence the code documents.
3. **Reuse-not-refork (A1) holds.** The loader uses `config::gauntlet_md::parse` (one
   grammar; duplicate keys refused by the grammar itself) and
   `config::resolve::expand_indirections` (presence-validated; the error names variable +
   locus, never the value). Manual `Debug` redacts resolved fields; the canary test
   checks lock, installed bytes, and Debug.
4. **Install idempotence is real.** Entries sorted by stem, no timestamps, deterministic
   serialization; the byte-identical-lock test has teeth.
5. **The re-forgery teeth are genuine.** The source-anchoring design (verify digests
   actual source AND installed bytes against the lock) plus the canonical-closure check
   are each proven by a test that fails under the attack they exist for
   (`full_lock_reforge_cannot_bless_installed_drift`,
   `noncanonical_lock_with_recomputed_digest_is_tamper`).
6. **The denylist `filesystem` entry follows the documented precedent exactly** — same
   argument shape as `model`/`digest`/`check`/`none` (last segment of an adapter crate
   name that is spec field vocabulary: §10.1's `permissions.filesystem` key AND §8.4's
   `filesystem` capability), compound spellings still denied, pin test updated.
7. **Role bodies are governance-free prose** citing spec sections; no settlement,
   authority, or budget decisions implemented in text (R4's mitigation satisfied);
   per-role allow/deny lists are coherent with §10.2's reading columns.
8. **Advisory fallback is honest**: separate `injectable/` surface, `enforcement:
   advisory` in the lock, visible weakness printed by the CLI.

## Residuals

### RES-D01 — a forged EMPTY lock passes verify (exit 0, zero roles)
- dimension: security (tamper story)
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/install.rs:227-264` (parse accepts zero entries),
  `install.rs:679-751` (verify iterates lock entries only), `install.rs:744-751`
  (empty drift list → `Ok`)
- defect: `RolesLock::parse` accepts a canonical, self-digest-consistent lock whose
  `entries:` block is empty, and `verify` then loops zero entries and returns
  `Ok(VerifyReport { roles: 0 })` → exit 0. Install can never emit such a lock
  (`EmptyRoleSet` refuses an empty roles dir), so a zero-entry lock is by construction
  non-runtime-written — precisely the hand-forged shape the tamper story exists to catch.
  The `LockMissing` guard catches DELETING the lock (fail); FORGING an empty one passes.
  An attacker with write access to `.gauntlet/roles.lock` — the same capability deletion
  requires — converts a detectable state into an undetectable one, and the plan's gate
  line `… && gauntlet-cli -- roles verify` goes green. Contradicts the module's own
  posture ("missing evidence is not passing evidence") and its documented contract
  ("a hand-edited or re-forged lock — is a typed configuration failure").
- reproduction: install for a registered harness; then write `.gauntlet/roles.lock` as
  `schema: gauntlet-roles-lock@v1\nharness: <id>\nenforcement: writable\nentries:\n` plus
  a correctly recomputed `lock_digest: sha256:<hex>` (body joined with trailing newline);
  `gauntlet roles verify` prints "verified 0 role(s)" and exits 0. No file is consulted.

### RES-D02 — `roles install` ignores the configured `roles.dir` (config precedence divergence)
- dimension: functional (contract divergence)
- severity: **major**
- locus: `crates/gauntlet-cli/src/main.rs:403` (`roles_dir: &…defaults::roles_dir_at(…)`),
  against `crates/gauntlet-app/src/config/resolve.rs:1166` (`pub roles_dir` on
  `ResolvedConfig`, resolved through GAUNTLET.md `roles.dir` + `GAUNTLET_ROLES_DIR` +
  anchoring; `.env.example:49` advertises `GAUNTLET_ROLES_DIR=`; this repo's `GAUNTLET.md:8-9`
  sets `roles.dir`)
- defect: spec §8.2 defines `roles.dir`; §8.1 makes every path overridable by the layer
  stack. The CLI's `roles_context()` already holds the `ResolvedConfig` and takes
  `state_dir` from it, but passes the static default `.agents/roles` for the roles tree.
  An operator override is silently ignored: two surfaces, one config key, different
  answers — and inconsistent within one command (state_dir honored, roles_dir not).
  Latent in-repo today only because the repo's GAUNTLET.md spells the default. Side
  observation: `roles_context()` re-sequences doctor's discovery steps inline rather
  than sharing a context-resolution function with `doctor::assemble` — same building
  blocks, second composition (a mild A1-style fork risk; steps currently agree).
- reproduction: set `GAUNTLET_ROLES_DIR` (or GAUNTLET.md `roles.dir`) to a tree that
  differs from `.agents/roles`; `gauntlet roles install --harness <id>` installs the
  DEFAULT tree's contents (or fails `EmptyRoleSet` if only the override is populated),
  while doctor resolves the override everywhere else.

### RES-D03 — `write_paths` refusal is weaker than the spec's protected-path posture
- dimension: security
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/install.rs:579-592`
- defect: the only `permissions.write_paths` validation refuses ABSOLUTE paths outside
  BOTH the project and state roots. An absolute path INSIDE the project root passes for
  any role — including `.wayfinder/answer-key.md`,
  `.gauntlet/comparison-contract.yaml`, `.gauntlet/roles.lock`, and the evidence dir.
  Spec §10.2 (builder Never: reference, comparison contract, acceptance gates, evidence
  store, answer-key status), §17.5.5 (evidence store, state store, frozen reference,
  answer key outside every builder-writable path), and the frozen prereg's D2 (roles.lock
  "NOT writable by roles that produce work graded against it") are all unencoded: a role
  file can declare write permission over the ruler and the lock itself and pass typed
  construction AND install. These declarations are the data a future PolicyProvider
  (§17.2) consumes, so the over-grant is recorded as legitimate. (The prereg's
  structural-transfer case — "a $VAR resolving to a path outside the workspace" — is
  satisfied only for the outside-the-roots case; relative entries like `workspaces`
  in `builder.md` pass wholly unchecked with undefined anchoring semantics.)
- reproduction: `builder.md` with `write_paths: ["/abs/spelled/project/.wayfinder/answer-key.md"]`
  loads (`RoleDefinition::load` → Ok) and installs; no typed error names it.

### RES-D04 — set-membership drift is uncovered: out-of-band ADDITIONS are invisible to verify
- dimension: functional (drift semantics)
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/install.rs:679-743`
- defect: verify's subject is exactly the lock's entry list × {source, installed}. Two
  drift directions are undetectable: (a) a NEW file added to the installed surface
  out-of-band — the Annex C.4 threat model (harness role state is agent-mutable; a
  hostile subagent drops `evil.md` into `<state>/roles/installed/<harness>/`) — verify
  passes; (b) a new role file added to `.agents/roles/` after install without
  reinstalling — installed state is stale, verify passes. AK-063's claim is "drift
  between the file and installed harness state"; the out-of-band ADDITION is the
  harness-mutation case the lock exists for. (Deletion of a recorded source IS caught as
  `SourceMissing`; only the set-growth directions escape. Related: the single lock
  records one harness; installing harness B orphans harness A's records with no report.)
- reproduction: install; `cp builder.md <state>/roles/installed/<harness>/evil.md`;
  `gauntlet roles verify` → exit 0.

### RES-D05 — `write_atomic` claims §10.3 discipline but omits the fsync half
- dimension: evidence (claim accuracy)
- severity: **minor**
- locus: `crates/gauntlet-app/src/roles/install.rs:808-824`
- defect: the doc comment says "the §10.3 result-document discipline applied to the
  installer's own outputs", but §10.3's discipline is write-to-tmp, FSYNC, rename. No
  fsync of file or parent directory: torn reads are prevented, crash durability is not.
  An interrupted install can also leave `<stem>.staging`/`roles.staging` orphans that
  nothing cleans or reports.
- reproduction: inspection; kill an install between staging write and rename → orphan
  persists; verify neither flags nor cleans it.

### RES-D06 — permission VALUE vocabularies are unvalidated: a hostile file widens past construction
- dimension: security (fail-closed surface gap)
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/definition.rs:52-58` (`filesystem`/`network`:
  free `String`; `tools`: free `Vec<String>`), `definition.rs:593-643`
  (`walk_permissions` validates only presence/kind/emptiness)
- defect: the loader's own rationale — "a typo must never silently weaken a constraint"
  (applied to context categories) — is not applied to the permission block's VALUES.
  `network: allow`, `filesystem: read-write`, or `tools: [browser, web, sandbox]` all
  pass typed construction. §8.4 fixes a CLOSED semantic-capability list for tools
  (`code_nav · filesystem · versioning · test · build · browser · visual · web ·
  research · security · sandbox · memory · evidence · domain · policy · benchmark ·
  schema`); §10.1's grammar shows `filesystem`/`network` as mode tokens; AGENTS.md's
  typing rule ("every fixed-value-set field in the spec is a type") applies. These are
  exactly the declarations a future PolicyProvider will lower into sandbox posture —
  a typo'd or widened token becomes a live grant question at composition time, and
  today it is silently admitted. This is a missing refusal in the surface the module
  documents as fail-closed.
- reproduction: role file with `network: allow` and `tools: [not_a_capability]` loads
  (`Ok`), installs, and is recorded in the lock; no typed error.

### RES-D07 — the typed deny-set has NO production consumer; plan step 2's "feeds the envelope builder's allowlist" is unimplemented at any level
- dimension: architecture (plan/prereg divergence)
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/mod.rs:19-23` (doc re-scoping consumption to
  Task 20), `definition.rs:323-340` (`context_deny_set`/`envelope_grant`), vs
  `crates/gauntlet-app/src/compare/envelope.rs` (no reference to `ContextDenySet`,
  `context_deny_set`, `envelope_grant`, or `RoleDefinition` anywhere outside `roles/`
  and its own tests)
- defect: the plan's step 2 states "context.deny feeds the envelope builder's allowlist
  (Task 10)". The frozen prereg's D6 frames consumption as a compile-shaped IN-TASK
  obligation ("the loader produces a typed deny-set that the envelope builder MUST
  consume"), and A2 claims the same. Built state: the task-10 envelope builder accepts
  no deny-set parameter, import, or hook; the ONLY reader of `context_deny_set()` is
  `tests/roles.rs`. The plan's load-bearing invariant (the file cannot GRANT beyond the
  builder) still holds — by construction-time over-grant refusal plus the builder's
  independent fixed allowlist — but the NARROWING feed (e.g. `builder.md` denying
  `comparison_contract`) reaches no runtime code path, and nothing compile-shaped
  compels Task 20 to consume it. The deferral is documented only in the new module's
  own prose; no deviation record is visible in this review's envelope (firewalled
  files may carry one — see Not reviewed). If a deviation-from-plan was recorded, this
  residual reduces to "recorded deferral"; on the tree alone it is an unfulfilled plan
  step and prereg decision.
- reproduction: `rg -n "ContextDenySet|context_deny_set|envelope_grant"` over
  `crates/` — matches only under `gauntlet-app/src/roles/` and `tests/roles.rs`.

### RES-D08 — the RuntimeProvided class has no per-role §10.2 backstop
- dimension: architecture (latent over-grant)
- severity: **minor**
- locus: `crates/gauntlet-app/src/roles/category.rs:50-56,159-167`
- defect: a role file's `allow` may name any RuntimeProvided category regardless of
  role: `builder.md` with `allow: [reference, evidence, run_history]` passes
  construction. `envelope_grant()` excludes them from the ENVELOPE (so §13.2 cannot be
  widened — the module's claim is true), but these are exactly the declarations a
  future composer would use for non-envelope context delivery, and §10.2's Never column
  (builder: evidence store; critic: evidence, other roles' context) is encoded nowhere
  as data. Before Task 20 consumes `context_allow()`, the §10.2 per-role policy table
  needs a typed backstop or the class needs per-role admission rules.
- reproduction: `role_document` variant with `allow: [reference, evidence]` and
  `role: builder` loads `Ok`.

### RES-D09 — role-artifact citation/prose fidelity nits
- dimension: functional (documentation)
- severity: **minor**
- locus: `.agents/roles/builder.md:32` ("Lower your unit's blast radius or authority
  (spec §12.2)"), `.agents/roles/orchestrator.md:30-31`
- defect: (a) §12.2 is "Facets"; the never-lower rule is §17.1 ("A tier may be raised
  by an operator; it is never lowered by an agent") — wrong section citation in a
  spec-citing contract artifact. (b) orchestrator.md says "If none exists, the run is
  blocked, not self-served", dropping §10.7's operator escape hatch
  (`GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`); a conservative narrowing, but it
  contradicts the cited section's full rule.
- reproduction: compare the lines against spec §12.2 and §10.7.

### RES-D10 — the category vocabulary cannot express §10.2's builder reading set
- dimension: architecture (vocabulary incompleteness)
- severity: **minor**
- locus: `crates/gauntlet-app/src/roles/category.rs:50-56` (RuntimeProvided =
  claim/evidence/instrument_output/target_state/run_history/metrics only), spec §10.2
  builder row ("reference (read-only), map, its unit(s), prior residuals for its unit")
- defect: the RuntimeProvided class encodes the §10.2 reading words for
  verifier/observer/optimizer but not builder's `map` and `unit(s)`. `builder.md`'s
  `allow: [reference, open_residuals]` cannot declare two of its four spec-sanctioned
  inputs; a Task-20 composer must supply them outside the typed context grammar,
  invisible to the loader's review. Vocabulary decision to record before composition.
- reproduction: attempt to spell "map" in any `context.allow` → `UnknownContextCategory`.

### RES-D11 — single_writer pin reason overstates guard coverage for the lock write sites
- dimension: evidence (pin accuracy)
- severity: **minor**
- locus: `crates/gauntlet-arch-tests/src/single_writer.rs:259-285` ("every site behind
  the lexical containment guard"), `install.rs:628-644` (lock dir `create_dir_all` +
  `write_atomic` with no `ensure_within_root` call), `single_writer.rs:181-192`
  (guard semantics = presence needle in same file, not call-site domination)
- defect: of the write sites pinned behind `ensure_within_root`, only the
  installed-directory family is actually dominated by a containment CALL; the lock-path
  sites are safe by fixed-leaf construction (`project_root/.gauntlet/roles.lock` — no
  unvalidated component enters the path), not by the guard. The inventory's reason line
  ("every site behind the lexical containment guard") is therefore inaccurate as
  written, and the checker's guard clause verifies needle PRESENCE, not domination.
  Materially safe today; the warrant text should say why.
- reproduction: code inspection of `install()`'s lock-path branch vs the pinned reason.

### RES-D12 — `roles verify` silently ignores unknown arguments
- dimension: functional (convention divergence)
- severity: **minor**
- locus: `crates/gauntlet-cli/src/main.rs:441` (`fn roles_verify(_args: &[String])`)
- defect: every other command in this CLI refuses unknown arguments with exit 2
  (doctor, reference digest, roles install); `roles verify --anything` ignores them and
  proceeds. Typos in flags go unnoticed (`--harness X` on verify silently does nothing).
- reproduction: `gauntlet roles verify --harness prime-agent` → runs plain verify, exit 0.

### RES-D13 — load-bearing guards with no teeth: `contained_join` escape refusal and `SourceMissing` are untested
- dimension: evidence (test teeth)
- severity: **major**
- locus: `crates/gauntlet-app/src/roles/install.rs:683-692` (`SourceMissing` branch),
  `install.rs:757-768` (`contained_join` traversal/absolute refusal → `LockTampered`);
  absent from `crates/gauntlet-app/tests/roles.rs`
- defect: the suite never deletes a source file (the `SourceMissing` DriftKind — one of
  four — has zero coverage) and never exercises `contained_join`'s refusal (a forged
  lock recording `source: ../…` or an absolute path). Deleting the containment check
  inside `contained_join` leaves the entire suite green — a mutation the charter's
  teeth standard cannot accept for a guard whose entire job is anti-escape on forged
  input. (Charter grades missing unit tests major; these guard security-relevant
  refusals, not convenience paths.)
- reproduction: mutate `contained_join` to `Ok(root.join(relative))` unconditionally, or
  delete the `SourceMissing` arm — no test in `roles.rs` fails.

### RES-D14 — stale `just roles-verify` placeholder exits 2 for a capability this task shipped
- dimension: functional (broken canonical-CLI wiring)
- severity: **major**
- locus: `justfile:56-59`
- defect: the recipe prints "roles-verify: not implemented — lands in plan Task 17
  (role drift detection)" and exits 2 — but Task 17 HAS landed (`gauntlet roles verify`
  exists, tested, wired). AGENTS.md makes the justfile the canonical human-and-agent
  CLI "whenever an applicable recipe exists"; an operator or agent following that rule
  gets a false "not implemented" and a non-zero exit for shipped functionality. The
  recipe's own comment ("must fail loudly until then") marks the update as this task's
  obligation. The plan's gate used raw cargo, so the task gate passed while the
  repository-facing surface regressed.
- reproduction: `just roles-verify` → stderr "not implemented", exit 2.

## Teeth assessment of the shipped tests (static)

Load-bearing tests and the mutations they catch: `install_and_verify` (verbatim bytes /
verify-green), `reinstall…byte_identical_lock` (timestamps/order), the three drift tests
(InstalledModified / SourceModified / InstalledMissing — each asserts the typed kind),
`hand_edited_lock…` (self-digest), `full_lock_reforge…` (source anchoring — the P3
prediction's core attack), `noncanonical_lock…` (canonical closure — would fail if the
closure check were deleted), over-grant / unknown-key / shadow / typo refusals (each
asserts the typed variant and message content), indirection classes (variable + locus
named), canary test, write-path escape, advisory fallback, registry sweep (both
capabilities), fail-closed ids, verify-without-install, deny-set/grant data, dimension
stem coupling, and repo-artifact loading. No shipped test was found that CANNOT fail for
a mutation in its named subject. The gaps are absences, not dead tests: RES-D01
(empty-lock forgery), RES-D04 (addition drift), RES-D13 (`contained_join`, `SourceMissing`),
and interior write-paths (RES-D03) have no test — consistent with those defects being
present.

## Not reviewed

- Firewalled narratives (see Firewall compliance) — including whether RES-D07's deferral
  and the AK-063 locator divergence (prereg D5: "record the locator-class divergence via
  a reference-candidates entry") were recorded as deviations in task reports; no
  reference-candidates surface is visible in `.gauntlet/` or `.wayfinder/` outside the
  firewalled reports, so D5's recording could not be verified from this envelope.
- Dynamic behavior of any kind: no compile, test, or gate command was run (static-only
  mandate). All reproduction steps above are derived from source, not executed.
- `just ruler` output; runtime behavior of the real prime-agent / generic-cli adapters
  beyond their registry tokens; doctor's full check set against roles.lock (D2's
  "must not trip doctor" claim was checked structurally only — doctor parses GAUNTLET.md,
  not `.gauntlet/roles.lock`; names do not collide).
- Prose quality of role bodies beyond spec-citation fidelity and the governance-logic
  rule; the `.agents/agents/**` operator surface (firewalled by charter).
