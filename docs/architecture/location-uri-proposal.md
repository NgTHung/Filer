# Location URI Proposal

Filer needs a stable string form for `LocationDescriptor` that can survive provider growth, VFS layers, bookmarks, cache keys, transport, and session restoration. This proposal defines a versioned identity URI for machines. It does not replace `display_path`, and it must not become the human-facing path shown in the UI.

The current model already separates identity from display. `LocationDescriptor` owns the provider scheme, provider reference, root path, ordered segments, and optional display path. `LocationId` hashes identity and ignores display. The URI format should preserve that split.

## Goals

- Encode local, profile, ephemeral, archive, mounted, virtual, and extension-backed locations.
- Preserve ordered VFS layers without making archives special.
- Keep provider secrets out of durable strings.
- Produce one canonical string for one identity.
- Round-trip through a typed `LocationDescriptor` without relying on display text.
- Leave room for unknown future segment kinds to round-trip even when the current runtime cannot execute them.

## Non-Goals

- Do not parse normal user-entered paths through this format.
- Do not use this as the default UI display string.
- Do not put credentials, signed URLs, tokens, SSH keys, or temporary auth material in the URI.
- Do not require every runtime to execute every segment kind.

## Recommended Form

Use an opaque, versioned Filer URI:

```text
floc:v1:<provider-kind>:<scheme>:<provider-id>:<encoded-root>!<segment-kind>:<segment-scheme>@<args>:<encoded-path>
```

Provider fields:

```text
<provider-kind> = local | profile | ephemeral
<scheme>        = provider scheme such as file, sftp, s3, webdav, memory
<provider-id>   = profile id, ephemeral id, or - for local
<encoded-root>  = encoded provider path payload
```

Segment fields:

```text
<segment-kind>   = archive | mount | virtual | provider | snapshot | view | extension
<segment-scheme> = implementation scheme, when needed
<args>           = optional key-value arguments after @
<encoded-path>   = encoded path inside that segment layer
```

Simple local path:

```text
floc:v1:local:file:-:%2Fhome%2Fme%2Fproject
```

SFTP profile path:

```text
floc:v1:profile:sftp:work:%2Fhome%2Fme%2Fproject
```

Archive member:

```text
floc:v1:local:file:-:%2Ftmp%2Fbundle.zip!archive::%2Fsrc%2Flib.rs
```

Nested archive:

```text
floc:v1:local:file:-:%2Ftmp%2Fouter.zip!archive::nested.zip!archive::%2Finner.txt
```

Git virtual layer:

```text
floc:v1:local:file:-:%2Frepo!virtual:git@rev=HEAD:%2Fsrc%2Flib.rs
```

Mounted VFS layer:

```text
floc:v1:profile:sftp:work:%2Ffolder%20first%2Ffolder%20foo%2Fa.zip!archive::%2Fb.tar!archive::%2Fdisk.vhdx!mount:fuse@driver=abc,efg=hhh:%2Ffolder%20test%2Fimportain.docx
```

The double colon in `archive::path` means the segment kind is `archive` and the segment has no implementation scheme. That keeps the grammar uniform.

## Argument Syntax

Arguments attach to the segment scheme with `@`:

```text
!mount:fuse@driver=abc,partition=2:%2FUsers
!virtual:git@rev=HEAD:%2Fsrc%2Flib.rs
!extension:thumbnailer@profile=default,size=large:%2Fimage.png
```

Rules:

- `@` separates segment scheme from arguments.
- `,` separates arguments.
- `=` separates key from value.
- Keys use ASCII identifiers: `[A-Za-z_][A-Za-z0-9_-]*`.
- Values are encoded payload strings.
- Arguments are sorted by key during canonical serialization.
- Duplicate keys are invalid.
- Empty argument lists are omitted.
- Secrets are invalid, even if encoded.

These two input forms must serialize to the same canonical URI:

```text
!mount:fuse@partition=2,driver=abc:%2FUsers
!mount:fuse@driver=abc,partition=2:%2FUsers
```

Canonical output:

```text
!mount:fuse@driver=abc,partition=2:%2FUsers
```

## Encoding Rules

Every payload field is encoded. Structural separators stay literal.

Structural separators:

```text
:
!
@
,
=
```

Payload fields:

