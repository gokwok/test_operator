#![cfg_attr(test, allow(dead_code))]

use std::{ffi::OsString, fmt::Write as _, num::NonZeroU32};

use clap::{
    builder::styling::{Ansi256Color, Styles},
    Args, ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum,
};
use operator_core::{
    ActionFocusPolicy, ActionTargetSelector, ActionVerification, ArtifactId, ClickMode, Locator,
    Point, SnapshotId, Surface, SurfaceKind, TypeTrailingKey, WindowId,
};
use serde::Serialize;
use serde_json::{Map, Value};

const HEADER_STYLE: &str = "\x1b[1;38;5;214m";
const COMMAND_STYLE: &str = "\x1b[1;38;5;255m";
const BODY_STYLE: &str = "\x1b[38;5;255m";
const MUTED_STYLE: &str = "\x1b[38;5;245m";
const RESET_STYLE: &str = "\x1b[0m";

const OBSERVE_HELP: &str = "Usage operator observe [OPTIONS] <COMMAND>

Create snapshots from frontmost, window, region, or fullscreen surfaces

Commands
  frontmost   Create a snapshot from the frontmost surface
  window      Create a snapshot from a specific window
  region      Create a snapshot from a specific screen region
  fullscreen  Create a snapshot from the full display or the active display
  help        Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator observe frontmost --capture all
  operator observe window --window-id 42 --capture elements

Use 'operator observe <command> --help' for detailed usage.
";

const SNAPSHOT_HELP: &str = "Usage operator snapshot [OPTIONS] <COMMAND>

Read stored snapshots by ID

Commands
  get   Read a stored snapshot by ID
  help  Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator snapshot get s_123

Use 'operator snapshot <command> --help' for detailed usage.
";

const ARTIFACT_HELP: &str = "Usage operator artifact [OPTIONS] <COMMAND>

Resolve stored capture artifacts by ID

Commands
  get   Resolve a stored capture artifact by ID
  help  Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator artifact get capture-1.png

Use 'operator artifact <command> --help' for detailed usage.
";

const LIST_HELP: &str = "Usage operator list [OPTIONS] <COMMAND>

List running apps or windows

Commands
  apps     List running applications
  windows  List windows, optionally filtered by app
  help     Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator list apps
  operator list windows --app TextEdit

Use 'operator list <command> --help' for detailed usage.
";

const INPUT_HELP: &str = "Usage operator input [OPTIONS] <COMMAND>

Pointer and keyboard actions against locators or target windows/apps

Commands
  click   Click a locator, coordinate, or target
  move    Move the pointer to a locator, coordinates, or target
  type    Type text into the focused or resolved target
  press   Press a special key
  hotkey  Press a key chord
  scroll  Scroll by delta against a locator or target
  drag    Drag between two locators
  swipe   Swipe between two locators
  help    Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator input click --text Save --app Notes --focus auto --verify focus
  operator input type \"hello operator\" --window-title Draft --after-key return

Use 'operator input <command> --help' for detailed usage.
";

const APP_HELP: &str = "Usage operator app [OPTIONS] <COMMAND>

Launch, switch, hide, quit, and relaunch applications

Commands
  launch    Launch an application by bundle identifier or name
  switch    Bring an application to the foreground
  quit      Quit an application
  relaunch  Relaunch an application
  hide      Hide an application
  unhide    Unhide an application
  help      Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator app launch Calculator
  operator app switch --app TextEdit

Use 'operator app <command> --help' for detailed usage.
";

const WINDOW_HELP: &str = "Usage operator window [OPTIONS] <COMMAND>

Focus, close, resize, or move application windows

Commands
  focus       Focus a specific window
  close       Close a specific window
  minimize    Minimize a specific window
  maximize    Maximize a specific window
  move        Move a specific window
  resize      Resize a specific window
  set-bounds  Set the full bounds of a specific window
  help        Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator window focus --window-id 42 --verify focus
  operator window resize --window-id 42 --width 900 --height 700 --verify geometry

Use 'operator window <command> --help' for detailed usage.
";

const MCP_HELP: &str = "Usage operator mcp [OPTIONS] <COMMAND>

Run MCP stdio server commands

Commands
  serve  Run the MCP stdio server
  help   Print this message or the help of the given subcommand(s)

