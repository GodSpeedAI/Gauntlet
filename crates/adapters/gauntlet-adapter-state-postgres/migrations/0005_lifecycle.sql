-- E3a: harness lifecycle observations (liveness and forensics only). Additive:
-- two tables and an append-only guard. Observations never touch the node's
-- status, budget, or native-session columns. See crates/gauntlet-ports/src/lifecycle.rs.

-- The latest applied observation per (invocation, epoch, source). Rows are
-- updated in place by the applying transaction. Rows are never deleted.
CREATE TABLE invocation_lifecycle (
    invocation_id TEXT   NOT NULL REFERENCES invocation_nodes (invocation_id),
    session_epoch BIGINT NOT NULL CHECK (session_epoch >= 0),
    source        TEXT   NOT NULL CHECK (source IN ('structured', 'process', 'screen')),
    last_sequence BIGINT NOT NULL CHECK (last_sequence > 0),
    state         TEXT   NOT NULL CHECK (state IN ('working', 'blocked', 'idle', 'done', 'unknown')),
    origin        TEXT   NOT NULL CHECK (origin IN ('source_provided', 'gauntlet_assigned')),
    observed_at   BIGINT NOT NULL CHECK (observed_at >= 0),
    applied_at    BIGINT NOT NULL CHECK (applied_at >= 0),
    PRIMARY KEY (invocation_id, session_epoch, source)
);

-- Every received observation with its outcome, in receipt order. Append-only.
-- sequence: the number used when applied, the source-provided number otherwise,
-- and NULL for a Gauntlet-assigned observation that consumed no number.
-- outcome_detail: last_applied for ignored_stale, the current epoch for
-- rejected_stale_session (NULL when no session was attached), NULL otherwise.
-- current_session_ref: the current session of a rejected_stale_session outcome.
CREATE TABLE invocation_observations (
    obs_id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    invocation_id       TEXT   NOT NULL REFERENCES invocation_nodes (invocation_id),
    session_ref         TEXT   NULL,
    session_epoch       BIGINT NOT NULL CHECK (session_epoch >= 0),
    CHECK ((session_ref IS NULL) = (session_epoch = 0)),
    source              TEXT   NOT NULL CHECK (source IN ('structured', 'process', 'screen')),
    origin              TEXT   NOT NULL CHECK (origin IN ('source_provided', 'gauntlet_assigned')),
    sequence            BIGINT NULL CHECK (sequence IS NULL OR sequence >= 0),
    CHECK (sequence IS NOT NULL OR (origin = 'gauntlet_assigned' AND outcome IN ('rejected_stale_session', 'ignored_terminal'))),
    state               TEXT   NOT NULL CHECK (state IN ('working', 'blocked', 'idle', 'done', 'unknown')),
    observed_at         BIGINT NOT NULL CHECK (observed_at >= 0),
    received_at         BIGINT NOT NULL CHECK (received_at >= 0),
    outcome             TEXT   NOT NULL CHECK (outcome IN ('applied', 'ignored_stale', 'rejected_stale_session', 'ignored_terminal')),
    outcome_detail      BIGINT NULL,
    current_session_ref TEXT   NULL,
    CHECK (CASE outcome
        WHEN 'ignored_stale' THEN outcome_detail IS NOT NULL AND current_session_ref IS NULL
        WHEN 'rejected_stale_session' THEN (outcome_detail IS NULL) = (current_session_ref IS NULL)
        ELSE outcome_detail IS NULL AND current_session_ref IS NULL
    END)
);

CREATE INDEX invocation_observations_by_node ON invocation_observations (invocation_id, obs_id);

-- Append-only: raw UPDATE or DELETE of a history row is refused.
CREATE FUNCTION invocation_observations_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'invocation_observations is append-only: % is refused', TG_OP;
END;
$$;

CREATE TRIGGER invocation_observations_no_update_or_delete
    BEFORE UPDATE OR DELETE ON invocation_observations
    FOR EACH ROW EXECUTE FUNCTION invocation_observations_append_only();

-- The lifecycle view rows may be advanced in place, never deleted.
CREATE FUNCTION invocation_lifecycle_no_delete() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'invocation_lifecycle rows are never deleted';
END;
$$;

CREATE TRIGGER invocation_lifecycle_no_delete
    BEFORE DELETE ON invocation_lifecycle
    FOR EACH ROW EXECUTE FUNCTION invocation_lifecycle_no_delete();
