use operator_core::{ClickMode, OperatorError, Point};
#[cfg(any(feature = "action-effects", test))]
use serde::{Deserialize, Serialize};

#[cfg(feature = "action-effects")]
pub(crate) const ACTION_EFFECT_HELPER_ARG: &str = "__operator-macos-action-effect-helper";
#[cfg(feature = "action-effects")]
pub(crate) const ACTION_EFFECT_PAYLOAD_ENV: &str = "OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD";

#[cfg(feature = "action-effects")]
mod helper;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ActionEffects;

impl ActionEffects {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn on_click(
        &self,
        point: Option<Point>,
        mode: ClickMode,
    ) -> Result<(), OperatorError> {
        backend::on_click(point, mode)
    }

    pub(crate) fn on_move(&self, point: Point) -> Result<(), OperatorError> {
        backend::on_move(point)
    }

    pub(crate) fn on_drag(&self, from: Point, to: Point) -> Result<(), OperatorError> {
        backend::on_drag(from, to)
    }

    pub(crate) fn on_scroll(
        &self,
        point: Option<Point>,
        dx: f64,
        dy: f64,
    ) -> Result<(), OperatorError> {
        backend::on_scroll(point, dx, dy)
    }

    pub(crate) fn on_keyboard(&self, label: &str) -> Result<(), OperatorError> {
        backend::on_keyboard(label)
    }
}

#[cfg(any(feature = "action-effects", test))]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct EffectRequest {
    kind: EffectKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    point: Option<EffectPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<EffectPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<EffectPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[cfg(any(feature = "action-effects", test))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EffectKind {
    Click,
    Move,
    Drag,
    Scroll,
    Keyboard,
}

#[cfg(any(feature = "action-effects", test))]
impl EffectRequest {
    #[cfg(feature = "action-effects")]
    fn click(point: Point, mode: ClickMode) -> Self {
        Self {
            kind: EffectKind::Click,
            point: Some(EffectPoint::from(point)),
            from: None,
            to: None,
            mode: Some(click_mode_name(mode).to_string()),
            dx: None,
            dy: None,
            label: None,
        }
    }

    #[cfg(feature = "action-effects")]
    fn move_pointer(point: Point) -> Self {
        Self {
            kind: EffectKind::Move,
            point: Some(EffectPoint::from(point)),
            from: None,
            to: None,
            mode: None,
            dx: None,
            dy: None,
            label: None,
        }
    }

    #[cfg(feature = "action-effects")]
    fn drag(from: Point, to: Point) -> Self {
        Self {
            kind: EffectKind::Drag,
            point: None,
            from: Some(EffectPoint::from(from)),
            to: Some(EffectPoint::from(to)),
            mode: None,
            dx: None,
            dy: None,
            label: None,
        }
    }

    #[cfg(feature = "action-effects")]
    fn scroll(point: Point, dx: f64, dy: f64) -> Self {
        Self {
            kind: EffectKind::Scroll,
            point: Some(EffectPoint::from(point)),
            from: None,
            to: None,
            mode: None,
            dx: Some(dx),
            dy: Some(dy),
            label: None,
        }
    }

    fn keyboard(label: &str) -> Option<Self> {
        let label = normalize_keyboard_label(label)?;
        Some(Self {
            kind: EffectKind::Keyboard,
            point: None,
            from: None,
            to: None,
            mode: None,
            dx: None,
            dy: None,
            label: Some(label),
        })
    }
}

#[cfg(any(feature = "action-effects", test))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct EffectPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[cfg(any(feature = "action-effects", test))]
impl From<Point> for EffectPoint {
    fn from(point: Point) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

#[cfg(feature = "action-effects")]
fn click_mode_name(mode: ClickMode) -> &'static str {
    match mode {
        ClickMode::Left => "left",
        ClickMode::Right => "right",
        ClickMode::Middle => "middle",
        ClickMode::Double => "double",
    }
}

#[cfg(any(feature = "action-effects", test))]
fn normalize_keyboard_label(label: &str) -> Option<String> {
    const MAX_LABEL_CHARS: usize = 48;

    let collapsed = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }

    let char_count = collapsed.chars().count();
    if char_count <= MAX_LABEL_CHARS {
        return Some(collapsed);
    }

    let truncated = collapsed
        .chars()
        .take(MAX_LABEL_CHARS.saturating_sub(3))
        .collect::<String>();
    Some(format!("{truncated}..."))
}

#[cfg(feature = "action-effects")]
pub fn try_run_action_effect_helper() -> Result<Option<i32>, String> {
    if !std::env::args().any(|arg| arg == ACTION_EFFECT_HELPER_ARG) {
        return Ok(None);
    }

    let payload = std::env::var(ACTION_EFFECT_PAYLOAD_ENV)
        .map_err(|_| format!("missing {ACTION_EFFECT_PAYLOAD_ENV} for action effect helper"))?;
    helper::run(&payload).map_err(|error| error.to_string())?;
    Ok(Some(0))
}

