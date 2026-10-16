BEGIN;

-- Where a key-based login gets its key: a file path (key_path) or the key
-- text pasted in MOD-17 and kept like a password (ssh_secret 'private_key').
ALTER TABLE ssh_host
  ADD COLUMN key_source TEXT NOT NULL DEFAULT 'file'
  CHECK (key_source IN ('file', 'content'));

-- SQLite cannot widen a CHECK constraint in place.
CREATE TABLE ssh_secret_new (
  host_id TEXT NOT NULL REFERENCES ssh_host(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('password', 'passphrase', 'private_key')),
  nonce BLOB NOT NULL,
  ciphertext BLOB NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (host_id, kind)
);
INSERT INTO ssh_secret_new (host_id, kind, nonce, ciphertext, updated_at)
  SELECT host_id, kind, nonce, ciphertext, updated_at FROM ssh_secret;
DROP TABLE ssh_secret;
ALTER TABLE ssh_secret_new RENAME TO ssh_secret;

UPDATE schema_version SET version = 12;

COMMIT;
