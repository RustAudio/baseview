use keyboard_types::Modifiers;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::{
    io, mem,
    path::{Path, PathBuf},
    str::Utf8Error,
};

use percent_encoding::percent_decode;
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{ClientMessageEvent, SelectionNotifyEvent, Timestamp};
use x11rb::{
    errors::ConnectionError,
    protocol::xproto::{self, ConnectionExt},
    x11_utils::Serialize,
};

use super::xcb_connection::GetPropertyError;
use crate::x11::{Window, WindowInner};
use crate::{DropData, Event, MouseEvent, PhyPoint, WindowHandler};
use DragNDropState::*;

/// The Drag-N-Drop session state of a `baseview` X11 window, for which it is the target.
///
/// For more information about what the heck is going on here, see the
/// [XDND (X Drag-n-Drop) specification](https://www.freedesktop.org/wiki/Specifications/XDND/).
pub(crate) enum DragNDropState {
    /// There is no active XDND session for this window.
    NoCurrentSession,
    /// At some point in this session's lifetime, we have decided we couldn't possibly handle the
    /// source's Drop data. Every request from this source window from now on will be rejected,
    /// until either a Leave or Drop event is received.
    PermanentlyRejected {
        /// The source window the rejected drag session originated from.
        source_window: xproto::Window,
    },
    /// We have registered a new session (after receiving an Enter event), and are now waiting
    /// for a position event.
    WaitingForPosition {
        /// The protocol version used in this session.
        protocol_version: u8,
        /// The source window the current drag session originates from.
        source_window: xproto::Window,
    },
    /// We have performed a request for data (via `XConvertSelection`), and are now waiting for a
    /// reply.
    ///
    /// More position events can still be received to further update the position data.
    WaitingForData {
        /// The source window the current drag session originates from.
        source_window: xproto::Window,
        /// The current position of the pointer, from the last received position event.
        position: PhyPoint,
        /// The timestamp of the event we made the selection request from.
        ///
        /// This is either from the first position event, or from the drop event if it arrived first.
        ///
        /// In very old versions of the protocol (v0), this timestamp isn't provided. In that case,
        /// this will be `None`.
        requested_at: Option<Timestamp>,
        /// This will be true if we received a drop event *before* we managed to fetch the data.
        ///
        /// If this is true, this means we must complete the drop upon receiving the data, instead
        /// of just going to [`Ready`].
        dropped: bool,
    },
    /// We have completed our quest for the drop data. All fields are populated, and the
    /// [`WindowHandler`] has been notified about the drop session.
    ///
    /// We are now waiting for the user to either drop the file, or leave the window.
    ///
    /// More position events can still be received to further update the position data.
    Ready {
        /// The source window the current drag session originates from.
        source_window: xproto::Window,
        position: PhyPoint,
        data: DropData,
    },
}

impl DragNDropState {
    pub fn handle_enter_event(
        &mut self, window: &WindowInner, handler: &mut dyn WindowHandler,
        event: &ClientMessageEvent,
    ) -> Result<(), GetPropertyError> {
        let data = event.data.as_data32();

        let source_window = data[0] as xproto::Window;
        let [protocol_version, _, _, flags] = data[1].to_le_bytes();

        // Fetch the list of supported data types. It can be either stored inline in the event, or
        // in a separate property on the source window.
        const FLAG_HAS_MORE_TYPES: u8 = 1 << 0;
        let has_more_types = (FLAG_HAS_MORE_TYPES & flags) == FLAG_HAS_MORE_TYPES;

        let extra_types;
        let supported_types = if !has_more_types {
            &data[2..5]
        } else {
            extra_types = window.xcb_connection.get_property(
                source_window,
                window.xcb_connection.atoms.XdndTypeList,
                xproto::Atom::from(xproto::AtomEnum::ATOM),
            )?;

            &extra_types
        };

        // We only support the TextUriList type
        let data_type_supported =
            supported_types.contains(&window.xcb_connection.atoms.TextUriList);

        // If there was an active drag session that we informed the handler about, we need to
        // generate the matching DragLeft before cancelling the previous session.
        let interrupted_active_drag = matches!(self, Ready { .. });

        // Clear any previous state, and mark the new session as started if we can handle the drop.
        *self = if data_type_supported {
            WaitingForPosition { source_window, protocol_version }
        } else {
            // Permanently reject the drop if the data isn't supported.
            PermanentlyRejected { source_window }
        };

        // Do this at the end, in case the handler panics, so that it doesn't poison our internal state.
        if interrupted_active_drag {
            handler.on_event(
                &mut crate::Window::new(Window { inner: window }),
                Event::Mouse(MouseEvent::DragLeft),
            );
        }

        Ok(())
    }

