# Filer Roadmap (TDD)

> Tests are written BEFORE implementation for each item.

## Phase 1: Core Foundation
- [x] **Error Types**
  - [x] Tests: error variants, display, conversion
  - [x] CoreError implementation

- [x] **Model Layer**
  - [x] Tests: NodeId determinism, equality, hash
  - [x] NodeId implementation
  - [x] Tests: SessionId equality, hash, generation
  - [x] SessionId implementation
  - [x] Tests: FileNode creation, is_dir, is_file, extension
  - [x] FileNode implementation
  - [x] Tests: NodeRegistry register, resolve, unregister
  - [x] NodeRegistry implementation

- [x] **VFS Layer**
  - [x] Tests: FsProvider trait with MockFs
  - [x] FsProvider trait
  - [x] Tests: LocalFs list, read, read_range, exists, metadata
  - [x] LocalFs implementation

- [ ] **Commands & Events**
  - [ ] Command enum with SessionId (Navigate, Search, LoadPreview, etc.)
  - [ ] Event enum with SessionId (DirectoryLoaded, FilesBatch, Error, etc.)
  - [ ] Session commands (CreateSession, DestroySession)
  - [ ] Session events (SessionCreated, SessionDestroyed)

## Phase 2: Pipeline System
- [ ] **Stage Trait**
  - [ ] Tests: Stage trait with mock stage
  - [ ] Stage trait definition

- [ ] **Filter Stages**
  - [ ] Tests: FilterHidden (show/hide hidden files)
  - [ ] FilterHidden implementation
  - [ ] Tests: FilterByExtension (include/exclude)
  - [ ] FilterByExtension implementation

- [ ] **Sort Stages**
  - [ ] Tests: SortBy name asc/desc
  - [ ] Tests: SortBy size, date
  - [ ] Tests: directories first option
  - [ ] SortBy implementation

- [ ] **Group Stages**
  - [ ] Tests: GroupBy extension, date, size
  - [ ] GroupBy implementation

- [ ] **Pipeline Executor**
  - [ ] Tests: empty pipeline passthrough
  - [ ] Tests: single stage
  - [ ] Tests: chained stages
  - [ ] Pipeline implementation

## Phase 3: Actor Infrastructure
- [ ] **Channels**
  - [ ] Tests: bounded channel send/recv
  - [ ] Tests: unbounded channel
  - [ ] Channel wrapper implementation

- [ ] **Actor Trait**
  - [ ] Actor trait definition
  - [ ] Tests: actor spawn and shutdown

- [ ] **Scanner Actor**
  - [ ] Tests: scan empty directory
  - [ ] Tests: scan directory with files
  - [ ] Tests: scan applies pipeline
  - [ ] Tests: scan cancellation
  - [ ] Tests: scan error handling
  - [ ] Scanner implementation

- [ ] **Navigator Actor**
  - [ ] Tests: navigate updates current directory
  - [ ] Tests: navigate adds to history
  - [ ] Tests: back moves history index
  - [ ] Tests: forward moves history index
  - [ ] Tests: up navigates to parent
  - [ ] Tests: history limit (max entries)
  - [ ] Navigator implementation

- [ ] **Command Router**
  - [ ] Tests: route Navigate to Navigator
  - [ ] Tests: route Search to Searcher
  - [ ] Tests: route by SessionId
  - [ ] Command routing implementation

## Phase 4: FilerCore API
- [ ] **Session Management**
  - [ ] Tests: create session returns SessionId
  - [ ] Tests: destroy session cleans up
  - [ ] Tests: get session by id
  - [ ] Tests: session isolation (events scoped)
  - [ ] Session struct (id, navigator_state, event_tx)
  - [ ] SessionManager implementation

- [ ] **Navigator State**
  - [ ] Tests: NavState snapshot (current, can_back, can_forward)
  - [ ] Tests: sort/filter settings per session
  - [ ] NavigatorState struct

- [ ] **Handle**
  - [ ] Tests: FilerCore::new() creates actors
  - [ ] Tests: send command with session
  - [ ] Tests: receive event with session
  - [ ] Tests: shutdown cleans all sessions
  - [ ] FilerCore implementation

- [ ] **Navigation Flow**
  - [ ] Tests: Navigate(session, path) -> DirectoryLoaded event
  - [ ] Tests: NavigateUp preserves session
  - [ ] Tests: Refresh current directory
  - [ ] Tests: Back/Forward with session
  - [ ] Integration tests

