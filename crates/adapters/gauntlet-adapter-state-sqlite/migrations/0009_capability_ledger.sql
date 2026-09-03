-- Migration 0009 — the capability ledger (plan Task 22; spec §19.1):
-- the durable developmental-memory observation log and the capability
-- register's transition history. Cross-run by design: the ledger
-- deliberately outlives runs (charter D7).
--
-- Both tables are INSERT-only (the adapter carries no UPDATE or DELETE —
-- idempotency comes from keys, never from rewriting history):
--
-- - capability_observations: the §19.1 left column's append-only log.
--   Idempotent on the declared observation identity
--   (capability, class, run_id, fact) through the UNIQUE constraint; the
--   row payload is the typed record's canonical JSON.
-- - capability_transitions: the register's first-class status
--   transitions — the ONLY status writer the port exposes. The current
--   status of a capability is derived from the latest transition's target
--   (no separately mutable status column exists anywhere).
CREATE TABLE capability_observations (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    capability TEXT    NOT NULL,
    class      TEXT    NOT NULL CHECK (class IN (
        'attempt',
        'failure',
        'correction',
        'comparison',
        'payment',
        'settlement',
        'recovery'
    )),
    run_id     TEXT    NOT NULL,
    fact       TEXT    NOT NULL,
    at         INTEGER NOT NULL,
    payload    TEXT    NOT NULL,
    UNIQUE (capability, class, run_id, fact)
);

CREATE INDEX idx_capability_observations_capability
    ON capability_observations (capability, seq);

CREATE TABLE capability_transitions (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    capability TEXT NOT NULL,
    payload    TEXT NOT NULL
);

CREATE INDEX idx_capability_transitions_capability
    ON capability_transitions (capability, seq);
