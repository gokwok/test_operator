//! Compact, human-readable digest of a [`Snapshot`]'s element tree.
//!
//! The digest is designed to be token-efficient so it can be injected into LLM
//! prompts, printed in CLI output, or consumed by any component that needs a
//! quick overview of the UI element hierarchy without the full JSON payload.

use serde::{Deserialize, Serialize};

use crate::{ElementId, Rect, Snapshot, UiElement};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A compact summary of the element tree within a [`Snapshot`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementDigest {
    pub entries: Vec<ElementDigestEntry>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub truncated_count: usize,
}

/// One entry in an [`ElementDigest`], corresponding to a single UI element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementDigestEntry {
    /// Short sequential display ID (e.g. `"e1"`, `"e2"`).  Use this to
    /// reference the element in actions; resolve it back to the underlying
    /// platform ID via [`ElementDigest::resolve_id`].
    pub display_id: String,
    /// Internal platform element ID (ax-path or equivalent).  Not shown in
    /// rendered output; use [`ElementDigest::resolve_id`] to look it up.
    pub element_id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub depth: usize,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Knobs for controlling digest generation.
#[derive(Clone, Debug)]
pub struct DigestOptions {
    /// Maximum number of entries to include before truncating.
    pub max_entries: usize,
    /// Maximum character length for label / value strings.
    pub max_label_len: usize,
}

impl Default for DigestOptions {
    fn default() -> Self {
        Self {
            max_entries: 24,
            max_label_len: 60,
        }
    }
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl ElementDigest {
    /// Build a digest from a snapshot, filtering to "digest-worthy" elements.
    pub fn from_snapshot(snapshot: &Snapshot, opts: &DigestOptions) -> Option<Self> {
        if snapshot.root_ids.is_empty() || snapshot.elements.is_empty() {
            return None;
        }

        let mut entries = Vec::new();
        let mut counter = 0usize;
        for root_id in &snapshot.root_ids {
            collect_entries(
                snapshot,
                root_id,
                0,
                None,
                opts.max_label_len,
                &mut entries,
                &mut counter,
            );
        }
        if entries.is_empty() {
            return None;
        }

        let truncated_count = entries.len().saturating_sub(opts.max_entries);
        entries.truncate(opts.max_entries);
        Some(Self {
            entries,
            truncated_count,
        })
    }

    /// Resolve a short display ID (e.g. `"e5"`) back to the underlying
    /// platform element ID (e.g. `"ax-0-1-2"`).
    pub fn resolve_id(&self, display_id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.display_id == display_id)
            .map(|e| e.element_id.as_str())
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl ElementDigestEntry {
    /// Render this entry as a single indented line.
    ///
    /// Example: `  - [e3] button label="OK" bounds=(10,20,80,30)`
    pub fn render_line(&self) -> String {
        let mut fields = Vec::new();
        if let Some(label) = self.label.as_deref() {
            fields.push(format!("label={}", quoted(label)));
        }
        if let Some(value) = self.value.as_deref() {
            fields.push(format!("value={}", quoted(value)));
        }
        // Only emit `enabled` when the element is disabled; enabled is the
        // expected default and adds noise when shown unconditionally.
        if self.enabled == Some(false) {
            fields.push("enabled=false".to_string());
        }
        if let Some(bounds) = self.bounds {
            fields.push(format!("bounds={}", format_rect(bounds)));
        }

        let suffix = if fields.is_empty() {
            String::new()
        } else {
            format!(" {}", fields.join(" "))
        };
        format!(
            "{}- [{}] {}{}",
            "  ".repeat(self.depth),
            self.display_id,
            self.role,
            suffix
        )
    }
}

impl ElementDigest {
    /// Render the full digest as a multi-line string.
    ///
    /// Each entry is on its own line; a trailing `"... N more ..."` line is
    /// appended when entries were truncated.
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = self.entries.iter().map(|e| e.render_line()).collect();
        if self.truncated_count > 0 {
            lines.push(format!("... {} more entries omitted", self.truncated_count));
        }
        lines.join("\n")
    }
}

impl Snapshot {
    /// Produce a compact, human-readable text representation of the element
    /// tree suitable for CLI display or LLM prompt injection.
    pub fn render_element_tree(&self, opts: &DigestOptions) -> String {
        let mut sections = Vec::<String>::new();

        // Header
        let mut header = format!("snapshot {} ({})", self.id, self.target);
        if let Some(ref assessment) = self.metadata.element_tree {
            if let Some(ref note) = assessment.note {
                header.push_str(&format!(" [warning: {note}]"));
            }
        }
        sections.push(header);

        // Element digest
        match ElementDigest::from_snapshot(self, opts) {
            Some(digest) => sections.push(digest.render()),
            None => sections.push("(no elements)".into()),
        }

        sections.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Helpers (private)
// ---------------------------------------------------------------------------

fn collect_entries(
    snapshot: &Snapshot,
    element_id: &ElementId,
    depth: usize,
    parent_label: Option<&str>,
    max_label_len: usize,
    out: &mut Vec<ElementDigestEntry>,
    counter: &mut usize,
) {
    let Some(element) = snapshot.elements.get(element_id) else {
        return;
    };

    // --- Pruning rules ---

    // 1. Skip decoration-only images (no label, no value — purely visual).
    if element.role == "image" && element.label.is_none() && element.value.is_none() {
        return;
    }

    // 2. Skip text nodes whose label duplicates the parent's label (the button
    //    already carries this information).
    if element.role == "text" {
        if let (Some(my_label), Some(plabel)) = (element.label.as_deref(), parent_label) {
            if plabel.contains(my_label) {
                return;
            }
        }
    }

    // 3. Skip decorative-named groups: groups whose label names a visual
    //    styling layer (background, blur, shadow, overlay, gradient, color).
    //    These are never interactive and carry no semantic information for an
    //    agent.  Drop the entire subtree.
    if element.role == "group" {
        if let Some(label) = element.label.as_deref() {
            if is_decorative_label(label) {
                return;
            }
        }
    }

    // 4. Collapse unlabeled passthrough groups: if this element is a group
    //    with no label/value it adds no semantic information — skip it and
    //    promote its children to the same depth.  This handles both the
    //    single-child and multi-child cases.
    if element.role == "group" && element.label.is_none() && element.value.is_none() {
        for child_id in &element.children {
            collect_entries(snapshot, child_id, depth, parent_label, max_label_len, out, counter);
        }
        return;
    }

    // 5. Collapse parent–child duplicate labels: if a group's label is
    //    identical to its single child's label, skip the group.
    if element.role == "group" && element.children.len() == 1 {
        if let Some(my_label) = element.label.as_deref() {
            if let Some(child) = snapshot.elements.get(&element.children[0]) {
                if child.label.as_deref() == Some(my_label) {
                    collect_entries(
                        snapshot,
                        &element.children[0],
                        depth,
                        parent_label,
                        max_label_len,
                        out,
                        counter,
                    );
                    return;
                }
            }
        }
    }

    // --- Emit this element if it carries useful information ---
    let emit = is_digest_worthy(element);

    if emit {
        *counter += 1;
        out.push(ElementDigestEntry {
            display_id: format!("e{counter}"),
            element_id: element.id.to_string(),
            role: element.role.clone(),
            label: truncate_option(element.label.as_deref(), max_label_len),
            value: truncate_option(element.value.as_deref(), max_label_len),
            enabled: element.enabled,
            bounds: element.bounds,
            depth,
        });
    }

    // 6. Skip subtrees of labelled interactive elements — the parent already
    //    carries all the information an agent needs to act on it.
    let dominated_interactive =
        emit && element.label.is_some() && is_interactive_role(&element.role);
    if dominated_interactive {
        return;
    }

    let my_label = element.label.as_deref();
    let child_depth = if emit { depth + 1 } else { depth };
    for child_id in &element.children {
        collect_entries(
            snapshot,
            child_id,
            child_depth,
            my_label,
            max_label_len,
            out,
            counter,
        );
    }
}

fn is_digest_worthy(element: &UiElement) -> bool {
    element.label.is_some()
        || element.value.is_some()
        || !element.children.is_empty()
        || element.role != "generic"
}

fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "button" | "textbox" | "checkbox" | "switch" | "slider" | "link" | "menuitem"
    )
}

/// Returns `true` when a group label names a purely visual / styling layer
/// (e.g. `"title card background color"`, `"hero blur"`, `"card shadow"`).
/// Such groups contain no interactive elements and should be dropped entirely.
fn is_decorative_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("background")
        || lower.contains("blur")
        || lower.contains("shadow")
        || lower.contains("overlay")
        || lower.contains("gradient")
        || lower.ends_with(" color")
        || lower.ends_with("_color")
}

fn truncate_option(value: Option<&str>, limit: usize) -> Option<String> {
    value.map(|v| truncate_to(v, limit))
}

fn truncate_to(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push_str("...");
    truncated
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "'"))
}

