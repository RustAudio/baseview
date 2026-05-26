use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::os::windows::prelude::OsStringExt;
use std::ptr::null_mut;
use std::rc::Weak;
use windows::core::implement;
use windows::Win32::Foundation::{E_UNEXPECTED, POINTL};
use windows::Win32::System::Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows_core::Ref;
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::ScreenToClient,
    UI::Shell::DragQueryFileW,
    UI::WindowsAndMessaging::{GetClientRect, GetCursorPos},
};

use crate::{DropData, DropEffect, Event, EventStatus, MouseEvent, PhyPoint, Point};

use super::WindowState;

#[implement(IDropTarget)]
pub(crate) struct DropTarget {
    window_state: Weak<WindowState>,

    // These are cached since DragOver and DragLeave callbacks don't provide them,
    // and handling drag move events gets awkward on the client end otherwise
    drag_position: Cell<Point>,
    drop_data: RefCell<DropData>,
    /// Whether drag client coordinates are physical pixels (`true`) or already logical
    /// (`false`). Cached when `is_drag_coords_physical` is called, and cleared on `DragLeave`
    /// and `Drop`.
    drag_coords_physical: Cell<Option<bool>>,
}

impl DropTarget {
    pub(crate) fn new(window_state: Weak<WindowState>) -> Self {
        Self {
            window_state,
            drag_position: Cell::new(Point::new(0.0, 0.0)),
            drop_data: RefCell::new(DropData::None),
            drag_coords_physical: Cell::new(None),
        }
    }

    #[allow(non_snake_case)]
    fn on_event(&self, pdwEffect: Option<*mut DROPEFFECT>, event: MouseEvent) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };

        unsafe {
            let event = Event::Mouse(event);
            let event_status = window_state.handle_event(event);

            if let Some(pdwEffect) = pdwEffect {
                match event_status {
                    EventStatus::AcceptDrop(DropEffect::Copy) => *pdwEffect = DROPEFFECT_COPY,
                    EventStatus::AcceptDrop(DropEffect::Move) => *pdwEffect = DROPEFFECT_MOVE,
                    EventStatus::AcceptDrop(DropEffect::Link) => *pdwEffect = DROPEFFECT_LINK,
                    EventStatus::AcceptDrop(DropEffect::Scroll) => *pdwEffect = DROPEFFECT_SCROLL,
                    _ => *pdwEffect = DROPEFFECT_NONE,
                }
            }
        }
    }

    /// Returns `true` when client coordinates from `GetCursorPos`/`ScreenToClient` are physical
    /// pixels and should be scaled with [`PhyPoint::to_logical`]. Returns `false` when they are
    /// already in logical space.
    ///
    /// For some reason, this can vary based on the combination of parent window AND drag source.
    /// Most of the time the coordinates are physical, but logical coordinates have been observed
    /// with Bitwig as the parent and Windows Explorer as the drag source.
    ///
    /// Cached on self.drag_coords_physical.
    fn is_drag_coords_physical(&self, window_state: &WindowState) -> bool {
        match self.drag_coords_physical.get() {
            Some(physical) => physical,
            None => {
                let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                unsafe { GetClientRect(window_state.hwnd, &mut rect) };
                let client_w = (rect.right - rect.left) as u32;
                let physical_w = window_state.window_info().physical_size().width;
                let logical_w = window_state.window_info().logical_size().width as u32;
                let physical = client_w.abs_diff(physical_w) < client_w.abs_diff(logical_w);

                self.drag_coords_physical.set(Some(physical));
                physical
            }
        }
    }

    fn end_drag_session(&self) {
        self.drag_coords_physical.set(None);
    }

    fn parse_coordinates(&self) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };

        // Some parents pass weird coordinates via OLE `pt`. Query the cursor directly instead.
        let mut pt = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut pt as *mut POINT);
            ScreenToClient(window_state.hwnd, &mut pt as *mut POINT);
        }

        let logical_point = if self.is_drag_coords_physical(&window_state) {
            PhyPoint::new(pt.x, pt.y).to_logical(&window_state.window_info())
        } else {
            Point::new(pt.x as f64, pt.y as f64)
        };
        self.drag_position.set(logical_point);
    }

    fn parse_drop_data(&self, data_object: &IDataObject) {
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        unsafe {
            let Ok(medium) = data_object.GetData(&format) else {
                self.drop_data.replace(DropData::None);
                return;
            };

            let hdrop = medium.u.hGlobal.0;

            let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, null_mut(), 0);
            if item_count == 0 {
                self.drop_data.replace(DropData::None);
                return;
            }

            let mut paths = Vec::with_capacity(item_count as usize);

            for i in 0..item_count {
                let characters = DragQueryFileW(hdrop, i, null_mut(), 0);
                let buffer_size = characters as usize + 1;
                let mut buffer = vec![0u16; buffer_size];

                DragQueryFileW(hdrop, i, buffer.as_mut_ptr().cast(), buffer_size as u32);

                paths.push(OsString::from_wide(&buffer[..characters as usize]).into())
            }

            self.drop_data.replace(DropData::Files(paths));
        }
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self, pdataobj: Ref<IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates();
        self.parse_drop_data(pdataobj.unwrap());

        let event = MouseEvent::DragEntered {
            position: self.drag_position.get(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        Ok(())
    }

    fn DragOver(
        &self, grfkeystate: MODIFIERKEYS_FLAGS, _pt: &POINTL, pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates();

        let event = MouseEvent::DragMoved {
            position: self.drag_position.get(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        self.end_drag_session();
        self.on_event(None, MouseEvent::DragLeft);
        Ok(())
    }

    fn Drop(
        &self, pdataobj: Ref<IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates();
        self.parse_drop_data(pdataobj.unwrap());

        let event = MouseEvent::DragDropped {
            position: self.drag_position.get(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        self.end_drag_session();
        Ok(())
    }
}
