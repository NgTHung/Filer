# CORE-037 fixture consolidation

## Plan

Preserve existing test bodies and assertions. Reuse their established Scanner,
FilerCore command/event, Searcher, pipeline, query, and operator boundaries.
This fixture refactor adds no production behavior or test boundary.

1. Record the baseline and start CORE-037.
2. Extract node construction and configurable provider support with the scanner integration suite.
3. Migrate search integration fixtures.
4. Migrate navigation integration fixtures, preserving absent timestamps.
5. Share node construction with the internal harness and migrate internal scanner builders. Keep its paging and stream instrumentation explicit.
6. Migrate internal search setup, retaining delay and failure injection locally.
7. Migrate pipeline, query, and operator builders in separate commits.
8. Compare test inventories, run full core tests and crate checks, and close the task.

Commit each migration after its focused tests pass. Keep each stage below 700
changed lines where practical. Existing tests establish the pre-refactor behavior;
do not add tests that merely restate fixture implementation.

## Baseline

Recorded before fixture edits at `67ffcf18b6a69404da957011c32cd4f44fdd7c9c` (see planning commit parent for the
exact revision). `cargo test -p filer-core` passed: 810 library tests, 38 integration
tests, and 23 doctests. Ten stress tests and 13 doctests were ignored by the
existing configuration. The default-feature test listing has SHA-256
`95a560b1a49562666839a2e1f81d6168b5906d23bffcd46bedcc56f437502c77`.

The table inventories source test declarations, including feature-gated cases.
The compiled default-feature list is also compared before and after migration.

