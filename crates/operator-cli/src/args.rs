#![cfg_attr(test, allow(dead_code))]

use std::{ffi::OsString, num::NonZeroU32};

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use operator_core::{
    ActionFocusPolicy, ActionTargetSelector, ActionVerification, ArtifactId, ClickMode, Locator,
    Point, SnapshotId, Surface, SurfaceKind, TypeTrailingKey, WindowId,
};
use serde::Serialize;
use serde_json::{Map, Value};

const ROOT_HELP: &str = "Operator automation CLI

Usage: operator [OPTIONS] [COMMAND]

Core:
  permissions   Inspect platform permission state
  capabilities  List runtime capabilities

Observe:
  observe       Capture UI state
  snapshot      Work with persisted snapshots
  artifact      Work with persisted artifacts

Query:
  list          Enumerate apps and windows
  focus         Inspect current focus

Action:
  input         Pointer and keyboard actions
  app           Application lifecycle actions
  window        Window management actions

MCP:
  mcp           MCP server commands

A2A:
  reserved      Reserved for future A2A commands

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const OBSERVE_HELP: &str = "Capture UI state

Usage: operator observe [OPTIONS] <COMMAND>

Commands:
  frontmost   Capture the frontmost surface
  window      Capture a specific window
  region      Capture a specific screen region
  fullscreen  Capture the full display or the active display
  help        Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const SNAPSHOT_HELP: &str = "Work with persisted snapshots

Usage: operator snapshot [OPTIONS] <COMMAND>

Commands:
  get   Load a persisted snapshot
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const ARTIFACT_HELP: &str = "Work with persisted artifacts

Usage: operator artifact [OPTIONS] <COMMAND>

Commands:
  get   Resolve a persisted artifact
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const INPUT_HELP: &str = "Pointer and keyboard actions

Usage: operator input [OPTIONS] <COMMAND>

Commands:
  click   Click at a locator or target
  move    Move the pointer to a locator, coordinates, or target
  type    Type text into the focused or resolved target
  press   Press a special key
  hotkey  Press a key chord
  scroll  Scroll by delta against a locator or target
  drag    Drag between two locators
  swipe   Swipe between two locators
  help    Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const APP_HELP: &str = "Application lifecycle actions

Usage: operator app [OPTIONS] <COMMAND>

Commands:
  launch    Launch an application by bundle identifier or name
  switch    Bring an application to the foreground
  quit      Quit an application
  relaunch  Relaunch an application
  hide      Hide an application
  unhide    Unhide an application
  help      Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const WINDOW_HELP: &str = "Window management actions

Usage: operator window [OPTIONS] <COMMAND>

Commands:
  focus       Focus a specific window
  close       Close a specific window
  minimize    Minimize a specific window
  maximize    Maximize a specific window
  move        Move a specific window
  resize      Resize a specific window
  set-bounds  Set the full bounds of a specific window
  help        Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

const MCP_HELP: &str = "MCP server commands

Usage: operator mcp [OPTIONS] <COMMAND>

Commands:
  serve  Run the MCP stdio server
  help   Print this message or the help of the given subcommand(s)

Options:
      --json                   Render structured JSON output
      --target <TARGET>        Select a runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override runtime timeout in milliseconds
  -h, --help                   Print help
";

