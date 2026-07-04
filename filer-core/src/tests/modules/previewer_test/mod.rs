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
    metadata_saw_cancel: Arc<Mutex<bool>>,
    metadata_calls: Arc<Mutex<usize>>,
    block_reads: bool,
    block_metadata: bool,
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
            cx.race(
                self.scheme(),
                std::future::pending::<Result<Vec<u8>, CoreError>>(),
            )
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
            cx.race(
                self.scheme(),
                std::future::pending::<Result<Vec<u8>, CoreError>>(),
            )
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
        cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::model::node::FileNode, CoreError> {
        *self.metadata_saw_cancel.lock().unwrap() = cx.cancel.is_some();
        *self.metadata_calls.lock().unwrap() += 1;
        if self.block_metadata {
            return cx
                .race(
                    self.scheme(),
                    std::future::pending::<Result<crate::model::node::FileNode, CoreError>>(),
                )
                .await;
        }
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
    let previewer = Previewer::with_components(
        cmd_rx,
        evt_tx,
        provider,
        registry,
        preview_reg,
        cache.clone(),
    );
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

include!("lifecycle_tests.rs");

include!("cache_tests.rs");

include!("cancel_tests.rs");

include!("metadata_provider_tests.rs");

include!("stale_event_tests.rs");

include!("clear_cache_tests.rs");
