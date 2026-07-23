CREATE TABLE activity (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    user_id INTEGER NOT NULL,
    username TEXT NOT NULL CHECK (username <> ''),
    project TEXT NOT NULL CHECK (project <> ''),
    task_id TEXT,
    action TEXT NOT NULL CHECK (action <> ''),
    detail TEXT
) STRICT;

CREATE INDEX idx_activity_project ON activity(project, id DESC);
CREATE INDEX idx_activity_task ON activity(project, task_id, id DESC);
