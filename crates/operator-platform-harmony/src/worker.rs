use hmdriver_rs::{Driver, HdcDriverBuilder, UiDriverBuilder};

use crate::HarmonyHdcConfig;

#[derive(Debug, Clone)]
pub struct HarmonyHdcWorker {
    config: HarmonyHdcConfig,
}

impl HarmonyHdcWorker {
    pub fn new(config: HarmonyHdcConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &HarmonyHdcConfig {
        &self.config
    }

    pub fn driver_builder(&self) -> HdcDriverBuilder {
        let mut builder =
            Driver::builder(self.config.addr().to_string()).timeout(self.config.timeout());
        if let Some(connect_key) = self.config.connect_key() {
            builder = builder.connect_key(connect_key.to_string());
        }
        if let Some(key_dir) = self.config.key_dir() {
            builder = builder.key_dir(key_dir.clone());
        }
        builder
    }

    pub fn ui_driver_builder(&self) -> UiDriverBuilder {
        let mut builder =
            UiDriverBuilder::new(self.config.addr().to_string()).timeout(self.config.timeout());
        if let Some(connect_key) = self.config.connect_key() {
            builder = builder.connect_key(connect_key.to_string());
        }
        if let Some(key_dir) = self.config.key_dir() {
            builder = builder.key_dir(key_dir.clone());
        }
        if let Some(agent_path) = self.config.agent_path() {
            builder = builder.agent_path(agent_path.clone());
        }
        if let Some(remote_agent_path) = self.config.remote_agent_path() {
            builder = builder.remote_agent_path(remote_agent_path.to_string());
        }
        builder.startup_delay(self.config.startup_delay())
    }
}
