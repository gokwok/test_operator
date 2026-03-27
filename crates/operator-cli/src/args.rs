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

#[derive(Clone, Copy)]
struct CommandHelpEntry {
    command: &'static str,
    about: &'static str,
}

#[derive(Clone, Copy)]
struct CommandHelpGroup {
    usage: &'static str,
    about: &'static str,
    entries_heading: &'static str,
    commands: &'static [CommandHelpEntry],
    examples: &'static [&'static str],
    footer: &'static str,
}

#[derive(Clone, Copy)]
struct RootHelpSection {
    heading: &'static str,
    commands: &'static [CommandHelpEntry],
}

const ROOT_ABOUT: &str = "Operator - Turn any desktop app into an API, from CLI to AI";
const ROOT_USAGE: &str = "operator [OPTIONS] [COMMAND]";
const ROOT_FOOTER: &str =
    "Use 'operator <group> --help' or 'operator <group> <command> --help' for detailed usage.";
const ROOT_EXAMPLES: &[&str] = &[
    "operator capture frontmost",
    "operator elements window --window-id 42",
    "operator list windows",
    "operator input click --text Save",
    "operator mcp serve",
];

const PRINT_HELP_ABOUT: &str = "Print this message or the help of the given subcommand(s)";
const PERMISSIONS_ABOUT: &str = "Check automation permissions and runtime readiness";
const CAPABILITIES_ABOUT: &str =
    "Show supported surfaces, queries, and actions for the active target";
const SHOW_ABOUT: &str = "Show the currently focused app, window, and element";
const AGENT_ABOUT: &str = "Execute a single-shot natural-language task against a target";

const CAPTURE_ABOUT: &str = "Take a screenshot of a surface";
const CAPTURE_FRONTMOST_ABOUT: &str = "Take a screenshot of the frontmost app surface";
const CAPTURE_WINDOW_ABOUT: &str = "Take a screenshot of a specific window";
const CAPTURE_REGION_ABOUT: &str = "Take a screenshot of a screen region defined by coordinates";
const CAPTURE_FULLSCREEN_ABOUT: &str = "Take a screenshot of the full display";

const ELEMENTS_ABOUT: &str = "Query the accessibility element tree for a surface";
const ELEMENTS_FRONTMOST_ABOUT: &str =
    "Query the accessibility element tree for the frontmost app surface";
const ELEMENTS_WINDOW_ABOUT: &str = "Query the accessibility element tree for a specific window";
const ELEMENTS_REGION_ABOUT: &str = "Query accessibility elements within a screen region";
const ELEMENTS_FULLSCREEN_ABOUT: &str = "Query the accessibility element tree for the full display";

const SNAPSHOT_ABOUT: &str = "Read a stored snapshot by ID";

const ARTIFACT_ABOUT: &str = "Read a stored capture artifact by ID";

const LIST_ABOUT: &str = "List running apps or windows";
const LIST_APPS_ABOUT: &str = "List running applications";
const LIST_WINDOWS_ABOUT: &str = "List windows, optionally filtered by app";

const INPUT_ABOUT: &str = "Pointer and keyboard actions against locators or target windows/apps";
const INPUT_CLICK_ABOUT: &str = "Click a locator, coordinates, or target";
const INPUT_MOVE_ABOUT: &str = "Move the pointer to a locator, coordinates, or target";
const INPUT_TYPE_ABOUT: &str = "Type text into the focused or resolved target";
const INPUT_PRESS_ABOUT: &str = "Press a single key";
const INPUT_HOTKEY_ABOUT: &str = "Press a key chord";
const INPUT_SCROLL_ABOUT: &str = "Scroll by delta against a locator or target";
const INPUT_DRAG_ABOUT: &str = "Drag between two locators";
const INPUT_SWIPE_ABOUT: &str = "Swipe between two locators";

const APP_ABOUT: &str = "Launch, switch, hide, quit, and relaunch applications";
const APP_LAUNCH_ABOUT: &str = "Launch an application by bundle identifier or name";
const APP_SWITCH_ABOUT: &str = "Bring an application to the foreground";
const APP_QUIT_ABOUT: &str = "Quit an application";
const APP_RELAUNCH_ABOUT: &str = "Relaunch an application";
const APP_HIDE_ABOUT: &str = "Hide an application";
const APP_UNHIDE_ABOUT: &str = "Unhide an application";

