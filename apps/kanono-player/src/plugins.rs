use std::path::Path;

use anyhow::{bail, Result};
use kanono_plugin_api::{ApiVersionFn, CreatePluginFn, DestroyPluginFn, Plugin, PLUGIN_API_VERSION};
use libloading::Library;

/// Owns both the plugin and its library so vtables remain valid until destruction.
#[allow(dead_code)]
pub struct LoadedPlugin {
    _library: Library,
    instance: *mut dyn Plugin,
    destroy: DestroyPluginFn,
}

#[allow(dead_code)]
impl LoadedPlugin {
    pub unsafe fn load(path: impl AsRef<Path>) -> Result<Self> {
        let library = Library::new(path.as_ref())?;
        let api_version: ApiVersionFn = *library.get(b"kanono_plugin_api_version\0")?;
        if api_version() != PLUGIN_API_VERSION {
            bail!("plugin API version does not match host API version");
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
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.instance) };
    }
}