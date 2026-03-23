use operator_core::{ClickMode, DragModifier, DragMotion, OperatorError, Point};

pub trait InputSynthesizer: Send + Sync {
    fn click(&self, point: Option<Point>, mode: ClickMode) -> Result<(), OperatorError>;
    fn move_pointer(&self, point: Point) -> Result<(), OperatorError>;
    fn drag(&self, from: Point, to: Point, motion: &DragMotion) -> Result<(), OperatorError>;
    fn hotkey(&self, keys: &[String]) -> Result<(), OperatorError>;
    fn press(&self, key: &str, count: u32) -> Result<(), OperatorError>;
    fn scroll(&self, point: Option<Point>, delta_x: f64, delta_y: f64)
        -> Result<(), OperatorError>;
    fn type_text(&self, text: &str) -> Result<(), OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemInputSynthesizer;

impl InputSynthesizer for SystemInputSynthesizer {
    fn click(&self, point: Option<Point>, mode: ClickMode) -> Result<(), OperatorError> {
        platform::click(point, mode)
    }

    fn move_pointer(&self, point: Point) -> Result<(), OperatorError> {
        platform::move_pointer(point)
    }

    fn drag(&self, from: Point, to: Point, motion: &DragMotion) -> Result<(), OperatorError> {
        platform::drag(from, to, motion)
    }

    fn hotkey(&self, keys: &[String]) -> Result<(), OperatorError> {
        platform::hotkey(keys)
    }

    fn press(&self, key: &str, count: u32) -> Result<(), OperatorError> {
        platform::press(key, count)
    }