Global Runtime Flags
      --json                   Emit machine-readable JSON output
      --target <TARGET>        Select the runtime target
      --timeout-ms <TIMEOUT_MS>
                               Override the runtime timeout for this command
  -h, --help                   Print help

Examples
  operator mcp serve

Use 'operator mcp <command> --help' for detailed usage.
";

const PERMISSIONS_AFTER_HELP: &str = "Examples
  operator permissions
  operator --json permissions";

const CAPABILITIES_AFTER_HELP: &str = "Examples
  operator capabilities
  operator capabilities --json";

const FOCUS_AFTER_HELP: &str = "Examples
  operator focus
  operator --json focus";

const OBSERVE_WINDOW_AFTER_HELP: &str = "Examples
  operator observe window --window-id 42 --capture elements
  operator observe window --window-id 42 --capture screenshot";

const SNAPSHOT_GET_AFTER_HELP: &str = "Examples
  operator snapshot get s_123
  operator --json snapshot get s_123";

const ARTIFACT_GET_AFTER_HELP: &str = "Examples
  operator artifact get capture-1.png
  operator --json artifact get capture-1.png";

const LIST_WINDOWS_AFTER_HELP: &str = "Examples
  operator list windows
  operator list windows --app TextEdit";

const INPUT_CLICK_AFTER_HELP: &str = "Examples
  operator input click --text Save --app Notes --focus auto --verify focus
  operator input click --snapshot s_123 --element e_45 --mode double";

const INPUT_TYPE_AFTER_HELP: &str = "Examples
  operator input type \"hello operator\" --window-title Draft --after-key return
  operator input type \"search\" --text Search --clear-before";

const APP_LAUNCH_AFTER_HELP: &str = "Examples
  operator app launch Calculator
  operator app launch com.apple.TextEdit";

const APP_SWITCH_AFTER_HELP: &str = "Examples
  operator app switch --app TextEdit
  operator app switch --window-title Draft";

const WINDOW_FOCUS_AFTER_HELP: &str = "Examples
  operator window focus --window-id 42 --verify focus
  operator window focus --window-id 7";

const WINDOW_RESIZE_AFTER_HELP: &str = "Examples
  operator window resize --window-id 42 --width 900 --height 700 --verify geometry
  operator window resize --app TextEdit --width 640 --height 480";

const MCP_SERVE_AFTER_HELP: &str = "Examples
  operator mcp serve";

const AGENT_AFTER_HELP: &str = "Examples
  operator agent \"Open Notes and type hello\"
  operator agent --model doubao-seed --max-steps 8 \"Summarize the frontmost window\"";

fn styled_global_runtime_flags() -> String {
    format!(
        "{header}Global Runtime Flags{reset}\n\
      {command}--json{reset}                   {body}Emit machine-readable JSON output{reset}\n\
      {command}--target <TARGET>{reset}        {body}Select the runtime target{reset}\n\
      {command}--timeout-ms <TIMEOUT_MS>{reset}\n\
                               {body}Override the runtime timeout for this command{reset}\n\
  {command}-h, --help{reset}                   {body}Print help{reset}\n",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        body = BODY_STYLE,
        reset = RESET_STYLE,
    )
}

