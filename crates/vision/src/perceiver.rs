// SPDX-License-Identifier: MPL-2.0
use std::collections::HashMap;

use idlewarden_capture::Frame;
use idlewarden_plugin_api::{Confidence, Value};

use crate::digits::{read as read_digits, Reading};
use crate::gray::Gray;
use crate::ncc::{best_match_multi_scale, SCALES};
use crate::probe::colour_fraction;
use crate::{Anchor, Extracted, Extractor, NumericKind, Perceiver, Roi, SignalRule, VisionError};

/// How far registration is allowed to move the layout before we call the anchor
/// misidentified rather than displaced.
const MAX_OFFSET: f64 = 0.25;

/// A plugin's declared anchors and rules, applied to every frame.
pub struct RuleSet {
    anchors: Vec<Anchor>,
    rules: Vec<SignalRule>,
    templates: HashMap<String, Gray>,
}

impl RuleSet {
    pub fn new(
        anchors: Vec<Anchor>,
        rules: Vec<SignalRule>,
        templates: HashMap<String, Gray>,
    ) -> Self {
        RuleSet {
            anchors,
            rules,
            templates,
        }
    }

    fn template(&self, name: &str) -> Result<&Gray, VisionError> {
        self.templates
            .get(name)
            .ok_or_else(|| VisionError::Asset(format!("template `{name}` was not loaded")))
    }

    /// Locate every anchor and average how far each has moved. The result
    /// re-registers every region for this frame.
    fn offset(&self, gray: &Gray) -> Result<(f64, f64), VisionError> {
        if self.anchors.is_empty() {
            return Ok((0.0, 0.0));
        }

        let (width, height) = (gray.width as f64, gray.height as f64);
        let mut total = (0.0, 0.0);

        for anchor in &self.anchors {
            let template = self.template(&anchor.template)?;
            let area = pixels(&anchor.search_area, gray.width, gray.height)
                .ok_or(VisionError::RegionOutOfBounds(anchor.search_area))?;
            let haystack = gray
                .crop(area.0, area.1, area.2, area.3)
                .ok_or(VisionError::RegionOutOfBounds(anchor.search_area))?;

            let found = best_match_multi_scale(&haystack, template, &SCALES)
                .filter(|found| found.score >= anchor.min_score)
                .ok_or_else(|| VisionError::AnchorLost(anchor.name.clone()))?;

            let found_centre_x = (area.0 + found.x) as f64 + found.width as f64 / 2.0;
            let found_centre_y = (area.1 + found.y) as f64 + found.height as f64 / 2.0;
            let expected_x = (anchor.search_area.x + anchor.search_area.w / 2.0) * width;
            let expected_y = (anchor.search_area.y + anchor.search_area.h / 2.0) * height;

            total.0 += (found_centre_x - expected_x) / width;
            total.1 += (found_centre_y - expected_y) / height;
        }

        let count = self.anchors.len() as f64;
        let offset = (total.0 / count, total.1 / count);

        if offset.0.abs() > MAX_OFFSET || offset.1.abs() > MAX_OFFSET {
            return Err(VisionError::AnchorLost(self.anchors[0].name.clone()));
        }
        Ok(offset)
    }

