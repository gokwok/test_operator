use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::driver::Driver;
use crate::error::{HdcError, Result};
use crate::forward::TcpForwardHandle;
use crate::types::{Bounds, Coord, DisplayRotation, Point, UiComponentInfo, UiEvent};

const UITEST_SERVICE_PORT: u16 = 8012;
const DEFAULT_REMOTE_AGENT_PATH: &str = "/data/local/tmp/agent.so";

#[derive(Debug, Clone)]
pub struct UiDriverBuilder {
    target: String,
    agent_path: Option<PathBuf>,
    remote_agent_path: String,
    key_dir: Option<PathBuf>,
    connect_key: Option<String>,
    version: Option<String>,
    timeout: Duration,
    startup_delay: Duration,
}

#[derive(Clone)]
pub struct UiDriver {
    inner: Rc<RefCell<UiSession>>,
    handle: String,
}

#[derive(Clone)]
pub struct UiComponent {
    inner: Rc<RefCell<UiSession>>,
    handle: String,
}

#[derive(Clone)]
pub struct UiWindow {
    inner: Option<Rc<RefCell<UiSession>>>,
    handle: String,
}

#[derive(Debug, Clone)]
pub struct UiSelector {
    filters: Vec<SelectorFilter>,
    index: usize,
    is_before: bool,
    is_after: bool,
}

#[derive(Clone)]
pub struct UiQuery {
    ui: UiDriver,
    selector: UiSelector,
}

#[derive(Debug, Clone)]
enum SelectorFilter {
    Text(String),
    Id(String),
    Key(String),
    Kind(String),
    Description(String),
    Enabled(bool),
    Clickable(bool),
    Focused(bool),
    Selected(bool),
    Checked(bool),
    LongClickable(bool),
    Scrollable(bool),
    Checkable(bool),
}

struct UiSession {
    driver: Driver,
    _forward: TcpForwardHandle,
    reader: TcpStream,
    writer: TcpStream,
}

