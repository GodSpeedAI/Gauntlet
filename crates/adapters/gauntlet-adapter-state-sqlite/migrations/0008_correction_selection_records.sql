-- Migration 0008 — the controller's §15.1 correction-selection records
-- (plan Task 12, AK-037: "The controller records the selected correction
-- together with rejected alternatives and their reasons").
--
-- The records table's CHECK enumerates the §18.1 minimum durable record
-- kinds; the correction selection is a NEW kind, and SQLite cannot ALTER a
-- CHECK, so the table is rebuilt: identical columns, the CHECK widened by
-- exactly one token, existing rows copied with their seq values preserved
-- (AUTOINCREMENT continues from the preserved maximum), old table dropped,
-- new table renamed. No row is edited, reordered, or lost — the migration
-- set stays append-only and the table's history stays intact.
CREATE TABLE records_0008 (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT    NOT NULL,
    record_type TEXT    NOT NULL CHECK (record_type IN (
        'iteration',
        'role_invocation',
        'reference_version',
        'observation',
        'comparison',
        'verification_record',
        'residual',
        'payment',
        'failure',
        'recovery_event',
        'candidate_revision',
        'promotion_decision',
        'escalation',
        'correction_selection'
    )),
    payload TEXT    NOT NULL,
    at      INTEGER NOT NULL
);

INSERT INTO records_0008 (seq, run_id, record_type, payload, at)
    SELECT seq, run_id, record_type, payload, at FROM records;

DROP TABLE records;

ALTER TABLE records_0008 RENAME TO records;
