#[cfg(test)]
mod operator_timeout_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_times_out_with_provider_context() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/backup");
        let _src_id = register(&registry, &src_path);
        let _dst_id = register(&registry, &dst_path);
        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/home/user", 1024),
        );
        // Far longer than the operation timeout, so the deadline fires first.
        provider.set_copy_delay_ms(10_000);

        let (cmd_tx, evt_rx) =
            spawn_operator_with_timeout(provider, registry, Duration::from_millis(20));
        let operation_id = OperationId::new();
        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        match final_event {
            Event::Error {
                code,
                target,
                session: s,
                ..
            } => {
                assert_eq!(code, ErrorCode::TimedOut);
                assert_eq!(target, Some(ErrorTarget::Provider("mock".to_string())));
                assert_eq!(s, session);
            }
            other => panic!("Expected TimedOut error, got: {other:?}"),
        }
    }
}