fn format_rect(rect: Rect) -> String {
    format!(
        "({},{},{},{})",
        format_scalar(rect.x),
        format_scalar(rect.y),
        format_scalar(rect.width),
        format_scalar(rect.height)
    )
}

fn format_scalar(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::SystemTime;

    use super::*;
    use crate::{ElementSource, SnapshotMetadata, Surface, SurfaceKind};

    fn make_element(id: &str, role: &str, label: Option<&str>, children: Vec<&str>) -> UiElement {
        UiElement {
            id: id.into(),
            role: role.into(),
            label: label.map(Into::into),
            value: None,
            bounds: Some(Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 40.0,
            }),
            enabled: None,
            children: children.into_iter().map(Into::into).collect(),
            confidence: None,
            source: ElementSource::Native,
        }
    }

    fn make_snapshot(elements: Vec<UiElement>, root_ids: Vec<&str>) -> Snapshot {
        let mut map = HashMap::new();
        for el in elements {
            map.insert(el.id.clone(), el);
        }
        Snapshot {
            id: "snap-1".into(),
            target: "harmony-pc".into(),
            surface: Surface {
                kind: SurfaceKind::Frontmost,
            },
            image_artifact: None,
            elements: map,
            root_ids: root_ids.into_iter().map(Into::into).collect(),
            metadata: SnapshotMetadata {
                platform: "harmony".into(),
                display_scale: None,
                capture_bounds: None,
                image_size_px: None,
                element_tree: None,
                capture_duration_ms: 50,
            },
            created_at: SystemTime::now(),
            expires_at: None,
        }
    }

    #[test]
    fn digest_renders_tree_with_indentation() {
        let snapshot = make_snapshot(
            vec![
                make_element("root", "window", Some("Settings"), vec!["btn"]),
                make_element("btn", "button", Some("Save"), vec![]),
            ],
            vec!["root"],
        );

        let digest = ElementDigest::from_snapshot(&snapshot, &DigestOptions::default()).unwrap();
        assert_eq!(digest.entries.len(), 2);
        assert_eq!(digest.entries[0].depth, 0);
        assert_eq!(digest.entries[1].depth, 1);

        // display_ids are short sequential IDs; element_ids retain the originals
        assert_eq!(digest.entries[0].display_id, "e1");
        assert_eq!(digest.entries[0].element_id, "root");
        assert_eq!(digest.entries[1].display_id, "e2");
        assert_eq!(digest.entries[1].element_id, "btn");

        let rendered = digest.render();
        assert!(rendered.contains("- [e1] window"));
        assert!(rendered.contains("  - [e2] button"));
    }

    #[test]
    fn digest_truncates_entries() {
        let elements: Vec<UiElement> = (0..10)
            .map(|i| {
                make_element(
                    &format!("e{i}"),
                    "button",
                    Some(&format!("Btn {i}")),
                    vec![],
                )
            })
            .collect();
        let root_ids: Vec<&str> = (0..10)
            .map(|i| &*Box::leak(format!("e{i}").into_boxed_str()))
            .collect();
        let snapshot = make_snapshot(elements, root_ids);

        let opts = DigestOptions {
            max_entries: 3,
            max_label_len: 60,
        };
        let digest = ElementDigest::from_snapshot(&snapshot, &opts).unwrap();
        assert_eq!(digest.entries.len(), 3);
        assert_eq!(digest.truncated_count, 7);
        assert!(digest.render().contains("... 7 more entries omitted"));
    }

    #[test]
    fn render_element_tree_includes_header_and_body() {
        let snapshot = make_snapshot(
            vec![make_element("btn", "button", Some("OK"), vec![])],
            vec!["btn"],
        );
        let text = snapshot.render_element_tree(&DigestOptions::default());
        assert!(text.starts_with("snapshot snap-1 (harmony-pc)"));
        assert!(text.contains("[e1] button label=\"OK\""));
    }

    #[test]
    fn empty_snapshot_shows_no_elements() {
        let snapshot = make_snapshot(vec![], vec![]);
        let text = snapshot.render_element_tree(&DigestOptions::default());
        assert!(text.contains("(no elements)"));
    }
}
