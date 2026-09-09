use std::{fs, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};
use kanono_plugin_api::{ApiVersionFn, CreatePluginFn, DestroyPluginFn, Plugin, PluginMetadata, PLUGIN_API_VERSION};
use libloading::Library;

/// Owns both the plugin and its library so vtables remain valid until destruction.
pub struct LoadedPlugin {
    _library: Library,
    instance: *mut dyn Plugin,
    destroy: DestroyPluginFn,
}

impl LoadedPlugin {
    pub unsafe fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let library = Library::new(path).with_context(|| format!("failed to load {}", path.display()))?;
        let api_version: ApiVersionFn = *library.get(b"kanono_plugin_api_version\0")?;
        if api_version() != PLUGIN_API_VERSION {
            bail!("{} has an incompatible plugin API version", path.display());
        }
        let create: CreatePluginFn = *library.get(b"kanono_create_plugin\0")?;
        let destroy: DestroyPluginFn = *library.get(b"kanono_destroy_plugin\0")?;
        let instance = create();
        if instance.is_null() { bail!("plugin returned a null instance"); }
        Ok(Self { _library: library, instance, destroy })
    }

    pub fn plugin(&mut self) -> &mut dyn Plugin {
        unsafe { &mut *self.instance }
    }

    pub fn metadata(&mut self) -> PluginMetadata {
        self.plugin().metadata()
    }
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        self.plugin().deactivate();
        unsafe { (self.destroy)(self.instance) };
    }
}

pub struct RegisteredPlugin {
    pub metadata: PluginMetadata,
    _loaded: LoadedPlugin,
}

#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<RegisteredPlugin>,
    failures: Vec<String>,
}

impl PluginRegistry {
    /// Scans a directory for native Linux components and activates valid plugins.
    /// A broken component is recorded but does not prevent other components loading.
    pub fn load_components(directory: impl AsRef<Path>) -> Self {
        let directory = directory.as_ref();
        let mut registry = Self::default();
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return registry,
            Err(error) => {
                registry.failures.push(format!("cannot read {}: {error}", directory.display()));
                return registry;
            }
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "so"))
            .collect();
        paths.sort();
        for path in paths {
            if let Err(error) = registry.load_component(&path) {
                registry.failures.push(error.to_string());
            }
        }
        registry
    }

    pub fn plugins(&self) -> &[RegisteredPlugin] {
        &self.plugins
    }

    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    fn load_component(&mut self, path: &Path) -> Result<()> {
        let mut loaded = unsafe { LoadedPlugin::load(path) }?;
        let metadata = loaded.metadata();
        if self.plugins.iter().any(|plugin| plugin.metadata.id == metadata.id) {
            bail!("{} duplicates plugin id {:?}", path.display(), metadata.id);
        }
        loaded.plugin().activate().map_err(|error| anyhow::anyhow!(error))?;
        self.plugins.push(RegisteredPlugin { metadata, _loaded: loaded });
        Ok(())
    }
}