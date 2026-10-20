BEGIN;

-- Where a command may run: 'local' and/or SSH host ids. No rows = this
-- machine only, so existing commands keep working unchanged.
CREATE TABLE command_target (
  command_id TEXT NOT NULL REFERENCES command_definition(id) ON DELETE CASCADE,
  target TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (command_id, target)
);

-- 014 bound a command to one host; that becomes its only target.
INSERT INTO command_target (command_id, target, sort_order)
  SELECT id, ssh_host_id, 0 FROM command_definition WHERE ssh_host_id IS NOT NULL;
UPDATE command_definition SET ssh_host_id = NULL;

-- A group member may pin one of its command's targets; NULL = default.
ALTER TABLE group_membership ADD COLUMN run_target TEXT;

-- 'local' is not a host, so no foreign key can clean these up.
CREATE TRIGGER ssh_host_targets_cleanup AFTER DELETE ON ssh_host BEGIN
  DELETE FROM command_target WHERE target = OLD.id;
  UPDATE group_membership SET run_target = NULL WHERE run_target = OLD.id;
END;

UPDATE schema_version SET version = 15;

COMMIT;
