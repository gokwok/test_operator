# Operator CLI Design

This document defines the target command structure, grouping, and `--help` text for the
`operator` CLI. It supersedes the current implementation where the two diverge.

Commands marked **[planned]** are not yet implemented.

---

## Root Help

```
Usage: operator [OPTIONS] <COMMAND>

Operator - Turn any desktop app into an API, from CLI to AI

Core
  permissions   Check automation permissions and runtime readiness
  capabilities  Show supported surfaces and actions for the active target
  snapshot      Read a stored snapshot by ID
  artifact      Read a stored capture artifact by ID

Observe
  capture       Take a screenshot of a surface
  elements      Query the accessibility element tree for a surface
  show          Show the currently focused app, window, and element

Interact
  click         Click a locator, coordinates, or target
  type          Type text into the focused or resolved target
  press         Press a single key
  hotkey        Press a key chord
  scroll        Scroll by delta against a locator or target
  drag          Drag from one locator to another
  swipe         Swipe from one locator to another
  move          Move the pointer to a locator or coordinates
  paste         Clipboard-aware paste [planned]

System
  app           Manage application lifecycle
  window        Manage application windows
  clipboard     Read/write the clipboard [planned]
  open          Open a URL or file with its default application [planned]

Integration
  mcp           Run the Operator MCP server

AI
  agent         Execute a natural-language task against a target

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture frontmost
  operator elements window --window-id 42
  operator click --text Save
  operator mcp serve

Use 'operator <command> --help' for detailed usage.
```

---

## Core

### `operator permissions`

```
Usage: operator permissions [OPTIONS]

Check automation permissions and runtime readiness

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator permissions
  operator --json permissions
```

### `operator capabilities`

```
Usage: operator capabilities [OPTIONS]

Show supported surfaces, queries, and actions for the active target

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capabilities
  operator --json capabilities
```

### `operator snapshot <SNAPSHOT-ID>`

```
Usage: operator snapshot [OPTIONS] <SNAPSHOT-ID>

Read a stored snapshot by ID

Arguments
  <SNAPSHOT-ID>   Snapshot identifier returned by a previous capture or elements command

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator snapshot s_abc123
  operator --json snapshot s_abc123
```

### `operator artifact <ARTIFACT-ID>`

```
Usage: operator artifact [OPTIONS] <ARTIFACT-ID>

Read a stored capture artifact by ID

Arguments
  <ARTIFACT-ID>   Artifact identifier (e.g. a screenshot filename) returned by a
                  previous capture command

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator artifact capture-1.png
  operator --json artifact capture-1.png
```

---

## Observe

### `operator capture`

```
Usage: operator capture [OPTIONS] <SURFACE>

Take a screenshot of the specified surface

Surfaces
  frontmost   The frontmost app and its windows
  window      A specific window by ID
  region      A screen region defined by coordinates
  fullscreen  The full display

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture frontmost
  operator capture window --window-id 42
  operator capture region --x 0 --y 0 --width 1440 --height 900
  operator capture fullscreen

Use 'operator capture <surface> --help' for detailed usage.
```

#### `operator capture frontmost`

```
Usage: operator capture frontmost [OPTIONS]

Take a screenshot of the frontmost app surface

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture frontmost
  operator --json capture frontmost
```

#### `operator capture window`

```
Usage: operator capture window [OPTIONS] --window-id <ID>

Take a screenshot of a specific window

Options
  --window-id <ID>   ID of the target window (from 'operator window list')

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture window --window-id 42
  operator --json capture window --window-id 42
```

#### `operator capture region`

```
Usage: operator capture region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>

Take a screenshot of a screen region defined by coordinates

Options
  --x <X>          Left edge of the region in screen points
  --y <Y>          Top edge of the region in screen points
  --width <W>      Width of the region in screen points
  --height <H>     Height of the region in screen points

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture region --x 0 --y 0 --width 800 --height 600
  operator capture region --x 100 --y 200 --width 400 --height 300
```

#### `operator capture fullscreen`

```
Usage: operator capture fullscreen [OPTIONS]

Take a screenshot of the full display

Options
  --display-id <ID>   Display to capture (optional, defaults to the active display)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator capture fullscreen
  operator capture fullscreen --display-id 2
```

---

