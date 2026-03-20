use std::collections::HashSet;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Capability {
    Capture,
    InspectTree,
    InspectText,
    PointerInput,
    KeyboardInput,
    WindowManagement,
    AppLifecycle,
    Clipboard,
    Permissions,
    DeviceInfo,
    Extension(CapabilityId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CapabilityId {
    pub namespace: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self::new(iter)
    }
}