#[derive(Debug, Parser)]
#[command(name = "operator", about = "Operator automation CLI")]
pub(crate) struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(crate) fn command() -> clap::Command {
        <Self as CommandFactory>::command().override_help(ROOT_HELP)
    }

    pub(crate) fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let mut command = Self::command();
        let mut matches = command.try_get_matches_from_mut(itr)?;
        <Self as FromArgMatches>::from_arg_matches_mut(&mut matches)
    }

    pub(crate) fn parse() -> Self {
        Self::try_parse_from(std::env::args_os()).unwrap_or_else(|error| error.exit())
    }

    pub(crate) fn prefers_json(&self) -> bool {
        self.common.json_output
            || self
                .command
                .common()
                .map(|common| common.json_output)
                .unwrap_or(false)
    }

    pub(crate) fn into_execution(self) -> Result<CliExecution, String> {
        self.command.into_execution(self.common)
    }

    #[cfg(test)]
    pub(crate) fn into_invocation(self) -> Result<ToolInvocation, String> {
        match self.into_execution()? {
            CliExecution::Tool(invocation) => Ok(invocation),
            CliExecution::McpServe => {
                Err("mcp serve does not map to a runtime tool invocation".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolInvocation {
    pub(crate) tool: &'static str,
    pub(crate) input: Value,
    pub(crate) json_output: bool,
}

#[derive(Debug)]
pub(crate) enum CliExecution {
    Tool(ToolInvocation),
    McpServe,
}

#[derive(Debug, Subcommand)]
enum Command {
    Permissions(CommonArgs),
    Capabilities(CommonArgs),
    Observe(ObserveArgs),
    Snapshot(SnapshotArgs),
    Artifact(ArtifactArgs),
    List(ListArgs),
    Focus(CommonArgs),
    Input(InputArgs),
    App(AppArgs),
    Window(WindowArgs),
    Mcp(McpArgs),
    #[command(hide = true, name = "artifact-get")]
    ArtifactGet(LegacyArtifactGetArgs),
    #[command(hide = true, name = "snapshot-get")]
    SnapshotGet(LegacySnapshotGetArgs),
    #[command(hide = true, name = "get-focus")]
    GetFocus(CommonArgs),
    #[command(hide = true, name = "list-apps")]
    ListApps(CommonArgs),
    #[command(hide = true, name = "list-windows")]
    ListWindows(ListWindowsArgs),
    #[command(hide = true, name = "permissions-status")]
    PermissionsStatus(CommonArgs),
    #[command(hide = true)]
    Click(ClickArgs),
    #[command(hide = true)]
    Move(MoveArgs),
    #[command(hide = true)]
    Drag(DragArgs),
    #[command(hide = true)]
    Swipe(SwipeArgs),
    #[command(hide = true)]
    Scroll(ScrollArgs),
    #[command(hide = true)]
    Hotkey(HotkeyArgs),
    #[command(hide = true)]
    Press(PressArgs),
    #[command(hide = true)]
    Type(TypeArgs),
    #[command(hide = true)]
    LaunchApp(LaunchAppArgs),
    #[command(hide = true)]
    CloseWindow(CloseWindowArgs),
    #[command(hide = true)]
    MinimizeWindow(MinimizeWindowArgs),
    #[command(hide = true)]
    MaximizeWindow(MaximizeWindowArgs),
    #[command(hide = true)]
    MoveWindow(MoveWindowArgs),
    #[command(hide = true)]
    ResizeWindow(ResizeWindowArgs),
    #[command(hide = true)]
    SetWindowBounds(SetWindowBoundsArgs),
    #[command(hide = true)]
    SwitchApp(LifecycleActionArgs),
    #[command(hide = true)]
    QuitApp(LifecycleActionArgs),
    #[command(hide = true)]
    RelaunchApp(LifecycleActionArgs),
    #[command(hide = true)]
    HideApp(LifecycleActionArgs),
    #[command(hide = true)]
    UnhideApp(LifecycleActionArgs),
    #[command(hide = true)]
    FocusWindow(FocusWindowArgs),
}

impl Command {
    fn common(&self) -> Option<&CommonArgs> {
        match self {
            Self::Permissions(args) => Some(args),
            Self::Capabilities(args) => Some(args),
            Self::Observe(args) => Some(&args.common),
            Self::Snapshot(args) => Some(&args.common),
            Self::Artifact(args) => Some(&args.common),
            Self::List(args) => args.common(),
            Self::Focus(args) => Some(args),
            Self::Input(args) => args.common(),
            Self::App(args) => Some(&args.common),
            Self::Window(args) => Some(&args.common),
            Self::Mcp(args) => Some(&args.common),
            Self::ArtifactGet(args) => Some(&args.common),
            Self::SnapshotGet(args) => Some(&args.common),
            Self::GetFocus(args) => Some(args),
            Self::ListApps(args) => Some(args),
            Self::ListWindows(args) => Some(&args.common),
            Self::PermissionsStatus(args) => Some(args),
            Self::Click(args) => Some(&args.common),
            Self::Move(args) => Some(&args.common),
            Self::Drag(args) => Some(&args.common),
            Self::Swipe(args) => Some(&args.common),
            Self::Scroll(args) => Some(&args.common),
            Self::Hotkey(args) => Some(&args.common),
            Self::Press(args) => Some(&args.common),
            Self::Type(args) => Some(&args.common),
            Self::LaunchApp(args) => Some(&args.common),
            Self::CloseWindow(args) => Some(&args.common),
            Self::MinimizeWindow(args) => Some(&args.common),
            Self::MaximizeWindow(args) => Some(&args.common),
            Self::MoveWindow(args) => Some(&args.common),
            Self::ResizeWindow(args) => Some(&args.common),
            Self::SetWindowBounds(args) => Some(&args.common),
            Self::SwitchApp(args) => Some(&args.common),
            Self::QuitApp(args) => Some(&args.common),
            Self::RelaunchApp(args) => Some(&args.common),
            Self::HideApp(args) => Some(&args.common),
            Self::UnhideApp(args) => Some(&args.common),
            Self::FocusWindow(args) => Some(&args.common),
        }
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Permissions(common) => invoke_without_specific_input(
                "permissions-status",
                merge_common(root_common, common),
            ),
            Self::Capabilities(common) => {
                invoke_without_specific_input("capabilities", merge_common(root_common, common))
            }
            Self::Observe(args) => args.into_invocation(root_common),
            Self::Snapshot(args) => args.into_invocation(root_common),
            Self::Artifact(args) => args.into_invocation(root_common),
            Self::List(args) => args.into_invocation(root_common),
            Self::Focus(common) => {
                invoke_without_specific_input("get-focus", merge_common(root_common, common))
            }
            Self::Input(args) => args.into_invocation(root_common),
            Self::App(args) => args.into_invocation(root_common),
            Self::Window(args) => args.into_invocation(root_common),
            Self::Mcp(args) => args.into_invocation(root_common),
            Self::ArtifactGet(args) => args.into_invocation(root_common),
            Self::SnapshotGet(args) => args.into_invocation(root_common),
            Self::GetFocus(common) => {
                invoke_without_specific_input("get-focus", merge_common(root_common, common))
            }
            Self::ListApps(common) => {
                invoke_without_specific_input("list-apps", merge_common(root_common, common))
            }
            Self::ListWindows(args) => args.into_invocation(root_common),
            Self::PermissionsStatus(common) => invoke_without_specific_input(
                "permissions-status",
                merge_common(root_common, common),
            ),
            Self::Click(args) => args.into_invocation(root_common),
            Self::Move(args) => args.into_invocation(root_common),
            Self::Drag(args) => args.into_invocation(root_common),
            Self::Swipe(args) => args.into_invocation(root_common),
            Self::Scroll(args) => args.into_invocation(root_common),
            Self::Hotkey(args) => args.into_invocation(root_common),
            Self::Press(args) => args.into_invocation(root_common),
            Self::Type(args) => args.into_invocation(root_common),
            Self::LaunchApp(args) => args.into_invocation(root_common),
            Self::CloseWindow(args) => args.into_invocation(root_common),
            Self::MinimizeWindow(args) => args.into_invocation(root_common),
            Self::MaximizeWindow(args) => args.into_invocation(root_common),
            Self::MoveWindow(args) => args.into_invocation(root_common),
            Self::ResizeWindow(args) => args.into_invocation(root_common),
            Self::SetWindowBounds(args) => args.into_invocation(root_common),
            Self::SwitchApp(args) => args.into_invocation("switch-app", root_common),
            Self::QuitApp(args) => args.into_invocation("quit-app", root_common),
            Self::RelaunchApp(args) => args.into_invocation("relaunch-app", root_common),
            Self::HideApp(args) => args.into_invocation("hide-app", root_common),
            Self::UnhideApp(args) => args.into_invocation("unhide-app", root_common),
            Self::FocusWindow(args) => args.into_invocation(root_common),
        }
    }

    fn into_execution(self, root_common: CommonArgs) -> Result<CliExecution, String> {
        match self {
            Self::Mcp(args) => args.into_execution(root_common),
            other => other.into_invocation(root_common).map(CliExecution::Tool),
        }
    }
}

#[derive(Debug, Clone, Default, Args)]
struct CommonArgs {
    #[arg(long, global = true, help = "Select a runtime target")]
    target: Option<String>,
    #[arg(long = "json", global = true, help = "Render structured JSON output")]
    json_output: bool,
    #[arg(long, global = true, help = "Override runtime timeout in milliseconds")]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Args)]
struct ListArgs {
    #[command(subcommand)]
    command: ListCommand,
}

impl ListArgs {
    fn common(&self) -> Option<&CommonArgs> {
        self.command.common()
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command.into_invocation(root_common)
    }
}

#[derive(Debug, Clone, Subcommand)]
enum ListCommand {
    Apps(CommonArgs),
    Windows(ListWindowsArgs),
}

impl ListCommand {
    fn common(&self) -> Option<&CommonArgs> {
        match self {
            Self::Apps(args) => Some(args),
            Self::Windows(args) => Some(&args.common),
        }
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Apps(common) => {
                invoke_without_specific_input("list-apps", merge_common(root_common, common))
            }
            Self::Windows(args) => args.into_invocation(root_common),
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(about = "Capture UI state", override_help = OBSERVE_HELP, arg_required_else_help = true)]
struct ObserveArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: ObserveCommand,
}

impl ObserveArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Subcommand)]
enum ObserveCommand {
    #[command(about = "Capture the frontmost surface")]
    Frontmost(ObserveFrontmostArgs),
    #[command(about = "Capture a specific window")]
    Window(ObserveWindowArgs),
    #[command(about = "Capture a specific screen region")]
    Region(ObserveRegionArgs),
    #[command(about = "Capture the full display or the active display")]
    Fullscreen(ObserveFullscreenArgs),
}

impl ObserveCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Frontmost(args) => {
                observe_invocation(common, SurfaceKind::Frontmost, args.capture.capture)
            }
            Self::Window(args) => observe_invocation(
                common,
                SurfaceKind::Window {
                    id: WindowId::from(args.window_id),
                },
                args.capture.capture,
            ),
            Self::Region(args) => observe_invocation(
                common,
                SurfaceKind::Region {
                    rect: operator_core::Rect {
                        x: args.x,
                        y: args.y,
                        width: args.width,
                        height: args.height,
                    },
                },
                args.capture.capture,
            ),
            Self::Fullscreen(args) => observe_invocation(
                common,
                SurfaceKind::Fullscreen {
                    display_id: args.display_id,
                },
                args.capture.capture,
            ),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct ObserveFrontmostArgs {
    #[command(flatten)]
    capture: CaptureArgs,
}

#[derive(Debug, Clone, Args)]
struct ObserveWindowArgs {
    #[arg(long)]
    window_id: u64,
    #[command(flatten)]
    capture: CaptureArgs,
}

#[derive(Debug, Clone, Args)]
struct ObserveRegionArgs {
    #[arg(long)]
    x: f64,
    #[arg(long)]
    y: f64,
    #[arg(long)]
    width: f64,
    #[arg(long)]
    height: f64,
    #[command(flatten)]
    capture: CaptureArgs,
}

#[derive(Debug, Clone, Args)]
struct ObserveFullscreenArgs {
    #[arg(long)]
    display_id: Option<u32>,
    #[command(flatten)]
    capture: CaptureArgs,
}

#[derive(Debug, Clone, Args)]
struct CaptureArgs {
    #[arg(long, value_enum, default_value_t = CaptureProfileArg::All)]
    capture: CaptureProfileArg,
}

fn observe_invocation(
    common: CommonArgs,
    surface_kind: SurfaceKind,
    capture: CaptureProfileArg,
) -> Result<ToolInvocation, String> {
    let mut input = common_input(&common);
    insert_serialized(&mut input, "surface", Surface { kind: surface_kind })?;
    let (include_screenshot, include_elements) = capture.flags();
    input.insert("include_screenshot".into(), Value::Bool(include_screenshot));
    input.insert("include_elements".into(), Value::Bool(include_elements));
    Ok(ToolInvocation {
        tool: "observe",
        input: Value::Object(input),
        json_output: common.json_output,
    })
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Work with persisted snapshots",
    override_help = SNAPSHOT_HELP,
    arg_required_else_help = true
)]
struct SnapshotArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: SnapshotCommand,
}