### `operator elements`

```
Usage: operator elements [OPTIONS] <SURFACE>

Query the accessibility element tree for the specified surface.
Returns structured UI element data including roles, labels, and bounding boxes.

Surfaces
  frontmost   The frontmost app and its windows
  window      A specific window by ID
  region      A screen region defined by coordinates
  fullscreen  The full display

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator elements frontmost
  operator elements window --window-id 42
  operator elements region --x 0 --y 0 --width 1440 --height 900

Use 'operator elements <surface> --help' for detailed usage.
```

#### `operator elements frontmost`

```
Usage: operator elements frontmost [OPTIONS]

Query the accessibility element tree for the frontmost app surface

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator elements frontmost
  operator --json elements frontmost
```

#### `operator elements window`

```
Usage: operator elements window [OPTIONS] --window-id <ID>

Query the accessibility element tree for a specific window

Options
  --window-id <ID>   ID of the target window (from 'operator window list')

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator elements window --window-id 42
  operator --json elements window --window-id 42
```

#### `operator elements region`

```
Usage: operator elements region [OPTIONS] --x <X> --y <Y> --width <W> --height <H>

Query accessibility elements whose bounds intersect a screen region

Options
  --x <X>          Left edge of the region in screen points
  --y <Y>          Top edge of the region in screen points
  --width <W>      Width of the region in screen points
  --height <H>     Height of the region in screen points

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator elements region --x 0 --y 0 --width 800 --height 600
```

Current macOS note:
- `region` enumerates visible accessible windows and keeps only the element subtrees whose bounds intersect the requested rect.

#### `operator elements fullscreen`

```
Usage: operator elements fullscreen [OPTIONS]

Query accessibility elements across visible windows on the desktop

Options
  --display-id <ID>   Display hint for the query (currently best-effort on macOS)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator elements fullscreen
```

Current macOS note:
- `fullscreen` enumerates visible accessible windows on the desktop.
- `--display-id` is accepted for contract parity but does not yet narrow the macOS AX query.

---

### `operator show`

```
Usage: operator show [OPTIONS]

Show the currently focused app, window, and element

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator show
  operator --json show
```

---

## Interact

All interact commands share a common set of optional locator and target flags
for directing the action at a specific element or window.

**Locator flags** — pick at most one group:
- `--text <TEXT>` — match an element by its visible text label
- `--role <ROLE> [--index <N>]` — match by accessibility role; `--index` disambiguates
  when multiple elements share the same role (default: 0)
- `--snapshot <ID> --element <ELEM-ID>` — target a specific element from a prior snapshot
- `--x <X> --y <Y>` — target absolute screen coordinates

**Target flags** — pick at most one; directs which app/window receives the action:
- `--app <NAME>` — target by application name or bundle ID
- `--window-id <ID>` — target by window ID
- `--window-title <TITLE>` — target by window title substring
- `--window-index <N>` — target by window index within the app
- `--pid <PID>` — target by process ID

**Focus policy:**
- `--focus auto|never` — whether to bring the target window to front before acting
  (default: `auto`)

**Verification** (repeatable):
- `--verify focus` — assert the target has focus after the action
- `--verify window-state` — assert the window state matches expectations
- `--verify geometry` — assert the window geometry matches expectations

---

### `operator click`

```
Usage: operator click [OPTIONS]

Click a locator, coordinates, or target

Options
  --mode left|right|middle|double   Click mode (default: left)

Locator (pick one group)
  --text <TEXT>                         Match element by visible text
  --role <ROLE> [--index <N>]           Match element by accessibility role
  --snapshot <ID> --element <ELEM-ID>   Match element by snapshot reference
  --x <X> --y <Y>                       Match by screen coordinates

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator click --text Save
  operator click --text "Open File" --app Finder --verify focus
  operator click --snapshot s_abc123 --element e_7
  operator click --x 200 --y 400
  operator click --role button --index 2 --mode double
```

### `operator type`

