use std::collections::VecDeque;

use operator_core::{ArtifactId, SnapshotId};
use serde::{Deserialize, Serialize};

use crate::session::VisualObservationSummary;

pub const VISUAL_WINDOW_CAP: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObservationCache {
    current_observation: Option<VisualObservationSummary>,
    visual_window: VecDeque<VisualFrame>,
}

impl ObservationCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.visual_window.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visual_window.is_empty()
    }

    pub fn clear(&mut self) {
        self.current_observation = None;
        self.visual_window.clear();
    }

    pub fn current_observation(&self) -> Option<&VisualObservationSummary> {
        self.current_observation.as_ref()
    }

    pub fn current_visual(&self) -> Option<&ArtifactId> {
        self.visual_window.back().map(|frame| &frame.artifact_id)
    }

    pub fn previous_visual(&self) -> Option<&ArtifactId> {
        self.visual_window
            .iter()
            .rev()
            .nth(1)
            .map(|frame| &frame.artifact_id)
    }

    pub fn record(&mut self, summary: VisualObservationSummary) {
        if let Some(artifact_id) = summary.screenshot_artifact.clone() {
            self.visual_window.push_back(VisualFrame {
                snapshot_id: summary.snapshot_id.clone(),
                artifact_id,
            });
            while self.visual_window.len() > VISUAL_WINDOW_CAP {
                self.visual_window.pop_front();
            }
        }

        self.current_observation = Some(summary);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualFrame {
    pub snapshot_id: SnapshotId,
    pub artifact_id: ArtifactId,
}