impl SnapshotArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Subcommand)]
enum SnapshotCommand {
    #[command(about = "Load a persisted snapshot")]
    Get(SnapshotGetArgs),
}

impl SnapshotCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Get(args) => args.into_invocation(common),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct SnapshotGetArgs {
    snapshot_id: String,
}

impl SnapshotGetArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        insert_serialized(
            &mut input,
            "snapshot_id",
            SnapshotId::from(self.snapshot_id),
        )?;
        Ok(ToolInvocation {
            tool: "snapshot-get",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Work with persisted artifacts",
    override_help = ARTIFACT_HELP,
    arg_required_else_help = true
)]
struct ArtifactArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: ArtifactCommand,
}

impl ArtifactArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Subcommand)]
enum ArtifactCommand {
    #[command(about = "Resolve a persisted artifact")]
    Get(ArtifactGetArgs),
}

impl ArtifactCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Get(args) => args.into_invocation(common),
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Pointer and keyboard actions",
    override_help = INPUT_HELP,
    arg_required_else_help = true
)]
struct InputArgs {
    #[command(subcommand)]
    command: InputCommand,
}

impl InputArgs {
    fn common(&self) -> Option<&CommonArgs> {
        self.command.common()
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command.into_invocation(root_common)
    }
}

