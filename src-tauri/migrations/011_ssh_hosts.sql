BEGIN;

CREATE TABLE ssh_host (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
  username TEXT NOT NULL,
  auth_method TEXT NOT NULL DEFAULT 'auto'
    CHECK (auth_method IN ('auto', 'agent', 'key', 'password')),
  key_path TEXT,
  jump_host_id TEXT REFERENCES ssh_host(id) ON DELETE SET NULL,
  use_system_ssh INTEGER NOT NULL DEFAULT 0 CHECK (use_system_ssh IN (0, 1)),
  agent_forwarding INTEGER NOT NULL DEFAULT 0 CHECK (agent_forwarding IN (0, 1)),
  default_dir TEXT,
  secret_store TEXT NOT NULL DEFAULT 'none'
    CHECK (secret_store IN ('none', 'keyring', 'sqlite')),
  color TEXT,
  tags TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  last_connected_at TEXT
);

-- Ciphertext only. The AES key lives in <app_data_dir>/secret.key, outside
-- the database, so a backup alone cannot reveal a password.
CREATE TABLE ssh_secret (
  host_id TEXT NOT NULL REFERENCES ssh_host(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('password', 'passphrase')),
  nonce BLOB NOT NULL,
  ciphertext BLOB NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (host_id, kind)
);

UPDATE schema_version SET version = 11;

COMMIT;
