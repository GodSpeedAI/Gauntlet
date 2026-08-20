# Task 9 — Builder brief (round 1)

Plan: `.agents/plans/gauntlet-impl.md` Task 9 (proves P3). Units in scope:
AK-028, AK-029, AK-030 (read them in `.wayfinder/answer-key.md` yourself).
Spec anchors: §5.3.6, §12.1–§12.4, §6.2 Observer, §21.3, §23.1 P3, Annex B.3
(`observation_bundle@v1`), I2/I8. AGENTS.md rules apply in full.

## Non-negotiables

1. Never edit `.wayfinder/` or `.agents/specs/`. Ruler defects become dated
   entries in `.agents/reports/reference-candidates.md` (three are specified
   below — write them exactly in the established format).
2. Never modify the frozen Task-4 shared contract suites
   (`crates/gauntlet-ports/src/contract/**`). Your adapters PASS
   `observer_suite` unmodified; you do not edit it.
3. Never weaken `just boundaries`, deny.toml, or any pinned inventory. The
   one allowlist edit permitted is the recorded-act reason-text update in
   `crates/gauntlet-arch-tests/src/deps.rs` specified below (no new entries —
   `sha2` is already adapter-layer-allowlisted; extend the existing entry's
   reason to name the observe adapters' use, update any pin test only if it
   pins reason TEXT, which it does not — it pins names).
4. One builder holds the build lock: run cargo strictly sequentially; never
   raise jobs (the repo `.cargo/config.toml` pins `jobs=1` — respect it).
   Prefer `cargo check -p <crate>` per slice; full `cargo nextest` runs at
   slice ends only. This host has ~7.8 GiB RAM and dies if you parallelize.
5. Append a checkpoint to `.agents/reports/task9/builder-checkpoints.md`
   after EVERY slice: what landed, test counts, deviations, open questions.
   Your final message may be lost — the checkpoint file is your contract.
   Also write `.agents/reports/task9/builder-round1.md` (final report:
   artifact inventory, test counts per gate, fail-first evidence you
   observed, honest limitations).
6. Testing loop: for each bounded behavior, add the discriminating test
   first where practical and record fail-first evidence in the checkpoint.
7. Rust discipline per AGENTS.md: typed errors with failure locus, no
   unwrap/expect in production paths (tests may expect with a stated
   invariant), clippy `-D warnings` clean, `cargo fmt` before finishing.
8. Bounded everything: any walk, parse, or collection loop carries an
   explicit typed cap constant (learn from Task 7's runaway-memory defect
   and Task 8's `MAX_SCENARIO_DEPTH`). No unbounded recursion, no
   unbounded Vec growth from external input.

## Design decisions already taken (do not re-litigate; flag friction in
checkpoints if one is wrong)

- **The port surface is NOT modified.** `ObservationFacet`'s 8 fields mirror
  Annex B.3 exactly; per-facet environment fingerprints are recorded in the
  facet's content-addressed evidence document (below), and the bundle-level
  `environment_fingerprint` (Annex B.3) is computed by the app assembler.
  Document this reconciliation in the ports module docs (§12.2 asks each
  facet to record the env fingerprint; B.3's wire facet has no such field —
  the evidence document is where the per-facet fingerprint lives).
- **The wrapper is the instrument** (plan Task 9 redesign trigger): each
  adapter records its own identity — instrument name `gauntlet-observe-<x>`
  (`gauntlet-observe-filesystem`, `-git`, `-build`, `-test`, `-command`)
  with `env!("CARGO_PKG_VERSION")` as version — and the exact wrapped
  command line on the facet. The versioning adapter's instrument is the
  wrapper, not git itself (uniformity; git's own version is not probed —
  record this as a known limitation for the critic).
- **Exit-status semantics**: a command that RUNS and exits nonzero is a
  successful observation of failing reality → a FACET carrying that exit
  status (this is P3's core). An instrument that cannot run (spawn failure,
  missing workspace, unreadable target, store write failure) is an
  `ObserverError` → both gap arrays. EXCEPTION: the versioning adapter maps
  git's own nonzero exit to `InstrumentFailed` (git failing means no usable
  repository state; record exit code + stderr in the detail) — document.