    fn scroll(
        &self,
        point: Option<Point>,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<(), OperatorError> {
        platform::scroll(point, delta_x, delta_y)
    }

    fn type_text(&self, text: &str) -> Result<(), OperatorError> {
        platform::type_text(text)
    }
}

const KEY_CODE_A: u16 = 0x00;
const KEY_CODE_S: u16 = 0x01;
const KEY_CODE_D: u16 = 0x02;
const KEY_CODE_F: u16 = 0x03;
const KEY_CODE_H: u16 = 0x04;
const KEY_CODE_G: u16 = 0x05;
const KEY_CODE_Z: u16 = 0x06;
const KEY_CODE_X: u16 = 0x07;
const KEY_CODE_C: u16 = 0x08;
const KEY_CODE_V: u16 = 0x09;
const KEY_CODE_B: u16 = 0x0B;
const KEY_CODE_Q: u16 = 0x0C;
const KEY_CODE_W: u16 = 0x0D;
const KEY_CODE_E: u16 = 0x0E;
const KEY_CODE_R: u16 = 0x0F;
const KEY_CODE_Y: u16 = 0x10;
const KEY_CODE_T: u16 = 0x11;
const KEY_CODE_1: u16 = 0x12;
const KEY_CODE_2: u16 = 0x13;
const KEY_CODE_3: u16 = 0x14;
const KEY_CODE_4: u16 = 0x15;
const KEY_CODE_6: u16 = 0x16;
const KEY_CODE_5: u16 = 0x17;
const KEY_CODE_EQUAL: u16 = 0x18;
const KEY_CODE_9: u16 = 0x19;
const KEY_CODE_7: u16 = 0x1A;
const KEY_CODE_MINUS: u16 = 0x1B;
const KEY_CODE_8: u16 = 0x1C;
const KEY_CODE_0: u16 = 0x1D;
const KEY_CODE_RIGHT_BRACKET: u16 = 0x1E;
const KEY_CODE_O: u16 = 0x1F;
const KEY_CODE_U: u16 = 0x20;
const KEY_CODE_LEFT_BRACKET: u16 = 0x21;
const KEY_CODE_I: u16 = 0x22;
const KEY_CODE_P: u16 = 0x23;
const KEY_CODE_RETURN: u16 = 0x24;
const KEY_CODE_L: u16 = 0x25;
const KEY_CODE_J: u16 = 0x26;
const KEY_CODE_QUOTE: u16 = 0x27;
const KEY_CODE_K: u16 = 0x28;
const KEY_CODE_SEMICOLON: u16 = 0x29;
const KEY_CODE_BACKSLASH: u16 = 0x2A;
const KEY_CODE_COMMA: u16 = 0x2B;
const KEY_CODE_SLASH: u16 = 0x2C;
const KEY_CODE_N: u16 = 0x2D;
const KEY_CODE_M: u16 = 0x2E;
const KEY_CODE_PERIOD: u16 = 0x2F;
const KEY_CODE_TAB: u16 = 0x30;
const KEY_CODE_SPACE: u16 = 0x31;
const KEY_CODE_GRAVE: u16 = 0x32;
const KEY_CODE_DELETE: u16 = 0x33;
const KEY_CODE_ESCAPE: u16 = 0x35;
const KEY_CODE_COMMAND: u16 = 0x37;
const KEY_CODE_SHIFT: u16 = 0x38;
const KEY_CODE_OPTION: u16 = 0x3A;
const KEY_CODE_CONTROL: u16 = 0x3B;
const KEY_CODE_FUNCTION: u16 = 0x3F;
const KEY_CODE_F17: u16 = 0x40;
const KEY_CODE_F18: u16 = 0x4F;
const KEY_CODE_F19: u16 = 0x50;
const KEY_CODE_F20: u16 = 0x5A;
const KEY_CODE_F5: u16 = 0x60;
const KEY_CODE_F6: u16 = 0x61;
const KEY_CODE_F7: u16 = 0x62;
const KEY_CODE_F3: u16 = 0x63;
const KEY_CODE_F8: u16 = 0x64;
const KEY_CODE_F9: u16 = 0x65;
const KEY_CODE_F11: u16 = 0x67;
const KEY_CODE_F13: u16 = 0x69;
const KEY_CODE_F16: u16 = 0x6A;
const KEY_CODE_F14: u16 = 0x6B;
const KEY_CODE_F10: u16 = 0x6D;
const KEY_CODE_F12: u16 = 0x6F;
const KEY_CODE_F15: u16 = 0x71;
const KEY_CODE_HOME: u16 = 0x73;
const KEY_CODE_PAGE_UP: u16 = 0x74;
const KEY_CODE_FORWARD_DELETE: u16 = 0x75;
const KEY_CODE_F4: u16 = 0x76;
const KEY_CODE_END: u16 = 0x77;
const KEY_CODE_F2: u16 = 0x78;
const KEY_CODE_PAGE_DOWN: u16 = 0x79;
const KEY_CODE_F1: u16 = 0x7A;
const KEY_CODE_LEFT_ARROW: u16 = 0x7B;
const KEY_CODE_RIGHT_ARROW: u16 = 0x7C;
const KEY_CODE_DOWN_ARROW: u16 = 0x7D;
const KEY_CODE_UP_ARROW: u16 = 0x7E;
const INPUT_EVENT_DELAY_MS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyModifier {
    Command,
    Control,
    Option,
    Shift,
    Function,
}

impl HotkeyModifier {
    fn key_code(self) -> u16 {
        match self {
            Self::Command => KEY_CODE_COMMAND,
            Self::Control => KEY_CODE_CONTROL,
            Self::Option => KEY_CODE_OPTION,
            Self::Shift => KEY_CODE_SHIFT,
            Self::Function => KEY_CODE_FUNCTION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHotkey {
    modifiers: Vec<HotkeyModifier>,
    key_code: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedDragMotion {
    duration_ms: Option<u64>,
    steps: u32,
    modifiers: Vec<DragModifier>,
}

fn parse_drag_motion(motion: &DragMotion) -> ParsedDragMotion {
    let mut modifiers = Vec::new();
    for modifier in &motion.modifiers {
        if !modifiers.contains(modifier) {
            modifiers.push(*modifier);
        }
    }

    ParsedDragMotion {
        duration_ms: motion.duration_ms,
        steps: motion.steps.map(std::num::NonZeroU32::get).unwrap_or(1),
        modifiers,
    }
}

fn drag_step_delay_ms(duration_ms: Option<u64>, steps: u32) -> u64 {
    match duration_ms {
        Some(0) => 0,
        Some(duration_ms) => (duration_ms / u64::from(steps)).max(1),
        None => INPUT_EVENT_DELAY_MS,
    }
}

fn drag_interpolated_point(from: Point, to: Point, step: u32, steps: u32) -> Point {
    let progress = f64::from(step) / f64::from(steps);
    Point {
        x: from.x + ((to.x - from.x) * progress),
        y: from.y + ((to.y - from.y) * progress),
    }
}

fn drag_modifier_key_code(modifier: DragModifier) -> u16 {
    match modifier {
        DragModifier::Command => KEY_CODE_COMMAND,
        DragModifier::Control => KEY_CODE_CONTROL,
        DragModifier::Option => KEY_CODE_OPTION,
        DragModifier::Shift => KEY_CODE_SHIFT,
        DragModifier::Function => KEY_CODE_FUNCTION,
    }
}

fn parse_hotkey(keys: &[String]) -> Result<ParsedHotkey, OperatorError> {
    if keys.is_empty() {
        return Err(OperatorError::Platform(
            "macOS hotkey requires at least one key".into(),
        ));
    }

    let mut modifiers = Vec::new();
    let mut key_code = None;

    for raw in keys {
        let token = raw.trim();
        if token.is_empty() {
            return Err(OperatorError::Platform(
                "macOS hotkey does not allow empty key tokens".into(),
            ));
        }

        let normalized = token.to_ascii_lowercase();
        if let Some(modifier) = hotkey_modifier(&normalized) {
            if !modifiers.contains(&modifier) {
                modifiers.push(modifier);
            }
            continue;
        }

        let resolved_key = hotkey_key_code(&normalized).ok_or_else(|| {
            OperatorError::Platform(format!("unsupported macOS hotkey key: {token}"))
        })?;
        if key_code.replace(resolved_key).is_some() {
            return Err(OperatorError::Platform(
                "macOS hotkey supports exactly one non-modifier key".into(),
            ));
        }
    }

    let key_code = key_code.ok_or_else(|| {
        OperatorError::Platform("macOS hotkey requires a non-modifier key".into())
    })?;

    Ok(ParsedHotkey {
        modifiers,
        key_code,
    })
}

fn hotkey_modifier(token: &str) -> Option<HotkeyModifier> {
    match token {
        "command" | "cmd" | "meta" | "super" => Some(HotkeyModifier::Command),
        "control" | "ctrl" | "ctl" => Some(HotkeyModifier::Control),
        "option" | "opt" | "alt" => Some(HotkeyModifier::Option),
        "shift" => Some(HotkeyModifier::Shift),
        "function" | "fn" => Some(HotkeyModifier::Function),
        _ => None,
    }
}

fn parse_press_key(key: &str) -> Result<u16, OperatorError> {
    let token = key.trim();
    if token.is_empty() {
        return Err(OperatorError::Platform(
            "macOS press does not allow an empty key".into(),
        ));
    }

    let normalized = token.to_ascii_lowercase();
    if hotkey_modifier(&normalized).is_some() {
        return Err(OperatorError::Platform(format!(
            "unsupported macOS press key: {token}"
        )));
    }

    hotkey_key_code(&normalized)
        .ok_or_else(|| OperatorError::Platform(format!("unsupported macOS press key: {token}")))
}

fn hotkey_key_code(token: &str) -> Option<u16> {
    Some(match token {
        "a" => KEY_CODE_A,
        "b" => KEY_CODE_B,
        "c" => KEY_CODE_C,
        "d" => KEY_CODE_D,
        "e" => KEY_CODE_E,
        "f" => KEY_CODE_F,
        "g" => KEY_CODE_G,
        "h" => KEY_CODE_H,
        "i" => KEY_CODE_I,
        "j" => KEY_CODE_J,
        "k" => KEY_CODE_K,
        "l" => KEY_CODE_L,
        "m" => KEY_CODE_M,
        "n" => KEY_CODE_N,
        "o" => KEY_CODE_O,
        "p" => KEY_CODE_P,
        "q" => KEY_CODE_Q,
        "r" => KEY_CODE_R,
        "s" => KEY_CODE_S,
        "t" => KEY_CODE_T,
        "u" => KEY_CODE_U,
        "v" => KEY_CODE_V,
        "w" => KEY_CODE_W,
        "x" => KEY_CODE_X,
        "y" => KEY_CODE_Y,
        "z" => KEY_CODE_Z,
        "0" => KEY_CODE_0,
        "1" => KEY_CODE_1,
        "2" => KEY_CODE_2,
        "3" => KEY_CODE_3,
        "4" => KEY_CODE_4,
        "5" => KEY_CODE_5,
        "6" => KEY_CODE_6,
        "7" => KEY_CODE_7,
        "8" => KEY_CODE_8,
        "9" => KEY_CODE_9,
        "-" => KEY_CODE_MINUS,
        "=" => KEY_CODE_EQUAL,
        "[" => KEY_CODE_LEFT_BRACKET,
        "]" => KEY_CODE_RIGHT_BRACKET,
        "\\" => KEY_CODE_BACKSLASH,
        ";" => KEY_CODE_SEMICOLON,
        "'" => KEY_CODE_QUOTE,
        "," => KEY_CODE_COMMA,
        "." => KEY_CODE_PERIOD,
        "/" => KEY_CODE_SLASH,
        "`" => KEY_CODE_GRAVE,
        "return" | "enter" => KEY_CODE_RETURN,
        "tab" => KEY_CODE_TAB,
        "space" | "spacebar" => KEY_CODE_SPACE,
        "delete" | "backspace" => KEY_CODE_DELETE,
        "forward-delete" | "forwarddelete" => KEY_CODE_FORWARD_DELETE,
        "escape" | "esc" => KEY_CODE_ESCAPE,
        "left" | "left-arrow" | "arrowleft" => KEY_CODE_LEFT_ARROW,
        "right" | "right-arrow" | "arrowright" => KEY_CODE_RIGHT_ARROW,
        "up" | "up-arrow" | "arrowup" => KEY_CODE_UP_ARROW,
        "down" | "down-arrow" | "arrowdown" => KEY_CODE_DOWN_ARROW,
        "home" => KEY_CODE_HOME,
        "end" => KEY_CODE_END,
        "pageup" | "page-up" => KEY_CODE_PAGE_UP,
        "pagedown" | "page-down" => KEY_CODE_PAGE_DOWN,
        "f1" => KEY_CODE_F1,
        "f2" => KEY_CODE_F2,
        "f3" => KEY_CODE_F3,
        "f4" => KEY_CODE_F4,
        "f5" => KEY_CODE_F5,
        "f6" => KEY_CODE_F6,
        "f7" => KEY_CODE_F7,
        "f8" => KEY_CODE_F8,
        "f9" => KEY_CODE_F9,
        "f10" => KEY_CODE_F10,
        "f11" => KEY_CODE_F11,
        "f12" => KEY_CODE_F12,
        "f13" => KEY_CODE_F13,
        "f14" => KEY_CODE_F14,
        "f15" => KEY_CODE_F15,
        "f16" => KEY_CODE_F16,
        "f17" => KEY_CODE_F17,
        "f18" => KEY_CODE_F18,
        "f19" => KEY_CODE_F19,
        "f20" => KEY_CODE_F20,
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{ffi::c_void, thread, time::Duration};

    use operator_core::{ClickMode, DragModifier, DragMotion, MouseButton, OperatorError, Point};

    use super::{
        drag_interpolated_point, drag_modifier_key_code, drag_step_delay_ms, parse_drag_motion,
        parse_hotkey, HotkeyModifier, INPUT_EVENT_DELAY_MS,
    };

    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: u32 = 0;
    const KCG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const KCG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const KCG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
    const KCG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const KCG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const KCG_EVENT_MOUSE_MOVED: u32 = 5;
    const KCG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const KCG_EVENT_OTHER_MOUSE_UP: u32 = 26;
    const KCG_MOUSE_EVENT_CLICK_STATE: u32 = 1;
    const KCG_SCROLL_EVENT_UNIT_LINE: u32 = 1;
    const KCG_EVENT_FLAG_MASK_SHIFT: u64 = 0x0002_0000;
    const KCG_EVENT_FLAG_MASK_CONTROL: u64 = 0x0004_0000;
    const KCG_EVENT_FLAG_MASK_OPTION: u64 = 0x0008_0000;
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;
    const KCG_EVENT_FLAG_MASK_FUNCTION: u64 = 0x0080_0000;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    type CGEventRef = *mut c_void;
    type CGEventSourceRef = *mut c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
        fn CGEventSourceCreate(state_id: u32) -> CGEventSourceRef;
        fn CGEventCreateMouseEvent(
            source: CGEventSourceRef,
            mouse_type: u32,
            mouse_cursor_position: CGPoint,
            mouse_button: u32,
        ) -> CGEventRef;
        fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            virtual_key: u16,
            key_down: bool,
        ) -> CGEventRef;
        fn CGEventCreateScrollWheelEvent(
            source: CGEventSourceRef,
            units: u32,
            wheel_count: u32,
            ...
        ) -> CGEventRef;
        fn CGEventKeyboardSetUnicodeString(
            event: CGEventRef,
            string_length: u64,
            unicode_string: *const u16,
        );
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
        fn CGEventPost(tap: u32, event: CGEventRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    struct EventSource(CGEventSourceRef);

    impl EventSource {
        fn new() -> Result<Self, OperatorError> {
            let source =
                unsafe { CGEventSourceCreate(KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE) };
            if source.is_null() {
                return Err(OperatorError::Platform(
                    "failed to create macOS event source".into(),
                ));
            }

            Ok(Self(source))
        }
    }

    impl Drop for EventSource {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0.cast_const()) };
        }
    }

    struct Event(CGEventRef);

    impl Event {
        fn current() -> Result<Self, OperatorError> {
            let event = unsafe { CGEventCreate(std::ptr::null_mut()) };
            if event.is_null() {
                return Err(OperatorError::Platform(
                    "failed to read current macOS event state".into(),
                ));
            }

            Ok(Self(event))
        }

        fn mouse(
            point: Point,
            button: MouseButton,
            event_type: u32,
        ) -> Result<Self, OperatorError> {
            let event = unsafe {
                CGEventCreateMouseEvent(
                    std::ptr::null_mut(),
                    event_type,
                    CGPoint {
                        x: point.x,
                        y: point.y,
                    },
                    mouse_button(button),
                )
            };
            if event.is_null() {
                return Err(OperatorError::Platform(
                    "failed to create macOS mouse event".into(),
                ));
            }

            Ok(Self(event))
        }

        fn keyboard(
            source: &EventSource,
            units: &[u16],
            key_down: bool,
        ) -> Result<Self, OperatorError> {
            let event = unsafe { CGEventCreateKeyboardEvent(source.0, 0, key_down) };
            if event.is_null() {
                return Err(OperatorError::Platform(
                    "failed to create macOS keyboard event".into(),
                ));
            }

            unsafe { CGEventKeyboardSetUnicodeString(event, units.len() as u64, units.as_ptr()) };
            Ok(Self(event))
        }

        fn keyboard_keycode(
            source: &EventSource,
            key_code: u16,
            key_down: bool,
        ) -> Result<Self, OperatorError> {
            let event = unsafe { CGEventCreateKeyboardEvent(source.0, key_code, key_down) };
            if event.is_null() {
                return Err(OperatorError::Platform(
                    "failed to create macOS keyboard event".into(),
                ));
            }

            Ok(Self(event))
        }

        fn scroll(source: &EventSource, delta_x: f64, delta_y: f64) -> Result<Self, OperatorError> {
            let event = unsafe {
                CGEventCreateScrollWheelEvent(
                    source.0,
                    KCG_SCROLL_EVENT_UNIT_LINE,
                    2,
                    scroll_delta(delta_y),
                    scroll_delta(delta_x),
                )
            };
            if event.is_null() {
                return Err(OperatorError::Platform(
                    "failed to create macOS scroll event".into(),
                ));
            }

            Ok(Self(event))
        }

        fn set_flags(&self, flags: u64) {
            unsafe { CGEventSetFlags(self.0, flags) };
        }

        fn point(&self) -> Point {
            let point = unsafe { CGEventGetLocation(self.0) };
            Point {
                x: point.x,
                y: point.y,
            }
        }

        fn set_click_state(&self, click_state: i64) {
            unsafe {
                CGEventSetIntegerValueField(self.0, KCG_MOUSE_EVENT_CLICK_STATE, click_state)
            };
        }

        fn post(&self) {
            unsafe { CGEventPost(KCG_HID_EVENT_TAP, self.0) };
        }
    }

    impl Drop for Event {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0.cast_const()) };
        }
    }

