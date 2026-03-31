// Imported from hmdriver_rs via git subtree; keep this module tree close to upstream.

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
mod swipe;
mod xpath;

pub use driver::{Driver, DriverBuilder, HdcDriver, HdcDriverBuilder};
pub use error::{HdcError, Result};
pub use forward::TcpForwardHandle;
pub use swipe::{SwipeArea, SwipeDirection, SwipeExt};
pub use types::{
    AppAbilityInfo, AppLabelInfo, AppVersion, Bounds, CommandStatus, Coord, CorrelatedWindow,
    CorrelatedWindowList, CurrentApp, DeviceInfo, DisplayRotation, DriverMessage,
    DriverMessageLevel, KeyCode, MissionEntry, MissionList, Point, ShellResult, UiComponentInfo,
    UiEvent, WindowDetail, WindowEntry, WindowList, WindowOffset, WindowRect, WindowScale,
};
pub use ui::{UiComponent, UiDriver, UiDriverBuilder, UiQuery, UiSelector, UiWindow};
pub use xpath::XPathNode;
