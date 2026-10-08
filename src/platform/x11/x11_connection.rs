use super::prelude::*;
use crate::wrappers::xlib::XlibXcbConnection;
use x11rb::connection::RequestConnection;
use x11rb::cookie::VoidCookie;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::errors::ConnectionError;
use x11rb::protocol::present;
use x11rb::protocol::xproto::{
    self, Atom, ChangeWindowAttributesAux, ConnectionExt, EventMask, Screen,
};
use x11rb::resource_manager;
use x11rb::xcb_ffi::XCBConnection;

mod get_property;
pub use get_property::GetPropertyError;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,

        // Drag-N-Drop Atoms
        XdndAware,
        XdndEnter,
        XdndLeave,
        XdndDrop,
        XdndPosition,
        XdndStatus,
        XdndSelection,
        XdndFinished,
        XdndActionPrivate,
        XdndActionCopy,
        XdndActionMove,
        XdndActionLink,
        XdndActionAsk,
        XdndTypeList,
        TextUriList: b"text/uri-list",
        None: b"None",
    }
}

/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
pub struct X11Connection {
    pub(crate) conn: Arc<XlibXcbConnection>,
    pub(crate) atoms: Atoms,
    pub(crate) resources: ConnectionResources,

    pub(crate) present_supported: bool,
}

impl X11Connection {
    pub fn connect() -> PlatformResult<Self> {
        let conn = XlibXcbConnection::open()?;
        let atoms = Atoms::new(&*conn)?.reply()?;

        Ok(Self {
            atoms,
            present_supported: conn.extension_information(present::X11_EXTENSION_NAME)?.is_some(),
            resources: ConnectionResources::load(&conn)?,

            conn: Arc::new(conn),
        })
    }

    pub fn default_screen(&self) -> &Screen {
        self.conn.default_screen()
    }

    pub fn get_property<T: bytemuck::Pod>(
        &self, window: xproto::Window, property: Atom, property_type: Atom,
    ) -> Result<Vec<T>, GetPropertyError> {
        get_property::get_property(window, property, property_type, &self.conn)
    }

    pub fn register_tree_structure_events(
        &self,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        let root = self.default_screen().root;

        self.conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
        )
    }
}

pub struct ConnectionResources {
    pub cursor_handle: CursorHandle,
    pub xft_dpi: Option<u32>,
}

impl ConnectionResources {
    fn load(conn: &XlibXcbConnection) -> PlatformResult<Self> {
        let resources = resource_manager::new_from_default(conn as &XCBConnection)?;

        Ok(Self {
            xft_dpi: resources.get_value::<u32>("Xft.dpi", "").ok().flatten(),

            cursor_handle: CursorHandle::new(
                conn as &XCBConnection,
                conn.default_screen_index().into(),
                &resources,
            )?
            .reply()?,
        })
    }
}
