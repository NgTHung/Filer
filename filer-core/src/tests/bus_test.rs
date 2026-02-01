use crate::bus::channel::Channel;
use crate::bus::EventBus;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

#[cfg(test)]
mod channel_tests {
    use super::*;

    #[test]
    fn test_bounded_channel_creation() {
        let channel: Channel<String> = Channel::bounded(10);
        
        // Verify we can send and receive
        channel.tx.send("test".to_string()).expect("Failed to send");
        let received = channel.rx.recv().expect("Failed to receive");
        assert_eq!(received, "test");
    }

    #[test]
    fn test_unbounded_channel_creation() {
        let channel: Channel<i32> = Channel::unbounded();
        
        // Send multiple messages
        for i in 0..100 {
            channel.tx.send(i).expect("Failed to send");
        }
        
        // Receive all messages
        for i in 0..100 {
            let received = channel.rx.recv().expect("Failed to receive");
            assert_eq!(received, i);
        }
    }

    #[test]
    fn test_bounded_channel_capacity() {
        let channel: Channel<i32> = Channel::bounded(5);
        
        // Fill the channel
        for i in 0..5 {
            channel.tx.send(i).expect("Failed to send");
        }
        
        // Next send should not block immediately but might based on implementation
        // Test that we can send and receive properly
        let tx = channel.tx.clone();
        let handle = thread::spawn(move || {
            tx.send(5).expect("Failed to send");
        });
        
        // Receive one to make space
        let first = channel.rx.recv().expect("Failed to receive");
        assert_eq!(first, 0);
        
        handle.join().expect("Thread panicked");
        
        // Verify the channel still works
        let last = channel.rx.recv_timeout(Duration::from_millis(100))
            .expect("Failed to receive");
        assert_eq!(last, 1);
    }

    #[test]
    fn test_channel_clone_sender() {
        let channel: Channel<String> = Channel::bounded(10);
        let tx1 = channel.tx.clone();
        let tx2 = channel.tx.clone();
        
        tx1.send("from_tx1".to_string()).expect("Failed to send from tx1");
        tx2.send("from_tx2".to_string()).expect("Failed to send from tx2");
        
        let msg1 = channel.rx.recv().expect("Failed to receive");
        let msg2 = channel.rx.recv().expect("Failed to receive");
        
        // Order might vary, so check both are received
        assert!(msg1 == "from_tx1" || msg1 == "from_tx2");
        assert!(msg2 == "from_tx1" || msg2 == "from_tx2");
        assert_ne!(msg1, msg2);
    }

    #[test]
    fn test_channel_multithreaded() {
        let channel: Channel<usize> = Channel::bounded(100);
        let num_senders = 5;
        let messages_per_sender = 20;
        
        let mut handles = vec![];
        
        // Spawn sender threads
        for thread_id in 0..num_senders {
            let tx = channel.tx.clone();
            let handle = thread::spawn(move || {
                for i in 0..messages_per_sender {
                    tx.send(thread_id * 1000 + i).expect("Failed to send");
                }
            });
            handles.push(handle);
        }
        
        // Collect all messages
        let mut received = vec![];
        for _ in 0..(num_senders * messages_per_sender) {
            let msg = channel.rx.recv().expect("Failed to receive");
            received.push(msg);
        }
        
        // Wait for all senders
        for handle in handles {
            handle.join().expect("Thread panicked");
        }
        
        // Verify we received the right count
        assert_eq!(received.len(), num_senders * messages_per_sender);
    }

    #[test]
    fn test_channel_drop_sender() {
        let channel: Channel<i32> = Channel::bounded(5);
        
        channel.tx.send(1).expect("Failed to send");
        drop(channel.tx);
        
        // Can still receive what was sent
        let msg = channel.rx.recv().expect("Failed to receive");
        assert_eq!(msg, 1);
        
        // Next receive should fail since sender is dropped
        let result = channel.rx.recv_timeout(Duration::from_millis(100));
        assert!(result.is_err());
    }

    #[test]
    fn test_channel_try_recv() {
        let channel: Channel<i32> = Channel::bounded(5);
        
        // Try receive on empty channel
        let result = channel.rx.try_recv();
        assert!(result.is_err());
        
        // Send and try receive
        channel.tx.send(42).expect("Failed to send");
        let msg = channel.rx.try_recv().expect("Failed to try_recv");
        assert_eq!(msg, 42);
    }

    #[test]
    fn test_channel_with_complex_types() {
        #[derive(Debug, Clone, PartialEq)]
        struct ComplexMessage {
            id: usize,
            data: Vec<String>,
            flag: bool,
        }
        
        let channel: Channel<ComplexMessage> = Channel::unbounded();
        
        let msg = ComplexMessage {
            id: 1,
            data: vec!["hello".to_string(), "world".to_string()],
            flag: true,
        };
        
        channel.tx.send(msg.clone()).expect("Failed to send");
        let received = channel.rx.recv().expect("Failed to receive");
        
        assert_eq!(received, msg);
    }

