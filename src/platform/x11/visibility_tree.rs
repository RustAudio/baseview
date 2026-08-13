use std::cell::{Cell, RefCell};
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{ConnectionExt, MapState, QueryTreeReply, Window};
use x11rb::protocol::ErrorKind;
use x11rb::x11_utils::X11Error;
use x11rb::xcb_ffi::XCBConnection;

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct AncestorVisibilityState {
    ancestry: RefCell<Vec<Ancestor>>,
    own_window_id: Window,
    own_window_viewable: Cell<bool>,
}

#[cfg_attr(debug_assertions, derive(Debug))]
struct Ancestor {
    id: Window,
    mapped: Cell<bool>,
}

impl AncestorVisibilityState {
    pub fn discover(connection: &XCBConnection, own_window_id: Window) -> Result<Self, ReplyError> {
        let mut current_window = own_window_id;
        let mut ancestry: Vec<Ancestor> = Vec::new();

        loop {
            let (Some(mapped), Some(tree)) = (
                fetch_is_window_mapped(connection, current_window)?,
                fetch_window_tree(connection, current_window)?,
            ) else {
                // We got a BadWindow while trying to get a window's info, it must have been destroyed.
                // Try to go back a layer and fetch the window's state and parent again

                crate::warn!("Failed to get info for window {}: XBadWindow", current_window);

                let Some(previous_parent) = ancestry.pop() else {
                    // No previous parent, this was the first window. Stop everything and return an empty state
                    break;
                };

                current_window = previous_parent.id;
                continue;
            };

            if tree.parent == current_window {
                // Weird, but that might also mean we're at the end of the tree (or the window has no parent yet)
                break;
            }

            // Sanity check if the current parent is actually registered to have the child in its children list
            if let Some(child_id) = ancestry.last().map(|a| a.id) {
                if !tree.children.contains(&child_id) {
                    // The child has been orphaned, it must have been reparented between our server queries.
                    // Go back a step and check again.

                    crate::warn!(
                        "Children of parent {} does not contain {}: {:?}",
                        current_window,
                        child_id,
                        &tree.children
                    );

                    let Some(_) = ancestry.pop() else { unreachable!() };
                    current_window = child_id;
                    continue;
                }
            }

            // All checks succeeded, now register the current window info and fetch info from the parent
            ancestry.push(Ancestor { id: current_window, mapped: mapped.into() });

            if tree.parent == tree.root {
                // No need to get info for the root, we assume it's always there. We can just stop here.
                break;
            }

            current_window = tree.parent;
        }

        Ok(Self {
            own_window_id,
            own_window_viewable: ancestry.iter().all(|a| a.mapped.get()).into(),
            ancestry: ancestry.into(),
        })
    }

    pub fn own_window_is_viewable(&self) -> bool {
        self.own_window_viewable.get()
    }

    /// Returns `true` if the window is currently tracked, `false` otherwise
    pub fn window_mapped(&self, mapped_window_id: Window) -> bool {
        let ancestry = self.ancestry.borrow();
        let Some(ancestor) = ancestry.iter().find(|a| a.id == mapped_window_id) else {
            return false;
        };

        ancestor.mapped.set(true);

        let all_mapped = ancestry.iter().all(|a| a.mapped.get());
        if all_mapped {
            self.own_window_viewable.set(true);
        }

        true
    }

    pub fn window_unmapped(&self, mapped_window_id: Window) -> bool {
        let ancestry = self.ancestry.borrow();
        let Some(ancestor) = ancestry.iter().find(|a| a.id == mapped_window_id) else {
            return false;
        };

        ancestor.mapped.set(false);

        if self.own_window_viewable.get() {
            self.own_window_viewable.set(false);
        }

        true
    }
}

/// Returns Ok(None) on BadWindow
fn fetch_is_window_mapped(
    connection: &XCBConnection, window: Window,
) -> Result<Option<bool>, ReplyError> {
    match connection.get_window_attributes(window)?.reply() {
        Ok(attr) => Ok(Some(attr.map_state != MapState::UNMAPPED)),
        Err(ReplyError::X11Error(X11Error { error_kind: ErrorKind::Window, .. })) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Returns Ok(None) on BadWindow
fn fetch_window_tree(
    connection: &XCBConnection, window: Window,
) -> Result<Option<QueryTreeReply>, ReplyError> {
    match connection.query_tree(window)?.reply() {
        Ok(tree) => Ok(Some(tree)),
        Err(ReplyError::X11Error(X11Error { error_kind: ErrorKind::Window, .. })) => Ok(None),
        Err(e) => Err(e),
    }
}
