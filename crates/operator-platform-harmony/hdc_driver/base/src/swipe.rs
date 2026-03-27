use crate::driver::Driver;
use crate::error::{HdcError, Result};
use crate::types::{Coord, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwipeArea {
    pub left: Coord,
    pub top: Coord,
    pub right: Coord,
    pub bottom: Coord,
}

pub struct SwipeExt<'a> {
    driver: &'a mut Driver,
}

impl SwipeArea {
    pub fn new<L, T, R, B>(left: L, top: T, right: R, bottom: B) -> Self
    where
        L: Into<Coord>,
        T: Into<Coord>,
        R: Into<Coord>,
        B: Into<Coord>,
    {
        Self {
            left: left.into(),
            top: top.into(),
            right: right.into(),
            bottom: bottom.into(),
        }
    }

    fn resolve(self, total: Point) -> Result<(i32, i32, i32, i32)> {
        let left = self.left.resolve(total.x)?;
        let top = self.top.resolve(total.y)?;
        let right = self.right.resolve(total.x)?;
        let bottom = self.bottom.resolve(total.y)?;
        if left < 0 || top < 0 || right <= 0 || bottom <= 0 {
            return Err(HdcError::protocol(
                "swipe area coordinates must be greater than 0",
            ));
        }
        if left >= right || top >= bottom {
            return Err(HdcError::protocol(
                "swipe area must satisfy left < right and top < bottom",
            ));
        }
        Ok((left, top, right, bottom))
    }
}

impl<'a> SwipeExt<'a> {
    pub(crate) fn new(driver: &'a mut Driver) -> Self {
        Self { driver }
    }

    pub fn swipe(
        &mut self,
        direction: SwipeDirection,
        scale: f64,
        area: Option<SwipeArea>,
        speed: Option<u32>,
    ) -> Result<()> {
        if !(0.0 < scale && scale <= 1.0) {
            return Err(HdcError::protocol(format!(
                "scale must be in range (0.0, 1.0], got {scale}"
            )));
        }

        let total = self.driver.display_size()?;
        let (left, top, right, bottom) = match area {
            Some(area) => area.resolve(total)?,
            None => (0, 0, total.x, total.y),
        };
        let width = right - left;
        let height = bottom - top;
        let h_offset = ((f64::from(width) * (1.0 - scale)) / 2.0).round() as i32;
        let v_offset = ((f64::from(height) * (1.0 - scale)) / 2.0).round() as i32;

        let (start, end) = match direction {
            SwipeDirection::Left => (
                Point {
                    x: right - h_offset,
                    y: top + height / 2,
                },
                Point {
                    x: left + h_offset,
                    y: top + height / 2,
                },
            ),
            SwipeDirection::Right => (
                Point {
                    x: left + h_offset,
                    y: top + height / 2,
                },
                Point {
                    x: right - h_offset,
                    y: top + height / 2,
                },
            ),
            SwipeDirection::Up => (
                Point {
                    x: left + width / 2,
                    y: bottom - v_offset,
                },
                Point {
                    x: left + width / 2,
                    y: top + v_offset,
                },
            ),
            SwipeDirection::Down => (
                Point {
                    x: left + width / 2,
                    y: top + v_offset,
                },
                Point {
                    x: left + width / 2,
                    y: bottom - v_offset,
                },
            ),
        };
        self.driver
            .swipe(start.x, start.y, end.x, end.y, speed.or(Some(2000)))
    }

    pub fn left(&mut self, scale: f64, area: Option<SwipeArea>, speed: Option<u32>) -> Result<()> {
        self.swipe(SwipeDirection::Left, scale, area, speed)
    }

    pub fn right(&mut self, scale: f64, area: Option<SwipeArea>, speed: Option<u32>) -> Result<()> {
        self.swipe(SwipeDirection::Right, scale, area, speed)
    }

    pub fn up(&mut self, scale: f64, area: Option<SwipeArea>, speed: Option<u32>) -> Result<()> {
        self.swipe(SwipeDirection::Up, scale, area, speed)
    }

    pub fn down(&mut self, scale: f64, area: Option<SwipeArea>, speed: Option<u32>) -> Result<()> {
        self.swipe(SwipeDirection::Down, scale, area, speed)
    }
}

#[cfg(test)]
mod tests {
    use super::{SwipeArea, SwipeDirection};
    use crate::types::{Coord, Point};

    #[test]
    fn swipe_area_resolves_percentages() {
        let area = SwipeArea::new(0.1_f64, 0.2_f64, 0.9_f64, 0.8_f64);

        let resolved = area.resolve(Point { x: 1000, y: 2000 }).unwrap();

        assert_eq!(resolved, (100, 400, 900, 1600));
    }

    #[test]
    fn swipe_area_rejects_invalid_box() {
        let area = SwipeArea {
            left: Coord::from(500),
            top: Coord::from(100),
            right: Coord::from(100),
            bottom: Coord::from(300),
        };

        assert!(area.resolve(Point { x: 1000, y: 2000 }).is_err());
    }

    #[test]
    fn swipe_direction_variants_are_stable() {
        assert_eq!(SwipeDirection::Left as u8, 0);
        assert_eq!(SwipeDirection::Right as u8, 1);
        assert_eq!(SwipeDirection::Up as u8, 2);
        assert_eq!(SwipeDirection::Down as u8, 3);
    }
}
