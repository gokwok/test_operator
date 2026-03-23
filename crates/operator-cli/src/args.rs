#![cfg_attr(test, allow(dead_code))]

use std::num::NonZeroU32;

use clap::{Args, Parser, Subcommand, ValueEnum};
use operator_core::{
    ActionFocusPolicy, ActionTargetSelector, ArtifactId, ClickMode, Locator, Point, SnapshotId,
    Surface, SurfaceKind, TypeTrailingKey, WindowId,
};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Parser)]
#[command(
    name = "operator",
    about = "Thin CLI wrapper around the Operator tool registry."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(crate) fn prefers_json(&self) -> bool {
        self.command.common().json_output
    }

    pub(crate) fn into_invocation(self) -> Result<ToolInvocation, String> {
        self.command.into_invocation()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolInvocation {
    pub(crate) tool: &'static str,
    pub(crate) input: Value,
    pub(crate) json_output: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    Observe(ObserveArgs),
    ArtifactGet(ArtifactGetArgs),
    SnapshotGet(SnapshotGetArgs),
    GetFocus(CommonArgs),
    ListApps(CommonArgs),
    ListWindows(ListWindowsArgs),
    PermissionsStatus(CommonArgs),
    Capabilities(CommonArgs),
    Click(ClickArgs),
    Move(MoveArgs),
    Drag(DragArgs),
    Swipe(SwipeArgs),
    Scroll(ScrollArgs),
    Hotkey(HotkeyArgs),
    Press(PressArgs),
    Type(TypeArgs),
    LaunchApp(LaunchAppArgs),
    SwitchApp(LifecycleActionArgs),
    QuitApp(LifecycleActionArgs),
    RelaunchApp(LifecycleActionArgs),
    HideApp(LifecycleActionArgs),
    UnhideApp(LifecycleActionArgs),
    FocusWindow(FocusWindowArgs),
}

impl Command {
    fn common(&self) -> &CommonArgs {
        match self {
            Self::Observe(args) => &args.common,
            Self::ArtifactGet(args) => &args.common,
            Self::SnapshotGet(args) => &args.common,
            Self::GetFocus(args) => args,
            Self::ListApps(args) => args,
            Self::ListWindows(args) => &args.common,
            Self::PermissionsStatus(args) => args,
            Self::Capabilities(args) => args,
            Self::Click(args) => &args.common,
            Self::Move(args) => &args.common,
            Self::Drag(args) => &args.common,
            Self::Swipe(args) => &args.common,
            Self::Scroll(args) => &args.common,
            Self::Hotkey(args) => &args.common,
            Self::Press(args) => &args.common,
            Self::Type(args) => &args.common,
            Self::LaunchApp(args) => &args.common,
            Self::SwitchApp(args) => &args.common,
            Self::QuitApp(args) => &args.common,
            Self::RelaunchApp(args) => &args.common,
            Self::HideApp(args) => &args.common,
            Self::UnhideApp(args) => &args.common,
            Self::FocusWindow(args) => &args.common,
        }
    }

    fn into_invocation(self) -> Result<ToolInvocation, String> {
        match self {
            Self::Observe(args) => args.into_invocation(),
            Self::ArtifactGet(args) => args.into_invocation(),
            Self::SnapshotGet(args) => args.into_invocation(),
            Self::GetFocus(common) => invoke_without_specific_input("get-focus", common),
            Self::ListApps(common) => invoke_without_specific_input("list-apps", common),
            Self::ListWindows(args) => args.into_invocation(),
            Self::PermissionsStatus(common) => {
                invoke_without_specific_input("permissions-status", common)
            }
            Self::Capabilities(common) => invoke_without_specific_input("capabilities", common),
            Self::Click(args) => args.into_invocation(),
            Self::Move(args) => args.into_invocation(),
            Self::Drag(args) => args.into_invocation(),
            Self::Swipe(args) => args.into_invocation(),
            Self::Scroll(args) => args.into_invocation(),
            Self::Hotkey(args) => args.into_invocation(),
            Self::Press(args) => args.into_invocation(),
            Self::Type(args) => args.into_invocation(),
            Self::LaunchApp(args) => args.into_invocation(),
            Self::SwitchApp(args) => args.into_invocation("switch-app"),
            Self::QuitApp(args) => args.into_invocation("quit-app"),
            Self::RelaunchApp(args) => args.into_invocation("relaunch-app"),
            Self::HideApp(args) => args.into_invocation("hide-app"),
            Self::UnhideApp(args) => args.into_invocation("unhide-app"),
            Self::FocusWindow(args) => args.into_invocation(),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct CommonArgs {
    #[arg(long)]
    target: Option<String>,
    #[arg(long = "json")]
    json_output: bool,
    #[arg(long)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Args)]
struct ObserveArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, value_enum, default_value_t = ObserveSurface::Frontmost)]
    surface: ObserveSurface,
    #[arg(long)]
    display_id: Option<u32>,
    #[arg(long)]
    window_id: Option<u64>,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
    #[arg(long)]
    width: Option<f64>,
    #[arg(long)]
    height: Option<f64>,
    #[arg(long)]
    include_screenshot: bool,
    #[arg(long)]
    include_elements: bool,
}

impl ObserveArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "surface", self.surface()?)?;
        input.insert(
            "include_screenshot".into(),
            Value::Bool(self.include_screenshot),
        );
        input.insert(
            "include_elements".into(),
            Value::Bool(self.include_elements),
        );
        Ok(ToolInvocation {
            tool: "observe",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }

    fn surface(&self) -> Result<Surface, String> {
        match self.surface {
            ObserveSurface::Frontmost => {
                reject_if_some(
                    self.display_id,
                    "--display-id is only valid with --surface fullscreen",
                )?;
                reject_if_some(
                    self.window_id,
                    "--window-id is only valid with --surface window",
                )?;
                reject_if_some(self.x, "--x is only valid with --surface region")?;
                reject_if_some(self.y, "--y is only valid with --surface region")?;
                reject_if_some(self.width, "--width is only valid with --surface region")?;
                reject_if_some(self.height, "--height is only valid with --surface region")?;
                Ok(Surface {
                    kind: SurfaceKind::Frontmost,
                })
            }
            ObserveSurface::Fullscreen => {
                reject_if_some(
                    self.window_id,
                    "--window-id is only valid with --surface window",
                )?;
                reject_if_some(self.x, "--x is only valid with --surface region")?;
                reject_if_some(self.y, "--y is only valid with --surface region")?;
                reject_if_some(self.width, "--width is only valid with --surface region")?;
                reject_if_some(self.height, "--height is only valid with --surface region")?;
                Ok(Surface {
                    kind: SurfaceKind::Fullscreen {
                        display_id: self.display_id,
                    },
                })
            }
            ObserveSurface::Window => {
                reject_if_some(
                    self.display_id,
                    "--display-id is only valid with --surface fullscreen",
                )?;
                reject_if_some(self.x, "--x is only valid with --surface region")?;
                reject_if_some(self.y, "--y is only valid with --surface region")?;
                reject_if_some(self.width, "--width is only valid with --surface region")?;
                reject_if_some(self.height, "--height is only valid with --surface region")?;
                let window_id = self
                    .window_id
                    .ok_or_else(|| "--window-id is required when --surface window".to_string())?;
                Ok(Surface {
                    kind: SurfaceKind::Window {
                        id: WindowId::from(window_id),
                    },
                })
            }
            ObserveSurface::Region => {
                reject_if_some(
                    self.display_id,
                    "--display-id is only valid with --surface fullscreen",
                )?;
                reject_if_some(
                    self.window_id,
                    "--window-id is only valid with --surface window",
                )?;
                let x = self
                    .x
                    .ok_or_else(|| "--x is required when --surface region".to_string())?;
                let y = self
                    .y
                    .ok_or_else(|| "--y is required when --surface region".to_string())?;
                let width = self
                    .width
                    .ok_or_else(|| "--width is required when --surface region".to_string())?;
                let height = self
                    .height
                    .ok_or_else(|| "--height is required when --surface region".to_string())?;
                Ok(Surface {
                    kind: SurfaceKind::Region {
                        rect: operator_core::Rect {
                            x,
                            y,
                            width,
                            height,
                        },
                    },
                })
            }
        }
    }
}

#[derive(Debug, Clone, Args)]
struct ArtifactGetArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    artifact_id: String,
}