const WINDOW_ABOUT: &str = "Focus, close, resize, or move application windows";
const WINDOW_FOCUS_ABOUT: &str = "Focus a specific window";
const WINDOW_CLOSE_ABOUT: &str = "Close a specific window";
const WINDOW_MINIMIZE_ABOUT: &str = "Minimize a specific window";
const WINDOW_MAXIMIZE_ABOUT: &str = "Maximize a specific window";
const WINDOW_MOVE_ABOUT: &str = "Move a specific window";
const WINDOW_RESIZE_ABOUT: &str = "Resize a specific window";
const WINDOW_SET_BOUNDS_ABOUT: &str = "Set the full bounds of a specific window";

const MCP_ABOUT: &str = "Run the Operator MCP entrypoint";
const MCP_SERVE_ABOUT: &str = "Start the MCP stdio server";

const ROOT_CORE_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "permissions",
        about: PERMISSIONS_ABOUT,
    },
    CommandHelpEntry {
        command: "capabilities",
        about: CAPABILITIES_ABOUT,
    },
];

const ROOT_OBSERVE_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "capture",
        about: CAPTURE_ABOUT,
    },
    CommandHelpEntry {
        command: "elements",
        about: ELEMENTS_ABOUT,
    },
    CommandHelpEntry {
        command: "show",
        about: SHOW_ABOUT,
    },
    CommandHelpEntry {
        command: "snapshot",
        about: SNAPSHOT_ABOUT,
    },
    CommandHelpEntry {
        command: "artifact",
        about: ARTIFACT_ABOUT,
    },
];

const ROOT_QUERY_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "list",
    about: LIST_ABOUT,
}];

const ROOT_ACTION_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "input",
        about: INPUT_ABOUT,
    },
    CommandHelpEntry {
        command: "app",
        about: APP_ABOUT,
    },
    CommandHelpEntry {
        command: "window",
        about: WINDOW_ABOUT,
    },
];

const ROOT_MCP_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "mcp",
    about: MCP_ABOUT,
}];

const ROOT_AGENT_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "agent",
    about: AGENT_ABOUT,
}];

const ROOT_HELP_SECTIONS: &[RootHelpSection] = &[
    RootHelpSection {
        heading: "Core",
        commands: ROOT_CORE_COMMANDS,
    },
    RootHelpSection {
        heading: "Observe",
        commands: ROOT_OBSERVE_COMMANDS,
    },
    RootHelpSection {
        heading: "Query",
        commands: ROOT_QUERY_COMMANDS,
    },
    RootHelpSection {
        heading: "Action",
        commands: ROOT_ACTION_COMMANDS,
    },
    RootHelpSection {
        heading: "MCP",
        commands: ROOT_MCP_COMMANDS,
    },
    RootHelpSection {
        heading: "Agent",
        commands: ROOT_AGENT_COMMANDS,
    },
];

const CAPTURE_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "frontmost",
        about: CAPTURE_FRONTMOST_ABOUT,
    },
    CommandHelpEntry {
        command: "window",
        about: CAPTURE_WINDOW_ABOUT,
    },
    CommandHelpEntry {
        command: "region",
        about: CAPTURE_REGION_ABOUT,
    },
    CommandHelpEntry {
        command: "fullscreen",
        about: CAPTURE_FULLSCREEN_ABOUT,
    },
];

const ELEMENTS_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "frontmost",
        about: ELEMENTS_FRONTMOST_ABOUT,
    },
    CommandHelpEntry {
        command: "window",
        about: ELEMENTS_WINDOW_ABOUT,
    },
    CommandHelpEntry {
        command: "region",
        about: ELEMENTS_REGION_ABOUT,
    },
    CommandHelpEntry {
        command: "fullscreen",
        about: ELEMENTS_FULLSCREEN_ABOUT,
    },
];

const LIST_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "apps",
        about: LIST_APPS_ABOUT,
    },
    CommandHelpEntry {
        command: "windows",
        about: LIST_WINDOWS_ABOUT,
    },
    CommandHelpEntry {
        command: "help",
        about: PRINT_HELP_ABOUT,
    },
];

