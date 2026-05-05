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
    Foundation::POINT, Graphics::Gdi::ScreenToClient, UI::Shell::DragQueryFileW,
};

use crate::{DropData, DropEffect, Event, EventStatus, MouseEvent, PhyPoint, Point};

use super::WindowState;

#[implement(IDropTarget)]
pub(super) struct DropTarget {
    window_state: Weak<WindowState>,

    // These are cached since DragOver and DragLeave callbacks don't provide them,
    // and handling drag move events gets awkward on the client end otherwise
    drag_position: Cell<Point>,
    drop_data: RefCell<DropData>,
}

impl DropTarget {
    pub(super) fn new(window_state: Weak<WindowState>) -> Self {
        Self {
            window_state,
            drag_position: Cell::new(Point::new(0.0, 0.0)),
            drop_data: RefCell::new(DropData::None),
        }
    }

    #[allow(non_snake_case)]
    fn on_event(&self, pdwEffect: Option<*mut DROPEFFECT>, event: MouseEvent) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };

        unsafe {
            let mut window = crate::Window::new(window_state.create_window());

            let event = Event::Mouse(event);
            let event_status =
                window_state.handler_mut().as_mut().unwrap().on_event(&mut window, event);

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

    fn parse_coordinates(&self, pt: POINTL) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };
        let mut pt = POINT { x: pt.x, y: pt.y };

        unsafe { ScreenToClient(window_state.hwnd, &mut pt) };
        let phy_point = PhyPoint::new(pt.x, pt.y);
        self.drag_position.set(phy_point.to_logical(&window_state.window_info()));
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
        &self, pdataobj: Ref<IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates(*pt);
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
        &self, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL, pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates(*pt);

        let event = MouseEvent::DragMoved {
            position: self.drag_position.get(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        self.on_event(None, MouseEvent::DragLeft);
        Ok(())
    }

    fn Drop(
        &self, pdataobj: Ref<IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let Some(window_state) = self.window_state.upgrade() else {
            return Err(E_UNEXPECTED.into());
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfkeystate.0 as usize);

        self.parse_coordinates(*pt);
        self.parse_drop_data(pdataobj.unwrap());

        let event = MouseEvent::DragDropped {
            position: self.drag_position.get(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        Ok(())
    }
}
