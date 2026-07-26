# Task Mirror Sync

Filer tracks its work in `.tasks/` markdown that only exists inside a git checkout, so anyone without that checkout cannot see the board. This design adds a deployed read-only mirror of the task system: a second filer-task-web instance on a remote host that receives published snapshots and serves the same screens without any write path. The `.tasks/` files stay the only source of truth, the mirror stays a derived copy, and no task content ever flows back from the mirror into a repository.

## Problem

The deployed service has no filesystem access to any checkout. It needs its own copy of task data and something must move changes to it. Publishing cannot assume GitHub, because not every project that feeds the mirror lives there. Several projects publish into one mirror, and each needs its own credential and its own freshness setting.

## Constraints

Task content stays in files. Tasks version with the code, branch with the code, and must remain editable by an agent inside a worktree with no network. Moving them into a central database would break all three, so the mirror is read-only by construction and not by policy.

The mirror is derived state. Deleting its data directory loses nothing that a republish cannot restore. This matches the property WEB-014 already established for the SQLite layer.

A snapshot reflects one branch of one checkout. When tasks move on feature branches the mirror shows whichever branch published last, so a deployment that cares should publish only from its main checkout.

## Approach

The mirror is a filer-task-web running against a synthetic checkout. Each publish sends the raw `.tasks/` files, and the mirror materializes them into a directory it registers as an ordinary project root. Every read route, `validate_repo` call, ready-work computation, and frontend screen then works unchanged, because the mirror sees the same bytes a local checkout would present.

The rejected alternative was publishing parsed task DTOs and serving reads from SQLite. That forces a database-backed twin of every file-backed read route, which duplicates logic across files for no benefit at a few hundred small tasks.

The local daemon publishes. filer-task-web already holds the project registry, already reopens and validates a project on every write, and already owns a database for state. That makes it the natural publisher, and it works for projects with no CI. The snapshot builder still lives in the `filer-task` library so a git hook or CI job can publish without the daemon running.

## Components

### Bundle format in `filer-task`

A new module builds a `TaskBundle` carrying raw file contents rather than parsed tasks: `format_version`, `project_name`, `generated_at`, an optional `source` describing branch and commit for display, a `content_hash`, and a sorted list of relative path and contents pairs. `config.json` travels with the bundle so the mirror enforces the same policy the source project does.

The builder runs `validate_repo` first and returns an error instead of a bundle when the repository has validation errors. A broken checkout can then never overwrite a good mirror. The `content_hash` is a SHA-256 over the sorted path and content pairs, computed with the `sha2` dependency the crate already uses for criterion line hashing. That hash gives publishing an idempotency check that costs nothing to compare.

### `filer-task bundle` command

The command writes a bundle to a file or to stdout. It does not speak HTTP, so `filer-task` gains no client dependency and stays usable as a pure library. Publishing without the daemon is a shell pipeline that posts stdout to the ingest endpoint.

### Publisher in `filer-task-web`

Migration `0005` adds two tables. `mirror_targets` holds the project name, mirror URL, token, an enabled flag, and `poll_seconds`. `publish_state` holds the project name, `last_hash`, `last_published_at`, `last_error`, and a dirty flag.

State is one row per project rather than a queue of pending changes. Every publish sends the complete current snapshot, so a later publish supersedes an earlier one and there is nothing to replay.

Three triggers mark a project dirty. `write::mutate` marks it after recording activity, so web-driven edits publish immediately. A per-project poll ticker marks it when the task tree changed, which is what catches agent and CLI edits the daemon never witnesses. `POST /api/projects/{project}/publish` marks it on demand. Setting `poll_seconds` to zero disables polling for that project.

A background task drains dirty projects. It builds the bundle, compares `content_hash` against `last_hash`, skips the push when they match, and retries with backoff on failure. Publishing never blocks or fails a write. Errors land in `last_error` and surface in the UI rather than propagating to the writer.

The poll ticker stats the task tree and only hashes contents when a modification time or size moved. At the current scale of roughly two hundred files under two kilobytes each, a full hash costs a few milliseconds, so polling stays cheap even without the stat fast path.

Publishing needs an HTTP client, which is the one new dependency: `reqwest` with `rustls-tls` and gzip, default features off. The mirror cannot pull instead, because the publishing daemon binds loopback on a workstation behind NAT.

### Ingest on the mirror

`POST /api/ingest/{project}` authenticates with a per-project bearer token. It rejects an unknown `format_version`, rejects any relative path that escapes the project root or is absolute, writes the tree into `<data_dir>/<project>.incoming`, then swaps it into place atomically and reloads the registry entry. A rejected or failed ingest leaves the previous snapshot serving, so a bad publish degrades to staleness rather than an empty board.

### Read-only mode

The router splits into a shared set of read routes and a write layer the mirror omits. Read-only becomes structural instead of a runtime check repeated across handlers, so a new write route cannot reach the mirror by being forgotten.

`GET /api/capabilities` reports whether the instance accepts writes and when each project last published. The frontend reads it through `static/js/lib/policy.js`, and the New Task screen, drawer field edits, criteria toggles, and palette write commands hide when writes are unavailable. The header shows the publish timestamp per project so a stale board is never mistaken for a live one.

`main.rs` binds `127.0.0.1` today, and the mirror needs a `--bind` option to serve a network. That option is only honored together with `--read-only`, which keeps the existing guarantee that a writable instance never leaves loopback.

## Testing

Bundle tests cover a round-trip through serialization, hash stability when files are supplied in a different order, and the builder refusing a repository with validation errors.

Ingest tests cover rejection of a bad token, an unknown format version, and a traversal path, plus atomicity: a failure partway through materialization must leave the previous snapshot intact and serving.

Router tests assert that every write route is absent in read-only mode and that `/api/capabilities` reports the mode correctly.

Publisher tests cover coalescing repeated dirty marks into a single push, skipping a push when the content hash is unchanged, retrying after a failed push, and the poll loop detecting a file edited outside the daemon.

## Staging

The change is well past the thousand-line guidance for a single landing. Three stages keep each one reviewable and independently useful.

The bundle format and the `bundle` command land first. They are self-contained, fully testable without a network, and immediately allow publishing by shell pipeline.

Ingest and read-only mode land second. Together with stage one they produce a working mirror updated by hand or by a git hook.

The publisher, its tables, and the poll loop land last. They remove the manual step and are the only stage that adds a dependency.