impl ArtifactGetArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(
            &mut input,
            "artifact_id",
            ArtifactId::from(self.artifact_id),
        )?;
        Ok(ToolInvocation {
            tool: "artifact-get",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct SnapshotGetArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    snapshot_id: String,
}

impl SnapshotGetArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(
            &mut input,
            "snapshot_id",
            SnapshotId::from(self.snapshot_id),
        )?;
        Ok(ToolInvocation {
            tool: "snapshot-get",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct ListWindowsArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    app: Option<String>,
}

impl ListWindowsArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        if let Some(app) = self.app {
            input.insert("app".into(), Value::String(app));
        }
        Ok(ToolInvocation {
            tool: "list-windows",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct ClickArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, value_enum, default_value_t = ClickModeArg::Left)]
    mode: ClickModeArg,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    locator: ClickLocatorArgs,
}

impl ClickArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "mode", self.mode.click_mode())?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "click",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct MoveArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    locator: MoveLocatorArgs,
}

impl MoveArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let locator = self.locator.into_locator()?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        if locator.is_none() && target_selector.is_none() {
            return Err("move requires a locator, coordinates, or target selector".to_string());
        }
        if let Some(locator) = locator {
            insert_serialized(&mut input, "locator", locator)?;
        }
        insert_action_target(&mut input, target_selector, focus_policy)?;
        Ok(ToolInvocation {
            tool: "move",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct DragArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    from: DragFromLocatorArgs,
    #[command(flatten)]
    to: DragToLocatorArgs,
    #[arg(long)]
    duration_ms: Option<u64>,
    #[arg(long)]
    steps: Option<u32>,
    #[arg(long = "modifier", value_enum)]
    modifiers: Vec<DragModifierArg>,
}

impl DragArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        if let Some(duration_ms) = self.duration_ms {
            input.insert("duration_ms".into(), Value::from(duration_ms));
        }
        if let Some(steps) = self.steps {
            input.insert("steps".into(), Value::from(steps));
        }
        if !self.modifiers.is_empty() {
            insert_serialized(&mut input, "modifiers", self.modifiers)?;
        }
        Ok(ToolInvocation {
            tool: "drag",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct SwipeArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    from: DragFromLocatorArgs,
    #[command(flatten)]
    to: DragToLocatorArgs,
    #[arg(long)]
    duration_ms: Option<u64>,
    #[arg(long)]
    steps: Option<u32>,
}

impl SwipeArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        if let Some(duration_ms) = self.duration_ms {
            input.insert("duration_ms".into(), Value::from(duration_ms));
        }
        if let Some(steps) = self.steps {
            input.insert("steps".into(), Value::from(steps));
        }
        Ok(ToolInvocation {
            tool: "swipe",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct ScrollArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, allow_hyphen_values = true)]
    delta_x: f64,
    #[arg(long, allow_hyphen_values = true)]
    delta_y: f64,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    locator: ScrollLocatorArgs,
}

impl ScrollArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("delta_x".into(), Value::from(self.delta_x));
        input.insert("delta_y".into(), Value::from(self.delta_y));
        insert_action_target(&mut input, target_selector, focus_policy)?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "scroll",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct HotkeyArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long = "key", required = true)]
    keys: Vec<String>,
    #[command(flatten)]
    action_target: ActionTargetArgs,
}

