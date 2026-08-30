# Task 16 — correction round 2 (post round-2 CONFIRM)

**Round-2 verdict: CONFIRM** (fresh blind executing falsifier, report:
`falsifier-round2.md`). No BLOCKING and no in-mechanism MAJOR stands.
Four findings adjudicated:

## Adjudication

| Id | Severity | Disposition |
|---|---|---|
| F1 | MAJOR-with-COR (teeth debt) | **ACCEPTED → COR-R2-1** (tests-only) |
| F2 | MINOR (deliberate-posture liveness cost) | **trigger-recorded residual R16-F2** |
| F3 | MINOR (locus-label honesty) | **trigger-recorded residual R16-F3** |
| F4 | MINOR-with-COR (teeth debt) | **ACCEPTED → COR-R2-2** (tests-only) |

Precedent: task15 settled on a CONFIRM round whose MAJORs were of the
missing-in-tree-teeth class, corrected tests-only (COR-D3) with the teeth
re-verified personally. Round 2 is the same shape. Tests-only corrections
change zero shipped behavior, so the confirmed mechanism is untouched;
only its guards are added.

## COR-R2-1 — permanent tooth for `probe_template`'s render half (F1)

The doctor's honesty probe (`crates/adapters/gauntlet-adapter-agent-cli/
src/template.rs:293`, called from the composition root's cli token leg)
has zero permanent in-tree callers-tested behavior. Round-2 mutation M1
proved a parse-only regression survives every shipped suite (58/58 adapter
in-tree, 62/62 gauntlet-cli incl. P8).

**Correction (tests-only):** a permanent test in
`crates/adapters/gauntlet-adapter-agent-cli/tests/` asserting
`probe_template`:
- REFUSES a parse-ok / render-impossible template (`'' {prompt_file}` ⇒
  typed `EmptyProgram` render error) — the discriminating leg: under a
  parse-only gutting this returns `Ok(())` and the test goes RED;
- ACCEPTS a valid template (no over-fail);
- REFUSES an unknown placeholder with the typed error.

Tooth proof owed: re-apply mutation M1 (gut `probe_template` to
`parse_template(source).map(|_| ())`) → new test RED with exactly that
leg failing → restore byte-identical (sha256 verified) → GREEN.

## COR-R2-2 — decode-locus exclusivity scan for the cli crate (F4)

The delegation await-ban / decode-locus scan
(`crates/gauntlet-arch-tests/src/delegation.rs`) scopes
`read_result_contract(` / `RoleResult::decode(` exclusivity to
`PRIME_ADAPTER_SRC` only; the cli adapter has the vendor-term scan but no
decode-locus scan. Today the only cli decode locus is `runner.rs`
`classify_contract` (poll path) — correct by inspection, unguarded
structurally.

**Correction (tests-only):** a delegation-scoped sibling check over
`crates/adapters/gauntlet-adapter-agent-cli/src` enforcing the same
decode-call exclusivity (poll/collect only), with the same teeth style as
the prime scan.

Tooth proof owed: plant a `read_result_contract(` call outside poll/collect
in the cli crate → `just boundaries` RED naming file:line → restore
byte-identical → GREEN.

## Trigger-recorded residuals (not corrected; CONFIRM stands)

- **R16-F2** (liveness posture): a hostile child can poison its own
  contract path (e.g. a directory at `GAUNTLET_RESULT_PATH`) and stay
  alive; poll then returns the typed Transport error forever — the
  `TimedOut` verdict is unreachable and the runner never self-stops the
  child; only explicit cancel retires it (executed evidence L5a: cancel
  worked, pinned Cancelled). Deliberate posture per §21.2
  (uncertainty-never-success; error is never coerced to a state);
  harm bounded by the operator/runtime cancel path. **Trigger:** first
  transport/lifecycle touch or the composed recovery E2E (task25) —
  revisit whether a wedged-read deadline override is owed.
- **R16-F3** (diagnostic label): `TemplateError::UnterminatedQuote`'s
  offset is a CHAR index rendered as "at byte N" (`template.rs:88` vs
  `219/251`); executed with `ää'` (reports 3, true byte 4). Typed and
  diagnosable. **Trigger:** next `template.rs` touch (one-word Display
  fix or byte-offset computation); not landed post-CONFIRM to keep the
  confirmed tree's shipped behavior byte-stable.
- **R16-note-1** (from the falsifier's not-attacked list): the
  SIGKILL-landed-but-unreapable `Ok(None)` reap-grace-expiry path is
  pinned only by code reading (not reachable with a normal child on this
  host). **Trigger:** next transport touch.
- **R16-note-2**: OS pid reuse against the transport's pid-keyed child
  map — noted, not attacked (exotic; the early-exit check in `stop`
  closes the practical window). **Trigger:** next transport touch.

## Deviation record

None for this round beyond the falsifier's own recorded one (first
mutation restore used `git checkout --`; all three files digest-verified
identical afterward — working-tree restore only, no history/index write).
