use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::location::{Location, LocationRef};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::preview::previewer::{PreviewCommand, Previewer};
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::{
    PreviewCache, PreviewData, PreviewOptions, PreviewProvider, PreviewRegistry,
};
use crate::vfs::provider::{Capabilities, FsProvider};

const TIMEOUT: Duration = Duration::from_millis(3000);

struct NullProvider;

#[async_trait]
impl FsProvider for NullProvider {
    fn scheme(&self) -> &'static str {
        "null"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: false,
            write: false,
            watch: false,
            search: false,
        }
    }
    async fn list(
        &self,
        _: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::model::node::FileNode>, CoreError> {
        Ok(vec![])
    }
    async fn read(&self, p: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(p.to_path_buf()))
    }
    async fn read_range(
        &self,
        p: &Path,
        _: u64,
        _: u64,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(p.to_path_buf()))
    }
    async fn exists(&self, _: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(false)
    }
    async fn metadata(
        &self,
        p: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::model::node::FileNode, CoreError> {
        Err(CoreError::not_found(p.to_path_buf()))
    }
}

struct RecordingProvider {
    read_header_saw_cancel: Arc<Mutex<bool>>,
    metadata_calls: Arc<Mutex<usize>>,
    block_reads: bool,
}

#[async_trait]
impl FsProvider for RecordingProvider {
    fn scheme(&self) -> &'static str {
        "recording"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(
        &self,
        _: &Path,
        _: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::model::node::FileNode>, CoreError> {
        Ok(vec![])
    }

    async fn read(&self, path: &Path, cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        if self.block_reads {
            cx.race(self.scheme(), std::future::pending::<Result<Vec<u8>, CoreError>>())
                .await
        } else {
            Err(CoreError::not_found(path.to_path_buf()))
        }
    }

    async fn read_range(
        &self,
        path: &Path,
        _: u64,
        _: u64,
        _: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }

    async fn read_header(
        &self,
        _: &Path,
        _: usize,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        *self.read_header_saw_cancel.lock().unwrap() = cx.cancel.is_some();
        if self.block_reads {
            cx.race(self.scheme(), std::future::pending::<Result<Vec<u8>, CoreError>>())
                .await
        } else {
            Ok(b"hello".to_vec())
        }
    }

    async fn exists(&self, _: &Path, _: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        path: &Path,
        _: &crate::ProviderCx<'_>,
    ) -> Result<crate::model::node::FileNode, CoreError> {
        *self.metadata_calls.lock().unwrap() += 1;
        Ok(crate::model::node::FileNode::from_path(
            path.to_path_buf(),
            None,
        )?)
    }
}

#[derive(Clone)]
struct MockPreviewProvider {
    result: PreviewData,
    call_count: Arc<Mutex<usize>>,
    delay_ms: u64,
}

impl MockPreviewProvider {
    fn instant(result: PreviewData) -> Self {
        Self {
            result,
            call_count: Arc::new(Mutex::new(0)),
            delay_ms: 0,
        }
    }

    fn slow(result: PreviewData, delay_ms: u64) -> Self {
        Self {
            result,
            call_count: Arc::new(Mutex::new(0)),
            delay_ms,
        }
    }

