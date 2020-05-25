// TODO: messy for now, will refactor when I have more of an idea of the API/architecture
// TODO: actually handle events
// TODO: set window title
// TODO: close window

use crate::Parent;
use crate::WindowOpenOptions;

struct X11Window {
    xcb_connection: xcb::Connection,
}

impl X11Window {
    pub fn run(options: WindowOpenOptions) -> Self {
        // Convert the parent to a X11 window ID if we're given one
        let parent = match options.parent {
            Parent::None => None,
            Parent::AsIfParented => None, // ???
            Parent::WithParent(p) => Some(p as u32),
        };

        // Connect to the X server
        let (conn, screen_num) = xcb::Connection::connect_with_xlib_display().unwrap();

        // Figure out screen information
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).unwrap();

        // Create window, connecting to the parent if we have one
        let win = conn.generate_id();
        let event_mask = &[
            (xcb::CW_BACK_PIXEL, screen.black_pixel()),
            (
                xcb::CW_EVENT_MASK,
                xcb::EVENT_MASK_EXPOSURE
                    | xcb::EVENT_MASK_BUTTON_PRESS
                    | xcb::EVENT_MASK_BUTTON_RELEASE
                    | xcb::EVENT_MASK_BUTTON_1_MOTION,
            ),
        ];
        xcb::create_window(
            // Connection
            &conn,
            // Depth
            xcb::COPY_FROM_PARENT as u8,
            // Window ID
            win,
            // Parent ID
            if let Some(p) = parent {
                p
            } else {
                screen.root()
            },
            // x
            0,
            // y
            0,
            // width
            1024,
            // height
            1024,
            // border width
            0,
            // class
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            // visual
            screen.root_visual(),
            // masks
            event_mask,
        );

        // Display the window
        xcb::map_window(&conn, win);
        conn.flush();

        let x11_window = Self {
            xcb_connection: conn,
        };

        x11_window.handle_events();

        return x11_window;
    }

    // Event loop
    fn handle_events(&self) {
        loop {
            let ev = self.xcb_connection.wait_for_event();
            if let Some(event) = ev {
                println!("{:?}", event.response_type());
            }
        }
    }
}

pub fn run(options: WindowOpenOptions) {
    X11Window::run(options);
}
