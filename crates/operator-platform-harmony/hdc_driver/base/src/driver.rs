use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openssl::base64;
use serde_json::Value;

use crate::auth::default_key_dir;
use crate::error::{HdcError, Result};
use crate::forward::{TcpForwardHandle, send_bytes_via_shell, send_file_via_shell};
use crate::protocol::DEFAULT_VERSION;
use crate::session::{Session, SessionOptions};
use crate::swipe::SwipeExt;
use crate::types::{
    AppAbilityInfo, AppLabelInfo, AppVersion, Coord, CorrelatedWindow, CorrelatedWindowList,
    CurrentApp, DeviceInfo, DisplayRotation, KeyCode, MissionEntry, MissionList, Point,
    ShellResult, WindowDetail, WindowEntry, WindowList, WindowOffset, WindowRect, WindowScale,
};
use crate::ui::{UiDriver, UiQuery, UiSelector, UiWindow};
use crate::xpath::XPathNode;

pub struct Driver {
    addr: String,
    options: SessionOptions,
    session: Session,
}

#[derive(Debug, Clone)]
pub struct DriverBuilder {
    addr: String,
    key_dir: PathBuf,
    version: String,
    connect_key: Option<String>,
    timeout: Duration,
}

pub type HdcDriver = Driver;
pub type HdcDriverBuilder = DriverBuilder;

impl Driver {
    pub fn builder(addr: impl Into<String>) -> DriverBuilder {
        DriverBuilder::new(addr)
    }

    pub fn shell(&mut self, command: impl AsRef<str>) -> Result<ShellResult> {
        self.session.exec_shell(command.as_ref())
    }

    pub fn list_apps(&mut self, _include_system_apps: bool) -> Result<Vec<String>> {
        let output = self.exec_stdout_checked("bm dump -a")?;
        Ok(parse_app_list(&output))
    }

    pub fn list_app_labels(&mut self) -> Result<Vec<AppLabelInfo>> {
        let output = self.exec_stdout_checked("bm dump -l -a")?;
        parse_app_labels(&output)
    }

    pub fn filter_desktop_bundles(&mut self, bundles: &[String]) -> Result<Vec<String>> {
        if bundles.is_empty() {
            return Ok(Vec::new());
        }

        let command = build_filter_desktop_bundles_command(bundles, 16);
        let output = self.exec_stdout_checked(&command)?;
        Ok(parse_app_list(&output))
    }

    pub fn start_app(&mut self, bundle: &str, ability: Option<&str>) -> Result<()> {
        let ability = match ability {
            Some(value) => value.to_string(),
            None => self.resolve_main_ability(bundle)?,
        };
        self.exec_side_effect_checked(&format!(
            "aa start -a {} -b {}",
            shell_escape(&ability),
            shell_escape(bundle)
        ))
    }

    pub fn stop_app(&mut self, bundle: &str) -> Result<()> {
        self.exec_side_effect_checked(&format!("aa force-stop {}", shell_escape(bundle)))
    }

    pub fn current_app(&mut self) -> Result<Option<CurrentApp>> {
        let output = self.exec_stdout_checked("aa dump -l")?;
        Ok(parse_current_app(&output))
    }

    pub fn has_app(&mut self, bundle: &str) -> Result<bool> {
        Ok(self.list_apps(true)?.iter().any(|item| item == bundle))
    }

    pub fn app_version(&mut self, bundle: &str) -> Result<AppVersion> {
        let info = self.get_app_info(bundle)?;
        parse_app_version(&info)
            .ok_or_else(|| HdcError::protocol(format!("failed to parse app version for {bundle}")))
    }

    pub fn get_app_info(&mut self, bundle: &str) -> Result<Value> {
        let output = self.exec_stdout_checked(&format!("bm dump -n {}", shell_escape(bundle)))?;
        parse_app_info_json(&output)
    }

    pub fn get_app_abilities(&mut self, bundle: &str) -> Result<Vec<AppAbilityInfo>> {
        let info = self.get_app_info(bundle)?;
        Ok(parse_app_abilities(&info))
    }

