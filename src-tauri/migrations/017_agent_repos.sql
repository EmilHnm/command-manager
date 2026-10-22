BEGIN;

-- Agent Studio projects may be a folder of several git repos (fe/, be/,
-- docs/...). Settings move to each repo; runs and tasks record theirs.
-- '[]' on old rows: a single repo at the project root, with the project's
-- own base_branch / setup_cmd / check_cmds.
ALTER TABLE agent_project ADD COLUMN repos TEXT NOT NULL DEFAULT '[]';
ALTER TABLE agent_run ADD COLUMN repos TEXT NOT NULL DEFAULT '[]';
ALTER TABLE agent_task ADD COLUMN repos TEXT NOT NULL DEFAULT '[]';

UPDATE schema_version SET version = 17;

COMMIT;