    pub fn handle_position_event(
        &mut self, window: &WindowInner, handler: &mut dyn WindowHandler,
        event: &ClientMessageEvent,
    ) -> Result<(), ReplyError> {
        let data = event.data.as_data32();

        let event_source_window = data[0] as xproto::Window;
        let (event_x, event_y) = decode_xy(data[2]);

        match self {
            // Someone sent us a position event without first sending an enter event.
            // Weird, but we'll still politely tell them we reject the drop.
            NoCurrentSession => Ok(send_status_event(event_source_window, window, false)?),

            // The current session's source window does not match the given event.
            // This means it can either be from a stale session, or a misbehaving app.
            // In any case, we ignore the event but still tell the source we reject the drop.
            WaitingForPosition { source_window, .. }
            | PermanentlyRejected { source_window, .. }
            | WaitingForData { source_window, .. }
            | Ready { source_window, .. }
                if *source_window != event_source_window =>
            {
                Ok(send_status_event(event_source_window, window, false)?)
            }

            // We decided to permanently reject this drop.
            // This means the WindowHandler can't do anything with the data, so we reject the drop.
            PermanentlyRejected { .. } => {
                Ok(send_status_event(event_source_window, window, false)?)
            }

            // This is the position event we were waiting for. Now we can request the selection data.
            // The code above already checks that source_window == event_source_window.
            WaitingForPosition { protocol_version, source_window: _ } => {
                // In version 0, time isn't specified
                let timestamp = (*protocol_version >= 1).then_some(data[3] as Timestamp);

                request_convert_selection(window, timestamp)?;

                // We set our state before translating position data, in case that fails.
                *self = WaitingForData {
                    requested_at: timestamp,
                    source_window: event_source_window,
                    position: PhyPoint::new(0, 0),
                    dropped: false,
                };

                let WaitingForData { position, .. } = self else { unreachable!() };
                *position = translate_root_coordinates(window, event_x, event_y)?;

                Ok(())
            }

            // We are still waiting for the data. So we'll just update the position in the meantime.
            WaitingForData { position, .. } => {
                *position = translate_root_coordinates(window, event_x, event_y)?;

                Ok(())
            }

            // We have already received the data. We can update the position and notify the handler
            Ready { position, data, .. } => {
                // Inform the source that we are still accepting the drop.
                // Do this first, in case translate_root_coordinates fails, or the handler panics.
                // Do not return right away on failure though, we can still inform the handler about
                // the new position.
                let status_result = send_status_event(event_source_window, window, true);

                *position = translate_root_coordinates(window, event_x, event_y)?;

                handler.on_event(
                    &mut crate::Window::new(Window { inner: window }),
                    Event::Mouse(MouseEvent::DragMoved {
                        position: position.to_logical(&window.window_info),
                        data: data.clone(),
                        // We don't get modifiers for drag n drop events.
                        modifiers: Modifiers::empty(),
                    }),
                );

                status_result?;
                Ok(())
            }
        }
    }