    pub fn get_app_main_ability(&mut self, bundle: &str) -> Result<Option<AppAbilityInfo>> {
        let mut abilities = self.get_app_abilities(bundle)?;
        abilities.sort_by(|left, right| {
            left.is_launcher_ability
                .cmp(&right.is_launcher_ability)
                .reverse()
                .then(right.score.cmp(&left.score))
                .then(left.name.cmp(&right.name))
        });
        Ok(abilities.into_iter().next())
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo> {
        Ok(DeviceInfo {
            product_name: self.param_get("const.product.name")?,
            model: self.param_get("const.product.model")?,
            sdk_version: self.param_get("const.ohos.apiversion")?,
            sys_version: self.param_get("const.product.software.version")?,
            cpu_abi: self.param_get("const.product.cpu.abilist")?,
            wlan_ip: parse_wlan_ip(&self.exec_stdout_checked("ifconfig")?),
            display_size: self.display_size()?,
            display_rotation: self.display_rotation()?,
        })
    }

    pub fn open_url(&mut self, url: &str) -> Result<()> {
        self.exec_side_effect_checked(&format!(
            "aa start -A ohos.want.action.viewData -e entity.system.browsable -U {}",
            shell_escape(url)
        ))
    }

    pub fn list_windows(&mut self) -> Result<WindowList> {
        let output = self.exec_stdout_checked("hidumper -s WindowManagerService -a '-a'")?;
        parse_window_list(&output)
    }

    pub fn get_window(&mut self, window_id: u32) -> Result<WindowDetail> {
        let output = self.exec_stdout_checked(&format!(
            "hidumper -s WindowManagerService -a '-w {window_id}'"
        ))?;
        parse_window_detail(&output)
    }

    pub fn list_missions(&mut self) -> Result<MissionList> {
        let output = self.exec_stdout_checked("hidumper -s AbilityManagerService -a '-l'")?;
        parse_mission_list(&output)
    }

    pub fn list_windows_with_missions(&mut self) -> Result<CorrelatedWindowList> {
        let windows = self.list_windows()?;
        let missions = self.list_missions()?;
        let mission_map = missions
            .missions
            .into_iter()
            .map(|mission| (mission.mission_id, mission))
            .collect::<std::collections::HashMap<u32, MissionEntry>>();

        Ok(CorrelatedWindowList {
            windows: windows
                .windows
                .into_iter()
                .map(|window| CorrelatedWindow {
                    mission: mission_map.get(&window.window_id).cloned(),
                    window,
                })
                .collect(),
            focused_window_id: windows.focused_window_id,
            highlighted_window_ids: windows.highlighted_window_ids,
            total_window_count: windows.total_window_count,
        })
    }

    pub fn correlate_windows_to_missions(&mut self) -> Result<CorrelatedWindowList> {
        self.list_windows_with_missions()
    }

    pub fn display_size(&mut self) -> Result<Point> {
        let output = self.exec_stdout_checked("hidumper -s RenderService -a screen")?;
        let (width, height) = parse_display_size(&output)
            .ok_or_else(|| HdcError::protocol("failed to read display size"))?;
        Ok(Point {
            x: width,
            y: height,
        })
    }

    pub fn display_rotation(&self) -> Result<DisplayRotation> {
        self.ui()?.display_rotation()
    }

    pub fn set_display_rotation(&self, rotation: DisplayRotation) -> Result<()> {
        self.ui()?.set_display_rotation(rotation)
    }

    pub fn click<X, Y>(&mut self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let (x, y) = self.resolve_point(x.into(), y.into())?;
        self.exec_side_effect_checked(&format!("uitest uiInput click {x} {y}"))
    }

    pub fn double_click<X, Y>(&mut self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let (x, y) = self.resolve_point(x.into(), y.into())?;
        self.exec_side_effect_checked(&format!("uitest uiInput doubleClick {x} {y}"))
    }

    pub fn long_click<X, Y>(&mut self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let (x, y) = self.resolve_point(x.into(), y.into())?;
        self.exec_side_effect_checked(&format!("uitest uiInput longClick {x} {y}"))
    }

    pub fn right_click<X, Y>(&mut self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let (x, y) = self.resolve_point(x.into(), y.into())?;
        self.exec_side_effect_checked(&build_right_click_command(x, y))
    }

    pub fn move_cursor<X, Y>(&mut self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let (x, y) = self.resolve_point(x.into(), y.into())?;
        self.exec_side_effect_checked(&build_move_cursor_command(x, y))
    }

    pub fn swipe<X1, Y1, X2, Y2>(
        &mut self,
        x1: X1,
        y1: Y1,
        x2: X2,
        y2: Y2,
        speed: Option<u32>,
    ) -> Result<()>
    where
        X1: Into<Coord>,
        Y1: Into<Coord>,
        X2: Into<Coord>,
        Y2: Into<Coord>,
    {
        let (x1, y1) = self.resolve_point(x1.into(), y1.into())?;
        let (x2, y2) = self.resolve_point(x2.into(), y2.into())?;
        let speed = normalize_velocity(speed)?;
        self.exec_side_effect_checked(&format!("uitest uiInput swipe {x1} {y1} {x2} {y2} {speed}"))
    }

    pub fn drag<X1, Y1, X2, Y2>(
        &mut self,
        x1: X1,
        y1: Y1,
        x2: X2,
        y2: Y2,
        speed: Option<u32>,
    ) -> Result<()>
    where
        X1: Into<Coord>,
        Y1: Into<Coord>,
        X2: Into<Coord>,
        Y2: Into<Coord>,
    {
        let (x1, y1) = self.resolve_point(x1.into(), y1.into())?;
        let (x2, y2) = self.resolve_point(x2.into(), y2.into())?;
        let speed = normalize_velocity(speed)?;
        self.exec_side_effect_checked(&format!("uitest uiInput drag {x1} {y1} {x2} {y2} {speed}"))
    }

    pub fn swipe_ext(&mut self) -> SwipeExt<'_> {
        SwipeExt::new(self)
    }

    pub fn input_text(&mut self, text: &str) -> Result<()> {
        self.exec_side_effect_checked(&format!("uitest uiInput text {}", shell_escape(text)))
    }

    pub fn press_key(&mut self, key: impl Into<KeyCode>) -> Result<()> {
        let key = key.into().raw();
        self.exec_side_effect_checked(&format!("uitest uiInput keyEvent {key}"))
    }

    pub fn press_keys<I, K>(&mut self, keys: I) -> Result<()>
    where
        I: IntoIterator<Item = K>,
        K: Into<KeyCode>,
    {
        let keys = keys
            .into_iter()
            .map(|item| item.into().raw())
            .collect::<Vec<u32>>();
        let command = build_press_keys_command(&keys)?;
        self.exec_side_effect_checked(&command)
    }

    pub fn go_home(&mut self) -> Result<()> {
        self.press_key(KeyCode::HOME)
    }

    pub fn go_back(&mut self) -> Result<()> {
        self.press_key(KeyCode::BACK)
    }

    pub fn screen_on(&mut self) -> Result<()> {
        self.exec_side_effect_checked("power-shell wakeup")
    }

    pub fn screen_off(&mut self) -> Result<()> {
        self.screen_on()?;
        self.press_key(KeyCode::POWER)
    }

    pub fn unlock(&mut self) -> Result<()> {
        self.screen_on()?;
        self.swipe(0.5_f64, 0.8_f64, 0.5_f64, 0.2_f64, Some(6000))
    }