fn styled_group_help(
    description: &str,
    usage: &str,
    commands: &[(&str, &str)],
    examples: &[&str],
    footer: &str,
) -> String {
    let width = commands
        .iter()
        .map(|(command, _)| command.len())
        .max()
        .unwrap_or(0)
        + 2;
    let mut help = String::new();
    writeln!(
        &mut help,
        "{header}Usage{reset} {command}{usage}{reset}",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write usage");
    help.push('\n');
    writeln!(
        &mut help,
        "{body}{description}{reset}",
        body = BODY_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write description");
    help.push('\n');
    writeln!(
        &mut help,
        "{header}Commands{reset}",
        header = HEADER_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write commands header");
    for (command, details) in commands {
        writeln!(
            &mut help,
            "  {command_style}{command:<width$}{reset} {body}{details}{reset}",
            command_style = COMMAND_STYLE,
            body = BODY_STYLE,
            reset = RESET_STYLE,
            width = width,
        )
        .expect("write command row");
    }
    help.push('\n');
    help.push_str(&styled_global_runtime_flags());
    help.push('\n');
    writeln!(
        &mut help,
        "{header}Examples{reset}",
        header = HEADER_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write examples header");
    for example in examples {
        writeln!(
            &mut help,
            "  {command}{example}{reset}",
            command = COMMAND_STYLE,
            reset = RESET_STYLE,
        )
        .expect("write example row");
    }
    help.push('\n');
    writeln!(
        &mut help,
        "{muted}{footer}{reset}",
        muted = MUTED_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write footer");

    help
}

fn root_help() -> String {
    format!(
        "{header}Usage{reset} {command}operator{reset} [OPTIONS] [COMMAND]\n\n\
{body}Operator - Turn any desktop app into an API, from CLI to AI{reset}\n\n\
{header}Core{reset}\n  {command}permissions{reset}   {body}Check automation permissions and runtime readiness{reset}\n  {command}capabilities{reset}  {body}Show supported surfaces, queries, and actions for the active target{reset}\n\n\
{header}Observe{reset}\n  {command}observe{reset}       {body}Create snapshots from frontmost, window, region, or fullscreen surfaces{reset}\n  {command}snapshot{reset}      {body}Read stored snapshots by ID{reset}\n  {command}artifact{reset}      {body}Resolve stored capture artifacts by ID{reset}\n\n\
{header}Query{reset}\n  {command}list{reset}          {body}List running apps or windows{reset}\n  {command}focus{reset}         {body}Show the currently focused app, window, and element{reset}\n\n\
{header}Action{reset}\n  {command}input{reset}         {body}Pointer and keyboard actions against locators or target windows/apps{reset}\n  {command}app{reset}           {body}Launch, switch, hide, quit, and relaunch applications{reset}\n  {command}window{reset}        {body}Focus, close, resize, or move application windows{reset}\n\n\
{header}MCP{reset}\n  {command}mcp{reset}           {body}Run MCP stdio server commands{reset}\n\n\
{header}A2A{reset}\n  {command}agent{reset}         {body}Execute a single-shot natural-language task against a target{reset}\n\n\
{header}Global Runtime Flags{reset}\n      {command}--json{reset}                   {body}Emit machine-readable JSON output{reset}\n      {command}--target <TARGET>{reset}        {body}Select the runtime target{reset}\n      {command}--timeout-ms <TIMEOUT_MS>{reset}\n                               {body}Override the runtime timeout for this command{reset}\n  {command}-h, --help{reset}                   {body}Print help{reset}\n\n\
{header}Examples{reset}\n  {command}operator observe frontmost{reset}\n  {command}operator list windows{reset}\n  {command}operator input click --text Save{reset}\n  {command}operator mcp serve{reset}\n\n\
{muted}Use 'operator <group> --help' or 'operator <group> <command> --help' for detailed usage.{reset}\n",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        body = BODY_STYLE,
        muted = MUTED_STYLE,
        reset = RESET_STYLE,
    )
}

fn observe_help() -> String {
    styled_group_help(
        "Create snapshots from frontmost, window, region, or fullscreen surfaces",
        "operator observe [OPTIONS] <COMMAND>",
        &[
            ("frontmost", "Create a snapshot from the frontmost surface"),
            ("window", "Create a snapshot from a specific window"),
            ("region", "Create a snapshot from a specific screen region"),
            (
                "fullscreen",
                "Create a snapshot from the full display or the active display",
            ),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &[
            "operator observe frontmost --capture all",
            "operator observe window --window-id 42 --capture elements",
        ],
        "Use 'operator observe <command> --help' for detailed usage.",
    )
}

fn snapshot_help() -> String {
    styled_group_help(
        "Read stored snapshots by ID",
        "operator snapshot [OPTIONS] <COMMAND>",
        &[
            ("get", "Read a stored snapshot by ID"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &["operator snapshot get s_123"],
        "Use 'operator snapshot <command> --help' for detailed usage.",
    )
}

fn artifact_help() -> String {
    styled_group_help(
        "Resolve stored capture artifacts by ID",
        "operator artifact [OPTIONS] <COMMAND>",
        &[
            ("get", "Resolve a stored capture artifact by ID"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &["operator artifact get capture-1.png"],
        "Use 'operator artifact <command> --help' for detailed usage.",
    )
}

fn list_help() -> String {
    styled_group_help(
        "List running apps or windows",
        "operator list [OPTIONS] <COMMAND>",
        &[
            ("apps", "List running applications"),
            ("windows", "List windows, optionally filtered by app"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &["operator list apps", "operator list windows --app TextEdit"],
        "Use 'operator list <command> --help' for detailed usage.",
    )
}

fn input_help() -> String {
    styled_group_help(
        "Pointer and keyboard actions against locators or target windows/apps",
        "operator input [OPTIONS] <COMMAND>",
        &[
            ("click", "Click a locator, coordinate, or target"),
            (
                "move",
                "Move the pointer to a locator, coordinates, or target",
            ),
            ("type", "Type text into the focused or resolved target"),
            ("press", "Press a special key"),
            ("hotkey", "Press a key chord"),
            ("scroll", "Scroll by delta against a locator or target"),
            ("drag", "Drag between two locators"),
            ("swipe", "Swipe between two locators"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &[
            "operator input click --text Save --app Notes --focus auto --verify focus",
            "operator input type \"hello operator\" --window-title Draft --after-key return",
        ],
        "Use 'operator input <command> --help' for detailed usage.",
    )
}

fn app_help() -> String {
    styled_group_help(
        "Launch, switch, hide, quit, and relaunch applications",
        "operator app [OPTIONS] <COMMAND>",
        &[
            (
                "launch",
                "Launch an application by bundle identifier or name",
            ),
            ("switch", "Bring an application to the foreground"),
            ("quit", "Quit an application"),
            ("relaunch", "Relaunch an application"),
            ("hide", "Hide an application"),
            ("unhide", "Unhide an application"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &[
            "operator app launch Calculator",
            "operator app switch --app TextEdit",
        ],
        "Use 'operator app <command> --help' for detailed usage.",
    )
}

fn window_help() -> String {
    styled_group_help(
        "Focus, close, resize, or move application windows",
        "operator window [OPTIONS] <COMMAND>",
        &[
            ("focus", "Focus a specific window"),
            ("close", "Close a specific window"),
            ("minimize", "Minimize a specific window"),
            ("maximize", "Maximize a specific window"),
            ("move", "Move a specific window"),
            ("resize", "Resize a specific window"),
            ("set-bounds", "Set the full bounds of a specific window"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &[
            "operator window focus --window-id 42 --verify focus",
            "operator window resize --window-id 42 --width 900 --height 700 --verify geometry",
        ],
        "Use 'operator window <command> --help' for detailed usage.",
    )
}

fn mcp_help() -> String {
    styled_group_help(
        "Run MCP stdio server commands",
        "operator mcp [OPTIONS] <COMMAND>",
        &[
            ("serve", "Run the MCP stdio server"),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
        &["operator mcp serve"],
        "Use 'operator mcp <command> --help' for detailed usage.",
    )
}

fn help_styles() -> Styles {
    Styles::styled()
        .header(Ansi256Color(214).on_default().bold())
        .usage(Ansi256Color(214).on_default().bold())
        .literal(Ansi256Color(255).on_default().bold())
        .placeholder(Ansi256Color(255).on_default())
        .context(Ansi256Color(255).on_default())
}

fn post_process_generated_help(help: &str) -> String {
    let help = help
        .replace("Usage:", "Usage")
        .replace("Options:", "Options")
        .replace("Arguments:", "Arguments")
        .replace("Commands:", "Commands")
        .replace("Examples:", "Examples");
    let help = help.replace(
        "\nExamples\n",
        &format!("\n{HEADER_STYLE}Examples{RESET_STYLE}\n"),
    );
    let help = move_leading_description_below_usage(&help);
    style_generated_examples(&help)
}

fn move_leading_description_below_usage(help: &str) -> String {
    let lines: Vec<&str> = help.lines().collect();
    let Some(usage_idx) = lines
        .iter()
        .position(|line| strip_ansi_for_help(line).starts_with("Usage "))
    else {
        return help.to_owned();
    };
    if usage_idx == 0 {
        return help.to_owned();
    }

    let description_lines: Vec<&str> = lines[..usage_idx]
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if description_lines.is_empty() {
        return help.to_owned();
    }

    let description = format!("{BODY_STYLE}{}{RESET_STYLE}", description_lines.join("\n"),);
    let usage = lines[usage_idx];
    let remainder = lines[usage_idx + 1..].join("\n");
    let remainder = remainder.trim_start_matches('\n');

    if remainder.is_empty() {
        format!("{usage}\n\n{description}\n")
    } else {
        format!("{usage}\n\n{description}\n\n{remainder}")
    }
}

fn style_generated_examples(help: &str) -> String {
    let marker = format!("{HEADER_STYLE}Examples{RESET_STYLE}\n");
    let Some(start) = help.find(&marker) else {
        return help.to_owned();
    };
    let before = &help[..start + marker.len()];
    let after = &help[start + marker.len()..];

    let mut lines = after.lines();
    let mut styled = String::from(before);
    let mut consumed_any = false;

    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            styled.push('\n');
            styled.push_str(&lines.collect::<Vec<_>>().join("\n"));
            if after.ends_with('\n') {
                styled.push('\n');
            }
            return styled;
        }

        if line.starts_with("  ") {
            styled.push_str("  ");
            styled.push_str(COMMAND_STYLE);
            styled.push_str(line.trim_start());
            styled.push_str(RESET_STYLE);
            styled.push('\n');
        } else {
            styled.push_str(line);
            styled.push('\n');
        }
        consumed_any = true;
    }

    if consumed_any && !styled.ends_with('\n') {
        styled.push('\n');
    }

    styled
}

fn strip_ansi_for_help(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            let _ = chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }

        cleaned.push(ch);
    }

    cleaned
}

fn legacy_command_replacement(command: &str) -> Option<&'static str> {
    match command {
        "snapshot-get" => Some("operator snapshot get"),
        "artifact-get" => Some("operator artifact get"),
        "get-focus" => Some("operator focus"),
        "list-apps" => Some("operator list apps"),
        "list-windows" => Some("operator list windows"),
        "permissions-status" => Some("operator permissions"),
        "click" => Some("operator input click"),
        "move" => Some("operator input move"),
        "type" => Some("operator input type"),
        "press" => Some("operator input press"),
        "hotkey" => Some("operator input hotkey"),
        "scroll" => Some("operator input scroll"),
        "drag" => Some("operator input drag"),
        "swipe" => Some("operator input swipe"),
        "launch-app" => Some("operator app launch"),
        "switch-app" => Some("operator app switch"),
        "quit-app" => Some("operator app quit"),
        "relaunch-app" => Some("operator app relaunch"),
        "hide-app" => Some("operator app hide"),
        "unhide-app" => Some("operator app unhide"),
        "focus-window" => Some("operator window focus"),
        "close-window" => Some("operator window close"),
        "minimize-window" => Some("operator window minimize"),
        "maximize-window" => Some("operator window maximize"),
        "move-window" => Some("operator window move"),
        "resize-window" => Some("operator window resize"),
        "set-window-bounds" => Some("operator window set-bounds"),
        _ => None,
    }
}

fn legacy_command_error(args: &[OsString]) -> Option<clap::Error> {
    let command = root_command_token(args)?;
    let replacement = legacy_command_replacement(command)?;
    Some(clap::Error::raw(
        clap::error::ErrorKind::InvalidSubcommand,
        format!("legacy flat command `{command}` has been removed; use `{replacement}` instead"),
    ))
}

fn root_command_token(args: &[OsString]) -> Option<&str> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        let arg = arg.to_str()?;
        match arg {
            "--json" => continue,
            "--target" | "--timeout-ms" => {
                let _ = iter.next();
            }
            "-h" | "--help" | "--" => return None,
            _ if arg.starts_with("--target=") || arg.starts_with("--timeout-ms=") => continue,
            _ if arg.starts_with('-') => return None,
            _ => return Some(arg),
        }
    }
    None
}

#[derive(Debug, Parser)]
#[command(
    name = "operator",
    about = "Operator - Turn any desktop app into an API, from CLI to AI"
)]
pub(crate) struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(crate) fn command() -> clap::Command {
        <Self as CommandFactory>::command()
            .color(ColorChoice::Always)
            .styles(help_styles())
            .override_help(root_help())
    }

    pub(crate) fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let argv = itr.into_iter().map(Into::into).collect::<Vec<OsString>>();
        if let Some(error) = legacy_command_error(&argv) {
            return Err(error);
        }
        let mut command = Self::command();
        let mut matches = command.try_get_matches_from_mut(argv)?;
        <Self as FromArgMatches>::from_arg_matches_mut(&mut matches)
    }

    pub(crate) fn parse() -> Self {
        let argv = std::env::args_os().collect::<Vec<OsString>>();
        if let Some(help) = custom_help(&argv) {
            print!("{help}");
            std::process::exit(0);
        }
        Self::try_parse_from(argv).unwrap_or_else(|error| error.exit())
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
            CliExecution::Agent(_) => {
                Err("agent command does not map to a runtime tool invocation".to_string())
            }
        }
    }
}

pub(crate) fn custom_help(args: &[OsString]) -> Option<String> {
    if !contains_help_flag(args) {
        return None;
    }

    match command_path(args).as_slice() {
        [] => Some(root_help()),
        ["observe"] => Some(observe_help()),
        ["snapshot"] => Some(snapshot_help()),
        ["artifact"] => Some(artifact_help()),
        ["list"] => Some(list_help()),
        ["input"] => Some(input_help()),
        ["app"] => Some(app_help()),
        ["window"] => Some(window_help()),
        ["mcp"] => Some(mcp_help()),
        _ => generated_help(args).map(|help| post_process_generated_help(&help)),
    }
}

fn contains_help_flag(args: &[OsString]) -> bool {
    args.iter().skip(1).any(|arg| {
        arg.to_str()
            .is_some_and(|arg| matches!(arg, "-h" | "--help"))
    })
}

fn command_path(args: &[OsString]) -> Vec<&str> {
    let mut path = Vec::new();
    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        let Some(arg) = arg.to_str() else {
            continue;
        };
        match arg {
            "-h" | "--help" | "--" => break,
            "--json" => continue,
            "--target" | "--timeout-ms" if path.is_empty() => {
                let _ = iter.next();
            }
            _ if path.is_empty()
                && (arg.starts_with("--target=") || arg.starts_with("--timeout-ms=")) =>
            {
                continue;
            }
            _ if arg.starts_with('-') => continue,
            _ => path.push(arg),
        }
    }

    path
}

fn generated_help(args: &[OsString]) -> Option<String> {
    let mut command = <Cli as CommandFactory>::command()
        .color(ColorChoice::Always)
        .styles(help_styles())
        .override_help(root_help());

    match command.try_get_matches_from_mut(args.to_owned()) {
        Ok(_) => None,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            Some(error.render().ansi().to_string())
        }
        Err(_) => None,
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
    Agent(AgentCommand),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        about = "Check automation permissions and runtime readiness",
        after_help = PERMISSIONS_AFTER_HELP
    )]
    Permissions(CommonArgs),
    #[command(
        about = "Show supported surfaces, queries, and actions for the active target",
        after_help = CAPABILITIES_AFTER_HELP
    )]
    Capabilities(CommonArgs),
    Observe(ObserveArgs),
    Snapshot(SnapshotArgs),
    Artifact(ArtifactArgs),
    List(ListArgs),
    #[command(
        about = "Show the currently focused app, window, and element",
        after_help = FOCUS_AFTER_HELP
    )]
    Focus(CommonArgs),
    Input(InputArgs),
    App(AppArgs),
    Window(WindowArgs),
    Mcp(McpArgs),
    #[command(
        about = "Execute a single-shot natural-language task against a target",
        after_help = AGENT_AFTER_HELP
    )]
    Agent(AgentArgs),
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
            Self::Agent(args) => Some(&args.common),
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
            Self::Agent(_) => {
                Err("agent command does not map to a runtime tool invocation".to_string())
            }
        }
    }

    fn into_execution(self, root_common: CommonArgs) -> Result<CliExecution, String> {
        match self {
            Self::Mcp(args) => args.into_execution(root_common),
            Self::Agent(args) => args.into_execution(root_common),
            other => other.into_invocation(root_common).map(CliExecution::Tool),
        }
    }
}

