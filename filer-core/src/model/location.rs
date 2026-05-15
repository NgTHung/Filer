use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocationId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderRef {
    Local,
    Profile(String),
    /// Session-local provider identity.
    ///
    /// Ephemeral providers are valid for runtime lookup but should not be used
    /// as persisted identity unless the corresponding descriptor can be
    /// reconstructed in the next session.
    Ephemeral(String),
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LocationSegment {
    ArchiveMember { path: PathBuf },
    Virtual { scheme: String, path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocationDescriptor {
    scheme: String,
    provider: ProviderRef,
    root: PathBuf,
    segments: Vec<LocationSegment>,
    display_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    id: LocationId,
    descriptor: LocationDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationRef {
    Id(LocationId),
    Descriptor(LocationDescriptor),
    Full {
        id: LocationId,
        descriptor: LocationDescriptor,
    },
}

impl LocationDescriptor {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            scheme: "file".to_string(),
            provider: ProviderRef::Local,
            root: path.into(),
            segments: Vec::new(),
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
            root: path.into(),
            segments: Vec::new(),
            display_path: None,
        }
    }

    pub fn ephemeral(
        scheme: impl Into<String>,
        provider: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            scheme: scheme.into(),
            provider: ProviderRef::Ephemeral(provider.into()),
            root: path.into(),
            segments: Vec::new(),
            display_path: None,
        }
    }

    pub fn with_segment(mut self, segment: LocationSegment) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn with_segments(mut self, segments: impl IntoIterator<Item = LocationSegment>) -> Self {
        self.segments.extend(segments);
        self
    }

    pub fn archive_member(self, path: impl Into<PathBuf>) -> Self {
        self.with_segment(LocationSegment::ArchiveMember { path: path.into() })
    }

    pub fn with_display_path(mut self, display_path: impl Into<String>) -> Self {
        self.display_path = Some(display_path.into());
        self
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn provider(&self) -> &ProviderRef {
        &self.provider
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn segments(&self) -> &[LocationSegment] {
        &self.segments
    }

    pub fn is_segmented(&self) -> bool {
        !self.segments.is_empty()
    }

    pub fn as_local_path(&self) -> Option<&Path> {
        match (&self.scheme[..], &self.provider, self.segments.is_empty()) {
            ("file", ProviderRef::Local, true) => Some(&self.root),
            _ => None,
        }
    }

    pub fn display_path(&self) -> String {
        self.display().into_owned()
    }

    pub fn display(&self) -> std::borrow::Cow<'_, str> {
        if let Some(display_path) = &self.display_path {
            std::borrow::Cow::Borrowed(display_path)
        } else {
            let mut display = self.root.display().to_string();
            for segment in &self.segments {
                display.push_str("!/");
                display.push_str(&segment.display());
            }
            std::borrow::Cow::Owned(display)
        }
    }

    fn hash_identity<H: Hasher>(&self, state: &mut H) {
        self.scheme.hash(state);
        self.provider.hash(state);
        self.root.hash(state);
        self.segments.hash(state);
    }
}

impl LocationSegment {
    fn display(&self) -> String {
        match self {
            LocationSegment::ArchiveMember { path } => path.display().to_string(),
            LocationSegment::Virtual { scheme, path } => {
                format!("{scheme}:{}", path.display())
            }
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
    pub fn id_only(id: LocationId) -> Self {
        Self::Id(id)
    }

    pub fn descriptor_only(descriptor: LocationDescriptor) -> Self {
        Self::Descriptor(descriptor)
    }

    pub fn from_location(location: &Location) -> Self {
        Self::Full {
            id: location.id(),
            descriptor: location.descriptor().clone(),
        }
    }

    pub fn id(&self) -> Option<LocationId> {
        match self {
            LocationRef::Id(id) | LocationRef::Full { id, .. } => Some(*id),
            LocationRef::Descriptor(_) => None,
        }
    }

    pub fn descriptor(&self) -> Option<&LocationDescriptor> {
        match self {
            LocationRef::Descriptor(descriptor) | LocationRef::Full { descriptor, .. } => {
                Some(descriptor)
            }
            LocationRef::Id(_) => None,
        }
    }
}

impl LocationId {
    pub fn from_descriptor(descriptor: &LocationDescriptor) -> Self {
        use rapidhash::fast::RapidHasher;
        let mut hasher = RapidHasher::default();
        descriptor.hash_identity(&mut hasher);
        LocationId(hasher.finish())
    }
}

impl fmt::Display for LocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "location:{}", self.0)
    }
}
