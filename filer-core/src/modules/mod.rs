//! Built-in modules for filer-core.
//!
//! Each module bundles command handlers with the actors they dispatch to.
//! All modules are optional — load only what you need:
//!
//! ```ignore
//! let core = FilerCore::new();
//!
//! // Standard file manager setup
//! let scan = ScanModule::new(Arc::new(LocalFs::new(core.registry())));
//! let nav = NavigationModule::new(scan.sender());
//! core.load(scan);
//! core.load(nav);
//!
//! // Or swap the scanner implementation
//! let scan = ScanModule::new(Arc::new(MyCustomFs::new()));
//! let nav = NavigationModule::new(scan.sender());
//! core.load(scan);
//! core.load(nav);
//!
//! // Or skip navigation entirely and build something else
//! core.load(MySearchOnlyModule::new());
//! ```

pub mod navigation;
pub mod operations;
pub mod preview;
pub mod scan;
pub mod search;
pub mod watch;
