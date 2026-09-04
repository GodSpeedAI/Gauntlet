-- Migration 0010 — the revision ledger (plan Task 23; spec §19.3/§19.4):
-- the durable candidate-revision store. Cross-run by design (charter
-- D3): the ledger deliberately outlives runs — it shares the run state
-- and capability ledger's database file.
--
-- Every table is INSERT-only (the adapter carries no UPDATE or DELETE —
-- idempotency comes from keys, never from rewriting history):
--
-- - revision_proposals: the admitted candidate-revision declarations,
--   idempotent on the declared revision id through the UNIQUE key; the
--   row payload is the typed record's canonical JSON.
-- - revision_evaluations: the isolated-evaluation records, idempotent on
--   the four-run identity through the UNIQUE key.
-- - revision_decisions: the §19.4 gate's first-class decision records —
--   the ONLY decision writer the port exposes. The current decision of a
--   revision is derived from the latest record's verdict (no separately
--   mutable decision column exists anywhere).
-- - revision_restores: the append-only rollback/restore history; the
--   rolled-back fact is the EXISTENCE of a row.
CREATE TABLE revision_proposals (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    revision   TEXT    NOT NULL UNIQUE,
    payload    TEXT    NOT NULL
);

CREATE TABLE revision_evaluations (
    seq                  INTEGER PRIMARY KEY AUTOINCREMENT,
    revision             TEXT    NOT NULL,
    held_out_control     TEXT    NOT NULL,
    held_out_treatment   TEXT    NOT NULL,
    regression_control   TEXT    NOT NULL,
    regression_treatment TEXT    NOT NULL,
    payload              TEXT    NOT NULL,
    UNIQUE (revision, held_out_control, held_out_treatment,
            regression_control, regression_treatment)
);

CREATE INDEX idx_revision_evaluations_revision
    ON revision_evaluations (revision, seq);

CREATE TABLE revision_decisions (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    revision   TEXT    NOT NULL,
    payload    TEXT    NOT NULL
);

CREATE INDEX idx_revision_decisions_revision
    ON revision_decisions (revision, seq);

CREATE TABLE revision_restores (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    revision   TEXT    NOT NULL,
    payload    TEXT    NOT NULL
);

CREATE INDEX idx_revision_restores_revision
    ON revision_restores (revision, seq);
