use operator_agent::{
    session::{LoopState, VisualObservationSummary},
    tools::ObservationCache,
};
use operator_core::{ArtifactId, SessionId, SnapshotId, TargetId};

fn observation(snapshot_id: &str, artifact_id: &str) -> VisualObservationSummary {
    VisualObservationSummary {
        snapshot_id: SnapshotId(snapshot_id.into()),
        surface: "frontmost".into(),
        screenshot_artifact: Some(ArtifactId(artifact_id.into())),
        image_size_px: None,
        root_element_count: 0,
        element_count: 0,
        element_digest: None,
    }
}

#[test]
fn observation_cache_evicts_oldest_visual_when_window_exceeds_two_entries() {
    let mut cache = ObservationCache::new();
    cache.record(observation("snap-1", "capture-1.png"));
    cache.record(observation("snap-2", "capture-2.png"));
    cache.record(observation("snap-3", "capture-3.png"));

    assert_eq!(cache.len(), 2);
    assert_eq!(
        cache.previous_visual(),
        Some(&ArtifactId("capture-2.png".into()))
    );
    assert_eq!(
        cache.current_visual(),
        Some(&ArtifactId("capture-3.png".into()))
    );
    assert_eq!(
        cache
            .current_observation()
            .map(|summary| summary.snapshot_id.clone()),
        Some(SnapshotId("snap-3".into()))
    );
}

#[test]
fn loop_state_exposes_current_and_previous_visual_from_observation_cache() {
    let mut state = LoopState::new(
        SessionId("sess-loop".into()),
        TargetId("macos".into()),
        "Inspect the current screen",
    );

    state.record_visual_observation(observation("snap-1", "capture-1.png"));
    assert_eq!(state.previous_visual(), None);
    assert_eq!(
        state.current_visual(),
        Some(&ArtifactId("capture-1.png".into()))
    );

    state.record_visual_observation(observation("snap-2", "capture-2.png"));
    assert_eq!(
        state.previous_visual(),
        Some(&ArtifactId("capture-1.png".into()))
    );
    assert_eq!(
        state.current_visual(),
        Some(&ArtifactId("capture-2.png".into()))
    );
    assert_eq!(
        state
            .current_observation()
            .map(|summary| summary.snapshot_id.clone()),
        Some(SnapshotId("snap-2".into()))
    );
}
