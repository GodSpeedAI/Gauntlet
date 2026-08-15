//! Generic CLI agent adapter (spec §4.1; plan Task 16): an `AgentRunner`
//! driven entirely by a configured command template with placeholders for
//! prompt file, workspace, result path, and model. No vendor name anywhere in
//! its surface — selection is configuration, not code.
