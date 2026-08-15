//! Prime-agent harness adapter (spec §4.1, Annex C; plan Task 15): a real
//! `AgentRunner` delegating through the harness's recursive sub-agent
//! mechanism. Delegation returns an admission handle only — results arrive
//! via the result contract (spec §10.3); nothing in this crate may await an
//! answer from the delegation call.