| Source | Test declarations |
| --- | ---: |
| `filer-core/tests/code_preview_test.rs` | 2 |
| `filer-core/tests/command_tracing_test.rs` | 1 |
| `filer-core/tests/large_directory_paging_test.rs` | 1 |
| `filer-core/tests/legacy_identity_absence_test.rs` | 1 |
| `filer-core/tests/navigation_flow_test.rs` | 19 |
| `filer-core/tests/node_entry_platform_test.rs` | 1 |
| `filer-core/tests/optional_features_test.rs` | 1 |
| `filer-core/tests/pipeline_filter_contract_test.rs` | 3 |
| `filer-core/tests/pipeline_paging_parity_test.rs` | 2 |
| `filer-core/tests/scanner_integration_test.rs` | 7 |
| `filer-core/tests/search_integration_test.rs` | 3 |
| `filer-core/src/tests/api/command_api_test.rs` | 3 |
| `filer-core/src/tests/api/command_router_test/route_handshake_emits_session_created.rs` | 16 |
| `filer-core/src/tests/api/command_router_test/route_navigation.rs` | 9 |
| `filer-core/src/tests/api/command_router_test/route_unwatch_session_to_watcher.rs` | 11 |
| `filer-core/src/tests/api/event_sink_test.rs` | 5 |
| `filer-core/src/tests/api/handle_test.rs` | 21 |
| `filer-core/src/tests/infra/actor_test.rs` | 7 |
| `filer-core/src/tests/infra/cancel_map_test.rs` | 2 |
| `filer-core/src/tests/infra/dir_cache_test.rs` | 16 |
| `filer-core/src/tests/infra/error_test.rs` | 17 |
| `filer-core/src/tests/infra/metadata_test/archive_extractor_tests.rs` | 6 |
| `filer-core/src/tests/infra/metadata_test/audio_extractor_tests.rs` | 7 |
| `filer-core/src/tests/infra/metadata_test/code_extractor_tests.rs` | 8 |
| `filer-core/src/tests/infra/metadata_test/document_extractor_tests.rs` | 5 |
| `filer-core/src/tests/infra/metadata_test/image_extractor_tests.rs` | 8 |
| `filer-core/src/tests/infra/metadata_test/registry_tests.rs` | 11 |
| `filer-core/src/tests/infra/metadata_test/video_extractor_tests.rs` | 6 |
| `filer-core/src/tests/infra/mime_test.rs` | 47 |
| `filer-core/src/tests/infra/preview_test.rs` | 14 |
| `filer-core/src/tests/infra/table_test.rs` | 14 |
| `filer-core/src/tests/infra/utils_test.rs` | 20 |
| `filer-core/src/tests/model/cancel_test.rs` | 5 |
| `filer-core/src/tests/model/capability_test.rs` | 10 |
| `filer-core/src/tests/model/directory_test.rs` | 8 |
| `filer-core/src/tests/model/location_test.rs` | 23 |
| `filer-core/src/tests/model/model_test.rs` | 7 |
| `filer-core/src/tests/model/operation_test.rs` | 8 |
| `filer-core/src/tests/model/progress_test.rs` | 4 |
| `filer-core/src/tests/model/query_test.rs` | 48 |
| `filer-core/src/tests/model/registry_test.rs` | 14 |
| `filer-core/src/tests/model/request_test.rs` | 2 |
| `filer-core/src/tests/model/session_manager_test.rs` | 23 |
| `filer-core/src/tests/modules/git_decorations_test.rs` | 14 |
| `filer-core/src/tests/modules/navigator_test/nav_state_serialization_tests.rs` | 4 |
| `filer-core/src/tests/modules/navigator_test/navigator_actor_tests.rs` | 14 |
| `filer-core/src/tests/modules/navigator_test/navigator_multiple_sessions.rs` | 3 |
| `filer-core/src/tests/modules/navigator_test/navigator_state_tests.rs` | 12 |
| `filer-core/src/tests/modules/operator_test/cache_invalidation_tests.rs` | 8 |
| `filer-core/src/tests/modules/operator_test/cancel_tests.rs` | 5 |
| `filer-core/src/tests/modules/operator_test/copy_tests.rs` | 5 |
| `filer-core/src/tests/modules/operator_test/create_file_tests.rs` | 2 |
| `filer-core/src/tests/modules/operator_test/create_folder_tests.rs` | 2 |
| `filer-core/src/tests/modules/operator_test/delete_tests.rs` | 4 |
| `filer-core/src/tests/modules/operator_test/lifecycle_tests.rs` | 2 |
| `filer-core/src/tests/modules/operator_test/location_operation_tests.rs` | 8 |
| `filer-core/src/tests/modules/operator_test/move_tests.rs` | 3 |
| `filer-core/src/tests/modules/operator_test/operator_timeout_tests.rs` | 1 |
| `filer-core/src/tests/modules/operator_test/rename_tests.rs` | 3 |
| `filer-core/src/tests/modules/previewer_test/cache_tests.rs` | 3 |
| `filer-core/src/tests/modules/previewer_test/cancel_tests.rs` | 9 |
| `filer-core/src/tests/modules/previewer_test/clear_cache_tests.rs` | 1 |
| `filer-core/src/tests/modules/previewer_test/lifecycle_tests.rs` | 1 |
| `filer-core/src/tests/modules/previewer_test/metadata_provider_tests.rs` | 1 |
| `filer-core/src/tests/modules/previewer_test/stale_event_tests.rs` | 2 |
| `filer-core/src/tests/modules/scanner_test/mock_provider_tests.rs` | 5 |
| `filer-core/src/tests/modules/scanner_test/ordered_paging_tests.rs` | 6 |
| `filer-core/src/tests/modules/scanner_test/paging_model_tests.rs` | 2 |
| `filer-core/src/tests/modules/scanner_test/paging_session_tests.rs` | 4 |
| `filer-core/src/tests/modules/scanner_test/scan_location_default_emits_directory_entry_page_loaded.rs` | 14 |
| `filer-core/src/tests/modules/scanner_test/scanner_cache_tests.rs` | 6 |
| `filer-core/src/tests/modules/scanner_test/scanner_command_tests.rs` | 4 |
| `filer-core/src/tests/modules/scanner_test/scanner_forwards_listing_options_to_provider.rs` | 16 |
| `filer-core/src/tests/modules/scanner_test/sparse_filter_returns_complete_page_without_empty_intermediate_page.rs` | 11 |
| `filer-core/src/tests/modules/scanner_test/stale_scan_location_result_is_suppressed.rs` | 6 |
| `filer-core/src/tests/modules/scanner_test/streaming_paging_tests.rs` | 8 |
| `filer-core/src/tests/modules/search_test/searcher_basic_tests.rs` | 5 |
| `filer-core/src/tests/modules/search_test/searcher_cancellation_tests.rs` | 8 |
| `filer-core/src/tests/modules/search_test/searcher_error_tests.rs` | 2 |
| `filer-core/src/tests/modules/search_test/searcher_filter_tests.rs` | 11 |
| `filer-core/src/tests/modules/search_test/searcher_hidden_tests.rs` | 3 |
| `filer-core/src/tests/modules/search_test/searcher_lifecycle_tests.rs` | 2 |
| `filer-core/src/tests/modules/search_test/searcher_limit_tests.rs` | 2 |
| `filer-core/src/tests/modules/search_test/searcher_location_tests.rs` | 5 |
| `filer-core/src/tests/modules/search_test/searcher_session_tests.rs` | 1 |
| `filer-core/src/tests/modules/search_test/searcher_timeout_tests.rs` | 1 |
| `filer-core/src/tests/modules/search_test/searcher_traversal_tests.rs` | 3 |
| `filer-core/src/tests/modules/watcher_test/burst_tests.rs` | 1 |
| `filer-core/src/tests/modules/watcher_test.rs` | 16 |
| `filer-core/src/tests/pipeline/pipeline_test/config.rs` | 16 |
| `filer-core/src/tests/pipeline/pipeline_test/filters.rs` | 12 |
| `filer-core/src/tests/pipeline/pipeline_test/grouped_nodes.rs` | 3 |
| `filer-core/src/tests/pipeline/pipeline_test/grouping.rs` | 6 |
| `filer-core/src/tests/pipeline/pipeline_test/paging_mode.rs` | 5 |
| `filer-core/src/tests/pipeline/pipeline_test/pipeline.rs` | 11 |
| `filer-core/src/tests/pipeline/pipeline_test/sorting.rs` | 17 |
| `filer-core/src/tests/vfs/context_test.rs` | 4 |
| `filer-core/src/tests/vfs/listing_stream_test.rs` | 9 |
| `filer-core/src/tests/vfs/provider_registry_test.rs` | 10 |
| `filer-core/src/tests/vfs/provider_secret_test.rs` | 2 |
| `filer-core/src/tests/vfs/vfs_test/context_cancellation_tests.rs` | 3 |
| `filer-core/src/tests/vfs/vfs_test/fixtures.rs` | 27 |
| `filer-core/src/tests/vfs/vfs_test/mock_provider_tests.rs` | 22 |
| `filer-core/src/tests/vfs/vfs_test/segmented_location_tests.rs` | 5 |

