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

#[derive(Clone, Copy)]
struct LeafHelpSection {
    heading: &'static str,
    rows: &'static [CommandHelpEntry],
}

#[derive(Clone, Copy)]
struct LeafHelp {
    usage: &'static str,
    about: &'static str,
    sections: &'static [LeafHelpSection],
    include_global_runtime_flags: bool,
    examples: &'static [&'static str],
    footer: &'static str,
}

const ROOT_ABOUT: &str = "Operator - Turn any desktop app into an API, from CLI to AI";
const ROOT_USAGE: &str = "operator [OPTIONS] <COMMAND>";
const ROOT_FOOTER: &str = "Use 'operator <command> --help' for detailed usage.";
const ROOT_EXAMPLES: &[&str] = &[
    "operator capture frontmost",
    "operator elements window --window-id 42",
    "operator click --text Save",
    "operator mcp serve",
];

const PERMISSIONS_ABOUT: &str = "Check automation permissions and runtime readiness";
const CAPABILITIES_ABOUT: &str =
    "Show supported surfaces, queries, and actions for the active target";
const SHOW_ABOUT: &str = "Show the currently focused app, window, and element";
const AGENT_ABOUT: &str = "Execute a natural-language task against a target";
const AGENT_LONG_ABOUT: &str =
    "Execute a natural-language task against the active target. The agent
observes the screen, plans actions, and drives the UI autonomously until
the task is complete or the step limit is reached.";

const CAPTURE_ABOUT: &str = "Take a screenshot of a surface";
const CAPTURE_FRONTMOST_ABOUT: &str = "Take a screenshot of the frontmost app surface";
const CAPTURE_WINDOW_ABOUT: &str = "Take a screenshot of a specific window";
const CAPTURE_REGION_ABOUT: &str = "Take a screenshot of a screen region defined by coordinates";
const CAPTURE_FULLSCREEN_ABOUT: &str = "Take a screenshot of the full display";

const ELEMENTS_ABOUT: &str = "Query the accessibility element tree for a surface";
const ELEMENTS_FRONTMOST_ABOUT: &str =
    "Query the accessibility element tree for the frontmost app surface";
const ELEMENTS_WINDOW_ABOUT: &str = "Query the accessibility element tree for a specific window";
const ELEMENTS_REGION_ABOUT: &str =
    "Query accessibility elements whose bounds intersect a screen region";
const ELEMENTS_FULLSCREEN_ABOUT: &str =
    "Query accessibility elements across visible windows on the desktop";
const ELEMENTS_REGION_FOOTER: &str =
    "macOS note: region queries enumerate visible accessible windows and keep only elements whose bounds intersect the requested rect.";
const ELEMENTS_FULLSCREEN_FOOTER: &str =
    "macOS note: fullscreen queries enumerate visible accessible windows on the desktop. `--display-id` is accepted for contract parity but does not yet narrow the AX query.";

const SNAPSHOT_ABOUT: &str = "Read a stored snapshot by ID";

const ARTIFACT_ABOUT: &str = "Read a stored capture artifact by ID";

const APP_LIST_ABOUT: &str = "List running application processes";
const WINDOW_LIST_ABOUT: &str = "List application windows";
const APP_LIST_FOOTER: &str =
    "macOS note: app list returns running application processes reported by System Events. Helper and background services may appear.";
const WINDOW_LIST_FOOTER: &str =
    "macOS note: window list without --app performs a full window enumeration and may be slow. Prefer `--app <NAME>` when the target app is already known.";

const INPUT_CLICK_ABOUT: &str = "Click a locator, coordinates, or target";
const INPUT_MOVE_ABOUT: &str = "Move the pointer to a locator, coordinates, or target";
const INPUT_TYPE_ABOUT: &str = "Type text into the focused or resolved target";
const INPUT_PRESS_ABOUT: &str = "Press a single key";
const INPUT_HOTKEY_ABOUT: &str = "Press a key chord";
const INPUT_SCROLL_ABOUT: &str = "Scroll by delta against a locator or target";
const INPUT_DRAG_ABOUT: &str = "Drag between two locators";
const INPUT_SWIPE_ABOUT: &str = "Swipe between two locators";
const ROOT_DRAG_ABOUT: &str = "Drag from one locator to another";
const ROOT_SWIPE_ABOUT: &str = "Swipe from one locator to another";
const ROOT_MOVE_ABOUT: &str = "Move the pointer to a locator or coordinates";
const PASTE_ABOUT: &str = "Clipboard-aware paste [planned]";

const APP_ABOUT: &str = "Manage application lifecycle";
const APP_LAUNCH_ABOUT: &str = "Launch an application by name or bundle identifier";
const APP_SWITCH_ABOUT: &str = "Bring an application to the foreground";
const APP_QUIT_ABOUT: &str = "Quit an application";
const APP_RELAUNCH_ABOUT: &str = "Quit and relaunch an application";
const APP_HIDE_ABOUT: &str = "Hide an application";
const APP_UNHIDE_ABOUT: &str = "Unhide a hidden application";

const WINDOW_ABOUT: &str = "Manage application windows";
const WINDOW_FOCUS_ABOUT: &str = "Bring a specific window to the foreground";
const WINDOW_CLOSE_ABOUT: &str = "Close a window";
const WINDOW_MINIMIZE_ABOUT: &str = "Minimize a window to the Dock";
const WINDOW_MAXIMIZE_ABOUT: &str = "Maximize a window to fill the display";
const WINDOW_MOVE_ABOUT: &str = "Move a window to new screen coordinates";
const WINDOW_RESIZE_ABOUT: &str = "Resize a window";
const WINDOW_SET_BOUNDS_ABOUT: &str = "Set the full position and size of a window in one operation";
const CLIPBOARD_ABOUT: &str = "Read/write the clipboard [planned]";
const OPEN_ABOUT: &str = "Open a URL or file with its default application [planned]";

const MCP_ABOUT: &str = "Run the Operator MCP server";
const MCP_SERVE_ABOUT: &str = "Start the MCP stdio server";
const MCP_SERVE_LONG_ABOUT: &str =
    "Start the MCP stdio server. Reads JSON-RPC messages from stdin and writes
responses to stdout. Intended to be launched by an MCP host such as Claude
Desktop or a custom integration.";

const ROOT_CORE_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "permissions",
        about: PERMISSIONS_ABOUT,
    },
    CommandHelpEntry {
        command: "capabilities",
        about: CAPABILITIES_ABOUT,
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
];