impl UiDriverBuilder {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            agent_path: None,
            remote_agent_path: DEFAULT_REMOTE_AGENT_PATH.to_string(),
            key_dir: None,
            connect_key: None,
            version: None,
            timeout: Duration::from_secs(20),
            startup_delay: Duration::from_millis(500),
        }
    }

    pub fn agent_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.agent_path = Some(path.into());
        self
    }

    pub fn remote_agent_path(mut self, path: impl Into<String>) -> Self {
        self.remote_agent_path = path.into();
        self
    }

    pub fn key_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.key_dir = Some(path.into());
        self
    }

    pub fn connect_key(mut self, value: impl Into<String>) -> Self {
        self.connect_key = Some(value.into());
        self
    }

    pub fn version(mut self, value: impl Into<String>) -> Self {
        self.version = Some(value.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn startup_delay(mut self, delay: Duration) -> Self {
        self.startup_delay = delay;
        self
    }

    pub fn connect(self) -> Result<UiDriver> {
        let mut driver_builder = Driver::builder(self.target).timeout(self.timeout);
        if let Some(key_dir) = self.key_dir {
            driver_builder = driver_builder.key_dir(key_dir);
        }
        if let Some(connect_key) = self.connect_key {
            driver_builder = driver_builder.connect_key(connect_key);
        }
        if let Some(version) = self.version {
            driver_builder = driver_builder.version(version);
        }
        let mut driver = driver_builder.connect()?;
        let agent_path = resolve_agent_path(self.agent_path)?;

        kill_uitest_daemon(&mut driver)?;
        push_agent(&mut driver, &agent_path, &self.remote_agent_path)?;
        start_uitest_daemon(&mut driver)?;
        thread::sleep(self.startup_delay);

        let local_port = free_local_port()?;
        let forward = driver.forward_tcp(local_port, UITEST_SERVICE_PORT)?;
        let stream = TcpStream::connect(("127.0.0.1", local_port))?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        let reader = stream.try_clone()?;

        let inner = Rc::new(RefCell::new(UiSession {
            driver,
            _forward: forward,
            reader,
            writer: stream,
        }));

        let handle = {
            let mut session = inner.borrow_mut();
            session
                .invoke("Driver.create", None, Vec::new())?
                .as_str()
                .ok_or_else(|| HdcError::protocol("Driver.create returned invalid handle"))?
                .to_string()
        };

        Ok(UiDriver { inner, handle })
    }
}

impl UiDriver {
    pub fn builder(target: impl Into<String>) -> UiDriverBuilder {
        UiDriverBuilder::new(target)
    }

    pub fn display_size(&self) -> Result<Point> {
        let value = self.invoke("Driver.getDisplaySize", Vec::new())?;
        parse_point(&value)
    }

    pub fn display_rotation(&self) -> Result<DisplayRotation> {
        let value = self.invoke("Driver.getDisplayRotation", Vec::new())?;
        let raw = value.as_i64().ok_or_else(|| {
            HdcError::protocol("Driver.getDisplayRotation returned invalid payload")
        })?;
        DisplayRotation::from_value(raw as i32)
    }

    pub fn set_display_rotation(&self, rotation: DisplayRotation) -> Result<()> {
        let _ = self.invoke(
            "Driver.setDisplayRotation",
            vec![Value::from(rotation.value())],
        )?;
        Ok(())
    }

    pub fn find_window(&self, active: bool) -> Result<Option<UiWindow>> {
        let value = self.invoke("Driver.findWindow", vec![json!({ "actived": active })])?;
        Ok(value
            .as_str()
            .map(|handle| UiWindow::new(self.inner.clone(), handle)))
    }

    pub fn find_active_window(&self) -> Result<Option<UiWindow>> {
        self.find_window(true)
    }

    pub fn click<X, Y>(&self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let point = self.resolve_point(x.into(), y.into())?;
        self.shell_action(&format!("uitest uiInput click {} {}", point.x, point.y))
    }

    pub fn double_click<X, Y>(&self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let point = self.resolve_point(x.into(), y.into())?;
        self.shell_action(&format!(
            "uitest uiInput doubleClick {} {}",
            point.x, point.y
        ))
    }

    pub fn long_click<X, Y>(&self, x: X, y: Y) -> Result<()>
    where
        X: Into<Coord>,
        Y: Into<Coord>,
    {
        let point = self.resolve_point(x.into(), y.into())?;
        self.shell_action(&format!("uitest uiInput longClick {} {}", point.x, point.y))
    }

    pub fn swipe<X1, Y1, X2, Y2>(&self, x1: X1, y1: Y1, x2: X2, y2: Y2, speed: u32) -> Result<()>
    where
        X1: Into<Coord>,
        Y1: Into<Coord>,
        X2: Into<Coord>,
        Y2: Into<Coord>,
    {
        let start = self.resolve_point(x1.into(), y1.into())?;
        let end = self.resolve_point(x2.into(), y2.into())?;
        self.shell_action(&format!(
            "uitest uiInput swipe {} {} {} {} {}",
            start.x, start.y, end.x, end.y, speed
        ))
    }

    pub fn select(&self, selector: UiSelector) -> UiQuery {
        UiQuery {
            ui: self.clone(),
            selector,
        }
    }

    pub fn query(&self) -> UiQuery {
        UiQuery::new(self.clone())
    }

    pub fn text(&self, value: impl Into<String>) -> UiQuery {
        self.query().text(value)
    }

    pub fn id(&self, value: impl Into<String>) -> UiQuery {
        self.query().id(value)
    }

    pub fn key(&self, value: impl Into<String>) -> UiQuery {
        self.query().key(value)
    }

    pub fn kind(&self, value: impl Into<String>) -> UiQuery {
        self.query().kind(value)
    }

    pub fn description(&self, value: impl Into<String>) -> UiQuery {
        self.query().description(value)
    }

    pub fn find_component(&self, selector: UiSelector) -> Result<Option<UiComponent>> {
        let mut components = self.find_components(selector.clone())?;
        Ok(components.drain(..).nth(selector.index))
    }

    pub fn find_one(&self, selector: UiSelector) -> Result<Option<UiComponent>> {
        self.find_component(selector)
    }

    pub fn find_components(&self, selector: UiSelector) -> Result<Vec<UiComponent>> {
        let by = self.selector_handle(&selector)?;
        let value = self.invoke("Driver.findComponents", vec![Value::from(by)])?;
        let array = value
            .as_array()
            .ok_or_else(|| HdcError::protocol("Driver.findComponents returned invalid payload"))?;
        Ok(array
            .iter()
            .filter_map(Value::as_str)
            .map(|handle| UiComponent::new(self.inner.clone(), handle))
            .collect())
    }

    pub fn find_all(&self, selector: UiSelector) -> Result<Vec<UiComponent>> {
        self.find_components(selector)
    }

    pub fn exists(&self, selector: UiSelector) -> Result<bool> {
        Ok(self.find_component(selector)?.is_some())
    }

    pub fn wait_for_component(
        &self,
        selector: UiSelector,
        timeout_ms: u64,
    ) -> Result<Option<UiComponent>> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if let Some(component) = self.find_component(selector.clone())? {
                return Ok(Some(component));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            thread::sleep((deadline - now).min(Duration::from_millis(250)));
        }
    }

    pub fn watch_toast_once(&self) -> Result<bool> {
        let value = self.invoke("Driver.uiEventObserverOnce", vec![Value::from("toastShow")])?;
        value.as_bool().ok_or_else(|| {
            HdcError::protocol("Driver.uiEventObserverOnce returned invalid payload")
        })
    }

    pub fn recent_ui_event(&self, timeout_ms: u64) -> Result<Option<UiEvent>> {
        let value = self.invoke("Driver.getRecentUiEvent", vec![Value::from(timeout_ms)])?;
        if value.is_null() {
            return Ok(None);
        }
        Ok(Some(parse_ui_event(&value)?))
    }

    fn resolve_point(&self, x: Coord, y: Coord) -> Result<Point> {
        let size = self.display_size()?;
        Ok(Point {
            x: x.resolve(size.x)?,
            y: y.resolve(size.y)?,
        })
    }

    fn selector_handle(&self, selector: &UiSelector) -> Result<String> {
        let mut current = "On#seed".to_string();
        for filter in selector.filters.iter().cloned() {
            let (api, arg) = filter.into_api_arg();
            let value = self.invoke_on(&api, &current, arg)?;
            current = value
                .as_str()
                .ok_or_else(|| {
                    HdcError::protocol(format!("{api} returned invalid selector handle"))
                })?
                .to_string();
        }
        if selector.is_before {
            let value = self.invoke_on("On.isBefore", "On#seed", Value::from(current.clone()))?;
            current = value
                .as_str()
                .ok_or_else(|| HdcError::protocol("On.isBefore returned invalid selector handle"))?
                .to_string();
        }
        if selector.is_after {
            let value = self.invoke_on("On.isAfter", "On#seed", Value::from(current.clone()))?;
            current = value
                .as_str()
                .ok_or_else(|| HdcError::protocol("On.isAfter returned invalid selector handle"))?
                .to_string();
        }
        Ok(current)
    }

    fn invoke(&self, api: &str, args: Vec<Value>) -> Result<Value> {
        self.inner
            .borrow_mut()
            .invoke(api, Some(self.handle.as_str()), args)
    }

    fn shell_action(&self, command: &str) -> Result<()> {
        shell_checked(&mut self.inner.borrow_mut().driver, command)
    }

    fn invoke_on(&self, api: &str, this: &str, arg: Value) -> Result<Value> {
        self.inner.borrow_mut().invoke(api, Some(this), vec![arg])
    }
}

