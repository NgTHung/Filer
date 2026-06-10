//! Tests for the FilerCore handle (Phase 4 – Handle)
//!
//! The FilerCore handle is the public API surface that external code (GUI, CLI,
//! web transport) uses to interact with the core. It wraps:
//! - A command sender (UI → Core)
//! - An event receiver (Core → UI)
//!
//! FilerCore::new() should spin up all internal actors (Router, Navigator,
//! Scanner, Searcher, Watcher, Previewer, Operator) and wire their channels.
//!
//! Tests are written BEFORE implementation (TDD).

#[cfg(test)]
mod handle_tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use tokio::time::timeout;

    use crate::api::commands::Command;
    use crate::api::events::Event;
    use crate::api::handle::FilerCore;

    use crate::model::session::SessionId;

    /// Timeout for async operations in tests
    const TEST_TIMEOUT: Duration = Duration::from_millis(1000);

    #[tokio::test]
    async fn test_new_creates_instance() {
        // FilerCore::new() should succeed and return a valid handle
        let _core = FilerCore::new();
        // If we get here without panic, new() succeeded
    }

    #[tokio::test]
    async fn test_new_provides_command_sender() {
        let core = FilerCore::new();
        // The command sender should be usable (not closed)
        let sender = core.command_sender();
        assert!(!sender.is_disconnected(), "command channel should be open");
    }

    #[tokio::test]
    async fn test_new_provides_event_receiver() {
        let core = FilerCore::new();
        // The event receiver should be usable (not closed)
        let receiver = core.event_receiver();
        assert!(!receiver.is_disconnected(), "event channel should be open");
    }

    #[tokio::test]
    async fn test_send_handshake_returns_session_created() {
        let core = FilerCore::new();

        // Send a Handshake command — the router should create a session
        // and emit SessionCreated
        let result = core.send(Command::Handshake);
        assert!(result.is_ok(), "send(Handshake) should succeed");

        // We should receive a SessionCreated event
        let event = timeout(TEST_TIMEOUT, core.event_receiver().recv_async()).await;
        assert!(event.is_ok(), "should receive an event within timeout");

        match event.unwrap() {
            Ok(Event::SessionCreated(session_id)) => {
                assert_ne!(
                    session_id,
                    SessionId::DEFAULT,
                    "should be a real session id"
                );
            }
            other => panic!("expected SessionCreated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_send_command_with_valid_session() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // First, create a session via Handshake
        core.send(Command::Handshake).unwrap();
        let session_id = match timeout(TEST_TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("expected SessionCreated, got {:?}", other),
        };

        // Now send a Navigate command using the valid session
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/tmp"),
            session: session_id,
            request: crate::model::request::RequestId::new(),
        })
        .unwrap();

        // The command should be accepted and routed (no Error event for invalid session)
        // We may receive DirectoryLoadedCompat or another event, but NOT an "Unknown session" error
        // Give time for routing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Drain events and check none are "Unknown session" errors
        while let Ok(event) = rx.try_recv() {
            if let Event::Error { message, .. } = &event {
                assert!(
                    !message.contains("Unknown session"),
                    "valid session should not produce Unknown session error"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_send_command_with_invalid_session_gets_error() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Send a command with a completely bogus session (no Handshake first)
        let bogus = SessionId::new();
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/tmp"),
            session: bogus,
            request: crate::model::request::RequestId::new(),
        })
        .unwrap();

        // Should receive an Error event about unknown session
        let event = timeout(TEST_TIMEOUT, rx.recv_async()).await;
        assert!(event.is_ok(), "should receive error event within timeout");

        match event.unwrap() {
            Ok(Event::Error {
                message, session, ..
            }) => {
                assert!(
                    message.contains("Unknown session"),
                    "error should mention unknown session, got: {}",
                    message
                );
                assert_eq!(session, bogus);
            }
            other => panic!("expected Error event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_try_recv_returns_none_when_no_events() {
        let core = FilerCore::new();
        // No commands sent, no events should be pending
        assert!(
            core.try_recv().is_none(),
            "try_recv should return None when no events"
        );
    }

    #[tokio::test]
    async fn test_try_recv_returns_event_after_command() {
        let core = FilerCore::new();

        core.send(Command::Handshake).unwrap();
        // Give the router time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let event = core.try_recv();
        assert!(event.is_some(), "try_recv should return SessionCreated");
        match event.unwrap() {
            Event::SessionCreated(_) => {} // expected
            other => panic!("expected SessionCreated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_events_are_session_scoped() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Create two sessions
        core.send(Command::Handshake).unwrap();
        core.send(Command::Handshake).unwrap();

        let session1 = match timeout(TEST_TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("expected SessionCreated for session 1, got {:?}", other),
        };
        let session2 = match timeout(TEST_TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("expected SessionCreated for session 2, got {:?}", other),
        };

        assert_ne!(
            session1, session2,
            "each handshake should get a unique session"
        );

        // Destroy session 1 — should get SessionDestroyed for session 1
        core.send(Command::DestroySession(session1)).unwrap();

        let event = timeout(TEST_TIMEOUT, rx.recv_async()).await;
        match event {
            Ok(Ok(Event::SessionDestroyed(id))) => {
                assert_eq!(id, session1, "should destroy session 1");
            }
            other => panic!("expected SessionDestroyed(session1), got {:?}", other),
        }

        // Session 2 should still be valid — commands with session2 should NOT error
        core.send(Command::NavigateUp {
            session: session2,
            request: crate::model::request::RequestId::new(),
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        while let Ok(event) = rx.try_recv() {
            if let Event::Error { message, .. } = &event {
                assert!(
                    !message.contains("Unknown session"),
                    "session 2 should still be valid, got: {}",
                    message
                );
            }
        }
    }

    #[tokio::test]
    async fn test_multiple_handshakes_unique_sessions() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        for _ in 0..5 {
            core.send(Command::Handshake).unwrap();
        }

        let mut session_ids = Vec::new();
        for _ in 0..5 {
            match timeout(TEST_TIMEOUT, rx.recv_async()).await {
                Ok(Ok(Event::SessionCreated(id))) => session_ids.push(id),
                other => panic!("expected SessionCreated, got {:?}", other),
            }
        }

        // All IDs unique
        let mut deduped = session_ids.clone();
        deduped.sort_by_key(|id| id.0);
        deduped.dedup();
        assert_eq!(deduped.len(), 5, "all 5 session IDs should be unique");
    }

    #[tokio::test]
    async fn test_destroy_session_emits_session_destroyed() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Create a session
        core.send(Command::Handshake).unwrap();
        let session_id = match timeout(TEST_TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("expected SessionCreated, got {:?}", other),
        };

        // Destroy it
        core.send(Command::DestroySession(session_id)).unwrap();
        let event = timeout(TEST_TIMEOUT, rx.recv_async()).await;
        match event {
            Ok(Ok(Event::SessionDestroyed(id))) => {
                assert_eq!(id, session_id);
            }
            other => panic!("expected SessionDestroyed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_destroyed_session_commands_get_error() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Create and destroy a session
        core.send(Command::Handshake).unwrap();
        let session_id = match timeout(TEST_TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("expected SessionCreated, got {:?}", other),
        };

        core.send(Command::DestroySession(session_id)).unwrap();
        // Drain the SessionDestroyed event
        let _ = timeout(TEST_TIMEOUT, rx.recv_async()).await;

        // Now send a command with the destroyed session
        core.send(Command::NavigateUp {
            session: session_id,
            request: crate::model::request::RequestId::new(),
        })
        .unwrap();

        let event = timeout(TEST_TIMEOUT, rx.recv_async()).await;
        match event {
            Ok(Ok(Event::Error {
                message, session, ..
            })) => {
                assert!(message.contains("Unknown session"));
                assert_eq!(session, session_id);
            }
            other => panic!("expected Error(Unknown session), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_shutdown_succeeds() {
        let core = FilerCore::new();
        let result = core.shutdown();
        assert!(result.is_ok(), "shutdown should succeed");
    }

    #[tokio::test]
    async fn test_shutdown_closes_command_channel() {
        let core = FilerCore::new();
        let sender = core.command_sender();

        core.shutdown().unwrap();

        // After shutdown, the command channel should eventually close.
        // Give actors a moment to wind down.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Sending a command after shutdown should fail
        let result = sender.send(Command::Handshake);
        assert!(
            result.is_err(),
            "sending after shutdown should fail (channel closed)"
        );
    }

    #[tokio::test]
    async fn test_shutdown_cleans_all_sessions() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Create several sessions
        for _ in 0..3 {
            core.send(Command::Handshake).unwrap();
        }

        // Drain SessionCreated events
        let mut sessions = Vec::new();
        for _ in 0..3 {
            match timeout(TEST_TIMEOUT, rx.recv_async()).await {
                Ok(Ok(Event::SessionCreated(id))) => sessions.push(id),
                other => panic!("expected SessionCreated, got {:?}", other),
            }
        }
        assert_eq!(sessions.len(), 3);

        // Shutdown should clean everything
        let result = core.shutdown();
        assert!(result.is_ok());

        // After shutdown + some delay, send attempts should fail
        tokio::time::sleep(Duration::from_millis(100)).await;
        let sender = core.command_sender();
        assert!(
            sender.send(Command::Handshake).is_err(),
            "command channel should be closed after shutdown"
        );
    }

    #[tokio::test]
    async fn test_send_after_shutdown_returns_error() {
        let core = FilerCore::new();
        core.shutdown().unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = core.send(Command::Handshake);
        assert!(
            result.is_err(),
            "send after shutdown should return CoreError"
        );
    }

    #[tokio::test]
    async fn test_event_receiver_clone_receives_same_events() {
        let core = FilerCore::new();

        // Clone two receivers
        let rx1 = core.event_receiver();
        let rx2 = core.event_receiver();

        core.send(Command::Handshake).unwrap();

        // Both clones of a flume receiver share the same queue —
        // only ONE of them will get the event (competing consumers).
        // This test verifies that event_receiver() returns a working clone.
        let event = timeout(TEST_TIMEOUT, async {
            tokio::select! {
                e = rx1.recv_async() => e,
                e = rx2.recv_async() => e,
            }
        })
        .await;

        match event {
            Ok(Ok(Event::SessionCreated(_))) => {} // one of the receivers got it
            other => panic!("expected SessionCreated from one receiver, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_command_sender_clone_sends_to_same_router() {
        let core = FilerCore::new();
        let rx = core.event_receiver();

        // Use a cloned sender
        let sender = core.command_sender();
        sender.send(Command::Handshake).unwrap();

        let event = timeout(TEST_TIMEOUT, rx.recv_async()).await;
        match event {
            Ok(Ok(Event::SessionCreated(_))) => {} // routed correctly
            other => panic!("expected SessionCreated, got {:?}", other),
        }
    }
}
