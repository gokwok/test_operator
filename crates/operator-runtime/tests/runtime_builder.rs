use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use operator_core::{
    Capability, CapabilitySet, DriverConfig, OperatorError, PlatformDriver, TargetDescriptor,
    TargetId,
};
use operator_runtime::{
    NamedTargetConfig, PlatformDriverFactory, RuntimeBuilder, RuntimeConfig, TargetResolver,
};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;

#[tokio::test]
async fn runtime_builder_registers_multiple_drivers() {
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("macos".into()),
        targets: BTreeMap::from([
            (
                "macos".into(),
                NamedTargetConfig {
                    platform: "macos".into(),
                    driver: "macos.system".into(),
                    description: None,
                    driver_config: DriverConfig::new(),
                },
            ),
            (
                "harmony-phone".into(),
                NamedTargetConfig {
                    platform: "harmony".into(),
                    driver: "harmony.bridge".into(),
                    description: None,
                    driver_config: DriverConfig::new(),
                },
            ),
        ]),
        ..RuntimeConfig::default()
    })
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
        .resolve_driver(&TargetId("harmony-phone".into()))
        .unwrap();

    assert_eq!(local_target.platform, "macos");
    assert_eq!(local_target.driver, "macos.system");
    assert_eq!(local_driver.platform_id(), "macos");
    assert_eq!(local_driver.driver_id(), "macos.system");

    assert_eq!(
        device_target,
        TargetDescriptor {
            id: TargetId("harmony-phone".into()),
            platform: "harmony".into(),
            driver: "harmony.bridge".into(),
            driver_config: DriverConfig::new(),
        }
    );
    assert_eq!(device_driver.platform_id(), "harmony");
    assert_eq!(device_driver.driver_id(), "harmony.bridge");
}

#[tokio::test]
async fn resolve_driver_keeps_target_not_found_for_undefined_named_target() {
    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .build()
        .await
        .unwrap();

    let error = runtime
        .core()
        .resolve_driver(&TargetId("missing-target".into()));

    match error {
        Err(error) => match error {
            OperatorError::TargetNotFound(target) => assert_eq!(target, "missing-target"),
            other => panic!("expected target not found, got {other:?}"),
        },
        Ok(_) => panic!("unknown target should fail before driver lookup"),
    }
}

#[tokio::test]
async fn resolve_driver_reports_driver_unavailable_for_known_target() {
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("windows-lab".into()),
        targets: BTreeMap::from([(
            "windows-lab".into(),
            NamedTargetConfig {
                platform: "windows".into(),
                driver: "windows.remote".into(),
                description: None,
                driver_config: DriverConfig::new(),
            },
        )]),
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .build()
    .await
    .unwrap();

    let error = runtime
        .core()
        .resolve_driver(&TargetId("windows-lab".into()));

    match error {
        Err(error) => match error {
            OperatorError::DriverUnavailable { target, driver } => {
                assert_eq!(target, "windows-lab");
                assert_eq!(driver, "windows.remote");
            }
            other => panic!("expected driver unavailable, got {other:?}"),
        },
        Ok(_) => panic!("missing driver registry entry should fail"),
    }
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
                    description: None,
                    driver_config: DriverConfig::new(),
                },
            ),
            (
                "windows-lab".into(),
                NamedTargetConfig {
                    platform: "windows".into(),
                    driver: "windows.remote".into(),
                    description: None,
                    driver_config: DriverConfig::from([(
                        "endpoint".into(),
                        json!("https://windows-lab.internal"),
                    )]),
                },
            ),
            (
                "harmony-phone".into(),
                NamedTargetConfig {
                    platform: "harmony".into(),
                    driver: "harmony.node".into(),
                    description: None,
                    driver_config: DriverConfig::from([("node".into(), json!("serial-1"))]),
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
            driver_config: DriverConfig::new(),
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
            driver_config: DriverConfig::from([(
                "endpoint".into(),
                json!("https://windows-lab.internal"),
            )]),
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
            driver_config: DriverConfig::from([("node".into(), json!("serial-1"))]),
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
            driver_config: DriverConfig::from([("device_id".into(), json!("abc123"))]),
        }
    );
}

