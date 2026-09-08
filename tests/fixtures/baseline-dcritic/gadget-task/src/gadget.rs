// The baseline-harness fixture project's one source file (task 26).
//
// FIXTURE DATA. The stable marker below is property X — the PRESERVED
// marker the bounded task demands the child keep — and it is what the
// harness's held-out instrument checks alongside the added marker
// (property Y). The project's DECLARED check (scripts/check.sh) never
// inspects this marker: that incompleteness is deliberate and is the
// baseline's measured variable. This file carries property X only; the
// deterministic child agent adds property Y per the scenario mode.
pub const BASELINE_MARKER_PRESERVED_V1: &str = "BASELINE_MARKER_PRESERVED_V1";
