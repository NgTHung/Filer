---
id: "WEB-030"
title: "Decouple identity from the browser cookie with session pairing"
status: Done
priority: "Medium"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015", "WEB-017"]
risk: "Medium"
impact: "Separates the stable user from its browser sessions, adds short-lived pairing PINs, and adds a server-side recovery CLI so identity follows the person instead of one cookie."
tags: ["web", "sessions", "state"]
last_updated: 2026-08-03
---

## Summary

The current identity lives in a single long-lived cookie: the users table owns one session_token per user, so switching browsers loses the identity and every write lands under a fresh name. Separate the stable user from its browser sessions with a sessions table holding many tokens per user. A new browser adopts an existing identity by entering its username and a short-lived, single-use pairing PIN generated from an already-authenticated browser; onboarding shows the pairing path beside first-time creation. The migration turns each existing identity cookie into a session row so attribution history survives. This stays passwordless and attribution-only: no endpoint checks permissions. Listing and revoking sessions is web:WEB-031, which builds on the table this task creates.

A six-digit PIN is safe here because its strength comes from its bounds, not its length: it expires in five minutes, burns on first use, and dies after five wrong guesses. Redemption asks for the username as well as the PIN so a mistyped PIN cannot silently land someone in another person's identity.

Recovery stays server-side because the server owns the database and is self-hosted: a CLI on the server machine mints a fresh session for a username or clears a user's sessions, so a person who loses every cookie (or the pairing PIN) can still regain their identity. There is no self-service recovery secret; whoever can run the CLI controls the server. A fresh session issued this way also lets a user re-pair their other browsers normally.

## Acceptance Criteria

- [x] A sessions table stores many rows per user with a unique token, user FK, created-at, and last-seen; the users table stops owning a single session token.
- [x] Last-seen advances at most once every five minutes per session so an authenticated read does not cost a database write per request.
- [x] A pairing PIN is six uniformly distributed digits, unique among live PINs, expires five minutes after it is minted, and is consumed by its first successful redemption.
- [x] Redeeming a valid username and PIN from a new browser issues that browser its own session for the existing user; expired, already-consumed, and wrong PINs are each rejected with a distinct clear error.
- [x] A PIN is invalidated after five failed redemption attempts, and further attempts against it are rejected as invalid.
- [x] Onboarding offers the pairing path beside first-time creation.
- [x] Renaming a user affects every session of that user.
- [x] Existing identities created before the migration keep working after it, with their old cookie becoming a session row and attribution history intact.
- [x] A server-side CLI command mints a fresh session for a username (printing the session cookie value) and a separate command clears a user's sessions, so a person who lost every cookie can recover their identity without a pairing PIN.
- [x] Tests cover session creation, PIN expiry, single-use semantics, the failed-attempt limit, last-seen coarsening, migration of pre-existing identity cookies, and the recovery CLI minting and clearing sessions.