## Phase 5: File Watching
- [ ] **Watcher Actor**
  - [ ] Tests: Watch command
  - [ ] Tests: Unwatch command
  - [ ] Tests: FsChanged event on file create
  - [ ] Tests: FsChanged event on file modify
  - [ ] Tests: FsChanged event on file delete
  - [ ] Tests: debouncing rapid changes
  - [ ] Watcher implementation

## Phase 6: Search
- [ ] **Query Parsing**
  - [ ] Tests: simple text query
  - [ ] Tests: glob pattern (*.rs, test?.txt)
  - [ ] Tests: size filter (size:>1mb, size:<100kb)
  - [ ] Tests: date filter (modified:<1w)
  - [ ] Tests: type filter (type:image)
  - [ ] Tests: combined filters
  - [ ] SearchQuery parser

- [ ] **Searcher Actor**
  - [ ] Tests: search by name
  - [ ] Tests: search recursive
  - [ ] Tests: search with filters
  - [ ] Tests: search cancellation
  - [ ] Tests: search progress events
  - [ ] Tests: search result streaming
  - [ ] Searcher implementation

## Phase 7: MIME & Metadata
- [ ] **MIME Detection**
  - [ ] Tests: detect from extension
  - [ ] Tests: detect from magic bytes
  - [ ] Tests: category classification
  - [ ] MimeDetector implementation

- [ ] **Basic Metadata**
  - [ ] Tests: size, dates, permissions
  - [ ] BasicMetadata implementation

- [ ] **Extended Metadata**
  - [ ] Tests: ImageExtractor (dimensions, EXIF)
  - [ ] ImageExtractor implementation
  - [ ] Tests: AudioExtractor (duration, tags)
  - [ ] AudioExtractor implementation
  - [ ] Tests: VideoExtractor (dimensions, duration)
  - [ ] VideoExtractor implementation
  - [ ] Tests: DocumentExtractor (pages, title)
  - [ ] DocumentExtractor implementation

- [ ] **Metadata Registry**
  - [ ] Tests: register extractor
  - [ ] Tests: extract by category
  - [ ] MetadataRegistry implementation

## Phase 8: Preview System
- [ ] **Preview Provider Trait**
  - [ ] Tests: provider trait with mock
  - [ ] PreviewProvider trait

- [ ] **Built-in Providers**
  - [ ] Tests: TextProvider (content, truncation)
  - [ ] TextProvider implementation
  - [ ] Tests: CodeProvider (syntax highlighting)
  - [ ] CodeProvider implementation
  - [ ] Tests: ImageProvider (thumbnail generation)
  - [ ] ImageProvider implementation

- [ ] **Preview Registry**
  - [ ] Tests: register provider
  - [ ] Tests: select provider by category
  - [ ] PreviewRegistry implementation

- [ ] **Preview Cache**
  - [ ] Tests: cache put/get
  - [ ] Tests: LRU eviction
  - [ ] Tests: cache invalidation
  - [ ] PreviewCache implementation

- [ ] **Previewer Actor**
  - [ ] Tests: generate preview command
  - [ ] Tests: preview ready event
  - [ ] Tests: preview from cache
  - [ ] Previewer implementation

## Phase 9: File Operations
- [ ] **Operations Actor**
  - [ ] Tests: copy single file
  - [ ] Tests: copy directory recursive
  - [ ] Tests: copy progress events
  - [ ] Copy implementation
  - [ ] Tests: move same filesystem
  - [ ] Tests: move cross filesystem
  - [ ] Move implementation
  - [ ] Tests: delete to trash
  - [ ] Tests: delete permanent
  - [ ] Delete implementation
  - [ ] Tests: rename file
  - [ ] Tests: rename directory
  - [ ] Rename implementation
  - [ ] Tests: create folder
  - [ ] Tests: create file
  - [ ] Create implementation

## Phase 10: Caching
- [ ] **Cache Actor**
  - [ ] Tests: cache directory listing
  - [ ] Tests: cache metadata
  - [ ] Tests: invalidate on FsChanged
  - [ ] Tests: LRU eviction by size
  - [ ] Cache implementation

## Phase 11: Archive Support
- [ ] **Archive VFS Provider**
  - [ ] Tests: detect archive type
  - [ ] Tests: list ZIP contents
  - [ ] Tests: read file from ZIP
  - [ ] ZIP implementation
  - [ ] Tests: list TAR contents
  - [ ] Tests: read file from TAR
  - [ ] TAR implementation