## Fixture inventory and intended reuse

- `tests/support/mod.rs` and `src/tests/fixtures.rs` duplicate the base NodeEntry constructor. Share it through a path module, with harness-local crate aliases.
- Scanner integration uses a flat listing, failure switch, and successful-call log.
- Search integration uses directory listings and yields between lists for cancellation.
- Navigation integration uses directory listings, replacement, and successful-call logs. Its entries deliberately have no modified timestamp.
- Internal scanner adds native/fallback paging, listing detail logs, delayed lists, mutable streams, and row-consumption accounting. Share builders; retain this instrumented provider.
- Internal search adds failing paths and delayed lists to the same directory setup. Share basic provider behavior and retain the timing wrapper.
- Pipeline builders select hidden flags or explicit extensions. Query builders select hidden flags and timestamps. Operator builders are equivalent files/directories; its provider records writes and injects operation failures.
- Stress MockFs uses a HashMap and real metadata lookup; LazyTreeFs generates a deep tree lazily. Keep these workload doubles.
- Paging CountingProvider measures listing/page work and generates large directories. Pipeline FixtureProvider compares native paging with fallback. Keep their counters and paging behavior.
- Watcher and git TestWatchProvider doubles drive watch receivers, subscription readiness, and teardown. Keep these event controls.
- VFS MockFs tests stored reads, metadata, and cancellation. NamedProvider exercises provider identities; OpenReaderCountingProvider checks reader routing. DefaultProvider tests default listing-stream fallback. Keep these contract-specific doubles.
- Preview StubProvider and MockPreviewProvider implement preview generation rather than directory listings. HeaderRecordingProvider records bounded reads; NullProvider returns empty listings and path-specific metadata errors; RecordingProvider records preview reads. SequencedPreviewProvider and CleanupInterleavingProvider control cancellation ordering. Keep their distinct behavior.
- Dir-cache and VFS fixture rows already delegate to the base node constructor. Pipeline paging and large-directory rows carry specialized display paths/capabilities or allocation-sensitive construction; retain those explicit fixtures.
