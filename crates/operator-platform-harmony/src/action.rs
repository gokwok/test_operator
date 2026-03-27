use std::num::NonZeroU32;

use operator_core::{
    Action, ActionCoordinates, ActionOutcome, ActionSideEffect, ClickMode, DragMotion,
    OperatorError, Point, TypeTrailingKey,
};

const KEY_DPAD_UP: u32 = 2012;
const KEY_DPAD_DOWN: u32 = 2013;
const KEY_DPAD_LEFT: u32 = 2014;
const KEY_DPAD_RIGHT: u32 = 2015;
const KEY_TAB: u32 = 2049;
const KEY_SPACE: u32 = 2050;
const KEY_ENTER: u32 = 2054;
const KEY_DEL: u32 = 2055;
const KEY_MINUS: u32 = 2057;
const KEY_EQUALS: u32 = 2058;
const KEY_LEFT_BRACKET: u32 = 2059;
const KEY_RIGHT_BRACKET: u32 = 2060;
const KEY_BACKSLASH: u32 = 2061;
const KEY_SEMICOLON: u32 = 2062;
const KEY_APOSTROPHE: u32 = 2063;
const KEY_SLASH: u32 = 2064;
const KEY_AT: u32 = 2065;
const KEY_PLUS: u32 = 2066;
const KEY_PAGE_UP: u32 = 2068;
const KEY_PAGE_DOWN: u32 = 2069;
const KEY_ESCAPE: u32 = 2070;
const KEY_FORWARD_DEL: u32 = 2071;
const KEY_CTRL_LEFT: u32 = 2072;
const KEY_ALT_LEFT: u32 = 2045;
const KEY_SHIFT_LEFT: u32 = 2047;
const KEY_META_LEFT: u32 = 2076;
const KEY_FUNCTION: u32 = 2078;
const KEY_MOVE_HOME: u32 = 2081;
const KEY_MOVE_END: u32 = 2082;
const KEY_INSERT: u32 = 2083;

pub(crate) fn unsupported_action_error(action: &Action) -> OperatorError {
    OperatorError::Platform(format!(
        "driver harmony.hdc first-phase action surface does not implement `{}` yet",
        action_name(action)
    ))
}

pub(crate) fn successful_action_outcome(detail: impl Into<String>) -> ActionOutcome {
    ActionOutcome {
        success: true,
        duration_ms: 0,
        detail: Some(detail.into()),
        coordinates: None,
        target_app: None,
        target_window: None,
        side_effects: Vec::new(),
        warnings: Vec::new(),
    }
}

pub(crate) fn point_coordinates(point: Point) -> ActionCoordinates {
    ActionCoordinates {
        point: Some(point),
        from: None,
        to: None,
    }
}

pub(crate) fn range_coordinates(from: Point, to: Point) -> ActionCoordinates {
    ActionCoordinates {
        point: None,
        from: Some(from),
        to: Some(to),
    }
}

pub(crate) fn click_detail(mode: ClickMode) -> &'static str {
    match mode {
        ClickMode::Left => "clicked",
        ClickMode::Right => "right-clicked",
        ClickMode::Double => "double-clicked",
        ClickMode::Middle => "clicked",
    }
}

pub(crate) fn press_detail(key: &str, count: u32) -> String {
    let key = canonical_key_name(key);
    if count == 1 {
        format!("pressed {key}")
    } else {
        format!("pressed {key} {count} times")
    }
}

pub(crate) fn type_side_effect(
    clear_before: bool,
    trailing_keys: &[TypeTrailingKey],
) -> ActionSideEffect {
    ActionSideEffect::Type {
        clear_before,
        trailing_keys: trailing_keys.to_vec(),
    }
}

pub(crate) fn drag_warnings(motion: &DragMotion) -> Vec<String> {
    let mut warnings = Vec::new();
    if motion.steps.is_some() {
        warnings.push("harmony.hdc ignores drag step counts in the first phase".into());
    }
    if !motion.modifiers.is_empty() {
        warnings.push("harmony.hdc ignores drag modifiers in the first phase".into());
    }
    warnings
}

pub(crate) fn swipe_warnings(steps: Option<NonZeroU32>) -> Vec<String> {
    if steps.is_some() {
        vec!["harmony.hdc ignores swipe step counts in the first phase".into()]
    } else {
        Vec::new()
    }
}

pub(crate) fn velocity_from_duration(
    from: Point,
    to: Point,
    duration_ms: Option<u64>,
) -> Option<u32> {
    let duration_ms = duration_ms?;
    if duration_ms == 0 {
        return Some(40_000);
    }

    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let distance = (dx * dx + dy * dy).sqrt().max(1.0);
    let pixels_per_second = distance * 1_000.0 / duration_ms as f64;
    Some(pixels_per_second.round().clamp(200.0, 40_000.0) as u32)
}

