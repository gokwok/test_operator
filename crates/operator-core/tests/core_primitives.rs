use operator_core::{
    ArtifactId, OperatorError, Point, Rect, SnapshotId, TargetConnection, TargetDescriptor,
    TargetId, WindowId,
};

#[test]
fn ids_round_trip_through_display() {
    assert_eq!(SnapshotId("snap-1".into()).to_string(), "snap-1");
    assert_eq!(TargetId("local:macos".into()).to_string(), "local:macos");
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
fn target_descriptor_keeps_connection_details() {
    let descriptor = TargetDescriptor {
        id: TargetId("device:harmony:abc123".into()),
        platform: "harmony".into(),
        device_id: Some("abc123".into()),
        connection: TargetConnection::Bridge {
            endpoint: Some("ws://127.0.0.1:9000".into()),
        },
    };

    assert!(matches!(
        descriptor.connection,
        TargetConnection::Bridge { endpoint: Some(_) }
    ));
}
