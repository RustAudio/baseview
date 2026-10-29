use crate::platform::BaseviewWindow;
use crate::wrappers::win32::cursor::SystemCursor;
use crate::wrappers::win32::window::WmCursor;

impl WmCursor for BaseviewWindow {
    fn on_set_cursor(&self, mouse_in_window: bool) -> bool {
        if mouse_in_window {
            // Here we need to set the cursor back to what the state says, since it can have changed when outside the window
            if let Ok(cursor) = SystemCursor::load(self.window_state.cursor_icon.get()) {
                cursor.set()
            }
            true
        } else {
            // Cursor is being changed by some other window, e.g. when having mouse on the borders to resize it
            false
        }
    }
}
