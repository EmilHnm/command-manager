BEGIN;

CREATE TABLE command_history (
  id TEXT PRIMARY KEY,
  command_line TEXT NOT NULL,
  shell_kind TEXT NOT NULL,
  cwd TEXT,
  last_exit_code INTEGER,
  run_count INTEGER NOT NULL DEFAULT 1,
  first_used_at TEXT NOT NULL,
  last_used_at TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('command', 'template')),
  UNIQUE (command_line, shell_kind)
);

CREATE INDEX idx_command_history_last_used ON command_history(last_used_at DESC);

UPDATE schema_version SET version = 5;

COMMIT;
