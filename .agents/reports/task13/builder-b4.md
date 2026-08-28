# Task 13 / B4 result contract — composition proofs p7 / p11 / g10 + teeth

**Provenance (orchestrator-written):** the original B4 builder died
mid-Item-1 (19th instance); a finisher (dispatched on the operator's usage
reset) completed Phase R-B4 + Items 2–3 with teeth evidence and died during
Item 4; the orchestrator completed Item 4 (boundary conformance + the gate
battery + this contract) per the standing heal+continue instruction. The
builders' evidence is theirs, as recorded in `builder-b4-checkpoints.md`.

## Delivered

- **`crates/gauntlet-cli/tests/p7_recovery.rs`** (AK-041/AK-042, P7):
  `commit_order` — the composed loop makes the §16.3 order observably
  durable-event → status row → in-memory advance (OrderProbe delegating
  decorator over the real store's own export), with the REORDER tooth;
  kill/resume legs — a real process-abort (`crash_helper`, in-test
  `build --bin`, no new child-binary surface) at each §16.3 window
  (before the event append → retry-as-if-never-verified; the event/row
  window → half-applied completion; after the row → durable, memory lost),
  each resumed through a FRESH composition driving the production
  `resume_run`: exactly one settlement record, no duplicate event, no
  re-dispatch of settled work, run identity and prior evidence intact;
  the anchored-replay leg; the lost-evidence leg. 4 tests.
- **`crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`** (AK-031,
  P11): comparator `pass` admitted through the real comparison chain with
  NO verification record does not settle (no `SettlementCommitted`, no
  row, projection open) across ≥2 honest shapes; the INCONCLUSIVE leg
  distinct from FAIL; the comparator-pass + FAIL-verification leg naming
  the real cause; the neutered reads-verification-records gate tooth.
  5 tests.
- **`crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs`**
  (AK-043): prevention — a builder-context write attempt targeting the
  key via an escaping locator (`../ruler/g10-answer-key.md`, built not
  spelled) refused by the real containment (typed
  `EvidenceLocatorEscapes`), key bytes byte-identical, no settlement
  under the refused ruler; detection — a direct mid-run tamper (the
  §11.4 operator-hand-edit shape) → the settle-time consult trips →
  terminal `aborted{reference_mutated}` with BOTH digests pinned as
  forensic evidence, tampered bytes preserved; the D4 tautology tooth
  (frozen-digest-vs-itself consult lets the settle commit → RED). 2
  tests. Recorded deviation: the escape needle's first draft dropped the
  separator (a normal component — the run settled, exposing the bug);
  fixed with the named-separator construction.
- **`teeth-b4.md`** — every tooth: break → RED name + tail → revert →
  cmp byte-identical → green.
- **Orchestrator conformance fixes (Item 4):** the never-gated files'
  literal needles rebuilt per the suite's idioms (composed path
  components; char-array tool/command tokens; prose hoisted out of the
  `Command::new` scan unit; concat! args scanned individually —
  recorded).

## Gates (orchestrator-run, final tree)

boundaries **215/215** · proofs **12/12** · `settle::` **22/22** ·
standing battery **33/33** · check clean · lint 0 · deny ok · ruler PASS.

## Interaction points with correction round 2 (recorded for COR-R2)

COR2-1a (one-settlement-per-unit admission) and COR2-1b (dispatched-set
completion guard) will strengthen surfaces these proofs exercise; the
proofs' assertions are compatible (they assert exactly-once/no-bypass
shapes COR2 strengthens). The p7 header records the touchpoints.