    pub fn handle_leave_event(
        &mut self, window: &WindowInner, handler: &mut dyn WindowHandler,
        event: &ClientMessageEvent,
    ) {
        let data = event.data.as_data32();
        let event_source_window = data[0] as xproto::Window;

        let current_source_window = match self {
            NoCurrentSession => return,
            WaitingForPosition { source_window, .. }
            | PermanentlyRejected { source_window, .. }
            | WaitingForData { source_window, .. }
            | Ready { source_window, .. } => *source_window,
        };

        // Only accept the leave event if it comes from the source window of the current drag session.
        if event_source_window != current_source_window {
            return;
        }

        // If there was an active drag session that we informed the handler about, we need to
        // generate the matching DragLeft before cancelling the previous session.
        let left_active_drag = matches!(self, Ready { .. });

        // Clear everything.
        *self = NoCurrentSession;

        // Do this at the end, in case the handler panics, so that it doesn't poison our internal state.
        if left_active_drag {
            handler.on_event(
                &mut crate::Window::new(Window { inner: window }),
                Event::Mouse(MouseEvent::DragLeft),
            );
        }
    }

    pub fn handle_drop_event(
        &mut self, window: &WindowInner, handler: &mut dyn WindowHandler,
        event: &ClientMessageEvent,
    ) -> Result<(), ConnectionError> {
        let data = event.data.as_data32();

        let event_source_window = data[0] as xproto::Window;

        match self {
            // Someone sent us a position event without first sending an enter event.
            // Weird, but we'll still politely tell them we reject the drop.
            NoCurrentSession => send_finished_event(event_source_window, window, false),

            // The current session's source window does not match the given event.
            // This means it can either be from a stale session, or a misbehaving app.
            // In any case, we ignore the event but still tell the source we reject the drop.
            WaitingForPosition { source_window, .. }
            | PermanentlyRejected { source_window, .. }
            | WaitingForData { source_window, .. }
            | Ready { source_window, .. }
                if *source_window != event_source_window =>
            {
                send_finished_event(event_source_window, window, false)
            }

            // We decided to permanently reject this drop.
            // This means the WindowHandler can't do anything with the data, so we reject the drop.
            PermanentlyRejected { .. } => {
                *self = NoCurrentSession;

                send_finished_event(event_source_window, window, false)
            }

            // We received a drop event without any position event. That's very weird, but not
            // irrecoverable: the drop event provides enough data as it is.
            // The code above already checks that source_window == event_source_window.
            WaitingForPosition { protocol_version, source_window: _ } => {
                // In version 0, time isn't specified
                let timestamp = (*protocol_version >= 1).then_some(data[2] as Timestamp);

                // We have the timestamp, we can use it to request to convert the selection,
                // even in this state.

                // If we fail to send the request when the drop has completed, we can't do anything.
                // Just cancel the drop.
                if let Err(e) = request_convert_selection(window, timestamp) {
                    *self = NoCurrentSession;

                    // Try to inform the source that we ended up rejecting the drop.
                    // If the initial request failed, this is likely to fail too, so we'll ignore
                    // it if it errors, so we can focus on the original error.
                    let _ = send_finished_event(event_source_window, window, false);

                    return Err(e);
                };

                *self = WaitingForData {
                    requested_at: timestamp,
                    source_window: event_source_window,
                    // We don't have usable position data. Maybe we'll receive a position later,
                    // but otherwise this will have to do.
                    position: PhyPoint::new(0, 0),
                    dropped: true,
                };

                Ok(())
            }

            // We are still waiting to receive the data.
            // In that case, we'll wait to receive all of it before finalizing the drop.
            WaitingForData { dropped, requested_at, .. } => {
                // If we have a timestamp, that means this is version >= 1.
                if let Some(requested_at) = *requested_at {
                    let event_timestamp = data[2] as Timestamp;

                    // Just in case, check if this drop event isn't stale
                    if requested_at > event_timestamp {
                        return Ok(());
                    }
                }

                // Indicate to the selection_notified handler that the user has performed the drop.
                // Now it should complete the drop instead of just waiting for more events.
                *dropped = true;

                Ok(())
            }

            // The normal case.
            Ready { .. } => {
                let Ready { data, position, .. } = mem::replace(self, NoCurrentSession) else {
                    unreachable!()
                };

                // Don't return immediately if sending the reply fails, we can still notify the window
                // handler about the drop.
                let reply_result = send_finished_event(event_source_window, window, true);

                handler.on_event(
                    &mut crate::Window::new(Window { inner: window }),
                    Event::Mouse(MouseEvent::DragDropped {
                        position: position.to_logical(&window.window_info),
                        data,
                        // We don't get modifiers for drag n drop events.
                        modifiers: Modifiers::empty(),
                    }),
                );

                reply_result
            }
        }
    }

