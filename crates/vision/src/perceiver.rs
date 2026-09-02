// SPDX-License-Identifier: MPL-2.0
use std::collections::HashMap;

use idlewarden_capture::Frame;
use idlewarden_plugin_api::{Confidence, Value};

use crate::gray::Gray;
use crate::ncc::{best_match_multi_scale, SCALES};
use crate::probe::colour_fraction;
use crate::{Anchor, Extracted, Extractor, Perceiver, Roi, SignalRule, VisionError};

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
            .ok_or_else(|| VisionError::AnchorLost(name.to_owned()))
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
            Extractor::Ocr { .. } => {
                return Err(VisionError::Ocr(format!(
                    "signal `{}` needs OCR, which is not implemented",
                    rule.id.0
                )))
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
            .ok_or_else(|| VisionError::Ocr("frame buffer does not match its size".to_owned()))?;
        let offset = self.offset(&gray)?;

        self.rules
            .iter()
            .map(|rule| self.extract(rule, frame, &gray, offset))
            .collect()
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
    fn an_ocr_rule_says_so_rather_than_inventing_a_number() {
        let mut rules = RuleSet::new(
            Vec::new(),
            vec![SignalRule {
                id: SignalId("gold".to_owned()),
                extractor: Extractor::Ocr {
                    roi: Roi {
                        x: 0.1,
                        y: 0.1,
                        w: 0.2,
                        h: 0.1,
                    },
                },
            }],
            HashMap::new(),
        );

        let error = rules
            .perceive(&frame(blank()))
            .expect_err("ocr is not implemented");

        assert!(matches!(error, VisionError::Ocr(message) if message.contains("gold")));
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
