-- PostgreSQL initial migration (slice E1a)
-- Merged from SQLite migrations 0001-0011, adapted to PostgreSQL.
-- All integers are BIGINT, timestamps are BIGINT epoch seconds,
-- blobs are BYTEA. Slice E1b (state_store recovery tests) deferred.

-- Event log: the durability spine (§6.2, migration 0001)
-- Offsets are managed by event_log_counter table to ensure gapless,
-- strictly increasing sequence without gaps even on rolled-back appends.
CREATE TABLE event_log (
    "offset"      BIGINT    PRIMARY KEY,
    run_id      TEXT      NOT NULL,
    kind        TEXT      NOT NULL,
    payload     TEXT      NOT NULL,
    at          BIGINT    NOT NULL
);

-- Counter table for gapless, serialized event_log offsets.
-- Updated in append transaction: next_offset is incremented atomically,
-- and the new offset is returned via RETURNING clause.
CREATE TABLE event_log_counter (
    id              SMALLINT PRIMARY KEY CHECK (id = 1),
    next_offset     BIGINT   NOT NULL
);

-- Initialize counter to 0 (first offset will be 1 after increment, matching SQLite AUTOINCREMENT)
INSERT INTO event_log_counter (id, next_offset) VALUES (1, 0);

-- Migration tracking table for append-only migrations
CREATE TABLE IF NOT EXISTS schema_migrations (
    version BIGINT PRIMARY KEY,
    name    TEXT   NOT NULL
);