- **WorkspaceRef resolution**: adapters are constructed with a workspace
  root `PathBuf`; a request resolves to `root.join(request.workspace.as_str())`.
  WorkspaceRef is a validated opaque string (domain `string_newtype!`), so
  joining is safe; if the resolved path does not exist →
  `ObserverError::Unavailable`. No membership list — existence on disk is
  the truth. Suite factories materialize their `live` workspaces under a
  scratch root in `build()`.
- **Env fingerprint**: sha256 over the sorted `KEY=VALUE` lines of the
  observing process environment, rendered `sha256:<hex>`. Adapters capture
  the environment ONCE at construction (`new` uses process env; tests use
  `from_parts(root, clock, store, env_pairs)` with an explicit pair list).
  Deterministic by construction under injected env.
- **EvidenceKind for facet raw output**: `EvidenceKind::Log` for all five
  adapters (raw instrument output document); the assembled bundle document
  is also stored as `Log`. Document the choice.
- **Digest rule (§12.4)**: `observation_digest` = sha256 over the canonical
  JSON of the B.3 bundle document with ONLY timestamp fields removed
  (`observed_at`, per-facet `started_at`/`ended_at`). Claims ARE covered by
  the digest (forensic tamper-evidence) and are NOT verification input
  (I2) — two different concerns; document crisply.
- **Canonical JSON**: use `gauntlet_ports::json` (`JsonValue` +
  `to_json_text`). Build objects with keys in the Annex B.3 documented
  order (order-preserving serializer; fixed construction order = canonical
  bytes). Strict decode via `parse` + unknown-key sweep (mirror
  `role_result.rs` conventions, including its strict-only decoding).
- **Claims exclusion is structural** (plan step 4): the bundle document has
  `facets: Vec<FacetRecord>` (instrument facets) and a SEPARATE field
  `claims_facet: ClaimsFacet` — a distinct type with no
  conversion/overlap into `FacetRecord`. `VerificationInputs` is built
  only from the instrument-facets array. There is no runtime
  `if kind == claims` anywhere; exclusion is the absence of a path.
- **project_revision** is caller-supplied at assembly (the runtime knows
  the revision it dispatched against; the versioning facet records repo
  state, not the revision contract). Document as a Task-12 wiring point.
- **Deadlines** follow the port-wide recorded convention: noted, passed to
  store writes, not enforced as process timeouts (Task-8 deferral class;
  Task 12 composes real deadline behavior).
- **Failure→gap mapping**: on `ObserverError`, the assembler records BOTH
  an `observation_failures` entry AND an `unavailable_evidence` entry
  (§21.3: observation failure ⇒ recorded unavailable evidence). The
  ObserverError carries `kind` + `detail` but no instrument name — record
  the facet-kind token as the instrument/probe identity and document the
  limitation. POPULATION LIVES IN EXACTLY ONE PLACE in the assembler
  (single match arm) — the T1 tooth deletes it.

## Slices

### D1 — ports: `facet_evidence` module (additive)

New file `crates/gauntlet-ports/src/observer_evidence.rs` (name your module
this or close; declare in `lib.rs`, re-export the types at the ports root).
Contents:

- `pub struct FacetEvidenceDoc` — the wire document every Task-9 observer
  writes as its raw-output evidence. Schema token `"facet_evidence@v1"`
  (repo convention extending Annex B.3; the spec does not define evidence
  content format — say so in the module doc). Fields:
  `instrument_name`, `instrument_version`, `command` (the exact command or
  query string), `env_fingerprint`, `exit_status: i64`, `started_at: u64`,
  `ended_at: u64`, `stdout: String`, `stderr: String` (lossy-UTF-8 rule:
  captured bytes are preserved lossily; state it in docs).
- `encode(&self) -> String` (canonical, keys in documented order) and
  `decode(text: &str) -> Result<Self, FacetEvidenceError>` — STRICT:
  unknown keys rejected, missing keys rejected, empty-string constraints
  rejected for `instrument_name`, `instrument_version`, `command`,
  `env_fingerprint`. Typed error enum `FacetEvidenceError` with locus.
- `pub fn command_line(program: &str, args: &[String]) -> String` — the
  unambiguous rendering of argv (single-quote args containing whitespace
  or quotes; document the rule). Shared so all adapters render commands
  identically.
- Unit tests: strict-decode teeth (unknown key, missing key, empty
  instrument name), encode→decode round-trip, command_line quoting pins.

### D2 — the five observe adapters

Per crate under `crates/adapters/gauntlet-adapter-observe-*/` (crate names
stay as-is; `observe-git` exposes `FacetKind::Versioning` — operator rename
absorbed, never `git` as a capability name in types):