pub(crate) fn clear_before_key_codes() -> Vec<u32> {
    vec![KEY_CTRL_LEFT, alpha_key_code('a')]
}

pub(crate) fn trailing_key_code(key: TypeTrailingKey) -> u32 {
    match key {
        TypeTrailingKey::Return => KEY_ENTER,
        TypeTrailingKey::Tab => KEY_TAB,
        TypeTrailingKey::Escape => KEY_ESCAPE,
        TypeTrailingKey::Delete => KEY_DEL,
    }
}

pub(crate) fn parse_hotkey_keys(keys: &[String]) -> Result<Vec<u32>, OperatorError> {
    if keys.is_empty() {
        return Err(OperatorError::Platform(
            "harmony.hdc hotkey requires at least one key".into(),
        ));
    }

    keys.iter().map(|key| parse_key_code(key)).collect()
}

pub(crate) fn parse_key_code(key: &str) -> Result<u32, OperatorError> {
    let normalized = canonical_key_name(key);
    let code = match normalized.as_str() {
        "command" | "cmd" | "meta" | "super" => KEY_META_LEFT,
        "control" | "ctrl" | "ctl" => KEY_CTRL_LEFT,
        "alt" | "option" => KEY_ALT_LEFT,
        "shift" => KEY_SHIFT_LEFT,
        "fn" | "function" => KEY_FUNCTION,
        "return" | "enter" => KEY_ENTER,
        "tab" => KEY_TAB,
        "space" => KEY_SPACE,
        "delete" | "backspace" => KEY_DEL,
        "forward-delete" | "forward_del" | "forwarddelete" => KEY_FORWARD_DEL,
        "escape" | "esc" => KEY_ESCAPE,
        "left" | "left-arrow" | "arrowleft" => KEY_DPAD_LEFT,
        "right" | "right-arrow" | "arrowright" => KEY_DPAD_RIGHT,
        "up" | "up-arrow" | "arrowup" => KEY_DPAD_UP,
        "down" | "down-arrow" | "arrowdown" => KEY_DPAD_DOWN,
        "home" => KEY_MOVE_HOME,
        "end" => KEY_MOVE_END,
        "pageup" | "page-up" => KEY_PAGE_UP,
        "pagedown" | "page-down" => KEY_PAGE_DOWN,
        "insert" => KEY_INSERT,
        "," | "comma" => 2043,
        "." | "period" => 2044,
        "-" | "minus" => KEY_MINUS,
        "=" | "equals" => KEY_EQUALS,
        "[" | "left-bracket" | "left_bracket" => KEY_LEFT_BRACKET,
        "]" | "right-bracket" | "right_bracket" => KEY_RIGHT_BRACKET,
        "\\" | "backslash" => KEY_BACKSLASH,
        ";" | "semicolon" => KEY_SEMICOLON,
        "'" | "apostrophe" => KEY_APOSTROPHE,
        "/" | "slash" => KEY_SLASH,
        "@" | "at" => KEY_AT,
        "+" | "plus" => KEY_PLUS,
        other if other.len() == 1 => return parse_single_key(other.chars().next().unwrap()),
        _ => {
            return Err(OperatorError::Platform(format!(
                "harmony.hdc does not support key `{key}`"
            )))
        }
    };

    Ok(code)
}

fn parse_single_key(ch: char) -> Result<u32, OperatorError> {
    if ch.is_ascii_alphabetic() {
        return Ok(alpha_key_code(ch.to_ascii_lowercase()));
    }
    if ch.is_ascii_digit() {
        return Ok(2_000 + u32::from(ch as u8 - b'0'));
    }

    Err(OperatorError::Platform(format!(
        "harmony.hdc does not support key `{ch}`"
    )))
}

fn alpha_key_code(ch: char) -> u32 {
    2_017 + u32::from(ch as u8 - b'a')
}

fn canonical_key_name(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

pub(crate) fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Click { .. } => "click",
        Action::Move => "move",
        Action::Type { .. } => "type",
        Action::Press { .. } => "press",
        Action::Scroll { .. } => "scroll",
        Action::Hotkey { .. } => "hotkey",
        Action::Drag { .. } => "drag",
        Action::Swipe { .. } => "swipe",
        Action::LaunchApp { .. } => "launch-app",
        Action::CloseWindow => "close-window",
        Action::MinimizeWindow => "minimize-window",
        Action::MaximizeWindow => "maximize-window",
        Action::MoveWindow { .. } => "move-window",
        Action::ResizeWindow { .. } => "resize-window",
        Action::SetWindowBounds { .. } => "set-window-bounds",
        Action::SwitchApp => "switch-app",
        Action::QuitApp => "quit-app",
        Action::RelaunchApp => "relaunch-app",
        Action::HideApp => "hide-app",
        Action::UnhideApp => "unhide-app",
        Action::FocusWindow { .. } => "focus-window",
    }
}