    pub fn screenshot(&mut self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref().to_path_buf();
        let remote = remote_temp_path("jpeg");
        let remote_quoted = shell_escape(&remote);
        let command = format!(
            "snapshot_display -f {remote} >/dev/null 2>&1 && (base64 {remote} 2>/dev/null || toybox base64 {remote} 2>/dev/null); ret=$?; rm -f {remote}; exit $ret",
            remote = remote_quoted
        );
        let result = self.shell(command)?;
        ensure_shell_success(&result, "capture screenshot")?;
        let bytes = decode_base64_output(result.stdout_text().as_ref())?;
        fs::write(&path, bytes)?;
        Ok(path)
    }

    pub fn dump_hierarchy(&mut self) -> Result<Value> {
        let remote = remote_temp_path("json");
        let remote_quoted = shell_escape(&remote);
        let command = format!(
            "uitest dumpLayout -p {remote} >/dev/null 2>&1 && cat {remote}; ret=$?; rm -f {remote}; exit $ret",
            remote = remote_quoted
        );
        let output = self.exec_stdout_checked(&command)?;
        Ok(serde_json::from_str(output.trim())?)
    }

    pub fn ui(&self) -> Result<UiDriver> {
        let builder = UiDriver::builder(self.addr.clone())
            .key_dir(self.options.key_dir.clone())
            .connect_key(self.options.connect_key.clone())
            .version(self.options.version.clone())
            .timeout(self.options.timeout);
        builder.connect()
    }

    pub fn xpath<'a>(&'a mut self, expression: &str) -> Result<XPathNode<'a>> {
        XPathNode::find(self, expression)
    }

    pub fn select(&self, selector: UiSelector) -> Result<UiQuery> {
        Ok(self.ui()?.select(selector))
    }

    pub fn query(&self) -> Result<UiQuery> {
        Ok(self.ui()?.query())
    }

    pub fn text(&self, value: impl Into<String>) -> Result<UiQuery> {
        Ok(self.ui()?.text(value))
    }

    pub fn id(&self, value: impl Into<String>) -> Result<UiQuery> {
        Ok(self.ui()?.id(value))
    }

    pub fn key(&self, value: impl Into<String>) -> Result<UiQuery> {
        Ok(self.ui()?.key(value))
    }

    pub fn kind(&self, value: impl Into<String>) -> Result<UiQuery> {
        Ok(self.ui()?.kind(value))
    }

    pub fn description(&self, value: impl Into<String>) -> Result<UiQuery> {
        Ok(self.ui()?.description(value))
    }

    pub fn find_window(&self, active: bool) -> Result<Option<UiWindow>> {
        self.ui()?.find_window(active)
    }

    pub fn find_active_window(&self) -> Result<Option<UiWindow>> {
        self.ui()?.find_active_window()
    }

    pub fn close(&mut self) -> Result<()> {
        self.session.close_active_command_channel()
    }

    pub fn send_file(
        &mut self,
        local_path: impl AsRef<Path>,
        remote_path: impl AsRef<str>,
    ) -> Result<()> {
        send_file_via_shell(&mut self.session, local_path.as_ref(), remote_path.as_ref())
    }

    pub fn send_bytes(
        &mut self,
        bytes: impl AsRef<[u8]>,
        remote_path: impl AsRef<str>,
    ) -> Result<()> {
        send_bytes_via_shell(&mut self.session, bytes.as_ref(), remote_path.as_ref())
    }

    pub fn forward_tcp(&self, local_port: u16, remote_port: u16) -> Result<TcpForwardHandle> {
        TcpForwardHandle::spawn(
            self.addr.clone(),
            self.options.clone(),
            local_port,
            remote_port,
        )
    }

    fn exec_stdout_checked(&mut self, command: &str) -> Result<String> {
        let result = self.shell(command)?;
        ensure_shell_success(&result, command)?;
        Ok(result.stdout_text().into_owned())
    }

    fn exec_side_effect_checked(&mut self, command: &str) -> Result<()> {
        self.exec_stdout_checked(command).map(|_| ())
    }

    fn resolve_point(&mut self, x: Coord, y: Coord) -> Result<(i32, i32)> {
        let output = self.exec_stdout_checked("hidumper -s RenderService -a screen")?;
        let (width, height) = parse_display_size(&output)
            .ok_or_else(|| HdcError::protocol("failed to read display size"))?;
        Ok((x.resolve(width)?, y.resolve(height)?))
    }

    fn resolve_main_ability(&mut self, bundle: &str) -> Result<String> {
        self.get_app_main_ability(bundle)?
            .map(|item| item.name)
            .ok_or_else(|| {
                HdcError::protocol(format!("failed to resolve main ability for {bundle}"))
            })
    }

    fn param_get(&mut self, key: &str) -> Result<String> {
        let output = self.exec_stdout_checked(&format!("param get {key}"))?;
        first_nonempty_line(&output)
            .map(ToOwned::to_owned)
            .ok_or_else(|| HdcError::protocol(format!("param get {key} returned empty output")))
    }
}

impl DriverBuilder {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            key_dir: default_key_dir(),
            version: DEFAULT_VERSION.to_string(),
            connect_key: None,
            timeout: Duration::from_secs(60),
        }
    }

    pub fn key_dir(mut self, key_dir: impl Into<PathBuf>) -> Self {
        self.key_dir = key_dir.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn connect_key(mut self, connect_key: impl Into<String>) -> Self {
        self.connect_key = Some(connect_key.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn connect(self) -> Result<Driver> {
        let options = SessionOptions {
            key_dir: self.key_dir,
            connect_key: self.connect_key.unwrap_or_else(|| self.addr.clone()),
            version: self.version,
            timeout: self.timeout,
        };
        let mut session = Session::connect(&self.addr, options.clone())?;
        session.authenticate()?;
        Ok(Driver {
            addr: self.addr,
            options,
            session,
        })
    }
}

fn ensure_shell_success(result: &ShellResult, context: &str) -> Result<()> {
    if result.failed() {
        let message = result
            .messages
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<&str>>()
            .join(" | ");
        return Err(HdcError::protocol(format!("{context} failed: {message}")));
    }
    Ok(())
}

fn parse_app_list(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("ID:"))
        .map(ToOwned::to_owned)
        .collect()
}

fn build_filter_desktop_bundles_command(bundles: &[String], parallelism: usize) -> String {
    let args = bundles
        .iter()
        .map(|bundle| shell_escape(bundle))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sh -c 'printf \"%s\\n\" {args} | xargs -n 1 -P {parallelism} sh -c '\\''bundle=\"$1\"; bm dump -n \"$bundle\" 2>/dev/null | grep -q \"\\\"hideDesktopIcon\\\": false\" && echo \"$bundle\"'\\'' _'"
    )
}

