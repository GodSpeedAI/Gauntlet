# COR-5 progress (task 26, corrections round 5 — final small corrections)

Builder: COR-5. Charter + round-2 falsifier report read; adjudication
binding: FR2-1/2/3 = recorded boundary (not a fixable defect) + one
best-effort hardening layer (W1) + the executed boundary disclosure (W2).

Baseline sha256 (untouched tree, matches the falsifier's ledger):
- `crates/gauntlet-cli/src/improve_cmd.rs`
  `2b019bb2937fd9bf91725b44d85d657b31179e17097fc0d89646c18ba5ca5af7`
- `crates/gauntlet-cli/tests/improve_baseline.rs`
  `1a386ad8b1f3bd23d8f82b16ed059db3190192da320cb87b9a09334ffd2cc488`
- `crates/gauntlet-app/src/baseline/report.rs`
  `c5568a6220da6f47a20fa3ad2e040a4e8ae512f3995a3a2e0f2ad9decc67dbea`
- `crates/gauntlet-app/tests/baseline_harness.rs`
  `555848ecb74b1d130ad7ac14236c7f0b7d986112e737a60402cec6ad4457b705`

## Design decisions (W1)

- Manifest file: `<snapdir>/<nonce>.manifest` — the nonce in the name keeps
  the existing nonce-set check true (the manifest IS wrapper output).
- The wrapper appends one `<name> <sha256-hex>` line per entry it writes
  (`.rs` and `.absent`); the copy-failed branch creates the manifest file
  with no line (`: >> "$manifest"`), so the invariant is: manifest exists
  IFF at least one durable facet execution happened. Digests via
  `sha256sum` + `cut` (host coreutils; PATH-resolved like the wrapper's
  other tools).
- Grading (`grade_judge_channel`), order fixed: wrapper pin → nonce set →
  manifest split/`snapshot_manifest_fact` → zero/zero fallback → count
  cross-check (manifest excluded from the count) → snapshot judgment over
  the filtered entries.
- Manifest rules: facet_executions 0 + manifest present = tamper (judge
  material outside the wrapper's output); facet_executions > 0 + manifest
  missing = tamper; malformed/duplicate/dangling lines = tamper; every
  `.rs`/`.absent` entry must carry a manifest line whose bare hex equals
  `content_digest(bytes).hex()` — else tamper (stale digest = FR2-2's
  naive shape; renamed/replaced `.absent` = FR2-3's naive shape).
- NO new Rust write-API token: the manifest is written by the shell
  wrapper (the existing "materialize the judge wrapper" write site); the
  grading side only reads. Arch write inventory: no new entry owed
  (verified against `single_writer.rs`'s token set).
- Disclosure of the layer's strength: catches naive in-place rewrites and
  nothing stronger — the manifest is child-writable; the boundary wording
  (W2) says exactly that.

## Status

- [x] Context read (falsifier-round2.md, charter.md, all five target
      files, arch inventory, Digest/content_digest API).
- [x] W1 RED-first: named tests
      (`a_snapshot_rewritten_under_a_stale_manifest_is_tamper`,
      `an_honest_manifest_grades_normally`) RED for the expected reasons
      (count cross-check counted the manifest / tamper fact named the
      count, not a manifest cross-check). Log: `/tmp/cor5/red-pre.log`.
- [x] W1 mechanism (wrapper body + `snapshot_manifest_fact` + grading
      wiring) → executor module 9/9 GREEN (`/tmp/cor5/w1-green.log`).
- [x] W1 teeth: `if let Some(fact) =` → `if false && let Some(fact) =`
      at the single call site →
      `a_snapshot_rewritten_under_a_stale_manifest_is_tamper` RED (the
      forged rewrite graded PASS — FR2-2's damage resurfaces) while
      `an_honest_manifest_grades_normally` stayed PASS; restore
      byte-identical (sha256 `8f51b9d3…` before = after); 9/9 GREEN
      re-proof (`/tmp/cor5/tooth-manifest-{red,green}.log`).
- [x] W2 RED-first: the three disclosure assertions updated first — app
      test + both CLI gate tests RED against the old wording
      (`/tmp/cor5/w2-red-{app,cli}.log`) — then
      `JUDGE_CHANNEL_BOUNDARY_DISCLOSURE` reworded → app GREEN,
      plant test GREEN, gate-shape test needed the designed T5 update
      (the wrapper's manifest now appears in the judge dirs; content
      loops skip it, and the honest leg's manifest coverage is asserted
      through the real binary) → both CLI tests GREEN
      (`/tmp/cor5/w2-green-*.log`).
- [x] Battery: arch-tests 237/237; check-package gauntlet-app/gauntlet-cli
      exit 0; test-package gauntlet-app 627/627; test-package
      gauntlet-cli 151/151 (re-run after a fmt reflow of the new code);
      real gate shape into scratch exit 0 with matrix rows IDENTICAL to
      COR-4's recorded table, the reworded disclosure present in `_meta`,
      and every D leg carrying `held_out_basis`; `./scripts/verify-ruler.sh`
      PASS. No load-flake fired (`b1_contract_wins_over_exit7` never
      tripped; nothing re-run in isolation).
- [x] cor5-report.md written.

## Notes for the orchestrator

- `.agents/CURRENT_STATUS.yml` is outside this round's EXCLUSIVE file
  scope — its task26 metadata/next_action lines need the orchestrator's
  transition update (COR-5 dispatched → landed).
- Suggestions for the orchestrator-owned records are in cor5-report.md
  §"For the orchestrator's records" (charter appends; reference-candidates
  entry; narrative wording carries the disclosure).
