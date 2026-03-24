use std::sync::Arc;

use operator_agent::tools::ToolExecutor;
use operator_core::{Capability, CapabilitySet, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};

#[tokio::test]
async fn catalog_filters_tools_by_target_capability_and_preserves_runtime_schema() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::Capture, Capability::InspectTree]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver)
        .build()
        .await
        .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());

    let catalog = executor
        .catalog(&TargetId("local:macos".into()))
        .expect("catalog should resolve for the target");

    let names = catalog
        .iter()
        .map(|spec| spec.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"artifact-get"));
    assert!(names.contains(&"snapshot-get"));
    assert!(names.contains(&"observe"));
    assert!(names.contains(&"get-focus"));
    assert!(names.contains(&"capabilities"));
    assert!(!names.contains(&"list-apps"));
    assert!(!names.contains(&"click"));

    let observe = catalog
        .iter()
        .find(|spec| spec.name == "observe")
        .expect("observe should remain available");
    assert!(observe.read_only);

    let runtime_observe = runtime
        .tools()
        .specs()
        .into_iter()
        .find(|spec| spec.name == "observe")
        .expect("runtime should keep observe");
    assert_eq!(observe.description, runtime_observe.description);
    assert_eq!(observe.input_schema, runtime_observe.input_schema);
}

#[tokio::test]
async fn catalog_hides_side_effect_tools_when_runtime_policy_disables_them() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([
            Capability::Capture,
            Capability::InspectTree,
            Capability::PointerInput,
        ]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        allow_side_effects: false,
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .register_driver(driver)
    .build()
    .await
    .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());

    let catalog = executor
        .catalog(&TargetId("local:macos".into()))
        .expect("catalog should resolve for the target");

    let names = catalog
        .iter()
        .map(|spec| spec.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"observe"));
    assert!(!names.contains(&"click"));
    assert!(!names.contains(&"move"));
    assert!(!names.contains(&"drag"));
}