    fn calls(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl PreviewProvider for MockPreviewProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    async fn generate(
        &self,
        _path: &Path,
        _mime: &MimeInfo,
        _options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        *self.call_count.lock().unwrap() += 1;
        Ok(self.result.clone())
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

fn text_preview() -> PreviewData {
    PreviewData::Text {
        content: "hello".to_string(),
        truncated: false,
        total_lines: 1,
    }
}

fn spawn_previewer(
    mock: MockPreviewProvider,
    registry: NodeRegistry,
) -> (
    flume::Sender<PreviewCommand>,
    Receiver<Event>,
    Arc<Mutex<PreviewCache>>,
) {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();

    let mut preview_reg = PreviewRegistry::new();
    preview_reg.register(Box::new(mock));
    let preview_reg = Arc::new(preview_reg);

    let cache = Arc::new(Mutex::new(PreviewCache::new(
        64 * 1024 * 1024,
        Duration::from_secs(300),
    )));

    let previewer = Previewer::with_components(
        cmd_rx,
        evt_tx,
        Arc::new(NullProvider),
        registry,
        preview_reg,
        cache.clone(),
    );
    tokio::spawn(async move { previewer.run().await });

    (cmd_tx, evt_rx, cache)
}

fn spawn_previewer_with_provider(
    provider: Arc<dyn FsProvider>,
    preview_reg: Arc<PreviewRegistry>,
    registry: NodeRegistry,
) -> (
    flume::Sender<PreviewCommand>,
    Receiver<Event>,
    Arc<Mutex<PreviewCache>>,
) {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let cache = Arc::new(Mutex::new(PreviewCache::new(
        64 * 1024 * 1024,
        Duration::from_secs(300),
    )));
    let previewer =
        Previewer::with_components(cmd_rx, evt_tx, provider, registry, preview_reg, cache.clone());
    tokio::spawn(async move { previewer.run().await });
    (cmd_tx, evt_rx, cache)
}

async fn wait_for_preview(evt_rx: &Receiver<Event>, session: SessionId) -> Event {
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(e @ Event::PreviewReadyCompat { session: s, .. })) if s == session => return e,
            Ok(Ok(e @ Event::PreviewFailedCompat { session: s, .. })) if s == session => return e,
            Ok(Ok(e @ Event::PreviewReady { session: s, .. })) if s == session => return e,
            Ok(Ok(e @ Event::PreviewFailed { session: s, .. })) if s == session => return e,
            Ok(Ok(e @ Event::Error { session: s, .. })) if s == session => return e,
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed"),
            Err(_) => panic!("timed out waiting for preview event"),
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_previewer_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<PreviewCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(PreviewCache::new(1024, Duration::from_secs(60))));
        let reg = Arc::new(PreviewRegistry::new());

        let previewer = Previewer::with_components(
            cmd_rx,
            evt_tx,
            Arc::new(NullProvider),
            NodeRegistry::new(),
            reg,
            cache,
        );
        let handle = tokio::spawn(async move { previewer.run().await });
        drop(cmd_tx);

        assert!(
            timeout(Duration::from_millis(500), handle).await.is_ok(),
            "Previewer should exit when command channel closes"
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_hit_skips_generation() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/cached.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        // Pre-populate cache
        cache.lock().unwrap().put(path, text_preview());

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert_eq!(
            mock.calls(),
            0,
            "Provider should not be called on cache hit"
        );
    }

    #[tokio::test]
    async fn test_cache_miss_calls_provider() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/uncached.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock.clone(), registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert_eq!(mock.calls(), 1);
    }

    #[tokio::test]
    async fn test_location_preview_cache_hit_skips_provider() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/location-cached.txt");
        let location = Location::local(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        cache.lock().unwrap().put(path, text_preview());

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReady { .. }));
        assert_eq!(
            mock.calls(),
            0,
            "Location preview should reuse direct-path cache entries"
        );
    }
}

#[cfg(test)]
mod cancel_tests {
    use super::*;

    #[tokio::test]
    async fn test_cancel_prevents_event_emission() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/slow.txt");
        let node_id = registry.clone().register(path.clone());