const ROOT_INTERACT_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "click",
        about: INPUT_CLICK_ABOUT,
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
        about: ROOT_DRAG_ABOUT,
    },
    CommandHelpEntry {
        command: "swipe",
        about: ROOT_SWIPE_ABOUT,
    },
    CommandHelpEntry {
        command: "move",
        about: ROOT_MOVE_ABOUT,
    },
    CommandHelpEntry {
        command: "paste",
        about: PASTE_ABOUT,
    },
];

const ROOT_SYSTEM_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "app",
        about: APP_ABOUT,
    },
    CommandHelpEntry {
        command: "window",
        about: WINDOW_ABOUT,
    },
    CommandHelpEntry {
        command: "clipboard",
        about: CLIPBOARD_ABOUT,
    },
    CommandHelpEntry {
        command: "open",
        about: OPEN_ABOUT,
    },
];

const ROOT_INTEGRATION_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "mcp",
    about: MCP_ABOUT,
}];

const ROOT_AI_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
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
        heading: "Interact",
        commands: ROOT_INTERACT_COMMANDS,
    },
    RootHelpSection {
        heading: "System",
        commands: ROOT_SYSTEM_COMMANDS,
    },
    RootHelpSection {
        heading: "Integration",
        commands: ROOT_INTEGRATION_COMMANDS,
    },
    RootHelpSection {
        heading: "AI",
        commands: ROOT_AI_COMMANDS,
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

const APP_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "list",
        about: APP_LIST_ABOUT,
    },
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
];

const WINDOW_GROUP_COMMANDS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "list",
        about: WINDOW_LIST_ABOUT,
    },
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
];

const MCP_GROUP_COMMANDS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "serve",
    about: MCP_SERVE_ABOUT,
}];

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

const APP_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator app [OPTIONS] <COMMAND>",
    about: APP_ABOUT,
    entries_heading: "Commands",
    commands: APP_GROUP_COMMANDS,
    examples: &[
        "operator app list",
        "operator app launch Notes",
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
        "operator window list",
        "operator window focus --window-id 42",
        "operator window resize --window-id 42 --width 1280 --height 800",
    ],
    footer: "Use 'operator window <command> --help' for detailed usage.",
};

const MCP_GROUP_HELP: CommandHelpGroup = CommandHelpGroup {
    usage: "operator mcp [OPTIONS] <COMMAND>",
    about: MCP_ABOUT,
    entries_heading: "Commands",
    commands: MCP_GROUP_COMMANDS,
    examples: &["operator mcp serve"],
    footer: "",
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

const APP_LIST_AFTER_HELP: &str = "Examples
  operator app list
  operator --json app list";

const INPUT_CLICK_AFTER_HELP: &str = "Examples
  operator click --text Save --app Notes --focus auto --verify focus
  operator click --snapshot s_123 --element e_45 --mode double";

const INPUT_TYPE_AFTER_HELP: &str = "Examples
  operator type \"hello operator\" --window-title Draft --after-key return
  operator type \"search\" --text Search --clear-before";

const APP_LAUNCH_AFTER_HELP: &str = "Examples
  operator app launch Notes
  operator app launch com.apple.TextEdit";

const APP_SWITCH_AFTER_HELP: &str = "Examples
  operator app switch --app TextEdit
  operator app switch --app Safari --verify focus";

const WINDOW_FOCUS_AFTER_HELP: &str = "Examples
  operator window focus --window-id 42
  operator window focus --window-id 42 --verify focus";

const WINDOW_LIST_AFTER_HELP: &str = "Examples
  operator window list
  operator window list --app TextEdit
  operator --json window list";

const WINDOW_RESIZE_AFTER_HELP: &str = "Examples
  operator window resize --window-id 42 --width 1280 --height 800
  operator window resize --app TextEdit --width 900 --height 600 --verify geometry";

const MCP_SERVE_AFTER_HELP: &str = "Examples
  operator mcp serve";

const AGENT_AFTER_HELP: &str = "Examples
  operator agent \"Open Notes and type hello\"
  operator agent \"Find the largest file in Downloads and move it to the Trash\"
  operator agent --model doubao-seed --max-steps 10 \"Summarize the frontmost window\"";

const MCP_SERVE_HELP: LeafHelp = LeafHelp {
    usage: "operator mcp serve [OPTIONS]",
    about: MCP_SERVE_LONG_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &["operator mcp serve"],
    footer: "",
};

const AGENT_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<TASK>",
    about: "Natural-language description of the task to perform",
}];

const AGENT_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--model <MODEL>",
        about: "Model to use for the agent [possible values: gpt-5.4, doubao-seed]",
    },
    CommandHelpEntry {
        command: "--max-steps <N>",
        about: "Maximum number of agent steps before stopping",
    },
];

const AGENT_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Arguments",
        rows: AGENT_ARGUMENT_ROWS,
    },
    LeafHelpSection {
        heading: "Options",
        rows: AGENT_OPTION_ROWS,
    },
];

const AGENT_HELP: LeafHelp = LeafHelp {
    usage: "operator agent [OPTIONS] <TASK>",
    about: AGENT_LONG_ABOUT,
    sections: AGENT_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator agent \"Open Notes and type hello\"",
        "operator agent \"Find the largest file in Downloads and move it to the Trash\"",
        "operator agent --model doubao-seed --max-steps 10 \"Summarize the frontmost window\"",
    ],
    footer: "",
};

const SNAPSHOT_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<SNAPSHOT-ID>",
    about: "Snapshot identifier returned by a previous capture or elements command",
}];

const ARTIFACT_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<ARTIFACT-ID>",
    about:
        "Artifact identifier (e.g. a screenshot filename) returned by a previous capture command",
}];

const APP_LAUNCH_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<APP>",
    about: "Application name (e.g. Notes) or bundle ID (e.g. com.apple.Notes)",
}];

const TYPE_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<TEXT>",
    about: "Text to type",
}];

const PRESS_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<KEY>",
    about: "Key name (e.g. return, escape, tab, space, f1, a, 0)",
}];

const HOTKEY_ARGUMENT_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "<KEY>...",
    about: "Keys to press simultaneously (e.g. command s, control shift z)",
}];

const CLICK_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--mode left|right|middle|double",
    about: "Click mode (default: left)",
}];

const TYPE_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--clear-before",
        about: "Clear the target field before typing",
    },
    CommandHelpEntry {
        command: "--delay-ms <MS>",
        about: "Delay between keystrokes in milliseconds",
    },
    CommandHelpEntry {
        command: "--after-key return|tab|escape|delete",
        about: "Key to press after typing (repeatable)",
    },
];

