use crate::window_handler::OpenWindowExample;
use crate::ExamplePluginMainThread;
use baseview::dpi::*;
use baseview::host::{Host, HostCallbacks, HostFdSupport, HostTimerSupport, TimerHandle};
use baseview::{Window, WindowSettings, WindowSize};
use clack_extensions::gui::{
    AspectRatioStrategy, GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, HostGui,
    PluginGuiImpl, Window as ClapWindow,
};
use clack_extensions::posix_fd::{FdFlags, HostPosixFd};
use clack_extensions::timer::{HostTimer, TimerId};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::HostMainThreadHandle;
use std::error::Error;
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::time::Duration;

pub struct ExamplePluginGui {
    pub handle: Window,
}

impl PluginGuiImpl for ExamplePluginMainThread<'_> {
    fn is_api_supported(&self, configuration: GuiConfiguration) -> bool {
        !configuration.is_floating
            && Some(configuration.api_type) == GuiApiType::default_for_current_platform()
    }

    fn get_preferred_api(&self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform()?,
            is_floating: false,
        })
    }

    fn create(&self, _configuration: GuiConfiguration) -> Result<(), PluginError> {
        let options = WindowSettings::new()
            .wait_for_parent()
            .with_size(PhysicalSize::new(400, 200))
            .with_min_size(LogicalSize::new(200.0, 100.0))
            .with_max_size(LogicalSize::new(600.0, 400.0));

        // SAFETY: this handle is valid for the lifetime of the plugin, and baseview guarantees
        // it releases all references to host callbacks on `drop` (which we do unconditionally)
        let host_handle = unsafe { self.host.with_arbitrary_lifetime() };

        let mut host = Host::new();

        if let Some(gui) = self.host_gui {
            host = host.with_callbacks(HostGuiCallbacks { ext: gui, host: host_handle });
        }

        if let Some(fd) = self.host_posix_fd {
            host = host.with_fd(HostFdCallbacks { ext: fd, host: host_handle });
        }

        if let Some(timer) = self.host_timer {
            host = host.with_timer(HostTimerCallbacks { ext: timer, host: host_handle })
        }

        let window = Window::create_with_host(options, OpenWindowExample::new, host)?;

        self.gui.replace(Some(ExamplePluginGui { handle: window }));
        Ok(())
    }

    fn destroy(&self) {
        let Some(gui) = self.gui.take() else { return };

        gui.handle.close()
    }

    fn set_scale(&self, scale: f64) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_scale called without a GUI active"));
        };
        gui.suggest_fallback_scale_factor(scale)?;

        Ok(())
    }

    fn get_size(&self) -> Option<GuiSize> {
        let Some(gui) = self.borrow_window() else {
            eprintln!("get_size called without a GUI active");
            return None;
        };

        let size = gui.size().to_native_size();

        Some(GuiSize { width: size.width, height: size.height })
    }

    fn can_resize(&self) -> bool {
        let Some(gui) = self.borrow_window() else { return false };

        gui.is_resizable()
    }

    fn get_resize_hints(&self) -> Option<GuiResizeHints> {
        let can_resize = self.can_resize();

        Some(GuiResizeHints {
            strategy: AspectRatioStrategy::Disregard, // Not supported

            can_resize_vertically: can_resize,
            can_resize_horizontally: can_resize,
        })
    }

    fn adjust_size(&self, size: GuiSize) -> Option<GuiSize> {
        let gui = self.borrow_window()?;

        let size = gui.adjust_size(NativeSize::new(size.width, size.height));

        Some(GuiSize { width: size.width, height: size.height })
    }

    fn set_size(&self, size: GuiSize) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_size called without a GUI active"));
        };

        gui.resize(NativeSize { width: size.width, height: size.height })?;

        Ok(())
    }

    fn set_parent(&self, window: ClapWindow) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_parent called without a GUI active"));
        };

        // SAFETY: The CLAP spec ensures the parent window handle is valid for at least this call
        let parent = unsafe { window.borrow_handle_unchecked()? };

        gui.set_parent(&parent)?;
        gui.show()?;

        Ok(())
    }

    fn set_transient(&self, _window: ClapWindow) -> Result<(), PluginError> {
        unimplemented!() // Not supported yet
    }

    fn suggest_title(&self, _title: &str) {
        // Not supported yet
    }

    fn show(&self) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("show called without a GUI active"));
        };
        gui.show()?;

        Ok(())
    }

    fn hide(&self) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("hide called without a GUI active"));
        };
        gui.show()?;

        Ok(())
    }
}

struct HostGuiCallbacks {
    ext: HostGui,
    host: HostMainThreadHandle<'static>,
}

impl HostCallbacks for HostGuiCallbacks {
    fn request_resize(&self, new_size: WindowSize) -> Result<(), Box<dyn Error>> {
        let new_size = new_size.to_native_size();
        self.ext.request_resize(&self.host, new_size.width, new_size.height)?;
        Ok(())
    }

    fn destroyed(&self) {
        self.ext.closed(&self.host, true);
    }
}

struct HostTimerCallbacks {
    ext: HostTimer,
    host: HostMainThreadHandle<'static>,
}

impl HostTimerSupport for HostTimerCallbacks {
    fn register_timer(&self, period: Duration) -> Result<TimerHandle, Box<dyn Error>> {
        let period_ms = period.as_millis().try_into().unwrap_or(u32::MAX);
        let timer = self.ext.register_timer(&self.host, period_ms)?;
        Ok(TimerHandle(timer.0))
    }

    fn unregister_timer(&self, timer: TimerHandle) -> Result<(), Box<dyn Error>> {
        self.ext.unregister_timer(&self.host, TimerId(timer.0))?;
        Ok(())
    }
}

struct HostFdCallbacks {
    ext: HostPosixFd,
    host: HostMainThreadHandle<'static>,
}

impl HostFdSupport for HostFdCallbacks {
    fn register_fd(&self, fd: BorrowedFd) -> Result<(), Box<dyn Error>> {
        self.ext.register_fd(&self.host, fd.as_raw_fd(), FdFlags::READ)?;
        Ok(())
    }

    fn unregister_fd(&self, fd: RawFd) -> Result<(), Box<dyn Error>> {
        self.ext.unregister_fd(&self.host, fd)?;
        Ok(())
    }
}