    pub fn click(point: Option<Point>, mode: ClickMode) -> Result<(), OperatorError> {
        let should_move = point.is_some();
        let point = point.unwrap_or(current_pointer_position()?);
        let button = click_button(mode);
        let click_count = click_count(mode);

        if should_move {
            Event::mouse(point, button, KCG_EVENT_MOUSE_MOVED)?.post();
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
        }

        for click_state in 1..=click_count {
            let down = Event::mouse(point, button, mouse_down_event(button))?;
            down.set_click_state(click_state);
            down.post();

            let up = Event::mouse(point, button, mouse_up_event(button))?;
            up.set_click_state(click_state);
            up.post();

            if click_state < click_count {
                thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
            }
        }

        Ok(())
    }

    pub fn move_pointer(point: Point) -> Result<(), OperatorError> {
        Event::mouse(point, MouseButton::Left, KCG_EVENT_MOUSE_MOVED)?.post();
        Ok(())
    }

    pub fn drag(from: Point, to: Point, motion: &DragMotion) -> Result<(), OperatorError> {
        let motion = parse_drag_motion(motion);
        let source = EventSource::new()?;
        let flags = motion.modifiers.iter().fold(0u64, |combined, modifier| {
            combined | drag_modifier_flag(*modifier)
        });

        for modifier in &motion.modifiers {
            Event::keyboard_keycode(&source, drag_modifier_key_code(*modifier), true)?.post();
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
        }

        let moved = Event::mouse(from, MouseButton::Left, KCG_EVENT_MOUSE_MOVED)?;
        moved.set_flags(flags);
        moved.post();
        thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));

