use std::ffi::OsString;
use std::mem::transmute;
use std::os::windows::prelude::OsStringExt;
use std::ptr::null_mut;
use std::rc::{Rc, Weak};

use winapi::shared::guiddef::{IsEqualIID, REFIID};
use winapi::shared::minwindef::{DWORD, WPARAM};
use winapi::shared::ntdef::{HRESULT, ULONG};
use winapi::shared::windef::POINTL;
use winapi::shared::winerror::{E_NOINTERFACE, E_UNEXPECTED, S_OK};
use winapi::shared::wtypes::DVASPECT_CONTENT;
use winapi::um::objidl::{IDataObject, FORMATETC, STGMEDIUM, TYMED_HGLOBAL};
use winapi::um::oleidl::{
    IDropTarget, IDropTargetVtbl, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE,
    DROPEFFECT_NONE, DROPEFFECT_SCROLL,
};
use winapi::um::shellapi::{DragQueryFileW, HDROP};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winuser::CF_HDROP;
use winapi::Interface;

use crate::{DropData, DropEffect, Event, EventStatus, MouseEvent, PhyPoint, Point};

use super::WindowState;

// These function pointers have to be stored in a (const) variable before they can be transmuted
// Transmuting is needed because winapi has a bug where the pt parameter has an incorrect
// type `*const POINTL`
const DRAG_ENTER_PTR: unsafe extern "system" fn(
    this: *mut IDropTarget,
    pDataObj: *const IDataObject,
    grfKeyState: DWORD,
    pt: POINTL,
    pdwEffect: *mut DWORD,
) -> HRESULT = DropTarget::drag_enter;
const DRAG_OVER_PTR: unsafe extern "system" fn(
    this: *mut IDropTarget,
    grfKeyState: DWORD,
    pt: POINTL,
    pdwEffect: *mut DWORD,
) -> HRESULT = DropTarget::drag_over;
const DROP_PTR: unsafe extern "system" fn(
    this: *mut IDropTarget,
    pDataObj: *const IDataObject,
    grfKeyState: DWORD,
    pt: POINTL,
    pdwEffect: *mut DWORD,
) -> HRESULT = DropTarget::drop;
const DROP_TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
    parent: IUnknownVtbl {
        QueryInterface: DropTarget::query_interface,
        AddRef: DropTarget::add_ref,
        Release: DropTarget::release,
    },
    DragEnter: unsafe { transmute(DRAG_ENTER_PTR) },
    DragOver: unsafe { transmute(DRAG_OVER_PTR) },
    DragLeave: DropTarget::drag_leave,
    Drop: unsafe { transmute(DROP_PTR) },
};

#[repr(C)]
pub(super) struct DropTarget {
    base: IDropTarget,

    window_state: Weak<WindowState>,

    // These are cached since DragOver and DragLeave callbacks don't provide them,
    // and handling drag move events gets awkward on the client end otherwise
    drag_position: Point,
    drop_data: DropData,
}

impl DropTarget {
    pub(super) fn new(window_state: Weak<WindowState>) -> Self {
        Self {
            base: IDropTarget { lpVtbl: &DROP_TARGET_VTBL },

            window_state,

            drag_position: Point::new(0.0, 0.0),
            drop_data: DropData::None,
        }
    }

    #[allow(non_snake_case)]
    fn on_event(&self, pdwEffect: Option<*mut DWORD>, event: MouseEvent) {
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

    fn parse_coordinates(&mut self, pt: POINTL) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };

        let phy_point = PhyPoint::new(pt.x, pt.y);
        self.drag_position = phy_point.to_logical(&window_state.window_info());
    }

    fn parse_drop_data(&mut self, data_object: &IDataObject) {
        let format = FORMATETC {
            cfFormat: CF_HDROP as u16,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL,
        };

        let mut medium = STGMEDIUM { tymed: 0, u: null_mut(), pUnkForRelease: null_mut() };

        unsafe {
            let hresult = data_object.GetData(&format, &mut medium);
            if hresult != S_OK {
                self.drop_data = DropData::None;
                return;
            }

            let hdrop = *(*medium.u).hGlobal() as HDROP;

            let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, null_mut(), 0);
            if item_count == 0 {
                self.drop_data = DropData::None;
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

            self.drop_data = DropData::Files(paths);
        }
    }

    #[allow(non_snake_case)]
    unsafe extern "system" fn query_interface(
        this: *mut IUnknown, riid: REFIID, ppvObject: *mut *mut winapi::ctypes::c_void,
    ) -> HRESULT {
        if IsEqualIID(&*riid, &IUnknown::uuidof()) || IsEqualIID(&*riid, &IDropTarget::uuidof()) {
            Self::add_ref(this);
            *ppvObject = this as *mut winapi::ctypes::c_void;
            return S_OK;
        }

        E_NOINTERFACE
    }

    unsafe extern "system" fn add_ref(this: *mut IUnknown) -> ULONG {
        let arc = Rc::from_raw(this);
        let result = Rc::strong_count(&arc) + 1;
        let _ = Rc::into_raw(arc);

        Rc::increment_strong_count(this);

        result as ULONG
    }

    unsafe extern "system" fn release(this: *mut IUnknown) -> ULONG {
        let arc = Rc::from_raw(this);
        let result = Rc::strong_count(&arc) - 1;
        let _ = Rc::into_raw(arc);

        Rc::decrement_strong_count(this);

        result as ULONG
    }

    #[allow(non_snake_case)]
    unsafe extern "system" fn drag_enter(
        this: *mut IDropTarget, pDataObj: *const IDataObject, grfKeyState: DWORD, pt: POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT {
        let drop_target = &mut *(this as *mut DropTarget);
        let Some(window_state) = drop_target.window_state.upgrade() else {
            return E_UNEXPECTED;
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfKeyState as WPARAM);

        drop_target.parse_coordinates(pt);
        drop_target.parse_drop_data(&*pDataObj);

        let event = MouseEvent::DragEntered {
            position: drop_target.drag_position,
            modifiers,
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }

    #[allow(non_snake_case)]
    unsafe extern "system" fn drag_over(
        this: *mut IDropTarget, grfKeyState: DWORD, pt: POINTL, pdwEffect: *mut DWORD,
    ) -> HRESULT {
        let drop_target = &mut *(this as *mut DropTarget);
        let Some(window_state) = drop_target.window_state.upgrade() else {
            return E_UNEXPECTED;
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfKeyState as WPARAM);

        drop_target.parse_coordinates(pt);

        let event = MouseEvent::DragMoved {
            position: drop_target.drag_position,
            modifiers,
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }

    unsafe extern "system" fn drag_leave(this: *mut IDropTarget) -> HRESULT {
        let drop_target = &mut *(this as *mut DropTarget);
        drop_target.on_event(None, MouseEvent::DragLeft);
        S_OK
    }

    #[allow(non_snake_case)]
    unsafe extern "system" fn drop(
        this: *mut IDropTarget, pDataObj: *const IDataObject, grfKeyState: DWORD, pt: POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT {
        let drop_target = &mut *(this as *mut DropTarget);
        let Some(window_state) = drop_target.window_state.upgrade() else {
            return E_UNEXPECTED;
        };

        let modifiers =
            window_state.keyboard_state().get_modifiers_from_mouse_wparam(grfKeyState as WPARAM);

        drop_target.parse_coordinates(pt);
        drop_target.parse_drop_data(&*pDataObj);

        let event = MouseEvent::DragDropped {
            position: drop_target.drag_position,
            modifiers,
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }
}
