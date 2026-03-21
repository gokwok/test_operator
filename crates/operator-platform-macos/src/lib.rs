//! macOS platform driver foundations for Operator.

mod apps;
mod capture;
mod driver;
mod input;
mod inspect;
mod locator;
mod permissions;

pub use apps::{AppService, SystemAppService};
pub use capture::{CaptureProvider, CaptureResult, SystemCaptureProvider};
pub use driver::MacosDriver;
pub use input::{InputSynthesizer, SystemInputSynthesizer};
pub use inspect::{InspectResult, SystemTreeInspector, TreeInspector};
pub use permissions::{PermissionReader, SystemPermissionReader};