        let down = Event::mouse(from, MouseButton::Left, KCG_EVENT_LEFT_MOUSE_DOWN)?;
        down.set_flags(flags);
        down.post();

        let step_delay_ms = drag_step_delay_ms(motion.duration_ms, motion.steps);
        for step in 1..=motion.steps {
            thread::sleep(Duration::from_millis(step_delay_ms));
            let point = drag_interpolated_point(from, to, step, motion.steps);
            let dragged = Event::mouse(point, MouseButton::Left, KCG_EVENT_LEFT_MOUSE_DRAGGED)?;
            dragged.set_flags(flags);
            dragged.post();
        }

        thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
        let up = Event::mouse(to, MouseButton::Left, KCG_EVENT_LEFT_MOUSE_UP)?;
        up.set_flags(flags);
        up.post();

        for modifier in motion.modifiers.iter().rev() {
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
            Event::keyboard_keycode(&source, drag_modifier_key_code(*modifier), false)?.post();
        }

        Ok(())
    }

    pub fn scroll(point: Option<Point>, delta_x: f64, delta_y: f64) -> Result<(), OperatorError> {
        let source = EventSource::new()?;
        if let Some(point) = point {
            Event::mouse(point, MouseButton::Left, KCG_EVENT_MOUSE_MOVED)?.post();
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
        }
        Event::scroll(&source, delta_x, delta_y)?.post();
        Ok(())
    }

