use std::{collections::HashMap, time::SystemTime};

use operator_core::{
    ElementId, ElementSource, Snapshot, SnapshotMetadata, Surface, SurfaceKind, UiElement,
};
use operator_runtime::{Session, SessionStatus};

pub fn test_element(id: &str) -> UiElement {
    UiElement {
        id: ElementId(id.into()),
        role: "AXButton".into(),
        label: Some(format!("Test Element {id}")),
        value: None,
        bounds: None,
        enabled: Some(true),
        children: vec![],
        confidence: Some(1.0),
        source: ElementSource::Native,
    }
}

pub fn test_snapshot(id: &str) -> Snapshot {
    let element = test_element("el-1");

    Snapshot {
        id: id.into(),
        target: "local:macos".into(),
        surface: Surface {
            kind: SurfaceKind::Frontmost,
        },
        image_artifact: None,
        elements: HashMap::from([(element.id.clone(), element.clone())]),
        root_ids: vec![element.id.clone()],
        metadata: SnapshotMetadata {
            platform: "macos".into(),
            display_scale: Some(2.0),
            capture_bounds: None,
            capture_duration_ms: 8,
        },
        created_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
    }
}

pub fn test_session(id: &str) -> Session {
    Session {
        id: id.into(),
        created_at: SystemTime::UNIX_EPOCH,
        task: format!("test session {id}"),
        status: SessionStatus::Running,
    }
}
