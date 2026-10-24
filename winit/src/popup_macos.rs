//! Popup windows on macOS: the stand-in for Wayland popup surfaces.
//!
//! A window opened with [`PlatformSpecific::popup_of`] set is a menu-like child of another
//! window. Two things make it behave like one:
//!
//! 1. [`place`] turns its position, given relative to the parent's content area, into a
//!    desktop position, moved as little as needed to keep it on the parent's monitor. This
//!    is what a Wayland compositor does for a "reactive" popup positioner.
//! 2. [`configure`] raises the new window to the pop-up menu level and adds a tracking area
//!    that is always active, so it gets mouse-moved events although it is never the key
//!    window. Hover is how menus highlight items and open submenus. Chromium and Flutter
//!    track the mouse in their floating windows the same way. It also shows the window
//!    with `orderFrontRegardless`: showing a winit window the usual way calls
//!    `makeKeyAndOrderFront:`.
//!
//! The window itself is winit's popup panel (`with_popup`), whose class refuses to become
//! the key or main window, so the parent keeps keyboard focus.
//!
//! [`PlatformSpecific::popup_of`]: crate::core::window::settings::PlatformSpecific::popup_of

use crate::core::window;
use crate::core::{Point, Rectangle, Size};

/// Where a popup of `size` goes so that it stays inside `area`, given where it wants to be.
///
/// Like a reactive Wayland positioner: a popup that would cross the right edge moves to
/// `flip_x` when that is given and fits, and otherwise shifts back just far enough. One that
/// would cross the bottom edge shifts up. One larger than the area is pinned to its
/// top-left. All values are in the same logical coordinate space.
pub fn fit(wanted: Point, flip_x: Option<f32>, size: Size, area: Rectangle) -> Point {
    let crosses_right = wanted.x + size.width > area.x + area.width;
    let wanted = match flip_x {
        Some(x) if crosses_right && x >= area.x => Point::new(x, wanted.y),
        _ => wanted,
    };
    let axis = |wanted: f32, length: f32, start: f32, extent: f32| {
        let end = start + extent;
        if wanted + length > end {
            (end - length).max(start)
        } else {
            wanted.max(start)
        }
    };
    Point::new(
        axis(wanted.x, size.width, area.x, area.width),
        axis(wanted.y, size.height, area.y, area.height),
    )
}

/// Resolves a popup's position against its parent before the window is created.
///
/// `settings.position` must be [`window::Position::Specific`], relative to the parent's
/// content area. Leaves `settings` unchanged when it is not a popup, the position is of
/// another kind, or the parent is gone.
pub fn place(
    settings: &mut window::Settings,
    parent: Option<&dyn winit::window::Window>,
) {
    let (Some(_), window::Position::Specific(offset), Some(parent)) = (
        settings.platform_specific.popup_of,
        settings.position,
        parent,
    ) else {
        return;
    };
    let scale = parent.scale_factor();
    let Ok(outer) = parent.outer_position() else {
        return;
    };
    let surface = parent.surface_position();
    let origin = Point::new(
        (f64::from(outer.x + surface.x) / scale) as f32,
        (f64::from(outer.y + surface.y) / scale) as f32,
    );
    let wanted = Point::new(origin.x + offset.x, origin.y + offset.y);
    let flip_x = settings
        .platform_specific
        .popup_flip_x
        .map(|x| origin.x + x);
    let area = parent.current_monitor().and_then(|monitor| {
        let position = monitor.position()?;
        let size = monitor.current_video_mode()?.size();
        let monitor_scale = monitor.scale_factor();
        Some(Rectangle::new(
            Point::new(
                (f64::from(position.x) / monitor_scale) as f32,
                (f64::from(position.y) / monitor_scale) as f32,
            ),
            Size::new(
                (f64::from(size.width) / monitor_scale) as f32,
                (f64::from(size.height) / monitor_scale) as f32,
            ),
        ))
    });
    let position = match area {
        Some(area) => fit(wanted, flip_x, settings.size, area),
        None => wanted,
    };
    settings.position = window::Position::Specific(position);
}

