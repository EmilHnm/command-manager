BEGIN;

-- Agent Studio: where a run keeps its worktrees, fixed when it starts so a
-- changed setting never moves a run's checked-out branches. NULL: the app
-- data folder (agents/wt).
ALTER TABLE agent_run ADD COLUMN worktree_root TEXT;

UPDATE schema_version SET version = 23;

COMMIT;
