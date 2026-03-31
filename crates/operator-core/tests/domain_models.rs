use std::{collections::HashMap, time::SystemTime};

use operator_core::{
    Action, Capability, CapabilitySet, ElementSource, ExecContext, Locator, ObserveResult,
    QueryRequest, Snapshot, SnapshotMetadata, Surface, SurfaceKind, TypeTrailingKey, UiElement,
};

#[test]
fn locator_requires_snapshot_context_for_element_lookup() {
    let locator = Locator::SnapshotElement {
        snapshot: "snap-1".into(),
        element: "el-1".into(),
    };

    assert!(matches!(locator, Locator::SnapshotElement { .. }));
}

#[test]
fn capability_extensions_keep_namespace_and_name() {
    let cap = Capability::Extension(operator_core::CapabilityId {
        namespace: "macos",
        name: "menu",
    });

    assert!(format!("{cap:?}").contains("menu"));
}

#[test]
fn action_and_query_variants_are_serializable() {
    let _surface = SurfaceKind::Frontmost;
    let query = QueryRequest::ListWindows {
        app: Some("Safari".into()),
    };
    let action = Action::Type {
        text: "hello".into(),
        clear_before: true,
        delay_ms: Some(25),
        trailing_keys: vec![TypeTrailingKey::Return],
    };

    assert!(serde_json::to_value(&query).is_ok());
    assert!(serde_json::to_value(&action).is_ok());
}

#[test]
fn capability_set_reports_supported_capabilities() {
    let set = CapabilitySet::new([
        Capability::Capture,
        Capability::Extension(operator_core::CapabilityId {
            namespace: "macos",
            name: "menu",
        }),
    ]);

    assert!(set.supports(&Capability::Capture));
}

#[test]
fn snapshot_and_exec_context_keep_typed_state() {
    let element_id = operator_core::ElementId("el-1".into());
    let snapshot = Snapshot {
        id: "snap-1".into(),
        target: "macos".into(),
        surface: Surface {
            kind: SurfaceKind::Frontmost,
        },
        image_artifact: None,
        elements: HashMap::from([(
            element_id.clone(),
            UiElement {
                id: element_id.clone(),
                role: "AXButton".into(),
                label: Some("Continue".into()),
                value: None,
                bounds: None,
                enabled: Some(true),
                children: vec![],
                confidence: Some(0.9),
                source: ElementSource::Native,
            },
        )]),
        root_ids: vec![element_id],
        metadata: SnapshotMetadata {
            platform: "macos".into(),
            display_scale: Some(2.0),
            capture_bounds: None,
            image_size_px: None,
            capture_duration_ms: 12,
        },
        created_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
    };
    let observe = ObserveResult { snapshot };
    let ctx = ExecContext {
        target: "macos".into(),
        session: Some("sess-1".into()),
        timeout_ms: Some(1_000),
    };

    assert_eq!(observe.snapshot.metadata.platform, "macos");
    assert_eq!(ctx.timeout_ms, Some(1_000));
}
