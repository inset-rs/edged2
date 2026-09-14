//! The glass behind the panel and its shape, on the window the host made.
//!
//! The host's own backgrounds fill a window whole. The panel wants its
//! leading corners rounded and its trailing corners square against the
//! screen's edge, so it places the view itself, one radius wider than the
//! window: the corners that hang past the edge are clipped square.
//!
//! The view goes beside the window's content view, in the window's frame.
//! Liquid Glass renders as its flat material there rather than as glass;
//! inside the content view it would render whole, but the content view is
//! the host's, drawn through a Metal layer that a view beneath it did not
//! stay beneath. The flat material is what the panel has.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSRect, NSSize};
use raw_window_handle::RawWindowHandle;

/// Puts the glass material, or the vibrancy blur where the system has no
/// glass, behind the content of the window `handle` names, rounded by
/// `corner_radius` on the side away from the screen's edge. Returns false
/// when the handle is not a live AppKit view.
pub fn install_backdrop(handle: RawWindowHandle, corner_radius: f64) -> bool {
    install(handle, corner_radius, corner_radius)
}

/// The same, rounded by `corner_radius` at every corner and no wider than
/// the window: for a window that stands free of any edge.
pub fn install_rounded_backdrop(handle: RawWindowHandle, corner_radius: f64) -> bool {
    install(handle, corner_radius, 0.0)
}

fn install(handle: RawWindowHandle, corner_radius: f64, past_trailing_edge: f64) -> bool {
    let RawWindowHandle::AppKit(appkit) = handle else {
        return false;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    // SAFETY: the host hands out the NSView of a live window, and AppKit
    // views are read on the main thread.
    let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    let Some(content) = view.window().and_then(|window| window.contentView()) else {
        return false;
    };
    // The backdrop goes behind the content view as a sibling in the window's
    // frame view: a subview of the content would draw over its Metal layer.
    // SAFETY: the view hierarchy of a live window, read on the main thread.
    let Some(frame_view) = (unsafe { content.superview() }) else {
        return false;
    };
    let frame = content.frame();
    let frame = NSRect::new(
        frame.origin,
        NSSize::new(frame.size.width + past_trailing_edge, frame.size.height),
    );
    let backdrop = backdrop_view(mtm, frame, corner_radius);
    backdrop.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    frame_view.addSubview_positioned_relativeTo(
        &backdrop,
        NSWindowOrderingMode::Below,
        Some(&content),
    );
    true
}

/// The glass material where the system has it (macOS 26), else the sidebar blur.
fn backdrop_view(mtm: MainThreadMarker, frame: NSRect, corner_radius: f64) -> Retained<NSView> {
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), frame);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(corner_radius);
        return Retained::into_super(glass);
    }
    let blur = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
    blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    blur.setState(NSVisualEffectState::Active);
    blur.setMaterial(NSVisualEffectMaterial::Sidebar);
    blur.setWantsLayer(true);
    if let Some(layer) = blur.layer() {
        layer.setCornerRadius(corner_radius);
        layer.setMasksToBounds(true);
    }
    Retained::into_super(blur)
}
