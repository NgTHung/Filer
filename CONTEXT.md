# Filer

Filer is a file explorer built around one provider-aware model of files, directories, and user activity. This glossary defines the language shared by core, clients, extensions, tasks, and design documents.

## Product boundaries

**Core**:
The shared authority for file-manager behavior across every Filer client.
_Avoid_: Backend, app core

**Client**:
A user-facing frontend that sends intent to Core and renders the resulting state.
_Avoid_: App when referring to every frontend

**Session**:
An isolated span of user activity for one client context, with its own navigation, view state, and authorization.
_Avoid_: Connection, window

**Session policy**:
The authorization rules that constrain what a Session may do and which Locations it may access.
_Avoid_: Authentication, credentials

## Locations and providers

**Location**:
The provider-aware identity of a file-system target, including targets inside nested layers such as archives. A Location is not limited to a local operating-system path.
_Avoid_: Path, URI, node ID

**Location descriptor**:
The reconstructable identity of a Location, formed from its scheme, Provider, root, and ordered Location segments. Display text does not change this identity.
_Avoid_: Location path, serialized path

**Location reference**:
A reference to a Location that carries its compact identity, its reconstructable descriptor, or both.
_Avoid_: Node reference, path reference

**Location segment**:
An ordered nested boundary within a Location, such as an archive member inside a file.
_Avoid_: Subpath

**Provider**:
The authority that supplies file-system entries and declares which file-manager actions it supports for its Locations.
_Avoid_: Backend, drive

**Provider profile**:
A durable, named Provider identity that can be resolved at runtime without making credentials part of portable state.
_Avoid_: Account, connection, credential

**Ephemeral provider**:
A Provider identity that exists only within a live Session and is not durable by itself.
_Avoid_: Provider profile

**Capability**:
A Provider's declared support and guarantees for an action at a Location.
_Avoid_: Permission

## Entries and directory results

**Node entry**:
An observed file, directory, or symbolic link associated with a Location. It is the canonical item returned by directory and search results.
_Avoid_: File node, path row

**Listing detail**:
The requested balance between listing cost and entry information, either fast identity and type data or fuller metadata.
_Avoid_: Quality, verbosity

**Directory snapshot**:
The result set produced by one directory load. It may be full or explicitly bounded, and its completeness is part of the result.
_Avoid_: Directory cache, page

**Directory page**:
A bounded window of entries in a directory continuation chain, with explicit progress and completeness.
_Avoid_: Snapshot, chunk

**Directory cursor**:
An opaque, transient continuation for the next Directory page in the same chain. A cursor is single-use and does not identify a durable position.
_Avoid_: Offset, bookmark

## Work and extensions

**Task project**:
The version-controlled tasks and policy that record planned work for one project.
_Avoid_: Board, backlog

**Taskroot**:
The library and command-line tool that enforces the Task project contract.
_Avoid_: Filer Task, filer-task

**Request**:
A correlated unit of asynchronous read-side user intent, such as a scan, search, preview, or metadata load.
_Avoid_: File operation, command

**File operation**:
A correlated mutation of file-system state, such as copy, move, delete, rename, or create.
_Avoid_: Request, action

**Accepted file operation**:
A File operation that Core has validated and admitted for execution. Acceptance records an obligation to report its outcome, not a guarantee of success.
_Avoid_: Sent command, completed operation

**Mutation queue**:
The ordered accepted File operations belonging to one Session. It includes work waiting for execution or a recovery decision.
_Avoid_: Request queue, history

**Session closure**:
The end of a Session's activity, completed after its accepted work has settled and its resources have been released.
_Avoid_: Window closure, cancellation request

**Extension**:
An optional contributor of file-manager capability or meaning through Core-owned contracts. Normal file management does not depend on an Extension.
_Avoid_: Core module, plugin when no packaging or isolation is implied

**Semantic output**:
Client-neutral file-manager meaning produced by Core or an Extension, such as a git status or available action.
_Avoid_: Widget, layout, pixel instruction
