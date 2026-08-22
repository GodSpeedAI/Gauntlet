-- Migration 0006 — the durable run -> evidence-production binding (plan
-- Task 11, spec 14.3). The EvidenceStore is content-addressed bytes with no
-- metadata; producer, locator, and recency facts therefore live here, in a
-- record the evidence owner wrote.
--
-- The primary key makes the record write-once per (run, evidence ref): a
-- second record naming DIFFERENT facts for the same ref is a constraint the
-- adapter reports, never an overwrite. Multiple refs MAY be recorded at one
-- locator across iterations (content addressing assigns a new ref to new
-- bytes); the verification chain orders them by the event-log offset of the
-- EvidenceProduced event each record's append minted -- the offset column is
-- that server-minted anchor, not a caller-supplied value.
CREATE TABLE evidence_productions (
    run_id           TEXT NOT NULL,
    evidence_ref     TEXT NOT NULL,
    producer_name    TEXT NOT NULL,
    producer_version TEXT NOT NULL,
    locator          TEXT NOT NULL,
    iteration_no     INTEGER NOT NULL,
    produced_at      INTEGER NOT NULL,
    event_log_offset INTEGER NOT NULL,
    PRIMARY KEY (run_id, evidence_ref)
);
CREATE INDEX evidence_productions_locator
    ON evidence_productions (run_id, locator, event_log_offset);