const PRESS_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--count <N>",
    about: "Number of times to press the key (default: 1)",
}];

const SCROLL_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--delta-x <DX>",
        about: "Horizontal scroll delta (positive = right, negative = left)",
    },
    CommandHelpEntry {
        command: "--delta-y <DY>",
        about: "Vertical scroll delta (positive = down, negative = up)",
    },
];

const DRAG_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--duration-ms <MS>",
        about: "Duration of the drag gesture in milliseconds",
    },
    CommandHelpEntry {
        command: "--steps <N>",
        about: "Number of interpolation steps along the drag path",
    },
    CommandHelpEntry {
        command: "--modifier command|control|option|shift|function",
        about: "Hold modifier key during drag (repeatable)",
    },
];

const SWIPE_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--duration-ms <MS>",
        about: "Duration of the swipe gesture in milliseconds",
    },
    CommandHelpEntry {
        command: "--steps <N>",
        about: "Number of interpolation steps along the swipe path",
    },
];

const SURFACE_WINDOW_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--window-id <ID>",
    about: "ID of the target window (from 'operator window list')",
}];

const SURFACE_REGION_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--x <X>",
        about: "Left edge of the region in screen points",
    },
    CommandHelpEntry {
        command: "--y <Y>",
        about: "Top edge of the region in screen points",
    },
    CommandHelpEntry {
        command: "--width <W>",
        about: "Width of the region in screen points",
    },
    CommandHelpEntry {
        command: "--height <H>",
        about: "Height of the region in screen points",
    },
];

const CAPTURE_FULLSCREEN_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--display-id <ID>",
    about: "Display to capture (optional, defaults to the active display)",
}];

const ELEMENTS_FULLSCREEN_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--display-id <ID>",
    about: "Display hint for the query (currently best-effort on macOS)",
}];

const WINDOW_LIST_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--app <NAME>",
    about: "Filter by application name or bundle ID (optional)",
}];

const WINDOW_FOCUS_OPTION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--window-id <ID>",
    about: "ID of the window to focus (from 'operator window list')",
}];

const WINDOW_MOVE_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--x <X>",
        about: "New left edge position in screen points",
    },
    CommandHelpEntry {
        command: "--y <Y>",
        about: "New top edge position in screen points",
    },
];

const WINDOW_RESIZE_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--width <W>",
        about: "New width in screen points",
    },
    CommandHelpEntry {
        command: "--height <H>",
        about: "New height in screen points",
    },
];

const WINDOW_SET_BOUNDS_OPTION_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--x <X>",
        about: "New left edge position in screen points",
    },
    CommandHelpEntry {
        command: "--y <Y>",
        about: "New top edge position in screen points",
    },
    CommandHelpEntry {
        command: "--width <W>",
        about: "New width in screen points",
    },
    CommandHelpEntry {
        command: "--height <H>",
        about: "New height in screen points",
    },
];

const INPUT_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--text <TEXT>",
        about: "Match element by visible text",
    },
    CommandHelpEntry {
        command: "--role <ROLE> [--index <N>]",
        about: "Match element by accessibility role",
    },
    CommandHelpEntry {
        command: "--snapshot <ID> --element <ELEM-ID>",
        about: "Match element by snapshot reference",
    },
    CommandHelpEntry {
        command: "--x <X> --y <Y>",
        about: "Match by screen coordinates",
    },
];

const SCROLL_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--text <TEXT>",
        about: "Scroll near element with this text",
    },
    CommandHelpEntry {
        command: "--role <ROLE> [--index <N>]",
        about: "Scroll near element with this role",
    },
    CommandHelpEntry {
        command: "--snapshot <ID> --element <ELEM-ID>",
        about: "Scroll near element from snapshot",
    },
    CommandHelpEntry {
        command: "--x <X> --y <Y>",
        about: "Scroll at screen coordinates",
    },
];

const MOVE_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--text <TEXT>",
        about: "Move to element with this text",
    },
    CommandHelpEntry {
        command: "--role <ROLE> [--index <N>]",
        about: "Move to element with this role",
    },
    CommandHelpEntry {
        command: "--snapshot <ID> --element <ELEM-ID>",
        about: "Move to element from snapshot",
    },
    CommandHelpEntry {
        command: "--x <X> --y <Y>",
        about: "Move to screen coordinates",
    },
];

const DRAG_FROM_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--from-text <TEXT>",
        about: "Drag from element with this text",
    },
    CommandHelpEntry {
        command: "--from-role <ROLE> [--from-index <N>]",
        about: "Drag from element with this role",
    },
    CommandHelpEntry {
        command: "--from-snapshot <ID> --from-element <ELEM-ID>",
        about: "Drag from element in snapshot",
    },
    CommandHelpEntry {
        command: "--from-x <X> --from-y <Y>",
        about: "Drag from screen coordinates",
    },
];

const DRAG_TO_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--to-text <TEXT>",
        about: "Drag to element with this text",
    },
    CommandHelpEntry {
        command: "--to-role <ROLE> [--to-index <N>]",
        about: "Drag to element with this role",
    },
    CommandHelpEntry {
        command: "--to-snapshot <ID> --to-element <ELEM-ID>",
        about: "Drag to element in snapshot",
    },
    CommandHelpEntry {
        command: "--to-x <X> --to-y <Y>",
        about: "Drag to screen coordinates",
    },
];

const SWIPE_FROM_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--from-text <TEXT>",
        about: "Swipe from element with this text",
    },
    CommandHelpEntry {
        command: "--from-role <ROLE> [--from-index <N>]",
        about: "Swipe from element with this role",
    },
    CommandHelpEntry {
        command: "--from-snapshot <ID> --from-element <ELEM-ID>",
        about: "Swipe from element in snapshot",
    },
    CommandHelpEntry {
        command: "--from-x <X> --from-y <Y>",
        about: "Swipe from screen coordinates",
    },
];

const SWIPE_TO_LOCATOR_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--to-text <TEXT>",
        about: "Swipe to element with this text",
    },
    CommandHelpEntry {
        command: "--to-role <ROLE> [--to-index <N>]",
        about: "Swipe to element with this role",
    },
    CommandHelpEntry {
        command: "--to-snapshot <ID> --to-element <ELEM-ID>",
        about: "Swipe to element in snapshot",
    },
    CommandHelpEntry {
        command: "--to-x <X> --to-y <Y>",
        about: "Swipe to screen coordinates",
    },
];

