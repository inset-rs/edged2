//! What the Dock shows on its tiles: the badge text, such as an unread count.

use std::collections::HashMap;

use objc2_app_kit::NSWorkspace;

use crate::ax::{self, Element};

/// The badge on each running application's Dock tile, keyed by bundle path.
///
/// Read from the Dock's own accessibility tree, which is the only place the
/// text is published. Empty until accessibility access is granted.
pub fn badges() -> HashMap<String, String> {
    let Some(dock) = dock_pid() else {
        return HashMap::new();
    };
    let dock = Element::application(dock);
    let Some(list) = dock
        .elements(ax::CHILDREN)
        .into_iter()
        .find(|child| child.string(ax::ROLE).as_deref() == Some(ax::LIST_ROLE))
    else {
        return HashMap::new();
    };
    list.elements(ax::CHILDREN)
        .into_iter()
        .filter(|tile| tile.string(ax::SUBROLE).as_deref() == Some(ax::APPLICATION_DOCK_ITEM))
        .filter(|tile| tile.boolean(ax::IS_APPLICATION_RUNNING).unwrap_or(false))
        .filter_map(|tile| {
            let path = tile.path(ax::URL)?;
            let label = tile.string(ax::STATUS_LABEL)?;
            Some((path.trim_end_matches('/').to_owned(), label))
        })
        .collect()
}

fn dock_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .find(|app| {
            app.bundleIdentifier()
                .is_some_and(|id| id.to_string() == "com.apple.dock")
        })
        .map(|app| app.processIdentifier())
}
