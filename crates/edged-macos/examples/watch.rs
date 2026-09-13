//! Prints each change macOS reports about the desktop as it happens, and how
//! long it took to start listening.
//!
//! ```text
//! cargo run -p edged-macos --example watch
//! ```

use std::time::Instant;

use objc2_core_foundation::CFRunLoop;

fn main() {
    let started = Instant::now();
    let mut watcher = edged_macos::Watcher::new(|change| println!("{change:?}"));
    println!("workspace subscriptions: {:?}", started.elapsed());
    for application in edged_macos::running_applications() {
        let at = Instant::now();
        let windows = edged_macos::listed_windows(application.pid);
        watcher.watch(&application, &windows);
        println!(
            "  {:<28} {} window(s)  {:?}",
            application.name,
            windows.len(),
            at.elapsed()
        );
    }
    println!(
        "listening after {:?}; switch windows, then ctrl-c",
        started.elapsed()
    );
    CFRunLoop::run();
}