const INPUT_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "click",
        about: INPUT_CLICK_ABOUT,
    },
    CommandHelpEntry {
        command: "move",
        about: INPUT_MOVE_ABOUT,
    },
    CommandHelpEntry {
        command: "type",
        about: INPUT_TYPE_ABOUT,
    },
    CommandHelpEntry {
        command: "press",
        about: INPUT_PRESS_ABOUT,
    },
    CommandHelpEntry {
        command: "hotkey",
        about: INPUT_HOTKEY_ABOUT,
    },
    CommandHelpEntry {
        command: "scroll",
        about: INPUT_SCROLL_ABOUT,
    },
    CommandHelpEntry {
        command: "drag",
        about: INPUT_DRAG_ABOUT,
    },
    CommandHelpEntry {
        command: "swipe",
        about: INPUT_SWIPE_ABOUT,
    },
    CommandHelpEntry {
        command: "help",
        about: PRINT_HELP_ABOUT,
    },
];

const APP_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "launch",
        about: APP_LAUNCH_ABOUT,
    },
    CommandHelpEntry {
        command: "switch",
        about: APP_SWITCH_ABOUT,
    },
    CommandHelpEntry {
        command: "quit",
        about: APP_QUIT_ABOUT,
    },
    CommandHelpEntry {
        command: "relaunch",
        about: APP_RELAUNCH_ABOUT,
    },
    CommandHelpEntry {
        command: "hide",
        about: APP_HIDE_ABOUT,
    },
    CommandHelpEntry {
        command: "unhide",
        about: APP_UNHIDE_ABOUT,
    },
    CommandHelpEntry {
        command: "help",
        about: PRINT_HELP_ABOUT,
    },
];

const WINDOW_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "focus",
        about: WINDOW_FOCUS_ABOUT,
    },
    CommandHelpEntry {
        command: "close",
        about: WINDOW_CLOSE_ABOUT,
    },
    CommandHelpEntry {
        command: "minimize",
        about: WINDOW_MINIMIZE_ABOUT,
    },
    CommandHelpEntry {
        command: "maximize",
        about: WINDOW_MAXIMIZE_ABOUT,
    },
    CommandHelpEntry {
        command: "move",
        about: WINDOW_MOVE_ABOUT,
    },
    CommandHelpEntry {
        command: "resize",
        about: WINDOW_RESIZE_ABOUT,
    },
    CommandHelpEntry {
        command: "set-bounds",
        about: WINDOW_SET_BOUNDS_ABOUT,
    },
    CommandHelpEntry {
        command: "help",
        about: PRINT_HELP_ABOUT,
    },
];

const MCP_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "serve",
        about: MCP_SERVE_ABOUT,
    },
    CommandHelpEntry {
        command: "help",
        about: PRINT_HELP_ABOUT,
    },
];

const CAPTURE_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator capture [OPTIONS] <SURFACE>",
    about: CAPTURE_ABOUT,
    entries_heading: "Surfaces",
    commands: CAPTURE_GROUP_COMMANDS,
    examples: &[
        "operator capture frontmost",
        "operator capture window --window-id 42",
    ],
    footer: "Use 'operator capture <surface> --help' for detailed usage.",
};

const ELEMENTS_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator elements [OPTIONS] <SURFACE>",
    about: ELEMENTS_ABOUT,
    entries_heading: "Surfaces",
    commands: ELEMENTS_GROUP_COMMANDS,
    examples: &[
        "operator elements frontmost",
        "operator elements window --window-id 42",
    ],
    footer: "Use 'operator elements <surface> --help' for detailed usage.",
};

const LIST_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator list [OPTIONS] <COMMAND>",
    about: LIST_ABOUT,
    entries_heading: "Commands",
    commands: LIST_GROUP_COMMANDS,
    examples: &["operator list apps", "operator list windows --app TextEdit"],
    footer: "Use 'operator list <command> --help' for detailed usage.",
};

const INPUT_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator input [OPTIONS] <COMMAND>",
    about: INPUT_ABOUT,
    entries_heading: "Commands",
    commands: INPUT_GROUP_COMMANDS,
    examples: &[
        "operator input click --text Save --app Notes --focus auto --verify focus",
        "operator input type \"hello operator\" --window-title Draft --after-key return",
    ],
    footer: "Use 'operator input <command> --help' for detailed usage.",
};

const APP_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator app [OPTIONS] <COMMAND>",
    about: APP_ABOUT,
    entries_heading: "Commands",
    commands: APP_GROUP_COMMANDS,
    examples: &[
        "operator app launch Calculator",
        "operator app switch --app TextEdit",
    ],
    footer: "Use 'operator app <command> --help' for detailed usage.",
};

