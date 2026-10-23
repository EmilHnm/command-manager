BEGIN;

-- Agent Studio: a CLI profile may pick a model / effort per step (lead,
-- work, review). NULL: the built-in defaults for that CLI.
ALTER TABLE agent_cli_profile ADD COLUMN step_models TEXT;

-- An invocation cut short (stopped, timed out, usage limit, app closed):
-- its session is continued instead of starting the step over.
ALTER TABLE agent_invocation ADD COLUMN resumable INTEGER NOT NULL DEFAULT 0;

UPDATE schema_version SET version = 18;

COMMIT;