```
Usage: operator type [OPTIONS] <TEXT>

Type text into the focused or resolved target

Arguments
  <TEXT>   Text to type

Options
  --clear-before              Clear the target field before typing
  --delay-ms <MS>             Delay between keystrokes in milliseconds
  --after-key return|tab|escape|delete   Key to press after typing (repeatable)

Locator (pick one group)
  --text <TEXT>                         Match element by visible text
  --role <ROLE> [--index <N>]           Match element by accessibility role
  --snapshot <ID> --element <ELEM-ID>   Match element by snapshot reference
  --x <X> --y <Y>                       Match by screen coordinates

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator type "hello world"
  operator type "search query" --text "Search..." --after-key return
  operator type "new content" --role textField --clear-before
  operator type "slow input" --delay-ms 50
```

### `operator press`

```
Usage: operator press [OPTIONS] <KEY>

Press a single key, optionally multiple times

Arguments
  <KEY>   Key name (e.g. return, escape, tab, space, f1, a, 0)

Options
  --count <N>   Number of times to press the key (default: 1)

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator press return
  operator press escape --app Notes
  operator press tab --count 3
```

### `operator hotkey`

```
Usage: operator hotkey [OPTIONS] <KEY>...

Press a key chord (modifier + key combination)

Arguments
  <KEY>...   Keys to press simultaneously (e.g. command s, control shift z)

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator hotkey command s
  operator hotkey command shift z --app TextEdit
  operator hotkey control c
```

### `operator scroll`

```
Usage: operator scroll [OPTIONS] --delta-x <DX> --delta-y <DY>

Scroll by delta at a locator or target

Options
  --delta-x <DX>   Horizontal scroll delta (positive = right, negative = left)
  --delta-y <DY>   Vertical scroll delta (positive = down, negative = up)

Locator (pick one group)
  --text <TEXT>                         Scroll near element with this text
  --role <ROLE> [--index <N>]           Scroll near element with this role
  --snapshot <ID> --element <ELEM-ID>   Scroll near element from snapshot
  --x <X> --y <Y>                       Scroll at screen coordinates

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator scroll --delta-x 0 --delta-y 300
  operator scroll --delta-x 0 --delta-y -200 --app Safari
  operator scroll --delta-x 0 --delta-y 100 --x 400 --y 500
```

### `operator drag`

```
Usage: operator drag [OPTIONS]

Drag from one locator to another

From Locator (pick one group, required)
  --from-text <TEXT>                                Drag from element with this text
  --from-role <ROLE> [--from-index <N>]             Drag from element with this role
  --from-snapshot <ID> --from-element <ELEM-ID>     Drag from element in snapshot
  --from-x <X> --from-y <Y>                         Drag from screen coordinates

To Locator (pick one group, required)
  --to-text <TEXT>                                  Drag to element with this text
  --to-role <ROLE> [--to-index <N>]                 Drag to element with this role
  --to-snapshot <ID> --to-element <ELEM-ID>         Drag to element in snapshot
  --to-x <X> --to-y <Y>                             Drag to screen coordinates

Options
  --duration-ms <MS>                   Duration of the drag gesture in milliseconds
  --steps <N>                          Number of interpolation steps along the drag path
  --modifier command|control|option|shift|function   Hold modifier key during drag (repeatable)

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator drag --from-text "file.txt" --to-text "Documents"
  operator drag --from-x 100 --from-y 200 --to-x 400 --to-y 500
  operator drag --from-snapshot s_abc123 --from-element e_3 --to-snapshot s_abc123 --to-element e_9
  operator drag --from-x 100 --from-y 200 --to-x 400 --to-y 500 --duration-ms 500 --steps 20
```

### `operator swipe`

```
Usage: operator swipe [OPTIONS]

Swipe from one locator to another

From Locator (pick one group, required)
  --from-text <TEXT>                                Swipe from element with this text
  --from-role <ROLE> [--from-index <N>]             Swipe from element with this role
  --from-snapshot <ID> --from-element <ELEM-ID>     Swipe from element in snapshot
  --from-x <X> --from-y <Y>                         Swipe from screen coordinates

To Locator (pick one group, required)
  --to-text <TEXT>                                  Swipe to element with this text
  --to-role <ROLE> [--to-index <N>]                 Swipe to element with this role
  --to-snapshot <ID> --to-element <ELEM-ID>         Swipe to element in snapshot
  --to-x <X> --to-y <Y>                             Swipe to screen coordinates

Options
  --duration-ms <MS>   Duration of the swipe gesture in milliseconds
  --steps <N>          Number of interpolation steps along the swipe path

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator swipe --from-x 200 --from-y 500 --to-x 200 --to-y 100
  operator swipe --from-x 100 --from-y 300 --to-x 600 --to-y 300 --duration-ms 300
```

