BEGIN;

ALTER TABLE command_definition ADD COLUMN shell_kind TEXT;

UPDATE schema_version SET version = 8;

COMMIT;
