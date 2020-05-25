use std::ptr::null_mut;

use baseview::{WindowOpenOptions, create_window, handle_msg};
use baseview::Parent;

fn main() {
    let window = WindowOpenOptions {
        title: "Baseview",
        width: 800,
        height: 400,
        parent: Parent::None
    };

    create_window(window);

    loop {
        if !handle_msg(null_mut()) {
            break;
        }
    }
}