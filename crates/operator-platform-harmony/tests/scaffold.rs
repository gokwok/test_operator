use operator_core::{DriverConfig, TargetDescriptor, TargetId};
use serde_json::json;

use operator_platform_harmony::{HarmonyHdcConfig, HarmonyHdcDriver, HarmonyHdcDriverFactory};
use operator_runtime::PlatformDriverFactory;

#[test]
fn factory_builds_harmony_hdc_scaffold_driver() {
    let factory = HarmonyHdcDriverFactory::new();
    let target = TargetDescriptor {
        id: TargetId("harmony-pc".into()),
        platform: "harmony".into(),
        driver: "harmony.hdc".into(),
        driver_config: DriverConfig::from([("addr".into(), json!("192.168.8.43:35319"))]),
    };

    let driver = factory.build(&target).expect("factory should build");

    assert_eq!(driver.platform_id(), "harmony");
    assert_eq!(driver.driver_id(), "harmony.hdc");
    assert_eq!(
        driver.capabilities(),
        operator_core::CapabilitySet::default()
    );
}

#[test]
fn worker_exposes_upstream_builder_entrypoints_without_connecting() {
    let config = HarmonyHdcConfig::try_from(&DriverConfig::from([
        ("addr".into(), json!("192.168.8.43:35319")),
        ("connect_key".into(), json!("pc-01")),
        ("startup_delay_ms".into(), json!(750_u64)),
    ]))
    .expect("config should parse");
    let driver = HarmonyHdcDriver::new(TargetId("harmony-pc".into()), config);

    let _ = driver.worker().driver_builder();
    let _ = driver.worker().ui_driver_builder();
}
