-- Per-project membership, pending access requests, and a server admin flag.
-- The backfill keeps write access for everyone who has already used a project:
-- the first user becomes admin, each project's latest registrant becomes its
-- owner (the admin when no registration was recorded), and every other user
-- with activity in a project becomes an editor there.

ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1));

CREATE TABLE project_members (
    project    TEXT    NOT NULL REFERENCES project_registrations(name) ON DELETE CASCADE,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    role       TEXT    NOT NULL CHECK (role IN ('owner', 'editor')),
    granted_by INTEGER REFERENCES users(id),
    granted_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (project, user_id)
) STRICT;

CREATE INDEX idx_project_members_user ON project_members(user_id);

CREATE TABLE access_requests (
    project      TEXT    NOT NULL REFERENCES project_registrations(name) ON DELETE CASCADE,
    user_id      INTEGER NOT NULL REFERENCES users(id),
    requested_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (project, user_id)
) STRICT;

CREATE INDEX idx_access_requests_user ON access_requests(user_id);

UPDATE users SET is_admin = 1 WHERE id = (SELECT MIN(id) FROM users);

INSERT INTO project_members (project, user_id, role)
SELECT a.project, a.user_id, 'owner'
FROM activity a
JOIN (
    SELECT activity.project, MAX(activity.id) AS registration_id
    FROM activity
    JOIN users u ON u.id = activity.user_id
    WHERE activity.action = 'project.register'
    GROUP BY activity.project
) latest ON latest.registration_id = a.id
JOIN project_registrations p ON p.name = a.project;

INSERT INTO project_members (project, user_id, role)
SELECT p.name, (SELECT MIN(id) FROM users), 'owner'
FROM project_registrations p
WHERE EXISTS (SELECT 1 FROM users)
  AND NOT EXISTS (SELECT 1 FROM project_members m WHERE m.project = p.name);

INSERT OR IGNORE INTO project_members (project, user_id, role)
SELECT DISTINCT a.project, a.user_id, 'editor'
FROM activity a
JOIN project_registrations p ON p.name = a.project
JOIN users u ON u.id = a.user_id;
