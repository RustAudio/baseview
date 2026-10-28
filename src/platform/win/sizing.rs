use crate::platform::{BaseviewWindow, PlatformError, PlatformResult};
use crate::wrappers::win32::window::{HWnd, WmSizing};
use crate::wrappers::win32::{Dpi, DpiAwarenessGuard, Rect, WindowStyle};
use crate::{HandlerError, WindowSize};
use dpi::PhysicalSize;
use tracing::warn;

impl BaseviewWindow {
    fn adapt_host_window_to_size(
        &self, previous_size: PhysicalSize<u32>, new_size: WindowSize,
    ) -> Result<(), PlatformError> {
        if let Err(e) = self.request_host_resize(new_size) {
            warn!("Resize request from Host failed: {}. Reverting to previous size.", e);

            if let Some(handler) = self.handler.get() {
                if let Err(e) = handler.resized(new_size) {
                    warn!("Window Handler failed to resize to previous window size: {}", e);
                }
            };

            self.window_state.shared.current_size.set(previous_size);
            if let Err(e) = self.window_state.resize(previous_size.into()) {
                warn!("Failed to resize back to previous window size: {}", e);
            }

            return Err(e.into());
        }

        Ok(())
    }

    fn request_host_resize(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        if self.shared_state.resize_host_originated.get() {
            return Ok(());
        };

        self.host.request_resize(new_size)
    }

    pub fn handle_new_size_on_show(&self) {
        if let Some(previous_size) = self.host_needs_new_size_notified_on_show.take() {
            let new_size = self.shared_state.current_size.get();
            let dpi = self.shared_state.current_dpi.get();

            let _ = self.adapt_host_window_to_size(
                previous_size,
                WindowSize::from_physical(new_size, dpi.scale_factor()),
            );
        }
    }

    pub fn initialize_window_dpi(&self, window: HWnd) -> Result<(), PlatformError> {
        let dpi = self
            .shared_state
            .dpi_scaling_strategy
            .get()
            .get_dpi_for_window(window, &self.shared_state.user32);

        let Some(dpi) = dpi else { return Ok(()) };

        if dpi == self.shared_state.current_dpi.get() {
            // If the host queried size before it gave us a parent, this is the one it's been using.
            let previous_host_size = self.initial_size.to_physical(1.0);
            let current_size = self.initial_size.to_physical(dpi.scale_factor());

            if previous_host_size != current_size {
                self.host_needs_new_size_notified_on_show.set(Some(previous_host_size));
            }
        } else {
            self.shared_state.current_dpi.set(dpi);

            // We cannot create a window in "logical" pixels, and we can't DPI-scale to physical pixels because we
            // have no way to know where the window will end up.
            // So, at window creation, we assume a DPI=96 (or parent DPI if available), and if it ends up wrong, we resize the window
            // to the actual logical size the user desired.
            let new_size = self.initial_size.to_physical(dpi.scale_factor());

            // Preemptively update so a synchronous WM_SIZE from SetWindowPos below
            // doesn't also emit Resized.
            let previous_size = self.shared_state.current_size.replace(new_size);
            let guard = DpiAwarenessGuard::new(
                &self.shared_state.user32,
                self.shared_state.dpi_scaling_strategy.get(),
            )?;
            window.resize_and_activate(new_size, dpi, &guard)?;

            self.host_needs_new_size_notified_on_show.set(Some(previous_size));
        }

        Ok(())
    }
}

impl WmSizing for BaseviewWindow {
    fn on_size(&self, window: HWnd, new_size: PhysicalSize<u32>) -> Result<(), ()> {
        let current_size = self.shared_state.current_size.get();

        // Only send the event if anything changed
        if current_size == new_size {
            return Ok(());
        }

        let previous = self.shared_state.current_size.replace(new_size);
        let new_size = WindowSize::from_physical(new_size, self.shared_state.scale_factor());

        let Some(handler) = self.handler.get() else { return Ok(()) };
        if let Err(e) = handler.resized(new_size) {
            warn!("Window Handler failed to resize: {}", e);
            self.shared_state.current_size.set(previous);

            if let Err(e) = self.window_state.resize(previous.into()) {
                warn!("Failed to resize back to previous window size: {}", e);
            }

            return Err(());
        }

        if let Err(e) = self.request_host_resize(new_size) {
            warn!("Resize request from Host failed: {}. Reverting to previous size.", e);

            if let Err(e) = handler.resized(new_size) {
                warn!("Window Handler failed to resize to previous window size: {}", e);
            }

            self.shared_state.current_size.set(previous);
            if let Err(e) = self.window_state.resize(previous.into()) {
                warn!("Failed to resize back to previous window size: {}", e);
            }

            return Err(());
        }

        window.invalidate_window();

        Ok(())
    }

    fn on_dpi_changed(
        &self, window: HWnd, new_dpi: Dpi, suggested_nc_rect: Rect,
    ) -> PlatformResult<()> {
        let dpi_ctx = DpiAwarenessGuard::new(
            &self.shared_state.user32,
            self.shared_state.dpi_scaling_strategy.get(),
        )?;
        let style = window.get_style()?;
        let suggested_rect =
            dpi_ctx.nc_area_to_client_area(suggested_nc_rect, style, Some(new_dpi))?;

        let new_size = suggested_rect.size();

        let changed = self.shared_state.current_size.get() != new_size
            || self.shared_state.current_dpi.get() != new_dpi;

        self.shared_state.current_dpi.set(new_dpi);
        let previous_size = self.shared_state.current_size.replace(new_size);

        // Windows makes us resize the window manually. This however will not send a WM_SIZE event,
        // hence why we are notifying the window handler manually below.
        let _ = window.set_nc_rect(suggested_nc_rect);

        if changed {
            let Some(handler) = self.handler.get() else { return Ok(()) };
            let new_size = WindowSize::from_physical(new_size, new_dpi.scale_factor());

            if let Err(e) = handler.resized(new_size) {
                warn!("Window Handler failed to resize: {}", e);
                self.shared_state.current_size.set(previous_size);

                if let Err(e) = self.window_state.resize(previous_size.into()) {
                    warn!("Failed to resize back to previous window size: {}", e);
                }

                return Err(e.into());
            }

            self.adapt_host_window_to_size(previous_size, new_size)?;
        }

        Ok(())
    }

    fn get_min_max_size(
        &self, window: HWnd,
    ) -> (Option<PhysicalSize<i32>>, Option<PhysicalSize<i32>>) {
        let sizing = self.shared_state.sizing_strategy;

        // Only implement this message if we actually need to specify a min/max size
        if let (None, None) = (sizing.min_size(), sizing.max_size()) {
            return (None, None);
        }

        let Ok(ctx) = DpiAwarenessGuard::new(
            &self.shared_state.user32,
            self.shared_state.dpi_scaling_strategy.get(),
        ) else {
            return (None, None);
        };

        let style = window.get_style().unwrap_or(WindowStyle::dummy());
        let dpi = self.shared_state.current_dpi.get();

        let min_size = if let Some(size) = sizing.min_size() {
            let size = size.to_physical(self.shared_state.scale_factor());
            let nc_size = ctx.client_area_to_nc_area(size.into(), style, Some(dpi));

            nc_size.ok().map(|rect| rect.size().cast())
        } else {
            None
        };

        let max_size = if let Some(size) = sizing.max_size() {
            let size = size.to_physical(self.shared_state.scale_factor());

            let nc_size = ctx.client_area_to_nc_area(size.into(), style, Some(dpi));

            nc_size.ok().map(|rect| rect.size().cast())
        } else {
            None
        };

        (min_size, max_size)
    }
}
