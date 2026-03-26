use std::borrow::Cow;

use crate::error::{HdcError, Result};
use crate::protocol::MessageLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverMessageLevel {
    Fail,
    Info,
    Ok,
    Unknown(u8),
}

impl DriverMessageLevel {
    pub(crate) fn from_protocol(level: MessageLevel) -> Self {
        match level {
            MessageLevel::Fail => Self::Fail,
            MessageLevel::Info => Self::Info,
            MessageLevel::Ok => Self::Ok,
            MessageLevel::Unknown(value) => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverMessage {
    pub level: DriverMessageLevel,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Ok,
    FailedHint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellResult {
    pub stdout: Vec<u8>,
    pub messages: Vec<DriverMessage>,
    pub status: CommandStatus,
}

impl ShellResult {
    pub fn stdout_text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    pub fn failed(&self) -> bool {
        matches!(self.status, CommandStatus::FailedHint)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coord {
    Pixels(i32),
    Percent(f64),
}

impl Coord {
    pub(crate) fn resolve(self, total: i32) -> Result<i32> {
        match self {
            Self::Pixels(value) => Ok(value),
            Self::Percent(value) if (0.0..=1.0).contains(&value) => {
                Ok((f64::from(total) * value).round() as i32)
            }
            Self::Percent(value) => Err(HdcError::protocol(format!(
                "percentage coordinate must be between 0.0 and 1.0, got {value}"
            ))),
        }
    }
}

impl From<i32> for Coord {
    fn from(value: i32) -> Self {
        Self::Pixels(value)
    }
}

impl From<u32> for Coord {
    fn from(value: u32) -> Self {
        Self::Pixels(value as i32)
    }
}

impl From<f32> for Coord {
    fn from(value: f32) -> Self {
        Self::Percent(f64::from(value))
    }
}

impl From<f64> for Coord {
    fn from(value: f64) -> Self {
        Self::Percent(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentApp {
    pub bundle_name: String,
    pub ability_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(u32);

impl KeyCode {
    pub const HOME: Self = Self(1);
    pub const BACK: Self = Self(2);
    pub const POWER: Self = Self(18);
    pub const CTRL_LEFT: Self = Self(2072);
    pub const CTRL_RIGHT: Self = Self(2073);
    pub const ALT_LEFT: Self = Self(2045);
    pub const ALT_RIGHT: Self = Self(2046);
    pub const SHIFT_LEFT: Self = Self(2047);
    pub const SHIFT_RIGHT: Self = Self(2048);
    pub const ENTER: Self = Self(2054);
    pub const DEL: Self = Self(2055);
    pub const A: Self = Self(2017);
    pub const B: Self = Self(2018);
    pub const C: Self = Self(2019);
    pub const D: Self = Self(2020);
    pub const E: Self = Self(2021);
    pub const F: Self = Self(2022);
    pub const G: Self = Self(2023);
    pub const H: Self = Self(2024);
    pub const I: Self = Self(2025);
    pub const J: Self = Self(2026);
    pub const K: Self = Self(2027);
    pub const L: Self = Self(2028);
    pub const M: Self = Self(2029);
    pub const N: Self = Self(2030);
    pub const O: Self = Self(2031);
    pub const P: Self = Self(2032);
    pub const Q: Self = Self(2033);
    pub const R: Self = Self(2034);
    pub const S: Self = Self(2035);
    pub const T: Self = Self(2036);
    pub const U: Self = Self(2037);
    pub const V: Self = Self(2038);
    pub const W: Self = Self(2039);
    pub const X: Self = Self(2040);
    pub const Y: Self = Self(2041);
    pub const Z: Self = Self(2042);

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<u32> for KeyCode {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
