use operator_core::{
    ArtifactId, DriverConfig, OperatorError, Point, Rect, SnapshotId, TargetDescriptor, TargetId,
    WindowId,
};
use serde_json::json;

#[test]
fn ids_round_trip_through_display() {
    assert_eq!(SnapshotId("snap-1".into()).to_string(), "snap-1");
    assert_eq!(TargetId("macos".into()).to_string(), "macos");
    assert_eq!(ArtifactId("artifact-1".into()).to_string(), "artifact-1");
    assert_eq!(WindowId(42).to_string(), "42");
}

#[test]
fn operator_error_messages_are_stable() {
    let err = OperatorError::TargetNotFound("missing".into());
    assert!(err.to_string().contains("missing"));
}

#[test]
fn geometry_types_hold_coordinates() {
    let point = Point { x: 10.0, y: 20.0 };
    let rect = Rect {
        x: point.x,
        y: point.y,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!(rect.x, 10.0);
    assert_eq!(rect.height, 40.0);
}

#[test]
fn target_descriptor_keeps_driver_details() {
    let descriptor = TargetDescriptor {
        id: TargetId("harmony-phone".into()),
        platform: "harmony".into(),
        driver: "harmony.node".into(),
        driver_config: DriverConfig::from([("node".into(), json!("serial-1"))]),
    };

    assert_eq!(descriptor.driver, "harmony.node");
    assert_eq!(
        descriptor.driver_config.get("node"),
        Some(&json!("serial-1"))
    );
}
