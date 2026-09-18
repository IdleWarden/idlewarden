// SPDX-License-Identifier: MPL-2.0
use idlewarden_capture::Frame;
use idlewarden_plugin_api::SignalId;
use idlewarden_vision::{Anchor, Extractor, Roi, SignalRule};

use super::{AuthoringError, Region, RegionKind};

const MIN_SCORE: f64 = 0.85;
const PROBE_TOLERANCE: u8 = 24;
const SEARCH_MARGIN: f64 = 0.5;

pub(super) struct Asset {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Default)]
pub(super) struct Built {
    pub anchors: Vec<Anchor>,
    pub signals: Vec<SignalRule>,
    pub assets: Vec<Asset>,
}

pub(super) fn build(regions: &[Region], frame: &Frame) -> Result<Built, AuthoringError> {
    let mut built = Built::default();

    for region in regions {
        match region.kind {
            RegionKind::Anchor => {
                let asset = crop(region, frame)?;
                built.anchors.push(Anchor {
                    name: region.name.clone(),
                    search_area: widened(region.area),
                    template: asset.path.clone(),
                    min_score: MIN_SCORE,
                });
                built.assets.push(asset);
            }
            RegionKind::TemplateMatch => {
                let asset = crop(region, frame)?;
                built.signals.push(SignalRule {
                    id: SignalId(region.name.clone()),
                    extractor: Extractor::TemplateMatch {
                        roi: widened(region.area),
                        template: asset.path.clone(),
                        min_score: MIN_SCORE,
                    },
                });
                built.assets.push(asset);
            }
            RegionKind::ColorProbe => {
                built.signals.push(SignalRule {
                    id: SignalId(region.name.clone()),
                    extractor: Extractor::ColorProbe {
                        roi: region.area,
                        rgb: mean_colour(region, frame)?,
                        tolerance: PROBE_TOLERANCE,
                    },
                });
            }
        }
    }
    Ok(built)
}

pub(super) fn widened(area: Roi) -> Roi {
    let dx = (area.w * SEARCH_MARGIN)
        .min(area.x)
        .min(1.0 - area.x - area.w)
        .max(0.0);
    let dy = (area.h * SEARCH_MARGIN)
        .min(area.y)
        .min(1.0 - area.y - area.h)
        .max(0.0);
    Roi {
        x: area.x - dx,
        y: area.y - dy,
        w: area.w + 2.0 * dx,
        h: area.h + 2.0 * dy,
    }
}

fn pixels(region: &Region, frame: &Frame) -> Result<(u32, u32, u32, u32), AuthoringError> {
    let (width, height) = (frame.size.width, frame.size.height);
    let left = (region.area.x * width as f64).round() as u32;
    let top = (region.area.y * height as f64).round() as u32;
    let w = (region.area.w * width as f64).round() as u32;
    let h = (region.area.h * height as f64).round() as u32;
    if w == 0 || h == 0 || left + w > width || top + h > height {
        return Err(AuthoringError::OutOfFrame(region.name.clone()));
    }
    Ok((left, top, w, h))
}

fn crop(region: &Region, frame: &Frame) -> Result<Asset, AuthoringError> {
    let (left, top, w, h) = pixels(region, frame)?;
    let stride = frame.size.width as usize * 4;

    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for y in top..top + h {
        let start = y as usize * stride + left as usize * 4;
        bgra.extend_from_slice(&frame.bgra[start..start + w as usize * 4]);
    }

    Ok(Asset {
        path: format!("assets/{}.png", region.name),
        width: w,
        height: h,
        bgra,
    })
}

fn mean_colour(region: &Region, frame: &Frame) -> Result<[u8; 3], AuthoringError> {
    let (left, top, w, h) = pixels(region, frame)?;
    let stride = frame.size.width as usize * 4;

    let mut sum = [0u64; 3];
    for y in top..top + h {
        for x in left..left + w {
            let index = y as usize * stride + x as usize * 4;
            sum[0] += frame.bgra[index + 2] as u64;
            sum[1] += frame.bgra[index + 1] as u64;
            sum[2] += frame.bgra[index] as u64;
        }
    }

    let count = (w as u64) * (h as u64);
    Ok(sum.map(|channel| (channel / count) as u8))
}
