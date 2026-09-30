#[cfg(test)]
mod context_cancellation_tests {
    use super::*;

    fn cancelled_cx() -> (crate::CancelSignal, crate::ProviderCx<'static>) {
        let cancel = crate::CancelSignal::new();
        cancel.cancel();
        let leaked = Box::leak(Box::new(cancel.clone()));
        (cancel, crate::ProviderCx::with_cancel(leaked))
    }

    #[tokio::test]
    async fn test_local_fs_list_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let (_cancel, cx) = cancelled_cx();

        let result = fs.list(dir.path(), &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn test_local_fs_read_header_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");
        tokio::fs::write(&path, b"hello").await.unwrap();
        let (_cancel, cx) = cancelled_cx();

        let result = fs.read_header(&path, MAGIC_BYTE_WINDOW, &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn test_local_fs_write_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");
        let (_cancel, cx) = cancelled_cx();

        let result = fs.write(&path, b"hello", &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
        assert!(!path.exists());
    }
}
