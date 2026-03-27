# hmdriver_rs API

本文档总结当前 crate 对外暴露的公开接口。以 [`src/lib.rs`](../src/lib.rs) 的 `pub use` 和公开模块为准，不包含内部协议、编解码、认证和 session 细节。

## 总览

当前 API 主要分成 4 层：

1. `Driver`
   - 主入口。
   - 负责设备连接、shell、应用管理、输入、截图、窗口与 mission 查询、文件发送、TCP 转发。
2. `UiDriver`
   - 基于 `agent.so + uitest daemon + Hypium` 的 UI 自动化入口。
   - 负责控件查找、组件操作、窗口对象和 UI 事件。
3. 查询与扩展对象
   - `UiSelector`
   - `UiQuery`
   - `UiComponent`
   - `UiWindow`
   - `XPathNode`
   - `SwipeExt`
4. 数据类型与错误类型
   - `Result<T>`
   - `HdcError`
   - `ShellResult`
   - `DeviceInfo`
   - `WindowList`
   - `MissionList`
   - 以及其他强类型结构

## 主入口：Driver

### 创建连接

```rust
use hmdriver_rs::Driver;

let mut driver = Driver::builder("192.168.8.43:35319")
    .connect()?;
```

也保留兼容别名：

- `HdcDriver = Driver`
- `HdcDriverBuilder = DriverBuilder`

### DriverBuilder

构建器类型：

- `Driver::builder(addr) -> DriverBuilder`
- `DriverBuilder::new(addr) -> DriverBuilder`
- `DriverBuilder::key_dir(path) -> Self`
- `DriverBuilder::version(version) -> Self`
- `DriverBuilder::connect_key(connect_key) -> Self`
- `DriverBuilder::timeout(Duration) -> Self`
- `DriverBuilder::connect() -> Result<Driver>`

### Driver 方法

#### Shell 与应用管理

- `shell(command) -> Result<ShellResult>`
- `list_apps(include_system_apps) -> Result<Vec<String>>`
- `start_app(bundle, ability) -> Result<()>`
- `stop_app(bundle) -> Result<()>`
- `current_app() -> Result<Option<CurrentApp>>`
- `has_app(bundle) -> Result<bool>`
- `app_version(bundle) -> Result<AppVersion>`
- `get_app_info(bundle) -> Result<serde_json::Value>`
- `get_app_abilities(bundle) -> Result<Vec<AppAbilityInfo>>`
- `get_app_main_ability(bundle) -> Result<Option<AppAbilityInfo>>`
- `open_url(url) -> Result<()>`

#### 设备与显示

- `device_info() -> Result<DeviceInfo>`
- `display_size() -> Result<Point>`
- `display_rotation() -> Result<DisplayRotation>`
- `set_display_rotation(rotation) -> Result<()>`
- `screen_on() -> Result<()>`
- `screen_off() -> Result<()>`
- `unlock() -> Result<()>`

#### 窗口与 mission

- `list_windows() -> Result<WindowList>`
- `get_window(window_id) -> Result<WindowDetail>`
- `list_missions() -> Result<MissionList>`
- `list_windows_with_missions() -> Result<CorrelatedWindowList>`
- `correlate_windows_to_missions() -> Result<CorrelatedWindowList>`
  - 兼容别名，等价于 `list_windows_with_missions()`
- `find_window(active) -> Result<Option<UiWindow>>`
- `find_active_window() -> Result<Option<UiWindow>>`

#### 输入与交互

- `click(x, y) -> Result<()>`
- `double_click(x, y) -> Result<()>`
- `long_click(x, y) -> Result<()>`
- `right_click(x, y) -> Result<()>`
- `swipe(x1, y1, x2, y2, speed) -> Result<()>`
- `drag(x1, y1, x2, y2, speed) -> Result<()>`
- `swipe_ext() -> SwipeExt<'_>`
- `input_text(text) -> Result<()>`
- `press_key(key) -> Result<()>`
- `press_keys(keys) -> Result<()>`
- `go_home() -> Result<()>`
- `go_back() -> Result<()>`