## Phase 12: Utils
- [x] **Path Utils**
  - [x] Tests: get_extension
  - [x] Tests: get_stem
  - [x] Tests: is_hidden
  - [x] Implementation

- [x] **Size Utils**
  - [x] Tests: format_size (B, KB, MB, GB)
  - [x] Tests: parse_size
  - [x] Implementation

- [x] **Time Utils**
  - [x] Tests: format_time
  - [x] Tests: format_relative
  - [x] Tests: format_duration
  - [x] Implementation

## Phase 13: GUI (Iced)
- [ ] **Application Structure**
  - [ ] Main window
  - [ ] App state
  - [ ] Message types

- [ ] **Core Integration**
  - [ ] Subscription to core events
  - [ ] Command dispatch
  - [ ] Async state updates

- [ ] **Views**
  - [ ] File list view
  - [ ] Sidebar (places/bookmarks)
  - [ ] Breadcrumb navigation
  - [ ] Preview panel
  - [ ] Search bar
  - [ ] Status bar

- [ ] **Interactions**
  - [ ] Single click selection
  - [ ] Double click open
  - [ ] Multi-selection (Ctrl/Shift)
  - [ ] Keyboard navigation
  - [ ] Context menu
  - [ ] Drag and drop

- [ ] **Performance**
  - [ ] Virtualized list for large directories
  - [ ] Lazy loading
  - [ ] Thumbnail caching

## Phase 14: Optional Features (feature-gated)
- [ ] **Encryption** (\`crypto\` feature)
  - [ ] Tests: encrypt/decrypt roundtrip
  - [ ] Tests: key derivation
  - [ ] Tests: vault create/open
  - [ ] Cipher, KeyStore, Vault implementation

- [ ] **S3** (\`s3\` feature)
  - [ ] Tests: list objects
  - [ ] Tests: get/put object
  - [ ] S3Fs implementation

- [ ] **WebDAV** (\`webdav\` feature)
  - [ ] Tests: PROPFIND, GET, PUT
  - [ ] WebDavFs implementation

- [ ] **FTP/SFTP** (\`ftp\`/\`sftp\` feature)
  - [ ] Tests: connect, list, download, upload
  - [ ] FtpFs implementation

- [ ] **FUSE** (\`fuse\` feature)
  - [ ] Tests: mount/unmount
  - [ ] Tests: FUSE operations
  - [ ] FuseFs implementation

- [ ] **Kubernetes** (\`kubernetes\` feature)
  - [ ] Tests: list namespaces, pods
  - [ ] Tests: get resource YAML
  - [ ] K8sFs implementation

## Phase 15: Ecosystem
- [ ] **Plugin System**
  - [ ] Plugin trait
  - [ ] Plugin registry
  - [ ] Dynamic loading

- [ ] **Themes**
  - [ ] Theme configuration
  - [ ] Icon packs
  - [ ] Dark/light mode

- [ ] **Sync & Backup**
  - [ ] Two-way sync
  - [ ] Incremental backup
  - [ ] Versioning

## Phase 16: Web Support
- [ ] **Transport Layer**
  - [ ] Tests: serialize Command to JSON
  - [ ] Tests: deserialize Event from JSON
  - [ ] Serde implementations for Command/Event
  - [ ] WebSocket server (tokio-tungstenite)
  - [ ] Session lifecycle (connect creates, disconnect destroys)

- [ ] **Protocol**
  - [ ] Tests: request/response correlation
  - [ ] Tests: event streaming
  - [ ] Message framing (request id, payload)
  - [ ] Error responses

- [ ] **Web Client Library** (`filer-web`)
  - [ ] WASM build of shared types
  - [ ] WebSocket client wrapper
  - [ ] Async event subscription
  - [ ] Reconnection handling

- [ ] **Web UI**
  - [ ] Framework choice (Leptos/Dioxus/Yew)
  - [ ] Core integration via WebSocket
  - [ ] File list view
  - [ ] Navigation
  - [ ] Search
  - [ ] Preview panel

## Phase 17: Mobile Support (Future)
- [ ] **React Native / Tauri Mobile**
  - [ ] Evaluate options
  - [ ] Shared core via FFI or WebSocket
  - [ ] Touch-optimized UI
