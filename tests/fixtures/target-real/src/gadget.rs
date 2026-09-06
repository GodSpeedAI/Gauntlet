// The target-real fixture project's one source file.
//
// The stable marker below is what fixture unit AK-951 demands and what
// scripts/check.sh verifies: the project's real check command. The
// deterministic child agent does not need to change this file for the
// seeded units to settle — the units' work is the evidence artifacts the
// child writes at their locators inside the admitted workspace, observed
// by the runtime's own instruments.
pub const GADGET_MARKER_REAL_V1: &str = "GADGET_MARKER_REAL_V1";

/// The gadget's one behavior: echo the marker back.
pub fn gadget() -> &'static str {
    GADGET_MARKER_REAL_V1
}