-- §18.1 durable records: run identity (frozen reference per run's life)
CREATE TABLE runs (
    run_id                  TEXT    PRIMARY KEY,
    created_at              BIGINT  NOT NULL,
    reference_id            TEXT    NOT NULL,
    reference_version       INTEGER NOT NULL,
    reference_digest        TEXT    NOT NULL,
    intent                  TEXT    NOT NULL,
    config_fingerprint      TEXT    NOT NULL,
    environment_fingerprint TEXT    NOT NULL,
    parent_run              TEXT,
    budget                  TEXT    NOT NULL
);

-- §18.2 checkpoints: every durable checkpoint, history kept
CREATE TABLE checkpoints (
    seq              BIGSERIAL PRIMARY KEY,
    run_id           TEXT    NOT NULL,
    iteration_no     BIGINT  NOT NULL,
    reference_digest TEXT    NOT NULL,
    unit_states      TEXT    NOT NULL,
    budget_spent     TEXT    NOT NULL,
    in_flight        TEXT    NOT NULL,
    event_log_offset BIGINT  NOT NULL
);

-- §16.3 settlements: keyed exactly as the spec (run_id, unit_id, verification_ref)
CREATE TABLE settlements (
    run_id            TEXT   NOT NULL,
    unit_id           TEXT   NOT NULL,
    verification_ref  TEXT   NOT NULL,
    evidence_ref      TEXT   NOT NULL,
    reference_digest  TEXT   NOT NULL,
    event_log_offset  BIGINT NOT NULL,
    settled_at        BIGINT NOT NULL,
    PRIMARY KEY (run_id, unit_id, verification_ref)
);

-- §18.1 durable records: the remaining kinds
CREATE TABLE records (
    seq         BIGSERIAL PRIMARY KEY,
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
    at      BIGINT  NOT NULL
);

-- Migration 0003: critic bindings indexes
CREATE INDEX records_role_invocation_by_run ON records (run_id, record_type, seq);
CREATE INDEX records_comparison_by_run ON records (run_id, record_type, seq);

-- Migration 0004: frozen reference evidence
CREATE TABLE frozen_reference_evidence (
    run_id      TEXT   NOT NULL,
    reference   TEXT   NOT NULL,
    evidence_id TEXT   NOT NULL,
    PRIMARY KEY (run_id, reference, evidence_id)
);

-- Migration 0005: observation bindings
CREATE TABLE observation_bindings (
    seq            BIGSERIAL PRIMARY KEY,
    run_id         TEXT   NOT NULL,
    observation_id TEXT   NOT NULL,
    evidence_ref   TEXT   NOT NULL,
    unit_id        TEXT   NOT NULL,
    at             BIGINT NOT NULL
);

CREATE INDEX observation_bindings_evidence
    ON observation_bindings (run_id, evidence_ref);
CREATE INDEX observation_bindings_unit
    ON observation_bindings (run_id, unit_id);

-- Migration 0006/0011: evidence productions (final schema with iteration_no in PK)
CREATE TABLE evidence_productions (
    run_id           TEXT   NOT NULL,
    evidence_ref     TEXT   NOT NULL,
    producer_name    TEXT   NOT NULL,
    producer_version TEXT   NOT NULL,
    locator          TEXT   NOT NULL,
    iteration_no     BIGINT NOT NULL,
    produced_at      BIGINT NOT NULL,
    event_log_offset BIGINT NOT NULL,
    content_digest   TEXT   NOT NULL DEFAULT '',
    PRIMARY KEY (run_id, evidence_ref, iteration_no)
);

CREATE INDEX evidence_productions_locator
    ON evidence_productions (run_id, locator, event_log_offset);
CREATE INDEX evidence_productions_ref
    ON evidence_productions (run_id, evidence_ref, event_log_offset);

-- Migration 0007: evidence production content
CREATE TABLE evidence_production_content (
    evidence_id   TEXT   PRIMARY KEY,
    producer_name TEXT   NOT NULL,
    content       BYTEA  NOT NULL,
    produced_at   BIGINT NOT NULL
);

-- Migration 0009: capability ledger (§19.1)
-- Developmental-memory observation log
CREATE TABLE capability_observations (
    seq        BIGSERIAL PRIMARY KEY,
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
    at         BIGINT  NOT NULL,
    payload    TEXT    NOT NULL,
    UNIQUE (capability, class, run_id, fact)
);

CREATE INDEX idx_capability_observations_capability
    ON capability_observations (capability, seq);

-- Capability register transitions
CREATE TABLE capability_transitions (
    seq        BIGSERIAL PRIMARY KEY,
    capability TEXT    NOT NULL,
    payload    TEXT    NOT NULL
);

CREATE INDEX idx_capability_transitions_capability
    ON capability_transitions (capability, seq);

-- Migration 0010: revision ledger (§19.3/§19.4)
-- Candidate-revision proposals
CREATE TABLE revision_proposals (
    seq      BIGSERIAL PRIMARY KEY,
    revision TEXT      NOT NULL UNIQUE,
    payload  TEXT      NOT NULL
);

-- Revision evaluations
CREATE TABLE revision_evaluations (
    seq                  BIGSERIAL PRIMARY KEY,
    revision             TEXT      NOT NULL,
    held_out_control     TEXT      NOT NULL,
    held_out_treatment   TEXT      NOT NULL,
    regression_control   TEXT      NOT NULL,
    regression_treatment TEXT      NOT NULL,
    payload              TEXT      NOT NULL,
    UNIQUE (revision, held_out_control, held_out_treatment,
            regression_control, regression_treatment)
);

CREATE INDEX idx_revision_evaluations_revision
    ON revision_evaluations (revision, seq);

-- Revision decisions
CREATE TABLE revision_decisions (
    seq      BIGSERIAL PRIMARY KEY,
    revision TEXT      NOT NULL,
    payload  TEXT      NOT NULL
);

CREATE INDEX idx_revision_decisions_revision
    ON revision_decisions (revision, seq);

-- Revision restores (rollback history)
CREATE TABLE revision_restores (
    seq      BIGSERIAL PRIMARY KEY,
    revision TEXT      NOT NULL,
    payload  TEXT      NOT NULL
);

CREATE INDEX idx_revision_restores_revision
    ON revision_restores (revision, seq);