impl HotkeyArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "keys", self.keys)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        Ok(ToolInvocation {
            tool: "hotkey",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct PressArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    key: String,
    #[arg(long, default_value = "1")]
    count: NonZeroU32,
    #[command(flatten)]
    action_target: ActionTargetArgs,
}

impl PressArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("key".into(), Value::String(self.key));
        insert_serialized(&mut input, "count", self.count)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        Ok(ToolInvocation {
            tool: "press",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct TypeArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    text: String,
    #[arg(long)]
    clear_before: bool,
    #[arg(long)]
    delay_ms: Option<u64>,
    #[arg(long = "trailing-key", value_enum)]
    trailing_keys: Vec<TypeTrailingKeyArg>,
    #[command(flatten)]
    action_target: ActionTargetArgs,
    #[command(flatten)]
    locator: TypeLocatorArgs,
}

impl TypeArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("text".into(), Value::String(self.text));
        input.insert("clear_before".into(), Value::Bool(self.clear_before));
        if let Some(delay_ms) = self.delay_ms {
            input.insert("delay_ms".into(), Value::from(delay_ms));
        }
        if !self.trailing_keys.is_empty() {
            let trailing_keys = self
                .trailing_keys
                .into_iter()
                .map(TypeTrailingKeyArg::trailing_key)
                .collect::<Vec<_>>();
            insert_serialized(&mut input, "trailing_keys", trailing_keys)?;
        }
        insert_action_target(&mut input, target_selector, focus_policy)?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "type",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct LaunchAppArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    bundle_id_or_name: String,
}

impl LaunchAppArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        input.insert(
            "bundle_id_or_name".into(),
            Value::String(self.bundle_id_or_name),
        );
        Ok(ToolInvocation {
            tool: "launch-app",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct LifecycleActionArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: LifecycleTargetArgs,
}

impl LifecycleActionArgs {
    fn into_invocation(self, tool: &'static str) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "target_selector", self.target.into_selector()?)?;
        Ok(ToolInvocation {
            tool,
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct FocusWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    window_id: u64,
}

impl FocusWindowArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "window_id", WindowId::from(self.window_id))?;
        Ok(ToolInvocation {
            tool: "focus-window",
            input: Value::Object(input),
            json_output: self.common.json_output,
        })
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ObserveSurface {
    Frontmost,
    Fullscreen,
    Window,
    Region,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ClickModeArg {
    Left,
    Right,
    Middle,
    Double,
}

impl ClickModeArg {
    fn click_mode(self) -> ClickMode {
        match self {
            Self::Left => ClickMode::Left,
            Self::Right => ClickMode::Right,
            Self::Middle => ClickMode::Middle,
            Self::Double => ClickMode::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
enum DragModifierArg {
    Command,
    Control,
    Option,
    Shift,
    Function,
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
enum TypeTrailingKeyArg {
    Return,
    Tab,
    Escape,
    Delete,
}

impl TypeTrailingKeyArg {
    fn trailing_key(self) -> TypeTrailingKey {
        match self {
            Self::Return => TypeTrailingKey::Return,
            Self::Tab => TypeTrailingKey::Tab,
            Self::Escape => TypeTrailingKey::Escape,
            Self::Delete => TypeTrailingKey::Delete,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FocusPolicyArg {
    Auto,
    Never,
}

impl FocusPolicyArg {
    fn focus_policy(self) -> ActionFocusPolicy {
        match self {
            Self::Auto => ActionFocusPolicy::Auto,
            Self::Never => ActionFocusPolicy::Never,
        }
    }
}

impl Default for FocusPolicyArg {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Args, Default)]
struct TargetSelectorArgs {
    #[arg(long)]
    app: Option<String>,
    #[arg(long)]
    pid: Option<u32>,
    #[arg(long = "window-id")]
    window_id: Option<u64>,
    #[arg(long = "window-title")]
    window_title: Option<String>,
    #[arg(long = "window-index")]
    window_index: Option<usize>,
}

impl TargetSelectorArgs {
    fn into_optional_selector(self) -> Result<Option<ActionTargetSelector>, String> {
        let selector_count = [
            self.app.is_some(),
            self.pid.is_some(),
            self.window_id.is_some(),
            self.window_title.is_some(),
            self.window_index.is_some(),
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();

        if selector_count > 1 {
            return Err("target selector flags are mutually exclusive".into());
        }

        let selector = if let Some(app) = self.app {
            Some(ActionTargetSelector::App(app))
        } else if let Some(pid) = self.pid {
            Some(ActionTargetSelector::Pid(pid))
        } else if let Some(window_id) = self.window_id {
            Some(ActionTargetSelector::WindowId(WindowId::from(window_id)))
        } else if let Some(window_title) = self.window_title {
            Some(ActionTargetSelector::WindowTitle(window_title))
        } else {
            self.window_index.map(ActionTargetSelector::WindowIndex)
        };

        Ok(selector)
    }

    fn into_required_selector(self) -> Result<ActionTargetSelector, String> {
        self.into_optional_selector()?
            .ok_or_else(|| "a target selector flag is required".to_string())
    }
}

#[derive(Debug, Clone, Args, Default)]
struct ActionTargetArgs {
    #[command(flatten)]
    selector: TargetSelectorArgs,
    #[arg(long, value_enum, default_value_t = FocusPolicyArg::Auto)]
    focus_policy: FocusPolicyArg,
}

impl ActionTargetArgs {
    fn into_parts(self) -> Result<(Option<ActionTargetSelector>, ActionFocusPolicy), String> {
        Ok((
            self.selector.into_optional_selector()?,
            self.focus_policy.focus_policy(),
        ))
    }
}

#[derive(Debug, Clone, Args, Default)]
struct LifecycleTargetArgs {
    #[command(flatten)]
    selector: TargetSelectorArgs,
}

impl LifecycleTargetArgs {
    fn into_selector(self) -> Result<ActionTargetSelector, String> {
        self.selector.into_required_selector()
    }
}

#[derive(Debug, Clone, Args, Default)]
struct DragFromLocatorArgs {
    #[arg(long = "from-snapshot")]
    from_snapshot: Option<String>,
    #[arg(long = "from-element")]
    from_element: Option<String>,
    #[arg(long = "from-text")]
    from_text: Option<String>,
    #[arg(long = "from-role")]
    from_role: Option<String>,
    #[arg(long = "from-index", default_value_t = 0)]
    from_index: usize,
    #[arg(long = "from-x", allow_hyphen_values = true)]
    from_x: Option<f64>,
    #[arg(long = "from-y", allow_hyphen_values = true)]
    from_y: Option<f64>,
}

impl DragFromLocatorArgs {
    fn into_locator(self) -> Result<Locator, String> {
        RawLocatorArgs {
            snapshot: self.from_snapshot,
            element: self.from_element,
            text: self.from_text,
            role: self.from_role,
            index: self.from_index,
            x: self.from_x,
            y: self.from_y,
        }
        .into_required_locator("from")
    }
}

#[derive(Debug, Clone, Args, Default)]
struct DragToLocatorArgs {
    #[arg(long = "to-snapshot")]
    to_snapshot: Option<String>,
    #[arg(long = "to-element")]
    to_element: Option<String>,
    #[arg(long = "to-text")]
    to_text: Option<String>,
    #[arg(long = "to-role")]
    to_role: Option<String>,
    #[arg(long = "to-index", default_value_t = 0)]
    to_index: usize,
    #[arg(long = "to-x", allow_hyphen_values = true)]
    to_x: Option<f64>,
    #[arg(long = "to-y", allow_hyphen_values = true)]
    to_y: Option<f64>,
}

impl DragToLocatorArgs {
    fn into_locator(self) -> Result<Locator, String> {
        RawLocatorArgs {
            snapshot: self.to_snapshot,
            element: self.to_element,
            text: self.to_text,
            role: self.to_role,
            index: self.to_index,
            x: self.to_x,
            y: self.to_y,
        }
        .into_required_locator("to")
    }
}

#[derive(Debug, Clone, Args, Default)]
struct ClickLocatorArgs {
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    element: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long, default_value_t = 0)]
    index: usize,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
}

impl ClickLocatorArgs {
    fn into_locator(self) -> Result<Option<Locator>, String> {
        RawLocatorArgs {
            snapshot: self.snapshot,
            element: self.element,
            text: self.text,
            role: self.role,
            index: self.index,
            x: self.x,
            y: self.y,
        }
        .into_locator()
    }
}

#[derive(Debug, Clone, Args, Default)]
struct MoveLocatorArgs {
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    element: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long, default_value_t = 0)]
    index: usize,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
}

impl MoveLocatorArgs {
    fn into_locator(self) -> Result<Option<Locator>, String> {
        RawLocatorArgs {
            snapshot: self.snapshot,
            element: self.element,
            text: self.text,
            role: self.role,
            index: self.index,
            x: self.x,
            y: self.y,
        }
        .into_locator()
    }
}

#[derive(Debug, Clone, Args, Default)]
struct ScrollLocatorArgs {
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    element: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long, default_value_t = 0)]
    index: usize,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
}

impl ScrollLocatorArgs {
    fn into_locator(self) -> Result<Option<Locator>, String> {
        RawLocatorArgs {
            snapshot: self.snapshot,
            element: self.element,
            text: self.text,
            role: self.role,
            index: self.index,
            x: self.x,
            y: self.y,
        }
        .into_locator()
    }
}

#[derive(Debug, Clone, Args, Default)]
struct TypeLocatorArgs {
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    element: Option<String>,
    #[arg(long = "locator-text")]
    locator_text: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long, default_value_t = 0)]
    index: usize,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
}

impl TypeLocatorArgs {
    fn into_locator(self) -> Result<Option<Locator>, String> {
        RawLocatorArgs {
            snapshot: self.snapshot,
            element: self.element,
            text: self.locator_text,
            role: self.role,
            index: self.index,
            x: self.x,
            y: self.y,
        }
        .into_locator()
    }
}

#[derive(Debug, Clone, Default)]
struct RawLocatorArgs {
    snapshot: Option<String>,
    element: Option<String>,
    text: Option<String>,
    role: Option<String>,
    index: usize,
    x: Option<f64>,
    y: Option<f64>,
}

impl RawLocatorArgs {
    fn into_required_locator(self, name: &str) -> Result<Locator, String> {
        self.into_locator()?
            .ok_or_else(|| format!("--{name}-* locator is required"))
    }

    fn into_locator(self) -> Result<Option<Locator>, String> {
        let snapshot_variant = self.snapshot.is_some() || self.element.is_some();
        let text_variant = self.text.is_some();
        let role_variant = self.role.is_some();
        let coords_variant = self.x.is_some() || self.y.is_some();
        let selected = [snapshot_variant, text_variant, role_variant, coords_variant]
            .into_iter()
            .filter(|flag| *flag)
            .count();

        if selected == 0 {
            return Ok(None);
        }

        if selected > 1 {
            return Err("locator flags are mutually exclusive".into());
        }

        if snapshot_variant {
            let snapshot = self
                .snapshot
                .ok_or_else(|| "--snapshot is required when --element is provided".to_string())?;
            let element = self
                .element
                .ok_or_else(|| "--element is required when --snapshot is provided".to_string())?;
            return Ok(Some(Locator::SnapshotElement {
                snapshot: SnapshotId::from(snapshot),
                element: element.into(),
            }));
        }

        if let Some(text) = self.text {
            return Ok(Some(Locator::Text(text)));
        }

        if let Some(role) = self.role {
            return Ok(Some(Locator::Role {
                role,
                index: self.index,
            }));
        }

        let x = self
            .x
            .ok_or_else(|| "--x is required when using coordinate locators".to_string())?;
        let y = self
            .y
            .ok_or_else(|| "--y is required when using coordinate locators".to_string())?;
        Ok(Some(Locator::Coords(Point { x, y })))
    }
}

fn invoke_without_specific_input(
    tool: &'static str,
    common: CommonArgs,
) -> Result<ToolInvocation, String> {
    Ok(ToolInvocation {
        tool,
        input: Value::Object(common_input(&common)),
        json_output: common.json_output,
    })
}

fn common_input(common: &CommonArgs) -> Map<String, Value> {
    let mut input = Map::new();
    if let Some(target) = &common.target {
        input.insert("target".into(), Value::String(target.clone()));
    }
    if let Some(timeout_ms) = common.timeout_ms {
        input.insert("timeout_ms".into(), Value::Number(timeout_ms.into()));
    }
    input
}

fn insert_serialized<T: Serialize>(
    map: &mut Map<String, Value>,
    key: &str,
    value: T,
) -> Result<(), String> {
    let value = serde_json::to_value(value)
        .map_err(|error| format!("failed to serialize {key}: {error}"))?;
    map.insert(key.to_string(), value);
    Ok(())
}

fn insert_action_target(
    map: &mut Map<String, Value>,
    target_selector: Option<ActionTargetSelector>,
    focus_policy: ActionFocusPolicy,
) -> Result<(), String> {
    if let Some(target_selector) = target_selector {
        insert_serialized(map, "target_selector", target_selector)?;
        insert_serialized(map, "focus_policy", focus_policy)?;
    } else if !matches!(focus_policy, ActionFocusPolicy::Auto) {
        insert_serialized(map, "focus_policy", focus_policy)?;
    }

    Ok(())
}

fn reject_if_some<T>(value: Option<T>, message: &str) -> Result<(), String> {
    if value.is_some() {
        return Err(message.to_string());
    }
    Ok(())
}
