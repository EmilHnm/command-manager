PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE run_session_new (
  id TEXT PRIMARY KEY,
  group_id TEXT REFERENCES command_group(id) ON DELETE SET NULL,
  template_id TEXT REFERENCES command_template(id) ON DELETE SET NULL,
  started_at TEXT NOT NULL,
  status TEXT NOT NULL
);

INSERT INTO run_session_new (id, group_id, template_id, started_at, status)
SELECT id, group_id, template_id, started_at, status FROM run_session;

DROP TABLE run_session;
ALTER TABLE run_session_new RENAME TO run_session;
CREATE INDEX idx_run_session_group ON run_session(group_id);
CREATE INDEX idx_run_session_template ON run_session(template_id);

UPDATE schema_version SET version = 4;

COMMIT;

PRAGMA foreign_keys = ON;
