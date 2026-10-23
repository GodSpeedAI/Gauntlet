-- E2b: the Gauntlet-owned invocation graph (see .agents/plans/agent-fabric.md,
-- "E2 design"). Additive: one table, its indexes, and an immutability guard.
-- One node per admitted invocation. The graph lives in the same database and
-- transactions as run state. No independent lineage truth exists.

CREATE TABLE invocation_nodes (
    invocation_id       TEXT   PRIMARY KEY,
    run_id              TEXT   NOT NULL,
    parent_id           TEXT   NULL REFERENCES invocation_nodes (invocation_id),
    root_id             TEXT   NOT NULL REFERENCES invocation_nodes (invocation_id) DEFERRABLE INITIALLY DEFERRED,
    depth               BIGINT NOT NULL CHECK (depth >= 0),
    CHECK ((parent_id IS NULL) = (depth = 0)),
    admission_seq       BIGINT GENERATED ALWAYS AS IDENTITY,
    role                TEXT   NOT NULL,
    runner_profile      TEXT   NOT NULL,
    harness             TEXT   NOT NULL,
    workspace           TEXT   NOT NULL,
    context_digest      TEXT   NOT NULL,
    result_contract     TEXT   NOT NULL,
    admission_key       TEXT   NOT NULL,
    budget_rounds       BIGINT NOT NULL CHECK (budget_rounds >= 0),
    budget_wall_seconds BIGINT NOT NULL CHECK (budget_wall_seconds >= 0),
    budget_tokens       BIGINT NOT NULL CHECK (budget_tokens >= 0),
    budget_cost_micro_usd BIGINT NOT NULL CHECK (budget_cost_micro_usd >= 0),
    status              TEXT   NOT NULL CHECK (status IN ('admitted', 'running', 'completed', 'failed', 'timed_out', 'cancelled')),
    failure_class       TEXT   NULL CHECK (failure_class IN ('malformed_result', 'contract_mismatch', 'orphaned_by_restart', 'agent_execution', 'missing_result')),
    CHECK ((status = 'failed') = (failure_class IS NOT NULL)),
    session_ref         TEXT   NULL,
    session_epoch       BIGINT NOT NULL DEFAULT 0 CHECK (session_epoch >= 0),
    CHECK ((session_ref IS NULL) = (session_epoch = 0)),
    result_digest       TEXT   NULL,
    CHECK ((status = 'completed') = (result_digest IS NOT NULL)),
    admitted_at         BIGINT NOT NULL CHECK (admitted_at >= 0),
    terminal_at         BIGINT NULL CHECK (terminal_at IS NULL OR terminal_at >= 0),
    CHECK ((terminal_at IS NULL) = (status IN ('admitted', 'running'))),
    UNIQUE (run_id, admission_key)
);

-- One root per run: a partial unique index over the parentless row.
CREATE UNIQUE INDEX invocation_nodes_one_root_per_run
    ON invocation_nodes (run_id) WHERE parent_id IS NULL;
CREATE INDEX invocation_nodes_parent ON invocation_nodes (parent_id);
CREATE INDEX invocation_nodes_run_status ON invocation_nodes (run_id, status);

-- Immutable facts never change after admission. Only status, failure_class,
-- session_ref, session_epoch, result_digest and terminal_at may be updated.
CREATE FUNCTION invocation_nodes_guard_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.invocation_id IS DISTINCT FROM OLD.invocation_id
        OR NEW.run_id IS DISTINCT FROM OLD.run_id
        OR NEW.parent_id IS DISTINCT FROM OLD.parent_id
        OR NEW.root_id IS DISTINCT FROM OLD.root_id
        OR NEW.depth IS DISTINCT FROM OLD.depth
        OR NEW.admission_seq IS DISTINCT FROM OLD.admission_seq
        OR NEW.role IS DISTINCT FROM OLD.role
        OR NEW.runner_profile IS DISTINCT FROM OLD.runner_profile
        OR NEW.harness IS DISTINCT FROM OLD.harness
        OR NEW.workspace IS DISTINCT FROM OLD.workspace
        OR NEW.context_digest IS DISTINCT FROM OLD.context_digest
        OR NEW.result_contract IS DISTINCT FROM OLD.result_contract
        OR NEW.admission_key IS DISTINCT FROM OLD.admission_key
        OR NEW.budget_rounds IS DISTINCT FROM OLD.budget_rounds
        OR NEW.budget_wall_seconds IS DISTINCT FROM OLD.budget_wall_seconds
        OR NEW.budget_tokens IS DISTINCT FROM OLD.budget_tokens
        OR NEW.budget_cost_micro_usd IS DISTINCT FROM OLD.budget_cost_micro_usd
        OR NEW.admitted_at IS DISTINCT FROM OLD.admitted_at
    THEN
        RAISE EXCEPTION 'invocation_nodes: immutable column changed for invocation %', OLD.invocation_id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER invocation_nodes_immutable
    BEFORE UPDATE ON invocation_nodes
    FOR EACH ROW EXECUTE FUNCTION invocation_nodes_guard_immutable();
