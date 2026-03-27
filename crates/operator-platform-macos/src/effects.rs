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
    use super::{ClickMode, OperatorError, Point};

    // OPE-143 only establishes the internal feature-gated facade. Rendering lands later.
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