const WINDOW_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator window [OPTIONS] <COMMAND>",
    about: WINDOW_ABOUT,
    entries_heading: "Commands",
    commands: WINDOW_GROUP_COMMANDS,
    examples: &[
        "operator window focus --window-id 42 --verify focus",
        "operator window resize --window-id 42 --width 900 --height 700 --verify geometry",
    ],
    footer: "Use 'operator window <command> --help' for detailed usage.",
};

const MCP_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator mcp [OPTIONS] <COMMAND>",
    about: MCP_ABOUT,
    entries_heading: "Commands",
    commands: MCP_GROUP_COMMANDS,
    examples: &["operator mcp serve"],
    footer: "Use 'operator mcp <command> --help' for detailed usage.",
};

const PERMISSIONS_AFTER_HELP: &str = "Examples
  operator permissions
  operator --json permissions";

const CAPABILITIES_AFTER_HELP: &str = "Examples
  operator capabilities
  operator capabilities --json";

const SHOW_AFTER_HELP: &str = "Examples
  operator show
  operator --json show";

const CAPTURE_WINDOW_AFTER_HELP: &str = "Examples
  operator capture window --window-id 42
  operator --json capture window --window-id 42";

const ELEMENTS_WINDOW_AFTER_HELP: &str = "Examples
  operator elements window --window-id 42
  operator --json elements window --window-id 42";

const SNAPSHOT_AFTER_HELP: &str = "Examples
  operator snapshot s_123
  operator --json snapshot s_123";

const ARTIFACT_AFTER_HELP: &str = "Examples
  operator artifact capture-1.png
  operator --json artifact capture-1.png";

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
  operator agent --target macos --model doubao-seed --max-steps 8 \"Summarize the frontmost window\"";

fn styled_global_runtime_flags() -> String {
    let flags = [
        ("--json", "Emit machine-readable JSON output"),
        ("--target <TARGET>", "Select the named runtime target"),
        (
            "--timeout-ms <TIMEOUT_MS>",
            "Override the runtime timeout for this command",
        ),
        ("-h, --help", "Print help"),
    ];
    let width = flags.iter().map(|(flag, _)| flag.len()).max().unwrap_or(0) + 2;

    let mut help = format!(
        "{header}Global Runtime Flags{reset}\n",
        header = HEADER_STYLE,
        reset = RESET_STYLE,
    );

    for (flag, description) in flags {
        writeln!(
            &mut help,
            "  {command}{flag:<width$}{reset} {body}{description}{reset}",
            command = COMMAND_STYLE,
            flag = flag,
            body = BODY_STYLE,
            description = description,
            reset = RESET_STYLE,
            width = width,
        )
        .expect("write global runtime flag row");
    }

    help
}

fn command_column_width(commands: &[CommandHelpEntry]) -> usize {
    commands
        .iter()
        .map(|entry| entry.command.len())
        .max()
        .unwrap_or(0)
        + 2
}

fn write_command_rows(help: &mut String, commands: &[CommandHelpEntry]) {
    let width = command_column_width(commands);
    for entry in commands {
        writeln!(
            help,
            "  {command_style}{command:<width$}{reset} {body}{details}{reset}",
            command_style = COMMAND_STYLE,
            command = entry.command,
            body = BODY_STYLE,
            details = entry.about,
            reset = RESET_STYLE,
            width = width,
        )
        .expect("write command row");
    }
}

fn styled_group_help(group: &CommandHelpGroup) -> String {
    let mut help = String::new();
    writeln!(
        &mut help,
        "{header}Usage{reset} {command}{usage}{reset}",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        usage = group.usage,
        reset = RESET_STYLE,
    )
    .expect("write usage");
    help.push('\n');
    writeln!(
        &mut help,
        "{body}{description}{reset}",
        body = BODY_STYLE,
        description = group.about,
        reset = RESET_STYLE,
    )
    .expect("write description");
    help.push('\n');
    writeln!(
        &mut help,
        "{header}{entries_heading}{reset}",
        header = HEADER_STYLE,
        entries_heading = group.entries_heading,
        reset = RESET_STYLE,
    )
    .expect("write commands header");
    write_command_rows(&mut help, group.commands);
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
    for example in group.examples {
        writeln!(
            &mut help,
            "  {command}{example}{reset}",
            command = COMMAND_STYLE,
            example = example,
            reset = RESET_STYLE,
        )
        .expect("write example row");
    }
    help.push('\n');
    writeln!(
        &mut help,
        "{muted}{footer}{reset}",
        muted = MUTED_STYLE,
        footer = group.footer,
        reset = RESET_STYLE,
    )
    .expect("write footer");

    help
}

