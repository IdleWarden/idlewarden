// SPDX-License-Identifier: MPL-2.0
use idlewarden_plugin_api::Point;

/// A rectangle in screen pixels. Origin can be negative: a monitor placed left
/// of the primary one starts at a negative x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// Window-relative `0.0..=1.0` to screen pixels, converted as late as possible
/// so that moving or resizing the window costs nothing upstream (ADR-0007).
pub(crate) fn to_screen(point: Point, client: Rect) -> Option<(i32, i32)> {
    if client.width <= 0 || client.height <= 0 {
        return None;
    }
    let span_x = (client.width - 1) as f64;
    let span_y = (client.height - 1) as f64;
    Some((
        client.left + (point.x.clamp(0.0, 1.0) * span_x).round() as i32,
        client.top + (point.y.clamp(0.0, 1.0) * span_y).round() as i32,
    ))
}

/// Screen pixels to the `0..=65535` space `SendInput` expects for absolute
/// motion, normalised over the whole virtual desktop rather than the primary
/// monitor.
pub(crate) fn to_absolute(x: i32, y: i32, desktop: Rect) -> Option<(i32, i32)> {
    if desktop.width <= 1 || desktop.height <= 1 {
        return None;
    }
    let scale = |value: i32, origin: i32, span: i32| {
        let offset = (value - origin) as i64;
        ((offset * 65535) / (span - 1) as i64).clamp(0, 65535) as i32
    };
    Some((
        scale(x, desktop.left, desktop.width),
        scale(y, desktop.top, desktop.height),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    const CLIENT: Rect = Rect {
        left: 100,
        top: 50,
        width: 800,
        height: 600,
    };

    #[test]
    fn the_origin_maps_to_the_top_left_of_the_window() {
        assert_eq!(to_screen(point(0.0, 0.0), CLIENT), Some((100, 50)));
    }

    #[test]
    fn the_far_corner_stays_inside_the_window() {
        assert_eq!(
            to_screen(point(1.0, 1.0), CLIENT),
            Some((899, 649)),
            "1.0 must land on the last pixel, not one past the edge"
        );
    }

    #[test]
    fn the_centre_lands_in_the_middle() {
        let (x, y) = to_screen(point(0.5, 0.5), CLIENT).expect("valid");

        assert!((x - 499).abs() <= 1 && (y - 349).abs() <= 1, "got {x},{y}");
    }

    #[test]
    fn a_window_with_no_area_is_refused_rather_than_dividing_by_zero() {
        let empty = Rect {
            left: 0,
            top: 0,
            width: 0,
            height: 10,
        };

        assert!(to_screen(point(0.5, 0.5), empty).is_none());
    }

    #[test]
    fn a_point_outside_the_unit_square_is_clamped_into_the_window() {
        assert_eq!(to_screen(point(-1.0, 2.0), CLIENT), Some((100, 649)));
    }

    #[test]
    fn the_virtual_desktop_corners_map_to_the_ends_of_the_range() {
        let desktop = Rect {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };

        assert_eq!(to_absolute(0, 0, desktop), Some((0, 0)));
        assert_eq!(to_absolute(1919, 1079, desktop), Some((65535, 65535)));
    }

    #[test]
    fn a_monitor_left_of_the_primary_one_still_maps_into_range() {
        let desktop = Rect {
            left: -1920,
            top: 0,
            width: 3840,
            height: 1080,
        };

        assert_eq!(
            to_absolute(-1920, 0, desktop),
            Some((0, 0)),
            "a negative origin is the normal multi-monitor case, not an error"
        );
        let (middle, _) = to_absolute(0, 0, desktop).expect("valid");
        assert!(
            (middle - 32767).abs() <= 20,
            "the primary origin sits mid-desktop, got {middle}"
        );
    }

    #[test]
    fn a_point_off_the_desktop_is_clamped_not_wrapped() {
        let desktop = Rect {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };

        assert_eq!(to_absolute(-500, 5000, desktop), Some((0, 65535)));
    }

    #[test]
    fn a_degenerate_desktop_is_refused() {
        let flat = Rect {
            left: 0,
            top: 0,
            width: 1,
            height: 1080,
        };

        assert!(to_absolute(0, 0, flat).is_none());
    }
}
