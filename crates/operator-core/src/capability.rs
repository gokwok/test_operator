use std::collections::HashSet;

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub enum Capability {
    Capture,
    InspectTree,
    InspectText,
    PointerInput,
    KeyboardInput,
    WindowQuery,
    WindowManagement,
    AppLifecycle,
    Clipboard,
    Permissions,
    DeviceInfo,
    Extension(CapabilityId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct CapabilityId {
    pub namespace: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CapabilitySet(HashSet<Capability>);

impl CapabilitySet {
    pub fn new<I>(capabilities: I) -> Self
    where
        I: IntoIterator<Item = Capability>,
    {
        Self(capabilities.into_iter().collect())
    }

    pub fn supports(&self, capability: &Capability) -> bool {
        self.0.contains(capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.0.iter()
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self::new(iter)
    }
}
