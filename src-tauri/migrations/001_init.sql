CREATE TABLE schema_version (
  version INTEGER NOT NULL
);

CREATE TABLE command_definition (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  execution_string TEXT NOT NULL,
  is_shell INTEGER NOT NULL DEFAULT 0 CHECK (is_shell IN (0, 1))
);

CREATE TABLE command_group (
  id TEXT PRIMARY KEY,
  group_name TEXT NOT NULL,
  autostart INTEGER NOT NULL DEFAULT 0 CHECK (autostart IN (0, 1))
);

CREATE TABLE group_membership (
  group_id TEXT NOT NULL REFERENCES command_group(id) ON DELETE CASCADE,
  command_id TEXT NOT NULL REFERENCES command_definition(id) ON DELETE CASCADE,
  execution_order INTEGER NOT NULL,
  PRIMARY KEY (group_id, command_id),
  UNIQUE (group_id, execution_order)
);

CREATE TABLE run_session (
  id TEXT PRIMARY KEY,
  group_id TEXT NOT NULL REFERENCES command_group(id),
  started_at TEXT NOT NULL,
  status TEXT NOT NULL
);

CREATE TABLE run_event (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES run_session(id),
  command_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  status TEXT NOT NULL,
  exit_code INTEGER,
  pid INTEGER
);

CREATE TABLE app_setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE INDEX idx_run_event_session ON run_event(session_id);
CREATE INDEX idx_run_session_group ON run_session(group_id);

INSERT INTO schema_version (version) VALUES (1);
INSERT INTO app_setting (key, value) VALUES ('ring_buffer_bytes', '2097152');
INSERT INTO app_setting (key, value) VALUES ('shutdown_timeout_secs', '8');