impl UiComponent {
    fn new(inner: Rc<RefCell<UiSession>>, handle: &str) -> Self {
        Self {
            inner,
            handle: handle.to_string(),
        }
    }

    pub fn text(&self) -> Result<String> {
        self.invoke("Component.getText", Vec::new())?
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| HdcError::protocol("Component.getText returned invalid payload"))
    }

    pub fn id(&self) -> Result<String> {
        self.invoke_string("Component.getId")
    }

    pub fn key(&self) -> Result<String> {
        self.invoke_string("Component.getId")
    }

    pub fn kind(&self) -> Result<String> {
        self.invoke_string("Component.getType")
    }

    pub fn description(&self) -> Result<String> {
        self.invoke_string("Component.getDescription")
    }

    pub fn bounds(&self) -> Result<Bounds> {
        let value = self.invoke("Component.getBounds", Vec::new())?;
        parse_bounds(&value)
    }

    pub fn center(&self) -> Result<Point> {
        Ok(self.bounds()?.center())
    }

    pub fn enabled(&self) -> Result<bool> {
        self.invoke_bool("Component.isEnabled")
    }

    pub fn clickable(&self) -> Result<bool> {
        self.invoke_bool("Component.isClickable")
    }

    pub fn focused(&self) -> Result<bool> {
        self.invoke_bool("Component.isFocused")
    }

    pub fn checkable(&self) -> Result<bool> {
        self.invoke_bool("Component.isCheckable")
    }

    pub fn selected(&self) -> Result<bool> {
        self.invoke_bool("Component.isSelected")
    }

    pub fn checked(&self) -> Result<bool> {
        self.invoke_bool("Component.isChecked")
    }

    pub fn long_clickable(&self) -> Result<bool> {
        self.invoke_bool("Component.isLongClickable")
    }

    pub fn scrollable(&self) -> Result<bool> {
        self.invoke_bool("Component.isScrollable")
    }

    pub fn exists(&self) -> Result<bool> {
        Ok(self.bounds().is_ok())
    }

    pub fn info(&self) -> Result<UiComponentInfo> {
        let bounds = self.bounds()?;
        let center = bounds.center();
        Ok(UiComponentInfo {
            id: self.id()?,
            key: self.key()?,
            kind: self.kind()?,
            text: self.text()?,
            description: self.description()?,
            selected: self.selected()?,
            checked: self.checked()?,
            enabled: self.enabled()?,
            focused: self.focused()?,
            checkable: self.checkable()?,
            clickable: self.clickable()?,
            long_clickable: self.long_clickable()?,
            scrollable: self.scrollable()?,
            bounds,
            center,
        })
    }

    pub fn click(&self) -> Result<()> {
        let center = self.center()?;
        self.inner.borrow_mut().driver.click(center.x, center.y)
    }

    pub fn click_if_exists(&self) -> Result<bool> {
        if !self.exists()? {
            return Ok(false);
        }
        self.click()?;
        Ok(true)
    }

    pub fn double_click(&self) -> Result<()> {
        let center = self.center()?;
        self.inner
            .borrow_mut()
            .driver
            .double_click(center.x, center.y)
    }

    pub fn long_click(&self) -> Result<()> {
        let center = self.center()?;
        self.inner
            .borrow_mut()
            .driver
            .long_click(center.x, center.y)
    }

    pub fn input_text(&self, text: &str) -> Result<()> {
        let center = self.center()?;
        self.inner.borrow_mut().driver.click(center.x, center.y)?;
        self.inner.borrow_mut().driver.input_text(text)
    }

    pub fn clear_text(&self) -> Result<()> {
        let _ = self.invoke("Component.clearText", Vec::new())?;
        Ok(())
    }

    pub fn pinch_in(&self, scale: f64) -> Result<()> {
        let _ = self.invoke("Component.pinchIn", vec![Value::from(scale)])?;
        Ok(())
    }

    pub fn pinch_out(&self, scale: f64) -> Result<()> {
        let _ = self.invoke("Component.pinchOut", vec![Value::from(scale)])?;
        Ok(())
    }

    pub fn drag_to(&self, target: &UiComponent) -> Result<()> {
        let from = self.center()?;
        let to = target.center()?;
        self.inner
            .borrow_mut()
            .driver
            .drag(from.x, from.y, to.x, to.y, Some(2000))
    }

    fn invoke(&self, api: &str, args: Vec<Value>) -> Result<Value> {
        self.inner
            .borrow_mut()
            .invoke(api, Some(self.handle.as_str()), args)
    }

    fn invoke_bool(&self, api: &str) -> Result<bool> {
        self.invoke(api, Vec::new())?
            .as_bool()
            .ok_or_else(|| HdcError::protocol(format!("{api} returned invalid payload")))
    }

    fn invoke_string(&self, api: &str) -> Result<String> {
        self.invoke(api, Vec::new())?
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| HdcError::protocol(format!("{api} returned invalid payload")))
    }
}

