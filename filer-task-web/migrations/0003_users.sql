CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    session_token TEXT NOT NULL UNIQUE CHECK (length(session_token) = 64),
    display_name TEXT NOT NULL CHECK (display_name <> ''),
    name_key TEXT NOT NULL UNIQUE CHECK (name_key <> ''),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
