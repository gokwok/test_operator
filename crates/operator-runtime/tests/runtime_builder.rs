use std::sync::Arc;

use operator_core::{Capability, CapabilitySet, TargetConnection, TargetDescriptor, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig, TargetResolver};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};

#[tokio::test]
async fn runtime_builder_registers_multiple_drivers() {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(Arc::new(MockPlatformDriver::new(
            "macos",
            CapabilitySet::new([Capability::Capture]),
        )))
        .register_driver(Arc::new(MockPlatformDriver::new(
            "harmony",
            CapabilitySet::new([Capability::Capture]),
        )))
        .build()
        .await
        .unwrap();

    let (local_target, local_driver) = runtime
        .core()
        .resolve_driver(&TargetId("local:macos".into()))
        .unwrap();
    let (device_target, device_driver) = runtime
        .core()
        .resolve_driver(&TargetId("device:harmony:abc123".into()))
        .unwrap();

    assert_eq!(local_target.platform, "macos");
    assert_eq!(local_target.connection, TargetConnection::Local);
    assert_eq!(local_driver.platform_id(), "macos");

    assert_eq!(
        device_target,
        TargetDescriptor {
            id: TargetId("device:harmony:abc123".into()),
            platform: "harmony".into(),
            device_id: Some("abc123".into()),
            connection: TargetConnection::Bridge { endpoint: None },
        }
    );
    assert_eq!(device_driver.platform_id(), "harmony");
}

#[test]
fn target_resolver_parses_local_and_bridge_targets() {
    let resolver = TargetResolver::new(TargetId("local:macos".into()));

    assert_eq!(
        resolver.resolve(None).unwrap(),
        TargetDescriptor {
            id: TargetId("local:macos".into()),
            platform: "macos".into(),
            device_id: None,
            connection: TargetConnection::Local,
        }
    );
    assert_eq!(
        resolver
            .resolve(Some(&TargetId("device:harmony:abc123".into())))
            .unwrap(),
        TargetDescriptor {
            id: TargetId("device:harmony:abc123".into()),
            platform: "harmony".into(),
            device_id: Some("abc123".into()),
            connection: TargetConnection::Bridge { endpoint: None },
        }
    );
}
