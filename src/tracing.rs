#![allow(unused, reason = "Some platform may not use all macros")]

#[cfg(feature = "tracing")]
pub use tracing::{debug, debug_span, error, span, warn};

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

    pub struct Span;
    pub struct SpanGuard;
    impl Span {
        pub fn entered(&self) -> SpanGuard {
            SpanGuard
        }
    }

    macro_rules! __span {
        ($($f:tt)*) => {
            {
                let _ = ($($f)*);
                crate::Span
            }
        };
    }

    pub(crate) use __span as span;
    pub(crate) use __span as debug_span;
}

#[cfg(not(feature = "tracing"))]
pub(crate) use tracing_impl::*;
