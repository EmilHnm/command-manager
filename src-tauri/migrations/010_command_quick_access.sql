BEGIN;

ALTER TABLE command_definition
  ADD COLUMN quick_access INTEGER NOT NULL DEFAULT 0 CHECK (quick_access IN (0, 1));

UPDATE schema_version SET version = 10;

COMMIT;