#[derive(Debug, Clone, Subcommand)]
enum InputCommand {
    #[command(about = "Click at a locator or target")]
    Click(InputClickArgs),
    #[command(about = "Move the pointer to a locator, coordinates, or target")]
    Move(InputMoveArgs),
    #[command(about = "Type text into the focused or resolved target")]
    Type(InputTypeArgs),
    #[command(about = "Press a special key")]
    Press(InputPressArgs),
    #[command(about = "Press a key chord")]
    Hotkey(InputHotkeyArgs),
    #[command(about = "Scroll by delta against a locator or target")]
    Scroll(InputScrollArgs),
    #[command(about = "Drag between two locators")]
    Drag(InputDragArgs),
    #[command(about = "Swipe between two locators")]
    Swipe(InputSwipeArgs),
}

impl InputCommand {
    fn common(&self) -> Option<&CommonArgs> {
        match self {
            Self::Click(args) => Some(&args.common),
            Self::Move(args) => Some(&args.common),
            Self::Type(args) => Some(&args.common),
            Self::Press(args) => Some(&args.common),
            Self::Hotkey(args) => Some(&args.common),
            Self::Scroll(args) => Some(&args.common),
            Self::Drag(args) => Some(&args.common),
            Self::Swipe(args) => Some(&args.common),
        }
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Click(args) => args.into_invocation(root_common),
            Self::Move(args) => args.into_invocation(root_common),
            Self::Type(args) => args.into_invocation(root_common),
            Self::Press(args) => args.into_invocation(root_common),
            Self::Hotkey(args) => args.into_invocation(root_common),
            Self::Scroll(args) => args.into_invocation(root_common),
            Self::Drag(args) => args.into_invocation(root_common),
            Self::Swipe(args) => args.into_invocation(root_common),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct InputClickArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, value_enum, default_value_t = ClickModeArg::Left)]
    mode: ClickModeArg,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: InputLocatorArgs,
}

