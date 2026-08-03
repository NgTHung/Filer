ALTER TABLE users RENAME TO users_legacy;

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    display_name TEXT NOT NULL CHECK (display_name <> ''),
    name_key TEXT NOT NULL UNIQUE CHECK (name_key <> ''),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

INSERT INTO users (id, display_name, name_key, created_at, updated_at)
    SELECT id, display_name, name_key, created_at, updated_at FROM users_legacy;

CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    token TEXT NOT NULL UNIQUE CHECK (length(token) = 64),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX idx_sessions_user ON sessions(user_id);

INSERT INTO sessions (user_id, token, created_at, last_seen)
    SELECT id, session_token, created_at, updated_at FROM users_legacy;

DROP TABLE users_legacy;

CREATE TABLE pairing_pins (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    pin TEXT NOT NULL UNIQUE CHECK (pin GLOB '[0-9][0-9][0-9][0-9][0-9][0-9]'),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0)
) STRICT;

CREATE INDEX idx_pairing_pins_user ON pairing_pins(user_id);

CREATE TABLE pairing_attempts (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    updated_at INTEGER NOT NULL
) STRICT;
