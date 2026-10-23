-- Migration 0005 (sqlite) carried to PostgreSQL: the durable run/iteration ->
-- observation-bundle binding, write-once per (run, iteration), with the bundle
-- content digest. The 0001 table used a different shape that no StateStore
-- code reads or writes. Nothing has shipped against it, so it is replaced.
DROP TABLE IF EXISTS observation_bindings;

CREATE TABLE observation_bindings (
    run_id             TEXT   NOT NULL,
    iteration_no       BIGINT NOT NULL,
    evidence_ref       TEXT   NOT NULL,
    observation_digest TEXT   NOT NULL,
    PRIMARY KEY (run_id, iteration_no)
);
