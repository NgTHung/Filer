# External Task Progress Publish Contract

This contract defines how `filer-task-web` publishes task progress to an external read-only service. The external service is a separate project. It does not run `filer-task-web`, import Rust types, read a checkout, or receive raw `.tasks/` files.

The `.tasks/` files remain the source of truth. `filer-task-web` validates them, converts them into stable progress data, and sends a complete snapshot. The receiver stores and presents that derived state for clients such as a mobile web app.

## Ownership

`filer-task-web` owns:

- validation before publication
- the versioned request and response contract
- conversion from task-domain values to protocol values
- target URL and credential configuration
- change detection, delivery, retry, and delivery status

The external service owns:

- its HTTP route and deployment
- credential verification
- snapshot persistence
- read APIs and user access control
- the JavaScript application and offline cache

Filer does not provide an inbound synchronization route, a remote data directory, a read-only server mode, or receiver storage.

## Transport

The target configuration contains the complete destination URL. The sender makes this request:

```http
PUT <configured-target-url>
Authorization: Bearer <project-token>
Content-Type: application/vnd.filer-task.progress+json
```

Production targets must use HTTPS. Loopback HTTP is allowed for tests. The sender must not put the token in the URL or logs.

One request carries the complete current state of one project. A task absent from a newer accepted snapshot is no longer part of that project. The receiver replaces its current project view instead of merging task records.

Delivery is at least once. The sender allows one in-flight request per project, and the receiver must accept a repeated `content_hash` without creating duplicate state.

## Request

```json
{
  "protocol_version": 1,
  "content_hash": "65f99b96bb8d9d34b09afad08b174f02cab244e32521a5c825e6a4cff6f1dd7e",
  "generated_at": "2026-08-08T10:20:30Z",
  "project": {
    "name": "filer"
  },
  "source": {
    "branch": "main",
    "commit": "b47a6df"
  },
  "tasks": [
    {
      "qualified_id": "web:WEB-022",
      "domain": "web",
      "id": "WEB-022",
      "title": "Publish task progression to an external service",
      "status": "to_do",
      "priority": "high",
      "type": "Epic",
      "parent": null,
      "milestone": null,
      "depends_on": [],
      "tags": ["sync", "web"],
      "last_updated": "2026-08-08",
      "criteria": [
        {
          "checked": false,
          "text": "An external service can consume a versioned progress snapshot."
        }
      ]
    }
  ],
  "milestones": [
    {
      "qualified_id": "milestones:MILESTONE-003",
      "name": "0.3.0",
      "title": "Core contract stabilization",
      "status": "in_progress",
      "done": 42,
      "total": 58,
      "criteria": [
        {
          "checked": false,
          "text": "All required core contracts are stable."
        }
      ]
    }
  ]
}
```

### Envelope fields

| Field | Requirement |
| --- | --- |
| `protocol_version` | Required integer. Version 1 is defined by this document. |
| `content_hash` | Required lowercase SHA-256 hex string over normalized progress content. |
| `generated_at` | Required UTC RFC 3339 timestamp. It describes generation time, not conflict order. |
| `project.name` | Required registered project name. |
| `source` | Optional branch and commit display metadata. |
| `tasks` | Required complete task array. |
| `milestones` | Required milestone progress array. |

### Task fields

Every task includes `qualified_id`, `domain`, `id`, `title`, `status`, `priority`, `type`, `parent`, `milestone`, `depends_on`, `tags`, `last_updated`, and `criteria`.

`status` is one of:

- `to_do`
- `in_progress`
- `blocked`
- `done`
- `deferred`
- `obsolete`

`priority` is one of `high`, `medium`, or `low`. `type` keeps the configured task type name because project policies may define new types.

`parent`, `milestone`, and `last_updated` are nullable. Arrays are present even when empty. Criteria preserve source order because their positions are meaningful. A criterion contains only `checked` and `text`; the receiver cannot mutate it.

The protocol excludes local paths, raw Markdown, arbitrary document sections, write preconditions, validation internals, and Rust-specific DTOs.

### Milestone fields

Each milestone includes its qualified task identity, milestone name, title, status, `done`, `total`, and criteria. Filer computes these aggregates so the receiver does not need to reproduce project policy or milestone rules.

## Stable content hash

The sender normalizes order before hashing:

- tasks by `qualified_id`
- milestones by `qualified_id`
- `depends_on` and `tags` lexicographically
- criteria in source order

The hash projection contains `project`, `source`, `tasks`, and `milestones`. It excludes `protocol_version`, `content_hash`, and `generated_at`. Generating the same progress state twice therefore produces the same hash.

The receiver treats the hash as an opaque snapshot identity and echoes it after acceptance. Transport integrity comes from HTTPS; the receiver does not need to reproduce the hash algorithm.

## Success response

The receiver returns `200 OK` after it has durably accepted the snapshot:

```json
{
  "status": "accepted",
  "content_hash": "65f99b96bb8d9d34b09afad08b174f02cab244e32521a5c825e6a4cff6f1dd7e"
}
```

It may return `"status": "unchanged"` when that hash is already current. The sender records success only when the response contains the submitted hash.

## Error response

Non-success responses use this shape when a response body is available:

```json
{
  "error": {
    "code": "unsupported_protocol_version",
    "message": "protocol_version 1 is not supported"
  }
}
```

Recommended status codes are:

| Status | Meaning |
| --- | --- |
| `400` | Malformed JSON or required field missing. |
| `401` | Publish credential missing or invalid. |
| `403` | Credential cannot publish this project. |
| `413` | Snapshot exceeds the receiver's limit. |
| `422` | Protocol version or field value is unsupported. |
| `429` | Receiver asks the sender to retry later. |
| `5xx` | Receiver could not accept the snapshot. |

The sender retries network failures, `408`, `429`, and `5xx` with bounded backoff and honors `Retry-After`. Other `4xx` responses are persistent delivery errors until configuration or the contract changes. Publishing failure never rolls back a local task write.

## Compatibility

Receivers must ignore unknown object fields. Senders must keep every version 1 required field. Adding an optional field is compatible. Removing or changing a required field, changing hash semantics, or adding a closed enum value requires a new `protocol_version`.

A receiver that does not support the submitted version returns `422` with `unsupported_protocol_version`.

## Conformance

The Filer implementation pins one valid serialized snapshot and representative invalid responses as fixtures. Tests must prove stable ordering, stable hashes, refusal to publish validation errors, response-hash checking, and retry classification. The external project can use the same fixtures without importing Filer code.

## Non-goals

Version 1 does not define mobile writes, bidirectional synchronization, deltas, event streams, remote project deletion, receiver storage, or receiver read APIs. Add those only when a concrete consumer requires them.