说明：

- 坐标参数支持像素值，也支持 `0.0..=1.0` 百分比值。
- `press_keys` 在 3 键以内优先走 `uitest uiInput keyEvent`，更长组合键走 `uinput`。

#### 截图、层级、文件与转发

- `screenshot(path) -> Result<PathBuf>`
- `dump_hierarchy() -> Result<serde_json::Value>`
- `send_file(local_path, remote_path) -> Result<()>`
- `forward_tcp(local_port, remote_port) -> Result<TcpForwardHandle>`

#### UI 自动化桥接

- `ui() -> Result<UiDriver>`
- `xpath(expression) -> Result<XPathNode<'_>>`
- `select(selector) -> Result<UiQuery>`
- `query() -> Result<UiQuery>`
- `text(value) -> Result<UiQuery>`
- `id(value) -> Result<UiQuery>`
- `key(value) -> Result<UiQuery>`
- `kind(value) -> Result<UiQuery>`
- `description(value) -> Result<UiQuery>`

#### 生命周期

- `close() -> Result<()>`

## UI 自动化：UiDriver

### UiDriverBuilder

```rust
use hmdriver_rs::UiDriver;

let ui = UiDriver::builder("192.168.8.43:35319")
    .connect()?;
```

构建器方法：

- `UiDriver::builder(target) -> UiDriverBuilder`
- `UiDriverBuilder::new(target) -> UiDriverBuilder`
- `agent_path(path) -> Self`
- `remote_agent_path(path) -> Self`
- `key_dir(path) -> Self`
- `connect_key(value) -> Self`
- `version(value) -> Self`
- `timeout(Duration) -> Self`
- `startup_delay(Duration) -> Self`
- `connect() -> Result<UiDriver>`

默认情况下，`UiDriverBuilder` 会在仓库内查找：

- `assets/uitest/uitest_agent_v1.1.0.so`

### UiDriver 方法

#### 显示与窗口

- `display_size() -> Result<Point>`
- `display_rotation() -> Result<DisplayRotation>`
- `set_display_rotation(rotation) -> Result<()>`
- `find_window(active) -> Result<Option<UiWindow>>`
- `find_active_window() -> Result<Option<UiWindow>>`

#### 坐标动作

- `click(x, y) -> Result<()>`
- `double_click(x, y) -> Result<()>`
- `long_click(x, y) -> Result<()>`
- `swipe(x1, y1, x2, y2, speed) -> Result<()>`

#### 查询入口

- `select(selector) -> UiQuery`
- `query() -> UiQuery`
- `text(value) -> UiQuery`
- `id(value) -> UiQuery`
- `key(value) -> UiQuery`
- `kind(value) -> UiQuery`
- `description(value) -> UiQuery`

#### 组件查找

- `find_component(selector) -> Result<Option<UiComponent>>`
- `find_one(selector) -> Result<Option<UiComponent>>`
- `find_components(selector) -> Result<Vec<UiComponent>>`
- `find_all(selector) -> Result<Vec<UiComponent>>`
- `exists(selector) -> Result<bool>`
- `wait_for_component(selector, timeout_ms) -> Result<Option<UiComponent>>`

#### UI 事件

- `watch_toast_once() -> Result<bool>`
- `recent_ui_event(timeout_ms) -> Result<Option<UiEvent>>`

## UiSelector

`UiSelector` 是不可变 builder，用来描述控件筛选条件。

创建与条件：

- `UiSelector::new()`
- `text(value)`
- `id(value)`
- `key(value)`
- `kind(value)`
- `description(value)`
- `enabled(bool)`
- `clickable(bool)`
- `focused(bool)`
- `selected(bool)`
- `checked(bool)`
- `long_clickable(bool)`
- `scrollable(bool)`
- `checkable(bool)`
- `index(usize)`
- `is_before(bool)`
- `is_after(bool)`