impl InputClickArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        insert_serialized(&mut input, "mode", self.mode.click_mode())?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "click",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputMoveArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: InputLocatorArgs,
}

impl InputMoveArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let locator = self.locator.into_locator()?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        if locator.is_none() && target_selector.is_none() {
            return Err("move requires a locator, coordinates, or target selector".to_string());
        }
        if let Some(locator) = locator {
            insert_serialized(&mut input, "locator", locator)?;
        }
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "move",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputTypeArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(value_name = "TEXT")]
    payload: String,
    #[arg(long)]
    clear_before: bool,
    #[arg(long)]
    delay_ms: Option<u64>,
    #[arg(long = "after-key", value_enum)]
    after_keys: Vec<TypeTrailingKeyArg>,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: InputLocatorArgs,
}

impl InputTypeArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("text".into(), Value::String(self.payload));
        input.insert("clear_before".into(), Value::Bool(self.clear_before));
        if let Some(delay_ms) = self.delay_ms {
            input.insert("delay_ms".into(), Value::from(delay_ms));
        }
        if !self.after_keys.is_empty() {
            let trailing_keys = self
                .after_keys
                .into_iter()
                .map(TypeTrailingKeyArg::trailing_key)
                .collect::<Vec<_>>();
            insert_serialized(&mut input, "trailing_keys", trailing_keys)?;
        }
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "type",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputPressArgs {
    #[command(flatten)]
    common: CommonArgs,
    key: String,
    #[arg(long, default_value = "1")]
    count: NonZeroU32,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl InputPressArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("key".into(), Value::String(self.key));
        insert_serialized(&mut input, "count", self.count)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "press",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputHotkeyArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(required = true)]
    keys: Vec<String>,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl InputHotkeyArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "keys", self.keys)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "hotkey",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputScrollArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, allow_hyphen_values = true)]
    delta_x: f64,
    #[arg(long, allow_hyphen_values = true)]
    delta_y: f64,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: InputLocatorArgs,
}

impl InputScrollArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("delta_x".into(), Value::from(self.delta_x));
        input.insert("delta_y".into(), Value::from(self.delta_y));
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "scroll",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputDragArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
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

impl InputDragArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
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
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct InputSwipeArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    action_target: InputActionTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[command(flatten)]
    from: DragFromLocatorArgs,
    #[command(flatten)]
    to: DragToLocatorArgs,
    #[arg(long)]
    duration_ms: Option<u64>,
    #[arg(long)]
    steps: Option<u32>,
}

impl InputSwipeArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(duration_ms) = self.duration_ms {
            input.insert("duration_ms".into(), Value::from(duration_ms));
        }
        if let Some(steps) = self.steps {
            input.insert("steps".into(), Value::from(steps));
        }
        Ok(ToolInvocation {
            tool: "swipe",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Application lifecycle actions",
    override_help = APP_HELP,
    arg_required_else_help = true
)]
struct AppArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: AppCommand,
}

impl AppArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Window management actions",
    override_help = WINDOW_HELP,
    arg_required_else_help = true
)]
struct WindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: WindowCommand,
}

impl WindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "MCP server commands",
    override_help = MCP_HELP,
    arg_required_else_help = true
)]
struct McpArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: McpCommand,
}

