//! Tests for the SessionManager
//!
//! Phase 4 – Session Management tests:
//! - create session returns SessionId
//! - destroy session cleans up
//! - get session by id (exists)
//! - session isolation (events scoped to correct client)
//! - broadcast to all sessions
//! - send to specific session
//! - send to nonexistent session
//! - create session with custom policy
//! - is_allowed delegates to policy
//! - clone shares state
//! - concurrent session creation

#[cfg(test)]
mod session_manager_tests {
    use std::path::PathBuf;

    use flume::{Receiver, Sender};

    use crate::api::events::Event;
    use crate::api::session_manager::{SendError, SessionManager};
    use crate::model::registry::NodeRegistry;
    use crate::model::session::{Operation, RestrictedPolicy, SessionId};

    /// Helper: create a SessionManager with a fresh NodeRegistry
    fn make_manager() -> SessionManager {
        let reg = NodeRegistry::new();
        SessionManager::new(reg)
    }

    /// Helper: create a bounded event channel pair
    fn event_channel() -> (Sender<Event>, Receiver<Event>) {
        flume::bounded(64)
    }

    // ── create session ──────────────────────────────────────────────

    #[test]
    fn test_create_session_returns_id() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();

        let id = mgr.create_session(tx);
        // SessionId should be non-default (counter starts at 1)
        assert_ne!(id, SessionId::DEFAULT);
    }

    #[test]
    fn test_create_session_increments_count() {
        let mgr = make_manager();
        assert_eq!(mgr.count(), 0);

        let (tx1, _rx1) = event_channel();
        mgr.create_session(tx1);
        assert_eq!(mgr.count(), 1);

        let (tx2, _rx2) = event_channel();
        mgr.create_session(tx2);
        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn test_create_session_ids_are_unique() {
        let mgr = make_manager();
        let (tx1, _rx1) = event_channel();
        let (tx2, _rx2) = event_channel();
        let (tx3, _rx3) = event_channel();

        let id1 = mgr.create_session(tx1);
        let id2 = mgr.create_session(tx2);
        let id3 = mgr.create_session(tx3);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    // ── exists (get session by id) ──────────────────────────────────

    #[test]
    fn test_get_session() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();

        let id = mgr.create_session(tx);
        assert!(mgr.exists(id));
    }

    #[test]
    fn test_exists_returns_false_for_unknown_id() {
        let mgr = make_manager();
        let bogus = SessionId::new();
        assert!(!mgr.exists(bogus));
    }

    #[test]
    fn test_exists_returns_false_for_default_id() {
        let mgr = make_manager();
        assert!(!mgr.exists(SessionId::DEFAULT));
    }

    // ── remove session ──────────────────────────────────────────────

    #[test]
    fn test_remove_session() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();
        let id = mgr.create_session(tx);

        assert!(mgr.exists(id));
        assert!(mgr.remove(id)); // returns true when found
        assert!(!mgr.exists(id)); // gone
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_returns_false() {
        let mgr = make_manager();
        assert!(!mgr.remove(SessionId::new()));
    }

    #[test]
    fn test_remove_idempotent() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();
        let id = mgr.create_session(tx);

        assert!(mgr.remove(id));
        assert!(!mgr.remove(id)); // second remove returns false
    }

    // ── session isolation ───────────────────────────────────────────

    #[test]
    fn test_session_isolation() {
        let mgr = make_manager();

        let (tx1, rx1) = event_channel();
        let (tx2, rx2) = event_channel();
        let id1 = mgr.create_session(tx1);
        let _id2 = mgr.create_session(tx2);

        // Send an event only to session 1
        let event = Event::Error {
            message: "for session 1 only".into(),
            recoverable: true,
            session: id1,
            request: None,
        };
        mgr.send_to(id1, event).unwrap();

        // Session 1 receives the event
        let received = rx1.try_recv();
        assert!(received.is_ok());
        match received.unwrap() {
            Event::Error { message, .. } => assert_eq!(message, "for session 1 only"),
            other => panic!("expected Error event, got {:?}", other),
        }

        // Session 2's channel is empty — no cross-contamination
        assert!(rx2.try_recv().is_err());
    }

    // ── broadcast ───────────────────────────────────────────────────

    #[test]
    fn test_broadcast_to_all() {
        let mgr = make_manager();

        let (tx1, rx1) = event_channel();
        let (tx2, rx2) = event_channel();
        let (tx3, rx3) = event_channel();
        let _s1 = mgr.create_session(tx1);
        let _s2 = mgr.create_session(tx2);
        let _s3 = mgr.create_session(tx3);

        let event = Event::Error {
            message: "broadcast".into(),
            recoverable: false,
            session: SessionId::DEFAULT, // broadcast, so session field is illustrative
            request: None,
        };
        mgr.broadcast(event);

        // All three sessions should receive the event
        for (label, rx) in [("s1", &rx1), ("s2", &rx2), ("s3", &rx3)] {
            let received = rx.try_recv();
            assert!(received.is_ok(), "{label} should have received broadcast");
            match received.unwrap() {
                Event::Error { message, .. } => assert_eq!(message, "broadcast"),
                other => panic!("{label}: expected Error event, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_broadcast_with_no_sessions() {
        let mgr = make_manager();
        // Should not panic even with zero sessions
        let event = Event::Error {
            message: "nobody home".into(),
            recoverable: true,
            session: SessionId::DEFAULT,
            request: None,
        };
        mgr.broadcast(event);
    }

    // ── send_to ─────────────────────────────────────────────────────

    #[test]
    fn test_send_to_specific() {
        let mgr = make_manager();
        let (tx, rx) = event_channel();
        let id = mgr.create_session(tx);

        let event = Event::Error {
            message: "hello".into(),
            recoverable: true,
            session: id,
            request: None,
        };
        let result = mgr.send_to(id, event);
        assert!(result.is_ok());

        let received = rx.try_recv().unwrap();
        match received {
            Event::Error { message, .. } => assert_eq!(message, "hello"),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_send_to_nonexistent() {
        let mgr = make_manager();
        let bogus = SessionId::new();

        let event = Event::Error {
            message: "lost".into(),
            recoverable: false,
            session: bogus,
            request: None,
        };
        let result = mgr.send_to(bogus, event);
        assert!(result.is_err());

        match result.unwrap_err() {
            SendError::SessionNotFound(id) => assert_eq!(id, bogus),
            other => panic!("expected SessionNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_send_to_closed_channel() {
        let mgr = make_manager();
        let (tx, rx) = event_channel();
        let id = mgr.create_session(tx);

        // Drop receiver to close the channel
        drop(rx);

        let event = Event::Error {
            message: "orphan".into(),
            recoverable: false,
            session: id,
            request: None,
        };
        let result = mgr.send_to(id, event);
        assert!(result.is_err());

        match result.unwrap_err() {
            SendError::ChannelClosed => {} // expected
            other => panic!("expected ChannelClosed, got {:?}", other),
        }
    }

    // ── create_session_with_policy ──────────────────────────────────

    #[test]
    fn test_create_session_with_policy() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();

        let policy = RestrictedPolicy {
            allowed_roots: vec![PathBuf::from("/srv/share")],
            allowed_ops: vec![Operation::Read],
            label: "web-user".into(),
        };

        let id = mgr.create_session_with_policy(tx, Box::new(policy));
        assert!(mgr.exists(id));
        assert_eq!(mgr.count(), 1);
    }

    // ── is_allowed ──────────────────────────────────────────────────

    #[test]
    fn test_is_allowed_with_allow_all() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();
        let id = mgr.create_session(tx); // default AllowAll policy

        // AllowAll permits everything
        assert!(mgr.is_allowed(id, Operation::Read, &PathBuf::from("/any/path")));
        assert!(mgr.is_allowed(id, Operation::Write, &PathBuf::from("/any/path")));
        assert!(mgr.is_allowed(id, Operation::Execute, &PathBuf::from("/any/path")));
        assert!(mgr.is_allowed(id, Operation::Watch, &PathBuf::from("/any/path")));
        assert!(mgr.is_allowed(id, Operation::Search, &PathBuf::from("/any/path")));
    }

    #[test]
    fn test_is_allowed_with_restricted_policy() {
        let mgr = make_manager();
        let (tx, _rx) = event_channel();

        let policy = RestrictedPolicy {
            allowed_roots: vec![PathBuf::from("/srv/share")],
            allowed_ops: vec![Operation::Read, Operation::Search],
            label: "reader".into(),
        };
        let id = mgr.create_session_with_policy(tx, Box::new(policy));

        // Allowed: Read under /srv/share
        assert!(mgr.is_allowed(id, Operation::Read, &PathBuf::from("/srv/share/docs")));
        // Allowed: Search under /srv/share
        assert!(mgr.is_allowed(id, Operation::Search, &PathBuf::from("/srv/share")));
        // Denied: Write not in allowed_ops
        assert!(!mgr.is_allowed(id, Operation::Write, &PathBuf::from("/srv/share/docs")));
        // Denied: Read outside allowed root
        assert!(!mgr.is_allowed(id, Operation::Read, &PathBuf::from("/etc/passwd")));
    }

    #[test]
    fn test_is_allowed_nonexistent_session() {
        let mgr = make_manager();
        let bogus = SessionId::new();
        // Non-existent session → false
        assert!(!mgr.is_allowed(bogus, Operation::Read, &PathBuf::from("/tmp")));
    }

    // ── clone shares state ──────────────────────────────────────────

    #[test]
    fn test_clone_shares_state() {
        let mgr = make_manager();
        let mgr2 = mgr.clone();

        let (tx, _rx) = event_channel();
        let id = mgr.create_session(tx);

        // Clone sees the same sessions
        assert!(mgr2.exists(id));
        assert_eq!(mgr2.count(), 1);

        // Removing via clone removes from original
        mgr2.remove(id);
        assert!(!mgr.exists(id));
        assert_eq!(mgr.count(), 0);
    }

    // ── concurrent session creation ─────────────────────────────────

    #[test]
    fn test_concurrent_session_creation() {
        use std::sync::Arc;
        use std::thread;

        let mgr = make_manager();
        let mgr = Arc::new(mgr);
        let mut handles = Vec::new();

        for _ in 0..50 {
            let m = Arc::clone(&mgr);
            handles.push(thread::spawn(move || {
                let (tx, _rx) = event_channel();
                m.create_session(tx)
            }));
        }

        let ids: Vec<SessionId> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All IDs should be unique
        let mut unique = ids.clone();
        unique.sort_by_key(|id| id.0);
        unique.dedup();
        assert_eq!(unique.len(), 50);

        // All sessions should exist
        assert_eq!(mgr.count(), 50);
        for id in &ids {
            assert!(mgr.exists(*id));
        }
    }

    // ── broadcast after partial removal ─────────────────────────────

    #[test]
    fn test_broadcast_after_removing_some_sessions() {
        let mgr = make_manager();

        let (tx1, rx1) = event_channel();
        let (tx2, rx2) = event_channel();
        let id1 = mgr.create_session(tx1);
        let _id2 = mgr.create_session(tx2);

        // Remove session 1
        mgr.remove(id1);

        let event = Event::Error {
            message: "still here".into(),
            recoverable: true,
            session: SessionId::DEFAULT,
            request: None,
        };
        mgr.broadcast(event);

        // Session 1 channel should NOT receive (removed)
        assert!(rx1.try_recv().is_err());
        // Session 2 channel should receive
        let received = rx2.try_recv();
        assert!(received.is_ok());
    }

    // ── multiple events queued ──────────────────────────────────────

    #[test]
    fn test_multiple_events_queued_in_order() {
        let mgr = make_manager();
        let (tx, rx) = event_channel();
        let id = mgr.create_session(tx);

        for i in 0..5 {
            let event = Event::Error {
                message: format!("msg-{}", i),
                recoverable: true,
                session: id,
                request: None,
            };
            mgr.send_to(id, event).unwrap();
        }

        for i in 0..5 {
            let received = rx.try_recv().unwrap();
            match received {
                Event::Error { message, .. } => assert_eq!(message, format!("msg-{}", i)),
                other => panic!("expected Error, got {:?}", other),
            }
        }
        // No more events
        assert!(rx.try_recv().is_err());
    }
}
