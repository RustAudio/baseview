use std::ptr::null_mut;

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
    };

    #[cfg(target_os = "macos")] {
        baseview::Window::open(window_open_options);
    }
    #[cfg(target_os = "windows")] {
        baseview::create_window(window_open_options);
        loop {
            if !baseview::handle_msg(null_mut()) {
                break;
            }
        }
    }
    #[cfg(target_os = "linux")] {
        baseview::run(window_open_options);
    }
}