impl McpArgs {
    fn into_execution(self, root_common: CommonArgs) -> Result<CliExecution, String> {
        self.command
            .into_execution(merge_common(root_common, self.common))
    }

    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        match self.into_execution(root_common)? {
            CliExecution::Tool(invocation) => Ok(invocation),
            CliExecution::McpServe => {
                Err("mcp serve does not map to a runtime tool invocation".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
enum McpCommand {
    #[command(about = "Run the MCP stdio server")]
    Serve,
}

impl McpCommand {
    fn into_execution(self, _common: CommonArgs) -> Result<CliExecution, String> {
        match self {
            Self::Serve => Ok(CliExecution::McpServe),
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
enum WindowCommand {
    #[command(about = "Focus a specific window")]
    Focus(WindowFocusArgs),
    #[command(about = "Close a specific window")]
    Close(WindowCloseArgs),
    #[command(about = "Minimize a specific window")]
    Minimize(WindowMinimizeArgs),
    #[command(about = "Maximize a specific window")]
    Maximize(WindowMaximizeArgs),
    #[command(about = "Move a specific window")]
    Move(WindowMoveArgs),
    #[command(about = "Resize a specific window")]
    Resize(WindowResizeArgs),
    #[command(about = "Set the full bounds of a specific window")]
    SetBounds(WindowSetBoundsArgs),
}

impl WindowCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Focus(args) => args.into_invocation(common),
            Self::Close(args) => args.into_invocation(common),
            Self::Minimize(args) => args.into_invocation(common),
            Self::Maximize(args) => args.into_invocation(common),
            Self::Move(args) => args.into_invocation(common),
            Self::Resize(args) => args.into_invocation(common),
            Self::SetBounds(args) => args.into_invocation(common),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct WindowFocusArgs {
    #[arg(long)]
    window_id: u64,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl WindowFocusArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        insert_serialized(&mut input, "window_id", WindowId::from(self.window_id))?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "focus-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowCloseArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
}

impl WindowCloseArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        Ok(ToolInvocation {
            tool: "close-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowMinimizeArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
    #[command(flatten)]
    verification: WindowStateVerificationArgs,
}

impl WindowMinimizeArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "minimize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowMaximizeArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
}

impl WindowMaximizeArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        Ok(ToolInvocation {
            tool: "maximize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowMoveArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long, allow_hyphen_values = true)]
    x: f64,
    #[arg(long, allow_hyphen_values = true)]
    y: f64,
}

impl WindowMoveArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("x".into(), Value::from(self.x));
        input.insert("y".into(), Value::from(self.y));
        Ok(ToolInvocation {
            tool: "move-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowResizeArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long)]
    width: f64,
    #[arg(long)]
    height: f64,
}

impl WindowResizeArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("width".into(), Value::from(self.width));
        input.insert("height".into(), Value::from(self.height));
        Ok(ToolInvocation {
            tool: "resize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct WindowSetBoundsArgs {
    #[command(flatten)]
    target: WindowTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long, allow_hyphen_values = true)]
    x: f64,
    #[arg(long, allow_hyphen_values = true)]
    y: f64,
    #[arg(long)]
    width: f64,
    #[arg(long)]
    height: f64,
}

impl WindowSetBoundsArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("x".into(), Value::from(self.x));
        input.insert("y".into(), Value::from(self.y));
        input.insert("width".into(), Value::from(self.width));
        input.insert("height".into(), Value::from(self.height));
        Ok(ToolInvocation {
            tool: "set-window-bounds",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Subcommand)]
enum AppCommand {
    #[command(about = "Launch an application by bundle identifier or name")]
    Launch(AppLaunchArgs),
    #[command(about = "Bring an application to the foreground")]
    Switch(AppLifecycleArgs),
    #[command(about = "Quit an application")]
    Quit(AppLifecycleArgs),
    #[command(about = "Relaunch an application")]
    Relaunch(AppLifecycleArgs),
    #[command(about = "Hide an application")]
    Hide(AppLifecycleArgs),
    #[command(about = "Unhide an application")]
    Unhide(AppLifecycleArgs),
}

impl AppCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Launch(args) => args.into_invocation(common),
            Self::Switch(args) => args.into_invocation("switch-app", common),
            Self::Quit(args) => args.into_invocation("quit-app", common),
            Self::Relaunch(args) => args.into_invocation("relaunch-app", common),
            Self::Hide(args) => args.into_invocation("hide-app", common),
            Self::Unhide(args) => args.into_invocation("unhide-app", common),
        }
    }
}

#[derive(Debug, Clone, Args)]
struct ArtifactGetArgs {
    artifact_id: String,
}

impl ArtifactGetArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        let mut input = common_input(&common);
        insert_serialized(
            &mut input,
            "artifact_id",
            ArtifactId::from(self.artifact_id),
        )?;
        Ok(ToolInvocation {
            tool: "artifact-get",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct LegacyArtifactGetArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    artifact_id: String,
}

impl LegacyArtifactGetArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        ArtifactGetArgs {
            artifact_id: self.artifact_id,
        }
        .into_invocation(common)
    }
}

#[derive(Debug, Clone, Args)]
struct LegacySnapshotGetArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    snapshot_id: String,
}

impl LegacySnapshotGetArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        SnapshotGetArgs {
            snapshot_id: self.snapshot_id,
        }
        .into_invocation(common)
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
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        if let Some(app) = self.app {
            input.insert("app".into(), Value::String(app));
        }
        Ok(ToolInvocation {
            tool: "list-windows",
            input: Value::Object(input),
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: ClickLocatorArgs,
}

impl ClickArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        insert_serialized(&mut input, "mode", self.mode.click_mode())?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "click",
            input: Value::Object(input),
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: MoveLocatorArgs,
}

impl MoveArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let locator = self.locator.into_locator()?;
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        if locator.is_none() && target_selector.is_none() {
            return Err("move requires a locator, coordinates, or target selector".to_string());
        }
        if let Some(locator) = locator {
            insert_serialized(&mut input, "locator", locator)?;
        }
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "move",
            input: Value::Object(input),
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
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
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
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
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
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
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "from", self.from.into_locator()?)?;
        insert_serialized(&mut input, "to", self.to.into_locator()?)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(duration_ms) = self.duration_ms {
            input.insert("duration_ms".into(), Value::from(duration_ms));
        }
        if let Some(steps) = self.steps {
            input.insert("steps".into(), Value::from(steps));
        }
        Ok(ToolInvocation {
            tool: "swipe",
            input: Value::Object(input),
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: ScrollLocatorArgs,
}

impl ScrollArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("delta_x".into(), Value::from(self.delta_x));
        input.insert("delta_y".into(), Value::from(self.delta_y));
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "scroll",
            input: Value::Object(input),
            json_output: common.json_output,
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
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl HotkeyArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        insert_serialized(&mut input, "keys", self.keys)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "hotkey",
            input: Value::Object(input),
            json_output: common.json_output,
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
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl PressArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.action_target.into_parts()?;
        input.insert("key".into(), Value::String(self.key));
        insert_serialized(&mut input, "count", self.count)?;
        insert_action_target(&mut input, target_selector, focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "press",
            input: Value::Object(input),
            json_output: common.json_output,
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
    verification: ActionVerificationArgs,
    #[command(flatten)]
    locator: TypeLocatorArgs,
}

impl TypeArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
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
        insert_verifications(&mut input, self.verification.into_verifications())?;
        if let Some(locator) = self.locator.into_locator()? {
            insert_serialized(&mut input, "locator", locator)?;
        }
        Ok(ToolInvocation {
            tool: "type",
            input: Value::Object(input),
            json_output: common.json_output,
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
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        launch_app_invocation(
            merge_common(root_common, self.common),
            self.bundle_id_or_name,
        )
    }
}

#[derive(Debug, Clone, Args)]
struct AppLaunchArgs {
    bundle_id_or_name: String,
}

impl AppLaunchArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        launch_app_invocation(common, self.bundle_id_or_name)
    }
}

#[derive(Debug, Clone, Args)]
struct CloseWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
}

