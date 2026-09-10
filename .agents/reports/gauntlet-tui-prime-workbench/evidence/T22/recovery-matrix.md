# T22 Evidence — Cold-Loss and Recovery Matrix

Date: 2026-09-09
Task: T22
Proof Level: P3
Confirmation: BUILDER evidence pending independent adversarial confirmation.
Scope: Destructive recovery cases over the projection store (SQLite), TUI host, Python REPL kernel, live authoritative streams, Prime availability, and SXR producer/CAS absence — executed against the real modules, with frozen procedures, pre-state digests/cursors, and retained failed-round evidence.

Preregistration: `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T22.prereg.yaml`, frozen, hash-verified before implementation began:
`prereg-check.sh: PASS — sha256 e9ab7ae44ca2512bb106256fb5f5058d457fd500066202d403c624eb966ea738` (re-verified after implementation; never modified).

## Settled Requirements

- **REQ-SAFE-001**: loss of TUI/renderer/local SQLite/kernel state MUST NOT alter canonical Gauntlet truth.
- **REQ-SAFE-005**: local inconsistency MUST trigger resync/degraded state, never mutate canonical sources.
- **REQ-SXR-004**: qualification includes incompatible flags/schema, invalid hashes, missing CAS, unavailable providers; optional producer absence is valid and declared.
- **REQ-SYNC-001**: source identity/epoch/cursor/retention/watermark contract; epoch replacement or retention expiry forces a fresh baseline.
- **REQ-SYNC-003**: loss of disposable projections reconstructs admitted consequential records; ephemeral kernel values and unretained output are LABELLED unavailable, never recreated.
- **REQ-VERIFY-001**: deleting the local Bun SQLite projection database reconstructs equivalent consequential projections.
- **REQ-VERIFY-002**: killing/restarting TUI and Python REPL kernel independently MUST NOT lose canonical truth.
- **REQ-VERIFY-005**: closing the workbench during admitted long-running Prime execution allows authorized work to continue and permits correct reattachment.
- **REQ-VERIFY-006**: active-view projections stay coherent across baselines, increments, reconnect generation changes, and late stale-frame delivery.

RECOV coverage: RECOV-001 (two cases: deletion, corruption), RECOV-002, RECOV-003, RECOV-004, RECOV-005, plus the REQ-SXR-004/REQ-SYNC-003 absence case. **RECOV-006 is a declared exclusion** (plugin lifecycle belongs to T15; prereg known_blindspots), recorded as data in `SCOPE_EXCLUSIONS` along with cross-surface isomorphism (T23) and cross-process Prime reality (T24) — never a silent gap.

## Implementation

1. **`workbench/src/recovery-matrix.ts`** (NEW) — the declarative case registry and executor:
   - `RECOVERY_CASES`: 7 frozen case declarations, each pinning (a) the exact destructive procedure as typed data (`DestructiveProcedure` union), (b) the pre-state evidence to capture, (c) the declared post-state contract.
   - Pre-state capture: per-source authoritative fingerprints (epoch, watermark, minted-cursor window, content digest) + T19-normalized canonical-section and full digests + authoritative cursors — a PURE function of the log, so it works while hosts are dead.
   - Corruption audit: `readProjectionRows` (READ-ONLY raw rows across ALL generations, so a naive rebuild cannot hide phantom rows behind generation-scoped reads) + `auditProjectionRows` (typed findings: `cursor_not_minted_by_source`, `generation_cursor_not_minted`, `cursor_behind_authoritative_watermark`, `row_disagrees_with_authoritative_fold`) against the sources' minted-cursor window and the T19 pure log fold.
   - `quarantineDatabase`: WAL-checkpoint → sha256 pin → RENAME out of the way. The corrupt state is RETAINED on disk as evidence — recovery never destroys what it recovered from.
   - `spawnRealTuiProcess`: deterministic real-process kill — spawn, await a readiness marker, SIGKILL, reap. No racing window (prereg honesty rule).
   - `RecoveryRoundLedger`: append-only; duplicate (case, round) records throw `RecoveryLedgerError`; a round that once failed keeps its record forever.
   - `buildMatrixWorld` / `referenceModel`: the shared fixture (T16 client over in-memory authoritative sources, T06/T07 store file- or memory-backed, T04 Prime adapter root+child, T05 kernel + T11 ledger with one artifactized / one pinned / one ephemeral object) and the authoritative-fold reference.
   - Boundary pin (recorded, not a workaround): `sxr-adapter.ts` is presentation-classified by the T19 renderer-boundary suite and this module is a policed state-layer file, so the RECOV-SXR case declares its procedure/contract here and consumes the REAL probe outcomes as the typed `SxrAvailabilityRecord`, produced by the tests through the real `executeSxrRefresh` / `parseSxrSlice0bEnvelopeOutput`. A missing probe record is a FAILED case — missing evidence is never passing.
