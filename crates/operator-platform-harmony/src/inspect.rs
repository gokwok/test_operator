#![allow(dead_code)]

use std::collections::HashMap;

use operator_core::{ElementId, ElementSource, OperatorError, Rect, UiElement};
use serde::Deserialize;
use serde_json::Value;

const MAX_SCROLL_CHILDREN: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InspectResult {
    pub(crate) elements: HashMap<ElementId, UiElement>,
    pub(crate) root_ids: Vec<ElementId>,
}

#[derive(Debug, Deserialize)]
struct HarmonyNode {
    #[serde(default)]
    attributes: HarmonyAttributes,
    #[serde(default)]
    children: Vec<HarmonyNode>,
}

#[allow(non_snake_case)]
#[derive(Debug, Default, Deserialize)]
struct HarmonyAttributes {
    #[serde(default)]
    accessibilityId: Option<String>,
    #[serde(default)]
    bounds: Option<String>,
    #[serde(default)]
    checkable: Option<String>,
    #[serde(default)]
    checked: Option<String>,
    #[serde(default)]
    clickable: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    enabled: Option<String>,
    #[serde(default)]
    focused: Option<String>,
    #[serde(default)]
    hashcode: Option<String>,
    #[serde(default)]
    hint: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    longClickable: Option<String>,
    #[serde(default)]
    origBounds: Option<String>,
    #[serde(default)]
    scrollable: Option<String>,
    #[serde(default)]
    selected: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    visible: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct CompactNode {
    id: ElementId,
    role: String,
    label: Option<String>,
    value: Option<String>,
    bounds: Option<Rect>,
    enabled: Option<bool>,
    children: Vec<CompactNode>,
}

pub(crate) fn build_inspect_result(hierarchy: Value) -> Result<InspectResult, OperatorError> {
    let root = serde_json::from_value::<HarmonyNode>(hierarchy).map_err(|error| {
        OperatorError::Platform(format!(
            "failed to decode Harmony dumpLayout hierarchy: {error}"
        ))
    })?;

    let mut roots = Vec::new();
    for (index, child) in root.children.iter().enumerate() {
        roots.extend(compact_node(child, &format!("hm-{index}")));
    }

    if roots.is_empty() {
        roots = compact_node(&root, "hm-root");
    }

    let mut elements = HashMap::new();
    let mut root_ids = Vec::new();
    for root in roots {
        root_ids.push(flatten_compact_node(root, &mut elements));
    }

    Ok(InspectResult { elements, root_ids })
}

fn flatten_compact_node(
    node: CompactNode,
    elements: &mut HashMap<ElementId, UiElement>,
) -> ElementId {
    let child_ids = node
        .children
        .into_iter()
        .map(|child| flatten_compact_node(child, elements))
        .collect::<Vec<_>>();

    elements.insert(
        node.id.clone(),
        UiElement {
            id: node.id.clone(),
            role: node.role,
            label: node.label,
            value: node.value,
            bounds: node.bounds,
            enabled: node.enabled,
            children: child_ids,
            confidence: Some(1.0),
            source: ElementSource::Native,
        },
    );

    node.id
}

fn compact_node(node: &HarmonyNode, path: &str) -> Vec<CompactNode> {
    if !is_visible(node) {
        return Vec::new();
    }

    let mut child_nodes = node
        .children
        .iter()
        .enumerate()
        .flat_map(|(index, child)| compact_node(child, &format!("{path}-{index}")))
        .collect::<Vec<_>>();

    let classification = classify_node(node);
    let bounds = node_bounds(&node.attributes);
    let enabled = parse_optional_bool(&node.attributes.enabled);

    if classification.scrollable && bounds.is_some() {
        child_nodes.truncate(MAX_SCROLL_CHILDREN);
        if !child_nodes.is_empty() {
            return vec![CompactNode {
                id: ElementId(path.into()),
                role: "scroll".into(),
                label: best_label(node),
                value: None,
                bounds,
                enabled,
                children: child_nodes,
            }];
        }
    }

    if classification.interactive && bounds.is_some() {
        let role = classification.role;
        let value = interactive_value(node, &role);
        return vec![CompactNode {
            id: ElementId(path.into()),
            role,
            label: interactive_label(node),
            value,
            bounds,
            enabled,
            children: Vec::new(),
        }];
    }

    if classification.semantic_leaf && bounds.is_some() && child_nodes.is_empty() {
        let role = classification.role;
        let value = semantic_value(node, &role);
        return vec![CompactNode {
            id: ElementId(path.into()),
            role,
            label: best_label(node),
            value,
            bounds,
            enabled,
            children: Vec::new(),
        }];
    }

    child_nodes
}

#[derive(Debug)]
struct NodeClassification {
    role: String,
    interactive: bool,
    scrollable: bool,
    semantic_leaf: bool,
}

fn classify_node(node: &HarmonyNode) -> NodeClassification {
    let clickable = parse_bool(&node.attributes.clickable);
    let long_clickable = parse_bool(&node.attributes.longClickable);
    let checkable = parse_bool(&node.attributes.checkable);
    let scrollable = parse_bool(&node.attributes.scrollable);
    let role = normalized_role(
        node.attributes.r#type.as_deref(),
        clickable,
        checkable,
        scrollable,
        node.attributes.text.as_deref(),
    );
    let has_label = best_label(node).is_some();
    let interactive = clickable
        || long_clickable
        || checkable
        || scrollable
        || matches!(role.as_str(), "button" | "checkbox" | "switch" | "textbox");
    let semantic_leaf = !interactive && has_label;

    NodeClassification {
        role,
        interactive,
        scrollable,
        semantic_leaf,
    }
}

fn normalized_role(
    ty: Option<&str>,
    clickable: bool,
    checkable: bool,
    scrollable: bool,
    text: Option<&str>,
) -> String {
    let ty = ty.unwrap_or("").trim();
    if scrollable {
        return "scroll".into();
    }
    if ty.contains("Switch") {
        return "switch".into();
    }
    if ty.contains("Check") || checkable {
        return "checkbox".into();
    }
    if ty.contains("Edit") || ty.contains("Input") || ty.contains("TextField") {
        return "textbox".into();
    }
    match ty {
        "Text" => {
            if clickable {
                "button".into()
            } else {
                "text".into()
            }
        }
        "Image" => "image".into(),
        "Button" => "button".into(),
        _ => {
            if clickable {
                "button".into()
            } else if trimmed(text).is_some() {
                "text".into()
            } else {
                "generic".into()
            }
        }
    }
}

fn interactive_label(node: &HarmonyNode) -> Option<String> {
    best_label(node).or_else(|| {
        let mut texts = Vec::new();
        gather_descendant_texts(node, &mut texts);
        choose_primary_label(texts)
    })
}

fn interactive_value(node: &HarmonyNode, role: &str) -> Option<String> {
    match role {
        "checkbox" | "switch" => Some(parse_bool(&node.attributes.checked).to_string()),
        "textbox" => trimmed(node.attributes.text.as_deref()),
        _ => None,
    }
}

fn semantic_value(node: &HarmonyNode, role: &str) -> Option<String> {
    match role {
        "textbox" => trimmed(node.attributes.text.as_deref()),
        _ => None,
    }
}

fn best_label(node: &HarmonyNode) -> Option<String> {
    trimmed(node.attributes.text.as_deref())
        .or_else(|| trimmed(node.attributes.description.as_deref()))
        .or_else(|| trimmed(node.attributes.hint.as_deref()))
}

fn gather_descendant_texts(node: &HarmonyNode, out: &mut Vec<(String, Option<Rect>)>) {
    if let Some(text) = trimmed(node.attributes.text.as_deref()) {
        out.push((text, node_bounds(&node.attributes)));
    }
    if let Some(text) = trimmed(node.attributes.description.as_deref()) {
        out.push((text, node_bounds(&node.attributes)));
    }
    for child in &node.children {
        gather_descendant_texts(child, out);
    }
}

fn choose_primary_label(mut texts: Vec<(String, Option<Rect>)>) -> Option<String> {
    if texts.is_empty() {
        return None;
    }

    texts.sort_by(|lhs, rhs| {
        let lhs_y = lhs.1.as_ref().map(|rect| rect.y as i64).unwrap_or(0);
        let rhs_y = rhs.1.as_ref().map(|rect| rect.y as i64).unwrap_or(0);
        lhs_y.cmp(&rhs_y).then_with(|| {
            let lhs_x = lhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            let rhs_x = rhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            lhs_x.cmp(&rhs_x)
        })
    });

    let primary = texts.first().map(|(text, _)| text.clone());
    let top_y = texts
        .first()
        .and_then(|(_, rect)| rect.as_ref().map(|rect| rect.y))
        .unwrap_or_default();
    let secondary = texts
        .iter()
        .filter(|(_, rect)| {
            rect.as_ref()
                .map(|rect| (rect.y - top_y).abs() <= 50.0)
                .unwrap_or(true)
        })
        .max_by(|lhs, rhs| {
            let lhs_x = lhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            let rhs_x = rhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            lhs_x.cmp(&rhs_x)
        })
        .map(|(text, _)| text.clone());

    match (primary, secondary) {
        (Some(primary), Some(secondary)) if primary != secondary => {
            Some(format!("{primary} {secondary}"))
        }
        (Some(primary), _) => Some(primary),
        _ => None,
    }
}

fn node_bounds(attributes: &HarmonyAttributes) -> Option<Rect> {
    attributes
        .bounds
        .as_deref()
        .and_then(parse_bounds)
        .or_else(|| attributes.origBounds.as_deref().and_then(parse_bounds))
}

fn parse_bounds(value: &str) -> Option<Rect> {
    let mut numbers = Vec::with_capacity(4);
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || (ch == '-' && current.is_empty()) {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(number) = current.parse::<f64>() {
                numbers.push(number);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(number) = current.parse::<f64>() {
            numbers.push(number);
        }
    }
    if numbers.len() < 4 {
        return None;
    }
    let (x1, y1, x2, y2) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    if x2 <= x1 || y2 <= y1 {
        return None;
    }
    Some(Rect {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    })
}

fn is_visible(node: &HarmonyNode) -> bool {
    parse_bool_default(&node.attributes.visible, true)
}

fn parse_bool(value: &Option<String>) -> bool {
    matches!(value.as_deref(), Some("true") | Some("1"))
}

fn parse_optional_bool(value: &Option<String>) -> Option<bool> {
    match value.as_deref() {
        Some("true") | Some("1") => Some(true),
        Some("false") | Some("0") => Some(false),
        _ => None,
    }
}

fn parse_bool_default(value: &Option<String>, default: bool) -> bool {
    parse_optional_bool(value).unwrap_or(default)
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::build_inspect_result;
    use operator_core::Rect;
    use serde_json::json;

    #[test]
    fn harmony_hdc_inspect_prunes_wrappers_and_synthesizes_button_labels() {
        let result = build_inspect_result(json!({
            "attributes": {},
            "children": [{
                "attributes": { "type": "Column", "bounds": "[0,0][300,400]" },
                "children": [{
                    "attributes": {
                        "type": "Button",
                        "clickable": "true",
                        "bounds": "[10,20][110,70]",
                        "enabled": "true"
                    },
                    "children": [{
                        "attributes": {
                            "type": "Text",
                            "text": "保存",
                            "bounds": "[20,30][70,50]"
                        }
                    }]
                }]
            }]
        }))
        .expect("inspect result");

        assert_eq!(result.root_ids.len(), 1);
        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("root element should exist");
        assert_eq!(root.role, "button");
        assert_eq!(root.label.as_deref(), Some("保存"));
        assert_eq!(
            root.bounds,
            Some(Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            })
        );
        assert!(root.children.is_empty());
    }

    #[test]
    fn harmony_hdc_inspect_keeps_scroll_roots_with_compact_children() {
        let result = build_inspect_result(json!({
            "attributes": {},
            "children": [{
                "attributes": {
                    "type": "List",
                    "scrollable": "true",
                    "bounds": "[0,0][300,600]"
                },
                "children": [
                    {
                        "attributes": {
                            "type": "Button",
                            "clickable": "true",
                            "bounds": "[0,0][300,80]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "第一项",
                                "bounds": "[20,20][120,50]"
                            }
                        }]
                    },
                    {
                        "attributes": {
                            "type": "Button",
                            "clickable": "true",
                            "bounds": "[0,80][300,160]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "第二项",
                                "bounds": "[20,100][120,130]"
                            }
                        }]
                    }
                ]
            }]
        }))
        .expect("inspect result");

        let scroll = result
            .elements
            .get(&result.root_ids[0])
            .expect("scroll root should exist");
        assert_eq!(scroll.role, "scroll");
        assert_eq!(scroll.children.len(), 2);
        let first = result
            .elements
            .get(&scroll.children[0])
            .expect("first child should exist");
        assert_eq!(first.role, "button");
        assert_eq!(first.label.as_deref(), Some("第一项"));
    }

    #[test]
    fn harmony_hdc_inspect_preserves_semantic_text_leaves() {
        let result = build_inspect_result(json!({
            "attributes": {},
            "children": [{
                "attributes": {
                    "type": "Text",
                    "text": "欢迎回来",
                    "bounds": "[12,32][160,62]",
                    "enabled": "true"
                }
            }]
        }))
        .expect("inspect result");

        assert_eq!(result.root_ids.len(), 1);
        let text = result
            .elements
            .get(&result.root_ids[0])
            .expect("text root should exist");
        assert_eq!(text.role, "text");
        assert_eq!(text.label.as_deref(), Some("欢迎回来"));
        assert_eq!(text.enabled, Some(true));
    }

    #[test]
    fn harmony_hdc_inspect_maps_form_values_and_checked_state() {
        let result = build_inspect_result(json!({
            "attributes": {},
            "children": [
                {
                    "attributes": {
                        "type": "TextInput",
                        "text": "alice@example.com",
                        "bounds": "[10,10][200,60]",
                        "enabled": "true"
                    }
                },
                {
                    "attributes": {
                        "type": "Checkbox",
                        "checkable": "true",
                        "checked": "true",
                        "description": "记住我",
                        "bounds": "[10,80][200,120]"
                    }
                }
            ]
        }))
        .expect("inspect result");

        let textbox = result
            .elements
            .get(&result.root_ids[0])
            .expect("textbox should exist");
        assert_eq!(textbox.role, "textbox");
        assert_eq!(textbox.value.as_deref(), Some("alice@example.com"));

        let checkbox = result
            .elements
            .get(&result.root_ids[1])
            .expect("checkbox should exist");
        assert_eq!(checkbox.role, "checkbox");
        assert_eq!(checkbox.label.as_deref(), Some("记住我"));
        assert_eq!(checkbox.value.as_deref(), Some("true"));
    }
}