const OPTIONAL_ACTION_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Target application by name or bundle ID",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Target window by ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Target window by title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Target window by index within the app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Target process by PID",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const APP_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Application name or bundle ID",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Window ID belonging to the application",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Window title belonging to the application",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Process ID",
    },
];

const WINDOW_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Target the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Target the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Target the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Target the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Target the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const WINDOW_CLOSE_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Close the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Close the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Close the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Close the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Close the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const WINDOW_MINIMIZE_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Minimize the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Minimize the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Minimize the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Minimize the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Minimize the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const WINDOW_MAXIMIZE_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Maximize the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Maximize the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Maximize the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Maximize the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Maximize the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const WINDOW_MOVE_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Move the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Move the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Move the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Move the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Move the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const WINDOW_RESIZE_TARGET_ROWS: &[CommandHelpEntry] = &[
    CommandHelpEntry {
        command: "--app <NAME>",
        about: "Resize the frontmost window of this app",
    },
    CommandHelpEntry {
        command: "--window-id <ID>",
        about: "Resize the window with this ID",
    },
    CommandHelpEntry {
        command: "--window-title <TITLE>",
        about: "Resize the window matching this title",
    },
    CommandHelpEntry {
        command: "--window-index <N>",
        about: "Resize the Nth window of the target app",
    },
    CommandHelpEntry {
        command: "--pid <PID>",
        about: "Resize the frontmost window of this process",
    },
    CommandHelpEntry {
        command: "--focus auto|never",
        about: "Window focus policy before action (default: auto)",
    },
];

const VERIFICATION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--verify focus|window-state|geometry",
    about: "Post-action verification (repeatable)",
}];

const WINDOW_STATE_VERIFICATION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--verify window-state",
    about: "Verify the window is minimized after the action",
}];

const GEOMETRY_VERIFICATION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--verify geometry",
    about: "Verify the window geometry after the action",
}];

const WINDOW_MOVE_VERIFICATION_ROWS: &[CommandHelpEntry] = &[CommandHelpEntry {
    command: "--verify geometry",
    about: "Verify the window position after the action",
}];

const SURFACE_WINDOW_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Options",
    rows: SURFACE_WINDOW_OPTION_ROWS,
}];

const SURFACE_REGION_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Options",
    rows: SURFACE_REGION_OPTION_ROWS,
}];

const CAPTURE_FULLSCREEN_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Options",
    rows: CAPTURE_FULLSCREEN_OPTION_ROWS,
}];

const ELEMENTS_FULLSCREEN_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Options",
    rows: ELEMENTS_FULLSCREEN_OPTION_ROWS,
}];

const SNAPSHOT_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Arguments",
    rows: SNAPSHOT_ARGUMENT_ROWS,
}];

const ARTIFACT_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Arguments",
    rows: ARTIFACT_ARGUMENT_ROWS,
}];

const CLICK_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: CLICK_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Locator (pick one group)",
        rows: INPUT_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const TYPE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Arguments",
        rows: TYPE_ARGUMENT_ROWS,
    },
    LeafHelpSection {
        heading: "Options",
        rows: TYPE_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Locator (pick one group)",
        rows: INPUT_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const PRESS_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Arguments",
        rows: PRESS_ARGUMENT_ROWS,
    },
    LeafHelpSection {
        heading: "Options",
        rows: PRESS_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const HOTKEY_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Arguments",
        rows: HOTKEY_ARGUMENT_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const SCROLL_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: SCROLL_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Locator (pick one group)",
        rows: SCROLL_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
];

const DRAG_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "From Locator (pick one group, required)",
        rows: DRAG_FROM_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "To Locator (pick one group, required)",
        rows: DRAG_TO_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Options",
        rows: DRAG_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
];

const SWIPE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "From Locator (pick one group, required)",
        rows: SWIPE_FROM_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "To Locator (pick one group, required)",
        rows: SWIPE_TO_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Options",
        rows: SWIPE_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
];

const MOVE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Locator (pick one group, required)",
        rows: MOVE_LOCATOR_ROWS,
    },
    LeafHelpSection {
        heading: "Target (optional, defaults to frontmost)",
        rows: OPTIONAL_ACTION_TARGET_ROWS,
    },
];

const APP_LAUNCH_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Arguments",
    rows: APP_LAUNCH_ARGUMENT_ROWS,
}];

const APP_SWITCH_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: APP_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const APP_LIFECYCLE_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Target (pick one, required)",
    rows: APP_TARGET_ROWS,
}];

const APP_QUIT_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: APP_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const WINDOW_LIST_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Options",
    rows: WINDOW_LIST_OPTION_ROWS,
}];

const WINDOW_FOCUS_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: WINDOW_FOCUS_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: VERIFICATION_ROWS,
    },
];

const WINDOW_CLOSE_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Target (pick one, required)",
    rows: WINDOW_CLOSE_TARGET_ROWS,
}];

const WINDOW_MINIMIZE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: WINDOW_MINIMIZE_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: WINDOW_STATE_VERIFICATION_ROWS,
    },
];

const WINDOW_MAXIMIZE_HELP_SECTIONS: &[LeafHelpSection] = &[LeafHelpSection {
    heading: "Target (pick one, required)",
    rows: WINDOW_MAXIMIZE_TARGET_ROWS,
}];

const WINDOW_MOVE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: WINDOW_MOVE_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: WINDOW_MOVE_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: WINDOW_MOVE_VERIFICATION_ROWS,
    },
];

const WINDOW_RESIZE_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: WINDOW_RESIZE_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: WINDOW_RESIZE_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: GEOMETRY_VERIFICATION_ROWS,
    },
];

const WINDOW_SET_BOUNDS_HELP_SECTIONS: &[LeafHelpSection] = &[
    LeafHelpSection {
        heading: "Options",
        rows: WINDOW_SET_BOUNDS_OPTION_ROWS,
    },
    LeafHelpSection {
        heading: "Target (pick one, required)",
        rows: WINDOW_TARGET_ROWS,
    },
    LeafHelpSection {
        heading: "Verification",
        rows: GEOMETRY_VERIFICATION_ROWS,
    },
];

const APP_SWITCH_HELP_ABOUT: &str =
    "Bring an application to the foreground. Switches to the app's frontmost window.\nUse 'operator window focus' to target a specific window within the app.";
const PRESS_HELP_ABOUT: &str = "Press a single key, optionally multiple times";
const SCROLL_HELP_ABOUT: &str = "Scroll by delta at a locator or target";
const DRAG_HELP_ABOUT: &str = "Drag from one locator to another";
const SWIPE_HELP_ABOUT: &str = "Swipe from one locator to another";
const MOVE_HELP_ABOUT: &str = "Move the pointer to a locator or coordinates without clicking";
const APP_HIDE_HELP_ABOUT: &str = "Hide an application (remove from screen without quitting)";

