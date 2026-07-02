#[cfg(test)]
mod scanner_command_tests {
    use std::path::PathBuf;

    use crate::{model::session, modules::scan::scanner::ScanCommand};

    #[test]
    fn test_scan_command_clone() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Scan {
            path: PathBuf::from("/test"),
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
                ScanCommand::Scan {
                    path: p1,
                    pipeline: pl1,
                    session: s1,
                    request: _,
                    ..
                },
                ScanCommand::Scan {
                    path: p2,
                    pipeline: pl2,
                    session: s2,
                    request: _,
                    ..
                },
            ) => {
                assert_eq!(s1, s2);
                assert_eq!(p1, p2);
                assert_eq!(pl1, pl2);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_scan_command_debug() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Scan {
            path: PathBuf::from("/test/path"),
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
        assert!(debug_str.contains("Scan"));
        assert!(debug_str.contains("/test/path"));
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
