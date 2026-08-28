#![expect(clippy::indexing_slicing, reason = "To be refactored later")]

use crate::dpi::PhysicalPosition;
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
use windows_sys::Win32::UI::Shell::DragQueryFileW;

use super::window_state::WindowState;
use crate::platform::BaseviewWindow;
use crate::wrappers::win32::window::{HWnd, WindowData};
use crate::{DropData, DropEffect, Event, EventStatus, MouseEvent};

#[implement(IDropTarget)]
pub(crate) struct DropTarget {
    hwnd: HWnd,
    window_state: Weak<WindowState>,

    // These are cached since DragOver and DragLeave callbacks don't provide them,
    // and handling drag move events gets awkward on the client end otherwise
    drag_position: Cell<PhysicalPosition<i32>>,
    drop_data: RefCell<DropData>,
}

impl DropTarget {
    pub(crate) fn new(window_state: Weak<WindowState>, hwnd: HWnd) -> Self {
        Self {
            hwnd,
            window_state,
            drag_position: Cell::new(PhysicalPosition::new(0, 0)),
            drop_data: RefCell::new(DropData::None),
        }
    }

    fn on_event(&self, pdw_effect: Option<*mut DROPEFFECT>, event: MouseEvent) {
        let Some(window_data_ptr) = self.hwnd.get_userdata_ptr() else {
            return;
        };

        let event = Event::Mouse(event);
        let event_status = unsafe {
            WindowData::<BaseviewWindow>::handle(window_data_ptr, |window| {
                window.inner().map(|w| w.handle_event(event))
            })
        };

        let effect = match event_status {
            Some(EventStatus::AcceptDrop(DropEffect::Copy)) => DROPEFFECT_COPY,
            Some(EventStatus::AcceptDrop(DropEffect::Move)) => DROPEFFECT_MOVE,
            Some(EventStatus::AcceptDrop(DropEffect::Link)) => DROPEFFECT_LINK,
            Some(EventStatus::AcceptDrop(DropEffect::Scroll)) => DROPEFFECT_SCROLL,
            _ => DROPEFFECT_NONE,
        };

        if let Some(pdw_effect) = pdw_effect {
            unsafe { pdw_effect.write(effect) };
        }
    }

    fn parse_coordinates(&self, pt: POINTL) {
        let Ok(phy_point) = self.hwnd.screen_to_client(PhysicalPosition::new(pt.x, pt.y)) else {
            return;
        };

        self.drag_position.set(phy_point);
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
                let buffer_size = (characters as usize).saturating_add(1);
                let mut buffer = vec![0u16; buffer_size];

                DragQueryFileW(hdrop, i, buffer.as_mut_ptr().cast(), buffer_size as u32);

                paths.push(OsString::from_wide(&buffer[..characters as usize]).into())
            }

            self.drop_data.replace(DropData::Files(paths));
        }
    }
}

#[allow(non_snake_case, reason = "To match trait")]
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
            position: self.drag_position.get().cast(),
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
            position: self.drag_position.get().cast(),
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
            position: self.drag_position.get().cast(),
            modifiers,
            data: self.drop_data.borrow().clone(),
        };

        self.on_event(Some(pdweffect), event);
        Ok(())
    }
}
