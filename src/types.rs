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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVersion {
    pub version_name: String,
    pub version_code: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppAbilityInfo {
    pub name: String,
    pub module_name: String,
    pub module_main_ability: String,
    pub main_module: String,
    pub is_launcher_ability: bool,
    pub score: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayRotation {
    Rotation0,
    Rotation90,
    Rotation180,
    Rotation270,
}

impl DisplayRotation {
    pub fn from_value(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::Rotation0),
            1 => Ok(Self::Rotation90),
            2 => Ok(Self::Rotation180),
            3 => Ok(Self::Rotation270),
            _ => Err(HdcError::protocol(format!(
                "unknown display rotation value: {value}"
            ))),
        }
    }

    pub fn value(self) -> i32 {
        match self {
            Self::Rotation0 => 0,
            Self::Rotation90 => 1,
            Self::Rotation180 => 2,
            Self::Rotation270 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn center(self) -> Point {
        Point {
            x: (self.left + self.right) / 2,
            y: (self.top + self.bottom) / 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiEvent {
    pub bundle_name: String,
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentInfo {
    pub id: String,
    pub key: String,
    pub kind: String,
    pub text: String,
    pub description: String,
    pub selected: bool,
    pub checked: bool,
    pub enabled: bool,
    pub focused: bool,
    pub checkable: bool,
    pub clickable: bool,
    pub long_clickable: bool,
    pub scrollable: bool,
    pub bounds: Bounds,
    pub center: Point,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub product_name: String,
    pub model: String,
    pub sdk_version: String,
    pub sys_version: String,
    pub cpu_abi: String,
    pub wlan_ip: Option<String>,
    pub display_size: Point,
    pub display_rotation: DisplayRotation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowScale {
    pub scale_x: f64,
    pub scale_y: f64,
    pub pivot_x: f64,
    pub pivot_y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowEntry {
    pub name: String,
    pub display_id: i32,
    pub pid: i32,
    pub window_id: u32,
    pub window_type: i32,
    pub mode: i32,
    pub flag: i32,
    pub z_order: i32,
    pub orientation: i32,
    pub rect: WindowRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowList {
    pub windows: Vec<WindowEntry>,
    pub focused_window_id: Option<u32>,
    pub highlighted_window_ids: Vec<u32>,
    pub total_window_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowDetail {
    pub name: String,
    pub display_id: i32,
    pub window_id: u32,
    pub pid: i32,
    pub window_type: i32,
    pub mode: i32,
    pub flag: i32,
    pub orientation: i32,
    pub first_frame_callback_called: bool,
    pub is_visible: bool,
    pub is_rs_visible: bool,
    pub focusable: bool,
    pub deco_status: bool,
    pub is_privacy_mode: bool,
    pub rect: WindowRect,
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset: WindowOffset,
    pub scale: WindowScale,
    pub parent_window_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionEntry {
    pub mission_id: u32,
    pub mission_name: String,
    pub locked_state: i32,
    pub mission_affinity: String,
    pub ability_record_id: Option<u32>,
    pub app_name: Option<String>,
    pub main_name: Option<String>,
    pub bundle_name: Option<String>,
    pub ability_type: Option<String>,
    pub state: Option<String>,
    pub app_state: Option<String>,
    pub ready: Option<bool>,
    pub window_attached: Option<bool>,
    pub launcher: Option<bool>,
    pub is_keep_alive: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionList {
    pub missions: Vec<MissionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatedWindow {
    pub window: WindowEntry,
    pub mission: Option<MissionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatedWindowList {
    pub windows: Vec<CorrelatedWindow>,
    pub focused_window_id: Option<u32>,
    pub highlighted_window_ids: Vec<u32>,
    pub total_window_count: Option<usize>,
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