fn parse_current_app(output: &str) -> Option<CurrentApp> {
    let block = output
        .split("Mission ID #")
        .find(|item| item.contains("state #FOREGROUND"))?;
    let bundle_name = extract_between(block, "bundle name [", "]")?;
    let ability_name = extract_between(block, "main name [", "]")?;
    Some(CurrentApp {
        bundle_name,
        ability_name,
    })
}

#[cfg(test)]
fn parse_main_ability_from_dump(output: &str) -> Option<String> {
    let value = parse_app_info_json(output).ok()?;
    parse_app_abilities(&value)
        .into_iter()
        .max_by(|left, right| {
            left.is_launcher_ability
                .cmp(&right.is_launcher_ability)
                .then(left.score.cmp(&right.score))
                .then(right.name.cmp(&left.name))
        })
        .map(|item| item.name)
}

fn parse_display_size(output: &str) -> Option<(i32, i32)> {
    let marker = "activeMode:";
    let start = output.find(marker)? + marker.len();
    let rest = output[start..].trim_start();
    let dims = rest.split(',').next()?.trim();
    let (width, height) = dims.split_once('x')?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

fn parse_mission_list(output: &str) -> Result<MissionList> {
    let missions = output
        .split("Mission ID #")
        .skip(1)
        .map(parse_mission_entry)
        .collect::<Result<Vec<MissionEntry>>>()?;
    Ok(MissionList { missions })
}

fn parse_mission_entry(block: &str) -> Result<MissionEntry> {
    let block = block.trim();
    let first_line = block
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| HdcError::protocol("mission block missing header"))?;

    let mission_id_raw = first_line
        .split_whitespace()
        .next()
        .ok_or_else(|| HdcError::protocol("mission block missing mission id"))?;
    let mission_name = extract_between(first_line, "mission name #[", "]  lockedState #")
        .ok_or_else(|| HdcError::protocol("mission block missing mission name"))?;
    let locked_state = extract_between(first_line, "lockedState #", "  mission affinity #[")
        .ok_or_else(|| HdcError::protocol("mission block missing lockedState"))?;
    let mission_affinity = extract_between(first_line, "mission affinity #[", "]")
        .ok_or_else(|| HdcError::protocol("mission block missing mission affinity"))?;

    Ok(MissionEntry {
        mission_id: parse_u32(mission_id_raw, "mission_id")?,
        mission_name,
        locked_state: parse_i32(&locked_state, "locked_state")?,
        mission_affinity,
        ability_record_id: extract_after_hash_u32(block, "AbilityRecord ID #"),
        app_name: extract_between(block, "app name [", "]"),
        main_name: extract_between(block, "main name [", "]"),
        bundle_name: extract_between(block, "bundle name [", "]"),
        ability_type: extract_between(block, "ability type [", "]"),
        state: extract_after_hash_token(block, "state #"),
        app_state: extract_after_hash_token(block, "app state #"),
        ready: extract_after_hash_bool(block, "ready #")?,
        window_attached: extract_after_hash_bool(block, "window attached #")?,
        launcher: extract_after_hash_bool(block, "launcher #")?,
        is_keep_alive: extract_after_colon_bool(block, "isKeepAlive:")?,
    })
}

fn parse_window_list(output: &str) -> Result<WindowList> {
    let mut windows = Vec::new();
    let mut focused_window_id = None;
    let mut highlighted_window_ids = Vec::new();
    let mut total_window_count = None;

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with("WindowName") || line.starts_with('-') {
            continue;
        }
        if let Some(value) = line.strip_prefix("Focus window:") {
            focused_window_id = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = line.strip_prefix("Highlighted windows:") {
            highlighted_window_ids = value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .filter_map(|item| item.parse().ok())
                .collect();
            continue;
        }
        if let Some(value) = line.strip_prefix("Total window num:") {
            total_window_count = value.trim().parse().ok();
            continue;
        }
        if line.starts_with("All Focus window:")
            || line.starts_with("DisplayId:")
            || line.starts_with("SingleHand:")
        {
            continue;
        }
        if let Some(window) = parse_window_list_entry(line)? {
            windows.push(window);
        }
    }

    Ok(WindowList {
        windows,
        focused_window_id,
        highlighted_window_ids,
        total_window_count,
    })
}

fn parse_window_list_entry(line: &str) -> Result<Option<WindowEntry>> {
    let Some(first_bracket) = line.find('[') else {
        return Ok(None);
    };
    let prefix = line[..first_bracket].trim();
    let columns = prefix.split_whitespace().collect::<Vec<&str>>();
    if columns.len() < 9 {
        return Ok(None);
    }
    let rect_group = bracket_groups(line)
        .into_iter()
        .next()
        .ok_or_else(|| HdcError::protocol("window list row missing rect group"))?;
    let rect_values = parse_i32_list(&rect_group)?;
    if rect_values.len() != 4 {
        return Err(HdcError::protocol("window rect group must have 4 integers"));
    }

    Ok(Some(WindowEntry {
        name: columns[0].to_string(),
        display_id: parse_i32(columns[1], "display_id")?,
        pid: parse_i32(columns[2], "pid")?,
        window_id: parse_u32(columns[3], "window_id")?,
        window_type: parse_i32(columns[4], "window_type")?,
        mode: parse_i32(columns[5], "mode")?,
        flag: parse_i32(columns[6], "flag")?,
        z_order: parse_i32(columns[7], "z_order")?,
        orientation: parse_i32(columns[8], "orientation")?,
        rect: WindowRect {
            x: rect_values[0],
            y: rect_values[1],
            width: rect_values[2],
            height: rect_values[3],
        },
    }))
}

