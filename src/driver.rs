use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openssl::base64;
use serde_json::Value;

use crate::auth::default_key_dir;
use crate::error::{HdcError, Result};
use crate::forward::{TcpForwardHandle, send_file_via_shell};
use crate::protocol::DEFAULT_VERSION;
use crate::session::{Session, SessionOptions};
use crate::types::{
    AppAbilityInfo, AppVersion, Coord, CurrentApp, DeviceInfo, DisplayRotation, KeyCode, Point,
    ShellResult,
};
use crate::ui::{UiDriver, UiQuery, UiSelector};
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
        AppAbilityInfo, DriverBuilder, build_press_keys_command, build_right_click_command,
        decode_base64_output, extract_ipv4_after, normalize_velocity, parse_app_abilities,
        parse_app_info_json, parse_app_list, parse_app_version, parse_current_app,
        parse_display_size, parse_main_ability_from_dump, parse_wlan_ip, shell_escape,
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
    fn shell_escape_wraps_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn decode_base64_output_ignores_whitespace() {
        let decoded = decode_base64_output("aGVs\nbG8=\n").unwrap();

        assert_eq!(decoded, b"hello");
    }
}
