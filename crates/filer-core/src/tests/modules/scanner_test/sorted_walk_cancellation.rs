use super::*;
use crate::model::session::SessionId;
use crate::modules::scan::paging::{PageLoad, PagingSessions};
use crate::{CancelSignal, PipelineConfig, ProviderCx};

const WALK_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::test]
async fn test_cancelling_an_active_sorted_walk_releases_its_stream_and_paging_state() {
    let path = Path::new("/tmp/ordered-stream-cancel");
    let control = Arc::new(StreamControl::default());
    let provider = MockProvider {
        stream_control: Some(control.clone()),
        ..MockProvider::streaming()
    };
    for index in 0..300 {
        provider.add_file(make_file(
            &format!("file{index}.txt"),
            "/tmp/ordered-stream-cancel",
            0,
            false,
        ));
    }
    let sessions = PagingSessions::new();
    let cancel = CancelSignal::new();
    let cx = ProviderCx::with_cancel(&cancel);
    let pipeline = PipelineConfig::with_default_sort();
    let request = DirectoryPageRequest {
        listing: ListingOptions::fast(),
        limit: 7,
        cursor: None,
    };

    let load = sessions.load_provider(&provider, path, SessionId::new(), request, &pipeline, &cx);
    let trigger = async {
        tokio::time::timeout(WALK_TIMEOUT, control.stalled.notified())
            .await
            .expect("the second batch should stall after the first batch returns");
        assert_eq!(provider.stream_stats().rows_yielded, 256);
        assert!(!control.released.load(Ordering::SeqCst));
        assert!(!cancel.is_cancelled());
        cancel.cancel();
    };

    let (load, ()) = tokio::time::timeout(WALK_TIMEOUT, async { tokio::join!(load, trigger) })
        .await
        .expect("cancellation should interrupt the stalled provider read");

    assert!(matches!(
        load.expect("cancellation should not fail the load"),
        PageLoad::Cancelled
    ));
    assert!(control.released.load(Ordering::SeqCst));
    assert_eq!(sessions.retained_rows(), 0);
    assert_eq!(sessions.len(), 0);
}
