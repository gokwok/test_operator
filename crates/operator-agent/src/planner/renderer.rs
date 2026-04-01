use operator_core::ImageSizePx;

use crate::model::ContentBlock;

use super::{PlannerContext, PlannerVisualSlot};

#[derive(Clone, Debug, PartialEq)]
pub struct PlannerVisualInput {
    pub slot: PlannerVisualSlot,
    pub image: ContentBlock,
}

#[derive(Clone, Debug, Default)]
pub struct PlannerRenderer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PlannerRenderHints {
    pub openai_screenshot_coordinate_contract: bool,
}

impl PlannerRenderer {
    pub fn new() -> Self {
        Self
    }

    pub(super) fn render_request(
        &self,
        task: &str,
        planner_context: &PlannerContext,
        visual_inputs: &[PlannerVisualInput],
        hints: PlannerRenderHints,
    ) -> Vec<ContentBlock> {
        let mut content = vec![ContentBlock::Text {
            text: render_request_text(task, planner_context, hints),
        }];

        for visual in visual_inputs {
            content.push(ContentBlock::Text {
                text: visual_label(visual.slot).to_string(),
            });
            content.push(visual.image.clone());
        }

        content
    }
}

fn render_request_text(
    task: &str,
    planner_context: &PlannerContext,
    hints: PlannerRenderHints,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!("Task\n{task}"));
    sections.push(render_target_section(planner_context));
    sections.push(render_history_section(planner_context));
    sections.push(render_observation_section(planner_context, hints));
    if !planner_context.notes.is_empty() {
        sections.push(render_notes_section(planner_context));
    }
    sections.push(render_ui_state_section(planner_context));

    sections.join("\n\n")
}

fn render_target_section(planner_context: &PlannerContext) -> String {
    let target = &planner_context.target;
    let observe_mode = if planner_context.include_elements {
        "element_tree"
    } else {
        "screenshot_only"
    };
    format!(
        "Target\n- id: {}\n- platform: {}\n- capabilities: {}\n- observe verification mode: {}",
        target.id,
        target.platform,
        target.capabilities_text(),
        observe_mode
    )
}

fn render_history_section(planner_context: &PlannerContext) -> String {
    if planner_context.recent_tool_results.is_empty() {
        return "Recent activity\n- none".to_string();
    }

    let items = planner_context
        .recent_tool_results
        .iter()
        .map(|result| format!("- {}", result.render_line()))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Recent activity\n{items}")
}

fn render_observation_section(
    planner_context: &PlannerContext,
    hints: PlannerRenderHints,
) -> String {
    let Some(observation) = planner_context.current_observation.as_ref() else {
        return "Current observation\n- none".to_string();
    };

    let screenshot = observation
        .screenshot_artifact
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".into());
    let mut lines = vec![
        "Current observation".to_string(),
        format!("- snapshot: {}", observation.snapshot_id),
        format!("- surface: {}", observation.surface),
        format!("- roots: {}", observation.root_element_count),
        format!("- elements: {}", observation.element_count),
        format!("- screenshot: {}", screenshot),
    ];

    if hints.openai_screenshot_coordinate_contract && observation.screenshot_artifact.is_some() {
        if let Some(size) = observation.image_size_px {
            lines.push(format!(
                "- screenshot image_size_px: {}",
                format_image_size(size)
            ));
        }
        lines.push(
            "- screenshot coordinate space: original image pixels with origin=(0,0) at the top-left".into(),
        );
    }

    if let Some(digest) = observation.element_digest.as_ref() {
        lines.push(
            "- element digest (SnapshotElement ids; native bounds use device coordinates):".into(),
        );
        lines.extend(
            digest
                .entries
                .iter()
                .map(|entry| format!("  {}", entry.render_line())),
        );
        if digest.truncated_count > 0 {
            lines.push(format!(
                "  - ... {} more element digest entries omitted",
                digest.truncated_count
            ));
        }
    }

    lines.join("\n")
}

fn render_notes_section(planner_context: &PlannerContext) -> String {
    let notes = planner_context
        .notes
        .iter()
        .map(|note| format!("- {note}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Notes\n{notes}")
}

fn render_ui_state_section(planner_context: &PlannerContext) -> String {
    let status = if planner_context.ui_state_stale {
        "yes"
    } else {
        "no"
    };

    let guidance = if planner_context.ui_state_stale {
        if planner_context.include_elements {
            "Do not choose `finish` until the UI is freshly verified by an element-inclusive observe."
        } else {
            "Do not choose `finish` until the UI is freshly verified by a screenshot or observe result."
        }
    } else {
        "A `finish` decision is allowed if the task outcome is already verified."
    };

    format!("UI state\n- stale: {status}\n- guidance: {guidance}")
}

fn visual_label(slot: PlannerVisualSlot) -> &'static str {
    match slot {
        PlannerVisualSlot::Previous => "Previous screenshot (older context).",
        PlannerVisualSlot::Current => "Current screenshot (latest UI state).",
    }
}

fn format_image_size(size: ImageSizePx) -> String {
    format!("{} x {}", size.width, size.height)
}
