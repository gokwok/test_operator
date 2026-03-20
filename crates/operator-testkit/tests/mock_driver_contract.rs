use std::time::{Duration, SystemTime};

use operator_core::{
    Capability, CapabilitySet, ExecContext, ObserveRequest, ObserveResult, Surface, SurfaceKind,
};
use operator_runtime::{SessionEvent, SessionStore, SnapshotStore};
use operator_testkit::{
    test_snapshot, InMemorySessionStore, InMemorySnapshotStore, MockPlatformDriver,
};

#[tokio::test]
async fn mock_driver_returns_stubbed_observe_result() {
    let snapshot = test_snapshot("snap-1");
    let driver = MockPlatformDriver::new("mock", CapabilitySet::new([Capability::Capture]));

    driver.push_observe_result(Ok(ObserveResult {
        snapshot: snapshot.clone(),
    }));

    let result = driver
        .observe(
            ObserveRequest {
                surface: Surface {
                    kind: SurfaceKind::Frontmost,
                },
                include_screenshot: true,
                include_elements: true,
            },
            &ExecContext {
                target: "local:mock".into(),
                session: Some("sess-1".into()),
                timeout_ms: Some(250),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.snapshot, snapshot);
}

#[tokio::test]
async fn memory_snapshot_store_lists_saved_ids() {
    let store = InMemorySnapshotStore::new();
    let mut first = test_snapshot("snap-1");
    let mut second = test_snapshot("snap-2");

    first.created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    second.created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(2);

    store.save(&second).await.unwrap();
    store.save(&first).await.unwrap();

    let listed = store.list(&first.target).await.unwrap();

    assert_eq!(listed, vec![first.id.clone(), second.id.clone()]);
}

#[tokio::test]
async fn memory_session_store_keeps_sessions_and_events() {
    let store = InMemorySessionStore::new();
    let session = operator_testkit::test_session("sess-1");

    store.create(&session).await.unwrap();
    store
        .append(
            &session.id,
            &SessionEvent::UserInput {
                text: "inspect app".into(),
            },
        )
        .await
        .unwrap();

    let loaded = store.get(&session.id).await.unwrap().unwrap();
    let listed = store.list(Some(10)).await.unwrap();

    assert_eq!(loaded, session);
    assert_eq!(listed, vec![session.id.clone()]);
    assert_eq!(store.events(&session.id).await.unwrap().len(), 1);
}