const PERMISSIONS_HELP: LeafHelp = LeafHelp {
    usage: "operator permissions [OPTIONS]",
    about: PERMISSIONS_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &["operator permissions", "operator --json permissions"],
    footer: "",
};

const CAPABILITIES_HELP: LeafHelp = LeafHelp {
    usage: "operator capabilities [OPTIONS]",
    about: CAPABILITIES_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &["operator capabilities", "operator --json capabilities"],
    footer: "",
};

const SHOW_HELP: LeafHelp = LeafHelp {
    usage: "operator show [OPTIONS]",
    about: SHOW_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &["operator show", "operator --json show"],
    footer: "",
};

const CAPTURE_FRONTMOST_HELP: LeafHelp = LeafHelp {
    usage: "operator capture frontmost [OPTIONS]",
    about: CAPTURE_FRONTMOST_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &[
        "operator capture frontmost",
        "operator --json capture frontmost",
    ],
    footer: "",
};

const CAPTURE_WINDOW_HELP: LeafHelp = LeafHelp {
    usage: "operator capture window [OPTIONS] --window-id <ID>",
    about: CAPTURE_WINDOW_ABOUT,
    sections: SURFACE_WINDOW_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator capture window --window-id 42",
        "operator --json capture window --window-id 42",
    ],
    footer: "",
};

const CAPTURE_REGION_HELP: LeafHelp = LeafHelp {
    usage: "operator capture region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
    about: CAPTURE_REGION_ABOUT,
    sections: SURFACE_REGION_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator capture region --x 0 --y 0 --width 800 --height 600",
        "operator capture region --x 100 --y 200 --width 400 --height 300",
    ],
    footer: "",
};

const CAPTURE_FULLSCREEN_HELP: LeafHelp = LeafHelp {
    usage: "operator capture fullscreen [OPTIONS]",
    about: CAPTURE_FULLSCREEN_ABOUT,
    sections: CAPTURE_FULLSCREEN_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator capture fullscreen",
        "operator capture fullscreen --display-id 2",
    ],
    footer: "",
};

const ELEMENTS_FRONTMOST_HELP: LeafHelp = LeafHelp {
    usage: "operator elements frontmost [OPTIONS]",
    about: ELEMENTS_FRONTMOST_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &[
        "operator elements frontmost",
        "operator --json elements frontmost",
    ],
    footer: "",
};

const ELEMENTS_WINDOW_HELP: LeafHelp = LeafHelp {
    usage: "operator elements window [OPTIONS] --window-id <ID>",
    about: ELEMENTS_WINDOW_ABOUT,
    sections: SURFACE_WINDOW_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator elements window --window-id 42",
        "operator --json elements window --window-id 42",
    ],
    footer: "",
};

const ELEMENTS_REGION_HELP: LeafHelp = LeafHelp {
    usage: "operator elements region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
    about: ELEMENTS_REGION_ABOUT,
    sections: SURFACE_REGION_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &["operator elements region --x 0 --y 0 --width 800 --height 600"],
    footer: ELEMENTS_REGION_FOOTER,
};

const ELEMENTS_FULLSCREEN_HELP: LeafHelp = LeafHelp {
    usage: "operator elements fullscreen [OPTIONS]",
    about: ELEMENTS_FULLSCREEN_ABOUT,
    sections: ELEMENTS_FULLSCREEN_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &["operator elements fullscreen"],
    footer: ELEMENTS_FULLSCREEN_FOOTER,
};

const SNAPSHOT_HELP: LeafHelp = LeafHelp {
    usage: "operator snapshot [OPTIONS] <SNAPSHOT-ID>",
    about: SNAPSHOT_ABOUT,
    sections: SNAPSHOT_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator snapshot s_abc123",
        "operator --json snapshot s_abc123",
    ],
    footer: "",
};

const ARTIFACT_HELP: LeafHelp = LeafHelp {
    usage: "operator artifact [OPTIONS] <ARTIFACT-ID>",
    about: ARTIFACT_ABOUT,
    sections: ARTIFACT_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator artifact capture-1.png",
        "operator --json artifact capture-1.png",
    ],
    footer: "",
};

const CLICK_HELP: LeafHelp = LeafHelp {
    usage: "operator click [OPTIONS]",
    about: INPUT_CLICK_ABOUT,
    sections: CLICK_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator click --text Save",
        "operator click --text \"Open File\" --app Finder --verify focus",
        "operator click --snapshot s_abc123 --element e_7",
        "operator click --x 200 --y 400",
        "operator click --role button --index 2 --mode double",
    ],
    footer: "",
};

const TYPE_HELP: LeafHelp = LeafHelp {
    usage: "operator type [OPTIONS] <TEXT>",
    about: INPUT_TYPE_ABOUT,
    sections: TYPE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator type \"hello world\"",
        "operator type \"search query\" --text \"Search...\" --after-key return",
        "operator type \"new content\" --role textField --clear-before",
        "operator type \"slow input\" --delay-ms 50",
    ],
    footer: "",
};

const PRESS_HELP: LeafHelp = LeafHelp {
    usage: "operator press [OPTIONS] <KEY>",
    about: PRESS_HELP_ABOUT,
    sections: PRESS_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator press return",
        "operator press escape --app Notes",
        "operator press tab --count 3",
    ],
    footer: "",
};

const HOTKEY_HELP: LeafHelp = LeafHelp {
    usage: "operator hotkey [OPTIONS] <KEY>...",
    about: INPUT_HOTKEY_ABOUT,
    sections: HOTKEY_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator hotkey command s",
        "operator hotkey command shift z --app TextEdit",
        "operator hotkey control c",
    ],
    footer: "",
};

const SCROLL_HELP: LeafHelp = LeafHelp {
    usage: "operator scroll [OPTIONS] --delta-x <DX> --delta-y <DY>",
    about: SCROLL_HELP_ABOUT,
    sections: SCROLL_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator scroll --delta-x 0 --delta-y 300",
        "operator scroll --delta-x 0 --delta-y -200 --app Safari",
        "operator scroll --delta-x 0 --delta-y 100 --x 400 --y 500",
    ],
    footer: "",
};

