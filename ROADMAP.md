# Filer Roadmap
## Phase 1: Core Foundation
- [ ] **Error Types**
  - [ ] Tests: error variants, display, conversion
  - [ ] CoreError implementation

- [ ] **Model Layer**
  - [ ] Tests: NodeId determinism, equality, hash
  - [ ] NodeId implementation
  - [ ] Tests: FileNode creation, is_dir, is_file, extension
  - [ ] FileNode implementation
  - [ ] Tests: NodeRegistry register, resolve, unregister
  - [ ] NodeRegistry implementation

- [ ] **VFS Layer**
  - [ ] Tests: FsProvider trait with MockFs
  - [ ] FsProvider trait
  - [ ] Tests: LocalFs list, read, read_range, exists, metadata
  - [ ] LocalFs implementation

- [ ] **Commands & Events**
  - [ ] Command enum (Navigate, Search, LoadPreview, etc.)
  - [ ] Event enum (DirectoryLoaded, FilesBatch, Error, etc.)

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

- [ ] **Command Router**
  - [ ] Tests: route Navigate to Scanner
  - [ ] Tests: route Search to Searcher
  - [ ] Command routing implementation

## Phase 4: FilerCore API
- [ ] **Handle**
  - [ ] Tests: FilerCore::new() creates actors
  - [ ] Tests: send command
  - [ ] Tests: receive event
  - [ ] Tests: shutdown
  - [ ] FilerCore implementation

- [ ] **Navigation Flow**
  - [ ] Tests: Navigate(path) -> DirectoryLoaded event
  - [ ] Tests: NavigateUp
  - [ ] Tests: Refresh
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
- [ ] **Path Utils**
  - [ ] Tests: get_extension
  - [ ] Tests: get_stem
  - [ ] Tests: is_hidden
  - [ ] Implementation

- [ ] **Size Utils**
  - [ ] Tests: format_size (B, KB, MB, GB)
  - [ ] Tests: parse_size
  - [ ] Implementation

- [ ] **Time Utils**
  - [ ] Tests: format_time
  - [ ] Tests: format_relative
  - [ ] Tests: format_duration
  - [ ] Implementation

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