```text
provider-id
root path
argument values
segment paths
```

The implementation must choose one byte-safe encoding for paths before this becomes a contract. UTF-8 percent encoding is easy, but it cannot represent every valid local path. Filer should prefer byte-safe percent encoding so non-UTF-8 paths can round-trip.

Path normalization is provider-owned. The URI serializer should not collapse `.`, `..`, separators, trailing slashes, or case unless the provider has already defined that identity rule.

## Typed Model

The URI should be derived from typed data, not parsed ad hoc in module code.

The current model can represent archive and virtual segments:

```rust
pub enum LocationSegment {
    ArchiveMember { path: PathBuf },
    Virtual { scheme: String, path: PathBuf },
}
```

The long-term model should make segment arguments first-class:

```rust
pub enum LocationSegment {
    ArchiveMember {
        path: PathBuf,
    },
    Virtual {
        scheme: String,
        args: BTreeMap<String, String>,
        path: PathBuf,
    },
    Mount {
        scheme: String,
        args: BTreeMap<String, String>,
        path: PathBuf,
    },
    Extension {
        scheme: String,
        args: BTreeMap<String, String>,
        path: PathBuf,
    },
    Unknown {
        kind: String,
        scheme: Option<String>,
        args: BTreeMap<String, String>,
        path: PathBuf,
    },
}
```

`Unknown` lets older clients preserve future locations without executing them. Runtime routing can still return structured unsupported-route errors.

## API Shape

Use explicit names so display and identity stay separate:

```rust
impl LocationDescriptor {
    pub fn identity_uri(&self) -> String;
    pub fn display_uri(&self) -> String;
    pub fn from_identity_uri(uri: &str) -> Result<Self, CoreError>;
}
```

Avoid implementing canonical identity through `Display` or `ToString`. Rust callers often expect `Display` to be readable. Identity parsing needs a stricter contract.

## Relationship To LocationId

`LocationId` can stay a compact hash for fast lookup. The identity URI should be the reconstructable identity form.

Use cases:

- `LocationId`: in-memory map keys, compact references, event payloads when a descriptor is already registered.
- `identity_uri`: bookmarks, persisted navigation state, cache keys that need reconstruction, cross-process transport, debugging stable identity.
- `display_path` or `display_uri`: UI text only.

The URI and `LocationId` must use the same identity fields. `display_path` must not affect either.

## Parent Locations

The URI format supports parent derivation through typed `LocationDescriptor` logic:

- For an unsegmented direct path, parent is the provider root parent.
- For a segmented location, parent pops or shortens the last segment.
- For mount and virtual segments, parent is segment-specific and may need provider rules.

Parent behavior should live on the typed model, such as `LocationDescriptor::parent()`, not in string manipulation code.

## Error Handling

Parsing errors should be explicit:

- unsupported URI version
- invalid provider kind
- missing field
- invalid escape sequence
- duplicate argument key
- unsupported segment kind
- secret-like argument rejected by policy
- path bytes cannot be represented on the current platform

Execution errors are separate:

- missing provider profile
- ephemeral provider no longer exists
- segment driver unavailable
- provider does not support a segment kind
- mounted filesystem cannot be opened

Parsing a URI should prove that the identity is well-formed. It should not prove that the runtime can access it.

## Migration Path

1. Add canonical URI tests around the current `LocationDescriptor` fields.
2. Add `identity_uri()` and `from_identity_uri()` without replacing existing APIs.
3. Replace segmented `NodeId` placeholder hashing with `identity_uri()` instead of `display_path`.
4. Add `LocationDescriptor::parent()` and move navigator `Up` behavior toward Location-native state.
5. Add first-class segment args before supporting drivers that need parameters.
6. Keep path and `NodeId` commands as explicit compatibility APIs until all internal authority has moved to `LocationRef`.

## Open Decisions

- Choose the byte-safe path encoding used for local `PathBuf` values on Windows and Unix.
- Decide whether `ProviderRef::Ephemeral` identity URIs may be persisted, or only emitted for runtime diagnostics and session-local transport.
- Decide whether provider ids are encoded as ordinary payload fields or constrained to profile-id syntax.
- Decide whether unknown segment kinds are preserved in the first implementation or deferred until the segment enum grows.
