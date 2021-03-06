//! The testing backend (disabled).
//!
//! Add a feature flag `testing` to enable the testing backend.
use std::panic;

#[path = "testing/logging.rs"]
#[allow(dead_code)]
mod logging;
#[path = "testing/wmapi.rs"]
pub mod wmapi;
pub use self::{logging::Logger, wmapi::TestingWm};

/// Call `with_testing_wm` if the testing backend is enabled. Otherwise,
/// output a warning message and return without calling the givne function.
///
/// This function is available even if the `testing` feature flag is disabled.
pub fn run_test(_cb: impl FnOnce(&dyn TestingWm) + Send + panic::UnwindSafe + 'static) {
    #[allow(clippy::explicit_write)] // bypass output redirection
    {
        use std::io::Write;
        writeln!(
            std::io::stderr(),
            "warning: testing backend is disabled, skipping some tests"
        )
        .unwrap();
    }
}