impl UiWindow {
    fn new(inner: Rc<RefCell<UiSession>>, handle: &str) -> Self {
        Self {
            inner: Some(inner),
            handle: handle.to_string(),
        }
    }

    pub fn handle(&self) -> &str {
        &self.handle
    }

    pub fn bounds(&self) -> Result<Bounds> {
        let value = self.invoke("UiWindow.getBounds", Vec::new())?;
        parse_bounds(&value)
    }

    pub fn display_id(&self) -> Result<i32> {
        let value = self.invoke("UiWindow.getBounds", Vec::new())?;
        read_i32_field(&value, "displayId")
    }

    pub fn title(&self) -> Result<String> {
        self.invoke("UiWindow.getTitle", Vec::new())?
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| HdcError::protocol("UiWindow.getTitle returned invalid payload"))
    }

    pub fn is_focused(&self) -> Result<bool> {
        self.invoke("UiWindow.isFocused", Vec::new())?
            .as_bool()
            .ok_or_else(|| HdcError::protocol("UiWindow.isFocused returned invalid payload"))
    }

    pub fn bundle_name(&self) -> Result<String> {
        self.invoke("UiWindow.getBundleName", Vec::new())?
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| HdcError::protocol("UiWindow.getBundleName returned invalid payload"))
    }

    pub fn window_mode(&self) -> Result<i32> {
        let value = self.invoke("UiWindow.getWindowMode", Vec::new())?;
        value
            .as_i64()
            .and_then(|item| i32::try_from(item).ok())
            .ok_or_else(|| HdcError::protocol("UiWindow.getWindowMode returned invalid payload"))
    }

    pub fn is_active(&self) -> Result<bool> {
        self.invoke("UiWindow.isActived", Vec::new())?
            .as_bool()
            .ok_or_else(|| HdcError::protocol("UiWindow.isActived returned invalid payload"))
    }

    fn invoke(&self, api: &str, args: Vec<Value>) -> Result<Value> {
        self.inner
            .as_ref()
            .ok_or_else(|| HdcError::protocol("ui window is detached from session"))?
            .borrow_mut()
            .invoke(api, Some(self.handle.as_str()), args)
    }
}

