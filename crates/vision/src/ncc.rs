// SPDX-License-Identifier: MPL-2.0
use crate::gray::Gray;

/// Scales tried around the authored template size, so a resolution change
/// degrades the score instead of failing outright (ADR-0006).
pub const SCALES: [f64; 5] = [0.75, 0.875, 1.0, 1.125, 1.25];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Found {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Normalised cross-correlation, clamped to `0.0..=1.0`.
    pub score: f64,
}

/// Best normalised cross-correlation of `needle` over `haystack`.
///
/// Normalised means a uniform brightness or contrast change does not move the
/// score, which is the whole reason to prefer it over a difference metric.
pub fn best_match(haystack: &Gray, needle: &Gray) -> Option<Found> {
    if needle.width == 0
        || needle.height == 0
        || needle.width > haystack.width
        || needle.height > haystack.height
    {
        return None;
    }

    let count = (needle.width as usize) * (needle.height as usize);
    let needle_mean = needle.pixels.iter().map(|&p| p as f64).sum::<f64>() / count as f64;
    let needle_dev: Vec<f64> = needle
        .pixels
        .iter()
        .map(|&p| p as f64 - needle_mean)
        .collect();
    let needle_norm = needle_dev.iter().map(|d| d * d).sum::<f64>().sqrt();
    if needle_norm == 0.0 {
        return None;
    }

    let mut best: Option<Found> = None;
    for top in 0..=haystack.height - needle.height {
        for left in 0..=haystack.width - needle.width {
            let score = correlate(haystack, needle, &needle_dev, needle_norm, left, top, count);
            if best.is_none_or(|found| score > found.score) {
                best = Some(Found {
                    x: left,
                    y: top,
                    width: needle.width,
                    height: needle.height,
                    score,
                });
            }
        }
    }
    best
}

fn correlate(
    haystack: &Gray,
    needle: &Gray,
    needle_dev: &[f64],
    needle_norm: f64,
    left: u32,
    top: u32,
    count: usize,
) -> f64 {
    let mut sum = 0.0;
    for y in 0..needle.height {
        for x in 0..needle.width {
            sum += haystack.at(left + x, top + y) as f64;
        }
    }
    let mean = sum / count as f64;

    let mut dot = 0.0;
    let mut norm = 0.0;
    for y in 0..needle.height {
        for x in 0..needle.width {
            let deviation = haystack.at(left + x, top + y) as f64 - mean;
            dot += deviation * needle_dev[(y as usize) * (needle.width as usize) + x as usize];
            norm += deviation * deviation;
        }
    }

    let norm = norm.sqrt();
    if norm == 0.0 {
        return 0.0;
    }
    (dot / (norm * needle_norm)).clamp(0.0, 1.0)
}

/// Best match across [`SCALES`], so the same template survives a window resize.
pub fn best_match_multi_scale(haystack: &Gray, needle: &Gray, scales: &[f64]) -> Option<Found> {
    let mut best: Option<Found> = None;
    for &scale in scales {
        let width = ((needle.width as f64) * scale).round() as u32;
        let height = ((needle.height as f64) * scale).round() as u32;
        let Some(scaled) = needle.resized(width.max(1), height.max(1)) else {
            continue;
        };
        if let Some(found) = best_match(haystack, &scaled) {
            if best.is_none_or(|current| found.score > current.score) {
                best = Some(found);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: u32, height: u32) -> Gray {
        let pixels = (0..width * height).map(|i| ((i * 7) % 251) as u8).collect();
        Gray::new(width, height, pixels).expect("valid")
    }

    fn paste(canvas: &mut Gray, patch: &Gray, left: u32, top: u32) {
        for y in 0..patch.height {
            for x in 0..patch.width {
                let index = ((top + y) as usize) * (canvas.width as usize) + (left + x) as usize;
                canvas.pixels[index] = patch.at(x, y);
            }
        }
    }

    #[test]
    fn a_template_is_found_where_it_was_pasted() {
        let mut canvas = gradient(40, 30);
        let patch = gradient(6, 5);
        paste(&mut canvas, &patch, 11, 7);

        let found = best_match(&canvas, &patch).expect("a match exists");

        assert_eq!((found.x, found.y), (11, 7));
        assert!(found.score > 0.99, "score was {}", found.score);
    }

    #[test]
    fn correlation_survives_a_uniform_brightness_change() {
        let mut canvas = gradient(40, 30);
        let patch = gradient(6, 5);
        let brighter = Gray::new(
            6,
            5,
            patch.pixels.iter().map(|&p| p.saturating_add(40)).collect(),
        )
        .expect("valid");
        paste(&mut canvas, &brighter, 11, 7);

        let found = best_match(&canvas, &patch).expect("a match exists");

        assert_eq!((found.x, found.y), (11, 7));
        assert!(
            found.score > 0.99,
            "normalisation must absorb a brightness shift, score was {}",
            found.score
        );
    }

    #[test]
    fn a_template_larger_than_the_haystack_is_refused() {
        let canvas = gradient(4, 4);
        let patch = gradient(5, 5);

        assert!(best_match(&canvas, &patch).is_none());
    }

    #[test]
    fn a_flat_template_has_no_correlation_to_offer() {
        let canvas = gradient(20, 20);
        let flat = Gray::new(4, 4, vec![128; 16]).expect("valid");

        assert!(
            best_match(&canvas, &flat).is_none(),
            "a template with no variance cannot be located; that is what a colour probe is for"
        );
    }

    #[test]
    fn a_resized_template_is_still_found_across_scales() {
        let mut canvas = gradient(60, 50);
        let patch = gradient(8, 6);
        let stretched = patch.resized(10, 8).expect("valid");
        paste(&mut canvas, &stretched, 20, 15);

        let single = best_match(&canvas, &patch).expect("a best position always exists");
        let multi = best_match_multi_scale(&canvas, &patch, &SCALES).expect("a match exists");

        assert!(
            multi.score > single.score,
            "trying scales must beat the authored size: {} vs {}",
            multi.score,
            single.score
        );
        assert_eq!((multi.x, multi.y), (20, 15));
    }
}
