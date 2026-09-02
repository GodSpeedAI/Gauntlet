// The fixture target project's one source file.
//
// The stable marker below is what fixture unit AK-901 demands and what
// scripts/check.sh verifies: the project's real check command. The seeded
// happy scenario's workspace effect writes exactly this file content into
// the run workspace, so the filesystem observer genuinely observes the
// change the reference demands — whatever the runtime then measures.
pub const GADGET_MARKER_V1: &str = "GADGET_MARKER_V1";

/// The gadget's one behavior: echo the marker back.
pub fn gadget() -> &'static str {
    GADGET_MARKER_V1
}
