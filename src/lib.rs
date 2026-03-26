pub mod cli;
pub mod driver;
pub mod error;
pub mod types;
pub mod ui;

mod auth;
mod codec;
mod forward;
mod protocol;
mod session;
mod xpath;

pub use driver::{Driver, DriverBuilder, HdcDriver, HdcDriverBuilder};
pub use error::{HdcError, Result};
pub use forward::TcpForwardHandle;
pub use types::{
    AppAbilityInfo, AppVersion, Bounds, CommandStatus, Coord, CurrentApp, DeviceInfo,
    DisplayRotation, DriverMessage, DriverMessageLevel, KeyCode, Point, ShellResult,
    UiComponentInfo, UiEvent,
};
pub use ui::{UiComponent, UiDriver, UiDriverBuilder, UiQuery, UiSelector};
pub use xpath::XPathNode;
