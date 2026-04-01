#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use operator_core::{ElementId, ElementSource, OperatorError, Rect, UiElement};
use serde::Deserialize;
use serde_json::Value;

const MAX_SCROLL_CHILDREN: usize = 40;
const MAX_CONTAINER_CHILDREN: usize = 80;
const SAME_ROW_Y_TOLERANCE: f64 = 50.0;
const MAX_SECONDARY_LABEL_CHARS: usize = 20;
const MAX_COMBINED_LABEL_CHARS: usize = 36;

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
struct ReducedNode {
    role: String,
    label: Option<String>,
    value: Option<String>,
    bounds: Option<Rect>,
    enabled: Option<bool>,
    children: Vec<ReducedNode>,
}

#[derive(Debug, Clone)]
struct NodeSemantics {
    role: String,
    bounds: Option<Rect>,
    enabled: Option<bool>,
    interactive: bool,
    scrollable: bool,
    structural: bool,
    keep_self: bool,
    label: Option<String>,
    value: Option<String>,
}

pub(crate) fn build_inspect_result(hierarchy: Value) -> Result<InspectResult, OperatorError> {
    let root = serde_json::from_value::<HarmonyNode>(hierarchy).map_err(|error| {
        OperatorError::Platform(format!(
            "failed to decode Harmony dumpLayout hierarchy: {error}"
        ))
    })?;

    let mut top_nodes = root
        .children
        .iter()
        .flat_map(|child| reduce_node(child, None))
        .collect::<Vec<_>>();
    if top_nodes.is_empty() {
        top_nodes = reduce_node(&root, None);
    }
    if top_nodes.is_empty() {
        return Ok(InspectResult {
            elements: HashMap::new(),
            root_ids: Vec::new(),
        });
    }

    let root_node = ReducedNode {
        role: "window".into(),
        label: semantic_label(&root.attributes, None, "window"),
        value: None,
        bounds: node_bounds(&root.attributes).or_else(|| union_bounds(&top_nodes)),
        enabled: parse_optional_bool(&root.attributes.enabled),
        children: top_nodes,
    };

    let mut elements = HashMap::new();
    let root_id = flatten_reduced_tree(root_node, &mut elements, "ax-0");
    Ok(InspectResult {
        elements,
        root_ids: vec![root_id],
    })
}

pub(crate) fn filter_inspect_result_to_region(
    result: InspectResult,
    region: Rect,
) -> InspectResult {
    let mut filtered = HashMap::new();
    let root_ids = result
        .root_ids
        .iter()
        .filter_map(|id| filter_element_to_region(&result.elements, &mut filtered, id, region))
        .collect::<Vec<_>>();

    InspectResult {
        elements: filtered,
        root_ids,
    }
}

