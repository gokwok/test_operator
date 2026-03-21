use std::collections::HashSet;

use operator_core::{ElementId, Locator, OperatorError, Point, Surface, SurfaceKind, UiElement};

use crate::{InspectResult, TreeInspector};

pub struct ResolvedLocator {
    pub point: Point,
    pub warning: Option<String>,
}

pub fn resolve_locator<I: TreeInspector>(
    locator: &Locator,
    tree_inspector: &I,
) -> Result<ResolvedLocator, OperatorError> {
    match locator {
        Locator::Coords(point) => Ok(ResolvedLocator {
            point: *point,
            warning: Some("coordinate fallback in use; prefer snapshot_id + element_id".into()),
        }),
        Locator::Text(text) => resolve_text_locator(text, tree_inspector),
        Locator::Role { role, index } => resolve_role_locator(role, *index, tree_inspector),
        Locator::SnapshotElement { .. } => Err(OperatorError::Platform(
            "snapshot locators must be normalized by runtime before reaching the macOS driver"
                .into(),
        )),
    }
}

fn resolve_text_locator<I: TreeInspector>(
    text: &str,
    tree_inspector: &I,
) -> Result<ResolvedLocator, OperatorError> {
    let needle = normalize(text);
    let inspection = inspect_frontmost(tree_inspector)?;
    let ordered = ordered_elements(&inspection);
    let mut partial = None;
    let mut matched_without_bounds = false;

    for element in ordered {
        let mut exact_match = false;
        let mut partial_match = false;
        for candidate in [element.label.as_deref(), element.value.as_deref()] {
            let Some(candidate) = candidate else {
                continue;
            };
            let candidate = normalize(candidate);
            if candidate == needle {
                exact_match = true;
                break;
            }
            if partial.is_none() && candidate.contains(&needle) {
                partial_match = true;
            }
        }

        if exact_match {
            if let Some(point) = point_for_element(element) {
                return Ok(ResolvedLocator {
                    point,
                    warning: None,
                });
            }
            matched_without_bounds = true;
            continue;
        }

        if partial_match && partial.is_none() {
            partial = point_for_element(element);
            matched_without_bounds |= partial.is_none();
        }
    }

    if let Some(point) = partial {
        return Ok(ResolvedLocator {
            point,
            warning: None,
        });
    }

    if matched_without_bounds {
        return Err(OperatorError::Platform(format!(
            "macOS text locator matched an element without bounds: {text}"
        )));
    }

    Err(OperatorError::Platform(format!(
        "macOS text locator not found: {text}"
    )))
}

fn resolve_role_locator<I: TreeInspector>(
    role: &str,
    index: usize,
    tree_inspector: &I,
) -> Result<ResolvedLocator, OperatorError> {
    let role = normalize(role);
    let inspection = inspect_frontmost(tree_inspector)?;
    let mut matches = ordered_elements(&inspection)
        .into_iter()
        .filter(|element| normalize(&element.role) == role)
        .filter_map(point_for_element)
        .collect::<Vec<_>>();

    if index >= matches.len() {
        return Err(OperatorError::Platform(format!(
            "macOS role locator not found: {role}[{index}]"
        )));
    }

    Ok(ResolvedLocator {
        point: matches.swap_remove(index),
        warning: None,
    })
}

fn inspect_frontmost<I: TreeInspector>(tree_inspector: &I) -> Result<InspectResult, OperatorError> {
    tree_inspector.inspect(&Surface {
        kind: SurfaceKind::Frontmost,
    })
}

fn ordered_elements(inspection: &InspectResult) -> Vec<&UiElement> {
    let mut ordered = Vec::new();
    let mut visited = HashSet::<ElementId>::new();
    for root_id in &inspection.root_ids {
        visit(root_id, inspection, &mut visited, &mut ordered);
    }
    ordered
}

fn visit<'a>(
    id: &ElementId,
    inspection: &'a InspectResult,
    visited: &mut HashSet<ElementId>,
    ordered: &mut Vec<&'a UiElement>,
) {
    if !visited.insert(id.clone()) {
        return;
    }

    let Some(element) = inspection.elements.get(id) else {
        return;
    };
    ordered.push(element);
    for child_id in &element.children {
        visit(child_id, inspection, visited, ordered);
    }
}

fn point_for_element(element: &UiElement) -> Option<Point> {
    element.bounds.map(|bounds| Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    })
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