fn parse_window_detail(output: &str) -> Result<WindowDetail> {
    let mut map = std::collections::HashMap::<String, String>::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with('-') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let rect = parse_bracket_i32_count(
        map.get("WindowRect")
            .ok_or_else(|| HdcError::protocol("window detail missing WindowRect"))?,
        4,
        "WindowRect",
    )?;
    let offset = parse_bracket_i32_count(
        map.get("Offset")
            .ok_or_else(|| HdcError::protocol("window detail missing Offset"))?,
        2,
        "Offset",
    )?;
    let scale = parse_bracket_f64_count(
        map.get("Scale")
            .ok_or_else(|| HdcError::protocol("window detail missing Scale"))?,
        4,
        "Scale",
    )?;

    Ok(WindowDetail {
        name: map_value(&map, "WindowName")?.to_string(),
        display_id: parse_i32(map_value(&map, "DisplayId")?, "DisplayId")?,
        window_id: parse_u32(map_value(&map, "WinId")?, "WinId")?,
        pid: parse_i32(map_value(&map, "Pid")?, "Pid")?,
        window_type: parse_i32(map_value(&map, "Type")?, "Type")?,
        mode: parse_i32(map_value(&map, "Mode")?, "Mode")?,
        flag: parse_i32(map_value(&map, "Flag")?, "Flag")?,
        orientation: parse_i32(map_value(&map, "Orientation")?, "Orientation")?,
        first_frame_callback_called: parse_bool(map_value(&map, "FirstFrameCallbackCalled")?)?,
        is_visible: parse_bool(map_value(&map, "IsVisible")?)?,
        is_rs_visible: parse_bool(map_value(&map, "isRSVisible")?)?,
        focusable: parse_bool(map_value(&map, "Focusable")?)?,
        deco_status: parse_bool(map_value(&map, "DecoStatus")?)?,
        is_privacy_mode: parse_bool(map_value(&map, "isPrivacyMode")?)?,
        rect: WindowRect {
            x: rect[0],
            y: rect[1],
            width: rect[2],
            height: rect[3],
        },
        scale_x: parse_f64(map_value(&map, "scaleX")?, "scaleX")?,
        scale_y: parse_f64(map_value(&map, "scaleY")?, "scaleY")?,
        offset: WindowOffset {
            x: offset[0],
            y: offset[1],
        },
        scale: WindowScale {
            scale_x: scale[0],
            scale_y: scale[1],
            pivot_x: scale[2],
            pivot_y: scale[3],
        },
        parent_window_id: parse_u32(map_value(&map, "ParentWindowId")?, "ParentWindowId")?,
    })
}

fn parse_app_info_json(output: &str) -> Result<Value> {
    let json_start = output
        .find('{')
        .ok_or_else(|| HdcError::protocol("bm dump output missing json payload"))?;
    let json_end = output
        .rfind('}')
        .map(|value| value + 1)
        .ok_or_else(|| HdcError::protocol("bm dump output missing json payload"))?;
    Ok(serde_json::from_str(&output[json_start..json_end])?)
}

fn parse_app_labels(output: &str) -> Result<Vec<AppLabelInfo>> {
    let values = serde_json::from_str::<Vec<Value>>(output)?;
    Ok(values
        .into_iter()
        .filter_map(|value| {
            let bundle_name = value.get("bundleName").and_then(Value::as_str)?.trim();
            let label = value.get("label").and_then(Value::as_str)?.trim();
            if bundle_name.is_empty() || label.is_empty() {
                return None;
            }

            Some(AppLabelInfo {
                bundle_name: bundle_name.to_string(),
                label: label.to_string(),
            })
        })
        .collect())
}

fn parse_app_abilities(value: &Value) -> Vec<AppAbilityInfo> {
    let main_entry = value
        .get("mainEntry")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(modules) = value.get("hapModuleInfos").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for module in modules {
        let module_main = module
            .get("mainAbility")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let ability_infos = module
            .get("abilityInfos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for ability in ability_infos {
            let Some(name) = ability.get("name").and_then(Value::as_str) else {
                continue;
            };
            let module_name = ability
                .get("moduleName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let is_launcher_ability = ability
                .get("skills")
                .and_then(Value::as_array)
                .map(|skills| {
                    skills.iter().any(|skill| {
                        skill
                            .get("actions")
                            .and_then(Value::as_array)
                            .map(|actions| {
                                actions
                                    .iter()
                                    .any(|action| action.as_str() == Some("action.system.home"))
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            let mut score = 0;
            if name == module_main {
                score += 1;
            }
            if !main_entry.is_empty() && module_name == main_entry {
                score += 1;
            }
            result.push(AppAbilityInfo {
                name: name.to_string(),
                module_name,
                module_main_ability: module_main.clone(),
                main_module: main_entry.clone(),
                is_launcher_ability,
                score,
            });
        }
    }
    result
}

fn parse_app_version(value: &Value) -> Option<AppVersion> {
    let version_name = value
        .get("versionName")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("applicationInfo")
                .and_then(|item| item.get("versionName"))
                .and_then(Value::as_str)
        })?;
    let version_code = value
        .get("versionCode")
        .and_then(Value::as_i64)
        .or_else(|| {
            value
                .get("applicationInfo")
                .and_then(|item| item.get("versionCode"))
                .and_then(Value::as_i64)
        })?;
    Some(AppVersion {
        version_name: version_name.to_string(),
        version_code,
    })
}

fn first_nonempty_line(output: &str) -> Option<&str> {
    output.lines().map(str::trim).find(|line| !line.is_empty())
}

fn parse_wlan_ip(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("127.0.0.1") {
            continue;
        }
        if let Some(ip) = extract_ipv4_after(trimmed, "inet addr:") {
            return Some(ip);
        }
        if let Some(ip) = extract_ipv4_after(trimmed, "inet ") {
            return Some(ip);
        }
    }
    None
}

fn extract_ipv4_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let rest = line[start..].trim_start();
    let token = rest.split_whitespace().next()?;
    let candidate = token.split('/').next()?;
    if candidate.split('.').count() == 4
        && candidate
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return Some(candidate.to_string());
    }
    None
}

fn bracket_groups(line: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut inside = false;
    for ch in line.chars() {
        match ch {
            '[' if !inside => {
                inside = true;
                current.clear();
            }
            ']' if inside => {
                inside = false;
                groups.push(current.trim().to_string());
            }
            _ if inside => current.push(ch),
            _ => {}
        }
    }
    groups
}

fn parse_i32_list(raw: &str) -> Result<Vec<i32>> {
    raw.split([',', ' '])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| parse_i32(item, "list item"))
        .collect()
}

fn parse_f64_list(raw: &str) -> Result<Vec<f64>> {
    raw.split([',', ' '])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| parse_f64(item, "list item"))
        .collect()
}