impl UiSelector {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            index: 0,
            is_before: false,
            is_after: false,
        }
    }

    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.filters.push(SelectorFilter::Text(value.into()));
        self
    }

    pub fn id(mut self, value: impl Into<String>) -> Self {
        self.filters.push(SelectorFilter::Id(value.into()));
        self
    }

    pub fn key(mut self, value: impl Into<String>) -> Self {
        self.filters.push(SelectorFilter::Key(value.into()));
        self
    }

    pub fn kind(mut self, value: impl Into<String>) -> Self {
        self.filters.push(SelectorFilter::Kind(value.into()));
        self
    }

    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.filters.push(SelectorFilter::Description(value.into()));
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Enabled(value));
        self
    }

    pub fn clickable(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Clickable(value));
        self
    }

    pub fn focused(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Focused(value));
        self
    }

    pub fn selected(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Selected(value));
        self
    }

    pub fn checked(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Checked(value));
        self
    }

    pub fn long_clickable(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::LongClickable(value));
        self
    }

    pub fn scrollable(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Scrollable(value));
        self
    }

    pub fn checkable(mut self, value: bool) -> Self {
        self.filters.push(SelectorFilter::Checkable(value));
        self
    }

    pub fn index(mut self, value: usize) -> Self {
        self.index = value;
        self
    }

    pub fn is_before(mut self, value: bool) -> Self {
        self.is_before = value;
        self
    }

    pub fn is_after(mut self, value: bool) -> Self {
        self.is_after = value;
        self
    }
}

impl Default for UiSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectorFilter {
    fn into_api_arg(self) -> (String, Value) {
        match self {
            Self::Text(value) => ("On.text".to_string(), Value::from(value)),
            Self::Id(value) => ("On.id".to_string(), Value::from(value)),
            Self::Key(value) => ("On.key".to_string(), Value::from(value)),
            Self::Kind(value) => ("On.type".to_string(), Value::from(value)),
            Self::Description(value) => ("On.description".to_string(), Value::from(value)),
            Self::Enabled(value) => ("On.enabled".to_string(), Value::from(value)),
            Self::Clickable(value) => ("On.clickable".to_string(), Value::from(value)),
            Self::Focused(value) => ("On.focused".to_string(), Value::from(value)),
            Self::Selected(value) => ("On.selected".to_string(), Value::from(value)),
            Self::Checked(value) => ("On.checked".to_string(), Value::from(value)),
            Self::LongClickable(value) => ("On.longClickable".to_string(), Value::from(value)),
            Self::Scrollable(value) => ("On.scrollable".to_string(), Value::from(value)),
            Self::Checkable(value) => ("On.checkable".to_string(), Value::from(value)),
        }
    }
}

