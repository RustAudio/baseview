#[cfg(feature = "tracing")]
pub use tracing::warn;

#[cfg(all(feature = "tracing", not(target_os = "macos")))]
pub use tracing::error;

#[cfg(not(feature = "tracing"))]
mod tracing_impl {
    macro_rules! __warn {
        ($($f:tt)*) => {
            {
                let _ = ($($f)*);
            }
        };
    }

    pub(crate) use __warn as warn;
    #[cfg(target_os = "macos")]
    pub(crate) use __warn as error;
}

#[cfg(not(feature = "tracing"))]
pub(crate) use tracing_impl::*;
