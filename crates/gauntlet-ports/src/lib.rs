//! Ports layer (spec §4.1, §6): the stable semantic capabilities core logic
//! depends on — `AgentRunner`, `Observer`, `Comparator`, `Verifier`,
//! `ReferenceProvider`, `PolicyProvider`, `StateStore`, `EvidenceStore`,
//! `EventLog`, `ToolProvider`, `Sandbox`, `WorkspaceProvider`, `BudgetMeter`,
//! `Clock`, `IdGenerator`, `HumanEscalation`, `OperatorSurface`.
//!
//! Ports name capabilities, never products. Adapters implement them; the
//! application layer consumes them; neither direction may invert (spec §24.3).