const DRAG_HELP: LeafHelp = LeafHelp {
    usage: "operator drag [OPTIONS]",
    about: DRAG_HELP_ABOUT,
    sections: DRAG_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator drag --from-text \"file.txt\" --to-text \"Documents\"",
        "operator drag --from-x 100 --from-y 200 --to-x 400 --to-y 500",
        "operator drag --from-snapshot s_abc123 --from-element e_3 --to-snapshot s_abc123 --to-element e_9",
        "operator drag --from-x 100 --from-y 200 --to-x 400 --to-y 500 --duration-ms 500 --steps 20",
    ],
    footer: "",
};

const SWIPE_HELP: LeafHelp = LeafHelp {
    usage: "operator swipe [OPTIONS]",
    about: SWIPE_HELP_ABOUT,
    sections: SWIPE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator swipe --from-x 200 --from-y 500 --to-x 200 --to-y 100",
        "operator swipe --from-x 100 --from-y 300 --to-x 600 --to-y 300 --duration-ms 300",
    ],
    footer: "",
};

const MOVE_HELP: LeafHelp = LeafHelp {
    usage: "operator move [OPTIONS]",
    about: MOVE_HELP_ABOUT,
    sections: MOVE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator move --text \"Submit\"",
        "operator move --x 400 --y 300",
        "operator move --role button --index 1 --app Safari",
    ],
    footer: "",
};

const APP_LIST_HELP: LeafHelp = LeafHelp {
    usage: "operator app list [OPTIONS]",
    about: APP_LIST_ABOUT,
    sections: &[],
    include_global_runtime_flags: true,
    examples: &["operator app list", "operator --json app list"],
    footer: APP_LIST_FOOTER,
};

const APP_LAUNCH_HELP: LeafHelp = LeafHelp {
    usage: "operator app launch [OPTIONS] <APP>",
    about: APP_LAUNCH_ABOUT,
    sections: APP_LAUNCH_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator app launch Notes",
        "operator app launch com.apple.TextEdit",
    ],
    footer: "",
};

const APP_SWITCH_HELP: LeafHelp = LeafHelp {
    usage: "operator app switch [OPTIONS]",
    about: APP_SWITCH_HELP_ABOUT,
    sections: APP_SWITCH_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator app switch --app TextEdit",
        "operator app switch --app Safari --verify focus",
    ],
    footer: "",
};

const APP_QUIT_HELP: LeafHelp = LeafHelp {
    usage: "operator app quit [OPTIONS]",
    about: APP_QUIT_ABOUT,
    sections: APP_QUIT_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator app quit --app Notes",
        "operator app quit --pid 1234",
    ],
    footer: "",
};

const APP_HIDE_HELP: LeafHelp = LeafHelp {
    usage: "operator app hide [OPTIONS]",
    about: APP_HIDE_HELP_ABOUT,
    sections: APP_LIFECYCLE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &["operator app hide --app Notes"],
    footer: "",
};

const APP_UNHIDE_HELP: LeafHelp = LeafHelp {
    usage: "operator app unhide [OPTIONS]",
    about: APP_UNHIDE_ABOUT,
    sections: APP_LIFECYCLE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &["operator app unhide --app Notes"],
    footer: "",
};

const APP_RELAUNCH_HELP: LeafHelp = LeafHelp {
    usage: "operator app relaunch [OPTIONS]",
    about: APP_RELAUNCH_ABOUT,
    sections: APP_LIFECYCLE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &["operator app relaunch --app Notes"],
    footer: "",
};

const WINDOW_LIST_HELP: LeafHelp = LeafHelp {
    usage: "operator window list [OPTIONS]",
    about: WINDOW_LIST_ABOUT,
    sections: WINDOW_LIST_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window list",
        "operator window list --app TextEdit",
        "operator --json window list",
    ],
    footer: WINDOW_LIST_FOOTER,
};

const WINDOW_FOCUS_HELP: LeafHelp = LeafHelp {
    usage: "operator window focus [OPTIONS] --window-id <ID>",
    about: WINDOW_FOCUS_ABOUT,
    sections: WINDOW_FOCUS_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window focus --window-id 42",
        "operator window focus --window-id 42 --verify focus",
    ],
    footer: "",
};

const WINDOW_CLOSE_HELP: LeafHelp = LeafHelp {
    usage: "operator window close [OPTIONS]",
    about: WINDOW_CLOSE_ABOUT,
    sections: WINDOW_CLOSE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window close --window-id 42",
        "operator window close --app TextEdit",
    ],
    footer: "",
};

const WINDOW_MINIMIZE_HELP: LeafHelp = LeafHelp {
    usage: "operator window minimize [OPTIONS]",
    about: WINDOW_MINIMIZE_ABOUT,
    sections: WINDOW_MINIMIZE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window minimize --window-id 42",
        "operator window minimize --app Notes --verify window-state",
    ],
    footer: "",
};

const WINDOW_MAXIMIZE_HELP: LeafHelp = LeafHelp {
    usage: "operator window maximize [OPTIONS]",
    about: WINDOW_MAXIMIZE_ABOUT,
    sections: WINDOW_MAXIMIZE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window maximize --window-id 42",
        "operator window maximize --app TextEdit",
    ],
    footer: "",
};

const WINDOW_MOVE_HELP: LeafHelp = LeafHelp {
    usage: "operator window move [OPTIONS] --x <X> --y <Y>",
    about: WINDOW_MOVE_ABOUT,
    sections: WINDOW_MOVE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window move --window-id 42 --x 100 --y 50",
        "operator window move --app TextEdit --x 0 --y 0 --verify geometry",
    ],
    footer: "",
};

const WINDOW_RESIZE_HELP: LeafHelp = LeafHelp {
    usage: "operator window resize [OPTIONS] --width <W> --height <H>",
    about: WINDOW_RESIZE_ABOUT,
    sections: WINDOW_RESIZE_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window resize --window-id 42 --width 1280 --height 800",
        "operator window resize --app TextEdit --width 900 --height 600 --verify geometry",
    ],
    footer: "",
};

const WINDOW_SET_BOUNDS_HELP: LeafHelp = LeafHelp {
    usage: "operator window set-bounds [OPTIONS] --x <X> --y <Y> --width <W> --height <H>",
    about: WINDOW_SET_BOUNDS_ABOUT,
    sections: WINDOW_SET_BOUNDS_HELP_SECTIONS,
    include_global_runtime_flags: true,
    examples: &[
        "operator window set-bounds --window-id 42 --x 0 --y 0 --width 1280 --height 800",
        "operator window set-bounds --app Notes --x 100 --y 100 --width 800 --height 600 --verify geometry",
    ],
    footer: "",
};

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
    if !group.footer.is_empty() {
        help.push('\n');
        writeln!(
            &mut help,
            "{muted}{footer}{reset}",
            muted = MUTED_STYLE,
            footer = group.footer,
            reset = RESET_STYLE,
        )
        .expect("write footer");
    }

    help
}