## UiQuery

`UiQuery` 是基于 `UiDriver + UiSelector` 的链式查询对象，更接近 `hmdriver2` 的使用方式。

### 条件拼接

- `text(value)`
- `id(value)`
- `key(value)`
- `kind(value)`
- `description(value)`
- `enabled(bool)`
- `clickable(bool)`
- `focused(bool)`
- `selected(bool)`
- `checked(bool)`
- `long_clickable(bool)`
- `scrollable(bool)`
- `checkable(bool)`
- `index(usize)`
- `nth(usize)`
- `is_before(bool)`
- `is_after(bool)`

### 查询与等待

- `count() -> Result<usize>`
- `all() -> Result<Vec<UiComponent>>`
- `first() -> Result<Option<UiComponent>>`
- `must_find() -> Result<UiComponent>`
- `find_component() -> Result<Option<UiComponent>>`
- `find_component_with_retry(retries, wait_time) -> Result<Option<UiComponent>>`
- `exists() -> Result<bool>`
- `wait(Duration) -> Result<Option<UiComponent>>`
- `must_wait(Duration) -> Result<UiComponent>`
- `exists_with_retry(retries, wait_time) -> Result<bool>`

### 直接动作

- `click() -> Result<()>`
- `click_with_retry(retries, wait_time) -> Result<()>`
- `double_click() -> Result<()>`
- `long_click() -> Result<()>`
- `input_text(text) -> Result<()>`

## UiComponent

`UiComponent` 表示一个已经找到的组件句柄。

### 属性读取

- `text() -> Result<String>`
- `id() -> Result<String>`
- `key() -> Result<String>`
- `kind() -> Result<String>`
- `description() -> Result<String>`
- `bounds() -> Result<Bounds>`
- `center() -> Result<Point>`
- `enabled() -> Result<bool>`
- `clickable() -> Result<bool>`
- `focused() -> Result<bool>`
- `checkable() -> Result<bool>`
- `selected() -> Result<bool>`
- `checked() -> Result<bool>`
- `long_clickable() -> Result<bool>`
- `scrollable() -> Result<bool>`
- `exists() -> Result<bool>`
- `info() -> Result<UiComponentInfo>`

### 动作

- `click() -> Result<()>`
- `click_if_exists() -> Result<bool>`
- `double_click() -> Result<()>`
- `long_click() -> Result<()>`
- `input_text(text) -> Result<()>`
- `clear_text() -> Result<()>`
- `pinch_in(scale) -> Result<()>`
- `pinch_out(scale) -> Result<()>`
- `drag_to(target) -> Result<()>`

## UiWindow

`UiWindow` 表示一个 Hypium 窗口句柄。当前只暴露只读元数据接口。

- `handle() -> &str`
- `bounds() -> Result<Bounds>`
- `display_id() -> Result<i32>`
- `title() -> Result<String>`
- `is_focused() -> Result<bool>`
- `bundle_name() -> Result<String>`
- `window_mode() -> Result<i32>`
- `is_active() -> Result<bool>`

## XPathNode

通过 `Driver::xpath(expr)` 获取。

当前实现的是实用子集 XPath，不是完整 XPath 引擎。支持常见的：

- `//Type[@text='...']`
- `and`
- `or`
- `contains(...)`
- `starts-with(...)`
- `text()` 作为 `text` 别名

公开方法：

- `exists() -> bool`
- `text() -> Result<Option<String>>`
- `bounds() -> Result<Option<Bounds>>`
- `center() -> Result<Option<Point>>`
- `click() -> Result<()>`
- `click_if_exists() -> Result<bool>`
- `double_click() -> Result<()>`
- `long_click() -> Result<()>`
- `input_text(text) -> Result<()>`

## SwipeExt

`SwipeExt` 是 `Driver::swipe_ext()` 返回的方向滑动封装。

