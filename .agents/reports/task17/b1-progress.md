# B1 progress — Task 17 builder (roles artifacts + harness installer)

- Builder B1 session; compile lock held here. Started after `just ruler`
  PASS at HEAD f8d4f3f. This file is checkpointed as work lands.
- Spec surfaces quoted and anchored: §10.1 (role front matter), §13.2
  (envelope allowlist + not-received list), §10.2 (role policy table),
  §10.3 (result contract), §8.1 (secrets by indirection). AK-063 read.
- Reuse confirmed, not forked: `config::gauntlet_md::parse` (front-matter
  grammar, one parse site), `config::resolve::expand_indirections`
  ($VAR / `${VAR}` presence-validated expansion — made `pub(crate)`,
  same-crate additive visibility change, recorded as B1-D8),
  `config::env_layer::EnvLayer` (the value source), domain `RoleId`,
  ports `DENY_LISTED_FIELD_NAMES` + envelope.rs docs for the category
  vocabulary, `defaults::*` layout names, sha2 idiom from
  `ports::observation_digest`.

## Design decisions (within the brief; numbered for the final report)

- **B1-D1 — closed 3-class context vocabulary (18 categories).**
  `envelope-permitted` (5): `reference, observation_bundle,
  comparison_contract, applicable_constraints, open_residuals` — the
  §13.2 allowlist lines, matching `EnvelopeInputs` members.
  `envelope-denied` (7): `builder_trajectory, builder_reasoning,
  implementation_rationale, attempt_count, builder_identity,
  prior_verdicts, operator_chat` — §13.2's not-received list in §10.1's
  spellings. `runtime-provided` (6): `claim, evidence, instrument_output,
  target_state, run_history, metrics` — §10.2's own words for roles whose
  context flows outside the envelope. Over-grant = an `allow` naming an
  envelope-denied category ⇒ typed construction error. Runtime-provided
  categories cannot over-grant the envelope: the ports envelope type has
  no member they could ride in (envelope.rs module doc).
- **B1-D2 — deny∩allow ⇒ typed error.** The frozen prereg's L3
  structural-transfer test names "a deny that shadows allow" among shapes
  "rejected by the typed construction". A file claiming both is a
  contradictory contract; the runtime must not silently pick a winner.
  Consequence: the effective envelope grant equals `allow ∩ permitted`;
  `deny` is defense-in-depth data the envelope side consumes (refuses the
  named categories even when another input could include them).
- **B1-D3 — critic dimensions ride the file stem, not a new key.**
  `critic-<dimension>.md` loads through the SAME constructor as every
  role file; §10.1's field set stays closed (no invented `dimension`
  key). A `critic-*` stem whose front matter is not `role: critic` is a
  typed error (name/identity mismatch).
- **B1-D4 — install materializes role files VERBATIM.** $VAR resolution
  happens at LOAD (presence validation, typed error naming variable +
  locus, never the value). Install writes byte-identical copies under the
  state root; no environment value is ever written by install (§8.1:
  generated files never contain credentials). Lock digests cover the
  verbatim bytes, so idempotence is structural (no timestamps, sorted
  entries).
- **B1-D5 — tamper story, stated honestly.** `roles.lock` carries a
  self-digest (SHA-256 over the canonical body excluding the
  `lock_digest` line) + canonical re-serialization byte-equality, so a
  naive hand-edit ⇒ typed tamper failure. A self-digest is forgeable by a
  committed attacker who recomputes it, so the DRIFT VERDICT never rests
  on the lock alone: verify independently compares (a) actual source
  bytes vs the lock's digest, (b) actual installed bytes vs the same
  digest. Forging the lock to bless drifted installed state forces the
  attacker to also edit the reviewed repo source file — which the
  repository's own review surface sees. Documented in the module.
- **B1-D6 — registry is a capability map; no vendor literal in app/cli
  source.** `HarnessRegistry: BTreeMap<harness id, RoleSurface>` with
  `RoleSurface::{WritableRoleDir, AdvisoryPromptArtifacts}`. The
  composition root keys entries with the adapters' EXPORTED runner-token
  constants (the app denylist denies the token spellings as substrings in
  app source — the arch test's own R2 fixture). Ids validated at insert
  (non-empty, no separators/traversal) so an installed path can never
  escape the state root via the id.
- **B1-D7 — envelope.rs untouched.** The "consume-side hook" is the
  typed API: `ContextDenySet` + `RoleDefinition::envelope_grant()`
  documented as what the envelope builder MUST consume when dispatch
  composition lands (task20, R-1b). Strictest reading of the touch
  surface ("NOTHING else"); zero call-site changes anywhere.
- **B1-D8 — additive visibility change in config:** `resolve.rs`'s
  private `expand_indirections` → `pub(crate)` (same-crate reuse; the
  one-word change the brief pre-authorized; recorded here and in the
  final report).
- **B1-D9 — one digest per lock entry.** Installed bytes are verbatim
  source bytes by construction, so source and installed share one
  expected digest; drift = any divergence among {source bytes, lock,
  installed bytes}.
- **B1-D10 — per-harness install dirs.** `<state_dir>/roles/installed/
  <harness>/` for writable surfaces, `<state_dir>/roles/injectable/
  <harness>/` for advisory (named for its actual function: per-invocation
  prompt-side injection, `enforcement: advisory` recorded in the lock —
  visible weakness).
- **B1-D11 — §10.1 field validation.** Required: `role`, `description`,
  `context` (with `allow` and `deny`), `permissions` (all four §10.1
  keys — fail-closed rather than grant-by-default), `result_contract`
  (§10.3: every invocation declares one). Optional: `runner`, `model`,
  `deadline_seconds` (typed `Option`, no invented default). Unknown key
  anywhere ⇒ typed error with the line number.

## Status log

- [x] Read IN-FLIGHT, prereg (frozen), plan §Task 17, spec §10.1/§13.2,
      AK-063; surfaces read (gauntlet_md, env_layer, resolve, envelope,
      main.rs, defaults, denylist derive rules, arch walk scope).
- [ ] Role artifacts (6 + 4 critic dimensions) under `.agents/roles/`.
- [ ] `roles::` module in gauntlet-app (category, definition, harness,
      install) + lib.rs export + resolve.rs visibility tweak.
- [ ] Integration tests `crates/gauntlet-app/tests/roles.rs` (test id
      `roles::install_and_verify` for AK-063; locator divergence is the
      orchestrator's to record — D5).
- [ ] RED evidence per tooth (deliberate breakage, captured, reverted).
- [ ] CLI `roles install|verify` wiring in main.rs.
- [ ] Gates: check-package → test-package → check → targeted nextest →
      lint → boundaries → ruler; doctor with scratch report dir.
- Mid-flight corrections from the orchestrator: none received yet.
