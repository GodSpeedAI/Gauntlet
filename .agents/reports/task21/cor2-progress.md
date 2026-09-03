# Task 21 — COR-2 progress (builder log, declare-first)

Tree: HEAD a71eba9 + staged COR-1 slice (untouched). COR-2 edits land
UNSTAGED on top; nothing is committed or unstaged by this slice.

- D1 (R2-F-1 ticket-id log escaping, 2 loci): STARTED 2026-09-02 —
  failing tests first in `operator_actions_no_bypass.rs` (control-byte
  ticket id in the §20.3 failed line; newline-carrying `--ticket`
  pre-parse forgery), then the sink fixes in `escalations_cmd.rs` +
  `operator_action.rs`.
  DONE 2026-09-02 — both tests RED for the executed expected reasons
  (raw `\u{1b}` inside the §20.3 failed line; forged `operator-action:`
  line present in stderr), then GREEN. Suite: 10/10.
- D2 (R2-F-2 access-log method escaping): STARTED 2026-09-02 — extend
  the existing escape test in `gauntlet-adapter-surface-json/src/http.rs`
  first, then `{method:?}` in `access_log_line`.
  DONE 2026-09-02 — extended test RED (raw ESC+BEL through the method
  field), then GREEN; parser/405/404 semantics untouched.
- D3 (R2-F-3 transient-accept backoff): STARTED 2026-09-02 — timing
  test first in the same module, then the named-constant sleep on the
  transient branch.
  DONE 2026-09-02 — timing test RED with the hot-spin reproduced
  (4 continues in ~40 µs vs the 120 ms floor), then GREEN with
  `TRANSIENT_ACCEPT_BACKOFF` (50 ms, documented at the classifier).
  C3's injected-sequence test still green (+~0.1 s runtime only).
- Gates: STARTED 2026-09-02 — full battery on the final tree under the
  compile lock (one compiler at a time), post-last-edit.
  DONE 2026-09-02 — all five gates green on the final tree: (1)
  operator_actions_no_bypass 10/10 surfaces (re-run after the fmt fix);
  (2) gauntlet-adapter-surface-json 11/11; (3) `cargo build -p
  gauntlet-cli --no-default-features` Finished; (4) e2e_full_loop_mock
  12/12 default + 12/12 surfaces; (5) `just lint` (clippy -D warnings +
  fmt) clean, `just boundaries` 228/228, `just ruler` PASS. One
  intermediate: gate 5's first run failed `cargo fmt --check` on one
  wrapping in a new test; fixed, gate 5 + gate 1 re-run green. Frozen
  artifacts, cli_help snapshots, manifests: diff-clean vs HEAD.

- D1 ADDENDUM (operator-assigned, same contract): the pre-parse `--unit`
  refusal (`escalations_cmd.rs`, the `{unit}` interpolation) + same-
  function sweep. STARTED 2026-09-02 — failing-first tests first (the
  mandated `--unit` control-byte/forged-line test + one for the sweep's
  second in-function finding, the `unknown argument '{other}'` arm),
  then the `{other:?}`/`{unit:?}` sink fixes; out-of-function same-class
  loci (the `escalations` dispatcher and `list` unknown-argument lines)
  RECORDED, not fixed (outside the locus). Gates 1, 2, 5 re-run after.
  DONE 2026-09-02 — both tests RED for the executed expected reasons,
  then GREEN; gate 1 = 12/12, gate 2 = 11/11 (sccache client shutdown
  timeout after the passing run — harmless, recorded), gate 5 = clippy +
  fmt clean, 228/228, ruler PASS (exit 0).

