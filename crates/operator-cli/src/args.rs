#![cfg_attr(test, allow(dead_code))]

use clap::{Args, Parser, Subcommand, ValueEnum};
use operator_core::{Locator, MouseButton, Point, SnapshotId, Surface, SurfaceKind, WindowId};
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
    SnapshotGet(SnapshotGetArgs),
    GetFocus(CommonArgs),
    ListApps(CommonArgs),
    ListWindows(ListWindowsArgs),
    PermissionsStatus(CommonArgs),
    Capabilities(CommonArgs),
    Click(ClickArgs),
    Drag(DragArgs),
    Scroll(ScrollArgs),
    Type(TypeArgs),
    LaunchApp(LaunchAppArgs),
    FocusWindow(FocusWindowArgs),
}

impl Command {
    fn common(&self) -> &CommonArgs {
        match self {
            Self::Observe(args) => &args.common,
            Self::SnapshotGet(args) => &args.common,
            Self::GetFocus(args) => args,
            Self::ListApps(args) => args,
            Self::ListWindows(args) => &args.common,
            Self::PermissionsStatus(args) => args,
            Self::Capabilities(args) => args,
            Self::Click(args) => &args.common,
            Self::Drag(args) => &args.common,
            Self::Scroll(args) => &args.common,
            Self::Type(args) => &args.common,
            Self::LaunchApp(args) => &args.common,
            Self::FocusWindow(args) => &args.common,
        }
    }

    fn into_invocation(self) -> Result<ToolInvocation, String> {
        match self {
            Self::Observe(args) => args.into_invocation(),
            Self::SnapshotGet(args) => args.into_invocation(),
            Self::GetFocus(common) => invoke_without_specific_input("get-focus", common),
            Self::ListApps(common) => invoke_without_specific_input("list-apps", common),
            Self::ListWindows(args) => args.into_invocation(),
            Self::PermissionsStatus(common) => {
                invoke_without_specific_input("permissions-status", common)
            }
            Self::Capabilities(common) => invoke_without_specific_input("capabilities", common),
            Self::Click(args) => args.into_invocation(),
            Self::Drag(args) => args.into_invocation(),
            Self::Scroll(args) => args.into_invocation(),
            Self::Type(args) => args.into_invocation(),
            Self::LaunchApp(args) => args.into_invocation(),
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
    #[arg(long, value_enum, default_value_t = ButtonArg::Left)]
    button: ButtonArg,
    #[command(flatten)]
    locator: ClickLocatorArgs,
}

impl ClickArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "button", self.button.mouse_button())?;
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
struct DragArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    from: DragFromLocatorArgs,
    #[command(flatten)]
    to: DragToLocatorArgs,
}

impl DragArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        Ok(ToolInvocation {
            tool: "drag",
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
}

impl ScrollArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        input.insert("delta_x".into(), Value::from(self.delta_x));
        input.insert("delta_y".into(), Value::from(self.delta_y));
        Ok(ToolInvocation {
            tool: "scroll",
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
    #[command(flatten)]
    locator: TypeLocatorArgs,
}

impl TypeArgs {
    fn into_invocation(self) -> Result<ToolInvocation, String> {
        let mut input = common_input(&self.common);
        input.insert("text".into(), Value::String(self.text));
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
enum ButtonArg {
    Left,
    Right,
    Middle,
}

impl ButtonArg {
    fn mouse_button(self) -> MouseButton {
        match self {
            Self::Left => MouseButton::Left,
            Self::Right => MouseButton::Right,
            Self::Middle => MouseButton::Middle,
        }
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
struct TypeLocatorArgs {
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    element: Option<String>,
    #[arg(long = "locator-text")]
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

impl TypeLocatorArgs {
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

fn reject_if_some<T>(value: Option<T>, message: &str) -> Result<(), String> {
    if value.is_some() {
        return Err(message.to_string());
    }
    Ok(())
}
