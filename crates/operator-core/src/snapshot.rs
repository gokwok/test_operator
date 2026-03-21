use std::{collections::HashMap, time::SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ArtifactId, ElementId, Rect, SnapshotId, Surface, TargetId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub target: TargetId,
    pub surface: Surface,
    pub image_artifact: Option<ArtifactId>,
    pub elements: HashMap<ElementId, UiElement>,
    pub root_ids: Vec<ElementId>,
    pub metadata: SnapshotMetadata,
    #[schemars(with = "String")]
    pub created_at: SystemTime,
    #[schemars(with = "Option<String>")]
    pub expires_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UiElement {
    pub id: ElementId,
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<Rect>,
    pub enabled: Option<bool>,
    pub children: Vec<ElementId>,
    pub confidence: Option<f32>,
    pub source: ElementSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum ElementSource {
    Native,
    Ocr,
    Vision,
    Hybrid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotMetadata {
    pub platform: String,
    pub display_scale: Option<f32>,
    pub capture_duration_ms: u64,
}