fn root_help() -> String {
    let mut help = String::new();
    writeln!(
        &mut help,
        "{header}Usage{reset} {command}{usage}{reset}",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        usage = ROOT_USAGE,
        reset = RESET_STYLE,
    )
    .expect("write usage");
    help.push('\n');
    writeln!(
        &mut help,
        "{body}{about}{reset}",
        body = BODY_STYLE,
        about = ROOT_ABOUT,
        reset = RESET_STYLE,
    )
    .expect("write description");
    help.push('\n');

    for section in ROOT_HELP_SECTIONS {
        writeln!(
            &mut help,
            "{header}{heading}{reset}",
            header = HEADER_STYLE,
            heading = section.heading,
            reset = RESET_STYLE,
        )
        .expect("write section header");
        write_command_rows(&mut help, section.commands);
        help.push('\n');
    }

    help.push_str(&styled_global_runtime_flags());
    help.push('\n');
    writeln!(
        &mut help,
        "{header}Examples{reset}",
        header = HEADER_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write examples header");
    for example in ROOT_EXAMPLES {
        writeln!(
            &mut help,
            "  {command}{example}{reset}",
            command = COMMAND_STYLE,
            example = example,
            reset = RESET_STYLE,
        )
        .expect("write example row");
    }
    help.push('\n');
    writeln!(
        &mut help,
        "{muted}{footer}{reset}",
        muted = MUTED_STYLE,
        footer = ROOT_FOOTER,
        reset = RESET_STYLE,
    )
    .expect("write footer");

    help
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

fn legacy_command_replacement(args: &[OsString]) -> Option<(String, String)> {
    let path = command_path(args);
    match path.as_slice() {
        ["snapshot-get", ..] => Some((
            "snapshot-get".into(),
            "operator snapshot <snapshot-id>".into(),
        )),
        ["artifact-get", ..] => Some((
            "artifact-get".into(),
            "operator artifact <artifact-id>".into(),
        )),
        ["snapshot", "get", ..] => Some((
            "snapshot get".into(),
            "operator snapshot <snapshot-id>".into(),
        )),
        ["artifact", "get", ..] => Some((
            "artifact get".into(),
            "operator artifact <artifact-id>".into(),
        )),
        ["observe", surface, ..] => Some((
            format!("observe {surface}"),
            observe_replacement(surface, args),
        )),
        ["observe"] => Some((
            "observe".into(),
            "operator capture <surface> or operator elements <surface>".into(),
        )),
        ["get-focus", ..] => Some(("get-focus".into(), "operator show".into())),
        ["focus", ..] => Some(("focus".into(), "operator show".into())),
        ["list-apps", ..] => Some(("list-apps".into(), "operator list apps".into())),
        ["list-windows", ..] => Some(("list-windows".into(), "operator list windows".into())),
        ["permissions-status", ..] => {
            Some(("permissions-status".into(), "operator permissions".into()))
        }
        ["click", ..] => Some(("click".into(), "operator input click".into())),
        ["move", ..] => Some(("move".into(), "operator input move".into())),
        ["type", ..] => Some(("type".into(), "operator input type".into())),
        ["press", ..] => Some(("press".into(), "operator input press".into())),
        ["hotkey", ..] => Some(("hotkey".into(), "operator input hotkey".into())),
        ["scroll", ..] => Some(("scroll".into(), "operator input scroll".into())),
        ["drag", ..] => Some(("drag".into(), "operator input drag".into())),
        ["swipe", ..] => Some(("swipe".into(), "operator input swipe".into())),
        ["launch-app", ..] => Some(("launch-app".into(), "operator app launch".into())),
        ["switch-app", ..] => Some(("switch-app".into(), "operator app switch".into())),
        ["quit-app", ..] => Some(("quit-app".into(), "operator app quit".into())),
        ["relaunch-app", ..] => Some(("relaunch-app".into(), "operator app relaunch".into())),
        ["hide-app", ..] => Some(("hide-app".into(), "operator app hide".into())),
        ["unhide-app", ..] => Some(("unhide-app".into(), "operator app unhide".into())),
        ["focus-window", ..] => Some(("focus-window".into(), "operator window focus".into())),
        ["close-window", ..] => Some(("close-window".into(), "operator window close".into())),
        ["minimize-window", ..] => {
            Some(("minimize-window".into(), "operator window minimize".into()))
        }
        ["maximize-window", ..] => {
            Some(("maximize-window".into(), "operator window maximize".into()))
        }
        ["move-window", ..] => Some(("move-window".into(), "operator window move".into())),
        ["resize-window", ..] => Some(("resize-window".into(), "operator window resize".into())),
        ["set-window-bounds", ..] => Some((
            "set-window-bounds".into(),
            "operator window set-bounds".into(),
        )),
        _ => None,
    }
}

fn observe_replacement(surface: &str, args: &[OsString]) -> String {
    match observe_capture_mode(args).as_deref() {
        Some("elements") => format!("operator elements {surface}"),
        Some("screenshot") | None => format!("operator capture {surface}"),
        Some("all") | Some("none") => {
            format!("operator capture {surface} or operator elements {surface}")
        }
        Some(other) => format!(
            "operator capture {surface} or operator elements {surface} (legacy --capture {other} is no longer supported)"
        ),
    }
}

fn observe_capture_mode(args: &[OsString]) -> Option<String> {
    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        let Some(arg) = arg.to_str() else {
            continue;
        };

        if let Some(value) = arg.strip_prefix("--capture=") {
            return Some(value.to_string());
        }

        if arg == "--capture" {
            return iter
                .next()
                .and_then(|value| value.to_str())
                .map(str::to_string);
        }
    }

    None
}

fn legacy_command_error(args: &[OsString]) -> Option<clap::Error> {
    let (legacy, replacement) = legacy_command_replacement(args)?;
    Some(clap::Error::raw(
        clap::error::ErrorKind::InvalidSubcommand,
        format!("legacy command path `{legacy}` has been removed; use `{replacement}` instead"),
    ))
}

#[derive(Debug, Parser)]
#[command(name = "operator", about = ROOT_ABOUT)]
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
        ["capture"] => Some(styled_group_help(&CAPTURE_GROUP_HELP)),
        ["elements"] => Some(styled_group_help(&ELEMENTS_GROUP_HELP)),
        ["list"] => Some(styled_group_help(&LIST_GROUP_HELP)),
        ["input"] => Some(styled_group_help(&INPUT_GROUP_HELP)),
        ["app"] => Some(styled_group_help(&APP_GROUP_HELP)),
        ["window"] => Some(styled_group_help(&WINDOW_GROUP_HELP)),
        ["mcp"] => Some(styled_group_help(&MCP_GROUP_HELP)),
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
    #[command(about = PERMISSIONS_ABOUT, after_help = PERMISSIONS_AFTER_HELP)]
    Permissions(CommonArgs),
    #[command(about = CAPABILITIES_ABOUT, after_help = CAPABILITIES_AFTER_HELP)]
    Capabilities(CommonArgs),
    Capture(CaptureArgs),
    Elements(ElementsArgs),
    Snapshot(SnapshotArgs),
    Artifact(ArtifactArgs),
    List(ListArgs),
    #[command(about = SHOW_ABOUT, after_help = SHOW_AFTER_HELP)]
    Show(CommonArgs),
    Input(InputArgs),
    App(AppArgs),
    Window(WindowArgs),
    Mcp(McpArgs),
    #[command(about = AGENT_ABOUT, after_help = AGENT_AFTER_HELP)]
    Agent(AgentArgs),
}

