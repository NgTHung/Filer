use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use filer_core::model::session::SessionId;
use filer_core::modules::operations::OperationsModule;
use filer_core::{
    Capabilities, Command, CoreError, Event, FilerCore, FsProvider, LocalFs, Location, LocationRef,
    NodeEntry, OperationId, ProviderCx, RequestId,
};
use tokio::sync::Barrier;
use tokio::time::timeout;

pub const TIMEOUT: Duration = Duration::from_secs(5);

pub struct CopyGate {
    local: LocalFs,
    pub entered: Barrier,
    pub release: Barrier,
    failures: AtomicUsize,
    pub deletes: AtomicUsize,
    pub native: Option<NativeCopyGate>,
}

pub struct NativeCopyGate {
    pub queued: tokio::sync::Notify,
    pub release: flume::Receiver<()>,
}

impl CopyGate {
    pub fn new(failures: usize) -> Self {
        Self {
            local: LocalFs::new(),
            entered: Barrier::new(2),
            release: Barrier::new(2),
            failures: AtomicUsize::new(failures),
            deletes: AtomicUsize::new(0),
            native: None,
        }
    }
}

#[async_trait::async_trait]
impl FsProvider for CopyGate {
    fn scheme(&self) -> &'static str {
        self.local.scheme()
    }

    fn capabilities(&self) -> Capabilities {
        self.local.capabilities()
    }

    async fn list(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        self.local.list(path, cx).await
    }

    async fn metadata(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        self.local.metadata(path, cx).await
    }

    async fn exists(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        self.local.exists(path, cx).await
    }

    async fn read(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        self.local.read(path, cx).await
    }

    async fn read_range(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        self.local.read_range(path, offset, length, cx).await
    }

    async fn preflight_transfer(
        &self,
        src: &Path,
        dst: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<(), CoreError> {
        self.local.preflight_transfer(src, dst, cx).await
    }

    fn mutation_futures_can_be_dropped(&self) -> bool {
        self.native.is_none() || self.local.mutation_futures_can_be_dropped()
    }

    async fn copy(&self, src: &Path, dst: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        if let Some(gate) = &self.native {
            let release = gate.release.clone();
            tokio::task::spawn_blocking(move || {
                let _ = release.recv();
            });
            let copy = self.local.copy(src, dst, cx);
            tokio::pin!(copy);
            std::future::poll_fn(|cx| {
                assert!(copy.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            gate.queued.notify_one();
            return copy.await;
        }
        if src.file_name().is_some_and(|name| name == "gated.txt") {
            self.entered.wait().await;
            self.release.wait().await;
            if self
                .failures
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    count.checked_sub(1)
                })
                .is_ok()
            {
                return Err(CoreError::io(dst, "injected copy failure"));
            }
        }
        self.local.copy(src, dst, cx).await
    }

    async fn delete(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        self.deletes.fetch_add(1, Ordering::Relaxed);
        self.local.delete(path, cx).await
    }

    async fn mkdir(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        self.local.mkdir(path, cx).await
    }
}

pub fn core(provider: Arc<CopyGate>) -> (FilerCore, flume::Receiver<Event>) {
    let core = FilerCore::new();
    core.load(OperationsModule::new(provider));
    let events = core.event_receiver();
    (core, events)
}

pub fn local_ref(path: &Path) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

pub fn copy(session: SessionId, sources: &[&Path], destination: &Path) -> Command {
    Command::Copy {
        sources: sources.iter().map(|path| local_ref(path)).collect(),
        destination: local_ref(destination),
        session,
        request: RequestId::new(),
        operation: OperationId::new(),
    }
}

pub fn create(session: SessionId, parent: &Path, name: &str) -> Command {
    Command::CreateFolder {
        parent: local_ref(parent),
        name: name.into(),
        session,
        request: RequestId::new(),
        operation: OperationId::new(),
    }
}

pub async fn next(events: &flume::Receiver<Event>) -> Event {
    timeout(TIMEOUT, events.recv_async())
        .await
        .expect("expected an operation event")
        .unwrap()
}

pub async fn accepted(events: &flume::Receiver<Event>, command: &Command) {
    let event = next(events).await;
    assert!(
        matches!(event, Event::OperationAccepted { session, request, operation, .. }
        if Some(session) == command.session_id() && Some(request) == command.request_id()
            && Some(operation) == command.operation_id()),
        "{event:?}"
    );
}

pub async fn started(events: &flume::Receiver<Event>, command: &Command) {
    let event = next(events).await;
    assert!(
        matches!(event, Event::OperationStarted { session, request, operation, .. }
        if Some(session) == command.session_id() && Some(request) == command.request_id()
            && Some(operation) == command.operation_id()),
        "{event:?}"
    );
}

pub async fn complete(events: &flume::Receiver<Event>, command: &Command) {
    let event = timeout(TIMEOUT, async {
        loop {
            let event = next(events).await;
            if !matches!(event, Event::ProgressUpdated { .. }) {
                return event;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(event, Event::OperationComplete { session, operation_id, success: true, .. }
        if Some(session) == command.session_id() && Some(operation_id) == command.operation_id()),
        "{event:?}"
    );
}
