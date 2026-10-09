use super::prelude::*;
use crate::platform::x11::host_handle::HostHandle;
use crate::utils::SizingStrategy;
use crate::{WindowSettings, WindowSize};
use std::cell::Cell;
use std::num::{NonZero, NonZeroU32};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use x11rb::properties::WmSizeHints;
use x11rb::protocol::xproto::{ConfigureNotifyEvent, ReparentNotifyEvent};

pub struct SizingState {
    new_size: Cell<Option<PhysicalSize<u16>>>,
    new_parent_size: Cell<Option<PhysicalSize<u16>>>,
    parent_id: Cell<Option<NonZeroU32>>,
}

impl SizingState {
    pub fn new(parent_id: Option<NonZeroU32>) -> Self {
        Self { new_size: None.into(), new_parent_size: None.into(), parent_id: parent_id.into() }
    }

    pub fn handle_parent_notify(&self, e: ReparentNotifyEvent) {
        self.parent_id.set(NonZero::new(e.parent));
    }

    pub fn non_coalesced_current_size(&self, shared: &WindowShared) -> PhysicalSize<u16> {
        self.new_size.get().unwrap_or_else(|| shared.sizing_state.size())
    }

    pub fn handle_coalesced_resize_events(
        &self, shared: &WindowShared, handler: &Handler, host_handle: &HostHandle,
    ) -> Result<(), FatalError> {
        let mut comes_from_parent = false;

        if let Some(new_parent_size) = self.new_parent_size.take() {
            if new_parent_size != shared.sizing_state.size() {
                // The parent was resized, which means we should resize ourselves too.
                if let Err(e) = shared.xcb_window.resize(new_parent_size.cast()) {
                    crate::warn!("Failed to resize window: {}", e);
                } else {
                    // Makes the rest of this function run on the new parent size immediately (without waiting for a ConfigureNotify round-trip)
                    // Also overrides any new sizes we may have received this event loop iteration,it would probably be invalidated anyway
                    self.new_size.set(Some(new_parent_size));
                    comes_from_parent = true;
                }
            }
        }

        let Some(new_size) = self.new_size.take() else { return Ok(()) };
        let previous = shared.sizing_state.store_size(new_size);

        if previous == new_size {
            return Ok(());
        };

        let scale_factor = shared.sizing_state.scale_factor();
        let new_size = shared.sizing_state.window_size();

        if let Err(()) = handler.resize(new_size) {
            shared.sizing_state.store_size(previous);
            shared.xcb_window.resize(previous.cast())?.check_warn();
            return Ok(());
        }

        // Host requests use resize_immediately, which stops the previous == new_size condition
        // So if we're here, it's guaranteed not to be from a host request

        if !comes_from_parent {
            if let Err(()) = host_handle.request_resize(new_size) {
                let previous = WindowSize::from_physical(previous.cast(), scale_factor);
                todo!() // roll back
            }
        }

        // Immediately schedule a redraw, do not wait for an "expose" event
        shared.present_state.request_present_notify();

        Ok(())
    }

    pub fn handle_configure_notify_event(&self, event: ConfigureNotifyEvent, window: &XcbWindow) {
        if event.window == 0 {
            return;
        }

        // These are coalesced and then handled asynchronously at the end of the event loop
        if event.window == window.id().get() {
            self.new_size.set(Some(PhysicalSize::new(event.width, event.height)));
        } else if self.parent_id.get().is_some_and(|pid| pid.get() == event.window) {
            // Also resize the window if the parent is resized
            // This works around some hosts that might not call set_size() right away (or at all...)
            self.new_parent_size.set(Some(PhysicalSize::new(event.width, event.height)));
        }
    }

    pub fn handle_host_resize(
        &self, new_size: Size, handler: &Handler, shared: &WindowShared,
    ) -> Result<(), PlatformError> {
        let scale_factor = shared.sizing_state.scale_factor();
        let new_size = new_size.to_physical(scale_factor);

        shared.sizing_state.resize_from_host(new_size, handler, shared)
    }

    pub fn handle_host_suggest_scale_factor(
        &self, scale: f64, handler: &Handler, shared: &WindowShared,
    ) -> Result<(), PlatformError> {
        shared.sizing_state.host_suggested_scale_factor.set(Some(scale));

        // If the scaling factor is already provided by the system, do nothing
        if shared.sizing_state.system_scale_factor.get().is_some() {
            return Ok(());
        };

        let current_logical_size = shared.sizing_state.size().to_logical::<f64>(1.0);
        let new_physical_size = current_logical_size.to_physical(scale);

        shared.sizing_state.resize_from_host(new_physical_size, handler, shared)
    }
}

