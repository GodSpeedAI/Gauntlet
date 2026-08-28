# Task 14 slice B1 — result contract (RECOVERED; see provenance)

**Provenance (honest record):** this slice was dispatched to a builder under
the `.agents/agents/builder.agent.md` charter. The builder session died to the
5-hour sub-agent usage window after landing the code (Items 1–4) but before
writing this report. The orchestrator recovered, verified, and completed the
slice personally per the operator's standing heal+continue instruction
(recorded in `.agents/reports/task14/b1-progress.md`, which the builder DID
keep current as it went — the checkpoint discipline is what made the recovery
cheap). Everything below was re-verified from the tree by the orchestrator.

## What landed

| File | Purpose |
|---|---|
| `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs` | The §8.6 default no-backend `Sandbox`: contained-tier-only posture, allowlist-is-a-finding, foreign-backend cross-wiring refusal, honest `Isolation::None`, identity wrap gated on own clean none-rung validation. + suite factory. |
| `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` | The real bubblewrap backend: live binary probe precedes every policy judgment (`BackendUnavailable` typed), Process rung honestly reported, allowlist refused (no host-allowlist capability — finding, never downgrade), absolute-root enforcement, irreversible refused, mask-then-bind rendering (denied dirs tmpfs'd, denied FILES /dev/null-ro-bound — empirically verified: bwrap aborts tmpfs-over-file), POSIX single-quote escaping, `--die-with-parent --unshare-net` baseline. + suite factory + LIVE confinement test. |
| `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` | Fs `PolicyProvider`: `<policy_dir>/<tier>.json` strict minimal-schema decode (unknown keys/tier-token mismatch/BOM/relative roots/unknown `$TOKEN` all typed `Invalid`), irreversible structurally `NoPolicyForTier` (no file ever read), `$WORKSPACE_ROOT`/`$PROJECT_ROOT`/`$HOME` anchoring, `permissions_for` honestly `Unavailable` (no artifact specified by the frozen ruler — not invented). |
| `.gauntlet/policies/{contained,local,cross_cutting}.json` | The versioned in-repo tier policies (backend bwrap, deny-by-default, `$WORKSPACE_ROOT` writable; local + cross_cutting deny `$PROJECT_ROOT/.git` + credential paths per §17.3). No `irreversible.json` — by design (AK-049's doctor check 5 enforces the absence). |
| `crates/gauntlet-ports/src/contract/sandbox.rs` | +3 shared suite cases: `no_unsandboxed_fallback` (findings-carrying/rung-downgraded validation authorizes no wrap), `wraps_from_process_start` (above none-rung: first process is the backend; at none-rung: above-contained tiers never clean-validate), `capability_mismatch` (none-rung backend clean-validates no tier above contained; every backend refuses irreversible at validate). Suite `bound()` now live wall-clock (fixed epoch was always-expired for clock-reading adapters). |
| `crates/gauntlet-ports/src/contract/stubs.rs` | `SandboxGreen` made an honest none-rung backend; +`SandboxBrokenFallback`, +`SandboxBrokenProcessStart` (one defect each). |
| `crates/gauntlet-ports/src/contract/suite_runs.rs` | +2 teeth: `sandbox_fallback_broken_is_caught`, `sandbox_process_start_broken_is_caught` (intended-case asserted by name). |
| `crates/gauntlet-arch-tests/src/denylist.rs` | Sanctioned growth: `SPEC_VOCABULARY_SEGMENTS` += `"none"` (§6.1 isolation token / §8.2 backend enum / §8.6 default — pre-existing literals, not adapter selection) with pin-test update; suite path fixtures moved to `Path::new("/").join(SEGMENT)` construction. |
| Root `Cargo.toml` | Three new members + workspace deps (alphabetical). |

## Design decisions

1. **Quoting**: `sh -c '<single-quote-escaped command>'` appended as the final
   argv after `--`; everything but `'` is literal inside single quotes, so no
   payload can escape (pinned by adversarial tests incl. embedded `--bind`
   and `; rm -rf /` spellings).
2. **Denial strength**: denied FILEs get a read-only `/dev/null` bind (real
   read-denial — empirically bwrap ABORTS `--tmpfs` over a file); denied DIRs
   get an empty tmpfs; a denied path that does not exist fails sandbox
   STARTUP (fail-closed, never runs unmasked).
3. **Allowlist honesty**: bwrap cannot enforce host allowlists → validation
   finding; shipped policies are all `deny_by_default` (§17.3 allows
   allowlists only when a claim requires it; none does).
4. **Suite factory-awareness** (builder deviation, recorded in progress file):
   the shared `policy()` fixture names the factory's own `backend_identity()`
   and uses absolute roots — otherwise NO honest real adapter could pass
   (cross-wiring defense + absolute-root rule are the adapters' own pinned
   semantics). Existing case names/rules untouched.
5. **The backend half of AK-050** lives here (none-rung backends never
   clean-validate above-contained tiers); the runner-capability half is B2's
   `govern` dispatch check.

## Test inventory (evidence)

- sandbox-none 12/12 · sandbox-bwrap 12/12 · policy-fs 11/11 · ports 107/107
  (from 105; +2 teeth) — all green, `nextest`.
- LIVE confinement EXECUTED under real bwrap 0.11.1 (not skipped):
  write-inside-writable-root succeeded; write-outside failed on the ro root;
  read of the denied FILE failed; the denied DIR presented empty. The
  live tests early-return only on genuine backend absence (absent here).
- The shipped repo policy artifacts decode through `policy_for` for all three
  dispatchable tiers anchored at the repo root (test
  `the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier`).

## Gate results

`just ruler` PASS (before and after) · `just prereg-check 14` PASS ·
`just check` clean · `just test` **1034/1034** · `just lint` clean ·
`just boundaries` **215/215** (no growth — fixtures only inside existing
tests) · `just fmt-check` clean · `just deny` ok.

## Deviations / deferred

- Fixed by recovery: policy-fs fixed-epoch test bound (6 red → green), suite
  `bound()` epoch (bwrap suite red → green), strict-expiry off-by-one,
  clippy collapses, absolute-path literals, `none` segment growth (all
  recorded in `b1-progress.md`).
- `permissions_for` stays honestly `Unavailable` (ruler names no artifact;
  §17.2's resolution is unpersisted scope) — fog for the closure record.
- Runner-capability-vs-tier dispatch check is B2 scope (AK-050's other half).
- Bounded claim: no OTHER unadapted §8.2 backend token (seatbelt/mxc/microvm)
  has an adapter — not faked; they will surface as `BackendUnavailable`-
  class facts in B2's doctor check 4.