fn styled_leaf_help(spec: &LeafHelp) -> String {
    let mut help = String::new();
    writeln!(
        &mut help,
        "{header}Usage{reset} {command}{usage}{reset}",
        header = HEADER_STYLE,
        command = COMMAND_STYLE,
        usage = spec.usage,
        reset = RESET_STYLE,
    )
    .expect("write usage");
    help.push('\n');
    writeln!(
        &mut help,
        "{body}{description}{reset}",
        body = BODY_STYLE,
        description = spec.about,
        reset = RESET_STYLE,
    )
    .expect("write description");
    help.push('\n');

    for section in spec.sections {
        writeln!(
            &mut help,
            "{header}{heading}{reset}",
            header = HEADER_STYLE,
            heading = section.heading,
            reset = RESET_STYLE,
        )
        .expect("write section header");
        write_command_rows(&mut help, section.rows);
        help.push('\n');
    }

    if spec.include_global_runtime_flags {
        help.push_str(&styled_global_runtime_flags());
        help.push('\n');
    }

    writeln!(
        &mut help,
        "{header}Examples{reset}",
        header = HEADER_STYLE,
        reset = RESET_STYLE,
    )
    .expect("write examples header");
    for example in spec.examples {
        writeln!(
            &mut help,
            "  {command}{example}{reset}",
            command = COMMAND_STYLE,
            example = example,
            reset = RESET_STYLE,
        )
        .expect("write example row");
    }

    if !spec.footer.is_empty() {
        help.push('\n');
        writeln!(
            &mut help,
            "{muted}{footer}{reset}",
            muted = MUTED_STYLE,
            footer = spec.footer,
            reset = RESET_STYLE,
        )
        .expect("write footer");
    }

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
        if section.commands.is_empty() {
            continue;
        }
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
        ["list", "apps", ..] => Some(("list apps".into(), "operator app list".into())),
        ["list", "windows", ..] => Some(("list windows".into(), "operator window list".into())),
        ["list", ..] => Some((
            "list".into(),
            "operator app list or operator window list".into(),
        )),
        ["list-apps", ..] => Some(("list-apps".into(), "operator app list".into())),
        ["list-windows", ..] => Some(("list-windows".into(), "operator window list".into())),
        ["permissions-status", ..] => {
            Some(("permissions-status".into(), "operator permissions".into()))
        }
        ["input"] => Some((
            "input".into(),
            "operator click, operator type, operator press, operator hotkey, operator scroll, operator drag, operator swipe, or operator move".into(),
        )),
        ["input", "click", ..] => Some(("input click".into(), "operator click".into())),
        ["input", "move", ..] => Some(("input move".into(), "operator move".into())),
        ["input", "type", ..] => Some(("input type".into(), "operator type".into())),
        ["input", "press", ..] => Some(("input press".into(), "operator press".into())),
        ["input", "hotkey", ..] => Some(("input hotkey".into(), "operator hotkey".into())),
        ["input", "scroll", ..] => Some(("input scroll".into(), "operator scroll".into())),
        ["input", "drag", ..] => Some(("input drag".into(), "operator drag".into())),
        ["input", "swipe", ..] => Some(("input swipe".into(), "operator swipe".into())),
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
        format!(
            "legacy command path `operator {legacy}` has been removed; use `{replacement}` instead"
        ),
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
        ["permissions", ..] => Some(styled_leaf_help(&PERMISSIONS_HELP)),
        ["capabilities", ..] => Some(styled_leaf_help(&CAPABILITIES_HELP)),
        ["capture"] => Some(styled_group_help(&CAPTURE_GROUP_HELP)),
        ["capture", "frontmost", ..] => Some(styled_leaf_help(&CAPTURE_FRONTMOST_HELP)),
        ["capture", "window", ..] => Some(styled_leaf_help(&CAPTURE_WINDOW_HELP)),
        ["capture", "region", ..] => Some(styled_leaf_help(&CAPTURE_REGION_HELP)),
        ["capture", "fullscreen", ..] => Some(styled_leaf_help(&CAPTURE_FULLSCREEN_HELP)),
        ["elements"] => Some(styled_group_help(&ELEMENTS_GROUP_HELP)),
        ["elements", "frontmost", ..] => Some(styled_leaf_help(&ELEMENTS_FRONTMOST_HELP)),
        ["elements", "window", ..] => Some(styled_leaf_help(&ELEMENTS_WINDOW_HELP)),
        ["elements", "region", ..] => Some(styled_leaf_help(&ELEMENTS_REGION_HELP)),
        ["elements", "fullscreen", ..] => Some(styled_leaf_help(&ELEMENTS_FULLSCREEN_HELP)),
        ["snapshot", ..] => Some(styled_leaf_help(&SNAPSHOT_HELP)),
        ["artifact", ..] => Some(styled_leaf_help(&ARTIFACT_HELP)),
        ["show", ..] => Some(styled_leaf_help(&SHOW_HELP)),
        ["click", ..] => Some(styled_leaf_help(&CLICK_HELP)),
        ["type", ..] => Some(styled_leaf_help(&TYPE_HELP)),
        ["press", ..] => Some(styled_leaf_help(&PRESS_HELP)),
        ["hotkey", ..] => Some(styled_leaf_help(&HOTKEY_HELP)),
        ["scroll", ..] => Some(styled_leaf_help(&SCROLL_HELP)),
        ["drag", ..] => Some(styled_leaf_help(&DRAG_HELP)),
        ["swipe", ..] => Some(styled_leaf_help(&SWIPE_HELP)),
        ["move", ..] => Some(styled_leaf_help(&MOVE_HELP)),
        ["app"] => Some(styled_group_help(&APP_GROUP_HELP)),
        ["app", "list", ..] => Some(styled_leaf_help(&APP_LIST_HELP)),
        ["app", "launch", ..] => Some(styled_leaf_help(&APP_LAUNCH_HELP)),
        ["app", "switch", ..] => Some(styled_leaf_help(&APP_SWITCH_HELP)),
        ["app", "quit", ..] => Some(styled_leaf_help(&APP_QUIT_HELP)),
        ["app", "hide", ..] => Some(styled_leaf_help(&APP_HIDE_HELP)),
        ["app", "unhide", ..] => Some(styled_leaf_help(&APP_UNHIDE_HELP)),
        ["app", "relaunch", ..] => Some(styled_leaf_help(&APP_RELAUNCH_HELP)),
        ["window"] => Some(styled_group_help(&WINDOW_GROUP_HELP)),
        ["window", "list", ..] => Some(styled_leaf_help(&WINDOW_LIST_HELP)),
        ["window", "focus", ..] => Some(styled_leaf_help(&WINDOW_FOCUS_HELP)),
        ["window", "close", ..] => Some(styled_leaf_help(&WINDOW_CLOSE_HELP)),
        ["window", "minimize", ..] => Some(styled_leaf_help(&WINDOW_MINIMIZE_HELP)),
        ["window", "maximize", ..] => Some(styled_leaf_help(&WINDOW_MAXIMIZE_HELP)),
        ["window", "move", ..] => Some(styled_leaf_help(&WINDOW_MOVE_HELP)),
        ["window", "resize", ..] => Some(styled_leaf_help(&WINDOW_RESIZE_HELP)),
        ["window", "set-bounds", ..] => Some(styled_leaf_help(&WINDOW_SET_BOUNDS_HELP)),
        ["mcp", "serve", ..] => Some(styled_leaf_help(&MCP_SERVE_HELP)),
        ["mcp"] => Some(styled_group_help(&MCP_GROUP_HELP)),
        ["agent", ..] => Some(styled_leaf_help(&AGENT_HELP)),
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
    #[command(about = SHOW_ABOUT, after_help = SHOW_AFTER_HELP)]
    Show(CommonArgs),
    #[command(about = INPUT_CLICK_ABOUT, after_help = INPUT_CLICK_AFTER_HELP)]
    Click(InputClickArgs),
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
    #[command(about = INPUT_MOVE_ABOUT)]
    Move(InputMoveArgs),
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
            Self::Show(args) => Some(args),
            Self::Click(args) => Some(&args.common),
            Self::Type(args) => Some(&args.common),
            Self::Press(args) => Some(&args.common),
            Self::Hotkey(args) => Some(&args.common),
            Self::Scroll(args) => Some(&args.common),
            Self::Drag(args) => Some(&args.common),
            Self::Swipe(args) => Some(&args.common),
            Self::Move(args) => Some(&args.common),
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
            Self::Show(common) => {
                invoke_without_specific_input("get-focus", merge_common(root_common, common))
            }
            Self::Click(args) => args.into_invocation(root_common),
            Self::Type(args) => args.into_invocation(root_common),
            Self::Press(args) => args.into_invocation(root_common),
            Self::Hotkey(args) => args.into_invocation(root_common),
            Self::Scroll(args) => args.into_invocation(root_common),
            Self::Drag(args) => args.into_invocation(root_common),
            Self::Swipe(args) => args.into_invocation(root_common),
            Self::Move(args) => args.into_invocation(root_common),
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
    #[arg(help = "Natural-language description of the task to perform")]
    task: String,
    #[arg(
        long,
        value_parser = ["gpt-5.4", "doubao-seed"],
        help = "Model to use for the agent"
    )]
    model: Option<String>,
    #[arg(long, help = "Maximum number of agent steps before stopping")]
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
    #[command(about = WINDOW_LIST_ABOUT, after_help = WINDOW_LIST_AFTER_HELP)]
    List(WindowListArgs),
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
            Self::List(args) => args.into_invocation(common),
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
    #[command(about = APP_LIST_ABOUT, after_help = APP_LIST_AFTER_HELP)]
    List(AppListArgs),
    #[command(about = APP_LAUNCH_ABOUT, after_help = APP_LAUNCH_AFTER_HELP)]
    Launch(AppLaunchArgs),
    #[command(about = APP_SWITCH_ABOUT, after_help = APP_SWITCH_AFTER_HELP)]
    Switch(AppLifecycleVerifiedArgs),
    #[command(about = APP_QUIT_ABOUT)]
    Quit(AppLifecycleVerifiedArgs),
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
            Self::List(args) => args.into_invocation(common),
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
struct WindowListArgs {
    #[arg(long)]
    app: Option<String>,
}

