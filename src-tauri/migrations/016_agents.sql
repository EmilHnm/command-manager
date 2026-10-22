BEGIN;

-- SCR-09 Agent Studio. Lists and small structures are JSON text: they are
-- always read and written whole with their row.

-- A folder (git repo) opened in Agent Studio and its settings.
CREATE TABLE agent_project (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  base_branch TEXT,
  setup_cmd TEXT,
  check_cmds TEXT NOT NULL DEFAULT '[]',
  notes TEXT,
  created_at TEXT NOT NULL,
  last_opened_at TEXT
);

-- Per-CLI defaults ("Đội & CLI").
CREATE TABLE agent_cli_profile (
  cli TEXT PRIMARY KEY,
  model TEXT,
  effort TEXT,
  bypass_permissions INTEGER NOT NULL DEFAULT 0,
  extra_args TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE agent_run (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES agent_project(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  request TEXT NOT NULL,
  leader_cli TEXT NOT NULL,
  roles TEXT NOT NULL,
  plan_gate INTEGER NOT NULL DEFAULT 1,
  merge_gate INTEGER NOT NULL DEFAULT 1,
  limits TEXT NOT NULL,
  stage TEXT NOT NULL,
  status TEXT NOT NULL,
  status_note TEXT,
  design_md TEXT,
  contract TEXT,
  leader_session TEXT,
  -- Gate 1 feedback the leader has not worked in yet.
  pending_feedback TEXT,
  base_branch TEXT NOT NULL,
  base_commit TEXT NOT NULL,
  integration_branch TEXT NOT NULL,
  integration_checks TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  finished_at TEXT
);
CREATE INDEX agent_run_project ON agent_run(project_id, created_at);

CREATE TABLE agent_task (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES agent_run(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  title TEXT NOT NULL,
  role TEXT NOT NULL,
  cli TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  acceptance TEXT NOT NULL DEFAULT '[]',
  scope TEXT NOT NULL DEFAULT '[]',
  depends_on TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  status_note TEXT,
  round INTEGER NOT NULL DEFAULT 0,
  worker_session TEXT,
  branch TEXT,
  worktree TEXT,
  checks TEXT,
  last_review TEXT,
  added INTEGER NOT NULL DEFAULT 0,
  removed INTEGER NOT NULL DEFAULT 0,
  out_of_scope TEXT NOT NULL DEFAULT '[]',
  sort_order INTEGER NOT NULL DEFAULT 0,
  UNIQUE (run_id, key)
);

-- One CLI process: a step of the leader or a worker turn.
CREATE TABLE agent_invocation (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES agent_run(id) ON DELETE CASCADE,
  task_id TEXT,
  cli TEXT NOT NULL,
  step TEXT NOT NULL,
  session_id TEXT,
  terminal_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  ok INTEGER,
  error TEXT,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL
);
CREATE INDEX agent_invocation_run ON agent_invocation(run_id, started_at);

UPDATE schema_version SET version = 16;

COMMIT;