impl UiQuery {
    const DEFAULT_RETRIES: u32 = 2;
    const DEFAULT_WAIT: Duration = Duration::from_secs(1);

    fn new(ui: UiDriver) -> Self {
        Self {
            ui,
            selector: UiSelector::new(),
        }
    }

    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.selector = self.selector.text(value);
        self
    }

    pub fn id(mut self, value: impl Into<String>) -> Self {
        self.selector = self.selector.id(value);
        self
    }

    pub fn key(mut self, value: impl Into<String>) -> Self {
        self.selector = self.selector.key(value);
        self
    }

    pub fn kind(mut self, value: impl Into<String>) -> Self {
        self.selector = self.selector.kind(value);
        self
    }

    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.selector = self.selector.description(value);
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.selector = self.selector.enabled(value);
        self
    }

    pub fn clickable(mut self, value: bool) -> Self {
        self.selector = self.selector.clickable(value);
        self
    }

    pub fn focused(mut self, value: bool) -> Self {
        self.selector = self.selector.focused(value);
        self
    }

    pub fn selected(mut self, value: bool) -> Self {
        self.selector = self.selector.selected(value);
        self
    }

    pub fn checked(mut self, value: bool) -> Self {
        self.selector = self.selector.checked(value);
        self
    }

    pub fn long_clickable(mut self, value: bool) -> Self {
        self.selector = self.selector.long_clickable(value);
        self
    }

    pub fn scrollable(mut self, value: bool) -> Self {
        self.selector = self.selector.scrollable(value);
        self
    }

    pub fn checkable(mut self, value: bool) -> Self {
        self.selector = self.selector.checkable(value);
        self
    }

    pub fn index(mut self, value: usize) -> Self {
        self.selector = self.selector.index(value);
        self
    }

    pub fn nth(self, value: usize) -> Self {
        self.index(value)
    }

    pub fn is_before(mut self, value: bool) -> Self {
        self.selector = self.selector.is_before(value);
        self
    }

    pub fn is_after(mut self, value: bool) -> Self {
        self.selector = self.selector.is_after(value);
        self
    }

    pub fn count(&self) -> Result<usize> {
        Ok(self.ui.find_components(self.selector.clone())?.len())
    }

    pub fn all(&self) -> Result<Vec<UiComponent>> {
        self.ui.find_components(self.selector.clone())
    }

    pub fn first(&self) -> Result<Option<UiComponent>> {
        self.find_component()
    }

    pub fn must_find(&self) -> Result<UiComponent> {
        self.require_component(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)
    }

    pub fn find_component(&self) -> Result<Option<UiComponent>> {
        self.find_component_with_retry(1, Duration::from_secs(0))
    }

    pub fn find_component_with_retry(
        &self,
        retries: u32,
        wait_time: Duration,
    ) -> Result<Option<UiComponent>> {
        let attempts = retries.max(1);
        for attempt in 0..attempts {
            if let Some(component) = self.ui.find_component(self.selector.clone())? {
                return Ok(Some(component));
            }
            if attempt + 1 < attempts {
                thread::sleep(wait_time);
            }
        }
        Ok(None)
    }

    pub fn exists(&self) -> Result<bool> {
        self.exists_with_retry(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)
    }

    pub fn wait(&self, timeout: Duration) -> Result<Option<UiComponent>> {
        self.ui
            .wait_for_component(self.selector.clone(), timeout.as_millis() as u64)
    }

    pub fn must_wait(&self, timeout: Duration) -> Result<UiComponent> {
        self.wait(timeout)?
            .ok_or_else(|| HdcError::protocol("ui component not found before timeout"))
    }

    pub fn exists_with_retry(&self, retries: u32, wait_time: Duration) -> Result<bool> {
        Ok(self
            .find_component_with_retry(retries, wait_time)?
            .is_some())
    }

    pub fn click(&self) -> Result<()> {
        self.click_with_retry(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)
    }

    pub fn click_with_retry(&self, retries: u32, wait_time: Duration) -> Result<()> {
        let component = self.require_component(retries, wait_time)?;
        component.click()
    }

    pub fn double_click(&self) -> Result<()> {
        let component = self.require_component(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)?;
        component.double_click()
    }

    pub fn long_click(&self) -> Result<()> {
        let component = self.require_component(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)?;
        component.long_click()
    }

    pub fn input_text(&self, text: &str) -> Result<()> {
        let component = self.require_component(Self::DEFAULT_RETRIES, Self::DEFAULT_WAIT)?;
        component.input_text(text)
    }

    fn require_component(&self, retries: u32, wait_time: Duration) -> Result<UiComponent> {
        self.find_component_with_retry(retries, wait_time)?
            .ok_or_else(|| HdcError::protocol("ui component not found"))
    }
}