        // Provider takes 200ms — plenty of time to cancel
        let mock = MockPreviewProvider::slow(text_preview(), 200);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        // Collect all events for 300ms — should see nothing for our session
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(e)) => events.push(e),
                _ => break,
            }
        }

        let session_events: Vec<_> = events
            .iter()
            .filter(|e| match e {
                Event::PreviewReadyCompat { session: s, .. }
                | Event::PreviewFailedCompat { session: s, .. } => *s == session,
                _ => false,
            })
            .collect();

        assert!(
            session_events.is_empty(),
            "Cancelled preview should not emit PreviewReadyCompat or PreviewFailedCompat"
        );
    }

    #[tokio::test]
    async fn test_cancel_location_preview_prevents_event_emission() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/slow-location.txt");

        let mock = MockPreviewProvider::slow(text_preview(), 200);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(e)) => events.push(e),
                _ => break,
            }
        }

        let session_events: Vec<_> = events
            .iter()
            .filter(|e| match e {
                Event::PreviewReadyCompat { session: s, .. }
                | Event::PreviewFailedCompat { session: s, .. } => *s == session,
                _ => false,
            })
            .collect();

        assert!(
            session_events.is_empty(),
            "Cancelled Location preview should not emit PreviewReadyCompat or PreviewFailedCompat"
        );
    }

    #[tokio::test]
    async fn test_generate_passes_cancel_context_to_mime_fallback() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/context-preview.txt");
        let node_id = registry.clone().register(path);

        let saw_cancel = Arc::new(Mutex::new(false));
        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: saw_cancel.clone(),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: false,
        });
        let mut preview_reg = PreviewRegistry::new();
        preview_reg.register(Box::new(MockPreviewProvider::instant(text_preview())));
        let (cmd_tx, evt_rx, _cache) =
            spawn_previewer_with_provider(provider, Arc::new(preview_reg), registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert!(
            *saw_cancel.lock().unwrap(),
            "preview MIME fallback must receive a cancel-aware ProviderCx"
        );
    }

    #[tokio::test]
    async fn test_cancel_extended_metadata_interrupts_blocked_provider_read() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/context-metadata.txt");
        let node_id = registry.clone().register(path);

        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: true,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadExtendedMetadata(
                node_id,
                session,
                RequestId::new(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::ExtendedMetadataLoadedCompat { session: s, .. }
                | Event::ExtendedMetadataLoaded { session: s, .. }
                | Event::Error { session: s, .. }
                    if s == session =>
                {
                    panic!("cancelled extended metadata emitted event: {event:?}");
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod metadata_provider_tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_metadata_uses_provider_metadata() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let node_id = registry.clone().register(path);

        let metadata_calls = Arc::new(Mutex::new(0));
        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: metadata_calls.clone(),
            block_reads: false,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadMetadata(
                node_id,
                session,
                RequestId::new(),
            ))
            .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::MetadataLoadedCompat { session: s, .. })) if s == session => break,
                Ok(Ok(Event::Error { session: s, .. })) if s == session => {
                    panic!("metadata load failed")
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => panic!("event channel closed"),
                Err(_) => panic!("timed out waiting for metadata event"),
            }
        }

        assert_eq!(*metadata_calls.lock().unwrap(), 1);
    }
}

#[cfg(test)]
mod stale_event_tests {
    use super::*;

    #[tokio::test]
    async fn test_stale_preview_result_is_suppressed() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/stale-preview.txt");
        let node_id = registry.clone().register(path);

        let mock = MockPreviewProvider::slow(text_preview(), 50);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut ready_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::PreviewReadyCompat {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                ready_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(ready_requests, vec![fresh_request]);
        assert!(
            !ready_requests.contains(&stale_request),
            "stale preview request should not emit PreviewReadyCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_location_preview_result_is_suppressed() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/stale-location-preview.txt");

        let mock = MockPreviewProvider::slow(text_preview(), 50);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut ready_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::PreviewReady {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                ready_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(ready_requests, vec![fresh_request]);
        assert!(
            !ready_requests.contains(&stale_request),
            "stale Location preview request should not emit PreviewReady"
        );
    }
}

#[cfg(test)]
mod clear_cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_clear_cache_causes_cache_miss() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/cached2.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        // Pre-populate cache
        cache.lock().unwrap().put(path, text_preview());

        // Clear the cache
        cmd_tx.send(PreviewCommand::ClearCache).unwrap();
        tokio::task::yield_now().await;

        // Now generate — should miss cache and call provider
        let session2 = SessionId::new();
        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session: session2,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let _ = wait_for_preview(&evt_rx, session2).await;
        assert_eq!(
            mock.calls(),
            1,
            "Provider should be called after cache clear"
        );

        let _ = session; // suppress unused warning
    }
}
