# Filer Roadmap

## Phase 1: Core Foundation
- [ ] **Model Layer**
  - [ ] FileNode structure
  - [ ] NodeId generation (hash-based)
  - [ ] FileTree implementation
  - [ ] SearchQuery parser

- [ ] **VFS Layer**
  - [ ] FsProvider trait
  - [ ] LocalFs implementation
  - [ ] Basic file operations (list, read, metadata)

- [ ] **Bus & Actors**
  - [ ] Event bus implementation
  - [ ] Actor trait and lifecycle
  - [ ] Scanner actor
  - [ ] Basic command routing

## Phase 2: Core Features
- [ ] **Directory Navigation**
  - [ ] Async directory scanning
  - [ ] Streaming results (batched)
  - [ ] Cancellation support
  - [ ] Error handling

- [ ] **Pipeline System**
  - [ ] Stage trait
  - [ ] FilterHidden stage
  - [ ] FilterByExtension stage
  - [ ] SortBy stage
  - [ ] GroupBy stage

- [ ] **File Watching**
  - [ ] Watcher actor
  - [ ] Change detection events
  - [ ] Debouncing

## Phase 3: Search
- [ ] **Basic Search**
  - [ ] Name matching (glob, regex)
  - [ ] Case sensitivity option
  - [ ] Recursive search

- [ ] **Advanced Search**
  - [ ] Query language parser
  - [ ] Filter by size, date, type
  - [ ] Content search (grep-like)

- [ ] **Searcher Actor**
  - [ ] Background search
  - [ ] Progress reporting
  - [ ] Result streaming

## Phase 4: Metadata & Preview
- [ ] **MIME Detection**
  - [ ] Extension-based detection
  - [ ] Magic bytes detection
  - [ ] Category classification

- [ ] **Metadata Extraction**
  - [ ] BasicMetadata (size, dates, permissions)
  - [ ] ImageExtractor (dimensions, EXIF)
  - [ ] AudioExtractor (duration, tags)
  - [ ] VideoExtractor (dimensions, duration)
  - [ ] DocumentExtractor (PDF, Office)

- [ ] **Preview System**
  - [ ] PreviewProvider trait
  - [ ] TextProvider (plain text)
  - [ ] CodeProvider (syntax highlighting)
  - [ ] ImageProvider (thumbnails)
  - [ ] PreviewCache (LRU)

## Phase 5: Advanced Features
- [ ] **Archive Support**
  - [ ] ArchiveFs provider
  - [ ] ZIP reading
  - [ ] TAR reading
  - [ ] Archive preview

- [ ] **File Operations**
  - [ ] Copy (with progress)
  - [ ] Move
  - [ ] Delete (trash support)
  - [ ] Rename
  - [ ] Create folder

- [ ] **Caching**
  - [ ] Cache actor
  - [ ] Directory cache
  - [ ] Metadata cache
  - [ ] Cache invalidation

## Phase 6: GUI (Iced)
- [ ] **Basic UI**
  - [ ] Main window layout
  - [ ] File list view
  - [ ] Sidebar (places)
  - [ ] Breadcrumb navigation

- [ ] **Core Integration**
  - [ ] Subscription to core events
  - [ ] Command dispatch
  - [ ] Async state updates

- [ ] **Interactions**
  - [ ] Click selection
  - [ ] Double-click open
  - [ ] Keyboard navigation
  - [ ] Context menu

- [ ] **Advanced UI**
  - [ ] Preview panel
  - [ ] Search bar
  - [ ] Status bar
  - [ ] Virtualized list (large dirs)

## Phase 7: Polish
- [ ] **Performance**
  - [ ] Benchmark large directories
  - [ ] Optimize memory usage
  - [ ] Profile hot paths

- [ ] **Testing**
  - [ ] Unit tests for core
  - [ ] Integration tests
  - [ ] Mock filesystem tests

- [ ] **Documentation**
  - [ ] API documentation
  - [ ] Architecture guide
  - [ ] Usage examples

## Phase 8: Encryption
- [ ] **Cipher Module**
  - [ ] AES-256-GCM implementation
  - [ ] ChaCha20-Poly1305 implementation
  - [ ] XChaCha20-Poly1305 implementation
  - [ ] Stream encryption/decryption

- [ ] **Key Management**
  - [ ] Argon2id key derivation
  - [ ] Scrypt key derivation
  - [ ] KeyStore (secure memory)
  - [ ] Key persistence (encrypted)

- [ ] **Vault**
  - [ ] Create/open encrypted vault
  - [ ] Filename encryption
  - [ ] Transparent encryption layer
  - [ ] Password change

## Phase 9: Remote Filesystems
- [ ] **S3 Provider**
  - [ ] Authentication (access key, IAM)
  - [ ] List objects
  - [ ] Get/Put/Delete objects
  - [ ] Multipart upload

- [ ] **WebDAV Provider**
  - [ ] PROPFIND/GET/PUT/DELETE
  - [ ] MKCOL (create directory)
  - [ ] MOVE/COPY
  - [ ] Authentication (Basic, Bearer)

- [ ] **FTP/SFTP Provider**
  - [ ] FTP connection
  - [ ] FTPS (TLS)
  - [ ] SFTP (SSH)
  - [ ] Key-based authentication

## Phase 10: Advanced Integrations
- [ ] **FUSE Support**
  - [ ] Mount filer as filesystem
  - [ ] FUSE operations (lookup, getattr, readdir, read, write)
  - [ ] Auto-unmount

- [ ] **Kubernetes Provider**
  - [ ] Kubeconfig/in-cluster auth
  - [ ] Browse namespaces/pods/resources
  - [ ] View resource YAML
  - [ ] Pod logs
  - [ ] Exec into pod
  - [ ] Copy to/from pod

## Phase 11: Ecosystem
- [ ] **Plugin System**
  - [ ] Plugin trait
  - [ ] Plugin registry
  - [ ] Dynamic loading
  - [ ] Plugin API stability

- [ ] **Themes**
  - [ ] Theme configuration
  - [ ] Custom icon packs
  - [ ] Dark/light mode

- [ ] **Sync & Backup**
  - [ ] Two-way sync
  - [ ] Incremental backup
  - [ ] Versioning
