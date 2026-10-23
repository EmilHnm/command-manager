BEGIN;

-- Agent Studio, after 019 had shipped (columns are never added to a
-- migration that may have run already).
-- Check commands the leader proposed for repos without any, waiting for the
-- user's approval at gate 1.
ALTER TABLE agent_run ADD COLUMN suggested_checks TEXT NOT NULL DEFAULT '[]';

-- Timeouts follow the project: the limits of its last run start the next
-- one; a heavy task may get its own agent timeout.
ALTER TABLE agent_project ADD COLUMN limits TEXT;
ALTER TABLE agent_task ADD COLUMN timeout_minutes INTEGER;

UPDATE schema_version SET version = 20;

COMMIT;