### `operator move`

```
Usage: operator move [OPTIONS]

Move the pointer to a locator or coordinates without clicking

Locator (pick one group, required)
  --text <TEXT>                         Move to element with this text
  --role <ROLE> [--index <N>]           Move to element with this role
  --snapshot <ID> --element <ELEM-ID>   Move to element from snapshot
  --x <X> --y <Y>                       Move to screen coordinates

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --window-title <TITLE>     Target window by title
  --window-index <N>         Target window by index within the app
  --pid <PID>                Target process by PID
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator move --text "Submit"
  operator move --x 400 --y 300
  operator move --role button --index 1 --app Safari
```

### `operator paste` [planned]

```
Usage: operator paste [OPTIONS]

Paste text using the clipboard: saves current clipboard, sets the new content,
sends Cmd+V (or platform equivalent), then restores the previous clipboard.
Preferred over 'type' for large text or content with special characters.

Arguments
  <TEXT>   Text to paste

Target (optional, defaults to frontmost)
  --app <NAME>               Target application by name or bundle ID
  --window-id <ID>           Target window by ID
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator paste "Hello, world!"
  operator paste "large block of text" --app TextEdit
```

---

## System

### `operator app`

```
Usage: operator app [OPTIONS] <COMMAND>

Manage application lifecycle

Commands
  list      List running application processes
  launch    Launch an application
  switch    Bring an application to the foreground
  quit      Quit an application
  hide      Hide an application
  unhide    Unhide a hidden application
  relaunch  Quit and relaunch an application

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app list
  operator app launch Notes
  operator app switch --app TextEdit
  operator app quit --app Safari

Use 'operator app <command> --help' for detailed usage.
```

#### `operator app list`

```
Usage: operator app list [OPTIONS]

List operable applications

Mode (pick one)
  --running                  List operable applications that are currently running (default)
  --all                      List all operable applications visible to the target

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app list
  operator app list --running
  operator app list --all
  operator --json app list --all
```

Current macOS note:
- `app list` defaults to `app list --running`.
- `--running` returns operable running apps on macOS and filters out `.prohibited` background-only processes.
- `--all` scans installed app bundles, merges them with the running set, and marks non-running apps with `is_running = false` and no `pid`.

#### `operator app switch`

```
Usage: operator app switch [OPTIONS]

Bring an application to the foreground. Switches to the app's frontmost window.
Use 'operator window focus' to target a specific window within the app.

Target (pick one, required)
  --app <NAME>               Application name or bundle ID
  --window-id <ID>           Window ID belonging to the application
  --window-title <TITLE>     Window title belonging to the application
  --pid <PID>                Process ID

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app switch --app TextEdit
  operator app switch --app Safari --verify focus
```

#### `operator app launch`

```
Usage: operator app launch [OPTIONS] <APP>

Launch an application by name or bundle identifier

Arguments
  <APP>   Application name (e.g. Notes) or bundle ID (e.g. com.apple.Notes)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app launch Notes
  operator app launch com.apple.TextEdit
```

#### `operator app quit`

```
Usage: operator app quit [OPTIONS]

Quit an application

Target (pick one, required)
  --app <NAME>               Application name or bundle ID
  --window-id <ID>           Window ID belonging to the application
  --window-title <TITLE>     Window title belonging to the application
  --pid <PID>                Process ID

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app quit --app Notes
  operator app quit --pid 1234
```

#### `operator app hide`

```
Usage: operator app hide [OPTIONS]

Hide an application (remove from screen without quitting)

Target (pick one, required)
  --app <NAME>               Application name or bundle ID
  --window-id <ID>           Window ID belonging to the application
  --window-title <TITLE>     Window title belonging to the application
  --pid <PID>                Process ID

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app hide --app Notes
```

#### `operator app unhide`

```
Usage: operator app unhide [OPTIONS]

Unhide a previously hidden application

Target (pick one, required)
  --app <NAME>               Application name or bundle ID
  --window-id <ID>           Window ID belonging to the application
  --window-title <TITLE>     Window title belonging to the application
  --pid <PID>                Process ID

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app unhide --app Notes
```

