/*
The code in this file was derived from the Winit project (https://github.com/rust-windowing/winit).
The original, unmodified code file this work is derived from can be found here:

https://github.com/rust-windowing/winit/blob/44aabdddcc9f720aec860c1f83c1041082c28560/src/platform_impl/linux/x11/util/window_property.rs

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

use std::error::Error;
use std::ffi::c_int;
use std::fmt;
use std::mem;
use std::sync::Arc;

use bytemuck::Pod;

use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{self, ConnectionExt};
use x11rb::xcb_ffi::XCBConnection;

#[derive(Debug, Clone)]
pub enum GetPropertyError {
    X11rbError(Arc<ReplyError>),
    TypeMismatch(xproto::Atom),
    FormatMismatch(c_int),
}

impl<T: Into<ReplyError>> From<T> for GetPropertyError {
    fn from(e: T) -> Self {
        Self::X11rbError(Arc::new(e.into()))
    }
}

impl fmt::Display for GetPropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GetPropertyError::X11rbError(err) => err.fmt(f),
            GetPropertyError::TypeMismatch(err) => write!(f, "type mismatch: {err}"),
            GetPropertyError::FormatMismatch(err) => write!(f, "format mismatch: {err}"),
        }
    }
}

impl Error for GetPropertyError {}

// Number of 32-bit chunks to retrieve per iteration of get_property's inner loop.
// To test if `get_property` works correctly, set this to 1.
const PROPERTY_BUFFER_SIZE: u32 = 1024; // 4k of RAM ought to be enough for anyone!

pub(super) fn get_property<T: Pod>(
    window: xproto::Window, property: xproto::Atom, property_type: xproto::Atom,
    conn: &XCBConnection,
) -> Result<Vec<T>, GetPropertyError> {
    let mut iter = PropIterator::new(conn, window, property, property_type);
    let mut data = vec![];

    loop {
        if !iter.next_window(&mut data)? {
            break;
        }
    }

    Ok(data)
}

/// An iterator over the "windows" of the property that we are fetching.
struct PropIterator<'a, T> {
    /// Handle to the connection.
    conn: &'a XCBConnection,

    /// The window that we're fetching the property from.
    window: xproto::Window,

    /// The property that we're fetching.
    property: xproto::Atom,

    /// The type of the property that we're fetching.
    property_type: xproto::Atom,

    /// The offset of the next window, in 32-bit chunks.
    offset: u32,

    /// The format of the type.
    format: u8,

    /// Keep a reference to `T`.
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: Pod> PropIterator<'a, T> {
    /// Create a new property iterator.
    fn new(
        conn: &'a XCBConnection, window: xproto::Window, property: xproto::Atom,
        property_type: xproto::Atom,
    ) -> Self {
        let format = match mem::size_of::<T>() {
            1 => 8,
            2 => 16,
            4 => 32,
            _ => unreachable!(),
        };

        Self {
            conn,
            window,
            property,
            property_type,
            offset: 0,
            format,
            _phantom: Default::default(),
        }
    }

    /// Get the next window and append it to `data`.
    ///
    /// Returns whether there are more windows to fetch.
    fn next_window(&mut self, data: &mut Vec<T>) -> Result<bool, GetPropertyError> {
        // Send the request and wait for the reply.
        let reply = self
            .conn
            .get_property(
                false,
                self.window,
                self.property,
                self.property_type,
                self.offset,
                PROPERTY_BUFFER_SIZE,
            )?
            .reply()?;

        // Make sure that the reply is of the correct type.
        if reply.type_ != self.property_type {
            return Err(GetPropertyError::TypeMismatch(reply.type_));
        }

        // Make sure that the reply is of the correct format.
        if reply.format != self.format {
            return Err(GetPropertyError::FormatMismatch(reply.format.into()));
        }

        // Append the data to the output.
        if mem::size_of::<T>() == 1 && mem::align_of::<T>() == 1 {
            // We can just do a bytewise append.
            data.extend_from_slice(bytemuck::cast_slice(&reply.value));
        } else {
            let old_len = data.len();
            let added_len = reply.value.len() / mem::size_of::<T>();

            data.resize(old_len + added_len, T::zeroed());
            bytemuck::cast_slice_mut::<T, u8>(&mut data[old_len..]).copy_from_slice(&reply.value);
        }

        // Check `bytes_after` to see if there are more windows to fetch.
        self.offset += PROPERTY_BUFFER_SIZE;
        Ok(reply.bytes_after != 0)
    }
}