2. **`workbench/src/recovery-matrix.test.ts`** (NEW) — the matrix executed end-to-end: registry freezing checks, the whole-matrix run, per-case teeth, ledger append-only teeth, and the prereg L1 replay transfer test (whole matrix run twice; identical verdicts, assertion sets, and stable canonical digests). The SXR probes drive the real adapter with a failing runner (the pinned T14 process-isolation port; producer ABSENCE needs no binary).
3. **`workbench/src/cold-rebuild.test.ts`** (NEW) — the two plan teeth:
   - **Attack 1**: an independently crafted self-consistent corrupt set (BOTH sources' generation cursors forged to never-minted tokens, phantom unit + phantom evidence rows carrying those tokens, a tampered real row — the rows agree with each other, disagree with the authorities). Detection fires BEFORE recovery; a naive rebuild WITHOUT quarantine genuinely fails (phantom rows survive `INSERT OR REPLACE`); quarantine + cold rebuild yields zero corrupt rows and byte-equality with the T19 referee; sources byte-identical throughout.
   - **Attack 2**: simultaneous kill — a REAL TUI process SIGKILLed, the in-process host closed, and the kernel terminated in one synchronous block — while a Prime child continues (mailbox "almost done" does NOT complete it), authoritative events land while dead, and Prime completes the child through its OWN schema-valid contract. Three independent fresh reconstructions (cold SQLite rebuild, pure log fold, fresh host reattached over the surviving projection file) agree byte-for-byte under the T19 referee; the fresh host's OWN trace fan-out equals the log-derived audit trail with the pre-kill trail preserved append-only per source.
   - Referee teeth: the T19 digest moves when a canonical verified bit is tampered and stays still when only presentation (placements/operator selection) changes.

## Case registry and results (final run, deterministic)

| Case | RECOV | Procedure | Verdict | Assertions | Sources byte-identical | Canonical digest (fold) |
|---|---|---|---|---|---|---|
| RECOV-001A-projection-deletion | RECOV-001 | unlink file db + WAL/SHM | **pass** | 6/6 | yes | `41178b00…cff1387` |
| RECOV-001B-projection-corruption | RECOV-001 | craft self-consistent corrupt rows | **pass** | 11/11 | yes | `0c731d7d…88875685` |
| RECOV-002-tui-kill-during-live-work | RECOV-002 | in-process close + real-process SIGKILL | **pass** | 12/12 | yes | `5ba6ea7f…dfaf84bc8` |
| RECOV-003-kernel-loss-objects | RECOV-003 | in-process terminate (3 persistence classes) | **pass** | 13/13 | yes | `fb33175d…a1cd2012` |
| RECOV-004-reconnect-mid-stream-stale-frames | RECOV-004 | kill mid-stream; late frames, gap, old epoch | **pass** | 10/10 | yes (post-epoch) | `bcada6a0…146cd96` |
| RECOV-005-prime-unavailable-degraded | RECOV-005 | capability-scoped withdrawal | **pass** | 7/7 | yes | `ea47f486…475085b` |
| RECOV-SXR-absence-incompatibility-cas-loss | REQ-SXR-004/REQ-SYNC-003 | producer absent / CEP envelope / CAS object missing | **pass** | 4/4 | yes | `44f2ba65…f5287d7d9` |

`all_passed=true`. Detections recorded per case (verbatim samples):

- **RECOV-001B**: `generation_cursor_not_minted@source_cursors:gauntlet: persisted cursor 'cursor-t22-forged-9a3f' was never minted by source 'gauntlet' under its live epoch`; `cursor_not_minted_by_source@units:unit-phantom …`; `row_disagrees_with_authoritative_fold@units:AK-402: status:tampered!=pending …`. Plus `epoch_changed` validation on a forged-epoch persisted position (T16 re-driven), with convergence after the fresh baseline.
- **RECOV-003**: `pin_after_kernel_loss:kernel_object_lost`, `artifactize_after_kernel_loss:kernel_object_lost`, `stale_generation_pin:unknown_object`, `pinned_after_loss:kernel_restarted_pinned_value_not_retained`, `ephemeral_after_loss:ephemeral_kernel_state_not_reconstructible`; artifactized value available ONLY at its locator; name reuse in the new generation mints a NEW ephemeral object; a live restarted kernel still cannot settle a unit (REQ-SAFE-004).
- **RECOV-004**: 4 typed stale-frame rejections (`superseded_generation`, `subscription_closed`), gap buffered then drained, `epoch_change:gauntlet:frame=epoch-2:tracked=epoch-1` visible resync, convergence asserted before further authoritative appends.
- **RECOV-005**: `recovery_gate_not_passed`; store digest and all unit rows unchanged during withdrawal; restoration resyncs to a fresh converged generation.
- **RECOV-SXR**: `producer_absent:spawn_failed`, `producer_incompatible:incompatible:cep_envelope_not_sxr_output`, `cas_object_missing:cas_object_missing`; zero fabricated rows.

## Failed-round preservation (prereg D3)

The corruption case deliberately executes the tempting wrong fix first: rebuilding WITHOUT quarantine. It **genuinely fails** — the phantom/corrupt rows survive in the file (5 findings remain; generation-scoped snapshots would even LOOK clean while the file stays poisoned) — and that failed round is recorded FIRST in the shared append-only ledger, followed by the declared procedure's passing round carrying the quarantined state's sha256:

```text
ROUND RECOV-001A-projection-deletion #1 pass (delete_projection_database)
ROUND RECOV-001B-projection-corruption #1 failed (naive_rebuild_without_quarantine)
ROUND RECOV-001B-projection-corruption #2 pass (quarantine_and_cold_rebuild_plus_epoch_validated_reattach) sha256=781dfa74af15ceb49564884d523714fbc9318f76c9f7fa120b70bcd827a7202a  (run-specific)
ROUND RECOV-002-tui-kill-during-live-work #1 pass (kill_tui_host)
ROUND RECOV-003-kernel-loss-objects #1 pass (kill_kernel)
ROUND RECOV-004-reconnect-mid-stream-stale-frames #1 pass (disconnect_and_reconnect_with_stale_frames)
ROUND RECOV-005-prime-unavailable-degraded #1 pass (withdraw_prime_availability)
ROUND RECOV-SXR-absence-incompatibility-cas-loss #1 pass (remove_sxr_producer_and_cas_object)
```

The corrupt database itself is quarantined (renamed, sha256-pinned) and kept on disk for the run's duration — asserted by the case (`corrupt_state_quarantined_and_retained`); the test layer sweeps only its own `test-t22-*` scratch prefixes in `afterAll`, after the assertions. Any case that throws is also recorded as a RETAINED failed round by the executor's catch path.

## Attacks vs. outcomes (plan teeth / prereg falsifiers)

- **Attack 1** (corrupt rows self-consistent but disagreeing with authoritative cursors): **detected and rebuilt**. Typed findings name the forged cursors, phantom rows, and tampered row before any recovery; naive rebuild fails and is retained; quarantine + cold rebuild is digest-equal to pre-state and byte-equal to the T19 referee; sources byte-identical (the workbench never mutated authorities to preserve the local view). PASS.
- **Attack 2** (kill TUI and kernel simultaneously during a real Run while Prime child continues): **canonical truth and authorized work survive**. Real SIGKILL reaped; child stays running through a "done-ish" mailbox message; completes only via Prime's own schema-valid contract; three independent reconstructions agree byte-for-byte; pre-kill audit trail preserved append-only per source; lost pinned/ephemeral kernel values typed unavailable with null locators; no cancel ever issued. PASS.

## Correction rounds during this task (all preserved above)

1. **RECOV-003 digest mismatch (MAJOR, fixed)**: the canonical-digest comparison initially folded under the case's nominal generation while the live store's generation carries the client minter's serial suffix — a generation-id mismatch, not a truth difference. Fixed by capturing pre-state under the client's REAL live generation.
2. **RECOV-004 scenario bugs (fixed)**: the "out-of-order" frame was actually the next expected sequence (gap never formed — fixed by delivering the later sequence first), and the convergence check ran after authoritative probe appends that rejected frames never consumed (fixed by asserting convergence before the probe appends).
3. **Vacuous assertion removed (MAJOR, fixed)**: the TUI-kill correction-history check initially compared two log-derived traces to each other (tautology). Replaced with T19's `normalizeLiveFold` over the fresh host's OWN trace fan-out — the model's history genuinely parsed back and compared against the log-derived audit trail.
4. **Shared-ledger bug (fixed)**: `runRecoveryMatrix` without an injected ledger returned an empty ledger (per-case ledgers were discarded). Fixed: one shared ledger is minted and threaded through every case.
5. **Boundary-compliant SXR seam (design decision, recorded)**: the first draft imported `sxr-adapter` from the state-layer matrix module, which the T19 renderer-boundary suite would fail (sxr-adapter is presentation-coupled by a pinned policy decision). Restructured: probes execute in the test layer; the matrix consumes typed outcomes and treats a missing record as a FAILED case.
6. **Observation (debug harness only, not reproduced)**: one `bun -e` diagnostic run hit a transient `unable to open database file` on the corruption case; the identical sequence passed on immediate retry and every `bun test` run (5+) has been green. Not observed in any gate context.

## Honest gaps / scope boundaries

- Kernel loss is the declared in-model boundary (`PrimeReplKernel.terminate`); TUI loss is both in-model and a deterministic real-process SIGKILL of a harness-owned child. Cross-process Prime/kernel reality is T24 (prereg blindspot).
- Transport loss is the deterministic in-process `AuthoritativeStreamPort` boundary (T16 prereg binds scope); real network targets are T20. The epoch-1→epoch-2 source change in RECOV-004 is the SOURCE's own authoritative storage replacement, asserted as the ONLY change — never a workbench write.
- Minted-cursor validation uses the sources' retained window; retention-expired cursors are the declared REQ-SYNC-001 fresh-baseline condition, not corruption (RECOV-004 exercises that path via `epoch_changed`).
- SXR probes use the real adapter parsers/boundary with a stubbed process runner (the pinned T14 seam); per REQ-SXR-004, optional provider absence requires no binary, and claiming enriched SXR conformance would require non-mock evidence — not claimed here.
- RECOV-006 and cross-surface isomorphism are excluded by declaration (T15/T23); cross-process Prime is T24.

## Gate results (verbatim, final runs)

| Gate | Result |
|---|---|
| `just tui-test recovery-matrix` | exit 0 — 13 pass, 0 fail, 169 expect() calls |
| `just tui-test cold-rebuild` | exit 0 — 3 pass, 0 fail, 54 expect() calls |
| `just tui-test detach-reattach` | exit 0 — 3 pass, 0 fail, 55 expect() calls (T16 suite stays green) |
| `just tui-check` | exit 0 — `tsc --noEmit -p tsconfig.json` clean |
| `just tui-lint` | exit 0 — `biome check .` clean at error level (57 warnings are the pre-existing repo-wide noConsole/noNonNullAssertion warnings; T22-owned files checked alone: 0 errors, 0 warnings). Note: two intermediate lint runs failed while the T21 parallel builder's in-flight files (`payment-budgets.*`, `large-case.bench.ts`, `long-horizon.bench.ts`) carried format/import errors; per protocol retried after ~30s+ until their tree stabilized. T22 files never contributed an error. |
| `just tui-preflight bootstrap` | PASS (exit 0) — pinned deps, bun.lock in sync, bun audit clean, spec + DESIGN digests match |
| `cd workbench && bun test` | exit 0 — 569 pass, 5 skip (pre-existing environment-gated legs: real sshd, `GAUNTLET_T13/T20_REAL_PTY=1`), 0 fail, 7102 expect() calls across 55 files |

`just prereg-check gauntlet-tui-prime-workbench-T22`: PASS (frozen hash match, pre- and post-implementation).

## What the independent adversarial verifier should attack

1. **The corruption audit's authority window**: `auditProjectionRows` treats the sources' retained minted-cursor window as ground truth. Attack with retention-expired or epoch-replaced sources to try to make the audit call legitimate stale state "corrupt" or forged state "valid" (the RECOV-001B residuals declare this boundary).
2. **The naive-rebuild failure's generality**: the failed round uses `rebuildFromAuthoritativeLog` on the SAME file. Try other naive recoveries (open the same store read-write and DELETE rows by hand; rebuild into the corrupt file under a fresh generation and swap generation tables) and check whether any leaves fewer traces than the audit detects.
3. **Determinism of the real-process kill**: the SIGKILL leg spawns `bun -e` and awaits a readiness marker. Verify it cannot hang or flake under load, and that the kill cannot be skipped silently when the marker never arrives (it throws — confirm).
4. **Digest-equality claims**: all "equals the authoritative fold" claims route through T19's canonical JSON (which includes `generation_id` in canonical sections). Try to pass a reconstruction off as equal while generations/rows differ, or to make two different states collide.
5. **The SXR seam**: the matrix consumes probe outcomes as data. Verify a fabricated `SxrAvailabilityRecord` cannot produce a pass (the case cross-checks `fabricated_rows` against the real projection and the exact typed codes), and that a missing record fails the case.
6. **Round-ledger replay**: try to erase or overwrite a failed round across reruns (append-only enforcement; the ledger lives in the report, and every rerun writes new rounds rather than mutating old ones).
7. **Capability-scoped degradation**: try to make Prime withdrawal alter canonical inspection (store digest) or hide Gauntlet-side unit affordances — the case asserts both stay untouched.

Confirmation: BUILDER evidence pending independent adversarial confirmation.

## Correction Round 1 (adversarial verification findings)

Focused adversarial verification returned **CONFIRM_RECOVERY with MAJOR-1** (corruption auditor's undisclosed blind spots) and **MINOR-1** (readiness wait without a deadline), plus three adjudicated informations recorded below as trigger records (not fixed, per the verifier's instruction).

### MAJOR-1 — detection instrument, not recovery mechanism (FIXED, option (a), RED-first)

The verifier proved `auditProjectionRows` audit-SILENT (zero findings) for four adjacent shapes: (a) a DELETED real unit row with intact cursors; (b) a row relabeled to a zombie generation_id (vanishes from generation-scoped reads); (c) an updated_at tamper; (d) focus_projection tamper (focus is part of `canonicalSections` but unaudited). Root cause: the auditor iterated FILE rows only (no expected→file direction) and read only units/evidence/sessions/source_cursors — so a partially-deleted or focus-tampered projection would be silently served until an unrelated trigger forced a rebuild.

**RED-first evidence (verbatim, BEFORE the fix — current auditor, all four shapes):**

```text
(a) deleted-real-unit-row findings=0 []
(b) zombie-generation-relabel findings=0 []
(c) updated-at-tamper findings=0 []
(d) focus-tamper findings=0 []
```

**Fix (implementation, `workbench/src/recovery-matrix.ts`):** the auditor now audits in BOTH directions and reads the full file:

- `readProjectionRows` additionally selects `generation_id` and `updated_at` on units/evidence/sessions, plus the `focus_projection` rows (never read before).
- `auditProjectionRows` takes `expectedGenerationId` (the generation the audited file's rows must be projected under — the live client's minted generation pre-recovery, the rebuild's generation post-recovery) and adds the fold→file direction:
  - every expected unit/evidence/session row must EXIST under the audited generation → new typed finding `expected_row_missing_from_file`; when the identity exists under OTHER generations only, the finding names the relabel (the zombie shape): "…the identity was relabeled to generation(s): gen-zombie".
  - unit/evidence `updated_at` must equal the fold's → new typed finding `row_updated_at_disagrees_with_authoritative_fold`.
  - the focus projection must equal the fold's authoritative focus (kind mapped unit/session/evidence → WorkItem/Agent/Evidence exactly as T19's normalizers map it) → new typed finding `focus_row_disagrees_with_authoritative_fold`; a missing expected focus row is `expected_row_missing_from_file@focus`.
- All existing call sites pin the correct generation (corruption case pre/naive audits → live generation; post-quarantine-rebuild audit → rebuild generation; cold-rebuild Attack 1 identically).

**GREEN evidence (verbatim, AFTER the fix — same four shapes):**

```text
(a) deleted-real-unit-row findings=1
    expected_row_missing_from_file@units:AK-402: the authoritative fold expects units:AK-402 but no row exists under the audited generation 'gen-fixed-a-1' (deleted or lost row)
(b) zombie-generation-relabel findings=1
    expected_row_missing_from_file@units:AK-402: no row for units:AK-402 under the audited generation 'gen-fixed-b-1'; the identity was relabeled to generation(s): gen-zombie
(c) updated-at-tamper findings=1
    row_updated_at_disagrees_with_authoritative_fold@units:AK-401: row updated_at 999999 disagrees with the authoritative fold 1760000000001
(d) focus-tamper findings=1
    focus_row_disagrees_with_authoritative_fold@focus: file focus WorkItem:AK-402 disagrees with the authoritative fold WorkItem:AK-401
```

**Regression tests (RED→GREEN, `workbench/src/recovery-matrix.test.ts`, describe "T22 correction round 1 (adversarial verification findings)"):** one `it` per shape — (a) deleted row (asserts `expected_row_missing_from_file@units:AK-402` with "deleted or lost row", AND that the untouched sibling produces no such finding — precision, not noise), (b) zombie relabel (asserts the relabel detail), (c) updated_at tamper, (d) focus tamper — plus a control `it` proving the UNtampered fixture still audits completely clean under the new fold→file direction (no false-positive noise), and two MINOR-1 tests (below). Each shape test would have been RED against the pre-fix auditor (its assertion message literally reads "AUDIT-SILENT: …" on failure); the RED state is additionally evidenced by the pre-fix probe transcript above, run before any fix landed.

### MINOR-1 — readiness wait deadline (FIXED)

`spawnRealTuiProcess` now bounds the readiness wait (`DEFAULT_READINESS_TIMEOUT_MS = 15_000`, overridable via `readinessTimeoutMs`): a child that hangs without emitting the marker is SIGKILLed and the call throws a typed `RecoveryCaseError` naming the deadline, instead of hanging the suite. The timer is cleared once readiness lands. Regression tests: a marker-less child with a 300 ms deadline is killed and fails (asserts the typed error and the deadline text), and a control pinning the default bound.

### Correction-round gate results (verbatim, after both fixes)

| Gate | Result |
|---|---|
| `just tui-test recovery-matrix` | exit 0 — 20 pass, 0 fail, 182 expect() calls (13 original + 7 correction-round tests) |
| `just tui-test cold-rebuild` | exit 0 — 3 pass, 0 fail, 54 expect() calls |
| `just tui-test detach-reattach` | exit 0 — 3 pass, 0 fail, 55 expect() calls |
| `just tui-check` | exit 0 — `tsc --noEmit -p tsconfig.json` clean |
| `just tui-lint` | exit 0 — `biome check .` clean at error level |
| `just tui-preflight bootstrap` | PASS (exit 0) |
| `cd workbench && bun test` | exit 0 — 576 pass, 5 skip (pre-existing environment-gated legs), 0 fail, 7116 expect() calls across 55 files |

### Trigger records (adjudicated informations — recorded, NOT fixed)

1. **Audit false-positive direction on retention-expired legitimate cursors**: the minted-cursor window is the sources' RETAINED window, so a projection legitimately carrying retention-expired cursors would be flagged as `cursor_not_minted_by_source`. Adjudicated SAFE: the worst case forces a REQ-SYNC-001 fresh-baseline rebuild (the declared recovery for expired history), never a wrong recovery; the boundary is disclosed in the auditor's contract and in the RECOV-001B residuals.
2. **T16's persisted-position "valid" for forged-but-plausible cursors**: `CursorValidation` only rejects epoch mismatch and retention gap, so a persisted position with a plausible-but-forged cursor under the correct epoch validates as "valid". Adjudicated SAFE: the position is noncanonical presentation state and cannot poison the projection — every reconciliation lays the full baseline through the live watermark and re-derives from real events, so the forged cursor influences only reported bookkeeping (`persisted_sequence`/`reconciled_since_cursor`), never row truth.
3. **Surgical hand-repair destroying quarantine evidence**: a hostile actor with write access to the workbench scratch directory could edit or delete the quarantined `.corrupt` file in place (or rebuild over it), destroying the retained failed-round evidence. Adjudicated OUTSIDE product paths (no product code deletes or rewrites quarantined state; the sha256 pin in the round ledger detects tampering after the fact). Recorded as a hostile-actor consideration for whoever owns the evidence directory lifecycle, not a T22 recovery-mechanism defect.

### Updated honest-gaps note (supersedes the corresponding pre-fix gap)

The auditor now covers both directions (file→fold and fold→file), focus, updated_at (units/evidence), and generation pinning. Remaining declared limits: session `updated_at` is not fold-comparable (T19's normalized session record drops it — session existence/identity/provenance IS audited); the fold→file direction pins ONE generation per audit call (rows under other generations are flagged only when an expected identity exists exclusively there — cross-generation rows are deliberately not wholesale-flagged so naive-rebuild files remain meaningfully auditable); the audit is row-logical, not byte-level (page-level corruption invisible to SQL reads is out of scope — SQLite's own integrity checking and the T19 digest over a fresh rebuild remain the compensating instruments).

Confirmation: BUILDER evidence pending independent adversarial confirmation.
