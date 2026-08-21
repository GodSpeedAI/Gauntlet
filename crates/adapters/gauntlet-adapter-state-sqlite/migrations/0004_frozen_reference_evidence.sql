-- Migration 0004 — preserve the immutable evidence object that binds a
-- frozen ReferenceVersion to its normalized bytes. Existing rows remain NULL:
-- those legacy identities cannot be safely rehydrated and are reported as
-- unavailable, never assigned a synthetic evidence reference.
ALTER TABLE runs ADD COLUMN reference_evidence_ref TEXT;