    pub fn handle_selection_notify_event(
        &mut self, window: &WindowInner, handler: &mut dyn WindowHandler,
        event: &SelectionNotifyEvent,
    ) -> Result<(), ConnectionError> {
        // Ignore the event if we weren't actually waiting for a selection notify event
        let WaitingForData { source_window, requested_at, position, dropped } = *self else {
            return Ok(());
        };

        // Ignore if this was meant for another window (?)
        if event.requestor != window.window_id {
            return Ok(());
        }

        // Ignore if this is stale selection data.
        if let Some(requested_at) = requested_at {
            if requested_at != event.time {
                return Ok(());
            }
        }

        // The sender should have set the data on our window, let's fetch it.
        match fetch_dnd_data(window) {
            Err(_e) => {
                *self = PermanentlyRejected { source_window };

                if dropped {
                    send_finished_event(source_window, window, false)
                } else {
                    send_status_event(source_window, window, false)
                }

                // TODO: Log warning
            }
            Ok(data) => {
                let logical_position = position.to_logical(&window.window_info);

                // Inform the source that we are (still) accepting the drop.

                // Handle the case where the user already dropped, but we only received the data later.
                if dropped {
                    *self = NoCurrentSession;

                    let reply_result = send_finished_event(source_window, window, true);

                    // Now that we have actual drop data, we can inform the handler about the drag AND drop events.
                    handler.on_event(
                        &mut crate::Window::new(Window { inner: window }),
                        Event::Mouse(MouseEvent::DragEntered {
                            position: logical_position,
                            data: data.clone(),
                            // We don't get modifiers for drag n drop events.
                            modifiers: Modifiers::empty(),
                        }),
                    );

                    handler.on_event(
                        &mut crate::Window::new(Window { inner: window }),
                        Event::Mouse(MouseEvent::DragDropped {
                            position: logical_position,
                            data: data.clone(),
                            // We don't get modifiers for drag n drop events.
                            modifiers: Modifiers::empty(),
                        }),
                    );

                    reply_result
                } else {
                    // Save the data, now that we finally have it!
                    *self = Ready { data: data.clone(), source_window, position };

                    let reply_result = send_status_event(source_window, window, true);

                    // Now that we have actual drop data, we can inform the handler about the drag event.
                    handler.on_event(
                        &mut crate::Window::new(Window { inner: window }),
                        Event::Mouse(MouseEvent::DragEntered {
                            position: logical_position,
                            data,
                            // We don't get modifiers for drag n drop events.
                            modifiers: Modifiers::empty(),
                        }),
                    );

                    reply_result
                }
            }
        }
    }
}

fn send_status_event(
    source_window: xproto::Window, window: &WindowInner, accepted: bool,
) -> Result<(), ConnectionError> {
    let conn = &window.xcb_connection;
    let (accepted, action) =
        if accepted { (1, conn.atoms.XdndActionPrivate) } else { (0, conn.atoms.None) };

    let event = ClientMessageEvent {
        response_type: xproto::CLIENT_MESSAGE_EVENT,
        window: source_window,
        format: 32,
        data: [window.window_id, accepted, 0, 0, action as _].into(),
        sequence: 0,
        type_: conn.atoms.XdndStatus,
    };

    conn.conn.send_event(false, source_window, xproto::EventMask::NO_EVENT, event.serialize())?;

    conn.conn.flush()
}