impl CloseWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        Ok(ToolInvocation {
            tool: "close-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct MinimizeWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
    #[command(flatten)]
    verification: WindowStateVerificationArgs,
}

impl MinimizeWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "minimize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct MaximizeWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
}

impl MaximizeWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        Ok(ToolInvocation {
            tool: "maximize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct MoveWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long, allow_hyphen_values = true)]
    x: f64,
    #[arg(long, allow_hyphen_values = true)]
    y: f64,
}

impl MoveWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("x".into(), Value::from(self.x));
        input.insert("y".into(), Value::from(self.y));
        Ok(ToolInvocation {
            tool: "move-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct ResizeWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long)]
    width: f64,
    #[arg(long)]
    height: f64,
}

impl ResizeWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("width".into(), Value::from(self.width));
        input.insert("height".into(), Value::from(self.height));
        Ok(ToolInvocation {
            tool: "resize-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct SetWindowBoundsArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: WindowChromeTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
    #[arg(long, allow_hyphen_values = true)]
    x: f64,
    #[arg(long, allow_hyphen_values = true)]
    y: f64,
    #[arg(long)]
    width: f64,
    #[arg(long)]
    height: f64,
}

impl SetWindowBoundsArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        let (target_selector, focus_policy) = self.target.into_parts()?;
        insert_serialized(&mut input, "target_selector", target_selector)?;
        insert_serialized(&mut input, "focus_policy", focus_policy)?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        input.insert("x".into(), Value::from(self.x));
        input.insert("y".into(), Value::from(self.y));
        input.insert("width".into(), Value::from(self.width));
        input.insert("height".into(), Value::from(self.height));
        Ok(ToolInvocation {
            tool: "set-window-bounds",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
struct LifecycleActionArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    target: LifecycleTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl LifecycleActionArgs {
    fn into_invocation(
        self,
        tool: &'static str,
        root_common: CommonArgs,
    ) -> Result<ToolInvocation, String> {
        lifecycle_action_invocation(
            tool,
            merge_common(root_common, self.common),
            self.target,
            self.verification,
        )
    }
}

#[derive(Debug, Clone, Args)]
struct AppLifecycleArgs {
    #[command(flatten)]
    target: LifecycleTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl AppLifecycleArgs {
    fn into_invocation(
        self,
        tool: &'static str,
        common: CommonArgs,
    ) -> Result<ToolInvocation, String> {
        lifecycle_action_invocation(tool, common, self.target, self.verification)
    }
}

#[derive(Debug, Clone, Args)]
struct FocusWindowArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long)]
    window_id: u64,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl FocusWindowArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let common = merge_common(root_common, self.common);
        let mut input = common_input(&common);
        insert_serialized(&mut input, "window_id", WindowId::from(self.window_id))?;
        insert_verifications(&mut input, self.verification.into_verifications())?;
        Ok(ToolInvocation {
            tool: "focus-window",
            input: Value::Object(input),
            json_output: common.json_output,
        })
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CaptureProfileArg {
    All,
    Elements,
    Screenshot,
    None,
}

impl CaptureProfileArg {
    fn flags(self) -> (bool, bool) {
        match self {
            Self::All => (true, true),
            Self::Elements => (false, true),
            Self::Screenshot => (true, false),
            Self::None => (false, false),
        }
    }
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

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
enum VerificationArg {
    Focus,
    WindowState,
    Geometry,
}

impl VerificationArg {
    fn verification(self) -> ActionVerification {
        match self {
            Self::Focus => ActionVerification::Focus,
            Self::WindowState => ActionVerification::WindowState,
            Self::Geometry => ActionVerification::Geometry,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum WindowStateVerificationArg {
    WindowState,
}

impl WindowStateVerificationArg {
    fn verification(self) -> ActionVerification {
        match self {
            Self::WindowState => ActionVerification::WindowState,
        }
    }
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
struct InputActionTargetArgs {
    #[command(flatten)]
    selector: TargetSelectorArgs,
    #[arg(long, value_enum, default_value_t = FocusPolicyArg::Auto)]
    focus: FocusPolicyArg,
}

impl InputActionTargetArgs {
    fn into_parts(self) -> Result<(Option<ActionTargetSelector>, ActionFocusPolicy), String> {
        Ok((
            self.selector.into_optional_selector()?,
            self.focus.focus_policy(),
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
struct WindowChromeTargetArgs {
    #[command(flatten)]
    selector: TargetSelectorArgs,
    #[arg(long, value_enum, default_value_t = FocusPolicyArg::Auto)]
    focus_policy: FocusPolicyArg,
}

impl WindowChromeTargetArgs {
    fn into_parts(self) -> Result<(ActionTargetSelector, ActionFocusPolicy), String> {
        Ok((
            self.selector.into_required_selector()?,
            self.focus_policy.focus_policy(),
        ))
    }
}

#[derive(Debug, Clone, Args, Default)]
struct WindowTargetArgs {
    #[command(flatten)]
    selector: TargetSelectorArgs,
    #[arg(long, value_enum, default_value_t = FocusPolicyArg::Auto)]
    focus: FocusPolicyArg,
}

impl WindowTargetArgs {
    fn into_parts(self) -> Result<(ActionTargetSelector, ActionFocusPolicy), String> {
        Ok((
            self.selector.into_required_selector()?,
            self.focus.focus_policy(),
        ))
    }
}

#[derive(Debug, Clone, Args, Default)]
struct ActionVerificationArgs {
    #[arg(long = "verify", value_enum)]
    verifications: Vec<VerificationArg>,
}

impl ActionVerificationArgs {
    fn into_verifications(self) -> Vec<ActionVerification> {
        self.verifications
            .into_iter()
            .map(VerificationArg::verification)
            .collect()
    }
}

#[derive(Debug, Clone, Args, Default)]
struct WindowStateVerificationArgs {
    #[arg(long = "verify", value_enum)]
    verifications: Vec<WindowStateVerificationArg>,
}

impl WindowStateVerificationArgs {
    fn into_verifications(self) -> Vec<ActionVerification> {
        self.verifications
            .into_iter()
            .map(WindowStateVerificationArg::verification)
            .collect()
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
struct InputLocatorArgs {
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

impl InputLocatorArgs {
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

fn launch_app_invocation(
    common: CommonArgs,
    bundle_id_or_name: String,
) -> Result<ToolInvocation, String> {
    let mut input = common_input(&common);
    input.insert("bundle_id_or_name".into(), Value::String(bundle_id_or_name));
    Ok(ToolInvocation {
        tool: "launch-app",
        input: Value::Object(input),
        json_output: common.json_output,
    })
}

fn lifecycle_action_invocation(
    tool: &'static str,
    common: CommonArgs,
    target: LifecycleTargetArgs,
    verification: ActionVerificationArgs,
) -> Result<ToolInvocation, String> {
    let mut input = common_input(&common);
    insert_serialized(&mut input, "target_selector", target.into_selector()?)?;
    insert_verifications(&mut input, verification.into_verifications())?;
    Ok(ToolInvocation {
        tool,
        input: Value::Object(input),
        json_output: common.json_output,
    })
}

fn merge_common(root: CommonArgs, local: CommonArgs) -> CommonArgs {
    CommonArgs {
        target: local.target.or(root.target),
        json_output: local.json_output || root.json_output,
        timeout_ms: local.timeout_ms.or(root.timeout_ms),
    }
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

fn insert_verifications(
    map: &mut Map<String, Value>,
    verifications: Vec<ActionVerification>,
) -> Result<(), String> {
    if !verifications.is_empty() {
        insert_serialized(map, "verifications", verifications)?;
    }

    Ok(())
}