fn flatten_reduced_tree(
    node: ReducedNode,
    elements: &mut HashMap<ElementId, UiElement>,
    path: &str,
) -> ElementId {
    let id = ElementId(path.into());
    let child_ids = node
        .children
        .into_iter()
        .enumerate()
        .map(|(index, child)| flatten_reduced_tree(child, elements, &format!("{path}-{index}")))
        .collect::<Vec<_>>();

    elements.insert(
        id.clone(),
        UiElement {
            id: id.clone(),
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

    id
}

fn filter_element_to_region(
    elements: &HashMap<ElementId, UiElement>,
    filtered: &mut HashMap<ElementId, UiElement>,
    id: &ElementId,
    region: Rect,
) -> Option<ElementId> {
    let element = elements.get(id)?;
    let children = element
        .children
        .iter()
        .filter_map(|child| filter_element_to_region(elements, filtered, child, region))
        .collect::<Vec<_>>();
    let intersects = element
        .bounds
        .is_some_and(|bounds| rects_intersect(bounds, region));
    let structural_shell = matches!(
        element.role.as_str(),
        "window" | "dialog" | "toolbar" | "nav" | "tabbar" | "group" | "list" | "scroll"
    ) && element.label.is_none()
        && element.value.is_none();
    let keep = !children.is_empty() || (intersects && !structural_shell);
    if !keep {
        return None;
    }

    let mut element = element.clone();
    element.children = children;
    filtered.insert(id.clone(), element);
    Some(id.clone())
}

fn reduce_node(node: &HarmonyNode, parent: Option<&HarmonyNode>) -> Vec<ReducedNode> {
    if !is_visible(node) {
        return Vec::new();
    }

    let mut children = node
        .children
        .iter()
        .flat_map(|child| reduce_node(child, Some(node)))
        .collect::<Vec<_>>();
    let semantics = classify_node(node, parent, &children);

    if !semantics.keep_self {
        return children;
    }

    if semantics.scrollable {
        children.truncate(MAX_SCROLL_CHILDREN);
    } else if semantics.structural {
        children.truncate(MAX_CONTAINER_CHILDREN);
    }

    let mut reduced = ReducedNode {
        role: semantics.role.clone(),
        label: semantics.label.clone(),
        value: semantics.value.clone(),
        bounds: semantics.bounds,
        enabled: semantics.enabled,
        children,
    };

    reduced = collapse_redundant_wrapper(reduced);

    if should_absorb_decorative_children(&reduced) {
        if reduced.label.is_none() {
            reduced.label = label_from_children(&reduced.children);
        }
        reduced.children.clear();
    }

    if should_absorb_auxiliary_textbox_children(&reduced) {
        reduced.children.clear();
    }

    reduced = collapse_redundant_wrapper(reduced);

    if is_unusable_leaf_button(&reduced) {
        return Vec::new();
    }

    if should_collapse_transparent_wrapper(&reduced) {
        return reduced.children;
    }

    vec![reduced]
}

fn classify_node(
    node: &HarmonyNode,
    parent: Option<&HarmonyNode>,
    children: &[ReducedNode],
) -> NodeSemantics {
    let clickable = parse_bool(&node.attributes.clickable);
    let long_clickable = parse_bool(&node.attributes.longClickable);
    let checkable = parse_bool(&node.attributes.checkable);
    let scrollable = parse_bool(&node.attributes.scrollable);
    let ty = node.attributes.r#type.as_deref().unwrap_or("").trim();
    let mut role = normalized_role(
        Some(ty),
        clickable,
        checkable,
        scrollable,
        node.attributes.text.as_deref(),
    );
    let bounds = node_bounds(&node.attributes);
    let enabled = parse_optional_bool(&node.attributes.enabled);
    let has_children = !children.is_empty();
    if role == "button"
        && clickable
        && is_layout_container_type(ty)
        && actionable_descendant_count(children) >= 2
    {
        role = layout_container_role(ty);
    }
    if role == "generic" && has_children && bounds.is_some() {
        role = "group".into();
    }
    let interactive = clickable
        || long_clickable
        || checkable
        || matches!(role.as_str(), "button" | "checkbox" | "switch" | "textbox");
    let structural = matches!(
        role.as_str(),
        "window" | "dialog" | "toolbar" | "nav" | "tabbar" | "group" | "list" | "scroll"
    );
    let label = node_label(node, parent, &role);
    let value = node_value(node, &role);
    let has_payload = label.is_some() || value.is_some();
    let opaque_clickable_wrapper = role == "button"
        && label.is_none()
        && value.is_none()
        && (children.is_empty()
            || (is_layout_container_type(ty)
                && children.iter().all(|child| {
                    child.children.is_empty() && matches!(child.role.as_str(), "image" | "group")
                })));
    let keep_self = bounds.is_some()
        && !opaque_clickable_wrapper
        && (interactive
            || scrollable
            || matches!(
                role.as_str(),
                "checkbox" | "switch" | "textbox" | "text" | "image" | "dialog"
            )
            || (structural && has_children)
            || children.len() >= 2
            || (has_payload && has_children)
            || (has_payload && !matches!(role.as_str(), "generic")));

    NodeSemantics {
        role,
        bounds,
        enabled,
        interactive,
        scrollable,
        structural,
        keep_self,
        label,
        value,
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
    if ty.contains("Dialog") || ty.contains("Popup") {
        return "dialog".into();
    }
    if ty.contains("SearchField") || ty == "Search" {
        return "textbox".into();
    }
    if ty.contains("ToolBar") || ty.contains("TitleBar") || ty.contains("AppBar") {
        return "toolbar".into();
    }
    if ty.contains("Nav") || ty.contains("Navigation") {
        return "nav".into();
    }
    if ty.contains("TabBar") || ty.contains("Tabs") {
        return "tabbar".into();
    }
    if scrollable {
        if ty.contains("List") || ty.contains("Grid") {
            return "list".into();
        }
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
    if ty.contains("ListItem") || ty.contains("GridItem") {
        return if clickable {
            "button".into()
        } else {
            "group".into()
        };
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
        "Column" | "Row" | "Flex" | "Stack" | "RelativeContainer" | "RelativeLayout"
        | "FrameNode" | "Swiper" | "GridRow" | "GridCol" => {
            if clickable {
                "button".into()
            } else {
                "group".into()
            }
        }
        _ => {
            if clickable {
                "button".into()
            } else if meaningful_text(text).is_some() {
                "text".into()
            } else {
                "generic".into()
            }
        }
    }
}

fn is_layout_container_type(ty: &str) -> bool {
    matches!(
        ty,
        "Column"
            | "Row"
            | "Flex"
            | "Stack"
            | "RelativeContainer"
            | "RelativeLayout"
            | "FrameNode"
            | "Swiper"
            | "GridRow"
            | "GridCol"
    )
}

fn layout_container_role(ty: &str) -> String {
    if ty.contains("Nav") || ty.contains("Navigation") {
        "nav".into()
    } else if ty.contains("ToolBar") || ty.contains("TitleBar") || ty.contains("AppBar") {
        "toolbar".into()
    } else {
        "group".into()
    }
}

fn actionable_descendant_count(children: &[ReducedNode]) -> usize {
    let mut count = 0;
    for child in children {
        count += actionable_node_count(child);
        if count >= 2 {
            break;
        }
    }
    count
}

fn actionable_node_count(node: &ReducedNode) -> usize {
    let self_count = usize::from(matches!(
        node.role.as_str(),
        "button" | "textbox" | "checkbox" | "switch" | "list" | "scroll"
    ));
    let mut total = self_count;
    for child in &node.children {
        total += actionable_node_count(child);
        if total >= 2 {
            break;
        }
    }
    total
}

fn node_label(node: &HarmonyNode, parent: Option<&HarmonyNode>, role: &str) -> Option<String> {
    match role {
        "textbox" => textbox_label(node, parent, role),
        "button" | "checkbox" | "switch" => control_label(node, parent, role),
        _ => semantic_label(&node.attributes, parent, role),
    }
}

fn textbox_label(node: &HarmonyNode, parent: Option<&HarmonyNode>, role: &str) -> Option<String> {
    let value = node_value(node, role);
    direct_semantic_label(&node.attributes, role)
        .or_else(|| explicit_label_from_ids(&node.attributes, parent, role))
        .or_else(|| descendant_label(node).filter(|label| Some(label.as_str()) != value.as_deref()))
        .or_else(|| generic_label_from_ids(&node.attributes, parent, role))
}

fn control_label(node: &HarmonyNode, parent: Option<&HarmonyNode>, role: &str) -> Option<String> {
    direct_semantic_label(&node.attributes, role)
        .or_else(|| explicit_label_from_ids(&node.attributes, parent, role))
        .or_else(|| descendant_label(node))
        .or_else(|| generic_label_from_ids(&node.attributes, parent, role))
}

fn semantic_label(
    attributes: &HarmonyAttributes,
    parent: Option<&HarmonyNode>,
    role: &str,
) -> Option<String> {
    direct_semantic_label(attributes, role)
        .or_else(|| explicit_label_from_ids(attributes, parent, role))
        .or_else(|| generic_label_from_ids(attributes, parent, role))
}

fn direct_semantic_label(attributes: &HarmonyAttributes, role: &str) -> Option<String> {
    match role {
        "textbox" => meaningful_text(attributes.description.as_deref())
            .or_else(|| meaningful_text(attributes.hint.as_deref())),
        _ => meaningful_text(attributes.text.as_deref())
            .or_else(|| meaningful_text(attributes.description.as_deref()))
            .or_else(|| meaningful_text(attributes.hint.as_deref())),
    }
}

fn explicit_label_from_ids(
    attributes: &HarmonyAttributes,
    parent: Option<&HarmonyNode>,
    role: &str,
) -> Option<String> {
    explicit_fallback_label_from_id(attributes.id.as_deref(), role).or_else(|| {
        parent.and_then(|parent| {
            explicit_fallback_label_from_id(parent.attributes.id.as_deref(), role)
        })
    })
}

fn generic_label_from_ids(
    attributes: &HarmonyAttributes,
    parent: Option<&HarmonyNode>,
    role: &str,
) -> Option<String> {
    generic_fallback_label_from_id(attributes.id.as_deref(), role).or_else(|| {
        parent.and_then(|parent| {
            generic_fallback_label_from_id(parent.attributes.id.as_deref(), role)
        })
    })
}

fn node_value(node: &HarmonyNode, role: &str) -> Option<String> {
    match role {
        "checkbox" | "switch" => Some(parse_bool(&node.attributes.checked).to_string()),
        "textbox" => meaningful_text(node.attributes.text.as_deref()),
        _ => None,
    }
}

fn descendant_label(node: &HarmonyNode) -> Option<String> {
    let mut texts = Vec::new();
    gather_descendant_texts(node, &mut texts);
    choose_primary_label(texts)
}

fn label_from_children(children: &[ReducedNode]) -> Option<String> {
    let texts = children
        .iter()
        .filter_map(|child| child.label.clone().map(|label| (label, child.bounds)))
        .collect::<Vec<_>>();
    choose_primary_label(texts)
}

fn gather_descendant_texts(node: &HarmonyNode, out: &mut Vec<(String, Option<Rect>)>) {
    for candidate in [
        meaningful_text(node.attributes.text.as_deref()),
        meaningful_text(node.attributes.description.as_deref()),
        meaningful_text(node.attributes.hint.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        out.push((candidate, node_bounds(&node.attributes)));
    }

    for child in &node.children {
        gather_descendant_texts(child, out);
    }
}

fn choose_primary_label(texts: Vec<(String, Option<Rect>)>) -> Option<String> {
    let mut texts = dedupe_text_candidates(texts);
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

    let primary = texts.first().map(|(text, _)| text.clone())?;
    let top_y = texts
        .first()
        .and_then(|(_, rect)| rect.as_ref().map(|rect| rect.y))
        .unwrap_or_default();
    let secondary = texts
        .iter()
        .skip(1)
        .filter(|(_, rect)| {
            rect.as_ref()
                .map(|rect| (rect.y - top_y).abs() <= SAME_ROW_Y_TOLERANCE)
                .unwrap_or(true)
        })
        .max_by(|lhs, rhs| {
            let lhs_x = lhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            let rhs_x = rhs.1.as_ref().map(|rect| rect.x as i64).unwrap_or(0);
            lhs_x.cmp(&rhs_x)
        })
        .map(|(text, _)| text.clone());

    match secondary {
        Some(secondary)
            if secondary != primary
                && primary.chars().count() <= MAX_SECONDARY_LABEL_CHARS
                && secondary.chars().count() <= MAX_SECONDARY_LABEL_CHARS
                && primary.chars().count() + secondary.chars().count()
                    < MAX_COMBINED_LABEL_CHARS =>
        {
            Some(format!("{primary} {secondary}"))
        }
        _ => Some(primary),
    }
}

fn dedupe_text_candidates(texts: Vec<(String, Option<Rect>)>) -> Vec<(String, Option<Rect>)> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(texts.len());
    for (text, rect) in texts {
        if seen.insert(text.clone()) {
            deduped.push((text, rect));
        }
    }
    deduped
}

fn collapse_redundant_wrapper(mut node: ReducedNode) -> ReducedNode {
    loop {
        if node.children.len() != 1 {
            return node;
        }

        let can_collapse = {
            let child = &node.children[0];
            let same_bounds = bounds_substantially_overlap(node.bounds, child.bounds);
            let compatible_labels =
                labels_compatible(node.label.as_deref(), child.label.as_deref());
            let compatible_values =
                values_compatible(node.value.as_deref(), child.value.as_deref());

            let redundant_interactive = matches!(node.role.as_str(), "button" | "textbox")
                && matches!(child.role.as_str(), "button" | "textbox")
                && same_bounds
                && compatible_labels
                && compatible_values;
            let redundant_structural = matches!(node.role.as_str(), "group" | "nav" | "toolbar")
                && matches!(child.role.as_str(), "group" | "nav" | "toolbar")
                && node.label.is_none()
                && node.value.is_none()
                && same_bounds;

            redundant_interactive || redundant_structural
        };

        if !can_collapse {
            return node;
        }

        let mut child = node.children.remove(0);
        if child.label.is_none() {
            child.label = node.label.take();
        }
        if child.value.is_none() {
            child.value = node.value.take();
        }
        if child.enabled.is_none() {
            child.enabled = node.enabled;
        }
        node = child;
    }
}

fn should_absorb_decorative_children(node: &ReducedNode) -> bool {
    matches!(node.role.as_str(), "button" | "checkbox" | "switch")
        && !node.children.is_empty()
        && node.children.iter().all(|child| {
            child.children.is_empty() && matches!(child.role.as_str(), "text" | "image")
        })
}

fn should_absorb_auxiliary_textbox_children(node: &ReducedNode) -> bool {
    node.role == "textbox"
        && node.children.iter().all(|child| {
            matches!(child.role.as_str(), "textbox" | "text" | "image" | "button")
                && child.children.is_empty()
                && child
                    .bounds
                    .zip(node.bounds)
                    .map(|(child_bounds, node_bounds)| {
                        child_bounds.width * child_bounds.height
                            <= node_bounds.width * node_bounds.height
                    })
                    .unwrap_or(true)
        })
}

fn should_collapse_transparent_wrapper(node: &ReducedNode) -> bool {
    matches!(node.role.as_str(), "group" | "nav" | "toolbar")
        && node.label.is_none()
        && node.value.is_none()
        && node.children.len() == 1
        && node
            .children
            .first()
            .and_then(|child| child.bounds)
            .is_some()
}

fn is_unusable_leaf_button(node: &ReducedNode) -> bool {
    node.role == "button"
        && node.label.is_none()
        && node.value.is_none()
        && node.children.is_empty()
}

fn labels_compatible(parent: Option<&str>, child: Option<&str>) -> bool {
    match (parent, child) {
        (None, _) | (_, None) => true,
        (Some(parent), Some(child)) => parent == child,
    }
}

fn values_compatible(parent: Option<&str>, child: Option<&str>) -> bool {
    match (parent, child) {
        (None, _) | (_, None) => true,
        (Some(parent), Some(child)) => parent == child,
    }
}

fn bounds_substantially_overlap(parent: Option<Rect>, child: Option<Rect>) -> bool {
    match (parent, child) {
        (Some(parent), Some(child)) => {
            let intersection = rect_intersection(parent, child);
            let child_area = child.width * child.height;
            let parent_area = parent.width * parent.height;
            child_area > 0.0
                && parent_area > 0.0
                && intersection / child_area >= 0.85
                && intersection / parent_area >= 0.55
        }
        _ => false,
    }
}

fn rect_intersection(lhs: Rect, rhs: Rect) -> f64 {
    let left = lhs.x.max(rhs.x);
    let top = lhs.y.max(rhs.y);
    let right = (lhs.x + lhs.width).min(rhs.x + rhs.width);
    let bottom = (lhs.y + lhs.height).min(rhs.y + rhs.height);

    if left < right && top < bottom {
        (right - left) * (bottom - top)
    } else {
        0.0
    }
}

fn union_bounds(nodes: &[ReducedNode]) -> Option<Rect> {
    let mut iter = nodes.iter().filter_map(|node| node.bounds);
    let first = iter.next()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x + first.width;
    let mut bottom = first.y + first.height;

    for bounds in iter {
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x + bounds.width);
        bottom = bottom.max(bounds.y + bounds.height);
    }

    Some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
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

fn rects_intersect(lhs: Rect, rhs: Rect) -> bool {
    let left = lhs.x.max(rhs.x);
    let top = lhs.y.max(rhs.y);
    let right = (lhs.x + lhs.width).min(rhs.x + rhs.width);
    let bottom = (lhs.y + lhs.height).min(rhs.y + rhs.height);

    left < right && top < bottom
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

fn meaningful_text(value: Option<&str>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_accessibility_instruction(trimmed) {
        return None;
    }

    if trimmed.chars().any(is_meaningful_label_char) || is_symbolic_label(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn is_accessibility_instruction(value: &str) -> bool {
    value.contains("单指双击即可执行")
        || value.contains("单指双击即可显示按钮")
        || value.contains("单指双击即可隐藏按钮")
        || value.contains("双击并按住即可弹出更多选项")
        || value.contains("双击并按住左滑可进行更多操作")
        || value.contains("double tap")
            && (value.contains("activate") || value.contains("more options"))
}

fn is_meaningful_label_char(ch: char) -> bool {
    ch.is_alphanumeric()
        || matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x3040..=0x30FF
                | 0xAC00..=0xD7AF
        )
}

fn is_symbolic_label(value: &str) -> bool {
    matches!(value, "+" | "-" | "×" | "x" | "X" | "..." | "…" | "⋮" | "⋯")
}

fn explicit_fallback_label_from_id(id: Option<&str>, _role: &str) -> Option<String> {
    let id = id?.trim();
    if id.is_empty() {
        return None;
    }

    match id {
        "sideButton" => Some("toggle sidebar".into()),
        "createNoteTop" => Some("create note".into()),
        "changeStyleMode" => Some("change style mode".into()),
        "more" => Some("more".into()),
        "quitEditor" => Some("quit editor".into()),
        "unDo" => Some("undo".into()),
        "reDo" => Some("redo".into()),
        "richMore" => Some("rich text more".into()),
        "saveNote" => Some("save note".into()),
        "EnhanceMaximizeBtn" => Some("maximize window".into()),
        "EnhanceMinimizeBtn" => Some("minimize window".into()),
        "EnhanceCloseBtn" => Some("close window".into()),
        "richTodo" => Some("todo".into()),
        "richStylus" => Some("stylus".into()),
        "pcRichFontStyle" => Some("font style".into()),
        "richFontStyles" => Some("font styles".into()),
        "getPicture" => Some("insert picture".into()),
        "funMore" => Some("more tools".into()),
        "getAIFunctionsNew" => Some("ai functions".into()),
        "__SearchField__searchComponent" | "searchComponent" => Some("搜索".into()),
        "title_area_NoteEditorManager" => Some("title".into()),
        "content_area_NoteEditorManager" => Some("content".into()),
        _ => None,
    }
}

fn generic_fallback_label_from_id(id: Option<&str>, role: &str) -> Option<String> {
    let id = id?.trim();
    if id.is_empty() || looks_opaque_identifier(id) {
        return None;
    }

    humanize_identifier(id, role)
}

fn looks_opaque_identifier(id: &str) -> bool {
    id.starts_with("note_item_")
        || id.contains('$')
        || id.chars().filter(|ch| ch.is_ascii_hexdigit()).count() > 20
        || !id.chars().any(char::is_alphabetic)
}

fn humanize_identifier(id: &str, role: &str) -> Option<String> {
    let mut words = Vec::new();
    let mut token = String::new();
    let mut previous_lowercase = false;

    for ch in id.chars() {
        if ch == '_' || ch == '-' || ch == ':' || ch == '.' {
            if !token.is_empty() {
                words.push(std::mem::take(&mut token));
            }
            previous_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_lowercase && !token.is_empty() {
            words.push(std::mem::take(&mut token));
        }

        token.push(ch.to_ascii_lowercase());
        previous_lowercase = ch.is_ascii_lowercase();
    }

    if !token.is_empty() {
        words.push(token);
    }

    words.retain(|word| {
        !word.is_empty()
            && !word.chars().any(|ch| ch.is_ascii_digit())
            && !matches!(
                word.as_str(),
                "btn"
                    | "button"
                    | "component"
                    | "common"
                    | "view"
                    | "area"
                    | "cover"
                    | "manager"
                    | "new"
            )
    });

    if words.is_empty() {
        return None;
    }

    if role == "textbox" && words == ["searchfield", "search"] {
        return Some("搜索".into());
    }

    Some(words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{build_inspect_result, filter_inspect_result_to_region};
    use operator_core::Rect;
    use serde_json::json;

    #[test]
    fn harmony_hdc_inspect_builds_single_window_root_and_button_leaf() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][300,400]" },
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

        assert_eq!(result.root_ids, vec!["ax-0".into()]);
        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        assert_eq!(root.role, "window");
        assert_eq!(root.children.len(), 1);

        let button = result
            .elements
            .get(&root.children[0])
            .expect("button child should exist");
        assert_eq!(button.id.0, "ax-0-0");
        assert_eq!(button.role, "button");
        assert_eq!(button.label.as_deref(), Some("保存"));
        assert_eq!(button.enabled, Some(true));
        assert!(button.children.is_empty());
    }

    #[test]
    fn harmony_hdc_inspect_keeps_list_structure_under_window_root() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][300,600]" },
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

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let list = result
            .elements
            .get(&root.children[0])
            .expect("list child should exist");
        assert_eq!(list.role, "list");
        assert_eq!(list.children.len(), 2);

        let first = result
            .elements
            .get(&list.children[0])
            .expect("first list item should exist");
        assert_eq!(first.role, "button");
        assert_eq!(first.label.as_deref(), Some("第一项"));
    }

    #[test]
    fn harmony_hdc_inspect_preserves_multi_child_groups() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][360,240]" },
            "children": [{
                "attributes": {
                    "type": "Column",
                    "bounds": "[0,0][360,240]"
                },
                "children": [
                    {
                        "attributes": {
                            "type": "Text",
                            "text": "欢迎回来",
                            "bounds": "[12,20][140,50]"
                        }
                    },
                    {
                        "attributes": {
                            "type": "Button",
                            "clickable": "true",
                            "bounds": "[12,80][180,132]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "继续",
                                "bounds": "[32,96][72,118]"
                            }
                        }]
                    }
                ]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let group = result
            .elements
            .get(&root.children[0])
            .expect("group child should exist");
        assert_eq!(group.role, "group");
        assert_eq!(group.children.len(), 2);

        let title = result
            .elements
            .get(&group.children[0])
            .expect("title should exist");
        assert_eq!(title.role, "text");
        assert_eq!(title.label.as_deref(), Some("欢迎回来"));
    }

    #[test]
    fn harmony_hdc_inspect_maps_form_values_and_checked_state() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][300,180]" },
            "children": [
                {
                    "attributes": {
                        "type": "TextInput",
                        "text": "alice@example.com",
                        "hint": "邮箱",
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

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let textbox = result
            .elements
            .get(&root.children[0])
            .expect("textbox should exist");
        assert_eq!(textbox.role, "textbox");
        assert_eq!(textbox.label.as_deref(), Some("邮箱"));
        assert_eq!(textbox.value.as_deref(), Some("alice@example.com"));

        let checkbox = result
            .elements
            .get(&root.children[1])
            .expect("checkbox should exist");
        assert_eq!(checkbox.role, "checkbox");
        assert_eq!(checkbox.label.as_deref(), Some("记住我"));
        assert_eq!(checkbox.value.as_deref(), Some("true"));
    }

    #[test]
    fn harmony_hdc_inspect_uses_descendant_title_for_clickable_note_rows() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Stack",
                    "description": "单指双击即可执行 双击并按住即可弹出更多选项。 双击并按住左滑可进行更多操作。",
                    "clickable": "true",
                    "bounds": "[0,0][420,76]"
                },
                "children": [{
                    "attributes": {
                        "type": "Row",
                        "clickable": "true",
                        "bounds": "[0,0][420,76]"
                    },
                    "children": [{
                        "attributes": {
                            "type": "Column",
                            "id": "note_item_foo",
                            "bounds": "[0,0][420,76]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "4/30天气",
                                "id": "noteListTitle",
                                "bounds": "[45,15][180,50]"
                            }
                        }]
                    }]
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let row = result
            .elements
            .get(&root.children[0])
            .expect("row button should exist");
        assert_eq!(row.role, "button");
        assert_eq!(row.label.as_deref(), Some("4/30天气"));
    }

    #[test]
    fn harmony_hdc_inspect_maps_search_field_to_textbox_and_collapses_wrapper() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Search",
                    "clickable": "true",
                    "bounds": "[0,0][420,76]",
                    "id": "searchComponent"
                },
                "children": [{
                    "attributes": {
                        "type": "SearchField",
                        "clickable": "true",
                        "scrollable": "true",
                        "hint": "搜索",
                        "bounds": "[10,0][390,76]",
                        "id": "__SearchField__searchComponent"
                    }
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let search = result
            .elements
            .get(&root.children[0])
            .expect("search field should exist");
        assert_eq!(search.role, "textbox");
        assert_eq!(search.label.as_deref(), Some("搜索"));
        assert!(search.children.is_empty());
    }

    #[test]
    fn harmony_hdc_inspect_uses_button_id_fallback_for_unlabeled_toolbar_controls() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Button",
                    "clickable": "true",
                    "bounds": "[0,0][76,76]",
                    "id": "saveNote"
                }
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let button = result
            .elements
            .get(&root.children[0])
            .expect("button should exist");
        assert_eq!(button.role, "button");
        assert_eq!(button.label.as_deref(), Some("save note"));
    }

    #[test]
    fn harmony_hdc_inspect_uses_parent_id_fallback_for_anonymous_clickable_glyph_buttons() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Row",
                    "bounds": "[0,0][120,76]",
                    "id": "pcRichFontStyle"
                },
                "children": [{
                    "attributes": {
                        "type": "Column",
                        "clickable": "true",
                        "bounds": "[0,0][76,76]"
                    },
                    "children": [{
                        "attributes": {
                            "type": "SymbolGlyph",
                            "bounds": "[15,15][61,61]"
                        }
                    }]
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let button = root
            .children
            .iter()
            .find_map(|id| result.elements.get(id))
            .and_then(|candidate| {
                if candidate.role == "button" {
                    Some(candidate)
                } else {
                    candidate
                        .children
                        .iter()
                        .find_map(|id| result.elements.get(id))
                }
            })
            .expect("button should exist");
        assert_eq!(button.role, "button");
        assert_eq!(button.label.as_deref(), Some("font style"));
    }

    #[test]
    fn harmony_hdc_inspect_promotes_clickable_list_items_with_descendant_text() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,180]" },
            "children": [{
                "attributes": {
                    "type": "List",
                    "scrollable": "true",
                    "bounds": "[0,0][420,180]"
                },
                "children": [{
                    "attributes": {
                        "type": "ListItem",
                        "clickable": "true",
                        "bounds": "[0,0][420,76]"
                    },
                    "children": [{
                        "attributes": {
                            "type": "Text",
                            "text": "探索",
                            "bounds": "[40,18][120,56]"
                        }
                    }]
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let list = result
            .elements
            .get(&root.children[0])
            .expect("list should exist");
        let item = result
            .elements
            .get(&list.children[0])
            .expect("list item should exist");
        assert_eq!(item.role, "button");
        assert_eq!(item.label.as_deref(), Some("探索"));
    }

    #[test]
    fn harmony_hdc_inspect_prefers_descendant_hint_over_generic_search_identifier() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Search",
                    "clickable": "true",
                    "bounds": "[0,0][420,76]",
                    "id": "Search.Input.searchFrameInput20"
                },
                "children": [{
                    "attributes": {
                        "type": "SearchField",
                        "clickable": "true",
                        "hint": "搜索内容",
                        "bounds": "[10,0][390,76]"
                    }
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let search = result
            .elements
            .get(&root.children[0])
            .expect("search field should exist");
        assert_eq!(search.role, "textbox");
        assert_eq!(search.label.as_deref(), Some("搜索内容"));
    }

    #[test]
    fn harmony_hdc_inspect_prefers_descendant_text_over_generic_button_identifier() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][240,120]" },
            "children": [{
                "attributes": {
                    "type": "Button",
                    "clickable": "true",
                    "bounds": "[0,0][160,53]",
                    "id": "download_content_7396"
                },
                "children": [{
                    "attributes": {
                        "type": "Text",
                        "text": "打开",
                        "bounds": "[20,10][80,40]"
                    }
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        let button = result
            .elements
            .get(&root.children[0])
            .expect("button should exist");
        assert_eq!(button.role, "button");
        assert_eq!(button.label.as_deref(), Some("打开"));
    }

    #[test]
    fn harmony_hdc_inspect_drops_unlabeled_clickable_layout_wrappers_without_semantics() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,240]" },
            "children": [{
                "attributes": {
                    "type": "Stack",
                    "clickable": "true",
                    "bounds": "[0,0][420,200]"
                },
                "children": [{
                    "attributes": {
                        "type": "Image",
                        "bounds": "[0,0][420,200]"
                    }
                }]
            }]
        }))
        .expect("inspect result");

        let root = result
            .elements
            .get(&result.root_ids[0])
            .expect("window root should exist");
        assert!(root
            .children
            .iter()
            .filter_map(|id| result.elements.get(id))
            .all(|child| child.role != "button"));
    }

    #[test]
    fn harmony_hdc_inspect_filters_compact_result_to_region() {
        let result = build_inspect_result(json!({
            "attributes": { "bounds": "[0,0][420,120]" },
            "children": [{
                "attributes": {
                    "type": "Row",
                    "bounds": "[0,0][420,120]"
                },
                "children": [
                    {
                        "attributes": {
                            "type": "Button",
                            "clickable": "true",
                            "bounds": "[0,0][120,80]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "左侧",
                                "bounds": "[20,20][60,50]"
                            }
                        }]
                    },
                    {
                        "attributes": {
                            "type": "Button",
                            "clickable": "true",
                            "bounds": "[300,0][420,80]"
                        },
                        "children": [{
                            "attributes": {
                                "type": "Text",
                                "text": "右侧",
                                "bounds": "[320,20][360,50]"
                            }
                        }]
                    }
                ]
            }]
        }))
        .expect("inspect result");

        let filtered = filter_inspect_result_to_region(
            result,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 100.0,
            },
        );

        assert_eq!(filtered.root_ids, vec!["ax-0".into()]);
        let root = filtered
            .elements
            .get(&filtered.root_ids[0])
            .expect("window root should exist");
        let group = filtered
            .elements
            .get(&root.children[0])
            .expect("group should remain");
        assert_eq!(group.children.len(), 1);
        let only = filtered
            .elements
            .get(&group.children[0])
            .expect("filtered button should exist");
        assert_eq!(only.label.as_deref(), Some("左侧"));
    }
}