fn parse_bracket_i32_count(raw: &str, expected: usize, label: &str) -> Result<Vec<i32>> {
    let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
    let values = parse_i32_list(trimmed)?;
    if values.len() != expected {
        return Err(HdcError::protocol(format!(
            "{label} expected {expected} integers, got {}",
            values.len()
        )));
    }
    Ok(values)
}

fn parse_bracket_f64_count(raw: &str, expected: usize, label: &str) -> Result<Vec<f64>> {
    let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
    let values = parse_f64_list(trimmed)?;
    if values.len() != expected {
        return Err(HdcError::protocol(format!(
            "{label} expected {expected} numbers, got {}",
            values.len()
        )));
    }
    Ok(values)
}

fn parse_i32(raw: &str, label: &str) -> Result<i32> {
    raw.parse()
        .map_err(|_| HdcError::protocol(format!("failed to parse {label} as i32: {raw}")))
}

fn parse_u32(raw: &str, label: &str) -> Result<u32> {
    raw.parse()
        .map_err(|_| HdcError::protocol(format!("failed to parse {label} as u32: {raw}")))
}

fn parse_f64(raw: &str, label: &str) -> Result<f64> {
    raw.parse()
        .map_err(|_| HdcError::protocol(format!("failed to parse {label} as f64: {raw}")))
}

fn parse_bool(raw: &str) -> Result<bool> {
    match raw {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(HdcError::protocol(format!(
            "failed to parse bool value: {other}"
        ))),
    }
}

fn extract_after_hash_token(haystack: &str, marker: &str) -> Option<String> {
    let start = haystack.find(marker)? + marker.len();
    let rest = haystack[start..].trim_start();
    Some(rest.split_whitespace().next()?.trim().to_string())
}

fn extract_after_hash_u32(haystack: &str, marker: &str) -> Option<u32> {
    extract_after_hash_token(haystack, marker)?.parse().ok()
}

fn extract_after_hash_bool(haystack: &str, marker: &str) -> Result<Option<bool>> {
    match extract_after_hash_token(haystack, marker) {
        Some(value) => parse_bool(&value).map(Some),
        None => Ok(None),
    }
}

fn extract_after_colon_bool(haystack: &str, marker: &str) -> Result<Option<bool>> {
    let Some(start) = haystack.find(marker) else {
        return Ok(None);
    };
    let rest = haystack[start + marker.len()..].trim_start();
    let Some(token) = rest.split_whitespace().next() else {
        return Ok(None);
    };
    parse_bool(token).map(Some)
}

fn map_value<'a>(map: &'a std::collections::HashMap<String, String>, key: &str) -> Result<&'a str> {
    map.get(key)
        .map(String::as_str)
        .ok_or_else(|| HdcError::protocol(format!("window detail missing {key}")))
}

fn normalize_velocity(value: Option<u32>) -> Result<u32> {
    let velocity = value.unwrap_or(600);
    if !(200..=40000).contains(&velocity) {
        return Err(HdcError::protocol(format!(
            "velocity must be between 200 and 40000, got {velocity}"
        )));
    }
    Ok(velocity)
}

fn build_press_keys_command(keys: &[u32]) -> Result<String> {
    if keys.is_empty() {
        return Err(HdcError::protocol("press_keys requires at least one key"));
    }
    if keys.len() <= 3 {
        let joined = keys
            .iter()
            .map(u32::to_string)
            .collect::<Vec<String>>()
            .join(" ");
        return Ok(format!("uitest uiInput keyEvent {joined}"));
    }

    let mut command = String::from("uinput -K");
    for key in keys {
        command.push_str(&format!(" -d {key}"));
    }
    for key in keys.iter().rev() {
        command.push_str(&format!(" -u {key}"));
    }
    Ok(command)
}

fn build_right_click_command(x: i32, y: i32) -> String {
    format!("uinput -M -m {x} {y} -c 1")
}

fn build_move_cursor_command(x: i32, y: i32) -> String {
    format!("uinput -M -m {x} {y}")
}

fn remote_temp_path(extension: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("/data/local/tmp/hmdriver-rs-{nanos}.{extension}")
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn extract_between(haystack: &str, left: &str, right: &str) -> Option<String> {
    let start = haystack.find(left)? + left.len();
    let rest = &haystack[start..];
    let end = rest.find(right)?;
    Some(rest[..end].to_string())
}

fn decode_base64_output(value: &str) -> Result<Vec<u8>> {
    let compact = value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return Err(HdcError::protocol("received empty base64 payload"));
    }
    Ok(base64::decode_block(&compact)?)
}

