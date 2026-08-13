use std::cell::{Cell, RefCell};
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{ConnectionExt, MapState, QueryTreeReply, Window};
use x11rb::protocol::ErrorKind;
use x11rb::x11_utils::X11Error;
use x11rb::xcb_ffi::XCBConnection;

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct AncestorVisibilityState {
    ancestry: AncestryList,
    own_window_viewable: Cell<bool>,
}

#[cfg_attr(debug_assertions, derive(Debug))]
struct AncestryList {
    inner: RefCell<Vec<Ancestor>>,
}

impl AncestryList {
    pub fn new() -> Self {
        Self { inner: RefCell::new(Vec::new()) }
    }

    pub fn pop_id(&self) -> Option<Window> {
        self.inner.borrow_mut().pop().map(|a| a.id)
    }

    pub fn last_id(&self) -> Option<Window> {
        self.inner.borrow().last().map(|a| a.id)
    }

    pub fn push(&self, ancestor: Ancestor) {
        self.inner.borrow_mut().push(ancestor);
    }

    pub fn check_all_mapped(&self) -> bool {
        self.inner.borrow().iter().all(|a| a.mapped.get())
    }

    pub fn set_mapped(&self, window: Window, mapped: bool) -> bool {
        let inner = self.inner.borrow();
        let Some(ancestor) = inner.iter().find(|a| a.id == window) else {
            return false;
        };

        ancestor.mapped.set(mapped);
        true
    }
}

#[cfg_attr(debug_assertions, derive(Debug))]
struct Ancestor {
    id: Window,
    mapped: Cell<bool>,
}

impl AncestorVisibilityState {
    pub fn discover(connection: &XCBConnection, own_window_id: Window) -> Result<Self, ReplyError> {
        let mut current_window = own_window_id;
        let ancestry = AncestryList::new();

        loop {
            let (Some(mapped), Some(tree)) = (
                fetch_is_window_mapped(connection, current_window)?,
                fetch_window_tree(connection, current_window)?,
            ) else {
                // We got a BadWindow while trying to get a window's info, it must have been destroyed.
                // Try to go back a layer and fetch the window's state and parent again

                crate::warn!("Failed to get info for window {}: XBadWindow", current_window);

                let Some(previous_parent) = ancestry.pop_id() else {
                    // No previous parent, this was the first window. Stop everything and return an empty state
                    break;
                };

                current_window = previous_parent;
                continue;
            };

            if tree.parent == current_window {
                // Weird, but that might also mean we're at the end of the tree (or the window has no parent yet)
                break;
            }

            // Sanity check if the current parent is actually registered to have the child in its children list
            if let Some(child_id) = ancestry.last_id() {
                if !tree.children.contains(&child_id) {
                    // The child has been orphaned, it must have been reparented between our server queries.
                    // Go back a step and check again.

                    crate::warn!(
                        "Children of parent {} does not contain {}: {:?}",
                        current_window,
                        child_id,
                        &tree.children
                    );

                    let Some(_) = ancestry.pop_id() else { unreachable!() };
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

        Ok(Self { own_window_viewable: ancestry.check_all_mapped().into(), ancestry })
    }

    pub fn own_window_is_viewable(&self) -> bool {
        self.own_window_viewable.get()
    }

    /// Returns `true` if this operation made our own window visible
    pub fn window_mapped(&self, mapped_window_id: Window) -> bool {
        if !self.ancestry.set_mapped(mapped_window_id, true) {
            return false;
        }

        if self.own_window_viewable.get() {
            return false;
        }

        let all_mapped = self.ancestry.check_all_mapped();
        if all_mapped {
            self.own_window_viewable.set(true);
        }

        all_mapped
    }

    pub fn window_unmapped(&self, mapped_window_id: Window) {
        if !self.ancestry.set_mapped(mapped_window_id, false) {
            return;
        }

        self.own_window_viewable.set(false);
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
