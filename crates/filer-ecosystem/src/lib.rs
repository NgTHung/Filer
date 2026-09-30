//! Ecosystem data model for Filer extensions, packages, and profile sync.
//!
//! This crate is deliberately runtime-free. It defines the wire-safe contracts
//! shared by `filer-core`, `filer-app`, future web clients, extension packages,
//! and profile synchronization. WASM execution, native trusted modules, and UI
//! rendering live in higher-level crates.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionRuntime {
    Wasm,
    NativeTrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    FsRead,
    FsWrite,
    Watch,
    Search,
    Network,
    Process,
    Secrets,
    Ui,
    Provider,
    Sync,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub schema_version: u16,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: ExtensionRuntime,
    pub entrypoint: String,
    #[serde(default)]
    pub permissions: BTreeSet<Permission>,
    #[serde(default)]
    pub commands: Vec<CommandContribution>,
    #[serde(default)]
    pub events: Vec<EventContribution>,
    #[serde(default)]
    pub ui: Vec<UiContribution>,
    #[serde(default)]
    pub preview_providers: Vec<PreviewContribution>,
    #[serde(default)]
    pub metadata_providers: Vec<MetadataContribution>,
    #[serde(default)]
    pub converters: Vec<ConverterContribution>,
    #[serde(default)]
    pub themes: Vec<ThemeContribution>,
    #[serde(default)]
    pub icon_packs: Vec<IconPackContribution>,
    #[serde(default)]
    pub providers: Vec<ProviderContribution>,
    #[serde(default)]
    pub sync: Option<SyncParticipation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandContribution {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub surfaces: BTreeSet<CommandSurface>,
    #[serde(default)]
    pub required_permissions: BTreeSet<Permission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSurface {
    CommandPalette,
    ContextMenu,
    Toolbar,
    Sidebar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventContribution {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiContribution {
    pub id: String,
    pub title: String,
    pub surface: UiSurface,
    #[serde(default)]
    pub required_permissions: BTreeSet<Permission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSurface {
    Panel,
    SidebarSection,
    StatusBadge,
    FileRowDecoration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewContribution {
    pub id: String,
    #[serde(default)]
    pub mime_categories: BTreeSet<String>,
    #[serde(default)]
    pub extensions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataContribution {
    pub id: String,
    #[serde(default)]
    pub mime_categories: BTreeSet<String>,
    #[serde(default)]
    pub extensions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConverterContribution {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub from_extensions: BTreeSet<String>,
    #[serde(default)]
    pub to_extensions: BTreeSet<String>,
    #[serde(default)]
    pub required_permissions: BTreeSet<Permission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeContribution {
    pub id: String,
    pub title: String,
    pub token_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconPackContribution {
    pub id: String,
    pub title: String,
    pub manifest_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderContribution {
    pub scheme: String,
    pub title: String,
    #[serde(default)]
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub read: bool,
    pub write: bool,
    pub watch: bool,
    pub search: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncParticipation {
    #[serde(default)]
    pub sync_settings: bool,
    #[serde(default)]
    pub sync_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPackage {
    pub package_schema_version: u16,
    pub manifest: ExtensionManifest,
    #[serde(default)]
    pub files: Vec<PackageFile>,
    #[serde(default)]
    pub signature: Option<PackageSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSignature {
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOperation {
    pub id: String,
    pub schema_version: u16,
    pub source_client_id: String,
    pub timestamp_ms: u64,
    pub kind: ProfileOperationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileOperationKind {
    InstallPackage {
        extension_id: String,
        package_path: String,
    },
    RemovePackage {
        extension_id: String,
    },
    EnableExtension {
        extension_id: String,
    },
    DisableExtension {
        extension_id: String,
    },
    SetExtensionConfig {
        extension_id: String,
        key: String,
        value_json: String,
    },
    SetTheme {
        theme_id: String,
    },
    AddProviderProfile {
        profile_id: String,
        provider_scheme: String,
    },
    UpdateWorkspace {
        workspace_id: String,
        value_json: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EcosystemRegistry {
    extensions: BTreeMap<String, ExtensionManifest>,
    command_owners: BTreeMap<String, String>,
}

impl EcosystemRegistry {
    pub fn load(
        manifests: impl IntoIterator<Item = ExtensionManifest>,
    ) -> Result<Self, EcosystemError> {
        let mut registry = Self::default();
        for manifest in manifests {
            registry.insert(manifest)?;
        }
        Ok(registry)
    }

    pub fn insert(&mut self, manifest: ExtensionManifest) -> Result<(), EcosystemError> {
        validate_manifest(&manifest)?;

        if self.extensions.contains_key(&manifest.id) {
            return Err(EcosystemError::DuplicateExtensionId(manifest.id));
        }

        for command in &manifest.commands {
            if let Some(owner) = self.command_owners.get(&command.key) {
                return Err(EcosystemError::DuplicateCommandKey {
                    key: command.key.clone(),
                    first_extension: owner.clone(),
                    second_extension: manifest.id.clone(),
                });
            }
        }

        for command in &manifest.commands {
            self.command_owners
                .insert(command.key.clone(), manifest.id.clone());
        }
        self.extensions.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ExtensionManifest> {
        self.extensions.get(id)
    }

    pub fn command_owner(&self, key: &str) -> Option<&str> {
        self.command_owners.get(key).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileState {
    pub installed: BTreeSet<String>,
    pub enabled: BTreeSet<String>,
    pub settings: BTreeMap<(String, String), String>,
    pub theme_id: Option<String>,
    pub provider_profiles: BTreeMap<String, String>,
    pub workspaces: BTreeMap<String, String>,
    applied_ops: BTreeSet<String>,
}

impl ProfileState {
    pub fn apply(&mut self, op: ProfileOperation) -> Result<bool, EcosystemError> {
        if op.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(EcosystemError::UnsupportedSchemaVersion(op.schema_version));
        }
        if !self.applied_ops.insert(op.id) {
            return Ok(false);
        }

        match op.kind {
            ProfileOperationKind::InstallPackage { extension_id, .. } => {
                self.installed.insert(extension_id);
            }
            ProfileOperationKind::RemovePackage { extension_id } => {
                self.installed.remove(&extension_id);
                self.enabled.remove(&extension_id);
                self.settings.retain(|(id, _), _| id != &extension_id);
            }
            ProfileOperationKind::EnableExtension { extension_id } => {
                if self.installed.contains(&extension_id) {
                    self.enabled.insert(extension_id);
                }
            }
            ProfileOperationKind::DisableExtension { extension_id } => {
                self.enabled.remove(&extension_id);
            }
            ProfileOperationKind::SetExtensionConfig {
                extension_id,
                key,
                value_json,
            } => {
                self.settings.insert((extension_id, key), value_json);
            }
            ProfileOperationKind::SetTheme { theme_id } => {
                self.theme_id = Some(theme_id);
            }
            ProfileOperationKind::AddProviderProfile {
                profile_id,
                provider_scheme,
            } => {
                self.provider_profiles.insert(profile_id, provider_scheme);
            }
            ProfileOperationKind::UpdateWorkspace {
                workspace_id,
                value_json,
            } => {
                self.workspaces.insert(workspace_id, value_json);
            }
        }
        Ok(true)
    }
}

pub fn validate_manifest(manifest: &ExtensionManifest) -> Result<(), EcosystemError> {
    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(EcosystemError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }
    validate_id("extension id", &manifest.id)?;
    validate_non_empty("name", &manifest.name)?;
    validate_non_empty("version", &manifest.version)?;
    validate_non_empty("entrypoint", &manifest.entrypoint)?;

    let mut local_commands = BTreeSet::new();
    for command in &manifest.commands {
        validate_key("command key", &command.key)?;
        validate_non_empty("command title", &command.title)?;
        require_declared_permissions(&manifest.permissions, &command.required_permissions)?;
        if !local_commands.insert(command.key.clone()) {
            return Err(EcosystemError::DuplicateCommandKey {
                key: command.key.clone(),
                first_extension: manifest.id.clone(),
                second_extension: manifest.id.clone(),
            });
        }
    }

    for event in &manifest.events {
        validate_key("event key", &event.key)?;
    }
    for ui in &manifest.ui {
        validate_id("ui contribution id", &ui.id)?;
        validate_non_empty("ui contribution title", &ui.title)?;
        require_declared_permissions(&manifest.permissions, &ui.required_permissions)?;
    }
    for preview in &manifest.preview_providers {
        validate_id("preview provider id", &preview.id)?;
    }
    for metadata in &manifest.metadata_providers {
        validate_id("metadata provider id", &metadata.id)?;
    }
    for converter in &manifest.converters {
        validate_id("converter id", &converter.id)?;
        validate_non_empty("converter title", &converter.title)?;
        require_declared_permissions(&manifest.permissions, &converter.required_permissions)?;
    }
    for theme in &manifest.themes {
        validate_id("theme id", &theme.id)?;
        validate_non_empty("theme title", &theme.title)?;
        validate_package_path(&theme.token_file)?;
    }
    for icon_pack in &manifest.icon_packs {
        validate_id("icon pack id", &icon_pack.id)?;
        validate_non_empty("icon pack title", &icon_pack.title)?;
        validate_package_path(&icon_pack.manifest_file)?;
    }
    for provider in &manifest.providers {
        validate_key("provider scheme", &provider.scheme)?;
        validate_non_empty("provider title", &provider.title)?;
        if !manifest.permissions.contains(&Permission::Provider) {
            return Err(EcosystemError::MissingPermission {
                permission: Permission::Provider,
            });
        }
    }
    if manifest.sync.is_some() && !manifest.permissions.contains(&Permission::Sync) {
        return Err(EcosystemError::MissingPermission {
            permission: Permission::Sync,
        });
    }

    Ok(())
}

pub fn validate_package(package: &ExtensionPackage) -> Result<(), EcosystemError> {
    if package.package_schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(EcosystemError::UnsupportedSchemaVersion(
            package.package_schema_version,
        ));
    }
    validate_manifest(&package.manifest)?;

    let mut paths = BTreeSet::new();
    for file in &package.files {
        validate_package_path(&file.path)?;
        validate_sha256(&file.sha256)?;
        if !paths.insert(file.path.clone()) {
            return Err(EcosystemError::DuplicatePackagePath(file.path.clone()));
        }
    }
    Ok(())
}

fn require_declared_permissions(
    declared: &BTreeSet<Permission>,
    required: &BTreeSet<Permission>,
) -> Result<(), EcosystemError> {
    for permission in required {
        if !declared.contains(permission) {
            return Err(EcosystemError::MissingPermission {
                permission: *permission,
            });
        }
    }
    Ok(())
}

fn validate_id(label: &'static str, value: &str) -> Result<(), EcosystemError> {
    validate_non_empty(label, value)?;
    if value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(EcosystemError::InvalidIdentifier {
            label,
            value: value.to_string(),
        })
    }
}

fn validate_key(label: &'static str, value: &str) -> Result<(), EcosystemError> {
    validate_non_empty(label, value)?;
    if value.split('.').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    }) {
        Ok(())
    } else {
        Err(EcosystemError::InvalidIdentifier {
            label,
            value: value.to_string(),
        })
    }
}

fn validate_non_empty(label: &'static str, value: &str) -> Result<(), EcosystemError> {
    if value.trim().is_empty() {
        Err(EcosystemError::MissingField(label))
    } else {
        Ok(())
    }
}

fn validate_package_path(path: &str) -> Result<(), EcosystemError> {
    validate_non_empty("package path", path)?;
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized.contains(':')
        || normalized
            .split('/')
            .any(|part| part == ".." || part.is_empty())
    {
        return Err(EcosystemError::UnsafePackagePath(path.to_string()));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), EcosystemError> {
    if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(EcosystemError::InvalidSha256(value.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcosystemError {
    UnsupportedSchemaVersion(u16),
    MissingField(&'static str),
    InvalidIdentifier {
        label: &'static str,
        value: String,
    },
    MissingPermission {
        permission: Permission,
    },
    DuplicateExtensionId(String),
    DuplicateCommandKey {
        key: String,
        first_extension: String,
        second_extension: String,
    },
    UnsafePackagePath(String),
    DuplicatePackagePath(String),
    InvalidSha256(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest(id: &str) -> ExtensionManifest {
        ExtensionManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            id: id.to_string(),
            name: "Git Tools".to_string(),
            version: "0.1.0".to_string(),
            runtime: ExtensionRuntime::Wasm,
            entrypoint: "extension.wasm".to_string(),
            permissions: BTreeSet::from([Permission::FsRead, Permission::Ui]),
            commands: vec![CommandContribution {
                key: "git.status".to_string(),
                title: "Git Status".to_string(),
                surfaces: BTreeSet::from([CommandSurface::CommandPalette]),
                required_permissions: BTreeSet::from([Permission::FsRead]),
            }],
            events: vec![EventContribution {
                key: "git.status_changed".to_string(),
            }],
            ui: vec![UiContribution {
                id: "git-panel".to_string(),
                title: "Git".to_string(),
                surface: UiSurface::Panel,
                required_permissions: BTreeSet::from([Permission::Ui]),
            }],
            preview_providers: Vec::new(),
            metadata_providers: Vec::new(),
            converters: Vec::new(),
            themes: Vec::new(),
            icon_packs: Vec::new(),
            providers: Vec::new(),
            sync: None,
        }
    }

    #[test]
    fn manifest_roundtrips_json() {
        let manifest = base_manifest("git-tools");
        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: ExtensionManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn registry_rejects_duplicate_command_keys() {
        let first = base_manifest("git-tools");
        let mut second = base_manifest("git-tools-2");
        second.commands[0].title = "Git Status Again".to_string();

        let error = EcosystemRegistry::load([first, second]).unwrap_err();
        assert!(matches!(
            error,
            EcosystemError::DuplicateCommandKey { key, .. } if key == "git.status"
        ));
    }

    #[test]
    fn manifest_rejects_undeclared_permission_use() {
        let mut manifest = base_manifest("git-tools");
        manifest.commands[0]
            .required_permissions
            .insert(Permission::Network);

        let error = validate_manifest(&manifest).unwrap_err();
        assert_eq!(
            error,
            EcosystemError::MissingPermission {
                permission: Permission::Network
            }
        );
    }

    #[test]
    fn provider_contribution_requires_provider_permission() {
        let mut manifest = base_manifest("sftp-provider");
        manifest.providers.push(ProviderContribution {
            scheme: "sftp".to_string(),
            title: "SFTP".to_string(),
            capabilities: ProviderCapabilities {
                read: true,
                write: true,
                watch: false,
                search: false,
            },
        });

        let error = validate_manifest(&manifest).unwrap_err();
        assert_eq!(
            error,
            EcosystemError::MissingPermission {
                permission: Permission::Provider
            }
        );
    }

    #[test]
    fn package_rejects_path_traversal() {
        let package = ExtensionPackage {
            package_schema_version: SUPPORTED_SCHEMA_VERSION,
            manifest: base_manifest("git-tools"),
            files: vec![PackageFile {
                path: "../extension.wasm".to_string(),
                sha256: "a".repeat(64),
                size_bytes: 12,
            }],
            signature: None,
        };

        let error = validate_package(&package).unwrap_err();
        assert_eq!(
            error,
            EcosystemError::UnsafePackagePath("../extension.wasm".to_string())
        );
    }

    #[test]
    fn profile_operations_are_idempotent() {
        let op = ProfileOperation {
            id: "op-1".to_string(),
            schema_version: SUPPORTED_SCHEMA_VERSION,
            source_client_id: "desktop".to_string(),
            timestamp_ms: 1,
            kind: ProfileOperationKind::InstallPackage {
                extension_id: "git-tools".to_string(),
                package_path: "git-tools.filerpack".to_string(),
            },
        };

        let mut state = ProfileState::default();
        assert!(state.apply(op.clone()).unwrap());
        assert!(!state.apply(op).unwrap());
        assert!(state.installed.contains("git-tools"));
    }
}
