pub mod cli;
pub mod driver;
pub mod error;
pub mod types;

mod auth;
mod codec;
mod protocol;
mod session;

pub use driver::{Driver, DriverBuilder, HdcDriver, HdcDriverBuilder};
pub use error::{HdcError, Result};
pub use types::{
    CommandStatus, Coord, CurrentApp, DriverMessage, DriverMessageLevel, KeyCode, ShellResult,
};