#[cfg(test)]
mod tests {
    use super::{
        AppAbilityInfo, AppLabelInfo, DriverBuilder, bracket_groups,
        build_filter_desktop_bundles_command, build_move_cursor_command, build_press_keys_command,
        build_right_click_command, decode_base64_output, extract_ipv4_after, normalize_velocity,
        parse_app_abilities, parse_app_info_json, parse_app_labels, parse_app_list,
        parse_app_version, parse_current_app, parse_display_size, parse_main_ability_from_dump,
        parse_mission_list, parse_window_detail, parse_window_list, parse_wlan_ip, shell_escape,
    };
    use crate::types::Coord;
    use serde_json::{Value, json};

    #[test]
    fn builder_uses_addr_as_default_connect_key() {
        let builder = DriverBuilder::new("192.168.8.43:35319");

        assert_eq!(builder.addr, "192.168.8.43:35319");
        assert!(builder.connect_key.is_none());
    }

    #[test]
    fn parse_app_list_filters_empty_lines_and_ids() {
        let output = "ID: 1\ncom.example.first\n\ncom.example.second\n";

        let apps = parse_app_list(output);

        assert_eq!(apps, vec!["com.example.first", "com.example.second"]);
    }

    #[test]
    fn parse_current_app_returns_foreground_bundle() {
        let output = r#"
Mission ID #12 {
  bundle name [com.example.demo]
  main name [EntryAbility]
  state #FOREGROUND
  isKeepAlive: false
}
"#;

        let current = parse_current_app(output).unwrap();

