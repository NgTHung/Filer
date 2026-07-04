use crate::actors::cancel::CancelMap;
use crate::model::session::SessionId;

#[cfg(test)]
mod cancel_map_tests {
    use super::*;

    #[tokio::test]
    async fn stale_remove_preserves_current_token() {
        let cancels = CancelMap::new();
        let session = SessionId::new();

        let stale = cancels.arm(session);
        let current = cancels.arm(session);
        cancels.remove_if_current(session, &stale).await;
        cancels.cancel(session);

        assert!(stale.is_cancelled());
        assert!(current.is_cancelled());
    }

    #[tokio::test]
    async fn current_remove_deletes_current_token() {
        let cancels = CancelMap::new();
        let session = SessionId::new();

        let current = cancels.arm(session);
        cancels.remove_if_current(session, &current).await;
        cancels.cancel(session);

        assert!(!current.is_cancelled());
    }
}
