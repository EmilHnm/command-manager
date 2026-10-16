BEGIN;

-- Keychain (Termius style): keys and identities saved once, reused by hosts.
-- Private key text, passphrases and identity passwords live in
-- ssh_keychain_secret (sealed like ssh_secret) or in the OS keyring.
CREATE TABLE ssh_key (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  algorithm TEXT NOT NULL,
  public_key TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  encrypted INTEGER NOT NULL DEFAULT 0 CHECK (encrypted IN (0, 1)),
  secret_store TEXT NOT NULL CHECK (secret_store IN ('keyring', 'sqlite')),
  created_at TEXT NOT NULL
);

CREATE TABLE ssh_identity (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  username TEXT NOT NULL,
  key_id TEXT REFERENCES ssh_key(id) ON DELETE SET NULL,
  secret_store TEXT NOT NULL DEFAULT 'none'
    CHECK (secret_store IN ('none', 'keyring', 'sqlite')),
  created_at TEXT NOT NULL
);

CREATE TABLE ssh_keychain_secret (
  owner_kind TEXT NOT NULL CHECK (owner_kind IN ('key', 'identity')),
  owner_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('password', 'passphrase', 'private_key')),
  nonce BLOB NOT NULL,
  ciphertext BLOB NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (owner_kind, owner_id, kind)
);

-- No foreign key can point at two tables, so these keep the secret rows
-- from outliving their key or identity.
CREATE TRIGGER ssh_key_secrets_cleanup AFTER DELETE ON ssh_key BEGIN
  DELETE FROM ssh_keychain_secret WHERE owner_kind = 'key' AND owner_id = OLD.id;
END;
CREATE TRIGGER ssh_identity_secrets_cleanup AFTER DELETE ON ssh_identity BEGIN
  DELETE FROM ssh_keychain_secret WHERE owner_kind = 'identity' AND owner_id = OLD.id;
END;

ALTER TABLE ssh_host ADD COLUMN identity_id TEXT REFERENCES ssh_identity(id) ON DELETE SET NULL;
ALTER TABLE ssh_host ADD COLUMN key_id TEXT REFERENCES ssh_key(id) ON DELETE SET NULL;

UPDATE schema_version SET version = 13;

COMMIT;
