-- Migration 0004 (sqlite) carried to PostgreSQL: frozen reference evidence
-- reference on the run identity row. Nullable, as in the sqlite schema.
ALTER TABLE runs ADD COLUMN reference_evidence_ref TEXT;

-- The sqlite reference_version is a 64-bit INTEGER and the store reads and
-- writes it as i64 / u64, so the PostgreSQL column is BIGINT (INTEGER is 32-bit).
ALTER TABLE runs ALTER COLUMN reference_version TYPE BIGINT;