#### `operator app relaunch`

```
Usage: operator app relaunch [OPTIONS]

Quit and relaunch an application

Target (pick one, required)
  --app <NAME>               Application name or bundle ID
  --window-id <ID>           Window ID belonging to the application
  --window-title <TITLE>     Window title belonging to the application
  --pid <PID>                Process ID

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator app relaunch --app Notes
```

---

### `operator window`

```
Usage: operator window [OPTIONS] <COMMAND>

Manage application windows

Commands
  list        List windows, optionally filtered by app
  focus       Bring a window to the foreground
  close       Close a window
  minimize    Minimize a window to the Dock
  maximize    Maximize a window to fill the display
  move        Move a window to new coordinates
  resize      Resize a window
  set-bounds  Set the full position and size of a window in one operation

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window list
  operator window list --app TextEdit
  operator window focus --window-id 42
  operator window resize --window-id 42 --width 1280 --height 800

Use 'operator window <command> --help' for detailed usage.
```

#### `operator window list`

```
Usage: operator window list [OPTIONS]

List application windows

Options
  --app <NAME>   Filter by application name or bundle ID (optional)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window list
  operator window list --app TextEdit
  operator --json window list
```

Current macOS note:
- `window list` without `--app` performs a full window enumeration and may be noticeably slower.
- Prefer `window list --app <APP>` when the target app is already known.

#### `operator window focus`

```
Usage: operator window focus [OPTIONS] --window-id <ID>

Bring a specific window to the foreground

Options
  --window-id <ID>   ID of the window to focus (from 'operator window list')

Verification
  --verify focus|window-state|geometry   Post-action verification (repeatable)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window focus --window-id 42
  operator window focus --window-id 42 --verify focus
```

#### `operator window close`

```
Usage: operator window close [OPTIONS]

Close a window

Target (pick one, required)
  --app <NAME>               Close the frontmost window of this app
  --window-id <ID>           Close the window with this ID
  --window-title <TITLE>     Close the window matching this title
  --window-index <N>         Close the Nth window of the target app
  --pid <PID>                Close the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window close --window-id 42
  operator window close --app TextEdit
```

#### `operator window minimize`

```
Usage: operator window minimize [OPTIONS]

Minimize a window to the Dock

Target (pick one, required)
  --app <NAME>               Minimize the frontmost window of this app
  --window-id <ID>           Minimize the window with this ID
  --window-title <TITLE>     Minimize the window matching this title
  --window-index <N>         Minimize the Nth window of the target app
  --pid <PID>                Minimize the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify window-state   Verify the window is minimized after the action

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window minimize --window-id 42
  operator window minimize --app Notes --verify window-state
```

#### `operator window maximize`

```
Usage: operator window maximize [OPTIONS]

Maximize a window to fill the display

Target (pick one, required)
  --app <NAME>               Maximize the frontmost window of this app
  --window-id <ID>           Maximize the window with this ID
  --window-title <TITLE>     Maximize the window matching this title
  --window-index <N>         Maximize the Nth window of the target app
  --pid <PID>                Maximize the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window maximize --window-id 42
  operator window maximize --app TextEdit
```

#### `operator window move`

```
Usage: operator window move [OPTIONS] --x <X> --y <Y>

Move a window to new screen coordinates

Options
  --x <X>   New left edge position in screen points
  --y <Y>   New top edge position in screen points

Target (pick one, required)
  --app <NAME>               Move the frontmost window of this app
  --window-id <ID>           Move the window with this ID
  --window-title <TITLE>     Move the window matching this title
  --window-index <N>         Move the Nth window of the target app
  --pid <PID>                Move the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify geometry   Verify the window position after the action

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window move --window-id 42 --x 100 --y 50
  operator window move --app TextEdit --x 0 --y 0 --verify geometry
```

#### `operator window resize`

```
Usage: operator window resize [OPTIONS] --width <W> --height <H>

Resize a window

Options
  --width <W>    New width in screen points
  --height <H>   New height in screen points

Target (pick one, required)
  --app <NAME>               Resize the frontmost window of this app
  --window-id <ID>           Resize the window with this ID
  --window-title <TITLE>     Resize the window matching this title
  --window-index <N>         Resize the Nth window of the target app
  --pid <PID>                Resize the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify geometry   Verify the window geometry after the action

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window resize --window-id 42 --width 1280 --height 800
  operator window resize --app TextEdit --width 900 --height 600 --verify geometry
```

