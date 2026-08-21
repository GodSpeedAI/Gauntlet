-- Migration 0003 — durable Task-10 critic binding lookup.
-- Role invocations and accepted comparisons remain immutable `records` rows;
-- these indexes make restart/recovery lookup by the only safe binding keys
-- (run/kind/append order) explicit without introducing an updateable shadow.
CREATE INDEX records_role_invocation_by_run
    ON records (run_id, record_type, seq);
CREATE INDEX records_comparison_by_run
    ON records (run_id, record_type, seq);
