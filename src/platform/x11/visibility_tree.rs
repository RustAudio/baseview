use crate::platform::X11Connection;
use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{ConnectionExt, MapState, QueryTreeReply};
use x11rb::protocol::ErrorKind;
use x11rb::x11_utils::X11Error;

pub enum AncestorVisibilityState {
    Floating {
        own_window_viewable: Cell<bool>,
        own_window_id: NonZeroU32,
    },
    Parented {
        ancestry: AncestryList,
        root_id: Cell<Option<NonZeroU32>>,
        own_window_viewable: Cell<bool>,
    },
}

pub struct AncestryList {
    inner: RefCell<Vec<Ancestor>>,
}

impl AncestryList {
    pub fn new(own_window: NonZeroU32) -> Self {
        Self { inner: RefCell::new(vec![Ancestor { id: own_window, mapped: false.into() }]) }
    }

    pub fn pop_id(&self) -> Option<NonZeroU32> {
        self.inner.borrow_mut().pop().map(|a| a.id)
    }

    pub fn last_id(&self) -> Option<NonZeroU32> {
        self.inner.borrow().last().map(|a| a.id)
    }

    pub fn push(&self, ancestor: Ancestor) {
        self.inner.borrow_mut().push(ancestor);
    }

    pub fn parent_id(&self) -> Option<NonZeroU32> {
        self.inner.borrow().get(1).map(|a| a.id)
    }
    pub fn own_window_id(&self) -> Option<NonZeroU32> {
        self.inner.borrow().first().map(|a| a.id)
    }

    pub fn remove_window(&self, id: NonZeroU32) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(index) = inner.iter().position(|a| a.id == id) else {
            return false;
        };

        inner.truncate(index.saturating_add(1));

        true
    }

    pub fn remove_after_window(&self, id: NonZeroU32) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(index) = inner.iter().position(|a| a.id == id) else {
            return false;
        };

        inner.truncate(index.saturating_add(2));

        true
    }

    pub fn check_all_mapped(&self) -> bool {
        self.inner.borrow().iter().all(|a| a.mapped.get())
    }

    pub fn set_mapped(&self, window: NonZeroU32, mapped: bool) -> bool {
        let inner = self.inner.borrow();
        let Some(ancestor) = inner.iter().find(|a| a.id == window) else {
            return false;
        };

        ancestor.mapped.set(mapped);
        true
    }
}

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct Ancestor {
    id: NonZeroU32,
    mapped: Cell<bool>,
}

impl AncestorVisibilityState {
    pub fn discover(
        connection: &X11Connection, own_window_id: NonZeroU32, parented: bool,
    ) -> Result<Self, ReplyError> {
        if !parented {
            return Ok(Self::Floating { own_window_viewable: Cell::new(false), own_window_id });
        }

        let this = Self::Parented {
            ancestry: AncestryList::new(own_window_id),
            own_window_viewable: Cell::new(false),
            root_id: Cell::new(NonZeroU32::new(connection.default_screen().root)),
        };

        this.try_regenerate_from_last_window(connection)?;

        Ok(this)
    }

    pub fn own_window_is_viewable(&self) -> bool {
        match self {
            Self::Parented { own_window_viewable, .. } => own_window_viewable.get(),
            Self::Floating { own_window_viewable, .. } => own_window_viewable.get(),
        }
    }

    pub fn parent_id(&self) -> Option<NonZeroU32> {
        match self {
            Self::Parented { ancestry, .. } => ancestry.parent_id(),
            _ => None,
        }
    }

    /// Returns `true` if this operation made our own window visible.
    pub fn window_mapped(&self, window_id: NonZeroU32) -> bool {
        match self {
            Self::Floating { own_window_id, own_window_viewable } => {
                if *own_window_id != window_id {
                    return false;
                }

                if own_window_viewable.get() {
                    return false;
                }

                own_window_viewable.set(true);
                true
            }
            Self::Parented { own_window_viewable, ancestry, .. } => {
                if !ancestry.set_mapped(window_id, true) {
                    return false;
                }

                if own_window_viewable.get() {
                    return false;
                }

                let all_mapped = ancestry.check_all_mapped();
                if all_mapped {
                    own_window_viewable.set(true);
                }

                all_mapped
            }
        }
    }

    pub fn window_unmapped(&self, window_id: NonZeroU32) {
        match self {
            Self::Floating { own_window_id, own_window_viewable } => {
                if *own_window_id != window_id {
                    return;
                }

                own_window_viewable.set(false);
            }
            Self::Parented { own_window_viewable, ancestry, .. } => {
                if !ancestry.set_mapped(window_id, false) {
                    return;
                }

                own_window_viewable.set(false);
            }
        }
    }

    pub fn window_destroyed(&self, window_id: NonZeroU32, connection: &X11Connection) {
        let Self::Parented { ancestry, .. } = &self else {
            return;
        };

        if !ancestry.remove_window(window_id) {
            return;
        }

        self.regenerate_from_last_window(connection);
    }

