use operator_core::{ClickMode, OperatorError, Point};

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
        path::Path,
        process::{Command, Stdio},
        thread,
    };

    use serde::Serialize;

    use super::{ClickMode, OperatorError, Point};

    const ACTION_EFFECTS_DRY_RUN_ENV: &str = "OPERATOR_ACTION_EFFECTS_DRY_RUN";
    const ACTION_EFFECTS_SCRIPT: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/action_effect.swift");

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

    pub(super) fn on_keyboard(_label: &str) -> Result<(), OperatorError> {
        Ok(())
    }

    #[derive(Debug, Serialize)]
    struct EffectRequest {
        kind: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        point: Option<EffectPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<EffectPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<EffectPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<&'static str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dx: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dy: Option<f64>,
    }

    impl EffectRequest {
        fn click(point: Point, mode: ClickMode) -> Self {
            Self {
                kind: "click",
                point: Some(EffectPoint::from(point)),
                from: None,
                to: None,
                mode: Some(click_mode_name(mode)),
                dx: None,
                dy: None,
            }
        }

        fn move_pointer(point: Point) -> Self {
            Self {
                kind: "move",
                point: Some(EffectPoint::from(point)),
                from: None,
                to: None,
                mode: None,
                dx: None,
                dy: None,
            }
        }

        fn drag(from: Point, to: Point) -> Self {
            Self {
                kind: "drag",
                point: None,
                from: Some(EffectPoint::from(from)),
                to: Some(EffectPoint::from(to)),
                mode: None,
                dx: None,
                dy: None,
            }
        }

        fn scroll(point: Point, dx: f64, dy: f64) -> Self {
            Self {
                kind: "scroll",
                point: Some(EffectPoint::from(point)),
                from: None,
                to: None,
                mode: None,
                dx: Some(dx),
                dy: Some(dy),
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct EffectPoint {
        x: f64,
        y: f64,
    }

    impl From<Point> for EffectPoint {
        fn from(point: Point) -> Self {
            Self {
                x: point.x,
                y: point.y,
            }
        }
    }

    fn click_mode_name(mode: ClickMode) -> &'static str {
        match mode {
            ClickMode::Left => "left",
            ClickMode::Right => "right",
            ClickMode::Middle => "middle",
            ClickMode::Double => "double",
        }
    }

    fn render(request: EffectRequest) -> Result<(), OperatorError> {
        if should_skip_render() {
            return Ok(());
        }

        if !Path::new(ACTION_EFFECTS_SCRIPT).exists() {
            return Err(OperatorError::Platform(format!(
                "macOS action effects helper is missing at {ACTION_EFFECTS_SCRIPT}"
            )));
        }

        let payload = serde_json::to_string(&request)?;
        let mut child = Command::new("/usr/bin/swift")
            .arg(ACTION_EFFECTS_SCRIPT)
            .arg(payload)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                OperatorError::Platform(format!(
                    "failed to invoke macOS action effects helper: {error}"
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
    use super::ActionEffects;
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
}
