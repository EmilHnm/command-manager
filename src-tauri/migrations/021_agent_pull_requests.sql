BEGIN;

-- Agent Studio: pull requests opened from gate 2, one per repo, so a
-- follow-up pushes to the same branch and updates them.
ALTER TABLE agent_run ADD COLUMN pull_requests TEXT NOT NULL DEFAULT '[]';

UPDATE schema_version SET version = 21;

COMMIT;