    #[test]
    fn test_channel_barrier_sync() {
        let channel: Channel<usize> = Channel::bounded(10);
        let barrier = Arc::new(Barrier::new(3));
        
        let mut handles = vec![];
        
        for i in 0..3 {
            let tx = channel.tx.clone();
            let barrier = Arc::clone(&barrier);
            
            let handle = thread::spawn(move || {
                barrier.wait();
                tx.send(i).expect("Failed to send");
            });
            handles.push(handle);
        }
        
        // All threads should send roughly at the same time
        let mut received = vec![];
        for _ in 0..3 {
            let msg = channel.rx.recv().expect("Failed to receive");
            received.push(msg);
        }
        
        for handle in handles {
            handle.join().expect("Thread panicked");
        }
        
        received.sort();
        assert_eq!(received, vec![0, 1, 2]);
    }
}

#[cfg(test)]
mod event_bus_tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestMessage {
        content: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct AnotherMessage {
        value: i32,
    }

    #[test]
    fn test_event_bus_creation() {
        let _bus = EventBus::new();
        // EventBus should be created successfully
        // Cannot test internal state since channels field is private
    }

    #[test]
    fn test_event_bus_register_and_subscribe() {
        let bus = EventBus::new();
        
        // Register a channel for TestMessage
        let tx = bus.register::<TestMessage>(10);
        
        // Subscribe to TestMessage
        let rx = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe");
        
        // Send a message through the sender
        let msg = TestMessage {
            content: "Hello".to_string(),
        };
        tx.send(msg.clone()).expect("Failed to send");
        
        // Receive through subscriber
        let received = rx.recv().expect("Failed to receive");
        assert_eq!(received, msg);
    }

    #[test]
    fn test_event_bus_publish() {
        let bus = EventBus::new();
        
        // Register and subscribe
        bus.register::<TestMessage>(10);
        let rx = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe");
        
        // Publish a message
        let msg = TestMessage {
            content: "Test".to_string(),
        };
        let _ = bus.publish(msg.clone());
        
        // Receive it
        let received = rx.recv().expect("Failed to receive");
        assert_eq!(received, msg);
    }

    #[test]
    fn test_event_bus_multiple_message_types() {
        let bus = EventBus::new();
        
        // Register two different message types
        bus.register::<TestMessage>(10);
        bus.register::<AnotherMessage>(10);
        
        // Subscribe to both
        let rx1 = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe to TestMessage");
        let rx2 = bus.subscribe::<AnotherMessage>()
            .expect("Failed to subscribe to AnotherMessage");
        
        // Publish messages
        let _ = bus.publish(TestMessage { content: "Hello".to_string() });
        let _ = bus.publish(AnotherMessage { value: 42 });
        
        // Receive both
        let msg1 = rx1.recv().expect("Failed to receive TestMessage");
        let msg2 = rx2.recv().expect("Failed to receive AnotherMessage");
        
        assert_eq!(msg1.content, "Hello");
        assert_eq!(msg2.value, 42);
    }

    #[test]
    fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        
        bus.register::<TestMessage>(10);
        
        // Multiple subscribers
        let rx1 = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe");
        let rx2 = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe");
        
        // Publish one message
        let msg = TestMessage {
            content: "Broadcast".to_string(),
        };
        let _ = bus.publish(msg.clone());
        
        // Both should receive it (if broadcast semantics)
        // Or only one should receive it (if queue semantics)
        // This test assumes broadcast - adjust based on actual implementation
        let result1 = rx1.recv_timeout(Duration::from_millis(100));
        let result2 = rx2.recv_timeout(Duration::from_millis(100));
        
        // At least one should succeed
        assert!(result1.is_ok() || result2.is_ok());
    }

    #[test]
    fn test_event_bus_subscribe_before_register() {
        let bus = EventBus::new();
        
        // Try to subscribe before registering
        let result = bus.subscribe::<TestMessage>();

        // Should return None since not registered
        assert!(result.is_none());
    }

    #[test]
    fn test_event_bus_thread_safety() {
        let bus = Arc::new(EventBus::new());
        let bus_clone = Arc::clone(&bus);
        
        // This test verifies that EventBus can be shared across threads
        // The actual implementation would need Send + Sync traits
        
        let handle = thread::spawn(move || {
            // Would subscribe in another thread
            let _rx = bus_clone.subscribe::<TestMessage>();
        });
        
        handle.join().expect("Thread panicked");
    }

    #[test]
    fn test_event_bus_high_throughput() {
        let bus = EventBus::new();
        bus.register::<TestMessage>(1000);
        
        let rx = bus.subscribe::<TestMessage>()
            .expect("Failed to subscribe");
        
        // Publish many messages
        let count = 1000;
        for i in 0..count {
            let _ = bus.publish(TestMessage {
                content: format!("Message {}", i),
            });
        }
        
        // Receive all
        for i in 0..count {
            let msg = rx.recv().expect("Failed to receive");
            assert_eq!(msg.content, format!("Message {}", i));
        }
    }
}
