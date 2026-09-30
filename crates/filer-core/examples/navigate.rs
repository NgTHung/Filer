//! # Navigate example
//!
//! Lists the first page of a directory through the public command and event
//! API. Pass a directory path, or omit it to list the current directory.

use filer_core::{Command, Event, FilerCore, Location, LocationRef, RequestId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    let core = FilerCore::with_defaults();
    let events = core.event_receiver();

    core.send(Command::Handshake)?;
    let session = loop {
        if let Event::SessionCreated(session) = events.recv_async().await? {
            break session;
        }
    };

    let request = RequestId::new();
    core.send(Command::Navigate {
        location: LocationRef::from_location(&Location::local(path)),
        session,
        request,
    })?;

    while let Ok(event) = events.recv_async().await {
        match event {
            Event::DirectoryPageLoaded {
                groups,
                request: loaded,
                ..
            } if loaded == request => {
                println!("first page has {} rows", groups.total_count);
                break;
            }
            Event::Error {
                message,
                request: Some(failed),
                ..
            } if failed == request => return Err(message.into()),
            _ => {}
        }
    }
    Ok(())
}