Common shape (duplicated per adapter — adapter→adapter deps are forbidden;
keep each copy small, D1 carries the shared codec):

- `pub struct <X>Observer { root, clock: Arc<dyn Clock>, store: Arc<dyn
  EvidenceStore>, env_fingerprint: String }` implementing `Observer`.
  `new(root, clock, store)` captures process env; `from_parts(...,
  env_pairs: &[(String, String)])` computes the fingerprint from explicit
  pairs (sorted, `KEY=VALUE` lines, sha2, `sha256:<hex>`).
- `observe`: resolve workspace path (see resolution rule); run the
  instrument; build `FacetEvidenceDoc`; `store.put(doc.encode().as_bytes(),
  EvidenceKind::Log, deadline)` → `EvidenceRef`; build
  `ObservationFacet::new(kind, InstrumentIdentity::new("gauntlet-observe-<x>",
  env!("CARGO_PKG_VERSION")), evidence_ref)` + `with_invocation(Some(command),
  Some(exit))` + `with_window(...)` + `with_summary(...)`. Errors →
  `ObserverError` typed with the failing step in the detail.
- `pub struct <X>ObserverFactory` implementing the ports
  `ObserverFactory`: constructed with `(clock, store)`; `build(live)`
  creates a scratch root (hand-rolled Scratch temp-dir idiom — `tempfile`
  is not allowlisted; copy the established idiom from
  `crates/adapters/gauntlet-adapter-agent-mock/tests/scenario_playback.rs`),
  materializes each live workspace (see per-adapter notes), returns the
  observer bound to that root. These factories are suite scaffolding
  (documented); production wiring is plan Task 12/15.
- Per-crate `tests/shared_contract.rs`: run the UNMODIFIED
  `observer_suite(&factory)` and assert zero findings, plus a per-adapter
  provenance test (AK-028 substance): observe a real scratch workspace →
  facet carries non-empty instrument name+version, non-empty command,
  recorded exit status, evidence ref whose stored bytes strict-decode as
  `facet_evidence@v1` with all fields present and exit status matching the
  facet. Use a tiny fixed `struct FixedClock` + an in-test scratch
  `EvidenceStore` (check `crates/gauntlet-ports/src/contract/stubs.rs` for
  an existing store stub you may reuse from the ports crate; do NOT
  dev-depend on the evidence-fs adapter — adapter→adapter is forbidden).

Per-adapter specifics:

- **filesystem**: walk the resolved workspace: relative paths (forward
  slashes), entry kind, size, sha256 of content. BOUNDED: cap depth (e.g.
  64), total entries (e.g. 10_000), per-file bytes hashed (e.g. 8 MiB —
  larger files record size + `"digest": null` + a `reason` field); NEVER
  follow symlinks (list them as entries, do not descend); all caps are
  named consts with doc comments. Output: JSONL manifest lines (one object
  per entry) into the doc's stdout field; command (query) string states
  the walk contract exactly (root key, caps, follow_symlinks=false).
  exit_status 0 on a completed walk. Unreadable entries are recorded IN
  the manifest with an error field (the observation reports reality);
  a failure to even read the workspace root is `InstrumentFailed`.
- **git (Versioning)**: run `git status --porcelain=v1 -b` with cwd =
  workspace, captured stdout/stderr; exit 0 → facet carrying git's exit
  0 and the status output as evidence; git nonzero → `InstrumentFailed`
  with exit code + stderr in the detail (documented exception above).
  git binary missing (spawn error) → `Unavailable`.
- **build**: wraps a declared `BuildCommand { program, args }` (constructor
  param — tool binding is Tasks 15–16); runs it with cwd = workspace;
  ANY exit status is a facet (the observed reality); spawn failure →
  `InstrumentFailed`. Kind `FacetKind::Build`.
- **test**: identical shape, kind `FacetKind::Test`.
- **command**: identical shape, kind is a constructor PARAMETER
  (`FacetKind`) — the generic command facet; reject nothing at the type
  level, document that the caller binds it to a class.
  Suite factory for command/build/test uses `program = "true"`-style
  trivial command (exists on this host; do not hardcode a path — bare
  name resolved via PATH is the honest form).

Cargo.toml per crate: add `sha2.workspace = true` (already
adapter-layer-allowlisted). No other new deps.