#[test]
fn named_target_config_rejects_unknown_top_level_fields() {
    let error = serde_json::from_value::<NamedTargetConfig>(json!({
        "platform": "windows",
        "driver": "windows.remote",
        "endpoint": "https://windows-lab.internal"
    }))
    .expect_err("top-level driver-specific fields should be rejected");

    assert!(
        error.to_string().contains("unknown field `endpoint`"),
        "unexpected serde error: {error}"
    );
}

#[tokio::test]
async fn runtime_builder_passes_driver_config_through_factory_initialization() {
    let seen_targets = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("harmony-phone".into()),
        targets: BTreeMap::from([(
            "harmony-phone".into(),
            NamedTargetConfig {
                platform: "harmony".into(),
                driver: "harmony.node".into(),
                description: None,
                driver_config: DriverConfig::from([
                    ("node".into(), json!("serial-1")),
                    ("endpoint".into(), json!("ws://127.0.0.1:9000")),
                ]),
            },
        )]),
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .register_factory(Arc::new(RecordingFactory::new(
        "harmony",
        "harmony.node",
        Arc::clone(&seen_targets),
    )))
    .build()
    .await
    .unwrap();

    let (first_target, first_driver) = runtime
        .core()
        .resolve_driver(&TargetId("harmony-phone".into()))
        .unwrap();
    let (_, second_driver) = runtime
        .core()
        .resolve_driver(&TargetId("harmony-phone".into()))
        .unwrap();

    assert_eq!(first_driver.driver_id(), "harmony.node");
    assert!(Arc::ptr_eq(&first_driver, &second_driver));
    assert_eq!(
        first_target.driver_config,
        DriverConfig::from([
            ("endpoint".into(), json!("ws://127.0.0.1:9000")),
            ("node".into(), json!("serial-1")),
        ])
    );

    let seen_targets = seen_targets.lock().expect("recording mutex poisoned");
    assert_eq!(
        seen_targets.len(),
        1,
        "factory should only run once per target"
    );
    assert_eq!(seen_targets[0], first_target);
}

#[tokio::test]
async fn resolve_driver_preserves_driver_config_validation_errors() {
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        default_target: TargetId("windows-lab".into()),
        targets: BTreeMap::from([(
            "windows-lab".into(),
            NamedTargetConfig {
                platform: "windows".into(),
                driver: "windows.remote".into(),
                description: None,
                driver_config: DriverConfig::from([(
                    "endpoint".into(),
                    json!("wss://windows-lab.internal"),
                )]),
            },
        )]),
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .register_driver(Arc::new(MockPlatformDriver::with_driver_id(
        "windows",
        "windows.remote",
        CapabilitySet::new([Capability::Capture]),
    )))
    .build()
    .await
    .unwrap();

    let error = runtime
        .core()
        .resolve_driver(&TargetId("windows-lab".into()));

    match error {
        Err(error) => match error {
            OperatorError::Platform(message) => {
                assert!(message.contains("windows.remote"));
                assert!(message.contains("does not accept target-level driver_config"));
            }
            other => panic!("expected platform validation error, got {other:?}"),
        },
        Ok(_) => panic!("static driver should reject unexpected driver_config"),
    }
}

struct RecordingFactory {
    driver: Arc<dyn PlatformDriver>,
    seen_targets: Arc<Mutex<Vec<TargetDescriptor>>>,
}

impl RecordingFactory {
    fn new(
        platform_id: &'static str,
        driver_id: &str,
        seen_targets: Arc<Mutex<Vec<TargetDescriptor>>>,
    ) -> Self {
        Self {
            driver: Arc::new(MockPlatformDriver::with_driver_id(
                platform_id,
                driver_id,
                CapabilitySet::new([Capability::Capture]),
            )),
            seen_targets,
        }
    }
}

impl PlatformDriverFactory for RecordingFactory {
    fn driver_id(&self) -> &str {
        self.driver.driver_id()
    }

    fn build(&self, target: &TargetDescriptor) -> Result<Arc<dyn PlatformDriver>, OperatorError> {
        self.seen_targets
            .lock()
            .expect("recording mutex poisoned")
            .push(target.clone());
        Ok(Arc::clone(&self.driver))
    }
}
