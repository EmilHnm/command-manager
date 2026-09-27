BEGIN;

CREATE TABLE command_history_new (
  id TEXT PRIMARY KEY,
  command_line TEXT NOT NULL,
  shell_kind TEXT NOT NULL,
  cwd TEXT,
  last_exit_code INTEGER,
  run_count INTEGER NOT NULL DEFAULT 1,
  first_used_at TEXT NOT NULL,
  last_used_at TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('command', 'template', 'shell', 'typed')),
  UNIQUE (command_line, shell_kind)
);

INSERT INTO command_history_new
  (id, command_line, shell_kind, cwd, last_exit_code, run_count,
   first_used_at, last_used_at, source)
SELECT id, command_line, shell_kind, cwd, last_exit_code, run_count,
       first_used_at, last_used_at, source
FROM command_history;

DROP TABLE command_history;
ALTER TABLE command_history_new RENAME TO command_history;
CREATE INDEX idx_command_history_last_used ON command_history(last_used_at DESC);

INSERT INTO app_setting (key, value) VALUES ('history_enabled', 'true')
  ON CONFLICT(key) DO NOTHING;
INSERT INTO app_setting (key, value) VALUES ('terminal_shell', '')
  ON CONFLICT(key) DO NOTHING;
INSERT INTO app_setting (key, value) VALUES ('ghost_text_enabled', 'true')
  ON CONFLICT(key) DO NOTHING;
INSERT INTO app_setting (key, value) VALUES ('history_max_entries', '5000')
  ON CONFLICT(key) DO NOTHING;
INSERT INTO app_setting (key, value)
VALUES (
  'history_block_patterns',
  'password=
token
-p\S+
authorization:'
)
  ON CONFLICT(key) DO NOTHING;

UPDATE schema_version SET version = 6;

COMMIT;