#### `operator window set-bounds`

```
Usage: operator window set-bounds [OPTIONS] --x <X> --y <Y> --width <W> --height <H>

Set the full position and size of a window in one operation

Options
  --x <X>        New left edge position in screen points
  --y <Y>        New top edge position in screen points
  --width <W>    New width in screen points
  --height <H>   New height in screen points

Target (pick one, required)
  --app <NAME>               Target the frontmost window of this app
  --window-id <ID>           Target the window with this ID
  --window-title <TITLE>     Target the window matching this title
  --window-index <N>         Target the Nth window of the target app
  --pid <PID>                Target the frontmost window of this process
  --focus auto|never         Window focus policy before action (default: auto)

Verification
  --verify geometry   Verify the window bounds after the action

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator window set-bounds --window-id 42 --x 0 --y 0 --width 1280 --height 800
  operator window set-bounds --app Notes --x 100 --y 100 --width 800 --height 600 --verify geometry
```

---

### `operator clipboard` [planned]

```
Usage: operator clipboard [OPTIONS] <COMMAND>

Read or write the system clipboard

Commands
  get   Read the current clipboard content
  set   Write content to the clipboard

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator clipboard get
  operator clipboard set "text to copy"
```

### `operator open` [planned]

```
Usage: operator open [OPTIONS] <PATH-OR-URL>

Open a file or URL with its default (or specified) application

Arguments
  <PATH-OR-URL>   File path or URL to open

Options
  --app <NAME>   Open with this application instead of the default

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator open /Users/me/Documents/report.pdf
  operator open https://example.com
  operator open report.pdf --app Preview
```

---

## Integration

### `operator mcp`

```
Usage: operator mcp [OPTIONS] <COMMAND>

Run the Operator MCP server

Commands
  serve   Start the MCP stdio server

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator mcp serve
```

#### `operator mcp serve`

```
Usage: operator mcp serve [OPTIONS]

Start the MCP stdio server. Reads JSON-RPC messages from stdin and writes
responses to stdout. Intended to be launched by an MCP host such as Claude
Desktop or a custom integration.

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator mcp serve
```

---

## AI

### `operator agent`

```
Usage: operator agent [OPTIONS] <TASK>

Execute a natural-language task against the active target. The agent
observes the screen, plans actions, and drives the UI autonomously until
the task is complete or the step limit is reached.

Arguments
  <TASK>   Natural-language description of the task to perform

Options
  --model <MODEL>          Model to use for the agent
                           [possible values: gpt-5.4, doubao-seed]
  --max-steps <N>          Maximum number of agent steps before stopping

Global Runtime Flags
  --json                     Emit machine-readable JSON output
  --target <TARGET>          Select the named runtime target
  --timeout-ms <TIMEOUT_MS>  Override the runtime timeout for this command
  -h, --help                 Print help

Examples
  operator agent "Open Notes and type hello"
  operator agent "Find the largest file in Downloads and move it to the Trash"
  operator agent --model doubao-seed --max-steps 10 "Summarize the frontmost window"
```

---

## Migration from Previous Command Paths

The following legacy command paths are removed. Use the replacements below.

| Legacy | Replacement |
|--------|-------------|
| `operator observe frontmost` | `operator capture frontmost` |
| `operator observe window` | `operator capture window` |
| `operator observe region` | `operator capture region` |
| `operator observe fullscreen` | `operator capture fullscreen` |
| `operator observe frontmost --capture elements` | `operator elements frontmost` |
| `operator observe window --capture elements` | `operator elements window` |
| `operator list apps` | `operator app list` |
| `operator list windows` | `operator window list` |
| `operator focus` | `operator show` |
| `operator input click` | `operator click` |
| `operator input type` | `operator type` |
| `operator input press` | `operator press` |
| `operator input hotkey` | `operator hotkey` |
| `operator input scroll` | `operator scroll` |
| `operator input drag` | `operator drag` |
| `operator input swipe` | `operator swipe` |
| `operator input move` | `operator move` |
