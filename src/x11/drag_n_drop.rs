/*
The code in this file was derived from the Winit project (https://github.com/rust-windowing/winit).
The original, unmodified code file this work is derived from can be found here:

https://github.com/rust-windowing/winit/blob/44aabdddcc9f720aec860c1f83c1041082c28560/src/platform_impl/linux/x11/dnd.rs

The original code this is based on is licensed under the following terms:
*/

/*
Copyright 2024 "The Winit contributors".

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

   http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

/*
The full licensing terms of the original source code, at the time of writing, can also be found at:
https://github.com/rust-windowing/winit/blob/44aabdddcc9f720aec860c1f83c1041082c28560/LICENSE .

The Derived Work present in this file contains modifications made to the original source code, is
Copyright (c) 2024 "The Baseview contributors",
and is licensed under either the Apache License, Version 2.0; or The MIT license, at your option.

Copies of those licenses can be respectively found at:
* https://github.com/RustAudio/baseview/blob/master/LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0 ;
* https://github.com/RustAudio/baseview/blob/master/LICENSE-MIT.

*/

use std::{
    io,
    os::raw::*,
    path::{Path, PathBuf},
    str::Utf8Error,
};

use percent_encoding::percent_decode;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Timestamp;
use x11rb::{
    errors::ConnectionError,
    protocol::xproto::{self, ConnectionExt},
    x11_utils::Serialize,
};

use crate::{DropData, Point};

use super::xcb_connection::{GetPropertyError, XcbConnection};

pub(crate) struct DragNDrop {
    // Populated by XdndEnter event handler
    pub version: Option<u32>,

    pub type_list: Option<Vec<xproto::Atom>>,

    // Populated by XdndPosition event handler
    pub source_window: Option<xproto::Window>,

    // Populated by SelectionNotify event handler (triggered by XdndPosition event handler)
    pub data: DropData,
    pub data_requested_at: Option<Timestamp>,

    pub logical_pos: Point,
}

impl DragNDrop {
    pub fn new() -> Self {
        Self {
            version: None,
            type_list: None,
            source_window: None,
            data: DropData::None,
            data_requested_at: None,
            logical_pos: Point::new(0.0, 0.0),
        }
    }

    pub fn reset(&mut self) {
        self.version = None;
        self.type_list = None;
        self.source_window = None;
        self.data = DropData::None;
        self.data_requested_at = None;
        self.logical_pos = Point::new(0.0, 0.0);
    }

    pub fn send_status(
        &self, this_window: xproto::Window, target_window: xproto::Window, accepted: bool,
        conn: &XcbConnection,
    ) -> Result<(), ConnectionError> {
        let (accepted, action) =
            if accepted { (1, conn.atoms.XdndActionPrivate) } else { (0, conn.atoms.None) };

        let event = xproto::ClientMessageEvent {
            response_type: xproto::CLIENT_MESSAGE_EVENT,
            window: target_window,
            format: 32,
            data: [this_window, accepted, 0, 0, action as _].into(),
            sequence: 0,
            type_: conn.atoms.XdndStatus as _,
        };

        conn.conn.send_event(
            false,
            target_window,
            xproto::EventMask::NO_EVENT,
            event.serialize(),
        )?;

        conn.conn.flush()
    }

    pub fn send_finished(
        &self, this_window: xproto::Window, target_window: xproto::Window, accepted: bool,
        conn: &XcbConnection,
    ) -> Result<(), ConnectionError> {
        let (accepted, action) =
            if accepted { (1, conn.atoms.XdndFinished) } else { (0, conn.atoms.None) };

        let event = xproto::ClientMessageEvent {
            response_type: xproto::CLIENT_MESSAGE_EVENT,
            window: target_window,
            format: 32,
            data: [this_window, accepted, action as _, 0, 0].into(),
            sequence: 0,
            type_: conn.atoms.XdndStatus as _,
        };

        conn.conn
            .send_event(false, target_window, xproto::EventMask::NO_EVENT, event.serialize())
            .map(|_| ())
    }

    pub fn get_type_list(
        &self, source_window: xproto::Window, conn: &XcbConnection,
    ) -> Result<Vec<xproto::Atom>, GetPropertyError> {
        conn.get_property(
            source_window,
            conn.atoms.XdndTypeList,
            xproto::Atom::from(xproto::AtomEnum::ATOM),
        )
    }

    pub fn convert_selection(
        &self, window: xproto::Window, time: xproto::Timestamp, conn: &XcbConnection,
    ) -> Result<(), ConnectionError> {
        conn.conn
            .convert_selection(
                window,
                conn.atoms.XdndSelection,
                conn.atoms.TextUriList,
                conn.atoms.XdndSelection,
                time,
            )
            .map(|_| ())
    }

    pub fn read_data(
        &self, window: xproto::Window, conn: &XcbConnection,
    ) -> Result<Vec<c_uchar>, GetPropertyError> {
        conn.get_property(window, conn.atoms.XdndSelection, conn.atoms.TextUriList)
    }

    pub fn parse_data(&self, data: &mut [c_uchar]) -> Result<Vec<PathBuf>, DndDataParseError> {
        if !data.is_empty() {
            let mut path_list = Vec::new();
            let decoded = percent_decode(data).decode_utf8()?.into_owned();
            for uri in decoded.split("\r\n").filter(|u| !u.is_empty()) {
                // The format is specified as protocol://host/path
                // However, it's typically simply protocol:///path
                let path_str = if uri.starts_with("file://") {
                    let path_str = uri.replace("file://", "");
                    if !path_str.starts_with('/') {
                        // A hostname is specified
                        // Supporting this case is beyond the scope of my mental health
                        return Err(DndDataParseError::HostnameSpecified(path_str));
                    }
                    path_str
                } else {
                    // Only the file protocol is supported
                    return Err(DndDataParseError::UnexpectedProtocol(uri.to_owned()));
                };

                let path = Path::new(&path_str).canonicalize()?;
                path_list.push(path);
            }
            Ok(path_list)
        } else {
            Err(DndDataParseError::EmptyData)
        }
    }
}

#[derive(Debug)]
pub enum DndDataParseError {
    EmptyData,
    InvalidUtf8(#[allow(dead_code)] Utf8Error),
    HostnameSpecified(#[allow(dead_code)] String),
    UnexpectedProtocol(#[allow(dead_code)] String),
    UnresolvablePath(#[allow(dead_code)] io::Error),
}

impl From<Utf8Error> for DndDataParseError {
    fn from(e: Utf8Error) -> Self {
        DndDataParseError::InvalidUtf8(e)
    }
}

impl From<io::Error> for DndDataParseError {
    fn from(e: io::Error) -> Self {
        DndDataParseError::UnresolvablePath(e)
    }
}