    pub fn hotkey(keys: &[String]) -> Result<(), OperatorError> {
        let parsed = parse_hotkey(keys)?;
        let source = EventSource::new()?;
        let flags = parsed.modifiers.iter().fold(0u64, |combined, modifier| {
            combined | modifier_flag(*modifier)
        });

        for modifier in &parsed.modifiers {
            Event::keyboard_keycode(&source, modifier.key_code(), true)?.post();
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
        }

        let key_down = Event::keyboard_keycode(&source, parsed.key_code, true)?;
        key_down.set_flags(flags);
        key_down.post();
        thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));

        let key_up = Event::keyboard_keycode(&source, parsed.key_code, false)?;
        key_up.set_flags(flags);
        key_up.post();

        for modifier in parsed.modifiers.iter().rev() {
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
            Event::keyboard_keycode(&source, modifier.key_code(), false)?.post();
        }

        Ok(())
    }

    pub fn press(key: &str, count: u32) -> Result<(), OperatorError> {
        let key_code = super::parse_press_key(key)?;
        let source = EventSource::new()?;

        for press_index in 0..count {
            Event::keyboard_keycode(&source, key_code, true)?.post();
            thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
            Event::keyboard_keycode(&source, key_code, false)?.post();
            if press_index + 1 < count {
                thread::sleep(Duration::from_millis(INPUT_EVENT_DELAY_MS));
            }
        }

        Ok(())
    }

    pub fn type_text(text: &str) -> Result<(), OperatorError> {
        let source = EventSource::new()?;
        for character in text.chars() {
            let mut units = [0u16; 2];
            let encoded = character.encode_utf16(&mut units);
            Event::keyboard(&source, encoded, true)?.post();
            Event::keyboard(&source, encoded, false)?.post();
        }
        Ok(())
    }

    fn mouse_button(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }

    fn mouse_down_event(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => KCG_EVENT_LEFT_MOUSE_DOWN,
            MouseButton::Right => KCG_EVENT_RIGHT_MOUSE_DOWN,
            MouseButton::Middle => KCG_EVENT_OTHER_MOUSE_DOWN,
        }
    }

    fn mouse_up_event(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => KCG_EVENT_LEFT_MOUSE_UP,
            MouseButton::Right => KCG_EVENT_RIGHT_MOUSE_UP,
            MouseButton::Middle => KCG_EVENT_OTHER_MOUSE_UP,
        }
    }

    fn modifier_flag(modifier: HotkeyModifier) -> u64 {
        match modifier {
            HotkeyModifier::Command => KCG_EVENT_FLAG_MASK_COMMAND,
            HotkeyModifier::Control => KCG_EVENT_FLAG_MASK_CONTROL,
            HotkeyModifier::Option => KCG_EVENT_FLAG_MASK_OPTION,
            HotkeyModifier::Shift => KCG_EVENT_FLAG_MASK_SHIFT,
            HotkeyModifier::Function => KCG_EVENT_FLAG_MASK_FUNCTION,
        }
    }

    fn scroll_delta(delta: f64) -> i32 {
        delta
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }

    fn drag_modifier_flag(modifier: DragModifier) -> u64 {
        match modifier {
            DragModifier::Command => KCG_EVENT_FLAG_MASK_COMMAND,
            DragModifier::Control => KCG_EVENT_FLAG_MASK_CONTROL,
            DragModifier::Option => KCG_EVENT_FLAG_MASK_OPTION,
            DragModifier::Shift => KCG_EVENT_FLAG_MASK_SHIFT,
            DragModifier::Function => KCG_EVENT_FLAG_MASK_FUNCTION,
        }
    }

    fn current_pointer_position() -> Result<Point, OperatorError> {
        Ok(Event::current()?.point())
    }

    fn click_button(mode: ClickMode) -> MouseButton {
        match mode {
            ClickMode::Left | ClickMode::Double => MouseButton::Left,
            ClickMode::Right => MouseButton::Right,
            ClickMode::Middle => MouseButton::Middle,
        }
    }

    fn click_count(mode: ClickMode) -> i64 {
        match mode {
            ClickMode::Double => 2,
            ClickMode::Left | ClickMode::Right | ClickMode::Middle => 1,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use operator_core::{ClickMode, DragMotion, OperatorError, Point};

    pub fn click(_point: Option<Point>, _mode: ClickMode) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn move_pointer(_point: Point) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn drag(_from: Point, _to: Point, _motion: &DragMotion) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn scroll(
        _point: Option<Point>,
        _delta_x: f64,
        _delta_y: f64,
    ) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn hotkey(_keys: &[String]) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn press(_key: &str, _count: u32) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn type_text(_text: &str) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use operator_core::{DragModifier, DragMotion, Point};

    use super::{
        drag_interpolated_point, drag_step_delay_ms, parse_drag_motion, parse_hotkey,
        parse_press_key, HotkeyModifier, ParsedDragMotion, ParsedHotkey, KEY_CODE_DOWN_ARROW,
        KEY_CODE_P, KEY_CODE_RETURN,
    };

    #[test]
    fn parse_hotkey_accepts_modifier_synonyms_and_deduplicates() {
        let parsed = parse_hotkey(&[
            "cmd".to_string(),
            "shift".to_string(),
            "command".to_string(),
            "P".to_string(),
        ])
        .unwrap();

        assert_eq!(
            parsed,
            ParsedHotkey {
                modifiers: vec![HotkeyModifier::Command, HotkeyModifier::Shift],
                key_code: KEY_CODE_P,
            }
        );
    }

    #[test]
    fn parse_hotkey_rejects_missing_primary_key() {
        let error = parse_hotkey(&["command".to_string(), "shift".to_string()]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "platform error: macOS hotkey requires a non-modifier key"
        );
    }

    #[test]
    fn parse_hotkey_supports_named_non_modifier_keys() {
        let parsed = parse_hotkey(&["control".to_string(), "return".to_string()]).unwrap();
        assert_eq!(
            parsed,
            ParsedHotkey {
                modifiers: vec![HotkeyModifier::Control],
                key_code: KEY_CODE_RETURN,
            }
        );
    }

    #[test]
    fn parse_press_accepts_named_navigation_keys() {
        assert_eq!(parse_press_key("down").unwrap(), KEY_CODE_DOWN_ARROW);
    }

    #[test]
    fn parse_press_rejects_modifier_only_keys() {
        let error = parse_press_key("shift").unwrap_err();
        assert_eq!(
            error.to_string(),
            "platform error: unsupported macOS press key: shift"
        );
    }

    #[test]
    fn parse_drag_motion_defaults_and_deduplicates_modifiers() {
        let parsed = parse_drag_motion(&DragMotion {
            duration_ms: Some(300),
            steps: Some(6.try_into().unwrap()),
            modifiers: vec![
                DragModifier::Command,
                DragModifier::Shift,
                DragModifier::Command,
            ],
        });

        assert_eq!(
            parsed,
            ParsedDragMotion {
                duration_ms: Some(300),
                steps: 6,
                modifiers: vec![DragModifier::Command, DragModifier::Shift],
            }
        );
    }

    #[test]
    fn drag_step_delay_distributes_duration_across_steps() {
        assert_eq!(drag_step_delay_ms(Some(300), 6), 50);
        assert_eq!(drag_step_delay_ms(None, 4), super::INPUT_EVENT_DELAY_MS);
    }

    #[test]
    fn drag_interpolation_reaches_target_on_last_step() {
        let from = Point { x: 10.0, y: 20.0 };
        let to = Point { x: 30.0, y: 60.0 };

        assert_eq!(
            drag_interpolated_point(from, to, 3, 6),
            Point { x: 20.0, y: 40.0 }
        );
        assert_eq!(drag_interpolated_point(from, to, 6, 6), to);
    }
}