pub fn send_finished_event(
    source_window: xproto::Window, window: &WindowInner, accepted: bool,
) -> Result<(), ConnectionError> {
    let conn = &window.xcb_connection;
    let (accepted, action) =
        if accepted { (1, conn.atoms.XdndFinished) } else { (0, conn.atoms.None) };

    let event = ClientMessageEvent {
        response_type: xproto::CLIENT_MESSAGE_EVENT,
        window: source_window,
        format: 32,
        data: [window.window_id, accepted, action as _, 0, 0].into(),
        sequence: 0,
        type_: conn.atoms.XdndStatus as _,
    };

    conn.conn.send_event(false, source_window, xproto::EventMask::NO_EVENT, event.serialize())?;

    conn.conn.flush()
}

fn request_convert_selection(
    window: &WindowInner, timestamp: Option<Timestamp>,
) -> Result<(), ConnectionError> {
    let conn = &window.xcb_connection;

    conn.conn.convert_selection(
        window.window_id,
        conn.atoms.XdndSelection,
        conn.atoms.TextUriList,
        conn.atoms.XdndSelection,
        timestamp.unwrap_or(x11rb::CURRENT_TIME),
    )?;

    conn.conn.flush()
}

fn decode_xy(data: u32) -> (u16, u16) {
    ((data >> 16) as u16, data as u16)
}

fn translate_root_coordinates(
    window: &WindowInner, x: u16, y: u16,
) -> Result<PhyPoint, ReplyError> {
    let root_id = window.xcb_connection.screen().root;
    if root_id == window.window_id {
        return Ok(PhyPoint::new(x as i32, y as i32));
    }

    let reply = window
        .xcb_connection
        .conn
        .translate_coordinates(root_id, window.window_id, x as i16, y as i16)?
        .reply()?;

    Ok(PhyPoint::new(reply.dst_x as i32, reply.dst_y as i32))
}

fn fetch_dnd_data(window: &WindowInner) -> Result<DropData, Box<dyn Error>> {
    let conn = &window.xcb_connection;

    let data: Vec<u8> =
        conn.get_property(window.window_id, conn.atoms.XdndSelection, conn.atoms.TextUriList)?;

    let path_list = parse_data(&data)?;

    Ok(DropData::Files(path_list))
}

// See: https://edeproject.org/spec/file-uri-spec.txt
// TL;DR: format is "file://<hostname>/<path>", hostname is optional and can be "localhost"
fn parse_data(data: &[u8]) -> Result<Vec<PathBuf>, ParseError> {
    if data.is_empty() {
        return Err(ParseError::EmptyData);
    }

    let decoded = percent_decode(data).decode_utf8().map_err(ParseError::InvalidUtf8)?;

    let mut path_list = Vec::new();
    for uri in decoded.split("\r\n").filter(|u| !u.is_empty()) {
        // We only support the file:// protocol
        let Some(mut uri) = uri.strip_prefix("file://") else {
            return Err(ParseError::UnsupportedProtocol(uri.into()));
        };

        if !uri.starts_with('/') {
            // Try (and hope) to see if it's just localhost
            if let Some(stripped) = uri.strip_prefix("localhost") {
                if !stripped.starts_with('/') {
                    // There is something else after "localhost" but before '/'
                    return Err(ParseError::UnsupportedHostname(uri.into()));
                }

                uri = stripped;
            } else {
                // We don't support hostnames.
                return Err(ParseError::UnsupportedHostname(uri.into()));
            }
        }

        let path = Path::new(uri).canonicalize().map_err(ParseError::CanonicalizeError)?;
        path_list.push(path);
    }
    Ok(path_list)
}

#[derive(Debug)]
enum ParseError {
    EmptyData,
    InvalidUtf8(Utf8Error),
    UnsupportedHostname(String),
    UnsupportedProtocol(String),
    CanonicalizeError(io::Error),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to parse Drag-n-Drop data: ")?;

        match self {
            ParseError::EmptyData => f.write_str("data is empty"),
            ParseError::InvalidUtf8(e) => e.fmt(f),
            ParseError::UnsupportedHostname(uri) => write!(f, "unsupported hostname in URI: {uri}"),
            ParseError::UnsupportedProtocol(uri) => write!(f, "unsupported protocol in URI: {uri}"),
            ParseError::CanonicalizeError(e) => write!(f, "unable to resolve path: {e}"),
        }
    }
}

impl Error for ParseError {}
