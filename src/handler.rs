use super::*;
use crate::platform::Result;

#[non_exhaustive]
pub enum DamageArea {
    FullWindow,
    // Later: single rect, perhaps list of rects
}

pub trait WindowHandler: 'static {
    /// Requests the handler to draw a new frame immediately.
    ///
    /// In order to reduce resource usage, this method is not called systematically at every frame
    /// interval.
    ///
    /// This method is automatically scheduled to be called in several situations:
    ///
    /// * When the window is first opened and shown to the user;
    /// * When the window is shown after being previously hidden;
    /// * When the window is resized;
    /// * When the platform explicitly requests a window redraw (such as redrawing what was previously obscured by another window).
    ///
    /// In those situations, the [`draw`] call will be preceded by a [`damage`] call, specifying which
    /// parts of the window need to be redrawn.
    ///
    /// This method can also be scheduled to be called from a call to [`WindowContext::request_redraw`],
    /// as a result of [polling] or any kind of event.
    ///
    /// If this method itself calls [`WindowContext::request_redraw`], then a redraw will be
    /// scheduled for the next frame interval.
    /// This is useful for handling animations that need to run for successive frames in order to look smooth.
    ///
    /// Platforms may wait until this method completes before performing operations, such as
    /// presenting the parent window or updating window decorations.
    ///
    /// Therefore, implementations should perform and complete all drawing inside this method
    /// (or return an error), in order to minimize rendering artifacts.
    ///
    /// # Errors
    ///
    /// If this returns an error, the window will be considered unable to render its contents, and
    /// will be subsequently closed.
    ///
    /// [`draw`]: WindowHandler::draw
    /// [`damage`]: WindowHandler::damage
    /// [polling]: WindowHandler::poll
    fn draw(&self) -> core::result::Result<(), HandlerError>;

    /// Notifies the handler that a given [`area`] of the window has been damaged and needs to be redrawn.
    ///
    /// This is useful for renderers that support partial rendering, in order to not have to redraw
    /// every single pixel on every frame.
    ///
    /// Implementing this method is optional, as it's only useful if the handler supports partial rendering.
    /// The default implementation of this method does nothing.
    ///
    /// [`area`]: DamageArea
    fn damage(&self, area: DamageArea) {
        let _ = area;
    }

    /// Requests the handler to poll updates from external sources, in preparation for rendering a new frame.
    ///
    /// This is useful when UIs need to check periodically on some sources (such as channels, queues, shared values, etc.)
    /// to figure if they need updating.
    ///
    /// If received updates must result in the window updating its contents, then this method should
    /// call [`WindowContext::request_redraw`].
    ///
    /// Implementing this method is optional, and not needed if the window does not need to update itself
    /// from external events. The default implementation for this method does nothing.
    ///
    /// # External update logic
    ///
    /// The goal is for all external update logic to be contained within this method.
    /// Therefore, this method will be called in various cases, including (but not limited to):
    ///
    /// * As a result of calling [`Window::request_poll`], at the platform's earliest convenience;
    /// * Whenever a new frame has been scheduled, right before actually calling [`WindowHandler::draw`].
    ///
    /// This all means that this method will be invoked very regularly, possibly multiple times per frame interval.
    /// Implementations should do their best to not block and finish their work as quick as possible.
    fn poll(&self) {}

    /// Informs the handler that the window has been resized.
    ///
    /// # Errors
    ///
    /// This operation can fail, in which case an [`HandlerError`] can be returned.
    /// This can happen if e.g. an underlying buffer could not be resized, or some kind of driver error.
    ///
    /// In case this `resized` operation fails, `baseview` will assume that it did not meaningfully
    /// change anything, and that the window is still able to render and operate at the previous size.
    ///
    /// It will also attempt to resize the underlying platform window and parent window back to the
    /// previous size, but this is only a best-effort attempt since those operations can also fail.
    fn resized(&self, new_size: WindowSize) -> core::result::Result<(), HandlerError>;
    fn on_event(&self, event: Event) -> EventStatus;
}

type DynBuilderResult = core::result::Result<Box<dyn WindowHandler>, HandlerError>;

pub struct WindowHandlerBuilder {
    inner: Box<dyn FnOnce(WindowContext) -> DynBuilderResult + Send + 'static>,
}

impl WindowHandlerBuilder {
    pub fn new<H: WindowHandler>(
        f: impl FnOnce(WindowContext) -> core::result::Result<H, HandlerError> + Send + 'static,
    ) -> WindowHandlerBuilder {
        Self { inner: Box::new(|c| Ok(Box::new(f(c)?))) }
    }

    pub fn build(self, ctx: WindowContext) -> Result<Box<dyn WindowHandler>> {
        match (self.inner)(ctx) {
            Ok(handle) => Ok(handle),
            Err(e) => Err(platform::PlatformError::Handler(e)),
        }
    }
}
