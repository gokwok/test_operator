#![allow(unexpected_cfgs)]

use operator_core::OperatorError;

use super::EffectRequest;

pub(super) fn run(payload: &str) -> Result<(), OperatorError> {
    let request: EffectRequest = serde_json::from_str(payload)?;
    platform::run(&request)
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use operator_core::OperatorError;

    use super::EffectRequest;

    pub(super) fn run(_request: &EffectRequest) -> Result<(), OperatorError> {
        Err(OperatorError::Platform(
            "macOS action effects helper is only available on macOS".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::time::Duration;

    use cocoa::{
        appkit::{
            NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSScreen,
            NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
        },
        base::{id, nil, NO, YES},
        foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString},
        quartzcore::{current_media_time, transaction, CALayer},
    };
    use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
    use core_graphics::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
    use objc::{class, msg_send, sel, sel_impl};
    use operator_core::OperatorError;

    use super::super::{EffectKind, EffectPoint, EffectRequest};

    const FRAME_INTERVAL: f64 = 1.0 / 120.0;
    const CLICK_DURATION: f64 = 0.55;
    const MOVE_DURATION: f64 = 0.52;
    const DRAG_DURATION: f64 = 0.72;
    const SCROLL_DURATION: f64 = 0.58;
    const KEYBOARD_DURATION: f64 = 0.95;
    const MIN_PANEL_WIDTH: f64 = 220.0;
    const MAX_PANEL_WIDTH: f64 = 420.0;

    unsafe extern "C" {
        fn CGShieldingWindowLevel() -> i32;
        fn CFAbsoluteTimeGetCurrent() -> f64;
    }

    pub(super) fn run(request: &EffectRequest) -> Result<(), OperatorError> {
        let pool = unsafe { NSAutoreleasePool::new(nil) };
        let outcome = run_inner(request);
        unsafe { pool.drain() };
        outcome
    }

    fn run_inner(request: &EffectRequest) -> Result<(), OperatorError> {
        let app = unsafe { NSApplication::sharedApplication(nil) };
        unsafe {
            app.setActivationPolicy_(
                NSApplicationActivationPolicy::NSApplicationActivationPolicyProhibited,
            );
            let _: () = msg_send![app, finishLaunching];
        }

        let overlay = unsafe { OverlayWindow::new()? };
        overlay.render(request)?;
        unsafe { overlay.show() };
        run_animation_for(effect_duration(request));
        unsafe { overlay.close() };
        Ok(())
    }

    fn effect_duration(request: &EffectRequest) -> f64 {
        match request.kind {
            EffectKind::Click if request.mode.as_deref() == Some("double") => CLICK_DURATION + 0.14,
            EffectKind::Click => CLICK_DURATION,
            EffectKind::Move => MOVE_DURATION,
            EffectKind::Drag => DRAG_DURATION,
            EffectKind::Scroll => SCROLL_DURATION,
            EffectKind::Keyboard => KEYBOARD_DURATION,
        }
    }

    fn run_animation_for(duration: f64) {
        let mut elapsed = 0.0;
        let mut last_tick = unsafe { CFAbsoluteTimeGetCurrent() };

        while elapsed < duration {
            let remaining = duration - elapsed;
            let slice = remaining.min(FRAME_INTERVAL);
            let _ = CFRunLoop::run_in_mode(
                unsafe { kCFRunLoopDefaultMode },
                Duration::from_secs_f64(slice),
                false,
            );

            let now = unsafe { CFAbsoluteTimeGetCurrent() };
            elapsed += (now - last_tick).max(0.0);
            last_tick = now;
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct DesktopFrame {
        min_x: f64,
        min_y: f64,
        width: f64,
        height: f64,
        scale: f64,
    }

    impl DesktopFrame {
        unsafe fn detect() -> Result<Self, OperatorError> {
            let screens = NSScreen::screens(nil);
            if screens == nil {
                return Err(OperatorError::Platform(
                    "failed to enumerate macOS screens for action effects".into(),
                ));
            }

            let count: usize = msg_send![screens, count];
            if count == 0 {
                return Err(OperatorError::Platform(
                    "no active macOS screen found for action effects".into(),
                ));
            }

            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;

            for index in 0..count {
                let screen: id = msg_send![screens, objectAtIndex: index];
                let frame = NSScreen::frame(screen);
                min_x = min_x.min(frame.origin.x);
                min_y = min_y.min(frame.origin.y);
                max_x = max_x.max(frame.origin.x + frame.size.width);
                max_y = max_y.max(frame.origin.y + frame.size.height);
            }

            let main_screen = NSScreen::mainScreen(nil);
            let scale = if main_screen == nil {
                1.0
            } else {
                NSScreen::backingScaleFactor(main_screen)
            };

            Ok(Self {
                min_x,
                min_y,
                width: (max_x - min_x).max(1.0),
                height: (max_y - min_y).max(1.0),
                scale: scale.max(1.0),
            })
        }
    }

    struct OverlayWindow {
        window: id,
        host_layer: CALayer,
        desktop: DesktopFrame,
    }

    impl OverlayWindow {
        unsafe fn new() -> Result<Self, OperatorError> {
            let desktop = DesktopFrame::detect()?;
            let frame = NSRect::new(
                NSPoint::new(desktop.min_x, desktop.min_y),
                NSSize::new(desktop.width, desktop.height),
            );

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                NSWindowStyleMask::NSBorderlessWindowMask,
                NSBackingStoreType::NSBackingStoreBuffered,
                NO,
            );
            if window == nil {
                return Err(OperatorError::Platform(
                    "failed to create macOS action effects window".into(),
                ));
            }

            window.setBackgroundColor_(NSColor::clearColor(nil));
            window.setOpaque_(NO);
            window.setHasShadow_(NO);
            window.setIgnoresMouseEvents_(YES);
            window.setLevel_(CGShieldingWindowLevel() as i64);
            window.setCollectionBehavior_(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient,
            );

            let content_view = window.contentView();
            if content_view == nil {
                return Err(OperatorError::Platform(
                    "failed to access macOS action effects content view".into(),
                ));
            }

            content_view.setWantsLayer(YES);

            let host_layer = new_layer();
            let bounds = CGRect::new(
                &CGPoint::new(0.0, 0.0),
                &CGSize::new(desktop.width, desktop.height),
            );
            transaction::begin();
            transaction::set_disable_actions(true);
            host_layer.set_frame(&bounds);
            host_layer.set_bounds(&bounds);
            host_layer.set_contents_scale(desktop.scale);
            host_layer.set_masks_to_bounds(false);
            content_view.setLayer(host_layer.id());
            transaction::commit();

            Ok(Self {
                window,
                host_layer,
                desktop,
            })
        }

        fn render(&self, request: &EffectRequest) -> Result<(), OperatorError> {
            transaction::begin();
            transaction::set_disable_actions(true);
            let outcome = match request.kind {
                EffectKind::Click => self.render_click(
                    required_point(request.point, "point")?,
                    request.mode.as_deref().unwrap_or("left"),
                ),
                EffectKind::Move => self.render_move(required_point(request.point, "point")?),
                EffectKind::Drag => self.render_drag(
                    required_point(request.from, "from")?,
                    required_point(request.to, "to")?,
                ),
                EffectKind::Scroll => self.render_scroll(
                    required_point(request.point, "point")?,
                    request.dx.unwrap_or_default(),
                    request.dy.unwrap_or_default(),
                ),
                EffectKind::Keyboard => self.render_keyboard(
                    request
                        .label
                        .as_deref()
                        .ok_or_else(|| missing_field("label"))?,
                ),
            };
            transaction::commit();
            transaction::flush();
            outcome
        }

        unsafe fn show(&self) {
            self.window.orderFrontRegardless();
        }

        unsafe fn close(&self) {
            self.window.orderOut_(nil);
            self.window.close();
        }

        fn local_point(&self, point: EffectPoint) -> CGPoint {
            CGPoint::new(
                (point.x - self.desktop.min_x).clamp(0.0, self.desktop.width),
                (self.desktop.height - point.y).clamp(0.0, self.desktop.height),
            )
        }

        fn render_click(&self, point: EffectPoint, mode: &str) -> Result<(), OperatorError> {
            let center = self.local_point(point);
            let accent = effect_accent();
            let soft = accent.with_alpha(0.28);

            let ring = add_circle_layer(
                &self.host_layer,
                center,
                92.0,
                Some(soft),
                Some(accent),
                3.0,
            );
            animate_scale(&ring, 0.34, 1.0, CLICK_DURATION, 0.0);
            animate_opacity(&ring, 0.96, 0.0, CLICK_DURATION, 0.0);

            let core = add_circle_layer(&self.host_layer, center, 18.0, Some(accent), None, 0.0);
            animate_scale(&core, 0.52, 1.18, 0.26, 0.0);
            animate_opacity(&core, 0.92, 0.0, 0.26, 0.0);

            if mode == "double" {
                let second_ring = add_circle_layer(
                    &self.host_layer,
                    center,
                    122.0,
                    None,
                    Some(accent.with_alpha(0.72)),
                    2.0,
                );
                animate_scale(&second_ring, 0.38, 1.0, CLICK_DURATION, 0.08);
                animate_opacity(&second_ring, 0.82, 0.0, CLICK_DURATION, 0.08);
            }

            Ok(())
        }

        fn render_move(&self, point: EffectPoint) -> Result<(), OperatorError> {
            let center = self.local_point(point);
            let accent = effect_accent();
            let start = CGPoint::new(center.x - 68.0, center.y + 42.0);
            let trail = add_line_layer(&self.host_layer, start, center, 10.0, accent);
            animate_scale_x(&trail, 0.18, 1.0, MOVE_DURATION, 0.0);
            animate_opacity(&trail, 0.86, 0.0, MOVE_DURATION, 0.0);

            let pulse = add_circle_layer(
                &self.host_layer,
                center,
                78.0,
                Some(accent.with_alpha(0.14)),
                Some(accent),
                2.5,
            );
            animate_scale(&pulse, 0.42, 1.0, MOVE_DURATION, 0.0);
            animate_opacity(&pulse, 0.94, 0.0, MOVE_DURATION, 0.0);

            let dot = add_circle_layer(
                &self.host_layer,
                center,
                14.0,
                Some(accent.with_alpha(0.98)),
                None,
                0.0,
            );
            animate_scale(&dot, 0.6, 1.0, 0.22, 0.0);
            animate_opacity(&dot, 0.88, 0.0, 0.22, 0.0);

            Ok(())
        }

        fn render_drag(&self, from: EffectPoint, to: EffectPoint) -> Result<(), OperatorError> {
            let from = self.local_point(from);
            let to = self.local_point(to);
            let path = effect_accent().with_alpha(0.92);
            let drop = effect_accent();

            let line = add_line_layer(&self.host_layer, from, to, 10.0, path);
            animate_scale_x(&line, 0.08, 1.0, 0.34, 0.0);
            animate_opacity(&line, 0.92, 0.0, DRAG_DURATION, 0.0);

            let handle = add_circle_layer(
                &self.host_layer,
                from,
                18.0,
                Some(path.with_alpha(0.22)),
                Some(path),
                2.0,
            );
            animate_scale(&handle, 0.7, 1.02, 0.28, 0.0);
            animate_opacity(&handle, 0.88, 0.0, 0.5, 0.0);

            let pulse = add_circle_layer(
                &self.host_layer,
                to,
                88.0,
                Some(drop.with_alpha(0.14)),
                Some(drop),
                3.0,
            );
            animate_scale(&pulse, 0.34, 1.0, DRAG_DURATION, 0.0);
            animate_opacity(&pulse, 0.95, 0.0, DRAG_DURATION, 0.0);

            let target = add_circle_layer(&self.host_layer, to, 20.0, Some(drop), None, 0.0);
            animate_scale(&target, 0.58, 1.12, 0.24, 0.0);
            animate_opacity(&target, 0.92, 0.0, 0.24, 0.0);

            Ok(())
        }

        fn render_scroll(&self, point: EffectPoint, dx: f64, dy: f64) -> Result<(), OperatorError> {
            let center = self.local_point(point);
            let accent = effect_accent();
            let (unit_x, unit_y) = normalized_vector(dx, -dy);
            let length = ((dx.abs() + dy.abs()) * 0.35).clamp(42.0, 88.0);
            let half = length * 0.48;
            let from = CGPoint::new(center.x - unit_x * half, center.y - unit_y * half);
            let to = CGPoint::new(center.x + unit_x * half, center.y + unit_y * half);

            let trail = add_line_layer(&self.host_layer, from, to, 12.0, accent);
            animate_scale_x(&trail, 0.2, 1.0, SCROLL_DURATION, 0.0);
            animate_opacity(&trail, 0.9, 0.0, SCROLL_DURATION, 0.0);

            let pulse = add_circle_layer(
                &self.host_layer,
                center,
                74.0,
                Some(accent.with_alpha(0.14)),
                Some(accent),
                2.5,
            );
            animate_scale(&pulse, 0.42, 1.0, SCROLL_DURATION, 0.0);
            animate_opacity(&pulse, 0.92, 0.0, SCROLL_DURATION, 0.0);

            Ok(())
        }

        fn render_keyboard(&self, label: &str) -> Result<(), OperatorError> {
            let panel_width = ((label.chars().count() as f64) * 9.5 + 96.0)
                .clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH);
            let panel_height = 86.0;
            let panel_x = (self.desktop.width - panel_width) / 2.0;
            let panel_y = 56.0;
            let panel_frame = CGRect::new(
                &CGPoint::new(panel_x, panel_y),
                &CGSize::new(panel_width, panel_height),
            );

            let panel = new_layer();
            panel.set_frame(&panel_frame);
            panel.set_corner_radius(18.0);
            unsafe {
                set_layer_background_color(&panel, Some(Rgba::new(0.07, 0.09, 0.12, 0.92)));
                set_layer_border_color(&panel, Some(Rgba::new(1.0, 0.47, 0.33, 0.72)));
                set_layer_shadow_color(&panel, Some(Rgba::new(0.0, 0.0, 0.0, 0.9)));
            }
            panel.set_border_width(1.4);
            panel.set_shadow_opacity(0.32);
            panel.set_shadow_radius(24.0);
            panel.set_shadow_offset(&CGSize::new(0.0, 12.0));
            panel.set_contents_scale(self.desktop.scale);
            panel.set_opacity(0.0);
            self.host_layer.add_sublayer(&panel);

            let accent_bar = new_layer();
            accent_bar.set_frame(&CGRect::new(
                &CGPoint::new((panel_width - 72.0) / 2.0, panel_height - 12.0),
                &CGSize::new(72.0, 4.0),
            ));
            accent_bar.set_corner_radius(2.0);
            unsafe {
                set_layer_background_color(&accent_bar, Some(Rgba::new(1.0, 0.47, 0.33, 0.98)));
            }
            panel.add_sublayer(&accent_bar);

            add_text_layer(
                panel.id(),
                self.desktop.scale,
                TextLayerSpec {
                    frame: CGRect::new(
                        &CGPoint::new(0.0, panel_height - 34.0),
                        &CGSize::new(panel_width, 16.0),
                    ),
                    text: "KEYBOARD",
                    font_name: "Menlo-Bold",
                    font_size: 12.0,
                    color: Rgba::new(1.0, 0.56, 0.42, 1.0),
                    alignment: "center",
                },
            );
            add_text_layer(
                panel.id(),
                self.desktop.scale,
                TextLayerSpec {
                    frame: CGRect::new(
                        &CGPoint::new(18.0, 22.0),
                        &CGSize::new(panel_width - 36.0, 28.0),
                    ),
                    text: label,
                    font_name: "Menlo",
                    font_size: 22.0,
                    color: Rgba::new(0.96, 0.98, 1.0, 1.0),
                    alignment: "center",
                },
            );

            animate_scale(&panel, 0.96, 1.0, KEYBOARD_DURATION, 0.0);
            animate_opacity(&panel, 0.98, 0.0, KEYBOARD_DURATION, 0.0);

            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct Rgba {
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    }

    impl Rgba {
        const fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
            Self {
                red,
                green,
                blue,
                alpha,
            }
        }

        fn with_alpha(self, alpha: f64) -> Self {
            Self { alpha, ..self }
        }
    }

    struct TextLayerSpec<'a> {
        frame: CGRect,
        text: &'a str,
        font_name: &'a str,
        font_size: f64,
        color: Rgba,
        alignment: &'a str,
    }

    fn effect_accent() -> Rgba {
        Rgba::new(1.0, 0.42, 0.35, 0.98)
    }

    fn required_point(
        point: Option<EffectPoint>,
        field: &str,
    ) -> Result<EffectPoint, OperatorError> {
        point.ok_or_else(|| missing_field(field))
    }

    fn missing_field(field: &str) -> OperatorError {
        OperatorError::Platform(format!(
            "macOS action effects payload missing required `{field}` field"
        ))
    }

    fn normalized_vector(x: f64, y: f64) -> (f64, f64) {
        let magnitude = (x * x + y * y).sqrt();
        if magnitude <= f64::EPSILON {
            (0.0, 1.0)
        } else {
            (x / magnitude, y / magnitude)
        }
    }

    fn add_circle_layer(
        parent: &CALayer,
        center: CGPoint,
        diameter: f64,
        fill: Option<Rgba>,
        border: Option<Rgba>,
        border_width: f64,
    ) -> CALayer {
        let layer = new_layer();
        layer.set_frame(&CGRect::new(
            &CGPoint::new(center.x - diameter / 2.0, center.y - diameter / 2.0),
            &CGSize::new(diameter, diameter),
        ));
        layer.set_corner_radius(diameter / 2.0);
        unsafe {
            set_layer_background_color(&layer, fill);
            set_layer_border_color(&layer, border);
        }
        layer.set_border_width(border_width);
        layer.set_opacity(0.0);
        parent.add_sublayer(&layer);
        layer
    }

    fn add_line_layer(
        parent: &CALayer,
        from: CGPoint,
        to: CGPoint,
        thickness: f64,
        color: Rgba,
    ) -> CALayer {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length = (dx * dx + dy * dy).sqrt().max(1.0);
        let mid = CGPoint::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        let angle = dy.atan2(dx);
        let rotation = CGAffineTransform::new(
            angle.cos(),
            angle.sin(),
            -angle.sin(),
            angle.cos(),
            0.0,
            0.0,
        );

        let layer = new_layer();
        layer.set_frame(&CGRect::new(
            &CGPoint::new(mid.x - length / 2.0, mid.y - thickness / 2.0),
            &CGSize::new(length, thickness),
        ));
        layer.set_corner_radius(thickness / 2.0);
        unsafe {
            set_layer_background_color(&layer, Some(color));
        }
        layer.set_affine_transform(&rotation);
        layer.set_opacity(0.0);
        parent.add_sublayer(&layer);
        layer
    }

    fn animate_scale(layer: &CALayer, from: f64, to: f64, duration: f64, delay: f64) {
        add_number_animation(layer.id(), "transform.scale", from, to, duration, delay);
    }

    fn animate_scale_x(layer: &CALayer, from: f64, to: f64, duration: f64, delay: f64) {
        add_number_animation(layer.id(), "transform.scale.x", from, to, duration, delay);
    }

    fn animate_opacity(layer: &CALayer, from: f64, to: f64, duration: f64, delay: f64) {
        add_number_animation(layer.id(), "opacity", from, to, duration, delay);
    }

    fn add_number_animation(
        target: id,
        key_path: &str,
        from: f64,
        to: f64,
        duration: f64,
        delay: f64,
    ) {
        unsafe {
            let animation: id =
                msg_send![class!(CABasicAnimation), animationWithKeyPath: nsstring(key_path)];
            let _: () = msg_send![animation, setFromValue: nsnumber(from)];
            let _: () = msg_send![animation, setToValue: nsnumber(to)];
            let _: () = msg_send![animation, setDuration: duration];
            if delay > 0.0 {
                let _: () = msg_send![animation, setBeginTime: current_media_time() + delay];
            }
            let _: () = msg_send![animation, setRemovedOnCompletion: NO];
            let _: () = msg_send![animation, setFillMode: nsstring("forwards")];
            let _: () = msg_send![target, addAnimation: animation forKey: nil];
        }
    }

    fn add_text_layer(parent: id, scale: f64, spec: TextLayerSpec<'_>) {
        unsafe {
            let layer: id = msg_send![class!(CATextLayer), layer];
            let _: () = msg_send![layer, setFrame: spec.frame];
            let _: () = msg_send![layer, setContentsScale: scale];
            let _: () = msg_send![layer, setString: nsstring(spec.text)];
            let _: () = msg_send![layer, setFont: nsstring(spec.font_name)];
            let _: () = msg_send![layer, setFontSize: spec.font_size];
            let _: () = msg_send![layer, setForegroundColor: cgcolor(spec.color)];
            let _: () = msg_send![layer, setAlignmentMode: nsstring(spec.alignment)];
            let _: () = msg_send![layer, setWrapped: NO];
            let _: () = msg_send![parent, addSublayer: layer];
        }
    }

    fn new_layer() -> CALayer {
        let layer = CALayer::new();
        unsafe {
            let _: id = msg_send![layer.id(), retain];
        }
        layer
    }

    unsafe fn set_layer_background_color(layer: &CALayer, color: Option<Rgba>) {
        let cgcolor = match color {
            Some(color) => cgcolor(color),
            None => nil,
        };
        let _: () = msg_send![layer.id(), setBackgroundColor: cgcolor];
    }

    unsafe fn set_layer_border_color(layer: &CALayer, color: Option<Rgba>) {
        let cgcolor = match color {
            Some(color) => cgcolor(color),
            None => nil,
        };
        let _: () = msg_send![layer.id(), setBorderColor: cgcolor];
    }

    unsafe fn set_layer_shadow_color(layer: &CALayer, color: Option<Rgba>) {
        let cgcolor = match color {
            Some(color) => cgcolor(color),
            None => nil,
        };
        let _: () = msg_send![layer.id(), setShadowColor: cgcolor];
    }

    unsafe fn cgcolor(color: Rgba) -> id {
        let ns_color = NSColor::colorWithCalibratedRed_green_blue_alpha_(
            nil,
            color.red,
            color.green,
            color.blue,
            color.alpha,
        );
        msg_send![ns_color, CGColor]
    }

    unsafe fn nsstring(value: &str) -> id {
        let string: id = NSString::alloc(nil).init_str(value);
        let _: () = msg_send![string, autorelease];
        string
    }

    unsafe fn nsnumber(value: f64) -> id {
        let number: id = msg_send![class!(NSNumber), numberWithDouble: value];
        let _: () = msg_send![number, autorelease];
        number
    }
}