impl Command {
    fn common(&self) -> Option<&CommonArgs> {
        match self {
            Self::Permissions(args) => Some(args),
            Self::Capabilities(args) => Some(args),
            Self::Capture(args) => Some(&args.common),
            Self::Elements(args) => Some(&args.common),
            Self::Snapshot(args) => Some(&args.common),
            Self::Artifact(args) => Some(&args.common),
            Self::List(args) => args.common(),
            Self::Show(args) => Some(args),
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
            Self::Capture(args) => args.into_invocation(root_common),
            Self::Elements(args) => args.into_invocation(root_common),
            Self::Snapshot(args) => args.into_invocation(root_common),
            Self::Artifact(args) => args.into_invocation(root_common),
            Self::List(args) => args.into_invocation(root_common),
            Self::Show(common) => {
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
    #[arg(long, global = true, help = "Select the named runtime target")]
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
#[command(about = LIST_ABOUT, arg_required_else_help = true)]
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
    #[command(about = LIST_APPS_ABOUT)]
    Apps(CommonArgs),
    #[command(about = LIST_WINDOWS_ABOUT, after_help = LIST_WINDOWS_AFTER_HELP)]
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
#[command(about = CAPTURE_ABOUT, arg_required_else_help = true)]
struct CaptureArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: CaptureCommand,
}

impl CaptureArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Args)]
#[command(about = ELEMENTS_ABOUT, arg_required_else_help = true)]
struct ElementsArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: ElementsCommand,
}

