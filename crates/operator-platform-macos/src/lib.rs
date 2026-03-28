//! macOS platform driver foundations for Operator.

mod apps;
mod capture;
mod driver;
mod effects;
mod input;
mod inspect;
mod locator;
mod permissions;

pub use apps::{AppService, SystemAppService};
pub use capture::{CaptureProvider, CaptureResult, SystemCaptureProvider};
pub use driver::MacosDriver;
#[cfg(feature = "action-effects")]
pub use effects::try_run_action_effect_helper;
pub use input::{InputSynthesizer, SystemInputSynthesizer};
pub use inspect::{InspectResult, SystemTreeInspector, TreeInspector};
pub use permissions::{PermissionReader, SystemPermissionReader};
pub(crate) use permissions::{
    ACCESSIBILITY_CHECK_ID, SCREEN_RECORDING_CHECK_ID, SYSTEM_EVENTS_CHECK_ID,
};
