use kanono_plugin_api::{Plugin, PluginError, PluginMetadata, PLUGIN_API_VERSION};

struct ExampleComponent;

impl Plugin for ExampleComponent {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata { id: "example-component", name: "Example Component", version: "0.1.0" }
    }

    fn activate(&mut self) -> Result<(), PluginError> { Ok(()) }
    fn deactivate(&mut self) {}
}

#[no_mangle]
pub fn kanono_plugin_api_version() -> u32 { PLUGIN_API_VERSION }

#[no_mangle]
pub fn kanono_create_plugin() -> *mut dyn Plugin {
    Box::into_raw(Box::new(ExampleComponent))
}

#[no_mangle]
pub unsafe fn kanono_destroy_plugin(plugin: *mut dyn Plugin) {
    drop(Box::from_raw(plugin));
}