use std::ffi::OsStr;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::OnceLock;
use windows::Win32::Foundation::S_OK;
use windows::Win32::System::Com::{IDataObject, DATADIR_GET, FORMATETC, STGMEDIUM, TYMED_HGLOBAL};
use windows::Win32::System::DataExchange::{RegisterClipboardFormatA, RegisterClipboardFormatW};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{ReleaseStgMedium, CF_HDROP};
use windows_core::{PCSTR, PCWSTR};
use windows_sys::Win32::UI::Shell::DragQueryFileW;

use crate::DropData;

pub unsafe fn parse_data_object(data_object: &IDataObject) -> DropData {
    let formats = enumerate_available_formats(data_object);

    if let Some(format) = formats.iter().find(|f| f.cfFormat == CF_HDROP.0) {
        if let Some(paths) =
            with_format(data_object, *format, |medium| parse_files_from_medium(medium))
        {
            return DropData::Files(paths);
        }
    }

    if let Some(format) = formats.iter().find(|f| f.cfFormat == uniform_resource_locator_w_format())
    {
        if let Some(url) = with_format(data_object, *format, |medium| {
            read_utf16_null_terminated_from_medium(medium)
        }) {
            return DropData::Url(url);
        }
    }

    if let Some(format) = formats.iter().find(|f| f.cfFormat == uniform_resource_locator_format()) {
        if let Some(url) =
            with_format(data_object, *format, |medium| read_ansi_c_string_from_medium(medium))
        {
            return DropData::Url(url);
        }
    }

    DropData::None
}

unsafe fn enumerate_available_formats(data_object: &IDataObject) -> Vec<FORMATETC> {
    let Ok(enumerator) = data_object.EnumFormatEtc(DATADIR_GET.0 as u32) else {
        return Vec::new();
    };

    let mut formats = Vec::new();

    loop {
        let mut batch = [FORMATETC::default()];
        let mut fetched = 0u32;
        let hr = enumerator.Next(&mut batch, Some(&mut fetched));
        if hr != S_OK || fetched == 0 {
            break;
        }

        let format = batch[0];
        if data_object.QueryGetData(&format as *const FORMATETC) == S_OK {
            formats.push(format);
        }
    }

    formats
}

fn uniform_resource_locator_w_format() -> u16 {
    static ID: OnceLock<u16> = OnceLock::new();
    *ID.get_or_init(|| {
        let name: Vec<u16> =
            OsStr::new("UniformResourceLocatorW").encode_wide().chain(std::iter::once(0)).collect();
        unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) as u16 }
    })
}

fn uniform_resource_locator_format() -> u16 {
    static ID: OnceLock<u16> = OnceLock::new();
    *ID.get_or_init(|| unsafe {
        RegisterClipboardFormatA(PCSTR(c"UniformResourceLocator".as_ptr().cast())) as u16
    })
}

unsafe fn with_format<T>(
    data_object: &IDataObject, format: FORMATETC, f: impl FnOnce(&STGMEDIUM) -> Option<T>,
) -> Option<T> {
    let mut medium = data_object.GetData(&format).ok()?;
    let result = f(&medium);
    ReleaseStgMedium(&mut medium as *mut STGMEDIUM);
    result
}

unsafe fn parse_files_from_medium(medium: &STGMEDIUM) -> Option<Vec<PathBuf>> {
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return None;
    }

    let hdrop = medium.u.hGlobal.0;
    let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, null_mut(), 0);
    if item_count == 0 {
        return None;
    }

    let mut paths = Vec::with_capacity(item_count as usize);

    for i in 0..item_count {
        let characters = DragQueryFileW(hdrop, i, null_mut(), 0);
        let buffer_size = characters as usize + 1;
        let mut buffer = vec![0u16; buffer_size];

        DragQueryFileW(hdrop, i, buffer.as_mut_ptr().cast(), buffer_size as u32);

        paths.push(std::ffi::OsString::from_wide(&buffer[..characters as usize]).into());
    }

    Some(paths)
}

unsafe fn read_utf16_null_terminated_from_medium(medium: &STGMEDIUM) -> Option<String> {
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return None;
    }

    let hglobal = medium.u.hGlobal;
    if hglobal.0.is_null() {
        return None;
    }

    let size = GlobalSize(hglobal);
    if size < 2 {
        return None;
    }

    let p = GlobalLock(hglobal);
    if p.is_null() {
        return None;
    }

    let wide = std::slice::from_raw_parts(p as *const u16, size / 2);
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    let s: String = std::char::decode_utf16(wide[..end].iter().copied())
        .map(|r| r.unwrap_or(std::char::REPLACEMENT_CHARACTER))
        .collect();
    let _ = GlobalUnlock(hglobal);

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

unsafe fn read_ansi_c_string_from_medium(medium: &STGMEDIUM) -> Option<String> {
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return None;
    }

    let hglobal = medium.u.hGlobal;
    if hglobal.0.is_null() {
        return None;
    }

    let size = GlobalSize(hglobal);
    if size == 0 {
        return None;
    }

    let p = GlobalLock(hglobal);
    if p.is_null() {
        return None;
    }

    let bytes = std::slice::from_raw_parts(p as *const u8, size);
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let _ = GlobalUnlock(hglobal);

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