        assert_eq!(current.bundle_name, "com.example.demo");
        assert_eq!(current.ability_name, "EntryAbility");
    }

    #[test]
    fn parse_main_ability_prefers_module_main_and_main_entry() {
        let output = r#"prefix {
  "mainEntry":"entry",
  "hapModuleInfos":[
    {
      "mainAbility":"MainAbility",
      "moduleName":"entry",
      "abilityInfos":[
        {"name":"MainAbility","moduleName":"entry","skills":[{"actions":["action.system.home"]}]},
        {"name":"OtherAbility","moduleName":"entry","skills":[]}
      ]
    }
  ]
} suffix"#;

        let ability = parse_main_ability_from_dump(output).unwrap();

        assert_eq!(ability, "MainAbility");
    }

    #[test]
    fn parse_app_info_json_extracts_embedded_payload() {
        let value = parse_app_info_json("prefix {\"bundleName\":\"com.example\"} suffix").unwrap();

        assert_eq!(
            value.get("bundleName").and_then(Value::as_str),
            Some("com.example")
        );
    }

    #[test]
    fn parse_app_labels_reads_bundle_and_human_label_pairs() {
        let labels = parse_app_labels(
            r#"[
                {"bundleName":"com.example.notes","label":"备忘录"},
                {"bundleName":"com.example.browser","label":"浏览器"}
            ]"#,
        )
        .unwrap();

        assert_eq!(
            labels,
            vec![
                AppLabelInfo {
                    bundle_name: "com.example.notes".into(),
                    label: "备忘录".into(),
                },
                AppLabelInfo {
                    bundle_name: "com.example.browser".into(),
                    label: "浏览器".into(),
                },
            ]
        );
    }

    #[test]
    fn build_filter_desktop_bundles_command_uses_parallel_device_side_dump() {
        let command = build_filter_desktop_bundles_command(
            &["com.example.notes".into(), "com.example.browser".into()],
            16,
        );

        assert!(command.contains("xargs -n 1 -P 16"));
        assert!(command.contains("'com.example.notes' 'com.example.browser'"));
        assert!(command.contains("bm dump -n \"$bundle\""));
        assert!(command.contains("\\\"hideDesktopIcon\\\": false"));
    }

    #[test]
    fn parse_app_abilities_scores_launcher_and_main_entry() {
        let value = json!({
            "mainEntry": "entry",
            "hapModuleInfos": [{
                "mainAbility": "MainAbility",
                "abilityInfos": [{
                    "name": "MainAbility",
                    "moduleName": "entry",
                    "skills": [{"actions": ["action.system.home"]}]
                }]
            }]
        });

        let abilities = parse_app_abilities(&value);

        assert_eq!(abilities.len(), 1);
        assert_eq!(
            abilities[0],
            AppAbilityInfo {
                name: "MainAbility".to_string(),
                module_name: "entry".to_string(),
                module_main_ability: "MainAbility".to_string(),
                main_module: "entry".to_string(),
                is_launcher_ability: true,
                score: 2,
            }
        );
    }

    #[test]
    fn parse_app_version_prefers_top_level_fields() {
        let version = parse_app_version(&json!({
            "versionName": "12.2.40",
            "versionCode": 999999,
            "applicationInfo": {
                "versionName": "ignored",
                "versionCode": 1
            }
        }))
        .unwrap();

        assert_eq!(version.version_name, "12.2.40");
        assert_eq!(version.version_code, 999999);
    }

    #[test]
    fn display_size_parser_extracts_dimensions() {
        let output = "activeMode: 1260x2720, refreshrate=120";

        let size = parse_display_size(output).unwrap();

        assert_eq!(size, (1260, 2720));
    }

    #[test]
    fn velocity_defaults_to_uiinput_default() {
        assert_eq!(normalize_velocity(None).unwrap(), 600);
    }

    #[test]
    fn press_keys_prefers_uiinput_for_three_keys_or_less() {
        let command = build_press_keys_command(&[1, 2, 3]).unwrap();

        assert_eq!(command, "uitest uiInput keyEvent 1 2 3");
    }

    #[test]
    fn press_keys_falls_back_to_uinput_for_longer_chords() {
        let command = build_press_keys_command(&[1, 2, 3, 4]).unwrap();

        assert_eq!(command, "uinput -K -d 1 -d 2 -d 3 -d 4 -u 4 -u 3 -u 2 -u 1");
    }

    #[test]
    fn right_click_builds_move_then_click_command() {
        let command = build_right_click_command(1560, 1040);

        assert_eq!(command, "uinput -M -m 1560 1040 -c 1");
    }

    #[test]
    fn move_cursor_builds_move_only_command() {
        let command = build_move_cursor_command(1560, 1040);

        assert_eq!(command, "uinput -M -m 1560 1040");
    }

    #[test]
    fn coord_rejects_invalid_percentage() {
        assert!(Coord::from(1.5_f64).resolve(100).is_err());
    }

    #[test]
    fn parse_wlan_ip_supports_legacy_ifconfig_output() {
        let output =
            "wlan0 Link encap:Ethernet\n          inet addr:192.168.8.43  Bcast:192.168.8.255";

        assert_eq!(parse_wlan_ip(output), Some("192.168.8.43".to_string()));
    }

    #[test]
    fn parse_wlan_ip_supports_modern_ifconfig_output() {
        let output = "wlan0: flags=4163<UP> mtu 1500\n    inet 192.168.8.43 netmask 255.255.255.0";

        assert_eq!(parse_wlan_ip(output), Some("192.168.8.43".to_string()));
    }

    #[test]
    fn extract_ipv4_after_rejects_non_ipv4_tokens() {
        assert_eq!(extract_ipv4_after("inet fe80::1", "inet "), None);
    }

    #[test]
    fn bracket_groups_extract_multiple_sections() {
        let groups = bracket_groups("foo [ 1 2 3 4 ] [ 5 6 ] [ 1, 1, 0.5, 0.5 ]");

        assert_eq!(groups, vec!["1 2 3 4", "5 6", "1, 1, 0.5, 0.5"]);
    }

    #[test]
    fn parse_window_list_extracts_rows_and_summary() {
        let output = r#"
WindowName           DisplayId Pid     WinId Type Mode Flag ZOrd Orientation [ x    y    w    h    ] [ OffsetX OffsetY ] [ ScaleX  ScaleY  PivotX  PivotY  ]
installer0           0         54500   225   1    102  0    114  0           [ 584  732  1600 1065 ] [ 0       0       ] [ 1       1       0.5     0.5     ]
browser0             0         45589   267   1    102  0    113  0           [ 0    0    2080 1303 ] [ 0       0       ] [ 1       1       0.5     0.500128]
Focus window: 225
Total window num: 92
Highlighted windows: 225
"#;

        let list = parse_window_list(output).unwrap();

        assert_eq!(list.windows.len(), 2);
        assert_eq!(list.focused_window_id, Some(225));
        assert_eq!(list.total_window_count, Some(92));
        assert_eq!(list.highlighted_window_ids, vec![225]);
        assert_eq!(list.windows[1].name, "browser0");
        assert_eq!(list.windows[1].rect.width, 2080);
    }

    #[test]
    fn parse_window_detail_extracts_visibility_and_rect() {
        let output = r#"
WindowName: filemanager0
DisplayId: 0
WinId: 266
Pid: 44667
Type: 1
Mode: 102
Flag: 0
Orientation: 18
FirstFrameCallbackCalled: 1
IsVisible: true
isRSVisible: true
Focusable: true
DecoStatus: true
isPrivacyMode: false
WindowRect: [ 1026, 320, 2068, 1394 ]
scaleX: 1
scaleY: 1
Offset: [ 0, 0 ]
Scale: [ 1, 1, 0.5, 0.5 ]
ParentWindowId: 0
"#;

        let detail = parse_window_detail(output).unwrap();

        assert_eq!(detail.name, "filemanager0");
        assert_eq!(detail.window_id, 266);
        assert!(detail.is_visible);
        assert_eq!(detail.rect.height, 1394);
        assert_eq!(detail.scale.pivot_x, 0.5);
    }

    #[test]
    fn parse_mission_list_extracts_bundle_and_flags() {
        let output = r#"
Mission ID #225  mission name #[#com.openclaw.hmos.installer:entry:EntryAbility]  lockedState #0  mission affinity #[]
  AbilityRecord ID #2058
    app name [com.openclaw.hmos.installer]
    main name [EntryAbility]
    bundle name [com.openclaw.hmos.installer]
    ability type [PAGE]
    state #FOREGROUND  start time [181348859]
    app state #FOREGROUND
    ready #1  window attached #0  launcher #0
    callee connections:
    isKeepAlive: false
Mission ID #266  mission name #[#com.huawei.hmos.filemanager:pc:MainAbility]  lockedState #0  mission affinity #[]
  AbilityRecord ID #2291
    app name [com.huawei.hmos.filemanager]
    main name [MainAbility]
    bundle name [com.huawei.hmos.filemanager]
    ability type [PAGE]
    state #FOREGROUND  start time [228934919]
    app state #FOREGROUND
    ready #1  window attached #0  launcher #0
    isKeepAlive: false
"#;

        let missions = parse_mission_list(output).unwrap();

        assert_eq!(missions.missions.len(), 2);
        assert_eq!(missions.missions[0].mission_id, 225);
        assert_eq!(
            missions.missions[0].bundle_name.as_deref(),
            Some("com.openclaw.hmos.installer")
        );
        assert_eq!(missions.missions[0].ready, Some(true));
        assert_eq!(missions.missions[0].window_attached, Some(false));
        assert_eq!(
            missions.missions[1].main_name.as_deref(),
            Some("MainAbility")
        );
    }

    #[test]
    fn shell_escape_wraps_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn decode_base64_output_ignores_whitespace() {
        let decoded = decode_base64_output("aGVs\nbG8=\n").unwrap();

        assert_eq!(decoded, b"hello");
    }
}
