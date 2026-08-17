-- Migration 0002 — the §18.1 minimum durable records around the spine.
--
-- The migration set is append-only: these tables are created once and the
-- adapter issues only INSERT and SELECT against them (no UPDATE, no DELETE
-- anywhere in the crate; idempotency comes from INSERT OR IGNORE on
-- content-free keys, never from rewriting rows).
--
-- runs: the authoritative run identity (one frozen reference per run's
-- life, spec §5.3.3). `budget` is canonical JSON (ceilings, reserve,
-- diagnostic allowance); spend lives in checkpoints, which is where §18.2
-- puts it.
CREATE TABLE runs (
    run_id                 TEXT    PRIMARY KEY,
    created_at             INTEGER NOT NULL,
    reference_id           TEXT    NOT NULL,
    reference_version      INTEGER NOT NULL,
    reference_digest       TEXT    NOT NULL,
    intent                 TEXT    NOT NULL,
    config_fingerprint     TEXT    NOT NULL,
    environment_fingerprint TEXT   NOT NULL,
    parent_run             TEXT,
    budget                 TEXT    NOT NULL
);

-- checkpoints: every durable §18.2 checkpoint, kept as history; resume
-- selects the latest by (iteration_no, seq).
CREATE TABLE checkpoints (
    seq              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           TEXT    NOT NULL,
    iteration_no     INTEGER NOT NULL,
    reference_digest TEXT    NOT NULL,
    unit_states      TEXT    NOT NULL,
    budget_spent     TEXT    NOT NULL,
    in_flight        TEXT    NOT NULL,
    event_log_offset INTEGER NOT NULL
);

-- settlements: §16.3 step 2's durable write, keyed exactly as the spec
-- keys replay idempotency — (run_id, unit_id, verification_ref).
CREATE TABLE settlements (
    run_id           TEXT    NOT NULL,
    unit_id          TEXT    NOT NULL,
    verification_ref TEXT    NOT NULL,
    evidence_ref     TEXT    NOT NULL,
    reference_digest TEXT    NOT NULL,
    event_log_offset INTEGER NOT NULL,
    settled_at       INTEGER NOT NULL,
    PRIMARY KEY (run_id, unit_id, verification_ref)
);

-- records: the remaining §18.1 durable records whose typed shapes land with
-- later tasks (observations, comparisons, verification records, residuals,
-- payments, ...). Payloads are validated canonical JSON at admission; the
-- CHECK keeps the kind vocabulary exactly at the spec's minimum list.
CREATE TABLE records (
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
        'escalation'
    )),
    payload TEXT    NOT NULL,
    at      INTEGER NOT NULL
);
