BEGIN;

ALTER TABLE command_group
  ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'startup'
  CHECK (execution_mode IN ('startup', 'sequential'));

UPDATE schema_version SET version = 9;

COMMIT;
