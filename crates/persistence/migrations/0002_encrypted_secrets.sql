CREATE TABLE encrypted_secrets (
  id TEXT PRIMARY KEY,
  ciphertext BLOB NOT NULL,
  nonce BLOB NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE secret_store_metadata (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
