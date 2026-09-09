//! Shared contract for Kanono components loaded from `.so` files.

/// Version checked by the host before it creates a plugin instance.
pub const PLUGIN_API_VERSION: u32 = 1;

/// A capability provided by an externally loaded component.
///
/// Plugin and host must use the same Rust toolchain and this exact crate version.
/// The C entry points make symbol lookup stable; Rust trait objects themselves are
/// not a stable ABI across arbitrary compiler versions.
pub trait Plugin: Send {
    fn metadata(&self) -> PluginMetadata;
    fn activate(&mut self) -> Result<(), PluginError>;
    fn deactivate(&mut self);
}

#[derive(Debug, Clone, Copy)]
pub struct PluginMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone)]
pub struct PluginError(pub String);

impl std::fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PluginError {}

/// Required `kanono_plugin_api_version` symbol signature.
pub type ApiVersionFn = unsafe fn() -> u32;

/// Required `kanono_create_plugin` symbol signature.
pub type CreatePluginFn = unsafe fn() -> *mut dyn Plugin;

/// Required `kanono_destroy_plugin` symbol signature.
pub type DestroyPluginFn = unsafe fn(*mut dyn Plugin);