### SwipeDirection

- `Left`
- `Right`
- `Up`
- `Down`

### SwipeArea

矩形区域定义：

- `SwipeArea::new(left, top, right, bottom) -> SwipeArea`

坐标同样支持像素或百分比值。

### SwipeExt 方法

- `swipe(direction, scale, area, speed) -> Result<()>`
- `left(scale, area, speed) -> Result<()>`
- `right(scale, area, speed) -> Result<()>`
- `up(scale, area, speed) -> Result<()>`
- `down(scale, area, speed) -> Result<()>`

## 数据类型

### 错误与结果

- `type Result<T> = std::result::Result<T, HdcError>`
- `enum HdcError`
  - `Io`
  - `OpenSsl`
  - `Utf8`
  - `ParseInt`
  - `Json`
  - `Protocol`
  - `Cli`

辅助构造：

- `HdcError::protocol(message)`
- `HdcError::cli(message)`

### 命令结果

- `CommandStatus`
  - `Ok`
  - `FailedHint`
- `DriverMessageLevel`
  - `Fail`
  - `Info`
  - `Ok`
  - `Unknown(u8)`
- `DriverMessage`
- `ShellResult`
  - `stdout_text()`
  - `failed()`

### 坐标与几何

- `Coord`
  - `Pixels(i32)`
  - `Percent(f64)`
- `Point`
- `Bounds`
  - `center()`

### 显示与设备

- `DisplayRotation`
  - `Rotation0`
  - `Rotation90`
  - `Rotation180`
  - `Rotation270`
  - `from_value(value)`
  - `value()`
- `DeviceInfo`

### 应用与前台状态

- `CurrentApp`
- `AppVersion`
- `AppAbilityInfo`

### UI 数据

- `UiEvent`
- `UiComponentInfo`

### 窗口与 mission

- `WindowRect`
- `WindowOffset`
- `WindowScale`
- `WindowEntry`
- `WindowList`
- `WindowDetail`
- `MissionEntry`
- `MissionList`
- `CorrelatedWindow`
- `CorrelatedWindowList`

### 按键

- `KeyCode`
  - 常用常量：
    - `HOME`
    - `BACK`
    - `POWER`
    - `CTRL_LEFT`
    - `CTRL_RIGHT`
    - `ALT_LEFT`
    - `ALT_RIGHT`
    - `SHIFT_LEFT`
    - `SHIFT_RIGHT`
    - `ENTER`
    - `DEL`
    - `A..Z`
  - `raw() -> u32`

## TcpForwardHandle

`Driver::forward_tcp()` 返回 `TcpForwardHandle`。

当前它是一个生命周期句柄：

- 没有公开方法
- `Drop` 时自动停止本地监听和转发线程

典型用法：

```rust
let _forward = driver.forward_tcp(18012, 8012)?;
```

只要 `_forward` 还活着，转发就保持有效。

## CLI 模块

`cli` 模块也对外公开，主要服务于内置二进制入口。

类型：

- `Cli`

方法：

- `Cli::parse() -> Result<Cli>`
- `Cli::parse_from(args) -> Result<Cli>`
- `shell_command() -> String`
- `effective_connect_key() -> String`

## 推荐接入方式

如果你是库使用方，建议优先按下面这层级使用：

1. 日常设备控制：`Driver`
2. UI 自动化：`Driver::ui()` 或 `Driver::text()/query()/select()`
3. 窗口与 mission 检查：`list_windows()` / `list_windows_with_missions()`
4. 更细粒度 UI 对象：`UiComponent` / `UiWindow`
5. 纯查询场景：`xpath()`

最外层推荐范式：

```rust
let mut driver = Driver::builder("192.168.8.43:35319").connect()?;

let info = driver.device_info()?;
let current = driver.current_app()?;
let windows = driver.list_windows_with_missions()?;
let ui = driver.ui()?;
let login = ui.text("登录").must_find()?;
login.click()?;
```