### D3 — `gauntlet-app/src/observation/`

Module `observation` (the plan gate filter is
`cargo nextest run -p gauntlet-app observation::` — your in-crate tests
MUST live under that path; verify the filter is non-empty before declaring
done — Task 8's plan-filter lesson).

- `mod.rs`: doc citing §5.3.6, §12, Annex B.3, I2/I8; submodules + re-exports.
- `bundle.rs`: `ObservationBundleDoc` (typed B.3 mirror): `schema` (const
  `"observation_bundle@v1"`), `run_id`, `iteration_no`, `observed_at`,
  `project_revision: String`, `environment_fingerprint: String`,
  `observation_digest: Digest`, `facets: Vec<FacetRecord>`,
  `claims_facet: ClaimsFacet`, `unavailable_evidence:
  Vec<UnavailableEvidenceRecord>`, `observation_failures:
  Vec<ObservationFailureRecord>`. `FacetRecord` = B.3 wire facet
  (kind token, instrument name/version, command, exit_status, started_at,
  ended_at, evidence_ref, summary) — converted FROM `ports::ObservationFacet`
  (define `impl From<&ObservationFacet> for FacetRecord` or a named ctor).
  `ClaimsFacet { source: const "builder", excluded_from_verification:
  const true, content: String }` — `content` is the builder claims (joined
  or JSON-array rendered — pick one, pin it in a test). Wire-record types
  for the gap arrays per B.3 shapes (`{kind, reason}`,
  `{kind, error_class, detail}`). `to_json()` canonical serialization +
  strict `decode` (unknown keys rejected) mirroring role_result
  conventions; error type `ObservationDocError` with locus. Conversion to
  domain `ObservationBundle` (`unavailable` locator = `facet:<kind-token>`,
  failure instrument = kind token — documented mapping) — provide
  `as_domain_view() -> ObservationBundle` (digest + arrays only).
- `assembly.rs`: `ObservationAssembler::assemble(plan, deadline) ->
  AssembledBundle { doc, bundle_ref, }` where `ObservationPlan { request:
  ObservationRequest, observers: Vec<Box<dyn Observer>>,
  project_revision, environment_fingerprint, claims_content: String,
  clock: Arc<dyn Clock> }`. Runs observers in plan order; `Ok` → facet
  record; `Err` → THE single population site pushing BOTH gap entries.
  `observed_at = clock.now()` once. Writes the finished doc bytes to the
  store (`EvidenceKind::Log`) — the assembler takes an
  `Arc<dyn EvidenceStore>` at construction; returns the doc + ref.
  Facet evidence provenance cross-check: for each facet, `store.get`
  the evidence, strict-decode as `FacetEvidenceDoc`, and if a field is
  missing/empty or the exit status DISAGREES with the facet record →
  record an `observation_failures` entry naming the mismatch (the facet
  stays; the disagreement is data). Do not fail the whole bundle on a
  mismatch (honest gap, not a crash).
- `digest.rs`: `observation_digest(doc) -> Digest` per the rule above;
  expose `normalized_bytes(doc) -> String` (timestamp-stripped canonical
  JSON) so tests can pin normalization directly. Tests: same doc, shifted
  timestamps → identical digest; one-byte content change in a facet,
  claims content change, gap-array change → digest changes.
- `input.rs`: `InstrumentFacets` (newtype over `Vec<FacetRecord>`,
  constructor private, built ONLY by `of_bundle`) and
  `VerificationInputs { facets: InstrumentFacets, observation_digest,
  bundle_ref, unavailable_evidence, observation_failures }` with
  `VerificationInputs::of_bundle(doc, bundle_ref)`. `of_bundle` reads
  `doc.facets` and the gap arrays ONLY; there is no code path from
  `ClaimsFacet` into these types (no `From`, no accessor reuse — claims
  content reaches only `ClaimsFacet::content()` for forensics). Tests:
  bundle with rich claims → inputs' canonical serialization contains
  none of the claims text (canary), inputs equal inputs built from the
  same bundle with claims content swapped; claims change → digest
  changes but inputs unchanged.

### D4 — composition-root proofs (gauntlet-cli)

`crates/gauntlet-cli/tests/p3_real_observation.rs` and
`crates/gauntlet-cli/tests/g6_hide_failed_observations.rs` (target names the
plan gate and AK locators use; location rationale header comment mirroring
`p1_reference_integrity.rs`). Add dev-deps to gauntlet-cli: the five
observe adapters + `gauntlet-adapter-agent-mock` (+ existing ones; the
composition root may name adapters).

**p3_real_observation** (AK-029; spec §23.1 P3; AK-028 corroborating):
1. Load `tests/fixtures/mock_runner_scenarios/lying-builder.yaml` via
   `load_scenario` (repo-root locator per the established idiom), scratch
   workspace root, `MockAgentRunner::new(scenario, root)`.
2. Drive the run to completion the way
   `crates/adapters/gauntlet-adapter-agent-mock/tests/scenario_playback.rs`
   does (invoke → poll → collect → `read_result_contract`) so the workspace
   effect (`out/check-target.txt`, content FAIL) lands and the result
   document exists with its claims.
3. Observe the SAME workspace with: `FilesystemObserver` +
   `CommandObserver` bound to kind `Test`, program `grep`, args
   [`PASS`,`out/check-target.txt`] (the fixture's own declared self-check,
   re-run as an instrument).
4. Assemble the bundle with claims content from the collected result's
   `payload().claims()`.
5. Assert: the command facet exists with `exit_status() == Some(1)` and
   its evidence carries the failing run (P3: the bundle records the
   instrument's observation of failure while the claims facet asserts
   success — the divergence lives in the bundle); the filesystem manifest
   lists the artifact; claims facet content matches the document claims;
   `VerificationInputs::of_bundle` canonical bytes contain the FAIL-facing
   facet and NONE of the claims text; digest stable across re-assembly
   with the same clock/env (DeterministicClock from the mock adapter is a
   dev-dep — use it); digest changes when claims content changes.
**g6_hide_failed_observations** (AK-030): one observer whose program does
not exist (spawn failure → `ObserverError`) + one healthy observer →
assert facets == 1, `unavailable_evidence` has exactly the failed kind,
`observation_failures` has exactly the failed kind with an error class,
`has_gaps()` true, and the gap entries are covered by the digest (mutate
the doc's gap array via a rebuilt doc → digest changes); plus a
store-write-failure case (scratch store that fails `put` → the observer
surfaces an error → gap entries again); plus the positive statement: a
bundle over all-healthy observers has EMPTY arrays explicitly (typed, not
Option).

### D5 — recorded acts + docs

- `crates/gauntlet-arch-tests/src/deps.rs`: extend ONLY the existing
  `sha2` adapter-entry REASON string to also name the observe adapters'
  use (env fingerprints + filesystem manifest digests, plan Task 9). No
  new entries. If any pin test pins the reason text (it pins names),
  update it — check first.
- Three reference-candidates entries in
  `.agents/reports/reference-candidates.md` (format per existing entries;
  all three are ADDITIONAL INSTANCES of the existing
  locator-materialization class first recorded 2026-08-17 AK-049 — state
  that):
  - AK-028 `tests/contract/observer::facet_provenance` → cite the
    per-adapter provenance tests + app `observation::` cross-check tests.
  - AK-029 `tests/proofs/p3_real_observation` → cite
    `gauntlet-cli tests/p3_real_observation.rs` (p1 precedent).
  - AK-030 `tests/antigaming/g6_hide_failed_observations` → cite
    `gauntlet-cli tests/g6_hide_failed_observations.rs` + app
    `observation::assembly` gap tests.
- Update `.agents/CURRENT_STATUS.md` ONLY if you hit a genuine blocker
  (escalation) — otherwise leave the IN-FLIGHT line alone.

### D6 — your gate run (orchestrator re-runs everything)

```bash
cargo nextest run --test p3_real_observation
cargo nextest run --test g6_hide_failed_observations
cargo nextest run -p gauntlet-app observation::    # MUST be non-empty
cargo nextest run -p gauntlet-ports observer        # ports tests
for c in filesystem git build test command; do cargo nextest run -p gauntlet-adapter-observe-$c; done
just check && just lint && just deny && just boundaries && just ruler
cargo nextest run --workspace
```

Record exact counts per gate in `builder-round1.md`. Lint/fmt last; do not
declare done with clippy or fmt red (Task 8's recovered miss was exactly
this). Do NOT run teeth yourself — the orchestrator runs them; but DESIGN
for them: the single gap-population site (T1), the structural exclusion
(T2), the env-fingerprint field (T3), timestamp exclusion in
`normalized_bytes` (T4).