impl UiSession {
    fn invoke(&mut self, api: &str, this: Option<&str>, args: Vec<Value>) -> Result<Value> {
        let request_id = format!(
            "{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_micros())
                .unwrap_or(0)
        );
        let request = json!({
            "module": "com.ohos.devicetest.hypiumApiHelper",
            "method": "callHypiumApi",
            "params": {
                "api": api,
                "this": this,
                "args": args,
                "message_type": "hypium"
            },
            "request_id": request_id
        });

        let payload = serde_json::to_vec(&request)?;
        self.writer.write_all(&payload)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut buffer = vec![0_u8; 4096];
        let size = self.reader.read(&mut buffer)?;
        if size == 0 {
            return Err(HdcError::protocol(format!(
                "{api} returned an empty response"
            )));
        }
        let response: Value = serde_json::from_slice(&buffer[..size])?;
        if let Some(exception) = response.get("exception") {
            if !exception.is_null() {
                return Err(HdcError::protocol(format!(
                    "{api} failed: {}",
                    exception
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| exception.to_string())
                )));
            }
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }
}

fn resolve_agent_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        if path.is_file() {
            return Ok(path);
        }
        return Err(HdcError::protocol(format!(
            "agent file not found: {}",
            path.display()
        )));
    }

    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("third_party/hmdriver2/hmdriver2/assets/uitest_agent_v1.1.0.so"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("third_party/hmdriver2/hmdriver2/assets/uitest_agent_v1.0.7.so"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            HdcError::protocol(
                "failed to locate agent.so, pass an explicit path with UiDriverBuilder::agent_path",
            )
        })
}

fn kill_uitest_daemon(driver: &mut Driver) -> Result<()> {
    let output = shell_stdout(driver, "ps -ef")?;
    for line in output.lines() {
        if !line.contains("uitest start-daemon singleness") {
            continue;
        }
        let pid = line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| HdcError::protocol("failed to parse uitest daemon pid"))?;
        let _ = shell_checked(driver, &format!("kill -9 {pid}"));
    }
    Ok(())
}

fn push_agent(driver: &mut Driver, local_path: &Path, remote_path: &str) -> Result<()> {
    let remote_staging_path = format!("{remote_path}.upload");
    let _ = shell_checked(
        driver,
        &format!(
            "rm -f {} {}",
            shell_escape(&remote_staging_path),
            shell_escape(remote_path)
        ),
    );
    driver.send_file(local_path, &remote_staging_path)?;
    shell_checked(
        driver,
        &format!(
            "cp {} {}",
            shell_escape(&remote_staging_path),
            shell_escape(remote_path)
        ),
    )?;
    shell_checked(driver, &format!("chmod +x {}", shell_escape(remote_path)))?;
    Ok(())
}

fn start_uitest_daemon(driver: &mut Driver) -> Result<()> {
    shell_checked(driver, "uitest start-daemon singleness")
}

fn shell_checked(driver: &mut Driver, command: &str) -> Result<()> {
    let result = driver.shell(command)?;
    if result.failed() {
        let message = result
            .messages
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<&str>>()
            .join(" | ");
        return Err(HdcError::protocol(format!("{command} failed: {message}")));
    }
    Ok(())
}

fn shell_stdout(driver: &mut Driver, command: &str) -> Result<String> {
    let result = driver.shell(command)?;
    if result.failed() {
        let message = result
            .messages
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<&str>>()
            .join(" | ");
        return Err(HdcError::protocol(format!("{command} failed: {message}")));
    }
    Ok(result.stdout_text().into_owned())
}