impl ElementsArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        self.command
            .into_invocation(merge_common(root_common, self.common))
    }
}

#[derive(Debug, Clone, Subcommand)]
enum CaptureCommand {
    #[command(about = CAPTURE_FRONTMOST_ABOUT)]
    Frontmost(SurfaceFrontmostArgs),
    #[command(about = CAPTURE_WINDOW_ABOUT, after_help = CAPTURE_WINDOW_AFTER_HELP)]
    Window(SurfaceWindowArgs),
    #[command(about = CAPTURE_REGION_ABOUT)]
    Region(SurfaceRegionArgs),
    #[command(about = CAPTURE_FULLSCREEN_ABOUT)]
    Fullscreen(SurfaceFullscreenArgs),
}

impl CaptureCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Frontmost(_) => surface_invocation(common, SurfaceKind::Frontmost, true, false),
            Self::Window(args) => surface_invocation(
                common,
                SurfaceKind::Window {
                    id: WindowId::from(args.window_id),
                },
                true,
                false,
            ),
            Self::Region(args) => surface_invocation(
                common,
                SurfaceKind::Region {
                    rect: operator_core::Rect {
                        x: args.x,
                        y: args.y,
                        width: args.width,
                        height: args.height,
                    },
                },
                true,
                false,
            ),
            Self::Fullscreen(args) => surface_invocation(
                common,
                SurfaceKind::Fullscreen {
                    display_id: args.display_id,
                },
                true,
                false,
            ),
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
enum ElementsCommand {
    #[command(about = ELEMENTS_FRONTMOST_ABOUT)]
    Frontmost(SurfaceFrontmostArgs),
    #[command(about = ELEMENTS_WINDOW_ABOUT, after_help = ELEMENTS_WINDOW_AFTER_HELP)]
    Window(SurfaceWindowArgs),
    #[command(about = ELEMENTS_REGION_ABOUT)]
    Region(SurfaceRegionArgs),
    #[command(about = ELEMENTS_FULLSCREEN_ABOUT)]
    Fullscreen(SurfaceFullscreenArgs),
}

impl ElementsCommand {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        match self {
            Self::Frontmost(_) => surface_invocation(common, SurfaceKind::Frontmost, false, true),
            Self::Window(args) => surface_invocation(
                common,
                SurfaceKind::Window {
                    id: WindowId::from(args.window_id),
                },
                false,
                true,
            ),
            Self::Region(args) => surface_invocation(
                common,
                SurfaceKind::Region {
                    rect: operator_core::Rect {
                        x: args.x,
                        y: args.y,
                        width: args.width,
                        height: args.height,
                    },
                },
                false,
                true,
            ),
            Self::Fullscreen(args) => surface_invocation(
                common,
                SurfaceKind::Fullscreen {
                    display_id: args.display_id,
                },
                false,
                true,
            ),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct SurfaceFrontmostArgs {}

#[derive(Debug, Clone, Args)]
struct SurfaceWindowArgs {
    #[arg(
        long = "window-id",
        value_name = "ID",
        help = "ID of the target window (from 'operator window list')"
    )]
    window_id: u64,
}

#[derive(Debug, Clone, Args)]
struct SurfaceRegionArgs {
    #[arg(
        long,
        value_name = "X",
        help = "Left edge of the region in screen points"
    )]
    x: f64,
    #[arg(
        long,
        value_name = "Y",
        help = "Top edge of the region in screen points"
    )]
    y: f64,
    #[arg(long, value_name = "W", help = "Width of the region in screen points")]
    width: f64,
    #[arg(long, value_name = "H", help = "Height of the region in screen points")]
    height: f64,
}

#[derive(Debug, Clone, Args)]
struct SurfaceFullscreenArgs {
    #[arg(
        long = "display-id",
        value_name = "ID",
        help = "Display to target (optional, defaults to the active display)"
    )]
    display_id: Option<u32>,
}

fn surface_invocation(
    common: CommonArgs,
    surface_kind: SurfaceKind,
    include_screenshot: bool,
    include_elements: bool,
) -> Result<ToolInvocation, String> {
    let mut input = common_input(&common);
    insert_serialized(&mut input, "surface", Surface { kind: surface_kind })?;
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
    about = SNAPSHOT_ABOUT,
    after_help = SNAPSHOT_AFTER_HELP,
    arg_required_else_help = true
)]
struct SnapshotArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(
        value_name = "SNAPSHOT-ID",
        help = "Snapshot identifier returned by a previous capture or elements command"
    )]
    snapshot_id: String,
}

