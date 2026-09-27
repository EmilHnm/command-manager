BEGIN;

CREATE TABLE IF NOT EXISTS command_template (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  template_string TEXT NOT NULL,
  is_shell INTEGER NOT NULL DEFAULT 0 CHECK (is_shell IN (0, 1)),
  last_run_at TEXT
);

CREATE TABLE IF NOT EXISTS template_param (
  template_id TEXT NOT NULL REFERENCES command_template(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  label TEXT NOT NULL,
  kind TEXT NOT NULL,
  default_value TEXT,
  required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
  options_json TEXT,
  is_secret INTEGER NOT NULL DEFAULT 0 CHECK (is_secret IN (0, 1)),
  param_order INTEGER NOT NULL,
  PRIMARY KEY (template_id, name)
);

CREATE TABLE IF NOT EXISTS template_preset (
  id TEXT PRIMARY KEY,
  template_id TEXT NOT NULL REFERENCES command_template(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  values_json TEXT NOT NULL,
  last_used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_template_param_template ON template_param(template_id, param_order);
CREATE INDEX IF NOT EXISTS idx_template_preset_template ON template_preset(template_id);

UPDATE schema_version SET version = 2;

COMMIT;
