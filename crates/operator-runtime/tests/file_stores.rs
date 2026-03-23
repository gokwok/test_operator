use std::time::SystemTime;

use operator_core::{
    ArtifactId, ElementSource, Snapshot, SnapshotMetadata, Surface, SurfaceKind, UiElement,
};
use operator_runtime::{
    ArtifactStore, FileArtifactStore, FileSessionStore, FileSnapshotStore, NullSessionStore,
    RuntimeConfig, Session, SessionEvent, SessionStatus, SessionStore, SnapshotStore,
};
use tempfile::tempdir;

#[tokio::test]
async fn snapshot_store_round_trips_snapshot_json() {
    let dir = tempdir().unwrap();
    let store = FileSnapshotStore::new(dir.path(), RuntimeConfig::default());
    let snapshot = test_snapshot("snap-1");

    store.save(&snapshot).await.unwrap();

    let loaded = store.get(&snapshot.id).await.unwrap().unwrap();
    let listed = store.list(&snapshot.target).await.unwrap();

    assert_eq!(loaded.id, snapshot.id);
    assert_eq!(listed, vec![snapshot.id.clone()]);
}

#[tokio::test]
async fn file_artifact_store_resolves_runtime_artifact_paths() {
    let dir = tempdir().unwrap();
    let store = FileArtifactStore::new(dir.path());
    let artifact_id = ArtifactId("capture-1.png".into());

    let resolved = store.resolve_artifact(&artifact_id).await.unwrap();

    assert_eq!(resolved, dir.path().join("artifacts").join("capture-1.png"));
}

#[tokio::test]
async fn file_artifact_store_rejects_invalid_artifact_ids() {
    let dir = tempdir().unwrap();
    let store = FileArtifactStore::new(dir.path());

    for invalid_id in [
        "../escape.png",
        "nested/escape.png",
        "nested\\escape.png",
        "/tmp/escape.png",
    ] {
        let error = store
            .resolve_artifact(&ArtifactId(invalid_id.into()))
            .await
            .unwrap_err();

        match error {
            operator_core::OperatorError::Platform(message) => {
                assert!(
                    message.contains("invalid artifact id"),
                    "unexpected message for {invalid_id}: {message}"
                );
            }
            other => panic!("unexpected error for {invalid_id}: {other:?}"),
        }
    }
}

#[tokio::test]
async fn file_snapshot_store_rejects_invalid_artifact_ids() {
    let dir = tempdir().unwrap();
    let store = FileSnapshotStore::new(dir.path(), RuntimeConfig::default());

    for invalid_id in [
        "../escape.png",
        "nested/escape.png",
        "nested\\escape.png",
        "/tmp/escape.png",
    ] {
        let error = store
            .resolve_artifact(&ArtifactId(invalid_id.into()))
            .await
            .unwrap_err();

        match error {
            operator_core::OperatorError::Platform(message) => {
                assert!(
                    message.contains("invalid artifact id"),
                    "unexpected message for {invalid_id}: {message}"
                );
            }
            other => panic!("unexpected error for {invalid_id}: {other:?}"),
        }
    }
}

#[tokio::test]
async fn null_session_store_is_a_safe_noop() {
    let store = NullSessionStore;
    let session = test_session("sess-1");

    store.create(&session).await.unwrap();
    store
        .append(
            &session.id,
            &SessionEvent::UserInput {
                text: "hello".into(),
            },
        )
        .await
        .unwrap();

    assert!(store.get(&session.id).await.unwrap().is_none());
    assert!(store.list(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn file_session_store_round_trips_session_metadata() {
    let dir = tempdir().unwrap();
    let store = FileSessionStore::new(dir.path());
    let session = test_session("sess-1");

    store.create(&session).await.unwrap();
    store
        .append(
            &session.id,
            &SessionEvent::ToolResult {
                name: "observe".into(),
                output: serde_json::json!({ "snapshot_id": "snap-1" }),
            },
        )
        .await
        .unwrap();

    let loaded = FileSessionStore::new(dir.path())
        .get(&session.id)
        .await
        .unwrap()
        .unwrap();
    let listed = FileSessionStore::new(dir.path())
        .list(Some(10))
        .await
        .unwrap();

    assert_eq!(loaded.id, session.id);
    assert_eq!(listed, vec![session.id.clone()]);
    assert!(dir.path().join("sessions").join("sess-1.jsonl").exists());
}

fn test_snapshot(id: &str) -> Snapshot {
    let element_id = operator_core::ElementId("el-1".into());

    Snapshot {
        id: id.into(),
        target: "local:macos".into(),
        surface: Surface {
            kind: SurfaceKind::Frontmost,
        },
        image_artifact: None,
        elements: std::collections::HashMap::from([(
            element_id.clone(),
            UiElement {
                id: element_id.clone(),
                role: "AXWindow".into(),
                label: Some("Operator".into()),
                value: None,
                bounds: None,
                enabled: Some(true),
                children: vec![],
                confidence: Some(1.0),
                source: ElementSource::Native,
            },
        )]),
        root_ids: vec![element_id],
        metadata: SnapshotMetadata {
            platform: "macos".into(),
            display_scale: Some(2.0),
            capture_duration_ms: 8,
        },
        created_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
    }
}

fn test_session(id: &str) -> Session {
    Session {
        id: id.into(),
        created_at: SystemTime::UNIX_EPOCH,
        task: "store smoke".into(),
        status: SessionStatus::Running,
    }
}
