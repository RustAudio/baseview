#![allow(unused, reason = "Some platform may not use all macros")]

#[cfg(feature = "tracing")]
pub use tracing::{debug, error, warn};

#[cfg(not(feature = "tracing"))]
mod tracing_impl {
    macro_rules! __void {
        ($($f:tt)*) => {
            {
                let _ = ($($f)*);
            }
        };
    }

    pub(crate) use __void as debug;
    pub(crate) use __void as error;
    pub(crate) use __void as warn;
}

#[cfg(not(feature = "tracing"))]
pub(crate) use tracing_impl::*;
