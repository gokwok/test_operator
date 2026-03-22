use operator_core::{MouseButton, OperatorError, Point};

pub trait InputSynthesizer: Send + Sync {
    fn click(&self, point: Point, button: MouseButton) -> Result<(), OperatorError>;
    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<(), OperatorError>;
    fn type_text(&self, text: &str) -> Result<(), OperatorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemInputSynthesizer;

impl InputSynthesizer for SystemInputSynthesizer {
    fn click(&self, point: Point, button: MouseButton) -> Result<(), OperatorError> {
        platform::click(point, button)
    }

    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<(), OperatorError> {
        platform::scroll(delta_x, delta_y)
    }

    fn type_text(&self, text: &str) -> Result<(), OperatorError> {
        platform::type_text(text)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{ffi::c_void, thread, time::Duration};

    use operator_core::{MouseButton, OperatorError, Point};

    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: u32 = 0;
    const KCG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const KCG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const KCG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const KCG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const KCG_EVENT_MOUSE_MOVED: u32 = 5;
    const KCG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const KCG_EVENT_OTHER_MOUSE_UP: u32 = 26;
    const KCG_SCROLL_EVENT_UNIT_LINE: u32 = 1;
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

        fn post(&self) {
            unsafe { CGEventPost(KCG_HID_EVENT_TAP, self.0) };
        }
    }

    impl Drop for Event {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0.cast_const()) };
        }
    }

    pub fn click(point: Point, button: MouseButton) -> Result<(), OperatorError> {
        Event::mouse(point, button, KCG_EVENT_MOUSE_MOVED)?.post();
        thread::sleep(Duration::from_millis(10));
        Event::mouse(point, button, mouse_down_event(button))?.post();
        Event::mouse(point, button, mouse_up_event(button))?.post();
        Ok(())
    }

    pub fn scroll(delta_x: f64, delta_y: f64) -> Result<(), OperatorError> {
        let source = EventSource::new()?;
        Event::scroll(&source, delta_x, delta_y)?.post();
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

    fn scroll_delta(delta: f64) -> i32 {
        delta
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use operator_core::{MouseButton, OperatorError, Point};

    pub fn click(_point: Point, _button: MouseButton) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS input synthesis is unavailable on non-macOS hosts".into(),
        ))
    }

    pub fn scroll(_delta_x: f64, _delta_y: f64) -> Result<(), OperatorError> {
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
