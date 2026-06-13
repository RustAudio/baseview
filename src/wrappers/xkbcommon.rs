use xkbcommon_dl as xkbc;

pub(crate) type Keycode = xkbcommon_dl::xkb_keycode_t;
/// A xkbcommon state object
pub struct XkbcommonState {
    state: *mut xkbc::xkb_state,
    xkb_common: &'static xkbc::XkbCommon,
}

impl XkbcommonState {
    pub fn new(xcb_connection: &crate::x11::XcbConnection) -> Option<Self> {
        let xkb_common = xkbc::xkbcommon_option()?;
        let xkb_x11 = xkbc::x11::xkbcommon_x11_option()?;
        let context =
            unsafe { (xkb_common.xkb_context_new)(xkbc::xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };

        let conn: *mut xkbc::x11::xcb_connection_t =
            xcb_connection.conn.xcb_connection().get_raw_xcb_connection();

        let state = unsafe {
            let device_id = (xkb_x11.xkb_x11_get_core_keyboard_device_id)(conn);
            assert!(device_id >= 0);
            let keymap = (xkb_x11.xkb_x11_keymap_new_from_device)(
                context,
                conn,
                device_id,
                xkbc::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            (xkb_x11.xkb_x11_state_new_from_device)(keymap, conn, device_id)
        };
        Some(XkbcommonState { state, xkb_common })
    }

    pub fn key_get_utf8(&self, code: Keycode) -> String {
        // A buffer to store the cstr
        let buffer_size = 32;
        let mut buffer = vec![0; buffer_size];
        let result = unsafe {
            (self.xkb_common.xkb_state_key_get_utf8)(
                self.state,
                code,
                buffer.as_mut_ptr(),
                buffer_size,
            )
        };

        // Convert back to String
        if result < 0 {
            "".to_string()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) };
            match c_str.to_str() {
                Ok(s) => s.to_string(),
                Err(_) => "".to_string(),
            }
        }
    }

    pub fn update_key(&mut self, code: Keycode, dir: xkbc::xkb_key_direction) {
        unsafe {
            (self.xkb_common.xkb_state_update_key)(self.state, code, dir);
        }
    }

    pub fn update_key_down(&mut self, code: Keycode) {
        self.update_key(code, xkbc::xkb_key_direction::XKB_KEY_DOWN)
    }

    pub fn update_key_up(&mut self, code: Keycode) {
        self.update_key(code, xkbc::xkb_key_direction::XKB_KEY_UP)
    }
}

impl Drop for XkbcommonState {
    fn drop(&mut self) {
        unsafe { (self.xkb_common.xkb_state_unref)(self.state) };
    }
}
