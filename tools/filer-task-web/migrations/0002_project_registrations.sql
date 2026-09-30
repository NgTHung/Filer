CREATE TABLE project_registrations (
    position INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (name <> ''),
    root BLOB NOT NULL UNIQUE
);
