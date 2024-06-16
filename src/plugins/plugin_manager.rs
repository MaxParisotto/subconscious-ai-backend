use crate::Plugin; // Ensure the correct path to the Plugin trait
use std::collections::HashMap;

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: HashMap::new(),
        }
    }

    pub fn register_plugin(&mut self, name: String, plugin: Box<dyn Plugin>) {
        self.plugins.insert(name, plugin);
    }

    pub fn initialize_plugins(&self) {
        for plugin in self.plugins.values() {
            plugin.initialize();
        }
    }

    pub fn execute_plugins(&self) {
        for plugin in self.plugins.values() {
            plugin.execute();
        }
    }
}
