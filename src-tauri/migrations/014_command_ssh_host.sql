BEGIN;

-- A saved command can run on an SSH host instead of this machine.
ALTER TABLE command_definition
  ADD COLUMN ssh_host_id TEXT REFERENCES ssh_host(id) ON DELETE SET NULL;

UPDATE schema_version SET version = 14;

COMMIT;