    fn extract(
        &self,
        rule: &SignalRule,
        frame: &Frame,
        gray: &Gray,
        offset: (f64, f64),
    ) -> Result<Extracted, VisionError> {
        let (value, confidence) = match &rule.extractor {
            Extractor::TemplateMatch {
                roi,
                template,
                min_score,
            } => {
                let roi = roi.translated(offset.0, offset.1);
                let area = pixels(&roi, gray.width, gray.height)
                    .ok_or(VisionError::RegionOutOfBounds(roi))?;
                let haystack = gray
                    .crop(area.0, area.1, area.2, area.3)
                    .ok_or(VisionError::RegionOutOfBounds(roi))?;
                let score = best_match_multi_scale(&haystack, self.template(template)?, &SCALES)
                    .map(|found| found.score)
                    .unwrap_or(0.0);
                (Value::Bool(score >= *min_score), Confidence::new(score))
            }
            Extractor::ColorProbe {
                roi,
                rgb,
                tolerance,
            } => {
                let roi = roi.translated(offset.0, offset.1);
                let area = pixels(&roi, frame.size.width, frame.size.height)
                    .ok_or(VisionError::RegionOutOfBounds(roi))?;
                let fraction = colour_fraction(
                    &frame.bgra,
                    frame.size.width,
                    frame.size.height,
                    area,
                    *rgb,
                    *tolerance,
                )
                .ok_or(VisionError::RegionOutOfBounds(roi))?;
                (Value::Bool(fraction >= 0.5), Confidence::new(fraction))
            }
            Extractor::Digits {
                roi,
                glyphs,
                min_score,
                value_type,
            } => {
                let roi = roi.translated(offset.0, offset.1);
                let area = pixels(&roi, gray.width, gray.height)
                    .ok_or(VisionError::RegionOutOfBounds(roi))?;
                let haystack = gray
                    .crop(area.0, area.1, area.2, area.3)
                    .ok_or(VisionError::RegionOutOfBounds(roi))?;

                let mut alphabet = Vec::with_capacity(glyphs.len());
                for (character, asset) in glyphs {
                    let glyph = single_char(character)?;
                    alphabet.push((glyph, self.template(asset)?.clone()));
                }

                let reading = read_digits(&haystack, &alphabet, *min_score);
                parse(&reading, *value_type)
            }
        };

        Ok(Extracted {
            id: rule.id.clone(),
            value,
            confidence,
        })
    }
}

impl Perceiver for RuleSet {
    fn perceive(&mut self, frame: &Frame) -> Result<Vec<Extracted>, VisionError> {
        let gray = Gray::from_bgra(frame.size.width, frame.size.height, &frame.bgra)
            .ok_or_else(|| VisionError::Frame("buffer does not match its size".to_owned()))?;
        let offset = self.offset(&gray)?;

        self.rules
            .iter()
            .map(|rule| self.extract(rule, frame, &gray, offset))
            .collect()
    }
}

/// A glyph key names one character. A longer key is an authoring mistake that
/// would otherwise silently match nothing.
fn single_char(key: &str) -> Result<char, VisionError> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(glyph), None) => Ok(glyph),
        _ => Err(VisionError::Asset(format!(
            "glyph key `{key}` is not a single character"
        ))),
    }
}

/// Turns a reading into a value, or into zero confidence.
///
/// A number that does not parse is reported as unreadable rather than as a
/// default: the Governor's confidence floor is what stops the agent, and a
/// plausible zero at full confidence would walk straight past it.
fn parse(reading: &Reading, kind: NumericKind) -> (Value, Confidence) {
    let unreadable = |zero: Value| (zero, Confidence::new(0.0));

    match kind {
        NumericKind::Int => match reading.text.parse::<i64>() {
            Ok(number) => (Value::Int(number), Confidence::new(reading.confidence)),
            Err(_) => unreadable(Value::Int(0)),
        },
        NumericKind::Float => match reading.text.parse::<f64>() {
            Ok(number) if number.is_finite() => {
                (Value::Float(number), Confidence::new(reading.confidence))
            }
            _ => unreadable(Value::Float(0.0)),
        },
        NumericKind::Ratio => match reading.text.parse::<f64>() {
            Ok(number) if (0.0..=1.0).contains(&number) => {
                (Value::Ratio(number), Confidence::new(reading.confidence))
            }
            _ => unreadable(Value::Ratio(0.0)),
        },
    }
}

