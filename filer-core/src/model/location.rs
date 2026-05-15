use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocationId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderRef {
    Local,
    Profile(String),
    Ephemeral(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocationDescriptor {
    scheme: String,
    provider: ProviderRef,
    path: PathBuf,
    display_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    id: LocationId,
    descriptor: LocationDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationRef {
    id: Option<LocationId>,
    descriptor: Option<LocationDescriptor>,
}

impl LocationDescriptor {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            scheme: "file".to_string(),
            provider: ProviderRef::Local,
            path: path.into(),
            display_path: None,
        }
    }

    pub fn provider_profile(
        scheme: impl Into<String>,
        profile: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            scheme: scheme.into(),
            provider: ProviderRef::Profile(profile.into()),
            path: path.into(),
            display_path: None,
        }
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn provider(&self) -> &ProviderRef {
        &self.provider
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn as_local_path(&self) -> Option<&Path> {
        match (&self.scheme[..], &self.provider) {
            ("file", ProviderRef::Local) => Some(&self.path),
            _ => None,
        }
    }

    pub fn display_path(&self) -> String {
        self.display().into_owned()
    }

    pub fn display(&self) -> Cow<'_, str> {
        if let Some(display_path) = &self.display_path {
            Cow::Borrowed(display_path)
        } else {
            Cow::Owned(self.path.display().to_string())
        }
    }
}

impl Location {
    pub fn new(descriptor: LocationDescriptor) -> Self {
        let id = LocationId::from_descriptor(&descriptor);
        Self { id, descriptor }
    }

    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::new(LocationDescriptor::local(path))
    }

    pub fn id(&self) -> LocationId {
        self.id
    }

    pub fn descriptor(&self) -> &LocationDescriptor {
        &self.descriptor
    }

    pub fn into_descriptor(self) -> LocationDescriptor {
        self.descriptor
    }

    pub fn as_local_path(&self) -> Option<&Path> {
        self.descriptor.as_local_path()
    }
}

impl LocationRef {
    pub fn new(id: Option<LocationId>, descriptor: Option<LocationDescriptor>) -> Self {
        Self { id, descriptor }
    }

    pub fn id_only(id: LocationId) -> Self {
        Self {
            id: Some(id),
            descriptor: None,
        }
    }

    pub fn descriptor_only(descriptor: LocationDescriptor) -> Self {
        Self {
            id: None,
            descriptor: Some(descriptor),
        }
    }

    pub fn from_location(location: &Location) -> Self {
        Self {
            id: Some(location.id()),
            descriptor: Some(location.descriptor().clone()),
        }
    }

    pub fn id(&self) -> Option<LocationId> {
        self.id
    }

    pub fn descriptor(&self) -> Option<&LocationDescriptor> {
        self.descriptor.as_ref()
    }
}

impl LocationId {
    pub fn from_descriptor(descriptor: &LocationDescriptor) -> Self {
        use rapidhash::fast::RapidHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = RapidHasher::default();
        descriptor.hash(&mut hasher);
        LocationId(hasher.finish())
    }
}

impl fmt::Display for LocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "location:{}", self.0)
    }
}