#[derive(Debug, Clone, Default, Args)]
struct CommonArgs {
    #[arg(long, global = true, help = "Select the runtime target")]
    target: Option<String>,
    #[arg(
        long = "json",
        global = true,
        help = "Emit machine-readable JSON output"
    )]
    json_output: bool,
    #[arg(
        long,
        global = true,
        help = "Override the runtime timeout for this command"
    )]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentCommand {
    pub(crate) task: String,
    pub(crate) model: Option<String>,
    pub(crate) max_steps: Option<NonZeroU32>,
    pub(crate) target: Option<String>,
    pub(crate) json_output: bool,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Args)]
struct AgentArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(help = "Natural-language task to execute")]
    task: String,
    #[arg(
        long,
        value_parser = ["gpt-5.4", "doubao-seed"],
        help = "Registered phase-1 model name"
    )]
    model: Option<String>,
    #[arg(long, help = "Override the maximum number of agent steps")]
    max_steps: Option<NonZeroU32>,
}

impl AgentArgs {
    fn into_execution(self, root_common: CommonArgs) -> Result<CliExecution, String> {
        let common = merge_common(root_common, self.common);
        Ok(CliExecution::Agent(AgentCommand {
            task: self.task,
            model: self.model,
            max_steps: self.max_steps,
            target: common.target,
            json_output: common.json_output,
            timeout_ms: common.timeout_ms,
        }))
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "List running apps or windows",
    override_help = LIST_HELP,
    arg_required_else_help = true
)]
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
    #[command(after_help = LIST_WINDOWS_AFTER_HELP)]
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
#[command(
    about = "Create snapshots from frontmost, window, region, or fullscreen surfaces",
    override_help = OBSERVE_HELP,
    arg_required_else_help = true
)]
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
    #[command(
        about = "Capture a specific window",
        after_help = OBSERVE_WINDOW_AFTER_HELP
    )]
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
    about = "Read stored snapshots by ID",
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
    #[command(
        about = "Read a stored snapshot by ID",
        after_help = SNAPSHOT_GET_AFTER_HELP
    )]
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
    about = "Resolve stored capture artifacts by ID",
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
    #[command(
        about = "Resolve a stored capture artifact by ID",
        after_help = ARTIFACT_GET_AFTER_HELP
    )]
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
    about = "Pointer and keyboard actions against locators or target windows/apps",
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
    #[command(about = "Click a locator, coordinate, or target", after_help = INPUT_CLICK_AFTER_HELP)]
    Click(InputClickArgs),
    #[command(about = "Move the pointer to a locator, coordinates, or target")]
    Move(InputMoveArgs),
    #[command(
        about = "Type text into the focused or resolved target",
        after_help = INPUT_TYPE_AFTER_HELP
    )]
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
    about = "Launch, switch, hide, quit, and relaunch applications",
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
    about = "Focus, close, resize, or move application windows",
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
    about = "Run MCP stdio server commands",
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
            CliExecution::Agent(_) => {
                Err("agent command does not map to a runtime tool invocation".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
enum McpCommand {
    #[command(about = "Run the MCP stdio server", after_help = MCP_SERVE_AFTER_HELP)]
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
    #[command(about = "Focus a specific window", after_help = WINDOW_FOCUS_AFTER_HELP)]
    Focus(WindowFocusArgs),
    #[command(about = "Close a specific window")]
    Close(WindowCloseArgs),
    #[command(about = "Minimize a specific window")]
    Minimize(WindowMinimizeArgs),
    #[command(about = "Maximize a specific window")]
    Maximize(WindowMaximizeArgs),
    #[command(about = "Move a specific window")]
    Move(WindowMoveArgs),
    #[command(
        about = "Resize a specific window",
        after_help = WINDOW_RESIZE_AFTER_HELP
    )]
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
    #[command(
        about = "Launch an application by bundle identifier or name",
        after_help = APP_LAUNCH_AFTER_HELP
    )]
    Launch(AppLaunchArgs),
    #[command(
        about = "Bring an application to the foreground",
        after_help = APP_SWITCH_AFTER_HELP
    )]
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
struct AppLaunchArgs {
    bundle_id_or_name: String,
}

impl AppLaunchArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        launch_app_invocation(common, self.bundle_id_or_name)
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
