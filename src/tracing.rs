#[cfg(feature = "tracing")]
pub use tracing::{error, warn};

#[cfg(not(feature = "tracing"))]
mod tracing_impl {
    macro_rules! __warn {
        ($($f:tt)*) => {
            #[allow(unused, dead_code)]
            {
                let _ = ($($f)*);
            }
        };
    }

    pub(crate) use __warn as warn;
    pub(crate) use __warn as error;
}

#[cfg(not(feature = "tracing"))]
pub(crate) use tracing_impl::*;