fn free_local_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn parse_point(value: &Value) -> Result<Point> {
    Ok(Point {
        x: read_i32_field(value, "x")?,
        y: read_i32_field(value, "y")?,
    })
}

fn parse_bounds(value: &Value) -> Result<Bounds> {
    Ok(Bounds {
        left: read_i32_field(value, "left")?,
        right: read_i32_field(value, "right")?,
        top: read_i32_field(value, "top")?,
        bottom: read_i32_field(value, "bottom")?,
    })
}

fn parse_ui_event(value: &Value) -> Result<UiEvent> {
    Ok(UiEvent {
        bundle_name: read_string_field(value, "bundleName")?,
        text: read_string_field(value, "text")?,
        kind: read_string_field(value, "type")?,
    })
}

fn read_i32_field(value: &Value, key: &str) -> Result<i32> {
    let raw = value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| HdcError::protocol(format!("missing integer field `{key}`")))?;
    i32::try_from(raw).map_err(|_| HdcError::protocol(format!("field `{key}` is out of range")))
}

fn read_string_field(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| HdcError::protocol(format!("missing string field `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::{
        SelectorFilter, UiQuery, UiSelector, UiWindow, parse_bounds, parse_point, parse_ui_event,
        shell_escape,
    };
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn selector_filter_maps_text_to_on_text() {
        let (api, value) = SelectorFilter::Text("Settings".to_string()).into_api_arg();
        assert_eq!(api, "On.text");
        assert_eq!(value, json!("Settings"));
    }

    #[test]
    fn parse_point_reads_xy_fields() {
        let point = parse_point(&json!({"x": 1260, "y": 2720})).unwrap();
        assert_eq!(point.x, 1260);
        assert_eq!(point.y, 2720);
    }

    #[test]
    fn parse_bounds_reads_edge_fields() {
        let bounds = parse_bounds(&json!({"left": 1, "right": 3, "top": 5, "bottom": 7})).unwrap();
        assert_eq!(bounds.left, 1);
        assert_eq!(bounds.right, 3);
        assert_eq!(bounds.top, 5);
        assert_eq!(bounds.bottom, 7);
    }

    #[test]
    fn parse_ui_event_reads_payload_fields() {
        let event = parse_ui_event(&json!({
            "bundleName": "com.example.app",
            "text": "hello",
            "type": "Toast"
        }))
        .unwrap();
        assert_eq!(event.bundle_name, "com.example.app");
        assert_eq!(event.text, "hello");
        assert_eq!(event.kind, "Toast");
    }

    #[test]
    fn shell_escape_quotes_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn selector_filter_maps_long_clickable_to_on_long_clickable() {
        let (api, value) = SelectorFilter::LongClickable(true).into_api_arg();
        assert_eq!(api, "On.longClickable");
        assert_eq!(value, json!(true));
    }

    #[test]
    fn selector_tracks_index_and_relative_flags() {
        let selector = UiSelector::new()
            .text("Settings")
            .index(2)
            .is_before(true)
            .is_after(false);

        assert_eq!(selector.filters.len(), 1);
        assert_eq!(selector.index, 2);
        assert!(selector.is_before);
        assert!(!selector.is_after);
    }

    #[test]
    fn ui_query_defaults_match_hmdriver_style_polling() {
        assert_eq!(UiQuery::DEFAULT_RETRIES, 2);
        assert_eq!(UiQuery::DEFAULT_WAIT, Duration::from_secs(1));
    }

    #[test]
    fn selector_chain_supports_extended_filters() {
        let selector = UiSelector::new()
            .text("Settings")
            .enabled(true)
            .long_clickable(false)
            .scrollable(true)
            .checkable(false)
            .index(1)
            .is_after(true);

        assert_eq!(selector.filters.len(), 5);
        assert_eq!(selector.index, 1);
        assert!(selector.is_after);
    }

    #[test]
    fn ui_query_wait_converts_duration_to_timeout_ms() {
        let timeout = Duration::from_millis(1500);

        assert_eq!(timeout.as_millis() as u64, 1500);
    }

    #[test]
    fn ui_window_exposes_raw_handle() {
        let window = UiWindow {
            inner: None,
            handle: "UiWindow#10".to_string(),
        };

        assert_eq!(window.handle(), "UiWindow#10");
    }
}