impl WindowListArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
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

#[derive(Debug, Clone, Args, Default)]
struct AppListArgs {}

impl AppListArgs {
    fn into_invocation(self, common: CommonArgs) -> Result<ToolInvocation, String> {
        invoke_without_specific_input("list-apps", common)
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
}

impl AppLifecycleArgs {
    fn into_invocation(
        self,
        tool: &'static str,
        common: CommonArgs,
    ) -> Result<ToolInvocation, String> {
        lifecycle_action_invocation(tool, common, self.target, Vec::new())
    }
}

#[derive(Debug, Clone, Args)]
struct AppLifecycleVerifiedArgs {
    #[command(flatten)]
    target: LifecycleTargetArgs,
    #[command(flatten)]
    verification: ActionVerificationArgs,
}

impl AppLifecycleVerifiedArgs {
    fn into_invocation(
        self,
        tool: &'static str,
        common: CommonArgs,
    ) -> Result<ToolInvocation, String> {
        lifecycle_action_invocation(
            tool,
            common,
            self.target,
            self.verification.into_verifications(),
        )
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
    #[arg(long)]
    app: Option<String>,
    #[arg(long)]
    pid: Option<u32>,
    #[arg(long = "window-id")]
    window_id: Option<u64>,
    #[arg(long = "window-title")]
    window_title: Option<String>,
}

impl LifecycleTargetArgs {
    fn into_selector(self) -> Result<ActionTargetSelector, String> {
        let selector_count = [
            self.app.is_some(),
            self.pid.is_some(),
            self.window_id.is_some(),
            self.window_title.is_some(),
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();

        if selector_count > 1 {
            return Err("target selector flags are mutually exclusive".into());
        }

        if let Some(app) = self.app {
            Ok(ActionTargetSelector::App(app))
        } else if let Some(pid) = self.pid {
            Ok(ActionTargetSelector::Pid(pid))
        } else if let Some(window_id) = self.window_id {
            Ok(ActionTargetSelector::WindowId(WindowId::from(window_id)))
        } else if let Some(window_title) = self.window_title {
            Ok(ActionTargetSelector::WindowTitle(window_title))
        } else {
            Err("a target selector flag is required".to_string())
        }
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
    verifications: Vec<ActionVerification>,
) -> Result<ToolInvocation, String> {
    let mut input = common_input(&common);
    insert_serialized(&mut input, "target_selector", target.into_selector()?)?;
    insert_verifications(&mut input, verifications)?;
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
