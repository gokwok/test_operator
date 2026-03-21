//! macOS platform driver foundations for Operator.

mod apps;
mod driver;
mod permissions;

pub use apps::{AppService, SystemAppService};
pub use driver::MacosDriver;
pub use permissions::{PermissionReader, SystemPermissionReader};