#[cfg(not(feature = "action-effects"))]
mod backend {
    use super::{ClickMode, OperatorError, Point};

    pub(super) fn on_click(_point: Option<Point>, _mode: ClickMode) -> Result<(), OperatorError> {
        Ok(())
    }

    pub(super) fn on_move(_point: Point) -> Result<(), OperatorError> {
        Ok(())
    }

    pub(super) fn on_drag(_from: Point, _to: Point) -> Result<(), OperatorError> {
        Ok(())
    }

    pub(super) fn on_scroll(
        _point: Option<Point>,
        _dx: f64,
        _dy: f64,
    ) -> Result<(), OperatorError> {
        Ok(())
    }

    pub(super) fn on_keyboard(_label: &str) -> Result<(), OperatorError> {
        Ok(())
    }
}

#[cfg(feature = "action-effects")]
mod backend {
    use std::{
        process::{Command, Stdio},
        thread,
    };

    use super::{
        ClickMode, EffectRequest, OperatorError, Point, ACTION_EFFECT_HELPER_ARG,
        ACTION_EFFECT_PAYLOAD_ENV,
    };

    const ACTION_EFFECTS_DRY_RUN_ENV: &str = "OPERATOR_ACTION_EFFECTS_DRY_RUN";

    pub(super) fn on_click(point: Option<Point>, mode: ClickMode) -> Result<(), OperatorError> {
        let Some(point) = point else {
            return Ok(());
        };

        render(EffectRequest::click(point, mode))
    }

    pub(super) fn on_move(point: Point) -> Result<(), OperatorError> {
        render(EffectRequest::move_pointer(point))
    }

    pub(super) fn on_drag(from: Point, to: Point) -> Result<(), OperatorError> {
        render(EffectRequest::drag(from, to))
    }

    pub(super) fn on_scroll(point: Option<Point>, dx: f64, dy: f64) -> Result<(), OperatorError> {
        if dx == 0.0 && dy == 0.0 {
            return Ok(());
        }

        let Some(point) = point else {
            return Ok(());
        };

        render(EffectRequest::scroll(point, dx, dy))
    }

    pub(super) fn on_keyboard(label: &str) -> Result<(), OperatorError> {
        let Some(request) = EffectRequest::keyboard(label) else {
            return Ok(());
        };
        render(request)
    }

    fn render(request: EffectRequest) -> Result<(), OperatorError> {
        if should_skip_render() {
            return Ok(());
        }

        let payload = serde_json::to_string(&request)?;
        let helper = std::env::current_exe().map_err(|error| {
            OperatorError::Platform(format!(
                "failed to resolve current executable for macOS action effects helper: {error}"
            ))
        })?;
        let mut child = Command::new(helper)
            .arg(ACTION_EFFECT_HELPER_ARG)
            .env(ACTION_EFFECT_PAYLOAD_ENV, payload)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                OperatorError::Platform(format!(
                    "failed to invoke macOS action effects helper process: {error}"
                ))
            })?;

        thread::spawn(move || {
            let _ = child.wait();
        });

        Ok(())
    }

    fn should_skip_render() -> bool {
        cfg!(test) || std::env::var_os(ACTION_EFFECTS_DRY_RUN_ENV).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionEffects, EffectKind, EffectRequest};
    use operator_core::{ClickMode, Point};

    #[test]
    fn facade_accepts_all_first_phase_effect_calls() {
        let effects = ActionEffects::new();

        effects
            .on_click(Some(Point { x: 12.0, y: 24.0 }), ClickMode::Left)
            .expect("click effect");
        effects
            .on_move(Point { x: 16.0, y: 32.0 })
            .expect("move effect");
        effects
            .on_drag(Point { x: 10.0, y: 20.0 }, Point { x: 30.0, y: 40.0 })
            .expect("drag effect");
        effects
            .on_scroll(Some(Point { x: 18.0, y: 36.0 }), 0.0, -120.0)
            .expect("scroll effect");
        effects.on_keyboard("cmd+k").expect("keyboard effect");
    }

    #[test]
    fn keyboard_effect_request_collapses_blank_input() {
        assert_eq!(EffectRequest::keyboard("   \n\t  "), None);
    }

    #[test]
    fn keyboard_effect_request_trims_and_truncates_labels() {
        let request = EffectRequest::keyboard("  cmd  +   shift  +  p  ").expect("request");
        assert_eq!(request.kind, EffectKind::Keyboard);
        assert_eq!(request.label.as_deref(), Some("cmd + shift + p"));

        let request = EffectRequest::keyboard(
            "this is a deliberately long keyboard hud label that should be truncated",
        )
        .expect("request");
        let label = request.label.expect("label");
        assert_eq!(label.len(), 48);
        assert!(label.ends_with("..."));
    }
}