impl SnapshotArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let Self {
            common,
            snapshot_id,
        } = self;
        let merged_common = merge_common(root_common, common);
        let mut input = common_input(&merged_common);
        insert_serialized(&mut input, "snapshot_id", SnapshotId::from(snapshot_id))?;
        Ok(ToolInvocation {
            tool: "snapshot-get",
            input: Value::Object(input),
            json_output: merged_common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = ARTIFACT_ABOUT,
    after_help = ARTIFACT_AFTER_HELP,
    arg_required_else_help = true
)]
struct ArtifactArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(
        value_name = "ARTIFACT-ID",
        help = "Artifact identifier returned by a previous capture command"
    )]
    artifact_id: String,
}

impl ArtifactArgs {
    fn into_invocation(self, root_common: CommonArgs) -> Result<ToolInvocation, String> {
        let Self {
            common,
            artifact_id,
        } = self;
        let merged_common = merge_common(root_common, common);
        let mut input = common_input(&merged_common);
        insert_serialized(&mut input, "artifact_id", ArtifactId::from(artifact_id))?;
        Ok(ToolInvocation {
            tool: "artifact-get",
            input: Value::Object(input),
            json_output: merged_common.json_output,
        })
    }
}

#[derive(Debug, Clone, Args)]
#[command(about = INPUT_ABOUT, arg_required_else_help = true)]
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
    #[command(about = INPUT_CLICK_ABOUT, after_help = INPUT_CLICK_AFTER_HELP)]
    Click(InputClickArgs),
    #[command(about = INPUT_MOVE_ABOUT)]
    Move(InputMoveArgs),
    #[command(about = INPUT_TYPE_ABOUT, after_help = INPUT_TYPE_AFTER_HELP)]
    Type(InputTypeArgs),
    #[command(about = INPUT_PRESS_ABOUT)]
    Press(InputPressArgs),
    #[command(about = INPUT_HOTKEY_ABOUT)]
    Hotkey(InputHotkeyArgs),
    #[command(about = INPUT_SCROLL_ABOUT)]
    Scroll(InputScrollArgs),
    #[command(about = INPUT_DRAG_ABOUT)]
    Drag(InputDragArgs),
    #[command(about = INPUT_SWIPE_ABOUT)]
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
#[command(about = APP_ABOUT, arg_required_else_help = true)]
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
#[command(about = WINDOW_ABOUT, arg_required_else_help = true)]
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
#[command(about = MCP_ABOUT, arg_required_else_help = true)]
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
    #[command(about = MCP_SERVE_ABOUT, after_help = MCP_SERVE_AFTER_HELP)]
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
    #[command(about = WINDOW_FOCUS_ABOUT, after_help = WINDOW_FOCUS_AFTER_HELP)]
    Focus(WindowFocusArgs),
    #[command(about = WINDOW_CLOSE_ABOUT)]
    Close(WindowCloseArgs),
    #[command(about = WINDOW_MINIMIZE_ABOUT)]
    Minimize(WindowMinimizeArgs),
    #[command(about = WINDOW_MAXIMIZE_ABOUT)]
    Maximize(WindowMaximizeArgs),
    #[command(about = WINDOW_MOVE_ABOUT)]
    Move(WindowMoveArgs),
    #[command(about = WINDOW_RESIZE_ABOUT, after_help = WINDOW_RESIZE_AFTER_HELP)]
    Resize(WindowResizeArgs),
    #[command(about = WINDOW_SET_BOUNDS_ABOUT)]
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
    #[command(about = APP_LAUNCH_ABOUT, after_help = APP_LAUNCH_AFTER_HELP)]
    Launch(AppLaunchArgs),
    #[command(about = APP_SWITCH_ABOUT, after_help = APP_SWITCH_AFTER_HELP)]
    Switch(AppLifecycleArgs),
    #[command(about = APP_QUIT_ABOUT)]
    Quit(AppLifecycleArgs),
    #[command(about = APP_RELAUNCH_ABOUT)]
    Relaunch(AppLifecycleArgs),
    #[command(about = APP_HIDE_ABOUT)]
    Hide(AppLifecycleArgs),
    #[command(about = APP_UNHIDE_ABOUT)]
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
