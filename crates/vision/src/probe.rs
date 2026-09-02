// SPDX-License-Identifier: MPL-2.0

/// Fraction of pixels in the rectangle whose colour sits within `tolerance` of
/// `rgb` on every channel.
///
/// A fraction rather than a yes/no, because that is what lets the confidence
/// fall smoothly as the screen drifts instead of flipping.
pub fn colour_fraction(
    bgra: &[u8],
    width: u32,
    height: u32,
    rect: (u32, u32, u32, u32),
    rgb: [u8; 3],
    tolerance: u8,
) -> Option<f64> {
    let (left, top, w, h) = rect;
    if w == 0 || h == 0 || left + w > width || top + h > height {
        return None;
    }
    if bgra.len() < (width as usize) * (height as usize) * 4 {
        return None;
    }

    let tolerance = tolerance as i32;
    let mut hits = 0u64;
    for y in top..top + h {
        for x in left..left + w {
            let index = ((y as usize) * (width as usize) + x as usize) * 4;
            let b = bgra[index] as i32;
            let g = bgra[index + 1] as i32;
            let r = bgra[index + 2] as i32;
            if (r - rgb[0] as i32).abs() <= tolerance
                && (g - rgb[1] as i32).abs() <= tolerance
                && (b - rgb[2] as i32).abs() <= tolerance
            {
                hits += 1;
            }
        }
    }

    Some(hits as f64 / ((w as u64) * (h as u64)) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(colours: &[[u8; 3]]) -> Vec<u8> {
        colours
            .iter()
            .flat_map(|[r, g, b]| [*b, *g, *r, 255])
            .collect()
    }

    #[test]
    fn a_uniform_patch_matches_entirely() {
        let bgra = canvas(&[[200, 30, 40]; 4]);

        let fraction = colour_fraction(&bgra, 2, 2, (0, 0, 2, 2), [200, 30, 40], 0);

        assert_eq!(fraction, Some(1.0));
    }

    #[test]
    fn a_half_matching_patch_reads_as_a_half() {
        let bgra = canvas(&[[200, 30, 40], [0, 0, 0], [200, 30, 40], [0, 0, 0]]);

        let fraction = colour_fraction(&bgra, 2, 2, (0, 0, 2, 2), [200, 30, 40], 0);

        assert_eq!(fraction, Some(0.5));
    }

    #[test]
    fn tolerance_is_inclusive_and_per_channel() {
        let bgra = canvas(&[[205, 30, 40]]);

        assert_eq!(
            colour_fraction(&bgra, 1, 1, (0, 0, 1, 1), [200, 30, 40], 5),
            Some(1.0)
        );
        assert_eq!(
            colour_fraction(&bgra, 1, 1, (0, 0, 1, 1), [200, 30, 40], 4),
            Some(0.0),
            "a channel outside tolerance must fail the whole pixel"
        );
    }

    #[test]
    fn only_the_requested_rectangle_is_read() {
        let bgra = canvas(&[[255, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]]);

        assert_eq!(
            colour_fraction(&bgra, 2, 2, (0, 0, 1, 1), [255, 0, 0], 0),
            Some(1.0)
        );
        assert_eq!(
            colour_fraction(&bgra, 2, 2, (1, 0, 1, 1), [255, 0, 0], 0),
            Some(0.0)
        );
    }

    #[test]
    fn a_rectangle_outside_the_frame_is_refused() {
        let bgra = canvas(&[[0, 0, 0]; 4]);

        assert!(colour_fraction(&bgra, 2, 2, (1, 1, 2, 2), [0, 0, 0], 0).is_none());
        assert!(colour_fraction(&bgra, 2, 2, (0, 0, 0, 1), [0, 0, 0], 0).is_none());
    }
}
