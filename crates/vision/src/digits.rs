// SPDX-License-Identifier: MPL-2.0
//! Reading numerals from a fixed game font (#32).
//!
//! This is not OCR and does not pretend to be. Idle-game readouts are numerals
//! drawn in one font on a flat background, which is the case template matching
//! already handles: a plugin ships a crop of each glyph and the same normalised
//! cross-correlation that finds a button finds a `7`. ADR-0006 records why that
//! trade beat shipping an ML runtime.
//!
//! The failure mode worth designing against is a *plausible* wrong number, so
//! the confidence returned is the weakest glyph in the reading rather than an
//! average: one badly-matched digit is enough to make the whole number wrong,
//! and averaging would hide it behind its neighbours.

use crate::gray::Gray;
use crate::ncc::{matches_above, Found, SCALES};

/// What a numeric region says, and how sure the weakest glyph in it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub text: String,
    pub confidence: f64,
}

/// Two glyphs may not claim the same place. Above this share of the narrower
/// one's width, the lower-scoring match is dropped.
const MAX_OVERLAP: f64 = 0.4;

/// How far off the line a glyph may sit, as a share of the line's height.
///
/// Every numeral in one readout shares a baseline and a size. Without this, a
/// glyph-sized patch of background elsewhere in the region joins the number and
/// `747` reads as `7477`: a wrong number, at full confidence, which is the
/// outcome this whole module exists to avoid.
const MAX_BASELINE_DRIFT: f64 = 0.34;

struct Candidate {
    glyph: char,
    found: Found,
}

/// Reads the glyphs present in `haystack`, left to right.
///
/// A readout is drawn at one size, so each scale in [`SCALES`] is read on its
/// own and the fullest reading wins. Letting matches from different scales
/// compete directly does not work: they tie at the top, and an accidental
/// two-pixel match at the wrong scale then decides the whole line.
///
/// Reading every scale is also what makes a resized window degrade the reading
/// instead of breaking it.
///
/// A region where nothing matches reads as empty with zero confidence. It never
/// guesses.
pub fn read(haystack: &Gray, glyphs: &[(char, Gray)], min_score: f64) -> Reading {
    SCALES
        .iter()
        .map(|&scale| read_at(haystack, glyphs, min_score, scale))
        .max_by(|a, b| {
            a.text
                .len()
                .cmp(&b.text.len())
                .then_with(|| a.confidence.total_cmp(&b.confidence))
        })
        .unwrap_or(Reading {
            text: String::new(),
            confidence: 0.0,
        })
}

