//! # Profile records
//!
//! A request names its machine and filesystem by id and digest instead of
//! carrying every hardware detail. The raw result stores the full record next
//! to the digest, so a later report can prove which environment produced a
//! sample. Entries keep the caller's order because the specification digests
//! records in a fixed, documented order.
//!
//! ```
//! use filer_core_benchmarks::ProfileRecord;
//!
//! let machine = ProfileRecord::new(
//!     "linux-x86_64-lab-01",
//!     [("os".to_string(), "linux".to_string())],
//! )?;
//! assert!(machine.digest().starts_with("sha256:"));
//! # Ok::<(), filer_core_benchmarks::ProtocolError>(())
//! ```

use std::collections::BTreeSet;

use crate::canonical::digest_records;
use crate::schema::is_valid_identifier;
use crate::{ErrorCode, ProtocolError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRecord {
    id: String,
    entries: Vec<(String, String)>,
}

impl ProfileRecord {
    pub fn new(
        id: impl Into<String>,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ProtocolError> {
        let id = id.into();
        if !is_valid_identifier(&id) {
            return Err(ProtocolError::new(
                ErrorCode::InvalidSchema,
                "profile id does not match the protocol syntax",
            ));
        }
        let entries = entries.into_iter().collect::<Vec<_>>();
        let mut names = BTreeSet::new();
        for (name, _) in &entries {
            if name.is_empty() || !names.insert(name.as_str()) {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidSchema,
                    "profile entry names must be non-empty and unique",
                )
                .with_field(name.clone()));
            }
        }
        Ok(Self { id, entries })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    pub fn digest(&self) -> String {
        digest_records(
            "profile",
            vec!["name", "value"],
            self.entries
                .iter()
                .map(|(name, value)| vec![name.clone(), value.clone()])
                .collect(),
        )
    }
}
