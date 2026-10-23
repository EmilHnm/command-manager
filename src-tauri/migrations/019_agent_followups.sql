BEGIN;

-- Agent Studio: a finished run takes follow-up requests (e.g. comments of a
-- human code review) and goes on with its context. Tasks belong to a batch:
-- 0 for the first request, n for the n-th follow-up.
ALTER TABLE agent_run ADD COLUMN followups TEXT NOT NULL DEFAULT '[]';
ALTER TABLE agent_task ADD COLUMN batch INTEGER NOT NULL DEFAULT 0;
-- What a worker left for the tasks after it: its summary and changed files.
ALTER TABLE agent_task ADD COLUMN handoff TEXT;

-- Model / effort picked for this run (leader) or task (reassigned); NULL:
-- the role's, then the CLI profile's.
ALTER TABLE agent_run ADD COLUMN leader_model TEXT;
ALTER TABLE agent_run ADD COLUMN leader_effort TEXT;
ALTER TABLE agent_task ADD COLUMN model TEXT;
ALTER TABLE agent_task ADD COLUMN effort TEXT;

UPDATE schema_version SET version = 19;

COMMIT;
