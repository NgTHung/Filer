#[cfg(test)]
mod scanner_command_tests {
    use std::path::PathBuf;

    use crate::{
        Location, LocationRef, model::session, modules::scan::scanner::ScanCommand,
    };

    #[test]
    fn test_scan_command_clone() {
        let session = session::SessionId::new();
        let location = LocationRef::from_location(&Location::local(PathBuf::from("/test")));
        let cmd = ScanCommand::ScanLocation {
            location: location.clone(),
            pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            load: crate::DirectoryLoadOptions::default(),
            session,
            request: crate::model::request::RequestId::new(),
        };

        let cloned = cmd.clone();

        match (cmd, cloned) {
            (
                ScanCommand::ScanLocation {
                    location: l1,
                    pipeline: pl1,
                    session: s1,
                    request: _,
                    ..
                },
                ScanCommand::ScanLocation {
                    location: l2,
                    pipeline: pl2,
                    session: s2,
                    request: _,
                    ..
                },
            ) => {
                assert_eq!(s1, s2);
                assert_eq!(l1, l2);
                assert_eq!(l1, location);
                assert_eq!(pl1, pl2);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_scan_command_debug() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::ScanLocation {
            location: LocationRef::from_location(&Location::local(PathBuf::from("/test/path"))),
            pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            load: crate::DirectoryLoadOptions::default(),
            session,
            request: crate::model::request::RequestId::new(),
        };

        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("ScanLocation"));
        assert!(debug_str.contains("/test/path"));
    }

    #[test]
    fn test_refresh_location_command_carries_location_ref() {
        let session = session::SessionId::new();
        let location =
            LocationRef::from_location(&Location::local(PathBuf::from("/test/refresh")));
        let cmd = ScanCommand::RefreshLocation {
            location: location.clone(),
            pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            load: crate::DirectoryLoadOptions::default(),
            session,
            request: crate::model::request::RequestId::new(),
        };

        match cmd {
            ScanCommand::RefreshLocation {
                location: routed,
                session: routed_session,
                ..
            } => {
                assert_eq!(routed, location);
                assert_eq!(routed_session, session);
            }
            other => panic!("Expected ScanCommand::RefreshLocation, got {other:?}"),
        }
    }

    #[test]
    fn test_cancel_command() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Cancel(session);
        let _cloned = cmd.clone();
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Cancel"));
    }
}