pub struct SizingStateShared {
    window_size: Cell<PhysicalSize<u16>>,
    sizing_strategy: SizingStrategy,

    system_scale_factor: Cell<Option<f64>>,
    host_suggested_scale_factor: Cell<Option<f64>>,
}

impl SizingStateShared {
    pub fn load(connection: &X11Connection, settings: &WindowSettings) -> Result<Self, FatalError> {
        let scaling = connection.resources.xft_dpi.map(|dpi| dpi as f64 / 96.0);
        let initial_scale_factor = scaling.unwrap_or(1.0);

        let sizing_strategy = SizingStrategy::from_settings(settings);
        let window_size = settings.size.to_physical(initial_scale_factor);

        Ok(Self {
            sizing_strategy,
            window_size: window_size.into(),
            system_scale_factor: scaling.into(),
            host_suggested_scale_factor: settings.fallback_scale_factor.into(),
        })
    }

    pub fn scale_factor(&self) -> f64 {
        if let Some(factor) = self.system_scale_factor.get() {
            return factor;
        };

        if let Some(factor) = self.host_suggested_scale_factor.get() {
            return factor;
        }

        1.0
    }

    pub fn size(&self) -> PhysicalSize<u16> {
        self.window_size.get()
    }

    pub fn window_size(&self) -> WindowSize {
        WindowSize::from_physical(self.window_size.get().cast(), self.scale_factor())
    }

    pub fn make_size_hints(&self) -> WmSizeHints {
        get_size_hints(&self.sizing_strategy, self.window_size.get(), self.scale_factor())
    }

    pub fn store_size(&self, size: PhysicalSize<u16>) -> PhysicalSize<u16> {
        self.window_size.replace(size)
    }

    pub fn resize_from_handler(&self, size: Size, window: &XcbWindow) -> PlatformResult<()> {
        let new_size = self.sizing_strategy.adjust_size(size, self.window_size()).physical;

        if new_size == self.window_size.get().cast() {
            return Ok(());
        }

        window.resize(new_size)?.check()?;

        if !self.sizing_strategy.is_resizable() {
            let size_hints = get_size_hints(&self.sizing_strategy, new_size, self.scale_factor());
            window.set_size_hints(size_hints)?.check()?;
        }

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it

        Ok(())
    }

    fn resize_from_host(
        &self, new_size: PhysicalSize<u16>, handler: &Handler, shared: &WindowShared,
    ) -> PlatformResult<()> {
        let previous = self.store_size(new_size);

        if previous == new_size {
            return Ok(());
        };

        if let Err(()) =
            handler.resize(WindowSize::from_physical(new_size.cast(), self.scale_factor()))
        {
            self.store_size(previous);
            return Ok(());
        }

        shared.xcb_window.resize(new_size.cast())?.check()?; // Will not call handler, as size is the same as above.
        if !self.sizing_strategy.is_resizable() {
            let size_hints = get_size_hints(&self.sizing_strategy, new_size, self.scale_factor());
            shared.xcb_window.set_size_hints(size_hints)?.check()?;
        }

        // These come from the Host, no need to notify it about the new size

        Ok(())
    }
}

pub fn get_size_hints(
    strategy: &SizingStrategy, current_size: PhysicalSize<impl Pixel>, scale_factor: f64,
) -> WmSizeHints {
    let mut size_hints = WmSizeHints::default();

    match strategy {
        SizingStrategy::Fixed => {
            size_hints.min_size = Some(to_size_hint(current_size));
            size_hints.max_size = size_hints.min_size;
        }
        SizingStrategy::Resizable { min_size, max_size } => {
            size_hints.min_size =
                min_size.map(|s| to_size_hint(s.to_physical::<i32>(scale_factor)));
            size_hints.max_size =
                max_size.map(|s| to_size_hint(s.to_physical::<i32>(scale_factor)));
        }
    }

    size_hints
}

fn to_size_hint(size: PhysicalSize<impl Pixel>) -> (i32, i32) {
    let size = size.cast();
    (size.width, size.height)
}