fn pixels(roi: &Roi, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    if !roi.is_within_unit_square() {
        return None;
    }
    let left = (roi.x * width as f64).round() as u32;
    let top = (roi.y * height as f64).round() as u32;
    let w = (roi.w * width as f64).round() as u32;
    let h = (roi.h * height as f64).round() as u32;
    if w == 0 || h == 0 || left + w > width || top + h > height {
        return None;
    }
    Some((left, top, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_capture::Size;
    use idlewarden_plugin_api::SignalId;

    const W: u32 = 200;
    const H: u32 = 160;

    fn blank() -> Vec<u8> {
        let mut bgra = Vec::with_capacity((W * H * 4) as usize);
        for i in 0..W * H {
            let shade = ((i * 3) % 97) as u8;
            bgra.extend_from_slice(&[shade, shade, shade, 255]);
        }
        bgra
    }

    fn stamp(bgra: &mut [u8], left: u32, top: u32, size: u32, rgb: [u8; 3]) {
        for y in top..top + size {
            for x in left..left + size {
                let index = ((y as usize) * (W as usize) + x as usize) * 4;
                bgra[index] = rgb[2];
                bgra[index + 1] = rgb[1];
                bgra[index + 2] = rgb[0];
            }
        }
    }

    /// A mark with enough internal structure to correlate against.
    fn mark(size: u32) -> Gray {
        let pixels = (0..size * size)
            .map(|i| {
                if (i / size + i % size) % 2 == 0 {
                    250
                } else {
                    10
                }
            })
            .collect();
        Gray::new(size, size, pixels).expect("valid")
    }

    fn stamp_mark(bgra: &mut [u8], left: u32, top: u32, mark: &Gray) {
        for y in 0..mark.height {
            for x in 0..mark.width {
                let shade = mark.at(x, y);
                let index = (((top + y) as usize) * (W as usize) + (left + x) as usize) * 4;
                bgra[index] = shade;
                bgra[index + 1] = shade;
                bgra[index + 2] = shade;
            }
        }
    }

    fn frame(bgra: Vec<u8>) -> Frame {
        Frame {
            id: 1,
            captured_at_ms: 0,
            size: Size {
                width: W,
                height: H,
            },
            bgra,
        }
    }

    fn anchor(x: f64, y: f64) -> Anchor {
        Anchor {
            name: "logo".to_owned(),
            search_area: Roi {
                x,
                y,
                w: 0.4,
                h: 0.4,
            },
            template: "logo".to_owned(),
            min_score: 0.8,
        }
    }

    fn probe_rule(roi: Roi) -> SignalRule {
        SignalRule {
            id: SignalId("ui.button".to_owned()),
            extractor: Extractor::ColorProbe {
                roi,
                rgb: [255, 0, 0],
                tolerance: 4,
            },
        }
    }

    fn templates(mark: Gray) -> HashMap<String, Gray> {
        HashMap::from([("logo".to_owned(), mark)])
    }

    fn digit(rows: [&str; 5]) -> Gray {
        let mut pixels = Vec::with_capacity(15);
        for row in rows {
            for cell in row.chars() {
                pixels.push(if cell == '#' { 20 } else { 235 });
            }
        }
        Gray::new(3, 5, pixels).expect("a 3x5 glyph")
    }

    fn four() -> Gray {
        digit(["#.#", "#.#", "###", "..#", "..#"])
    }

    fn seven() -> Gray {
        digit(["###", "..#", ".#.", ".#.", ".#."])
    }

    /// Paints a light plate at `left, top` and stamps the glyphs onto it with a
    /// one-pixel gap, the way a game draws a readout.
    fn stamp_readout(bgra: &mut [u8], left: u32, top: u32, glyphs: &[&Gray]) {
        let width: u32 = glyphs.iter().map(|g| g.width + 1).sum::<u32>() + 1;
        for y in top..top + 7 {
            for x in left..left + width {
                let index = ((y as usize) * (W as usize) + x as usize) * 4;
                bgra[index] = 235;
                bgra[index + 1] = 235;
                bgra[index + 2] = 235;
            }
        }

        let mut cursor = left + 1;
        for glyph in glyphs {
            stamp_mark(bgra, cursor, top + 1, glyph);
            cursor += glyph.width + 1;
        }
    }

    fn digits_rule(value_type: NumericKind, min_score: f64) -> SignalRule {
        let mut glyphs = std::collections::BTreeMap::new();
        glyphs.insert("4".to_owned(), "four.png".to_owned());
        glyphs.insert("7".to_owned(), "seven.png".to_owned());

        SignalRule {
            id: SignalId("resource.gold".to_owned()),
            extractor: Extractor::Digits {
                roi: Roi {
                    x: 0.0,
                    y: 0.0,
                    w: 0.2,
                    h: 0.1,
                },
                glyphs,
                min_score,
                value_type,
            },
        }
    }

    fn digit_templates() -> HashMap<String, Gray> {
        HashMap::from([
            ("four.png".to_owned(), four()),
            ("seven.png".to_owned(), seven()),
        ])
    }

    #[test]
    fn a_numeric_readout_becomes_an_int_with_the_weakest_glyphs_confidence() {
        let mut bgra = blank();
        stamp_readout(&mut bgra, 2, 2, &[&seven(), &four(), &seven()]);

        let mut rules = RuleSet::new(
            Vec::new(),
            vec![digits_rule(NumericKind::Int, 0.8)],
            digit_templates(),
        );
        let extracted = rules.perceive(&frame(bgra)).expect("no anchors to lose");

        assert_eq!(extracted[0].value, Value::Int(747));
        assert!(
            extracted[0].confidence.get() > 0.9,
            "a clean readout should be near-certain, was {}",
            extracted[0].confidence.get()
        );
    }

    #[test]
    fn a_region_with_no_numerals_reports_no_confidence_rather_than_zero_gold() {
        let mut rules = RuleSet::new(
            Vec::new(),
            vec![digits_rule(NumericKind::Int, 0.8)],
            digit_templates(),
        );

        let extracted = rules.perceive(&frame(blank())).expect("no anchors to lose");

        assert_eq!(extracted[0].value, Value::Int(0));
        assert_eq!(
            extracted[0].confidence.get(),
            0.0,
            "a confident zero would walk straight past the Governor's floor"
        );
    }

    #[test]
    fn a_reading_outside_zero_to_one_is_not_a_ratio() {
        let mut bgra = blank();
        stamp_readout(&mut bgra, 2, 2, &[&seven(), &four()]);

        let mut rules = RuleSet::new(
            Vec::new(),
            vec![digits_rule(NumericKind::Ratio, 0.8)],
            digit_templates(),
        );
        let extracted = rules.perceive(&frame(bgra)).expect("no anchors to lose");

        assert_eq!(
            extracted[0].confidence.get(),
            0.0,
            "74 is not a fraction, and clamping it to 1.0 would be a guess"
        );
    }

    #[test]
    fn a_glyph_the_bundle_did_not_load_is_a_structural_failure_not_a_low_score() {
        let mut rules = RuleSet::new(
            Vec::new(),
            vec![digits_rule(NumericKind::Int, 0.8)],
            HashMap::from([("four.png".to_owned(), four())]),
        );

        let error = rules
            .perceive(&frame(blank()))
            .expect_err("seven.png is missing");

        assert!(matches!(error, VisionError::Asset(message) if message.contains("seven.png")));
    }

    #[test]
    fn without_anchors_a_probe_reads_the_authored_region() {
        let mut bgra = blank();
        stamp(&mut bgra, 100, 80, 20, [255, 0, 0]);
        let mut rules = RuleSet::new(
            Vec::new(),
            vec![probe_rule(Roi {
                x: 0.5,
                y: 0.5,
                w: 0.1,
                h: 0.125,
            })],
            HashMap::new(),
        );

        let extracted = rules.perceive(&frame(bgra)).expect("no anchors to lose");

        assert_eq!(extracted[0].value, Value::Bool(true));
        assert_eq!(extracted[0].confidence.get(), 1.0);
    }

    #[test]
    fn a_displaced_layout_is_re_registered_by_its_anchor() {
        let logo = mark(16);
        let shift = 20u32;

        let mut moved = blank();
        stamp_mark(&mut moved, 52 + shift, 40 + shift, &logo);
        stamp(&mut moved, 100 + shift, 80 + shift, 20, [255, 0, 0]);

        let mut rules = RuleSet::new(
            vec![anchor(0.1, 0.1)],
            vec![probe_rule(Roi {
                x: 0.5,
                y: 0.5,
                w: 0.1,
                h: 0.125,
            })],
            templates(logo),
        );

        let extracted = rules
            .perceive(&frame(moved))
            .expect("the anchor is present");

        assert_eq!(
            extracted[0].value,
            Value::Bool(true),
            "the probe must follow the anchor, not the authored pixels"
        );
        assert!(extracted[0].confidence.get() > 0.9);
    }

    #[test]
    fn the_same_layout_without_the_anchor_reads_the_wrong_place() {
        let logo = mark(16);
        let shift = 20u32;

        let mut moved = blank();
        stamp_mark(&mut moved, 52 + shift, 40 + shift, &logo);
        stamp(&mut moved, 100 + shift, 80 + shift, 20, [255, 0, 0]);

        let mut unanchored = RuleSet::new(
            Vec::new(),
            vec![probe_rule(Roi {
                x: 0.5,
                y: 0.5,
                w: 0.1,
                h: 0.125,
            })],
            HashMap::new(),
        );

        let extracted = unanchored.perceive(&frame(moved)).expect("no anchors");

        assert_eq!(
            extracted[0].value,
            Value::Bool(false),
            "this is the failure anchoring exists to prevent"
        );
    }

    #[test]
    fn a_missing_anchor_is_a_structural_error_not_a_low_confidence() {
        let logo = mark(16);
        let bgra = blank();

        let mut rules = RuleSet::new(
            vec![anchor(0.1, 0.1)],
            vec![probe_rule(Roi {
                x: 0.5,
                y: 0.5,
                w: 0.1,
                h: 0.125,
            })],
            templates(logo),
        );

        let error = rules
            .perceive(&frame(bgra))
            .expect_err("the anchor is absent");

        assert!(matches!(error, VisionError::AnchorLost(name) if name == "logo"));
    }

    #[test]
    fn confidence_falls_as_the_probed_region_degrades() {
        let roi = Roi {
            x: 0.5,
            y: 0.5,
            w: 0.1,
            h: 0.125,
        };

        let mut full = blank();
        stamp(&mut full, 100, 80, 20, [255, 0, 0]);
        let mut half = blank();
        stamp(&mut half, 100, 80, 20, [255, 0, 0]);
        stamp(&mut half, 100, 80, 20 / 2 + 4, [0, 0, 255]);

        let mut rules = RuleSet::new(Vec::new(), vec![probe_rule(roi)], HashMap::new());
        let strong = rules.perceive(&frame(full)).expect("valid")[0]
            .confidence
            .get();
        let weak = rules.perceive(&frame(half)).expect("valid")[0]
            .confidence
            .get();

        assert!(
            weak < strong,
            "a partly covered button must read less confidently: {weak} vs {strong}"
        );
    }

    #[test]
    fn a_region_outside_the_window_is_refused() {
        let mut rules = RuleSet::new(
            Vec::new(),
            vec![probe_rule(Roi {
                x: 0.9,
                y: 0.9,
                w: 0.3,
                h: 0.3,
            })],
            HashMap::new(),
        );

        let error = rules
            .perceive(&frame(blank()))
            .expect_err("the roi leaves the window");

        assert!(matches!(error, VisionError::RegionOutOfBounds(_)));
    }
}