/// Makes a newly created popup window behave like a menu. See the module docs.
pub fn configure(window: &dyn winit::window::Window) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSPopUpMenuWindowLevel, NSTrackingArea, NSTrackingAreaOptions, NSView,
    };
    use objc2_foundation::NSRect;
    use raw_window_handle::RawWindowHandle;

    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("popup window configured off the main thread; skipped");
        return;
    };
    let Ok(handle) = window.rwh_06_window_handle().window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: winit hands out a valid NSView pointer for the window's lifetime, and we are
    // on the main thread, where AppKit objects may be used.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let Some(ns_window) = view.window() else {
        return;
    };
    ns_window.setLevel(NSPopUpMenuWindowLevel);
    let options = NSTrackingAreaOptions::MouseMoved
        | NSTrackingAreaOptions::MouseEnteredAndExited
        | NSTrackingAreaOptions::ActiveAlways
        | NSTrackingAreaOptions::InVisibleRect;
    // SAFETY: the view outlives its tracking areas; AppKit removes them with the view.
    let area = unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            mtm.alloc(),
            NSRect::ZERO,
            options,
            Some(view),
            None,
        )
    };
    view.addTrackingArea(&area);
    // Not `makeKeyAndOrderFront:`, which is what showing a window through winit does:
    // that would take focus from the parent.
    ns_window.orderFrontRegardless();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(1000.0, 800.0))
    }

    #[test]
    fn a_popup_that_fits_stays_where_it_was_asked() {
        let at = Point::new(100.0, 200.0);
        assert_eq!(fit(at, None, Size::new(300.0, 400.0), screen()), at);
    }

    #[test]
    fn crossing_the_bottom_edge_moves_it_up_just_enough() {
        let at = fit(Point::new(100.0, 600.0), None, Size::new(300.0, 400.0), screen());
        assert_eq!(at, Point::new(100.0, 400.0));
    }

    #[test]
    fn crossing_the_right_edge_moves_it_left_just_enough() {
        let at = fit(Point::new(900.0, 100.0), None, Size::new(300.0, 400.0), screen());
        assert_eq!(at, Point::new(700.0, 100.0));
    }

    #[test]
    fn taller_than_the_screen_pins_to_the_top() {
        let at = fit(Point::new(100.0, 300.0), None, Size::new(300.0, 1200.0), screen());
        assert_eq!(at, Point::new(100.0, 0.0));
    }

    #[test]
    fn a_second_monitor_offset_is_respected() {
        let right = Rectangle::new(Point::new(1000.0, 0.0), Size::new(800.0, 600.0));
        let at = fit(Point::new(1700.0, 500.0), None, Size::new(300.0, 400.0), right);
        assert_eq!(at, Point::new(1500.0, 200.0));
    }

    #[test]
    fn crossing_the_right_edge_flips_to_the_other_side() {
        // A submenu beside an item ending at x 900 flips to the item's left side at 500.
        let at = fit(Point::new(900.0, 100.0), Some(500.0), Size::new(300.0, 400.0), screen());
        assert_eq!(at, Point::new(500.0, 100.0));
    }

    #[test]
    fn a_flip_is_ignored_while_the_popup_fits() {
        let at = fit(Point::new(100.0, 100.0), Some(-200.0), Size::new(300.0, 400.0), screen());
        assert_eq!(at, Point::new(100.0, 100.0));
    }

    #[test]
    fn a_flip_off_the_left_edge_falls_back_to_shifting() {
        let at = fit(Point::new(900.0, 100.0), Some(-200.0), Size::new(300.0, 400.0), screen());
        assert_eq!(at, Point::new(700.0, 100.0));
    }

    #[test]
    fn left_of_the_area_is_pulled_in() {
        let right = Rectangle::new(Point::new(1000.0, 0.0), Size::new(800.0, 600.0));
        let at = fit(Point::new(950.0, 10.0), None, Size::new(100.0, 100.0), right);
        assert_eq!(at, Point::new(1000.0, 10.0));
    }
}