    pub fn window_reparented(
        &self, window_id: NonZeroU32, new_parent: Option<NonZeroU32>, connection: &X11Connection,
    ) {
        let Self::Parented { ancestry, root_id, .. } = &self else {
            return;
        };

        if !ancestry.remove_after_window(window_id) {
            return;
        }

        if let Some(new_parent) = new_parent {
            if Some(new_parent) == root_id.get() {
                return;
            }

            ancestry.push(Ancestor { id: new_parent, mapped: Cell::new(false) });

            self.regenerate_from_last_window(connection);
        }
    }

    pub fn regenerate_from_last_window(&self, connection: &X11Connection) {
        if let Err(e) = self.try_regenerate_from_last_window(connection) {
            crate::warn!("Failed to generate window ancestry list: {}", e)
        }
    }

    fn try_regenerate_from_last_window(
        &self, connection: &X11Connection,
    ) -> Result<(), ReplyError> {
        let Self::Parented { ancestry, own_window_viewable, root_id } = &self else {
            return Ok(());
        };

        let Some(mut current_window) = ancestry.pop_id() else { return Ok(()) };

        let mut shitlist = Vec::new();
        let mut rechecked_children = Vec::new();

        loop {
            let Some((mut mapped, tree)) = fetch_window_info(connection, current_window)? else {
                // We got a BadWindow while trying to get a window's info, it must have been destroyed.
                // Try to go back a layer and fetch the window's state and parent again

                crate::warn!("Failed to get info for window {}: XBadWindow", current_window);

                let Some(previous_parent) = ancestry.pop_id() else {
                    // No previous parent, this was the first window. Stop everything and return an empty state
                    break;
                };

                if shitlist.contains(&previous_parent) {
                    crate::warn!(
                        "Failed to get info for window {} in the past already. Stopping.",
                        previous_parent
                    );
                    break;
                }

                if shitlist.len() > 10 {
                    crate::warn!(
                        "Too many failures while trying to build X ancestry tree. Stopping."
                    );
                    break;
                }

                shitlist.push(previous_parent);

                current_window = previous_parent;
                continue;
            };

            if tree.parent == current_window.get() {
                // Weird, but that might also mean we're at the end of the tree (or the window has no parent yet)
                break;
            }

            // Sanity check if the current parent is actually registered to have the child in its children list
            if let Some(child_id) = ancestry.last_id() {
                if !tree.children.contains(&child_id.get()) {
                    // The child has been orphaned, it must have been reparented between our server queries.
                    // Go back a step and check again.

                    if rechecked_children.contains(&child_id) {
                        crate::warn!(
                            "Children of parent {} does not contain {}: {:?}",
                            current_window,
                            child_id,
                            &tree.children
                        );
                    } else {
                        rechecked_children.push(child_id);
                    }

                    let Some(_) = ancestry.pop_id() else { unreachable!() };
                    current_window = child_id;
                    continue;
                }
            }

            if ancestry.own_window_id().is_some_and(|id| id != current_window) {
                // Despite what's documented, all windows down the parent tree must have the event mask
                // bit set, otherwise events are not propagated through to us.
                if let Err(e) =
                    connection.register_tree_structure_events_for_window(current_window)?.check()
                {
                    crate::warn!(
                        "Could not register SubstructureNotify event for window {}: {}",
                        current_window,
                        e
                    );
                    mapped = true; // Assume it is mapped, since we'll possibly not get any events from this window
                }
            }

            // All checks succeeded, now register the current window info and fetch info from the parent
            ancestry.push(Ancestor { id: current_window, mapped: mapped.into() });

            if tree.parent == tree.root {
                // No need to get info for the root, we assume it's always there. We can just stop here.
                break;
            }

            // If parent == 0, assume there's no parent and just break
            if let Some(parent) = NonZeroU32::new(tree.parent) {
                current_window = parent;
            } else {
                break;
            }

            if let Some(root) = NonZeroU32::new(tree.root) {
                if Some(root) != root_id.get() {
                    root_id.set(Some(root))
                }
            }
        }

        own_window_viewable.set(ancestry.check_all_mapped());

        Ok(())
    }
}

/// Returns Ok(None) on BadWindow.
fn fetch_window_info(
    connection: &X11Connection, window: NonZeroU32,
) -> Result<Option<(bool, QueryTreeReply)>, ReplyError> {
    let attrs_cookie = connection.conn.get_window_attributes(window.get())?;
    let tree_cookie = connection.conn.query_tree(window.get())?;

    let mapped = match attrs_cookie.reply() {
        Ok(attr) => attr.map_state != MapState::UNMAPPED,
        Err(ReplyError::X11Error(X11Error { error_kind: ErrorKind::Window, .. })) => {
            tree_cookie.discard_reply_and_errors();
            return Ok(None);
        }
        Err(e) => {
            tree_cookie.discard_reply_and_errors();
            return Err(e);
        }
    };

    let tree = match tree_cookie.reply() {
        Ok(tree) => tree,
        Err(ReplyError::X11Error(X11Error { error_kind: ErrorKind::Window, .. })) => {
            return Ok(None)
        }
        Err(e) => return Err(e),
    };

    Ok(Some((mapped, tree)))
}
