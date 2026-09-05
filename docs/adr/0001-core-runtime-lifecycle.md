---
status: accepted
date: 2026-09-05
---

# Core composition and work lifetime

Core must distinguish replaceable read intent from accepted file operations.
Starting another copy or closing a view must not silently cancel work you already
submitted. We chose startup composition, typed extension commands, bounded
mutation queues, and graceful Session closure to make those guarantees explicit.
This records the accepted design. The implementation tasks below remain open.

## Composition and validation

Register built-in modules and compiled-in extensions before accepting client
commands, then freeze registration. Live replacement of built-in mechanics was
a consequence of the current architecture, not a product requirement. Startup
composition avoids replacing handlers while old actors and captured dependencies
continue running.

Built-in commands retain typed dispatch. Compiled-in extensions expose typed
registration and command handles so callers cannot pair an arbitrary string with
an unrelated payload. Any heterogeneous storage stays inside Core. Session and
request or operation correlation belongs to the shared command contract.
The first extension host remains trusted and compiled in. WASM hosting and
independently installed binaries are outside its scope. This is a scope decision,
not a measured claim about WASM performance.

Reject invalid commands with structured, correlated errors. A successful channel
send alone must not imply that a file operation has been accepted. Acceptance
means Core validated and admitted the operation to a bounded queue with an
operation identity. A full queue produces an explicit busy rejection before
acceptance. Cancellation, queue recovery, and Session closure must remain
available when mutation admission is full. Admission does not guarantee success
or durability across process crashes.

## Read requests

New navigation, preview, or search intent may supersede the previous request
that updates the same view. Scope replacement by purpose and view, not merely
by Session identity. Independent metadata loads and directory continuation pages
must not cancel one another because they share a Session. Clients still reject
stale results by correlation identity; cancellation alone cannot prevent a late
result from arriving.

## File operations

Use a FIFO mutation queue per Session initially. Starting a new operation never
implicitly cancels its predecessor. Explicit cancellation remains available for
individual queued or running operations. Cross-Session conflict scheduling,
parallel mutations within a Session, durable queues, and automatic rollback are
not promises of this decision.

If an operation fails, report its failure and any partial changes, then pause
the remaining mutations in that Session. Other Sessions and unrelated reads
continue. The client explicitly chooses retry, continue, or cancel the remainder.
A queue cannot infer independence: a failed copy followed by a delete could
otherwise remove the only remaining copy. Retry must account for partial changes;
it must not imply an automatic safe replay or rollback.

## Session closure and application exit

Session closure stops admission of new work and cancels disposable reads. All
accepted mutations, including queued ones, continue unless explicitly cancelled
or paused after failure. Recovery and cancellation controls remain available
while a Session is closing. Preserve operation outcomes and Session routing until
the client has received the terminal lifecycle result.

Emit SessionDestroyed only after the Session's accepted work has settled and its
resources have been released. It is a completion signal, not an acknowledgement
that cancellation was requested. Await underlying filesystem work where needed;
joining an outer future alone does not establish that provider activity stopped.
A failed queue remains paused until the client resolves or cancels its remainder.

When the last window closes with pending mutations, offer finish operations and
quit or explicit cancellation. Finishing keeps the process, Core, and its event
consumer alive through completion and cleanup. If a queue fails during this
process, interrupt exit and keep or reopen an operations window for recovery.
Do not use a timeout to silently abandon accepted mutations. Forced process
termination and crash recovery are outside this guarantee.

## Deliberate limits

Restricted-Session authorization is deferred. Input validation remains required.
The supported native execution model uses the OS user's filesystem permissions;
future Core policy may narrow that access but cannot grant missing OS rights or
require root. The existing SessionPolicy types are not evidence of enforced
restricted access. Supporting restricted clients requires provider-aware checks
through every execution route.

Separate Session event streams are deferred. Use one runtime event consumer that
distributes events to clients by Session identity. Cloned receivers currently
compete for messages. Shared backpressure remains an accepted limitation, so
Session state separation does not imply independent event delivery.

Preserve useful abstractions unless measured UX benefits justify changing them.
Current actor and dispatch overhead has not been established as a bottleneck.
Use the public browse measurements and existing benchmark work to evaluate
changes; this decision introduces no arbitrary latency budget or rewrite mandate.

## Implementation ownership

- core:API-018 and API-019 own startup composition and typed extension commands.
- core:REL-007 owns command validation and explicit rejection.
- core:OPS-004 and OPS-005 own bounded FIFO admission and failure recovery.
- core:REL-008 owns Session closure and graceful runtime completion.
- core:REL-009 owns read-request supersession scopes.
- core:REL-010 and REL-011 retain deferred authorization and event isolation.
- app:UI-016 owns graceful exit and failure recovery UI outside UI-011.
- core:MODULES-005 retains the future host; CORE-032 retains browse evidence.

These tasks replace the affected lifecycle assumptions from completed CORE-020
without reopening its historical checklist. The read-only UI-011 validation
track stays active and does not acquire mutation UI or a full application rewrite.
