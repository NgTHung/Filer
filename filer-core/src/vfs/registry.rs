//! # Provider Registry
//!
//! Defines provider profile contracts and runtime provider lookup. Portable
//! profiles carry stable provider identity and capability shape, while live
//! providers stay in a runtime registry so secrets never become serialized
//! profile data.
//!
//! ```
//! # use filer_core::{Capabilities, ProviderProfile, ProviderProfileId};
//! # use std::path::PathBuf;
//! let profile = ProviderProfile::new(
//!     ProviderProfileId::new("work"),
//!     "archive",
//!     "Work archive",
//!     PathBuf::from("/archives/work.zip"),
//!     Capabilities { read: true, write: false, watch: false, search: false },
//! );
//! assert_eq!(profile.id().as_str(), "work");
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rapidhash::fast::RandomState;
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;
use crate::model::location::{LocationDescriptor, ProviderRef};
use crate::vfs::provider::Capabilities;
use crate::vfs::provider::FsProvider;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderProfileId(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    id: ProviderProfileId,
    scheme: String,
    display_name: String,
    default_root: PathBuf,
    capabilities: Capabilities,
}

#[derive(Clone)]
pub struct ProviderRegistry {
    local: Arc<dyn FsProvider>,
    profiles: Arc<scc::HashMap<String, RegisteredProvider, RandomState>>,
    ephemeral: Arc<scc::HashMap<String, Arc<dyn FsProvider>, RandomState>>,
}

#[derive(Clone)]
struct RegisteredProvider {
    provider: Arc<dyn FsProvider>,
}

impl ProviderProfileId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ProviderProfile {
    pub fn new(
        id: ProviderProfileId,
        scheme: impl Into<String>,
        display_name: impl Into<String>,
        default_root: PathBuf,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            id,
            scheme: scheme.into(),
            display_name: display_name.into(),
            default_root,
            capabilities,
        }
    }

    pub fn id(&self) -> &ProviderProfileId {
        &self.id
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn default_root(&self) -> &Path {
        &self.default_root
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
}

impl ProviderRegistry {
    pub fn new(local: Arc<dyn FsProvider>) -> Self {
        Self {
            local,
            profiles: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            ephemeral: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    pub fn register_profile(
        &self,
        profile: ProviderProfile,
        provider: Arc<dyn FsProvider>,
    ) -> Result<(), CoreError> {
        if profile.scheme() != provider.scheme() {
            return Err(CoreError::invalid_input(format!(
                "provider profile '{}' uses scheme '{}' but provider reports '{}'",
                profile.id().as_str(),
                profile.scheme(),
                provider.scheme()
            )));
        }

        let key = profile.id().as_str().to_string();
        let entry = RegisteredProvider { provider };
        let _ = self.profiles.remove_sync(&key);
        let _ = self.profiles.insert_sync(key, entry);
        Ok(())
    }

    pub fn register_ephemeral(
        &self,
        id: impl Into<String>,
        provider: Arc<dyn FsProvider>,
    ) -> Option<Arc<dyn FsProvider>> {
        let id = id.into();
        let old = self
            .ephemeral
            .remove_sync(&id)
            .map(|(_, provider)| provider);
        let _ = self.ephemeral.insert_sync(id, provider);
        old
    }

    pub fn resolve(&self, provider: &ProviderRef) -> Result<Arc<dyn FsProvider>, CoreError> {
        match provider {
            ProviderRef::Local => Ok(self.local.clone()),
            ProviderRef::Profile(id) => self
                .profiles
                .read_sync(id, |_, entry| entry.provider.clone())
                .ok_or_else(|| {
                    CoreError::unsupported_provider(
                        format!("profile:{id}"),
                        format!("unknown provider profile: {id}"),
                    )
                }),
            ProviderRef::Ephemeral(id) => self
                .ephemeral
                .read_sync(id, |_, provider| provider.clone())
                .ok_or_else(|| {
                    CoreError::unsupported_provider(
                        format!("ephemeral:{id}"),
                        format!("unknown ephemeral provider: {id}"),
                    )
                }),
        }
    }

    pub fn resolve_descriptor(
        &self,
        descriptor: &LocationDescriptor,
    ) -> Result<Arc<dyn FsProvider>, CoreError> {
        self.resolve(descriptor.provider())
    }

    pub fn capabilities(&self, provider: &ProviderRef) -> Result<Capabilities, CoreError> {
        Ok(self.resolve(provider)?.capabilities())
    }
}
