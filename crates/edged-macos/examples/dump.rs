//! Prints what macOS reports about the desktop right now: the screens, their
//! Spaces, and every application with the windows it has open.
//!
//! ```text
//! cargo run -p edged-macos --example dump
//! ```

fn main() {
    if !edged_macos::is_trusted() {
        println!("accessibility access not granted: no windows will be listed\n");
    }

    println!("SCREENS");
    for screen in edged_macos::screens() {
        let frame = screen.frame;
        let visible = screen.visible_frame;
        println!(
            "  {}{}  {:.0}x{:.0} at ({:.0}, {:.0})  visible {:.0}x{:.0} at ({:.0}, {:.0})  @{}x  {}",
            screen.display_id,
            if screen.is_main { " (main)" } else { "" },
            frame.width,
            frame.height,
            frame.x,
            frame.y,
            visible.width,
            visible.height,
            visible.x,
            visible.y,
            screen.scale,
            screen.uuid,
        );
    }

    println!("\nSPACES");
    for space in edged_macos::spaces() {
        let current = edged_macos::current_space(&space.display_uuid) == space.id;
        println!(
            "  {:>6}  {:?}{}{}",
            space.id,
            space.kind,
            space
                .index
                .map(|index| format!("  desktop {}", index + 1))
                .unwrap_or_default(),
            if current { "  <- showing" } else { "" },
        );
    }

    let badges = edged_macos::badges();
    println!("\nAPPLICATIONS");
    let mut applications = edged_macos::running_applications();
    applications.sort_by(|left, right| left.name.cmp(&right.name));
    for application in applications {
        let windows = edged_macos::listed_windows(application.pid);
        let badge = application
            .bundle_path
            .as_ref()
            .and_then(|path| badges.get(path))
            .map(|label| format!("  badge {label}"))
            .unwrap_or_default();
        println!(
            "  {}{}  (pid {}, {} window{}){badge}",
            application.name,
            if application.is_active { " *" } else { "" },
            application.pid,
            windows.len(),
            if windows.len() == 1 { "" } else { "s" },
        );
        for window in windows {
            println!(
                "      [{}] {:?}{}{}  spaces {:?}",
                window.id,
                window.title,
                if window.is_minimized {
                    " minimized"
                } else {
                    ""
                },
                if window.is_fullscreen {
                    " fullscreen"
                } else {
                    ""
                },
                window.spaces,
            );
        }
    }
    println!("\nlaunches at login: {}", edged_macos::launches_at_login());
}
