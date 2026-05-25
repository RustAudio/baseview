// https://github.com/superlistapp/super_native_extensions/blob/beabd4aca7f353a94f41b635aace9e625ca89aff/super_native_extensions/rust/src/win32/drag.rs
// used as a reference

use windows::core::implement;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, S_OK,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows_core::BOOL;

use super::drag_source::DragSourceDataObject;
use crate::DropData;

pub fn start_drag(data: DropData) {
    if !matches!(data, DropData::Files(ref paths) if !paths.is_empty()) {
        return;
    }

    let data_object = DragSourceDataObject::create(data);
    let drop_source = DropSource::create();
    let mut effects_out = DROPEFFECT_NONE;
    unsafe {
        let _ = DoDragDrop(
            &data_object,
            &drop_source,
            DROPEFFECT_COPY,
            &mut effects_out as *mut DROPEFFECT,
        );
    }
}

#[implement(IDropSource)]
pub struct DropSource {}

impl DropSource {
    pub fn create() -> IDropSource {
        Self {}.into()
    }
}

#[allow(non_snake_case)]
impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(
        &self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS,
    ) -> windows_core::HRESULT {
        if fescapepressed.as_bool() {
            DRAGDROP_S_CANCEL
        } else if grfkeystate.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> windows_core::HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}