fn read_at(haystack: &Gray, glyphs: &[(char, Gray)], min_score: f64, scale: f64) -> Reading {
    let empty = Reading {
        text: String::new(),
        confidence: 0.0,
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    for (glyph, template) in glyphs {
        let width = ((template.width as f64) * scale).round().max(1.0) as u32;
        let height = ((template.height as f64) * scale).round().max(1.0) as u32;
        let Some(scaled) = template.resized(width, height) else {
            continue;
        };
        for found in matches_above(haystack, &scaled, min_score) {
            candidates.push(Candidate {
                glyph: *glyph,
                found,
            });
        }
    }

    candidates.sort_by(|a, b| b.found.score.total_cmp(&a.found.score));

    let Some(line) = candidates.first().map(|best| best.found) else {
        return empty;
    };

    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in candidates {
        if !on_the_same_line(&line, &candidate.found) {
            continue;
        }
        if kept
            .iter()
            .any(|existing| overlaps(&existing.found, &candidate.found))
        {
            continue;
        }
        kept.push(candidate);
    }

    if kept.is_empty() {
        return empty;
    }
    kept.sort_by_key(|candidate| candidate.found.x);

    Reading {
        confidence: kept
            .iter()
            .map(|candidate| candidate.found.score)
            .fold(f64::INFINITY, f64::min),
        text: kept.iter().map(|candidate| candidate.glyph).collect(),
    }
}

/// The strongest match defines the line. A glyph sitting above or below it
/// belongs to something else, not to this number.
fn on_the_same_line(line: &Found, other: &Found) -> bool {
    if line.height != other.height {
        return false;
    }
    let drift = (line.y as f64 - other.y as f64).abs();
    drift <= line.height as f64 * MAX_BASELINE_DRIFT
}

fn overlaps(a: &Found, b: &Found) -> bool {
    let left = a.x.max(b.x);
    let right = (a.x + a.width).min(b.x + b.width);
    if right <= left {
        return false;
    }
    let shared = (right - left) as f64;
    let narrower = a.width.min(b.width) as f64;
    shared / narrower > MAX_OVERLAP
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A glyph is a 3x5 block of ink on a light ground, distinct enough that a
    /// wrong one cannot score as well as the right one.
    fn glyph(rows: [&str; 5]) -> Gray {
        let mut pixels = Vec::with_capacity(15);
        for row in rows {
            for cell in row.chars() {
                pixels.push(if cell == '#' { 20 } else { 235 });
            }
        }
        Gray::new(3, 5, pixels).expect("a 3x5 glyph")
    }

    fn one() -> Gray {
        glyph([".#.", "##.", ".#.", ".#.", "###"])
    }

    fn two() -> Gray {
        glyph(["###", "..#", "###", "#..", "###"])
    }

    fn dot() -> Gray {
        glyph(["...", "...", "...", "...", ".#."])
    }

    fn alphabet() -> Vec<(char, Gray)> {
        vec![('1', one()), ('2', two()), ('.', dot())]
    }

    /// Lays glyphs out left to right with one blank column between them, on a
    /// light ground, the way a readout is drawn.
    fn line(glyphs: &[&Gray]) -> Gray {
        let height = 5;
        let width = glyphs.iter().map(|g| g.width + 1).sum::<u32>() + 1;
        let mut pixels = vec![235u8; (width * height) as usize];

        let mut left = 1;
        for glyph in glyphs {
            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    pixels[(y * width + left + x) as usize] = glyph.at(x, y);
                }
            }
            left += glyph.width + 1;
        }
        Gray::new(width, height, pixels).expect("a readout line")
    }

    fn degrade(gray: &Gray) -> Gray {
        let pixels = gray
            .pixels
            .iter()
            .enumerate()
            .map(|(index, &pixel)| {
                let noise = ((index * 71) % 120) as i32 - 60;
                (pixel as i32 + noise).clamp(0, 255) as u8
            })
            .collect();
        Gray::new(gray.width, gray.height, pixels).expect("same size")
    }

    #[test]
    fn a_readout_is_read_left_to_right() {
        let haystack = line(&[&two(), &one(), &two()]);

        let reading = read(&haystack, &alphabet(), 0.8);

        assert_eq!(reading.text, "212");
        assert!(
            reading.confidence > 0.9,
            "an undegraded readout should be near-certain, was {}",
            reading.confidence
        );
    }

    #[test]
    fn a_decimal_separator_is_a_glyph_like_any_other() {
        let haystack = line(&[&one(), &dot(), &two()]);

        let reading = read(&haystack, &alphabet(), 0.8);

        assert_eq!(
            reading.text, "1.2",
            "dropping the separator would turn 1.2 into 12, which is the worst \
             outcome available to a numeric readout"
        );
    }

    #[test]
    fn a_region_with_nothing_in_it_reads_as_empty_rather_than_guessing() {
        let blank = Gray::new(20, 5, vec![235; 100]).expect("a blank strip");

        let reading = read(&blank, &alphabet(), 0.8);

        assert_eq!(reading.text, "");
        assert_eq!(reading.confidence, 0.0);
    }

    #[test]
    fn confidence_falls_when_the_region_is_degraded() {
        let clean = line(&[&two(), &one()]);
        let noisy = degrade(&clean);

        let sharp = read(&clean, &alphabet(), 0.5);
        let blurred = read(&noisy, &alphabet(), 0.5);

        assert!(
            blurred.confidence < sharp.confidence,
            "a degraded readout must be less trusted, not equally trusted: \
             {} against {}",
            blurred.confidence,
            sharp.confidence
        );
    }

    #[test]
    fn the_confidence_is_the_weakest_glyph_not_the_average() {
        let clean = line(&[&two(), &one(), &two()]);
        let mut scuffed = clean.clone();
        for y in 0..scuffed.height {
            let index = (y * scuffed.width + 2) as usize;
            scuffed.pixels[index] = 128;
        }

        let reading = read(&scuffed, &alphabet(), 0.5);
        let scores_of_each: Vec<f64> = alphabet()
            .iter()
            .flat_map(|(_, template)| matches_above(&scuffed, template, 0.5))
            .map(|found| found.score)
            .collect();
        let best = scores_of_each.iter().cloned().fold(0.0, f64::max);

        assert!(
            reading.confidence <= best,
            "one damaged glyph must not be averaged away by its neighbours"
        );
    }

    #[test]
    fn background_beside_the_line_does_not_join_the_number() {
        let readout = line(&[&two(), &one(), &two()]);
        let mut wider = vec![235u8; ((readout.width + 12) * 5) as usize];
        let width = readout.width + 12;

        for y in 0..5 {
            for x in 0..readout.width {
                wider[(y * width + x) as usize] = readout.at(x, y);
            }
            // A patch of textured background to the right of the readout, the
            // sort of thing a game draws next to a counter.
            for x in readout.width..width {
                wider[(y * width + x) as usize] = (((x * 37 + y * 11) % 200) + 30) as u8;
            }
        }

        let reading = read(
            &Gray::new(width, 5, wider).expect("valid"),
            &alphabet(),
            0.8,
        );

        assert_eq!(
            reading.text, "212",
            "a glyph-sized patch of background must not become a digit"
        );
    }

    #[test]
    fn a_raised_threshold_rejects_rather_than_returning_a_worse_guess() {
        let noisy = degrade(&line(&[&two(), &one()]));

        let strict = read(&noisy, &alphabet(), 0.99);

        assert_eq!(strict.text, "");
        assert_eq!(strict.confidence, 0.0);
    }
}
