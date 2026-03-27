use std::sync::Arc;

use operator_core::{Capability, CapabilitySet, TargetDescriptor, TargetId};
use operator_runtime::{NamedTargetConfig, RuntimeBuilder, RuntimeConfig, TargetResolver};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};

#[tokio::test]
async fn runtime_builder_registers_multiple_drivers() {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_drivers(vec![
            Arc::new(MockPlatformDriver::new(
                "macos",
                CapabilitySet::new([Capability::Capture]),
            )) as Arc<dyn operator_core::PlatformDriver>,
            Arc::new(MockPlatformDriver::with_driver_id(
                "harmony",
                "harmony.bridge",
                CapabilitySet::new([Capability::Capture]),
            )) as Arc<dyn operator_core::PlatformDriver>,
        ])
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
    assert_eq!(local_target.driver, "macos.system");
    assert_eq!(local_driver.platform_id(), "macos");
    assert_eq!(local_driver.driver_id(), "macos.system");

    assert_eq!(
        device_target,
        TargetDescriptor {
            id: TargetId("device:harmony:abc123".into()),
            platform: "harmony".into(),
            driver: "harmony.bridge".into(),
        }
    );
    assert_eq!(device_driver.platform_id(), "harmony");
    assert_eq!(device_driver.driver_id(), "harmony.bridge");
}

#[test]
fn target_resolver_prefers_named_targets_and_falls_back_to_legacy_syntax() {
    let resolver = TargetResolver::new(
        TargetId("macos".into()),
        std::collections::BTreeMap::from([
            (
                "macos".into(),
                NamedTargetConfig {
                    platform: "macos".into(),
                    driver: "macos.system".into(),
                },
            ),
            (
                "windows-lab".into(),
                NamedTargetConfig {
                    platform: "windows".into(),
                    driver: "windows.remote".into(),
                },
            ),
            (
                "harmony-phone".into(),
                NamedTargetConfig {
                    platform: "harmony".into(),
                    driver: "harmony.node".into(),
                },
            ),
        ]),
    );

    assert_eq!(
        resolver.resolve(None).unwrap(),
        TargetDescriptor {
            id: TargetId("macos".into()),
            platform: "macos".into(),
            driver: "macos.system".into(),
        }
    );
    assert_eq!(
        resolver
            .resolve(Some(&TargetId("windows-lab".into())))
            .unwrap(),
        TargetDescriptor {
            id: TargetId("windows-lab".into()),
            platform: "windows".into(),
            driver: "windows.remote".into(),
        }
    );
    assert_eq!(
        resolver
            .resolve(Some(&TargetId("harmony-phone".into())))
            .unwrap(),
        TargetDescriptor {
            id: TargetId("harmony-phone".into()),
            platform: "harmony".into(),
            driver: "harmony.node".into(),
        }
    );
    assert_eq!(
        resolver
            .resolve(Some(&TargetId("device:harmony:abc123".into())))
            .unwrap(),
        TargetDescriptor {
            id: TargetId("device:harmony:abc123".into()),
            platform: "harmony".into(),
            driver: "harmony.bridge".into(),
        }
    );
}
